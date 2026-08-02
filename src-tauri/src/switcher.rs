// OmniDeck — session app switcher: hide/show launched apps instead of killing them.
//
// Gamescope (steamcompmgr) ignores `_NET_ACTIVE_WINDOW` and the `GAMESCOPECTRL_BASELAYER_APPID`
// root property for plain (non-STEAM_GAME) windows — verified live on the M2 host — but its
// focus does follow window *mapping*: unmap the launched app's toplevels and focus falls back
// to OmniDeck; map them again and the newest window retakes focus. So the switcher primitive
// is unmap/show: the app's process keeps running (audio keeps playing — hide YouTube Music,
// browse the dashboard, bring it back), which is what "switch" should mean on a console.
//
// Refinement (couch test 2026-07-09): "keeps running" is only a feature while you can HEAR
// it. A hidden app that is silent — a PWA still spinning a software renderer, a paused
// video — kept burning real watts (~300 W measured) behind the dashboard. So on hide, any
// hidden process group WITHOUT an active (uncorked) audio stream is SIGSTOPped, and every
// stopped group is SIGCONTed on re-show; return_home() CONTs before TERM so close works on
// frozen groups too. Music apps are never frozen — the audio check is the policy.
//
// Ownership: only windows whose _NET_WM_PID belongs to one of our launched process groups
// (watchdog::live_groups; every launch is a group leader) are ever touched — never OmniDeck's
// own window, gamescope's internals, or a Steam game's (Steam has gamescope's native
// focus-return path).
use std::sync::Mutex;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, MapState, Window};
use x11rb::rust_connection::RustConnection;

/// Windows we unmapped on the last "hide" — remapped on the next toggle.
static HIDDEN: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// Shared X11 connection for the polled entry points. The navpad's activation gate calls
/// `any_app_visible` ~3×/s from the gamepad thread; opening a fresh connection per check
/// (socket + auth + setup exchange, a new FD each time) was pure churn. Cached here and
/// reused by every switcher entry point; a cheap liveness round-trip on reuse reconnects
/// transparently when the server went away (session teardown, desktop test restarts).
/// The hotkey thread deliberately keeps its OWN connection — it parks in `wait_for_event`,
/// which would wedge anything sharing it.
static X11: Mutex<Option<(RustConnection, usize)>> = Mutex::new(None);

/// Run `f` against the shared connection's root window, (re)connecting as needed.
/// Returns None only when X is unreachable.
fn with_x11<T>(f: impl FnOnce(&RustConnection, Window) -> T) -> Option<T> {
    let mut guard = crate::sync::lock_or_recover(&X11, "switcher.X11");
    if let Some((conn, _)) = guard.as_ref() {
        // One round-trip to prove the cached connection is still live — still far cheaper
        // than a full reconnect, and it turns a dead cache into a reconnect instead of
        // every request inside `f` silently failing forever.
        if !conn.get_input_focus().is_ok_and(|c| c.reply().is_ok()) {
            tracing::info!("switcher: shared X11 connection lost — reconnecting");
            *guard = None;
        }
    }
    if guard.is_none() {
        *guard = x11rb::connect(None).ok();
    }
    let (conn, screen_num) = guard.as_ref()?;
    let root = conn.setup().roots[*screen_num].root;
    Some(f(conn, root))
}

/// Process groups we froze (SIGSTOP) when their windows were hidden. Disjoint from any
/// group that was audibly playing at hide time. Drained + SIGCONTed on the next re-show.
static STOPPED: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// `(ppid, pgid)` for `pid` from /proc/<pid>/stat fields 4 and 5 (0s when gone/unreadable).
fn parent_and_pgid(pid: u32) -> (u32, u32) {
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else { return (0, 0) };
    // comm (field 2) can contain spaces/parens — split after the LAST ')'. After that the
    // remaining whitespace fields are: state(0) ppid(1) pgrp(2) ...
    let Some(rest) = stat.rsplit_once(')').map(|(_, r)| r) else { return (0, 0) };
    let mut it = rest.split_whitespace();
    let ppid = it.nth(1).and_then(|s| s.parse().ok()).unwrap_or(0); // skip state, take ppid
    let pgid = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (ppid, pgid)
}

/// Process-group id for `pid` (0 when gone/unreadable).
fn pgid_of(pid: u32) -> u32 {
    parent_and_pgid(pid).1
}

