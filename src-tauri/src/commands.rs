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

/// Run `f` on the blocking pool and await it. Every command whose body does real work
/// (X11 round-trips, process spawns, fsync) must route through this instead of running
/// inline on the main thread (sync commands do — that's the UI-freeze class bg_image and
/// media_play were converted to fix). One place owns the join-error policy.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())
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
        .map_err(|e| spawn_error("steam", &e))?;
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
    // config.toml keeps the real value. Same for the phone-remote token (its ONE
    // deliberate exposure is the pairing URL from remote_status — see remote.rs).
    cfg.media_server.token.clear();
    // Same for the PIN hash: a 4–6 digit PIN is offline-crackable from its argon2 hash,
    // so the webview only gets presence (has_pin) — verification happens in verify_pin.
    cfg.has_pin = Some(!cfg.settings.pin_hash.is_empty());
    cfg.settings.pin_hash.clear();
    cfg.remote.token.clear();
    cfg
}

/// Back up config.toml to `dest` as a sanitized TOML snapshot (roadmap #5). Credentials
/// (media-server token, SteamGridDB key) are excluded unless `include_credentials` — the
/// backup is meant to travel (email, cloud drive), the secrets are not. Returns the path
/// written so the UI can toast it.
#[tauri::command]
pub async fn backup_config(dest: String, include_credentials: bool) -> Result<String, String> {
    // blocking: write_atomic fsyncs the file and its directory.
    blocking(move || config::backup_to(std::path::Path::new(&dest), include_credentials)).await?
}

/// Restore config.toml from a backup file. The contents pass through the same
/// `Settings::normalize` gates as a hand-edited config, and credential fields left empty in
/// the backup keep their current values. Returns the freshly loaded config (token masked,
/// same as `get_config`) so the UI can re-render without a restart.
#[tauri::command]
pub async fn restore_config(src: String) -> Result<config::Config, String> {
    // blocking: fsync via write_atomic, and SAVE_LOCK can wait behind another saver.
    blocking(move || {
        let mut cfg = config::restore_from(std::path::Path::new(&src))?;
        cfg.media_server.token.clear(); // never hand the real token to the webview
        cfg.remote.token.clear();
        Ok(cfg)
    })
    .await?
}

/// Check GitHub for a newer release (roadmap #4, check-only — acting on it is per-distro
/// follow-up). Result is cached for the process lifetime (unauthed GitHub API = 60 req/hr);
/// `force` bypasses the cache for a manual "Check now". The frontend gates the automatic
/// boot-time call on `settings.check_updates`; a manual check always works.
#[tauri::command]
pub async fn check_update(force: bool) -> Result<crate::update::UpdateInfo, String> {
    crate::update::check(force).await
}

