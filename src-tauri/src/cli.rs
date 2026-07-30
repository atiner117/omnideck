// OmniDeck — headless CLI surface (debug/inspection helpers, no window opens).
// With no subcommand, OmniDeck launches its GUI. Parsed BEFORE the GPU re-exec so a CLI
// invocation never triggers it. clap gives --version/--help and rejects unknown flags.
use clap::Parser;

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
    /// Snapshot MPRIS players on the session bus (what Now Playing would show)
    Media,
    /// Probe the configured media server (sections + first library's items)
    Mediasrv,
    /// Render + report the auto-generated mpv profile set (VapourSynth interpolation)
    Mpvprofiles,
    /// Downscale a wallpaper into the display-sized cache and report the result
    Bgprep {
        /// Path to the source image
        path: String,
    },
    /// One-shot support bundle: version, session, GPU/capability, config health,
    /// library/playback/controller status — paste this into a bug report
    Doctor,
}

/// The `doctor` support bundle. Offline on purpose (no update check, no media-server
/// round-trips — `omnideck mediasrv` covers those): a bug reporter shouldn't need working
/// networking to produce the bundle. Secrets never print — the config section reports
/// key/token PRESENCE, not values.
fn doctor() {
    println!("OmniDeck doctor — v{}", env!("CARGO_PKG_VERSION"));
    println!();

    // Session: the single biggest behavior fork in the app (fullscreen, hotkeys, navpad).
    println!("[session]");
    println!("  gamescope session: {}", crate::session::in_session());
    println!("  DISPLAY:           {}", std::env::var("DISPLAY").unwrap_or_else(|_| "(unset)".into()));
    println!("  XDG_SESSION_TYPE:  {}", std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "(unset)".into()));
    match crate::gpu::session_display_mode() {
        Some((w, h, hz)) => println!("  display mode:      {w}x{h} @ {hz:.1} Hz"),
        None => println!("  display mode:      unknown (RandR probe only works in-session)"),
    }
    println!();

    println!("[capability]");
    print!("{}", crate::capability::report(&crate::capability::probe()));
    println!();

    // Config health: path + parse state + a few load-bearing facts. Presence-only for
    // credentials — a doctor bundle gets pasted into public issues.
    let cfg = crate::config::load_or_create();
    println!("[config]");
    println!("  path:         {}", cfg.config_path);
    match &cfg.config_error {
        Some(e) => println!("  state:        BROKEN — {e}"),
        None => println!("  state:        ok"),
    }
    println!("  apps:         {} tile(s)", cfg.apps.len());
    println!("  favorites:    {}", cfg.favorites.len());
    println!("  steamgriddb:  {}", if cfg.settings.steamgriddb_key.is_empty() { "no key" } else { "key set" });
    println!(
        "  media server: {}",
        if crate::media_server::server().is_some() {
            "configured (config or adopted shim pairing)"
        } else {
            "not configured"
        }
    );
    println!();

    println!("[library]");
    println!("  steam games: {}", crate::library::scan().games.len());
    println!();

    // Playback stack: mpv presence decides direct-play; VapourSynth decides interpolation.
    println!("[playback]");
    println!(
        "  mpv:                 {}",
        if crate::apps::has_bin("mpv") { "found" } else { "MISSING — media tiles fall back to jellyfinmediaplayer" }
    );
    println!(
        "  jellyfinmediaplayer: {}",
        if crate::apps::has_bin("jellyfinmediaplayer") { "found" } else { "not installed" }
    );
    println!(
        "  mpv VapourSynth:     {}",
        if crate::media_profiles::vapoursynth_available() {
            "yes (auto-profiles eligible)"
        } else {
            "no (bare launch; install a VapourSynth-enabled mpv for interpolation)"
        }
    );
    println!();

    // Controllers, through the same gilrs path the app uses (evdev, no window needed).
    println!("[controllers]");
    match gilrs::Gilrs::new() {
        Ok(g) => {
            let pads: Vec<String> = g.gamepads().map(|(_, p)| p.name().to_string()).collect();
            if pads.is_empty() {
                println!("  none detected (check /dev/input permissions — user in `input` group?)");
            } else {
                for p in &pads {
                    println!("  · {p}");
                }
            }
        }
        Err(e) => println!("  gilrs init FAILED: {e}"),
    }
    println!();
    println!("(network probes are separate on purpose: `omnideck mediasrv` tests the media server)");
}

