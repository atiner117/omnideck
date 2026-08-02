// OmniDeck — event-driven MPRIS Now Playing (zbus).
//
// Replaces the old `playerctl` integration: the frontend used to fork+exec `playerctl` every
// 4 s (plus a 250 ms re-poll after each media key), so track changes took up to 4 s to show
// and an always-on launcher paid a process spawn per tick. Here one watcher task holds a
// session-bus connection and pushes `media-changed` events the moment a player's
// PropertiesChanged signal fires — updates in milliseconds, zero polling, and the `playerctl`
// runtime dependency is gone. Works for native players (Spotify, Feishin) and browser PWAs
// (YouTube Music in Chromium/Brave) since browsers expose MPRIS too.
//
// Design (see NOTES-RESEARCH §3): a single task, two signal streams —
//   * `NameOwnerChanged` tracks `org.mpris.MediaPlayer2.*` names appearing/disappearing
//     (never hardcode player names: browsers register PID-embedded ones per window), and
//   * one bus-wide match on `PropertiesChanged` at `/org/mpris/MediaPlayer2` (matched back to
//     a player via its unique owner name), so there are no per-player tasks to cancel.
// Shared state maps well-known name -> PlayerState; "the" player is the most recently active
// Playing one, else the most recently active. `media_now_playing` reads the same state (no
// I/O), covering the frontend's initial fetch before its event listener attaches.
use futures_util::StreamExt;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::Emitter;
use zbus::zvariant::OwnedValue;

#[derive(Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct MediaInfo {
    pub status: String, // "Playing" | "Paused" | "Stopped"
    pub title: String,
    pub artist: String,
    pub player: String,
}

struct PlayerState {
    owner: String,    // unique bus name (":1.42") — PropertiesChanged senders are matched on this
    identity: String, // human name ("Spotify"); falls back to the bus-name suffix
    status: String,
    title: String,
    artist: String,
    last_change: Instant, // recency decides which player the Now Playing card shows
}

static STATE: OnceLock<Mutex<HashMap<String, PlayerState>>> = OnceLock::new();
/// The live session-bus connection, swapped on every (re)connect and set back to `None` while
/// disconnected so `control()` reports "temporarily unavailable" instead of acting on a dead
/// handle. (Was a `OnceLock` — unreplaceable, so a single dropped bus wedged Now Playing for
/// the life of the process.)
static CONN: Mutex<Option<zbus::Connection>> = Mutex::new(None);

fn state() -> &'static Mutex<HashMap<String, PlayerState>> {
    STATE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A clone of the current session-bus connection, or None while (re)connecting.
fn current_conn() -> Option<zbus::Connection> {
    crate::sync::lock_or_recover(&CONN, "mpris.conn").clone()
}

const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";
const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";

#[zbus::proxy(
    interface = "org.mpris.MediaPlayer2.Player",
    default_path = "/org/mpris/MediaPlayer2"
)]
trait Player {
    fn play_pause(&self) -> zbus::Result<()>;
    fn pause(&self) -> zbus::Result<()>;
    fn next(&self) -> zbus::Result<()>;
    fn previous(&self) -> zbus::Result<()>;
    #[zbus(property)]
    fn playback_status(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn metadata(&self) -> zbus::Result<HashMap<String, OwnedValue>>;
}

#[zbus::proxy(
    interface = "org.mpris.MediaPlayer2",
    default_path = "/org/mpris/MediaPlayer2"
)]
trait MediaPlayer2 {
    #[zbus(property)]
    fn identity(&self) -> zbus::Result<String>;
}

/// `xesam:title` / first `xesam:artist` out of an MPRIS Metadata dict (`a{sv}`; artist is `as`).
fn title_artist(md: &HashMap<String, OwnedValue>) -> (String, String) {
    let title = md
        .get("xesam:title")
        .and_then(|v| <&str>::try_from(v).ok())
        .unwrap_or_default()
        .to_string();
    let artist = md
        .get("xesam:artist")
        .and_then(|v| <Vec<String>>::try_from(v.clone()).ok())
        .and_then(|a| a.into_iter().next())
        .unwrap_or_default();
    (title, artist)
}

