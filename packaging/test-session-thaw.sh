#!/usr/bin/env bash
# Tests for packaging/omnideck-session-child.sh — no gamescope, no display, no OmniDeck build.
#
#   ./packaging/test-session-thaw.sh
#
# 1. omnideck_thaw_stopped_groups: CONT+TERMs a stopped group under ROOT, leaves a RUNNING
#    group under ROOT alone, and never touches a stopped group outside ROOT's tree.
# 2. End to end against a gamescopereaper stand-in (python subreaper: spawn the primary
#    client, wait for it, SIGTERM every remaining child, wait for ALL of them — the
#    loop that hung on the couch box):
#      a. negative control — a "crashing OmniDeck" that leaves a SIGSTOPped group, run
#         directly as the primary: the reaper must NOT finish (the bug, reproduced);
#      b. the same client under omnideck-session-child: the reaper finishes, and the
#         wrapper reports the client's own exit status (137, SIGKILL);
#      c. session teardown: SIGTERM to the wrapper while its client runs is forwarded,
#         the client exits 143, and the frozen group is still thawed.
set -u
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHILD="$here/omnideck-session-child.sh"
# shellcheck source=omnideck-session-child.sh
. "$CHILD"

T="$(mktemp -d)"
fails=0
ok()  { echo "  ✓ $*"; }
bad() { echo "  ✗ $*"; fails=$((fails + 1)); }
state() { ps -o stat= -p "$1" 2>/dev/null | tr -d ' '; }  # empty = gone
# Wait up to $1 s for process $2 to be gone (or a zombie).
gone_within() {
  local i
  for i in $(seq $(( $1 * 10 ))); do
    case "$(state "$2")" in ''|Z*) return 0 ;; esac
    sleep 0.1
  done
  return 1
}
cleanup() {
  for f in "$T"/*.pid; do
    [ -r "$f" ] || continue
    p="$(cat "$f")"
    kill -CONT -- "-$p" 2>/dev/null; kill -KILL -- "-$p" 2>/dev/null; kill -KILL "$p" 2>/dev/null
  done
  [ -n "${root:-}" ] && kill -KILL "$root" 2>/dev/null
  rm -rf "$T"
}
trap cleanup EXIT

echo "── omnideck_thaw_stopped_groups ──"
set -m  # background jobs get their own process groups, like OmniDeck's process_group(0)
# ROOT stands in for gamescopereaper: it owns one group that will be frozen and one that won't.
bash -c "set -m; sleep 300 & echo \$! > '$T/frozen.pid'; sleep 300 & echo \$! > '$T/running.pid'; wait" 2>/dev/null &
root=$!
# A stopped group OUTSIDE root's tree (same session, same user) — must stay stopped.
sleep 300 &
echo $! > "$T/outside.pid"
disown $!  # no "Killed" job chatter when cleanup removes it
set +m
for _ in $(seq 50); do [ -s "$T/frozen.pid" ] && [ -s "$T/running.pid" ] && break; sleep 0.1; done
frozen="$(cat "$T/frozen.pid")"; running="$(cat "$T/running.pid")"; outside="$(cat "$T/outside.pid")"
kill -STOP -- "-$frozen"; kill -STOP -- "-$outside"
sleep 0.2
case "$(state "$frozen")" in T*) ok "setup: group $frozen is stopped" ;; *) bad "setup: $frozen not stopped ($(state "$frozen"))" ;; esac

omnideck_thaw_stopped_groups "$root" 2>"$T/thaw.log"
if gone_within 3 "$frozen"; then ok "stopped group under root was thawed and terminated"
else bad "stopped group $frozen survived (state $(state "$frozen"))"; fi
case "$(state "$running")" in S*|R*) ok "running group under root left alone" ;; *) bad "running group touched (state '$(state "$running")')" ;; esac
case "$(state "$outside")" in T*) ok "stopped group outside root's tree left alone" ;; *) bad "outside group touched (state '$(state "$outside")')" ;; esac
grep -q "group $frozen" "$T/thaw.log" && ok "logged the thawed group" || bad "no log line for group $frozen"
kill -KILL -- "-$running" "-$outside" 2>/dev/null; kill -CONT -- "-$outside" 2>/dev/null
kill -KILL "$root" 2>/dev/null; wait "$root" 2>/dev/null; root=""

echo "── end to end under a gamescopereaper stand-in ──"
# The crashing client: freezes a background group (the switcher's SIGSTOP), then dies by
# SIGKILL — no graceful exit hook, no signal handler, nothing of OmniDeck's own runs.
cat > "$T/fake-omnideck" <<EOF
#!/usr/bin/env bash
set -m
sleep 300 &
echo \$! > "\$1"
kill -STOP -- "-\$!"
kill -KILL \$\$
EOF
chmod +x "$T/fake-omnideck"
cat > "$T/reaper.py" <<'EOF'
import ctypes, os, signal, subprocess, sys, time
ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0)  # PR_SET_CHILD_SUBREAPER
primary = subprocess.Popen(sys.argv[2:])
term_after = os.environ.get("REAPER_TERM_AFTER")
if term_after:  # session teardown: the client gets SIGTERM while it is still running
    time.sleep(float(term_after))
    primary.send_signal(signal.SIGTERM)
rc = primary.wait()
print(f"primary rc={rc}", flush=True)
me = os.getpid()
for p in os.listdir("/proc"):  # SIGTERM the remaining direct children (KillAllChildren)
    if p.isdigit():
        try:
            stat = open(f"/proc/{p}/stat").read().rsplit(")", 1)[1].split()
        except OSError:
            continue
        if int(stat[1]) == me:
            os.kill(int(p), signal.SIGTERM)
deadline = time.monotonic() + float(sys.argv[1])
while True:  # WaitForAllChildren, bounded so the negative control can report the hang
    try:
        pid, _ = os.waitpid(-1, os.WNOHANG)
    except ChildProcessError:
        print("reaper: all children gone", flush=True)
        sys.exit(0)
    if pid == 0:
        if time.monotonic() > deadline:
            print("reaper: STILL WAITING", flush=True)
            sys.exit(3)
        time.sleep(0.05)
EOF

python3 -I "$T/reaper.py" 3 "$T/fake-omnideck" "$T/neg.pid" > "$T/neg.log" 2>&1
neg_rc=$?
if [ "$neg_rc" = 3 ] && grep -q 'STILL WAITING' "$T/neg.log"; then
  ok "negative control: without the wrapper the reaper hangs on the stopped group"
else
  bad "negative control did not hang (rc=$neg_rc): $(tr '\n' ' ' < "$T/neg.log")"
fi
# Clear the stranded group from the negative control.
neg="$(cat "$T/neg.pid" 2>/dev/null)"; [ -n "$neg" ] && { kill -CONT -- "-$neg"; kill -KILL -- "-$neg"; } 2>/dev/null

python3 -I "$T/reaper.py" 5 "$CHILD" "$T/fake-omnideck" "$T/pos.pid" > "$T/pos.log" 2>&1
pos_rc=$?
if [ "$pos_rc" = 0 ] && grep -q 'all children gone' "$T/pos.log"; then
  ok "with omnideck-session-child the reaper finishes"
else
  bad "reaper did not finish under the wrapper (rc=$pos_rc): $(tr '\n' ' ' < "$T/pos.log")"
fi
grep -q 'primary rc=137' "$T/pos.log" && ok "wrapper exits with the client's status (137)" \
  || bad "wrapper exit status wrong: $(grep 'primary rc' "$T/pos.log")"
grep -q 'thawing + terminating stopped group' "$T/pos.log" && ok "wrapper logged the thaw" \
  || bad "wrapper did not log a thaw"

# c. teardown: the wrapper itself gets SIGTERM while a still-running client holds a frozen
#    group — the wrapper must forward it, wait for the client, then thaw.
cat > "$T/fake-omnideck-live" <<EOF
#!/usr/bin/env bash
set -m
sleep 300 &
echo \$! > "\$1"
kill -STOP -- "-\$!"
set +m
exec sleep 300
EOF
chmod +x "$T/fake-omnideck-live"
REAPER_TERM_AFTER=0.5 python3 -I "$T/reaper.py" 5 "$CHILD" "$T/fake-omnideck-live" "$T/term.pid" > "$T/term.log" 2>&1
term_rc=$?
if [ "$term_rc" = 0 ] && grep -q 'primary rc=143' "$T/term.log" && grep -q 'all children gone' "$T/term.log"; then
  ok "SIGTERM to the wrapper reaches the client, then the frozen group is thawed"
else
  bad "teardown path failed (rc=$term_rc): $(tr '\n' ' ' < "$T/term.log")"
fi

echo
if [ "$fails" -eq 0 ]; then echo "session-thaw: all checks passed"; else echo "session-thaw: $fails check(s) FAILED"; fi
[ "$fails" -eq 0 ]
