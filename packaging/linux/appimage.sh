#!/bin/sh
# AppImage do LazyAICode a partir do build de release (cargo xtask dist appimage).
# Precisa do appimagetool no PATH (https://github.com/AppImage/appimagetool).
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT="$ROOT/packaging/out"
APPDIR="$OUT/LazyAICode.AppDir"
ARCH=$(uname -m)
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/128x128/apps"
install -m755 "$ROOT/target/release/lazyaicode" "$ROOT/target/release/lazyaicode-cli" "$APPDIR/usr/bin/"
install -m644 "$ROOT/packaging/linux/lazyaicode.desktop" "$APPDIR/usr/share/applications/lazyaicode.desktop"
install -m644 "$ROOT/packaging/linux/lazyaicode.desktop" "$APPDIR/lazyaicode.desktop"
install -m644 "$ROOT/assets/icons/128x128.png" "$APPDIR/usr/share/icons/hicolor/128x128/apps/lazyaicode.png"
install -m644 "$ROOT/assets/icons/128x128.png" "$APPDIR/lazyaicode.png"
cat > "$APPDIR/AppRun" <<'RUN'
#!/bin/sh
HERE=$(dirname "$(readlink -f "$0")")
# `lazyaicode-cli …` também roda de dentro do AppImage: LazyAICode.AppImage cli <args>
if [ "${1:-}" = "cli" ]; then shift; exec "$HERE/usr/bin/lazyaicode-cli" "$@"; fi
exec "$HERE/usr/bin/lazyaicode" "$@"
RUN
chmod +x "$APPDIR/AppRun"
ARCH=$ARCH appimagetool "$APPDIR" "$OUT/LazyAICode-${VERSION:-dev}-$ARCH.AppImage"
echo "$OUT/LazyAICode-${VERSION:-dev}-$ARCH.AppImage"
