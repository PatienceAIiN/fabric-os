# ADR-0007: M2 persistent userspace (Debian + systemd via rootless podman)

## Problem
M2 needs a full, persistent Linux userspace with a real init, booting from disk
in QEMU, on a 7 GB laptop, without host root for image assembly.

## Existing approaches / alternatives
1. debootstrap/mmdebstrap into a loop-mounted image. Needs root and packages
   not installed here.
2. Download a prebuilt cloud image. Heavier, opaque, harder to customise.
3. **Build the rootfs with rootless podman, export it, bake an ext4 image with
   `mkfs.ext4 -d`, direct-kernel-boot from a virtio disk.** Selected.

## Selected approach
- Rootfs defined declaratively in `build/m2/Containerfile`: `debian:stable-slim`
  + `systemd-sysv` + `udev`; original `/etc/os-release`; serial autologin on
  ttyS0; a self-test unit that prints boot markers, maintains a persistent
  boot counter, and powers off only when the kernel cmdline has
  `ainos.autopoweroff`.
- `scripts/build-rootfs.sh` runs export + `mkfs.ext4 -d` inside `podman
  unshare` so container uid 0 maps to namespace root and ownership is recorded
  correctly WITHOUT host root and WITHOUT a loop mount.
- Boot: this distro kernel has `CONFIG_VIRTIO_BLK=y`, `CONFIG_VIRTIO_PCI=y`,
  `CONFIG_EXT4_FS=y` (all built in), so `scripts/boot-disk.sh` direct-boots
  with `root=/dev/vda` and NO initramfs. Headless, guarded, timeout + serial.

## Evidence
systemd runs as PID 1 (`/usr/lib/systemd/systemd`), reaches multi-user, root
remounts rw from /dev/vda. Persistence proven: two consecutive boots of the
same disk report boot count 1 then 2. Host stayed responsive (guarded VM).

## Tradeoffs
Debian userspace on a Fedora host kernel (fine: kernel/userspace decoupled).
No bootloader yet (direct kernel boot); a UEFI bootloader (GRUB/systemd-boot)
and an in-image kernel come in a later milestone toward self-hosting install.
Image files are currently owned as recorded by export; adequate for boot.

## Security implications
Base image is a supply-chain input; future work pins by digest and records
provenance/signature (see LANDSCAPE, section 70). Guest fully isolated in KVM.

## Future migration path
M3+: add a bootloader and self-contained kernel for full-disk boot; layer the
ai-native-os daemons (`userspace/*`) into the image; move from Debian base to a
purpose-built rootfs once the daemon set stabilises.
