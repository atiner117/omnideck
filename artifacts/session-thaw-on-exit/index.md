---
title: "Session wrapper thaws switcher-frozen apps after OmniDeck exits"
kind: walkthrough
status: draft
created: 2026-10-14
updated: 2026-10-14
thread: claude-code:implementation-lane (fix/thaw-frozen-groups-on-exit)
---

## Problem
The deck switcher SIGSTOPs hidden silent app groups. When gamescope's primary client exits,
gamescopereaper SIGTERMs the remaining tree once and then waits for every descendant
(`src/Apps/gamescopereaper.cpp`: `KillAllChildren(SIGTERM)` + `WaitForAllChildren()`).
A stopped process does not act on that SIGTERM, so gamescope never exits: `omnideck-display
restart`, the power-menu quit and a DM restart all hung on a dead screen behind a frozen
Brave/Jellyfin PWA (r2d2).

## What already exists elsewhere (not duplicated here)
- `origin/main`: `RunEvent::Exit` calls `switcher::resume_stopped_groups()` (graceful exit only);
  `return_home()` / `close_group()` already CONT-then-TERM.
- PR #98 (`fix/reliability-20260923`, `signals.rs`): thaw on SIGTERM/SIGINT/SIGHUP.
- Neither covers SIGKILL, a panic or a crash. This change is the layer that
  does not depend on OmniDeck's last moments.

## Change
- `packaging/omnideck-session-child.sh` (installed as `/usr/local/bin/omnideck-session-child`):
  becomes gamescope's client, runs OmniDeck as a job (TERM/INT/HUP forwarded; INT/QUIT
  dispositions reset), and after it exits runs `omnideck_thaw_stopped_groups "$PPID"`:
  every state-`T` process of the same uid and SID that descends from gamescopereaper gets
  its process group CONT then TERM. Exits with OmniDeck's status.
- `install-session.sh` installs it and the generated launcher uses
  `gamescope … -- omnideck-session-child "$BIN"`. PKGBUILD ships it next to the installer.
- `packaging/test-session-thaw.sh` + a CI step.

## Review order
1. `omnideck_thaw_stopped_groups` scoping (uid + SID + ancestry to the reaper; own pgid is
   never group-signalled).
2. The wait loop and trap in the wrapper (status propagation, signal forwarding).
3. Installer/launcher wiring.

## Gotchas
- Ancestry works because gamescopereaper is a child subreaper: OmniDeck's orphans reparent
  to it, which is the wrapper's `$PPID`.
- Host wrappers that run their own gamescope (r2d2's `~/.local/bin/omnideck-session-r2d2`)
  only benefit once their client line becomes `-- /usr/local/bin/omnideck-session-child "$BIN"`.

## Verify
`bash packaging/test-session-thaw.sh` — function scoping, a negative control that
reproduces the hang under a python subreaper stand-in, the fix under the same stand-in
(SIGKILLed client, exit 137) and the teardown path (SIGTERM to the wrapper, exit 143).
On hardware: freeze a PWA via the deck, `kill -KILL $(pgrep -x omnideck)`, gamescope
must exit and the session log must show `omnideck-session-child: thawing + terminating`.
