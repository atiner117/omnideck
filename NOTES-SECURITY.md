# NOTES-SECURITY.md (refreshed 2026-06-27, HEAD `1f1d617`)

> Candid maintainer posture doc. **Much stronger than the GLM-REVIEW snapshot** — no critical/
> high release-blockers remain in this dimension; the one thing to fix before public release is
> the **reqwest timeout gap**. See [[NOTES.md]] for the scoreboard.

## What's FIXED since GLM-REVIEW ✅ (don't re-flag)
- **CSP** was `null` → strict policy (`tauri.conf.json:22-33`): `script-src 'self'`,
  `object-src 'none'`, `frame-src 'none'`, `base-uri 'self'`. + zero `{@html}`/`innerHTML` sinks
  → XSS surface effectively closed. (§4.1 closed.)
- **`search_provider` flag-injection** closed FE (`+page.svelte:624-626`) + BE
  (`is_safe_browser_arg` `lib.rs:376-379` + tests `lib.rs:230-239` rejecting `--no-sandbox`,
  `--renderer-cmd-prefix`, `--app=file://`). (§4.2 closed.)
- **`get_art` unbounded read** → image-extension allowlist + 32 MiB cap (`lib.rs:319-340`).
  `~/.ssh/id_rsa` isn't `.png` → secret-exfil blocked. (§4.4 closed.)
- **Unbounded fetch OOM** → `fetch_capped` byte caps (`icons.rs:84-100`, `steamgriddb.rs:30-46`)
  + magic-byte sniff + DDG-placeholder rejection. (§4.5 size closed.)
- **No shell anywhere** — every external binary (`steam`/`systemctl`/`playerctl`/`xprop`/`kill`/
  `flatpak`) is `Command::new(..).arg(verb)` against an allowlist.

## Threat model (unchanged intent)
Local trusted users (family couch PC) vs strangers on AUR/Flatpak (malicious `config.toml`,
crafted SteamGridDB art, custom-launcher form); session vs desktop (gamescope `xprop`/`kill`/
`systemctl` surface differs); webview exposure of Rust→JS data. Single-user local launcher ⇒
several "injection" findings are low-severity by design (the user's own config, the user's
privileges).

## Remaining surfaces — by priority

