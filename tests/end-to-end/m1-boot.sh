#!/usr/bin/env bash
# M1 end-to-end test: assemble initramfs and assert it boots to userspace in
# QEMU. Runs the VM under tools/rg so CI/local runs cannot thrash the host.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/../.." && pwd)"
[[ -r "$HERE/build/rootfs-src/busybox.uclibc" ]] || { echo "SKIP: no busybox (run scripts that extract it)"; exit 0; }
"$HERE/scripts/build-initramfs.sh" >/dev/null
QEMU_TIMEOUT=60 "$HERE/tools/rg" --profile vm -- "$HERE/scripts/boot-qemu.sh" >/dev/null 2>"$HERE/build/boot-test.err" \
  && { echo "M1 boot test: PASS"; exit 0; } \
  || { echo "M1 boot test: FAIL"; tail -5 "$HERE/build/boot-test.err"; exit 1; }
