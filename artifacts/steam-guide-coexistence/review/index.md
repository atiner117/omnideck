---
title: "Review: 0252dc5 fix(switcher): Guide Home wins over Steam Big Picture"
kind: review
status: done
created: 2026-10-09
updated: 2026-10-09
thread: claude-code:reviewer-subagent (Opus 5.5, first-pass same-family lane)
---

# Review of 0252dc5 (parent 17b1857, branch fix/steam-guide-coexistence)

Scope: `git show 0252dc5` (switcher.rs +102, watchdog.rs +5, packaging/test-session.sh +30),
read against the surrounding switcher/watchdog/gamepad/navpad/commands code, the fixup ticket
(`origin/feat/guide-home-recent-apps:artifacts/recent-apps-switcher/tickets/input-and-state/fixup-steam-guide-coexistence/index.md`)
and gamescope `steamcompmgr.cpp` (`is_focus_priority_greater` ~4142, `pick_primary_focus_and_override` ~4328).
I did not rebuild or re-run the tests: there is no `target/` cache in this worktree, and the
findings below are about runtime behavior, which a unit run cannot show. "VERIFIED" means I
confirmed it by reading the code paths. "PLAUSIBLE" means it depends on Steam client behavior
nobody has probed yet.

The approach is sound. It re-maps OmniDeck's own toplevel so the deck becomes the newest
mapped window, which is the compositor's own focus rule, and it never touches a Steam window.
That is better than the ticket's "unmap Steam's BPM window" proposal. No lock-order deadlock
exists: nothing reachable from `settle_on_deck` or the guard closure re-enters `with_x11`.
`navpad::invalidate` is an atomic, `live_groups` takes only `LIVE_GROUPS`, `steam_game_active`
runs outside `with_x11`, and no path holds `LIVE_GROUPS`/`STEAM_WATCHED` and then calls
`with_x11`. The problems are what the re-map does to focus *ordering*, and one unrecoverable
failure path.

## Findings (most severe first)

### 1. HIGH: re-mapping the deck lifts it above a running Steam game that OmniDeck did not launch (violates "never steal focus from a running Steam game") (PLAUSIBLE)
`src-tauri/src/switcher.rs:571-580` (home_toggle `!on_deck` branch), `:530-544` (guard), `:497-506` (settle re-map).

`steam_game_active()` (`watchdog.rs:310`) only knows games launched through OmniDeck's
`launch_game` (`STEAM_WATCHED`). A game started from Big Picture is invisible to it. Without
`-e`, gamescope ranks game-id windows by `map_sequence` (steamcompmgr.cpp ~4180). Before this
commit OmniDeck's map_sequence was from session start, so any game outranked it. After a
re-map it outranks everything already mapped, **including a running game behind BPM**.

Failure scenario: the user launches a game from Big Picture (not from OmniDeck) and taps PS
in-game. OmniDeck sees the game focused, so `steam_client_focused` is false and the result is
`Unchanged`. Steam then raises BPM ("Resume game" panel). The user taps PS again to resume.
OmniDeck now sees BPM (769) focused, `steam_game_active()` is false, and `settle_on_deck`
re-maps the deck. OmniDeck goes Home and spawns a guard. Steam handles the same press and
resumes the game. If Steam resumes by unmapping/withdrawing BPM, gamescope picks the newest
remaining window, which is now the **deck, not the game**. The user is on OmniDeck Home with
their game running unfocused behind it. Guide from Home only cycles owned apps
(`recent_apps`), so the only way back is Steam's own PS handling, and for 2.5 s the guard takes
that back too. The same reordering happens when the guard fires while a non-OmniDeck game is
running behind BPM.

Fix: before any deck re-map (in `settle_on_deck`, used by all three callers), refuse if a
Steam game is live by *any* launcher, not just OmniDeck's. Either scan `/proc` for any
`reaper SteamLaunch AppId=` (generalise `steam_app_process_running`/`cmdline_is_steam_launch`
to "any appid", excluding `Install=1`), or check for any VIEWABLE root child whose `STEAM_GAME`
is present and not 769. Replace `steam_game_active()` at `:534`/`:574` with that helper.
Before trusting the BPM→Home branch, probe on r2d2 how Steam resumes a game from BPM
(unmap vs `GAMESCOPECTRL_BASELAYER_*`).

### 2. HIGH: a failed deck re-map leaves OmniDeck's own window unmapped forever (unrecoverable black/Steam screen) (VERIFIED by code path)
`src-tauri/src/switcher.rs:501-506`, `:465-473`.

