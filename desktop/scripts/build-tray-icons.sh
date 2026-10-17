#!/bin/bash
# Renders the tray glyph (src-tauri/icons/tray.svg, white + alpha) to the PNGs the app embeds.
# Needs rsvg-convert (librsvg: `brew install librsvg`, `apt install librsvg2-bin`).
set -euo pipefail
cd "$(dirname "$0")/../src-tauri/icons"
for px in 32 64; do
    rsvg-convert -w "$px" -h "$px" tray.svg -o "tray-${px}.png"
done
echo "✓ tray-32.png, tray-64.png"
