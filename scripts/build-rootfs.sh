#!/usr/bin/env bash
# build-rootfs.sh — export the M2 systemd image into a bootable ext4 disk.
# Runs under `podman unshare` so container uid 0 maps to a namespace root and
# mkfs.ext4 -d records correct ownership WITHOUT host root. No sudo, no loop mount.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
IMG="${IMG:-ainos-m2:latest}"
OUT="$HERE/build/rootfs.ext4"
SIZE="${SIZE:-1536M}"
MKFS=/usr/sbin/mkfs.ext4

podman unshare bash -euo pipefail -c '
  HERE="'"$HERE"'"; IMG="'"$IMG"'"; OUT="'"$OUT"'"; SIZE="'"$SIZE"'"; MKFS="'"$MKFS"'"
  rootdir="$HERE/build/m2/rootfs"
  rm -rf "$rootdir"; mkdir -p "$rootdir"
  cid=$(podman create "$IMG")
  podman export "$cid" | tar -C "$rootdir" -xf -
  podman rm "$cid" >/dev/null
  # fstab: root is mounted rw by the kernel via root=/dev/vda; keep proc/sys.
  printf "/dev/vda / ext4 defaults 0 1\n" > "$rootdir/etc/fstab"
  rm -f "$OUT"
  "$MKFS" -q -F -L ainos-root -d "$rootdir" "$OUT" "$SIZE"
'
echo "wrote $OUT ($(du -h "$OUT" | cut -f1)); files: $(find build/m2/rootfs | wc -l)"