`settle_on_deck` unmaps the deck (`set_mapped(&mine,false)`) and then re-maps it. If the map
does not confirm within `set_mapped`'s 8 attempts (~420 ms), the function logs and returns
`false` with **OmniDeck still unmapped**. The `set_mapped` doc comment (`:950-955`) says map
requests through steamcompmgr "can get swallowed — seen live in the nested-session harness",
so this is a known transient, not a hypothetical. Results by caller:
- `hide_all` (`:622-626`) re-maps the app, so the app is in front and OmniDeck is gone.
- the guard (`:538`) ignores the result entirely.
- the `!on_deck` branch returns `Unchanged`.

There is no recovery after that. `own_toplevels` filters on `MapState::VIEWABLE` (`:470`), so
every later attempt finds `mine.is_empty()` and returns false (`:499`). OmniDeck is the
session's only real client. Once its window is unmapped, the user's only exits are Steam or a
logout. That violates "failures must be recoverable".

Fix: in `own_toplevels`, include *unmapped* own InputOutput toplevels that are not useless
(1x1/`InputOnly`), or cache the deck window id once (e.g. the window `set_steam_game_atom_if_gamescope`
stamps). On a failed re-map, keep retrying the map (it is our own window, so re-mapping it is
always safe) before returning, and log at error level from the guard too. Better still,
never return from `settle_on_deck` with the deck unmapped.

### 3. MEDIUM: the guard cannot be cancelled and will undo deliberate user moves made within 2.5 s (VERIFIED for the BPM tile; PLAUSIBLE timing)
`src-tauri/src/switcher.rs:530-544`, `apps.rs:57-58`.

The guard's only cancel conditions are `steam_game_active()` and "fired once". It does not
notice that the user has left Home since it was spawned. Scenarios:
- **Default "Big Picture" tile** (`apps.rs:58`, `steam steam://open/bigpicture`, launched via
  `watch_child`, so it is not in `STEAM_WATCHED`). The user taps Guide (Home), moves to the
  tile, and presses A within 2.5 s. With Steam warm, BPM maps in ~0.5 s, the guard sees a 769
  window focused, and it re-maps the deck. The deliberate launch is reverted. It is one-shot,
  so a retry works, but it looks broken. The same applies to `steam://gameproperties`
  (`commands.rs:690`) and any custom `steam://…` tile.
- **Home → app → Steam re-answers.** Tap 1 goes Home and arms guard A. Steam happens not to
  answer tap 1, or answers late. Tap 2 within 2.5 s runs `show_group(app)` and the app comes
  front. Steam answers tap 2 by raising BPM ~0.4 s later. Guard A fires, `foreground_group` is
  None because BPM is focused, so the deck is re-mapped **over the just-restored app**. The user
  asked for the app and gets Home. It is recoverable on the next tap, but it breaks
  acceptance check 1 ("second tap restores the same Jellyfin").
- Each Home spawns a new guard and older guards keep running, so rapid Home/App/Home taps can
  stack several one-shot reclaims.

Fix: add a switcher epoch (an `AtomicU64` bumped by `show_group`, `deck_cancel`, every launch
command, and each new guard). The guard exits when the epoch moves. That also makes "only the
newest guard is live" automatic.

### 4. MEDIUM: the pooled X connection is held for up to ~1.4 s by the guard (≤~2.7 s inside hide_all), stalling the gamepad thread while the user is navigating (VERIFIED)
`src-tauri/src/switcher.rs:535-540` (guard holds `with_x11` across `settle_on_deck`), `:488-491`
and `:507-510` (50 ms sleep loops), `set_mapped` sleeps (`:982-984`); callers that block:
`gamepad.rs:198` (`omnideck_in_front`, every forwarded pad event once its 50 ms cache expires) and
`navpad.rs:161` (`owned_app_focused`, forced by the generation bump `set_mapped` itself makes at `:989`).

When the guard fires, it holds the `X11` mutex across unmap (≤420 ms), map (≤420 ms) and the
focus poll (≤500 ms). In `hide_all` the new settle adds a first poll (≤500 ms) and the
failure-path app re-map (≤420 ms) to a closure that already runs `pactl` (≤3 s) under the same
lock. The guard fires ~0.4 s after Home, which is exactly when the user starts moving on the
dashboard, so every pad event (D-pad, A, Guide) stalls in the gamilrs loop until the lock frees.
Events are delayed, not lost, and Guide press/release ordering survives because the drain
precedes `tick`. Still, a 1–1.5 s dead pad right after Home is a visible regression and breaks
"no blocking of the gamepad input thread" in spirit. No deadlock was found (see intro).

