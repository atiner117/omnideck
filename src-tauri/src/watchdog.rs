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
    /// Kernel start time of the group leader at launch (/proc/<pid>/stat field 22) — the
    /// identity every later signal is verified against, so a recycled pid/pgid can never
    /// be signalled (see group_verified). Internal bookkeeping: not part of the IPC shape.
    #[serde(skip)]
    #[cfg_attr(test, ts(skip))]
    pub starttime: u64,
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
    // ...and then the apps LIVE_GROUPS can no longer see. A process group outlives its leader,
    // so a frozen app whose leader has been reaped is absent from the loop above while its
    // members are still SIGSTOPped behind an unmapped window — the loop "closed everything"
    // and reached nobody. The switcher holds the per-member identity records that make
    // signalling those survivors verified rather than blind.
    any |= crate::switcher::close_stopped_groups();
    // ...and a running Steam game. Steam launches are never in LIVE_GROUPS (the `steam
    // steam://rungameid/…` leader exits at once; the game lives under Steam's own reaper), so
    // until 2026-09 a Guide hold over a Steam game closed nothing — the one app the couch
    // launches most was the one the close chord could not reach.
    any |= close_steam_games();
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

/// Kernel start time (clock ticks since boot) of `pid` from /proc/<pid>/stat field 22.
/// None when the process is gone or unreadable.
pub(crate) fn proc_start_time(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // comm (field 2) can contain spaces/parens — parse after the LAST ')'. The remaining
    // whitespace fields start at state (field 3), so starttime (field 22) is index 19.
    let rest = stat.rsplit_once(')')?.1;
    rest.split_whitespace().nth(19)?.parse().ok()
}

/// True while `group` is still the process group we launched: it is in LIVE_GROUPS and the
/// leader's kernel start time matches the one recorded at launch. This is the pid-recycling
/// guard — a window's stale _NET_WM_PID or a check-then-signal gap must never end in a
/// signal to a recycled pgid (that freezes/kills an unrelated process).
fn group_verified(group: u32) -> bool {
    if group == 0 {
        return false; // pgid 0 means "our own group" to kill(2) — never a valid target
    }
    let Some(recorded) = crate::sync::lock_or_recover(&LIVE_GROUPS, "watchdog.LIVE_GROUPS")
        .iter()
        .find(|a| a.group == group)
        .map(|a| a.starttime)
    else {
        return false;
    };
    match proc_start_time(group) {
        // 0 = start time was unreadable at launch (the leader raced away between spawn and the
        // /proc read). This one case degrades to a liveness check, so it is NOT recycle-proof;
        // the alternative — refusing every later signal — would leave that app un-closable by
        // the Guide button. Rare and deliberately chosen, not a gap the rest of the path shares.
        Some(now) => recorded == 0 || now == recorded,
        None => false, // leader gone: the group is dead (or the pid recycled) — never signal blind
    }
}

/// Signal ONE recorded process, verified against the kernel start time captured when we last
/// saw it. This is the per-member counterpart to `signal_group_verified`, and it exists for a
/// lifecycle the group-level check cannot serve: a process group outlives its leader. If the
/// switcher froze a group and the leader then exited and was reaped, `group_verified` fails
/// (no LIVE_GROUPS record, no readable leader) and every surviving member stays SIGSTOPped
/// forever — the launcher's own space-heater fix, stranded.
///
/// `(pid, starttime)` pairs recorded at freeze time are exactly as trustworthy as the leader's:
/// a recycled pid gets a different start time and is refused here, so this is a verified
/// signal, never a blind one.
pub(crate) fn signal_pid_verified(pid: u32, starttime: u64, sig: i32) -> bool {
    if pid <= 1 {
        return false; // 0 = our own group to kill(2), 1 = init — never valid targets
    }
    if proc_start_time(pid) != Some(starttime) {
        return false; // gone, or a different process wearing the same pid
    }
    unsafe { libc::kill(pid as i32, sig) == 0 }
}