/// Prepare the custom wallpaper: a display-sized, cached copy served over `omnideck://`
/// (the frontend wraps the returned path in its `artUrl`). Returns None when the source is
/// unreadable/undecodable — the frontend then falls back to the full-image `get_art` path,
/// so a failure is never worse than before, just not faster. See background.rs for why.
#[tauri::command]
pub async fn bg_image(path: String) -> Option<String> {
    // blocking: the first run decodes + re-encodes a full-resolution photo (hundreds of
    // ms) — as a sync command this ran inline on the main thread and froze the UI at
    // startup, the exact stall background.rs exists to remove.
    blocking(move || {
        let display = crate::gpu::session_display_mode().map(|(w, h, _)| (w, h));
        let out = crate::background::prepared(&path, display)?;
        Some(out.to_string_lossy().into_owned())
    })
    .await
    .ok()
    .flatten()
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
pub async fn media_play(app: tauri::AppHandle, id: String, name: String) -> Result<String, String> {
    // blocking: the body does an X11/RandR probe, the (first-play) mpv capability probe,
    // and profile-template I/O — as a sync command all of that ran inline on the main
    // thread, freezing the UI and every other IPC call for the duration.
    blocking(move || media_play_blocking(app, id, name)).await?
}

fn media_play_blocking(app: tauri::AppHandle, id: String, name: String) -> Result<String, String> {
    if !crate::media_server::valid_id(&id) {
        return Err("invalid media id".into());
    }
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
    // Per-LAUNCH exit key, not per item (same fix launchTile got): replaying an item while
    // an earlier instance is still alive must not share a key, or the first exit clears the
    // survivor's Now Playing card. Returned so the frontend keys its card identically.
    static MEDIA_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let key = format!(
        "media-{id}#{}",
        MEDIA_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    // Through the normal launch path: own process group + watch_child, so the Guide
    // button, the switcher, and the Now Playing card treat playback like any launched app.
    launch_command(app, exec, Some(name), Some(key.clone()))?;
    Ok(key)
}

/// The four save commands fsync the config file AND its directory (write_atomic) — that's
/// disk I/O that must not run inline on the main thread, so they ride the blocking pool
/// like every other I/O command. save_recent_apps fires on every app launch.
#[tauri::command]
pub async fn save_settings(settings: config::Settings) -> Result<(), String> {
    blocking(move || config::save_settings(settings)).await?
}

// --- Deck switcher (iOS-style app cards) ---
//
// The frontend owns the overlay UI + card navigation; these four commands are the window/
// process actions behind it. Opening the deck hides every app so OmniDeck's overlay is what
// shows; picking a card maps that one app; the close card SIGTERMs its group.

/// Open the deck: hide all launched apps (so the overlay is visible) and return the live-app
/// cards. An empty list means nothing is running — the frontend can skip showing the deck.
/// Async: hide_all's unmap-verify loop can sleep ~400 ms and the freeze policy shells out
/// to pactl — none of that may run inline on the main thread (sync commands do).
#[tauri::command]
pub async fn deck_open() -> Vec<watchdog::LiveApp> {
    blocking(|| {
        // Same gate show_group enforces: outside a session the hide can't work and every
        // card select is refused — opening an inert deck just swallowed controller input.
        if !crate::switcher::session_ok() {
            return Vec::new();
        }
        crate::switcher::hide_all();
        watchdog::live_apps()
    })
    .await
    .unwrap_or_default()
}

/// Deck dismissed without picking a card (second Guide tap, B, Escape, scrim): restore what
/// deck_open hid — re-show + SIGCONT — so the tap-tap round trip lands back in the app.
#[tauri::command]
pub async fn deck_cancel() -> bool {
    blocking(crate::switcher::deck_cancel).await.unwrap_or(false)
}

/// Current live-app cards without touching window state (e.g. refreshing after one closes).
#[tauri::command]
pub fn deck_list() -> Vec<watchdog::LiveApp> {
    watchdog::live_apps()
}

/// Only group ids the watchdog is actually tracking may reach `kill`: the id comes from the
/// webview, and an arbitrary u32 (or a stale id from a recycled pgid) must not signal
/// unrelated processes — `kill -TERM -1` would TERM the user's entire session.
fn known_group(group: u32) -> Result<(), String> {
    if watchdog::live_groups().contains(&group) { Ok(()) } else { Err("unknown app group".into()) }
}

/// Bring one app group to the front (deck card selected).
#[tauri::command]
pub async fn deck_show(group: u32) -> Result<(), String> {
    known_group(group)?;
    let ok = blocking(move || crate::switcher::show_group(group)).await.unwrap_or(false);
    if ok { Ok(()) } else { Err("could not show that app".into()) }
}

/// Close one app group (deck card's close affordance / Select).
#[tauri::command]
pub async fn deck_close(group: u32) -> Result<(), String> {
    known_group(group)?;
    let ok = blocking(move || watchdog::close_group(group)).await.unwrap_or(false);
    if ok { Ok(()) } else { Err("could not close that app".into()) }
}

/// Map a spawn failure to a message the UI can toast. Custom-launcher input arrives as a
/// raw argv vector, so a typo'd binary otherwise surfaces as an opaque "No such file or
/// directory". Mapping the error at the spawn (instead of pre-flighting PATH ourselves)
/// can never disagree with execvp's real resolution — permission bits, ACLs, and the
/// unset-PATH default search all stay the kernel's/libc's call.
fn spawn_error(cmd: &str, e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::NotFound => {
            format!("command not found: {cmd} (not an executable on PATH)")
        }
        std::io::ErrorKind::PermissionDenied => format!("cannot run {cmd}: permission denied"),
        _ => format!("failed to launch {cmd}: {e}"),
    }
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
    // Per-tile `[launch_overrides]` (config.rs): extra argv is appended BEFORE the BROWSER
    // token is processed, so override args on a browser tile go through the same URL-only
    // guard as everything else; env is applied at spawn below.
    let overrides = id
        .as_deref()
        .and_then(|i| config::load_or_create().launch_overrides.get(i).cloned());
    if let Some(ov) = &overrides {
        exec.extend(ov.args.iter().cloned());
        if !ov.args.is_empty() || !ov.env.is_empty() {
            tracing::info!(
                "launch_overrides[{}]: +{} arg(s), {} env var(s)",
                id.as_deref().unwrap_or(""),
                ov.args.len(),
                ov.env.len()
            );
        }
    }
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
    // Per-tile env AFTER the session defaults above, so an override can also retarget
    // XDG_CURRENT_DESKTOP/QT_QPA_PLATFORMTHEME for a stubborn app.
    if let Some(ov) = &overrides {
        for (k, v) in &ov.env {
            command.env(k, v);
        }
    }
    let child = command.spawn().map_err(|e| spawn_error(cmd, &e))?;
    watchdog::watch_child(app, child, name.unwrap_or_else(|| cmd.clone()), id);
    Ok(())
}

#[tauri::command]
pub fn get_catalog() -> Vec<apps::App> {
    apps::catalog()
}

#[tauri::command]
pub async fn save_apps(apps: Vec<apps::App>) -> Result<(), String> {
    blocking(move || config::save_apps(apps)).await?
}

#[tauri::command]
pub async fn save_favorites(favorites: Vec<String>) -> Result<(), String> {
    blocking(move || config::save_favorites(favorites)).await?
}

#[tauri::command]
pub async fn save_recent_apps(recent_apps: Vec<String>) -> Result<(), String> {
    blocking(move || config::save_recent_apps(recent_apps)).await?
}

/// Open Steam's per-game Properties dialog for the focused game.
#[tauri::command]
pub fn game_properties(appid: String) -> Result<(), String> {
    std::process::Command::new("steam")
        .arg(format!("steam://gameproperties/{appid}"))
        .spawn()
        .map(|_| ())
        .map_err(|e| spawn_error("steam", &e))
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

/// Switch between OmniDeck and a launched app without closing it. With a launch `id` (a Now
/// Playing card's entry), brings THAT app's group forward like a deck card — the global
/// toggle re-mapped EVERY hidden app at once, so a per-app ⇄ button surfaced them all. A
/// STALE id (the app just exited, its card not yet removed) is a no-op for the same reason:
/// falling through to the toggle would surface every hidden app. Only an ABSENT id (legacy
/// callers) means the global toggle.
#[tauri::command]
pub async fn switch_app(id: Option<String>) -> bool {
    blocking(move || match id.as_deref() {
        Some(key) => watchdog::group_of_id(key).map(crate::switcher::show_group).unwrap_or(false),
        None => crate::switcher::toggle().is_some(),
    })
    .await
    .unwrap_or(false)
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

// --- Sleep timer (sleep_timer.rs) ---

/// Arm the sleep timer: pause playback in `minutes`. Re-setting REPLACES a running timer.
/// Deliberately not persisted across restarts (see sleep_timer.rs). Returns the initial
/// status so the UI can render the countdown without a second round-trip.
#[tauri::command]
pub fn set_sleep_timer(app: tauri::AppHandle, minutes: u32) -> Result<crate::sleep_timer::SleepTimerStatus, String> {
    crate::sleep_timer::set(app, minutes)
}

/// Cancel the sleep timer; false when none was armed (idempotent).
#[tauri::command]
pub fn cancel_sleep_timer() -> bool {
    crate::sleep_timer::cancel()
}

/// Remaining/total seconds of the armed timer, or None. The frontend calls this once at
/// mount (before its `sleep-timer-tick` listener attaches), then relies on events.
#[tauri::command]
pub fn get_sleep_timer() -> Option<crate::sleep_timer::SleepTimerStatus> {
    crate::sleep_timer::get()
}

#[cfg(test)]
mod tests {
    use super::{is_safe_browser_arg, spawn_error};

    #[test]
    fn spawn_errors_map_to_clear_messages() {
        // Drive real spawns so the io::Error kinds are the ones execvp actually produces.
        let cmd = "omnideck-test-no-such-binary";
        let e = std::process::Command::new(cmd).spawn().unwrap_err();
        assert_eq!(
            spawn_error(cmd, &e),
            format!("command not found: {cmd} (not an executable on PATH)")
        );

        // A file that exists but isn't executable -> permission denied, not "not found".
        let plain = std::env::temp_dir()
            .join(format!("omnideck-test-notexec-{}", std::process::id()));
        std::fs::write(&plain, "data").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644)).unwrap();
        let cmd = plain.to_string_lossy();
        let e = std::process::Command::new(plain.as_os_str()).spawn().unwrap_err();
        assert_eq!(spawn_error(&cmd, &e), format!("cannot run {cmd}: permission denied"));
        let _ = std::fs::remove_file(&plain);
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