/// Signal an entire process group (`kill <sig> -<pgid>`). True when the kill succeeded.
/// Every freeze/thaw path goes through these — one place to change the mechanism.
fn signal_pgroup(group: u32, sig: &str) -> bool {
    std::process::Command::new("kill")
        .args([sig, &format!("-{group}")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// SIGCONT a frozen process group (dead groups just fail the kill — harmless).
fn cont_group(group: u32) -> bool {
    signal_pgroup(group, "-CONT")
}

/// SIGSTOP a process group. True only when the freeze actually landed.
fn stop_group(group: u32) -> bool {
    signal_pgroup(group, "-STOP")
}

/// Forget a group we froze (it was closed/killed): a dead pgid must not linger in STOPPED,
/// where the exit hook's blanket SIGCONT could one day hit a recycled pgid.
pub(crate) fn forget_stopped(group: u32) {
    crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").retain(|&g| g != group);
}

/// Which of `groups` owns `pid`, matching its process group OR any ANCESTOR's — Electron
/// apps (Feishin) run their audio in a child that `setsid`s into its own group, so an exact
/// pgid match misses it and the app looks silent. Walks up to 16 parents (cycle/runaway
/// guard). Returns the owning group, or None.
fn owning_group(mut pid: u32, groups: &[u32]) -> Option<u32> {
    for _ in 0..16 {
        if pid <= 1 {
            break;
        }
        let (ppid, pgid) = parent_and_pgid(pid);
        if groups.contains(&pid) {
            return Some(pid);
        }
        if groups.contains(&pgid) {
            return Some(pgid);
        }
        pid = ppid;
    }
    None
}

/// The launched apps' currently-viewable toplevels as `(window, process group)`.
fn visible_owned(
    conn: &x11rb::rust_connection::RustConnection,
    root: Window,
    groups: &[u32],
) -> Vec<(Window, u32)> {
    let Ok(Ok(net_wm_pid)) = conn.intern_atom(false, b"_NET_WM_PID").map(|c| c.reply()) else {
        return Vec::new();
    };
    let net_wm_pid = net_wm_pid.atom;
    let Ok(Ok(tree)) = conn.query_tree(root).map(|c| c.reply()) else { return Vec::new() };
    let mut visible = Vec::new();
    for &win in &tree.children {
        let Ok(Ok(attrs)) = conn.get_window_attributes(win).map(|c| c.reply()) else { continue };
        if attrs.map_state != MapState::VIEWABLE {
            continue;
        }
        let Ok(Ok(prop)) = conn
            .get_property(false, win, net_wm_pid, AtomEnum::CARDINAL, 0, 1)
            .map(|c| c.reply())
        else {
            continue;
        };
        let Some(pid) = prop.value32().and_then(|mut v| v.next()) else { continue };
        let pgid = pgid_of(pid);
        if groups.contains(&pgid) {
            visible.push((win, pgid));
        }
    }
    visible
}

/// True when a launched app's window is currently in front (viewable). The navpad uses
/// this as its activation gate: gamescope focuses whatever is mapped on top, so "an owned
/// window is viewable" is exactly "the controller's input should go to the app".
pub fn any_app_visible() -> bool {
    // The navpad polls this ~3x/s for the whole session; with nothing launched the answer
    // is trivially false — don't touch X at all to say so (the pooled connection stays idle).
    let groups = crate::watchdog::live_groups();
    if groups.is_empty() {
        return false;
    }
    with_x11(|conn, root| !visible_owned(conn, root, &groups).is_empty()).unwrap_or(false)
}

/// Toggle the launched app(s): if any owned window is visible, hide them all (focus falls
/// back to OmniDeck); else re-show whatever the last toggle hid. Returns a short description
/// of what happened, or None if there was nothing to act on.
pub fn toggle() -> Option<&'static str> {
    // Session-only, enforced at the chokepoint for every caller (UI command, Guide press,
    // hotkey): on a desktop, unmapping would hide the app's window from the real WM, which
    // has its own idea of window management. (OMNIDECK_FORCE_HOTKEY tests on desktop X11.)
    if !session_ok() {
        return None;
    }
    with_x11(toggle_inner).flatten()
}

fn toggle_inner(conn: &RustConnection, root: Window) -> Option<&'static str> {
    let groups = crate::watchdog::live_groups();
    let visible = visible_owned(conn, root, &groups);

    if !visible.is_empty() {
        // Hide: unmap every owned visible toplevel; gamescope refocuses OmniDeck.
        let wins: Vec<Window> = visible.iter().map(|&(w, _)| w).collect();
        let failed = set_mapped(conn, &wins, false);
        if !failed.is_empty() {
            tracing::warn!("switcher: {} window(s) resisted unmap", failed.len());
        }
        {
            let mut hidden = crate::sync::lock_or_recover(&HIDDEN, "switcher.HIDDEN");
            // APPEND (don't overwrite): an app launched while another was hidden must not
            // orphan the first one's windows — the next show brings the whole set back.
            for &(win, _) in &visible {
                if !hidden.contains(&win) && !failed.contains(&win) {
                    hidden.push(win);
                }
            }
        }
        freeze_silent_groups(&visible, &failed);
        return Some("hidden — OmniDeck focused");
    }

    // Nothing visible: continue every frozen group BEFORE mapping, so the re-shown
    // windows can actually repaint and take focus.
    resume_stopped_groups();

    // Re-show the set we hid (skip windows that died while hidden).
    let hidden: Vec<Window> =
        std::mem::take(&mut *crate::sync::lock_or_recover(&HIDDEN, "switcher.HIDDEN"));
    if hidden.is_empty() {
        return None;
    }
    let failed = set_mapped(conn, &hidden, true);
    if !failed.is_empty() {
        // Put the strays back so the next toggle retries instead of stranding the app
        // invisible with an empty HIDDEN list (Guide would then do nothing forever).
        tracing::warn!("switcher: {} window(s) did not remap — kept for retry", failed.len());
        crate::sync::lock_or_recover(&HIDDEN, "switcher.HIDDEN").extend(failed);
    }
    Some("re-shown — app focused")
}

/// Session gate shared by every window-touching entry point (toggle, deck, navpad, hotkey):
/// on a desktop, unmapping/injecting into foreign windows belongs to the real WM.
pub(crate) fn session_ok() -> bool {
    crate::session::in_session() || std::env::var_os("OMNIDECK_FORCE_HOTKEY").is_some()
}

/// What the most recent `hide_all()` actually did — the windows it unmapped and the groups
/// it froze — so dismissing the deck (rather than picking a card) can put exactly that state
/// back. Consumed by `deck_cancel`; cleared when a card is chosen instead.
static LAST_HIDE: Mutex<(Vec<Window>, Vec<u32>)> = Mutex::new((Vec::new(), Vec::new()));

/// Hide EVERY launched app so OmniDeck (and the deck overlay) is what's on screen — the
/// deck-switcher's "open" step. Same as toggle's hide half: unmap owned toplevels, remember
/// them, freeze the silent ones. Returns true if anything was hidden.
pub fn hide_all() -> bool {
    if !session_ok() {
        return false;
    }
    with_x11(|conn, root| {
        let visible = visible_owned(conn, root, &crate::watchdog::live_groups());
        if visible.is_empty() {
            return false;
        }
        let wins: Vec<Window> = visible.iter().map(|&(w, _)| w).collect();
        let failed = set_mapped(conn, &wins, false);
        {
            let mut hidden = crate::sync::lock_or_recover(&HIDDEN, "switcher.HIDDEN");
            for &(win, _) in &visible {
                if !hidden.contains(&win) && !failed.contains(&win) {
                    hidden.push(win);
                }
            }
        }
        let frozen_now = freeze_silent_groups(&visible, &failed);
        // Snapshot THIS hide's effect (not the whole HIDDEN/STOPPED backlog): a deck dismissed
        // without picking a card restores exactly what opening it took away — apps hidden on an
        // earlier deck round stay hidden.
        let hidden_now: Vec<Window> =
            visible.iter().map(|&(w, _)| w).filter(|w| !failed.contains(w)).collect();
        *crate::sync::lock_or_recover(&LAST_HIDE, "switcher.LAST_HIDE") = (hidden_now, frozen_now);
        true
    })
    .unwrap_or(false)
}

/// Undo the last `hide_all()` (deck dismissed without picking a card): SIGCONT the groups
/// that hide froze, then remap the windows it unmapped — the app that was in front comes
/// back. Without this, a Guide tap followed by a second tap / B / scrim click stranded the
/// foreground app invisible and (if silent) SIGSTOPped, with no way back but the deck.
pub fn deck_cancel() -> bool {
    if !session_ok() {
        return false;
    }
    // Connection established FIRST, like every sibling (with_x11 proves it before the
    // closure runs): failing after the snapshot was taken and the groups thawed would
    // strand the windows unmapped with the restore state already destroyed.
    with_x11(|conn, _root| {
        let (wins, groups) = std::mem::take(&mut *crate::sync::lock_or_recover(
            &LAST_HIDE,
            "switcher.LAST_HIDE",
        ));
        if wins.is_empty() && groups.is_empty() {
            return false;
        }
        // Resume before mapping, same as toggle: the windows must be able to repaint/take focus.
        for g in &groups {
            cont_group(*g);
        }
        crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").retain(|g| !groups.contains(g));
        let failed = set_mapped(conn, &wins, true);
        crate::sync::lock_or_recover(&HIDDEN, "switcher.HIDDEN")
            .retain(|w| !wins.contains(w) || failed.contains(w));
        true
    })
    .unwrap_or(false)
}

/// Bring ONE launched app group to the front (the deck-switcher's "open this card"): map
/// its toplevels, then resume it if frozen. Other apps stay hidden. Returns true on success.
pub fn show_group(group: u32) -> bool {
    if !session_ok() {
        return false;
    }
    with_x11(|conn, root| {
        // Find the windows FIRST: if the app has none left (died while frozen, transient X
        // failure), leave its STOPPED entry alone — thawing before this check left the group
        // running invisibly with no record to ever re-freeze it.
        let wins = windows_of_group(conn, root, group);
        if wins.is_empty() {
            return false;
        }

        // Map BEFORE thawing. The map requests come from OUR connection (steamcompmgr does the
        // actual mapping), so a SIGSTOPped client doesn't block them — and if every map fails
        // (wedged compositor), the group must stay frozen with its STOPPED entry and the deck's
        // dismiss snapshot intact, not thawed-and-invisible with no record to re-freeze it.
        let failed = set_mapped(conn, &wins, true);
        if failed.len() == wins.len() {
            return false;
        }

        // At least one window is up — resume the group so it can repaint and take focus.
        cont_group(group);
        crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").retain(|&g| g != group);
        // A card was chosen — the deck's dismiss snapshot no longer applies.
        *crate::sync::lock_or_recover(&LAST_HIDE, "switcher.LAST_HIDE") = (Vec::new(), Vec::new());
        // Drop the now-shown windows from the hidden set (keep any that failed to map for retry).
        crate::sync::lock_or_recover(&HIDDEN, "switcher.HIDDEN")
            .retain(|w| !wins.contains(w) || failed.contains(w));
        true
    })
    .unwrap_or(false)
}

/// All toplevels (any map state) whose _NET_WM_PID belongs to `group` — used to re-map a
/// specific app's windows after they were unmapped (they're not VIEWABLE, so visible_owned
/// can't find them).
fn windows_of_group(
    conn: &x11rb::rust_connection::RustConnection,
    root: Window,
    group: u32,
) -> Vec<Window> {
    let Ok(Ok(net_wm_pid)) = conn.intern_atom(false, b"_NET_WM_PID").map(|c| c.reply()) else {
        return Vec::new();
    };
    let net_wm_pid = net_wm_pid.atom;
    let Ok(Ok(tree)) = conn.query_tree(root).map(|c| c.reply()) else { return Vec::new() };
    let mut out = Vec::new();
    for &win in &tree.children {
        let Ok(Ok(prop)) = conn
            .get_property(false, win, net_wm_pid, AtomEnum::CARDINAL, 0, 1)
            .map(|c| c.reply())
        else {
            continue;
        };
        if let Some(pid) = prop.value32().and_then(|mut v| v.next()) {
            if owning_group(pid, &[group]).is_some() {
                out.push(win);
            }
        }
    }
    out
}

/// SIGSTOP every just-hidden process group that is NOT audibly playing (see header:
/// background music is a feature; a silent hidden renderer is a space heater). Windows
/// that resisted unmap keep their group running — a still-visible window must not freeze.
/// Returns the groups frozen by THIS call (the deck's dismiss snapshot).
fn freeze_silent_groups(visible: &[(Window, u32)], failed: &[Window]) -> Vec<u32> {
    // Exclude the WHOLE group when ANY of its windows resisted unmap — filtering per-window
    // let a two-window group freeze via its unmapped sibling while the resister stayed on
    // screen as a frozen, input-dead window.
    let failed_groups: Vec<u32> =
        visible.iter().filter(|(w, _)| failed.contains(w)).map(|&(_, g)| g).collect();
    let mut candidates: Vec<u32> = visible
        .iter()
        .map(|&(_, g)| g)
        .filter(|g| !failed_groups.contains(g))
        .collect();
    candidates.sort_unstable();
    candidates.dedup();
    let mut frozen = Vec::new();
    if candidates.is_empty() {
        return frozen;
    }
    let audible = audible_groups(&candidates);
    let mut stopped = crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED");
    for g in candidates {
        if audible.contains(&g) {
            tracing::info!("switcher: hidden group {g} is playing audio — left running");
            continue;
        }
        if stopped.contains(&g) {
            continue;
        }
        if stop_group(g) {
            tracing::info!("switcher: froze silent hidden group {g}");
            stopped.push(g);
            frozen.push(g);
        }
    }
    frozen
}

/// SIGCONT everything `freeze_silent_groups` stopped (dead groups just fail the kill).
/// Also the process-exit hook (lib.rs): frozen groups must not outlive the launcher —
/// SIGTERM can't wake a SIGSTOPped process, so exiting without this stranded them forever.
pub(crate) fn resume_stopped_groups() {
    let stopped: Vec<u32> =
        std::mem::take(&mut *crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED"));
    for g in stopped {
        cont_group(g);
    }
}

/// Which of `candidates` have an ACTIVE (uncorked) audio stream, via the PipeWire pulse
/// shim: `pactl list sink-inputs` blocks carrying `application.process.id` whose process
/// group matches. Fail-open — if pactl is missing or errors, every candidate counts as
/// audible: leaving a silent app running is recoverable, freezing the user's music is rude.
fn audible_groups(candidates: &[u32]) -> Vec<u32> {
    // Bounded: `.output()` alone waits forever, and a wedged pipewire-pulse would hang the
    // whole deck-open path (this runs on the Guide-tap flow) until pactl exited.
    let mut cmd = std::process::Command::new("pactl");
    cmd.args(["list", "sink-inputs"]).env("LC_ALL", "C");
    let out = match crate::proc::output_with_timeout(cmd, std::time::Duration::from_secs(3)) {
        Some(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => {
            tracing::info!("switcher: pactl unavailable — treating all hidden groups as audible");
            return candidates.to_vec();
        }
    };
    let mut audible = Vec::new();
    for block in out.split("Sink Input #").skip(1) {
        // "Corked: yes" = the stream exists but is paused — that's silence, freezable.
        if !block.contains("Corked: no") {
            continue;
        }
        let Some(pid) = block
            .split("application.process.id")
            .nth(1)
            .and_then(|r| r.split('"').nth(1))
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        // Match the stream's process to a launched group via ancestry (Electron audio is a
        // setsid'd child — an exact pgid check froze Feishin mid-song, the 2026-07-09 bug).
        if let Some(g) = owning_group(pid, candidates) {
            if !audible.contains(&g) {
                audible.push(g);
            }
        }
    }
    audible
}

/// Map or unmap `wins` and VERIFY each reached the requested state, re-issuing the request a
/// few times. Fire-and-forget is not enough: map/unmap of foreign toplevels is asynchronous
/// through steamcompmgr (maps are SubstructureRedirect'ed to it), and a request that lands
/// while it is still digesting the previous transition can get swallowed — seen live in the
/// nested-session harness as a re-shown window that never became viewable. Returns the
/// windows that never confirmed (destroyed windows are treated as done — they're gone).
fn set_mapped(
    conn: &x11rb::rust_connection::RustConnection,
    wins: &[Window],
    mapped: bool,
) -> Vec<Window> {
    let want = if mapped { MapState::VIEWABLE } else { MapState::UNMAPPED };
    let mut pending: Vec<Window> = wins.to_vec();
    for attempt in 0..8 {
        pending.retain(|&win| {
            match conn.get_window_attributes(win).map(|c| c.reply()) {
                Ok(Ok(attrs)) => {
                    // UNVIEWABLE counts as hidden too (mapped but ancestor unmapped).
                    !(attrs.map_state == want || (!mapped && attrs.map_state != MapState::VIEWABLE))
                }
                _ => false, // window is gone — nothing left to (un)map
            }
        });
        if pending.is_empty() {
            break;
        }
        for &win in &pending {
            let _ = if mapped { conn.map_window(win) } else { conn.unmap_window(win) };
        }
        let _ = conn.flush();
        // First pass sends the initial request immediately; later passes give steamcompmgr
        // time to process before re-checking.
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
    }
    // Every visibility change funnels through here, so this is THE place to drop navpad's
    // activation cache — a hand-placed call at each caller was one forgotten site away from
    // the stale-cache input bug the cache's generation counter exists to prevent.
    crate::navpad::invalidate();
    pending
}

#[cfg(test)]
mod tests {
    use super::pgid_of;

    #[test]
    fn pgid_of_self_is_nonzero_and_bogus_pid_is_zero() {
        assert_ne!(pgid_of(std::process::id()), 0);
        assert_eq!(pgid_of(0), 0); // /proc/0 never exists
    }
}
