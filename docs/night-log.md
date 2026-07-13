# Night-log — autonomous loop

Append-only journal written by the nightly `/omnideck-night` loop. Newest entries at the **top**.
Each iteration adds one block so a fresh (cold) agent — and Andrew in the morning — can see what
happened and what to pick up next. Do not delete history; only prepend.

Entry template:

```
## <YYYY-MM-DD HH:MM> — <one-line increment title>
- **Vision tie:** which VISION.md priority / NOTES-* item this advances.
- **Branch / PR:** loop/night-YYYYMMDD — <PR url>
- **Changed:** what actually changed (files/areas), briefly.
- **Verify:** bun run check (pass/fail) · bun run build (pass/fail) · cargo check (pass/fail) · cargo clippy (pass/fail)
- **Outcome:** shipped to draft PR / reverted / stopped (reason).
- **Next candidate:** the best thing to pick up next, so the next iteration starts warm.
```

---

<!-- entries below -->

## 2026-07-13 00:00 — audit run: backlog exhausted, tracked VISION.md/night-log.md in git
- **Vision tie:** loop-continuity infra (VISION.md guardrail: "if nothing is safely shippable,
  log that and stop rather than inventing scope").
- **Branch / PR:** `loop/fable-trackdocs-20260712-200008` — draft PR against
  `feat/media-audio-fps-config` (see PR list for URL).
- **Changed:** No product code. Audited `gh pr list` (30 open drafts) against
  `NOTES-REVIEW-DEEP-2026-07-11.md` (26 items) and `NOTES-DEEPDIVE-ROADMAP.md` (5 numbered
  features): **every named item already has an open draft PR**, including the two
  "integration" branches (`loop/fable-integration-20260712`,
  `loop/fable-integration-page-20260712`) that already consolidate the overlapping small
  branches into two green super-branches (see `NOTES-FABLE-LANDING-2026-07-12.md` for the
  landing order). Review #26 (consolidate NOTES) is gitignore-moot
  (`NOTES-*.md`/`NOTES.md` are in `.gitignore`); its only actionable part
  (`docs/ARCHITECTURE.md`) already shipped in `loop/fable-archdoc-160138`. Remaining
  parking-lot roadmap items are each blocked or too risky to ship unattended: guide-button
  chord *remap* was explicitly deferred by `loop/fable-input-153828`'s own commit (hold-ms +
  kill-switch shipped; full keysym remap needs more plumbing); HDR signaling has no verifiable
  detection surface on this box (`gpu.rs` only reads RandR mode, not gamescope HDR state);
  Steam family-view is gated on parental controls (#2), which is drafted but not merged;
  cloud sync is explicitly "niche, defer" in the roadmap. One real gap found and fixed:
  `VISION.md` and this file were **never committed on any branch** (pure untracked
  working-tree files) — now tracked so the loop's compass/journal survive a fresh clone or a
  worktree reset.
- **Verify:** bun run check (n/a, docs-only) · bun run build (n/a, docs-only) · cargo check
  (n/a, docs-only) · cargo clippy (n/a, docs-only) — no source files touched.
- **Outcome:** shipped to draft PR (docs-only).
- **Next candidate:** **not more code** — the safely-shippable backlog is exhausted. The
  bottleneck is now landing debt: 30 open draft PRs need human triage/merge (start with the
  two integration branches per `NOTES-FABLE-LANDING-2026-07-12.md`'s rebase order). Only after
  that lands does it make sense to revisit the higher-risk parking-lot items flagged above.

## 2026-07-11 (session) — autonomous run over GLM-5.2's deep review
Branch `loop/night-20260711` (branched off `feat/media-audio-fps-config` for this test run).
9 commits; each verified `cargo check`+`clippy` (backend) or `bun run build` (frontend) green;
`main` untouched; nothing pushed or merged.

| commit | review # | item |
|--------|----------|------|
| `74bdf39` | #1 (HIGH) | atomic config writes (temp sibling + `sync_all` + rename) |
| `2ef44d9` | #1 bonus | serialize `mutate_and_save` RMW with a Mutex |
| `ef4bc2e` | #3 | don't cache a failed `user_id` resolve (no more poisoned "no user") |
| `6bc9a54` | #4 | one bounded retry on transient blip + `warn!` instead of silent empty rows |
| `1a33534` | #5 | validate Jellyfin `id`/`parent` before URL interpolation |
| `fa4d647` | #14 | delete git-tracked `_layout-rail.svelte.bak` |
| `c674012` | #15 | real `app.html` title + `color-scheme`/`theme-color` meta |
| `f605425` | #8 | cache `capability::probe()` — stop re-scanning sysfs/PCI on every play |
| `abc1042` | #24 | navpad emit-failure backoff/disarm (no ~125 Hz log-spam) |

**Deferred (need a human / runtime testing, not safe unattended):**
- **#2 MPRIS reconnect (HIGH)** — needs a backoff sleep (async-timer dep decision) *and*
  real D-Bus-restart testing. Do not fake with a tight reconnect loop.
- **#22 mpv token → header** — `stream_url` is shared by the mpv path *and* the cli debug
  path (a raw reqwest GET); moving the token to a header without breaking the cli path needs
  coordinated changes at both sites. S effort but cross-cutting.
- **#7 server() config caching** — the robust fix swaps a hot-path `OnceLock` for `RwLock`
  + invalidation on `save_settings`; touches the read path on every media command.

**Next candidates:** #7, #22, #6 launcher argv hygiene, #23 VapourSynth re-probe, then frontend
polish #17/#18/#19. #9/#10 (`+page.svelte` decomposition) are the big multi-day lifts.

## 2026-07-11 — Atomic config writes (review #1, HIGH)
- **Vision tie:** reliability gap #1 (HIGH) in NOTES-REVIEW-DEEP-2026-07-11.md — GLM-5.2's deep review; on the "very good → A+++" path.
- **Branch / PR:** `loop/night-20260711` — local commit `74bdf39` (not yet pushed).
- **Changed:** `src-tauri/src/config.rs` — new `write_atomic()` (temp sibling + `sync_all` + `rename`, mirroring `background.rs`); replaces the two `fs::write` calls (first-run + `mutate_and_save`).
- **Verify:** `cargo check` PASS (0 warn) · `cargo clippy` PASS (0 warn) · frontend gates n/a (backend-only change).
- **Outcome:** committed to branch, green. Awaiting human review/push (not merged).
- **Next candidate:** review #2 — MPRIS watcher reconnect/backoff (`mpris.rs:295-335`, HIGH). Also cheap: the #1 *bonus* (Mutex around the `mutate_and_save` RMW to fix last-writer-wins) + generalize `write_atomic` to `media_profiles.rs` (review #21).
