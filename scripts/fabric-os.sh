#!/usr/bin/env bash
# fabric-os.sh — launch the Fabric OS desktop (splash -> login -> desktop),
# fullscreen, on your display. Ctrl+W or close the window to exit.
set -uo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"; cd "$HERE"
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
[ -f scripts/ai-env.sh ] && source scripts/ai-env.sh >/dev/null 2>&1 || true
ADDR="127.0.0.1:8787"; export FABRIC_ADDR="$ADDR"
# build if needed
[ -x target/release/fabric-desktop ] || ./tools/rg --profile build -- cargo build --release -p fabric-desktop >/dev/null 2>&1
# start server
./target/release/fabric-desktop >/tmp/fabric-os.log 2>&1 &
SRV=$!
trap 'kill $SRV 2>/dev/null' EXIT
sleep 1
echo "Fabric OS desktop: http://$ADDR   (login: admin / fabric)"
BROWSER="$(command -v brave-browser google-chrome chromium 2>/dev/null | head -1)"
MODE="${1:-kiosk}"   # kiosk (fullscreen) | app (window)
PROFILE="$(mktemp -d)/fabric-profile"
if [ "$MODE" = app ]; then
  "$BROWSER" --user-data-dir="$PROFILE" --app="http://$ADDR" --window-size=1400,880 >/dev/null 2>&1
else
  "$BROWSER" --user-data-dir="$PROFILE" --kiosk --app="http://$ADDR" >/dev/null 2>&1
fi
