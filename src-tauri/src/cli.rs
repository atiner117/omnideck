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
