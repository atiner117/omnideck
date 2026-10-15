#!/usr/bin/env bash
# Install OmniDeck as a selectable gamescope session in your display manager (SDDM/GDM).
#
# OmniDeck runs as a *plain* gamescope session — gamescope composites OmniDeck's window
# directly and fullscreens it. We deliberately do NOT use `gamescope-session-plus`/`--steam`
# mode: that path only displays a window it accepts as a Steam game, so a normal app like
# OmniDeck just produces a black screen (gamescope has nothing focusable to show).
#
# Requires: gamescope   (Arch: pacman -S gamescope)
#
# Usage: ./install-session.sh [/path/to/omnideck-binary]
#          (auto-detects `omnideck` in PATH or a locally built release binary)
#        ./install-session.sh --print-session-dirs
#          print SDDM's effective Wayland SessionDir list and where the session entry
#          would go, then exit — no root, no binary or gamescope needed.
set -euo pipefail

# --- where the display manager looks for sessions ---------------------------------------
# The session entry always goes to /usr/local/share/wayland-sessions: GDM (via XDG_DATA_DIRS)
# and SDDM's built-in default scan it on every distro, and unlike /usr/share it stays writable
# on immutable distros (Bazzite/SteamOS), where /usr/share/wayland-sessions exists but is
# read-only.
#
# SDDM's list is configurable, though ([Wayland] SessionDir=, comma-separated), and distros
# override it: CachyOS ships /usr/lib/sddm/sddm.conf.d/general.conf with only
# /usr/share/wayland-sessions, so an entry in /usr/local is invisible to the greeter and
# autologin fails with "Unable to find autologin session entry". So: resolve the EFFECTIVE
# list the way SDDM does, and when it doesn't include the canonical dir, also symlink the
# entry into the first listed dir that accepts it. (`sddm --example-config` prints the
# compiled-in defaults, not the effective value, so it can't be used for this.)
CANONICAL_SESSION_DIR=/usr/local/share/wayland-sessions
SDDM_DEFAULT_SESSION_DIRS="/usr/local/share/wayland-sessions,/usr/share/wayland-sessions"
# Test hook: prefix for SDDM's config paths, so --print-session-dirs can run on fixtures.
SDDM_ROOT="${OMNIDECK_SDDM_ROOT:-}"

