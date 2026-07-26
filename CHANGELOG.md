# Changelog

All notable changes to OmniDeck are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[SemVer](https://semver.org/) (pre-1.0: minor bumps may break).

## [0.2.0] — 2026-07-27

### Added
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
