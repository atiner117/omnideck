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

## 2026-08-02 12:36 — Wave 2 FINAL pick: phone-as-remote (authed LAN HTTP)
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 2 complete; VISION couch-first control —
  the phone becomes a second remote. Context: #68 merged + #45 closed this session at
  Andrew's direction.
- **Branch / PR:** `pick/remote` — https://github.com/atiner117/omnideck/pull/69
  (needs-hardware label kept).
- **Changed:** both picks from draft #47 (`8a84fd7` + `23d72b8` rebind-race fix) onto
  main `9f431e9` + TWO adaptations + one integration fix. remote.rs (std::net, zero
  deps, off by default, urandom token constant-time compared / IPC-masked /
  backup-stripped). Adaptations: (a) `stop` joined main's Verb-enum control() — gains
  the 2s frozen-player timeout; (b) volume wpctl/pactl shell-outs bounded via
  proc::output_with_timeout + has_bin fallback (were unbounded .status() — the P0
  wedged-PipeWire class). Integration fix: synthetic remote presses set
  EXTERNAL_ACTIVITY so a phone-driven session can't dim (only exists because #59 +
  remote both landed). DISCARDED: a manual Config Default impl w/ config_version —
  #10's lane content, not this feature. SIX conflicted files; Config.ts via regen
  (23 export tests, RemoteConfig/RemoteStatus new).
- **Verify:** bun run check (pass, 353 files, 0 errors) · bun run build (pass) · bun
  run test (13 pass) · cargo clippy --release -D warnings (pass) · cargo test
  --release (90 pass — 9 remote tests incl. hermetic loopback auth/routing matrix —
  1 ignored) · bindings in sync. needs-hardware: phone pairing, live transport/volume/
  nav, rebind under enable/disable cycling.
