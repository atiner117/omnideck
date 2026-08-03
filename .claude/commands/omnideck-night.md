---
description: One autonomous iteration of nightly omnideck work — pick a vision-driven increment, build it green, open/update a draft PR, log it. Designed to be driven by `/loop`.
---

You are running **one iteration** of the omnideck nightly loop. Work autonomously and safely.
The whole point is a single small, reviewable, *green* increment — not volume. When the increment
is shipped to a draft PR (or you decide to stop), **end the iteration**; `/loop` will wake you for
the next one.

## Iteration contract

1. **Orient.** Read `VISION.md` (the compass) and the top ~30 lines of `docs/night-log.md` (what
   recent iterations did and their "next candidate" notes). Skim the relevant `NOTES-*.md` for the
   area you're about to touch.

   Then **inventory every open PR — all of them, before you choose anything:**
   ```bash
   # --limit high enough to never truncate; note the count, it's your denominator
   gh pr list --state open --limit 200 --json number --jq 'length'
   gh pr list --state open --limit 200 --json number,title,headRefName,baseRefName \
     --jq '.[] | "\(.number)\t\(.headRefName)\t\(.title)"'
   ```
   There is a deep backlog of *unmerged* drafts. Because they never landed, their features do not
   exist on `main` — so `VISION.md` and the roadmap will read as though the work is still open when
   a finished draft is already sitting there. **An open PR's title outranks the roadmap docs**, which
   are gitignored (`.gitignore:28`) and have been found stale before.

   Rules for using that inventory:
   - **Never narrow the range and then draw a conclusion about absence.** It is fine to *analyse* a
     subset — it is never fine to say "no PR exists for X" from a subset. Absence claims require the
     full list.
   - Before writing that any lane, feature, or branch has no PR, **search the full inventory for it
     by name** and show what you searched:
     ```bash
     gh pr list --state open --limit 200 --search '<lane-or-branch-keyword>' \
       --json number,title,headRefName --jq '.[] | "\(.number)\t\(.headRefName)\t\(.title)"'
     ```
   - If you report on a subset, **state the subset and the total** ("triaged #18–#47; 39 open in
     total, #9–#17 not covered") so nobody mistakes partial coverage for complete coverage.
   - If a candidate increment already has an open PR, **do not reimplement it** — pick something
     else, or write up the landing path for the existing one.

   > This exists because the 2026-07-26 iteration triaged #18–#47, silently dropped #9–#17, and
   > concluded the `fable-audio` and `fable-settings` lanes had "no PR" and should have PRs opened —
   > when those lanes *are* PRs #15 and #17. Following that would have opened duplicates.

2. **Propose one increment.** Choose the single highest-value change that advances a VISION.md
   priority and is **shippable and verifiable tonight**. Small and coherent — one concern. Prefer a
   "next candidate" left by the previous iteration if it still makes sense.
   - If nothing is safely shippable (blocked, needs a human decision, too large to land green),
     **write a night-log entry saying so and stop the loop** (call the loop's stop mechanism / do
     not schedule another wake-up). Do not invent scope.

3. **Branch — never touch `main`.** Use one branch per night:
   `loop/night-$(date +%Y%m%d)`. Create it from an up-to-date `main` if it doesn't exist yet,
   otherwise continue on it (each night accumulates its increments into one branch / one PR):
   ```bash
   git fetch origin main --quiet
   BR="loop/night-$(date +%Y%m%d)"
   git show-ref --verify --quiet "refs/heads/$BR" && git switch "$BR" || git switch -c "$BR" origin/main
   ```
   Never `git switch main`, never commit to `main`, never force-push.

4. **Implement** the increment. Additive and contained. No broad refactors or mass renames. No new
   runtime dependencies unless you justify it in the PR body and the night-log.

5. **Verify green (the real stopping gate — not the clock).** Run whichever apply to what you
   touched; **all that you run must pass**:
   ```bash
   bun run check        # svelte-check typecheck (frontend)
   bun run build        # frontend build must succeed
   cargo check  --quiet --manifest-path src-tauri/Cargo.toml     # if Rust changed
   cargo clippy --quiet --all-targets --manifest-path src-tauri/Cargo.toml  # if Rust changed
   ```
   If it isn't green and you can't fix it cleanly this iteration, **revert your increment**
   (`git restore` / `git reset --hard HEAD` on just this change) so the branch stays green, log the
   attempt, and either try a smaller increment or stop. Never leave the branch broken.

6. **Commit** with a clear, scoped message describing the increment.

7. **Push over gh-token HTTPS and open/update a DRAFT PR.** The SSH `origin` needs a yubikey touch
   and won't work unattended — push via the gh credential helper to an explicit HTTPS URL instead,
   which uses the keyring token non-interactively. This does **not** change the `origin` remote.
   ```bash
   git -c credential.helper='!gh auth git-credential' \
       push https://github.com/atiner117/omnideck.git "HEAD:refs/heads/$BR"
   # open a draft PR the first time; subsequent pushes update it automatically
   gh pr view "$BR" >/dev/null 2>&1 || \
     gh pr create --draft --base main --head "$BR" \
       --title "night $(date +%Y-%m-%d): <increment title>" \
       --body "Autonomous nightly increment. See docs/night-log.md. Draft — review before merge."
   ```

8. **Log it.** Prepend an entry to `docs/night-log.md` using the template there: increment title,
   vision tie, branch/PR url, what changed, the verify results, outcome, and a **next candidate**
   for the following iteration. Commit + push the log update too.

9. **Never merge.** Andrew reviews the draft PR in the morning. Your job ends at "green draft PR +
   logged."

## Hard rules
- **No absence claim from a partial search.** "There is no PR / no test / no implementation for X"
  is only publishable after searching the *complete* set, and you must show the search. If you only
  looked at part of it, say what you didn't look at instead.
- `main` is untouchable: no direct commits, no merge, no force-push.
- One increment per iteration; if in doubt, smaller.
- The gate is **green build**, not elapsed time. If an increment runs long, commit it as WIP on the
  branch, note it in the log, and stop — don't leave work uncommitted or the branch red.
- Respect `NOTES-A11Y.md` / `NOTES-PERFORMANCE.md` / `NOTES-SECURITY.md` — a regression there is not
  shippable.
- Offload bounded, mechanical sub-work (bulk edits, summarizing large diffs) to the local model via
  the `delegate` skill to conserve usage; keep judgment on Claude.