Fix: release `with_x11` between polls. Do the unmap/map in one `with_x11` call, then poll focus
with short separate `with_x11` calls and sleep outside the lock. Alternatively, give the guard
its own short-lived `RustConnection`, as the hotkey thread does. Apply the same treatment to
the two sleep loops inside `settle_on_deck`.

### 5. MEDIUM: harness "16/16" does not prove the app is *in front* after a return (test gap; VERIFIED)
`packaging/test-session.sh:145-147` (`toggle_expect` "shown" means `stub_visible` = X map state),
`:264-265` (steam-back), `:287` (pad-back), `:297` (pad-pick); `stub_visible` at `:119`.

steam-back, pad-back and pad-pick assert only that the stub is *mapped*. If real Steam raised
BPM over the restored app (the finding-3 second scenario, or Steam answering the second PS
press), those cases still pass. steam-home correctly uses `stable active_deck`, but no
"back" case checks `active_stub`. The fake-Steam stub never reacts to Guide, so the
"Steam answers PS" path is covered only by whatever real Steam happens to be running on the
test host. With `OMNIDECK_TEST_KEYBOARD_ONLY=1` or no Steam, it is not covered at all.

Fix: make the "shown" expectation `stub_visible && active_stub` (or add explicit
`wait_for 6 "stable active_stub"` after steam-back/pad-back/pad-pick). Optionally teach the fake
Steam stub to re-map itself N ms after a virtual Guide (watch the session log for
`guide: tap`) so the guard path is exercised deterministically.

### 6. LOW: Home fails outright when there is no GAMESCOPE_FOCUSED_WINDOW (desktop `OMNIDECK_FORCE_HOTKEY`, older gamescope) (VERIFIED by code path)
`src-tauri/src/switcher.rs:484-492`, `:622-626`.

`settled()` is true immediately when the property is absent, so `deck_is_focused` runs right
after the unmap with no wait. It falls back to X input focus, which still names the unmapped
window or has reverted to root/None (the WM/compositor refocuses asynchronously). The result is
false, `steam_client_focused` returns false (no property), and `hide_all` re-maps the app and
fails. Before this commit, Home "worked" on that path. Target gamescope 3.16.31 has the
property, so this affects the `OMNIDECK_FORCE_HOTKEY` desktop test mode (`hotkey.rs:15`).

Fix: when the property is absent, skip `settle_on_deck` (return true, as before), since the
Steam-exposure problem is a gamescope-without-`-e` problem.

### 7. LOW: the `!on_deck` BPM→Home branch leaves an owned app mapped and unfrozen behind the deck (VERIFIED)
`src-tauri/src/switcher.rs:571-580`.

Steam can cover a running owned app (e.g. Jellyfin in front, Steam answers a PS press the
guard missed). The next Guide tap then re-maps the deck over BPM, but Jellyfin stays mapped,
not in `HIDDEN`, not frozen, and not in `LAST_HIDE`. A silent PWA keeps rendering behind the
deck, which is the ~300 W case the header's freeze policy exists for, until the user toggles
back. It is functionally recoverable: `show_group` handles `on_deck && was_visible`.

Fix: after a successful settle in this branch, run the hide half (unmap + freeze silent) for
any still-viewable owned windows, or call `hide_all`'s body with the deck-focused precondition.

### 8. LOW: failure honesty: the guard result is discarded, and hide_all's "restored the app" is unverified (VERIFIED)
- `:538`: the `settle_on_deck(...)` return value is dropped. If the re-map succeeded but the
  deck never got focus (e.g. Steam pins focus via `GAMESCOPECTRL_BASELAYER_WINDOW`, which
  `pick_primary_focus_and_override` honours before map order), no message is logged and the
  frontend still believes Home.
- `:622-626`: the app is re-mapped but `wait_for_group_focus` is not consulted, so
  "restored the app" may be false. The frontend's `unchanged` toast (`+page.svelte:1184`)
  renders on the hidden deck, so the user sees nothing. Under Steam-controlled focus this
  leaves the app mapped, unfrozen, and behind BPM.

Fix: log the guard's settle failure at warn. In `hide_all`, verify with `wait_for_group_focus`
and log honestly when the restore did not take focus.

