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
// focus-return path). "Steam's" is decided by the window, not the process: when the Steam
// client itself is a launched tile (`steam://open/bigpicture`) it is a child of ours, and
// every game it starts shares that process group — pid ownership alone would make Big
// Picture AND the game "ours" (navpad driving a game with arrow keys/mouse, a Guide tap
// SIGSTOPping the whole Steam tree). Any window carrying the STEAM_GAME atom is therefore
// never owned, whatever its pid. See any_app_visible() for the related front-ness rule.
use std::sync::Mutex;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, MapState, Window};
use x11rb::rust_connection::RustConnection;

/// Windows we unmapped on the last "hide" — remapped on the next toggle.
static HIDDEN: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// Most recently foregrounded owned groups, newest first. Intersect with live apps at use.
static RECENT: Mutex<Vec<u32>> = Mutex::new(Vec::new());
fn remember_group(group: u32) {
    let mut recent = crate::sync::lock_or_recover(&RECENT, "switcher.RECENT");
    recent.retain(|&g| g != group);
    recent.insert(0, group);
}
pub fn recent_apps() -> Vec<crate::watchdog::LiveApp> {
    let mut apps = crate::watchdog::live_apps();
    let recent = crate::sync::lock_or_recover(&RECENT, "switcher.RECENT");
    apps.sort_by_key(|a| recent.iter().position(|&g| g == a.group).unwrap_or(usize::MAX));
    let known = apps.iter().take_while(|a| recent.contains(&a.group)).count();
    apps[known..].reverse();
    apps
}
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

/// A process group we froze, plus the identity of every member that was in it at freeze time.
///
/// The member roster is what makes a thaw survive the leader's death. A process group outlives
/// its leader: kill the `sh` that launched an app and its children keep the same pgid, still
/// SIGSTOPped. The leader-based check (`watchdog::group_verified`) then refuses to signal —
/// correctly, since it can no longer prove whose group this is — and the survivors are frozen
/// forever. Recording `(pid, start time)` per member gives the thaw path its own proof of
/// identity, so it can stay recycle-safe without depending on the leader.
///
/// The roster is captured immediately AFTER the SIGSTOP lands, which is what makes it
/// complete: a frozen process cannot fork, so the group can shrink (members exit) but never
/// grow behind our back.
#[derive(Clone)]
struct Frozen {
    group: u32,
    /// `(pid, kernel start time)` for each member — start time is the anti-recycling proof.
    members: Vec<(u32, u64)>,
}

/// Process groups we froze (SIGSTOP) when their windows were hidden. Disjoint from any
/// group that was audibly playing at hide time. SIGCONTed on the next re-show, and an entry
/// is dropped only once its thaw is known to have landed (or nothing of it survives).
static STOPPED: Mutex<Vec<Frozen>> = Mutex::new(Vec::new());

/// Groups we thawed back into the foreground AFTER their leader was gone.
///
/// `LIVE_GROUPS` is keyed on the launch leader and never gets such a group back, so the moment
/// we re-show one it is running with no handle on it anywhere: `return_home` cannot see it and
/// the user is left holding a window that will not close. Keeping the freeze-time roster here
/// is that handle — the same `(pid, start time)` pairs, so close-all stays a verified signal.
/// Entries are dropped when the group is closed, or when nothing recorded in it is alive.
static LEADERLESS: Mutex<Vec<Frozen>> = Mutex::new(Vec::new());

/// Remember a group we just thawed without its leader, replacing any older record for it and
/// pruning records whose processes have all gone.
fn remember_leaderless(f: &Frozen) {
    let mut list = crate::sync::lock_or_recover(&LEADERLESS, "switcher.LEADERLESS");
    list.retain(|o| {
        o.group != f.group
            && o.members
                .iter()
                .any(|&(p, s)| crate::watchdog::proc_start_time(p) == Some(s))
    });
    list.push(f.clone());
}

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

/// Every live pid in process group `group`, paired with its kernel start time. Called with
/// the group already SIGSTOPped, so the roster it returns cannot go stale by growing.
fn group_members(group: u32) -> Vec<(u32, u64)> {
    let Ok(entries) = std::fs::read_dir("/proc") else { return Vec::new() };
    let mut out = Vec::new();
    for ent in entries.flatten() {
        let Some(pid) = ent.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };
        if pgid_of(pid) != group {
            continue;
        }
        // Skipping an unreadable start time is the safe direction: no recorded identity means
        // this member simply won't be signalled by the fallback, rather than signalled blind.
        if let Some(start) = crate::watchdog::proc_start_time(pid) {
            out.push((pid, start));
        }
    }
    out
}

/// What one thaw attempt did. The caller needs all three apart: `Stuck` means keep the record
/// and retry, and `Leaderless` means the group is running again with nothing in `LIVE_GROUPS`
/// to manage it — whoever leaves it running owes it a handle (see `remember_leaderless`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Thaw {
    /// Thawed through the leader, or nothing of the group survives: drop the record.
    Done,
    /// Thawed member-by-member because the leader is gone. Droppable, but not forgettable.
    Leaderless,
    /// Members are still stopped — keep the record so a later attempt can retry.
    Stuck,
}

