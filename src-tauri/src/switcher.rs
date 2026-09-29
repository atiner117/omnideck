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

/// Most recently foregrounded owned groups, newest first. Store identifiers only; every
/// consumer intersects with watchdog's live registry before mapping or displaying a card.
static RECENT: Mutex<Vec<u32>> = Mutex::new(Vec::new());

fn remember_group(group: u32) {
    let mut recent = crate::sync::lock_or_recover(&RECENT, "switcher.RECENT");
    recent.retain(|&g| g != group);
    recent.insert(0, group);
}

/// Running cards ordered by actual selection/focus, then by launch time for unseen apps.
pub fn recent_apps() -> Vec<crate::watchdog::LiveApp> {
    let apps = crate::watchdog::live_apps();
    let recent = crate::sync::lock_or_recover(&RECENT, "switcher.RECENT");
    order_apps(apps, &recent)
}

fn order_apps(mut apps: Vec<crate::watchdog::LiveApp>, recent: &[u32]) -> Vec<crate::watchdog::LiveApp> {
    apps.sort_by_key(|a| recent.iter().position(|&g| g == a.group).unwrap_or(usize::MAX));
    // Unseen apps remain in launch order; put the newest first without disturbing recents.
    let known = apps.iter().take_while(|a| recent.contains(&a.group)).count();
    apps[known..].reverse();
    apps
}

/// A closed group must not be selected again if its pid is recycled later.
pub(crate) fn forget_recent(group: u32) {
    crate::sync::lock_or_recover(&RECENT, "switcher.RECENT").retain(|&g| g != group);
}

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
/// Members captured when a group is frozen; their start times allow a safe thaw even if
/// the launched leader exits before the user returns to that app.
type FrozenMembers = std::collections::HashMap<u32, Vec<(u32, u64)>>;
static FROZEN_MEMBERS: std::sync::LazyLock<Mutex<FrozenMembers>> =
    std::sync::LazyLock::new(|| Mutex::new(FrozenMembers::new()));

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

/// All group signals verify the original leader's kernel starttime before kill(2).
fn cont_group(group: u32) -> bool {
    crate::watchdog::signal_group_verified(group, libc::SIGCONT)
}

fn stop_group(group: u32) -> bool {
    // Snapshot identities BEFORE stopping anything. If procfs is unavailable, never
    // freeze a group whose surviving members we cannot later thaw by identity.
    let Ok(entries) = std::fs::read_dir("/proc") else { return false };
    let members: Vec<(u32, u64)> = entries.flatten()
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .filter(|&pid| parent_and_pgid(pid).1 == group)
        .filter_map(|pid| Some((pid, crate::watchdog::proc_start_time(pid)?)))
        .collect();
    if !members.iter().any(|&(pid, _)| pid == group) { return false; }
    if !crate::watchdog::signal_group_verified(group, libc::SIGSTOP) { return false; }
    crate::sync::lock_or_recover(&FROZEN_MEMBERS, "switcher.FROZEN_MEMBERS").insert(group, members);
    true
}

fn thaw_group(group: u32) -> bool {
    if cont_group(group) { return true; }
    let members = crate::sync::lock_or_recover(&FROZEN_MEMBERS, "switcher.FROZEN_MEMBERS")
        .get(&group).cloned().unwrap_or_default();
    if members.is_empty() { return false; }
    // A vanished leader cannot authorize a group signal. Thaw individually by recorded
    // identity instead; a PID recycled since freezing is refused.
    let mut all_thawed = true;
    for (pid, start) in members {
        if crate::watchdog::proc_start_time(pid) == Some(start) {
            all_thawed &= crate::watchdog::signal_pid_verified(pid, start, libc::SIGCONT);
        }
    }
    all_thawed
}

/// Drop a dead group's freeze record only after safely thawing any surviving members.
pub(crate) fn forget_stopped(group: u32) {
    if crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").contains(&group)
        && !thaw_group(group)
    {
        let members = crate::sync::lock_or_recover(&FROZEN_MEMBERS, "switcher.FROZEN_MEMBERS")
            .get(&group).cloned().unwrap_or_default();
        if members.iter().any(|&(pid, start)| crate::watchdog::proc_start_time(pid) == Some(start)) {
            tracing::warn!("switcher: group {group} still has surviving frozen members — retaining for exit retry");
            return;
        }
    }
    crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").retain(|&g| g != group);
    crate::sync::lock_or_recover(&FROZEN_MEMBERS, "switcher.FROZEN_MEMBERS").remove(&group);
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
        if let Some(group) = owning_group(pid, groups) {
            visible.push((win, group));
        }
    }
    visible
}

