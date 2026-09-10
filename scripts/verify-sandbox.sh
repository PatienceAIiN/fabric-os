#!/usr/bin/env bash
# verify-sandbox.sh — live proof (M25) that the capability->sandbox mapping
# actually confines a process via systemd user-service sandboxing. An agent
# with no fs.write capability gets a read-only world; writes are blocked.
set -uo pipefail
d=$(mktemp -d)
systemd-run --user --wait --pipe -q -- /bin/sh -c "echo hi > $d/plain" >/dev/null 2>&1
base=$([ -f "$d/plain" ] && echo ok || echo fail)
systemd-run --user --wait --pipe -q \
  --property=ProtectSystem=strict --property=NoNewPrivileges=yes \
  --property=ReadOnlyPaths="$d" \
  -- /bin/sh -c "echo hi > $d/blocked" >/dev/null 2>&1
blocked=$([ -f "$d/blocked" ] && echo "LEAKED" || echo "blocked")
rm -rf "$d"
echo "unsandboxed write: $base ; sandboxed write: $blocked"
[ "$base" = ok ] && [ "$blocked" = blocked ] && { echo "RESULT: PASS"; exit 0; }
echo "RESULT: FAIL"; exit 1
