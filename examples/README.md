# Examples

## basic_receipt.sh

A shell script that sends a simple coffee shop receipt using raw ESC/POS commands via netcat. Demonstrates text formatting (bold, alignment, double-size), tabular layout, and paper cut.

```bash
./examples/basic_receipt.sh
```

## receipt.receipt

A [receiptio](https://github.com/receiptline/receiptio) format file showing text formatting, table borders, and QR code generation. Send it with:

```bash
receiptio -d localhost -p 9100 examples/receipt.receipt
```

Or convert to raw ESC/POS first:

```bash
receiptio -o /tmp/receipt.raw examples/receipt.receipt
cat /tmp/receipt.raw | nc -w 1 localhost 9100
```
