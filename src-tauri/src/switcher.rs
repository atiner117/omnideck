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

/// Windows we unmapped on the last "hide" — remapped on the next toggle.
static HIDDEN: Mutex<Vec<u32>> = Mutex::new(Vec::new());

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
    let Ok((conn, screen_num)) = x11rb::connect(None) else { return false };
    let root = conn.setup().roots[screen_num].root;
    !visible_owned(&conn, root, &crate::watchdog::live_groups()).is_empty()
}

/// Toggle the launched app(s): if any owned window is visible, hide them all (focus falls
/// back to OmniDeck); else re-show whatever the last toggle hid. Returns a short description
/// of what happened, or None if there was nothing to act on.
pub fn toggle() -> Option<&'static str> {
    // Session-only, enforced at the chokepoint for every caller (UI command, Guide press,
    // hotkey): on a desktop, unmapping would hide the app's window from the real WM, which
    // has its own idea of window management. (OMNIDECK_FORCE_HOTKEY tests on desktop X11.)
    if !crate::session::in_session()
        && std::env::var_os("OMNIDECK_FORCE_HOTKEY").is_none()
    {
        return None;
    }
    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots[screen_num].root;

    let groups = crate::watchdog::live_groups();
    let visible = visible_owned(&conn, root, &groups);

    if !visible.is_empty() {
        // Hide: unmap every owned visible toplevel; gamescope refocuses OmniDeck.
        let wins: Vec<Window> = visible.iter().map(|&(w, _)| w).collect();
        let failed = set_mapped(&conn, &wins, false);
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
    let failed = set_mapped(&conn, &hidden, true);
    if !failed.is_empty() {
        // Put the strays back so the next toggle retries instead of stranding the app
        // invisible with an empty HIDDEN list (Guide would then do nothing forever).
        tracing::warn!("switcher: {} window(s) did not remap — kept for retry", failed.len());
        crate::sync::lock_or_recover(&HIDDEN, "switcher.HIDDEN").extend(failed);
    }
    Some("re-shown — app focused")
}

/// Session gate shared by the deck entry points (see toggle's note).
fn session_ok() -> bool {
    crate::session::in_session() || std::env::var_os("OMNIDECK_FORCE_HOTKEY").is_some()
}

/// Hide EVERY launched app so OmniDeck (and the deck overlay) is what's on screen — the
/// deck-switcher's "open" step. Same as toggle's hide half: unmap owned toplevels, remember
/// them, freeze the silent ones. Returns true if anything was hidden.
pub fn hide_all() -> bool {
    if !session_ok() {
        return false;
    }
    let Ok((conn, screen_num)) = x11rb::connect(None) else { return false };
    let root = conn.setup().roots[screen_num].root;
    let visible = visible_owned(&conn, root, &crate::watchdog::live_groups());
    if visible.is_empty() {
        return false;
    }
    let wins: Vec<Window> = visible.iter().map(|&(w, _)| w).collect();
    let failed = set_mapped(&conn, &wins, false);
    {
        let mut hidden = crate::sync::lock_or_recover(&HIDDEN, "switcher.HIDDEN");
        for &(win, _) in &visible {
            if !hidden.contains(&win) && !failed.contains(&win) {
                hidden.push(win);
            }
        }
    }
    freeze_silent_groups(&visible, &failed);
    true
}

/// Bring ONE launched app group to the front (the deck-switcher's "open this card"): resume
/// it if frozen, then map its toplevels. Other apps stay hidden. Returns true on success.
pub fn show_group(group: u32) -> bool {
    if !session_ok() {
        return false;
    }
    let Ok((conn, screen_num)) = x11rb::connect(None) else { return false };
    let root = conn.setup().roots[screen_num].root;

    // Resume this group first so its windows can repaint/focus when mapped.
    let _ = std::process::Command::new("kill").args(["-CONT", &format!("-{group}")]).status();
    crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").retain(|&g| g != group);

    let wins = windows_of_group(&conn, root, group);
    if wins.is_empty() {
        return false;
    }
    let failed = set_mapped(&conn, &wins, true);
    // Drop the now-shown windows from the hidden set (keep any that failed to map for retry).
    crate::sync::lock_or_recover(&HIDDEN, "switcher.HIDDEN")
        .retain(|w| !wins.contains(w) || failed.contains(w));
    failed.len() < wins.len()
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
fn freeze_silent_groups(visible: &[(Window, u32)], failed: &[Window]) {
    let mut candidates: Vec<u32> = visible
        .iter()
        .filter(|(w, _)| !failed.contains(w))
        .map(|&(_, g)| g)
        .collect();
    candidates.sort_unstable();
    candidates.dedup();
    if candidates.is_empty() {
        return;
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
        let ok = std::process::Command::new("kill")
            .args(["-STOP", &format!("-{g}")])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            tracing::info!("switcher: froze silent hidden group {g}");
            stopped.push(g);
        }
    }
}

/// SIGCONT everything `freeze_silent_groups` stopped (dead groups just fail the kill).
fn resume_stopped_groups() {
    let stopped: Vec<u32> =
        std::mem::take(&mut *crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED"));
    for g in stopped {
        let _ = std::process::Command::new("kill").args(["-CONT", &format!("-{g}")]).status();
    }
}

/// Which of `candidates` have an ACTIVE (uncorked) audio stream, via the PipeWire pulse
/// shim: `pactl list sink-inputs` blocks carrying `application.process.id` whose process
/// group matches. Fail-open — if pactl is missing or errors, every candidate counts as
/// audible: leaving a silent app running is recoverable, freezing the user's music is rude.
fn audible_groups(candidates: &[u32]) -> Vec<u32> {
    let out = match std::process::Command::new("pactl")
        .args(["list", "sink-inputs"])
        .env("LC_ALL", "C")
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
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
