// OmniDeck — test-only control channel for the nested-session harness
// (packaging/test-session.sh).
//
// Entirely inert unless OMNIDECK_TEST_CONTROL names a FIFO at startup. When set, a thread
// reads newline-delimited commands from it:
//
//   launch <argv...>   spawn a program through the REAL launch path (commands::launch_command),
//                      so the watchdog owns its process group and the switcher/hotkeys treat
//                      it exactly like a user-launched app — that ownership chain is the thing
//                      the harness exists to test. argv is whitespace-split, no shell.
//   quit               exit OmniDeck (same as the power-menu Exit).
//
// Same-user trust domain only: the harness creates the FIFO 0600 in $XDG_RUNTIME_DIR and
// sets the env var for the one process it spawns. Never set in a real session.
use std::io::BufRead;

pub fn spawn_if_enabled(app: tauri::AppHandle) {
    let Some(path) = std::env::var_os("OMNIDECK_TEST_CONTROL") else { return };
    tracing::warn!("testhook: control channel ENABLED at {path:?} (test builds only)");
    std::thread::spawn(move || loop {
        // Opening a FIFO read-only blocks until a writer connects; EOF when it closes.
        // Reopen per writer so the harness can `echo cmd > fifo` repeatedly.
        let file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("testhook: open {path:?}: {e} — channel closed");
                return;
            }
        };
        for line in std::io::BufReader::new(file).lines() {
            let Ok(line) = line else { break };
            let mut words = line.split_whitespace().map(str::to_string);
            match words.next().as_deref() {
                Some("launch") => {
                    let exec: Vec<String> = words.collect();
                    let name = exec.first().cloned();
                    match crate::commands::launch_command(app.clone(), exec, name, None) {
                        Ok(()) => tracing::info!("testhook: launched"),
                        Err(e) => tracing::error!("testhook: launch failed: {e}"),
                    }
                }
                Some("quit") => crate::commands::quit(app.clone()),
                Some(other) => tracing::warn!("testhook: unknown command {other:?}"),
                None => {}
            }
        }
    });
}
