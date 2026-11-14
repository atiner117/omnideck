---
title: "Guide dashboard return and mapped-app focus fixup"
kind: ticket
status: done
created: 2026-09-29
updated: 2026-09-29
thread: t3code/0075e72c
---

Goal: make the PR #101 PS/Home contract literal, including the **Home dashboard** (not simply an OmniDeck window showing the last category). Current r2d2 process runs older checkout `21268d6`: its logs confirm tap opens the deck and hold closes apps; PR #101 has not been deployed there. This fixup corrects source gaps independently of deployment.

In scope: report the direction of a successful owned-app Guide tap from backend to UI so a return to Home selects dashboard, not an arbitrary category; overview Home also selects dashboard; failed/no-op actions surface honestly. When an owned window is already mapped behind focused Home, selecting its card or tapping Guide must actually bring it forward (focus following a targeted map transition), with failure recovery. Correct the stale Now Playing tooltip. Keep the task overview live-app-only. Out of scope: Steam/Proton-game controls, Jellyfin server authentication, live r2d2 session restart/deployment without a safe maintenance decision.

Depends on spec §Product contract and the original input/overview tickets. Likely files `src-tauri/src/{switcher,commands}.rs`, `src/{routes/+page.svelte,lib/backend.ts,lib/npActions.ts}`, `packaging/test-session.sh`, `src/lib/backend.test.ts`. Acceptance: tap from any OmniDeck-owned app reveals dashboard, second tap restores the actual last app; overview Home goes dashboard; cancellation doesn't reset category; mapped-behind-Home app is focused when selected; unsupported Steam game actions aren't falsely marked successful. Build/test/ts-rs, nested gamescope and read-only independent review before PR update. R2D2 remains on old binary unless explicitly deployed.

Result: target direction is returned by the Rust IPC; successful app→Home and explicit overview Home select the dashboard. Already-mapped target windows are remapped for gamescope focus with verification and rollback on failures. Unsupported/no-op results are surfaced without pretending to switch; Steam game control remains a separate slice. Independent review PASS after a P2 rollback fix. Release build, clippy, 114 Rust tests passed (one pre-existing ignored, 115 total), frontend check/build/50 tests, ts-rs exports, and nested gamescope **13/13** passed, including OCR checks that app→Home and overview Home select the dashboard. Failure injection and real r2d2 controller still need couch validation.
