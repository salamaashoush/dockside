#!/usr/bin/env bash
set -euo pipefail

# Generate macOS .icns icon from a source image
# Usage: ./scripts/generate-icon.sh <source-image.png>
#
# The source image should be at least 1024x1024 pixels

SOURCE="${1:-assets/icon.png}"
OUTPUT="assets/AppIcon.icns"
ICONSET_DIR="assets/AppIcon.iconset"

if [[ ! -f "$SOURCE" ]]; then
    echo "Error: Source image not found: $SOURCE"
    echo ""
    echo "Please provide a source image (1024x1024 PNG recommended):"
    echo "  ./scripts/generate-icon.sh path/to/icon.png"
    echo ""
    echo "Or create a placeholder icon:"
    echo "  ./scripts/generate-placeholder-icon.sh"
    exit 1
fi

echo "Generating icon from: $SOURCE"

# Create iconset directory
rm -rf "$ICONSET_DIR"
mkdir -p "$ICONSET_DIR"

# Generate all required sizes
# Standard sizes
sips -z 16 16     "$SOURCE" --out "$ICONSET_DIR/icon_16x16.png"
sips -z 32 32     "$SOURCE" --out "$ICONSET_DIR/icon_16x16@2x.png"
sips -z 32 32     "$SOURCE" --out "$ICONSET_DIR/icon_32x32.png"
sips -z 64 64     "$SOURCE" --out "$ICONSET_DIR/icon_32x32@2x.png"
sips -z 128 128   "$SOURCE" --out "$ICONSET_DIR/icon_128x128.png"
sips -z 256 256   "$SOURCE" --out "$ICONSET_DIR/icon_128x128@2x.png"
sips -z 256 256   "$SOURCE" --out "$ICONSET_DIR/icon_256x256.png"
sips -z 512 512   "$SOURCE" --out "$ICONSET_DIR/icon_256x256@2x.png"
sips -z 512 512   "$SOURCE" --out "$ICONSET_DIR/icon_512x512.png"
sips -z 1024 1024 "$SOURCE" --out "$ICONSET_DIR/icon_512x512@2x.png"

# Convert to icns
iconutil -c icns "$ICONSET_DIR" -o "$OUTPUT"

# Cleanup
rm -rf "$ICONSET_DIR"

echo "Icon generated: $OUTPUT"
