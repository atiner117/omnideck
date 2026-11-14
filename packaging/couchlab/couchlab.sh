#!/usr/bin/env bash
# couchlab — agent-driven closed loop against a LIVE OmniDeck gamescope session on the
# couch box (default host: r2d2), run from any machine with SSH to it.
#
# Why: the display-path bugs (NVIDIA overlay plane, composite-force atom reverting,
# wrong-pitch scanout after an EDID/mode change) and the switcher bugs were only ever
# caught by Andrew's eyes on the TV. This gives an agent the same loop without hands:
#
#   state             one page of ground truth: EDID, CRTC mode, DRM planes, gamescope
#                     atoms, keeper/shim/omnideck liveness — plus derived RISK flags that
#                     encode every corruption signature seen so far (see risk_flags)
#   shot [label]      gamescope's composited frame (what the GPU sent) → run dir
#   seq <label> <steps>   drive the REAL controller path with the uinput virtual pad
#                     (examples/virtual-pad `seq` grammar), shot before + after
#   scenario <name>   canned sequences: deck-toggle, nav-sweep, wake
#   judge <png> [q]   ask the LOCAL vision model (llama-swap on r2d2, never cloud) what the
#                     frame shows — clean UI / bands / blocks / blank
#   watch <secs>      poll state every 2 s and report any risk flag that appears
#   report [dir]      print a run's summary
#
# What it cannot see: the TV panel itself. The composited frame is pre-scanout; if `shot`
# is clean and the TV is not, the fault is scanout/link/TV — `state` names which of the
# known scanout signatures is present. A camera on the TV wall or an HDMI capture between
# the PC and the TV is the only way to close that last gap.
#
# Env: COUCHLAB_HOST (r2d2), COUCHLAB_RUNS (/tmp/couchlab), COUCHLAB_VLM_URL
#      (https://llamaswap-r2d2.tiner.tech/v1), COUCHLAB_VLM_MODEL (qwen35b-hauhau),
#      COUCHLAB_PAD (path of virtual-pad on the host; default = the host's release build)
set -u
HOST="${COUCHLAB_HOST:-r2d2}"
RUNS="${COUCHLAB_RUNS:-/tmp/couchlab}"
VLM_URL="${COUCHLAB_VLM_URL:-https://llamaswap-r2d2.tiner.tech/v1}"
VLM_MODEL="${COUCHLAB_VLM_MODEL:-qwen35b-hauhau}"
PAD="${COUCHLAB_PAD:-\$HOME/Projects/omnideck/src-tauri/target/release/examples/virtual-pad}"
SSH=(ssh -o BatchMode=yes -o ConnectTimeout=10 "$HOST")
# NOTE: the host's login shell may be fish with aliases (r2d2: ls=eza) — every remote
# one-liner goes through `bash -c`, and multi-line blocks through `bash -s` (rsh).

die() { echo "couchlab: $*" >&2; exit 1; }
rsh() { "${SSH[@]}" bash -s 2>&1 | grep -vE 'agent refused operation|ssh-askpass'; }
newrun() { local d="$RUNS/$(date +%Y%m%d-%H%M%S)-${1:-run}"; mkdir -p "$d"; echo "$d"; }

