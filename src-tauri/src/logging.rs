// OmniDeck — logging: stderr (what eprintln! gave us) PLUS a daily-rotating file under
// $XDG_STATE_HOME/omnideck (default ~/.local/state/omnideck/omnideck.<date>.log, 7 files
// kept). The file is the point: a gamescope session's stderr is buried in the display
// manager's session log and gone once you're staring at a black screen — both M2 debugging
// rounds started with SDDM log forensics. `RUST_LOG` filters both sinks (default `info`).
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub fn init() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    // XDG spec: empty counts as unset.
    let state_dir = std::env::var_os("XDG_STATE_HOME")
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/state")))
        .map(|p| p.join("omnideck"));

    // Best-effort: no writable state dir (weird sandbox, $HOME-less service) just means no
    // file log — never a reason to refuse to start.
    let file_layer = state_dir
        .and_then(|dir| {
            std::fs::create_dir_all(&dir).ok()?;
            tracing_appender::rolling::Builder::new()
                .rotation(tracing_appender::rolling::Rotation::DAILY)
                .filename_prefix("omnideck")
                .filename_suffix("log")
                .max_log_files(7)
                .build(&dir)
                .ok()
        })
        .map(|appender| tracing_subscriber::fmt::layer().with_writer(appender).with_ansi(false));

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        // No escape codes in piped/session logs (fmt doesn't auto-detect ttys).
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr()));

    tracing_subscriber::registry().with(filter).with(stderr_layer).with(file_layer).init();

    // Panics land in the file too (a session crash currently vanishes with the compositor).
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("panic: {info}");
        previous(info);
    }));
}