/// SIGCONT a frozen group.
///
/// Two verified attempts, never a blind one. The group-level signal is preferred (one kill(2)
/// reaches every member, and it re-checks the leader's launch start time). When it fails
/// because the LEADER is gone, the members recorded at freeze time are thawed individually,
/// each verified against its own start time — the case that used to strand a whole app's
/// worth of processes in state `T`.
fn cont_frozen(f: &Frozen) -> Thaw {
    if crate::watchdog::signal_group_verified(f.group, libc::SIGCONT) {
        return Thaw::Done;
    }
    let (mut survivors, mut thawed) = (0u32, 0u32);
    for &(pid, start) in &f.members {
        if crate::watchdog::proc_start_time(pid) != Some(start) {
            continue; // exited (or the pid was recycled — either way, not ours to signal)
        }
        survivors += 1;
        if crate::watchdog::signal_pid_verified(pid, start, libc::SIGCONT) {
            thawed += 1;
        }
    }
    if survivors == 0 {
        return Thaw::Done; // nothing left to strand
    }
    tracing::info!(
        "switcher: group {} lost its leader — thawed {thawed}/{survivors} surviving member(s)",
        f.group
    );
    if thawed > 0 { Thaw::Leaderless } else { Thaw::Stuck }
}

/// SIGCONT the frozen record for `group`, if we have one, KEEPING it running. Groups we never
/// froze are trivially "resumed". Returns whether the record may now be dropped.
///
/// This is the re-show path (deck dismiss / card picked), so a leaderless thaw here puts a
/// window back on screen that `LIVE_GROUPS` cannot account for — record it, or the user gets a
/// window the Guide button can neither hide nor close.
fn cont_group(group: u32) -> bool {
    let record = crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED")
        .iter()
        .find(|f| f.group == group)
        .cloned();
    let Some(f) = record else { return true };
    match cont_frozen(&f) {
        Thaw::Done => true,
        Thaw::Leaderless => {
            remember_leaderless(&f);
            true
        }
        Thaw::Stuck => false,
    }
}

/// SIGSTOP a process group and record what was in it. `None` when the freeze didn't land.
fn stop_group(group: u32) -> Option<Frozen> {
    if !crate::watchdog::signal_group_verified(group, libc::SIGSTOP) {
        return None;
    }
    Some(Frozen { group, members: group_members(group) })
}

/// Forget a group we froze because it is being closed. Thaws first: SIGTERM cannot be acted on
/// by a SIGSTOPped process, and if the leader is already gone the group-level CONT-then-TERM in
/// `watchdog::close_group` reaches nobody — so without this a leaderless frozen group would be
/// "closed" on paper and left frozen on the machine. Members that survive the thaw are TERMed
/// individually, again identity-verified.
pub(crate) fn forget_stopped(group: u32) {
    let record = crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED")
        .iter()
        .find(|f| f.group == group)
        .cloned();
    if let Some(f) = record {
        thaw_and_term(&f);
    }
    crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").retain(|f| f.group != group);
    // A group closed from a card may also be one we re-showed leaderless earlier.
    let leaderless = crate::sync::lock_or_recover(&LEADERLESS, "switcher.LEADERLESS")
        .iter()
        .find(|f| f.group == group)
        .cloned();
    if let Some(f) = leaderless {
        thaw_and_term(&f);
    }
    crate::sync::lock_or_recover(&LEADERLESS, "switcher.LEADERLESS").retain(|f| f.group != group);
}

/// Close one frozen record: thaw first (a SIGSTOPped process cannot act on SIGTERM, so
/// TERM-only would "close" it on paper and leave it frozen on the machine), then TERM each
/// member against its own recorded start time. Returns whether a TERM actually reached
/// something still alive.
fn thaw_and_term(f: &Frozen) -> bool {
    cont_frozen(f);
    let mut any = false;
    for &(pid, start) in &f.members {
        any |= crate::watchdog::signal_pid_verified(pid, start, libc::SIGTERM);
    }
    any
}