# ── state ────────────────────────────────────────────────────────────────────────────
# Everything below is read-only on the host.
state_remote() {
  rsh <<'REMOTE'
export DISPLAY=:0
conn=$(ls -d /sys/class/drm/card*-HDMI-A-1 2>/dev/null | head -1)
echo "time: $(date '+%F %T %Z')"
echo "edid_md5: $(md5sum $conn/edid 2>/dev/null | cut -c1-8)  override_md5: $(md5sum /lib/firmware/edid/sony_a8f_120hz.bin 2>/dev/null | cut -c1-8)"
echo "connector: $(cat $conn/status 2>/dev/null) $(cat $conn/enabled 2>/dev/null) dpms=$(cat $conn/dpms 2>/dev/null)"
crtc=$(modetest -M nvidia-drm -p 2>/dev/null | awk '/^CRTCs:/{p=1} /^Planes:/{p=0} p' | grep -E "^[0-9]+\s+[0-9]+\s+\(" | awk '$2!=0' | head -1)
echo "crtc: ${crtc:-none}"
mode=$(modetest -M nvidia-drm -p 2>/dev/null | awk '/^CRTCs:/{p=1} /^Planes:/{p=0} p' | grep -E "^\s+#0 " | head -1 | awk '{print $2, $3, $(NF-6)}')
echo "crtc_mode: ${mode:-none}"
echo "planes_with_fb:"; modetest -M nvidia-drm -p 2>/dev/null | awk '/^Planes:/{p=1} p' | grep -E "^[0-9]+\s+[0-9]+\s+[0-9]+" | awk '$3!=0 {print "  plane " $1 " crtc " $2 " fb " $3}'
for a in GAMESCOPE_COMPOSITE_FORCE GAMESCOPE_DISPLAY_HDR_ENABLED GAMESCOPE_DISPLAY_SUPPORTS_HDR GAMESCOPE_DISPLAY_REFRESH_RATE_FEEDBACK GAMESCOPE_FOCUSED_WINDOW GAMESCOPE_FOCUSED_APP GAMESCOPE_VRR_ENABLED; do
  v=$(xprop -root "$a" 2>/dev/null | awk -F' = ' '{print $2}'); echo "atom $a: ${v:-<unset>}"
done
echo "xroot: $(xdpyinfo 2>/dev/null | awk '/dimensions/{print $2}')"
echo "modes_external: $(xprop -root GAMESCOPE_DISPLAY_MODE_LIST_EXTERNAL 2>/dev/null | grep -oE '[0-9]+x[0-9]+@[0-9]+' | sort -u | tr '\n' ' ')"
# gamescope's xwm does not maintain _NET_CLIENT_LIST; enumerate visible toplevels with
# xdotool when present, else at least name the focused window.
echo "windows:"
if command -v xdotool >/dev/null; then
  for w in $(xdotool search --onlyvisible --name . 2>/dev/null | head -12); do echo "  $w pid=$(xdotool getwindowpid $w 2>/dev/null) '$(xdotool getwindowname $w 2>/dev/null | cut -c1-60)'"; done
else
  fw=$(xprop -root GAMESCOPE_FOCUSED_WINDOW 2>/dev/null | awk -F' = ' '{print $2}' | cut -d, -f1)
  [ -n "$fw" ] && echo "  focused $fw $(xprop -id $fw WM_NAME _NET_WM_PID 2>/dev/null | awk -F' = ' '{printf "%s ", $2}')"
fi
for n in gamescope-wl omnideck; do p=$(pgrep -x $n | head -1); echo "proc $n: ${p:-DOWN} $( [ -n "$p" ] && ps -o lstart= -p $p)"; done
echo "proc keeper: $(pgrep -f omnideck-composite-keeper | head -1)"
echo "proc shim: $(pgrep -f 'python.*jellyfin-mpv-shim' | head -1)  shim_display: $(p=$(pgrep -f 'python.*jellyfin-mpv-shim' | head -1); [ -n "$p" ] && tr '\0' '\n' < /proc/$p/environ | grep -E '^DISPLAY=' | cut -d= -f2)"
echo "display_mode_file: $(cat ~/.config/omnideck/display-mode 2>/dev/null)"
# Switcher ground truth: SIGSTOPped process groups (the switcher freezes hidden apps) and
# whether the app currently in front (navpad ACTIVE) is among them.
echo "stopped_groups: $(ps -o pgid=,stat= -u "$USER" | awk '$2 ~ /T/ {print $1}' | sort -u | tr '\n' ' ')"
echo "navpad: $(grep -hoE 'navpad: (ACTIVE|inactive)' $(ls -t ~/.local/state/omnideck/omnideck.*.log | head -1) 2>/dev/null | tail -1 | awk '{print $2}')"
L=~/.local/state/omnideck/gamescope-session.log
echo "gs_log: disconnects=$(grep -c disconnected $L 2>/dev/null) rearms=$(grep -c re-armed $L 2>/dev/null) last_mode='$(grep 'selecting mode' $L 2>/dev/null | tail -1 | grep -oE '[0-9]+x[0-9]+@[0-9]+Hz')' flags='$(head -1 $L 2>/dev/null | grep -oE 'flags=.*')'"
echo "omnideck_log_tail:"; tail -3 $(ls -t ~/.local/state/omnideck/omnideck.*.log 2>/dev/null | head -1) 2>/dev/null | cut -c1-150 | sed 's/^/  /'
REMOTE
}

