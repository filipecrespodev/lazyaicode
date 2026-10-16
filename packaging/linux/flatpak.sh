#!/bin/sh
# Bundle .flatpak (cargo xtask dist flatpak). Precisa de flatpak-builder.
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT="$ROOT/packaging/out"
mkdir -p "$OUT"
cd "$ROOT"
flatpak-builder --force-clean --repo="$OUT/flatpak-repo" "$OUT/flatpak-build" packaging/linux/flatpak/ai.lazyaicode.LazyAICode.yml
flatpak build-bundle "$OUT/flatpak-repo" "$OUT/LazyAICode-${VERSION:-dev}.flatpak" ai.lazyaicode.LazyAICode
echo "$OUT/LazyAICode-${VERSION:-dev}.flatpak"