/// The player the Now Playing card should show: most recently active among Playing ones,
/// else most recently active overall. None when there's no player or it has no metadata
/// (matches the old playerctl behavior of returning nothing for a title-less player).
fn best_info(players: &HashMap<String, PlayerState>) -> Option<MediaInfo> {
    let p = players
        .values()
        .max_by_key(|p| (p.status == "Playing", p.last_change))?;
    if p.title.is_empty() && p.artist.is_empty() {
        return None;
    }
    Some(MediaInfo {
        status: p.status.clone(),
        title: p.title.clone(),
        artist: p.artist.clone(),
        player: p.identity.clone(),
    })
}

/// True while any MPRIS player reports `Playing` — the screensaver idle detector
/// (gamepad.rs) uses this to suppress `idle` during playback (roadmap: the dim/blank must
/// never trigger mid-movie). Unlike `now_playing()` this counts a title-less player too:
/// something is audibly/visibly playing even without metadata.
pub fn any_playing() -> bool {
    crate::sync::lock_or_recover(state(), "mpris.players")
        .values()
        .any(|p| p.status == "Playing")
}

/// Snapshot for the `media_now_playing` command (frontend's initial fetch).
pub fn now_playing() -> Option<MediaInfo> {
    let players = crate::sync::lock_or_recover(state(), "mpris.players");
    best_info(&players)
}

fn emit_current(app: &tauri::AppHandle) {
    let info = best_info(&crate::sync::lock_or_recover(state(), "mpris.players"));
    let _ = app.emit("media-changed", info);
}

/// Control the tracked player. Errs when no session bus / no player — the UI toasts it.
pub async fn control(action: &str) -> Result<(), String> {
    // ONE list owns both validation and dispatch: parse the verb up front (fail fast,
    // before any D-Bus work), then match the enum exhaustively below. Keeping two string
    // lists in sync was a footgun — a verb added to the guard but not the dispatch would
    // silently fall into the catch-all and fire the wrong control.
    enum Verb {
        PlayPause,
        Next,
        Previous,
    }
    let verb = match action {
        "play-pause" => Verb::PlayPause,
        "next" => Verb::Next,
        "previous" => Verb::Previous,
        _ => return Err(format!("unknown media action: {action}")),
    };
    let conn = current_conn().ok_or("media controls temporarily unavailable (reconnecting to D-Bus)")?;
    let name = crate::sync::lock_or_recover(state(), "mpris.players")
        .iter()
        .max_by_key(|(_, p)| (p.status == "Playing", p.last_change))
        .map(|(name, _)| name.clone())
        .ok_or("no media player is running")?;
    let player = PlayerProxy::builder(&conn)
        .destination(name)
        .map_err(|e| e.to_string())?
        .build()
        .await
        .map_err(|e| e.to_string())?;
    let call = async {
        match verb {
            Verb::PlayPause => player.play_pause().await,
            Verb::Next => player.next().await,
            Verb::Previous => player.previous().await,
        }
    };
    // Bounded: zbus sets NO method timeout by default, and a deck-frozen (SIGSTOPped)
    // paused player keeps its bus connection open but never replies — without this, every
    // transport press pended forever (dead button, no toast) and then all fired at once
    // when the app was thawed.
    match tokio::time::timeout(Duration::from_secs(2), call).await {
        Ok(r) => r.map_err(|e| e.to_string()),
        Err(_) => Err("player did not respond (it may be frozen while hidden)".into()),
    }
}

