---
title: "Recent-app switcher independent review"
kind: review
status: done
created: 2026-09-28
updated: 2026-09-29
thread: t3code/0075e72c
---

# Independent review and fixups

Fresh read-only reviewer examined the diff against the spec. Initial verdict HOLD for these code-verified P1 findings; coordinator implemented and rechecked each:

1. `src-tauri/src/switcher.rs` originally hid every owned mapped window and `deck_cancel` remapped them all, stealing focus from the previously foreground app in a two-app case. `LAST_HIDE` now records the focused group and cancel restores only that group's just-hidden windows. Overview from focused Home does not alter background mapped apps; unknown/foreign focus refuses hide rather than stranding windows.
2. Owned window detection used exact process group while card show used ancestor ownership. `visible_owned` now uses the same bounded ancestry matcher, including Electron children that create their own session.
3. Failed X unmap was ignored and overview falsely claimed Home. `hide_all` now rolls back partial hides and reports failure; `deck_open` propagates the error to the UI and preserves recovery state.
4. Card close and silent-app STOP/CONT signalled unverified recycled PGIDs via shell `kill`. Signals now require the tracked leader's kernel start time and go through direct `kill(2)` with no fork. Frozen members are recorded by `(pid,starttime)` before STOP so they can be safely thawed after leader exit; a failed `/proc` scan declines to freeze. Records remain for retry if a surviving member cannot be thawed.
5. Keyboard Tab on Close did not update the highlighted card, and mapped background apps stole pad input from Home. Close focus now synchronizes selection without stealing DOM focus; navpad activation checks *focused* owned window rather than merely viewable owned windows.

Reviewer's final targeted verdict: **PASS** on P1 findings after fixes; the `/proc`-scan fallback caveat was addressed after that verdict by declining STOP without an identity roster.

Validation: Rust release clippy and tests (113 pass, one pre-existing ignored), frontend check/build/tests (50 pass), Tauri no-bundle release build, ts-rs export, and nested gamescope harness (10/10) pass. The nested harness initially failed because it kept an older overview open between independent keyboard/pad cases and only logged axis events *after* navpad consumed them; it now dismisses the overview before the next case and logs axis before routing. `cargo audit` reports three existing lockfile advisories (quick-xml 0.39.4 ×2, rustls 0.23.40); libc was already locked and this PR only adds it as a direct dependency. `cargo deny` is not installed locally. Full-tree `cargo fmt --check` is not baseline-clean, so no unrelated formatting was applied.

Remaining product gap, deliberately not mislabeled as implemented: Steam games (e.g. GTA V) are Steam-owned, not OmniDeck-owned, so they cannot yet be safely focused/closed through this overview. A separate feasibility ticket covers a gamescope-safe focus path and game-only stop operation.

## 2026-09-29 dashboard/focus fixup review

Independent reviewer inspected the uncommitted fixup against `tickets/input-and-state/fixup-dashboard-focus/index.md`. Initial verdict **HOLD (P2)**: `show_group` could unmap an already-visible app, fail remapping or focus verification, then leave its window hidden while reporting failure. Coordinator added `rollback_show` to restore original map state on unmap/map/thaw/focus errors, record failed restores for retry, and refreeze the previously stopped group only if no group window remains viewable. Reviewer re-read all failure branches and returned **PASS**, no remaining concrete P1/P2 findings. Caveats: compositor focus may race a 500ms verification timeout; failure injection and dashboard visual assertions are not automated by the nested harness. Live r2d2 still runs an older binary; source review is not evidence of deployment.