/// Close EVERY group we froze, and forget them all — the close-all counterpart to
/// `forget_stopped`.
///
/// `watchdog::return_home` closes what LIVE_GROUPS lists, and that list is keyed on the launch
/// leader: once the leader exits, `watch_child`'s reaper drops the entry while the group's
/// members live on, SIGSTOPped behind an unmapped window. Those are precisely the processes a
/// Guide hold most needs to reach, and they were the ones it could not see — the hold logged
/// nothing and left the machine exactly as it found it. Signalling them is safe for the same
/// reason the thaw fallback is: the roster carries each member's own start time, so a recycled
/// pid is refused rather than TERMed blind.
pub(crate) fn close_stopped_groups() -> bool {
    let mut records: Vec<Frozen> =
        std::mem::take(&mut *crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED"));
    // Plus the ones already thawed back into the foreground without a leader — same blind spot,
    // and after a re-show this is the ONLY list that still knows about them.
    records
        .extend(std::mem::take(&mut *crate::sync::lock_or_recover(&LEADERLESS, "switcher.LEADERLESS")));
    let mut any = false;
    for f in &records {
        any |= thaw_and_term(f);
    }
    if any {
        tracing::info!("switcher: close-all reached {} leaderless group(s)", records.len());
    }
    any
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
    let steam_game = steam_game_atom(conn);
    let Ok(Ok(tree)) = conn.query_tree(root).map(|c| c.reply()) else { return Vec::new() };
    let mut visible = Vec::new();
    for &win in &tree.children {
        let Ok(Ok(attrs)) = conn.get_window_attributes(win).map(|c| c.reply()) else { continue };
        if attrs.map_state != MapState::VIEWABLE {
            continue;
        }
        if is_steam_window(conn, win, steam_game) {
            continue; // Steam client / Steam game: gamepad-native, gamescope-managed
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

/// True when a launched app's window is the one gamescope has IN FRONT. The navpad uses
/// this as its activation gate: it must only drive the app that is actually receiving input.
///
/// "Viewable" is not enough: a launched PWA stays mapped underneath a Steam game, and while
/// the game was in front the bridge kept turning the sticks into arrow keys and mouse motion
/// for it — KH3 flipped to keyboard/mouse mode and the left stick went dead (couch box,
/// 2026-09-13). So the owned window must also be gamescope's `GAMESCOPE_FOCUSED_WINDOW`.
/// Without that root property (older gamescope) fall back to "any owned window viewable".
pub fn any_app_visible() -> bool {
    // The navpad polls this ~3x/s for the whole session; with nothing launched the answer
    // is trivially false — don't touch X at all to say so (the pooled connection stays idle).
    let groups = crate::watchdog::live_groups();
    if groups.is_empty() {
        return false;
    }
    with_x11(|conn, root| {
        let visible = visible_owned(conn, root, &groups);
        if visible.is_empty() {
            return false;
        }
        match gamescope_focused_window(conn, root) {
            Some(focused) => visible.iter().any(|&(w, _)| w == focused),
            None => true,
        }
    })
    .unwrap_or(false)
}

/// gamescope's `GAMESCOPE_FOCUSED_WINDOW` root property (the X window it is presenting and
/// routing input to). `None` when the property is absent (not gamescope, or too old).
fn gamescope_focused_window(conn: &RustConnection, root: Window) -> Option<Window> {
    let atom = conn.intern_atom(false, b"GAMESCOPE_FOCUSED_WINDOW").ok()?.reply().ok()?.atom;
    let prop = conn.get_property(false, root, atom, AtomEnum::CARDINAL, 0, 1).ok()?.reply().ok()?;
    prop.value32().and_then(|mut v| v.next()).filter(|&w| w != 0)
}

/// True when OmniDeck's own window is the one gamescope presents — or when there is no
/// gamescope focus property to consult (plain desktop, older gamescope). The pad→UI path
/// gates on this: a Steam game is never owned (see header), so the navpad stays out of
/// it, but without this check every press in the game was ALSO forwarded to the hidden
/// dashboard — on the couch box (2026-09-13) each Cross in KH3 fired the focused tile
/// behind the game (Jellyfin, then Assassin's Creed) and the focus flips wedged Steam
/// Input. Cached for 50 ms: gilrs delivers axis events at pad rate.
pub fn omnideck_in_front() -> bool {
    use std::time::{Duration, Instant};
    static CACHE: Mutex<Option<(Instant, bool)>> = Mutex::new(None);
    let mut cache = crate::sync::lock_or_recover(&CACHE, "switcher.IN_FRONT");
    if let Some((at, v)) = *cache {
        if at.elapsed() < Duration::from_millis(50) {
            return v;
        }
    }
    let me = std::process::id();
    let v = with_x11(|conn, root| match gamescope_focused_window(conn, root) {
        Some(focused) => window_pid(conn, focused) == Some(me),
        None => true,
    })
    .unwrap_or(true);
    *cache = Some((Instant::now(), v));
    v
}

/// `_NET_WM_PID` of a window, if it carries one.
fn window_pid(conn: &RustConnection, win: Window) -> Option<u32> {
    let atom = conn.intern_atom(false, b"_NET_WM_PID").ok()?.reply().ok()?.atom;
    let prop = conn.get_property(false, win, atom, AtomEnum::CARDINAL, 0, 1).ok()?.reply().ok()?;
    prop.value32().and_then(|mut v| v.next())
}

/// Resolve the foreground group using gamescope's actual presented X window. A mapped
/// background app is not foreground; a Steam-tagged window is never in `visible`.
fn foreground_group(conn: &RustConnection, root: Window, visible: &[(Window, u32)]) -> Option<u32> {
    if let Some(win) = gamescope_focused_window(conn, root) {
        return visible.iter().find(|&&(w, _)| w == win).map(|&(_, group)| group);
    }
    // Older gamescope / nested desktop: X input focus may be a child of the toplevel.
    let mut win = conn.get_input_focus().ok()?.reply().ok()?.focus;
    for _ in 0..16 {
        if let Some(&(_, group)) = visible.iter().find(|&&(w, _)| w == win) { return Some(group); }
        let parent = conn.query_tree(win).ok()?.reply().ok()?.parent;
        if parent == 0 || parent == win { break; }
        win = parent;
    }
    None
}

fn deck_is_focused(conn: &RustConnection, root: Window) -> bool {
    if let Some(win) = gamescope_focused_window(conn, root) {
        return window_pid(conn, win) == Some(std::process::id());
    }
    let Ok(Ok(focus)) = conn.get_input_focus().map(|c| c.reply()) else { return false };
    let mut win = focus.focus;
    for _ in 0..16 {
        if let Some(pid) = window_pid(conn, win) { return pid == std::process::id(); }
        let Ok(Ok(tree)) = conn.query_tree(win).map(|c| c.reply()) else { return false };
        if tree.parent == 0 || tree.parent == win { break; }
        win = tree.parent;
    }
    false
}

/// Navpad compatibility: keep HEAD's gamescope focus gate and Steam exclusion.
pub fn owned_app_focused() -> bool { any_app_visible() }

#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SwitchResult { Home, App, Unchanged }

/// Tap from an owned foreground app returns Home; tap from Home returns the most recent
/// *running* owned app. Never raise an owned window over Steam or another foreign window.
pub fn home_toggle() -> SwitchResult {
    if !session_ok() { return SwitchResult::Unchanged; }
    let state = with_x11(|conn, root| {
        let visible = visible_owned(conn, root, &crate::watchdog::live_groups());
        (foreground_group(conn, root, &visible), deck_is_focused(conn, root))
    });
    let Some((front, on_deck)) = state else { return SwitchResult::Unchanged };
    if let Some(group) = front {
        remember_group(group);
        if hide_all() {
            deck_home(); // tap commits Home; a later overview cancel must not reopen the app
            return SwitchResult::Home;
        }
        return SwitchResult::Unchanged;
    }
    if !on_deck { return SwitchResult::Unchanged; }
    for app in recent_apps() {
        if show_group(app.group) { return SwitchResult::App; }
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
/// them, freeze the silent ones. Returns true once Home is foreground (including empty overview).
pub fn hide_all() -> bool {
    if !session_ok() {
        return false;
    }
    with_x11(|conn, root| {
        let visible = visible_owned(conn, root, &crate::watchdog::live_groups());
        let front = foreground_group(conn, root, &visible);
        if front.is_none() {
            if !deck_is_focused(conn, root) { return false; }
            // Overview opened from Home: cancel stays Home. STOPPED/LEADERLESS retain
            // their own identity-checked records for close-all and session-exit recovery.
            deck_home();
            return true;
        }
        let wins: Vec<Window> = visible.iter().map(|&(w, _)| w).collect();
        let failed = set_mapped(conn, &wins, false);
        if !failed.is_empty() {
            let succeeded: Vec<_> = wins.iter().copied().filter(|w| !failed.contains(w)).collect();
            let _ = set_mapped(conn, &succeeded, true);
            return false; // never freeze or claim Home on a partial unmap
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
        let hidden_now: Vec<Window> = visible.iter().filter(|&&(_, group)| Some(group) == front)
            .map(|&(win, _)| win).collect();
        let frozen_front: Vec<u32> = frozen_now.into_iter().filter(|g| Some(*g) == front).collect();
        *crate::sync::lock_or_recover(&LAST_HIDE, "switcher.LAST_HIDE") = (hidden_now, frozen_front, front);
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
    with_x11(|conn, root| {
        let (wins, groups, foreground) = crate::sync::lock_or_recover(&LAST_HIDE, "switcher.LAST_HIDE").clone();
        let Some(group) = foreground else { return true; }; // overview opened from Home
        let owned = windows_of_group(conn, root, group);
        let restore: Vec<Window> = wins.into_iter().filter(|w| owned.contains(w)).collect();
        if restore.is_empty() {
            // A failed X query and an exited app look identical here. Only discard the
            // snapshot after the tracked group AND any frozen/leaderless survivors are gone.
            let tracked = crate::watchdog::live_groups().contains(&group)
                || crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").iter().any(|f| f.group == group)
                || crate::sync::lock_or_recover(&LEADERLESS, "switcher.LEADERLESS").iter().any(|f| f.group == group);
            if tracked { return false; }
            deck_home();
            return true; // verified exit while overview was open
        }
        let failed = set_mapped(conn, &restore, true);
        if !failed.is_empty() {
            let succeeded: Vec<Window> = restore.iter().copied().filter(|w| !failed.contains(w)).collect();
            let _ = set_mapped(conn, &succeeded, false);
            return false; // keep snapshot and STOPPED for retry
        }
        let prior = if groups.contains(&group) {
            crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED")
                .iter().find(|f| f.group == group).cloned()
        } else { None };
        if prior.is_some() && !cont_group(group) {
            let _ = set_mapped(conn, &restore, false);
            return false;
        }
        if !wait_for_group_focus(conn, root, group) {
            let _ = set_mapped(conn, &restore, false);
            repark_after_failed_focus(conn, &restore, group, prior);
            return false; // retain LAST_HIDE for retry
        }
        crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").retain(|f| f.group != group);
        remember_group(group);
        *crate::sync::lock_or_recover(&LAST_HIDE, "switcher.LAST_HIDE") = (Vec::new(), Vec::new(), None);
        crate::sync::lock_or_recover(&HIDDEN, "switcher.HIDDEN").retain(|w| !restore.contains(w));
        true
    })
    .unwrap_or(false)
}

/// Explicit Home selection retains hidden groups but drops the cancel snapshot.
pub fn deck_home() {
    *crate::sync::lock_or_recover(&LAST_HIDE, "switcher.LAST_HIDE") = (Vec::new(), Vec::new(), None);
}

fn wait_for_group_focus(conn: &RustConnection, root: Window, group: u32) -> bool {
    for _ in 0..10 {
        let visible = visible_owned(conn, root, &[group]);
        if foreground_group(conn, root, &visible) == Some(group) { return true; }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    false
}

/// A failed focus transfer after SIGCONT must not leave a stale STOPPED record for a
/// running process, or lose the only identity-checked handle when its leader vanished.
fn repark_after_failed_focus(conn: &RustConnection, wins: &[Window], group: u32, prior: Option<Frozen>) {
    let Some(prior) = prior else { return };
    let still_viewable = wins.iter().any(|w| conn.get_window_attributes(*w).ok()
        .and_then(|c| c.reply().ok()).is_some_and(|a| a.map_state == MapState::VIEWABLE));
    let refrozen = if still_viewable { None } else { stop_group(group) };
    let mut stopped = crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED");
    stopped.retain(|f| f.group != group);
    if let Some(f) = refrozen { stopped.push(f); }
    else {
        drop(stopped);
        remember_leaderless(&prior); // close-all can still reach verified survivors
        tracing::warn!("switcher: could not re-freeze group {group} after failed focus");
    }
}

fn rollback_show(conn: &RustConnection, wins: &[Window], was_visible: &[Window]) {
    let newly_shown: Vec<Window> = wins.iter().copied().filter(|w| !was_visible.contains(w)).collect();
    let failed_hide = set_mapped(conn, &newly_shown, false);
    if !failed_hide.is_empty() { tracing::warn!("switcher: rollback could not re-hide {} window(s)", failed_hide.len()); }
    let failed_restore = set_mapped(conn, was_visible, true);
    if !failed_restore.is_empty() {
        let mut hidden = crate::sync::lock_or_recover(&HIDDEN, "switcher.HIDDEN");
        for win in failed_restore { if !hidden.contains(&win) { hidden.push(win); } }
        tracing::warn!("switcher: could not restore a previously mapped window");
    }
}

/// Bring ONE launched app group to the front (the deck-switcher's "open this card"): map
/// its toplevels, then resume it if frozen. Other apps stay hidden. Returns true on success.
pub fn show_group(group: u32) -> bool {
    if !session_ok() || !crate::watchdog::live_groups().contains(&group) { return false; }
    with_x11(|conn, root| {
        let wins = windows_of_group(conn, root, group);
        if wins.is_empty() { return false; }
        let visible = visible_owned(conn, root, &crate::watchdog::live_groups());
        let already_focused = foreground_group(conn, root, &visible) == Some(group);
        let on_deck = deck_is_focused(conn, root);
        if !on_deck && !already_focused { return false; } // never raise over Steam/foreign focus
        let was_visible: Vec<Window> = visible.iter().filter(|&&(_, g)| g == group).map(|&(w, _)| w).collect();
        if on_deck && !was_visible.is_empty() {
            let resisted = set_mapped(conn, &was_visible, false);
            if !resisted.is_empty() {
                rollback_show(conn, &wins, &was_visible);
                return false;
            }
        }
        let failed = set_mapped(conn, &wins, true);
        if failed.len() == wins.len() {
            rollback_show(conn, &wins, &was_visible);
            return false;
        }
        let prior = crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED")
            .iter().find(|f| f.group == group).cloned();
        if !cont_group(group) {
            rollback_show(conn, &wins, &was_visible);
            return false; // preserve STOPPED and cancel snapshot for retry
        }
        if on_deck && !wait_for_group_focus(conn, root, group) {
            rollback_show(conn, &wins, &was_visible);
            repark_after_failed_focus(conn, &wins, group, prior);
            return false;
        }
        crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").retain(|f| f.group != group);
        remember_group(group);
        *crate::sync::lock_or_recover(&LAST_HIDE, "switcher.LAST_HIDE") = (Vec::new(), Vec::new(), None);
        crate::sync::lock_or_recover(&HIDDEN, "switcher.HIDDEN")
            .retain(|w| !wins.contains(w) || failed.contains(w));
        true
    })
    .unwrap_or(false)
}

/// The STEAM_GAME atom (Steam stamps it on Big Picture — 769 — and on every game window;
/// OmniDeck stamps 769 on its own window in a session, which ownership already excludes by
/// pid). `None` only if the X server could not intern it, in which case nothing is skipped.
fn steam_game_atom(conn: &x11rb::rust_connection::RustConnection) -> Option<x11rb::protocol::xproto::Atom> {
    conn.intern_atom(false, b"STEAM_GAME").ok()?.reply().ok().map(|r| r.atom)
}

/// Does `win` carry a STEAM_GAME property? Such windows are never owned (see header).
fn is_steam_window(
    conn: &x11rb::rust_connection::RustConnection,
    win: Window,
    steam_game: Option<x11rb::protocol::xproto::Atom>,
) -> bool {
    let Some(atom) = steam_game else { return false };
    conn.get_property(false, win, atom, AtomEnum::CARDINAL, 0, 1)
        .ok()
        .and_then(|c| c.reply().ok())
        .is_some_and(|p| p.value32().and_then(|mut v| v.next()).is_some())
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
    let steam_game = steam_game_atom(conn);
    let Ok(Ok(tree)) = conn.query_tree(root).map(|c| c.reply()) else { return Vec::new() };
    let mut out = Vec::new();
    for &win in &tree.children {
        if is_steam_window(conn, win, steam_game) {
            continue;
        }
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
        if stopped.iter().any(|f| f.group == g) {
            continue;
        }
        if let Some(record) = stop_group(g) {
            tracing::info!(
                "switcher: froze silent hidden group {g} ({} member(s))",
                record.members.len()
            );
            stopped.push(record);
            frozen.push(g);
        }
    }
    frozen
}

/// SIGCONT everything `freeze_silent_groups` stopped (dead groups just fail the kill).
/// Also the process-exit hook (lib.rs): frozen groups must not outlive the launcher —
/// SIGTERM can't wake a SIGSTOPped process, so exiting without this stranded them forever.
pub(crate) fn resume_stopped_groups() {
    let stopped: Vec<Frozen> =
        std::mem::take(&mut *crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED"));
    // Put back anything that could NOT be thawed rather than dropping it: on the toggle path a
    // retained record means the next Guide press tries again, and on the exit path it is at
    // least visible in the log instead of silently stranded.
    let mut stuck = Vec::new();
    for f in stopped {
        match cont_frozen(&f) {
            Thaw::Done => {}
            // Left running with no LIVE_GROUPS entry — keep a handle so close-all can reach it.
            Thaw::Leaderless => remember_leaderless(&f),
            Thaw::Stuck => stuck.push(f),
        }
    }
    if !stuck.is_empty() {
        tracing::warn!("switcher: {} frozen group(s) would not thaw — kept for retry", stuck.len());
        crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").extend(stuck);
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
    use super::{
        cont_frozen, cont_group, group_members, pgid_of, stop_group, SwitchResult, Thaw, LEADERLESS, STOPPED,
    };

    /// Held by every test that touches the process-wide registries (`STOPPED`, watchdog's
    /// `LIVE_GROUPS`). Cargo runs tests as threads in ONE process, and `return_home` signals
    /// every group in those registries — so without this, the close-all test would SIGTERM the
    /// thaw test's leader out from under it. Poison-tolerant like the rest of the codebase: a
    /// failing test must not turn its neighbour into a second, misleading failure.
    static REGISTRIES: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn guide_tap_result_reports_direction_for_dashboard_reset() {
        assert_eq!(serde_json::to_string(&SwitchResult::Home).unwrap(), "\"home\"");
        assert_eq!(serde_json::to_string(&SwitchResult::App).unwrap(), "\"app\"");
        assert_eq!(serde_json::to_string(&SwitchResult::Unchanged).unwrap(), "\"unchanged\"");
    }

    #[test]
    fn pgid_of_self_is_nonzero_and_bogus_pid_is_zero() {
        assert_ne!(pgid_of(std::process::id()), 0);
        assert_eq!(pgid_of(0), 0); // /proc/0 never exists
    }

    /// Run state from /proc/<pid>/stat field 3 — 'T' is SIGSTOPped. None once the pid is gone.
    fn proc_state(pid: u32) -> Option<char> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        stat.rsplit_once(')')?.1.split_whitespace().next()?.chars().next()
    }

    /// Poll `f` for up to ~2s. Process state changes are asynchronous, so the alternative is
    /// a flaky fixed sleep.
    fn wait_for(mut f: impl FnMut() -> bool) -> bool {
        for _ in 0..200 {
            if f() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        false
    }

    /// SIGKILLs the whole test group on the way out, however the test ends — a SIGSTOPped
    /// `sleep` left behind by a failed assertion would otherwise never wake up to exit.
    struct KillGroup(u32);
    impl Drop for KillGroup {
        fn drop(&mut self) {
            unsafe { libc::kill(-(self.0 as i32), libc::SIGKILL) };
        }
    }

    /// Regression (review 2026-09-10, finding 1): a frozen process group whose LEADER dies
    /// must still be thawable.
    ///
    /// The freeze/thaw path verified identity only through the group leader — a LIVE_GROUPS
    /// record plus a readable leader start time. But a process group outlives its leader, and
    /// `watch_child`'s reaper removes the record the moment the leader exits. So: hide an app
    /// (group SIGSTOPped), leader dies, record removed — and every surviving member stayed in
    /// state 'T' forever, because the thaw now refused to signal a group it could no longer
    /// identify. This walks that exact lifecycle against real processes.
    ///
    /// **The member is deliberately OUR child, `setpgid`'d into the leader's group, not a
    /// child of the leader.** That detail is what makes this test test anything. If every
    /// survivor were a child of the leader, killing the leader would leave the group with no
    /// member whose parent is in another group in the same session — i.e. *orphaned* — and
    /// POSIX then has the kernel send SIGHUP+SIGCONT to the whole group. The survivors get
    /// thawed (and usually killed by the SIGHUP) by the kernel, which masks the bug entirely:
    /// written that way, this test passes against the unfixed code. Keeping a live parent
    /// outside the group is what holds the group un-orphaned and reproduces the real strand.
    #[test]
    fn frozen_group_is_thawed_after_its_leader_dies() {
        use std::os::unix::process::CommandExt as _;
        let _serial = crate::sync::lock_or_recover(&REGISTRIES, "switcher.tests.REGISTRIES");

        let mut leader = std::process::Command::new("sh")
            .args(["-c", "sleep 30"])
            .process_group(0) // its own group, the way every launch spawns
            .spawn()
            .expect("spawn test group leader");
        let group = leader.id();
        let _cleanup = KillGroup(group);

        // A second member of the SAME group whose parent is this test process — see above.
        let mut member_child = std::process::Command::new("sleep")
            .arg("30")
            .process_group(group as i32)
            .spawn()
            .expect("spawn group member");
        let member_pid = member_child.id();

        assert!(wait_for(|| group_members(group).len() >= 2), "second member never appeared");
        crate::watchdog::track_group_for_test(group); // what a launch records

        // Hide → freeze. The roster is captured here, while the group cannot fork.
        let frozen = stop_group(group).expect("SIGSTOP should land on a live tracked group");
        assert!(frozen.members.len() >= 2, "roster missed a member: {:?}", frozen.members);
        assert!(frozen.members.iter().any(|&(p, _)| p == member_pid), "member not in roster");
        assert!(wait_for(|| frozen.members.iter().all(|&(p, _)| proc_state(p) == Some('T'))));

        // The leader dies and is reaped, and the watcher drops its record.
        let member = frozen
            .members
            .iter()
            .find(|&&(p, _)| p == member_pid)
            .copied()
            .expect("a non-leader member");
        unsafe { libc::kill(group as i32, libc::SIGKILL) };
        leader.wait().unwrap();
        crate::watchdog::forget_group_for_test(group);
        assert!(wait_for(|| proc_state(group).is_none()), "leader was not reaped");

        // The survivor is still frozen, and the leader-based check can no longer vouch for it
        // — this is precisely the state the old code called "done".
        assert_eq!(proc_state(member.0), Some('T'));
        assert!(
            !crate::watchdog::signal_group_verified(group, libc::SIGCONT),
            "leader-based verification should fail once the leader is gone"
        );

        // The fix: per-member identity recorded at freeze time thaws the survivor. The outcome
        // is `Leaderless`, not `Done` — the distinction is what tells the re-show path this
        // group is now running with nothing in LIVE_GROUPS able to hide or close it.
        assert_eq!(
            cont_frozen(&frozen),
            Thaw::Leaderless,
            "leaderless group reported as un-thawable"
        );
        assert!(
            wait_for(|| proc_state(member.0) != Some('T')),
            "surviving member left SIGSTOPped"
        );

        unsafe { libc::kill(member_pid as i32, libc::SIGKILL) };
        let _ = member_child.wait();
    }

    /// The other half of the same guard: per-member thawing must stay recycle-safe. A recorded
    /// pid whose start time no longer matches is a DIFFERENT process, and signalling it is
    /// exactly the bug the leader check existed to prevent — so the fallback must refuse it
    /// rather than trade one stranding bug for one stray-signal bug.
    #[test]
    fn member_thaw_refuses_a_recycled_pid() {
        use super::Frozen;

        // Our own pid with a deliberately wrong start time stands in for a recycled member:
        // same pid, different process. If the check were dropped, this would SIGSTOP the test
        // runner itself — which is why it asserts on a signal that would be very obvious.
        let me = std::process::id();
        let real = crate::watchdog::proc_start_time(me).unwrap();
        assert!(!crate::watchdog::signal_pid_verified(me, real ^ 0xffff, libc::SIGSTOP));

        // ...and a group with nothing verifiable left is reported as settled, not retried
        // forever (there is no one to strand).
        let dead = Frozen { group: u32::MAX, members: vec![(u32::MAX, 1)] };
        assert_eq!(cont_frozen(&dead), Thaw::Done);

        // pid 0 (kill's "our own group") and pid 1 (init) are never valid targets.
        assert!(!crate::watchdog::signal_pid_verified(0, 0, libc::SIGCONT));
        assert!(!crate::watchdog::signal_pid_verified(1, 1, libc::SIGCONT));
    }

    /// Spawn `sh -c "sleep 30"` as its own group leader plus a second member of that group whose
    /// parent is THIS test process. That parentage is load-bearing everywhere it is used: a group
    /// made only of the leader's own children becomes *orphaned* when the leader dies, and POSIX
    /// then has the kernel SIGHUP+SIGCONT it — which thaws the survivor for us and makes these
    /// tests pass against unfixed code. A live parent outside the group holds it un-orphaned.
    fn spawn_group_with_outside_parented_member(
    ) -> (std::process::Child, std::process::Child, u32, u32) {
        use std::os::unix::process::CommandExt as _;
        let leader = std::process::Command::new("sh")
            .args(["-c", "sleep 30"])
            .process_group(0) // its own group, the way every launch spawns
            .spawn()
            .expect("spawn test group leader");
        let group = leader.id();
        let member = std::process::Command::new("sleep")
            .arg("30")
            .process_group(group as i32)
            .spawn()
            .expect("spawn group member");
        let member_pid = member.id();
        assert!(wait_for(|| group_members(group).len() >= 2), "second member never appeared");
        (leader, member, group, member_pid)
    }

    /// Regression (leaderless session test, 2026-09-10): re-showing a leaderless survivor must
    /// not hand the user a window nothing can close.
    ///
    /// Dismissing the deck thaws the group and maps its windows back. For a group whose leader
    /// has been reaped that is a one-way door: `LIVE_GROUPS` will never list it again, so the
    /// re-shown window is invisible to `return_home` — the Guide hold, the user's last escape
    /// hatch, would run and reach nothing. Fixing the strand without this just moves it from
    /// "frozen and invisible" to "running and unclosable".
    #[test]
    fn a_leaderless_group_re_shown_from_the_deck_can_still_be_closed() {
        let _serial = crate::sync::lock_or_recover(&REGISTRIES, "switcher.tests.REGISTRIES");
        let (mut leader, mut member_child, group, member_pid) =
            spawn_group_with_outside_parented_member();
        let _cleanup = KillGroup(group);

        crate::watchdog::track_group_for_test(group);
        let frozen = stop_group(group).expect("SIGSTOP should land on a live tracked group");
        let member = *frozen.members.iter().find(|&&(p, _)| p == member_pid).expect("in roster");
        assert!(wait_for(|| proc_state(member.0) == Some('T')), "member never froze");
        crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").push(frozen);

        unsafe { libc::kill(group as i32, libc::SIGKILL) };
        leader.wait().unwrap();
        crate::watchdog::forget_group_for_test(group);
        assert!(wait_for(|| proc_state(group).is_none()), "leader was not reaped");

        // Deck dismissed: the thaw lands and the caller drops the STOPPED record (deck_cancel
        // and show_group both do exactly this on a `true`).
        assert!(cont_group(group), "leaderless thaw should report the record as droppable");
        crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").retain(|f| f.group != group);
        assert!(wait_for(|| proc_state(member.0) != Some('T')), "survivor left SIGSTOPped");
        assert!(
            crate::sync::lock_or_recover(&LEADERLESS, "switcher.LEADERLESS")
                .iter()
                .any(|f| f.group == group),
            "a re-shown leaderless group must leave a handle behind"
        );

        // Guide hold. With no LIVE_GROUPS entry and no STOPPED record, this handle is all there is.
        assert!(crate::watchdog::return_home(), "close-all reported that it reached nothing");
        assert!(
            wait_for(|| matches!(proc_state(member.0), None | Some('Z'))),
            "re-shown survivor was not closed (state {:?})",
            proc_state(member.0)
        );
        let _ = member_child.wait();
        assert!(
            crate::sync::lock_or_recover(&LEADERLESS, "switcher.LEADERLESS")
                .iter()
                .all(|f| f.group != group),
            "closed group left a leaderless handle behind"
        );
    }

    /// Regression (leaderless session test, 2026-09-10): the Guide HOLD — "close everything" —
    /// must reach a frozen group whose leader has already exited.
    ///
    /// Same lifecycle as the thaw test above, and the same reason it was broken: `return_home`
    /// walks `LIVE_GROUPS`, which is keyed on the launch leader, and `watch_child`'s reaper
    /// drops that entry the moment the leader exits. The group's members keep the pgid and stay
    /// SIGSTOPped behind an unmapped window — invisible to close-all, so the hold ran, signalled
    /// nobody, reported false, and the survivor sat in state 'T' behind a window the user could
    /// neither see nor close. Having the per-group helper (`forget_stopped`) did not cover this:
    /// nothing on the Guide-hold path called it.
    ///
    /// The member is our child `setpgid`'d into the leader's group, NOT a child of the leader —
    /// see the thaw test for why that topology is load-bearing (a group of the leader's own
    /// children becomes orphaned on its death and the kernel SIGHUP+SIGCONTs it, which makes
    /// this test pass against unfixed code).
    #[test]
    fn close_all_reaches_a_frozen_group_that_lost_its_leader() {
        use std::os::unix::process::CommandExt as _;
        let _serial = crate::sync::lock_or_recover(&REGISTRIES, "switcher.tests.REGISTRIES");

        let mut leader = std::process::Command::new("sh")
            .args(["-c", "sleep 30"])
            .process_group(0)
            .spawn()
            .expect("spawn test group leader");
        let group = leader.id();
        let _cleanup = KillGroup(group);

        let mut member_child = std::process::Command::new("sleep")
            .arg("30")
            .process_group(group as i32)
            .spawn()
            .expect("spawn group member");
        let member_pid = member_child.id();
        assert!(wait_for(|| group_members(group).len() >= 2), "second member never appeared");

        // Launch → hide: the group is tracked, then frozen and recorded exactly as hide_all
        // records it (freeze_silent_groups pushes the roster into STOPPED).
        crate::watchdog::track_group_for_test(group);
        let frozen = stop_group(group).expect("SIGSTOP should land on a live tracked group");
        let member = frozen
            .members
            .iter()
            .find(|&&(p, _)| p == member_pid)
            .copied()
            .expect("member not in roster");
        assert!(wait_for(|| proc_state(member.0) == Some('T')), "member never froze");
        crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED").push(frozen);

        // The leader exits and is reaped — the group leaves LIVE_GROUPS, survivor and all.
        unsafe { libc::kill(group as i32, libc::SIGKILL) };
        leader.wait().unwrap();
        crate::watchdog::forget_group_for_test(group);
        assert!(wait_for(|| proc_state(group).is_none()), "leader was not reaped");
        assert!(
            !crate::watchdog::live_groups().contains(&group),
            "the reaped group must be invisible to the LIVE_GROUPS close loop"
        );
        assert_eq!(proc_state(member.0), Some('T'), "survivor should still be frozen");

        // The Guide hold. Before the fix this walked an empty list and returned false, so the
        // caller never even logged a close.
        assert!(crate::watchdog::return_home(), "close-all reported that it reached nothing");
        // 'Z' counts: TERM landed and the survivor is ours to reap. Anything but 'T' would be a
        // weaker assertion — it would also pass on a bare thaw that never closed anything.
        assert!(
            wait_for(|| matches!(proc_state(member.0), None | Some('Z'))),
            "surviving member was not closed (state {:?})",
            proc_state(member.0)
        );
        let _ = member_child.wait();

        // Closed means forgotten: a record left behind would have the exit hook blanket-CONT a
        // pgid that may since have been recycled.
        assert!(
            !crate::sync::lock_or_recover(&STOPPED, "switcher.STOPPED")
                .iter()
                .any(|f| f.group == group),
            "closed group left a frozen record behind"
        );
    }
}
