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
///
/// No path-root allowlist ON PURPOSE (audit follow-up): backgrounds legitimately live
/// anywhere (network photo mounts, external drives), and this surface only DISPLAYS the
/// image locally — the CSP allows no upload/exfil channel. What a crafted/imported config
/// must not be able to do is read a NON-image through it, so the gate is content-based:
/// extension AND magic bytes must both say image, the target must be a regular file
/// (canonicalized, so a symlink still lands on a sniffed real image), capped at 32 MiB.
#[tauri::command]
pub fn get_art(path: String) -> Option<String> {
    use base64::Engine;
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
    let canonical = std::fs::canonicalize(&path).ok()?;
    let meta = std::fs::metadata(&canonical).ok()?;
    if !meta.is_file() || meta.len() > 32 * 1024 * 1024 {
        return None;
    }
    let bytes = std::fs::read(&canonical).ok()?;
    if !sniff_matches(mime, &bytes) {
        return None;
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:{mime};base64,{b64}"))
}

/// True when `bytes` starts with the magic signature of the claimed image type.
fn sniff_matches(mime: &str, bytes: &[u8]) -> bool {
    match mime {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(b"\xFF\xD8\xFF"),
        "image/webp" => bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        _ => false,
    }
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
    let mut cfg = config::load_or_create();
    // The media-server token stays out of the webview: the frontend only needs to know
    // whether a server is configured (media_available covers that). Masked, not moved —
    // config.toml keeps the real value.
    cfg.media_server.token.clear();
    cfg
}

/// Prepare the custom wallpaper: a display-sized, cached copy served over `omnideck://`
/// (the frontend wraps the returned path in its `artUrl`). Returns None when the source is
/// unreadable/undecodable — the frontend then falls back to the full-image `get_art` path,
/// so a failure is never worse than before, just not faster. See background.rs for why.
#[tauri::command]
pub fn bg_image(path: String) -> Option<String> {
    let display = crate::gpu::session_display_mode().map(|(w, h, _)| (w, h));
    let out = crate::background::prepared(&path, display)?;
    Some(out.to_string_lossy().into_owned())
}

/// True when a media server is reachable-by-configuration (config or adopted shim pairing).
#[tauri::command]
pub fn media_available() -> bool {
    crate::media_server::server().is_some()
}

/// Landing sections for the media library modal (Continue Watching / Latest / libraries).
#[tauri::command]
pub async fn media_sections() -> Result<crate::media_server::MediaSections, String> {
    crate::media_server::server().ok_or("no media server configured")?.sections().await
}

/// Children of a library / series / season — every drill-down level is the same call.
#[tauri::command]
pub async fn media_browse(parent: String) -> Result<Vec<crate::media_server::MediaItem>, String> {
    crate::media_server::server().ok_or("no media server configured")?.browse(&parent).await
}

/// Fetch+cache an item's poster; returns the on-disk path for an omnideck:// URL.
#[tauri::command]
pub async fn media_poster(id: String) -> Option<String> {
    let path = crate::media_server::server()?.poster(&id).await?;
    Some(path.to_string_lossy().into_owned())
}

/// Play a media item: mpv direct-stream by default (real 4K hwdec), the Jellyfin desktop
/// client when installed and preferred. The stream URL is built server-side from the item
/// id — the frontend never supplies a URL, so there's nothing to validate away.
#[tauri::command]
pub fn media_play(app: tauri::AppHandle, id: String, name: String) -> Result<(), String> {
    let srv = crate::media_server::server().ok_or("no media server configured")?;
    let ms = config::load_or_create().media_server;
    let prefer_mpv = ms.kind.is_empty() || ms.prefer_mpv; // adopted-pairing default: mpv
    let has_mpv = apps::has_bin("mpv");
    let has_client = apps::has_bin("jellyfinmediaplayer");
    let exec: Vec<String> = if has_mpv && (prefer_mpv || !has_client) {
        let mut exec = vec!["mpv".to_string()];
        // Resolve the session's real refresh rate once and reuse it for both the CLI
        // override and the auto-profile tier (avoids a second X11/RandR round-trip).
        let display = crate::gpu::session_display_mode();
        // Injected FIRST so any explicit flag later on the line wins (mpv: last occurrence
        // rules). The VapourSynth interpolation profiles read mpv's display_fps at filter
        // init — usually before mpv has detected the panel — and silently fall back to 60
        // without this. The explicit `[media_server] display_fps` config value wins over the
        // session probe (it's how daily use outside the session gets the real rate); floor at
        // 30, matching the .vpy scripts — a rate they treat as "unknown" (<=30) must not be
        // forced as the pacing target either.
        let fps_override = if ms.display_fps > 30.0 {
            ms.display_fps
        } else {
            display.map(|(_, _, hz)| hz).unwrap_or(0.0)
        };
        if fps_override > 30.0 {
            exec.push(format!("--display-fps-override={fps_override:.3}"));
        }
        if !ms.mpv_args.is_empty() {
            // User-supplied config (e.g. --include of a shim profile set): its hwdec
            // choice must rule — VapourSynth filters need auto-copy, and a CLI --hwdec
            // here would silently override the config and disable the whole vf chain.
            exec.extend(ms.mpv_args.iter().cloned());
        } else {
            let auto = if ms.auto_profiles {
                crate::media_profiles::auto_include(display, ms.display_fps, ms.audio_samplerate)
            } else {
                None
            };
            match auto {
                // OmniDeck's generated display-aware profile set (F-keys switch
                // interpolation/denoise); hwdec=auto-copy comes from the included conf.
                Some(conf) => exec.push(format!("--include={}", conf.display())),
                // Bare launch: non-copy hardware decode is the fastest correct default.
                None => exec.push("--hwdec=auto-safe".into()),
            }
        }
        exec.push("--force-window=immediate".into());
        exec.push(format!("--force-media-title={name}"));
        exec.push(srv.stream_url(&id));
        exec
    } else if has_client {
        vec!["jellyfinmediaplayer".into()]
    } else {
        return Err("neither mpv nor jellyfinmediaplayer is installed".into());
    };
    // Through the normal launch path: own process group + watch_child, so the Guide
    // button, the switcher, and the Now Playing card treat playback like any launched app.
    launch_command(app, exec, Some(name), Some(format!("media-{id}")))
}

#[tauri::command]
pub fn save_settings(settings: config::Settings) -> Result<(), String> {
    config::save_settings(settings)
}

// --- Deck switcher (iOS-style app cards) ---
//
// The frontend owns the overlay UI + card navigation; these four commands are the window/
// process actions behind it. Opening the deck hides every app so OmniDeck's overlay is what
// shows; picking a card maps that one app; the close card SIGTERMs its group.

/// Open the deck: hide all launched apps (so the overlay is visible) and return the live-app
/// cards. An empty list means nothing is running — the frontend can skip showing the deck.
#[tauri::command]
pub fn deck_open() -> Vec<watchdog::LiveApp> {
    crate::switcher::hide_all();
    watchdog::live_apps()
}

/// Current live-app cards without touching window state (e.g. refreshing after one closes).
#[tauri::command]
pub fn deck_list() -> Vec<watchdog::LiveApp> {
    watchdog::live_apps()
}

/// Bring one app group to the front (deck card selected).
#[tauri::command]
pub fn deck_show(group: u32) -> Result<(), String> {
    if crate::switcher::show_group(group) { Ok(()) } else { Err("could not show that app".into()) }
}

/// Close one app group (deck card's close affordance / Select).
#[tauri::command]
pub fn deck_close(group: u32) -> Result<(), String> {
    if watchdog::close_group(group) { Ok(()) } else { Err("could not close that app".into()) }
}

/// Resolve `cmd` the way `Command::new` will: a name containing `/` is taken as a path,
/// anything else is searched in `$PATH`. Returns the resolved path if it names an existing
/// executable file. This is defense/UX only (a clear "not found" error instead of a raw
/// spawn failure) — the spawn below still goes through `Command::new` with NO shell.
fn resolve_argv0(cmd: &str) -> Option<std::path::PathBuf> {
    resolve_argv0_in(cmd, std::env::var_os("PATH").as_deref())
}

/// Inner, PATH-injectable form of [`resolve_argv0`] so tests don't mutate process env.
fn resolve_argv0_in(cmd: &str, path: Option<&std::ffi::OsStr>) -> Option<std::path::PathBuf> {
    fn is_executable(p: &std::path::Path) -> bool {
        use std::os::unix::fs::PermissionsExt;
        // metadata() follows symlinks, matching what exec() will do.
        p.metadata()
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    if cmd.contains('/') {
        let p = std::path::PathBuf::from(cmd);
        return is_executable(&p).then_some(p);
    }
    std::env::split_paths(path?)
        .map(|dir| dir.join(cmd))
        .find(|candidate| is_executable(candidate))
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
        if crate::session::in_session() {
            exec.insert(1, if is_firefox { "--kiosk".into() } else { "--start-fullscreen".into() });
            if !is_firefox {
                // Pin Chromium-family to Xwayland: gamescope exports a Wayland socket, and a
                // browser that picks it puts its window where NONE of our machinery can reach
                // it — the switcher/watchdog manage windows through X (_NET_WM_PID, unmap/map),
                // and the navpad's virtual keyboard is delivered via Xwayland focus. (Firefox
                // is already pinned by the GDK_BACKEND=x11 we inherit from gpu.rs.)
                exec.insert(2, "--ozone-platform=x11".into());
                // Pin the device scale to 1: under Xwayland at 2560x1440 Chromium can
                // auto-detect a 2x HiDPI scale and render the page into the top-left
                // quarter/half of the output (the couch-test "PWA on the left half"). We
                // drive one known display; force 1:1 so the page fills it.
                exec.insert(3, "--force-device-scale-factor=1".into());
            }
        }
    }
    let (cmd, args) = exec.split_first().ok_or("empty command")?;
    // Pre-flight argv[0]: custom-launcher input arrives as a raw argv vector, so a typo'd
    // binary otherwise dies in spawn() with an opaque "No such file or directory". Resolve
    // against PATH here and return a clear Err the UI can toast. Check only — the spawn
    // still uses the original `cmd` (no shell, no rewriting).
    if resolve_argv0(cmd).is_none() {
        return Err(format!("command not found: {cmd} (not an executable on PATH)"));
    }
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
    if crate::session::in_session() {
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

/// Switch between OmniDeck and the launched app without closing it (UI path; Guide press /
/// Ctrl+Alt+Home do the same). Returns true if something was hidden or re-shown.
#[tauri::command]
pub fn switch_app() -> bool {
    crate::switcher::toggle().is_some()
}

/// True when OmniDeck is running as a gamescope session (vs. a window on the desktop). Lets
/// the UI relabel "Exit OmniDeck" as "Log out" — in a session, quitting returns to the greeter.
#[tauri::command]
pub fn in_gamescope_session() -> bool {
    crate::session::in_session()
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
    use super::{is_safe_browser_arg, resolve_argv0_in};
    use std::ffi::OsString;
    use std::os::unix::fs::PermissionsExt;

    /// Unique scratch dir per test (no tempfile dep in this crate).
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("omnideck-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn resolves_argv0_on_path_and_rejects_missing() {
        let dir = scratch("resolve");
        // An executable file...
        let exe = dir.join("fakelauncher");
        std::fs::write(&exe, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        // ...and a non-executable one in the same dir.
        let plain = dir.join("notexec");
        std::fs::write(&plain, "data").unwrap();
        std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644)).unwrap();

        let path = OsString::from(dir.as_os_str());
        // Bare name resolves through the supplied PATH.
        assert_eq!(resolve_argv0_in("fakelauncher", Some(&path)), Some(exe.clone()));
        // Typo'd / absent binary -> None (launch_command turns this into a clear Err).
        assert_eq!(resolve_argv0_in("fakelaunchr", Some(&path)), None);
        // Present but not executable -> None.
        assert_eq!(resolve_argv0_in("notexec", Some(&path)), None);
        // No PATH at all -> None for bare names.
        assert_eq!(resolve_argv0_in("fakelauncher", None), None);

        // Names with '/' bypass PATH: taken as a path, checked directly.
        let abs = exe.to_string_lossy().into_owned();
        assert_eq!(resolve_argv0_in(&abs, None), Some(exe));
        assert_eq!(resolve_argv0_in("/definitely/not/a/real/binary", Some(&path)), None);
        // A directory is not an executable file.
        assert_eq!(resolve_argv0_in(&dir.to_string_lossy(), None), None);

        let _ = std::fs::remove_dir_all(&dir);
    }

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