/// Pause every player that is currently Playing (the sleep timer's expiry action). Unlike
/// `control` this fans out to ALL Playing players, not just the tracked one — falling
/// asleep to music while a paused video sits behind it must silence the music, whichever
/// the Now Playing card happens to show. Pause (not Stop, not kill): position is kept, so
/// resuming in the morning is one button. Best-effort per player — one dead proxy mustn't
/// keep the others playing all night. Returns how many players were actually paused.
/// (Players with no MPRIS presence — e.g. mpv without an MPRIS plugin — are not reachable
/// from here; that gap is documented, not papered over with a kill.)
pub async fn pause_all() -> usize {
    // current_conn() is None while the supervised watcher (9e7eb5b) is mid-reconnect —
    // nothing to pause through in that window; the timer's best-effort contract covers it.
    let Some(conn) = current_conn() else { return 0 };
    let playing: Vec<String> = crate::sync::lock_or_recover(state(), "mpris.players")
        .iter()
        .filter(|(_, p)| p.status == "Playing")
        .map(|(name, _)| name.clone())
        .collect();
    let mut paused = 0;
    for name in playing {
        let Ok(builder) = PlayerProxy::builder(&conn).destination(name.clone()) else { continue };
        let Ok(player) = builder.build().await else { continue };
        match player.pause().await {
            Ok(()) => paused += 1,
            Err(e) => tracing::warn!("sleep timer: pausing {name} failed: {e}"),
        }
    }
    paused
}

/// Fetch a player's full state once (on appear / on a Metadata-invalidated signal).
async fn fetch_player(conn: &zbus::Connection, name: &str, owner: String) -> Option<PlayerState> {
    let player = PlayerProxy::builder(conn).destination(name.to_string()).ok()?.build().await.ok()?;
    let root = MediaPlayer2Proxy::builder(conn).destination(name.to_string()).ok()?.build().await.ok()?;
    let status = player.playback_status().await.unwrap_or_else(|_| "Stopped".into());
    let (title, artist) = title_artist(&player.metadata().await.unwrap_or_default());
    let identity = match root.identity().await {
        Ok(id) if !id.is_empty() => id,
        _ => name.trim_start_matches(MPRIS_PREFIX).to_string(),
    };
    Some(PlayerState { owner, identity, status, title, artist, last_change: Instant::now() })
}

/// One-shot snapshot for the `omnideck media` debug CLI: list every MPRIS player on the
/// session bus and what the Now Playing card would show.
pub async fn report() -> String {
    let conn = match zbus::Connection::session().await {
        Ok(c) => c,
        Err(e) => return format!("no D-Bus session bus: {e}\n"),
    };
    let dbus = match zbus::fdo::DBusProxy::new(&conn).await {
        Ok(d) => d,
        Err(e) => return format!("DBus proxy failed: {e}\n"),
    };
    let mut players = HashMap::new();
    if let Ok(names) = dbus.list_names().await {
        for name in names {
            let name = name.to_string();
            if !name.starts_with(MPRIS_PREFIX) {
                continue;
            }
            let Ok(owner_name) = zbus::names::BusName::try_from(name.as_str()) else { continue };
            let owner = match dbus.get_name_owner(owner_name).await {
                Ok(o) => o.to_string(),
                Err(_) => continue,
            };
            if let Some(p) = fetch_player(&conn, &name, owner).await {
                players.insert(name, p);
            }
        }
    }
    let mut s = format!("MPRIS players: {}\n", players.len());
    for (name, p) in &players {
        s.push_str(&format!(
            "  - {} ({name}): {} — {} / {}\n",
            p.identity, p.status, p.title, p.artist
        ));
    }
    match best_info(&players) {
        Some(i) => s.push_str(&format!(
            "Now playing: [{}] {} — {} ({})\n",
            i.status, i.title, i.artist, i.player
        )),
        None => s.push_str("Now playing: (none)\n"),
    }
    s
}

/// Bumped at the start of every `run_session`. A lingering owner-change task from a previous
/// session checks this before touching shared state so it can't write into a newer session.
static SESSION_GEN: AtomicU64 = AtomicU64::new(0);

