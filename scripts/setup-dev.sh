#!/usr/bin/env bash
# setup-dev.sh — reproducible, sudo-free dev setup for a low-RAM host.
# Installs a user-local Rust toolchain via rustup (no root). Prints, but does
# not run, the packages that need sudo. All heavy steps run under tools/rg.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
RG="$HERE/tools/rg"

echo "[1/3] host check"
"$HERE/scripts/doctor.sh" || true

echo "[2/3] Rust toolchain (user-local, no sudo)"
if command -v cargo >/dev/null 2>&1; then
  echo "  cargo present: $(cargo --version)"
else
  export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
  export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
  echo "  installing rustup + stable into $CARGO_HOME (guarded, minimal profile)"
  tmp="$(mktemp)"; curl -fsSL https://sh.rustup.rs -o "$tmp"
  # cap the installer + first toolchain download so the desktop stays smooth
  "$RG" --profile build -- sh "$tmp" -y --profile minimal --default-toolchain stable --no-modify-path
  rm -f "$tmp"
  # keep builds lean on a small machine: thin LTO off, capped codegen units,
  # and default to guarded job count via .cargo/config (written by workspace).
  echo '  add to your shell rc:  . "$HOME/.cargo/env"'
  . "$CARGO_HOME/env"
  echo "  installed: $(cargo --version)"
fi

echo "[3/3] optional system packages (need sudo; run yourself if/when needed)"
cat <<'PKGS'
  For M1 (QEMU boot) and later kernel/eBPF work:
    sudo dnf install qemu-kvm qemu-img edk2-ovmf \
                     bpftool flex bison protobuf-compiler \
                     kernel-devel elfutils-libelf-devel libbpf-devel
  These are intentionally NOT auto-installed (no passwordless sudo, and to
  keep this machine's package state under your control).
PKGS
echo "done. Next:  $HERE/tools/rg --profile test -- cargo test"
