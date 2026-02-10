use anyhow::Result;
use codepage_437::{BorrowFromCp437, CP437_CONTROL};
use eframe::egui;
use encoding_rs::Encoding;
use qrcode::{Color as QrColor, QrCode};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const ESC: u8 = 0x1B;
const GS: u8 = 0x1D;
const FS: u8 = 0x1C;
const DLE: u8 = 0x10;
const LF: u8 = 0x0A;
const FF: u8 = 0x0C;
const CR: u8 = 0x0D;
const HT: u8 = 0x09;
const CAN: u8 = 0x18;
const DC2: u8 = 0x12;
const SOH: u8 = 0x01;
const STX: u8 = 0x02;
const ETX: u8 = 0x03;
const EOT: u8 = 0x04;
const ENQ: u8 = 0x05;
const ACK: u8 = 0x06;
const BEL: u8 = 0x07;
const BS: u8 = 0x08;
const VT: u8 = 0x0B;
const SO: u8 = 0x0E;
const SI: u8 = 0x0F;
const DC1: u8 = 0x11;
const DC3: u8 = 0x13;
const DC4: u8 = 0x14;
const ETB: u8 = 0x17;
const RS: u8 = 0x1E;

#[derive(Debug, Clone, Copy, PartialEq)]
enum PaperSize {
    Size58mm,
    Size80mm,
}

impl PaperSize {
    fn width_px(&self) -> f32 {
        // Printable area width (print head), not full paper
        // 80mm paper: 72mm print head = 576 dots (48 cols * 12 dots)
        // 58mm paper: 48mm print head = 384 dots (32 cols * 12 dots)
        (self.chars_per_line() as f32) * 12.0
    }

    fn chars_per_line(&self) -> usize {
        match self {
            PaperSize::Size58mm => 32,
            PaperSize::Size80mm => 48,
        }
    }

    fn label(&self) -> &str {
        match self {
            PaperSize::Size58mm => "58mm",
            PaperSize::Size80mm => "80mm",
        }
    }
}

#[derive(Debug, Clone)]
enum ReceiptElement {
    Text {
        content: String,
        bold: bool,
        underline: bool,
        double_width: bool,
        double_height: bool,
        inverted: bool,
        alignment: Alignment,
        density: u8,
        offset: u16,
        left_margin: u16,
        character_spacing: u8,
        double_strike: bool,
        font: u8,
        print_area_width: u16,
    },
    RasterImage {
        width: usize, // Width in pixels (for display)
        height: usize,
        data: Vec<u8>,
        offset: u16,
        density: u8,
        alignment: Alignment,
        bytes_per_line: usize, // Actual bytes per line from command (for data reading)
        print_area_width: u16,
    },
    QrCode {
        data: String,
        size: usize,
        alignment: Alignment,
        offset: u16,
        print_area_width: u16,
    },
    PaperCut {
        cut_type: String,
    },
    CashDrawer {
        pin: u8,
        on_time: u8,
        off_time: u8,
    },
    Separator,
    FormFeed,
}

#[derive(Debug, Clone)]
enum Alignment {
    Left,
    Center,
    Right,
}

#[derive(Debug)]
struct PrinterState {
    bold: bool,
    underline: bool,
    double_width: bool,
    double_height: bool,
    inverted: bool,
    alignment: Alignment,
    print_density: u8,
    encoding: &'static Encoding,
    code_page: u8,
    horizontal_offset: u16,
    left_margin: u16,
    print_area_width: u16,
    line_spacing: u8,
    character_spacing: u8,
    double_strike: bool,
    font: u8, // 0=Font A, 1=Font B, etc.
}

impl Default for PrinterState {
    fn default() -> Self {
        Self {
            bold: false,
            underline: false,
            double_width: false,
            double_height: false,
            inverted: false,
            alignment: Alignment::Left,
            print_density: 4,
            encoding: encoding_rs::UTF_8,
            code_page: 0,
            horizontal_offset: 0,
            left_margin: 0,
            print_area_width: 0, // 0 = use default (full width)
            line_spacing: 30,    // Default: 1/6 inch = ~30 dots at 203 DPI
            character_spacing: 0,
            double_strike: false,
            font: 0, // Default: Font A
        }
    }
}

struct EscPosRenderer {
    state: PrinterState,
    current_line: Vec<u8>, // Store raw bytes, decode using current encoding when flushing
    debug: bool,
    buffer: Vec<u8>,
    elements: Vec<ReceiptElement>,
    in_command_sequence: bool,
    qr_data: Vec<u8>,
    qr_size: u8,
    qr_error_correction: u8,
    response_queue: Vec<u8>,
    last_was_binary: bool, // Track if last command was binary (raster, etc.)
}

impl EscPosRenderer {
    fn new(debug: bool) -> Self {
        Self {
            state: PrinterState::default(),
            current_line: Vec::new(),
            debug,
            buffer: Vec::new(),
            elements: Vec::new(),
            in_command_sequence: false,
            qr_data: Vec::new(),
            qr_size: 3,
            qr_error_correction: 0,
            response_queue: Vec::new(),
            last_was_binary: false,
        }
    }

    fn log_debug(&self, msg: &str) {
        if self.debug {
            eprintln!("[DEBUG] {}", msg);
        }
    }

    fn take_elements(&mut self) -> Vec<ReceiptElement> {
        std::mem::take(&mut self.elements)
    }

