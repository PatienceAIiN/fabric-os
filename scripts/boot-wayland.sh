#!/usr/bin/env bash
# boot-wayland.sh — bring up the Wayland graphical environment (M3) headless and
# prove a client connects. Uses the upstream Weston compositor (prefer upstream,
# ADR-0010). Headless + noop/pixman renderer: no GPU needed, safe on any host.
set -uo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
LOG="$HERE/build/wayland.log"; mkdir -p "$HERE/build"
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
DISP="wayland-ainos"
SOCK="$XDG_RUNTIME_DIR/$DISP"
rm -f "$SOCK"

weston --backend=headless --width=1280 --height=800 --socket="$DISP" \
       --idle-time=0 >"$LOG" 2>&1 &
WPID=$!
# wait up to 8s for the socket
for _ in $(seq 1 40); do [ -S "$SOCK" ] && break; sleep 0.2; done

result=1
if [ -S "$SOCK" ]; then
  echo "compositor up: socket $DISP created"
  # connect a real client; it renders offscreen but must connect+run briefly
  WAYLAND_DISPLAY="$DISP" timeout 3 weston-terminal >/dev/null 2>&1 &
  CPID=$!
  sleep 1.5
  if kill -0 "$CPID" 2>/dev/null || grep -qi "connected\|created surface\|client" "$LOG"; then
    echo "client connected to compositor"
    result=0
  fi
  kill "$CPID" 2>/dev/null
fi
kill "$WPID" 2>/dev/null; wait "$WPID" 2>/dev/null
echo "----"
if [ "$result" = 0 ]; then echo "RESULT: PASS (Wayland env up, client connected)"; else
  echo "RESULT: FAIL (see $LOG)"; tail -8 "$LOG"; fi
exit "$result"