# Trim leading/trailing whitespace from $1 into REPLY (no subshell per config line).
_trim() {
  local s="$1"
  s="${s#"${s%%[![:space:]]*}"}"
  s="${s%"${s##*[![:space:]]}"}"
  REPLY="$s"
}

# SDDM's config files in the order it reads them; later files override earlier ones per key:
# /usr/lib/sddm/sddm.conf.d/*.conf, then /etc/sddm.conf.d/*.conf (each sorted by name),
# then /etc/sddm.conf.
sddm_conf_files() {
  local LC_ALL=C f # byte-order glob sort, like SDDM's QDir name sort
  for f in "$SDDM_ROOT"/usr/lib/sddm/sddm.conf.d/*.conf "$SDDM_ROOT"/etc/sddm.conf.d/*.conf \
           "$SDDM_ROOT"/etc/sddm.conf; do
    [ -f "$f" ] && [ -r "$f" ] && printf '%s\n' "$f"
  done
  return 0
}

# Print the last `SessionDir=` value from a `[Wayland]` section across the given files
# (last wins), or nothing when none sets it. Ignores comments and every other section
# (notably [X11] SessionDir).
sddm_wayland_session_dir_value() {
  local f line section key value="" found=0
  for f in "$@"; do
    section=""
    while IFS= read -r line || [ -n "$line" ]; do
      line="${line%$'\r'}"
      _trim "$line"; line="$REPLY"
      case "$line" in
        '' | '#'* | ';'*) continue ;;
        '['*']') _trim "${line:1:${#line}-2}"; section="$REPLY"; continue ;;
      esac
      [ "$section" = Wayland ] || continue
      case "$line" in *=*) ;; *) continue ;; esac
      _trim "${line%%=*}"; key="$REPLY"
      [ "$key" = SessionDir ] || continue
      _trim "${line#*=}"; value="$REPLY"; found=1
    done <"$f"
  done
  [ "$found" = 1 ] && printf '%s' "$value"
  return 0
}

# SDDM's effective Wayland session dirs, one per line, in its search order. Falls back to
# SDDM's built-in default when no config sets the key (or sets it empty).
sddm_session_dirs() {
  local files=() raw entry seen=" "
  mapfile -t files < <(sddm_conf_files)
  raw=""
  [ "${#files[@]}" -gt 0 ] && raw="$(sddm_wayland_session_dir_value "${files[@]}")"
  _trim "$raw"; raw="$REPLY"
  [ -n "$raw" ] || raw="$SDDM_DEFAULT_SESSION_DIRS"
  local IFS=,
  for entry in $raw; do
    _trim "$entry"; entry="$REPLY"
    entry="${entry#\"}"; entry="${entry%\"}"
    while [ "${#entry}" -gt 1 ] && [ "${entry%/}" != "$entry" ]; do entry="${entry%/}"; done
    [ -n "$entry" ] || continue
    case "$seen" in *" $entry "*) continue ;; esac
    seen="$seen$entry "
    printf '%s\n' "$entry"
  done
}

# SDDM dirs the entry must ALSO be linked into: none when SDDM already scans the canonical
# dir, otherwise its list in order (the installer uses the first one that accepts the link).
session_link_candidates() {
  local dirs=() d
  mapfile -t dirs < <(sddm_session_dirs)
  for d in "${dirs[@]}"; do
    [ "$d" = "$CANONICAL_SESSION_DIR" ] && return 0
  done
  printf '%s\n' "${dirs[@]}"
}

print_session_dirs() {
  local f d links=()
  while IFS= read -r f; do printf 'config      %s\n' "$f"; done < <(sddm_conf_files)
  while IFS= read -r d; do printf 'sessiondir  %s\n' "$d"; done < <(sddm_session_dirs)
  printf 'install     %s/omnideck.desktop\n' "$CANONICAL_SESSION_DIR"
  mapfile -t links < <(session_link_candidates)
  for d in "${links[@]}"; do printf 'link        %s/omnideck.desktop\n' "$d"; done
  return 0
}

case "${1:-}" in
  --print-session-dirs) print_session_dirs; exit 0 ;;
  -h | --help) sed -n '2,/^set -euo/{/^set -euo/d;s/^# \{0,1\}//;p}' "${BASH_SOURCE[0]}"; exit 0 ;;
  -*) echo "✗ unknown option: $1 (try --help)" >&2; exit 2 ;;
esac

# --- locate the OmniDeck binary ---
BIN="${1:-}"
[ -z "$BIN" ] && BIN="$(command -v omnideck 2>/dev/null || true)"
if [ -z "$BIN" ]; then
  here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  cand="$here/src-tauri/target/release/omnideck"
  [ -x "$cand" ] && BIN="$cand"
fi
[ -n "$BIN" ] && [ -x "$BIN" ] || { echo "✗ OmniDeck binary not found. Pass it: ./install-session.sh /path/to/omnideck"; exit 1; }

command -v gamescope >/dev/null || {
  echo "✗ gamescope not found. Install it (Arch: pacman -S gamescope)."; exit 1; }

# Canonical location (see the header above for why /usr/local; SDDM's list is handled below).
WAYLAND_SESSIONS="$CANONICAL_SESSION_DIR"
LAUNCHER=/usr/local/bin/omnideck-session
mapfile -t LINK_CANDIDATES < <(session_link_candidates)

echo "Installing OmniDeck gamescope session"
echo "  binary:   $BIN"
echo "  launcher: $LAUNCHER"
echo "  session:  $WAYLAND_SESSIONS/omnideck.desktop   (needs sudo)"
if [ "${#LINK_CANDIDATES[@]}" -gt 0 ]; then
  echo "  SDDM scans only: ${LINK_CANDIDATES[*]}  (will symlink the entry there too)"
fi

sudo mkdir -p "$WAYLAND_SESSIONS" "$(dirname "$LAUNCHER")"

# Launcher: run gamescope as the session compositor, fullscreen, with OmniDeck as the
# sole client. No --steam. When OmniDeck quits, gamescope exits and you return to the
# greeter. `-f` fullscreens the app; gamescope uses the monitor's native resolution.
sudo tee "$LAUNCHER" >/dev/null <<EOF
#!/usr/bin/env bash
# OmniDeck gamescope session launcher (generated by install-session.sh).
# --force-windows-fullscreen sizes every launched app (incl. the full Brave browser, which
# otherwise opens windowed/pillarboxed) to fill the display.
# --adaptive-sync enables VRR when the display supports it (no-op otherwise).
#
# Without flags gamescope uses the display's EDID-*preferred* mode, which on many gaming
# monitors is 60 Hz even when the panel does 144/165. Put per-host overrides in
#   ~/.config/omnideck/session.conf   (sourced as bash), e.g.:
#     GAMESCOPE_FLAGS="-W 2560 -H 1440 -r 165 -O DP-3"
GAMESCOPE_FLAGS=""
CONF="\${XDG_CONFIG_HOME:-\$HOME/.config}/omnideck/session.conf"
[ -r "\$CONF" ] && . "\$CONF"
# Keep gamescope's own output (mode selection, VRR, Xwayland) — SDDM's session log is
# truncated per login, which left "did -r 165 apply?" unanswerable after the fact.
GSLOG="\${XDG_STATE_HOME:-\$HOME/.local/state}/omnideck/gamescope-session.log"
mkdir -p "\$(dirname "\$GSLOG")"
[ -f "\$GSLOG" ] && mv -f "\$GSLOG" "\$GSLOG.old"   # keep one previous session
# shellcheck disable=SC2086  # word-splitting GAMESCOPE_FLAGS is the point
exec gamescope -f --force-windows-fullscreen --adaptive-sync \\
  --xwayland-count 1 --hide-cursor-delay 3000 \$GAMESCOPE_FLAGS -- "$BIN" > "\$GSLOG" 2>&1
EOF
sudo chmod +x "$LAUNCHER"

# wayland-sessions entry — makes "OmniDeck" appear in the display-manager session list.
sudo tee "$WAYLAND_SESSIONS/omnideck.desktop" >/dev/null <<EOF
[Desktop Entry]
Name=OmniDeck
Comment=OmniDeck 10-foot media & game launcher (gamescope session)
Exec=$LAUNCHER
Type=Application
DesktopNames=gamescope
EOF

# SDDM doesn't scan the canonical dir on this system (e.g. CachyOS): link the entry into the
# first dir SDDM does scan that accepts it, so the greeter lists it and autologin finds it.
SESSION_LINK=""
for d in "${LINK_CANDIDATES[@]}"; do
  if sudo mkdir -p "$d" 2>/dev/null && sudo ln -sfn "$WAYLAND_SESSIONS/omnideck.desktop" "$d/omnideck.desktop" 2>/dev/null; then
    SESSION_LINK="$d/omnideck.desktop"
    echo "  linked:   $SESSION_LINK -> $WAYLAND_SESSIONS/omnideck.desktop"
    break
  fi
done
if [ "${#LINK_CANDIDATES[@]}" -gt 0 ] && [ -z "$SESSION_LINK" ]; then
  cat >&2 <<EOF

⚠ SDDM is configured to scan only: ${LINK_CANDIDATES[*]}
  and none of those accepted a link, so SDDM will not list OmniDeck. Add the canonical dir
  to SDDM's list with a drop-in (files in /etc/sddm.conf.d override /usr/lib/sddm/sddm.conf.d):
    sudo mkdir -p /etc/sddm.conf.d
    printf '[Wayland]\nSessionDir=%s\n' "$WAYLAND_SESSIONS,$(IFS=,; echo "${LINK_CANDIDATES[*]}")" | sudo tee /etc/sddm.conf.d/zz-omnideck-sessiondir.conf
  then restart SDDM (or reboot).
EOF
fi

RM_LINK=""
[ -n "$SESSION_LINK" ] && RM_LINK=" \"$SESSION_LINK\""

cat <<EOF

✓ Installed. Log out and pick "OmniDeck" from your display manager's session list.
  OmniDeck fullscreens itself inside gamescope; quitting it (power menu) returns you here.

Optional: force a display mode (gamescope defaults to the EDID-preferred mode — often
60 Hz on high-refresh monitors). Create ~/.config/omnideck/session.conf with e.g.:
  GAMESCOPE_FLAGS="-W 2560 -H 1440 -r 165 -O DP-3"

To remove:  sudo rm "$WAYLAND_SESSIONS/omnideck.desktop" "$LAUNCHER"$RM_LINK
EOF
