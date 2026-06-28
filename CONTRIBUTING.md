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

```
src/routes/+page.svelte   # the UI (grid, nav, prefs, add-apps) — Svelte 5 runes
src-tauri/src/
  lib.rs          # Tauri commands, gamepad (gilrs) thread, GPU re-exec, CLI flags
  capability.rs   # GPU/tier detection (gamescope session | cage kiosk | window)
  library.rs      # Steam VDF/ACF library scan + local art resolution
  apps.rs         # app/media tile catalog (detected native/flatpak + browser entries)
  config.rs       # ~/.config/omnideck/config.toml load/save
  steamgriddb.rs  # optional box-art fetch
```

Handy headless commands for debugging (no window opens):

```bash
omnideck probe     # detected GPU + capability tier
omnideck scan      # Steam library scan result
omnideck config    # effective config + path
omnideck catalog   # media/app catalog (what's detected/offered)
omnideck gridart <appid>   # test a SteamGridDB fetch
omnideck --help    # all subcommands; --version for the version
```

## Code style

- Rust: `cargo fmt` and `cargo clippy` clean before submitting.
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
