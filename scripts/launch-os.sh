#!/usr/bin/env bash
# launch-os.sh — boot Fabric OS in QEMU (graphical console window) AND open its
# desktop (served by the booted OS) in your browser. Splash -> login -> desktop.
set -uo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
KERNEL="$HERE/build/vmlinuz"; DISK="$HERE/build/rootfs.ext4"
MEM="${QEMU_MEM:-2048}"; DISP="${DISPLAY_BACKEND:-gtk}"
accel="tcg"; [ -r /dev/kvm ] && [ -w /dev/kvm ] && accel="kvm"
[ -r "$KERNEL" ] && [ -r "$DISK" ] || { echo "build the image first (scripts/build-rootfs.sh)"; exit 1; }
echo "Booting Fabric OS…  a QEMU window opens (console) and the desktop opens in your browser."
qemu-system-x86_64 \
  -machine q35,accel="$accel" -cpu host -smp 2 -m "$MEM" \
  -kernel "$KERNEL" -append "root=/dev/vda rw console=ttyS0 quiet" \
  -drive file="$DISK",if=virtio,format=raw \
  -netdev user,id=n0,hostfwd=tcp::8788-:8787 -device virtio-net-pci,netdev=n0 \
  -vga std -display "$DISP" -serial file:/tmp/fabric-os-serial.log &
QP=$!
trap 'kill $QP 2>/dev/null' EXIT
# wait for the OS to serve its desktop, then open it
for _ in $(seq 1 40); do
  [ "$(curl -s -o /dev/null -w '%{http_code}' --max-time 2 localhost:8788/ 2>/dev/null)" = "200" ] && break
  sleep 2
done
echo "Fabric OS desktop: http://localhost:8788   (login: admin / fabric)"
B="$(command -v brave-browser google-chrome chromium 2>/dev/null | head -1)"
[ -n "$B" ] && "$B" --user-data-dir="$(mktemp -d)/fp" --app="http://localhost:8788" >/dev/null 2>&1 &
wait $QP
