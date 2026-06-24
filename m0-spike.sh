#!/usr/bin/env bash
# OmniDeck dev launcher. The binary now self-configures its GPU/webview env
# (see ensure_gpu_env in src-tauri/src/lib.rs), so this just picks where to run it.
#
#   ./m0-spike.sh desktop     # windowed on your desktop (Tier 3)
#   ./m0-spike.sh gamescope   # nested gamescope at your display's res/refresh
#
# Override the nested size/refresh for your monitor:  W=3840 H=2160 R=120 ./m0-spike.sh gamescope
set -u
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="$HERE/src-tauri/target/release/omnideck"
[ -x "$BIN" ] || { echo "!! build first: cd '$HERE' && bun run tauri build --no-bundle"; exit 1; }

MODE="${1:-gamescope}"
W="${W:-2560}"; H="${H:-1440}"; R="${R:-165}"   # defaults to a 2K 165Hz panel

case "$MODE" in
  desktop)
    echo ">> OmniDeck on the desktop (Tier 3)"
    exec "$BIN" ;;
  gamescope)
    command -v gamescope >/dev/null || { echo "!! gamescope not found"; exit 1; }
    echo ">> nested gamescope (SDL backend, ${W}x${H}@${R}Hz). Ctrl-C or close to exit."
    exec gamescope --backend sdl -W "$W" -H "$H" -r "$R" -- "$BIN" ;;
  *)
    echo "usage: $0 [desktop|gamescope]   (env W/H/R override the nested size)"; exit 2 ;;
esac