    fn take_responses(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.response_queue)
    }

    fn process_data(&mut self, new_data: &[u8]) -> Result<()> {
        self.buffer.extend_from_slice(new_data);

        let mut i = 0;
        let data = self.buffer.clone();

        while i < data.len() {
            let byte = data[i];
            let start_pos = i;

            match byte {
                DLE => {
                    // Enter command sequence - block text accumulation
                    self.in_command_sequence = true;
                    // DLE commands (real-time status, etc.)
                    i += 1;
                    if i >= data.len() {
                        i = start_pos;
                        break;
                    }
                    let subcmd = data[i];
                    i += 1;
                    match subcmd {
                        0x04 | 0x05 => {
                            // DLE EOT, DLE ENQ - real-time status
                            if i < data.len() {
                                let _n = data[i];
                                i += 1;

                                // Queue status response: 0x12 = online, no errors
                                // Bit format: 00010010
                                //   Bit 3 = 1: Paper present
                                //   Bit 4 = 1: Online
                                self.response_queue.push(0x12);
                                self.log_debug(
                                    "DLE EOT/ENQ: queued status response 0x12 (online, no errors)",
                                );
                            }
                        }
                        0x14 => {
                            // DLE DC4 - real-time commands
                            if i + 1 < data.len() {
                                i += 2;
                            }
                        }
                        _ => {}
                    }
                    // Command processed - allow text accumulation again
                    self.in_command_sequence = false;
                }
                CAN => {
                    // Cancel print data in page mode
                    i += 1;
                }
                DC2 => {
                    // DC2 - Cancel bold OR DC2 # n (print density for zj-58)
                    i += 1;
                    if i < data.len() && data[i] == b'#' {
                        // DC2 # n - Set print density (zj-58 CUPS driver)
                        i += 1;
                        if i < data.len() {
                            let density = data[i];
                            self.state.print_density = (density / 32).min(8); // Map 0-255 to 0-8
                            self.log_debug(&format!("DC2 #: print density={}", density));
                            i += 1;
                        }
                    } else {
                        // Standard DC2 - Cancel bold
                        self.state.bold = false;
                    }
                }
                DC1 => {
                    // DC1 / XON - Device control / flow control
                    i += 1;
                }
                DC3 => {
                    // DC3 / XOFF - Device control / flow control
                    i += 1;
                }
                DC4 => {
                    // DC4 - Device control (standalone, not DLE DC4)
                    i += 1;
                }
                SO => {
                    // SO - Shift Out (alternate character set)
                    i += 1;
                }
                SI => {
                    // SI - Shift In (standard character set)
                    i += 1;
                }
                VT => {
                    // VT - Vertical tab
                    i += 1;
                }
                SOH | STX | ETX | EOT | ENQ | ACK | BEL | ETB | RS => {
                    // Other control characters - just skip
                    i += 1;
                }
                BS => {
                    // Backspace - remove last byte if present
                    if !self.current_line.is_empty() {
                        self.current_line.pop();
                    }
                    i += 1;
                }
                ESC => {
                    // Enter command sequence - block text accumulation
                    self.in_command_sequence = true;
                    i += 1;
                    if i >= data.len() {
                        i = start_pos;
                        break;
                    }
                    match self.handle_esc_command(&data, i) {
                        Ok(new_i) => {
                            if new_i == i || new_i <= start_pos {
                                // Handler didn't make progress - waiting for more data
                                i = start_pos;
                                // Keep in_command_sequence = true
                                break;
                            }
                            i = new_i;
                            // Command fully processed - allow text accumulation again
                            self.in_command_sequence = false;
                        }
                        Err(e) => return Err(e),
                    }
                }
                GS => {
                    // Enter command sequence - block text accumulation
                    self.in_command_sequence = true;
                    i += 1;
                    if i >= data.len() {
                        i = start_pos;
                        break;
                    }
                    match self.handle_gs_command(&data, i) {
                        Ok(new_i) => {
                            if new_i == i || new_i <= start_pos {
                                // Handler didn't make progress - waiting for more data
                                i = start_pos;
                                // Keep in_command_sequence = true
                                break;
                            }
                            i = new_i;
                            // Command fully processed - allow text accumulation again
                            self.in_command_sequence = false;
                        }
                        Err(e) => return Err(e),
                    }
                }
                FS => {
                    // Enter command sequence - block text accumulation
                    self.in_command_sequence = true;
                    i += 1;
                    if i >= data.len() {
                        i = start_pos;
                        break;
                    }
                    // FS command handling - many commands have unknown parameter counts
                    let cmd = data[i];
                    i += 1;
                    match cmd {
                        b'.' => {
                            // FS . n - Print NV bit image - 1 parameter
                            // Don't consume parameter if next byte is a command start
                            if i < data.len() {
                                let next = data[i];
                                // Only consume if not a command byte (ESC/GS/FS/DLE)
                                if next != ESC && next != GS && next != FS && next != DLE {
                                    i += 1;
                                }
                            }
                        }
                        b'p' => {
                            // FS p n m - Print NV bit image - 2 parameters
                            if i + 1 < data.len() {
                                i += 2;
                            }
                        }
                        b'q' => {
                            // FS q n [xL xH yL yH d1...dk] - Define NV bit image
                            if i < data.len() {
                                let n = data[i];
                                i += 1;
                                if n > 0 && i + 4 < data.len() {
                                    let xl = data[i] as usize;
                                    let xh = data[i + 1] as usize;
                                    let yl = data[i + 2] as usize;
                                    let yh = data[i + 3] as usize;
                                    let width = xl + (xh << 8);
                                    let height = yl + (yh << 8);
                                    let data_size = width.div_ceil(8) * height;
                                    i += 4 + data_size.min(data.len() - i);
                                }
                            }
                        }
                        b'(' => {
                            // FS ( fn pL pH [data...] - Extended commands with length
                            if i + 3 < data.len() {
                                let _fn = data[i]; // function code (e.g., 'A')
                                let p_l = data[i + 1] as usize;
                                let p_h = data[i + 2] as usize;
                                let len = p_l + (p_h << 8);
                                i += 3 + len.min(data.len() - i);
                            }
                        }
                        b'C' | b'g' | b'!' | b'&' | b'S' | b'-' => {
                            // Commands with 1 parameter
                            if i < data.len() {
                                i += 1;
                            }
                        }
                        _ => {
                            // Unknown FS subcommands - try to consume 1-2 likely parameter bytes
                            // Many proprietary commands use 1-2 bytes
                            if i < data.len() && (data[i] < 0x1B || data[i] > 0x7E) {
                                // Next byte doesn't look like a command start, consume it as parameter
                                i += 1;
                                // If it was high-bit, might be a 2-byte parameter
                                if i < data.len()
                                    && data[i - 1] > 0x7F
                                    && (data[i] < 0x1B || data[i] > 0x7E)
                                {
                                    i += 1;
                                }
                            }
                            if self.debug {
                                self.log_debug(&format!(
                                    "FS command 0x{:02X} - consumed {} parameter bytes",
                                    cmd,
                                    i - (start_pos + 2)
                                ));
                            }
                        }
                    }
                    // Command processed - allow text accumulation again
                    self.in_command_sequence = false;
                }
                LF => {
                    // LF: Print and line feed - flush current line and advance
                    self.in_command_sequence = false; // Exit command sequence, allow text again
                    self.last_was_binary = false; // LF marks start of text content
                    if !self.current_line.is_empty() {
                        self.flush_line();
                        self.current_line.clear();
                    } else if !self.elements.is_empty() {
                        // Only add separator for blank lines if we've already printed something
                        // This avoids extra spacing after init commands like ESC @
                        self.elements.push(ReceiptElement::Separator);
                    }
                    i += 1;
                }
                CR => {
                    // CR: Print and carriage return - flush current line
                    self.in_command_sequence = false; // Exit command sequence, allow text again
                    self.last_was_binary = false; // CR marks start of text content
                    if !self.current_line.is_empty() {
                        self.flush_line();
                        self.current_line.clear();
                    }
                    i += 1;
                }
                FF => {
                    self.current_line.clear();
                    // Only add FormFeed if the last element isn't already one
                    if !matches!(self.elements.last(), Some(ReceiptElement::FormFeed)) {
                        self.elements.push(ReceiptElement::FormFeed);
                    }
                    i += 1;
                }
                HT => {
                    // Only add tabs if not in command sequence
                    if !self.in_command_sequence {
                        // Add 4 spaces as tab
                        self.current_line.extend_from_slice(b"    ");
                    }
                    i += 1;
                }
                0x20..=0x7E | 0x80..=0xFF => {
                    // Printable characters (both ASCII and extended codepage)
                    if i == data.len() - 1 && !self.buffer.is_empty() {
                        break;
                    }
                    // Only accumulate text if we're NOT in a command sequence AND not after binary data
                    if !self.in_command_sequence && !self.last_was_binary {
                        if self.debug {
                            self.log_debug(&format!(
                                "Adding byte to line: 0x{:02X} at position {}",
                                byte, i
                            ));
                        }
                        self.current_line.push(byte);
                    }
                    i += 1;
                }
                0x00..=0x1F | 0x7F => {
                    // Control characters (including DEL)
                    // Silently consume these - they're control codes, not printable text
                    i += 1;
                }
            }
        }

        self.buffer.drain(0..i);

        // Don't auto-flush at buffer end - only flush on explicit line terminators (LF, CR)
        // This prevents fragmenting text that arrives in multiple TCP packets

        Ok(())
    }

    fn flush_line(&mut self) {
        if self.current_line.is_empty() {
            return;
        }

        if self.debug {
            self.log_debug(&format!(
                "Flushing line: {} bytes, codepage={}",
                self.current_line.len(),
                self.state.code_page
            ));
        }

        // Decode bytes using current codepage
        let decoded = if self.state.code_page == 0 {
            // CP437 - use codepage-437 crate
            String::borrow_from_cp437(&self.current_line, &CP437_CONTROL)
        } else {
            // Other codepages - use encoding_rs
            let (decoded_cow, _encoding_used, had_errors) =
                self.state.encoding.decode(&self.current_line);

            if self.debug {
                if had_errors {
                    self.log_debug(&format!(
                        "Decoding errors in line, codepage={}",
                        self.state.code_page
                    ));
                }
                self.log_debug(&format!("Decoded: {:?}", decoded_cow));
            }

            decoded_cow.into_owned()
        };

        self.elements.push(ReceiptElement::Text {
            content: decoded,
            bold: self.state.bold,
            underline: self.state.underline,
            double_width: self.state.double_width,
            double_height: self.state.double_height,
            inverted: self.state.inverted,
            alignment: self.state.alignment.clone(),
            density: self.state.print_density,
            offset: self.state.horizontal_offset,
            left_margin: self.state.left_margin,
            character_spacing: self.state.character_spacing,
            double_strike: self.state.double_strike,
            font: self.state.font,
            print_area_width: self.state.print_area_width,
        });

        // Reset horizontal offset after use (ESC $ is one-time positioning)
        self.state.horizontal_offset = 0;
    }

    fn handle_esc_command(&mut self, data: &[u8], mut i: usize) -> Result<usize> {
        let cmd = data[i];
        match cmd {
            b'@' => {
                self.state = PrinterState::default();
                i += 1;
            }
            b'E' => {
                i += 1;
                if i < data.len() {
                    self.state.bold = data[i] == 1;
                    i += 1;
                }
            }
            b'-' => {
                i += 1;
                if i < data.len() {
                    let n = data[i];
                    // n = 0: off, n = 1 or 2: on (with thickness)
                    // Only consider actual values 1-2, not ASCII '1' '2'
                    self.state.underline = n == 1 || n == 2;
                    i += 1;
                }
            }
            b'a' => {
                i += 1;
                if i < data.len() {
                    self.state.alignment = match data[i] {
                        0 => Alignment::Left,
                        1 => Alignment::Center,
                        2 => Alignment::Right,
                        _ => Alignment::Left,
                    };
                    i += 1;
                }
            }
            b'!' => {
                i += 1;
                if i < data.len() {
                    let mode = data[i];
                    self.state.bold = (mode & 0x08) != 0;
                    self.state.double_height = (mode & 0x10) != 0;
                    self.state.double_width = (mode & 0x20) != 0;
                    self.state.underline = (mode & 0x80) != 0;
                    i += 1;
                }
            }
            b'd' => {
                i += 1;
                if i < data.len() {
                    let lines = data[i];
                    for _ in 0..lines {
                        self.elements.push(ReceiptElement::Separator);
                    }
                    i += 1;
                }
            }
            b'*' => {
                i += 1;
                i = self.handle_raster_graphics(data, i)?;
            }
            b'~' => {
                i += 1;
                if i < data.len() {
                    self.state.print_density = data[i].min(8);
                    i += 1;
                }
            }
            b'p' => {
                i += 1;
                if i + 2 < data.len() {
                    let pin = data[i];
                    let on_time = data[i + 1];
                    let off_time = data[i + 2];
                    self.elements.push(ReceiptElement::CashDrawer {
                        pin,
                        on_time,
                        off_time,
                    });
                    i += 3;
                }
            }
            b' ' => {
                // ESC SP n - Set right-side character spacing
                i += 1;
                if i < data.len() {
                    self.state.character_spacing = data[i];
                    self.log_debug(&format!("ESC SP: character spacing = {}", data[i]));
                    i += 1;
                }
            }
            b'$' => {
                // ESC $ - Set absolute horizontal print position
                i += 1;
                if i + 1 < data.len() {
                    let nl = data[i] as u16;
                    let nh = data[i + 1] as u16;
                    self.state.horizontal_offset = nl + (nh << 8);
                    self.log_debug(&format!(
                        "ESC $: set horizontal offset to {}",
                        self.state.horizontal_offset
                    ));
                    i += 2;
                }
            }
            b'\\' => {
                // ESC \ - Set relative horizontal print position
                i += 1;
                if i + 1 < data.len() {
                    let nl = data[i] as i16;
                    let nh = data[i + 1] as i16;
                    let relative_offset = nl + (nh << 8);
                    // Add to current horizontal offset (can be negative)
                    self.state.horizontal_offset =
                        ((self.state.horizontal_offset as i16) + relative_offset).max(0) as u16;
                    self.log_debug(&format!(
                        "ESC \\: relative offset {} -> total {}",
                        relative_offset, self.state.horizontal_offset
                    ));
                    i += 2;
                }
            }
            b'K' | b'L' | b'Y' | b'Z' => {
                // ESC K/L/Y/Z - Select bit image mode
                i += 1;
                if i + 1 < data.len() {
                    let nl = data[i] as usize;
                    let nh = data[i + 1] as usize;
                    let width = nl + (nh << 8);
                    i += 2;
                    // Skip image data
                    let bytes_needed = match cmd {
                        b'K' | b'L' => width,
                        b'Y' | b'Z' => width * 2,
                        _ => width,
                    };
                    if i + bytes_needed <= data.len() {
                        i += bytes_needed;
                    }
                }
            }
            b'D' => {
                // ESC D - Set horizontal tab positions
                i += 1;
                // Read tab positions until NUL
                while i < data.len() && data[i] != 0 {
                    i += 1;
                }
                if i < data.len() {
                    i += 1; // skip NUL
                }
            }
            b'S' | b'T' | b'U' | b'W' => {
                // ESC S/T - Standard/page mode selection
                // ESC U - Unidirectional printing
                // ESC W - Set print area in page mode
                i += 1;
                if i < data.len() {
                    if cmd == b'W' && i + 7 < data.len() {
                        // W takes 8 parameters
                        i += 8;
                    } else {
                        i += 1;
                    }
                }
            }
            b'c' => {
                // ESC c - Paper sensor commands
                i += 1;
                if i + 1 < data.len() {
                    i += 2;
                }
            }
            b'i' => {
                // ESC i - Partial cut (obsolete)
                i += 1;
            }
            b's' => {
                // ESC s - Select paper sensor(s)
                i += 1;
                if i < data.len() {
                    i += 1;
                }
            }
            0x06 => {
                // ESC ACK n - Enable/disable panel buttons (or ASB in some implementations)
                i += 1;
                if i < data.len() {
                    let _n = data[i];
                    self.log_debug(&format!(
                        "ESC ACK: n=0x{:02X} (acknowledged, not implemented)",
                        _n
                    ));
                    i += 1;
                }
            }
            b'u' => {
                // ESC u - Transmit peripheral device status (obsolete)
                i += 1;
                if i < data.len() {
                    i += 1;
                }
            }
            b'v' => {
                // ESC v - Transmit paper sensor status (obsolete)
                i += 1;
                if i < data.len() {
                    i += 1;
                }
            }
            b't' => {
                // ESC t - Select character code table (ESC/POS standard)
                i += 1;
                if i < data.len() {
                    self.state.code_page = data[i];
                    // Map codepage numbers to encoding_rs encodings
                    // Note: CP437 (codepage 0) is handled specially in flush_line()
                    self.state.encoding = match data[i] {
                        0 => encoding_rs::WINDOWS_1252,  // CP437 (handled specially)
                        1 => encoding_rs::WINDOWS_1252,  // Katakana (approximation)
                        2 => encoding_rs::WINDOWS_1252,  // CP850
                        3 => encoding_rs::WINDOWS_1252,  // CP860
                        4 => encoding_rs::WINDOWS_1252,  // CP863
                        5 => encoding_rs::WINDOWS_1252,  // CP865
                        16 => encoding_rs::WINDOWS_1252, // Windows-1252 (Western European)
                        17 => encoding_rs::WINDOWS_1251, // CP866 -> Windows-1251 (Cyrillic)
                        18 => encoding_rs::WINDOWS_1250, // CP852 -> Windows-1250 (Central European)
                        19 => encoding_rs::WINDOWS_1252, // CP858 (like CP850 with Euro)
                        20 => encoding_rs::SHIFT_JIS,    // Shift JIS (Japanese)
                        21 => encoding_rs::SHIFT_JIS,
                        255 => encoding_rs::SHIFT_JIS,
                        _ => encoding_rs::WINDOWS_1252, // Default fallback
                    };
                    if self.debug {
                        self.log_debug(&format!("ESC t: selected codepage {}", data[i]));
                    }
                    i += 1;
                }
            }
            b'M' => {
                // ESC M n - Select character font
                // n=0: Font A, n=1: Font B, n=2: Font C (if supported)
                i += 1;
                if i < data.len() {
                    self.state.font = data[i];
                    self.log_debug(&format!("ESC M: font = {}", data[i]));
                    i += 1;
                }
            }
            b'R' | b'r' | b'%' => {
                // Character set, region, user-defined char mode
                i += 1;
                if i < data.len() {
                    i += 1;
                }
            }
            b'2' => {
                // ESC 2 - Set default line spacing (1/6 inch = ~30 dots at 203 DPI)
                self.state.line_spacing = 30;
                self.log_debug("ESC 2: reset to default line spacing (30 dots)");
                i += 1;
            }
            b'3' => {
                // ESC 3 n - Set line spacing to n dots
                i += 1;
                if i < data.len() {
                    self.state.line_spacing = data[i];
                    self.log_debug(&format!("ESC 3: line spacing = {} dots", data[i]));
                    i += 1;
                }
            }
            b'{' => {
                // Upside down mode
                i += 1;
                if i < data.len() {
                    i += 1;
                }
            }
            b'G' => {
                // ESC G n - Double-strike mode (makes text darker/bolder)
                i += 1;
                if i < data.len() {
                    self.state.double_strike = data[i] != 0;
                    self.log_debug(&format!(
                        "ESC G: double-strike = {}",
                        self.state.double_strike
                    ));
                    i += 1;
                }
            }
            b'J' => {
                // ESC J n - Print and feed n lines (used by zj-58 CUPS driver)
                i += 1;
                if i < data.len() {
                    let lines = data[i];
                    self.log_debug(&format!("ESC J: feed {} lines", lines));
                    // Add line feeds as specified (each line is ~1/6 inch or ~4.23mm)
                    // Display exactly as ESC/POS specifies for accurate virtual printer behavior
                    for _ in 0..lines {
                        self.elements.push(ReceiptElement::Separator);
                    }
                    i += 1;
                }
            }
            b'V' => {
                // 90-degree rotation
                i += 1;
                if i < data.len() {
                    i += 1;
                }
            }
            b'(' => {
                // ESC ( - Extended commands
                i += 1;
                if i + 2 < data.len() {
                    let p_l = data[i + 1] as usize;
                    let p_h = data[i + 2] as usize;
                    let len = p_l + (p_h << 8);
                    i += 3 + len;
                }
            }
            b'&' => {
                // ESC & - Define user-defined characters
                i += 1;
                if i + 2 < data.len() {
                    let y = data[i] as usize;
                    let c1 = data[i + 1] as usize;
                    let c2 = data[i + 2] as usize;
                    i += 3;
                    let num_chars = if c2 >= c1 { c2 - c1 + 1 } else { 0 };
                    let bytes_per_char = y * 12_usize.div_ceil(8);
                    i += num_chars * bytes_per_char;
                }
            }
            b'?' => {
                // ESC ? - Cancel user-defined characters
                i += 1;
                if i < data.len() {
                    i += 1;
                }
            }
            b'=' => {
                // ESC = - Select peripheral device
                i += 1;
                if i < data.len() {
                    i += 1;
                }
            }
            b'<' => {
                // ESC < - Return home
                i += 1;
            }
            _ => {
                // Unknown ESC command - assume it has at least 1 parameter
                if self.debug {
                    self.log_debug(&format!("Unknown ESC command: 0x{:02X}", cmd));
                }
                i += 1;
                // Try to consume 1 parameter byte to prevent leakage
                if i < data.len() {
                    i += 1;
                }
            }
        }
        Ok(i)
    }

    fn handle_gs_command(&mut self, data: &[u8], mut i: usize) -> Result<usize> {
        let cmd = data[i];
        match cmd {
            b'8' => {
                // GS 8 - Extended command (L = raster graphics)
                let start_i = i - 1;
                i += 1;
                if i < data.len() {
                    if data[i] == b'L' {
                        i = self.handle_gs_8l(data, i)?;
                    } else {
                        // Other GS 8 subcommands (structure: GS 8 fn p1 p2 p3 p4 data...)
                        let subcmd = data[i];
                        i += 1; // skip subcommand

                        // Read length bytes
                        if i + 4 > data.len() {
                            // Not enough data for length - wait for more
                            if self.debug {
                                self.log_debug(&format!(
                                    "GS 8 0x{:02X}: waiting for length bytes",
                                    subcmd
                                ));
                            }
                            return Ok(start_i);
                        }

                        let p1 = data[i] as usize;
                        let p2 = data[i + 1] as usize;
                        let p3 = data[i + 2] as usize;
                        let p4 = data[i + 3] as usize;
                        let len = p1 | (p2 << 8) | (p3 << 16) | (p4 << 24);
                        i += 4;

                        // Check if we have all the data
                        let skip = len.min(1_000_000);
                        if i + skip > data.len() {
                            // Not enough data - wait for more
                            if self.debug {
                                self.log_debug(&format!(
                                    "GS 8 0x{:02X}: waiting for {} data bytes (have {})",
                                    subcmd,
                                    skip,
                                    data.len() - i
                                ));
                            }
                            return Ok(start_i);
                        }

                        // Skip all the data
                        i += skip;
                    }
                }
            }
            b'V' => {
                i += 1;
                if i < data.len() {
                    i = self.handle_paper_cut(data, i)?;
                }
            }
            b'v' => {
                i += 1;
                if i < data.len() {
                    i = self.handle_raster_graphics_gs(data, i)?;
                }
            }
            b'!' => {
                // GS ! - Select character size (width and height multipliers)
                // Bits 0-2: width (0-7), Bits 4-6: height (0-7)
                i += 1;
                if i < data.len() {
                    let mode = data[i];
                    let width_mul = (mode & 0x07) + 1;
                    let height_mul = ((mode >> 4) & 0x07) + 1;
                    self.state.double_width = width_mul > 1;
                    self.state.double_height = height_mul > 1;
                    i += 1;
                }
            }
            b'B' => {
                i += 1;
                if i < data.len() {
                    self.state.inverted = data[i] == 1;
                    i += 1;
                }
            }
            b'L' => {
                // GS L nL nH - Set left margin (in dots)
                i += 1;
                if i + 1 < data.len() {
                    let nl = data[i] as u16;
                    let nh = data[i + 1] as u16;
                    self.state.left_margin = nl + (nh << 8);
                    self.log_debug(&format!(
                        "GS L: left margin = {} dots",
                        self.state.left_margin
                    ));
                    i += 2;
                }
            }
            b'W' => {
                // GS W nL nH - Set print area width (in dots)
                i += 1;
                if i + 1 < data.len() {
                    let nl = data[i] as u16;
                    let nh = data[i + 1] as u16;
                    self.state.print_area_width = nl + (nh << 8);
                    self.log_debug(&format!(
                        "GS W: print area width = {} dots",
                        self.state.print_area_width
                    ));
                    i += 2;
                }
            }
            b'H' | b'h' | b'w' | b'k' => {
                // Barcode height, HRI position, barcode width, barcode print
                i += 1;
                if i < data.len() {
                    if cmd == b'k' {
                        // Barcode data follows
                        let barcode_type = data[i];
                        i += 1;
                        if barcode_type < 6 {
                            // Variable length barcode - find NUL terminator
                            while i < data.len() && data[i] != 0 {
                                i += 1;
                            }
                            if i < data.len() {
                                i += 1; // skip NUL
                            }
                        } else {
                            // Fixed length barcode
                            if i < data.len() {
                                let len = data[i] as usize;
                                i += 1 + len;
                            }
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            b'(' => {
                // Extended commands
                i += 1;
                if i < data.len() {
                    let subcmd = data[i];
                    if subcmd == b'k' {
                        // QR Code commands
                        i = self.handle_qr_code(data, i)?;
                    } else {
                        // Other extended commands
                        if i + 2 < data.len() {
                            let p_l = data[i + 1] as usize;
                            let p_h = data[i + 2] as usize;
                            let len = p_l + (p_h << 8);
                            i += 3 + len;
                        }
                    }
                }
            }
            b'a' => {
                // GS a n - Enable/disable Automatic Status Back (ASB)
                // n bits specify which status types to report automatically
                i += 1;
                if i < data.len() {
                    let asb_flags = data[i];
                    self.log_debug(&format!("GS a: ASB flags=0x{:02X}", asb_flags));

                    // If ASB is enabled (n != 0), send 4-byte ASB status immediately
                    if asb_flags != 0 {
                        // ASB format (4 bytes):
                        // Byte 0: 0x10 = binary 00010000
                        //   Bit 0,1 = 0 (fixed)
                        //   Bit 2 = 0 (drawer pin LOW)
                        //   Bit 3 = 0 (online)
                        //   Bit 4 = 1 (fixed)
                        //   Bit 5 = 0 (cover closed)
                        //   Bit 6 = 0 (not feeding paper)
                        //   Bit 7 = 0 (fixed)
                        // Byte 1: 0x00 = all OK (no errors, not waiting)
                        // Byte 2: 0x00 = paper sensors OK (paper present)
                        // Byte 3: 0x00 = reserved
                        self.response_queue.push(0x10);
                        self.response_queue.push(0x00);
                        self.response_queue.push(0x00);
                        self.response_queue.push(0x00);
                        self.log_debug("GS a: queued 4-byte ASB status (online, no errors)");
                    }
                    i += 1;
                }
            }
            b'I' => {
                // GS I n - Transmit printer ID information
                // Response format: 0x5f + "string" + 0x00 (block data format)
                i += 1;
                if i < data.len() {
                    let n = data[i];
                    self.log_debug(&format!("GS I: query type=0x{:02X}", n));

                    // Queue response based on query type (block data format)
                    match n {
                        0x42 => {
                            // Manufacturer name (0x42 = 66)
                            // Send in block data format: 0x5f + "CITIZEN" + 0x00
                            // (use CITIZEN not EPSON so receiptio switches to 'escpos' mode)
                            self.response_queue.push(0x5f); // Block data start
                            self.response_queue.extend_from_slice(b"CITIZEN");
                            self.response_queue.push(0x00); // Null terminator
                            self.log_debug("GS I 0x42: sent manufacturer 'CITIZEN' (block data)");
                        }
                        0x43 => {
                            // Model name (0x43 = 67)
                            // Send in block data format: 0x5f + "CT-S310" + 0x00
                            self.response_queue.push(0x5f); // Block data start
                            self.response_queue.extend_from_slice(b"CT-S310");
                            self.response_queue.push(0x00); // Null terminator
                            self.log_debug("GS I 0x43: sent model 'CT-S310' (block data)");
                        }
                        _ => {
                            self.log_debug(&format!("GS I: unknown query type 0x{:02X}", n));
                        }
                    }
                    i += 1;
                }
            }
            b'r' => {
                // GS r n - Transmit status
                i += 1;
                if i < data.len() {
                    let _n = data[i];
                    self.log_debug(&format!("GS r: transmit status n=0x{:02X}", _n));

                    // Send 1-byte status response
                    // Status byte format: bit pattern must have (value & 0x90) === 0
                    // 0x08 = 00001000 (online, paper present, no errors)
                    //   Bit 3 = 1: paper present
                    //   Bit 4 = 0: online (not offline)
                    //   Bit 7 = 0: (required by receiptio)
                    self.response_queue.push(0x08);
                    self.log_debug("GS r: queued status response 0x08 (online, paper OK)");
                    i += 1;
                }
            }
            b'$' => {
                // GS $ nL nH - Set absolute vertical print position
                // Used by receiptio for positioning each line
                i += 1;
                if i + 1 < data.len() {
                    let nl = data[i] as u16;
                    let nh = data[i + 1] as u16;
                    let vertical_pos = nl + (nh << 8);
                    self.log_debug(&format!("GS $: set vertical position to {}", vertical_pos));
                    // VirtualESC renders sequentially, so we acknowledge but don't use this
                    i += 2;
                }
            }
            0x00 | 0x80 | 0xF7 => {
                // Additional GS commands found in real data
                i += 1;
                // Consume likely parameter
                if i < data.len() {
                    i += 1;
                }
            }
            _ => {
                // Unknown GS command - assume it has at least 1 parameter
                if self.debug {
                    self.log_debug(&format!("Unknown GS command: 0x{:02X}", cmd));
                }
                i += 1;
                // Try to consume 1 parameter byte to prevent leakage
                if i < data.len() {
                    i += 1;
                }
            }
        }
        Ok(i)
    }

    fn handle_raster_graphics(&mut self, data: &[u8], i: usize) -> Result<usize> {
        let start_i = i - 2; // Point to ESC byte, not '*' byte (i-1=*, i-2=ESC)

        if i + 3 > data.len() {
            self.log_debug("ESC * incomplete: not enough header bytes");
            return Ok(start_i);
        }

        let m = data[i];
        let nl = data[i + 1] as usize;
        let nh = data[i + 2] as usize;
        let width = nl + (nh << 8);
        let height = match m {
            0 | 1 => 8,
            32 | 33 => 24,
            _ => 8,
        };

        let mut pos = i + 3;

        // Validate dimensions
        if width == 0 || width > 10000 {
            self.log_debug(&format!("ESC * invalid width: {}", width));
            return Ok(pos);
        }

        // ESC * uses COLUMN-based format, not raster!
        // Each column is height/8 bytes (8-dot) or height/8*3 bytes (24-dot)
        let bytes_per_column = height / 8;
        let total_bytes = width * bytes_per_column;

        self.log_debug(&format!(
            "ESC * column-based: m={}, width={}, height={}, bytes_per_col={}, need {} bytes",
            m, width, height, bytes_per_column, total_bytes
        ));

        if total_bytes > 1_000_000 {
            self.log_debug("ESC * dimensions too large, skipping");
            return Ok(pos);
        }

        if pos + total_bytes > data.len() {
            self.log_debug(&format!(
                "ESC * incomplete: have {}, need {}",
                data.len() - pos,
                total_bytes
            ));
            return Ok(start_i);
        }

        // Additional safety check before slicing
        if pos >= data.len() || pos + total_bytes > data.len() {
            self.log_debug("ESC * bounds check failed");
            return Ok(start_i);
        }

        // Flush any pending text before image
        if !self.current_line.is_empty() {
            self.flush_line();
            self.current_line.clear();
        }

        // Convert column-based data to row-based raster data for rendering
        let column_data = &data[pos..pos + total_bytes];
        let raster_data = self.column_to_raster(column_data, width, height);

        self.elements.push(ReceiptElement::RasterImage {
            width,
            height,
            data: raster_data,
            offset: self.state.horizontal_offset,
            density: self.state.print_density,
            alignment: self.state.alignment.clone(),
            bytes_per_line: width.div_ceil(8), // Calculate from pixel width
            print_area_width: self.state.print_area_width,
        });

        // Reset offset after rendering
        self.state.horizontal_offset = 0;

        // Mark that we just processed binary data - don't treat following ASCII bytes as text
        self.last_was_binary = true;

        pos += total_bytes;

        Ok(pos)
    }

    fn column_to_raster(&self, column_data: &[u8], width: usize, height: usize) -> Vec<u8> {
        let bytes_per_column = height / 8;
        let bytes_per_row = width.div_ceil(8);
        let mut raster_data = vec![0u8; bytes_per_row * height];

        // Convert column format to raster format
        // Column format: each byte represents 8 vertical pixels in a column
        // Raster format: each byte represents 8 horizontal pixels in a row

        for col in 0..width {
            let column_offset = col * bytes_per_column;

            for byte_in_col in 0..bytes_per_column {
                if column_offset + byte_in_col >= column_data.len() {
                    break;
                }

                let col_byte = column_data[column_offset + byte_in_col];

                // Each bit in this byte represents a pixel at a different row
                for bit in 0..8 {
                    let y = byte_in_col * 8 + bit;
                    if y >= height {
                        break;
                    }

                    // Extract the pixel value (1 = black, 0 = white)
                    let pixel = (col_byte >> (7 - bit)) & 1;

                    // Set the corresponding bit in the raster data
                    let row_byte_idx = y * bytes_per_row + (col / 8);
                    let row_bit_idx = 7 - (col % 8);

                    if row_byte_idx < raster_data.len() {
                        raster_data[row_byte_idx] |= pixel << row_bit_idx;
                    }
                }
            }
        }

        raster_data
    }

    fn handle_raster_graphics_gs(&mut self, data: &[u8], i: usize) -> Result<usize> {
        let start_i = i - 2; // Point to GS byte, not 'v' byte (i-1=v, i-2=GS)

        self.log_debug(&format!("GS v: entered handler at position {}", i));

        if i + 6 > data.len() {
            self.log_debug(&format!(
                "GS v incomplete: not enough header bytes (have {}, need {})",
                data.len() - i,
                6
            ));
            return Ok(start_i);
        }

        // zj-58 format: GS v variant m xL xH yL yH [data]
        // escRasterMode[] = "\x1dv0\0" sends: GS v '0' 0x00
        // Then mputnum(width) and mputnum(height) send little-endian 2-byte values
        let variant = data[i]; // '0' = 0x30
        let _m = data[i + 1]; // 0x00 (mode)
        let xl = data[i + 2] as usize;
        let xh = data[i + 3] as usize;
        let yl = data[i + 4] as usize;
        let yh = data[i + 5] as usize;

        self.log_debug(&format!(
            "GS v: raw bytes at i: [{:02X} {:02X} {:02X} {:02X} {:02X} {:02X}]",
            data[i],
            data[i + 1],
            data[i + 2],
            data[i + 3],
            data[i + 4],
            data[i + 5]
        ));
        self.log_debug(&format!(
            "GS v: variant=0x{:02X} m=0x{:02X}, xl=0x{:02X} xh=0x{:02X} yl=0x{:02X} yh=0x{:02X}",
            variant, _m, xl, xh, yl, yh
        ));

        let mut pos = i + 6;

        // GS v 0: xL/xH are width in BYTES, yL/yH are height in DOTS (pixels)
        let width_in_bytes = xl + (xh << 8);
        let height = yl + (yh << 8);
        let width = width_in_bytes * 8; // Convert bytes to pixels for rendering

        // Validate dimensions
        if width_in_bytes == 0 || height == 0 {
            self.log_debug(&format!(
                "GS v invalid dimensions: {} bytes x {} pixels",
                width_in_bytes, height
            ));
            return Ok(pos);
        }

        if width > 10000 || height > 10000 {
            self.log_debug(&format!(
                "GS v dimensions too large: {}x{} pixels, attempting to skip raster data",
                width, height
            ));
            // Still need to skip the raster data even if dimensions seem wrong
            // Otherwise the raster bytes will be processed as text
            let total_bytes = width_in_bytes * height;
            if total_bytes > 5_000_000 {
                self.log_debug("GS v: calculated bytes too large, cannot skip safely");
                return Ok(start_i); // Wait for correct data or give up
            }
            if pos + total_bytes > data.len() {
                self.log_debug(&format!(
                    "GS v: not enough data to skip (need {} more bytes)",
                    total_bytes - (data.len() - pos)
                ));
                return Ok(start_i); // Wait for more data
            }
            return Ok(pos + total_bytes); // Skip past the raster data
        }

        let total_bytes = width_in_bytes * height;

        self.log_debug(&format!(
            "GS v raster: width={} pixels ({} bytes), height={} pixels, need {} bytes",
            width, width_in_bytes, height, total_bytes
        ));

        if total_bytes > 5_000_000 {
            self.log_debug("GS v raster: calculated bytes too large, skipping");
            return Ok(pos);
        }

        if pos + total_bytes > data.len() {
            self.log_debug(&format!(
                "GS v incomplete: have {}, need {}",
                data.len() - pos,
                total_bytes
            ));
            return Ok(start_i);
        }

        // Additional safety check before slicing
        if pos >= data.len() || pos + total_bytes > data.len() {
            self.log_debug("GS v bounds check failed");
            return Ok(start_i);
        }

        // Flush any pending text before image (already cleared by caller)
        if !self.current_line.is_empty() {
            self.flush_line();
            self.current_line.clear();
        }

        // Debug: dump first 64 bytes of raster data to see the pattern
        if self.debug {
            let preview_len = std::cmp::min(64, total_bytes);
            let mut hex_str = String::new();
            for i in 0..preview_len {
                hex_str.push_str(&format!("{:02X} ", data[pos + i]));
                if (i + 1) % 16 == 0 {
                    hex_str.push('\n');
                }
            }
            self.log_debug(&format!(
                "GS v raster data (first {} bytes):\n{}",
                preview_len, hex_str
            ));

            // Also show bytes per line calculation
            self.log_debug(&format!(
                "Width={} pixels -> {} bytes per line, {} total lines",
                width, width_in_bytes, height
            ));

            // Save raster data to a PBM file for inspection
            use std::io::Write;
            let filename = format!("raster_{}x{}.pbm", width, height);
            if let Ok(mut file) = std::fs::File::create(&filename) {
                // PBM format: P4 (binary)
                writeln!(file, "P4").ok();
                writeln!(file, "{} {}", width, height).ok();
                file.write_all(&data[pos..pos + total_bytes]).ok();
                self.log_debug(&format!("Saved raster to {}", filename));
            }
        }

        // GS v data is in standard raster format (row-based), NOT column format
        // Just use the data directly
        self.elements.push(ReceiptElement::RasterImage {
            width,
            height,
            data: data[pos..pos + total_bytes].to_vec(),
            offset: self.state.horizontal_offset,
            density: self.state.print_density,
            alignment: self.state.alignment.clone(),
            bytes_per_line: width_in_bytes, // Use actual bytes from command
            print_area_width: self.state.print_area_width,
        });

        // Reset offset after rendering
        self.state.horizontal_offset = 0;

        // Mark that we just processed binary data - don't treat following ASCII bytes as text
        self.last_was_binary = true;

        pos += total_bytes;

        Ok(pos)
    }

    fn handle_gs_8l(&mut self, data: &[u8], mut i: usize) -> Result<usize> {
        let start_i = i - 1;

        // GS 8 L p1 p2 p3 p4 m fn a bx by c xL xH yL yH d1...dk
        if i + 10 > data.len() {
            self.log_debug("GS 8 L incomplete: not enough header bytes");
            return Ok(start_i);
        }

        i += 1; // skip 'L'

        let p1 = data[i] as u32;
        let p2 = data[i + 1] as u32;
        let p3 = data[i + 2] as u32;
        let p4 = data[i + 3] as u32;
        let data_len = p1 | (p2 << 8) | (p3 << 16) | (p4 << 24);

        let m = data[i + 4];
        let _fn = data[i + 5];
        let _a = data[i + 6];
        let _bx = data[i + 7];
        let _by = data[i + 8];
        let _c = data[i + 9];

        i += 10;

        if m == 48 || m == 112 {
            if i + 4 > data.len() {
                self.log_debug("GS 8 L incomplete: not enough dimension bytes");
                return Ok(start_i);
            }

            let xl = data[i] as usize;
            let xh = data[i + 1] as usize;
            let yl = data[i + 2] as usize;
            let yh = data[i + 3] as usize;

            let width = xl | (xh << 8);
            let height = yl | (yh << 8);

            i += 4;

            let image_bytes = width.div_ceil(8) * height;

            self.log_debug(&format!(
                "GS 8 L raster: m={}, width={}, height={}, need {} bytes",
                m, width, height, image_bytes
            ));

            if data_len as usize > 100_000 || image_bytes > 5_000_000 {
                self.log_debug("GS 8 L: dimensions too large, skipping");
                // data_len includes m,fn,a,bx,by,c (6 bytes) which we already consumed
                // We need to skip the remaining data_len - 6 bytes
                let skip = (data_len as usize).saturating_sub(6);
                if i + skip <= data.len() {
                    return Ok(i + skip);
                } else {
                    // Not enough data to skip - wait for more
                    return Ok(start_i);
                }
            }

            if i + image_bytes > data.len() {
                self.log_debug(&format!(
                    "GS 8 L incomplete: have {}, need {}",
                    data.len() - i,
                    image_bytes
                ));
                return Ok(start_i);
            }

            if !self.current_line.is_empty() {
                self.flush_line();
                self.current_line.clear();
            }

            self.elements.push(ReceiptElement::RasterImage {
                width,
                height,
                data: data[i..i + image_bytes].to_vec(),
                offset: self.state.horizontal_offset,
                density: self.state.print_density,
                alignment: self.state.alignment.clone(),
                bytes_per_line: width.div_ceil(8), // Calculate from pixel width
                print_area_width: self.state.print_area_width,
            });

            // Reset offset after rendering
            self.state.horizontal_offset = 0;

            // Mark that we just processed binary data
            self.last_was_binary = true;

            i += image_bytes;
        } else {
            let skip = (data_len as usize).saturating_sub(6);
            i += skip.min(data.len() - i);
        }

        Ok(i)
    }

    fn handle_qr_code(&mut self, data: &[u8], mut i: usize) -> Result<usize> {
        let start_i = i - 1;

        // GS ( k pL pH cn fn [parameters]
        if i + 4 > data.len() {
            self.log_debug("GS ( k incomplete: not enough header bytes");
            return Ok(start_i);
        }

        i += 1; // skip 'k'

        let p_l = data[i] as usize;
        let p_h = data[i + 1] as usize;
        let param_len = p_l | (p_h << 8);

        let cn = data[i + 2];
        let fn_code = data[i + 3];

        i += 4;

        if cn != 49 {
            // Not a QR code command
            let skip = param_len.saturating_sub(2);
            i += skip.min(data.len() - i);
            return Ok(i);
        }

        match fn_code {
            65 | 67 => {
                // 65: Set QR model, 67: Set module size
                if i < data.len() {
                    if fn_code == 67 {
                        self.qr_size = data[i];
                    }
                    i += 1;
                }
            }
            69 => {
                // Set error correction level
                if i < data.len() {
                    self.qr_error_correction = data[i];
                    i += 1;
                }
            }
            80 => {
                // Store QR data
                let data_len = param_len.saturating_sub(3);
                if i + data_len > data.len() {
                    self.log_debug("GS ( k QR data incomplete");
                    return Ok(start_i);
                }
                self.qr_data = data[i..i + data_len].to_vec();
                i += data_len;
            }
            81 => {
                // Print QR code
                if !self.qr_data.is_empty() {
                    if !self.current_line.is_empty() {
                        self.flush_line();
                        self.current_line.clear();
                    }

                    let qr_string = String::from_utf8_lossy(&self.qr_data).to_string();
                    let size = (self.qr_size as usize).clamp(1, 16);

                    self.elements.push(ReceiptElement::QrCode {
                        data: qr_string,
                        size,
                        alignment: self.state.alignment.clone(),
                        offset: self.state.horizontal_offset,
                        print_area_width: self.state.print_area_width,
                    });

                    // Reset horizontal offset after use
                    self.state.horizontal_offset = 0;

                    self.qr_data.clear();
                }
            }
            _ => {
                // Unknown QR function
                let skip = param_len.saturating_sub(2);
                i += skip.min(data.len() - i);
            }
        }

        Ok(i)
    }

    fn handle_paper_cut(&mut self, data: &[u8], mut i: usize) -> Result<usize> {
        let mode = data[i];
        i += 1;

        let cut_type = match mode {
            0 | 48 => "FULL CUT",
            1 | 49 => "PARTIAL CUT",
            65 => "FEED & FULL CUT",
            66 => "FEED & PARTIAL CUT",
            _ => "UNKNOWN CUT",
        };

        self.flush_line();
        self.elements.push(ReceiptElement::PaperCut {
            cut_type: cut_type.to_string(),
        });

        Ok(i)
    }
}

#[derive(Clone)]
struct AppState {
    elements: Arc<Mutex<Vec<ReceiptElement>>>,
    connections: Arc<Mutex<Vec<String>>>,
    paper_size: Arc<Mutex<PaperSize>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            elements: Arc::new(Mutex::new(Vec::new())),
            connections: Arc::new(Mutex::new(Vec::new())),
            paper_size: Arc::new(Mutex::new(PaperSize::Size80mm)),
        }
    }
}

struct VirtualEscPosApp {
    state: AppState,
}

impl VirtualEscPosApp {
    fn new(_cc: &eframe::CreationContext, state: AppState) -> Self {
        Self { state }
    }
}

impl eframe::App for VirtualEscPosApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();

        // Force light mode, ignoring OS dark mode
        ctx.set_visuals(egui::Visuals::light());

        let mut style = (*ctx.style()).clone();
        style.visuals.panel_fill = egui::Color32::WHITE;
        style.visuals.window_fill = egui::Color32::WHITE;
        style.visuals.popup_shadow = egui::epaint::Shadow::NONE;
        style.visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::BLACK;
        style.visuals.widgets.noninteractive.bg_fill = egui::Color32::WHITE;
        style.visuals.widgets.inactive.fg_stroke.color = egui::Color32::BLACK;
        style.visuals.widgets.inactive.bg_fill = egui::Color32::from_gray(245);
        style.visuals.widgets.active.fg_stroke.color = egui::Color32::BLACK;
        style.visuals.widgets.active.bg_fill = egui::Color32::from_gray(230);
        style.visuals.widgets.hovered.fg_stroke.color = egui::Color32::BLACK;
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_gray(250);
        style.visuals.widgets.open.fg_stroke.color = egui::Color32::BLACK;
        style.visuals.widgets.open.bg_fill = egui::Color32::from_gray(250);
        style.visuals.extreme_bg_color = egui::Color32::WHITE;
        style.visuals.faint_bg_color = egui::Color32::from_gray(250);
        style.visuals.selection.bg_fill = egui::Color32::from_gray(248);
        style.visuals.selection.stroke.color = egui::Color32::BLACK;
        ctx.set_style(style);

        let mut current_paper_size = *self.state.paper_size.lock().unwrap();
        let mut paper_size_changed = false;

        egui::TopBottomPanel::top("menu_bar")
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::WHITE)
                    .inner_margin(4.0),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.scope(|ui| {
                        let style = ui.style_mut();
                        // Dropdown button (inactive state)
                        style.visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_gray(245);
                        style.visuals.widgets.inactive.bg_fill = egui::Color32::from_gray(245);
                        style.visuals.widgets.inactive.fg_stroke.color = egui::Color32::BLACK;

                        // Noninteractive (selected items with checkmark)
                        style.visuals.widgets.noninteractive.weak_bg_fill =
                            egui::Color32::from_gray(248);
                        style.visuals.widgets.noninteractive.bg_fill =
                            egui::Color32::from_gray(248);
                        style.visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::BLACK;

                        // Hover state
                        style.visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_gray(250);
                        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_gray(250);
                        style.visuals.widgets.hovered.fg_stroke.color = egui::Color32::BLACK;

                        // Active/clicked state
                        style.visuals.widgets.active.weak_bg_fill = egui::Color32::from_gray(240);
                        style.visuals.widgets.active.bg_fill = egui::Color32::from_gray(240);
                        style.visuals.widgets.active.fg_stroke.color = egui::Color32::BLACK;

                        // Open state
                        style.visuals.widgets.open.weak_bg_fill = egui::Color32::from_gray(250);
                        style.visuals.widgets.open.bg_fill = egui::Color32::from_gray(250);
                        style.visuals.widgets.open.fg_stroke.color = egui::Color32::BLACK;

                        // Selection highlight
                        style.visuals.selection.bg_fill = egui::Color32::from_gray(248);
                        style.visuals.selection.stroke.color = egui::Color32::BLACK;

                        egui::ComboBox::from_id_salt("paper_size")
                            .selected_text(current_paper_size.label())
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_value(
                                        &mut current_paper_size,
                                        PaperSize::Size58mm,
                                        "58mm",
                                    )
                                    .clicked()
                                {
                                    let old_size = *self.state.paper_size.lock().unwrap();
                                    if old_size != PaperSize::Size58mm {
                                        *self.state.paper_size.lock().unwrap() =
                                            PaperSize::Size58mm;
                                        paper_size_changed = true;
                                    }
                                }
                                if ui
                                    .selectable_value(
                                        &mut current_paper_size,
                                        PaperSize::Size80mm,
                                        "80mm",
                                    )
                                    .clicked()
                                {
                                    let old_size = *self.state.paper_size.lock().unwrap();
                                    if old_size != PaperSize::Size80mm {
                                        *self.state.paper_size.lock().unwrap() =
                                            PaperSize::Size80mm;
                                        paper_size_changed = true;
                                    }
                                }
                            });
                    });

                    ui.separator();

                    // Clear button
                    ui.scope(|ui| {
                        let style = ui.style_mut();
                        style.visuals.widgets.inactive.weak_bg_fill =
                            egui::Color32::from_rgb(245, 245, 245);
                        style.visuals.widgets.inactive.bg_fill =
                            egui::Color32::from_rgb(245, 245, 245);
                        style.visuals.widgets.inactive.fg_stroke.color = egui::Color32::BLACK;
                        style.visuals.widgets.hovered.weak_bg_fill =
                            egui::Color32::from_rgb(230, 230, 230);
                        style.visuals.widgets.hovered.bg_fill =
                            egui::Color32::from_rgb(230, 230, 230);
                        style.visuals.widgets.active.weak_bg_fill =
                            egui::Color32::from_rgb(210, 210, 210);
                        style.visuals.widgets.active.bg_fill =
                            egui::Color32::from_rgb(210, 210, 210);

                        if ui.button("Clear").clicked() {
                            self.state.elements.lock().unwrap().clear();
                        }
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.colored_label(
                            egui::Color32::DARK_GRAY,
                            format!("{}cpl | :9100", current_paper_size.chars_per_line()),
                        );
                    });
                });
            });

        // Clear receipt when paper size changes
        if paper_size_changed {
            self.state.elements.lock().unwrap().clear();
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_gray(245)))
            .show(ctx, |ui| {
                let connections = self.state.connections.lock().unwrap();
                if !connections.is_empty() {
                    ui.label(format!("Active connections: {}", connections.len()));
                    for conn in connections.iter() {
                        ui.label(conn);
                    }
                    ui.separator();
                }
                drop(connections);

                // Fixed width scroll area matching 80mm receipt paper
                let printer_width_px = current_paper_size.width_px();
                let printer_chars_per_line = current_paper_size.chars_per_line();

                // Center the receipt area horizontally
                ui.vertical_centered(|ui| {
                    ui.set_width(printer_width_px + 2.0); // +2 for border

                    // Receipt paper frame with border
                    egui::Frame::none()
                        .fill(egui::Color32::WHITE)
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(200)))
                        .inner_margin(0.0)
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .auto_shrink([false; 2])
                                .max_height(ui.available_height())
                                .show(ui, |ui| {
                                    ui.set_width(printer_width_px);
                                    let elements = self.state.elements.lock().unwrap();

                                    if elements.is_empty() {
                                        ui.add_space(100.0);
                                        ui.vertical_centered(|ui| {
                                            ui.colored_label(
                                                egui::Color32::DARK_GRAY,
                                                "Receipt empty",
                                            );
                                            ui.add_space(10.0);
                                            ui.colored_label(
                                                egui::Color32::GRAY,
                                                "Send print job to port 9100",
                                            );
                                            if paper_size_changed {
                                                ui.add_space(5.0);
                                                ui.colored_label(
                                                    egui::Color32::from_rgb(200, 150, 0),
                                                    format!(
                                                        "Paper size changed to {}",
                                                        current_paper_size.label()
                                                    ),
                                                );
                                            }
                                        });
                                    }

                                    for element in elements.iter() {
                                        match element {
                                            ReceiptElement::Text {
                                                content,
                                                bold,
                                                underline,
                                                double_width,
                                                double_height,
                                                inverted,
                                                alignment,
                                                density,
                                                offset,
                                                left_margin,
                                                character_spacing,
                                                double_strike,
                                                font,
                                                print_area_width,
                                            } => {
                                                let mut job = egui::text::LayoutJob::default();

                                                // Use print_area_width (GS W) for content sizing
                                                // when set, otherwise fall back to full printer width
                                                let effective_width = if *print_area_width > 0 {
                                                    *print_area_width as f32
                                                } else {
                                                    printer_width_px
                                                };

                                                // Calculate font size to fit chars per line
                                                // Measure actual monospace advance width ratio
                                                let char_width =
                                                    effective_width / printer_chars_per_line as f32;
                                                let ref_size = 20.0_f32;
                                                let ref_galley = ui.fonts(|f| {
                                                    f.layout_job(
                                                        egui::text::LayoutJob::simple_singleline(
                                                            "M".to_string(),
                                                            egui::FontId::monospace(ref_size),
                                                            egui::Color32::BLACK,
                                                        ),
                                                    )
                                                });
                                                let mono_ratio = ref_galley.size().x / ref_size;
                                                let base_font_size = char_width / mono_ratio;

                                                // Apply font selection (Font B is ~75% of Font A size)
                                                let font_multiplier = match font {
                                                    1 => 0.75, // Font B - smaller
                                                    2 => 0.65, // Font C - even smaller (if used)
                                                    _ => 1.0,  // Font A - standard
                                                };

                                                let mut size = base_font_size * font_multiplier;
                                                if *double_width || *double_height {
                                                    size = base_font_size * font_multiplier * 1.5;
                                                }

                                                // Always use monospace for consistent character widths
                                                // ESC/POS printers use fixed-width fonts
                                                // Bold will be rendered by egui's text rendering (stroke weight)
                                                let font_id = egui::FontId::monospace(size);

                                                // Apply bold, double-strike, and density
                                                let color = if *inverted {
                                                    egui::Color32::WHITE
                                                } else {
                                                    // Bold or double-strike makes text darker
                                                    if *bold || *double_strike {
                                                        egui::Color32::BLACK
                                                    } else {
                                                        match density {
                                                            0 => egui::Color32::LIGHT_GRAY,
                                                            1 => egui::Color32::GRAY,
                                                            2 => egui::Color32::DARK_GRAY,
                                                            _ => egui::Color32::BLACK, // 3-8: normal black
                                                        }
                                                    }
                                                };

                                                let bg_color = if *inverted {
                                                    egui::Color32::BLACK
                                                } else {
                                                    egui::Color32::TRANSPARENT
                                                };

                                                // Apply character spacing (ESC SP)
                                                let extra_letter_spacing =
                                                    *character_spacing as f32;

                                                job.append(
                                                    content,
                                                    0.0,
                                                    egui::TextFormat {
                                                        font_id,
                                                        color,
                                                        background: bg_color,
                                                        underline: if *underline {
                                                            egui::Stroke::new(1.0, color)
                                                        } else {
                                                            egui::Stroke::NONE
                                                        },
                                                        extra_letter_spacing,
                                                        ..Default::default()
                                                    },
                                                );

                                                let galley = ui.fonts(|f| f.layout_job(job));

                                                // Allocate full width for 80mm receipt paper
                                                let line_height = galley.size().y;

                                                let (rect, _) = ui.allocate_exact_size(
                                                    egui::vec2(printer_width_px, line_height),
                                                    egui::Sense::hover(),
                                                );

                                                // Apply left margin (GS L)
                                                let margin_offset = *left_margin as f32;

                                                // Center the printable area within the paper
                                                let area_offset = if *print_area_width > 0 {
                                                    (printer_width_px - *print_area_width as f32)
                                                        / 2.0
                                                } else {
                                                    0.0
                                                };

                                                // Calculate base position from alignment
                                                // All alignments use area_offset so content
                                                // stays within the GS W print area
                                                let base_x = match alignment {
                                                    Alignment::Left => {
                                                        rect.left() + area_offset + margin_offset
                                                    }
                                                    Alignment::Center => {
                                                        rect.left()
                                                            + area_offset
                                                            + margin_offset
                                                            + (effective_width
                                                                - galley.size().x
                                                                - margin_offset)
                                                                / 2.0
                                                    }
                                                    Alignment::Right => {
                                                        rect.left() + area_offset + effective_width
                                                            - galley.size().x
                                                    }
                                                };

                                                // Apply horizontal offset (from ESC $ / ESC \ commands)
                                                // Offset is in pixels, add to base position
                                                let final_x = if *offset > 0 {
                                                    rect.left() + margin_offset + *offset as f32
                                                } else {
                                                    base_x
                                                };

                                                let pos = egui::pos2(final_x, rect.top());

                                                ui.painter().galley(pos, galley, color);
                                            }
                                            ReceiptElement::RasterImage {
                                                width,
                                                height,
                                                data,
                                                offset,
                                                density,
                                                alignment,
                                                bytes_per_line,
                                                print_area_width,
                                            } => {
                                                render_raster_image(
                                                    ui,
                                                    *width,
                                                    *height,
                                                    data,
                                                    *offset,
                                                    *density,
                                                    alignment,
                                                    printer_width_px,
                                                    *bytes_per_line,
                                                    *print_area_width,
                                                );
                                            }
                                            ReceiptElement::QrCode {
                                                data,
                                                size,
                                                alignment,
                                                offset,
                                                print_area_width,
                                            } => {
                                                render_qr_code(
                                                    ui,
                                                    data,
                                                    *size,
                                                    alignment,
                                                    *offset,
                                                    *print_area_width,
                                                    printer_width_px,
                                                );
                                            }
                                            ReceiptElement::PaperCut { cut_type } => {
                                                ui.separator();
                                                ui.horizontal(|ui| {
                                                    ui.label("✂");
                                                    ui.strong(format!("PAPER CUT: {}", cut_type));
                                                });
                                                ui.separator();
                                            }
                                            ReceiptElement::CashDrawer {
                                                pin,
                                                on_time,
                                                off_time,
                                            } => {
                                                ui.separator();
                                                ui.horizontal(|ui| {
                                                    ui.label("💰");
                                                    ui.strong("CASH DRAWER OPEN");
                                                });
                                                ui.label(format!(
                                                    "Pin: {}  On: {}ms  Off: {}ms",
                                                    pin,
                                                    *on_time as u32 * 2,
                                                    *off_time as u32 * 2
                                                ));
                                                ui.separator();
                                            }
                                            ReceiptElement::Separator => {
                                                ui.add_space(4.0);
                                            }
                                            ReceiptElement::FormFeed => {
                                                // Don't add artificial spacing - only show protocol breaks
                                            }
                                        }
                                    }
                                });
                        });
                });
            });
    }
}

