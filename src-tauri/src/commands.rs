// OmniDeck — the Tauri IPC surface: every #[tauri::command] the frontend can invoke.
// Commands stay thin — they validate/dispatch into the domain modules (config, library,
// icons, mpris, watchdog, …). Typed mirrors live in src/lib/backend.ts.
use crate::{apps, capability, config, icons, library, mpris, steamgriddb, watchdog};

/// System power actions (logind handles auth for the active local session, no sudo needed).
#[tauri::command]
pub fn power_action(action: String) -> Result<(), String> {
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
pub fn get_capability() -> capability::Capability {
    capability::probe()
}

#[tauri::command]
pub fn get_library() -> library::Library {
    library::scan()
}

/// Read a local image file and return it as a data URL the webview can display.
/// (Still used for the custom background image; game art moved to the omnideck:// protocol.)
#[tauri::command]
pub fn get_art(path: String) -> Option<String> {
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
pub fn launch_game(app: tauri::AppHandle, appid: String, name: Option<String>, id: Option<String>) -> Result<(), String> {
    use tauri::Emitter;
    // Steam's URI handler returns immediately, so the running game has no child handle
    // here; watch_steam_game polls Steam's registry to detect start/exit instead.
    std::process::Command::new("steam")
        .arg(format!("steam://rungameid/{appid}"))
        .spawn()
        .map_err(|e| e.to_string())?;
    let label = name.unwrap_or_else(|| format!("game {appid}"));
    let _ = app.emit("app-launched", label.clone());
    watchdog::watch_steam_game(app, appid, label, id);
    Ok(())
}

#[tauri::command]
pub fn get_apps() -> Vec<apps::App> {
    apps::list()
}

#[tauri::command]
pub fn get_config() -> config::Config {
    config::load_or_create()
}

#[tauri::command]
pub fn save_settings(settings: config::Settings) -> Result<(), String> {
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
pub fn launch_command(app: tauri::AppHandle, exec: Vec<String>, name: Option<String>, id: Option<String>) -> Result<(), String> {
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
    let mut command = std::process::Command::new(cmd);
    command
        .args(args)
        // Own process group so return_home() can SIGTERM the whole group (browsers fork
        // helpers/persistent processes that would otherwise survive a single-pid kill).
        .process_group(0);
    // Inside the gamescope session there's no desktop environment, so Qt/KDE apps (System
    // Settings, Dolphin, …) load no platform theme and ignore the user's KDE color scheme —
    // they come up light in a 10-foot dark UI. Claim KDE for launched children so Qt loads
    // plasma-integration and reads ~/.config/kdeglobals (the user's real theme, dark included).
    // Harmless on non-KDE hosts: without the plugin Qt just falls back to its default theme.
    if std::env::var_os("GAMESCOPE_WAYLAND_DISPLAY").is_some() {
        if std::env::var_os("XDG_CURRENT_DESKTOP").is_none() {
            command.env("XDG_CURRENT_DESKTOP", "KDE");
        }
        if std::env::var_os("QT_QPA_PLATFORMTHEME").is_none() {
            command.env("QT_QPA_PLATFORMTHEME", "kde");
        }
    }
    let child = command.spawn().map_err(|e| e.to_string())?;
    watchdog::watch_child(app, child, name.unwrap_or_else(|| cmd.clone()), id);
    Ok(())
}

#[tauri::command]
pub fn get_catalog() -> Vec<apps::App> {
    apps::catalog()
}

#[tauri::command]
pub fn save_apps(apps: Vec<apps::App>) -> Result<(), String> {
    config::save_apps(apps)
}

#[tauri::command]
pub fn save_favorites(favorites: Vec<String>) -> Result<(), String> {
    config::save_favorites(favorites)
}

#[tauri::command]
pub fn save_recent_apps(recent_apps: Vec<String>) -> Result<(), String> {
    config::save_recent_apps(recent_apps)
}

/// Open Steam's per-game Properties dialog for the focused game.
#[tauri::command]
pub fn game_properties(appid: String) -> Result<(), String> {
    std::process::Command::new("steam")
        .arg(format!("steam://gameproperties/{appid}"))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Current media snapshot from the MPRIS watcher's state (no I/O). The frontend calls this
/// once at mount — before its `media-changed` listener attaches — then relies on events.
#[tauri::command]
pub fn media_now_playing() -> Option<mpris::MediaInfo> {
    mpris::now_playing()
}

/// Control the active MPRIS player (play-pause / next / previous) over the session bus.
#[tauri::command]
pub async fn media_control(action: String) -> Result<(), String> {
    mpris::control(&action).await
}

/// Quit the launcher. In a gamescope session this exits CLIENTCMD, which ends the
/// session and returns to the display manager.
#[tauri::command]
pub fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

/// Close the currently-foregrounded launched app and return to OmniDeck (UI/keyboard path;
/// the gamepad Guide button does the same). Returns true if an app was running.
#[tauri::command]
pub fn close_current_app() -> bool {
    watchdog::return_home()
}

/// True when OmniDeck is running as a gamescope session (vs. a window on the desktop). Lets
/// the UI relabel "Exit OmniDeck" as "Log out" — in a session, quitting returns to the greeter.
#[tauri::command]
pub fn in_gamescope_session() -> bool {
    std::env::var_os("GAMESCOPE_WAYLAND_DISPLAY").is_some()
}

/// Fetch missing vertical box art from SteamGridDB (no-op without a configured key). Cached.
#[tauri::command]
pub async fn grid_art(appid: String) -> Option<String> {
    let key = config::load_or_create().settings.steamgriddb_key;
    steamgriddb::box_art(&appid, &key).await
}

/// Fetch a web/streaming tile's site icon as a data URL (cached). `url` may be a bare
/// URL or our `--app=<url>` exec token; returns None for non-web entries.
#[tauri::command]
pub async fn app_icon(url: String) -> Option<String> {
    icons::favicon(&url).await
}

#[cfg(test)]
mod tests {
    use super::is_safe_browser_arg;

    #[test]
    fn rejects_unsafe_browser_args() {
        assert!(is_safe_browser_arg("https://duckduckgo.com/?q=cats"));
        assert!(is_safe_browser_arg("--app=https://www.netflix.com"));
        assert!(is_safe_browser_arg("http://192.168.1.5:8080")); // local SearXNG over http
        assert!(!is_safe_browser_arg("--renderer-cmd-prefix=/bin/sh -c id")); // RCE flag
        assert!(!is_safe_browser_arg("--no-sandbox"));
        assert!(!is_safe_browser_arg("--app=file:///etc/passwd")); // non-http scheme
    }
}
