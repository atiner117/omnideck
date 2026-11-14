---
title: "Controller Home and recent-app task overview"
kind: spec
status: done
created: 2026-09-28
updated: 2026-09-28
thread: t3code/0075e72c
---

# Controller Home and recent-app task overview

## Product contract

- **PS/Home tap** alternates: from an app, go Home without terminating it; from Home, return to the most recently foregrounded *running* app. Do not open the task overview on tap. A tap while the overview is up dismisses it, returning to the prior foreground app; Home is always reachable via a separate overview action.
- **PS/Home hold** opens the overview at the configured threshold (currently 800 ms), *while still held*. Releasing after the hold does nothing. Holding never closes/kills apps.
- The overview is a horizontally navigable most-recent-first row, with a Home card and live app cards. A opens a card; Select/keyboard Delete closes the selected app (Home cannot be closed). B/Escape dismisses to the foreground app before opening; going Home is an explicit choice. Cards identify the app rather than pretending that its launcher, game, and OS process are the same thing. A dead app disappears; no phantom resurrection of exited processes.
- "Close" is a deliberate per-card action. No implicit close-all or SIGTERM on PS/Home. Sound-producing apps may keep playing in the background. The controller overview must not make gamescope or the controller input loop block on X/pactl/kill.

## What exists / constraints

Before this slice, tap opened an owned-process deck; hold called `watchdog::return_home()` and SIGTERMs **every** owned process group. `DeckSwitcher` and `deck_open/show/close/cancel` already offer cards, but `watchdog::LIVE_GROUPS` only contains OmniDeck-spawned process groups. Steam games launch via URI, aren't in that registry, and gamescope ignores ordinary X11 activation requests; the Sep-15 couch test confirmed a Guide tap may not bring OmniDeck over a Steam game. Rockstar runs *within* the Steam GTA process tree; Heroic was independently launched by OmniDeck. Never conflate them or signal Steam's own process group to close a game. Existing frozen groups need CONT-before-TERM and guaranteed thaw on shutdown.

## Delivery slices

1. **Safe owned-app flow (this PR):** remap tap/hold; tap uses session-gated home/last *owned* app, hold enters overview; keep specific per-card close and cancel restore. Use explicit last-foreground owned group rather than re-showing all hidden groups, so choosing one app never surfaces another. Gate the virtual gamepad keyboard on X **focus**, not merely a mapped background window. Verify group start times before STOP/CONT/TERM and retain identities of frozen members if the leader exits. Add empty state/Home action; sort cards most-recent-first and make close discoverable/focusable. Update keyboard equivalent: Ctrl+Alt+Home = tap, add a separate overview shortcut; Ctrl+Alt+End remains an explicit emergency close action. Update input help and nested-session harness. Existing on-screen cards/Now Playing buttons retain their semantics. Avoid broad animation or continuous process polling.
2. **Steam support (follow-up, separate PR after live proof):** represent Steam game sessions with stable appid/launch id and an owner distinct from Steam/launcher helper processes. Prove a foreground-to-deck focus transfer on gamescope 3.16.25/NVIDIA before surfacing a Steam card as actionable; verify map/unmap of OmniDeck or another compositor-approved method. Prove a *game-only* graceful close mechanism and watchdog exit race tolerance. Until then do not advertise Steam game close as supported or kill Steam/the reaper blindly. Fallback should show an honest non-actionable Steam game indicator rather than lying that it closed.
3. **External/detached launchers:** identify actual windows by ancestry/start time (not just exact pgid), distinguish a launcher UI card from its child game, and avoid acting on shared or recycled groups. Add preview thumbnails only after a compositor-safe capture path exists; no high-frequency screenshots on the input thread.

## Validation / acceptance for slice 1

- Fast tap from owned app hides it and shows OmniDeck; second tap remaps **only** last foreground group; second app stays hidden. From Home with nothing restorable: stay Home, never launch a duplicate.
- Hold opens overview (not closes), at threshold; release is a no-op. Select card, B cancel, Home selection, per-card close; two-pad overlapping Mode events cannot accidentally trigger a tap/hold. Exit while frozen thaws stopped groups before gamescope tears down.
- Desktop (outside gamescope) never unmaps foreign windows. A failed X mapping/closed card leaves state recoverable and reports failure; no stale group accidentally signalled. Reduced-motion respected; visible button labels and focus indication pass keyboard/gamepad checks.
- `bun run check`, `bun run build`, `bun run test`, Rust clippy/test, ts-rs sync where needed; nested gamescope harness if a desktop-capable environment exists. Steam game/real DualSense and TV focus need Andrew's couch validation and must be called out, not assumed green.