/// Send `sig` to the whole process group iff it still verifies as ours. Direct kill(2) —
/// no fork+exec (this runs on the Guide-button path, which must not jank) — and
/// errno-accurate: only actual delivery counts as success (ESRCH = already gone,
/// EPERM = not ours; both report false).
pub(crate) fn signal_group_verified(group: u32, sig: i32) -> bool {
    if !group_verified(group) {
        return false;
    }
    // The verify-to-kill window is now microseconds and crosses no await/lock. Closing it
    // completely needs pidfd_send_signal, which has no process-GROUP form — accepted.
    unsafe { libc::kill(-(group as i32), sig) == 0 }
}

/// SIGTERM a whole process group (CONT first so a switcher-frozen group can act on it).
/// Browsers fork a persistent main process, so signalling the GROUP (-pid) reaches every
/// helper — and the leader IS the group (spawns use process_group(0)), so no bare-pid
/// fallback is needed. Both signals are identity-verified against the launch start time.
fn signal_group(pid: u32) -> bool {
    let _ = signal_group_verified(pid, libc::SIGCONT);
    signal_group_verified(pid, libc::SIGTERM)
}

/// Test-only stand-ins for the two halves of `watch_child`'s bookkeeping — registering a
/// launched group, and the reaper thread dropping it once the leader exits. They let the
/// freeze/thaw lifecycle be exercised against real processes without a `tauri::AppHandle`.
#[cfg(test)]
pub(crate) fn track_group_for_test(group: u32) {
    crate::sync::lock_or_recover(&LIVE_GROUPS, "watchdog.LIVE_GROUPS").push(LiveApp {
        group,
        name: "test".into(),
        id: None,
        starttime: proc_start_time(group).unwrap_or(0),
    });
}

#[cfg(test)]
pub(crate) fn forget_group_for_test(group: u32) {
    crate::sync::lock_or_recover(&LIVE_GROUPS, "watchdog.LIVE_GROUPS").retain(|a| a.group != group);
}