### 9. LOW: Guide *hold* (deck_open) gets the settle but no guard (PLAUSIBLE)
`commands.rs:494-501` → `hide_all`; the guard is spawned only from `home_toggle` (`:566`, `:578`).
If Steam answers the hold's press or release with BPM, the overview is covered. A then goes
nowhere: `omnideck_in_front` is false, so `gamepad.rs:198` drops it. If Steam acts on
press-down (~0.4 s, before the 800 ms `guide_hold_ms`), `deck_open` fails with "could not bring
OmniDeck to the foreground". The ares harness pad-overview/pad-pick pass, which suggests
Steam ignores long presses there, but that is unverified on r2d2 with a DualSense.
Recoverable with a tap (closes the deck and re-shows the app). Probe on r2d2. If it reproduces,
spawn the guard from `deck_open` too.

### Nits
- `own_toplevels` (`:465`) will also unmap/re-map any viewable GDK popup or TEMP window with our
  `_NET_WM_PID` (native `<select>`/context menu). It is harmless for the controller UI today; filtering
  `override_redirect` would make that explicit. External unmap makes GDK synthesize ICONIFIED, so
  WebKit briefly reports `document.hidden` (`Waves.svelte:81-84` pauses rAF). The existing
  mapped-back harness case already exercises this, so treat it as known and acceptable.
- A Steam launch that never starts keeps `STEAM_WATCHED` populated for up to 120 s
  (`watchdog.rs:388`), and during that time the guard and BPM→Home are disabled, so the original bug
  returns briefly.
- `steam_client_focused` treats a 769 window with no `_NET_WM_PID` as Steam. That is correct for CEF
  windows, but worth a comment.

## Alignment with the ticket
- Good: Steam's windows are never unmapped or signalled, and gamescope's own rule is used. That is
  a sound deviation from the ticket's "unmap BPM" proposal, but it should be recorded in the ticket
  as superseded.
- Open: acceptance checks 1–3 require **physical DualSense on r2d2**. Everything so far is the
  nested ares harness with a virtual pad, and its "back" assertions are map-state only (finding 5).
- Open: the in-game Steam path (check 2) is untested, and finding 1 is exactly that path.

## Verdict
HOLD: the deck re-map can strand OmniDeck's own window unmapped with no recovery (finding 2),
and it reorders focus above Steam games OmniDeck did not launch (finding 1). Both are small,
local fixes. Findings 3–5 should land in the same pass before r2d2 couch verification.

## Coordinator response (2026-10-09, follow-up commit)

1. **Fixed.** `steam_game_present()` refuses any re-map while a Steam game is live, from any source: OmniDeck's `SteamWatchGuard`, a viewable foreign window whose `STEAM_GAME` is non-zero and not 769, or any `/proc/*/cmdline` with a `SteamLaunch` argument. New harness case `steam-game` (a fake `reaper … SteamLaunch` plus map order app > Steam > deck) asserts that Home declines and the app stays in front. Baseline 17b1857 fails it.
2. **Fixed.** `DECK_WINS` keeps the deck toplevels across the unmap. `remap_deck()` retries the map and is called again at the start of `home_toggle`, `hide_all` and `raise_deck`, so a swallowed map request is repaired on the next press.
3. **Fixed.** `GUARD_EPOCH` is bumped by `show_group`, `deck_cancel`, `launch_game`, `launch_command` and every new guard, and the guard exits when it changes. Found while fixing: Steam also answers the Guide tap that *returns to the app*. The guard is now `guard_from_steam(target)`, so the tap back to an app gets its own one-shot guard that re-raises that app (`raise_group`).
4. **Fixed.** Every wait loop now runs outside `with_x11`. `hide_all` has three phases (unmap under the lock, settle with no lock, record/freeze under the lock). The guard and `raise_deck`/`raise_group` use short separate `with_x11` calls.
5. **Fixed.** `toggle_expect … shown` now also requires `active_stub`.
6. Desktop/no-`GAMESCOPE_FOCUSED_WINDOW`: `settle_on_deck` returns true, as before. The guard is now spawned from `hide_all`, so the Guide-hold overview is covered too. **Not addressed:** the Big-Picture→Home branch can leave an owned app that Steam covered mapped and unfrozen behind the deck. It's harmless (the app can still be picked from the overview) and is left as a note.

Verification on ares (nested gamescope 3.16.31, real Steam warmed by OmniDeck, virtual uinput pad): clippy `-D warnings` clean, `cargo test --release` 133 pass / 1 ignored, harness **17/17 on 5 consecutive runs**. Baseline 17b1857 on the same harness: 12/17. Still unverified: physical DualSense on r2d2, the A8F TV, a real Steam game in front (acceptance check 2), and a cross-family review.
