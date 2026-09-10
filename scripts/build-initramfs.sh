#!/usr/bin/env bash
# build-initramfs.sh — assemble a minimal busybox initramfs for M1 QEMU boot.
# No sudo, no kernel compile. Uses a static busybox extracted from a container.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
BB="${1:-$HERE/build/rootfs-src/busybox.uclibc}"
OUT="$HERE/build/initramfs.cpio.gz"
[[ -x "$BB" || -f "$BB" ]] || { echo "no busybox at $BB"; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work"/{bin,sbin,proc,sys,dev,etc}
install -m 0755 "$BB" "$work/bin/busybox"
for a in sh mount umount ls cat uname poweroff dmesg mkdir sleep echo grep; do
  ln -sf busybox "$work/bin/$a"
done

cat > "$work/init" <<'INIT'
#!/bin/sh
/bin/busybox mount -t proc  none /proc
/bin/busybox mount -t sysfs none /sys
/bin/busybox mount -t devtmpfs none /dev 2>/dev/null
echo
echo "==== AI-NATIVE-OS M1 BOOT OK ===="
echo "kernel: $(/bin/busybox uname -sr)"
echo "cpus:   $(/bin/busybox grep -c ^processor /proc/cpuinfo)"
echo "mem:    $(/bin/busybox grep MemTotal /proc/meminfo)"
echo "init:   pid $$ as $(/bin/busybox id -u 2>/dev/null || echo 0)"
echo "==== SMOKE TEST COMPLETE ===="
/bin/busybox poweroff -f
INIT
chmod 0755 "$work/init"

( cd "$work" && find . -print0 | cpio --null -ov --format=newc 2>/dev/null | gzip -9 ) > "$OUT"
echo "wrote $OUT ($(du -h "$OUT" | cut -f1))"
