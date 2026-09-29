---
title: "Input and owned-app focus semantics"
kind: ticket
status: done
created: 2026-09-28
updated: 2026-09-28
thread: t3code/0075e72c
---

Goal: short Guide/PS toggles Home and last owned running app; long hold enters task overview, never kills. In scope: gamepad per-device press state, hotkey parity, backend session-gated last-owned-group focus/hide, cancellation/exit safety, unit tests. Out of scope: Steam game kill/focus; thumbnails. Detached child windows are recognized through bounded ancestry ownership, matching card-show semantics. Dependency: spec §Product contract and §Delivery slices. Likely files: `src-tauri/src/{gamepad,hotkey,switcher,commands,watchdog,config}.rs`, `src/lib/backend.ts`. Acceptance: holds emit distinct event once at threshold; tap on release only; no app killed; two app groups don't remap together; desktop no-op; safe on X failure; test targeted Rust.
