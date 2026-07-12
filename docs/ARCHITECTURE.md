# OmniDeck architecture

A one-file map of how the pieces fit. For setup and usage see the
[README](../README.md); for contribution mechanics see
[CONTRIBUTING](../CONTRIBUTING.md). Code comments are the source of truth for
module-level detail — this document is the 10,000-foot view that tells you which file to
open.

## The shape of the app

OmniDeck is a Tauri v2 app: one Rust process hosting a WebKitGTK webview that renders a
SvelteKit (Svelte 5 runes, SPA/adapter-static) frontend. There is no server and no SSR —
the webview loads the built `build/` site and everything dynamic crosses the IPC boundary
as Tauri commands or events.

```
src/                       frontend (SvelteKit, Svelte 5 runes)
  routes/+page.svelte      the launcher UI: XMB rail, modals, unified input router
  lib/backend.ts           typed IPC layer — the single Rust↔JS contract
  lib/bindings/            ts-rs GENERATED types (never hand-edit; see below)
  lib/*.svelte             extracted presentational components (modals, cards, waves)
src-tauri/src/             backend (Rust)
  lib.rs                   entrypoint: CLI dispatch, GPU env re-exec, Tauri builder
  commands.rs              the #[tauri::command] IPC surface (thin; dispatches to modules)
  cli.rs                   headless debug subcommands (probe/scan/config/media/…)
  ...domain modules        see the module map in lib.rs
src-tauri/profiles/        embedded mpv/VapourSynth profile templates (media_profiles.rs)
packaging/                 AUR PKGBUILD, gamescope session files, install scripts
```

## The IPC contract

`src/lib/backend.ts` is the only place the frontend calls `invoke()`; every command has a
typed wrapper there. The TypeScript types for anything a Rust struct sends over IPC are
**generated** by [ts-rs] into `src/lib/bindings/` — a field rename on the Rust side fails
the frontend build instead of silently becoming `undefined`. Regenerate after changing a
`ts(export)` struct:

```bash
cd src-tauri && TS_RS_EXPORT_DIR=$PWD/../src/lib/bindings cargo test export_bindings
```

CI fails if committed bindings drift from the Rust structs. Events flow the other way
(backend → frontend) as Tauri events with `on*` wrappers in the same file (`gamepad-event`,
`media-changed`, `app-exited`, `guide-tap`, …).

## Config is king

`~/.config/omnideck/config.toml` (config.rs) is hand-editable and owns all curated state:
settings, app tiles, favorites, recents, the media-server pairing. Two invariants:

- **Never clobber.** A parse error falls back to defaults *in memory* and flags
  `config_error` for the UI; every save path refuses to write while that flag is set, so an
  automatic save (recents fire on every launch) can never replace a user's broken-but-
  fixable file with defaults.
- **Normalize on load.** Every table gets a `normalize()` pass — numerics clamped, colors/
  URLs/enums pattern-checked — so a hand-edited (or maliciously crafted/imported) value
  can't reach CSS, a browser launch, or a spawn without passing the same gates.

## Input: three paths into one router

- **Gamepad** (gamepad.rs): gilrs reads evdev on a dedicated thread (gilrs is `!Send`) and
  forwards coalesced, typed events to the webview. The Guide button is handled *in the
  thread* (tap = deck switcher, hold = close current app) because evdev keeps working while
  a launched app owns window focus.
- **Keyboard** in OmniDeck: ordinary DOM events, routed by the page's unified input router
  in `+page.svelte`.
- **Escape hatches while an app is focused**: hotkey.rs grabs Ctrl+Alt+Home/End on the X
  root window (grabs resolve before focus delivery), and navpad.rs bridges the pad into a
  virtual uinput keyboard/mouse so the controller drives the *launched* app.

## Session vs desktop

`session::in_session()` (gamescope detection) is the biggest behavior fork: fullscreen the
webview, enable the hotkey grabs and navpad bridge, probe the display mode over RandR,
claim a desktop identity for launched Qt apps. On a plain desktop OmniDeck is just a
window and deliberately does none of that. gpu.rs re-execs the process once at startup
with GPU-appropriate webview env (NVIDIA needs WebKitGTK workarounds; Mesa doesn't).

## Launching and tracking apps

commands.rs spawns tiles (argv-only, `Command::new` — never a shell) in their own process
group; watchdog.rs tracks children (and Steam games via Steam's registry, since
`steam://rungameid` returns immediately) and emits launch/exit events the UI turns into
Now Playing cards. switcher.rs hides/shows launched app windows through X11
(`_NET_WM_PID`-based grouping) for the deck switcher. Browser tiles go through a `BROWSER`
token that resolves to the host browser in PWA mode; only http(s) URLs may follow it
(flag-injection guard).

## Media stack

- mpris.rs: one session-bus watcher pushes Now Playing changes as events — no polling.
- media_server.rs: Jellyfin browse/play (config or an adopted jellyfin-mpv-shim pairing);
  stream URLs are built server-side from item ids, never from frontend input.
- media_profiles.rs: renders an embedded, display-aware mpv profile set (VapourSynth
  interpolation/denoise) into `~/.config/omnideck/mpv-profiles/`, generated-header-owned:
  a file whose header the user removed is theirs and is never rewritten.

## Network policy

All HTTP goes through the shared `http::client()` (http.rs): connect/read/total timeouts
so a hung CDN can't wedge a command, plus an SSRF blocklist (literal, resolver-tricky, and
DNS-rebinding forms) re-checked on every redirect hop. Art fetches (steamgriddb.rs,
icons.rs) and everything else reuse it; the `omnideck://` protocol (asset.rs) serves
on-disk art to the webview without base64 round-trips.

## Logging & diagnostics

logging.rs writes stderr *and* a daily-rotating file under `$XDG_STATE_HOME/omnideck`
(7 kept) — a gamescope session's stderr dies with the compositor, the file survives.
Panics land there too. The CLI (cli.rs) is the headless debug surface; run
`omnideck --help` for the list (`probe`, `scan`, `config`, `media`, `mediasrv`,
`mpvprofiles`, …). No window opens for any subcommand.

## Testing

`cargo test` covers the pure logic (config normalization, URL/arg guards, profile
policy) and doubles as the bindings generator/drift check. `bun run check` +
`bun run build` gate the frontend. A few tests marked `#[ignore]` need real hardware
(an X server, a controller) and are run manually against the target box.

[ts-rs]: https://github.com/Aleph-Alpha/ts-rs
