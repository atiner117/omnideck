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
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
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
static CONN: OnceLock<zbus::Connection> = OnceLock::new();

fn state() -> &'static Mutex<HashMap<String, PlayerState>> {
    STATE.get_or_init(|| Mutex::new(HashMap::new()))
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
    let players = state().lock().ok()?;
    best_info(&players)
}

fn emit_current(app: &tauri::AppHandle) {
    let info = state().lock().ok().and_then(|p| best_info(&p));
    let _ = app.emit("media-changed", info);
}

/// Control the tracked player. Errs when no session bus / no player — the UI toasts it.
pub async fn control(action: &str) -> Result<(), String> {
    let conn = CONN.get().ok_or("no D-Bus session bus (MPRIS unavailable)")?;
    let name = state()
        .lock()
        .map_err(|e| e.to_string())?
        .iter()
        .max_by_key(|(_, p)| (p.status == "Playing", p.last_change))
        .map(|(name, _)| name.clone())
        .ok_or("no media player is running")?;
    let player = PlayerProxy::builder(conn)
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

/// Long-running watcher; spawned once at app setup. Exits (with a log line) only if the
/// session bus is unreachable — in that environment `playerctl` wouldn't have worked either.
pub async fn watch(app: tauri::AppHandle) {
    let conn = match zbus::Connection::session().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[omnideck] mpris: no session bus ({e}) — Now Playing disabled");
            return;
        }
    };
    let _ = CONN.set(conn.clone());
    let dbus = match zbus::fdo::DBusProxy::new(&conn).await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[omnideck] mpris: DBus proxy failed ({e}) — Now Playing disabled");
            return;
        }
    };

    // Signal streams FIRST, snapshot second, so a player appearing in between isn't missed.
    let mut owner_changes = match dbus.receive_name_owner_changed().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[omnideck] mpris: NameOwnerChanged subscribe failed ({e})");
            return;
        }
    };
    let props_rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.DBus.Properties")
        .and_then(|b| b.member("PropertiesChanged"))
        .and_then(|b| b.path(MPRIS_PATH))
        .map(|b| b.build());
    let props_stream = match props_rule {
        Ok(rule) => zbus::MessageStream::for_match_rule(rule, &conn, None).await,
        Err(e) => Err(e),
    };
    let mut props_stream = match props_stream {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[omnideck] mpris: PropertiesChanged subscribe failed ({e})");
            return;
        }
    };

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
                if let Ok(mut players) = state().lock() {
                    players.insert(name, p);
                }
            }
        }
    }
    emit_current(&app);

    // Player appear/disappear.
    let conn_a = conn.clone();
    let app_a = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(sig) = owner_changes.next().await {
            let Ok(args) = sig.args() else { continue };
            let name = args.name().to_string();
            if !name.starts_with(MPRIS_PREFIX) {
                continue;
            }
            match args.new_owner().as_ref() {
                Some(owner) => {
                    // appeared (or changed owner): fetch initial state
                    let owner = owner.to_string();
                    if let Some(p) = fetch_player(&conn_a, &name, owner).await {
                        if let Ok(mut players) = state().lock() {
                            players.insert(name, p);
                        }
                    }
                }
                None => {
                    // player closed (possibly mid-song): drop it so the card clears
                    if let Ok(mut players) = state().lock() {
                        players.remove(&name);
                    }
                }
            }
            emit_current(&app_a);
        }
    });

    // Property changes from any player, matched back via the sender's unique name.
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
        if let Ok(mut players) = state().lock() {
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
            if let Some(p) = fetch_player(&conn, &name, owner).await {
                if let Ok(mut players) = state().lock() {
                    players.insert(name, p);
                }
            }
        }
        emit_current(&app);
    }
    eprintln!("[omnideck] mpris: PropertiesChanged stream ended — Now Playing updates stopped");
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
