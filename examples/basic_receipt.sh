#!/bin/bash
# Basic receipt example for VirtualESC
# Usage: ./examples/basic_receipt.sh
# Requires: VirtualESC running on localhost:9100

echo "Sending basic receipt to VirtualESC..."
(
  # Initialize printer
  printf "\x1B\x40"

  # Center alignment + double height/width
  printf "\x1B\x61\x01"
  printf "\x1B\x21\x30"
  printf "COFFEE SHOP\n"
  printf "\x1B\x21\x00"

  # Normal center text
  printf "123 Main Street\n"
  printf "Tel: (555) 123-4567\n"
  printf "\n"

  # Left alignment for items
  printf "\x1B\x61\x00"
  printf "%s\n" "--------------------------------"

  # Bold header
  printf "\x1B\x45\x01"
  printf "%-24s%8s\n" "Item" "Price"
  printf "\x1B\x45\x00"
  printf "%s\n" "--------------------------------"

  # Items
  printf "%-24s%8s\n" "Espresso" "\$3.50"
  printf "%-24s%8s\n" "Croissant" "\$4.00"
  printf "%-24s%8s\n" "Orange Juice" "\$5.00"
  printf "%s\n" "--------------------------------"

  # Bold total
  printf "\x1B\x45\x01"
  printf "%-24s%8s\n" "TOTAL" "\$12.50"
  printf "\x1B\x45\x00"

  printf "\n"

  # Center for footer
  printf "\x1B\x61\x01"
  printf "Thank you!\n"
  printf "\n\n"

  # Cut paper
  printf "\x1B\x69"
) | nc -w 1 localhost 9100

echo "Receipt sent!"
