# Changelog

All notable changes to OmniDeck are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[SemVer](https://semver.org/) (pre-1.0: minor bumps may break).

## [Unreleased] — 0.2.0

### Added
- **Continue Watching actually resumes — and you can mark a title watched**: picking a
  half-watched film off the Continue Watching row used to restart it from 0:00 (the row was
  a label, not a feature). `MediaItem` now carries the server's resume position and watched
  flag; playback passes `--start=` to mpv — appended late in the argv so a per-launch resume
  beats a profile default — resumable rows read **"N min left"** instead of total runtime,
  and watched items show a ✓. **West / `W`** toggles watched state on a playable row: an
  optimistic flip that reverts if the server refuses. Deliberately *not* offered on a series
  or season row — Jellyfin's `PlayedItems` accepts one, but marking a show watched clears
  every episode's resume point and un-marking never gives them back, so a single press could
  destroy a 60-episode series' progress.
- **Remote artwork is a disk-first resource** (`artwork_cache.rs`): cold boots re-fetched
  every Jellyfin poster over the network, so art popped in on the rail on every launch.
  Posters now land in an FNV-1a-keyed disk cache with `.meta` sidecars — under 24 h old
  serves from disk with zero network, older revalidates with `If-None-Match`/
  `If-Modified-Since` (a 304 costs headers), and a network error serves the stale copy
  instead of a blank tile. True-LRU 200 MB budget (`[media_server] art_cache_mb`), atomic
  writes, and rail art warmed by a bounded prefetch once sections load. `omnideck doctor`
  reports the cache; `--clear-art-cache` empties it.
- **Library view modes** (`[appearance] layout`): the XMB rail is now one of four —
  **rail**, **large grid**, **compact grid**, and a **detail list** (thumbnail, type/source
  subline, last-played for games). Grids wrap top/bottom preserving the column, and falling
  off a row edge moves to the next category, so the XMB "never trapped" rule holds in 2D;
  D-pad, stick, keyboard and hold-repeat all route through the same unit-tested nav math.
  Offscreen grid rows skip paint via `content-visibility`.
- **Six built-in themes**: a theme is a named set of overrides for the design tokens, applied
  by stamping `data-theme` on `<html>` — `omnidark` (the current look, rendered
  byte-identically), `oled` (true `#000` for OLED panels), `light`, `high-contrast`, `crt`
  (phosphor green under static, *unanimated* scanlines) and `deck`. All colour work is CSS,
  so switching is a single attribute write and nothing lands on the gamepad rAF path; your
  accent rides on top of any theme unchanged.
- **TV overscan calibration**: a console-style safe-area screen — accent frame and corner
  markers at the inset boundary, live percentage card, D-pad/stick to grow or shrink 0–10 %
  in 0.5 % steps, A saves and B cancels. The inset applies globally through one `--overscan`
  custom property (modals and toasts contained too); **0 % renders byte-identical to before**.
- **Screensaver — OLED burn-in protection** (`[screensaver]`): the gamepad thread tracks
  input activity and emits idle/active transitions, and a layout-mounted overlay stages down
  through dim veil → near-black accent drift (nothing static keeps burning) → true black with
  a clock that moves each minute. Any input restores instantly, MPRIS playback suppresses
  engagement, and it is reduced-motion aware. It runs its own DOM + gamepad idle timer as
  well as reading the backend config, so it works on a plain desktop too.
- **Audio output switcher** (`audio.rs` + `AudioOutputModal`): enumerate and switch
  PipeWire/PulseAudio sinks from the couch — "TV over HDMI" versus "the receiver" without
  dropping to a desktop. `pactl -f json list sinks` with a `list short sinks` fallback for
  older pactl; every invocation is bounded by a 3 s deadline and runs on the blocking pool,
  so even a wedged sound server never reaches the IPC thread.
- **Sleep timer** (`sleep_timer.rs` + `SleepTimer.svelte`): "pause playback in N minutes",
  with presets that show the "until HH:MM" they land on, a countdown, and a final-minute
  warning. Expiry **pauses every playing MPRIS player, never kills** — falling asleep to
  music while a paused video sits behind it must silence the music, and keeping the position
  makes resuming in the morning one button. Re-arming replaces the running timer race-free.
  Deliberately not persisted across restarts: a sleep timer is about tonight's session.
- **Phone as a remote** (`[remote]`, **off by default**, port 8765): a self-contained phone
  page served by a hand-rolled `std::net` HTTP server — **no new dependency** — exposing
  MPRIS transport, volume, and navigation. Nav arrives as the same synthetic gamepad events
  a real pad produces, so it drives OmniDeck's own UI (a fullscreened app ignores it).
  Pairing is a 32-byte `/dev/urandom` token generated on first enable.
- **Parental-controls PIN** (`pin.rs` + `PinModal.svelte`): a phone-order PIN pad over an
  argon2id-hashed PIN (PHC format, fresh random salt, never stored or logged in plaintext)
  gating locked categories. Hashing runs on the blocking pool, so the deliberately-slow
  verify never janks the UI or the gamepad thread. The module header states the threat model
  plainly: **deterrence, not access control**.
- **Update check** (`update.rs`, `settings.check_updates`, default on): a check-only probe of
  the GitHub latest-release API compares the newest published tag against the running
  version, caches the answer for the process lifetime (the unauthenticated API allows
  60 req/hr; a manual "check now" bypasses the cache), and never proposes a draft or
  prerelease. Acting on an update is out of scope — OmniDeck tells you, your package manager
  does the rest.
- **Config backup & restore**: `backup_config` / `restore_config` write and read sanitized
  TOML snapshots. A restore passes the same `normalize()` gates as a hand-edited file and
  deliberately works *while the live config is broken* — restoring is how you fix that — but
  creating a backup **from** a broken state is refused, so defaults can't be laundered into a
  "good" snapshot. Both paths take the same atomic-write lock as every other config write.
- **`[launch_overrides."<tile-id>"]` — per-tile env and extra args** for tiles launched
  through `launch_command` (custom launchers and catalog apps). Steam titles hand off to the
  Steam client, so their options belong in Steam's own Launch Options. Hand-editable for now
  ("config is king"); an empty map serializes to nothing, so generated configs stay clean.
- **`[input]` config**: `guide_hold_ms` — the Guide long-hold-to-close threshold, clamped
  200–5000 ms (below 200 every tap reads as "close"; above 5000 the hold reads as broken) —
  and `session_hotkeys`, a kill-switch for the `Ctrl+Alt+Home`/`End` global grabs for
  keyboards that need those chords. Both are read once at startup: the input threads park in
  blocking waits, so a change needs a relaunch.
- **`omnideck doctor` and `omnideck logs`** — the support story next to the existing probes.
  `doctor` prints a one-command bug-report bundle: version, session detection, the full
  capability probe, config health, Steam library count, playback stack, and controllers via
  the same gilrs path the app uses — **presence-only for keys and tokens, values never
  printed** — and it is offline on purpose. `logs` lists the rotating log files with sizes
  and tails the newest (`-n N`); `omnideck logs --path` prints just the newest path, for
  `tail -f $(omnideck logs --path)`.
- **`docs/ARCHITECTURE.md`**: the committed 10,000-foot map — process shape, the ts-rs IPC
  contract, config invariants, the three input paths, the session-versus-desktop fork, the
  launch and media stacks, network policy, logging, testing — linked from CONTRIBUTING, with
  every claim re-verified against the tree before it shipped.
- **Durable boot-error panel**: if capability/catalog/config-and-library loading fails at
  startup, a persistent `role="alert"` panel now lists exactly which subsystem(s) failed
  and offers **Retry** (or `F5`) — replacing a 5 s toast that was gone before a couch user
  looked up, and media-load failures that used to be swallowed entirely. Retry re-runs only
  the failed loaders; `config.toml` parse errors move into the same durable panel instead
  of a fleeting toast.
- **Controller-reachable Now Playing transport**: a D-pad/A-navigable overlay (`L1` or `N`,
  only while something's playing) surfaces prev/play-pause/next, Switch, and Close/Dismiss
  for the primary Now Playing card — previously those buttons only responded to a pointer
  or Tab, so a controller on the dashboard couldn't touch media transport at all.
- **A vitest unit-test net for pure frontend logic** (`bun run test`): the first slice
  pulled out of the main page — the shared `clamp` helper and the O(window)
  rail-virtualization math — is now covered by tests, the prerequisite for safely
  decomposing that file further.
- **Playback controls are per-dimension toggles** (`omnideck-toggles.lua`, shipped in the
  generated profile set): one key per dimension instead of a preset profile per
  combination — F4 cycles interpolation (off/smooth/ultra), F5 upscaling quality, F6
  tone smoothing (deband), F3 denoise (composes with interpolation), F2 stretch-to-fill,
  F1 reset, F9 status; every toggle answers on the OSD. The combo-profiles
  (`interpolate-basic-stretched` et al) are gone — four dimensions would have needed 24
  of them. Also fixes the **ultra seek-desync** (couch find: skipping around while
  optical-flow interpolation is active desynced audio *without* the A-V counter
  noticing): the script drops and re-applies the filter around every seek, verified over
  IPC (ultra stays 82.54 fps across seeks).
- **Deck switcher — iOS-style app cards** (`switcher.rs`/`watchdog.rs` + `+page.svelte`):
  a Guide tap (or Ctrl+Alt+Home) now opens a row of cards, one per running app — pick one
  to bring it forward, **Select** (or the card's ✕) to close it, **B/Guide** to dismiss.
  Replaces the old blind "toggle to the most-recent app". The backend hides every app when
  the deck opens (so its overlay shows) and maps just the chosen one; Guide-**hold** still
  closes everything. Verified end-to-end in the nested harness (`pad-deck`/`pad-pick`).
- **Custom wallpaper is downscaled once, not decoded huge every launch** (`background.rs`):
  a big photo (the couch-test host's was 4000x3000 / 3.9 MB) was loaded as a base64
  `data:` URL — a ~5 MB DOM string plus a 12 MP main-thread decode — which stalled the
  dashboard to 12-18 fps at startup. It's now resized to display size once, cached under
  `~/.cache/omnideck/bg/`, and served over `omnideck://` (measured 4 MB → 761 KB,
  2560x1920). Falls back to the old full-image path if a source can't be prepared.
  `omnideck bgprep <path>` reports the result.
- **navpad — the controller drives launched apps** (`navpad.rs`): a virtual
  keyboard/mouse over `/dev/uinput`, active only while a launched app's window is in
  front (the switcher's visibility ground truth). Dpad/left stick → arrow-key pulses
  with 400 ms/90 ms console repeat, A → Enter, B → Esc, X → Space, right stick → mouse
  pointer (squared response), R2/L2 → left/right mouse button (hold = drag/long-press),
  L1/R1 → scroll wheel. Kernel-level delivery, so it works for any client — Chromium,
  Firefox, mpv, Qt — with zero per-app integration. Everything held is auto-released if
  the app vanishes mid-press. Requires membership in the `input` group; without it the
  bridge logs once and stays off.
- **Silent hidden apps are frozen**: the switcher still keeps hidden apps *running* when
  they're audibly playing (background music stays a feature — checked via the PipeWire
  pulse shim, uncorked streams matched to the launch process group), but silent hidden
  groups get SIGSTOP and are SIGCONTed on re-show; `return_home` CONTs before TERM so
  Guide-hold close works on frozen apps. Root cause of the 2026-07-09 couch finding:
  a hidden software-rendering PWA kept drawing ~300 W behind the dashboard.
- **Browsers pinned to Xwayland in-session** (`--ozone-platform=x11` for
  Chromium-family): gamescope exports a Wayland socket, and a browser that picks it
  escapes every piece of session machinery (switcher unmap/map, `_NET_WM_PID`
  ownership, navpad focus). Firefox is already pinned via inherited `GDK_BACKEND=x11`.
- **Auto-tuned mpv playback profiles** (`media_profiles.rs`): with a VapourSynth-enabled
  mpv, direct-play now auto-generates and `--include=`s a display-aware profile set under
  `~/.config/omnideck/mpv-profiles/` — GPU upscale/tone-map/deband (`high-quality` +
  `vo=gpu-next`) with F-key–switchable motion interpolation (F4 basic targets the panel's
  full refresh rate; F6 ultra targets display/2 above 100 Hz AND a per-CPU pixel-rate
  budget of `threads × 12 Mpx/s` — both empirically anchored: full-rate optical flow
  desyncs on a 14700K at 1080p→165, and a 4K source→60 measured 13.5 of 16 cores on a
  7800X3D with easy synthetic motion, so ultra lowers or declines over-budget targets
  instead of drifting; `packaging/bench-profiles.sh` reproduces the measurements on any
  host). The session's real mode (RandR ground truth, e.g. 2560x1440@165)
  is baked into the scripts, because mpv injects `display_fps=0` at filter init and does
  not forward `--display-fps-override` into VapourSynth — this is what un-sticks
  interpolation from the 60 fps fallback on high-refresh panels. Rendered files keep a
  `# omnideck-generated` header; strip it and OmniDeck never rewrites that file. Opt out
  with `[media_server] auto_profiles = false` (or set `mpv_args`, which always wins).
  `omnideck mpvprofiles` renders + reports the set; `packaging/test-profiles.sh`
  validates each filter's output rate headlessly. media_play additionally passes
  `--display-fps-override` from the session mode so mpv's `display-resample` pacing is
  deterministic too. Two `[media_server]` knobs tune the generated set: `display_fps`
  (Hz) bakes an explicit panel rate for daily use *outside* the session — where the RandR
  probe is unavailable and the profiles would otherwise fall back to 60 — and is also
  passed as `--display-fps-override`; `audio_samplerate` (Hz) forces mpv's output rate
  (e.g. `96000` for a fixed-rate DAC / LDAC), left native (bit-perfect) when unset. Both
  default to 0 = off, so nothing changes for configs that don't set them.
- **Jellyfin media library — "play your own 4K media", delivered** (Appendix B of the
  2026-07 review): a **Media Library** tile in Movies & TV opens an in-app browser —
  Continue Watching (with resume %), Latest, and your libraries, drilling
  series → seasons → episodes — and plays through **mpv with a direct stream**
  (hardware decode, no transcode, no browser — the exact `--hwdec` depends on the profile
  path above), wired into the existing watchdog so
  Guide-close and Now Playing just work. Posters are fetched lazily, sniffed, cached
  (100 MiB, oldest-evicted) and served over the rooted `omnideck://` protocol. Configure
  via `[media_server]` in config.toml — or don't: an existing **jellyfin-mpv-shim pairing
  is adopted automatically** (server + token), so a shim user gets a working library with
  zero setup. The token never reaches the webview or the logs. `omnideck mediasrv`
  probes the whole path (sections, browse, poster, one byte of the stream) headlessly.
  `[media_server] mpv_args` passes extra flags to the direct-play mpv (e.g. `--include=`
  an existing jellyfin-mpv-shim profile set for VapourSynth interpolation/denoise); when
  set, OmniDeck's own `--hwdec` default steps aside so the profile's `auto-copy`
  (required by VapourSynth filters) isn't overridden from the command line.
- **AUR packaging, validated in CI**: corrected `PKGBUILD` (release tarball + `b2sums`,
  full hicolor icon set, `.install` post-install hint, `StartupWMClass`, `options=(!lto)`),
  committed `.SRCINFO`, and a `packaging.yml` workflow that lints (`namcap`), checks
  `.SRCINFO` sync, and builds the package in a clean Arch container.
- **Supply-chain CI**: `cargo-deny` (advisories / licenses / bans / sources, `deny.toml`)
  and `cargo-audit` (RustSec) jobs; a version-sync job keeps all five version sources
  agreeing (`Cargo.toml`, `tauri.conf.json`, `PKGBUILD`, `package.json`, `Cargo.lock`).
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
  media | mediasrv | mpvprofiles`, plus `--help`/`--version`; unknown flags are rejected
  instead of ignored.
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
- **Live wallpaper — the wave** (Settings → Background → Live wallpaper): PSP-style
  accent-tinted ribbons drifting under the rail. Half-resolution canvas at ~24 fps,
  paused while hidden, a single static frame under `prefers-reduced-motion`. Default on;
  one toggle off.
- **Ambient music** (Settings → Sound → Ambient music, off by default): a synthesized
  slowly-breathing pad — four soft partials over a root that glides between neighbouring
  keys every ~35 s behind a sweeping lowpass. No audio assets, whisper-quiet by design,
  volume row appears when enabled.
- **Session display-mode ground truth**: at session startup the app logs the mode
  gamescope actually set (via its Xwayland RandR) — `session display mode: 2560x1440 @
  165 Hz` — because the UI fps meter cannot prove it: WebKitGTK's software-compositing
  frame clock paces rAF at ~60 regardless of the panel (the meter's 100/240 "highs" are
  burst-frame noise). The generated session launcher also keeps gamescope's own output in
  `$XDG_STATE_HOME/omnideck/gamescope-session.log` (one previous session retained);
  re-run `install-session.sh` to pick that up.
- **Automated session pre-flight** (`packaging/test-session.sh`): boots OmniDeck in a
  *nested* gamescope on the desktop and drives the real input paths end to end — first
  paint, `Ctrl+Alt+Home/End` chords (X grabs), and the gamepad Guide short-press/hold via
  a virtual uinput pad (`examples/virtual-pad.rs`) — so switcher/hotkey regressions are
  caught without logging out. Uses an env-gated FIFO test hook (`OMNIDECK_TEST_CONTROL`,
  inert in production) to launch a deterministic stub client (`examples/x11-stub.rs`)
  through the real watchdog-owned launch path. Bare metal still owns: display mode,
  real Steam launch/return, suspend, SDDM login (see `M2-SESSION-TEST.md` §0.5).

### Changed
- **One input router instead of three, and `+page.svelte` is being decomposed**: the three
  parallel input paths (keyboard `if`-chain, gamepad `if`-chain, stick handling) collapse
  into a single ordered `OVERLAYS` roster where each entry declares `open()` plus its
  `key`/`pad`/`stickX`/`stickY` handlers, and `anyModal` derives from the roster instead of a
  hand-maintained 11-term boolean that had to be kept in sync in three places. Alongside it,
  the deck switcher (`DeckSwitcher.svelte`), the Jellyfin browse state
  (`medianav.svelte.ts`, a `MediaNav` class) and the add-launcher form
  (`LauncherForm.svelte`) moved out of the page, which keeps what is genuinely page-level:
  input routing, the status toast, the Now Playing cards, and error reporting.
- **The Settings column is table-driven**: four string-key dispatchers (read / adjust / cycle
  / numeric-and-text setters) collapse into one pass over a typed `SettingDef` table
  (`settings-defs.ts`). Behaviour-preserving — same display values, same steps, same
  visibility rules — but a new setting is now a table row instead of an edit in four
  `switch` statements.
- **App icons load in a window, like game art**: icon fetching fired for every app tile at
  mount (O(library)); it now follows visibility. Focus clamps became `$derived` instead of
  write-back `$effect`s, removing a class of effect-ordering surprise, and the
  custom-launcher command box splits argv **quote-aware** (`argv.ts`) instead of on
  whitespace — a quoted path with spaces now survives, and an unbalanced quote is an error
  rather than a mangled launch.
- **The switcher shares one pooled X11 connection** across its entry points: navpad polls
  "is a launched app visible?" ~3×/s for the whole session, and each poll used to pay a fresh
  connect and auth handshake. A cheap liveness round-trip on reuse reconnects transparently
  if the server went away. The hotkey thread deliberately keeps its own connection — it parks
  in `wait_for_event`, which would wedge anything sharing it.
- **Configuring Jellyfin takes effect without a restart**: the resolved server moved from a
  process-lifetime `OnceLock` to an invalidatable cache, invalidated after every config save
  — including a restored backup that changes `[media_server]`. Resolution stays lazy. The
  config file also now carries a `config_version` stamp, so a future shape change has a
  migration hook to hang off.
- **Config saves and the deck/media/background commands run off the UI thread**: a shared
  `blocking()` helper carries their fsyncs/blocking work on the async runtime's blocking
  pool instead of the main thread. Media-library sections now fetch concurrently
  (`tokio::join!`, ~4 serial LAN round-trips → ~1). The hardware capability probe is
  memoized (it scanned `/dev/dri`/PCI/Vulkan ICDs/`PATH` on every `media_play`/launch/boot
  call; now once per process).
- The ts-rs binding-drift check now also catches newly-*added* generated files, not just
  changed ones — closes the gap that let a binding ship uncommitted while CI stayed green.
- Frontend styling now resolves through a small `surface`/`text`/`border` CSS
  custom-property token system instead of raw hex repeated across components (34
  references, 5 components) — a pure refactor; rendering is byte-identical today.
- The app window/title now says "OmniDeck" (was the SvelteKit starter default), with dark
  `color-scheme`/`theme-color` so shell chrome matches the UI.
- **Controller click is A/cross, not R2** (navpad): the right stick is the primary pointer,
  so A now left-clicks where it is (what the user expects). Enter moved to X, play/pause to
  Y. R2/L2 still click too (for hold/drag).
- **Hidden apps only stay running while audibly playing** — the switcher's silence check now
  matches an audio stream to its launch app by process *ancestry*, not exact group, so an
  Electron app's `setsid`'d audio child is found; Feishin no longer gets frozen mid-song.
- **Browsers in-session get `--force-device-scale-factor=1`** so a Chromium PWA fills the
  panel instead of rendering into a corner/half (Xwayland HiDPI auto-scale — the
  couch-test "PWA on the left half").
- **`OMNIDECK_WEBKIT_DMABUF=1` escape hatch** (gpu.rs): keeps WebKitGTK's zero-copy dmabuf
  renderer ON on NVIDIA instead of the blank-screen workaround that also caps smoothness
  (~78 fps). Opt-in per driver — the fast path to a truly 165 Hz dashboard where a newer
  driver renders it correctly.
- **PSP-clean chrome pass**: the footer hint wall is gone — diagnostics left, three hints
  right, and the full keyboard/controller reference lives in a **Help overlay** (`?` /
  `F1`, footer button; the wizard mentions it). **Settings** is grouped into sections
  (Appearance / Background / Home & Library / Sound / Search / Launchers) with header
  rows the navigation skips. **Search** dims the on-screen keyboard while a physical
  keyboard is typing (back on D-pad touch) and says when only the web row is left.
  **Emoji chrome is gone**: category rail, header buttons, and the power menu use a
  monochrome stroke-glyph set (`$lib/icons.ts`); app tiles keep their fetched brand
  icons. Modals cap at 92 vh and scroll instead of overflowing at large UI scales.
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

### Fixed
- **Launch failures say what actually went wrong**: raw OS errors from `spawn()` are mapped
  at the spawn site into actionable messages ("not found on `PATH`", permission denied, …)
  instead of surfacing an errno. Done as error mapping rather than a `PATH` pre-flight on
  purpose — the pre-flight was racy (the binary can vanish between check and spawn) and the
  mapped version is both simpler and correct.
- **`config.toml` writes are now atomic and serialized**: a crash/power-loss/full-disk
  mid-write could leave a truncated file — and because the loader deliberately refuses to
  overwrite an unparseable config, one interrupted write used to wedge *all* future saves
  (UI stuck on defaults) until hand-fixed. Writes now go to a temp sibling, `fsync`, then
  rename over the destination, with a process-wide lock serializing every load→mutate→save.
- **The Now Playing (MPRIS) watcher supervises and reconnects**: it used to connect once
  and never recover, so a `dbus-daemon`/player/session restart left a frozen Now Playing
  card and dead media controls forever. It's now a supervisor loop with bounded backoff
  (1s→5s→15s) that clears the card on disconnect and reconnects automatically.
- **Jellyfin client reliability**: a `/Users/Me` response missing an `Id` used to cache
  `None` for the process lifetime, wedging all media until restart — the user-id cache now
  only ever stores a success. Transient network errors get one retry instead of turning a
  section into a spurious empty row; Continue Watching/Latest failures are logged instead
  of looking identical to an empty library.
- **navpad backs off and disables itself** after 20 consecutive `/dev/uinput` write
  failures instead of logging a warning on every ~8 ms gamepad tick and flooding the
  session log; a later success logs recovery and resets the counter.
- **Now Playing cards get a unique id per launch**: relaunching an app or game while an
  earlier instance was still exiting used to give both the same identity, so the older
  process's exit event cleared the newer card too.
- **Two `omnideck-toggles.lua` bugs**: the status OSD ignored its caller's requested
  duration (always 1.6 s instead of the intended 3 s), and toggling interpolation during
  the seek self-heal window could double-append the filter label.
- **Deck-switcher ordering bugs** (found in an xhigh-effort review of the full diff):
  `deck_cancel()` now connects to X *before* consuming the restore snapshot (a failed
  connect no longer strands the foreground app unmapped); `show_group()` now maps windows
  *before* thawing them (a total map failure now leaves the group frozen and recoverable
  instead of running invisibly); `switch_app` with a stale launch id is now a no-op instead
  of falling back to the surface-every-hidden-app toggle; closing a group now forgets its
  stale `STOPPED` bookkeeping entry.
- **Every interactive shell-out is bounded** (`proc.rs`, new): the `pactl` audibility probe
  and first-play mpv capability probe now run under a timeout with concurrent stdout
  draining (no `>64 KiB` pipe deadlock) and are reaped on every kill path (no zombie
  processes left behind).
- **MPRIS `control()` parses the verb into an enum once** instead of a `_ => previous()`
  catch-all that could silently map a future/unrecognized verb to Previous.
- **Guide-hold / Ctrl+Alt+End now closes EVERY running launched app** (2026-07 audit):
  it only signalled the most-recently-launched one, so with app B stacked over a
  still-running app A, "close" left A holding the screen. Deliberate semantics: close is
  the escape hatch, the switcher is how you keep apps alive.
- **A poisoned internal mutex now logs a warning and recovers** instead of silently
  no-op'ing every later critical section; `flatpak list` is cached per run instead of
  shelling out on every app scan; ~10 inlined session-detection env checks collapsed
  into one `session::in_session()` (all 2026-07 audit).
- **Left-stick Y was inverted** (M2, DualShock 4): gilrs's convention is positive Y =
  stick up; the UI consumed it unnegated, so up moved down. Now negated exactly once, and
  the harness asserts the convention end to end through a virtual pad (raw `ABS_Y` min →
  `LeftStickY +1` → focus up).
- **The stick now navigates the search / add-apps / power dialogs** (rows), instead of
  being swallowed by every modal; the D-pad keeps its modal-specific role (the on-screen
  keyboard in search). Bumpers still page the search results.
- **Guide hold closes at the 800 ms threshold — while the button is still down** —
  instead of waiting for release (release-time close felt laggy and unconfirmed on
  hardware). A release already in the event queue still wins, so a ~790 ms press can't
  misfire as a hold.
- **The Jellyfin tile no longer launches `jellyfin-mpv-shim`** (a background cast target
  with no UI — the tile appeared to do nothing). It now opens the desktop client when
  installed, else the server's web client as a PWA, reading the server address from the
  shim's own pairing config.
- **App switcher hide/show is now verified, not fire-and-forget**: map/unmap of a
  launched app's windows goes through gamescope's compositor asynchronously, and a
  request landing while it digests the previous transition could be swallowed — stranding
  the app invisible with the switcher thinking nothing was hidden (Guide did nothing from
  then on). The switcher now confirms each transition and retries, and keeps unconfirmed
  windows in the hidden set so the next toggle recovers them. Found by the nested-session
  harness (~1 in 3 runs); on hardware it would have looked like "the app randomly never
  comes back".
- **`GDK_BACKEND=x11` is pinned inside gamescope sessions**: the atom/switcher/hotkey
  machinery manages OmniDeck's window through X, but GTK connects to any Wayland socket
  it sees (a leaked parent compositor socket under nested gamescope; potentially a future
  gamescope exporting its own) — putting the window where none of that machinery can
  reach it. Desktop Wayland is untouched (the backend stays unforced there).
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

### Security
- **The mpv direct-play stream authenticates with an `X-Emby-Token` header, not an `api_key`
  query parameter**: the token no longer appears on URL-shaped surfaces — mpv's log, OSD and
  IPC socket, its watch-later state, or the server's HTTP access log.
- **The PIN hash never crosses IPC**: `pin_hash` is masked behind a `has_pin: Option<bool>`
  presence flag that is cleared before any disk write, and `locked_categories` is writable
  only through the PIN-verified `set_locked_categories` — a plain settings save can no longer
  empty the locked list to walk around the PIN.
- **The artwork fetch can't become an open proxy**: `get_artwork` fetches with the media
  server's token attached, so its URL argument is gated by `url_within_base` before anything
  leaves the machine — a compromised webview naming a lookalike host gets nothing (tested).
- **The audio switcher only accepts sinks it enumerated itself**: `audio_set_output` matches
  its argument against our own sink list before it reaches `pactl`, which is invoked
  argv-only and never through a shell, so a crafted frontend value has nowhere to go
  (unit-tested with an injection-shaped id).
- **Credentials stay out of backups and off the wire**: config backups exclude the
  media-server token and the SteamGridDB key by default (a backup is meant to travel), and a
  restore carrying none keeps the credentials already on the machine. The phone remote's
  token is masked over IPC, stripped from backups the same way, compared in constant time
  (length differences fold into the accumulator — no early return), and never logged.
- **`argon2` added as a runtime dependency** (`pin.rs`) — deliberately: a PIN gate is worth a
  vetted password hash rather than a hand-rolled one. It is the only new runtime dep in this
  release; the phone remote and its HTTP server added none.
- **Jellyfin media/parent ids are validated at the IPC boundary** (`browse()`, `poster()`,
  `media_play()`): alphanumeric + hyphen, bounded — rejecting `../`, `&`, `/` injection from
  an arbitrary frontend-supplied string before it's interpolated into a URL path or query.
- `anyhow` bumped 1.0.102 → 1.0.103 (RUSTSEC unsoundness advisory).
- **DNS-rebinding closed in the SSRF guard** (2026-07 audit): the blocklist now also
  resolves hostnames and re-checks every returned address (IPv6 ranges included), at the
  fetch entry points and on every redirect hop — a public-looking domain that resolves to
  `127.0.0.1`/`10.x` no longer walks past the literal-IP check. Groundwork for the
  planned LAN media-server integration.
- **`get_art` (custom background) is content-gated** (2026-07 audit): canonicalized,
  regular-files-only, and magic-byte sniffed against the claimed image type — a
  crafted/imported config can no longer feed a non-image through the background loader.
  Deliberately NOT path-rooted: backgrounds legitimately live on photo mounts, and the
  surface is display-only (no exfil channel under the CSP).
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

## [0.1.0] — first tagged release
Initial public snapshot: XMB-style controller-first launcher, Steam library scan +
launch with exit watchdog, curated app/media catalog with favicon fetching, SteamGridDB
box art, MPRIS Now Playing, capability probe (gamescope session / desktop / kiosk
tiers), gamescope session installer, hand-editable `config.toml`.
