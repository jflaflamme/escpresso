#!/bin/bash
# Test print darkness/density control
# Tests: DC2 # command for print density with both text and images

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Testing print darkness control..."

(
  # Initialize printer
  printf "\x1B\x40"

  # Center alignment
  printf "\x1B\x61\x01"

  # Header
  printf "=== DARKNESS CONTROL TEST ===\n"
  printf "DC2 # n command (0-255)\n\n"

  # DC2 # n - Set print density (0-255)
  # n/32 gives density level 0-8
  # Higher values = darker print

  # Level 1: Very Light (32)
  printf "\x12\x23\x20"
  printf "%s\n" "------- Level 1: LIGHT (32) -------"
  printf "The quick brown fox jumps\n"
  printf "over the lazy dog. 0123456789\n"
  printf "ABCDEFGHIJKLMNOPQRSTUVWXYZ\n"
  if [ -f "$SCRIPT_DIR/logo.bin" ]; then
    # Logo inherits center alignment from text
    cat "$SCRIPT_DIR/logo.bin"
  fi
  printf "\n\n"

  # Level 2: Light (64)
  printf "\x12\x23\x40"
  printf "%s\n" "------- Level 2: LIGHT+ (64) ------"
  printf "The quick brown fox jumps\n"
  printf "over the lazy dog. 0123456789\n"
  printf "ABCDEFGHIJKLMNOPQRSTUVWXYZ\n"
  if [ -f "$SCRIPT_DIR/logo.bin" ]; then
    # Logo inherits center alignment from text
    cat "$SCRIPT_DIR/logo.bin"
  fi
  printf "\n\n"

  # Level 4: Normal (128)
  printf '%b' "\x12\x23\x80"
  printf "%s\n" "------ Level 4: NORMAL (128) ------"
  printf "The quick brown fox jumps\n"
  printf "over the lazy dog. 0123456789\n"
  printf "ABCDEFGHIJKLMNOPQRSTUVWXYZ\n"
  if [ -f "$SCRIPT_DIR/logo.bin" ]; then
    # Logo inherits center alignment from text
    cat "$SCRIPT_DIR/logo.bin"
  fi
  printf "\n\n"

  # Level 6: Dark (192)
  printf '%b' "\x12\x23\xC0"
  printf "%s\n" "------ Level 6: DARK (192) --------"
  printf "The quick brown fox jumps\n"
  printf "over the lazy dog. 0123456789\n"
  printf "ABCDEFGHIJKLMNOPQRSTUVWXYZ\n"
  if [ -f "$SCRIPT_DIR/logo.bin" ]; then
    # Logo inherits center alignment from text
    cat "$SCRIPT_DIR/logo.bin"
  fi
  printf "\n\n"

  # Level 8: Very Dark (255)
  printf '%b' "\x12\x23\xFF"
  printf "%s\n" "----- Level 8: VERY DARK (255) ----"
  printf "The quick brown fox jumps\n"
  printf "over the lazy dog. 0123456789\n"
  printf "ABCDEFGHIJKLMNOPQRSTUVWXYZ\n"
  if [ -f "$SCRIPT_DIR/logo.bin" ]; then
    # Logo inherits center alignment from text
    cat "$SCRIPT_DIR/logo.bin"
  fi
  printf "\n\n"

  # Reset to normal
  printf '%b' "\x12\x23\x80"
  printf "================================\n"
  printf "Test Complete\n"
  printf "5 density levels tested\n"

  # Cut paper
  printf "\x1B\x69"

) | nc -w 1 localhost 9100

echo "Darkness control test sent!"
