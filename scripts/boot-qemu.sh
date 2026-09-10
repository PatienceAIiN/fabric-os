#!/usr/bin/env bash
# boot-qemu.sh — boot the M1 initramfs in QEMU, headless, guarded, with a
# timeout. Asserts the guest reached userspace. Never runs unguarded: it is
# invoked through tools/rg --profile vm so it cannot thrash the host.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
KERNEL="$HERE/build/vmlinuz"
INITRD="$HERE/build/initramfs.cpio.gz"
LOG="$HERE/build/boot-serial.log"
MEM="${QEMU_MEM:-512}"
TIMEOUT="${QEMU_TIMEOUT:-60}"

[[ -r "$KERNEL" ]] || { echo "missing kernel $KERNEL (run: sudo cp /boot/vmlinuz-\$(uname -r) build/vmlinuz)"; exit 1; }
[[ -r "$INITRD" ]] || { echo "missing initramfs; run scripts/build-initramfs.sh"; exit 1; }

accel="tcg"; [[ -r /dev/kvm && -w /dev/kvm ]] && accel="kvm"
echo "boot: accel=$accel mem=${MEM}M timeout=${TIMEOUT}s"

set +e
timeout --foreground "$TIMEOUT" \
  qemu-system-x86_64 \
    -machine q35,accel="$accel" \
    -cpu host -smp 2 -m "$MEM" \
    -kernel "$KERNEL" \
    -initrd "$INITRD" \
    -append "console=ttyS0 rdinit=/init panic=-1 quiet" \
    -nographic -no-reboot -display none \
    -serial mon:stdio 2>&1 | tee "$LOG"
rc=$?
set -e

echo "----"
if grep -q "M1 BOOT OK" "$LOG" && grep -q "SMOKE TEST COMPLETE" "$LOG"; then
  echo "RESULT: PASS (guest reached userspace and ran init)"
  exit 0
else
  echo "RESULT: FAIL (no boot marker; rc=$rc). See $LOG"
  exit 1
fi
