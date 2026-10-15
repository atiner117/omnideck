#!/usr/bin/env bash
# Unit test for install-session.sh's SDDM SessionDir resolution (--print-session-dirs).
# Builds fake SDDM config trees and checks the effective Wayland SessionDir list and the
# install/link plan. No root, no gamescope, no OmniDeck binary; safe in CI.
#
# Usage: ./packaging/test-install-session.sh
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
script="$here/install-session.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

pass=0
fail=0

# Fresh fake root per case. Files: put <relative-path> <content>.
newroot() { root="$tmp/$1"; mkdir -p "$root"; }
put() { mkdir -p "$(dirname "$root/$1")"; printf '%b' "$2" >"$root/$1"; }

# Lines of --print-session-dirs for one kind (sessiondir|install|link), space-joined.
plan() { OMNIDECK_SDDM_ROOT="$root" "$script" --print-session-dirs | awk -v k="$1" '$1 == k { print $2 }' | paste -sd' ' -; }

check() { # <case> <kind> <expected>
  local got
  got="$(plan "$2")"
  if [ "$got" = "$3" ]; then
    pass=$((pass + 1))
  else
    fail=$((fail + 1))
    printf 'FAIL %s [%s]\n  expected: %s\n  got:      %s\n' "$1" "$2" "$3" "$got"
  fi
}

L=/usr/local/share/wayland-sessions
U=/usr/share/wayland-sessions
INSTALL="$L/omnideck.desktop"

# 1. No SDDM config at all (GDM, or SDDM on compiled-in defaults): canonical dir only.
newroot none
check none sessiondir "$L $U"
check none install "$INSTALL"
check none link ""

# 2. CachyOS: default.conf (commented defaults) then general.conf overriding [Wayland]
#    SessionDir to /usr/share only — the reported bug. The entry must be linked there.
newroot cachyos
put usr/lib/sddm/sddm.conf.d/default.conf '[X11]\nSessionDir=/usr/local/share/xsessions,/usr/share/xsessions\n\n[Wayland]\n# Directory containing available Wayland sessions\n#SessionDir=/usr/local/share/wayland-sessions,/usr/share/wayland-sessions\n'
put usr/lib/sddm/sddm.conf.d/general.conf '[General]\nNumlock=on\n\n[Wayland]\nSessionCommand=/usr/share/sddm/scripts/wayland-session\nSessionDir=/usr/share/wayland-sessions\n'
put usr/lib/sddm/sddm.conf.d/zz-wayland.conf '[General]\nDisplayServer=wayland\n\n[Wayland]\nCompositorCommand=kwin_wayland --drm\n'
check cachyos sessiondir "$U"
check cachyos install "$INSTALL"
check cachyos link "$U/omnideck.desktop"

# 3. The r2d2 workaround: an /etc drop-in restores both dirs; /etc wins over /usr/lib.
newroot r2d2
put usr/lib/sddm/sddm.conf.d/general.conf '[Wayland]\nSessionDir=/usr/share/wayland-sessions\n'
put etc/sddm.conf.d/zz-autologin.conf '[Autologin]\nUser=atiner\nSession=omnideck\n\n[Wayland]\nSessionDir=/usr/local/share/wayland-sessions,/usr/share/wayland-sessions\n'
check r2d2 sessiondir "$L $U"
check r2d2 link ""

# 4. /etc/sddm.conf is read last and beats every conf.d file.
newroot mainconf
put etc/sddm.conf.d/10-dirs.conf "[Wayland]\nSessionDir=$L\n"
put etc/sddm.conf '[Wayland]\nSessionDir=/opt/sessions\n'
check mainconf sessiondir "/opt/sessions"
check mainconf link "/opt/sessions/omnideck.desktop"

# 5. Files in a conf.d dir are read in byte order ("10-" before "9-"), last wins.
newroot order
put etc/sddm.conf.d/9-late.conf '[Wayland]\nSessionDir=/nine\n'
put etc/sddm.conf.d/10-early.conf '[Wayland]\nSessionDir=/ten\n'
check order sessiondir "/nine"

# 6. Parsing: [X11] SessionDir is ignored, whitespace/CRLF/quotes/trailing slashes are
#    normalized, empty and duplicate list entries dropped, comments skipped.
newroot parse
put etc/sddm.conf.d/a.conf '[X11]\nSessionDir=/x11-only\n'
put etc/sddm.conf.d/b.conf ' [ Wayland ] \r\n ; SessionDir=/commented\r\n  SessionDir  =  "/a/" , /b//,, /a ,/c \r\n[X11]\nSessionDir=/also-x11\n'
check parse sessiondir "/a /b /c"
check parse link "/a/omnideck.desktop /b/omnideck.desktop /c/omnideck.desktop"

# 7. A list that already contains the canonical dir (anywhere) needs no link.
newroot listed
put etc/sddm.conf "[Wayland]\nSessionDir=$U,$L/\n"
check listed sessiondir "$U $L"
check listed link ""

# 8. An empty SessionDir= falls back to SDDM's built-in default rather than "no dirs".
newroot empty
put etc/sddm.conf '[Wayland]\nSessionDir=\n'
check empty sessiondir "$L $U"
check empty link ""

# 9. Unknown flags are rejected instead of being taken as the binary path.
if "$script" --bogus >/dev/null 2>&1; then
  fail=$((fail + 1)); echo "FAIL flags: --bogus was accepted"
else
  pass=$((pass + 1))
fi

echo "install-session: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