/// Parse argv and run a headless subcommand if one was given. Returns true when a subcommand
/// ran (the caller should exit instead of launching the GUI).
pub fn handle() -> bool {
    let Some(command) = Cli::parse().command else { return false };
    match command {
        CliCommand::Probe => {
            let cap = crate::capability::probe();
            print!("{}", crate::capability::report(&cap));
            println!(
                "\n--- json ---\n{}",
                serde_json::to_string_pretty(&cap).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
            );
        }
        CliCommand::Scan => {
            let lib = crate::library::scan();
            print!("{}", crate::library::report(&lib));
        }
        CliCommand::Config => {
            let cfg = crate::config::load_or_create();
            print!("{}", crate::config::report(&cfg));
        }
        CliCommand::Gridart { appid } => {
            let key = crate::config::load_or_create().settings.steamgriddb_key;
            if key.is_empty() {
                println!("gridart: no steamgriddb_key set in config.toml [settings]");
            } else {
                let got =
                    tauri::async_runtime::block_on(crate::steamgriddb::box_art(&appid, &key)).is_some();
                println!(
                    "gridart {appid}: {}",
                    if got { "OK (box art cached)" } else { "no result / network error" }
                );
            }
        }
        CliCommand::Catalog => {
            for a in crate::apps::catalog() {
                println!("{} {}  [{}]", a.icon, a.name, a.exec.join(" "));
            }
        }
        CliCommand::Media => {
            print!("{}", tauri::async_runtime::block_on(crate::mpris::report()));
        }
        CliCommand::Mpvprofiles => {
            print!("{}", crate::media_profiles::report());
        }
        CliCommand::Bgprep { path } => {
            let src = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            match crate::background::prepared(&path, None) {
                Some(out) => {
                    let dst = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
                    println!(
                        "bgprep OK: {} ({} KiB) -> {} ({} KiB)",
                        path, src / 1024, out.display(), dst / 1024
                    );
                }
                None => println!("bgprep FAILED for {path} (unreadable or undecodable)"),
            }
        }
        CliCommand::Doctor => doctor(),
        CliCommand::Mediasrv => {
            let Some(srv) = crate::media_server::server() else {
                println!("no media server configured (config [media_server] or shim pairing)");
                return true;
            };
            match tauri::async_runtime::block_on(srv.sections()) {
                Err(e) => println!("sections FAILED: {e}"),
                Ok(s) => {
                    println!("server: {}", s.server_name);
                    println!("resume: {} item(s)", s.resume.len());
                    for i in s.resume.iter().take(3) {
                        println!("  · {} [{}] {:.0}%", i.name, i.kind, i.played_pct.unwrap_or(0.0));
                    }
                    println!("latest: {} item(s)", s.latest.len());
                    for i in s.latest.iter().take(3) {
                        println!("  · {} [{}]", i.name, i.kind);
                    }
                    println!("libraries:");
                    for l in &s.libraries {
                        println!("  · {} ({}) id={}", l.name, l.kind, l.id);
                    }
                    if let Some(l) = s.libraries.first() {
                        match tauri::async_runtime::block_on(srv.browse(&l.id)) {
                            Err(e) => println!("browse({}) FAILED: {e}", l.name),
                            Ok(items) => {
                                println!("{}: {} item(s), first 5:", l.name, items.len());
                                for i in items.iter().take(5) {
                                    println!("  · {} [{}] {} min", i.name, i.kind, i.runtime_mins.unwrap_or(0));
                                }
                                if let Some(first) = items.first() {
                                    let p = tauri::async_runtime::block_on(srv.poster(&first.id));
                                    println!("poster({}) -> {:?}", first.name, p);
                                    // One byte of the direct stream proves the play path
                                    // without printing the tokened URL or downloading a movie.
                                    let status = tauri::async_runtime::block_on(async {
                                        crate::http::client()
                                            .get(srv.stream_url(&first.id))
                                            .header("Range", "bytes=0-0")
                                            .send()
                                            .await
                                            .map(|r| r.status().to_string())
                                    });
                                    println!("stream({}) -> {:?}", first.name, status);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    true
}