- **Outcome:** shipped to draft PR #69 (supersedes #47 — close #47 when #69 lands).
  WAVE 2 COMPLETE: #61–#68 merged, #69 in draft — 9 features salvaged, 1 verified
  obsolete (#23), 41→16 open drafts since the triage began.
- **Next candidate:** lanes #10 (config_version + Emby header + re-resolve — diff vs
  7cdd45f first) #12 #13 #24, then the +page wave (#17 SettingDef first, strict
  triage order). Wave 4 reworks (#41–#43) last.

## 2026-08-02 10:22 — Wave 2 pick 8: sleep timer — pause playback in N minutes
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 2, parking-lot sleep timer; VISION couch
  ergonomics (falling asleep to media). Context: #67 merged + #25 closed this session
  at Andrew's direction.
- **Branch / PR:** `pick/sleep` — https://github.com/atiner117/omnideck/pull/68
- **Changed:** pick `42ab518` from draft #45 onto main `1291401` + ONE adaptation
  commit. Hidden-dep check FIRST (triage's "forked from #38's tip" worry): every
  cross-module call verified on main (sync::lock_or_recover = #48 helper) — clean.
  sleep_timer.rs (set/cancel/get, generation-counter race-free re-arm, not persisted
  by design), mpris::pause_all (pause-not-kill, all Playing players), SleepTimer.svelte
  (presentational, +page wiring deferred). ADAPTATION: pause_all was written against
  the old OnceLock CONN — rewired onto 9e7eb5b's supervised watcher via current_conn()
  (None mid-reconnect → no-op). The triage filed that drift under #18 but it bit here —
  caught by the gate (compile error), fixed, recorded. Conflicts: Cargo.toml tokio line
  (kept main's — feature superset), lib.rs tail, backend.ts export line.
- **Verify:** bun run check (pass, 351 files, 0 errors) · bun run build (pass) · bun
  run test (13 pass) · cargo clippy --release -D warnings (pass) · cargo test
  --release (81 pass — 6 new sleep-timer tests — 1 ignored) · bindings in sync ·
  Cargo.lock clean (no new deps). needs-runtime-verify: live MPRIS pause at expiry.
- **Outcome:** shipped to draft PR #68 (supersedes #45 — close #45 when #68 lands).
- **Next candidate:** #47 remote (last Wave 2 feature; keep needs-hardware label;
  gamepad.rs + mpris.rs edges — hunk-check both against the P0/supervisor changes
  first). Then lanes #10 #12 #13 #24 and the +page wave (#17 SettingDef first).

## 2026-08-02 09:28 — Wave 2 pick 7: update-check backend (GitHub latest-release probe)
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 2, roadmap #4 (check half only — acting on
  an update stays per-distro follow-up). Hunk-checked first: zero update symbols on
  main, genuinely live. Context: #66 merged + #27 closed this session at Andrew's
  direction.
- **Branch / PR:** `pick/update` — https://github.com/atiner117/omnideck/pull/67
- **Changed:** pick `a19ad7f` from draft #25 onto main `ce6a3a8` + bindings-regen
  commit. New update.rs (check_update(force) → UpdateInfo; process-lifetime cache for
  the 60 req/hr unauthed API, force bypass; drafts/prereleases never offered; rides
  http::client() with the SSRF/timeout policy, compile-time URL). settings.check_updates
  (default true) gates the boot-time call. FIVE conflicted files, all appended-tail
  shapes (config.rs vs PIN, lib.rs handler list, commands.rs vs backup, backend.ts
  export line merged LiveApp+UpdateInfo, Settings.ts via regen — 20 export tests).
- **Verify:** bun run check (pass, 349 files, 0 errors) · bun run build (pass) · bun
  run test (13 pass) · cargo clippy --release -D warnings (pass) · cargo test
  --release (75 pass — 5 new update:: tests — 1 ignored). No new deps. Live-network
  probe deliberately not smoked headlessly (unauthed API budget); couch "Check now".
- **Outcome:** shipped to draft PR #67 (supersedes #25 — close #25 when #67 lands).
- **Next candidate:** #45 sleeptimer (verify no hidden dep on #38's integration tip
  per triage) or #47 remote (needs-hardware label). Then lanes #10 #12 #13 #24 and
  the +page wave (#17 SettingDef first).

## 2026-08-01 21:02 — Wave 2 pick 6: [input] config — hold threshold + hotkey kill-switch
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 2, roadmap #6 (input tuning). Hunk-checked
  FIRST per the #23 lesson: InputConfig/guide_hold_ms/session_hotkeys have zero hits on
  main, the 800ms hold is still a const, 1c2b31e's gamepad work was navpad-gating in a
  different region — genuinely live, unlike #23. Context: #65 merged this session.
- **Branch / PR:** `pick/input` — https://github.com/atiner117/omnideck/pull/66
- **Changed:** pick `9e202e5` from draft #27 onto main `dac31db` + bindings-regen commit.
  [input] table (guide_hold_ms clamped 200–5000; session_hotkeys kill-switch for the
  Ctrl+Alt grabs); gamepad.rs reads the threshold at thread start, hotkey.rs gates its
  grabs; read-once-at-startup documented. FIVE config.rs conflict regions (the
  accumulated both-appended shape vs screensaver/overrides/PIN/backup) — all merged.
  The pick's Config.ts was stale (pre-Wave-1/2 generation) — regenerated, 19 export
  tests, new InputConfig.ts.
- **Verify:** bun run check (pass, 348 files, 0 errors) · bun run build (pass) · bun
  run test (13 pass) · cargo clippy --release -D warnings (pass) · cargo test
  --release (70 pass — new clamp test — 1 ignored) · omnideck config smoke (pass).
  No new deps. needs-hardware: hold-feel at custom thresholds + kill-switch in-session.
- **Outcome:** shipped to draft PR #66 (supersedes #27 — close #27 when #66 lands).
- **Next candidate:** #25 update-check (update.rs new — hunk-check the config.rs/
  LiveApp.ts edges first) or #45 sleeptimer (verify no hidden dep on #38's integration
  content per triage). Then #47 remote, lanes #10 #12 #13 #24, the +page wave (#17).

## 2026-08-01 20:55 — Wave 2 pick 5 NOT PICKED: #23 verified fully superseded, closed
- **Vision tie:** PR-TRIAGE-2026-07-26 follow-through — the triage's own "verify first"
  discipline applied to its Wave 2 #23 row, which turned out to be wrong.
- **Branch / PR:** `docs/triage-23` (this record + triage addendum) — no code PR; #23
  closed instead. Context: #64 merged + #21 closed this session at Andrew's direction.
