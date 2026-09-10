#!/usr/bin/env bash
# release.sh — experimental release: verify everything, then stage artifacts.
# Runs the full guarded build + boots + demos + benchmarks. Never ships if any
# check fails.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"; cd "$HERE"
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
VER=$(cat VERSION)
echo "== ai-native-os release $VER =="
./scripts/build-all.sh
echo "== benchmarks =="
./tools/rg --profile build -- cargo build --release -p bench >/dev/null 2>&1
./tools/rg --profile tiny -- ./target/release/aios-bench 100000
echo "== staging =="
OUT="build/release/$VER"; mkdir -p "$OUT"
cp -f README.md ARCHITECTURE.md ROADMAP.md SECURITY.md CHANGELOG.md VERSION docs/STATUS.md "$OUT/" 2>/dev/null || true
./tools/rg --profile build -- cargo build --release -p aios -p aiosd >/dev/null 2>&1
cp -f target/release/aios target/release/aiosd "$OUT/" 2>/dev/null || true
echo "staged in $OUT:"; ls -1 "$OUT"
echo "release $VER OK (experimental channel)"
