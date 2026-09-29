---
title: "Steam game focus and safe task control"
kind: ticket
status: draft
created: 2026-09-28
updated: 2026-09-28
thread: t3code/0075e72c
---

Goal: make Steam games (including GTA V Enhanced and launchers inside Proton) truthfully selectable/closable from overview. In scope: prototype gamescope focus transfer from Steam game to OmniDeck; stable appid and Steam session tracking resilient to install-script reaper gap; game-only graceful stop and exit detection; live nested and r2d2 couch validation. Out of scope: terminating Steam itself or blind SIGTERM by guessed pgid, arbitrary foreign windows. Depends on the first two tickets and spec §Delivery slices. Likely files: `src-tauri/src/{watchdog,switcher,commands}.rs`, `src/lib/DeckSwitcher.svelte`, nested harness. Acceptance: game runs while deck opens, resume without duplicate launch, close affects game but not Steam/Rockstar of unrelated game, crash removes card, under failed focus capability app UI honestly reports unavailable.
