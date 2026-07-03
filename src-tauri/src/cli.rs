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
    }
    true
}
