# Changelog

All notable changes to OmniDeck are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[SemVer](https://semver.org/) (pre-1.0: minor bumps may break).

## [Unreleased] — 0.2.0

### Added
- **AUR packaging, validated in CI**: corrected `PKGBUILD` (release tarball + `b2sums`,
  full hicolor icon set, `.install` post-install hint, `StartupWMClass`, `options=(!lto)`),
  committed `.SRCINFO`, and a `packaging.yml` workflow that lints (`namcap`), checks
  `.SRCINFO` sync, and builds the package in a clean Arch container.
- **Supply-chain CI**: `cargo-deny` (advisories / licenses / bans / sources, `deny.toml`)
  and `cargo-audit` (RustSec) jobs; a version-sync job keeps `Cargo.toml`,
  `tauri.conf.json`, and `PKGBUILD` agreeing.
- **`omnideck://` asset protocol**: Steam library art and SteamGridDB capsules are served
  as plain URLs from one canonicalize-and-allowlist chokepoint instead of base64 `data:`
  URLs pinned in webview state — a large-memory win on big libraries.
- **Event-driven Now Playing (zbus MPRIS)**: a session-bus watcher pushes `media-changed`
  events the moment a player changes track/state. Replaces the 4 s `playerctl` poll;
  `playerctl` is no longer needed at runtime. Media keys (play/pause/next/previous) go
  over D-Bus too.
- **Virtualized XMB rail**: only the rows around the focus are rendered (offset-preserving
  spacer), so navigation cost is constant regardless of library size, and game art loads
  just ahead of visibility instead of all at once at startup.
- **Proper CLI** (clap): `omnideck probe | scan | config | catalog | gridart <appid> |
  media`, plus `--help`/`--version`; unknown flags are rejected instead of ignored.
- **Generated IPC types** (ts-rs): the TypeScript side of the Rust↔JS contract is generated
  from the Rust structs into `src/lib/bindings/`; CI fails if they drift, so a Rust field
  rename breaks the build instead of silently becoming `undefined` in the frontend.
- **App switcher** (session): switching no longer kills the launched app — it hides it
  (gamescope refocuses OmniDeck) and brings it back on the next switch, process and audio
  intact. Console-style bindings: **Guide press / `Ctrl+Alt+Home`** = switch,
  **Guide hold (≥ 0.8 s) / `Ctrl+Alt+End`** = close and return. The chords are global X
  grabs (work while the app owns keyboard focus — the keyboard twin of the Guide button,
  which reads evdev directly). Born from the first real M2 hardware session runs, recorded
  in `packaging/M2-RESULTS.md`.
- **Launched Qt/KDE apps follow your KDE theme** (session): children get
  `XDG_CURRENT_DESKTOP=KDE` + `QT_QPA_PLATFORMTHEME=kde` so System Settings & friends load
  plasma-integration and render with your color scheme (dark mode included) instead of
  falling back to light Fusion.
- **First-run wizard & a11y baseline**: dialog semantics (`role="dialog"`, focus
  management), keyboard-focusable rows, `:focus-visible` rings, `aria-label`s on icon
  buttons, `prefers-reduced-motion` support, footer contrast fix.
- **Session display-mode override**: the generated `omnideck-session` launcher sources
  `~/.config/omnideck/session.conf` (`GAMESCOPE_FLAGS="-W 2560 -H 1440 -r 165 -O DP-3"`)
  so high-refresh panels aren't stuck at the EDID-preferred 60 Hz, and enables
  `--adaptive-sync` (VRR) by default.
- **File logging** (`tracing`): everything that used to go only to stderr now also lands in
  a daily-rotating file under `$XDG_STATE_HOME/omnideck/` (default
  `~/.local/state/omnideck/omnideck.<date>.log`, 7 days kept), including panics — so a
  broken gamescope session can be debugged after logging back into the desktop instead of
  via display-manager log forensics. `RUST_LOG` filters both sinks (default `info`).
- **Config error surfacing**: a `config.toml` that fails to parse now shows a toast with
  the parse error ("using defaults until fixed") instead of silently reverting — and the
  app **refuses to overwrite** the broken file until it's fixed.

### Changed
- **NVIDIA/WebKitGTK workarounds are now session-aware** (2026 behavior): dmabuf renderer
  disabled on X11/gamescope; `__NV_DISABLE_EXPLICIT_SYNC=1` on Wayland (keeps the
  hardware-accelerated path); `GDK_BACKEND=x11` is no longer forced on Wayland desktops.
- **The gamescope session runs plain gamescope** — `gamescope-session-plus` is not used or
  required; docs, capability diagnostics, and `install-session.sh` (now always installs to
  `/usr/local/share/wayland-sessions`) agree.
- Settings changes apply through fine-grained mutation (`patchSettings`) — no more
  whole-config rebuild (and background-image refetch) on every nudge.
- Power actions report polkit denials as a visible error toast instead of silently doing
  nothing; "Exit" is labeled "Log out" inside a session.
- Shared HTTP client with real timeouts (connect 5 s / read 10 s / total 15 s) — a hung
  CDN or captive portal can no longer wedge art/icon fetches or the `gridart` CLI.
- SteamGridDB art cache is capped at 100 MB (oldest evicted; refetches on demand).
- Custom launchers de-duplicate their ids with a numeric suffix instead of silently
  overwriting a same-named entry; empty/symbol-only names are rejected.

### Security
- `quick-xml` RUSTSEC-2026-0194/0195 (DoS, via `plist`/`tauri-utils`): documented ignores
  in the audit gates — the parser never sees untrusted XML in a Linux launcher, and no
  fixed release exists on our tree yet (drop the ignores when `plist` adopts quick-xml 0.41).
- Tauri capabilities scoped to exactly what the frontend uses (dropped `core:default` and
  the unused `opener` plugin + its dependency tree).
- Config values are sanitized on load: accent/background colors must be `#rrggbb` (they
  flow into CSS), `search_provider` must be http(s) (it flows into a browser launch),
  enums reset to safe defaults.
- SSRF guards on icon/art fetching: private/loopback/link-local IPs are refused —
  including `inet_aton` short/hex forms (`127.1`, `0x7f.0.0.1`) — and every **redirect
  hop** is re-checked, so a public host can't 302 into the internal network.
  SteamGridDB image URLs must be https.
- Byte-capped downloads everywhere (content-length can lie); image responses are
  magic-byte sniffed.
- `quinn-proto` bumped past RUSTSEC-2026-0185 (remote memory exhaustion, 7.5 high) —
  caught by the new supply-chain gate on its first CI run.

### Fixed
- A broken `config.toml` can no longer be clobbered by automatic saves (recent-apps
  writes fired on every launch).
- Steam-exit watchdog no longer spins forever if Steam crashes mid-game (15 min unknown
  budget) and correlates exits by launch id, not display name.
- Guide-button "close app" only reports success when a signal actually reached the app.
- Held D-pad auto-repeat stops when a modal opens (no more phantom navigation behind
  dialogs); axis jitter is coalesced before crossing IPC (~10× fewer events on drifty
  sticks).
- Stale/deleted art files show the styled name tile instead of a broken image, without a
  refetch loop.
- Various leaked timers cancelled on unmount; stale async resolves (background image,
  search-engine favicon) are dropped by sequence guards.

## [0.1.0] — first tagged release
Initial public snapshot: XMB-style controller-first launcher, Steam library scan +
launch with exit watchdog, curated app/media catalog with favicon fetching, SteamGridDB
box art, MPRIS Now Playing, capability probe (gamescope session / desktop / kiosk
tiers), gamescope session installer, hand-editable `config.toml`.