- **Changed:** attempted the pick of `89e859b` (fsutil.rs + media_profiles conversion);
  the conflict revealed main's media_profiles.rs ALREADY calls
  crate::config::write_atomic at the same site — converted by `1c2b31e` (#48 deep-review
  fixes), which was in the triage's own `c6ab9ef` baseline. Config half = `981a977`.
  fsutil.rs is strictly weaker than main's write_atomic (no fsync of contents/dir, no
  symlink write-through, no permission preservation; pid-only vs pid+seq temp naming).
  Zero live content — aborted the pick, deleted the branch, closed #23 citing both
  commits. Triage status block gains the correction + the lesson: file-level overlap
  ≠ live content; check hunks before picking.
- **Verify:** verification-only iteration — git log -S evidence, no build gates apply.
- **Outcome:** #23 closed as fully superseded; triage doc corrected (this branch).
- **Next candidate:** #27 [input] config (re-read the gamepad.rs merge — P0-hardened;
  after the #23 lesson, check its hunks against `1c2b31e` FIRST) or #25 update-check
  (update.rs is new — likely genuinely live). Then #45 sleeptimer, #47 remote, the
  +page wave (#17 first).

## 2026-08-01 20:42 — Wave 2 pick 4: config backup/restore, atomic + serialized
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 2, roadmap #5 — the "adapt to 981a977"
  pick done as flagged, not blind. Context: #63 merged + #14 closed this session at
  Andrew's direction.
- **Branch / PR:** `pick/backup` — https://github.com/atiner117/omnideck/pull/64
- **Changed:** pick `569663a` from draft #21 onto main `99d36ff` (backup_config /
  restore_config + backend.ts wrappers; sanitized snapshots, credentials stripped by
  default, restore re-normalizes hostile input and works from the broken-config state)
  + TWO adaptation commits: (a) restore takes SAVE_LOCK and both paths write through
  write_atomic — no truncated config.toml/backup possible; (b) both commands moved to
  the blocking pool (write_atomic fsyncs — the documented blocking() class). ONE
  conflict: config.rs tests module (fourth pick in a row with that shape) — merged.
- **Verify:** bun run check (pass, 347 files, 0 errors) · bun run build (pass) · bun
  run test (13 pass) · cargo clippy --release -D warnings (pass) · cargo test
  --release (68 pass — 4 new backup tests incl. hostile-backup normalization —
  1 ignored) · export suite ran, bindings zero drift. No new deps.
- **Outcome:** shipped to draft PR #64 (supersedes #21 — close #21 when #64 lands).
- **Next candidate:** #23 atomic (pick ONLY the fsutil/media_profiles half — config
  half already done by 981a977 per triage) or #27 [input] (re-read the gamepad.rs
  merge — P0-hardened). Then #25 update-check, #45 sleeptimer, #47 remote. The +page
  wave (#17 SettingDef first) is the remaining big block.

## 2026-08-01 18:40 — Wave 2 pick 3: launcher spawn-error mapping (argv lane)
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 2, #6 backend (launcher robustness).
  Context: #62 merged + #31 closed this session at Andrew's direction.
- **Branch / PR:** `pick/argv` — https://github.com/atiner117/omnideck/pull/63
- **Changed:** both picks from draft #14 onto main `914ac9e`: `88d8ce7` (PATH-resolve
  pre-flight) then `3411922` (the refinement that REMOVES the pre-flight for a
  spawn_error() helper mapping raw OS errors to actionable messages at the spawn site
  — race-free, net −71/+33). ONE conflict: the spawn() line vs #62's override-env
  block — kept the override block, applied spawn_error on the same spawn.
- **Verify:** bun run check (pass, 347 files, 0 errors) · bun run build (pass) · bun
  run test (13 pass) · cargo clippy --release -D warnings (pass) · cargo test
  --release (64 pass — new spawn_errors_map_to_clear_messages — 1 ignored).
  Rust-only, no bindings, no new deps.
- **Outcome:** shipped to draft PR #63 (supersedes #14 — close #14 when #63 lands).
- **Next candidate:** #21 backup (adapt to 981a977: reuse write_atomic) or #27
  [input] config (gamepad.rs was P0-hardened — re-read the merge per triage). After
  the small picks: the +page wave, #17 SettingDef first.

