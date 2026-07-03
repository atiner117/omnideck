#!/usr/bin/env bash
# OmniDeck — automated nested-gamescope session pre-flight (companion to M2-SESSION-TEST.md).
#
# Runs OmniDeck inside a NESTED gamescope window on your desktop and drives the input paths
# that used to need a logout to test. Inside nested gamescope the app sees the exact same
# world as the real session (GAMESCOPE_WAYLAND_DISPLAY, private Xwayland, steamcompmgr
# focus-follows-mapping), so these are real tests of hotkey.rs / switcher.rs / gamepad.rs:
#
#   1. boot        OmniDeck window appears and paints non-black
#   2. kbd-hide    Ctrl+Alt+Home hides a launched app  (X grab → switcher unmap)
#   3. kbd-show    Ctrl+Alt+Home again brings it back  (remap)
#   4. kbd-close   Ctrl+Alt+End closes it              (watchdog pgid kill)
#   5. pad-hide    Guide short-press hides it          (uinput virtual pad → gilrs)
#   6. pad-show    Guide short-press again shows it
#   7. pad-close   Guide hold >= 800 ms closes it
#
# Still bare-metal only: display-mode/165 Hz (real EDID), real Steam launch + focus return
# (STEAM_GAME atom), suspend, SDDM login. Everything else regresses HERE first.
#
# Needs: gamescope, xdotool, imagemagick, cargo (test tools), /dev/uinput write access
# (input group). Steam note: the virtual pad is a real evdev device — a running desktop
# Steam may also see its Guide presses; close Steam for a clean run.
#
# Usage: ./packaging/test-session.sh [/path/to/omnideck-binary]
set -u

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="${1:-$here/src-tauri/target/release/omnideck}"
[ -x "$BIN" ] || { echo "✗ binary not found: $BIN  (build: bun run tauri build --no-bundle)"; exit 1; }
for tool in gamescope xdotool import magick; do
  command -v "$tool" >/dev/null || { echo "✗ missing tool: $tool"; exit 1; }
done

# Test tools (see the examples' headers): x11-stub = deterministic launch target (GTK/Qt
# dialogs re-map themselves after our unmap and flake the harness); virtual-pad = uinput
# gamepad. Built up front so cargo latency never sits inside a timed test step.
echo "building test tools (cargo) …"
cargo build -q --manifest-path "$here/src-tauri/Cargo.toml" --example x11-stub --example virtual-pad \
  || { echo "✗ cargo build of test tools failed"; exit 1; }
STUB_BIN="$here/src-tauri/target/debug/examples/x11-stub"
PAD_BIN="$here/src-tauri/target/debug/examples/virtual-pad"

RUN="$(mktemp -d "${XDG_RUNTIME_DIR:-/tmp}/omnideck-test.XXXXXX")"
FIFO="$RUN/control"
GSLOG="$RUN/gamescope.log"
mkfifo -m 600 "$FIFO"

PASS=0; FAIL=0
ok()   { echo "  ✓ $1"; PASS=$((PASS+1)); }
bad()  { echo "  ✗ $1"; FAIL=$((FAIL+1)); }

# Poll `eval $2` until success or $1 seconds elapse. Quiet; returns the condition's status.
wait_for() {
  local deadline=$((SECONDS + $1)); shift
  until eval "$@" >/dev/null 2>&1; do
    [ $SECONDS -ge $deadline ] && return 1
    sleep 0.3
  done
}

cleanup() {
  [ -p "$FIFO" ] && { echo quit > "$FIFO"; } 2>/dev/null &
  sleep 1
  [ -n "${GS_PID:-}" ] && kill "$GS_PID" 2>/dev/null
  [ -n "${STUB_PID:-}" ] && kill "$STUB_PID" 2>/dev/null
  # Keep the session log around when something failed — the temp dir is about to go.
  [ "${FAIL:-0}" -gt 0 ] && [ -f "$GSLOG" ] && cp "$GSLOG" /tmp/omnideck-test-last.log \
    && echo "  (session log saved to /tmp/omnideck-test-last.log)"
  rm -rf "$RUN"
}
trap cleanup EXIT

echo "── nested gamescope session (1280x720 window will appear) ──"
OMNIDECK_TEST_CONTROL="$FIFO" gamescope -W 1280 -H 720 --xwayland-count 1 \
  -- "$BIN" >"$GSLOG" 2>&1 &
GS_PID=$!

# The nested Xwayland display, from gamescope's own log line ("Starting Xwayland on :1").
if ! wait_for 15 "grep -q 'Starting Xwayland on' '$GSLOG'"; then
  echo "✗ gamescope did not start an Xwayland — log tail:"; tail -20 "$GSLOG"; exit 1
fi
NESTED="$(grep -o 'Starting Xwayland on :[0-9]*' "$GSLOG" | head -1 | grep -o ':[0-9]*')"
export DISPLAY="$NESTED"
echo "  nested display: $NESTED   (log: $GSLOG)"