/// Long-running Now Playing watcher, spawned once at app setup. A supervisor loop: connect,
/// subscribe, snapshot, and stream player changes until the connection dies — then clear the
/// now-stale state, push `None` so the card can't linger, and reconnect with bounded backoff.
/// An always-on living-room launcher outlives dbus-daemon / player / user-session restarts, so
/// the old "subscribe once, exit on stream end" left a frozen Now Playing card forever.
pub async fn watch(app: tauri::AppHandle) {
    let mut backoff = Duration::ZERO;
    let mut logged_down = false;
    loop {
        if !backoff.is_zero() {
            tokio::time::sleep(backoff).await;
        }
        match run_session(&app).await {
            // Clean stream end (e.g. dbus-daemon restart): reconnect promptly, no error noise.
            Ok(()) => {
                if logged_down {
                    tracing::info!("mpris: session bus back — Now Playing reconnected");
                }
                backoff = Duration::from_secs(1);
                logged_down = false;
            }
            Err(e) => {
                // Log the FIRST failure of a down period only, so a persistently-absent bus
                // (headless/CI) doesn't flood the session log every backoff tick.
                if !logged_down {
                    tracing::warn!("mpris: session-bus watcher down ({e}) — retrying with backoff");
                    logged_down = true;
                }
                backoff = match backoff {
                    Duration::ZERO => Duration::from_secs(1),
                    d if d < Duration::from_secs(5) => Duration::from_secs(5),
                    _ => Duration::from_secs(15), // cap
                };
            }
        }
        // The session is over: any straggler task from it must stop touching state NOW,
        // not when the next session happens to start — the gap is the whole backoff window.
        SESSION_GEN.fetch_add(1, Ordering::SeqCst);
        // Connection is gone: drop it (control() now reports "temporarily unavailable"), clear
        // cached players, and push `None` so no stale card survives the gap.
        *crate::sync::lock_or_recover(&CONN, "mpris.conn") = None;
        let had_players = {
            let mut players = crate::sync::lock_or_recover(state(), "mpris.players");
            let had = !players.is_empty();
            players.clear();
            had
        };
        // Emit only when a card could actually be showing: on a host with no session bus at
        // all, an unconditional emit here pushed a null `media-changed` into the webview
        // every backoff tick (15 s) for the life of the process.
        if had_players {
            emit_current(&app);
        }
    }
}

