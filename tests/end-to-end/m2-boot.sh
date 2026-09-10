#!/usr/bin/env bash
# M2 end-to-end test: bake the systemd rootfs into an ext4 disk and assert it
# boots to multi-user under systemd in QEMU. Guarded via tools/rg.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/../.." && pwd)"
podman image exists ainos-m2:latest 2>/dev/null || { echo "SKIP: build ainos-m2 image first (scripts/build-all.sh builds it)"; exit 0; }
"$HERE/scripts/build-rootfs.sh" >/dev/null 2>&1
QEMU_TIMEOUT=120 "$HERE/tools/rg" --profile vm -- "$HERE/scripts/boot-disk.sh" >/dev/null 2>"$HERE/build/boot-disk-test.err" \
  && { echo "M2 boot test: PASS"; exit 0; } \
  || { echo "M2 boot test: FAIL"; tail -6 "$HERE/build/boot-disk-test.err"; exit 1; }
