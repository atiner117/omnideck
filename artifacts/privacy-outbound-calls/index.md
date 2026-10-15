---
title: "Make 'zero data collection' strictly true: opt-in favicon fetch + update check"
kind: walkthrough
status: done
created: 2026-10-14
updated: 2026-10-14
thread: claude-code:omnideck-privacy (branch fix/privacy-outbound-calls)
---

## Change areas (by behavior)

1. **Favicon fetch is opt-in** (`src-tauri/src/icons.rs`, `commands.rs::app_icon`).
   New `settings.fetch_remote_icons` (default false). `favicon(url, allow_network)` first
   consults the on-disk cache (`local_lookup`, no network); on a miss with fetching off it
   returns None *before* `is_blocked_host_resolved`, so not even a DNS lookup leaks the
   domain. The Google s2 fallback is removed; candidates are DDG → the site's /favicon.ico.
2. **Update check defaults off** (`config.rs`). `check_updates` default true → false.
   Container-level `#[serde(default)]` means configs missing the field read false; configs
   that already contain `check_updates = true` (every generated config wrote the field) keep it.
3. **Settings → Privacy** (`src/lib/settings-defs.ts`): two cycle rows. Turning icons on
   awaits the save, clears `iconTried`, bumps `iconEpoch` so the icon `$effect`s re-ask
   (`src/routes/+page.svelte`). `patchSettings` now returns the save promise.
4. **Docs**: README "Privacy" table of every outbound connection; CHANGELOG [Unreleased].

## Review order

icons.rs gate ordering (cache → network gate → SSRF check → fetch) → config default flip and
its test → +page.svelte re-fetch wiring → README claims vs code.

## Decisions not obvious from the diff

- Cached icons still render with fetching off: reading a local file is not a network call,
  and blanking icons users already have would look like a regression.
- Existing explicit `check_updates = true` is preserved rather than migrated: silently
  flipping a written user value is worse than a documented default change. README says how
  to opt out.
- On origin/main the frontend never calls `check_update` at boot (only the command exists),
  so the flip is defense-in-depth for whichever branch wires the boot-time call.
- NOTES-SECURITY.md is gitignored (private maintainer notes), so the public outbound list
  lives in README → Privacy only.

## Verify

```bash
cd src-tauri && cargo clippy --release --all-targets -- -D warnings && cargo test --release
TS_RS_EXPORT_DIR="$PWD/../src/lib/bindings" cargo test --release export_bindings && git diff --exit-code ../src/lib/bindings
cd .. && bun run check && bun run test && bun run build
```
Manual (couch): fresh config → web tiles show emoji, no requests to icons.duckduckgo.com
(`ss -tnp | grep omnideck` or a DNS log); Settings → Privacy → Fetch site icons → on →
icons appear without a restart.
