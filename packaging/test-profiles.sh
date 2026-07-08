#!/usr/bin/env bash
# Headless validation of OmniDeck's generated VapourSynth profiles: feed a synthetic
# 23.976 fps source through each .vpy with a chosen --display-fps-override and check
# mpv's measured filter-output rate (estimated-vf-fps) against the profile's contract:
#
#   interpolate-basic.vpy  -> full display rate       (165 -> ~165, 60 -> ~60)
#   interpolate-ultra.vpy  -> display/2 above 100 Hz  (165 -> ~82.5, 60 -> ~60)
#   denoise.vpy            -> source rate unchanged   (-> ~23.976)
#
# Skips (exit 0) when mpv/VapourSynth/the rendered set are absent, so it is safe in CI.
# Usage: ./test-profiles.sh [profile-dir]   (default: ~/.config/omnideck/mpv-profiles)
set -euo pipefail

dir="${1:-${XDG_CONFIG_HOME:-$HOME/.config}/omnideck/mpv-profiles}"

command -v mpv >/dev/null 2>&1 || { echo "SKIP: mpv not installed"; exit 0; }
# (capture, don't `| grep -q`: with pipefail, grep -q's early exit SIGPIPEs mpv)
vf_help="$(mpv --no-config --vf=help 2>/dev/null || true)"
grep -qi vapoursynth <<<"$vf_help" \
  || { echo "SKIP: mpv built without the vapoursynth filter"; exit 0; }
[ -d "$dir" ] || { echo "SKIP: $dir missing — run 'omnideck mpvprofiles' first"; exit 0; }

# Play ~4 s of a synthetic 23.976 fps clip through the filter and print the last
# estimated-vf-fps mpv measured. vo=null keeps it headless but real-time paced —
# estimated-vf-fps is a wallclock measurement, so don't --untimed it away. The panel
# rate goes in via OMNIDECK_DISPLAY_FPS (the .vpy scripts' explicit override): under
# vo=null mpv injects display_fps=0 and --display-fps-override is not forwarded to
# VapourSynth, so the option alone can't drive this test. No --quiet: it suppresses
# the status line that carries our VF_FPS message.
measured_fps() { # <vpy> <display-fps>
  OMNIDECK_DISPLAY_FPS="$2" \
  mpv --no-config --vo=null --ao=null \
      --vf=vapoursynth:"$1" \
      --length=4 \
      --term-status-msg='VF_FPS=${estimated-vf-fps}' \
      'av://lavfi:testsrc2=rate=24000/1001:size=1280x720:duration=5' 2>&1 \
    | grep -o 'VF_FPS=[0-9.]*' | tail -1 | cut -d= -f2
}

fail=0
check() { # <label> <vpy> <override> <expected> <tolerance>
  local got
  got="$(measured_fps "$dir/$2" "$3" || true)"
  if [ -z "$got" ]; then
    echo "FAIL: $1 — no estimated-vf-fps (filter failed to run?)"
    fail=1
    return
  fi
  if awk -v g="$got" -v e="$4" -v t="$5" 'BEGIN{exit !(g>=e-t && g<=e+t)}'; then
    echo "OK:   $1 -> ${got} fps (expected ~$4)"
  else
    echo "FAIL: $1 -> ${got} fps (expected $4 ± $5)"
    fail=1
  fi
}

echo "profile dir: $dir"
check "basic @165Hz panel " interpolate-basic.vpy 165  165   17
check "ultra @165Hz panel " interpolate-ultra.vpy 165  82.5  9
check "basic @60Hz panel  " interpolate-basic.vpy 60   60    6
check "ultra @60Hz panel  " interpolate-ultra.vpy 60   60    6
check "denoise passthrough" denoise.vpy           165  23.976 2.5

exit "$fail"
