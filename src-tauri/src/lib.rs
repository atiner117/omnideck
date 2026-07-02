// OmniDeck — M0 spike backend.
// Goal of M0: prove a WebKitGTK/Tauri webview both RENDERS and ACCEPTS controller
// input inside gamescope on NVIDIA. This file owns the *real* input path we plan to
// ship: gilrs reads evdev on a dedicated std thread (gilrs is !Send, so it cannot
// live in a tokio task) and forwards typed events to the webview via Tauri events.
mod apps;
mod asset;
mod capability;
mod config;
mod http;
mod icons;
mod library;
mod steamgriddb;

use clap::Parser;
use serde::Serialize;
use std::sync::atomic::{AtomicU32, Ordering};
use tauri::Emitter;

#[derive(Clone, Serialize)]
struct GamepadEvent {
    kind: String,
    code: String,
    value: f32,
    gamepad: String,
    name: String,
}

/// PID of the most-recently-launched foreground app (PWA/native) that OmniDeck spawned,
/// or 0 for none. Lets the Guide ("Home") button / a UI action close the launched app and
/// return to OmniDeck — inside gamescope a launched window stacks on top of us with no other
/// way back. Steam games use a separate path (gamescope refocuses us when the game exits).
static CURRENT_CHILD: AtomicU32 = AtomicU32::new(0);

/// Close the current foreground app so gamescope refocuses OmniDeck. Best-effort SIGTERM by
/// PID; the child's `watch_child` thread reaps it and emits `app-exited`. Returns true if a
/// running app was signalled.
fn return_home() -> bool {
    let pid = CURRENT_CHILD.load(Ordering::SeqCst);
    if pid == 0 {
        return false;
    }
    // Signal the whole process GROUP (negative pid). Browsers (Brave/Chromium) fork a
    // persistent main process, so SIGTERM to the single spawned pid can leave a window
    // behind; the child is spawned as its own group leader (process_group(0) in launch),
    // so -pid reaches every forked helper. Fall back to the bare pid if that misses.
    let grp = format!("-{pid}");
    let grp_ok = std::process::Command::new("kill").args(["-TERM", &grp]).status().map(|s| s.success()).unwrap_or(false);
    let pid_ok = std::process::Command::new("kill").args(["-TERM", &pid.to_string()]).status().map(|s| s.success()).unwrap_or(false);
    // Only report success if a signal actually reached something — otherwise the caller would
    // emit "app-closed" / swallow the Guide press while the window is still on screen.
    grp_ok || pid_ok
}

fn gamepad_loop(handle: tauri::AppHandle) {
    let mut gilrs = match gilrs::Gilrs::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("[omnideck] gilrs init FAILED: {e}");
            let _ = handle.emit("gamepad-status", format!("gilrs init FAILED: {e}"));
            return;
        }
    };

    let pads: Vec<String> = gilrs
        .gamepads()
        .map(|(id, g)| format!("{id:?}:{}", g.name()))
        .collect();
    eprintln!("[omnideck] gilrs ready — {} pad(s): {pads:?}", pads.len());
    let _ = handle.emit(
        "gamepad-status",
        format!("gilrs ready — {} pad(s) connected: {pads:?}", pads.len()),
    );

    // Coalesce noisy AxisChanged: a jittery resting stick streams ~125 events/s/axis; the
    // frontend only needs coarse values for its 0.6 deadband. Emit only when an axis has moved
    // at least AXIS_EPS from its last EMITTED value (cuts IPC volume ~10x on drifty sticks).
    let mut last_axis: std::collections::HashMap<(gilrs::GamepadId, gilrs::Axis), f32> =
        std::collections::HashMap::new();
    const AXIS_EPS: f32 = 0.05;

    loop {
        while let Some(gilrs::Event { id, event, .. }) = gilrs.next_event() {
            let name = gilrs.gamepad(id).name().to_string();
            // Guide/Home button closes a launched app and returns to OmniDeck. gilrs reads
            // evdev directly, so this fires even while the launched app holds window focus.
            if let gilrs::EventType::ButtonPressed(gilrs::Button::Mode, _) = &event {
                if return_home() {
                    let _ = handle.emit("app-closed", ());
                    continue; // swallow the press; don't also forward it as a UI event
                }
            }
            // Drop sub-epsilon axis jitter before it crosses the IPC boundary.
            if let gilrs::EventType::AxisChanged(a, v, _) = &event {
                let key = (id, *a);
                if last_axis.get(&key).is_some_and(|p| (*p - *v).abs() < AXIS_EPS) {
                    continue;
                }
                last_axis.insert(key, *v);
            }
            let (kind, code, value) = match event {
                gilrs::EventType::ButtonPressed(b, _) => {
                    ("button_pressed".to_string(), format!("{b:?}"), 1.0)
                }
                gilrs::EventType::ButtonReleased(b, _) => {
                    ("button_released".to_string(), format!("{b:?}"), 0.0)
                }
                gilrs::EventType::ButtonChanged(b, v, _) => {
                    ("button_changed".to_string(), format!("{b:?}"), v)
                }
                gilrs::EventType::AxisChanged(a, v, _) => {
                    ("axis_changed".to_string(), format!("{a:?}"), v)
                }
                gilrs::EventType::Connected => ("connected".to_string(), String::new(), 0.0),
                gilrs::EventType::Disconnected => {
                    ("disconnected".to_string(), String::new(), 0.0)
                }
                _ => ("other".to_string(), String::new(), 0.0),
            };
            let _ = handle.emit(
                "gamepad-event",
                GamepadEvent {
                    kind,
                    code,
                    value,
                    gamepad: format!("{id:?}"),
                    name,
                },
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(8));
    }
}

