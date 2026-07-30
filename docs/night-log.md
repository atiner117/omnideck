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

## 2026-07-30 12:58 — Wave 1 pick 3: PinModal component cherry-picked onto post-rewrite main
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 1 — third zero-overlap pick; roadmap #2
  (parental controls), frontend half. Standalone by design: no bindings dependency, ships
  independent of #19's PIN backend.
- **Branch / PR:** `pick/pinui` — https://github.com/atiner117/omnideck/pull/56
- **Changed:** clean cherry-pick of `3c61d5b` from draft #36 onto main `c6ab9ef`: one new
  file `src/lib/PinModal.svelte` (+150). Purely presentational (CatalogModal/Wizard
  pattern), exports PIN_ROWS/PIN_FLAT/PIN_COLS/PIN_MAX for the page router; only import is
  the existing `Modal.svelte`. Not mounted anywhere until the +page wave — by design.
- **Verify:** bun run check (pass, 342 files, 0 errors) · bun run build (pass) · bun run
  test (13 pass) · cargo clippy --release -D warnings (pass) · cargo test --release
  (44 pass, 1 ignored; exit status checked directly). No visual smoke possible solo (not
  mounted); svelte-check/build compile the component.
- **Outcome:** shipped to draft PR #56 (supersedes #36 — close #36 when #56 lands). Wave 1
  now 3 drafts in flight: #54 doctor, #55 logs, #56 pinui — all independent, any merge order.
  Same night-log prepend-conflict caveat as picks 1–2.
- **Next candidate:** per triage, the remaining Wave 1 solos: #37 archdoc (`e1d2b17` — but
  re-check ARCHITECTURE.md claims against post-#48 main first, CONTRIBUTING.md may conflict
  with `bed913c`) or the paired picks (#15 audio backend + #35 modal as one iteration;
  #18 idle backend + #29 saver overlay as another).

## 2026-07-13 07:15 — Phone-as-remote: authed LAN HTTP remote (parking lot)
- **Vision tie:** NOTES-FEATURE-BACKLOG-2026-07-12 parking lot (phone-as-remote); VISION
  couch-first control — the phone already in your hand becomes a second remote.
- **Branch / PR:** `loop/fable-remote-20260713-062013` —
  https://github.com/atiner117/omnideck/pull/47 (draft, base `loop/fable-integration-20260712`,
  labeled `needs-hardware`).
- **Changed:** new `src-tauri/src/remote.rs` — hand-rolled std::net HTTP server (ZERO new
  deps), off by default (`[remote] enabled = false`, port 8765); 32-byte /dev/urandom token
  on first enable, stored/masked/backup-stripped exactly like media_server.token,
  constant-time compare, never logged; endpoints: transport (MPRIS, `stop` added to the
  proxy), volume (wpctl→pactl fallback — no audio path exists on this base), nav via new
  `gamepad::emit_synthetic_button` (same `gamepad-event` a real pad emits; drives OmniDeck's
  UI only — a fullscreened launched app ignores it, stated in the PR). Self-contained phone
  page at `/` (`remote_page.html`, inline CSS/JS, pairing via URL #fragment → localStorage).
  Commands `remote_status` (QR-able pairing URL) + `set_remote_enabled`; settings-UI wiring
  is a follow-up. Shared-file diffs kept additive for #42/#43/#45 neighbors.
- **Verify:** cargo check (pass) · cargo clippy (pass) · cargo test (67 pass, incl. hermetic
  loopback auth/routing matrix) · bun run check (pass, 0 errors) · page smoke-tested in a
  real browser at phone viewport (pairing JS verified).
- **Outcome:** shipped to draft PR #47.
- **Next candidate:** settings-UI wiring for the remote (QR render of `remote_status.url` +
  enable toggle — frontend lane, base `loop/fable-integration-page-20260712`); or sleep
  timer if #45 didn't cover the dim path.

## 2026-07-13 06:50 — TV overscan calibration: console-style safe-area screen (parking lot)
- **Vision tie:** NOTES-FEATURE-BACKLOG-2026-07-12 parking lot (overscan/TV calibration);
  VISION controller-first living-room launcher on a TV — TVs crop edges, every console has
  this screen.
- **Branch / PR:** `loop/fable-overscan-20260713-061955` —
  https://github.com/atiner117/omnideck/pull/46 (draft, base `loop/fable-integration-page-20260712`).
- **Changed:** new `src/lib/components/OverscanCalibration.svelte` (full-screen overlay:
  accent frame + corner markers at the safe-area boundary, live % card; owns the draft,
  D-pad/stick grow-shrink 0–10% in 0.5% steps, A/Enter save, B/Esc cancel; input forwarded
  via the unified overlay roster — no own listeners). Global apply: `--overscan` on `<main>`
  + one `main.overscan` rule (height/width/margin calc + `translateZ(0)` so position:fixed
  modals/toasts are contained too — all screens respect the inset, live while adjusting;
  0% renders byte-identical to today). One `action` row under Settings→Appearance launches
  it. Additive `settings.overscan_pct` in config.rs (serde default 0, clamp 0–10, test);
  `Settings.ts` binding regenerated + committed. Expected trivial overlap with #41/#44
  (config.rs + minimal +page edits) noted in the PR body.
- **Verify:** bun run check (pass, 0 errors) · bun run build (pass) · cargo check (pass) ·
  cargo clippy (pass) · cargo test (40 pass).
- **Outcome:** shipped to draft PR #46. NEEDS HARDWARE: real-TV couch check (marker
  visibility at the panel edge, step feel on the 4K OLED).