/// One connect→subscribe→snapshot→stream cycle. Returns `Ok(())` when the property stream ends
/// (connection closed), or `Err` if any setup step fails — either way the supervisor clears
/// state and reconnects.
async fn run_session(app: &tauri::AppHandle) -> zbus::Result<()> {
    let my_gen = SESSION_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let conn = zbus::Connection::session().await?;
    *crate::sync::lock_or_recover(&CONN, "mpris.conn") = Some(conn.clone());
    let dbus = zbus::fdo::DBusProxy::new(&conn).await?;

    // Subscribe to BOTH signal streams before the initial snapshot, so a player that appears in
    // the gap is caught by the stream rather than missed.
    let mut owner_changes = dbus.receive_name_owner_changed().await?;
    let props_rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.DBus.Properties")?
        .member("PropertiesChanged")?
        .path(MPRIS_PATH)?
        .build();
    let mut props_stream = zbus::MessageStream::for_match_rule(props_rule, &conn, None).await?;

    // Initial snapshot of already-running players.
    if let Ok(names) = dbus.list_names().await {
        for name in names {
            let name = name.to_string();
            if !name.starts_with(MPRIS_PREFIX) {
                continue;
            }
            let Ok(bus_name) = zbus::names::BusName::try_from(name.as_str()) else { continue };
            let owner = match dbus.get_name_owner(bus_name).await {
                Ok(o) => o.to_string(),
                Err(_) => continue, // vanished between ListNames and now
            };
            if let Some(p) = fetch_player(&conn, &name, owner).await {
                crate::sync::lock_or_recover(state(), "mpris.players").insert(name, p);
            }
        }
    }
    emit_current(app);

    // Player appear/disappear, in a child task that ends on its own when the connection closes
    // (its stream yields None). The generation check stops a task that outlives its session
    // (e.g. a slow fetch racing a reconnect) from writing into the next session's state.
    let conn_a = conn.clone();
    let app_a = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(sig) = owner_changes.next().await {
            if SESSION_GEN.load(Ordering::SeqCst) != my_gen {
                break; // superseded by a newer session
            }
            let Ok(args) = sig.args() else { continue };
            let name = args.name().to_string();
            if !name.starts_with(MPRIS_PREFIX) {
                continue;
            }
            match args.new_owner().as_ref() {
                Some(owner) => {
                    let owner = owner.to_string();
                    if let Some(p) = fetch_player(&conn_a, &name, owner).await {
                        crate::sync::lock_or_recover(state(), "mpris.players").insert(name, p);
                    }
                }
                None => {
                    crate::sync::lock_or_recover(state(), "mpris.players").remove(&name);
                }
            }
            emit_current(&app_a);
        }
    });

    // Property changes from any player, matched back via the sender's unique name. This loop
    // ending means the property stream (and thus the connection) died → return to reconnect.
    while let Some(msg) = props_stream.next().await {
        let Ok(msg) = msg else { continue };
        let Some(sender) = msg.header().sender().map(|s| s.to_string()) else { continue };
        let body = msg.body();
        let Ok((iface, changed, invalidated)) =
            body.deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
        else {
            continue;
        };
        if iface != "org.mpris.MediaPlayer2.Player" {
            continue;
        }
        // Apply the delta under the lock; remember whether a re-fetch is needed.
        let mut refetch: Option<String> = None; // well-known name
        {
            let mut players = crate::sync::lock_or_recover(state(), "mpris.players");
            if let Some((name, p)) = players.iter_mut().find(|(_, p)| p.owner == sender) {
                if let Some(s) = changed.get("PlaybackStatus").and_then(|v| <&str>::try_from(v).ok()) {
                    p.status = s.to_string();
                }
                if let Some(v) = changed.get("Metadata") {
                    if let Ok(md) = HashMap::<String, OwnedValue>::try_from(v.clone()) {
                        (p.title, p.artist) = title_artist(&md);
                    }
                }
                p.last_change = Instant::now();
                if invalidated.iter().any(|i| i == "Metadata" || i == "PlaybackStatus") {
                    refetch = Some(name.clone());
                }
            }
        }
        if let Some(name) = refetch {
            if let Some(p) = fetch_player(&conn, &name, sender.clone()).await {
                crate::sync::lock_or_recover(state(), "mpris.players").insert(name, p);
            }
        }
        emit_current(app);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::Value;

    fn md(title: Option<&str>, artists: &[&str]) -> HashMap<String, OwnedValue> {
        let mut m = HashMap::new();
        if let Some(t) = title {
            m.insert("xesam:title".to_string(), OwnedValue::try_from(Value::from(t)).unwrap());
        }
        if !artists.is_empty() {
            let v: Vec<String> = artists.iter().map(|s| s.to_string()).collect();
            m.insert("xesam:artist".to_string(), OwnedValue::try_from(Value::from(v)).unwrap());
        }
        m
    }

    #[test]
    fn parses_title_and_first_artist() {
        let (t, a) = title_artist(&md(Some("Song"), &["First", "Second"]));
        assert_eq!(t, "Song");
        assert_eq!(a, "First"); // xesam:artist is an array — take the first
        let (t, a) = title_artist(&md(None, &[]));
        assert!(t.is_empty() && a.is_empty());
    }

    #[test]
    fn best_prefers_playing_then_recency() {
        let mut players = HashMap::new();
        let old = Instant::now() - std::time::Duration::from_secs(60);
        players.insert(
            "org.mpris.MediaPlayer2.paused".to_string(),
            PlayerState {
                owner: ":1.1".into(),
                identity: "Paused One".into(),
                status: "Paused".into(),
                title: "Newer But Paused".into(),
                artist: String::new(),
                last_change: Instant::now(),
            },
        );
        players.insert(
            "org.mpris.MediaPlayer2.playing".to_string(),
            PlayerState {
                owner: ":1.2".into(),
                identity: "Playing One".into(),
                status: "Playing".into(),
                title: "Older But Playing".into(),
                artist: String::new(),
                last_change: old,
            },
        );
        assert_eq!(best_info(&players).unwrap().title, "Older But Playing");
    }

    #[test]
    fn empty_metadata_yields_none() {
        let mut players = HashMap::new();
        players.insert(
            "org.mpris.MediaPlayer2.blank".to_string(),
            PlayerState {
                owner: ":1.3".into(),
                identity: "Blank".into(),
                status: "Playing".into(),
                title: String::new(),
                artist: String::new(),
                last_change: Instant::now(),
            },
        );
        assert!(best_info(&players).is_none());
    }
}