/// X input focus may belong to a child of the app's top-level window. Walk only its
/// ancestors; never infer foreground from every mapped app just because it is viewable.
fn foreground_group(conn: &RustConnection, visible: &[(Window, u32)]) -> Option<u32> {
    let mut win = conn.get_input_focus().ok()?.reply().ok()?.focus;
    for _ in 0..16 {
        if let Some(&(_, group)) = visible.iter().find(|&&(w, _)| w == win) {
            return Some(group);
        }
        let parent = conn.query_tree(win).ok()?.reply().ok()?.parent;
        if parent == win || parent == 0 {
            break;
        }
        win = parent;
    }
    None
}

/// True only while X input focus belongs to an owned app's viewable window. An app can
/// remain mapped behind Home or its task overview; viewability alone must never steal the
/// pad's A/D-pad input from OmniDeck.
pub fn owned_app_focused() -> bool {
    let groups = crate::watchdog::live_groups();
    if groups.is_empty() { return false; }
    with_x11(|conn, root| {
        let visible = visible_owned(conn, root, &groups);
        foreground_group(conn, &visible).is_some()
    }).unwrap_or(false)
}

/// Only restore an owned app if OmniDeck itself is focused. A Steam game or foreign
/// launcher may be in front while an owned app is hidden; don't cover it with another app.
fn deck_is_focused(conn: &RustConnection) -> bool {
    let Ok(Ok(atom)) = conn.intern_atom(false, b"_NET_WM_PID").map(|c| c.reply()) else { return false };
    let Ok(Ok(focus)) = conn.get_input_focus().map(|c| c.reply()) else { return false };
    let mut win = focus.focus;
    for _ in 0..16 {
        if let Ok(Ok(prop)) = conn.get_property(false, win, atom.atom, AtomEnum::CARDINAL, 0, 1).map(|c| c.reply()) {
            if let Some(pid) = prop.value32().and_then(|mut v| v.next()) {
                return pid == std::process::id();
            }
        }
        let Ok(Ok(tree)) = conn.query_tree(win).map(|c| c.reply()) else { return false };
        if tree.parent == win || tree.parent == 0 { break; }
        win = tree.parent;
    }
    false
}

/// Direction of a Guide tap. The webview needs to select its dashboard ONLY when the
/// backend actually returned from an app, not when it restored an app or failed to switch.
#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SwitchResult {
    Home,
    App,
    Unchanged,
}

/// Tap: from a focused owned app go Home; from a focused Home return to the most recent
/// running owned app. Foreign/Steam windows cannot be controlled by this ownership path.
pub fn home_toggle() -> SwitchResult {
    if !session_ok() {
        return SwitchResult::Unchanged;
    }
    let focused = with_x11(|conn, root| {
        let owned = visible_owned(conn, root, &crate::watchdog::live_groups());
        (foreground_group(conn, &owned), deck_is_focused(conn))
    });
    let Some((visible, on_deck)) = focused else { return SwitchResult::Unchanged };
    if let Some(group) = visible {
        remember_group(group);
        return if hide_all() { SwitchResult::Home } else { SwitchResult::Unchanged };
    }
    if !on_deck { return SwitchResult::Unchanged; }
    // Home -> most recently used *running* group. Never remap the entire hidden set:
    // multiple apps may be suspended, and only one should return to the foreground.
    for app in recent_apps() {
        if show_group(app.group) {
            return SwitchResult::App;
        }
    }
    SwitchResult::Unchanged
}

/// Session gate shared by every window-touching entry point (Home, deck, navpad, hotkey):
/// on a desktop, unmapping/injecting into foreign windows belongs to the real WM.
pub(crate) fn session_ok() -> bool {
    crate::session::in_session() || std::env::var_os("OMNIDECK_FORCE_HOTKEY").is_some()
}

/// What the most recent `hide_all()` actually did — the windows it unmapped and the groups
/// it froze — so dismissing the deck (rather than picking a card) can put exactly that state
/// back. Consumed by `deck_cancel`; cleared when a card is chosen instead.
static LAST_HIDE: Mutex<(Vec<Window>, Vec<u32>, Option<u32>)> = Mutex::new((Vec::new(), Vec::new(), None));

