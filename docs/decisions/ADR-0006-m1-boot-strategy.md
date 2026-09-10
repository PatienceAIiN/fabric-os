# ADR-0006: M1 bootable-Linux strategy (distro kernel + busybox initramfs)

## Problem
M1 requires a Linux that boots and operates reliably in QEMU, on a 7 GB RAM
laptop, without destabilising it and without a passwordless root build farm.

## Existing approaches / alternatives
1. Compile a custom kernel + full rootfs (debootstrap/mmdebstrap). Rejected
   here: kernel compilation is explicitly barred on this host (ADR-0001), and
   debootstrap is not installed and needs root.
2. Download a prebuilt cloud image and boot it. Heavier (GBs), slower to
   iterate, more RAM in-guest.
3. **Distro kernel + minimal busybox initramfs, direct kernel boot.** Selected.

## Selected approach
- Reuse the host's upstream distro kernel (`/boot/vmlinuz-$(uname -r)`), staged
  to `build/vmlinuz`. No kernel compile (see ADR-0001).
- Build a ~700 KB initramfs from a statically linked busybox extracted from a
  rootless container image (no host package changes, no sudo). `/init` mounts
  proc/sys/dev, prints a boot marker, runs a smoke test, powers off.
- Boot with QEMU q35 + KVM, 512 MB guest, `-nographic -no-reboot`, a hard
  `timeout`, and serial capture. Success = boot markers present in the log.
- The VM is always launched under `tools/rg --profile vm` (RAM/CPU capped), so
  a hung guest cannot thrash the host. Verified: PASS, host PSI stayed low.

## Tradeoffs
This proves boot-to-userspace and gives an automatable regression test, but it
is an initramfs-only system: no persistent rootfs, no systemd yet. Those come
in M2 (full userspace), where a real disk image is assembled — likely via
rootless podman export rather than a root-only debootstrap.

## Security implications
Static busybox from a pinned image is a supply-chain input; future work pins by
digest and records provenance. The guest is fully isolated in QEMU/KVM.

## Performance implications
Boot-to-poweroff completes in seconds under KVM. `-cpu host` requires KVM;
falls back to TCG automatically when /dev/kvm is unavailable (slower).

## Future migration path
M2: assemble a persistent ext4 rootfs image with a real init (systemd), add
virtio-blk boot, keep the same guarded-QEMU harness and boot-marker assertions.