# Derived flags. Each one is a corruption signature that has been seen on the TV:
#   OVERLAY_PLANE_FB  a second plane holds a framebuffer (09-20 / 09-26 displaced blocks)
#   ATOM_NOT_FORCED   GAMESCOPE_COMPOSITE_FORCE != 1 (direct scanout with wrong tiling, 09-13)
#   PITCH_MISMATCH    gamescope's X root size != CRTC size (4K buffers on a 1080p CRTC, 09-26)
#   MODE_MISMATCH     display-mode file asks for a mode the CRTC is not in
#   FROZEN_WHILE_APP_IN_FRONT  a SIGSTOPped group exists while the pad is handed to an app
#   NO_COMPOSITOR / NO_APP / NO_KEEPER / SHIM_NO_DISPLAY  liveness
risk_flags() {
  local s="$1" flags=()
  [ "$(grep -c '^  plane ' <<<"$s")" -gt 1 ] && flags+=(OVERLAY_PLANE_FB)
  grep -q 'atom GAMESCOPE_COMPOSITE_FORCE: 1' <<<"$s" || flags+=(ATOM_NOT_FORCED)
  local xr crtc; xr=$(grep '^xroot:' <<<"$s" | awk '{print $2}'); crtc=$(grep '^crtc:' <<<"$s" | grep -oE '\([0-9]+x[0-9]+\)' | tr -d '()')
  [ -n "$xr" ] && [ -n "$crtc" ] && [ "$xr" != "$crtc" ] && flags+=("PITCH_MISMATCH($xr vs $crtc)")
  local want; want=$(grep '^display_mode_file:' <<<"$s" | awk '{print $2}')
  case "$want" in 4k60) [[ "$crtc" == 3840x2160 ]] || flags+=("MODE_MISMATCH(want $want got $crtc)");; 1080p120) [[ "$crtc" == 1920x1080 ]] || flags+=("MODE_MISMATCH(want $want got $crtc)");; esac
  grep -q 'proc gamescope-wl: DOWN' <<<"$s" && flags+=(NO_COMPOSITOR)
  grep -q 'proc omnideck: DOWN' <<<"$s" && flags+=(NO_APP)
  grep -q 'proc keeper: $' <<<"$s" && flags+=(NO_KEEPER)
  grep -qE 'proc shim: [0-9]+ +shim_display: *$' <<<"$s" && flags+=(SHIM_NO_DISPLAY)
  # An app is in front (pad handed to it) but some group is still frozen: either the
  # front app itself is stopped (dead controls) or a hidden one never thawed.
  if grep -q '^navpad: ACTIVE' <<<"$s" && [ -n "$(grep '^stopped_groups:' <<<"$s" | cut -d: -f2 | tr -d ' ')" ]; then flags+=("FROZEN_WHILE_APP_IN_FRONT($(grep '^stopped_groups:' <<<"$s" | cut -d: -f2 | xargs))"); fi
  [ ${#flags[@]} -eq 0 ] && echo "risk: none" || echo "risk: ${flags[*]}"
}

cmd_state() {
  local s; s=$(state_remote) || die "state: ssh failed"
  echo "$s"; risk_flags "$s"
  if [ -n "${RUN:-}" ]; then echo "$s" > "$RUN/state${1:+-$1}.txt"; risk_flags "$s" >> "$RUN/state${1:+-$1}.txt"; fi
}

# ── shot ─────────────────────────────────────────────────────────────────────────────
cmd_shot() {
  local label="${1:-shot}" ; RUN="${RUN:-$(newrun "$label")}"
  rsh <<'REMOTE' >/dev/null
export DISPLAY=:0; rm -f /tmp/gamescope.png
xprop -root -f GAMESCOPECTRL_REQUEST_SCREENSHOT 8s -set GAMESCOPECTRL_REQUEST_SCREENSHOT x
for i in 1 2 3 4 5 6; do [ -s /tmp/gamescope.png ] && break; sleep 0.5; done
REMOTE
  scp -q -o BatchMode=yes "$HOST:/tmp/gamescope.png" "$RUN/$label.png" 2>/dev/null || die "shot: no screenshot from gamescope (is the session up?)"
  magick "$RUN/$label.png" -resize 1280x720 -quality 85 "$RUN/$label.jpg" 2>/dev/null
  local mean; mean=$(magick "$RUN/$label.png" -format '%[fx:mean]' info: 2>/dev/null)
  echo "shot: $RUN/$label.png (mean luma ${mean:-?}) preview $RUN/$label.jpg"
  echo "$label mean=${mean:-?}" >> "$RUN/shots.txt"
}

# ── judge (local VLM, never cloud) ───────────────────────────────────────────────────
cmd_judge() {
  local img="$1"; shift; local q="${*:-You are checking a TV frame from a media launcher. Describe it in two sentences. Then, on the last line alone, classify it as exactly one of: CLEAN_UI (a coherent app/launcher/video frame, dark themes count as CLEAN_UI), BLACK (nothing drawn at all except maybe a tiny clock), WHITE (mostly white/blank), BANDS (content torn into shifted horizontal strips), BLOCKS (content duplicated or displaced in rectangular blocks), SPARKLE (random coloured dots/noise), OTHER.}"
  [ -f "$img" ] || die "judge: no such image $img"
  local tmp; tmp=$(mktemp --suffix=.jpg); magick "$img" -resize 1280x720 -quality 80 "$tmp"
  python3 - "$tmp" "$q" "$VLM_URL" "$VLM_MODEL" <<'PY'
import sys,json,base64,urllib.request
img,q,url,model=sys.argv[1:5]
b=base64.b64encode(open(img,'rb').read()).decode()
req={"model":model,"max_tokens":200,"temperature":0.1,"messages":[{"role":"user","content":[{"type":"text","text":q},{"type":"image_url","image_url":{"url":"data:image/jpeg;base64,"+b}}]}]}
r=urllib.request.Request(url+"/chat/completions",data=json.dumps(req).encode(),headers={"Content-Type":"application/json"})
d=json.load(urllib.request.urlopen(r,timeout=180))
print(d["choices"][0]["message"]["content"].strip())
PY
  rm -f "$tmp"
}

# ── seq / scenario ───────────────────────────────────────────────────────────────────
# seq <label> <steps> [shot-offsets]
#   shot-offsets: comma-separated seconds after the pad starts at which to grab a frame
#   (e.g. "1.2,3") — that is how you SEE the deck while it is up, mid-sequence.
# Ground truth beyond pixels: every omnideck log line written during the run is saved to
# $RUN/omnideck.log (guide taps, switcher freeze/thaw, navpad hand-offs).
cmd_seq() {
  local label="$1" steps="$2" offsets="${3:-}"; [ -n "$steps" ] || die "seq: need steps"
  RUN="${RUN:-$(newrun "$label")}"
  echo "run: $RUN"
  cmd_state before >/dev/null; cmd_shot before
  local logf mark; logf=$("${SSH[@]}" bash -c "'ls -t ~/.local/state/omnideck/omnideck.*.log | head -1'" 2>/dev/null)
  mark=$("${SSH[@]}" bash -c "'wc -l < $logf'" 2>/dev/null)
  echo "pad: $steps"
  # The pad runs detached on the host (its own 1.5 s hotplug settle comes first), so we can
  # shoot frames at chosen offsets while the sequence is still running.
  "${SSH[@]}" bash -c "'export DISPLAY=:0; nohup $PAD seq $steps > /tmp/couchlab-pad.log 2>&1 < /dev/null &'" >/dev/null 2>&1
  local t0=$(date +%s.%N)
  if [ -n "$offsets" ]; then
    for off in ${offsets//,/ }; do
      local now; now=$(date +%s.%N); local wait; wait=$(python3 -c "print(max(0, 1.5 + $off - ($now - $t0)))")
      sleep "$wait"; cmd_shot "t+${off}s"
    done
  fi
  # wait for the pad to finish (steps total + settle), capped
  local total; total=$(python3 -c "import re;print(sum(int(m) for m in re.findall(r':(\d+)', '$steps'))/1000 + 0.3*len('$steps'.split(',')) + 2.5)")
  local elapsed; elapsed=$(python3 -c "print($(date +%s.%N) - $t0)")
  sleep "$(python3 -c "print(max(0, $total - $elapsed))")"
  scp -q -o BatchMode=yes "$HOST:/tmp/couchlab-pad.log" "$RUN/pad.log" 2>/dev/null
  "${SSH[@]}" bash -c "'tail -n +$((mark+1)) $logf'" 2>/dev/null | cut -c1-200 > "$RUN/omnideck.log"
  cmd_shot after; cmd_state after | tail -1
  echo "omnideck log during run ($(wc -l < "$RUN/omnideck.log") lines):"; sed 's/^/  /' "$RUN/omnideck.log" | cut -c1-160 | head -20
  cmd_report "$RUN"
}
cmd_scenario() {
  case "${1:-}" in
    deck-toggle) cmd_seq deck-toggle "guide,sleep:2500,guide,sleep:1000" "1.2" ;;
    nav-sweep)   cmd_seq nav-sweep "down,down,down,sleep:600,up,up,up,sleep:600,right,sleep:600,left,sleep:600" "0.8,2.0" ;;
    wake)        cmd_seq wake "south:60,sleep:1200" ;;
    *) die "scenario: deck-toggle | nav-sweep | wake" ;;
  esac
}

