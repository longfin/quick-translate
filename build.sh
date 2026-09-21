#!/bin/bash
# Builds QuickTranslate.app into ./build
# Usage: ./build.sh [--install]
#   CODESIGN_IDENTITY="Developer ID Application: ..." ./build.sh   # optional stable signing identity
set -euo pipefail
cd "$(dirname "$0")"

APP_NAME="QuickTranslate"
APP_DIR="build/${APP_NAME}.app"

echo "▶ swift build -c release"
swift build -c release 2>&1 | grep -vE "^\[|Compiling|Emitting|Linking" || true
BIN_PATH="$(swift build -c release --show-bin-path)/${APP_NAME}"
[ -x "$BIN_PATH" ] || { echo "build failed: $BIN_PATH not found"; exit 1; }

rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"
cp "$BIN_PATH" "$APP_DIR/Contents/MacOS/${APP_NAME}"
cp Resources/Info.plist "$APP_DIR/Contents/Info.plist"
cp -R Resources/*.lproj "$APP_DIR/Contents/Resources/"
cp Resources/AppIcon.icns Resources/MenuBarIcon.png Resources/MenuBarIcon@2x.png "$APP_DIR/Contents/Resources/"
echo "APPL????" > "$APP_DIR/Contents/PkgInfo"

# Prefer an explicit identity; else a local self-signed "QuickTranslate Dev" cert if present; else ad-hoc.
# A stable identity keeps the Accessibility grant valid across rebuilds (see README).
if [ -n "${CODESIGN_IDENTITY:-}" ]; then
    IDENTITY="$CODESIGN_IDENTITY"
elif security find-identity -v -p codesigning 2>/dev/null | grep -q '"QuickTranslate Dev"'; then
    IDENTITY="QuickTranslate Dev"
else
    IDENTITY="-"
fi
echo "▶ codesign (identity: ${IDENTITY})"
codesign --force --deep --sign "$IDENTITY" "$APP_DIR"

echo "✓ built $APP_DIR"

if [ "${1:-}" = "--install" ]; then
    pkill -x "$APP_NAME" 2>/dev/null && sleep 1 || true
    rm -rf "/Applications/${APP_NAME}.app"
    cp -R "$APP_DIR" /Applications/
    echo "✓ installed to /Applications/${APP_NAME}.app"
    if [ "$IDENTITY" = "-" ]; then
        # Ad-hoc signatures change on every build, which silently invalidates the
        # Accessibility grant in TCC. Clear the stale entry so the app prompts again.
        tccutil reset Accessibility dev.swen.QuickTranslate >/dev/null 2>&1 || true
        tccutil reset ListenEvent dev.swen.QuickTranslate >/dev/null 2>&1 || true
        echo "ℹ ad-hoc signed: Accessibility permission reset — allow it again when prompted"
    fi
    open "/Applications/${APP_NAME}.app"
fi
