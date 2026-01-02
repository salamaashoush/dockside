#!/usr/bin/env bash
set -euo pipefail

# Generate a placeholder icon for development
# Uses ImageMagick if available, otherwise creates a simple colored square

OUTPUT_PNG="assets/icon.png"
OUTPUT_ICNS="assets/AppIcon.icns"
ICONSET_DIR="assets/AppIcon.iconset"

mkdir -p assets

# Check if ImageMagick is available
if command -v convert &> /dev/null; then
    echo "Generating icon with ImageMagick..."

    # Create a modern-looking icon with gradient background
    convert -size 1024x1024 \
        -define gradient:angle=135 \
        gradient:'#1e3a5f'-'#0d1b2a' \
        -fill '#00d9ff' \
        -font Helvetica-Bold \
        -pointsize 500 \
        -gravity center \
        -annotate 0 'D' \
        -alpha set \
        \( +clone -channel A -morphology Distance Euclidean:1,50\! +channel +level-colors '#1e3a5f' \) \
        +swap -gravity center -composite \
        "$OUTPUT_PNG"

    echo "Generated: $OUTPUT_PNG"
else
    echo "ImageMagick not found. Creating simple placeholder..."
    echo "Install ImageMagick for better icon: brew install imagemagick"

    # Create a simple 1024x1024 PNG using sips (available on macOS)
    # We'll use a base64-encoded minimal blue PNG
    python3 << 'PYTHON'
import struct
import zlib
import base64

def create_png(width, height, color):
    def png_chunk(chunk_type, data):
        chunk_len = struct.pack('>I', len(data))
        chunk_crc = struct.pack('>I', zlib.crc32(chunk_type + data) & 0xffffffff)
        return chunk_len + chunk_type + data + chunk_crc

    # PNG signature
    signature = b'\x89PNG\r\n\x1a\n'

    # IHDR chunk
    ihdr_data = struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0)
    ihdr = png_chunk(b'IHDR', ihdr_data)

    # IDAT chunk (image data)
    raw_data = b''
    r, g, b = color
    for y in range(height):
        raw_data += b'\x00'  # filter type
        for x in range(width):
            raw_data += bytes([r, g, b])

    compressed = zlib.compress(raw_data, 9)
    idat = png_chunk(b'IDAT', compressed)

    # IEND chunk
    iend = png_chunk(b'IEND', b'')

    return signature + ihdr + idat + iend

# Deep blue color
png_data = create_png(1024, 1024, (30, 58, 95))

with open('assets/icon.png', 'wb') as f:
    f.write(png_data)

print("Created simple placeholder icon")
PYTHON
fi

# Generate .icns from the PNG
if [[ -f "$OUTPUT_PNG" ]]; then
    echo "Generating .icns file..."

    rm -rf "$ICONSET_DIR"
    mkdir -p "$ICONSET_DIR"

    sips -z 16 16     "$OUTPUT_PNG" --out "$ICONSET_DIR/icon_16x16.png" 2>/dev/null
    sips -z 32 32     "$OUTPUT_PNG" --out "$ICONSET_DIR/icon_16x16@2x.png" 2>/dev/null
    sips -z 32 32     "$OUTPUT_PNG" --out "$ICONSET_DIR/icon_32x32.png" 2>/dev/null
    sips -z 64 64     "$OUTPUT_PNG" --out "$ICONSET_DIR/icon_32x32@2x.png" 2>/dev/null
    sips -z 128 128   "$OUTPUT_PNG" --out "$ICONSET_DIR/icon_128x128.png" 2>/dev/null
    sips -z 256 256   "$OUTPUT_PNG" --out "$ICONSET_DIR/icon_128x128@2x.png" 2>/dev/null
    sips -z 256 256   "$OUTPUT_PNG" --out "$ICONSET_DIR/icon_256x256.png" 2>/dev/null
    sips -z 512 512   "$OUTPUT_PNG" --out "$ICONSET_DIR/icon_256x256@2x.png" 2>/dev/null
    sips -z 512 512   "$OUTPUT_PNG" --out "$ICONSET_DIR/icon_512x512.png" 2>/dev/null
    sips -z 1024 1024 "$OUTPUT_PNG" --out "$ICONSET_DIR/icon_512x512@2x.png" 2>/dev/null

    iconutil -c icns "$ICONSET_DIR" -o "$OUTPUT_ICNS"
    rm -rf "$ICONSET_DIR"

    echo "Generated: $OUTPUT_ICNS"
fi

echo ""
echo "Done! To use a custom icon, run:"
echo "  ./scripts/generate-icon.sh path/to/your-icon.png"
