# VISION — omnideck

> **North star for the nightly autonomous loop.** This is the compass a cold agent reads at the
> start of every iteration to decide *what to build next*. Andrew owns and curates this file; the
> loop only reads it. Keep it short and current — if priorities change, edit here, not in code.

## What omnideck is
A **controller-first, living-room launcher** — a couch-usable front end (Tauri + SvelteKit shell,
Rust backend, mpv-based media) for games and media, driven by a gamepad, targeting an OLED TV.
Think Steam Big Picture / console-OS ergonomics, self-hosted and open.

## Where the detailed roadmap lives (read these to pick work)
The real backlog and architecture thinking already exist in the `NOTES-*.md` corpus. Treat these as
the source of candidate work, in rough priority order:
- `NOTES-DEEPDIVE-ROADMAP.md` — forward-looking features, each with design + effort + risk. **Primary
  backlog.**
- `NOTES-DEEPDIVE-FRONTEND-SPLIT.md`, `NOTES-DEEPDIVE-MEDIA-SERVER.md` — the big in-flight
  architecture tracks; prefer increments that unblock or advance these.
- `NOTES-A11Y.md`, `NOTES-PERFORMANCE.md`, `NOTES-SECURITY.md` — cross-cutting quality bars every
  increment must respect (reduced-motion, no jank on the gamepad thread, no plaintext secrets).
- `NOTES-ARCHITECTURE.md`, `NOTES.md` — orientation.

## Near-term priorities (edit me)
1. Advance the frontend-split / media-server tracks in small, shippable slices.
2. High bang-for-effort roadmap items that plug into the new `SettingsPanel` / `stores/settings.ts`
   (e.g. OLED burn-in protection, audio-output switcher).
3. Quality: accessibility (reduced-motion), performance (never jank the gamepad rAF loop), and
   polish on already-shipped surfaces.

## Guardrails / non-goals for the loop
- **One coherent increment per iteration** — small enough to review in the morning.
- **Never touch `main` directly**; never merge; never force-push. Output is a reviewable draft PR.
- **No new runtime dependencies** without justifying it in the PR body and the night-log.
- **Respect the quality NOTES** above — an increment that regresses a11y/perf/security is not shippable.
- **Don't refactor broadly** or rename across many files unprompted; prefer additive, contained change.
- Security features (PINs, etc.) are **deterrence, not access control** — never imply otherwise.
- If nothing is safely shippable tonight, **log that and stop** rather than inventing scope.
