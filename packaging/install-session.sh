#!/usr/bin/env bash
# Install OmniDeck as a selectable gamescope session in your display manager (SDDM/GDM).
#
# Requires: gamescope + gamescope-session-plus
#   Arch:    paru -S gamescope-session-git
#   Bazzite: already shipped
#
# Usage: ./install-session.sh [/path/to/omnideck-binary]
#   (auto-detects `omnideck` in PATH or a locally built release binary)
set -euo pipefail

# --- locate the OmniDeck binary ---
BIN="${1:-}"
[ -z "$BIN" ] && BIN="$(command -v omnideck 2>/dev/null || true)"
if [ -z "$BIN" ]; then
  here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  cand="$here/src-tauri/target/release/omnideck"
  [ -x "$cand" ] && BIN="$cand"
fi
[ -n "$BIN" ] && [ -x "$BIN" ] || { echo "✗ OmniDeck binary not found. Pass it: ./install-session.sh /path/to/omnideck"; exit 1; }

command -v gamescope-session-plus >/dev/null || {
  echo "✗ gamescope-session-plus not found. Install it (Arch: paru -S gamescope-session-git)."; exit 1; }

# Writable on immutable distros (Bazzite) and on SDDM's session search path elsewhere.
BASE=/usr/local/share
SESSIONS_D="$BASE/gamescope-session-plus/sessions.d"
WAYLAND_SESSIONS="$BASE/wayland-sessions"

echo "Installing OmniDeck gamescope session"
echo "  binary: $BIN"
echo "  into:   $BASE  (needs sudo)"

sudo mkdir -p "$SESSIONS_D" "$WAYLAND_SESSIONS"

# sessions.d entry — gamescope-session-plus sources this and runs CLIENTCMD inside gamescope.
sudo tee "$SESSIONS_D/omnideck" >/dev/null <<EOF
# OmniDeck gamescope session — the UI run as the session's main client.
export CLIENTCMD="$BIN"
EOF

# wayland-sessions entry — makes "OmniDeck" appear in the display-manager session menu.
sudo tee "$WAYLAND_SESSIONS/gamescope-session-omnideck.desktop" >/dev/null <<EOF
[Desktop Entry]
Name=OmniDeck
Comment=OmniDeck 10-foot media & game launcher (gamescope session)
Exec=gamescope-session-plus omnideck
Type=Application
DesktopNames=gamescope
EOF

cat <<'EOF'

✓ Installed. Log out and pick "OmniDeck" from your display manager's session list.

NOTE: gamescope (--steam mode) only shows windows tagged with the STEAM_GAME atom.
OmniDeck attempts to set this itself, but if you get a BLACK SCREEN on first boot,
that's the thing to report — we'll tune the atom handling for your setup.

To remove:  sudo rm /usr/local/share/gamescope-session-plus/sessions.d/omnideck \
                    /usr/local/share/wayland-sessions/gamescope-session-omnideck.desktop
EOF
