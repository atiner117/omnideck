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
//
// Lifecycle: `watch()` never exits. If the session bus dies (dbus restart, logout while the
// launcher stays alive) the streams end; the watcher clears the shared state (card clears),
// unpublishes the connection, and reconnects with bounded backoff (1 s → 5 s → 15 s).
use futures_util::future::{self, Either};
use futures_util::StreamExt;
use serde::Serialize;
use std::collections::HashMap;
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
// Re-settable (not OnceLock): after a session-bus restart the watcher publishes the *new*
// connection here, so `control()` acts on a live bus instead of failing on a dead one forever.
// None = no usable bus right now. Guard is never held across an .await — clone the Connection
// out (zbus Connections are cheap Arc-style handles) and drop the lock first.
static CONN: Mutex<Option<zbus::Connection>> = Mutex::new(None);

fn state() -> &'static Mutex<HashMap<String, PlayerState>> {
    STATE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn current_conn() -> Option<zbus::Connection> {
    crate::sync::lock_or_recover(&CONN, "mpris.conn").clone()
}

fn set_conn(conn: Option<zbus::Connection>) {
    *crate::sync::lock_or_recover(&CONN, "mpris.conn") = conn;
}

const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";
const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";

#[zbus::proxy(
    interface = "org.mpris.MediaPlayer2.Player",
    default_path = "/org/mpris/MediaPlayer2"
)]
trait Player {
    fn play_pause(&self) -> zbus::Result<()>;
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
    let conn = current_conn().ok_or("no D-Bus session bus (MPRIS unavailable)")?;
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
    match action {
        "play-pause" => player.play_pause().await,
        "next" => player.next().await,
        "previous" => player.previous().await,
        _ => return Err(format!("unknown media action: {action}")),
    }
    .map_err(|e| e.to_string())
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

/// Reconnect backoff between watch cycles: 1 s → 5 s → 15 s, capped. Always sleeps at least
/// the first step so a persistently-broken bus can never turn the watcher into a hot loop.
const RECONNECT_BACKOFF_SECS: [u64; 3] = [1, 5, 15];

/// A cycle that survives this long before its streams end counts as "was actually working",
/// so the next reconnect starts the backoff ladder over instead of escalating.
const BACKOFF_RESET_AFTER: Duration = Duration::from_secs(60);

/// Long-running watcher; spawned once at app setup and never exits. Each cycle connects to
/// the session bus, subscribes, and pumps signals until the streams end (dbus restart,
/// session-bus crash, logout while the launcher stays alive). It then clears the shared
/// state — so the Now Playing card clears instead of showing the last song forever — drops
/// the published connection, and reconnects with bounded backoff.
pub async fn watch(app: tauri::AppHandle) {
    let mut failures: usize = 0;
    loop {
        let started = Instant::now();
        match watch_cycle(&app).await {
            // Streams ended after a working subscription — the bus went away under us.
            Ok(()) => tracing::warn!("mpris: session bus connection lost — reconnecting"),
            // Couldn't even get subscribed (no bus yet / bus still restarting). Warn once,
            // then demote to debug so a genuinely bus-less environment doesn't spam the log.
            Err(e) if failures == 0 => tracing::warn!("mpris: {e} — will retry"),
            Err(e) => tracing::debug!("mpris: {e} — will retry"),
        }
        // Tear down: a dead bus means dead state. Clear the players (emits `media-changed:
        // None`, clearing the card) and unpublish the connection so `control()` reports
        // "unavailable" instead of erroring against a stale bus.
        set_conn(None);
        crate::sync::lock_or_recover(state(), "mpris.players").clear();
        emit_current(&app);

        if started.elapsed() >= BACKOFF_RESET_AFTER {
            failures = 0;
        }
        let delay = RECONNECT_BACKOFF_SECS[failures.min(RECONNECT_BACKOFF_SECS.len() - 1)];
        failures = failures.saturating_add(1);
        tokio::time::sleep(Duration::from_secs(delay)).await;
    }
}

/// One connect→subscribe→pump cycle. `Err` = setup failed before subscribing; `Ok(())` = the
/// subscription worked and ran until a signal stream ended (bus gone → caller reconnects).
async fn watch_cycle(app: &tauri::AppHandle) -> Result<(), String> {
    let conn = zbus::Connection::session()
        .await
        .map_err(|e| format!("no session bus ({e})"))?;
    let dbus = zbus::fdo::DBusProxy::new(&conn)
        .await
        .map_err(|e| format!("DBus proxy failed ({e})"))?;

    // Signal streams FIRST, snapshot second, so a player appearing in between isn't missed.
    let mut owner_changes = dbus
        .receive_name_owner_changed()
        .await
        .map_err(|e| format!("NameOwnerChanged subscribe failed ({e})"))?;
    let props_rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.DBus.Properties")
        .and_then(|b| b.member("PropertiesChanged"))
        .and_then(|b| b.path(MPRIS_PATH))
        .map(|b| b.build())
        .map_err(|e| format!("PropertiesChanged match rule failed ({e})"))?;
    let mut props_stream = zbus::MessageStream::for_match_rule(props_rule, &conn, None)
        .await
        .map_err(|e| format!("PropertiesChanged subscribe failed ({e})"))?;

    // Publish only once subscribed, so `control()` never sees a half-set-up connection.
    set_conn(Some(conn.clone()));

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

    // Pump both signal streams from this one task. (This used to be a detached spawn for
    // owner_changes + an inline loop for props; folded together so nothing is orphaned when
    // a cycle ends and the caller reconnects.) Either stream ending means the bus is gone.
    loop {
        match future::select(owner_changes.next(), props_stream.next()).await {
            Either::Left((Some(sig), _)) => on_owner_changed(&conn, app, sig).await,
            Either::Right((Some(msg), _)) => on_properties_changed(&conn, app, msg).await,
            Either::Left((None, _)) | Either::Right((None, _)) => return Ok(()),
        }
    }
}

/// Player appear/disappear (`NameOwnerChanged` for an `org.mpris.MediaPlayer2.*` name).
async fn on_owner_changed(
    conn: &zbus::Connection,
    app: &tauri::AppHandle,
    sig: zbus::fdo::NameOwnerChanged,
) {
    let Ok(args) = sig.args() else { return };
    let name = args.name().to_string();
    if !name.starts_with(MPRIS_PREFIX) {
        return;
    }
    match args.new_owner().as_ref() {
        Some(owner) => {
            // appeared (or changed owner): fetch initial state
            let owner = owner.to_string();
            if let Some(p) = fetch_player(conn, &name, owner).await {
                crate::sync::lock_or_recover(state(), "mpris.players").insert(name, p);
            }
        }
        None => {
            // player closed (possibly mid-song): drop it so the card clears
            crate::sync::lock_or_recover(state(), "mpris.players").remove(&name);
        }
    }
    emit_current(app);
}

/// Property change from any player, matched back via the sender's unique name.
async fn on_properties_changed(
    conn: &zbus::Connection,
    app: &tauri::AppHandle,
    msg: zbus::Result<zbus::Message>,
) {
    let Ok(msg) = msg else { return };
    let Some(sender) = msg.header().sender().map(|s| s.to_string()) else { return };
    let body = msg.body();
    let Ok((iface, changed, invalidated)) =
        body.deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
    else {
        return;
    };
    if iface != "org.mpris.MediaPlayer2.Player" {
        return;
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
        let owner = sender.clone();
        if let Some(p) = fetch_player(conn, &name, owner).await {
            crate::sync::lock_or_recover(state(), "mpris.players").insert(name, p);
        }
    }
    emit_current(app);
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
