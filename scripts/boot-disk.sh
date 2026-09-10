#!/usr/bin/env bash
# boot-disk.sh — boot the M2 persistent ext4 rootfs (systemd) from a virtio
# disk in QEMU. Direct kernel boot (kernel has virtio_blk + ext4 built in, so
# no initramfs). Headless, guarded via tools/rg, timeout + serial capture.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
KERNEL="$HERE/build/vmlinuz"
DISK="$HERE/build/rootfs.ext4"
LOG="$HERE/build/boot-disk-serial.log"
MEM="${QEMU_MEM:-1024}"
TIMEOUT="${QEMU_TIMEOUT:-120}"
EXTRA="${QEMU_APPEND:-ainos.autopoweroff}"   # empty => interactive shell

[[ -r "$KERNEL" ]] || { echo "missing $KERNEL"; exit 1; }
[[ -r "$DISK"   ]] || { echo "missing $DISK (run scripts/build-rootfs.sh)"; exit 1; }
accel="tcg"; [[ -r /dev/kvm && -w /dev/kvm ]] && accel="kvm"
echo "boot: accel=$accel mem=${MEM}M timeout=${TIMEOUT}s append='$EXTRA'"

set +e
timeout --foreground "$TIMEOUT" \
  qemu-system-x86_64 \
    -machine q35,accel="$accel" -cpu host -smp 2 -m "$MEM" \
    -kernel "$KERNEL" \
    -append "root=/dev/vda rw console=ttyS0 systemd.show_status=0 $EXTRA" \
    -drive file="$DISK",if=virtio,format=raw \
    -nographic -no-reboot -display none -serial mon:stdio 2>&1 | tee "$LOG"
rc=$?
set -e
echo "----"
os_ok=1
if grep -q "OS-TEST COMPLETE" "$LOG"; then
  grep -q "BLOCKED" "$LOG" || os_ok=0            # demo1 injection must be blocked
  grep -q "read after revoke:       false" "$LOG" || os_ok=0  # demo3 revocation
fi
if grep -q "M2 USERSPACE OK" "$LOG" && grep -q "SELFTEST COMPLETE" "$LOG" && [ "$os_ok" = 1 ]; then
  echo "RESULT: PASS (systemd + ai-native-os security demos passed in guest)"; exit 0
else
  echo "RESULT: FAIL (no marker; rc=$rc). See $LOG"; exit 1
fi
