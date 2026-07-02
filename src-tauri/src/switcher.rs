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
        for &win in &visible {
            let _ = conn.unmap_window(win);
        }
        let _ = conn.flush();
        if let Ok(mut hidden) = HIDDEN.lock() {
            // APPEND (don't overwrite): an app launched while another was hidden must not
            // orphan the first one's windows — the next show brings the whole set back.
            for win in visible {
                if !hidden.contains(&win) {
                    hidden.push(win);
                }
            }
        }
        return Some("hidden — OmniDeck focused");
    }

    // Nothing visible: re-show the set we hid (skip windows that died while hidden).
    let hidden: Vec<Window> = HIDDEN.lock().map(|mut h| std::mem::take(&mut *h)).unwrap_or_default();
    if hidden.is_empty() {
        return None;
    }
    for &win in &hidden {
        let _ = conn.map_window(win);
    }
    let _ = conn.flush();
    Some("re-shown — app focused")
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