#[allow(clippy::too_many_arguments)]
fn render_raster_image(
    ui: &mut egui::Ui,
    width: usize,
    height: usize,
    data: &[u8],
    offset: u16,
    density: u8,
    alignment: &Alignment,
    printer_width_px: f32,
    bytes_per_line: usize,
    print_area_width: u16,
) {
    // Use the actual bytes_per_line from the command, not recalculated
    let mut pixels = Vec::with_capacity(width * height);

    // Apply density/darkness control to raster images
    // Density 0-8 maps to different gray levels for lighter/darker printing
    let ink_color = match density {
        0 => egui::Color32::from_gray(180), // Very light
        1 => egui::Color32::from_gray(130), // Light
        2 => egui::Color32::from_gray(80),  // Slightly light
        _ => egui::Color32::BLACK,          // 3-8: normal black
    };

    for y in 0..height {
        for x in 0..width {
            let byte_idx = y * bytes_per_line + (x / 8);
            // MSB-first bit order: bit 7 (0x80) is leftmost pixel, bit 0 (0x01) is rightmost
            let bit_idx = 7 - (x % 8);

            if byte_idx < data.len() {
                let bit = (data[byte_idx] >> bit_idx) & 1;
                // Standard ESC/POS: 1=black (printed), 0=white (not printed)
                if bit == 1 {
                    pixels.push(ink_color); // Bit 1 = black
                } else {
                    pixels.push(egui::Color32::WHITE); // Bit 0 = white
                }
            } else {
                pixels.push(egui::Color32::WHITE);
            }
        }
    }

    let image = egui::ColorImage {
        size: [width, height],
        pixels,
    };

    let texture = ui.ctx().load_texture(
        format!("raster_{}x{}_{}", width, height, offset),
        image,
        egui::TextureOptions::NEAREST,
    );

    // Use print_area_width (GS W) for alignment when set,
    // otherwise fall back to full printer width
    let effective_width = if print_area_width > 0 {
        print_area_width as f32
    } else {
        printer_width_px
    };

    // Scale up the image for better visibility (thermal printers are 203 DPI, screens are ~96 DPI)
    // Use adaptive scaling: small images (text) get 3x, large images (logos) get 1x
    // Clamp so the image never exceeds the printable area
    let scale_factor = if width > 300 || height > 150 {
        1.0
    } else {
        3.0_f32.min(effective_width / width as f32)
    };
    let display_width = width as f32 * scale_factor;
    let display_height = height as f32 * scale_factor;

    // Allocate full printer width for proper alignment
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(printer_width_px, display_height),
        egui::Sense::hover(),
    );

    // Center the printable area within the paper width
    let area_offset = if print_area_width > 0 {
        (printer_width_px - print_area_width as f32) / 2.0
    } else {
        0.0
    };

    // Calculate horizontal position based on alignment and offset
    // For CENTER/RIGHT, center the printable area within the paper.
    // For LEFT, use left edge only.
    let x_offset = match alignment {
        Alignment::Left => offset as f32 * scale_factor,
        Alignment::Center => {
            area_offset + (effective_width - display_width) / 2.0 + offset as f32 * scale_factor
        }
        Alignment::Right => {
            area_offset + effective_width - display_width - offset as f32 * scale_factor
        }
    };

    let pos = egui::pos2(rect.left() + x_offset, rect.top());
    let size = egui::vec2(display_width, display_height);

    ui.painter().image(
        texture.id(),
        egui::Rect::from_min_size(pos, size),
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        egui::Color32::WHITE,
    );
}