### 1. reqwest has NO timeout — `medium` · `new` (fix before release)
`icons.rs:85` `reqwest::get(url).await.ok()?` + `steamgriddb.rs:31` (same) and `:65`
`reqwest::Client::new()` (no `.timeout`). A server that accepts the connection but never
sends/closes (captive portal, dead CDN edge) makes `.chunk().await`/`.json().await` hang
forever. `grid_art`/`app_icon` are async Tauri commands → each hung call stays inflight
forever (the FE's `iconInflight`/spinner pins open). The icon path tries 6 candidates
sequentially (`icons.rs:57-69`). **Bonus hang:** the `--gridart` CLI arm (`lib.rs:613`) does
`tauri::async_runtime::block_on(box_art(..))` → freezes the **whole process** on a hung CDN.
**Action:** one shared client in `once_cell::Lazy` / `tauri::State`:
`reqwest::Client::builder().connect_timeout(Duration::from_secs(5)).timeout(Duration::from_secs(15)).build()`;
route every `reqwest::get(..)` + the steamgriddb API/asset calls through it. Defense-in-depth:
wrap each candidate in `tokio::time::timeout`.

### 2. `opener` plugin + `opener:default` is dead attack surface — `low` · `open`
`capabilities/default.json:8` grants `opener:default`; `lib.rs:632` registers
`.plugin(tauri_plugin_opener::init())`; but `grep -rn 'plugin-opener|openUrl|opener' src/` →
**zero hits** — the frontend never calls it. (The old note here mischaracterized it as "the
controlled execution surface"; it's unused.) **Action:** remove `opener:default` from
`default.json`, drop the `.plugin(..)` line, drop the `@tauri-apps/plugin-opener` /
`tauri-plugin-opener` deps. Least-privilege: don't ship a capability nothing calls.

### 3. Favicon SSRF probe vector — `low` · `new`
`icons.rs:58-62` fetches `https://{d}/favicon.ico` + DDG + Google for a tile host `d`;
`domain_of` (`icons.rs:18-30`) accepts any dotted host → a crafted/imported config tile URL
like `http://169.254.169.254` makes OmniDeck probe internal HTTP services (timing/existence)
via the DDG/Google proxies. **Action:** before fetching, resolve the host and reject
loopback/private/link-local ranges (127/8, 10/8, 172.16/12, 192.168/16, 169.254/16, ::1,
fc00::/7). Low severity (single-user + import-config model) but a real door before strangers
install via AUR.

### 4. Hand-edited colors flow into CSS unvalidated — `low` · `new`
`+page.svelte:900` `<main style="--accent:{accent}; … background-color:{cfg?.settings?.background_color}">`
— both straight from `config.toml`. Svelte escapes the attribute quotes and `script-src 'self'`
blocks JS, so worst case is CSS-declaration injection (cosmetic, or a `url(…)` beacon on that
element). **Action:** in `Settings::normalize()` (`config.rs:70`) validate hex against
`^#[0-9a-fA-F]{6}$`, default on mismatch. Cheap; removes the only untrusted→style path.

### 5. `normalize()` clamps numerics but leaves strings unsanitized — `low` · `partial`
`config.rs:70-77` clamps the numeric fields only. `search_provider`/`background_image`/`accent`/
`background_color` pass through unchanged. `search_provider` is defended at the use-site
(webSearch + `is_safe_browser_arg`), so not currently exploitable, but `background_image`/
`accent` flow into `get_art`/CSS with no load-time check. **Action:** extend `normalize()` to
validate color hex + `search_provider` URL scheme (default on mismatch). Belt-and-suspenders for
#4 and future code paths. (`sort`/`search_mode`/`recents_show`/`background_default` are still
free-form Strings — an enum/allowlist check is the natural extension.)

### 6. SteamGridDB `img_url` fetched without scheme check — `low` · `new`
`steamgriddb.rs:74,82` takes `img_url` straight from the API JSON, no `https://` assertion.
Mitigated (rustls, no `file://` feature, TLS), but a compromised API response could redirect to
an internal service (SSRF) or serve a ≤16 MiB non-image. **Action:** one line —
`if !img_url.starts_with("https://") { return None; }` before the fetch.

### 7. `core:default` still broad — `low` · `partial` (prior §4.6)
`capabilities/default.json:6-9` = `core:default` + `opener:default`. The FE only uses `invoke`
+ `listen` — a fraction of `core:default`. **Action:** after removing `opener:default`, replace
`core:default` with the explicit set (`core:window:default`, `core:event:default`,
`core:app:default`) in a scoped `capabilities/launcher.json`. Lowest priority; completes
least-privilege.

### 8. CSP `asset:` widening is currently unused — `nit` · `new`
`tauri.conf.json:26` `img-src` includes `asset: http://asset.localhost` but there's no
`app.security.assetProtocol` block (art is served as `data:`). **Action:** drop `asset:`/
`http://asset.localhost` now; re-add with a scoped `assetProtocol.scope` when the
`omnideck://`/asset migration ([[NOTES-PERFORMANCE.md]]) actually lands.

## Silent failures — corrected: 4 remain (not "~15")
The old note said "~15 remaining `.catch(()=>{})`". **Accurate current count: 4** —
`+page.svelte:546` (250 ms media re-poll), `:758` (`inGamescopeSession`), `:804` (4 s media
poll), `:896` (favicon fetch). All benign transient polls; route to `console.debug` at minimum
so field reports are diagnosable. Everything else now goes through `reportError()` →
`aria-live` toast (`:1174`). (5 empty `try{}/catch{}` remain — `:208` canvas, `:347` AudioContext,
`:374/377/380` art-load — all defensible safety-nets; add a one-line comment each.)

## CSP + Svelte 5 direction — `low` · `open` (research incomplete)
`style-src 'unsafe-inline'` is currently required: OmniDeck uses Svelte component `<style>`
(injected into `<head>`) **and** inline `style="…"` attributes (CSS custom props on `<main>`
`:900`, dynamic `background-image`). Removing `unsafe-inline` is hard while inline `style=`
exists; `script-src 'self'` is already strict (a nonce is only realistic there, post-asset-
migration). Realistic end-state: **keep `style-src 'unsafe-inline'`** (lower risk than script)
until styles move to nonces/hashes. Deep research (cited path) is TODO — see [[NOTES-RESEARCH.md]].

## Runtime confirmation for custom launchers — `low` · `open` (design choice)
`launch_command` (`lib.rs:384-421`) runs a non-BROWSER custom exec verbatim (`Command::new(cmd)
.args(args)` `:412`) — no shell, so **not** injection, but the custom-launcher form
(`+page.svelte:580-594`) does `cmd.split(/\s+/)` (`:588`): no binary-existence check, no preview,
can't quote paths with spaces, and a typo (`rm -rf …`) is one Enter away. **Action (UX/safety):**
resolve argv[0] via PATH (warn if not found); split respecting quotes; optional confirm dialog
for non-catalog exec. Do **not** add a shell. (Custom-id collision is a separate bug —
[[NOTES-FRONTEND.md]] §3.)

**Suggested edit priority:** (1) reqwest timeout client; (2) remove opener + scoped caps; (3)
`normalize()` validates hex/URL; (4) SSRF private-range reject + `img_url` scheme check. Asset
protocol (enables stricter CSP) is [[NOTES-PERFORMANCE.md]].

**Cross-refs:** [[NOTES.md]], [[NOTES-ARCHITECTURE.md]] (get_art/asset), [[NOTES-PACKAGING.md]]
(Flatpak sandbox), [[NOTES-PUBLIC-RELEASE.md]] (threat model), [[NOTES-RESEARCH.md]] (CSP/Svelte5
TODO). GLM-REVIEW.md §4 (historical).

## Transitive `glib` unsoundness (Dependabot #1, GHSA-wrw7-89jp-8q8g) — `medium` · `tolerated`
`glib 0.18.5` (in `src-tauri/Cargo.lock`) has an unsoundness in `VariantStrIter`'s `Iterator` /
`DoubleEndedIterator` impls: `impl_get` passed a C out-pointer as `&p` instead of `&mut p`; recent
rustc optimizes the (now-disregarded) NULL write away, so `CStr::from_ptr` gets NULL → **NULL-deref
crash** when iterating a GVariant string array. Fixed upstream in **glib 0.20.0**.

**Why it isn't fixed here — no upgrade path on Tauri 2.** glib is *transitive*, pulled by the whole
Tauri 2 GTK3 stack: `tauri 2.11 → wry 0.55 → webkit2gtk 2.0 → gtk 0.18 → glib 0.18.5`. `gtk = "0.18"`
requires `glib ^0.18`, so glib 0.20 is semver-incompatible and `cargo update` **cannot** move it.
This is inherent to *every* Tauri 2 Linux app — gtk3-rs (0.18 line) stays on glib 0.18; glib 0.20
lives in gtk4-rs, which Tauri only adopts at its future gtk4 migration. Resolution is therefore a
**Tauri upgrade**, not an omnideck code change: it clears automatically when we move to that Tauri.

**Exposure: low.** Availability/integrity-low crash only, reached solely transitively through
GTK/webkit GVariant iteration; a single-user local launcher has no path to feed it
attacker-controlled GVariant data. Not worth vendoring a patched glib fork via `[patch.crates-io]`
(large crate, re-applied on every Tauri 2.x bump).

**Decision (2026-07-12): tolerate + document.** Dependabot alert #1 dismissed as `tolerated_risk`.
Revisit at the Tauri (gtk4 / glib-0.20) upgrade.
