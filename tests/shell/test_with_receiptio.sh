#!/bin/bash
# Test using receiptio tool

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Testing with receiptio..."

# Check if receiptio is installed
if ! command -v receiptio &> /dev/null; then
    echo "WARNING: receiptio not found. Skipping test."
    echo "To install: npm install -g receiptio"
    exit 0
fi

# Check if virtualesc is running
if ! nc -z localhost 9100 2>/dev/null; then
    echo "ERROR: virtualesc not running on localhost:9100"
    echo "Start it with: ./target/release/virtualesc"
    exit 1
fi

echo "Sending receipt to virtualesc..."
echo "Using file: $SCRIPT_DIR/test_receiptio.receipt"

# Use timeout to prevent hanging (receiptio may wait for connection close)
timeout 3s receiptio -d 127.0.0.1 -p generic "$SCRIPT_DIR/test_receiptio.receipt" || true

echo ""
echo "Receipt sent!"
