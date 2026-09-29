---
title: "Focus and process identity safety fixup"
kind: ticket
status: done
created: 2026-09-28
updated: 2026-09-28
thread: t3code/0075e72c
---

Reviewer found that the original implementation remapped every previously visible owned app on overview cancel, treated mapped background apps as pad targets, mismatched child-window ownership, and sent unverified process-group signals. Fixup: remember the pre-overview focused group; leave Home-focused background windows alone; refuse uncertain focus; use ancestry ownership consistently; gate navpad on actual X focus; roll back X hide failures; verify leader starttime before every group signal and snapshot frozen members before STOP for safe leaderless thaw. Acceptance: independent targeted re-review PASS, release clippy/tests and nested keyboard/pad flow green.
