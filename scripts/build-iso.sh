 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.#!/usr/bin/env bash
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.# build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.# Live squashfs rootfs + generic dracut initramfs + GRUB (grub2-mkrescue).
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.set -euo pipefail
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.HERE="$(cd "$(dirname "$0")/.." && pwd)"; cd "$HERE"
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.KREL="$(uname -r)"
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.ISO_DIR="build/iso"; ROOTFS="build/iso-rootfs"; OUT="build/fabric-os.iso"
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.IMG="${IMG:-ainos-m2:latest}"
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.rm -rf "$ISO_DIR" "$ROOTFS"; mkdir -p "$ISO_DIR/LiveOS" "$ISO_DIR/boot/grub"
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.echo "[1/5] export Fabric rootfs (rootless podman)"
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.podman unshare bash -euo pipefail -c '
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.  HERE="'"$HERE"'"; IMG="'"$IMG"'"; ROOTFS="'"$ROOTFS"'"
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.  cid=$(podman create "$IMG"); mkdir -p "$ROOTFS"
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.  podman export "$cid" | tar -C "$ROOTFS" -xf -; podman rm "$cid" >/dev/null
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.  # /lib/modules for the running kernel so the live system has drivers
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.  mkdir -p "$ROOTFS/lib/modules"
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.  cp -a "/usr/lib/modules/'"$KREL"'" "$ROOTFS/lib/modules/" 2>/dev/null || true
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.  mksquashfs "$ROOTFS" "'"$HERE"'/'"$ISO_DIR"'/LiveOS/squashfs.img" -noappend -comp zstd -quiet
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.'
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.echo "  squashfs: $(du -h "$ISO_DIR/LiveOS/squashfs.img" | cut -f1)"
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.echo "[2/5] kernel"
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.cp -f "build/vmlinuz" "$ISO_DIR/boot/vmlinuz"
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.echo "[3/5] generic live initramfs (dracut, needs sudo)"
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.sudo dracut --force --no-hostonly --nomdadmconf --nolvmconf \
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.  --add "dmsquash-live" \
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.  --add-drivers "ahci nvme sd_mod usb_storage uas ext4 squashfs overlay loop isofs virtio_blk virtio_pci virtio_net e1000e" \
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.  "$ISO_DIR/boot/initramfs.img" "$KREL"
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.sudo chown "$(id -u):$(id -g)" "$ISO_DIR/boot/initramfs.img"
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.echo "[4/5] grub config"
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.cat > "$ISO_DIR/boot/grub/grub.cfg" <<GRUB
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.set timeout=5
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.set default=0
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.insmod all_video
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.menuentry "Fabric OS (live)" {
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.  linux /boot/vmlinuz root=live:CDLABEL=FABRICOS rd.live.image quiet console=tty0
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.  initrd /boot/initramfs.img
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.}
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.menuentry "Fabric OS (safe / verbose)" {
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.  linux /boot/vmlinuz root=live:CDLABEL=FABRICOS rd.live.image
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.  initrd /boot/initramfs.img
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.}
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.GRUB
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.echo "[5/5] build ISO (UEFI + BIOS hybrid)"
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.grub2-mkrescue -volid FABRICOS -o "$OUT" "$ISO_DIR" 2>/dev/null
 build-iso.sh — build a bootable hybrid (UEFI+BIOS) live ISO of Fabric OS.echo "ISO: $OUT ($(du -h "$OUT" | cut -f1))"
