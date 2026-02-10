#!/usr/bin/env python3
"""
Convert an image to ESC/POS raster format for thermal printers.
Supports both ESC * (column-based) and GS v (raster) formats.
"""

import sys
from PIL import Image
import struct

def image_to_escpos_raster(image_path, max_width=250, threshold=128):
    """
    Convert image to ESC * column-based raster format.

    Args:
        image_path: Path to image file
        max_width: Maximum width in pixels (thermal printers typically 384 or 576)
        threshold: Grayscale threshold for black/white conversion (0-255)

    Returns:
        bytes: ESC/POS command sequence
    """
    # Open and convert to grayscale
    img = Image.open(image_path).convert('L')

    # Resize if needed, maintaining aspect ratio
    if img.width > max_width:
        aspect = img.height / img.width
        img = img.resize((max_width, int(max_width * aspect)), Image.Resampling.LANCZOS)

    width = img.width
    height = img.height

    # Convert to monochrome (black and white)
    img = img.point(lambda x: 0 if x < threshold else 255, mode='1')

    # Use GS v format instead of ESC * to avoid horizontal banding
    # GS v is cleaner for images as it doesn't require strip-by-strip processing
    result = bytearray()
    result.extend(b'\x1D\x76\x30')  # GS v 0
    result.append(0)  # mode = normal

    # Raster data: width must be padded to byte boundary
    bytes_per_row = (width + 7) // 8

    # Width (in BYTES) and height (in pixels) in little-endian
    # According to ESC/POS spec: xL,xH = bytes, yL,yH = dots
    result.extend(struct.pack('<H', bytes_per_row))
    result.extend(struct.pack('<H', height))

    for y in range(height):
        for byte_idx in range(bytes_per_row):
            byte_val = 0
            for bit in range(8):
                x = byte_idx * 8 + bit
                if x < width:
                    pixel = img.getpixel((x, y))
                    if pixel == 0:  # black = print dot
                        byte_val |= (1 << (7 - bit))
            result.append(byte_val)

    return bytes(result)

def image_to_gs_v_raster(image_path, max_width=384, threshold=128):
    """
    Convert image to GS v 0 raster format.

    Args:
        image_path: Path to image file
        max_width: Maximum width in pixels
        threshold: Grayscale threshold for black/white conversion (0-255)

    Returns:
        bytes: ESC/POS command sequence
    """
    # Open and convert to grayscale
    img = Image.open(image_path).convert('L')

    # Resize if needed
    if img.width > max_width:
        aspect = img.height / img.width
        img = img.resize((max_width, int(max_width * aspect)), Image.Resampling.LANCZOS)

    width = img.width
    height = img.height

    # Convert to monochrome
    img = img.point(lambda x: 0 if x < threshold else 255, mode='1')

    # Build GS v 0 command
    # Format: GS v 0 m xL xH yL yH [raster data]
    # m = mode (0 = normal, 1 = double width, 2 = double height, 3 = quad)
    result = bytearray()
    result.extend(b'\x1D\x76\x30')  # GS v 0
    result.append(0)  # mode = normal

    # Raster data: width must be padded to byte boundary
    bytes_per_row = (width + 7) // 8

    # Width (in BYTES) and height (in pixels) in little-endian
    # According to ESC/POS spec: xL,xH = bytes, yL,yH = dots
    result.extend(struct.pack('<H', bytes_per_row))
    result.extend(struct.pack('<H', height))

    for y in range(height):
        for byte_idx in range(bytes_per_row):
            byte_val = 0
            for bit in range(8):
                x = byte_idx * 8 + bit
                if x < width:
                    pixel = img.getpixel((x, y))
                    if pixel == 0:  # black = print dot
                        byte_val |= (1 << (7 - bit))
            result.append(byte_val)

    return bytes(result)

def main():
    if len(sys.argv) < 2:
        print("Usage: ./convert_image.py <image_file> [output.bin] [--gs-v] [--threshold N]")
        print("  --gs-v: Use GS v format instead of ESC * (default)")
        print("  --threshold N: Black/white threshold 0-255 (default: 128, higher = darker)")
        sys.exit(1)

    image_path = sys.argv[1]
    output_path = sys.argv[2] if len(sys.argv) > 2 and not sys.argv[2].startswith('--') else None
    use_gs_v = '--gs-v' in sys.argv

    # Parse threshold parameter
    threshold = 128
    for i, arg in enumerate(sys.argv):
        if arg == '--threshold' and i + 1 < len(sys.argv):
            try:
                threshold = int(sys.argv[i + 1])
                threshold = max(0, min(255, threshold))  # Clamp to 0-255
            except ValueError:
                print(f"Warning: Invalid threshold value, using default 128")

    try:
        if use_gs_v:
            print(f"Converting {image_path} to GS v raster format (threshold={threshold})...")
            data = image_to_gs_v_raster(image_path, threshold=threshold)
        else:
            print(f"Converting {image_path} to ESC * raster format (threshold={threshold})...")
            data = image_to_escpos_raster(image_path, threshold=threshold)

        if output_path:
            with open(output_path, 'wb') as f:
                f.write(data)
            print(f"Written {len(data)} bytes to {output_path}")
        else:
            # Output to stdout for piping to nc
            sys.stdout.buffer.write(data)

    except FileNotFoundError:
        print(f"Error: Image file '{image_path}' not found", file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)

if __name__ == '__main__':
    main()