# ── watch ────────────────────────────────────────────────────────────────────────────
cmd_watch() {
  local secs="${1:-60}" end=$((SECONDS+secs)) last=""
  while [ $SECONDS -lt $end ]; do
    local s r; s=$(state_remote); r=$(risk_flags "$s")
    [ "$r" != "$last" ] && { echo "$(date +%T) $r  crtc=$(grep '^crtc:' <<<"$s" | grep -oE '\([0-9]+x[0-9]+\)') atom=$(grep 'atom GAMESCOPE_COMPOSITE_FORCE' <<<"$s" | awk '{print $NF}')"; last="$r"; }
    sleep 2
  done
}

cmd_report() {
  local d="${1:-$(ls -td "$RUNS"/*/ 2>/dev/null | head -1)}"; [ -d "$d" ] || die "report: no run dir"
  echo "== $d"; ls "$d" | sed 's/^/  /'
  for f in "$d"/state-*.txt; do [ -f "$f" ] && echo "  $(basename "$f"): $(tail -1 "$f")"; done
  [ -f "$d/shots.txt" ] && sed 's/^/  shot /' "$d/shots.txt"
}

case "${1:-}" in
  state)    shift; cmd_state "$@" ;;
  shot)     shift; cmd_shot "$@" ;;
  judge)    shift; cmd_judge "$@" ;;
  seq)      shift; cmd_seq "$@" ;;
  scenario) shift; cmd_scenario "$@" ;;
  watch)    shift; cmd_watch "$@" ;;
  report)   shift; cmd_report "$@" ;;
  *) sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
esac