- **Next candidate:** parking lot is nearly drained (#45 sleep timer, #46 overscan tonight);
  what's left there needs design/hardware (phone-as-remote, button remap UI). The draft
  backlog #10–#46 is deep — the morning review/merge pass now beats new scope.

## 2026-07-13 06:45 — Sleep timer: pause playback in N minutes (parking-lot item)
- **Vision tie:** NOTES-FEATURE-BACKLOG-2026-07-12 parking lot ("Sleep timer — stop playback +
  dim in N min"); VISION couch/console ergonomics — falling asleep to media is the living-room
  case the launcher didn't cover.
- **Branch / PR:** `loop/fable-sleeptimer-20260713-061945` — https://github.com/atiner117/omnideck/pull/45
  (draft, base `loop/fable-integration-20260712`).
- **Changed:** New `src-tauri/src/sleep_timer.rs`: `set_sleep_timer(minutes)` /
  `cancel_sleep_timer` / `get_sleep_timer` (→ `SleepTimerStatus { remaining_secs, total_secs }`,
  bindings committed). One 1 s-poll tokio task per arming; a process-wide generation counter
  makes re-arm REPLACE the running timer race-free (stale task exits on its next tick).
  Deliberately NOT persisted across restarts (a sleep timer is about tonight's session —
  documented). Expiry pauses — never kills — every Playing MPRIS player via new
  `mpris::pause_all()` (+ `Pause` on the player proxy), then emits `sleep-timer-fired`
  (payload: players paused); `sleep-timer-tick` (remaining secs) fires 1/s over the final
  minute so the UI can warn/dim. `tokio` named as a direct dep for `time` only (already in
  the graph via tauri/zbus — no new runtime dependency). Frontend:
  `src/lib/SleepTimer.svelte` (presets 15/30/45/60/90 with "until HH:MM" hints, countdown
  header with reduced-motion-aware final-minute pulse, Cancel row; purely presentational,
  page owns focus/routing via documented focus contract, NOT wired into `+page.svelte`) +
  typed wrappers/listeners in `backend.ts`.
- **Verify:** bun run check (pass, 0 errors) · cargo check (pass) · cargo clippy --all-targets
  (pass) · cargo test (65 pass; 5 new: duration round-up/saturation, arm validation, re-arm
  replacement, cancel idempotence, countdown status).
- **Outcome:** shipped to draft PR #45. needs-runtime-verify: actual MPRIS pause at expiry
  (headless has no live players); mpv-without-MPRIS-plugin is documented as unreachable
  (left playing rather than killed).
- **Next candidate:** integration pass mounts SleepTimer in `+page.svelte` (moon row in the
  power/quick menu?), routes dpad focus with the exported `SLEEP_PRESETS` clamp, and dims the
  screen on `sleep-timer-fired` — pairs naturally with the #18/#29 screensaver overlay.

## 2026-07-12 21:55 — Library view modes: rail / large grid / compact grid / list (round-2 Lane D)
- **Vision tie:** NOTES-FEATURE-BACKLOG-2026-07-12 Lane D (layout/view options); VISION
  controller-first ergonomics — one presentation was the launcher's biggest visual gap.
- **Branch / PR:** `loop/fable-layouts-20260712-212147` — https://github.com/atiner117/omnideck/pull/44
  (draft, base `loop/fable-integration-page-20260712`).
- **Changed:** New additive `[appearance]` config section (`appearance.layout`, serde default
  `rail`, normalized) + `save_appearance` IPC; new `src/lib/components/` — `GridView.svelte`
  (large + compact poster grids), `ListView.svelte` (detail rows), `LayoutPicker.svelte`
  (self-contained settings entry, slots into a future Appearance section), `layouts.ts`
  (pure 2D nav math). Page keeps owning focus/input routing: grids get column-preserving
  top↔bottom wrap, within-row left/right, row-edge exit = category switch (never trapped);
  D-pad/stick/keyboard/hold-repeat work in all modes. Unused `settings.grid_columns` became
  the grid density knob (3–12, compact packs ~1.5x). Live-switch, persisted; bindings
  regenerated + committed (`Appearance.ts`, plus previously-missing `LiveApp.ts`).
- **Verify:** bun run check (pass, 0 errors) · bun run build (pass) · cargo check (pass) ·
  cargo clippy (pass) · cargo test (42 pass, incl. new appearance normalize test) · nav math
  26/26 standalone assertions.
- **Outcome:** shipped to draft PR #44.
- **Next candidate:** couch pass on real hardware (grid hold-repeat feel, compact grid on 4K);
  then Lane A themes can consume the same `[appearance]` section (`theme`/`accent` fields
  merge beside `layout`).

## 2026-07-12 21:50 — Artwork disk cache + startup perf findings (round-2 Lane C)
- **Vision tie:** NOTES-FEATURE-BACKLOG-2026-07-12 Lane C; NOTES-PERFORMANCE quality bar (cold
  boots re-fetched every poster → art pop-in on the rail every launch).
- **Branch / PR:** `loop/fable-artcache-20260712-212133` —
  https://github.com/atiner117/omnideck/pull/43 (draft, base `loop/fable-integration-20260712`)
- **Changed:** new `src-tauri/src/artwork_cache.rs` — URL-hash (inline FNV-1a 64, pinned by
  test) → file under `$XDG_CACHE_HOME/omnideck/artwork` + `.meta` sidecar (ETag/Last-Modified);
  <24 h hits are zero-network, older revalidate via If-None-Match/If-Modified-Since (304 =
  headers only), errors serve stale; atomic writes (fsutil); true-LRU sweep (hits bump mtime),
  200 MB default via new additive `[media_server] art_cache_mb`. `media_server::poster()`
  delegates (legacy id-keyed `omnideck/media` cache removed + dir cleaned up once);
  `media_sections` prefetches rail art (4 bounded workers); new `get_artwork(url)` command
  gated by `url_within_base` (no open proxy for the token-authenticated fetch); `omnideck
  doctor` gains `[art cache]` + `--clear-art-cache`; `getArtwork` in backend.ts; bindings
  regenerated. 7 new tests incl. a hermetic loopback HTTP e2e (fetch → disk hit → 304).
- **Startup findings (measured, nothing changed):** vs a 22 ms `--help` baseline: capability
  probe +≈1 ms, config +≈0, library scan +≈12, catalog +≈10 — too cheap to defer. The
  update-check (#25) has no frontend boot caller yet, so it can't be on the critical path;
  when wired, gate on `settings.check_updates` and fire post-first-paint. The real pop-in was
  the network poster fetches this PR moves to disk.
- **Verify:** cargo check ✅ · cargo clippy ✅ (0 warnings) · cargo test ✅ (67/67) ·
  bun run check ✅ (0 errors) · `omnideck doctor` / `--clear-art-cache` exercised live.
- **Outcome:** shipped to draft PR #43.
- **Next candidate:** wire `get_artwork` consumers (backdrops/episode thumbs) once a layout
  uses them; when Lane B's row lands, point its poster loads at the same cache (they already
  share `media_poster`); wire boot-time update-check gated + deferred.

## 2026-07-12 21:33 — Continue Watching: Jellyfin resume/watched state + row (round-2 Lane B)
- **Vision tie:** NOTES-FEATURE-BACKLOG-2026-07-12 Lane B — "the single biggest missing
  media-launcher feature"; VISION.md media-launcher pillar (the launcher finally knows where
  you left off).
- **Branch / PR:** `loop/fable-resume-20260712-212105` —
  https://github.com/atiner117/omnideck/pull/42 (draft, base `loop/fable-integration-20260712`)
- **Changed:** `media_server.rs` — `MediaItem` + `position_secs`/`played` (tick→secs, 0 = no
  resume point), new `continue_watching()`/`recently_added()`/`set_played()` (POST/DELETE
  `PlayedItems`), `valid_id` gate on every frontend-supplied id before URL interpolation,
  bounded 1-retry on transient transport errors, un-poisoned `user()` cache (no longer caches
  a 200-with-no-Id). `commands.rs` — `get_continue_watching`/`get_recently_added`/
  `mark_watched`/`mark_unwatched`; `media_play(start_secs?)` → mpv `--start=<secs>` (floored,
  NaN/negative rejected). Frontend: typed wrappers in `backend.ts` + self-contained
  `src/lib/ContinueWatchingRow.svelte` (resume-on-click, per-card mark-watched, progress bar,
  exported `refresh()`); NOT wired into +page — integration pass owns placement. Bindings
  regenerated. New tests: tick conversion, id validation, path shapes, UserData parsing,
  start-flag gating.
- **Verify:** bun run check ✅ (0 errors) · bun run build ✅ · cargo check ✅ · cargo clippy ✅ ·
  cargo test ✅ (64 passed)
- **Outcome:** shipped to draft PR #42.
- **Next candidate:** integration pass mounts ContinueWatchingRow on the home screen (+ input-
  router focus), then Lane C (artwork disk cache / startup perf) is the last untouched backend
  lane. Couch-test resume against the real Jellyfin before promoting #42.

## 2026-07-13 01:55 — theme system: 6 built-in themes over the design tokens (round-2 Lane A)
- **Vision tie:** NOTES-FEATURE-BACKLOG-2026-07-12 Lane A (theme system on the #16 tokens);
  VISION.md priority 3 (polish on shipped surfaces) + a11y bar (High Contrast theme,
  motion-free CRT scanlines).
- **Branch / PR:** `loop/fable-themes-20260712-212059` —
  https://github.com/atiner117/omnideck/pull/41 (draft, base `loop/fable-integration-page-20260712`)
- **Changed:** new `src/lib/themes/` (registry + `applyTheme()` in themes.ts; per-theme
  `:root[data-theme]` token blocks in themes.css — OmniDark default, OLED Black, Light,
  High Contrast, Retro CRT w/ static scanlines, Deck). Gamepad-cyclable "Theme" row in the
  table-driven Appearance settings section. `+page.svelte` wiring only: `$effect` applies the
  theme; page background follows `var(--bg)` unless a custom background_color is set.
  `config.rs`: additive `settings.theme` (serde default, whitelist-normalized, tests) +
  regenerated bindings. Accent override = the existing `settings.accent`, untouched.
  NOTE: cherry-picked #16 tokens commit `9fe9d89` onto this branch — backlog said the tokens
  were in the page integration branch but they only landed in the backend one.
- **Verify:** bun run check (pass, 0 errors) · bun run build (pass) · cargo check (pass, via
  clippy) · cargo clippy (pass) · cargo test (pass, 40)
- **Next candidate:** Lane D (layout/view modes) shares the Appearance section — its
  `appearance.layout` row should follow the same settings-defs pattern. Also: delete the
  Light-theme page shim in themes.css once +page.svelte adopts the tokens.

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