/// Emit a launched event, then watch the child and emit an exited event when it ends.
/// (Lets the UI show a "now playing" state and know when focus returns.)
fn watch_child(app: tauri::AppHandle, mut child: std::process::Child, name: String, id: Option<String>) {
    let pid = child.id();
    CURRENT_CHILD.store(pid, Ordering::SeqCst); // newest launch becomes the "current" app
    // The frontend correlates Now Playing entries by this launch id (the tile id), falling back
    // to the name for any legacy caller, so two same-named launchables don't clobber on exit.
    let exit_key = id.unwrap_or_else(|| name.clone());
    let _ = app.emit("app-launched", name);
    std::thread::spawn(move || {
        let _ = child.wait();
        // Clear only if a newer launch hasn't already replaced us as the current app.
        let _ = CURRENT_CHILD.compare_exchange(pid, 0, Ordering::SeqCst, Ordering::SeqCst);
        let _ = app.emit("app-exited", exit_key);
    });
}

/// Stamp our window with STEAM_GAME=769 once (best-effort). Returns whether xprop succeeded.
fn stamp_steam_atom_once() -> bool {
    std::process::Command::new("xprop")
        .args(["-name", "omnideck", "-f", "STEAM_GAME", "32c", "-set", "STEAM_GAME", "769"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Stamp STEAM_GAME=769 on our window once inside any gamescope session (best-effort, via
/// xprop). install-session.sh runs a *plain* gamescope session (no `--steam`); there this
/// atom drives the Steam-game focus-return path: when a launched game window is destroyed,
/// gamescope re-shows the window tagged STEAM_GAME=769 ("main application") — i.e. us (see
/// watch_steam_game + its ChimeraOS note). Load-bearing until a hardware session test proves
/// focus-return works without it — see packaging/M2-RESULTS.md.
fn set_steam_game_atom_if_gamescope() {
    // Only relevant inside a gamescope (steamcompmgr) session.
    if std::env::var_os("GAMESCOPE_WAYLAND_DISPLAY").is_none() {
        return;
    }
    std::thread::spawn(|| {
        if std::process::Command::new("xprop").arg("-version").output().is_err() {
            eprintln!(
                "[omnideck] WARNING: `xprop` not found — install `xorg-xprop`, or the gamescope \
                 session may be a black screen (cannot set the STEAM_GAME atom)."
            );
            return;
        }
        // The window appears a moment after the webview initializes; retry for ~12s.
        for attempt in 1..=40 {
            std::thread::sleep(std::time::Duration::from_millis(300));
            if stamp_steam_atom_once() {
                eprintln!("[omnideck] STEAM_GAME=769 set on window (attempt {attempt})");
                return;
            }
        }
        eprintln!(
            "[omnideck] WARNING: could not set STEAM_GAME after 40 tries (window not found by \
             name 'omnideck'). If the session is black, set it manually — see \
             packaging/M2-SESSION-TEST.md."
        );
    });
}

/// Locate Steam's registry.vdf, which records per-app running state.
fn steam_registry_path() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    for rel in [
        ".steam/registry.vdf",
        ".steam/steam/registry.vdf",
        ".local/share/Steam/registry.vdf",
    ] {
        let p = std::path::Path::new(&home).join(rel);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Heuristic VDF scan: is `<appid>` marked `"running" "1"` in the registry text?
/// Some(true)=running, Some(false)=present-and-stopped, None=unknown/not found.
/// The quoted appid anchors the match so "570" can't hit inside "12570".
fn steam_app_running(text: &str, appid: &str) -> Option<bool> {
    let start = text.find(&format!("\"{appid}\""))?;
    let window = &text[start..(start + 400).min(text.len())];
    // Steam has used both "running" and "Running" across versions.
    let (k, klen) = window
        .find("\"running\"")
        .map(|i| (i, "\"running\"".len()))
        .or_else(|| window.find("\"Running\"").map(|i| (i, "\"Running\"".len())))?;
    let after = &window[k + klen..];
    let q1 = after.find('"')?;
    let q2 = after[q1 + 1..].find('"')?;
    Some(&after[q1 + 1..q1 + 1 + q2] == "1")
}

#[cfg(test)]
mod tests {
    use super::steam_app_running;
    const SAMPLE: &str = r#"
"Registry" { "HKCU" { "Software" { "Valve" { "Steam" { "apps" {
  "570"   { "running"  "1"  "installed"  "1" }
  "12570" { "running"  "0" }
  "440"   { "Running"  "0"  "name"  "Team Fortress" }
}}}}}}"#;

    #[test]
    fn detects_running_and_stopped() {
        assert_eq!(steam_app_running(SAMPLE, "570"), Some(true));
        assert_eq!(steam_app_running(SAMPLE, "440"), Some(false)); // case-insensitive key
        assert_eq!(steam_app_running(SAMPLE, "12570"), Some(false));
        assert_eq!(steam_app_running(SAMPLE, "99999"), None); // not present
    }

    #[test]
    fn quoted_appid_does_not_match_substring() {
        // "57" must NOT match the "570"/"12570" blocks (quote-anchored).
        assert_eq!(steam_app_running(SAMPLE, "57"), None);
    }

    #[test]
    fn rejects_unsafe_browser_args() {
        use super::is_safe_browser_arg;
        assert!(is_safe_browser_arg("https://duckduckgo.com/?q=cats"));
        assert!(is_safe_browser_arg("--app=https://www.netflix.com"));
        assert!(is_safe_browser_arg("http://192.168.1.5:8080")); // local SearXNG over http
        assert!(!is_safe_browser_arg("--renderer-cmd-prefix=/bin/sh -c id")); // RCE flag
        assert!(!is_safe_browser_arg("--no-sandbox"));
        assert!(!is_safe_browser_arg("--app=file:///etc/passwd")); // non-http scheme
    }
}

/// Exit watchdog for a Steam launch (M2): the `steam://` URI returns immediately, so we
/// poll registry.vdf — wait for the game to flip to running (cold start can be slow),
/// then wait for it to stop — then tell the UI.
///
/// Focus return is normally AUTOMATIC: gamescope shows the window whose STEAM_GAME=769
/// ("main application") once a higher-priority game window is destroyed (per ChimeraOS
/// gamescope-session docs). So our window reappearing is gamescope's job, not ours. The
/// re-stamp below is a belt-and-suspenders no-op if the atom is still set; if M2 shows
/// gamescope NOT returning to us, the stronger lever is GAMESCOPECTRL_BASELAYER_APPID on
/// the root window (pins our appid as the base layer) — add that only if needed.
fn watch_steam_game(app: tauri::AppHandle, appid: String, name: String, id: Option<String>) {
    let exit_key = id.unwrap_or_else(|| name.clone());
    std::thread::spawn(move || {
        let reg = match steam_registry_path() {
            Some(p) => p,
            None => return, // can't observe; UI just stays on "now playing" until user backs out
        };
        let running = |reg: &std::path::Path| -> Option<bool> {
            std::fs::read_to_string(reg).ok().and_then(|t| steam_app_running(&t, &appid))
        };
        // Phase 1: confirm it actually started (up to ~120s for a cold Steam + shader pre-cache).
        let mut started = false;
        for _ in 0..240 {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if running(&reg) == Some(true) {
                started = true;
                break;
            }
        }
        if !started {
            eprintln!("[omnideck] watchdog: '{name}' never reported running; giving up");
            let _ = app.emit("app-exited", exit_key);
            return;
        }
        eprintln!("[omnideck] watchdog: '{name}' is running");
        // Phase 2: wait for exit. running()==None means "unknown" (registry momentarily
        // unreadable, or the appid block vanished after a Steam restart). Tolerate brief None
        // runs, but give up after a long stretch so a Steam crash mid-game can't spin this
        // thread at 1 Hz forever.
        let mut unknown = 0u32;
        loop {
            match running(&reg) {
                Some(false) => break,      // confirmed stopped
                Some(true) => unknown = 0, // confirmed running — reset the unknown counter
                None => {
                    unknown += 1;
                    if unknown >= 900 {
                        eprintln!("[omnideck] watchdog: '{name}' state unknown for ~15 min; giving up");
                        break;
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(1000));
        }
        eprintln!("[omnideck] watchdog: '{name}' exited — refocusing OmniDeck");
        let _ = app.emit("app-exited", exit_key);
        // Best-effort focus recovery in a gamescope session.
        if std::env::var_os("GAMESCOPE_WAYLAND_DISPLAY").is_some() {
            stamp_steam_atom_once();
        }
    });
}

/// System power actions (logind handles auth for the active local session, no sudo needed).
#[tauri::command]
fn power_action(action: String) -> Result<(), String> {
    let verb = match action.as_str() {
        "suspend" => "suspend",
        "reboot" => "reboot",
        "poweroff" => "poweroff",
        _ => return Err(format!("unknown power action: {action}")),
    };
    // `.status()` (not `.spawn()`): wait for systemctl's exit so a polkit denial — it execs,
    // prints to stderr, then exits non-zero *after* logind decides — surfaces as Err (the UI
    // toasts it) instead of resolving Ok the instant fork+exec succeeds. systemctl returns
    // promptly once logind accepts the request, so blocking here is fine.
    let status = std::process::Command::new("systemctl")
        .arg(verb)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        return Ok(());
    }
    let code = status.code().map(|c| c.to_string()).unwrap_or_else(|| "signal".into());
    Err(format!(
        "`systemctl {verb}` was denied (exit {code}). In a display-manager session this is \
         usually polkit: the session may not be an active local seat."
    ))
}

#[tauri::command]
fn get_capability() -> capability::Capability {
    capability::probe()
}

#[tauri::command]
fn get_library() -> library::Library {
    library::scan()
}

/// Read a local image file and return it as a data URL the webview can display.
/// (v1: avoids asset-protocol config; switch to the asset protocol if memory matters.)
#[tauri::command]
fn get_art(path: String) -> Option<String> {
    use base64::Engine;
    // Only serve image files: this turns a local path into a data URL, so restrict the
    // extensions (don't let a crafted config read e.g. ~/.ssh/id_rsa), and cap the size so a
    // huge/unexpected file can't balloon into memory as base64.
    let lower = path.to_ascii_lowercase();
    let mime = if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else {
        return None;
    };
    if std::fs::metadata(&path).ok()?.len() > 32 * 1024 * 1024 {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:{mime};base64,{b64}"))
}

/// Launch a Steam game by appid. In a gamescope session Steam stamps the game
/// window's STEAM_GAME atom so it foregrounds; the exit watchdog is M2.
#[tauri::command]
fn launch_game(app: tauri::AppHandle, appid: String, name: Option<String>, id: Option<String>) -> Result<(), String> {
    // Steam's URI handler returns immediately, so the running game has no child handle
    // here; watch_steam_game polls Steam's registry to detect start/exit instead.
    std::process::Command::new("steam")
        .arg(format!("steam://rungameid/{appid}"))
        .spawn()
        .map_err(|e| e.to_string())?;
    let label = name.unwrap_or_else(|| format!("game {appid}"));
    let _ = app.emit("app-launched", label.clone());
    watch_steam_game(app, appid, label, id);
    Ok(())
}

#[tauri::command]
fn get_apps() -> Vec<apps::App> {
    apps::list()
}

#[tauri::command]
fn get_config() -> config::Config {
    config::load_or_create()
}

#[tauri::command]
fn save_settings(settings: config::Settings) -> Result<(), String> {
    config::save_settings(settings)
}

/// True if `arg` is safe to pass to a browser after the BROWSER token: an http(s) URL, or
/// our `--app=<http(s) URL>` PWA form. Rejects flags so a crafted `search_provider` or a
/// hand-edited config can't inject e.g. Chromium's `--renderer-cmd-prefix` (arbitrary exec).
fn is_safe_browser_arg(arg: &str) -> bool {
    let u = arg.strip_prefix("--app=").unwrap_or(arg);
    u.starts_with("https://") || u.starts_with("http://")
}

/// Launch an arbitrary app/media command (argv form). A leading "BROWSER" token is
/// resolved to the host's browser (Chromium-family `--app=` PWA mode; Firefox opens
/// the URL directly since it lacks `--app`).
#[tauri::command]
fn launch_command(app: tauri::AppHandle, exec: Vec<String>, name: Option<String>, id: Option<String>) -> Result<(), String> {
    let mut exec = exec;
    if exec.first().map(|s| s == "BROWSER").unwrap_or(false) {
        // Only URLs may follow the BROWSER token (flag-injection guard — see is_safe_browser_arg).
        for a in &exec[1..] {
            if !is_safe_browser_arg(a) {
                return Err(format!("refusing unsafe browser argument: {a}"));
            }
        }
        let browser = apps::detect_browser().ok_or("no browser found")?;
        let is_firefox = browser.contains("firefox");
        if is_firefox {
            for a in exec.iter_mut() {
                if let Some(url) = a.strip_prefix("--app=") {
                    *a = url.to_string();
                }
            }
        }
        exec[0] = browser;
        // Inside a gamescope session a browser PWA opens windowed and doesn't fill the
        // screen; ask it to start fullscreen (Firefox uses --kiosk, Chromium --start-fullscreen).
        if std::env::var_os("GAMESCOPE_WAYLAND_DISPLAY").is_some() {
            exec.insert(1, if is_firefox { "--kiosk".into() } else { "--start-fullscreen".into() });
        }
    }
    let (cmd, args) = exec.split_first().ok_or("empty command")?;
    use std::os::unix::process::CommandExt;
    let child = std::process::Command::new(cmd)
        .args(args)
        // Own process group so return_home() can SIGTERM the whole group (browsers fork
        // helpers/persistent processes that would otherwise survive a single-pid kill).
        .process_group(0)
        .spawn()
        .map_err(|e| e.to_string())?;
    watch_child(app, child, name.unwrap_or_else(|| cmd.clone()), id);
    Ok(())
}

#[tauri::command]
fn get_catalog() -> Vec<apps::App> {
    apps::catalog()
}

#[tauri::command]
fn save_apps(apps: Vec<apps::App>) -> Result<(), String> {
    config::save_apps(apps)
}

#[tauri::command]
fn save_favorites(favorites: Vec<String>) -> Result<(), String> {
    config::save_favorites(favorites)
}

#[tauri::command]
fn save_recent_apps(recent_apps: Vec<String>) -> Result<(), String> {
    config::save_recent_apps(recent_apps)
}

/// Open Steam's per-game Properties dialog for the focused game.
#[tauri::command]
fn game_properties(appid: String) -> Result<(), String> {
    std::process::Command::new("steam")
        .arg(format!("steam://gameproperties/{appid}"))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[derive(Clone, Serialize)]
struct MediaInfo {
    status: String, // "Playing" | "Paused" | "Stopped"
    title: String,
    artist: String,
    player: String,
}

/// Current media metadata via MPRIS (`playerctl`). None if playerctl is missing or no
/// player is active. Works for native players (Feishin, Spotify) and browser PWAs
/// (YouTube Music in a Chromium/Brave window) since browsers expose MPRIS too.
#[tauri::command]
fn media_now_playing() -> Option<MediaInfo> {
    let out = std::process::Command::new("playerctl")
        .args([
            "metadata",
            "--format",
            "{{status}}\t{{title}}\t{{artist}}\t{{playerName}}",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&out.stdout);
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let mut parts = line.splitn(4, '\t');
    let status = parts.next().unwrap_or("").to_string();
    let title = parts.next().unwrap_or("").to_string();
    let artist = parts.next().unwrap_or("").to_string();
    let player = parts.next().unwrap_or("").to_string();
    if title.is_empty() && artist.is_empty() {
        return None;
    }
    Some(MediaInfo {
        status,
        title,
        artist,
        player,
    })
}

/// Control the active MPRIS player (play-pause / next / previous) via `playerctl`.
#[tauri::command]
fn media_control(action: String) -> Result<(), String> {
    let verb = match action.as_str() {
        "play-pause" => "play-pause",
        "next" => "next",
        "previous" => "previous",
        _ => return Err(format!("unknown media action: {action}")),
    };
    std::process::Command::new("playerctl")
        .arg(verb)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Quit the launcher. In a gamescope session this exits CLIENTCMD, which ends the
/// session and returns to the display manager.
#[tauri::command]
fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

/// Close the currently-foregrounded launched app and return to OmniDeck (UI/keyboard path;
/// the gamepad Guide button does the same). Returns true if an app was running.
#[tauri::command]
fn close_current_app() -> bool {
    return_home()
}

/// True when OmniDeck is running as a gamescope session (vs. a window on the desktop). Lets
/// the UI relabel "Exit OmniDeck" as "Log out" — in a session, quitting returns to the greeter.
#[tauri::command]
fn in_gamescope_session() -> bool {
    std::env::var_os("GAMESCOPE_WAYLAND_DISPLAY").is_some()
}

/// Fetch missing vertical box art from SteamGridDB (no-op without a configured key). Cached.
#[tauri::command]
async fn grid_art(appid: String) -> Option<String> {
    let key = config::load_or_create().settings.steamgriddb_key;
    steamgriddb::box_art(&appid, &key).await
}

/// Fetch a web/streaming tile's site icon as a data URL (cached). `url` may be a bare
/// URL or our `--app=<url>` exec token; returns None for non-web entries.
#[tauri::command]
async fn app_icon(url: String) -> Option<String> {
    icons::favicon(&url).await
}

/// Re-exec self once with GPU-appropriate WebKit env so the webview renders on any GPU. The
/// NVIDIA WebKitGTK workaround is *session-specific* (2026): on X11/gamescope the dmabuf
/// renderer is the bug; on Wayland the bug is a startup crash fixed by disabling explicit sync
/// (no perf cost) — and `GDK_BACKEND=x11` must NOT be forced (it reintroduces the
/// fractional-scaling/blur/input regressions Wayland users left X11 to escape). AMD/Intel
/// (Mesa) need nothing. Env must be set before the webview initializes, so we re-exec.
#[cfg(unix)]
fn ensure_gpu_env() {
    if std::env::var_os("OMNIDECK_ENV_READY").is_some() {
        return;
    }
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.args(std::env::args_os().skip(1));
    cmd.env("OMNIDECK_ENV_READY", "1");
    if capability::probe().nvidia_present {
        let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default().to_ascii_lowercase();
        let in_gamescope = std::env::var_os("GAMESCOPE_WAYLAND_DISPLAY").is_some()
            || std::env::var_os("STEAM_GAMESCOPE").is_some();
        if in_gamescope || session == "x11" {
            // X11/gamescope: the dmabuf renderer paints blank on NVIDIA — disable it.
            cmd.env("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        } else if session == "wayland" {
            // Wayland: WebKitGTK won't start on NVIDIA without this (explicit-sync crash); keeps
            // the hardware-accelerated fast path, unlike disabling dmabuf.
            cmd.env("__NV_DISABLE_EXPLICIT_SYNC", "1");
        } else {
            // Unknown session type: take the conservative X11-style workaround.
            cmd.env("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        // Last-resort: WEBKIT_DISABLE_COMPOSITING_MODE forces SOFTWARE paint (caps animation
        // smoothness — the category-switch fps dip). Set OMNIDECK_GPU_COMPOSITING=1 to try GPU
        // compositing instead: smoother *if* driver + WebKitGTK render correctly without it.
        if std::env::var_os("OMNIDECK_GPU_COMPOSITING").is_none() {
            cmd.env("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }
    use std::os::unix::process::CommandExt;
    let _ = cmd.exec(); // replaces this process; returns only on failure
}

#[cfg(not(unix))]
fn ensure_gpu_env() {}

/// Headless CLI surface. With no subcommand, OmniDeck launches its GUI; the subcommands are
/// debug/inspection helpers. `--version` and `--help` come for free from clap.
#[derive(Parser)]
#[command(name = "omnideck", version, about = "10-foot, controller-first media & game launcher for Linux")]
struct Cli {
    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(clap::Subcommand)]
enum CliCommand {
    /// Capability probe: tier + GPU/KMS/Vulkan detection (human-readable + JSON)
    Probe,
    /// Scan the Steam library
    Scan,
    /// Print the resolved config (path + settings + apps)
    Config,
    /// Fetch + cache SteamGridDB box art for an appid (needs steamgriddb_key in config)
    Gridart {
        /// Steam appid, e.g. 570
        appid: String,
    },
    /// List the bundled app/media catalog
    Catalog,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Headless subcommands; no subcommand = launch the GUI. Parsed BEFORE ensure_gpu_env so a
    // CLI invocation never triggers the GPU re-exec. clap gives --version/--help and rejects
    // unknown flags (the old hand-rolled chain silently ignored them and ran only the first).
    match Cli::parse().command {
        Some(CliCommand::Probe) => {
            let cap = capability::probe();
            print!("{}", capability::report(&cap));
            println!(
                "\n--- json ---\n{}",
                serde_json::to_string_pretty(&cap).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
            );
            return;
        }
        Some(CliCommand::Scan) => {
            let lib = library::scan();
            print!("{}", library::report(&lib));
            return;
        }
        Some(CliCommand::Config) => {
            let cfg = config::load_or_create();
            print!("{}", config::report(&cfg));
            return;
        }
        Some(CliCommand::Gridart { appid }) => {
            let key = config::load_or_create().settings.steamgriddb_key;
            if key.is_empty() {
                println!("gridart: no steamgriddb_key set in config.toml [settings]");
            } else {
                let got = tauri::async_runtime::block_on(steamgriddb::box_art(&appid, &key)).is_some();
                println!(
                    "gridart {appid}: {}",
                    if got { "OK (box art cached)" } else { "no result / network error" }
                );
            }
            return;
        }
        Some(CliCommand::Catalog) => {
            for a in apps::catalog() {
                println!("{} {}  [{}]", a.icon, a.name, a.exec.join(" "));
            }
            return;
        }
        None => {}
    }

    // Re-exec once with GPU-appropriate webview env (NVIDIA needs workarounds; Mesa doesn't).
    ensure_gpu_env();

    tauri::Builder::default()
        // omnideck:// serves on-disk art files (Steam librarycache + our art cache) as URLs
        // instead of base64 data URLs pinned in reactive state — see asset.rs / NOTES-PERFORMANCE.
        .register_asynchronous_uri_scheme_protocol("omnideck", |_ctx, request, responder| {
            let path = request.uri().path().to_string();
            // blocking pool (reused threads) — a fast scroll fires dozens of these at once
            drop(tauri::async_runtime::spawn_blocking(move || responder.respond(asset::respond(&path))));
        })
        .invoke_handler(tauri::generate_handler![
            get_capability,
            get_library,
            get_art,
            launch_game,
            get_apps,
            get_config,
            save_settings,
            launch_command,
            grid_art,
            get_catalog,
            save_apps,
            save_favorites,
            save_recent_apps,
            game_properties,
            media_now_playing,
            media_control,
            quit,
            close_current_app,
            in_gamescope_session,
            power_action,
            app_icon
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || gamepad_loop(handle));
            set_steam_game_atom_if_gamescope();
            // In a gamescope session take the whole output: a windowed (e.g. 1280x720)
            // toplevel gets scaled/letterboxed by gamescope, so request real fullscreen
            // and let the webview render at the monitor's native resolution. On the
            // desktop we stay windowed (this only triggers inside gamescope).
            if std::env::var_os("GAMESCOPE_WAYLAND_DISPLAY").is_some() {
                use tauri::Manager;
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.set_fullscreen(true);
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
