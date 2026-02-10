#!/bin/bash
# Test horizontal alignment
# Tests: ESC a (text and image alignment)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Testing horizontal alignment..."

(
  # Initialize printer
  printf "\x1B\x40"

  # Header (centered)
  printf "\x1B\x61\x01"
  printf "=== ALIGNMENT TEST ===\n\n"

  # LEFT ALIGNMENT
  printf "\x1B\x61\x00"
  printf "%s\n" "---- LEFT ALIGNED ----"
  printf "Text aligned to left\n"
  printf "Second line of text\n"

  if [ -f "$SCRIPT_DIR/logo.bin" ]; then
    cat "$SCRIPT_DIR/logo.bin"
  fi
  printf "\n"

  # CENTER ALIGNMENT
  printf "\x1B\x61\x01"
  printf "%s\n" "--- CENTER ALIGNED ---"
  printf "Text aligned to center\n"
  printf "Second line centered\n"

  if [ -f "$SCRIPT_DIR/logo.bin" ]; then
    cat "$SCRIPT_DIR/logo.bin"
  fi
  printf "\n"

  # RIGHT ALIGNMENT
  printf "\x1B\x61\x02"
  printf "%s\n" "--- RIGHT ALIGNED ----"
  printf "Text aligned to right\n"
  printf "Second line to right\n"

  if [ -f "$SCRIPT_DIR/logo.bin" ]; then
    cat "$SCRIPT_DIR/logo.bin"
  fi
  printf "\n"

  # Footer (centered)
  printf "\x1B\x61\x01"
  printf "==================\n"
  printf "Test Complete\n"
  printf "ESC a: Text + image alignment\n"

  # Cut paper
  printf "\x1B\x69"

) | nc -w 1 localhost 9100

echo "Alignment test sent!"
