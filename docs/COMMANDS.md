# ESC/POS Commands Reference - Implementation Status

This document tracks the ESC/POS commands implemented in the virtualesc printer emulator.

## Control Characters (0x00-0x1F)

| Hex  | Name | Description | Status |
|------|------|-------------|--------|
| 0x09 | HT   | Horizontal tab | ✅ Implemented (4 spaces) |
| 0x0A | LF   | Print and line feed | ✅ Implemented (flushes line, adds spacing) |
| 0x0C | FF   | Form feed (End job) | ✅ Implemented (clears buffer) |
| 0x0D | CR   | Print and carriage return | ✅ Implemented (flushes line) |
| 0x10 | DLE  | Data link escape | ✅ Implemented (DLE EOT, ENQ, DC4) |
| 0x18 | CAN  | Cancel | ✅ Implemented |
| 0x1B | ESC  | Escape | ✅ Implemented (see ESC commands below) |
| 0x1C | FS   | File separator | ✅ Implemented (see FS commands below) |
| 0x1D | GS   | Group separator | ✅ Implemented (see GS commands below) |

Other control characters (0x00-0x1F) are silently ignored.

## ESC Commands (0x1B prefix)

### Text Formatting

| Command | Hex | Description | Status |
|---------|-----|-------------|--------|
| ESC @   | 1B 40 | Initialize printer | ✅ Implemented |
| ESC !   | 1B 21 n | Select print mode | ✅ Implemented (bold, double width/height, underline) |
| ESC E   | 1B 45 n | Bold on/off | ✅ Implemented |
| ESC G   | 1B 47 n | Double-strike mode | ✅ Implemented (consumed) |
| ESC -   | 1B 2D n | Underline mode | ✅ Implemented |
| ESC M   | 1B 4D n | Character font selection | ✅ Implemented (consumed) |
| ESC R   | 1B 52 n | International character set | ✅ Implemented (consumed) |
| ESC t   | 1B 74 n | Code table selection | ✅ Implemented (consumed) |
| ESC r   | 1B 72 n | Color selection | ✅ Implemented (consumed) |
| ESC {   | 1B 7B n | Upside-down printing | ✅ Implemented (consumed) |
| ESC V   | 1B 56 n | 90° clockwise rotation | ✅ Implemented (consumed) |
| ESC %   | 1B 25 n | User-defined character set | ✅ Implemented (consumed) |

### Print Position & Spacing

| Command | Hex | Description | Status |
|---------|-----|-------------|--------|
| ESC SP  | 1B 20 n | Right-side character spacing | ✅ Implemented |
| ESC 2   | 1B 32 | Default line spacing (1/6 inch) | ✅ Implemented |
| ESC 3   | 1B 33 n | Set line spacing | ✅ Implemented |
| ESC a   | 1B 61 n | Justification (left/center/right) | ✅ Implemented |
| ESC $   | 1B 24 nL nH | Absolute print position | ✅ Implemented |
| ESC \\  | 1B 5C nL nH | Relative print position | ✅ Implemented |
| ESC D   | 1B 44 ... 00 | Set horizontal tab positions | ✅ Implemented |

### Paper Control

| Command | Hex | Description | Status |
|---------|-----|-------------|--------|
| ESC d   | 1B 64 n | Print and feed n lines | ✅ Implemented |
| ESC J   | 1B 4A n | Print and feed paper n dots | ✅ Implemented |

### Graphics

| Command | Hex | Description | Status |
|---------|-----|-------------|--------|
| ESC *   | 1B 2A m nL nH [data] | Bit image mode | ✅ Implemented (8/24-dot) |
| ESC K   | 1B 4B nL nH [data] | Single-density graphics | ✅ Implemented |
| ESC L   | 1B 4C nL nH [data] | Double-density graphics | ✅ Implemented |
| ESC Y   | 1B 59 nL nH [data] | Double-speed graphics | ✅ Implemented |
| ESC Z   | 1B 5A nL nH [data] | Quad-density graphics | ✅ Implemented |

### Peripheral Devices

| Command | Hex | Description | Status |
|---------|-----|-------------|--------|
| ESC p   | 1B 70 m t1 t2 | Generate pulse (cash drawer) | ✅ Implemented |

### Mode Selection

| Command | Hex | Description | Status |
|---------|-----|-------------|--------|
| ESC S   | 1B 53 n | Standard mode selection | ✅ Implemented (consumed) |
| ESC T   | 1B 54 n | Print direction in page mode | ✅ Implemented (consumed) |
| ESC U   | 1B 55 n | Unidirectional printing | ✅ Implemented (consumed) |
| ESC W   | 1B 57 [8 params] | Set print area in page mode | ✅ Implemented (consumed) |
| ESC <   | 1B 3C | Return home | ✅ Implemented |

### Status & Configuration

| Command | Hex | Description | Status |
|---------|-----|-------------|--------|
| ESC =   | 1B 3D n | Select peripheral device | ✅ Implemented (consumed) |
| ESC ?   | 1B 3F n | Cancel user-defined characters | ✅ Implemented |
| ESC c 3 | 1B 63 33 n | Paper sensor signal | ✅ Implemented (consumed) |
| ESC c 4 | 1B 63 34 n | Paper sensor detection | ✅ Implemented (consumed) |
| ESC c 5 | 1B 63 35 n | Panel button enable/disable | ✅ Implemented (consumed) |
| ESC i   | 1B 69 | Partial cut (obsolete) | ✅ Implemented (consumed) |
| ESC s   | 1B 73 n | Select paper sensor | ✅ Implemented (consumed) |
| ESC u   | 1B 75 n | Transmit peripheral device status | ✅ Implemented (consumed) |
| ESC v   | 1B 76 n | Transmit paper sensor status | ✅ Implemented (consumed) |

### Extended Commands

| Command | Hex | Description | Status |
|---------|-----|-------------|--------|
| ESC (   | 1B 28 [varies] | Extended commands | ✅ Implemented (generic parser) |
| ESC &   | 1B 26 y c1 c2 [data] | Define user-defined characters | ✅ Implemented |

## GS Commands (0x1D prefix)

### Text Formatting

| Command | Hex | Description | Status |
|---------|-----|-------------|--------|
| GS !    | 1D 21 n | Select character size | ✅ Implemented |
| GS B    | 1D 42 n | White/black reverse printing | ✅ Implemented |

### Print Position

| Command | Hex | Description | Status |
|---------|-----|-------------|--------|
| GS L    | 1D 4C nL nH | Set left margin | ✅ Implemented (consumed) |
| GS W    | 1D 57 nL nH | Set print area width | ✅ Implemented (consumed) |

### Graphics

| Command | Hex | Description | Status |
|---------|-----|-------------|--------|
| GS v    | 1D 76 m a xL xH yL yH [data] | Raster image | ✅ Implemented |

### Paper Control

| Command | Hex | Description | Status |
|---------|-----|-------------|--------|
| GS V    | 1D 56 m | Cut paper | ✅ Implemented (full/partial) |

### Barcodes

| Command | Hex | Description | Status |
|---------|-----|-------------|--------|
| GS H    | 1D 48 n | Select HRI printing position | ✅ Implemented (consumed) |
| GS h    | 1D 68 n | Set barcode height | ✅ Implemented (consumed) |
| GS w    | 1D 77 n | Set barcode width | ✅ Implemented (consumed) |
| GS k    | 1D 6B m [data] | Print barcode | ✅ Implemented (data consumed) |

### Extended Commands

| Command | Hex | Description | Status |
|---------|-----|-------------|--------|
| GS (    | 1D 28 [varies] | Extended commands | ✅ Implemented (generic parser) |

## FS Commands (0x1C prefix)

| Command | Hex | Description | Status |
|---------|-----|-------------|--------|
| FS .    | 1C 2E n | Cancel user-defined character | ✅ Implemented (consumed) |
| FS p    | 1C 70 n m | Print NV bit image | ✅ Implemented (consumed) |
| FS q    | 1C 71 n m | Define NV bit image | ✅ Implemented (consumed) |
| FS C    | 1C 43 n | Select Kanji character mode | ✅ Implemented (consumed) |
| FS g    | 1C 67 n | Select Kanji character code system | ✅ Implemented (consumed) |
| FS !    | 1C 21 n | Set Kanji character mode | ✅ Implemented (consumed) |
| FS &    | 1C 26 [data] | Select Kanji character font | ✅ Implemented (consumed) |
| FS (    | 1C 28 [varies] | Extended Kanji commands | ✅ Implemented (generic parser) |

## DLE Commands (0x10 prefix)

| Command | Hex | Description | Status |
|---------|-----|-------------|--------|
| DLE EOT | 10 04 n | Real-time status transmission | ✅ Implemented (consumed) |
| DLE ENQ | 10 05 n | Real-time request to printer | ✅ Implemented (consumed) |
| DLE DC4 | 10 14 fn ... | Real-time commands | ✅ Implemented (consumed) |

## Implementation Notes

### Protocol Compliance

Based on [official Epson ESC/POS reference](https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/commands.html):

**Line Control:**
- **LF (0x0A)**: "Print and line feed" - flushes current text buffer and adds vertical spacing (4px)
- **CR (0x0D)**: "Print and carriage return" - flushes current text buffer
- **FF (0x0C)**: "End job (in Standard mode)" - clears current buffer (FormFeed element not rendered to avoid artificial spacing)

**Common Patterns:**
- CR+LF together: CR flushes the line, LF adds spacing (standard text line)
- LF alone: Flushes and adds spacing
- Empty LF: Adds blank line spacing

### Binary Garbage Filtering

The renderer includes aggressive binary garbage filtering to prevent command bytes that fall in the printable ASCII range from appearing as text. Filters detect:

- Patterns like `8x8x8x` (repeating)
- High ratios of non-alphanumeric characters
- Mixed case with symbols in short strings
- Common binary patterns: `<R`, `>g`, `:0B`, etc.
- Angle brackets and parentheses in short strings
- Multiple @ symbols
- High digit-to-letter ratios with symbols

### Raster Graphics

Both ESC * and GS v raster graphics are fully supported:
- Automatic width/height calculation
- Proper byte consumption based on image dimensions
- Rendering to GUI with proper black/white conversion
- Supports 8-dot and 24-dot ESC * modes
- Supports GS v 0/1/2/3 modes

### Printer Specifications

Emulates an 80mm thermal receipt printer:
- **Paper Width**: 80mm
- **Resolution**: 203 DPI (~640px width)
- **Characters per Line**: 48 (standard mode)
- **Font**: Monospace, dynamically sized to fit 48 characters
- **Window Size**: Fixed width matching paper dimensions

### Text Rendering

Text elements support:
- **Bold** (via ESC E or ESC !)
- **Underline** (via ESC - or ESC !)
- **Double width/height** (via ESC ! or GS !) - 1.5x font size
- **Alignment** (left/center/right via ESC a) - within full 80mm width
- **Inverted** (white on black via GS B)
- **Print density** levels (via ESC ~) - affects text color from light gray to black

### Paper Control

- Paper cuts displayed with visual separator and scissors emoji
- Cash drawer pulses shown with visual indicator
- Form feeds collapsed to reduce clutter
- Line feeds and separators properly spaced

## Sources

- [Epson ESC/POS Command Reference](https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/commands.html)
- [ReceiptIO Library](https://github.com/receiptline/receiptio)
