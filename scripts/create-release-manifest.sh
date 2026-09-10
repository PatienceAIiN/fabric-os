#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
release_dir="${root_dir}/build/release"
mkdir -p "$release_dir"
manifest="${release_dir}/SHA256SUMS"
: > "$manifest"

for artifact in \
  "${root_dir}/build/rootfs.ext4" \
  "${root_dir}/build/vmlinuz" \
  "${root_dir}/build/initramfs.cpio.gz" \
  "${root_dir}/website/dist/index.html"; do
  if [[ -f "$artifact" ]]; then
    (cd "$root_dir" && sha256sum "${artifact#${root_dir}/}") >> "$manifest"
  fi
done

if [[ ! -s "$manifest" ]]; then
  echo "No release artifacts found; build the OS and website first." >&2
  exit 1
fi

echo "Wrote ${manifest}"
