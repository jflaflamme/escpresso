#!/bin/bash
# Test raster image rendering
# Tests: ESC * command with logo and checkerboard pattern

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Testing raster images with text..."

(
  # Initialize printer
  printf "\x1B\x40"

  # Center alignment
  printf "\x1B\x61\x01"

  # Header
  printf "=== RASTER IMAGE TEST ===\n\n"

  # Logo image (from converted file)
  if [ -f "$SCRIPT_DIR/logo.bin" ]; then
    printf "Company Logo:\n"
    # Use center alignment for logo (same as text)
    printf "\x1B\x61\x01"
    cat "$SCRIPT_DIR/logo.bin"
    printf "\n\n"
  fi

  # Separator
  printf "%s\n\n" "-------------------"

  # Checkerboard pattern
  printf "Checkerboard Pattern:\n"
  # ESC * - Column-based raster
  # mode = 0 (8-dot single density)
  # width = 32 columns (0x20 0x00)
  printf "\x1B\x2A\x00\x20\x00"
  printf "\xAA\x55\xAA\x55\xAA\x55\xAA\x55"
  printf "\xAA\x55\xAA\x55\xAA\x55\xAA\x55"
  printf "\xAA\x55\xAA\x55\xAA\x55\xAA\x55"
  printf "\xAA\x55\xAA\x55\xAA\x55\xAA\x55"
  printf "\x0A\x0A"

  # Footer
  printf "%s\n" "-------------------"
  printf "Test Complete\n"
  printf "Logo: ESC * (24-dot)\n"
  printf "Pattern: 32x8 pixels\n"

  # Cut paper (if supported)
  printf "\x1B\x69"

) | nc -w 1 localhost 9100

echo "Raster image test sent!"
