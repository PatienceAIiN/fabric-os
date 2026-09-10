#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out_dir="${root_dir}/build/release"
mkdir -p "$out_dir"

cargo_bin="$(command -v cargo || true)"
if [[ -z "$cargo_bin" && -x "${CARGO_HOME:-${HOME}/.cargo}/bin/cargo" ]]; then
  cargo_bin="${CARGO_HOME:-${HOME}/.cargo}/bin/cargo"
fi
if [[ -z "$cargo_bin" ]]; then
  echo "cargo is required; run scripts/setup-dev.sh first" >&2
  exit 1
fi
"$cargo_bin" metadata --format-version 1 --locked --no-deps > "${out_dir}/cargo-metadata.json"

if [[ -f "${root_dir}/website/package-lock.json" ]]; then
  npm --prefix "${root_dir}/website" sbom --package-lock-only --sbom-format=cyclonedx > "${out_dir}/website-sbom.json"
fi

echo "Wrote SBOM inputs to ${out_dir}. Add kernel, rootfs, firmware, and model records from the release build before signing."