# ── 1. boot: window mapped + non-black paint ──
if wait_for 30 "xdotool search --onlyvisible --name '^omnideck$' | grep -q ."; then
  APP_WID="$(xdotool search --onlyvisible --name '^omnideck$' | head -1)"
  if wait_for 30 "import -silent -window '$APP_WID' '$RUN/boot.png' &&
                  magick '$RUN/boot.png' -format '%[fx:mean>0.02?1:0]' info: | grep -qx 1"; then
    ok "boot: OmniDeck window painted (not black)"
  else
    bad "boot: window mapped but stayed black — check $GSLOG / OMNIDECK_GPU_COMPOSITING"
  fi
else
  bad "boot: OmniDeck window never appeared"; tail -20 "$GSLOG"; exit 1
fi

# Launch/wait/find helpers for the stub app (goes through the REAL launch path via the
# FIFO test hook, so the watchdog owns its process group — required by switcher/hotkeys).
# The stub is examples/x11-stub.rs: one plain toplevel with _NET_WM_PID set (the property
# the switcher keys ownership on) that never re-maps itself — see its header for why real
# toolkit dialogs (zenity) flake here while real launch targets don't.
stub_launch() {
  echo "launch $STUB_BIN omnideck-harness-stub" > "$FIFO"
  wait_for 10 "xdotool search --onlyvisible --name omnideck-harness-stub | grep -q ." || return 1
  # Track liveness by pid (from the window), never by pgrep -f: a command-line pattern
  # can match unrelated processes — including the shell running this script.
  STUB_PID="$(xdotool getwindowpid "$(xdotool search --name omnideck-harness-stub | head -1)")"
}
stub_visible() { xdotool search --onlyvisible --name omnideck-harness-stub 2>/dev/null | grep -q .; }
stub_alive()   { [ -n "${STUB_PID:-}" ] && kill -0 "$STUB_PID" 2>/dev/null; }
stub_kill()    { [ -n "${STUB_PID:-}" ] && kill "$STUB_PID" 2>/dev/null; }

# True once `eval $1` holds across a 0.5 s gap — i.e. the state has actually settled, not
# just flickered true for one X round-trip. The switcher unmap and gamescope's focus refollow
# are separate async steps; reading between them is what made the naive check race.
stable() { eval "$1" >/dev/null 2>&1 && sleep 0.5 && eval "$1" >/dev/null 2>&1; }

# Press a chord/button ($1) and confirm the stub reached $2 ("hidden"|"shown"), stably. The
# switcher is only ever pressed by a human at human speed; firing the next toggle before the
# compositor finished the last one makes them race (two toggles 40 ms apart cancel out). So
# each step waits for a settled state before returning, and the caller pauses between steps.
toggle_expect() {
  local press="$1" want="$2" cond deadline
  [ "$want" = hidden ] && cond="! stub_visible" || cond="stub_visible"
  eval "$press"
  deadline=$((SECONDS + 6))
  until stable "$cond"; do
    [ $SECONDS -ge $deadline ] && return 1
    sleep 0.3
  done
  sleep 0.5   # let the compositor's focus refollow finish before the next chord
  return 0
}

# ── 2-4. keyboard chords (Ctrl+Alt+Home / End) ──
echo "── keyboard chords ──"
KEY="xdotool key --clearmodifiers"
if stub_launch; then
  toggle_expect "$KEY ctrl+alt+Home" hidden && ok "kbd-hide: Ctrl+Alt+Home hid the app" || bad "kbd-hide: app still visible"
  toggle_expect "$KEY ctrl+alt+Home" shown  && ok "kbd-show: Ctrl+Alt+Home brought it back" || bad "kbd-show: app did not remap"
  $KEY ctrl+alt+End
  if wait_for 8 "! stub_alive"; then ok "kbd-close: Ctrl+Alt+End closed the app"; else bad "kbd-close: process still running"; stub_kill; fi
else
  bad "kbd: stub app never appeared (test hook / launch path broken?)"
fi

# ── 5-7. gamepad Guide button (virtual pad over uinput) ──
echo "── gamepad Guide (virtual pad) ──"
if [ -w /dev/uinput ]; then
  PAD="$PAD_BIN"
  if stub_launch; then
    toggle_expect "$PAD guide-short" hidden && ok "pad-hide: Guide short-press hid the app" || bad "pad-hide: app still visible"
    toggle_expect "$PAD guide-short" shown  && ok "pad-show: Guide short-press brought it back" || bad "pad-show: app did not remap"
    eval "$PAD guide-hold 1200"
    if wait_for 8 "! stub_alive"; then ok "pad-close: Guide hold closed the app"; else bad "pad-close: process still running"; stub_kill; fi
  else
    bad "pad: stub app never appeared on relaunch"
  fi
else
  echo "  – skipped: no write access to /dev/uinput (add yourself to the 'input' group)"
fi

echo
echo "── result: $PASS passed, $FAIL failed ──"
[ $FAIL -eq 0 ] && echo "Nested pre-flight PASS. Bare metal still owns: 165 Hz mode, Steam launch/return, suspend, SDDM login."
exit $((FAIL > 0))
