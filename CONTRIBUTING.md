# Contributing to OmniDeck

Thanks for your interest! OmniDeck aims to be the go-to living-room launcher for Linux —
easy for everyone, deep for tinkerers. Contributions of all kinds are welcome.

## Dev setup

```bash
bun install
bun run tauri dev
```

Requirements: `webkit2gtk-4.1`, Rust 1.80+, Node 20+ or Bun. On Arch:
`sudo pacman -S webkit2gtk-4.1 base-devel`.

## Project layout

The 10,000-foot map — how the pieces fit, the IPC/bindings contract, the input paths, the
session-vs-desktop fork — lives in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). The short
version:

```
src/routes/+page.svelte   # the UI (grid, nav, prefs, add-apps) — Svelte 5 runes
src/lib/                  # UI components + helpers (backend.ts = typed IPC, nav.ts, …)
src-tauri/src/
  lib.rs          # app wiring: registers Tauri commands + spawns the setup tasks (~100 lines)
  commands.rs     # the #[tauri::command] handlers the frontend calls
  cli.rs          # headless subcommands (clap): probe|scan|config|catalog|media|…
  capability.rs   # GPU/tier detection (gamescope session | cage kiosk | window)
  gpu.rs          # NVIDIA/WebKit env + the #[cfg(unix)] GPU re-exec; RandR display mode
  library.rs      # Steam VDF/ACF library scan + local art resolution
  apps.rs         # app/media tile catalog (detected native/flatpak + browser entries)
  config.rs       # ~/.config/omnideck/config.toml atomic load/save
  asset.rs        # the rooted omnideck:// image protocol (art/posters/wallpaper cache)
  gamepad.rs      # gilrs input thread; navpad.rs = pad → uinput keyboard/mouse bridge
  switcher.rs / watchdog.rs   # session app switcher/deck + launch tracking & close
  hotkey.rs       # global X chords in the gamescope session (Ctrl+Alt+Home/End)
  media_server.rs # Jellyfin browse/play; media_profiles.rs = generated mpv/VapourSynth set
  mpris.rs        # event-driven Now Playing (zbus) with a reconnect supervisor
  background.rs   # custom-wallpaper downscale cache; steamgriddb.rs = optional box-art
  http.rs         # shared reqwest client + SSRF blocklist; logging.rs / sync.rs = infra
```

Handy headless commands for debugging (no window opens):

```bash
omnideck probe     # detected GPU + capability tier
omnideck scan      # Steam library scan result
omnideck config    # effective config + path
omnideck catalog   # media/app catalog (what's detected/offered)
omnideck gridart <appid>   # test a SteamGridDB fetch
omnideck media     # MPRIS players on the session bus (what Now Playing shows)
omnideck --help    # all subcommands; --version for the version
```

## Code style

- Rust: `cargo clippy` clean before submitting. The tree is hand-formatted in a compact
  style (not `cargo fmt` output) and CI does not run rustfmt — match the surrounding code
  rather than reformatting whole files, which would bury real changes in style churn.
- Keep platform-specific code isolated (see `capability.rs`, the `#[cfg(unix)]` GPU
  re-exec) so a future Windows/macOS target stays cheap.
- Prefer detection over hardcoding (e.g. catalog apps only appear if installed).

## Good places to help

- **Hardware testing** — especially AMD/Intel GPUs and the no-GPU `cage` tier (the dev
  fleet is all-NVIDIA, so cross-GPU testing is gold).
- **Native/flatpak catalog** — verified Flathub IDs for more media/music apps.
- **More sources** — Heroic games (not just the app), Lutris, emulators (RetroArch/ES).
- **The gamescope session** — session files, the `STEAM_GAME` atom, the exit watchdog.
- **Packaging** — AUR, Flatpak (Flathub), AppImage.
- **Onboarding** — a friendly first-run wizard.

## Licensing of contributions

By contributing, you agree your contributions are licensed under **GPL-3.0-or-later**, the
project's license.