## 2026-08-01 18:31 — Wave 2 pick 2: [launch_overrides] per-tile env + args
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 2; roadmap parking-lot "per-game launch
  options", scoped to the launch_command spawn path (Steam titles keep Steam's own
  Launch Options — documented). Context: #61 (PIN backend) merged + #19 closed this
  session, at Andrew's direction, after all four CI checks went green.
- **Branch / PR:** `pick/overrides` — https://github.com/atiner117/omnideck/pull/62
- **Changed:** one pick, `6d63db2` from draft #31, onto main `d3b1fb9`:
  [launch_overrides."<tile-id>"] table (env map + extra args), applied in
  launch_command BEFORE the BROWSER token so browser tiles keep the URL-only argv
  guard; normalize() drops env entries Command::env can't represent (empty/=/NUL
  keys, NUL values). Hand-editable only; empty map serializes to nothing. THREE
  config.rs conflicts (defaults list, normalize block, tests module vs #59+#61
  additions) — all both-appended, kept everything. Bindings regenerated: 18 export
  tests (new LaunchOverride.ts), zero drift.
- **Verify:** bun run check (pass, 347 files, 0 errors) · bun run build (pass) ·
  bun run test (13 pass) · cargo clippy --release -D warnings (pass) · cargo test
  --release (63 pass — incl. NUL/'=' injection-shape drops — 1 ignored) · omnideck
  config smoke (pass). No new deps.
