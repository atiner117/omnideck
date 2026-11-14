# NOTES-PERFORMANCE.md (refreshed 2026-06-27, HEAD `1f1d617`)

> Candid maintainer doc. Good for typical family libraries (~50-200 games); power-user scale
> needs the asset-protocol + virtualization work. Corrected line numbers + folded in the asset
> + zbus research.

## Wins since GLM-REVIEW ✅
- FPS tracker (current/avg/lo/hi) in the footer (`+page.svelte:117-120,1176`), rAF-sampled but
  committed to state only every 500 ms (`:778-797`) → no per-frame reactive cost.
- `OMNIDECK_GPU_COMPOSITING` A/B toggle (`lib.rs:573-575`) — software paint default for
  reliability on NVIDIA.
- Debounced `settledFocus` (150 ms) for background/hero swaps (`:300-306`) — the previous FPS killer.
- `fetch_capped` size limits + `iconInflight`/`iconTried` + lazy art (`Math.abs(i-focus) <= 8`,
  `:967`).
- Input: hold-repeat, single-threshold deadband (`:865-875`); config clamping prevents
  pathological values.

## Current bottlenecks
- **`cfg = { ...cfg }` full-rebuild cascade** (`+page.svelte:396,420,429,441,453,538,676,679`) →
  re-sorts `games` + rebuilds the whole derived graph on every settings nudge, and re-issues an
  uncached `getArt` for the background image. Fix + `patchSettings()` helper → [[NOTES-FRONTEND.md]] §1.
- **Art as base64 data URLs in `$state`** (`art`/`heroes`/`appIcons`/`bgImageUrl` `:133-140`) —
  a 600×900 capsule is ~2.67× its file size in the webview heap (base64 + UTF-16), re-diffed on
  navigation. For 200+ games that's hundreds of MB. **Biggest lever → asset protocol below.**
- **MPRIS polling**: `setInterval(pollMedia, 4000)` (`:806`) + a 250 ms re-poll after each media
  key (`:546`), each a `playerctl` fork+exec. → zbus below.
- **Per-icon `<canvas>` luminance/dominant-color** (`computeIconBg` `:192-211`) — 24×24 canvas +
  `getImageData` per fetched favicon; objects GC'd under pressure, not released. Low impact
  (one-shot per icon, cached).
- **rAF runs forever** (`:795`) — now cheap (commits every 500 ms), but still spins for the clock
  + FPS counter. Gate behind a debug flag if battery matters on a couch device.
- **Grid won't scale past ~200-300 games** without virtualization — `{#each items as t, i (t.id)}`
  with `Math.abs(i-focus) <= 4/8` (`:964,967`) is O(n) per focus change.

## Highest leverage: `omnideck://` asset protocol migration
**Why:** bytes stay on disk (webview holds a URL string, decodes to GPU on paint) instead of
~2.67×-inflated base64 strings pinned in reactive state. Drops RSS dramatically, makes re-renders
cheaper, faster initial grid render (images fetch in parallel as DOM mounts, not N sequential
`invoke` round-trips), and lets CSP drop `data:`.

**Preferred design — custom `omnideck://` scheme (not `asset:`):** avoids the
`requireLiteralLeadingDot` pitfall (`.cache`/`.steam` segments) and reuses OmniDeck's existing
ext/size allowlist as one code-level chokepoint. Register `register_asynchronous_uri_scheme_protocol`
in `tauri::Builder`; canonicalize + prefix-allowlist (Steam librarycache, `$CACHE/omnideck/art`,
`$CACHE/omnideck/icons`, the user `background_image`); sniff MIME; add `omnideck:` +
`http://omnideck.localhost` to `img-src`. Per-source migration (Steam librarycache → bind path
directly; SteamGridDB/favicon → return cache path not data URL; bg image → push canonicalized
path into the allowlist) → full sketch + handler in [[NOTES-RESEARCH.md]] §2. Cleanup: remove
`get_art`/`to_data_url`×2 + the `base64` crate; keep `data:` in CSP one release as fallback.

## Grid virtualization (for large libraries)
Window the renderer (only visible + ~8 each side; current lazy logic approximates this). Keep the
XMB "focus-prominent" scroll; preload heroes only for adjacent items. Candidates: `svelte-virtual-list`
or a custom windowed renderer. Matters less for family libraries; matters a lot past ~300 games.

## Replace `playerctl` polling with zbus MPRIS subscription
`listen("media-changed")` instead of the 4 s poll — updates in ms, no fork+exec per tick, drops
the `playerctl` runtime dep, and fixes the `splitn(4,'\t')` fragility (`lib.rs:482`). ~150-200 LOC
new `mpris.rs` module, phased (replace read path first, then `media_control`). Full design +
gotchas → [[NOTES-RESEARCH.md]] §3.

## Other
- Fine-grained rune updates via direct mutation (the `patchSettings` fix) instead of full-object
  replacement — also fixes the bg-image/favicon `$effect` re-fire ([[NOTES-FRONTEND.md]] §5).
- Profile with browser devtools + `tracing` spans around library scan + art fetch.
- LRU with a size cap in Rust for the art cache (e.g. max 100 MB) once on the asset protocol.

## Research topics
- Svelte 5 compiler output for large `{#each}` + runes — does it tree-shake unused deriveds?
- `omnideck://` vs `asset:` performance on NVIDIA WebKitGTK (compositing interaction).
- 10-ft UI best practices (60 fps on modest HW, reduced-motion fallback).
- Memory: base64 vs `asset:`/`omnideck://` in WebKit (dev fleet is NVIDIA-heavy).

**Target:** <60 ms navigation on a 500-game library at <200 MB RSS. Family libraries are already
there; power-user scale needs the asset + virtualization work.

**Cross-refs:** [[NOTES.md]] (roadmap #8), [[NOTES-FRONTEND.md]] §1/§5 (cfg-rebuild fix),
[[NOTES-ARCHITECTURE.md]] (split), [[NOTES-SECURITY.md]] (CSP win from asset migration),
[[NOTES-RESEARCH.md]] §2/§3 (asset + zbus), GLM-REVIEW.md §3 (historical).
