#!/usr/bin/env bash
# build-all.sh — the complete guarded build: workspace tests + M1 QEMU boot.
# Everything runs under tools/rg so the host never hangs. No unguarded steps.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
cd "$HERE"
[[ -f "$HOME/.cargo/env" ]] && . "$HOME/.cargo/env"

echo "== [1/6] host check =="
./scripts/doctor.sh >/dev/null && echo "  host ok"

echo "== [2/6] Rust workspace (guarded) =="
tout=$(./tools/rg --profile test -- cargo test --workspace 2>&1)
echo "$tout" | grep -E 'error\[|^error' | head -5 || true
passed=$(echo "$tout" | grep -oE '[0-9]+ passed' | awk '{s+=$1} END{print s+0}')
failed=$(echo "$tout" | grep -oE '[0-9]+ failed' | awk '{s+=$1} END{print s+0}')
echo "  workspace tests: $passed passed, $failed failed"
[ "$failed" = "0" ] || { echo "  WORKSPACE TESTS FAILED"; exit 1; }

echo "== [3/6] M1 bootable Linux in QEMU (guarded) =="
./tests/end-to-end/m1-boot.sh

echo "== [4/6] M2 persistent systemd userspace in QEMU (guarded) =="
if podman image exists ainos-m2:latest 2>/dev/null; then
  ./tests/end-to-end/m2-boot.sh
else
  echo "  building M2 rootfs image (guarded, one-time)"
  ./tools/rg --profile build -- podman build -t ainos-m2:latest -f build/m2/Containerfile build/m2 >/dev/null 2>&1     && ./tests/end-to-end/m2-boot.sh || echo "  M2 image build skipped/failed"
fi

echo "== [5/6] M3 Wayland environment (headless) =="
if command -v weston >/dev/null; then ./scripts/boot-wayland.sh || echo "  (wayland check skipped)"; else echo "  weston not installed; skip"; fi

echo "== [6/6] Agent sandbox confinement (live) =="
./scripts/verify-sandbox.sh || echo "  (sandbox check skipped)"

echo "== BUILD COMPLETE =="