fn render_qr_code(
    ui: &mut egui::Ui,
    data: &str,
    size: usize,
    alignment: &Alignment,
    offset: u16,
    print_area_width: u16,
    printer_width_px: f32,
) {
    match QrCode::new(data.as_bytes()) {
        Ok(qr) => {
            let colors = qr.to_colors();
            let width = qr.width();
            let module_size = size.clamp(1, 8);
            let pixel_size = width * module_size;

            let mut pixels = Vec::with_capacity(pixel_size * pixel_size);

            for y in 0..width {
                for _ in 0..module_size {
                    for x in 0..width {
                        let idx = y * width + x;
                        let color = match colors[idx] {
                            QrColor::Dark => egui::Color32::BLACK,
                            QrColor::Light => egui::Color32::WHITE,
                        };
                        for _ in 0..module_size {
                            pixels.push(color);
                        }
                    }
                }
            }

            let image = egui::ColorImage {
                size: [pixel_size, pixel_size],
                pixels,
            };

            let texture = ui.ctx().load_texture(
                format!("qr_{}", data.chars().take(20).collect::<String>()),
                image,
                egui::TextureOptions::NEAREST,
            );

            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(printer_width_px, pixel_size as f32),
                egui::Sense::hover(),
            );

            // Use print_area_width (GS W) for alignment when set,
            // otherwise fall back to full printer width
            let effective_width = if print_area_width > 0 {
                print_area_width as f32
            } else {
                printer_width_px
            };

            // Center the printable area within the paper width
            let area_offset = if print_area_width > 0 {
                (printer_width_px - print_area_width as f32) / 2.0
            } else {
                0.0
            };

            // Calculate base position from alignment
            // For CENTER/RIGHT, center the printable area within the paper.
            // For LEFT, use left edge only.
            let base_x = match alignment {
                Alignment::Left => 0.0,
                Alignment::Center => area_offset + (effective_width - pixel_size as f32) / 2.0,
                Alignment::Right => area_offset + effective_width - pixel_size as f32,
            };

            // Apply horizontal offset (from ESC $ / ESC \ commands)
            let final_x = if offset > 0 { offset as f32 } else { base_x };

            let pos = egui::pos2(rect.left() + final_x, rect.top());
            let size = egui::vec2(pixel_size as f32, pixel_size as f32);

            ui.painter().image(
                texture.id(),
                egui::Rect::from_min_size(pos, size),
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        Err(e) => {
            ui.colored_label(egui::Color32::RED, format!("QR Code Error: {:?}", e));
        }
    }
}

async fn handle_client(
    mut socket: tokio::net::TcpStream,
    addr: std::net::SocketAddr,
    state: AppState,
    debug: bool,
) -> Result<()> {
    {
        let mut connections = state.connections.lock().unwrap();
        connections.push(format!("Connected: {}", addr));
    }

    let mut renderer = EscPosRenderer::new(debug);
    let mut buffer = vec![0u8; 8192];

    // Open file for raw data capture if debug enabled
    let mut raw_file = if debug {
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open("escpos_capture.raw")
            .ok()
    } else {
        None
    };

    loop {
        match socket.read(&mut buffer).await {
            Ok(0) => {
                let mut connections = state.connections.lock().unwrap();
                connections.retain(|c| !c.contains(&addr.to_string()));
                break;
            }
            Ok(n) => {
                // Save raw data if debug enabled
                if let Some(ref mut file) = raw_file {
                    use std::io::Write;
                    let _ = file.write_all(&buffer[..n]);
                }

                if debug {
                    eprintln!("[DEBUG] Received {} bytes: {:02X?}", n, &buffer[..n]);
                }

                if let Err(e) = renderer.process_data(&buffer[..n]) {
                    eprintln!("Error processing data: {}", e);
                }

                // Send any queued responses (status queries, etc.)
                let responses = renderer.take_responses();
                if !responses.is_empty() {
                    if debug {
                        eprintln!(
                            "[DEBUG] Sending {} response bytes: {:02X?}",
                            responses.len(),
                            responses
                        );
                    }
                    if let Err(e) = socket.write_all(&responses).await {
                        eprintln!("Error sending responses: {}", e);
                    }
                    if let Err(e) = socket.flush().await {
                        eprintln!("Error flushing socket: {}", e);
                    }
                }

                let new_elements = renderer.take_elements();
                if !new_elements.is_empty() {
                    let mut elements = state.elements.lock().unwrap();
                    elements.extend(new_elements);
                }
            }
            Err(e) => {
                eprintln!("Error reading from socket: {}", e);
                break;
            }
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    let debug = std::env::var("DEBUG").is_ok();
    let state = AppState::new();
    let state_clone = state.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let listener = match TcpListener::bind("0.0.0.0:9100").await {
                Ok(listener) => listener,
                Err(e) => {
                    eprintln!("ERROR: Failed to bind to port 9100: {}", e);
                    eprintln!("Port 9100 is already in use. Please:");
                    eprintln!("  1. Stop any other escpresso instances");
                    eprintln!("  2. Check for other applications using port 9100:");
                    eprintln!("     lsof -i :9100");
                    eprintln!("     netstat -tulpn | grep 9100");
                    std::process::exit(1);
                }
            };
            println!("TCP Server listening on 0.0.0.0:9100");
            if debug {
                eprintln!("[DEBUG] Debug mode enabled");
            }

            loop {
                match listener.accept().await {
                    Ok((socket, addr)) => {
                        let state = state_clone.clone();
                        let debug_flag = debug;
                        tokio::spawn(async move {
                            if let Err(e) = handle_client(socket, addr, state, debug_flag).await {
                                eprintln!("Error handling client {}: {}", addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Error accepting connection: {}", e);
                    }
                }
            }
        });
    });

    let default_width = PaperSize::Size80mm.width_px();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([default_width + 40.0, 800.0]) // Receipt width + padding
            .with_title("escpresso"),
        ..Default::default()
    };

    eframe::run_native(
        "escpresso",
        options,
        Box::new(move |cc| Ok(Box::new(VirtualEscPosApp::new(cc, state)))),
    )
    .map_err(|e| anyhow::anyhow!("Failed to run app: {}", e))
}
