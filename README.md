# escpresso

A virtual ESC/POS thermal receipt printer emulator with real-time GUI preview. Accepts ESC/POS commands over TCP and renders receipts visually, making it useful for POS development, testing, and debugging without a physical printer.

## Features

- **TCP server** on port 9100 (standard POS printer port)
- **Real-time GUI preview** using egui — see receipts render as data arrives
- **58mm and 80mm paper sizes** with switchable UI
- **Text formatting** — bold, underline, double width/height, inverted, alignment
- **Raster graphics** — ESC \* bit images and GS v 0 raster images
- **QR codes** via GS ( k
- **Code page support** — CP437, Windows-1252, and more via encoding_rs
- **Print density** control (light to dark)
- **Paper cut visualization** with separator lines
- **receiptio compatible** — works with the [receiptio](https://github.com/receiptline/receiptio) CLI tool
- **100+ ESC/POS commands** parsed ([full list](docs/COMMANDS.md))

## Installation

### From crates.io

```bash
cargo install escpresso
```

### Prerequisites (Linux)

- Rust 1.73+ (for `div_ceil` stabilization)
- System dependencies for egui/eframe:

```bash
# Debian/Ubuntu
sudo apt install libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev \
  libxcb-xfixes0-dev libxkbcommon-dev libssl-dev
```

### Build from source

```bash
git clone https://github.com/jflaflamme/escpresso.git
cd escpresso
cargo build --release
```

The binary is at `target/release/escpresso`.

## Usage

### Start the emulator

```bash
escpresso
```

A GUI window opens and a TCP server starts on `localhost:9100`.

### Send ESC/POS commands

```bash
# Simple text
printf '\x1B\x40Hello World\n\x1B\x69' | nc -w 1 localhost 9100

# Centered bold text
printf '\x1B\x40\x1B\x61\x01\x1B\x45\x01RECEIPT\n\x1B\x45\x00\x1B\x69' | nc -w 1 localhost 9100
```

### Use with receiptio

[receiptio](https://github.com/receiptline/receiptio) converts a simple text format into ESC/POS commands:

```bash
npm install -g receiptio
```

Create a receipt file (`receipt.receipt`):
```
{image:https://receiptline.github.io/receiptio/logo.png}
^^^RECEIPT

{border:line}
|Item            |  Price|
|Apple           |  $1.50|
|Orange          |  $2.00|
{border:space}

^^Total: $3.50

{code:https://example.com; option:qrcode,3,L}

{cut}
```

Send to escpresso:
```bash
receiptio -d 192.168.x.x -p 9100 receipt.receipt    # or use localhost
receiptio -o receipt.raw receipt.receipt              # save to file first
cat receipt.raw | nc -w 1 localhost 9100              # then send
```

## Supported Commands

| Category | Commands |
|----------|----------|
| Text formatting | ESC !, ESC E, ESC -, GS !, GS B |
| Alignment | ESC a (left/center/right) |
| Position | ESC $, ESC \\, ESC D (tabs) |
| Line spacing | ESC 2, ESC 3, ESC d, ESC J |
| Graphics | ESC \* (bit image), GS v 0 (raster) |
| QR codes | GS ( k |
| Barcodes | GS k, GS H, GS h, GS w |
| Paper control | GS V (cut), ESC p (cash drawer) |
| Code pages | ESC t (code table selection) |
| Character sets | ESC R (international), FS commands (Kanji) |
| Initialization | ESC @, DLE commands |

See [docs/COMMANDS.md](docs/COMMANDS.md) for the complete list with hex codes and implementation status.

## Testing

### Shell tests

Shell scripts in `tests/shell/` send ESC/POS command sequences via netcat:

```bash
# Start escpresso first, then in another terminal:
./tests/shell/test_alignment.sh      # Text alignment tests
./tests/shell/test_raster.sh         # Raster image tests
./tests/shell/run_all.sh             # Run all tests
```

### Rust tests

```bash
cargo test
```

### Raw file testing

The `tests/raw/` directory contains binary ESC/POS captures from various sources:

```bash
cat tests/raw/test1_format.raw | nc -w 1 localhost 9100
```

## Code Structure

The codebase is a single `src/main.rs` with these main components:

- **`EscPosRenderer`** — The ESC/POS command parser and state machine. Processes raw bytes into `ReceiptElement`s.
- **`ReceiptElement`** — Enum representing rendered items: text lines, raster images, QR codes, separators, paper cuts.
- **`PrinterState`** — Tracks current formatting (bold, underline, alignment, density, code page, etc.).
- **`PaperSize`** — 58mm or 80mm paper width configuration.
- **TCP server** — Async Tokio listener that accepts connections and feeds data to the renderer.
- **GUI** — eframe/egui app that renders `ReceiptElement`s as a scrollable receipt preview.

## About

Built by [Innolabs](https://innolabs.dev) — POS integration and Odoo specialists in Cambodia.

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Ensure code passes `cargo fmt --check` and `cargo clippy -- -D warnings`
4. Add tests if applicable
5. Submit a pull request

## License

[MIT](LICENSE)