/// Emit a launched event, then watch the child and emit an exited event when it ends.
/// (Lets the UI show a "now playing" state and know when focus returns.)
pub fn watch_child(app: tauri::AppHandle, mut child: std::process::Child, name: String, id: Option<String>) {
    let pid = child.id();
    crate::sync::lock_or_recover(&LIVE_GROUPS, "watchdog.LIVE_GROUPS").push(LiveApp {
        group: pid,
        name: name.clone(),
        id: id.clone(),
        // Recorded before the child can exit-and-recycle: the identity every later
        // STOP/CONT/TERM is checked against (group_verified).
        starttime: proc_start_time(pid).unwrap_or(0),
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

/// Steam appids with a live launch watcher (launching or running). Held by
/// [`SteamWatchGuard`] for the watcher thread's lifetime.
static STEAM_WATCHED: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<String>>> =
    std::sync::LazyLock::new(Default::default);

/// Claim `appid` for one watcher; `None` when it is already launching/running. Dropping the
/// guard releases it, whichever way the watcher ends (started+exited, or gave up).
pub(crate) struct SteamWatchGuard(String);
impl SteamWatchGuard {
    pub(crate) fn claim(appid: &str) -> Option<Self> {
        STEAM_WATCHED
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(appid.to_string())
            .then(|| Self(appid.to_string()))
    }
}
impl Drop for SteamWatchGuard {
    fn drop(&mut self) {
        STEAM_WATCHED.lock().unwrap_or_else(|e| e.into_inner()).remove(&self.0);
    }
}

/// Does `cmdline` (a /proc/<pid>/cmdline, NUL-separated) belong to Steam's launch wrapper
/// for `appid`? Every Linux Steam launch since the 2022 client goes through
/// `ubuntu12_32/reaper SteamLaunch AppId=<appid> -- <game>`, and that reaper outlives the
/// game by design (it is the process Steam waits on), so its presence is the running state.
/// Exact-argument match: "AppId=570" must not hit "AppId=5700".
fn cmdline_is_steam_launch(cmdline: &[u8], appid: &str) -> bool {
    let want = format!("AppId={appid}");
    let mut args = cmdline.split(|b| *b == 0);
    while let Some(a) = args.next() {
        if a == b"SteamLaunch" {
            if args.next().map(|n| n == want.as_bytes()) != Some(true) {
                return false;
            }
            // Steam's install-script evaluator uses the SAME reaper shape with one extra
            // token — `SteamLaunch AppId=<id> Install=1 -- … iscriptevaluator.exe` — and runs
            // for a few seconds BEFORE the real launch. Couch box 2026-09-13: the watcher saw
            // it, then its absence, and declared the game exited 4 s before its exe started.
            return args.next().is_none_or(|x| x != b"Install=1");
        }
    }
    false
}

/// Is a `reaper SteamLaunch AppId=<appid>` process alive? Scans /proc; the registry.vdf
/// "Running" flag this used to rely on is no longer written by current Steam clients
/// (observed 2026-09-13: a 681-byte registry.vdf with no "apps" block at all), which made
/// every launch look like it never started.
fn steam_app_process_running(appid: &str) -> bool {
    let Ok(rd) = std::fs::read_dir("/proc") else { return false };
    rd.flatten().any(|e| {
        e.file_name().to_str().is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit()))
            && std::fs::read(e.path().join("cmdline"))
                .map(|c| cmdline_is_steam_launch(&c, appid))
                .unwrap_or(false)
    })
}

/// `(ppid, starttime)` from /proc/<pid>/stat — same field discipline as `proc_start_time`
/// (parse after the LAST ')' so a comm with spaces/parens can't shift the fields).
fn proc_ppid_and_start(pid: u32) -> Option<(u32, u64)> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rest = stat.rsplit_once(')')?.1;
    let mut f = rest.split_whitespace();
    let ppid = f.nth(1)?.parse().ok()?; // state, PPID
    let start = f.nth(17)?.parse().ok()?; // …, starttime (field 22)
    Some((ppid, start))
}

/// Every process under Steam's launch reaper(s) for `appid` — the reaper, the game, and
/// anything the game forked — **deepest first**, each with its kernel start time so the
/// caller can signal it recycle-safely via `signal_pid_verified`.
///
/// Deepest first matters: SIGTERM the reaper before the game and the game is reparented
/// to Steam (a subreaper) and keeps running with nothing left that knows it belongs to
/// this launch. One /proc walk builds the parent map; roots are the cmdline matches.
fn steam_app_process_tree(appid: &str) -> Vec<(u32, u64)> {
    let Ok(rd) = std::fs::read_dir("/proc") else { return Vec::new() };
    let mut roots = Vec::new();
    let mut children: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
    let mut start: std::collections::HashMap<u32, u64> = std::collections::HashMap::new();
    for e in rd.flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else { continue };
        let Some((ppid, st)) = proc_ppid_and_start(pid) else { continue };
        start.insert(pid, st);
        children.entry(ppid).or_default().push(pid);
        if std::fs::read(e.path().join("cmdline")).is_ok_and(|c| cmdline_is_steam_launch(&c, appid)) {
            roots.push(pid);
        }
    }
    // Breadth-first from the roots, then reversed: parents come before children in `order`,
    // so the reverse is deepest-first. A pid reached twice (impossible in a tree, cheap to
    // guard) is kept once.
    let mut order: Vec<u32> = Vec::new();
    let mut queue: std::collections::VecDeque<u32> = roots.into_iter().collect();
    while let Some(pid) = queue.pop_front() {
        if order.contains(&pid) {
            continue;
        }
        order.push(pid);
        if let Some(kids) = children.get(&pid) {
            queue.extend(kids.iter().copied());
        }
    }
    order.into_iter().rev().filter_map(|pid| start.get(&pid).map(|&st| (pid, st))).collect()
}

