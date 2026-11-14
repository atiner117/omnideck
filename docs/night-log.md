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

## 2026-09-11 22:54 — STOP (eleventh iteration). Nothing moved since last night's entry.
- **Vision tie:** `VISION.md` line 37 — *"If nothing is safely shippable tonight, log that and stop
  rather than inventing scope."* The standing next-candidate is still *"none until `main` moves."*
- **Branch / PR:** no new branch. Appended to `loop/night-20260830` / **#95** (chain tip).
- **State (complete open set, `--limit 200`, count first → 11, #85–#95):** `main` still
  `487bd4d` — 22 days (every PR's `baseRefOid` is `487bd4d`; newest merged PR is still #83). All
  eleven heads identical to the 09-10 entry (#95 `e04cd0c`, #94 `3c896ac`, … #85 `dc3ed28`);
  the only push since was that entry itself. 11/11 `CLEAN`, all CI green, **0 reviews, 0 comments**.
  The direct `main` checks (gh-token `fetch`, `gh api` commits) were permission-gated again.
- **Changed:** `docs/night-log.md` only.
- **Verify:** n/a — one markdown file on a 6/6-green tip.
- **Outcome:** **stopped — no wake-up scheduled.** Nothing to add to the 09-10 reasoning: a
  twelfth stacked head deepens a merge-ready queue that has not had its merge day.
- **Next candidate:** unchanged — **none until `main` moves.** Then: re-inventory, reorder this
  log newest-first, resume from `themes.ts` cycle-wrap tests (08-29 list).

## 2026-09-10 22:50 — STOP (tenth iteration). `main` unmoved — but the queue was restacked today.
- **Vision tie:** `VISION.md` line 37 — *"If nothing is safely shippable tonight, log that and stop
  rather than inventing scope."* The standing next-candidate — *"none until `main` moves"* — is
  still unsatisfied. Tonight is not a copy of the last nine entries, though: the queue changed
  shape, and the loop's branch had to catch up before it could even write this.
- **Branch / PR:** no new branch. Appended to `loop/night-20260830` / **#95**. Queue stays at eleven.
- **State (complete open set, `--limit 200`, count first → 11, #85–#95):**
  - **`main` still `487bd4d`** — 21 days. Same indirect check as 09-03 onward (gh-token `fetch`,
    `gh api` commits read, `git merge`/`reset` all permission-gated tonight): newest merged PR is
    still **#83 → `487bd4d` (2026-08-20T21:53Z)**. Proves no PR merged, not that nobody pushed.
  - **Ten of eleven heads moved today** (all but #86 `5c0807d`). A daytime merge-day session
    (worktree `/tmp/mday-prep`, git identity `merge sim`, 19:56–21:38Z) turned the eleven
    independent heads into a **stacked chain**: `#87` merges `#85`+`#86`; `#88` is on `#87`; …
    `#95` (`e0158ee`) is on `#94`. Each PR's head branch now *contains* every PR below it, so
    **#95 is the whole queue integrated**, and the landing order is simply **#85 → #86 → #87 →
    … → #95**, each a clean merge once its predecessor lands. This retires the 08-30 landing-order
    analysis — read that entry for *why*, not for *what to do*.
  - **Five integration commits** landed inside the chain, all by the merge-sim identity today:
    `dc3ed28` fix(switcher) leaderless frozen groups from Guide restore/close (on #85, after
    `92a7864`'s three review fixes); `322c995` fix(cli) `is_folder` in the `MediaItem` test literal
    (#87+#88); `34b2c88` fix(e2e) `is_folder` in the fixture builder (#88+#93); `dfc43de`
    fix(asset) `O_NONBLOCK` on the pre-validation open (#91, so a special file can't block it);
    `de1d869` fix(e2e) verify the WebKit shim downloads before unpacking (#93). The 09-05 worry —
    "#93's e2e job gates the rest of the queue" — is answered: the chain's tip runs it, **6/6
    green**. All **11/11** `MERGEABLE` / `CLEAN`, **0 reviews, 0 comments** on every PR.
  - **Side effect worth knowing:** the union-merges concatenated each branch's own night-log entry
    into this file, so below this entry the order is 08-22, 08-21, 08-23 … 08-29, *then* 09-08 …
    08-30, *then* 08-20 and older. Nothing is lost; it is just no longer newest-first. Not fixed
    tonight (a 600-line block reorder on the tip of a stack is not a 22:50 job).
- **Changed:** `docs/night-log.md` only. No source file touched. The loop's local branch was at
  `7f07366`, nine docs commits behind #95's new tip; the fast-forward was permission-gated, so this
  entry was committed on a scratch branch at `e0158ee` and pushed to `#95` as a plain fast-forward.
- **Verify:** **n/a, deliberately not run** — one markdown file on top of `e0158ee`, whose 6/6 CI
  is already green. Green by construction.
- **Outcome:** **stopped — loop halted, no wake-up scheduled.** Reasoning unchanged in kind: a
  twelfth head — or a further commit on the stack's tip — still deepens a queue that has now been
  *prepared* for a merge day but has not had one. The loop is not dry: the 08-29 candidates
  (`themes.ts` cycle-wrap, `SleepTimer`'s `formatRemaining`/`endsAt`, `MediaNav.marking` guard)
  remain unstarted.
- **Next candidate:** unchanged — **none until `main` moves.** When it does: re-inventory from
  scratch (the stack means any partially-merged state will look odd — verify each remaining PR
  is still `CLEAN` against the new `main` before assuming), reorder this log newest-first as the
  first docs commit of the new night branch, then resume from `themes.ts`.
- **For Andrew:** the queue is stacked and green; landing is #85 first, then straight up the
  numbers. Two open asks: (1) after the merges, the local `loop/night-20260830` in the main
  worktree is still `7f07366` — `git switch loop/night-20260830 && git pull --ff-only` (or just
  delete it; the branch is done once #95 lands); (2) the loop's `main` check is still gated —
  allowing the gh-token `fetch` or the `gh api` commits read would let it verify `main` directly.

## 2026-08-22 — `omnideck watched <id> [--un]`: the mark-watched path, headless + verified
- **Vision tie:** not a feature — the *verification* surface. CLAUDE.md §Verify §3 exists so an
  agent can exercise real backend logic without a TV; `set_played` was the one couch action with
  no headless entry point, so #83's transport fixes could only be checked from the sofa. #83's
  follow-up list and the 2026-08-21 log both named this exact command as the cheapest
  agent-shippable item.
- **Branch / PR:** `loop/night-20260822` — https://github.com/atiner117/omnideck/pull/87
- **Open-PR inventory:** **2 open, total** — `gh pr list --state open --limit 200 --json number
  --jq 'length'` → `2`: **#85** (`fix/review-20260821`, P0/P1 fixes from the five-angle review)
  and **#86** (`loop/night-20260821`, the 0.2.0 changelog top-off). That is the whole list, not a
  subset. Checked both file lists before building: #85 touches `media_server.rs` (poison-tolerant
  lock) and 22 other files but **not** `cli.rs` and **not** `set_played`; #86 is `CHANGELOG.md` +
  `docs/night-log.md` only. No open PR adds a CLI subcommand, so this isn't a duplicate.
- **Changed:** `media_server.rs` — new `item(&self, id)` reads one item by id
  (`/Users/{user}/Items/{id}`, a bare `BaseItemDto`), wrapped into a one-element array so
  `items_of` stays the single field mapping; same two `valid_id` gates as `set_played` (item id
  *and* user id), pinned by a no-IO test against a dead loopback port. `cli.rs` — `watched <id>
  [--un]`: read state → flip through the same `PlayedItems` transport → **re-read**. The re-read
  is the entire value: a redirect that downgrades POST→GET and a jellyfin#8168 server that
  answers 200 without applying both look like success in the response body, and only a second
  read separates them. A `set_played` error deliberately does *not* short-circuit the re-read —
  "refused, nothing changed" vs "errored after applying" are different bugs. Prints
  `before:` / `set_played -> OK|FAILED` / `after:` / `VERIFIED|MISMATCH|UNKNOWN`.
  Two new tests (the first in `cli.rs`): the argv contract, and `watch_state` rendering a missing
  `UserData` as `played=(none)` rather than guessing "unwatched". Docs: CLAUDE.md §Verify §3
  gains the command **with an explicit "this WRITES to the server, and marking watched clears
  that item's resume point" caveat**; README's helper list was also missing `bgprep`/`logs`/
  `doctor`, filled in.
- **Verify:** `cargo clippy --all-targets -- -D warnings` **clean** in debug *and* `--release` ·
  `cargo test` **112 pass / 1 ignored** (110 before, +2), identical under `--release` ·
  `bun run check` **369 files, 0 errors / 0 warnings** · `bun run build` **pass** ·
  `bun run test` **47/47**. No new deps; no `#[ts(export)]` type changed, so `git status` stayed
  clean after the release test run (no bindings churn).
- **Deliberately NOT run:** the command against Andrew's live Jellyfin. It mutates real watch
  state, and marking watched destroys that item's resume point — an unattended loop should not
  pick one of his titles and flip it. `cargo run` also needs an approval this session didn't
  have. So: guards and argv are test-covered, the network path is one `cargo run -- watched <id>`
  away whenever Andrew wants it.
- **Merge-order note:** branched off `main` (`487bd4d`), so it does not contain #86. Both PRs
  prepend to this file → **expect a trivial conflict here if #86 merges first; keep both
  entries.** No `CHANGELOG.md` bullet was added for the same reason (#86 owns that file right
  now) — this command wants one line under `[Unreleased]` → *Added* once #86 lands.
- **Outcome:** shipped to draft PR #87. Additive, backend-only, zero frontend risk.
- **Next candidate:** still **the couch pass** — it is the only thing between here and the 0.2.0
  tag and the only thing an agent cannot do (`needs-hardware` has stacked up across #77/#78/#79/
  #81/#83/#84; capture it in a dated `NOTES-COUCHTEST-*.md`). This increment makes one slice of
  it cheaper: the mark-watched half can now be checked from a terminal before the TV is on.
  Agent-shippable alternatives if another is wanted first: #81's `parse_backup` normalize gap
  (cheapest of its five review follow-ups); making the shared-retry idempotency a mechanism
  rather than prose (opt-in per call site) before anything non-idempotent is added to `send()`;
  or the `BROWSE_KINDS` allowlist gap from #83's review. Still loose: #39's `0dabfea` (L2/R2
  synthesis, needs-hardware).

## 2026-08-21 — 0.2.0 changelog top-off: the 30 PRs merged since 2026-07-26
- **Vision tie:** not a feature — the release blocker. The last two iterations both named
  "changelog top-off for 0.2.0 (docs-only)" as the next candidate, and it gates the tag.
- **Branch / PR:** `loop/night-20260821` — https://github.com/atiner117/omnideck/pull/86
- **Open-PR inventory:** **1 open, total** — `gh pr list --state open --limit 200 --json number
  --jq 'length'` → `1`: **#85** (`fix/review-20260821`, "close the P0/P1 findings from the
  2026-08-21 five-angle review"). That is the whole list, not a subset. Searched it for
  changelog/release keywords before building — #85 is review fixes to #83's transport and resume
  state, nothing touching CHANGELOG.md or RELEASING.md, so this is not a duplicate. Note #85 is
  **unmerged**, so its fixes are deliberately *not* in the changelog: the notes describe `main`.
- **The finding:** the `[0.2.0]` section was last edited 2026-07-26 by #51. Thirty PRs merged
  after that (**#54–#84**, plus docs #52/#53/#65) and **none** were documented — the release notes
  described a version two months of work out of date. Separately the heading read
  `## [0.2.0] — 2026-07-27`, but **v0.2.0 is not tagged** (`git tag` → only `v0.1.0`), so it
  claimed a release date for a release that never happened. `RELEASING.md:28` is explicit that the
  heading stays `## [Unreleased] — X.Y.Z` until the bump PR stamps the date, so #50 retitled it
  early. Retitled back to `## [Unreleased] — 0.2.0`; the release step now works as written.
- **Changed:** `CHANGELOG.md` only, +150/−1. New bullets in all four subsections, in the
  section's existing voice (user-facing, no PR numbers — the mapping lives here and in the PR
  body). *Added* (16): resume + mark-watched (#81/#83), artwork disk cache (#84), library view
  modes (#79), themes (#80), overscan calibration (#78), screensaver (#59), audio output switcher
  (#58), sleep timer (#68), phone remote (#69), parental PIN (#56+#61), update check (#67),
  config backup/restore (#64), `[launch_overrides]` (#62), `[input]` (#66), `doctor`/`logs`
  (#54/#55), `docs/ARCHITECTURE.md` (#57). *Changed* (5): the `OVERLAYS` input-router
  unification + `+page.svelte` decomposition (#77/#74/#75/#76), table-driven settings (#60), icon
  windowing + derived clamps + quote-aware argv (#72), pooled X11 connection (#71), Jellyfin
  server re-resolve + `config_version` (#70). *Fixed* (1): spawn-error mapping (#63). *Security*
  (6): `X-Emby-Token` header instead of `api_key` query param (#70), PIN IPC mask +
  `set_locked_categories` (#61), `get_artwork` gated by `url_within_base` (#84), enumerated-sink
  validation (#58), credentials excluded from backups + the remote's constant-time token
  (#64/#69), and the `argon2` dependency note (#61).
- **Verification discipline (docs can be wrong silently — grep, don't recall):** every config key,
  command and module name asserted in the new text was checked against the tree before it shipped
  — `art_cache_mb`, `guide_hold_ms`, `session_hotkeys`, `launch_overrides`, `overscan_pct`,
  `check_updates`, `locked_categories`, `config_version`, `[appearance]`, `set_locked_categories`,
  `has_pin`, `url_within_base`, `get_artwork`, `X-Emby-Token`, `--http-header-fields`, and the
  `src/lib/components/` + `src/lib/themes/` files (#78/#79/#80 put them in subdirectories, not
  `src/lib/` — the PR bodies' paths would have been wrong if copied). The "only new runtime
  dependency" claim was verified by diffing the manifests over the window
  (`git diff 43d7c45..HEAD -- src-tauri/Cargo.toml package.json`) → `argon2` alone.
- **Verify:** bun run check (**pass**, 369 files, 0 errors / 0 warnings) · bun run build
  (**pass**) · bun run test (**47 pass**, 6 files). Rust untouched, so clippy/cargo test don't
  apply — the branch is at main's tip (`487bd4d`) otherwise. No CI job reads `CHANGELOG.md`
  (checked `ci.yml`; the version-sync job compares only the five version *sources*), so the
  `[Unreleased]` retitle can't break a gate. No new deps, no version change, no bindings churn.
- **Outcome:** shipped to draft PR. Docs-only, zero runtime risk.
- **Next candidate:** **the couch pass** is now the only thing between here and a 0.2.0 tag —
  everything else is written. It is the one thing an agent cannot do: the `needs-hardware` items
  have stacked up (#77 input layer, #78 overscan on the actual TV, #79 view modes, #81 resume
  landing on the right frame, #83 the ✓ surviving a reopen against a live Jellyfin, #84 the
  second-launch art pop-in disappearing). Capture it in a dated `NOTES-COUCHTEST-*.md`. Then
  RELEASING.md §1 (retitle `[Unreleased]` → `[0.2.0] — <date>`, `.SRCINFO`) and §2 (tag).
  If another agent-shippable increment is wanted first: #81's five review follow-ups, cheapest
  first (`parse_backup`'s remaining normalize gap), or #83's logged follow-ups (an
  `omnideck watched <id> [--un]` CLI subcommand would make the #85 transport fixes observable
  headlessly). Still loose: #39's `0dabfea` (L2/R2 synthesis, needs-hardware).

## 2026-08-23 — Browse rows classified by the server's `IsFolder`, not a five-name allowlist
- **Vision tie:** VISION §3 (polish on already-shipped surfaces) and the media-server track.
  Not a new feature — closing a **latent correctness gap** #83's five-angle review found and
  logged rather than fixed, and which the 2026-08-22 entry listed as an agent-shippable
  alternative to the couch pass.
- **Branch / PR:** `loop/night-20260823` — https://github.com/atiner117/omnideck/pull/88
- **Open-PR inventory:** **3 open, total** — `gh pr list --state open --limit 200 --json number
  --jq 'length'` → `3`. That is the complete list, not a subset: **#85** `fix/review-20260821`
  (P0/P1 fixes from the five-angle review), **#86** `loop/night-20260821` (0.2.0 changelog
  top-off), **#87** `loop/night-20260822` (`omnideck watched <id>`). Checked all three file
  lists before choosing: **none touches `src/lib/mediarow.ts` or `src/lib/medianav.svelte.ts`**,
  so this is not a duplicate and not a collision on the frontend half.
- **The bug:** `BROWSE_KINDS` was an allowlist of five `Type` names, so any container kind
  Jellyfin returns that isn't in it — `MusicAlbum`, `Playlist`, `UserView`, `AggregateFolder`,
  anything upstream adds later — fell through as **playable**. Two consequences: Enter handed a
  folder id to the play path instead of drilling in, and the row passed `toggleWatched`'s
  `!r.browse` gate, so **W offered to mark a whole container watched** — which clears every
  child's resume point with no undo. That is exactly what `toggleWatched`'s own doc comment
  says must never be reachable from one un-confirmed press. Latent today only because v1
  filters library views to `movies|tvshows|homevideos`; nothing structural was holding it.
- **Changed:** `media_server.rs` — `MediaItem.is_folder: Option<bool>` from
  `BaseItemDto.IsFolder`, mapped in `items_of`. `mediarow.ts` — new `isBrowse(i)`:
  `i.is_folder ?? BROWSE_KINDS.has(i.kind)`. **`??` not `||` on purpose** — `false` is a real
  server answer and must survive, `null` is the unknown one. `medianav.svelte.ts` calls it in
  `row()`. `BROWSE_KINDS` stays, demoted to the fallback and widened to the container kinds
  Jellyfin can actually return. Bindings regenerated (`MediaItem.ts`).
- **Why this can't regress:** if the server sends `IsFolder` we use its own answer; if it
  doesn't, `None` → the name-list path, i.e. **today's exact behaviour**. Worst case is the
  status quo, never worse. The widened fallback then narrows the gap even on a response with
  no `IsFolder`.
- **Verify:** `bun run check` **369 files, 0 errors / 0 warnings** · `bun run build` **pass** ·
  `bun run test` **50/50** (47 before, +3) · `cargo clippy --release --all-targets -D warnings`
  **clean** · `cargo test --release` **109 pass / 1 ignored**. Bindings: re-exported and
  `diff -rq` against `src/lib/bindings` is identical, so CI's clean-diff check passes. No new
  deps. No a11y/perf/security surface touched (one `??` in an existing pure function).
- **Not verified — be honest about this:** `IsFolder` was **not** confirmed against Andrew's
  live Jellyfin. Web search (searxng) and the live-config/`cargo run` probes were both
  unavailable in this session's permission set, so the claim "Jellyfin populates IsFolder" rests
  on the API shape, not on an observed response. This is why the fallback exists and why the
  field is `Option<bool>` — **if the assumption is wrong the code degrades to the old
  behaviour instead of misclassifying.** One `cargo run -- mediasrv` against the real server
  would upgrade this from "safe either way" to "confirmed working".
- **Merge-order note:** branched off `main` (`487bd4d`), so it contains neither #86 nor #87.
  Two things to expect: (1) this file — #86 and #87 also prepend here, so a trivial conflict is
  likely; **keep all entries**. (2) **A semantic, non-textual conflict with #87:** that branch
  adds two `MediaItem { … }` literals in `cli.rs` (~427 and ~442) for its `watched` tests. They
  predate `is_folder`, so whichever of #87/#88 merges second, `cargo check` will fail with
  "missing field `is_folder`" until each literal gets `is_folder: None`. Git will not flag it —
  the two branches touch different files. Two lines, but it will not show up until build.
- **Outcome:** shipped to draft PR #88. Additive; the only behaviour change is that containers
  the old list missed now drill down instead of being offered as playable.
- **Next candidate:** still **the couch pass** — unchanged as the top item and still the one
  thing an agent cannot do (`needs-hardware` stacked across #77/#78/#79/#81/#83/#84; capture in
  a dated `NOTES-COUCHTEST-*.md`), and still the only thing between here and the 0.2.0 tag.
  Agent-shippable alternatives, now one shorter: #81's `parse_backup` normalize gap (**note it
  collides with #85, which touches `config.rs` — check that diff first**); making shared-retry
  idempotency a mechanism rather than prose (opt-in per call site) before anything
  non-idempotent joins `send()`; or the *server-side* half of tonight's fix — `set_played` still
  trusts its caller not to hand it a container id, so a Rust-side `Type`/`IsFolder` check would
  make the playable-only rule real instead of TS-only. Still loose: #39's `0dabfea` (L2/R2
  synthesis, needs-hardware).

## 2026-08-24 — The playable-only watch-state rule becomes a rule: enforced in `set_played`
- **Vision tie:** VISION §3 (polish on already-shipped surfaces) and the media-server track.
  Not a new feature — closing the **last unfixed correctness follow-up** from #83's five-angle
  review, which logged it rather than fixing it, and which the 2026-08-23 entry named as the
  best agent-shippable item after the couch pass.
- **Branch / PR:** `loop/night-20260824` — https://github.com/atiner117/omnideck/pull/89
- **Open-PR inventory:** **4 open, total** — `gh pr list --state open --limit 200 --json number
  --jq 'length'` → `4`. Complete list, not a subset: **#85** `fix/review-20260821`, **#86**
  `loop/night-20260821` (0.2.0 changelog), **#87** `loop/night-20260822` (`omnideck watched
  <id>`), **#88** `loop/night-20260823` (browse rows by `IsFolder`). All four report
  `baseRefOid=487bd4d3`, which is also main's tip — **main has not moved since 2026-08-19**, so
  nothing has merged and every draft is still outstanding. Checked all four file lists before
  choosing; none touches `set_played`'s body or `played_request`.
- **Roadmap doc is stale — don't pick from it cold.** `NOTES-DEEPDIVE-ROADMAP.md` (dated
  2026-07-05) still lists parental controls (#2), the audio switcher (#3) and the update manager
  (#4) as unbuilt. #85's file list contains `pin.rs`, `audio.rs` and `update.rs`. All three are
  **done**; the doc is gitignored and was never refreshed. This is the exact trap the loop
  contract warns about, now confirmed a second time — treat open-PR titles as outranking it.
- **The gap:** the "only a single playable item may be marked watched" rule lived **only in the
  frontend row model**. So it was a convention, not a rule — `cli.rs` (and #87 adds exactly such
  a caller), a new Tauri command, or any future Rust code could hand a series/season/library-view
  id straight to `PlayedItems`. Jellyfin then clears the server-side resume point of **every
  child**; un-marking restores the `Played` flag but never the positions. That is the only
  irreversible operation in the whole media client, and its guard was one layer away from the
  code that performs it.
- **Changed:** `media_server.rs` only. New pure `is_container(&Value)` beside `played_request` —
  split out for the same reason that one was, so the rule is unit-testable without a live
  server. It reads the server's own `IsFolder` and falls back to a `CONTAINER_KINDS` `Type`
  name-list **only when the field is absent**: a nested `match`, deliberately **not an `||`**
  over both sources, because `IsFolder: false` is a real answer that must beat a stale list
  entry. `set_played` probes `GET /Users/{user}/Items/{id}` and refuses a container before
  issuing the mutating verb, naming the `Type` in the error.
- **Trade-offs, stated rather than buried:** (1) **one extra LAN round-trip per toggle** — paid
  deliberately for the only irreversible call we make; it is not on the rAF/input path, so
  NOTES-PERFORMANCE is unaffected. (2) **Fails closed** if the probe errors — costs no behaviour
  we would otherwise have had, since a server that can't answer the GET wouldn't have applied
  the POST. (3) **Unknown → allow**, so a server sending neither field keeps today's behaviour
  instead of losing a legitimate toggle.
- **Verify:** `cargo clippy --all-targets -D warnings` **clean** · `cargo test --release`
  **110 pass / 0 fail / 1 ignored** (+1 new) · `bun run check` **369 files, 0 errors / 0
  warnings** · `bun run build` **pass** · `bun run test` **47/47**. No new deps. No
  `#[ts(export)]` struct changed, so `src/lib/bindings/` is untouched and CI's clean-diff check
  is unaffected. `git status` showed exactly one modified file before the commit.
- **Not verified — be honest about this:** not exercised against the live Jellyfin (no
  `cargo run -- mediasrv` or network probe in this session's permission set), so
  `/Users/{userId}/Items/{itemId}` and `IsFolder` rest on the documented API shape, not an
  observed response. Note the failure direction: because this **fails closed**, a wrong route
  would make toggles *stop working*, not misfire — visible immediately, never silently
  destructive. One `omnideck watched <id>` run (#87) against the real server settles it.
- **Merge-order note:** branched off `main` (`487bd4d`) — contains none of #85–#88. Expect a
  trivial conflict in **this file** (#86/#87/#88 all prepend here too); **keep all entries**.
  The `media_server.rs` overlap should be textually clean — this touches only `set_played`'s
  body and adds a helper after `played_request`. **#88 overlaps semantically, not textually:**
  it applies the same `IsFolder`-first idea to the frontend row model. Complementary — TS stops
  the UI offering the action, this stops the backend performing it — but if both land,
  `BROWSE_KINDS` (TS) and `CONTAINER_KINDS` (Rust) become two copies of the same fallback
  knowledge.
- **Outcome:** shipped to draft PR #89. Additive; the only behaviour change is that a container
  id now returns an error instead of silently wiping child resume points.
- **Next candidate:** still **the couch pass** — unchanged as the top item, still the only thing
  an agent cannot do (`needs-hardware` now stacked across #77/#78/#79/#81/#83/#84/#88/#89), and
  still the only thing between here and the 0.2.0 tag. **Worth flagging plainly: four green
  drafts are now queued behind a review that hasn't happened, and main hasn't moved in five
  days — the bottleneck is merging, not building.** Agent-shippable alternatives if that stays
  blocked: #81's `parse_backup` normalize gap (**collides with #85's `config.rs` — read that
  diff first**); making shared-retry idempotency a mechanism rather than prose (opt-in per call
  site) before anything non-idempotent joins `send()`; de-duplicating the two container-kind
  lists once #88 and #89 have both landed. Still loose: #39's `0dabfea` (L2/R2 synthesis,
  needs-hardware).

## 2026-08-26 — Close the two fail-open gaps in the shared HTTP fetch path (`icons.rs` + `http.rs`)
- **Vision tie:** VISION §3 (quality bars) and `NOTES-SECURITY.md`. Not a new feature — the last
  two *security-shaped* leftovers of the 2026-08-21 five-angle review, whose P0/P1 set #85
  landed and whose P2/P3 set nobody has touched since.
- **Branch / PR:** `loop/night-20260826` — https://github.com/atiner117/omnideck/pull/90
- **Open-PR inventory:** **5 open, total** — `gh pr list --state open --limit 200 --json number
  --jq 'length'` → `5`. Complete list, not a subset: **#85** `fix/review-20260821`, **#86**
  `loop/night-20260821`, **#87** `loop/night-20260822`, **#88** `loop/night-20260823`, **#89**
  `loop/night-20260824`. All five report `baseRefOid=487bd4d3` — **main has not moved since
  2026-08-19, now seven days.** Pulled all five *file lists* (23+6+5+2+2) before choosing and
  keyword-searched the full inventory for `ssrf` / `icons` / `favicon` / `http`: only #86 matched
  (changelog prose). **No open PR touches `http.rs` or `icons.rs`** — that absence claim rests on
  the complete set, both ways.
- **Local `main` was stale again** (`dfdfa32`, PR #48 era) — the documented trap. Branched from
  the *fetched* main SHA `487bd4d3` taken from the PRs' `baseRefOid`, not from `refs/heads/main`.
  Note for the next agent: `git fetch origin` fails here (SSH wants a yubikey touch); use
  `gh pr list --json baseRefOid` or the gh-token HTTPS remote to learn where main actually is.
- **Why this and not the roadmap:** `NOTES-DEEPDIVE-ROADMAP.md` is gitignored and stale (third
  confirmation). `NOTES-CODE-REVIEW-2026-08-21.md` is *committed, dated, and cross-verified*, and
  its P2/P3/test-gap sections are a far better backlog for a cold agent. **Next iterations should
  pick from it.** Ranked leftovers there, all still open: lock hygiene (`media_server.rs:150-168`,
  `update.rs:103,125` raw `.unwrap()` where the tree uses `sync::lock_or_recover`); TOCTOU
  triple-resolution in `asset.rs`/`get_art`; the `remote.rs` header-cap parser trap; `AudioSink`
  is the one ts-rs source-of-truth violation; `Modal.svelte:63` `.osk` is the single animated
  surface without a reduced-motion rule; test gaps ranked `npActions.ts` → `settings-defs.ts` →
  `themes.ts`/`SleepTimer` → `MediaNav.marking`.
- **Changed:** two files, one concern — *the SSRF guard must hold on every host we actually fetch,
  and must not be silently droppable.*
  1. **`icons.rs`** — `favicon()` gates `host`, then quietly builds candidates for a **second**
     host, `root_domain(host)`, including a direct `https://{root}/favicon.ico` **fetch** that was
     never checked. `--app=https://foo.127.1` clears the entry gate (the `foo` label makes the
     host non-numeric; it also doesn't resolve) and derives `127.1` — an `inet_aton` short form
     for `127.0.0.1`, so an icon fetch becomes a loopback probe. Needs no crafting either: a
     public `sub.example.com` whose bare `example.com` resolves internally under split-horizon
     DNS. The derived domain now gets its own `is_blocked_host_resolved`; a blocked root is never
     a legitimate icon source, so the **whole** domain is dropped rather than only its direct
     fetch — no reason to hand an internal hostname to DDG/Google as a query parameter either.
     (Redirect *hops* were already covered by the client's redirect policy; this is the
     initial-URL side, for the candidate that never had one.)
  2. **`http.rs`** — `.build().unwrap_or_default()` failed open on **two** controls at once: a
     default `reqwest::Client` carries neither the timeout policy nor the SSRF redirect policy.
     The fallback bought nothing — the only realistic failure is TLS-backend init, where every
     HTTPS fetch errors anyway, so there was no usable degraded mode being preserved. Now
     `.expect` with a message naming what it refuses to hand back.
- **Verify:** `cargo clippy --release --all-targets -- -D warnings` **clean** (debug too) ·
  `cargo test --release` **110 pass / 0 fail / 1 ignored** (+1 new) · `bun run check` **369 files,
  0 errors / 0 warnings** · `bun run build` **pass** · `bun run test` **47/47**. `git status`
  showed exactly the two intended files before the commit. No new deps. No `#[ts(export)]` struct
  changed, so `src/lib/bindings/` is untouched and CI's clean-diff check is unaffected.
- **Not verified — be honest about this:** the loopback-probe vector was reasoned from the code
  plus `inet_aton` short-form semantics and pinned with a **no-IO** unit test
  (`derived_root_domain_can_escape_the_entry_gate`); it was *not* demonstrated end-to-end against
  a live listener, which would mean a network fetch inside a test. Failure direction is safe: the
  fix is fail-closed, so a wrongly-blocked root domain costs one fallback icon candidate, never a
  wrong fetch.
- **Outcome:** shipped to draft PR #90. Additive and contained; the only behaviour change is that
  a root domain resolving somewhere internal stops being fetched.
- **Next candidate:** **the couch pass is still the top item and still the only thing an agent
  cannot do** — `needs-hardware` now stacked across #77/#78/#79/#81/#83/#84/#88/#89, and it is
  still the only thing between here and the 0.2.0 tag. **Say the real bottleneck plainly: six
  green drafts are queued behind a review that hasn't happened, and main has not moved in seven
  days. The constraint is merging, not building** — and note that #85/#87/#88/#89 all touch
  `media_server.rs`, so the conflict cost of the queue grows with every media increment. Tonight
  deliberately avoided that file for exactly this reason. If the queue stays blocked, the best
  agent-shippable work is the `NOTES-CODE-REVIEW-2026-08-21.md` P2 list above, preferring items in
  files no open PR touches: `asset.rs` TOCTOU, `remote.rs` parser, or the `npActions.ts` /
  `settings-defs.ts` test gaps (pure modules, zero conflict risk). Avoid `media_server.rs`,
  `config.rs`, `commands.rs`, `+page.svelte` and `Modal.svelte` until the queue drains.

## 2026-08-27 — Close the TOCTOU triple-resolution in the `omnideck://` asset chokepoint
- **Vision tie:** VISION §3 (quality bars) / `NOTES-SECURITY.md`, via `NOTES-CODE-REVIEW-2026-08-21.md`
  **P2 — "TOCTOU triple-resolution"**. Continues the previous iteration's explicit instruction:
  with the merge queue blocked, work the committed review's P2 list, preferring files no open PR
  touches.
- **Branch / PR:** `loop/night-20260827` — https://github.com/atiner117/omnideck/pull/91
- **Open-PR inventory:** **6 open, total** — `gh pr list --state open --limit 200 --json number
  --jq 'length'` → `6`. Complete list, not a subset: **#85** `fix/review-20260821`, **#86**
  `loop/night-20260821`, **#87** `loop/night-20260822`, **#88** `loop/night-20260823`, **#89**
  `loop/night-20260824`, **#90** `loop/night-20260826`. Pulled the full file list of all six
  before choosing (`gh pr list --json number,files`); the union is `media_server.rs`,
  `commands.rs`, `remote.rs`, `config.rs`, `switcher.rs`, `watchdog.rs`, `update.rs`, `audio.rs`,
  `pin.rs`, `http.rs`, `icons.rs`, `+page.svelte`, `Modal.svelte`, `LauncherForm.svelte`,
  `AudioOutputModal.svelte`, `medianav.svelte.ts`, `mediarow.ts`, `CHANGELOG.md`, packaging, and
  the NOTES-*. **`asset.rs` is in none of the six** — that absence claim rests on the complete
  set, checked file-by-file, not on a keyword search.
- **Main still has not moved.** All six PRs report `baseRefOid=487bd4d3`, unchanged since
  2026-08-19 — now **eight days**. Local `refs/heads/main` is still stale (`dfdfa32`, PR #48 era);
  branched from the fetched `487bd4d` taken from `baseRefOid`, per the documented trap. `git fetch
  origin` still can't run here (SSH wants a yubikey touch).
- **Why this one:** #90's next-candidate note ranked the remaining P2 items by conflict risk and
  named exactly three safe picks — `asset.rs` TOCTOU, the `remote.rs` parser, and the
  `npActions.ts`/`settings-defs.ts` test gaps. `remote.rs` turned out to be in #85's diff, so it
  was out. Between the `asset.rs` fix and the test gaps, the fix is the higher-value one: it's a
  real security-shaped defect in the single chokepoint for every art request, and it's one file.
- **Changed:** one file, one concern — `src-tauri/src/asset.rs`.
  `resolve_and_read()` resolved the requested path **three separate times**: `canonicalize()`,
  then `metadata()`, then `read()`. Every allowlisted root is a user-writable cache dir
  (SteamGridDB art, artwork cache, downscaled wallpapers), so the inode that cleared the root /
  extension / size gates was not necessarily the inode whose bytes got served. Now the open
  happens first and each check interrogates that descriptor: new `fd_path()` reads
  `readlink("/proc/self/fd/N")` for the path the kernel actually resolved (`..`/symlinks already
  collapsed) and feeds the root-allowlist + MIME gates; `f.metadata()` is an `fstat(2)` on the
  same handle and **now also rejects non-regular files**, which the path-based check never did;
  the bytes come off that fd, `take(MAX_BYTES)`-capped so an append after the fstat can't overrun
  the cap. Linux-only via `/proc`, matching `proc.rs`/`switcher.rs`. No new deps. No
  `#[ts(export)]` struct touched → `src/lib/bindings/` untouched, CI's clean-diff check
  unaffected. `git status` showed exactly the one intended file before the commit.
- **Deliberately half-done, and why:** the same P2 item also names `commands.rs:63-86` (`get_art`),
  which has the identical canonicalize→metadata→read shape. `commands.rs` is in **#85**'s diff, so
  fixing it tonight would put a conflict into the queue. Deferred until #85 lands — it is the
  natural follow-up and the fix is a copy of this one.
- **Verify:** `cargo clippy --release --all-targets -- -D warnings` **clean** (debug profile too) ·
  `cargo test --release` **111 pass / 0 fail / 1 ignored** (+2 new) · `bun run check` **369 files,
  0 errors / 0 warnings** · `bun run build` **pass** · `bun run test` **47/47**.
- **Not verified — be honest:** the swap window was reasoned from the code and closed
  structurally; it was *not* demonstrated with a live racing writer against a cache dir. Failure
  direction is safe — every new check is fail-closed, so a wrongly-rejected file costs one 404'd
  art request, never a wrong read.
- **Residual, logged not fixed:** `File::open` on a FIFO planted in a cache dir blocks until a
  writer appears. Pre-existing (the old code hit it inside `fs::read`) and unchanged here; closing
  it needs `O_NONBLOCK` via `libc`/`rustix` — a new dependency, out of scope for one increment.
- **Outcome:** shipped to draft PR #91.
- **Next candidate:** **the bottleneck is still merging, not building — say it plainly.** Seven
  green drafts are now queued behind a review that hasn't happened, and main is eight days cold;
  the couch pass (`needs-hardware`, stacked across #77/#78/#79/#81/#83/#84/#88/#89) remains the
  only thing an agent cannot do and the only thing between here and the 0.2.0 tag. Every media
  increment raises the conflict cost of the queue, so keep avoiding `media_server.rs`,
  `commands.rs`, `config.rs`, `remote.rs`, `+page.svelte`, `Modal.svelte`. Remaining zero-conflict
  work from `NOTES-CODE-REVIEW-2026-08-21.md`: the **`npActions.ts` test gap** (ranked #1, pure and
  branchy, feeds two surfaces whose drift is its reason to exist) then **`settings-defs.ts`** (282
  lines, zero tests), then `themes.ts` cycle-wrap / `SleepTimer`'s `formatRemaining`/`endsAt`.
  Lock hygiene (`sync::lock_or_recover` in `media_server.rs`/`update.rs`) and the `AudioSink`
  ts-rs violation both sit in queued files — **don't**.

## 2026-08-28 — Cover `npActions.ts`: the #1-ranked test gap, and zero conflict with the queue
- **Vision tie:** VISION §3 (quality bars) via `NOTES-CODE-REVIEW-2026-08-21.md` →
  **Test-coverage gaps, item 1**: "`npActions.ts` — pure, branchy, feeds two surfaces whose drift
  is its stated reason to exist." This is the exact pick the 2026-08-27 entry left as its
  next candidate, and it still made sense, so it was taken unchanged.
- **Branch / PR:** `loop/night-20260828` — https://github.com/atiner117/omnideck/pull/92
- **Open-PR inventory:** **7 open, total** — `gh pr list --state open --limit 200 --json number
  --jq 'length'` → `7`. Complete list, not a subset: **#85** `fix/review-20260821`, **#86**
  `loop/night-20260821`, **#87** `loop/night-20260822`, **#88** `loop/night-20260823`, **#89**
  `loop/night-20260824`, **#90** `loop/night-20260826`, **#91** `loop/night-20260827` (last
  night's asset.rs TOCTOU fix). Pulled the full file list of all seven (`gh pr list --json
  number,files`) before choosing. **`npActions.ts` is in none of the seven** — that absence claim
  rests on the complete set, checked file-by-file, not on a keyword search.
- **Main still has not moved.** All seven PRs report `baseRefOid=487bd4d3` — unchanged since
  2026-08-19, now **nine days**. Branched from that fetched SHA per the documented stale-local-ref
  trap. Two caveats, stated honestly: `git fetch origin` still can't run here (SSH wants a yubikey
  touch), and the `gh api .../commits/main` cross-check was **denied by the permission prompt**,
  so main's tip is corroborated *indirectly* — by seven independent PRs agreeing on the same base
  — rather than read directly.
- **Changed:** one new file, `src/lib/npActions.test.ts` (18 tests). `npActions.ts` itself is
  **byte-identical to main** — `git diff` against it is empty; this increment is test-only.
  Pinned: the gating (transport only with MPRIS metadata; ⇄ only for `kind === "app"` **and**
  `inSession`; close only for `"app"`; ✕ suppressed on the synthetic unlaunched-media card, which
  owns no launch id to dismiss), the render order both surfaces depend on, the `run()` call
  arguments incl. `switchApp` receiving *this* card's id (the original drift), the `after` hook
  (fires on the three terminal actions, never on transport, fires even when the IPC **rejects** so
  the overlay can't stick open on a backend error, and is optional — the card stack omits it), and
  per-path error routing to `onerror`. Backend mocked with `vi.mock("./backend")`, so no Tauri
  runtime is needed and the node vitest env stays as-is. No new deps.
- **Mutation-checked, not just green.** A passing new test file proves nothing on its own, so two
  deliberate mutations were applied and reverted: `api.switchApp(c.id)` → `api.switchApp()`, and
  replacing the `c.kind !== "media"` dismiss guard with `if (true)`. Each failed **exactly one**
  intended test (`2 failed | 63 passed`), then the source was restored and confirmed clean.
- **One real snag, worth remembering:** the first draft was green on `bun run test` but **failed
  `bun run check` with 19 errors** — `ReturnType<typeof vi.fn>` erases the callback signature, so
  the mocks didn't satisfy `cardActions`' options parameter. Fixed by deriving the types from the
  function under test (`type Opts = Parameters<typeof cardActions>[1]`, then `Mock<Opts["onerror"]>`
  / `vi.fn<Opts["onerror"]>()`). **`bun run test` passing is not the gate — CI runs `check` too.**
- **Verify:** `bun run check` **pass** (370 files, 0 errors / 0 warnings) · `bun run build`
  **pass** · `bun run test` **pass, 65/65** (was 47; +18) · `cargo check` / `cargo clippy` **n/a**,
  no Rust touched · ts-rs bindings **n/a**, no `#[ts(export)]` struct touched, so CI's
  clean-`src/lib/bindings` diff check is unaffected.
- **Outcome:** shipped to draft PR #92.
- **Next candidate:** **the bottleneck is the merge queue, not the build — this is now the third
  night in a row saying so.** Eight green drafts (#85–#92) are stacked behind a review that hasn't
  happened and a couch pass (`needs-hardware`, across #77/#78/#79/#81/#83/#84/#88/#89) that no
  agent can do; main is nine days cold and that couch pass is the only thing between here and the
  0.2.0 tag. If the next iteration finds main *still* at `487bd4d`, consider stopping the loop and
  saying so plainly rather than deepening the stack — each increment raises the queue's conflict
  cost. If it continues: keep avoiding `media_server.rs`, `commands.rs`, `config.rs`, `remote.rs`,
  `http.rs`, `icons.rs`, `asset.rs`, `+page.svelte`, `Modal.svelte`. Remaining zero-conflict work
  from the committed review, in order: **`settings-defs.ts`** (282 lines of cycle/normalize/visible
  predicates, zero tests — same shape as tonight, and the next-largest untested pure module), then
  `themes.ts` cycle-wrap / unknown-id restart, then `SleepTimer`'s `formatRemaining`/`endsAt`, then
  the `MediaNav.marking` re-entrancy guard. Still **don't** touch lock hygiene
  (`sync::lock_or_recover`) or the `AudioSink` ts-rs violation — both sit in queued files. The
  `commands.rs:63-86` `get_art` TOCTOU twin of last night's `asset.rs` fix remains deferred behind
  #85 for the same reason.

## 2026-08-29 — Playwright screenshot harness; self-hosted Inter; contain-intrinsic-size corrections
- **Not a loop iteration** — interactive session with Andrew. Logged here anyway because the
  two findings below are traps the next cold agent would otherwise re-discover the hard way.
- **Vision tie:** "I haven't had time to test" — CLAUDE.md §Verify. This adds the missing rung
  between the vitest unit tests and `packaging/test-session.sh`: the whole frontend, driven in a
  real browser, no TV/controller/Rust build.
- **Branch / PR:** `feat/e2e-screenshot-harness` — see PR link in the commit trailer / gh.
- **Changed:**
  - `e2e/` + `playwright.config.ts` — mock Tauri IPC (`window.__TAURI_INTERNALS__` installed via
    `addInitScript`, so no production code changes), fixtures typed against the ts-rs bindings via
    a `Wire<T>` mapped type (bigint→number), and a 16-shot tour of the grid/rail/list/modals.
    Runs chromium + webkit; `e2e/install-webkit-deps.sh` makes webkit start on Arch.
  - `static/fonts/` + `src/lib/fonts.css` — Inter was named in the CSS but never shipped.
  - `ListView.svelte` / `GridView.svelte` — `contain-intrinsic-size` corrections (below).
- **Verify:** `bun run check` pass (0 errors / 370 files) · `bun run test` pass · `bun run build`
  pass · `bun run check:e2e` pass · 16/16 screenshots on both engines. Rust untouched.

### Trap 1 — `contain-intrinsic-size` is a CONTENT box, not a border box
Measuring a row with `getBoundingClientRect()` and pasting that number in **over-states it by the
element's padding**, which the UA then adds on top again. I shipped exactly that mistake on `.lrow`
(`calc((2.6rem + 0.7rem) * scale)`, padding included) and made the scroll extent **+17.8%** wrong —
four times worse than the flat `64px` it replaced (−4.3%). Correct value is the content box alone —
the thumbnail, `calc(2.6rem * var(--scale, 1))` — now −0.9%..−1.6% across all four UI scales in both
engines. If you touch this property, the number you want is *not* the one dev-tools shows you.

### Trap 2 — a 15-row fixture cannot test `content-visibility` at all
`content-visibility: auto` only skips content outside the viewport **plus a generous margin**, so
with the default fixture library (15 rows) nothing is ever skipped and the intrinsic size is never
consulted. Every variant then measures identical, which reads as "my change is fine" — it is
measuring nothing. **Verification method: rebuild the fixture with ~500 games and compare
`.lwrap`/`.gwrap` `scrollHeight` against a control that forces `content-visibility: visible`.**
That is the only way the property engages. This is also how `.gtile` was settled: at 500 tiles,
ground truth / no declaration / a deliberately wrong value all produce an identical scroll extent,
because a `1fr` grid track gives a definite width and `aspect-ratio` derives the height — so its
`240px` was inert, and was removed rather than "corrected".

### Also found, not fixed
- **Buttons never inherited the app's font.** UA stylesheets hard-set a font on form controls, and
  nearly every surface here is a `<button>` (rail tiles, grid tiles, list rows, deck cards). Only
  `Modal.svelte`/`PinModal` set `font: inherit` locally. Fixed globally in `fonts.css` — but note
  this means every screenshot taken before today shows the *wrong typeface*.
- `ScreensaverOverlay` renders outside `<main>`, so it still doesn't get the font stack (the
  `font-family` lives on `main`, not `:root`). Untouched — no screenshot covers it.
- `03-games-rail-scrolled` has ~0.002% run-to-run pixel noise (rail transform still settling).
  Harmless for review shots, but it must be fixed before these become `toHaveScreenshot()` baselines.
- **Next candidate:** move `font-family` from `main` to `:root`, then wire the e2e run into
  `ci.yml` as its own job (Ubuntu runners need no ICU workaround, so webkit works there natively).

## 2026-08-29 — Pin the Settings table: `settings-defs.ts` coverage (review gap #2)
- **Vision tie:** VISION §3 (quality bars) via `NOTES-CODE-REVIEW-2026-08-21.md` →
  **test-coverage gaps, item 2**: "`settings-defs.ts` — 282 lines of cycle/normalize/visible
  predicates, zero tests." Exactly the candidate the 2026-08-28 entry left behind, taken unchanged.
- **Branch / PR:** `loop/night-20260829` — https://github.com/atiner117/omnideck/pull/94
- **Open-PR inventory:** **9 open, total** — `gh pr list --state open --limit 200 --json number
  --jq 'length'` → `9`. Complete list, not a subset: **#85** `fix/review-20260821`, **#86**
  `loop/night-20260821`, **#87** `loop/night-20260822`, **#88** `loop/night-20260823`, **#89**
  `loop/night-20260824`, **#90** `loop/night-20260826`, **#91** `loop/night-20260827`, **#92**
  `loop/night-20260828`, **#93** `feat/e2e-screenshot-harness`. Pulled the full file list of all
  nine (`gh pr list --json number,files`) before choosing; `settings-defs.ts` is in **none** of
  them — that absence claim rests on the complete set, checked file-by-file.
- **#93 is new and is not this loop's work.** Opened 2026-08-30T01:38Z under `atiner117`: a
  Playwright e2e screenshot harness (`e2e/`, 16 screenshots over a mocked `__TAURI_INTERNALS__`),
  self-hosted Inter, and `contain-intrinsic-size` fixes. It touches `GridView`/`ListView`/
  `+layout.svelte`/`fonts.css` — **add those to the avoid-list** alongside the queued Rust files.
  It is the missing rung between vitest and `packaging/test-session.sh`, so it is worth reviewing
  before more frontend work stacks on top of it.
- **Main is still `487bd4d` — ten days cold, and now nine drafts deep.** All nine PRs report the
  same `baseRefOid=487bd4d3`, which is how main's tip was corroborated (`git fetch origin` needs a
  yubikey touch and can't run here). The 08-28 entry said to consider stopping if main hadn't
  moved. Judgement call, stated plainly: this increment was taken anyway because it is **test-only
  and touches zero files in any open PR**, so it adds ~no conflict cost to the queue — but the
  bottleneck is unchanged and is now the fourth night running. **The queue, not the build, is what
  needs Andrew.**
- **Changed:** one new file, `src/lib/settings-defs.test.ts` (38 tests). `settings-defs.ts` is
  **byte-identical to main** — `git diff` against it is empty. Pinned: the structure the page
  assumes but never checks (unique keys; **no orphaned section header** — headers are never
  filtered by `visibleSettings`, so a section whose rows are all conditional would render as a
  lone header; action rows == the two keys `doAction` dispatches, so a new action row that isn't
  wired fails here instead of becoming a dead button); `normalizeNum` clamping/rounding plus two
  properties over *every* numeric row — its default is already in range (else the first nudge
  jumps) and one D-pad step actually moves it (else it's a dead knob); a generic **no-dead-end**
  cycle property (feed each row's own patch back in, return to start within a lap); the
  hand-edited-value fallbacks incl. the deliberate asymmetry (off-list accent → `ACCENTS[1]`,
  off-list bg colour → `[0]`); the search-provider branch that clears `search_provider` **only**
  when it still holds a preset URL, so a typed SearXNG URL survives; the sound preset ladder,
  its float epsilon, Custom→Off, the preview blip firing only when switching *on*, and volume 0
  clearing `sound`; and every visibility predicate. `./sfx` mocked (blip swallows its own errors,
  so it would pass unmocked and prove nothing); node env unchanged; no new deps.
- **Mutation-checked, not just green.** Four mutations applied together — bgcolor fallback index
  `-1 → 0`, dropping the `SEARCH_MODES.some(...)` guard, unconditional blip, and dropping
  `sound: v > 0` from the volume setter — gave **exactly four failures, one per mutation, each the
  intended test** (`4 failed | 81 passed`). Source restored and re-verified clean.
- **One documented wart, pinned rather than silently blessed:** a `background_image` path ending in
  `/` renders blank instead of `(none)` — `split("/").pop()` returns `""`, which isn't nullish so
  the `??` never fires. Cosmetic; left as-is because this increment is test-only.
- **Harness note for the next agent:** the sandbox refuses `perl -0pi` in-place rewrites and
  refuses to run a repo-local shell script, so mutation checks have to be driven with the Edit tool
  (batch the mutations, run once, read the failing test names, revert). Also `bun run test` passing
  is *not* the gate — CI runs `check` too.
- **Verify:** `bun run check` **pass** (370 files, 0 errors / 0 warnings) · `bun run build`
  **pass** · `bun run test` **pass, 85/85** (was 47 on this base; +38) · `cargo check` /
  `cargo clippy` **n/a**, no Rust touched · ts-rs bindings **n/a**, no `#[ts(export)]` struct
  touched.
- **Outcome:** shipped to draft PR #94.
- **Next candidate:** **review the queue before adding to it.** If it's still nine deep and main is
  still `487bd4d`, the honest move is to stop rather than ship a tenth. If work continues, the
  remaining zero-conflict items from the committed review, in order: `themes.ts` cycle-wrap /
  unknown-id restart (small — `nextTheme`/`normalizeTheme`/`themeLabel`, and the Rust
  `theme_ids_match_frontend` test already guards the registry, so this is the frontend half), then
  `SleepTimer`'s exported `formatRemaining`/`endsAt`, then the `MediaNav.marking` re-entrancy
  guard. Avoid-list, now ten files: `media_server.rs`, `commands.rs`, `config.rs`, `remote.rs`,
  `http.rs`, `icons.rs`, `asset.rs`, `+page.svelte`, `Modal.svelte`, and (new, via #93)
  `GridView.svelte` / `ListView.svelte` / `+layout.svelte`. Still don't touch lock hygiene
  (`sync::lock_or_recover`) or the `AudioSink` ts-rs violation, and the `commands.rs:63-86`
  `get_art` TOCTOU twin stays deferred behind #85. **Note:** every night branch prepends to this
  file at the same anchor, so #86–#94 all conflict here on merge — resolve by keeping all entries,
  newest first.

## 2026-09-08 22:52 — STOP (ninth iteration, second check today). Nothing moved since 02:50.
- **Vision tie:** `VISION.md` line 37 — *"If nothing is safely shippable tonight, log that and stop
  rather than inventing scope."* The standing next-candidate — *"none until `main` moves"* — is
  unsatisfied for the ninth consecutive iteration.
- **Branch / PR:** no new branch. Appended to `loop/night-20260830` / **#95**, as on every night
  since 08-31. The queue stays at eleven heads.
- **State, three calls, no analysis:**
  - **`main` still `487bd4d`** — nineteen days cold. Same indirect check (direct `gh api` commits
    read and gh-token `ls-remote`/`fetch` remain permission-gated): newest merged PR is still
    **#83 → `487bd4d` (2026-08-20T21:53Z)**. Same standing caveat — proves no PR merged, not that
    nobody pushed directly.
  - **Queue: complete open set, `--limit 200`, count first → 11** (#85–#95), unchanged for the
    eighth consecutive check. All **11/11** still `MERGEABLE` / `mergeStateStatus=CLEAN`, and every
    `headRefOid` is byte-identical to last night's. Nineteen days, still zero rebase debt.
  - **Zero review activity.** #93 queried directly this time rather than inferred: `reviews=0`,
    `comments=0`, head still `5dfef88`, `updatedAt` still 2026-09-05T14:34Z. Three days on, the
    09-06 reading of that commit as "the queue starting to move" stays refuted.
- **Changed:** `docs/night-log.md` only. No source file touched.
- **Verify:** **n/a, deliberately not run** — one markdown file on a branch that is `487bd4d` plus
  docs commits. Green by construction.
- **Outcome:** **stopped — loop halted, no wake-up scheduled.** Unchanged reasoning: shipping a
  twelfth head deepens a queue Andrew has not had a merge day for. The loop is not dry — the 08-29
  entry's three zero-conflict candidates (`themes.ts` cycle-wrap, `SleepTimer`'s
  `formatRemaining`/`endsAt`, the `MediaNav.marking` re-entrancy guard) remain unstarted.
- **Next candidate:** unchanged — **none until `main` moves.** When it does: re-inventory the open
  PRs from scratch (every list above will be stale; do not reuse them as an avoid-list), then resume
  from the 08-29 candidates starting with `themes.ts`.
- **For Andrew:** no new asks; the two from 09-06 still stand (re-gate #85, then #88, against #93's
  new e2e job before merging; and unblock the loop's `main` check by allowing the gh-token
  `ls-remote`/`fetch` or the `gh api` commits read — `gh pr diff` too).

## 2026-09-08 02:50 — STOP (eighth night). Nothing moved; short entry by design.
- **Vision tie:** `VISION.md` line 37 — *"If nothing is safely shippable tonight, log that and stop
  rather than inventing scope."* The 09-07 next-candidate is a conditional instruction to this
  iteration: *"none until `main` moves."* It has not moved.
- **Branch / PR:** no new branch. Appended to `loop/night-20260830` / **#95**, as on every night
  since 08-31. The queue stays at eleven heads.
- **State, three calls, no analysis:**
  - **`main` still `487bd4d`** — eighteen days cold, thirteenth night. Same indirect check as the
    last five nights (the `gh api …/commits/main` read and the gh-token `ls-remote`/`fetch` are
    still permission-gated): newest merged PR is **#83 → `487bd4d` (2026-08-20T21:53Z)**. Same
    standing caveat — this proves no PR merged, not that nobody pushed directly.
  - **Queue: complete open set, `--limit 200`, count first → 11** (#85–#95), unchanged for the
    seventh consecutive check. All **11/11** still `MERGEABLE` / `mergeStateStatus=CLEAN`. Eighteen
    days, still zero rebase debt.
  - **Zero review activity.** Every `updatedAt` is identical to last night except #95's own log
    commit — a review or comment would have bumped it, so this is read off data already fetched.
- **#93's 09-06 commit did not continue.** Still `5dfef88`, `updatedAt` still 2026-09-05T14:34Z. Two
  days on, the 09-06 entry's reading of that commit as "the queue starting to move" is refuted for
  now. #93 is still green on all five checks and still the recommended first landing.
- **Changed:** `docs/night-log.md` only. No source file touched.
- **Verify:** **n/a, deliberately not run** — one markdown file on a branch that is `487bd4d` plus
  docs commits. Green by construction.
- **Outcome:** **stopped — loop halted, no wake-up scheduled.** Unchanged reasoning. The loop is not
  dry: the 08-29 entry's three zero-conflict candidates (`themes.ts` cycle-wrap, `SleepTimer`'s
  `formatRemaining`/`endsAt`, the `MediaNav.marking` re-entrancy guard) remain unstarted.
- **Next candidate:** unchanged — **none until `main` moves.** When it does: re-inventory the open
  PRs from scratch (the lists above will be stale; do not reuse them as an avoid-list), then resume
  from the 08-29 candidates starting with `themes.ts`.
- **For Andrew:** no new asks. The two from 09-06 still stand (re-gate #85, then #88, against #93's
  new e2e job before merging; and unblock the loop's `main` check by allowing the gh-token
  `ls-remote`/`fetch` or the `gh api` commits read — `gh pr diff` too).

## 2026-09-07 02:48 — STOP (seventh night). Condition re-checked in three calls; nothing moved, including #93.
- **Vision tie:** `VISION.md` line 37 — *"If nothing is safely shippable tonight, log that and stop
  rather than inventing scope."* The 09-06 next-candidate instructs this iteration: *"none until
  `main` moves."* It has not. Nothing re-derived, no candidate re-ranked, no landing order restated.
- **Branch / PR:** no new branch. Appended to `loop/night-20260830` / **#95**, as on 08-31, 09-01,
  09-03, 09-04 and 09-06. The queue stays at eleven heads.
- **State is byte-identical to last night.** Three calls, no analysis:
  - **`main` still `487bd4d`** — seventeen days cold, twelfth night running. Same indirect check as
    the last four nights (`gh api …/commits/main` and the gh-token `ls-remote`/`fetch` are *still*
    permission-gated): newest merged PR is **#83 → `487bd4d` (2026-08-20T21:53Z)**. Same standing
    caveat — this proves no PR merged, not that nobody pushed directly.
  - **Queue: complete open set, `--limit 200`, count first → 11** (#85–#95), unchanged for the sixth
    consecutive check. All **11/11** still `MERGEABLE` / `mergeStateStatus=CLEAN`. Seventeen days,
    still zero rebase debt.
- **The 09-06 signal did not continue.** #93 is still at **`5dfef88`**, `updatedAt` still
  **2026-09-05T14:34Z** — no further commits, and it did not merge. Stated as one data point, not a
  trend: yesterday's entry read the manual `ci:` commit as the queue starting to move, and after one
  day that reading is neither confirmed nor refuted. #93 remains green on all five checks and is the
  Tier-1 head with no code conflicts, so the recommendation to land it first is unchanged.
- **Changed:** `docs/night-log.md` only. No source file touched.
- **Verify:** **n/a, deliberately not run** — one markdown file on a branch that is `487bd4d` plus
  docs commits. Green by construction.
- **Outcome:** **stopped — loop halted, no wake-up scheduled.** Same reasoning as 09-06: a twelfth
  head while Andrew is working the queue by hand adds review burden at the wrong moment. The loop is
  not dry — the 08-29 entry's three zero-conflict candidates (`themes.ts` cycle-wrap, `SleepTimer`'s
  `formatRemaining`/`endsAt`, the `MediaNav.marking` re-entrancy guard) remain unstarted.
- **Next candidate:** unchanged — **none until `main` moves.** When it does: re-inventory the open
  PRs from scratch (the lists above will be stale; do not reuse them as an avoid-list), then resume
  from the 08-29 candidates starting with `themes.ts`.
- **For Andrew:** nothing new to action beyond the 09-06 entry's two asks (re-gate #85, then #88,
  against #93's new e2e job before merging them; and unblock the loop's `main` check by allowing the
  gh-token `ls-remote`/`fetch` or the `gh api` commits read — `gh pr diff` too, so it can read queued
  changes instead of inferring from file paths).

## 2026-09-06 02:55 — STOP (sixth night) — but the queue moved for the first time in 15 days: #93 got a human commit today.
- **Vision tie:** `VISION.md` line 37 — *"If nothing is safely shippable tonight, log that and stop
  rather than inventing scope."* The 09-04 next-candidate is a conditional instruction to this
  iteration: *"none until `main` moves… If it is still `487bd4d`, stop again immediately."* `main` is
  still `487bd4d`, so the stop holds — but the condition around it changed, and that is what this
  entry is for. **Reverses the 09-04 headline claim**, which said there had been "no review activity
  of any kind on any of the eleven." That is now false.
- **Branch / PR:** no new branch. Appended to `loop/night-20260830` / **#95**, as on 08-31, 09-01,
  09-03 and 09-04. The queue stays at eleven heads.
- **`main` still `487bd4d`** — sixteen days cold, eleventh night running. Verified the same indirect
  way as the last two nights, because the direct paths are *still* permission-gated (`git ls-remote`
  over the gh-token HTTPS helper, and `gh api …/commits/main`). Substitute that worked:
  `gh pr list --state merged` → newest merge is still **#83 → `487bd4d` (2026-08-20T21:53Z)**, with
  #84 behind it. Same standing caveat: this proves *no PR merged*, not that nobody pushed directly.
- **Queue:** complete open set, `--limit 200`, count first → **11** (#85–#95), unchanged for the
  fifth consecutive check. All **11/11** still `MERGEABLE` / `mergeStateStatus=CLEAN`. Sixteen days,
  still zero rebase debt.

### The new fact: #93 was touched today, by hand
`updatedAt` on **#93** is **2026-09-05T14:34Z** — today, and *not* this loop (the loop's only writes
are the night-log commits on #95, timestamped 02:49Z). The cause is a new commit on the branch:

- **`5dfef88` — "ci: run the e2e harness as its own job" (2026-09-05T14:33Z)**, sitting on top of
  #93's four original 08-30 commits.

Attribution caveat, stated plainly: every commit on #93 shows author `atiner117`, which is also the
loop's identity, so the author field does not by itself prove a human wrote it. What does: the loop
never ran at 14:33Z, and no night-log entry claims this work. So this is Andrew (or another manual
session) working the queue — **the first non-loop activity on any PR in fifteen days.**

**#93 is now green on all five checks**, including the new job:
`lint · test · build (linux)` · `e2e (screenshot tour · chromium + webkit)` · `cargo-deny` ·
`cargo-audit` · `version sync` — all `SUCCESS`.

### A landing-order consequence the 08-30 analysis does not cover
The 08-30 overlap map put #93 in Tier 1 — "collides with nothing but the `docs/night-log.md`
prepend." **That is still true textually, and I re-verified it against the complete open set** (not a
subset): searching all 11 PRs for `.github/` paths returns exactly two, touching *different* files —

| PR | `.github/` path |
|---|---|
| #93 | `.github/workflows/ci.yml` (+58/−0) |
| #85 | `.github/workflows/packaging.yml` |

So there is no CI merge conflict. **But the semantic consequence is new:** once #93 lands, the e2e
screenshot job becomes part of CI for every PR merged after it — and **none of the other ten have
ever run it.** A queue that is currently 11/11 clean could acquire a failing check on the first
merge, which would look like rot but would be an untested gate, not a regression.

Which of the remaining ten are actually exposed, by what they touch in the frontend:

- **#85** — `+page.svelte` plus `Modal`/`LauncherForm`/`AudioOutputModal`. This is the grid page the
  screenshot tour walks. **Highest exposure; re-gate this one first.**
- **#88** — `medianav.svelte.ts` / `mediarow.ts`, which feed list rows. Moderate exposure.
- **#92, #94** — frontend paths are `*.test.ts` only. No render surface; no exposure.
- **#86, #87, #89, #90, #91, #95** — Rust, CLI and docs only. No exposure.

I could not read the spec to say whether it does baseline image comparison (`toHaveScreenshot`) or
just render assertions — `gh pr diff` and the branch-protection read were both permission-gated this
session. Nor could I confirm whether the new job is a *required* check on `main`. Both would sharpen
the risk estimate; neither changes the recommendation.

- **Changed:** `docs/night-log.md` only. No source file touched.
- **Verify:** **n/a, deliberately not run** — one markdown file on a branch that is `487bd4d` plus
  docs commits. Green by construction.
- **Outcome:** **stopped — loop halted, no wake-up scheduled.** Andrew being mid-flight on #93 is a
  reason to stay out of the way, not a reason to ship: a twelfth head while he is actively working
  the queue would add review burden at exactly the wrong moment. The loop is still not dry — the
  08-29 entry's three zero-conflict candidates (`themes.ts` cycle-wrap, `SleepTimer`'s
  `formatRemaining`/`endsAt`, the `MediaNav.marking` re-entrancy guard) remain unstarted.
- **Next candidate:** unchanged — **none until `main` moves.** When it does: re-inventory the open
  PRs from scratch (the list above will be stale; do not reuse it as an avoid-list), then resume
  from the 08-29 candidates starting with `themes.ts`.
- **For Andrew:** #93 is green and self-contained — landing it first is a good call. Two asks after
  that: (1) re-gate **#85** and then **#88** against the new e2e job before merging them, since they
  are the only queued PRs with real render surface; (2) the loop still cannot verify `main` directly
  — allowing either the gh-token HTTPS `ls-remote`/`fetch` or the `gh api` commits read would fix
  that, and allowing `gh pr diff` would let it read queued changes instead of inferring from paths.

## 2026-09-04 — STOP (fifth night). Condition re-checked in three calls; nothing moved, analysis not re-derived.
- **Vision tie:** `VISION.md` line 37 — *"If nothing is safely shippable tonight, log that and stop
  rather than inventing scope."* The 09-03 next-candidate is a direct instruction to this iteration:
  *"none until `main` moves… If it is still `487bd4d`, stop again immediately and do not re-derive
  this analysis."* Honoured — nothing re-derived, no candidate re-ranked, no landing order restated.
- **Branch / PR:** no new branch. Appended to `loop/night-20260830` / **#95**, as on 08-31, 09-01
  and 09-03. The queue stays at eleven heads.
- **`main` still `487bd4d`, now fifteen days cold — tenth night running.** Verified the same
  indirect way as 09-03, because the direct paths are still closed: `git ls-remote` over the
  gh-token HTTPS helper and `gh api …/commits/main` were **both permission-gated again** this
  session. Substitute that did work: `gh pr list --state merged` → newest merge is still **#83 →
  `487bd4d` (2026-08-20T21:53Z)**, with #84 (2026-08-20T11:25Z) behind it. Nothing merged since.
  Same caveat as last night, restated because it has not been fixed: this proves *no PR merged*,
  not that nobody pushed to `main` directly.
- **Queue:** complete open set, `--limit 200`, count first → **11** (#85–#95), unchanged for the
  fourth consecutive check. All **11/11** still `MERGEABLE` / `mergeStateStatus=CLEAN`. Fifteen days
  and still zero rebase debt.
- **One new datum, and it is the only thing this entry adds.** I pulled `updatedAt` alongside
  mergeability this time. Every open PR's `updatedAt` is **still its original creation night**
  (#85 08-21, #86 08-22, … #94 08-30) — the sole exception is #95, whose 09-04 timestamp is this
  loop's own night-log commits. So there has been **no review activity of any kind on any of the
  eleven**: no merge, no comment, no review, no push. Previous nights inferred the review-throughput
  bottleneck from `main` not moving; this measures it directly on the PRs themselves.
- **Landing order: not re-derived.** It is in the 08-30 entry. Read that one.
- **Changed:** `docs/night-log.md` only. No source file touched.
- **Verify:** **n/a, deliberately not run** — one markdown file on a branch that is `487bd4d` plus
  docs commits. Green by construction.
- **Outcome:** **stopped — loop halted, no wake-up scheduled.** The blocker is unchanged and is not
  that the loop has run dry: the 08-29 entry still holds three ready zero-conflict candidates
  (`themes.ts` cycle-wrap, `SleepTimer`'s `formatRemaining`/`endsAt`, the `MediaNav.marking`
  re-entrancy guard). Adding a twelfth PR to an eleven-deep queue that has received zero review
  events in fifteen days would add review burden, not value.
- **Next candidate:** unchanged — **none until `main` moves.** When it does: re-inventory the open
  PRs from scratch (the list above will be stale; do not reuse it as an avoid-list), then resume
  from the 08-29 candidates starting with `themes.ts`.
- **For Andrew:** the ask is the same as last night and is now the only thing gating the loop —
  review and merge from the queue (start with the 08-30 landing order). Optionally, allowing either
  the gh-token HTTPS `ls-remote`/`git fetch` or the `gh api` commits read would let the loop verify
  `main` directly instead of inferring it from the merged-PR list.

## 2026-09-03 22:55 — STOP (fourth night). One new fact: the SSH fetch path is gone, so `main` was verified a different way.
- **Vision tie:** `VISION.md` line 37 — *"If nothing is safely shippable tonight, log that and stop
  rather than inventing scope."* The 09-01 next-candidate is a conditional instruction to this
  iteration: *"none until `main` moves… If it is still `487bd4d`, stop again immediately and do not
  re-derive this analysis."* Honoured — no analysis re-derived, no candidate re-ranked.
- **Branch / PR:** no new branch. Appended to `loop/night-20260830` / **#95**, as on 08-31 and 09-01.
  The queue stays at eleven heads.
- **`main` verification changed, and this is the part worth recording.** `git fetch origin main`
  **failed**: `origin` is SSH and every FIDO2 key errored (`ssh-askpass` missing, `device not found`
  / `invalid format` for all three yubikeys) → `Permission denied (publickey)`. The local
  `refs/remotes/origin/main` therefore proves nothing — it is a cached ref, and this repo has been
  bitten by stale worktree-local `main` refs before. `ls-remote` over the gh-token HTTPS helper and
  a direct `gh api …/commits/main` were both unavailable this session (permission-gated).
  **Substitute check that did work:** `gh pr list --state merged --limit 5` → newest merge is
  **#84 (2026-08-20) and #83 → merge commit `487bd4d` (2026-08-20)**; nothing merged after.
  So `main` is still **`487bd4d`**, now **fourteen days** cold. Ninth night running.
- **Queue:** complete open set, `--limit 200`, count first → **11** (#85–#95), unchanged from 09-01.
  All **11/11** re-checked individually: `MERGEABLE` / `mergeStateStatus=CLEAN`. Fourteen days and
  still zero rebase debt — which remains the whole argument for draining the queue now.
- **Landing order: not re-derived.** It is in the 08-30 entry. Read that one.
- **Changed:** `docs/night-log.md` only. No source file touched.
- **Verify:** **n/a, deliberately not run** — one markdown file on a branch that is `487bd4d` plus
  docs commits. Green by construction.
- **Outcome:** **stopped — loop halted, no wake-up scheduled.** The blocker is unchanged and is not
  the loop running dry: the 08-29 entry still holds three ready zero-conflict candidates
  (`themes.ts` cycle-wrap, `SleepTimer`'s `formatRemaining`/`endsAt`, the `MediaNav.marking`
  re-entrancy guard). The bottleneck is **review throughput**, which only Andrew can supply.
- **Next candidate:** unchanged — **none until `main` moves.** When it does: re-inventory the open
  PRs from scratch (the list above will be stale; do not reuse it as an avoid-list), then resume
  from the 08-29 candidates starting with `themes.ts`.
- **For Andrew, one operational note:** if you want the loop to keep verifying `main` unattended,
  either allow the gh-token HTTPS `ls-remote`/`git fetch` or the `gh api` commits read. Tonight the
  merged-PR list was a sound substitute, but it is indirect — it proves no PR merged, not that no
  one pushed to `main` directly.

## 2026-09-01 — STOP (third night). Condition re-checked in four calls; analysis deliberately not re-derived.
- **Vision tie:** `VISION.md` line 37 — *"If nothing is safely shippable tonight, log that and stop
  rather than inventing scope."* The 08-31 entry's next-candidate is a direct instruction to this
  iteration: *"none until `main` moves… If it is still `487bd4d`, stop again immediately and do not
  re-derive this analysis."* Honoured literally — this entry is four checks and a pointer, nothing more.
- **Branch / PR:** no new branch. Appended to `loop/night-20260830` / **#95**, same as 08-31, so the
  queue stays at eleven heads.
- **The four checks:**
  1. `git rev-parse origin/main` → **`487bd4d`**. Unmoved; **twelve days cold** (merge of #83,
     2026-08-20). Seventh night running.
  2. Complete open set: `gh pr list --state open --limit 200 --json number --jq 'length'` → **11**.
     Unchanged from 08-31 — same eleven (#85–#95). No new PRs, none closed, none merged.
  3. All **11/11** still `MERGEABLE` / `mergeStateStatus=CLEAN`, checked for every one, not a sample.
     Twelve days and nothing has rotted. Still the argument for draining now.
  4. `VISION.md` last modified **2026-07-12** (`02ba782`). The 08-30 entry's offer — *"if Andrew
     would rather the loop write nothing at all while the queue is deep, say so in `VISION.md`"* —
     went unanswered, so the standing contract holds: log the stop, don't invent scope.
- **Landing order: not re-derived.** It is in the 08-30 entry and nothing has changed. Read that one.
- **Changed:** `docs/night-log.md` only. No source file touched anywhere in the tree.
- **Verify:** all **n/a, deliberately not run** — one markdown file on a branch that is `487bd4d`
  plus docs commits. Green by construction.
- **Outcome:** **stopped — loop halted, no wake-up scheduled.** Three consecutive nights is not the
  loop running dry: the 08-29 entry still has three ready zero-conflict candidates sitting unstarted
  (`themes.ts` cycle-wrap, `SleepTimer`'s `formatRemaining`/`endsAt`, the `MediaNav.marking`
  re-entrancy guard). The bottleneck is **review throughput**, which only Andrew can supply. A
  twelfth head would only add rebase debt to eleven PRs that are currently all clean.
- **Next candidate:** unchanged — **none until `main` moves.** When it does: re-inventory the open
  PRs from scratch (the list above will be stale; do not reuse it as an avoid-list), then resume
  from the 08-29 candidates starting with `themes.ts`.

## 2026-08-31 — STOP again: the 08-30 stop condition still holds. No twelfth PR.
- **Vision tie:** `VISION.md` line 37 — *"If nothing is safely shippable tonight, log that and stop
  rather than inventing scope."* Plus the 08-30 entry's own next-candidate, which is an explicit
  conditional instruction to this iteration: *"none until `main` moves… If it is still `487bd4d`,
  stop again immediately and do not re-derive this analysis."*
- **Branch / PR:** deliberately **no new branch**. This entry is appended to the existing
  `loop/night-20260830` / **#95** so the queue does not get a twelfth head. The date mismatch
  between branch name and entry is intentional and is the point.
- **Stop condition re-checked, and it holds:**
  - `main` is still **`487bd4d`** (merge of #83, 2026-08-20) — **eleven days cold**, sixth night
    running. Corroborated across the complete open set: all 11 PRs report `baseRefOid=487bd4d`.
    (`gh api repos/.../commits/main` needed interactive approval in this sandbox, so the base-OID
    agreement across every open PR is the corroboration, not a single lookup.)
  - `VISION.md` is unchanged — the 08-30 entry offered "if Andrew would rather the loop write
    nothing at all while the queue is deep, say so in `VISION.md`." Nothing was said, so the
    standing contract still applies: log the stop, don't invent scope.
- **Open-PR inventory: 11 open, total** — the complete set, not a subset.
  `gh pr list --state open --limit 200 --json number --jq 'length'` → `11`.
  **#85** `fix/review-20260821` · **#86** `loop/night-20260821` · **#87** `loop/night-20260822` ·
  **#88** `loop/night-20260823` · **#89** `loop/night-20260824` · **#90** `loop/night-20260826` ·
  **#91** `loop/night-20260827` · **#92** `loop/night-20260828` · **#93**
  `feat/e2e-screenshot-harness` · **#94** `loop/night-20260829` · **#95** `loop/night-20260830`.
  The only change since 08-30 is #95 — which is last night's own log-only stop entry, not new work.
- **Still 11/11 `MERGEABLE` / `mergeStateStatus=CLEAN`** (checked for every one, not a sample).
  Nothing has rotted in the extra day. That remains the argument for draining now.
- **The landing order is not re-derived here — it is in the 08-30 entry directly below, and nothing
  about it has changed.** Read that one. Summary pointer only: Tier 1 (#92, #94, #91, #90, #86, #93)
  collide with nothing but the mechanical `docs/night-log.md` prepend; Tier 2 is the
  `media_server.rs` cluster (#85 → #89 → #88 → #87), to be rebased and re-gated one at a time.
- **Changed:** `docs/night-log.md` only. No source file touched anywhere in the tree.
- **Verify:** `bun run check` / `bun run build` / `bun run test` / `cargo check` / `cargo clippy` —
  **all n/a, deliberately not run.** One markdown file changed on a branch that is `487bd4d` plus
  docs commits; the suite would prove nothing about it. Green by construction.
- **Outcome:** **stopped, by design — loop halted, no wake-up scheduled.** Two nights in a row is
  the signal, not a glitch: the loop is not out of ideas (the 08-29 entry left three ready
  zero-conflict candidates — `themes.ts` cycle-wrap, `SleepTimer`'s `formatRemaining`/`endsAt`, the
  `MediaNav.marking` re-entrancy guard). It is out of **review throughput**, which only Andrew can
  supply. Restarting the loop before `main` moves would only add rebase debt.
- **Next candidate:** unchanged — **none until `main` moves.** When it does: re-inventory the open
  PRs from scratch (do not reuse the list above as an avoid-list; it will be stale), then resume
  from the 08-29 candidates starting with `themes.ts`.

## 2026-08-30 — STOP: the queue is the bottleneck. No code tonight; landing-order analysis instead.
- **Vision tie:** `VISION.md` line 37 — *"If nothing is safely shippable tonight, log that and stop
  rather than inventing scope."* Taking that literally, and taking the 08-29 entry's own
  next-candidate literally: *"review the queue before adding to it. If it's still nine deep and main
  is still `487bd4d`, the honest move is to stop rather than ship a tenth."* It is now **ten** deep.
- **Branch / PR:** `loop/night-20260830` — log-only, no source change.
- **Open-PR inventory: 10 open, total.** `gh pr list --state open --limit 200 --json number --jq
  'length'` → `10`. The complete set, not a subset: **#85** `fix/review-20260821`, **#86**
  `loop/night-20260821`, **#87** `loop/night-20260822`, **#88** `loop/night-20260823`, **#89**
  `loop/night-20260824`, **#90** `loop/night-20260826`, **#91** `loop/night-20260827`, **#92**
  `loop/night-20260828`, **#93** `feat/e2e-screenshot-harness`, **#94** `loop/night-20260829`.
- **Main has not moved: `487bd4d`, now ten days cold** (merge of #83, 2026-08-20). Corroborated two
  ways — `gh api repos/atiner117/omnideck/commits/main` returns `487bd4d`, and all ten PRs report
  `baseRefOid=487bd4d`. Fifth night running.

### The good news, and it is better than it looks
**All ten PRs are `MERGEABLE` / `mergeStateStatus=CLEAN` against main right now** (checked for all
10/10, not a sample). Nothing has rotted. Total queue: **+3396 / −83 across 63 file-paths.**

**But that CLEAN status is a snapshot that dies on the first merge.** Nine of the ten touch
`docs/night-log.md`, all prepending at the same anchor. The moment any one of them lands, the other
eight flip to CONFLICTING — on the log file alone, not on code. **That is expected and benign.**
Resolution rule, every time: keep all entries, newest first. Do not let eight scary-looking red
"conflicting" badges suggest the queue has gone bad; it hasn't.

### Complete overlap map (all 63 paths across all 10 PRs)
Exactly **three** paths are shared by more than one PR. Every other file in the queue is touched by
exactly one PR:

| Path | PRs | Nature |
|---|---|---|
| `docs/night-log.md` | 86, 87, 88, 89, 90, 91, 92, 93, 94 (9) | Mechanical. Same anchor. Keep all, newest first. |
| `src-tauri/src/media_server.rs` | 85, 87, 88, 89 (4) | **The only real code cluster.** |
| `.gitignore` | 85, 93 (2) | Two unrelated additions; almost certainly both-keep. |

### Recommended landing order
**Tier 1 — six PRs that collide with nothing but the log file.** Land these in any order; each is a
single-concern change whose only conflict is the mechanical night-log prepend:
- **#92** `npActions.test.ts` and **#94** `settings-defs.test.ts` — test-only, zero production code.
  Cheapest possible merges; they cannot break the app.
- **#91** `asset.rs` (TOCTOU fix) · **#90** `http.rs` + `icons.rs` (fail-open fixes) — security
  hardening, one file-set each, nothing else in the queue touches those files.
- **#86** `CHANGELOG.md` (0.2.0 top-off) — pairs with #85 for the release; see below.
- **#93** the Playwright harness — 16 files but all new (`e2e/`, `static/fonts/`) plus
  `GridView`/`ListView`/`+layout.svelte`/`fonts.css`, which **no other open PR touches**. Only
  overlap is `.gitignore` with #85.

**Tier 2 — the `media_server.rs` cluster (85, 87, 88, 89).** Land deliberately, rebasing each on the
previous and re-running the Rust gate between. Suggested order **#85 → #89 → #88 → #87**, on these
grounds: #85 is the oldest (2026-08-21), the largest (23 files), and is the *security* PR closing
P0/P1 review findings against code already on main — fixes outrank features, and landing it first
means the three feature PRs rebase onto the fixed baseline rather than the other way round. Order
among #89/#88/#87 is low-stakes.

**Honest limit on that recommendation:** I could establish that those four touch the same *file*,
but **not whether they touch the same hunks** — `gh pr diff` and `gh api .../pulls/N/files` both
require interactive approval in this sandbox, and `git fetch` of the PR branches does too (SSH
`origin` wants a yubikey touch; the HTTPS-helper fetch needs approval). So Tier 2's ordering is
argued from priority and size, **not** from a verified hunk-overlap analysis. A human with the
branches fetched can settle it in one `git merge-tree` run.

### Two other things worth knowing before merging
- **#85 carries the version-sync files** — `src-tauri/Cargo.toml`, `Cargo.lock`,
  `packaging/PKGBUILD`, `packaging/.SRCINFO`. CI enforces the five-way version sync, so #85 must
  land internally self-consistent. Nothing else in the queue touches `Cargo.lock`, so there is no
  cross-PR version hazard — but #85 + #86 (CHANGELOG) are effectively **the 0.2.0 release pair** and
  are the natural thing to land together.
- **Two PRs regenerate ts-rs bindings** — #88 (`bindings/MediaItem.ts`) and #85
  (`bindings/AudioSink.ts`). Different files, so no conflict, but both need
  `cargo test --release export_bindings` re-run after any rebase or CI will fail on drift.

- **Changed:** `docs/night-log.md` only. No source file touched anywhere in the tree.
- **Verify:** `bun run check` / `bun run build` / `bun run test` / `cargo check` / `cargo clippy`
  — **all n/a, and deliberately not run.** This increment changes one markdown file; running the
  suite would prove nothing about it. The branch is `487bd4d` plus a single docs commit, so it is
  green by construction.
- **Outcome:** **stopped, by design.** No eleventh code PR. The loop is not blocked on ideas — the
  08-29 entry left three good zero-conflict candidates ready to go (`themes.ts` cycle-wrap,
  `SleepTimer`'s `formatRemaining`/`endsAt`, the `MediaNav.marking` re-entrancy guard). It is
  blocked on **review throughput**, which only Andrew can supply. Shipping more drafts against a
  ten-day-cold `main` adds rebase debt to a queue that is currently, unusually, 100% clean — the
  best moment to drain it is now, before anything rots.
- **Stated tension, not hidden:** this entry is itself an eleventh PR, which is mildly ironic. It is
  log-only (one file, no code), it is the one PR in the queue that costs a minute rather than a
  review, and it carries the landing order — so it should be read *first* and merged or simply
  closed after reading. If Andrew would rather the loop write nothing at all while the queue is
  deep, say so in `VISION.md` and the next iteration will honour it.
- **Next candidate:** **none until `main` moves.** The next iteration should re-check
  `gh api repos/atiner117/omnideck/commits/main`. If it is still `487bd4d`, stop again immediately
  and do not re-derive this analysis — it is above, and nothing about it changes until the queue
  drains. If main *has* moved, resume from the 08-29 list (`themes.ts` first), and rebuild the
  avoid-list from the then-current open PRs rather than reusing the stale one.

## 2026-08-20 — Review gate on #83 (mark-watched): 5-angle review, corroborated fixes on-branch
- **Vision tie:** same gate #81 got — unreviewed autonomous work doesn't merge unreviewed.
  Five parallel review agents (line-by-line, Rust transport, frontend races, input gating,
  IPC contract/tests) over `2471f6c..` (#83 rebased onto post-#84 main via merge).
- **Branch / PR:** `loop/night-20260819` — https://github.com/atiner117/omnideck/pull/83
- **Fixed (corroborated):**
  1. *Stale resume state after a toggle* (all 5 agents): `toggleWatched` flipped only `played`;
     the row's `sub` ("43% · 18 min left") and `startSecs` kept describing a resume point the
     server had just cleared — Enter then resumed a title the user had declared done, and the
     same id cached in other stack frames (root rail vs drilled-in list) never updated at all.
     Now: `syncToggled()` updates every non-browse copy of the id across the stack on server
     confirm — `played`, `startSecs = undefined`, `sub → subPlain`. The previous session's
     uncommitted `rowSubPlain()` was finished (truthiness matches `rowSub`, so `SeriesName: ""`
     can't blank the line), wired via a `subPlain` baked at row construction, and tested.
  2. *Mutating verbs through a redirect-following client* (2 agents): a 301/302/303 (http→https
     proxy, slash normalisation) downgrades POST/DELETE to GET — browse keeps working while
     every toggle silently no-ops. `send()` now fails loudly when a non-GET lands anywhere but
     the requested URL.
  3. *2xx trusted blindly*: `set_played` now reads the `UserItemDataDto` answer and errors when
     the server acknowledges but reports the opposite `Played` (jellyfin#8168 deployments);
     empty/non-JSON bodies still pass (Emby-lineage 204s).
  4. *`user` path segment unvalidated* (2 agents): the id from config/shim-cred/`/Users/Me` is
     interpolated like an item id but was never gated; `user()` now applies `valid_id` to both
     sources. New no-IO test pins both `set_played` guards.
  5. Error toast read as success (`⚠ Mark watched` → `⚠ Couldn't mark watched`); ✓ span now
     always rendered so the sub-label doesn't jump ~86px when it toggles (`.cstate` reserves
     72px); Help sheet documents the W/□ overload; night-log local/remote `main` wording fixed.
- **Reviewed and left as follow-ups (logged, not fixed):** shared-retry idempotency is prose,
  not a mechanism (make retry opt-in per call site before adding `?datePlayed=`); a toggle can
  hang ~30 s on a dead server (2×15 s budget) while `marking` pins the row; `BROWSE_KINDS` is an
  allowlist so an *unlisted* container kind would become mark-watchable (latent — libraries are
  filtered today); the playable-only gate exists solely in TS, a future Rust caller could hand
  `PlayedItems` a series id (doc moved caveat aside, consider a server-side Type check); hint
  line advertises W during an in-flight drill-down when the action refuses; no `cli.rs`
  subcommand can exercise `set_played` headlessly (an `omnideck watched <id> [--un]` would make
  the transport fixes observable); `X-Emby-Token` survives cross-host public redirects
  (pre-existing, whole client). Clean bills: PLAYABLE gate IS action-level, no West/W binding
  conflicts, nothing on the rAF path, IPC contract + bindings exact, optimistic-flip mechanics
  and in-flight guard correct.
- **Verify:** bun run check · build · test (47) · clippy `-D warnings` · cargo test — see PR.
- **Outcome:** fixes pushed to the PR branch; merge stays Andrew's call.
- **Next candidate:** changelog top-off for 0.2.0 (docs-only), then the couch pass.

## 2026-08-20 — Wave 4 pick 3 (THE LAST ONE): artwork disk cache (adapts #43)
- **Vision tie:** Andrew-directed iteration. Round-2 backlog **Lane C** — the final item of
  the entire post-rewrite draft backlog; #81's log named it next. Cold boots re-fetched
  every poster over the network; this makes remote artwork a disk-first resource.
- **Branch / PR:** `pick/artcache` — https://github.com/atiner117/omnideck/pull/84
  (supersedes #43 — close it when this lands).
- **Changed:** #43's `550a8a0` + `98dc8cd` (streaming-cap hardening) onto main `c2ba3b2`.
  New self-contained `artwork_cache.rs`: FNV-1a-keyed files + .meta sidecars, 24 h
  freshness then ETag/If-Modified-Since revalidation (network errors serve stale), atomic
  writes, TRUE-LRU 200 MB budget (`[media_server] art_cache_mb`, additive), legacy
  id-keyed cache dir removed once. Wiring: poster() delegates to the cache;
  `prefetch_posters` warms rail art post-sections (4 bounded workers); new `get_artwork`
  command gated by `url_within_base` (token-authed fetch must not become an open proxy);
  `omnideck doctor` gains an `[art cache]` section + `--clear-art-cache`.
  **THREE repairs** for main-side drift the draft predates:
  (1) the draft's base carried a `crate::fsutil` module that never landed — atomic writes
  now route through main's identical (and fsyncing) `config::write_atomic`, one
  implementation instead of two;
  (2) main's `valid_id` injection gate (post-draft) kept in `poster()` even though the
  fetch moved into the cache;
  (3) main already HAS `doctor` (#54) — the `--clear-art-cache` flag merged into the
  existing Doctor variant instead of the draft's duplicate; the draft's `[art cache]`
  section edits applied cleanly onto main's doctor body (same lineage).
  Also NOT taken: the draft's 2-arg `mediaPlay` (predates #81's startSecs; would have
  regressed the resume plumbing and the launch-key return type).
- **Verify:** bun run check (pass, 369 files, 0 errors) · bun run build (pass) · bun run
  test (44 pass) · cargo clippy --release --all-targets -D warnings (pass) · cargo test
  --release (**107 pass**, 1 ignored — was 98: +9 incl. the hermetic loopback HTTP
  miss→hit→304 test) · bindings regenerated, drift clean · no new deps · whole-tree
  conflict-marker sweep clean (the #82 lesson).
- **Outcome:** shipped to draft PR (supersedes #43). needs-hardware: none strictly — the
  loopback test covers the protocol — but the first couch boot after merge should feel
  the pop-in disappear on the second launch.
- **Next candidate:** the backlog is DRAINED. Next per the plan: the 0.2.0 changelog
  top-off (~27 merged PRs since 07-27, docs-only), then #81's five review follow-ups
  (cheapest first: parse_backup's remaining normalize gap), then the VISION rewrite
  unblocks autonomous scope-picking again. Still loose: #39's `0dabfea` (L2/R2,
  needs-hardware — land before couch night or not at all for 0.2.0).

## 2026-08-19 23:05 — mark watched/unwatched from the couch (West / W)
- **Vision tie:** VISION priority 1 (media-server track, small shippable slices) — and it is
  literally the **"next candidate" #81's log left**: the natural pair to resume. #81 made Continue
  Watching resume; without this you still can't tell the server you're *done* with a title, so the
  rail only ever grows.
- **Branch / PR:** `loop/night-20260819` — https://github.com/atiner117/omnideck/pull/83
- **Open-PR inventory:** **1 open, total, read before choosing** — #43
  (`loop/fable-artcache-20260712-212133`, artwork disk cache, Lane C). That is the *whole* list, not
  a subset: `gh pr list --state open --limit 200 --json number --jq 'length'` → `1`. The backlog
  that used to be 8+ drafts is drained — #78/#79/#80/#81/#82 all merged since the last iteration, so
  remote `main` had moved to `c2ba3b2` (the local ref stayed stale — see the note below). Searched the inventory for this increment's keywords before
  building: nothing open touches watched/played state, so this is not a duplicate of #43 or of
  anything else.
- **Changed:** `send_get` → `send(reqwest::Method, url)` (shared one-retry transport);
  `set_played()` on Jellyfin `PlayedItems` (POST sets / DELETE clears); `played_request()` split out
  as a pure fn so the verb+path choice is unit-testable without a server (the `mpv_start_flag`
  pattern); `media_set_played` command + `mediaSetPlayed` IPC; `MediaNav.toggleWatched()` with an
  optimistic flip + revert-on-failure + an in-flight id guard; West/`W` added to the #77 overlay
  roster's media entry (no hold-repeat — it's a toggle); hint line flips "watched"/"unwatch".
- **THE JUDGEMENT CALL:** gated to **playable rows only**. Jellyfin's `PlayedItems` accepts a
  series/season, and it was tempting to allow it — but marking a show watched clears **every
  episode's resume point**, and un-marking returns only the flag, *never* the positions. So the
  "undo" is lossy, and one unconfirmed button press would be able to destroy a 60-episode series'
  progress. The hint line therefore doesn't advertise the binding on a 📁 row either. The same
  lossiness applies in miniature to a single item (that IS what "I'm done" means) and is documented
  on `set_played`, `mediaSetPlayed`, and `toggleWatched`.
- **The other thing a reviewer should look at:** the retry transport is now shared with a *mutating*
  verb. That's only safe because both `PlayedItems` verbs set an **absolute** state rather than
  incrementing a counter, so a replay lands on the same result. Documented on `send()` so a future
  non-idempotent verb doesn't silently inherit the blind retry.
- **Verify:** bun run check (pass, 369 files, 0 errors) · bun run build (pass) · bun run test
  (44 pass, 6 files) · cargo clippy --release --all-targets -D warnings (pass) · cargo test
  --release (**99 pass**, 1 ignored — +1 new: `played_request_flips_only_the_verb`). Bindings
  untouched (no `#[ts(export)]` struct changed; `git status --porcelain src/lib/bindings` clean).
  No new deps.
  needs-hardware: not exercised against a live Jellyfin server. The endpoint shape is Jellyfin's
  documented one, but the round trip — and that the ✓ *survives a reopen*, i.e. the server really
  persisted it — wants one couch test.
- **Note for the next agent:** `origin` is SSH and `git fetch origin` **hangs on the yubikey prompt**
  for two minutes before failing. Fetch anonymously instead — the repo is public:
  `git fetch https://github.com/atiner117/omnideck.git main`. Local `main` is stale at `dfdfa32`
  (PR #48) and the `gh/*` remote refs are leftovers from a remote that no longer exists; don't
  trust either as "current main".
- **Outcome:** shipped to draft PR #83.
- **Next candidate:** **#43 artwork disk cache** is now the only open PR and the last untouched
  Wave 4 lane — re-pick it against current main the way #81 re-picked #42 (expect the same "authored
  against a far older main" audit). After that, the **0.2.0 release** is the highest-value non-feature
  work: the version bump is already done, so it needs a CHANGELOG top-off (#83 + the Wave 4 merges
  aren't in it), a couch pass over the needs-hardware items (#77 input layer, #81 resume, #83
  watched), and a tag. Still loose: #39's `0dabfea` (L2/R2 synthesis), needs-hardware.

## 2026-08-18 23:03 — Wave 4 pick 2: Continue Watching actually resumes
- **Vision tie:** VISION priority 2 / round-2 backlog **Lane B** (resume + watched state) — the
  "#80 log named #42 as next" candidate, taken as planned.
- **Branch / PR:** `pick/resume` — https://github.com/atiner117/omnideck/pull/81
  (adapts #42; supersedes it — close #42 when this lands).
- **Open-PR inventory:** **8 open, all read before choosing** — #80, #79, #78 (the last three
  nights' picks, all off `f1e04c7`), and #46, #44, #43, #42, #41 (the original `fable-*` drafts).
  No subset. #42 is the only PR touching resume/watched state; this adapts it rather than
  duplicating it.
- **THE FINDING that reshaped the increment:** main *looks* like it already has Continue
  Watching — `medianav.svelte.ts:51` builds a `"Continue watching"` group out of
  `sections.resume`. It is a **facade**: `activate()` calls `onplay(r.id, r.name)` with no
  position, `media_play` had no position parameter, and `MediaItem` never carried one. **Picking
  a half-watched film off that row restarts it from 0:00 on main today.** So the valuable
  increment was not #42's new parallel rail — it was making the rail main already ships do the
  thing its label promises.
- **Changed:** `MediaItem` += `position_secs` (100 ns ticks → whole secs, **0 ticks collapses to
  None** so a never-started item can't ask mpv to seek) + `played`; `media_play(start_secs)` →
  mpv `--start=`, pushed LATE in the argv so it beats `mpv_args`/the profile include (mpv: last
  occurrence rules — per-launch intent should outrank a config default); `MediaRow` += `startSecs`
  /`played`, threaded through `onplay` → `api.mediaPlay`; resumable rows read **"N min left"**
  instead of total runtime; `MediaModal` shows a ✓ for watched items (`role="img"` + `aria-label`,
  so it joins the row's accessible name instead of being a bare decorative glyph). Browse rows get
  no `startSecs` — a series/season has no position of its own.
  `mpv_start_flag` rejects NaN/inf/sub-second **and negative** — a negative `--start` is
  *end-relative* in mpv, so a stray −30 would have seeked 30 s from the END of the film.
- **FIVE things deliberately NOT taken from #42** (it was authored against a far older main):
  (1) `valid_id`, the bounded transport retry, the non-poisoning `user()` cache, and the
  `browse`/`poster` id gates — **all four landed on main independently since**, so re-applying
  them is a no-op or a conflict; (2) its `sections()` rewrite onto shared path builders — #42's
  `sections()` is **serial** with `unwrap_or_default()`, main's is concurrent `tokio::join!` with
  per-section warn logging, so taking it would have **regressed** main; (3)
  `ContinueWatchingRow.svelte` — a second, self-fetching rail duplicating the group `MediaModal`
  already renders, which #42 never mounts, so it would land as dead code; (4) `tokio = { features
  = ["time"] }` — needed only for #42's 250 ms retry sleep, and main's retry doesn't sleep, so
  **no new dep**; (5) `get_recently_added` — `media_sections` already returns `latest` and nothing
  consumed the new command.
- **Verify:** bun run check (pass, 359 files, 0 errors) · bun run build (pass) · bun run test
  (21 pass) · cargo clippy --release --all-targets -D warnings (pass) · cargo test --release
  (**94 pass**, 1 ignored — was 91: +3). Bindings regenerated; `diff -rq` vs the generated set
  clean. No new deps.
  needs-hardware: not verified against a live Jellyfin server. The `UserData` shape is the same
  one `played_pct` already reads in-tree, but *mpv landing on the right frame* wants one couch
  test — resume is the feature you notice instantly when it's off by a scene.
- **Outcome:** shipped to draft PR #81.
- **COLLISION NOTE for Andrew:** #78/#79/#80/#81 are all off `f1e04c7`. #81 is the *least*
  entangled of the four — it touches no `settings-defs.ts` row and no config field, so its only
  expected conflict is the `docs/night-log.md` prepend point it shares with #79/#80. Merge order
  doesn't matter for it.
- **Review pass (2026-08-20, Andrew-requested, /code-review high):** the review was NOT
  clean — the headline label was dead where it mattered most. Fixed on this branch:
  (1) **"N min left" never rendered for episodes** — `sub = series ? pct+series : pct+mins`
  discarded the label for anything with a SeriesName, and /Items/Resume is mostly episodes;
  the label logic is now a pure, tested module (`mediarow.ts`) and episodes read
  "43% · Some Show · 18 min left".
  (2) **left<=0 fell back to the FULL runtime** — double flooring made a nearly-finished
  film read "90 min" like an untouched one; now "<1 min left".
  (3) **the label leaked onto browse rows** — `startSecs` was masked for containers but
  `sub` was computed from the unmasked value, so a folder could advertise "N min left" it
  would never honor; gating now happens once, in mediarow.ts.
  (4) **mpv watch-later beat --start** — a stale save-position-on-quit entry is applied at
  file load, AFTER argv, so local state silently overrode the server position;
  `--no-resume-playback` now rides with `--start`.
  Plus: `runtime_mins` routed through ticks_to_secs (the raw 600_000_000 was the same
  constant duplicated), UserData indexed once in items_of, the ✓ moved onto the shared
  `.cstate on` vocabulary (it was accent-colored + unaligned vs every other modal ✓),
  mediaPlay's orphaned exit-key JSDoc re-attached, `startSecs ?? null` per the file's own
  optional-arg convention, BROWSE_KINDS hoisted to a module Set, +9 mediarow unit tests.
- **Review follow-ups (logged, NOT fixed here):** (a) `start_secs` is `Option<f64>` but its
  only producer is whole non-negative seconds — an `Option<u64>` signature deletes the
  NaN/negative defence class; (b) no upper bound vs runtime — a stale position past EOF
  makes mpv exit instantly while a Now Playing card and "(resuming)" toast still appear;
  (c) `played ?? false` collapses the server's tri-state at the row boundary; (d) a
  ✓-watched row that also carries a residual position resumes into the credits with no
  "play from start" affordance — pair that with the mark-watched follow-up's context
  action; (e) the "(resuming)" toast predicts mpv behavior the jellyfinmediaplayer path
  deliberately ignores.
- **Next candidate:** the natural pair to this one — **mark-watched from the couch**: `set_played`
  on `JellyfinServer` (POST/DELETE `/Users/{u}/PlayedItems/{id}`, both idempotent) plus
  `mark_watched`/`mark_unwatched`, wired to a context action on a media row (West/Y, say) through
  the #77 overlay roster. That needs `send_get` generalised to take a `reqwest::Method` — a
  ~5-line change preserving main's existing retry semantics; #42 has a `request()` version of this
  to crib the shape from, but its retry sleeps and main's doesn't. After that, **#43 artwork disk
  cache** is the remaining untouched Wave 4 lane, and #39's `0dabfea` (L2/R2 synthesis) is still
  loose and needs-hardware.

## 2026-08-17 23:20 — Wave 4 pick 1: theme system (the pick's tokens didn't exist on main)
- **Vision tie:** VISION priority 2 — a high bang-for-effort roadmap item plugging into the
  table-driven Settings surface; round-2 backlog **Lane A**. First item of Wave 4, which the
  #79 log named as next ("take #41 first: #43 and #44's tokens both assume it").
- **Branch / PR:** `pick/themes` — https://github.com/atiner117/omnideck/pull/80
  (supersedes #41 — close it when this lands).
- **Open-PR inventory:** 7 open, ALL of them read before choosing: #79, #78, #46, #44, #43,
  #42, #41. #79/#78 are the previous two nights (supersede #44/#46); #43/#42/#41 are the
  untouched Wave 4 lanes. No PR existed for this work other than #41 itself, which this
  supersedes rather than duplicates.
- **Changed:** #41's `532a3cf`+`e8e7927`+`4a99ac8` onto main `f1e04c7`, but **NOT verbatim —
  the pick's entire token vocabulary is absent from main.** #41 was authored against a
  `src/lib/tokens.css` from its own `d8f81de`, which never merged; main got tokens via
  `fa6fe2c`, *inside* `+page.svelte`'s `:global(:root)`, under different names. The pick
  themes `--bg`/`--surface-2`/`--surface-3`/`--text`/`--text-bright` (none exist here) and
  leaves main's `--surface-deep`/`--surface-card`/`--text-soft`/`--text-label`/`--text-dim`/
  `--danger` unthemed — verbatim, OLED's `--bg:#000` would be read by nothing and Light would
  keep dark-navy cards. Kept the pick's ARCHITECTURE (registry, `applyTheme`, `data-theme`
  stamp, settings row, config normalize); rewrote all six palettes + the Light shim against
  main's real 9 tokens. THREE further repairs: (1) the Light shim missed `Modal.svelte`'s
  hardcoded `.prefs { background:#121826 }` — the panel the Theme picker itself lives in, so
  Light would have looked like a no-op from Settings; (2) the shim redundantly re-set
  properties main has since tokenized (`.clock`, `.badge` color, `.xsheadlbl`, `.infogrid dt`,
  `.numedit` bg) — trimmed to only what is still hardcoded; (3) the scanline z-index comment
  cited overscan z50 from unmerged #46 — re-derived against main's real stack (chrome 2 → 5 →
  Modal 10/11 → NP 12 → Wizard 20 → deck 40/41 → transport 44/45 → boot-errors 60 → toast 70
  → screensaver 200/201); z5 still correct. `pageBg` resolves to `var(--surface-deep)`, not
  the pick's `var(--bg)`. The #79 `parse_backup` trap does NOT apply: `theme` lives inside
  `Settings`, whose `normalize()` both config paths already call. OmniDark is default and
  renders byte-identical.
- **Verify:** bun run check (pass, 360 files, 0 errors) · bun run build (pass) · bun run test
  (21 pass) · cargo clippy --release --all-targets -D warnings (pass) · cargo test --release
  (93 pass, 1 ignored — was 91: +2 theme tests). Bindings regenerated; `diff -rq` vs the
  generated set clean. No new deps.
  **The new tests earned their keep immediately** — `theme_ids_match_frontend` failed twice
  during the build on a doc comment that mentioned `data-theme`, hence the comment-stripping
  helper; `every_theme_overrides_every_base_token` is the guard that would have caught the
  verbatim-pick bug unaided.
  needs-hardware: this is a purely visual feature and NO palette has been seen on a panel.
  Light most of all (the one theme carrying a shim), then OLED true-black on the TV and CRT
  scanline density at 10 feet.
- **Outcome:** shipped to draft PR #80.
- **COLLISION NOTE for Andrew:** #78, #79 and #80 are all off `f1e04c7` and all add a
  `settings-defs.ts` row + a config field; #79 and #80 also both prepend a night-log entry at
  the same spot. Expect a trivial conflict in `docs/night-log.md` and small refreshes in
  `settings-defs.ts` / `config.rs` for whichever merges later. Nothing else overlaps —
  themes touch only the token layer.
- **Next candidate:** Wave 4 continues with **#42 Continue Watching** (Jellyfin resume/watched
  + row component) then **#43 artwork disk cache**. #43's `5c0a042`-era tokens now have a real
  consumer, and both were authored pre-router — expect the same "read the whole removed region"
  discipline. Also still loose: #39's `0dabfea` (L2/R2 synthesis, needs-hardware). Worth
  flagging: `+page.svelte` and `Modal.svelte` finishing their token conversion would let the
  Light shim in themes.css be deleted outright — a clean, self-contained follow-up.

## 2026-08-16 23:10 — Wave 3 pick 7 (LAST): library view modes — rail / grid / list
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 3 step 7, the final item in the +page
  wave (frontend-split track, VISION priority 1). Closes out the wave that ran
  #26 → #30 → #32 → #28 → #46 → #44. Context: #78 (overscan, step 6) is still
  OPEN and unmerged — branched off main anyway per the #73 precedent, see the
  collision note below.
- **Branch / PR:** `pick/layouts` — https://github.com/atiner117/omnideck/pull/79
  (supersedes #44 — close it when this lands).
- **Changed:** #44's `6af24bb` + `5c0a042` onto main `f1e04c7`. New additive
  `[appearance]` config section (`layout`: rail | grid | grid-compact | list) with
  a `save_appearance` command; four new frontend files (GridView, ListView,
  LayoutPicker, layouts.ts — the last is pure 2D nav math). The page stays the
  single owner of `focus` + input routing, so every input path works in every
  mode. `settings.grid_columns` — previously dead — becomes the grid density knob.
  **FOUR repairs**, all for main-side changes the pick predates:
  (1) `focus` is a read-only `$derived` since #72's clamp refactor — the pick's
  three `focus = ` writes would not have compiled; routed through `focusRaw`.
  (2) `save_settings` is `async` + `blocking()` on main (I/O off the main thread);
  `save_appearance` follows that shape, not the pick's sync one.
  (3) main added a manual `impl Default for Config` that the container-level
  `serde(default)` fills missing fields from — caught by the compiler; without the
  `appearance` line the additive-default promise breaks for existing configs.
  (4) main added `parse_backup()`, a SECOND normalize path whose doc comment
  promises every field is sanitized like a hand-edited config. The pick only
  normalized in `load_or_create` (parse_backup did not exist yet), so a restored
  backup could smuggle `layout = "mosaic"` past the check. Added there too.
  Rail is the default and renders byte-identical to today.
- **Verify:** bun run check (pass, 364 files, 0 errors) · bun run build (pass) ·
  bun run test (21 pass) · cargo clippy --release --all-targets -D warnings (pass)
  · cargo test --release (93 pass, 1 ignored — was 91: +1 layout-normalize test,
  +1 binding export). Bindings regenerated; `diff -rq` of the generated set vs
  `src/lib/bindings` is clean. No new deps.
  needs-hardware: grid density/readability at 10 feet and the 2D nav feel on a
  real pad — the whole point of the feature is something only a TV can judge.
- **Outcome:** shipped to draft PR #79.
- **COLLISION NOTE for Andrew:** #78 and #79 are both off `f1e04c7` and both add
  a settings row + a config field. Whichever merges SECOND will want a trivial
  refresh in `settings-defs.ts`, `config.rs` (`Config` struct + both `Default`
  and `defaults()` sites) and a bindings regen. Merging #78 first is the smaller
  fixup. Nothing else overlaps — overscan touches `<main>`'s CSS var, layouts
  touches the item-render branch.
- **Review pass (2026-08-18, Andrew-requested):** a /code-review at high effort
  cleared the 2D nav math (no logic findings; the intentional behaviors — wheel
  moves by row, hold-repeat exits into the category axis, no-move/no-blip — were
  checked and confirmed as design) and produced 7 findings; the 4 cheap ones are
  fixed on this branch:
  (1) list mode kept the rail's 8-above art window, but scrollIntoView pins a
  wrapped-to focus at the BOTTOM of the viewport — rows above it rendered emoji
  fallbacks; list now uses a 32/40 window (the 4K worst case).
  (2) the Grid-columns row's `lo: 3` fought the backend's 1–12 clamp — a
  hand-edited value of 1–2 JUMPED UP to 3 on a decrease press; floor is now 1.
  (3) layouts.ts was extracted to be unit-testable but shipped untested — added
  layouts.test.ts (wrap, short-row clamp, single-row no-op, row-edge null, the
  compact-density math; 21 → 35 vitest).
  (4) grid/list render the FULL library (not the rail's window), so per-tile
  `favorites.includes()` was O(items x favorites) per render — both views now
  derive a Set.
- **Review follow-ups (logged for the loop, NOT fixed here):**
  (5) `parse_backup` still skips screensaver/launch_overrides/input normalize
  despite its doc-comment contract — PRE-EXISTING on main (#79 narrowed the gap
  by adding appearance); a small standalone config.rs pick.
  (6) the layout-mode whitelist is hand-duplicated (config.rs `matches!` vs
  layouts.ts LAYOUT_MODES) — drift means a saved mode silently reverts to rail
  on next boot; a ts-rs-exported enum would make it impossible. Same class of
  problem #80's theme tests solve by cross-checking — pick either mechanism.
  (7) ListView re-implements +page's appSource/fmtPlayed as appDetail/fmtPlayed
  and the copies already differ ("never played" vs "never") — consolidate into a
  shared helper next time either is touched.
- **Next candidate:** Wave 3 is DONE once #78 + #79 land. Next is **Wave 4**: the
  round-2 lanes — #41 themes (its `tokens.css` is what `5c0a042` tokenized
  GridView/ListView for, so it now has a consumer and lands cleanly), then #42
  Continue Watching, then #43 artwork disk cache. Take #41 first: #43 and #44's
  tokens both assume it. Also still loose: #39's `0dabfea` (L2/R2 synthesis,
  needs-hardware). Plus review follow-ups (5)–(7) above — (5) is the cheapest.

## 2026-08-11 13:23 — Wave 3 pick 6: TV overscan calibration (z-index invariant repaired)
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 3 step 6; VISION 10-foot TV target — TVs crop
  edges, every console ships this screen. Context: #77 merged + #28 closed this session;
  campaign walkthrough written to the epic artifacts (changeset-walkthroughs/campaign-61-77).
- **Branch / PR:** `pick/overscan` — https://github.com/atiner117/omnideck/pull/78
  (needs-hardware kept).
- **Changed:** `126517f` + `20d82e1` onto main `f1e04c7` + ONE repair.
  OverscanCalibration.svelte (+132), --overscan on <main> + one rule with translateZ(0)
  so fixed modals/toasts are contained; 0% is byte-identical to today; settings.overscan_pct
  clamped 0–10 (assertion added to the EXISTING normalize test — why cargo stays at 91).
  ORDERING PAYOFF: the settings row auto-merged into settings-defs.ts (#60) and the roster
  entry auto-merged into OVERLAYS (#77) — this pick was authored against both, so post-wave
  main is finally its intended base.
  THE REPAIR: `20d82e1` existed solely to raise .ovcal from z 40 (tied with the old
  .ebanner, which painted over the frame) to 50. Main replaced that banner with the durable
  boot-error panel at z 60 (6208ef8) mounted at top:5vh — right over the top edge markers —
  so 50 silently lost the commit's own invariant. Set to 65: above the panel, below the
  toasts (70) that main documents as clearing every overlay. Also: the both-sides merge
  duplicated the $lib/sfx import (pick predates #76's blip) — stale copy dropped;
  Settings.ts settled by regeneration (23 export tests), not by hand.
- **Verify:** bun run check (pass, 360 files, 0 errors) · bun run build (pass) · bun run
  test (21 pass) · cargo clippy --release -D warnings (pass) · cargo test --release
  (91 pass) · bindings clean. needs-hardware: marker visibility at the real panel edge,
  step feel on the 4K OLED, and the z-65 choice with a boot error raised mid-calibration.
- **Outcome:** shipped to draft PR #78 (supersedes #46 — close when it lands).
- **Next candidate:** #44 layouts (`6af24bb`,`5c0a042`) — LAST wave pick; also authored
  against #60's table + #77's router, so expect the same auto-merge payoff. Then Wave 4
  reworks (#41 themes — needs re-expression on main's tokens, NOT a rebase; #42 resume;
  #43 artcache) and #39's last loose commit `0dabfea` (L2/R2 synthesis, needs-hardware).

## 2026-08-11 07:35 — Wave 3 pick 5: the router rewrite (roster was missing npOpen)
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 3 step 5 — the biggest +page rewrite; the
  triage's own note says "everything later assumes it". Context: #76 merged + #32
  closed this session at Andrew's direction.
- **Branch / PR:** `pick/router` — https://github.com/atiner117/omnideck/pull/77
- **Changed:** `8b4f268` from draft #28 onto main `3ff9090`. Three parallel input
  paths (keyboard chain, pad chain, stick handling) collapse into ONE ordered OVERLAYS
  roster; each entry declares open()+key/pad/stickX/stickY; anyModal derives from the
  roster instead of a hand-synced 11-term boolean. FOUR conflict regions; took the
  roster architecture, then repaired it — THE ROSTER IS A CLOSED LIST, so an overlay
  missing from it is INPUT-DEAD, not merely unstyled:
  (1) `npOpen` (the #48/bcfeb03 Now Playing transport) had NO entry — the pick predates
  it. As-is the transport would render but ignore keyboard AND pad, and roster-derived
  anyModal would stop suppressing rail input so arrows would scroll the grid behind it.
  Added the entry with main's handlers verbatim at INDEX 1, matching the pre-roster
  precedence (deck → np → wizard).
  (2) the media entry still called the pre-#75 free functions — rewired onto the
  MediaNav class (.open/.move/.activate/.back incl. stickY).
  Audited the other nine 1:1 against main's anyModal; no orphaned `if (npOpen)` block
  survived. Screensaver/remote need no entry BY DESIGN (layout-mounted self-listener;
  synthetic gamepad-events through the normal pad path).
- **Verify:** bun run check (pass, 359 files, 0 errors) · bun run build (pass) · bun
  run test (21 pass) · cargo clippy --release -D warnings (pass) · cargo test
  --release (91 pass — ritual). No bindings, no deps.
  needs-session-verify PRIORITY: this reshapes the whole input layer — test-session.sh
  covers deck/pick/close; NP transport + wizard + OSK-in-search want a real controller.
- **Outcome:** shipped to draft PR #77 (supersedes #28 — close when it lands).
- **Next candidate:** #46 overscan (`126517f`,`20d82e1`) then #44 layouts
  (`6af24bb`,`5c0a042`) — both were AUTHORED against this router, so post-#77 main is
  much closer to their intended base than anything earlier in the wave was. Then Wave 4
  reworks (#41 themes / #42 resume / #43 artcache) and #39's last loose commit
  `0dabfea` (L2/R2 synthesis, needs-hardware).

## 2026-08-03 11:26 — Wave 3 pick 4: LauncherForm extraction + the #39 carry-fix
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 3 step 4 — the +page.svelte cluster. Also
  lands one of the two loose commits the triage flagged on #39's branch (`7a3be33`),
  applicable now that #72 put argv.ts on main. Context: #75 merged + #30 closed this
  session at Andrew's direction.
- **Branch / PR:** `pick/form` — https://github.com/atiner117/omnideck/pull/76
- **Changed:** `95dc85d` + `7a3be33` onto main `1dedf1d`. LauncherForm.svelte new
  (+72); page keeps persistence + collision toast; .confirm-btns/.cbtn move to
  Modal.svelte's shared vocabulary, .frow into the component; +page −62. Carry-fix
  swaps the component's `cmd.split(/\s+/)` for splitArgv (restoring what main's inline
  addCustom did). SIX conflict regions read in full — TWO neighbour findings:
  (1) the pick predates fa6fe2c's design tokens and hardcodes #7e8aa0/#1b2540/#2c3a5c/
  #cdd7e6/#9fb0c8 in Modal.svelte — ported its NEW rules onto var(--surface)/
  var(--border)/var(--text-soft)/var(--text-muted)/var(--text-dim), each mapping
  checked against the :root block, instead of reintroducing raw hex; (2) `7a3be33`'s
  SECOND half restores the old ebanner/bootBanner + an inline libErr line — superseded
  by #48's durable bootErrors panel (6208ef8, the commit that killed draft #22), and
  libErr doesn't exist on main. Kept main's panel; took only the quote-split half.
  Verified the component carries main's slug hardening + numeric de-dup verbatim, that
  the 10 remaining .cbtn uses sit inside <Modal>, and .frow has zero uses left.
- **Verify:** bun run check (pass, 359 files, 0 errors) · bun run build (pass) · bun
  run test (21 pass) · cargo clippy --release -D warnings (pass) · cargo test
  --release (91 pass — ritual). No bindings, no deps.
  needs-session-verify: adding a launcher with a quoted path from the couch.
- **Outcome:** shipped to draft PR #76 (supersedes #32 — close when it lands). #39's
  other loose commit `0dabfea` (L2/R2 synthesis) still pending — needs-hardware.
- **Next candidate:** #28 router (`8b4f268`) — the BIGGEST +page rewrite; everything
  after it assumes the unified overlay roster. Expect many regions; the neighbour
  discipline matters most here (main has gained npOpen, bootErrors, mediaNav.open,
  DeckSwitcher, screensaver + remote overlays since July). Then #46 → #44.

## 2026-08-03 10:47 — Wave 3 pick 3: MediaNav extraction (neighbour check caught a regression)
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 3 step 3 — the +page.svelte cluster
  (frontend-split track). Context: #74 merged + #26 closed this session at Andrew's
  direction.
- **Branch / PR:** `pick/medianav` — https://github.com/atiner117/omnideck/pull/75
- **Changed:** `38e2ec2` from draft #30 onto main `cd6f44c`. medianav.svelte.ts new
  (+95): MediaNav class owns open/loading/stack/focus/posters; page keeps input
  routing, status toast, Now-Playing cards, error reporting — injected as
  onerror/onplay/holdstop. +page −98. THE #74 LESSON PAID OFF — read both removed
  regions in full instead of trusting the pick's boundaries, found TWO problems:
  (1) the pick's onplay keys the NP card `media-${id}`, but main uses the per-LAUNCH
  key the backend RETURNS (`media-<id>#<seq>`) — taking the pick verbatim would have
  silently regressed the exact fix that superseded #13's launch-token commit (replay
  shares a key → first exit clears the live card). Rewrote the callback, carried
  main's explanatory comment. (2) the pick's overlay-active expression predates #48's
  NP transport overlay and omits `npOpen` — would have stopped the transport
  suppressing rail input. Kept npOpen + took the mediaNav.open rename. Verified the
  module carries main's newer behaviors (pct/series subtitle, resume/latest dedup)
  BEFORE dropping the inline code.
- **Verify:** bun run check (pass, 358 files, 0 errors) · bun run build (pass) · bun
  run test (21 pass) · cargo clippy --release -D warnings (pass) · cargo test
  --release (91 pass — ritual). Zero stale mediaOpen/mediaStack/mediaFocus/
  mediaLoading refs left. No bindings, no deps.
  needs-session-verify: drill-down/back/play against live Jellyfin from the couch.
- **Outcome:** shipped to draft PR #75 (supersedes #30 — close #30 when #75 lands).
- **Next candidate:** #32 form (`95dc85d` LauncherForm extraction) PLUS `7a3be33`
  (the quote-split carry-fix — now applicable since #72 landed argv.ts). Same
  discipline: read whole removed regions. Then #28 router (biggest +page rewrite,
  everything later assumes it) → #46 overscan → #44 layouts.

## 2026-08-03 06:14 — Wave 3 pick 2: DeckSwitcher extraction (CSS-neighbour trap)
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 3 step 2 — the +page.svelte cluster
  (frontend-split track, VISION priority 1). Context: #60 merged + #17 closed this
  session; #73 (a parallel session's re-issue of the same SettingDef port) closed in
  favor of #60 after diffing both heads — functionally identical, only an explanatory
  comment differed. #73 existed because that sandbox blocked merge commits entirely;
  this session could merge, so #60 kept the cleaner artifact.
- **Branch / PR:** `pick/deck` — https://github.com/atiner117/omnideck/pull/74
- **Changed:** `bd4bfe3` from draft #26 onto main `ba1a118`. DeckSwitcher.svelte new
  (+82), +page −54; page keeps state/input routing, passes apps/focus/iconFor +
  callbacks. ONE conflict, and a TRAP: the pick deletes the deck CSS block, but since
  July that block gained a neighbour — #48's Now Playing transport styles
  (.np-scrim/.np-transport/.np-t-*) sit INSIDE the removed span. Taking the pick's
  side wholesale would have silently deleted the transport overlay's stylesheet.
  Split the region: deck rules dropped, NP rules kept; verified both ways (component
  has 11 deck rules + 0 NP; page has 0 deck + 4 np-t). A11y: the component carries
  its own prefers-reduced-motion rule.
- **Verify:** bun run check (pass, 357 files, 0 errors) · bun run build (pass) · bun
  run test (21 pass) · cargo clippy --release -D warnings (pass) · cargo test
  --release (91 pass — ritual, no Rust changes). No bindings, no deps.
  needs-session-verify: deck open/pick/close behavior — packaging/test-session.sh.
- **Outcome:** shipped to draft PR #74 (supersedes #26 — close #26 when #74 lands).
- **Next candidate:** #30 medianav (`38e2e2c` MediaNav browse-state extraction) —
  fresh conflict check against post-#74 main; the wave stays sequential
  (#30 → #32 (+7a3be33) → #28 router → #46 overscan → #44 layouts). LESSON for the
  remaining extractions: each removed span may have accreted neighbours since July —
  read the WHOLE removed region, don't trust the pick's boundaries.

## 2026-08-02 22:52 — Wave 3 pick 1 REFRESHED: #60 (SettingDef) updated across 13 merges
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 3 step 1 — kept mergeable. Process note:
  a PARALLEL iteration had already opened PR #60 (2026-07-31, based on post-#59 main);
  this session nearly opened a duplicate — the #53 lesson (full inventory before
  absence claims) applies to sessions too, caught here by a stale-branch collision.
  Updated #60 in place per the 07-29 precedent. Context: #72 merged + #13/#24 closed
  this session at Andrew's direction — all lane drafts (#9–#17) now drained.
- **Branch / PR:** `pick/settings` — https://github.com/atiner117/omnideck/pull/60
  (updated in place; branch is checked out in the main workspace — pushed to the ref
  from a detached worktree HEAD).
- **Changed:** merged main (through #72 — Wave 2 + all three lane picks) into the
  branch. ONE +page.svelte conflict: the settings-row onclick — kept the port's
  settingRowClick extraction, rewired `focus = i` → `focusRaw = i` (#72 made focus
  $derived; a write to it would fail svelte-check). night-log slotted chronologically
  (the 07-31 entry sits between the Aug-1 and Jul-30 blocks).
- **Verify:** bun run check (pass, 356 files, 0 errors) · bun run build (pass) · bun
  run test (21 pass) · cargo clippy --release -D warnings (pass) · cargo test
  --release (91 pass — ritual, no Rust changes).
- **Outcome:** #60 refreshed and green — ready for Andrew's review. The +page wave's
  step 1 is mergeable again.
- **Next candidate:** merge #60, then #26 deck (`bd4bfe3` DeckSwitcher extraction) —
  fresh conflict check against post-#60 main; the wave stays strictly sequential
  (#26 → #30 → #32 (+7a3be33) → #28 → #46 → #44).

## 2026-08-02 22:44 — Lane pick 3: fable-frontend + quotesplit (launch token superseded)
- **Vision tie:** PR-TRIAGE-2026-07-26 remaining lanes; NOTES-PERFORMANCE (icon fetch
  was O(library) at mount) + launcher UX (paths with spaces). Deliberately landed
  BEFORE the +page wave: these lanes sat under the extractions in the original
  integration layering — landing first shrinks the wave's conflict surface. Context:
  #71 merged + #12 closed this session at Andrew's direction.
- **Branch / PR:** `pick/frontend` — https://github.com/atiner117/omnideck/pull/72
- **Changed:** hunk-check split #13: launch token `2ba8a99` SUPERSEDED (f176d25's
  launchId.ts is the same feature, evolved — triage's collision flag confirmed);
  icon windowing `416afd4` + derived clamps `2332d2e` live (minus-line test: the code
  they replace exists verbatim on main). #24's `b7978d7` fully live (argv.ts new).
  All three picked CLEAN — zero conflicts. + argv.test.ts (8 tests, one per
  documented tokenizer rule — nav/osk/launchId all have suites, the new pure module
  shouldn't be the exception). Unblocks the wave's 7a3be33 carry-fix (#32).
- **Verify:** bun run check (pass, 355 files, 0 errors) · bun run build (pass) · bun
  run test (21 pass — 8 new argv) · cargo clippy/test (pass, 91 — no Rust changes,
  ritual run). No bindings, no deps.
- **Outcome:** shipped to draft PR #72 (supersedes #13 AND #24 — close both when it
  lands). ALL LANES DONE: #9-#17 fully drained (landed or verified-superseded).
- **Next candidate:** Wave 3 — the +page wave in strict triage order: #17 SettingDef
  (`2b6f329`,`fad46d9`) first, then #26 deck → #30 medianav → #32 form (+7a3be33) →
  #28 router → #46 overscan → #44 layouts. One pick per iteration, fresh conflict
  check each (main has moved ~20 merges past the triage baseline).

## 2026-08-02 17:53 — Lane pick 2: fable-backend — pooled X11 connection (half superseded)
- **Vision tie:** PR-TRIAGE-2026-07-26 remaining lanes; NOTES-PERFORMANCE posture — the
  navpad polls any_app_visible ~3x/s and paid a fresh X connect+auth per poll. Context:
  #70 merged + #10 closed this session at Andrew's direction.
- **Branch / PR:** `pick/x11` — https://github.com/atiner117/omnideck/pull/71
- **Changed:** hunk-check split the lane: `ca208ee` (VapourSynth re-probe) SUPERSEDED
  by 1c2b31e's refined version — THIRD lane item that commit pre-empted; only
  `e564ced` (pooled X11) live. Picked it + one completion commit. with_x11 (liveness
  round-trip on reuse, transparent reconnect; hotkey thread keeps its own conn —
  wait_for_event would wedge a shared one). Merge care: main's #48 deck-flow logic won
  everywhere (early-exit, freeze/LAST_HIDE snapshot, find-first + map-before-thaw) —
  only the pooling structure taken from the pick; premature-resume ordering discarded.
  Completion: deck_cancel (born in #48) converted too, connect-first ordering kept.
  Mid-resolution cargo check used to arbitrate before committing.
- **Verify:** bun run check (pass, 353 files, 0 errors) · bun run build (pass) · bun
  run test (13 pass) · cargo clippy --release -D warnings (pass) · cargo test
  --release (91 pass — 1 ignored). Rust-only, no bindings, no new deps.
  needs-session-verify: deck open/pick/cancel cycles over the pooled conn —
  packaging/test-session.sh covers exactly these paths.
- **Outcome:** shipped to draft PR #71 (supersedes #12 — close #12 when #71 lands).
- **Next candidate:** #13 frontend polish (focus-clamp refactor, icon-load windowing,
  launch token — triage flags launch-token may collide with f176d25's per-launch
  instance ids: hunk-check) and #24 quotesplit — both touch +page edges; consider
  folding into the +page wave instead. #17 SettingDef starts Wave 3 proper.

## 2026-08-02 12:48 — Lane pick 1: fable-media — re-resolve, mpv token header, config_version
- **Vision tie:** PR-TRIAGE-2026-07-26 remaining lanes; media-server track reliability.
  Context: #69 merged + #47 closed this session at Andrew's direction — Wave 2 fully
  landed.
- **Branch / PR:** `pick/media` — https://github.com/atiner117/omnideck/pull/70
- **Changed:** all three #10 commits onto main `2f9aa52` + one fix. Hunk-check first:
  API-header auth already on main (7cdd45f) but the mpv-stream api_key param wasn't;
  server() still OnceLock; config_version zero hits — all three live. `158cfc2`
  (RwLock re-resolve cache, Arc<JellyfinServer>, invalidate() after config saves;
  KEPT 7cdd45f's valid_id; EXTENDED: restore_from invalidates too — #64 postdates the
  pick), `ba5b856` (mpv stream auth via X-Emby-Token header — token out of URL-shaped
  surfaces), `f7ef503` (CONFIG_VERSION=1, serialized first, migration hook). Fix: the
  manual Config Default predated Waves 1-2 — completed with the accumulated fields
  (caught by the export suite at compile).
- **Verify:** bun run check (pass, 353 files, 0 errors) · bun run build (pass) · bun
  run test (13 pass) · cargo clippy --release -D warnings (pass) · cargo test
  --release (91 pass — 1 ignored) · **live mediasrv smoke against the real Jellyfin**:
  12 resume + 16 latest browsed through the new RwLock path · 23 export tests,
  bindings in sync. No new deps.
- **Outcome:** shipped to draft PR #70 (supersedes #10 — close #10 when #70 lands).
- **Next candidate:** #12 backend polish (#20 pooled X11 conn, #23 VapourSynth
  re-probe — hunk-check both halves) or #13 frontend polish / #24 quotesplit (+page
  edges — may belong with the +page wave). Then #17 SettingDef starts Wave 3 proper.

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

## 2026-07-31 22:57 — Wave 3 pick 1: table-driven Settings model (SettingDef table)
- **Vision tie:** PR-TRIAGE-2026-07-26 Wave 3 step 1 — the sequenced +page.svelte cluster
  (frontend-split track, VISION priority 1) opens with #17's SettingDef table; #46 overscan
  and #44 layouts explicitly depend on these settings-defs. Wave 1 fully merged last night
  (#54–#59 all in main), so the wave was unblocked.
- **Branch / PR:** `pick/settings` — https://github.com/atiner117/omnideck/pull/60
- **Changed:** #17's two commits ported onto main `705d96b`: new `src/lib/settings-defs.ts`
  (+254, byte-identical to `2b6f329`) and the +page.svelte collapse (−177/+43) of the four
  string-key dispatchers (settingValue/adjustSetting/cycleSetting/NUM_META+setNum+setText)
  into one pass over SETTING_DEFS. NOTE: `git cherry-pick`/`apply` were sandbox-blocked
  this session, so the port was done by hand with extra verification: every hunk site
  confirmed textually identical to the draft's base first (main's #48/Wave-1 work never
  touched the settings machinery — no rows dropped), then the result diffed against
  `fad46d9` to confirm the settings hunks match exactly (only main-side features differ).
- **Verify:** bun run check (pass, 347 files, 0 errors) · bun run build (pass) · bun run
  test (13 pass) · no Rust touched.
- **Outcome:** shipped to draft PR #60 (supersedes #17 — close it when this lands).
- **Next candidate:** Wave 3 step 2: #26 DeckSwitcher extraction (`bd4bfe3`) onto a
  `pick/deck` branch off whatever main is then — but re-check against main first: main's
  deck code gained `deckCancel`/freeze handling since the draft, so the extraction may
  need the same hand-merge treatment as tonight. Steps stay sequential: #26 → #30 → #32
  → #28 → #46 → #44.

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
