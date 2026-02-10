#!/bin/bash
# Run all escpresso tests
# Make sure escpresso is running on localhost:9100 before executing

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "================================"
echo "escpresso Test Suite"
echo "================================"
echo ""

# Check if escpresso is listening
if ! nc -z localhost 9100 2>/dev/null; then
    echo "ERROR: escpresso is not running on localhost:9100"
    echo "Start it with: escpresso"
    exit 1
fi

echo "✓ escpresso detected on localhost:9100"
echo ""

# Run core tests
echo "[1/4] Alignment tests..."
"$SCRIPT_DIR/test_alignment.sh"
echo ""
read -p "Press Enter to continue to next test..."
echo ""

echo "[2/4] Darkness/density tests..."
"$SCRIPT_DIR/test_darkness.sh"
echo ""
read -p "Press Enter to continue to next test..."
echo ""

echo "[3/4] Raster image tests..."
"$SCRIPT_DIR/test_raster.sh"
echo ""
read -p "Press Enter to continue to next test..."
echo ""

echo "[4/4] Receiptio compatibility..."
"$SCRIPT_DIR/test_with_receiptio.sh"

echo ""
echo "================================"
echo "✓ All 4 tests completed!"
echo "================================"