/// SIGTERM every Steam game a watcher currently tracks (see `watch_steam_game`): the whole
/// reaper subtree, deepest first, each pid verified against the start time read moments
/// ago. Returns true if any signal landed. The watcher then sees the reaper gone for three
/// polls and emits `app-exited` exactly as it does for a normal quit — no special path.
pub(crate) fn close_steam_games() -> bool {
    let watched: Vec<String> =
        STEAM_WATCHED.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect();
    let mut any = false;
    for appid in watched {
        let tree = steam_app_process_tree(&appid);
        if tree.is_empty() {
            continue; // still launching (no reaper yet) or already gone
        }
        tracing::info!(appid, procs = tree.len(), "watchdog: closing the Steam game's process tree");
        for (pid, st) in tree {
            any |= signal_pid_verified(pid, st, libc::SIGTERM);
        }
    }
    any
}

/// Exit watchdog for a Steam launch (M2): the `steam://` URI returns immediately, so we
/// poll for the game's `reaper SteamLaunch AppId=` process (registry.vdf's "Running" flag
/// as a fallback for older clients) — wait for it to appear (cold start can be slow), then
/// wait for it to go away — then tell the UI.
///
/// Focus return is normally AUTOMATIC: gamescope shows the window whose STEAM_GAME=769
/// ("main application") once a higher-priority game window is destroyed (per ChimeraOS
/// gamescope-session docs). So our window reappearing is gamescope's job, not ours. The
/// re-stamp below is a belt-and-suspenders no-op if the atom is still set; if M2 shows
/// gamescope NOT returning to us, the stronger lever is GAMESCOPECTRL_BASELAYER_APPID on
/// the root window (pins our appid as the base layer) — add that only if needed.
///
/// One watcher per appid: the guard is claimed here and released when the thread ends, so
/// a launch of an app that is already launching/running is a no-op for the caller
/// (`launch_game` checks [`SteamWatchGuard::claim`] BEFORE talking to Steam — a second
/// `steam://rungameid` only makes Steam pop "already running" and re-raise the game
/// mid-load, which wedged KH3 on a black frame on the couch box, 2026-09-13).
pub fn watch_steam_game(app: tauri::AppHandle, appid: String, name: String, id: Option<String>, guard: SteamWatchGuard) {
    let exit_key = id.unwrap_or_else(|| name.clone());
    std::thread::spawn(move || {
        let _guard = guard; // released when this thread returns, on every path below
        let reg = steam_registry_path();
        // Once the process has been seen, its absence is a confirmed exit — no registry
        // needed. Before that, fall back to the registry (None = unknown).
        let mut seen_process = false;
        let mut absent_polls = 0u32;
        let mut running = || -> Option<bool> {
            if steam_app_process_running(&appid) {
                seen_process = true;
                absent_polls = 0;
                return Some(true);
            }
            if seen_process {
                // One missed poll is not an exit (proc scan racing a Steam relaunch, a
                // reaper re-exec); three in a row is.
                absent_polls += 1;
                return Some(absent_polls < 3);
            }
            reg.as_deref()
                .and_then(|r| std::fs::read_to_string(r).ok())
                .and_then(|t| steam_app_running(&t, &appid))
        };
        // Phase 1: confirm it actually started (up to ~120s for a cold Steam + shader pre-cache).
        let mut started = false;
        for _ in 0..240 {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if running() == Some(true) {
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
            match running() {
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
    use super::{
        close_steam_games, cmdline_is_steam_launch, proc_start_time, steam_app_process_tree,
        steam_app_running, SteamWatchGuard,
    };
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

    // A real /proc/<pid>/cmdline of Steam's launch wrapper (r2d2, 2026-09-13).
    const REAPER: &[u8] = b"/home/atiner/.local/share/Steam/ubuntu12_32/reaper SteamLaunch AppId=2552450 -- /mnt/games/Steam/steamapps/common/SteamLinuxRuntime_4/_v2-entry-point ";

    #[test]
    fn reaper_cmdline_matches_exact_appid_only() {
        assert!(cmdline_is_steam_launch(REAPER, "2552450"));
        assert!(!cmdline_is_steam_launch(REAPER, "255245")); // prefix
        assert!(!cmdline_is_steam_launch(REAPER, "25524500")); // longer
        // The game binary itself is not the reaper.
        assert!(!cmdline_is_steam_launch(b"/games/KH3.exe -AppId=2552450 ", "2552450"));
        // SteamLaunch with nothing after it must not panic or match.
        assert!(!cmdline_is_steam_launch(b"reaper SteamLaunch ", "2552450"));
        // Steam's pre-launch install-script evaluator: same reaper, extra Install=1 token.
        assert!(!cmdline_is_steam_launch(
            b"reaper SteamLaunch AppId=2552450 Install=1 -- /x/iscriptevaluator.exe ",
            "2552450"
        ));
        assert!(cmdline_is_steam_launch(b"reaper SteamLaunch AppId=2552450 -- /x/game.exe ", "2552450"));
        assert!(!cmdline_is_steam_launch(b"", "2552450"));
    }

    #[test]
    fn one_watcher_per_appid_until_released() {
        let a = SteamWatchGuard::claim("900001").expect("first claim");
        assert!(SteamWatchGuard::claim("900001").is_none(), "second launch must be a no-op");
        assert!(SteamWatchGuard::claim("900002").is_some(), "other appids unaffected");
        drop(a);
        assert!(SteamWatchGuard::claim("900001").is_some(), "released when the watcher ends");
    }

    /// Guide hold must reach a Steam game (couch box 2026-09-15: it closed nothing). The
    /// stand-in for `reaper SteamLaunch AppId=<id> -- game` is `sh` carrying the marker in
    /// its argv as unused positional parameters, with a forked child like the real reaper.
    /// The tree must come back deepest first and the close must take BOTH processes.
    #[test]
    fn close_steam_games_terminates_the_whole_reaper_tree_deepest_first() {
        let appid = format!("9{}", std::process::id()); // unique per test process
        let _guard = SteamWatchGuard::claim(&appid).expect("appid not yet watched");
        // Two commands, so sh must stay resident as the parent (a lone `sleep` gets exec'd).
        let mut sh = std::process::Command::new("sh")
            .args(["-c", "sleep 30; sleep 30", "SteamLaunch", &format!("AppId={appid}")])
            .stdin(std::process::Stdio::null())
            .spawn()
            .expect("spawn sh");
        let wait_for = |mut f: Box<dyn FnMut() -> bool>| -> bool {
            for _ in 0..300 {
                if f() {
                    return true;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            false
        };
        let a = appid.clone();
        assert!(wait_for(Box::new(move || steam_app_process_tree(&a).len() >= 2)), "sh never forked sleep");
        let tree = steam_app_process_tree(&appid);
        assert_eq!(tree.len(), 2, "sh + its sleep child: {tree:?}");
        assert_eq!(tree[1].0, sh.id(), "the root (sh) must come LAST (deepest first)");
        assert_ne!(tree[0].0, sh.id());

        assert!(close_steam_games(), "no signal landed");
        // sh dies on SIGTERM (default action) — reap it so the pid can't linger as a zombie.
        assert!(wait_for(Box::new(move || sh.try_wait().ok().flatten().is_some())), "sh survived SIGTERM");
        let (kid, kid_start) = tree[0];
        assert!(
            wait_for(Box::new(move || proc_start_time(kid) != Some(kid_start))),
            "the forked sleep survived SIGTERM"
        );
        // Nothing tracked any more: a second close is a no-op, not a blind kill.
        assert!(!close_steam_games());
    }
}
