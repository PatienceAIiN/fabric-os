#!/usr/bin/env bash
# build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.
# Live squashfs rootfs + generic dracut initramfs + GRUB (grub2-mkrescue).
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"; cd "$HERE"
KREL="$(uname -r)"
ISO_DIR="build/iso"; ROOTFS="build/iso-rootfs"; OUT="build/fabric-os.iso"
IMG="${IMG:-ainos-m2:latest}"
rm -rf "$ISO_DIR" "$ROOTFS"; mkdir -p "$ISO_DIR/LiveOS" "$ISO_DIR/boot/grub"

echo "[1/5] export Fabric rootfs (rootless podman)"
podman unshare bash -euo pipefail -c '
  HERE="'"$HERE"'"; IMG="'"$IMG"'"; ROOTFS="'"$ROOTFS"'"
  cid=$(podman create "$IMG"); mkdir -p "$ROOTFS"
  podman export "$cid" | tar -C "$ROOTFS" -xf -; podman rm "$cid" >/dev/null
  # /lib/modules for the running kernel so the live system has drivers
  mkdir -p "$ROOTFS/lib/modules"
  cp -a "/usr/lib/modules/'"$KREL"'" "$ROOTFS/lib/modules/" 2>/dev/null || true
  mksquashfs "$ROOTFS" "'"$HERE"'/'"$ISO_DIR"'/LiveOS/squashfs.img" -noappend -comp zstd -quiet
'
echo "  squashfs: $(du -h "$ISO_DIR/LiveOS/squashfs.img" | cut -f1)"

echo "[2/5] kernel"
cp -f "build/vmlinuz" "$ISO_DIR/boot/vmlinuz"

echo "[3/5] generic live initramfs (dracut, needs sudo)"
sudo dracut --force --no-hostonly --nomdadmconf --nolvmconf \
  --add "dmsquash-live" \
  --add-drivers "ahci nvme sd_mod usb_storage uas ext4 squashfs overlay loop isofs virtio_blk virtio_pci virtio_net e1000e" \
  "$ISO_DIR/boot/initramfs.img" "$KREL"
sudo chown "$(id -u):$(id -g)" "$ISO_DIR/boot/initramfs.img"

echo "[4/5] grub config"
cat > "$ISO_DIR/boot/grub/grub.cfg" <<GRUB
set timeout=5
set default=0
insmod all_video
menuentry "Fabric OS (live)" {
  linux /boot/vmlinuz root=live:CDLABEL=FABRICOS rd.live.image quiet console=tty0
  initrd /boot/initramfs.img
}
menuentry "Fabric OS (safe / verbose)" {
  linux /boot/vmlinuz root=live:CDLABEL=FABRICOS rd.live.image
  initrd /boot/initramfs.img
}
GRUB

echo "[5/5] build ISO (UEFI + BIOS hybrid)"
grub2-mkrescue -volid FABRICOS -o "$OUT" "$ISO_DIR" 2>/dev/null
echo "ISO: $OUT ($(du -h "$OUT" | cut -f1))"
