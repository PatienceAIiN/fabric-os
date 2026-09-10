#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root_dir"

./tools/rg --profile test -- cargo test --workspace --locked
(cd website && npm ci && npm run build)
./scripts/generate-sbom.sh
./scripts/create-release-manifest.sh

echo "Release verification completed for source tests, website build, and SBOM inputs."
echo "Kernel, rootfs, firmware, model, signature, QEMU, recovery, and legal gates remain release-owner responsibilities."
