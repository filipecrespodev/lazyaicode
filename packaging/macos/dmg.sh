#!/bin/sh
# LazyAICode.app + DMG assinado e notarizado (cargo xtask dist dmg).
#
# Assinatura (opcional fora do CI):
#   APPLE_SIGNING_IDENTITY="Developer ID Application: Nome (TEAMID)"
# Notarização (opcional):
#   APPLE_ID, APPLE_TEAM_ID, APPLE_APP_PASSWORD (senha de app do appleid.apple.com)
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT="$ROOT/packaging/out"
APP="$OUT/LazyAICode.app"
VERSION=${VERSION:-0.0.0}
ARCH=$(uname -m)
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
install -m755 "$ROOT/target/release/lazyaicode" "$ROOT/target/release/lazyaicode-cli" "$APP/Contents/MacOS/"
cp "$ROOT/assets/icons/icon.icns" "$APP/Contents/Resources/icon.icns"
sed "s/__VERSION__/$VERSION/g" "$ROOT/packaging/macos/Info.plist" > "$APP/Contents/Info.plist"

if [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
  for bin in lazyaicode-cli lazyaicode; do
    codesign --force --options runtime --timestamp --entitlements "$ROOT/packaging/macos/entitlements.plist" \
      --sign "$APPLE_SIGNING_IDENTITY" "$APP/Contents/MacOS/$bin"
  done
  codesign --force --options runtime --timestamp --entitlements "$ROOT/packaging/macos/entitlements.plist" \
    --sign "$APPLE_SIGNING_IDENTITY" "$APP"
  codesign --verify --deep --strict "$APP"
fi

DMG="$OUT/LazyAICode-$VERSION-macos-$ARCH.dmg"
STAGE="$OUT/dmg-stage"
rm -rf "$STAGE" "$DMG"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname LazyAICode -srcfolder "$STAGE" -ov -format UDZO "$DMG"
rm -rf "$STAGE"

if [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
  codesign --force --timestamp --sign "$APPLE_SIGNING_IDENTITY" "$DMG"
fi
if [ -n "${APPLE_ID:-}" ] && [ -n "${APPLE_TEAM_ID:-}" ] && [ -n "${APPLE_APP_PASSWORD:-}" ]; then
  xcrun notarytool submit "$DMG" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD" --wait
  xcrun stapler staple "$DMG"
  xcrun stapler validate "$DMG"
fi
# a CLI também vai num zip à parte (o .app só tem o app gráfico no PATH do Finder)
(cd "$ROOT/target/release" && zip -q "$OUT/lazyaicode-cli-$VERSION-macos-$ARCH.zip" lazyaicode-cli)
echo "$DMG"
