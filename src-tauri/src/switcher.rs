// OmniDeck — session app switcher: hide/show launched apps instead of killing them.
//
// Gamescope (steamcompmgr) ignores `_NET_ACTIVE_WINDOW` and the `GAMESCOPECTRL_BASELAYER_APPID`
// root property for plain (non-STEAM_GAME) windows — verified live on the M2 host — but its
// focus does follow window *mapping*: unmap the launched app's toplevels and focus falls back
// to OmniDeck; map them again and the newest window retakes focus. So the switcher primitive
// is unmap/show: the app's process keeps running (audio keeps playing — hide YouTube Music,
// browse the dashboard, bring it back), which is what "switch" should mean on a console.
//
// Ownership: only windows whose _NET_WM_PID belongs to one of our launched process groups
// (watchdog::live_groups; every launch is a group leader) are ever touched — never OmniDeck's
// own window, gamescope's internals, or a Steam game's (Steam has gamescope's native
// focus-return path).
use std::sync::Mutex;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, MapState, Window};

/// Windows we unmapped on the last "hide" — remapped on the next toggle. (window, pgid)
static HIDDEN: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// Process-group id for `pid` from /proc/<pid>/stat field 5 (0 when gone/unreadable).
fn pgid_of(pid: u32) -> u32 {
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else { return 0 };
    // comm (field 2) can contain spaces/parens — split after the LAST ')'.
    let Some(rest) = stat.rsplit_once(')').map(|(_, r)| r) else { return 0 };
    rest.split_whitespace().nth(2).and_then(|s| s.parse().ok()).unwrap_or(0)
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
    let net_wm_pid = conn.intern_atom(false, b"_NET_WM_PID").ok()?.reply().ok()?.atom;

    let groups = crate::watchdog::live_groups();

    // Collect the launched apps' currently-viewable toplevels.
    let tree = conn.query_tree(root).ok()?.reply().ok()?;
    let mut visible: Vec<Window> = Vec::new();
    for &win in &tree.children {
        let Ok(attrs) = conn.get_window_attributes(win).map(|c| c.reply()) else { continue };
        let Ok(attrs) = attrs else { continue };
        if attrs.map_state != MapState::VIEWABLE {
            continue;
        }
        let Ok(reply) = conn
            .get_property(false, win, net_wm_pid, AtomEnum::CARDINAL, 0, 1)
            .map(|c| c.reply())
        else {
            continue;
        };
        let Ok(prop) = reply else { continue };
        let Some(pid) = prop.value32().and_then(|mut v| v.next()) else { continue };
        if groups.contains(&pgid_of(pid)) {
            visible.push(win);
        }
    }

    if !visible.is_empty() {
        // Hide: unmap every owned visible toplevel; gamescope refocuses OmniDeck.
        let failed = set_mapped(&conn, &visible, false);
        if !failed.is_empty() {
            tracing::warn!("switcher: {} window(s) resisted unmap", failed.len());
        }
        let mut hidden = crate::sync::lock_or_recover(&HIDDEN, "switcher.HIDDEN");
        // APPEND (don't overwrite): an app launched while another was hidden must not
        // orphan the first one's windows — the next show brings the whole set back.
        for win in visible {
            if !hidden.contains(&win) && !failed.contains(&win) {
                hidden.push(win);
            }
        }
        return Some("hidden — OmniDeck focused");
    }

    // Nothing visible: re-show the set we hid (skip windows that died while hidden).
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
