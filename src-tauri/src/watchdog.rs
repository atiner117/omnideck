// OmniDeck — launch/exit tracking + focus return.
// Everything about knowing what the user launched and getting back to OmniDeck afterwards:
// the "current child" (PWAs/native apps we spawned), the Steam exit watchdog (Steam's URI
// handler returns immediately, so we poll registry.vdf), and the STEAM_GAME atom that drives
// gamescope's focus-return path.
use std::sync::Mutex;
use tauri::Emitter;

/// A still-running launched app: its process-group leader pid (each launch is its own group
/// leader via process_group(0)) plus the label/id the deck switcher shows on its card.
#[derive(Clone, serde::Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct LiveApp {
    pub group: u32,
    pub name: String,
    #[cfg_attr(test, ts(optional = nullable))]
    pub id: Option<String>,
}

/// ALL still-running launched apps. The switcher matches session windows to these groups
/// (via the pids) to know which windows belong to launched apps (vs OmniDeck itself or
/// gamescope's own), the deck switcher lists them as cards, and `return_home` signals every
/// one. Inside gamescope a launched window stacks on top of us with no other way back;
/// Steam games use a separate path (gamescope refocuses us when the game exits).
static LIVE_GROUPS: Mutex<Vec<LiveApp>> = Mutex::new(Vec::new());

/// Process-group ids of the launched apps still alive (switcher window-ownership matching).
pub fn live_groups() -> Vec<u32> {
    crate::sync::lock_or_recover(&LIVE_GROUPS, "watchdog.LIVE_GROUPS")
        .iter()
        .map(|a| a.group)
        .collect()
}

/// Full snapshot of the live launched apps (the deck switcher's cards).
pub fn live_apps() -> Vec<LiveApp> {
    crate::sync::lock_or_recover(&LIVE_GROUPS, "watchdog.LIVE_GROUPS").clone()
}

/// The live group whose launch id is `id` — lets a Now Playing card's per-app actions
/// route through the deck primitives (show THIS group) instead of the global toggle.
pub fn group_of_id(id: &str) -> Option<u32> {
    crate::sync::lock_or_recover(&LIVE_GROUPS, "watchdog.LIVE_GROUPS")
        .iter()
        .find(|a| a.id.as_deref() == Some(id))
        .map(|a| a.group)
}

/// Close EVERY still-running launched app so gamescope refocuses OmniDeck. Deliberate
/// semantics: "close" is the console-style escape hatch — the user wants their launcher
/// back, and with app B stacked over a still-running app A, closing only the newest (the
/// old most-recent-child behavior) left A holding the screen with the button apparently
/// dead. The switcher is the tool for keeping apps alive; close closes.
///
/// Best-effort SIGTERM per group; each child's `watch_child` thread reaps its own exit and
/// emits `app-exited`. Returns true if a signal reached anything.
pub fn return_home() -> bool {
    let groups = live_groups();
    let mut any = false;
    for pid in groups {
        any |= signal_group(pid);
    }
    // Only report success if a signal actually reached something — otherwise the caller would
    // emit "app-closed" / swallow the Guide press while the window is still on screen.
    any
}

/// Close a single launched app group (the deck switcher's per-card close). Same CONT-then-
/// TERM discipline as return_home; the child's watch_child thread reaps + emits app-exited.
pub fn close_group(group: u32) -> bool {
    let ok = signal_group(group);
    // The group is going away — its pgid must not linger in the switcher's freeze list,
    // where the exit hook's blanket SIGCONT could later hit a recycled pgid.
    crate::switcher::forget_stopped(group);
    ok
}

/// SIGTERM a whole process group (CONT first so a switcher-frozen group can act on it).
/// Browsers fork a persistent main process, so signalling the GROUP (-pid) reaches every
/// helper; fall back to the bare pid.
fn signal_group(pid: u32) -> bool {
    let grp = format!("-{pid}");
    let _ = std::process::Command::new("kill").args(["-CONT", &grp]).status();
    let grp_ok = std::process::Command::new("kill").args(["-TERM", &grp]).status().map(|s| s.success()).unwrap_or(false);
    let pid_ok = std::process::Command::new("kill").args(["-TERM", &pid.to_string()]).status().map(|s| s.success()).unwrap_or(false);
    grp_ok || pid_ok
}

/// Emit a launched event, then watch the child and emit an exited event when it ends.
/// (Lets the UI show a "now playing" state and know when focus returns.)
pub fn watch_child(app: tauri::AppHandle, mut child: std::process::Child, name: String, id: Option<String>) {
    let pid = child.id();
    crate::sync::lock_or_recover(&LIVE_GROUPS, "watchdog.LIVE_GROUPS").push(LiveApp {
        group: pid,
        name: name.clone(),
        id: id.clone(),
    });
    // The frontend correlates Now Playing entries by this launch id (the tile id), falling back
    // to the name for any legacy caller, so two same-named launchables don't clobber on exit.
    let exit_key = id.unwrap_or_else(|| name.clone());
    let _ = app.emit("app-launched", name);
    std::thread::spawn(move || {
        let _ = child.wait();
        // Clear only if a newer launch hasn't already replaced us as the current app.
        crate::sync::lock_or_recover(&LIVE_GROUPS, "watchdog.LIVE_GROUPS").retain(|a| a.group != pid);
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
pub fn set_steam_game_atom_if_gamescope() {
    // Only relevant inside a gamescope (steamcompmgr) session.
    if !crate::session::in_session() {
        return;
    }
    std::thread::spawn(|| {
        if std::process::Command::new("xprop").arg("-version").output().is_err() {
            tracing::warn!(
                "`xprop` not found — install `xorg-xprop`, or the gamescope session may be a \
                 black screen (cannot set the STEAM_GAME atom)."
            );
            return;
        }
        // The window appears a moment after the webview initializes; retry for ~12s.
        for attempt in 1..=40 {
            std::thread::sleep(std::time::Duration::from_millis(300));
            if stamp_steam_atom_once() {
                tracing::info!("STEAM_GAME=769 set on window (attempt {attempt})");
                return;
            }
        }
        tracing::warn!(
            "could not set STEAM_GAME after 40 tries (window not found by name 'omnideck'). \
             If the session is black, set it manually — see packaging/M2-SESSION-TEST.md."
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
pub fn watch_steam_game(app: tauri::AppHandle, appid: String, name: String, id: Option<String>) {
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
            tracing::warn!("watchdog: '{name}' never reported running; giving up");
            let _ = app.emit("app-exited", exit_key);
            return;
        }
        tracing::info!("watchdog: '{name}' is running");
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
                        tracing::warn!("watchdog: '{name}' state unknown for ~15 min; giving up");
                        break;
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(1000));
        }
        tracing::info!("watchdog: '{name}' exited — refocusing OmniDeck");
        let _ = app.emit("app-exited", exit_key);
        // Best-effort focus recovery in a gamescope session.
        if crate::session::in_session() {
            stamp_steam_atom_once();
        }
    });
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
}