- **Outcome:** shipped to draft PR #62 (supersedes #31 — close #31 when #62 lands).
- **Next candidate:** #14 argv PATH-resolve (small) or #21 backup (needs the same
  "adapt to 981a977" treatment as the PIN pick — reuse write_atomic, don't duplicate).
  The +page wave (#17 first) is the bigger prize once the small picks drain.

## 2026-08-01 18:23 — Wave 2 pick 1: PIN backend (argon2) cherry-picked onto post-Wave-1 main
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 2; roadmap #2 backend half — pairs with the
  already-landed PinModal (#56). Context: Wave 1 (#54–#59) was merged 2026-07-30 at
  Andrew's direction and the triage close-list executed (16 drafts closed, 41→25 open).
- **Branch / PR:** `pick/pin` — https://github.com/atiner117/omnideck/pull/61
- **Changed:** two picks from draft #19 onto main `705d96b`: `ea5def6` (new pin.rs —
  argon2id PHC hashes, fresh salt, set_pin/verify_pin on the blocking pool; threat model
  = deterrence, documented) + `b429c0c` (locked_categories writes PIN-gated via
  set_locked_categories; pin_hash masked over IPC behind has_pin, cleared pre-save).
  Conflicts: lib.rs appended-tail (kept all) + config.rs save path — kept
  `has_pin = None`, DROPPED the pick's create_dir_all as redundant with main's
  write_atomic (the triage's "adapt to 981a977" note, applied). New dep: argon2.
  Bindings regenerated, 17 export tests, zero drift.
- **Verify:** bun run check (pass, 346 files, 0 errors) · bun run build (pass) · bun run
  test (13 pass) · cargo clippy --release -D warnings (pass) · cargo test --release
  (60 pass — 6 new pin:: tests — 1 ignored) · cargo audit (pass) · cargo deny NOT
  runnable locally (not installed) — CI's cargo-deny job is the gate for the new dep.
- **Outcome:** shipped to draft PR #61 (supersedes #19 — close #19 when #61 lands).
  Also this session: #52 (triage doc, status updated) and #53 (loop inventory guard)
  merged at Andrew's direction.
- **Next candidate:** Wave 2 small picks: #31 [launch_overrides] (`6d63db2`, serde adds)
  or #14 argv PATH-resolve — both small. #21 backup needs the same "adapt to 981a977"
  treatment as this pick. Then the +page wave, #17 SettingDef first.

## 2026-07-30 18:56 — Wave 1 pick 6 (FINAL): screensaver pair — idle backend + OLED overlay
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 1 complete; roadmap #1 (OLED screensaver) —
  the "useless without #18's idle events" pair lands as one unit.
- **Branch / PR:** `pick/saver` — https://github.com/atiner117/omnideck/pull/59
- **Changed:** three picks onto main `c6ab9ef`: `d65988d` (idle/active events from the
  gamepad thread, [screensaver] config table, mpris-playing counts as activity),
  `3ab7311` (notify_activity command — DOM input resets the clock), `5fdd860`
  (ScreensaverOverlay.svelte + NEW src/routes/+layout.svelte, layout-mounted, three
  stages dim/art/blank, reduced-motion aware, defensive self-timer). TWO conflicts,
  both the familiar tail kind: config.rs test imports (write_atomic vs ScreensaverConfig
  — merged), lib.rs invoke_handler (deck_cancel vs notify_activity — kept both).
  Bindings REGENERATED and verified in sync (17 export tests incl.
  export_bindings_screensaverconfig; git status on bindings clean).
- **Verify:** bun run check (pass, 344 files, 0 errors) · bun run build (pass) · bun run
  test (13 pass) · cargo clippy --release -D warnings (pass) · cargo test --release
  (47 pass — 3 new screensaver tests — 1 ignored) · omnideck config loads/normalizes with
  the table absent. needs-hardware: dim/art/blank staging + wake feel on the OLED.
- **Outcome:** shipped to draft PR #59 (supersedes #18 AND #29 — close both when it
  lands). WAVE 1 COMPLETE: #54 #55 #56 #57 #58 #59 — six drafts on post-rewrite main
  superseding eight stranded originals (#33 #34 #36 #37 #15 #35 #18 #29).
- **Next candidate:** Andrew's review/merge pass on the six. After merges: close the
  eight superseded drafts + the #48-superseded close-list from the triage (#20 #22 #38
  #39 #40 + #9 #11 #16). Then Wave 2/3: the sequenced +page.svelte wave (#26 #28 #30
  #32 …) — those need the triage's ordering and fresh conflict checks against whatever
  merged first.

## 2026-07-30 13:25 — Wave 1 pick 5: audio backend + AudioOutputModal (first paired pick)
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 1, roadmap #3 (audio output switching) — the
  triage's "dead UI without the backend" pair lands as one reviewable unit.
- **Branch / PR:** `pick/audio` — https://github.com/atiner117/omnideck/pull/58
- **Changed:** three picks onto main `c6ab9ef`: `5690127` (new audio.rs — pactl sink
  enumeration, JSON + short fallback, audio_outputs/audio_set_output commands), `ef8e27f`
  (3s deadline-kill on every pactl call, spawn_blocking off the IPC thread), `eec94cb`
  (new AudioOutputModal.svelte, standalone, local AudioSink TS type). ONE conflict:
  lib.rs invoke_handler tail (main's deck_cancel vs audio's two commands) — kept all three.
  Deliberate: audio.rs keeps its own run_with_timeout instead of proc.rs's
  output_with_timeout (proc.rs nulls stderr; audio needs it for error messages) —
  flagged in the PR as a candidate proc.rs follow-up, not silently unified.
- **Verify:** bun run check (pass, 342 files, 0 errors) · bun run build (pass) · bun run
  test (13 pass) · cargo clippy --release -D warnings (pass) · cargo test --release
  (51 pass — 7 new audio:: tests incl. injection-id rejection + real timeout-kill —
  1 ignored) · host sanity: pactl get-default-sink answers (PipeWire). In-session sink
  switch still needs the couch box.
- **Outcome:** shipped to draft PR #58 (supersedes #15 AND #35 — close both when it
  lands). Wave 1: 5 of 6 done (#54 #55 #56 #57 #58).
- **Next candidate:** the last Wave 1 pair: #18 screensaver idle backend + #29 overlay
  component on one pick/saver branch. #29 touches +layout.svelte per the triage —
  expect the first frontend-file conflict potential; check what #48/fa6fe2c did there.

## 2026-07-30 13:13 — Wave 1 pick 4: ARCHITECTURE.md cherry-picked + claims re-verified
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 1 — the flagged-careful pick ("re-check
  ARCHITECTURE.md claims post-#48"); deep-review #26's public half — the repo finally
  ships a committed architecture doc.
- **Branch / PR:** `pick/archdoc` — https://github.com/atiner117/omnideck/pull/57
- **Changed:** cherry-pick of `e1d2b17` from draft #37 onto main `c6ab9ef`
  (docs/ARCHITECTURE.md new +126, CONTRIBUTING.md link — auto-merged cleanly with
  `bed913c`'s refresh, read post-merge) + follow-up commit `617e4f5` fixing the two claims
  that drifted: launch section now covers proc.rs (P0 bounded shell-outs, deadline+kill),
  testing section now covers vitest (nav/osk/launchId). Verified-unchanged claims were
  spot-checked in code, not assumed: _NET_WM_PID grouping, argv-only spawns,
  guide tap/hold in-thread, event-driven MPRIS, max_log_files(7), backend.ts sole
  invoke() caller (npActions.ts routes through it).
- **Verify:** docs-only diff (CONTRIBUTING +4, ARCHITECTURE +127, no code) — gates run
  anyway: bun run check (pass, 0 errors) · bun run build (pass) · bun run test (pass) ·
  cargo clippy --release -D warnings (pass) · cargo test --release (pass).
- **Outcome:** shipped to draft PR #57 (supersedes #37 — close #37 when #57 lands).
  Wave 1 solos now ALL in flight: #54 doctor, #55 logs, #56 pinui, #57 archdoc.
- **Next candidate:** Wave 1's remaining items are the paired picks — #15 audio backend +
  #35 AudioOutputModal as one iteration (triage: modal is dead UI without the backend),
  then #18 idle backend + #29 screensaver overlay. After those, Wave 2 / the sequenced
  +page.svelte wave.

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

## 2026-07-30 12:51 — Wave 1 pick 2: `omnideck logs` cherry-picked onto post-rewrite main
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 1 — second zero-overlap pick; completes the
  support story next to `doctor` (#54): session crashes land in the rotating file, `logs`
  finally tells the user where.
- **Branch / PR:** `pick/logs` — https://github.com/atiner117/omnideck/pull/55
- **Changed:** clean cherry-pick of `802aff6` from draft #34 onto main `c6ab9ef`:
  `src-tauri/src/cli.rs` +60 (new `Logs { -n, --path }` subcommand), `src-tauri/src/logging.rs`
  +18/−6 (state-dir derivation extracted into shared `logging::state_dir()`; `init()` behavior
  unchanged). Zero conflicts — parallel to #54 off the same trunk, merges in either order.
- **Verify:** bun run check (pass, 0 errors) · bun run build (pass) · bun run test (13 pass) ·
  cargo clippy --release -D warnings (pass) · cargo test --release (44 pass, 1 ignored; exit
  status checked directly per the pick-1 caution) · live smoke: `omnideck logs` listed this
  host's 6 rotated files + tailed the newest; `logs --path` printed the newest path.
- **Outcome:** shipped to draft PR #55 (supersedes #34 — close #34 when #55 lands). Same
  night-log prepend-conflict caveat as pick 1: #52/#54/#55 all add entries at the top of this
  file; keep all, newest on top.
- **Next candidate:** Wave 1 pick 3 per triage: #36 PinModal (`3c61d5b`, new
  `PinModal.svelte`, pairs with #19's backend) or #37 archdoc (`e1d2b17` — but re-check
  ARCHITECTURE.md claims against post-#48 main before shipping). The #35 audio modal needs
  the #15 backend picked together — bigger bite, save for its own iteration.

## 2026-07-30 11:55 — Wave 1 pick 1: `omnideck doctor` cherry-picked onto post-rewrite main
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 1 (Andrew approved the triage 2026-07-30) — start
  draining the stranded drafts with the zero-overlap picks; release-prep QoL (one-command
  support bundle for public issues).
- **Branch / PR:** `pick/doctor` — https://github.com/atiner117/omnideck/pull/54
- **Changed:** clean cherry-pick of `fc0a7b3` from draft #33 onto main `c6ab9ef`:
  `src-tauri/src/cli.rs` +92, new `Doctor` subcommand. Zero conflicts, as the triage predicted;
  all 9 internal APIs the pick calls were re-checked against post-rewrite main — none drifted
  (`gpu::DisplayMode` is still the `(w, h, hz)` tuple alias).
- **Verify:** bun run check (pass, 0 errors) · bun run build (pass) · bun run test (13 pass) ·
  cargo clippy --release -D warnings (pass) · cargo test --release (44 pass, 1 ignored) · live
  smoke `cargo run --release -- doctor` (real bundle: v0.2.0, GamescopeSession tier, 14 tiles,
  44 games, mpv+VapourSynth found, no secret values printed). Caution for future iterations:
  the first cargo-test attempt SIGABRT'd compiling deps under parallel-build memory pressure
  and a `| tail` pipe masked the failure as "OK" — always check cargo's own exit status, not
  the pipe's.
- **Outcome:** shipped to draft PR #54 (supersedes #33 — close #33 when #54 lands). Note: this
  entry and PR #52's triage entries will both sit at the top of this file — expect a trivial
  prepend conflict when the second one merges; keep both, newest on top.
- **Next candidate:** Wave 1 pick 2: #34 `omnideck logs` (`802aff6`, cli.rs + logging.rs) —
  triage says it picks cleanly before/after doctor; cli.rs now differs from #34's base, so
  expect a trivial context-line conflict at worst.

## 2026-07-29 22:51 — Triage correction: "PR-less" lane branches are open drafts #9–#17
- **Vision tie:** guardrails / landing path for the draft backlog — the 2026-07-26 triage is the
  document Andrew will use to drain 41 open drafts, and it contained the exact mistake PR #53
  guards against: absence claims from a partial (#18–#47) inventory.
- **Branch / PR:** `loop/night-20260726` — https://github.com/atiner117/omnideck/pull/52 (updated
  in place; a second competing triage doc on a fresh branch would have made things worse).
- **Changed:** `docs/PR-TRIAGE-2026-07-26.md` only. Verified against the FULL open-PR inventory
  (41 open, `gh pr list --state open --limit 200`): every row of the "PR-less branches" table maps
  1:1 to drafts #9–#17 (`fable-audio`=#15, `fable-settings`=#17, `fable-frontend`=#13,
  `fable-backend`=#12, `fable-media`=#10, `fable-argv`=#14, `fable-mpris`=#11, `fable-tokens`=#16,
  `loop/night-20260711`=#9). Rewrote that table with the PR column; actions now read "land/close
  the existing draft" instead of "open a PR"/"delete"; #9/#11/#16 flagged as joining the close
  list; Wave-3 step 1 and the #35 note now point at #17/#15; header/coverage corrected from
  "30 (#18–#47)" to "39 (#9–#47)". Cherry-pick analysis itself untouched.
- **Verify:** docs-only — no build gates apply (bun/cargo untouched).
- **Outcome:** shipped to existing draft PR #52 (title/body updated to match).
- **Next candidate:** with the triage now trustworthy, start grinding Wave 1: cherry-pick #33
  (doctor, `fc0a7b3`) onto a fresh `pick/doctor` branch off main, full Verify gate, PR that
  supersedes #33 — one pick per iteration.

## 2026-07-26 03:45 — Merge-triage of the 30 open fable drafts vs post-rewrite main
- **Vision tie:** priority 1 (frontend-split / media-server tracks) — the tracks are blocked not
  by missing code but by 30 unreviewable drafts stranded behind the 2026-07-19 history rewrite;
  this unblocks them. New features tonight would only have deepened the conflict pile.
- **Branch / PR:** `loop/night-20260726` — https://github.com/atiner117/omnideck/pull/52
- **Changed:** new `docs/PR-TRIAGE-2026-07-26.md`. Deterministic git analysis (merge-base, tree
  ids, file-level overlap; `git merge-tree` was sandbox-blocked, so overlap is honestly labeled a
  conflict *superset*): all drafts root at pre-rewrite `c7c5067`, but the duplicated trunk is
  tree-identical (`369ed3b^{tree}` == `4b9389b^{tree}`), so each PR = 1–4 cherry-pickable commits.
  Six drafts superseded by #48 (close: #20, #22, #40, #38, #39 + mpris/tokens lanes and all of
  `loop/night-20260711`); eight zero-overlap clean picks (#33 #34 #35 #36 #29 #37 + pairs); a
  sequenced +page.svelte wave; two PR-less prerequisite lanes flagged (fable-audio backend for
  #35, fable-settings SettingDef table for #44/#46).
- **Verify:** docs-only — no build gates apply (bun/cargo untouched).
- **Outcome:** shipped to draft PR #52.
- **Next candidate:** if Andrew agrees with the triage, the loop can grind Waves 1–2 one
  cherry-pick per iteration (start: #33 doctor, then #34 logs — both zero-overlap `cli.rs`
  picks, full Verify gate each). Otherwise: `fable-audio` backend + #35 modal as one pick.

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
