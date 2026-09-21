#!/bin/bash
# Regenerates Resources/AppIcon.icns, the menu bar icon and docs/logo.png from Resources/Icon/generate.swift
set -euo pipefail
cd "$(dirname "$0")"
OUT="$(mktemp -d)"
swift Resources/Icon/generate.swift "$OUT"
ICONSET="$OUT/AppIcon.iconset"; mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
    sips -z $s $s "$OUT/AppIcon-1024.png" --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
    d=$((s*2)); sips -z $d $d "$OUT/AppIcon-1024.png" --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o Resources/AppIcon.icns
cp "$OUT/MenuBarIcon.png" "$OUT/MenuBarIcon@2x.png" Resources/
cp "$OUT/logo-256.png" docs/logo.png
cp "$OUT/AppIcon-1024.png" docs/icon-1024.png
rm -rf "$OUT"
echo "✓ Resources/AppIcon.icns, Resources/MenuBarIcon{,@2x}.png, docs/logo.png"
