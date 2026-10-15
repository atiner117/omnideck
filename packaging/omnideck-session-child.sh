#!/usr/bin/env bash
# omnideck-session-child — gamescope's client in the OmniDeck session (installed as
# /usr/local/bin/omnideck-session-child by install-session.sh).
#
#   gamescope … -- omnideck-session-child /path/to/omnideck [args…]
#
# Runs OmniDeck, and once it has exited (by any route: power-menu quit, SIGTERM from a
# display-manager restart, a crash, SIGKILL) thaws every process the deck switcher left
# SIGSTOPped and SIGTERMs it, THEN exits so gamescopereaper can finish.
#
# Why: the switcher SIGSTOPs hidden silent apps ("switcher: froze silent hidden group").
# When its primary client exits, gamescopereaper SIGTERMs the remaining tree and waits for
# every descendant — and a stopped process never acts on SIGTERM, so gamescope stayed up
# on a dead screen forever (r2d2 couch box: `omnideck-display restart` and the power-menu
# quit both hung behind a frozen Brave/Jellyfin PWA; recovery was a hand-run
# `kill -CONT -- -<pgid>; kill -TERM -- -<pgid>` per stopped group). OmniDeck thaws its
# own frozen groups on a graceful exit, but a signal death, panic or SIGKILL skips that —
# this is the belt-and-braces layer that does not depend on OmniDeck's last moments.
#
# Source it (`. omnideck-session-child.sh`) to get the function alone, without running
# anything — packaging/test-session-thaw.sh does that.

# omnideck_thaw_stopped_groups [ROOT_PID]
#   CONT then TERM every job-control-stopped (state T, not tracer-stopped t) process that is
#   ours (same uid), in our session (same SID) and — when ROOT_PID is given — descended from
#   ROOT_PID (gamescopereaper is a subreaper, so OmniDeck's orphans reparent to it and the
#   walk still finds them). Whole process groups are signalled, so a frozen browser's
#   helpers wake with it; our own group is never group-signalled, only the stopped pid.
#   Prints one line per signalled target on stderr. Always returns 0.
omnideck_thaw_stopped_groups() {
  local root="${1:-}" self_sid self_pgid line pid pgid
  self_sid="$(ps -o sid= -p $$ | tr -d ' ')"
  self_pgid="$(ps -o pgid= -p $$ | tr -d ' ')"
  [ -n "$self_sid" ] || return 0
  # "pid pgid" per target, deduplicated by group.
  ps -e -o pid=,ppid=,pgid=,sid=,uid=,stat= | awk \
    -v root="$root" -v sid="$self_sid" -v uid="$(id -u)" -v self="$$" '
      { pp[$1] = $2; pg[$1] = $3; ss[$1] = $4; uu[$1] = $5; st[$1] = $6 }
      END {
        for (p in st) {
          if (st[p] !~ /^T/ || ss[p] != sid || uu[p] != uid || p == self) continue
          if (root != "") {
            q = p; n = 0
            while (q != root && q > 1 && n < 64) { q = pp[q]; n++ }
            if (q != root) continue
          }
          if (!(pg[p] in seen)) { seen[pg[p]] = 1; print p, pg[p] }
        }
      }' | while read -r pid pgid; do
    if [ "$pgid" = "$self_pgid" ]; then
      echo "omnideck-session-child: thawing + terminating stopped pid $pid" >&2
      kill -CONT "$pid" 2>/dev/null
      kill -TERM "$pid" 2>/dev/null
    else
      echo "omnideck-session-child: thawing + terminating stopped group $pgid" >&2
      kill -CONT -- "-$pgid" 2>/dev/null
      kill -TERM -- "-$pgid" 2>/dev/null
    fi
  done
  return 0
}

# Sourced: provide the function only.
[ "${BASH_SOURCE[0]}" = "$0" ] || return 0

set -u
[ $# -ge 1 ] || { echo "usage: omnideck-session-child /path/to/omnideck [args…]" >&2; exit 2; }

# Run OmniDeck as a background job so a TERM/INT/HUP aimed at THIS shell can be forwarded
# (a foreground child would defer the trap until it exited on its own). Async jobs of a
# non-interactive shell start with SIGINT/SIGQUIT ignored — reset them so OmniDeck keeps
# the dispositions it would have had as gamescope's direct client.
# Trap first (a signal in the gap would otherwise kill us with the default action); the
# subshell resets caught signals to their defaults, so OmniDeck does not inherit the trap.
child=""
trap '[ -n "$child" ] && kill -TERM "$child" 2>/dev/null' TERM INT HUP
( trap - INT QUIT; exec "$@" ) &
child=$!
# `wait` returns early (>128) when a trapped signal arrives; keep waiting while the child
# lives. Bash keeps the reaped child's status, so the final `wait` reports OmniDeck's own.
while kill -0 "$child" 2>/dev/null; do
  wait "$child"
done
wait "$child"
rc=$?
trap - TERM INT HUP

# $PPID is gamescopereaper here (or whatever runs us) — the root of the session's tree.
omnideck_thaw_stopped_groups "$PPID"
exit "$rc"