/// Hide EVERY launched app so OmniDeck (and the deck overlay) is what's on screen — the
/// deck-switcher's "open" step. Same as toggle's hide half: unmap owned toplevels, remember
/// them, freeze the silent ones. Returns true when Home is reachable (including if already
/// Home); false if X failed or a window resisted unmapping.
pub fn hide_all() -> bool {
    if !session_ok() {
        return false;
    }
    with_x11(|conn, root| {
        let visible = visible_owned(conn, root, &crate::watchdog::live_groups());
        if visible.is_empty() || deck_is_focused(conn) {
            // Already at Home: a background app may still have a mapped window. Do not
            // freeze/unmap it just to draw an overlay that is already in front.
            *crate::sync::lock_or_recover(&LAST_HIDE, "switcher.LAST_HIDE") = (Vec::new(), Vec::new(), None);
            return true;
        }
        let foreground = foreground_group(conn, &visible);
        let Some(group) = foreground else {
            // An app is mapped but X focus is unknown or belongs to a foreign Steam window.
            // Hiding it without knowing what to restore would strand the session on cancel.
            return false;
        };
        remember_group(group);
        let wins: Vec<Window> = visible.iter().map(|&(w, _)| w).collect();
        let failed = set_mapped(conn, &wins, false);
        if !failed.is_empty() {
            // If even one owned window stays mapped, gamescope may keep that app in front.
            // Roll the successful unmaps back; don't claim that the task overview is visible.
            let unmapped: Vec<Window> = wins.iter().copied().filter(|w| !failed.contains(w)).collect();
            let rollback_failed = set_mapped(conn, &unmapped, true);
            crate::sync::lock_or_recover(&HIDDEN, "switcher.HIDDEN").extend(&rollback_failed);
            *crate::sync::lock_or_recover(&LAST_HIDE, "switcher.LAST_HIDE") =
                (rollback_failed, Vec::new(), Some(group));
            tracing::warn!("switcher: {} window(s) resisted hide", failed.len());
            return false;
        }
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
        *crate::sync::lock_or_recover(&LAST_HIDE, "switcher.LAST_HIDE") = (hidden_now, frozen_now, Some(group));
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
    with_x11(|conn, root| {
        let (wins, groups, foreground) = std::mem::take(&mut *crate::sync::lock_or_recover(
            &LAST_HIDE,
            "switcher.LAST_HIDE",
        ));
        let Some(group) = foreground else {
            return true; // overview started from Home; leave other owned apps hidden
        };
        // A can be mapped behind B before opening the overview. Restore ONLY B, not every
        // window that happened to be mapped: remapping A may steal gamescope's focus.
        let owned = windows_of_group(conn, root, group);
        let restore: Vec<Window> = wins.into_iter().filter(|w| owned.contains(w)).collect();
        if restore.is_empty() { return true; } // the app exited while the overview was open
        let failed = set_mapped(conn, &restore, true);
        if failed.len() == restore.len() {
            *crate::sync::lock_or_recover(&LAST_HIDE, "switcher.LAST_HIDE") = (failed, groups, Some(group));
            return false;
        }
        if groups.contains(&group) && thaw_group(group) {
            crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").retain(|&g| g != group);
            crate::sync::lock_or_recover(&FROZEN_MEMBERS, "switcher.FROZEN_MEMBERS").remove(&group);
        }
        crate::sync::lock_or_recover(&HIDDEN, "switcher.HIDDEN")
            .retain(|w| !restore.contains(w) || failed.contains(w));
        if !failed.is_empty() {
            *crate::sync::lock_or_recover(&LAST_HIDE, "switcher.LAST_HIDE") = (failed, Vec::new(), Some(group));
            return false;
        }
        true
    })
    .unwrap_or(false)
}

/// Choose Home from the task overview: keep apps hidden, but discard the temporary
/// cancellation snapshot. B/Escape is the separate operation that restores it.
pub fn deck_home() {
    *crate::sync::lock_or_recover(&LAST_HIDE, "switcher.LAST_HIDE") = (Vec::new(), Vec::new(), None);
}

/// Undo an unsuccessful card selection: windows that were hidden stay hidden, and windows
/// originally mapped behind Home are restored. Never leave a formerly visible app unmapped
/// just because gamescope did not accept its attempted focus transfer.
fn rollback_show(conn: &RustConnection, wins: &[Window], was_visible: &[Window]) {
    let newly_shown: Vec<Window> = wins.iter().copied().filter(|w| !was_visible.contains(w)).collect();
    let failed_hide = set_mapped(conn, &newly_shown, false);
    if !failed_hide.is_empty() {
        tracing::warn!("switcher: rollback could not re-hide {} app window(s)", failed_hide.len());
    }
    let failed_restore = set_mapped(conn, was_visible, true);
    if !failed_restore.is_empty() {
        let mut hidden = crate::sync::lock_or_recover(&HIDDEN, "switcher.HIDDEN");
        for win in failed_restore {
            if !hidden.contains(&win) { hidden.push(win); }
        }
        tracing::warn!("switcher: could not restore a previously mapped app window");
    }
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
        let visible = visible_owned(conn, root, &crate::watchdog::live_groups());
        let already_focused = foreground_group(conn, &visible) == Some(group);
        let on_deck = deck_is_focused(conn);
        if !on_deck && !already_focused {
            return false; // never raise an owned app over a focused Steam/foreign window
        }
        // A background app can be viewable behind Home. Mapping a VIEWABLE window is a
        // no-op in gamescope: force a targeted unmap/remap so the compositor refocuses it.
        // This is only done from OmniDeck, never while that app is already foreground.
        let was_visible: Vec<Window> = visible.iter().filter(|&&(_, g)| g == group).map(|&(w, _)| w).collect();
        if on_deck && !was_visible.is_empty() {
            let resisted = set_mapped(conn, &was_visible, false);
            if !resisted.is_empty() {
                rollback_show(conn, &wins, &was_visible);
                return false;
            }
        }

        // Map BEFORE thawing. The map requests come from OUR connection (steamcompmgr does the
        // actual mapping), so a SIGSTOPped client doesn't block them — and if every map fails
        // (wedged compositor), the group must stay frozen with its STOPPED entry and the deck's
        // dismiss snapshot intact, not thawed-and-invisible with no record to re-freeze it.
        let failed = set_mapped(conn, &wins, true);
        if failed.len() == wins.len() {
            rollback_show(conn, &wins, &was_visible);
            return false;
        }

        // At least one window is up — resume the group so it can repaint and take focus.
        let was_stopped = crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").contains(&group);
        if thaw_group(group) {
            crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").retain(|&g| g != group);
            crate::sync::lock_or_recover(&FROZEN_MEMBERS, "switcher.FROZEN_MEMBERS").remove(&group);
        } else if crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").contains(&group) {
            rollback_show(conn, &wins, &was_visible);
            return false; // still frozen: preserve the card and record for a retry
        }
        // Mapping alone is not proof of a successful switch: gamescope can leave a
        // viewable app behind Home. Wait briefly for the mapped group to receive X focus.
        if on_deck {
            let mut focused = false;
            for _ in 0..10 {
                let now_visible = visible_owned(conn, root, &[group]);
                if foreground_group(conn, &now_visible) == Some(group) {
                    focused = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            if !focused {
                rollback_show(conn, &wins, &was_visible);
                if was_stopped && !wins.iter().any(|w| {
                    conn.get_window_attributes(*w).ok().and_then(|c| c.reply().ok())
                        .is_some_and(|a| a.map_state == MapState::VIEWABLE)
                }) && stop_group(group) {
                    crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").push(group);
                }
                return false; // preserve cancellation snapshot for recovery
            }
        }
        remember_group(group);
        // A card was chosen — the deck's dismiss snapshot no longer applies.
        *crate::sync::lock_or_recover(&LAST_HIDE, "switcher.LAST_HIDE") = (Vec::new(), Vec::new(), None);
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
    let stopped: Vec<u32> = crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").clone();
    for g in stopped {
        if thaw_group(g) {
            crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").retain(|&v| v != g);
            crate::sync::lock_or_recover(&FROZEN_MEMBERS, "switcher.FROZEN_MEMBERS").remove(&g);
        } else {
            tracing::warn!("switcher: could not thaw group {g}; retaining its freeze record");
        }
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
    use super::{order_apps, parent_and_pgid, SwitchResult};
    use crate::watchdog::LiveApp;

    #[test]
    fn proc_stat_of_self_is_readable_and_bogus_pid_is_zero() {
        // A process in a nested PID namespace may legitimately report pgrp=0 in /proc.
        // Check the readable parent instead of assuming every namespace has a nonzero pgid.
        assert_ne!(parent_and_pgid(std::process::id()).0, 0);
        assert_eq!(parent_and_pgid(0), (0, 0)); // /proc/0 never exists
    }

    #[test]
    fn guide_tap_result_reports_direction_for_dashboard_reset() {
        assert_eq!(serde_json::to_string(&SwitchResult::Home).unwrap(), "\"home\"");
        assert_eq!(serde_json::to_string(&SwitchResult::App).unwrap(), "\"app\"");
        assert_eq!(serde_json::to_string(&SwitchResult::Unchanged).unwrap(), "\"unchanged\"");
    }

    #[test]
    fn recent_cards_prefer_last_selected_then_newest_unseen() {
        let apps = [1, 2, 3, 4].map(|group| LiveApp { group, name: group.to_string(), id: None, starttime: 1 }).to_vec();
        let ordered = order_apps(apps, &[2, 1, 99]); // stale 99 is never displayed
        assert_eq!(ordered.iter().map(|a| a.group).collect::<Vec<_>>(), vec![2, 1, 4, 3]);
    }
}
