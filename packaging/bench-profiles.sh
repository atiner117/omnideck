#!/usr/bin/env bash
# Interpolation sustainability bench: measured output fps + CPU cores consumed, per
# (profile, display-fps, source-size). Collects the per-host tier data that informs the
# ultra cap policy in the generated .vpy profiles (synthetic testsrc2 motion is EASY —
# real film costs a multiple, so read "cores" as a lower bound, not a promise).
# Needs: mpv built with VapourSynth + the MVTools plugin.
# Usage: bench-profiles.sh [profile-dir]   (default: ~/.config/omnideck/mpv-profiles;
#   on another host: scp -r the rendered dir + this script over and point at it)
# Reference (2026-07-08, ares i7-14700K/28t): 1080p cases 2.3-3.4 cores; 4K->60 basic
# 6.2, 4K->60 ultra 10.4, 4K->120 basic 13.1 — all sustained their targets.
set -uo pipefail
D="${1:-$HOME/.config/omnideck/mpv-profiles}"
NPROC=$(nproc)

# Reaped-children cpu seconds of the MAIN shell ($$ survives subshells; /proc/self in a
# $() would read the fork's zeroed counters).
child_cpu() { awk '{print ($16+$17)/100}' "/proc/$$/stat"; }

bench() { # <vpy> <dfps> <size> <label>
  local t0 t1 c0 c1 out fps real cpu
  c0=$(child_cpu); t0=$(date +%s.%N)
  out=$(OMNIDECK_DISPLAY_FPS="$2" mpv --no-config --vo=null --ao=null \
      --vf=vapoursynth:"$D/$1" --length=8 \
      --term-status-msg='VF_FPS=${estimated-vf-fps}' \
      "av://lavfi:testsrc2=rate=24000/1001:size=$3:duration=9" 2>&1)
  t1=$(date +%s.%N); c1=$(child_cpu)
  fps=$(grep -o 'VF_FPS=[0-9.]*' <<<"$out" | tail -1 | cut -d= -f2)
  real=$(awk -v a="$t0" -v b="$t1" 'BEGIN{print b-a}')
  cpu=$(awk -v a="$c0" -v b="$c1" -v r="$real" 'BEGIN{printf "%.1f", (b-a)/r}')
  printf "%-26s measured %-9s fps | %4s cores / %s (%3.0f%% headroom)\n" \
    "$4" "${fps:-FAIL}" "$cpu" "$NPROC" \
    "$(awk -v c="$cpu" -v n="$NPROC" 'BEGIN{print (1-c/n)*100}')"
}

echo "host: $(hostname), $NPROC threads, $(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | xargs)"
echo "--- 1080p source ---"
bench interpolate-basic.vpy 165 1920x1080 "basic 1080p->165"
bench interpolate-ultra.vpy 165 1920x1080 "ultra 1080p->82.5"
bench interpolate-basic.vpy 120 1920x1080 "basic 1080p->120"
bench interpolate-ultra.vpy 120 1920x1080 "ultra 1080p->60(halved)"
echo "--- 4K source ---"
bench interpolate-basic.vpy 60 3840x2160 "basic 4K->60"
bench interpolate-ultra.vpy 60 3840x2160 "ultra 4K->60"
bench interpolate-basic.vpy 120 3840x2160 "basic 4K->120"
