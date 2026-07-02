// OmniDeck — backend entrypoint: CLI dispatch, GPU env re-exec, and the Tauri builder.
//
// Module map:
//   commands   — the #[tauri::command] IPC surface (mirrored by src/lib/backend.ts)
//   gamepad    — gilrs input thread (evdev → typed Tauri events)
//   watchdog   — launch/exit tracking + gamescope focus return (STEAM_GAME atom)
//   gpu        — NVIDIA/WebKitGTK env re-exec (session-specific workarounds)
//   cli        — headless debug subcommands (probe/scan/config/gridart/catalog/media)
//   mpris      — event-driven Now Playing over the session bus
//   asset      — the omnideck:// protocol serving on-disk art
//   apps, capability, config, http, icons, library, steamgriddb — domain modules
mod apps;
mod asset;
mod capability;
mod cli;
mod commands;
mod config;
mod gamepad;
mod gpu;
mod hotkey;
mod http;
mod icons;
mod library;
mod mpris;
mod steamgriddb;
mod switcher;
mod watchdog;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Headless subcommands (parsed before the GPU re-exec); no subcommand = launch the GUI.
    if cli::handle() {
        return;
    }

    // Re-exec once with GPU-appropriate webview env (NVIDIA needs workarounds; Mesa doesn't).
    gpu::ensure_gpu_env();

    tauri::Builder::default()
        // omnideck:// serves on-disk art files (Steam librarycache + our art cache) as URLs
        // instead of base64 data URLs pinned in reactive state — see asset.rs / NOTES-PERFORMANCE.
        .register_asynchronous_uri_scheme_protocol("omnideck", |_ctx, request, responder| {
            let path = request.uri().path().to_string();
            // blocking pool (reused threads) — a fast scroll fires dozens of these at once
            drop(tauri::async_runtime::spawn_blocking(move || responder.respond(asset::respond(&path))));
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_capability,
            commands::get_library,
            commands::get_art,
            commands::launch_game,
            commands::get_apps,
            commands::get_config,
            commands::save_settings,
            commands::launch_command,
            commands::grid_art,
            commands::get_catalog,
            commands::save_apps,
            commands::save_favorites,
            commands::save_recent_apps,
            commands::game_properties,
            commands::media_now_playing,
            commands::media_control,
            commands::quit,
            commands::close_current_app,
            commands::in_gamescope_session,
            commands::power_action,
            commands::app_icon
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || gamepad::gamepad_loop(handle));
            // Event-driven Now Playing: one session-bus watcher pushes `media-changed` events.
            tauri::async_runtime::spawn(mpris::watch(app.handle().clone()));
            // Session-only: global Ctrl+Alt+Home returns home while a launched app has focus.
            hotkey::spawn_if_session(app.handle().clone());
            watchdog::set_steam_game_atom_if_gamescope();
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
        .expect("error while running tauri application")
}
