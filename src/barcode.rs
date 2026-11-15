//! GS k barcodes: turns the symbology byte and the data of a GS k command into
//! modules (1 = bar, 0 = space) the preview draws, plus the human readable
//! interpretation (HRI) a printer prints with them.
//!
//! Code 128 is encoded here, because ESC/POS sends it in its own form: the
//! data starts with `{A`, `{B` or `{C` to pick the code set, `{` escapes
//! switch sets and insert function characters, and in code set C each byte is
//! a value 0-99 (not two ASCII digits). Every other symbology goes through
//! the `barcoders` crate.

use barcoders::sym::code39::Code39;
use barcoders::sym::code93::Code93;
use barcoders::sym::ean13::EAN13;
use barcoders::sym::ean8::EAN8;
use barcoders::sym::tf::TF;

/// The symbology's name, the HRI text, and the modules (or why the data
/// cannot be encoded) for GS k with symbology `m` and `data`.
pub fn encode(m: u8, data: &[u8]) -> (&'static str, String, Result<Vec<u8>, String>) {
    let text: String = data.iter().map(|&b| b as char).collect();
    let err = |e: barcoders::error::Error| format!("{:?}", e);
    match m {
        0 | 65 => (
            "UPC-A",
            text.clone(),
            EAN13::new(format!("0{}", text))
                .map(|b| b.encode())
                .map_err(err),
        ),
        1 | 66 => ("UPC-E", text, Err("UPC-E is not drawn yet".to_string())),
        2 | 67 => (
            "EAN-13",
            text.clone(),
            EAN13::new(&text).map(|b| b.encode()).map_err(err),
        ),
        3 | 68 => (
            "EAN-8",
            text.clone(),
            EAN8::new(&text).map(|b| b.encode()).map_err(err),
        ),
        4 | 69 => {
            // The printer adds the '*' start/stop characters itself.
            let body = text.trim_matches('*').to_string();
            (
                "CODE39",
                text,
                Code39::new(&body).map(|b| b.encode()).map_err(err),
            )
        }
        5 | 70 => (
            "ITF",
            text.clone(),
            TF::interleaved(&text).map(|b| b.encode()).map_err(err),
        ),
        6 | 71 => ("CODABAR", text.clone(), codabar(&text)),
        72 => (
            "CODE93",
            text.clone(),
            Code93::new(&text).map(|b| b.encode()).map_err(err),
        ),
        73 => match code128(data) {
            Ok((modules, hri)) => ("CODE128", hri, Ok(modules)),
            Err(e) => ("CODE128", text, Err(e)),
        },
        _ => ("unknown", text, Err(format!("unknown symbology m={}", m))),
    }
}

/// Bar and space widths of the 107 Code 128 symbols (values 0-102, the starts
/// A/B/C at 103-105 and the stop at 106), bar first.
const C128: [&str; 107] = [
    "212222", "222122", "222221", "121223", "121322", "131222", "122213", "122312", "132212",
    "221213", "221312", "231212", "112232", "122132", "122231", "113222", "123122", "123221",
    "223211", "221132", "221231", "213212", "223112", "312131", "311222", "321122", "321221",
    "312212", "322112", "322211", "212123", "212321", "232121", "111323", "131123", "131321",
    "112313", "132113", "132311", "211313", "231113", "231311", "112133", "112331", "132131",
    "113123", "113321", "133121", "313121", "211331", "231131", "213113", "213311", "213131",
    "311123", "311321", "331121", "312113", "312311", "332111", "314111", "221411", "431111",
    "111224", "111422", "121124", "121421", "141122", "141221", "112214", "112412", "122114",
    "122411", "142112", "142211", "241211", "221114", "413111", "241112", "134111", "111242",
    "121142", "121241", "114212", "124112", "124211", "411212", "421112", "421211", "212141",
    "214121", "412121", "111143", "111341", "131141", "114113", "114311", "411113", "411311",
    "113141", "114131", "311141", "411131", "211412", "211214", "211232", "2331112",
];

#[derive(Clone, Copy, PartialEq, Debug)]
enum Set {
    A,
    B,
    C,
}

/// The value of character `c` in code set A or B, if it has one.
fn value_in(set: Set, c: u8) -> Option<u8> {
    match set {
        Set::A => match c {
            0x20..=0x5F => Some(c - 0x20),
            0x00..=0x1F => Some(c + 64),
            _ => None,
        },
        Set::B => match c {
            0x20..=0x7F => Some(c - 0x20),
            _ => None,
        },
        Set::C => None,
    }
}

/// Encodes ESC/POS Code 128 data: the symbol values (start, data, check,
/// stop) as modules, and the HRI text.
fn code128(data: &[u8]) -> Result<(Vec<u8>, String), String> {
    let mut i = 0;
    let mut set = match data {
        [b'{', b'A', ..] => Set::A,
        [b'{', b'B', ..] => Set::B,
        [b'{', b'C', ..] => Set::C,
        // A printer refuses data without a code set; read it as set B
        // rather than show nothing.
        _ => Set::B,
    };
    if data.len() >= 2 && data[0] == b'{' && matches!(data[1], b'A' | b'B' | b'C') {
        i = 2;
    }
    let mut values: Vec<u8> = vec![match set {
        Set::A => 103,
        Set::B => 104,
        Set::C => 105,
    }];
    let mut hri = String::new();
    let mut shift_next = false;

    while i < data.len() {
        let c = data[i];
        if c == b'{' && i + 1 < data.len() {
            let e = data[i + 1];
            i += 2;
            match e {
                b'A' | b'B' | b'C' => {
                    let to = match e {
                        b'A' => Set::A,
                        b'B' => Set::B,
                        _ => Set::C,
                    };
                    if to != set {
                        values.push(match to {
                            Set::A => 101,
                            Set::B => 100,
                            Set::C => 99,
                        });
                        set = to;
                    }
                }
                b'S' => {
                    if set == Set::C {
                        return Err("SHIFT is not allowed in code set C".into());
                    }
                    values.push(98);
                    shift_next = true;
                }
                b'1' => values.push(102),
                b'2' | b'3' => {
                    if set == Set::C {
                        return Err("FNC2/FNC3 are not allowed in code set C".into());
                    }
                    values.push(if e == b'2' { 97 } else { 96 });
                }
                b'4' => match set {
                    Set::A => values.push(101),
                    Set::B => values.push(100),
                    Set::C => return Err("FNC4 is not allowed in code set C".into()),
                },
                b'{' => {
                    let in_set = if shift_next { other(set) } else { set };
                    shift_next = false;
                    let v = value_in(in_set, b'{')
                        .ok_or_else(|| "'{' cannot be encoded in code set A".to_string())?;
                    values.push(v);
                    hri.push('{');
                }
                other => {
                    return Err(format!("unknown escape {{{}", other as char));
                }
            }
            continue;
        }
        if set == Set::C && !shift_next {
            if c > 99 {
                return Err(format!("code set C takes values 0-99, got {}", c));
            }
            values.push(c);
            hri.push_str(&format!("{:02}", c));
        } else {
            let in_set = if shift_next { other(set) } else { set };
            shift_next = false;
            let v = value_in(in_set, c).ok_or_else(|| {
                format!(
                    "byte 0x{:02X} cannot be encoded in code set {:?}",
                    c, in_set
                )
            })?;
            values.push(v);
            if c >= 0x20 {
                hri.push(c as char);
            }
        }
        i += 1;
    }

    let check = values
        .iter()
        .enumerate()
        .map(|(pos, &v)| pos.max(1) as u32 * v as u32)
        .sum::<u32>()
        % 103;
    values.push(check as u8);
    values.push(106);

    let mut modules = Vec::new();
    for v in values {
        let mut bar = true;
        for w in C128[v as usize].bytes() {
            modules.resize(modules.len() + (w - b'0') as usize, u8::from(bar));
            bar = !bar;
        }
    }
    Ok((modules, hri))
}

/// Codabar patterns (1 = bar, 0 = space). Encoded here rather than with
/// `barcoders`, whose 2.0 table has the B and C start/stop patterns swapped.
const CODABAR: [(char, &str); 20] = [
    ('0', "101010011"),
    ('1', "101011001"),
    ('2', "101001011"),
    ('3', "110010101"),
    ('4', "101101001"),
    ('5', "110101001"),
    ('6', "100101011"),
    ('7', "100101101"),
    ('8', "100110101"),
    ('9', "110100101"),
    ('-', "101001101"),
    ('$', "101100101"),
    (':', "1101011011"),
    ('/', "1101101011"),
    ('.', "1101101101"),
    ('+', "1011011011"),
    ('A', "1011001001"),
    ('B', "1001001011"),
    ('C', "1010010011"),
    ('D', "1010011001"),
];

/// Codabar: the data carries its own start and stop characters (A-D); the
/// characters are separated by one narrow space.
fn codabar(text: &str) -> Result<Vec<u8>, String> {
    let chars: Vec<char> = text.chars().map(|c| c.to_ascii_uppercase()).collect();
    let is_guard = |c: &char| matches!(c, 'A'..='D');
    if chars.len() < 3 || !is_guard(&chars[0]) || !is_guard(&chars[chars.len() - 1]) {
        return Err("Codabar needs a start and a stop character (A-D)".into());
    }
    let mut modules = Vec::new();
    for (n, c) in chars.iter().enumerate() {
        if n > 0 {
            modules.push(0);
        }
        let (_, pattern) = CODABAR
            .iter()
            .find(|(k, _)| k == c)
            .ok_or_else(|| format!("'{}' is not a Codabar character", c))?;
        modules.extend(pattern.bytes().map(|b| b - b'0'));
    }
    Ok(modules)
}

/// Code set A for B and B for A: what SHIFT reads the next character in.
fn other(set: Set) -> Set {
    match set {
        Set::A => Set::B,
        _ => Set::A,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use barcoders::sym::code128::Code128;

    #[test]
    fn code128_set_b_matches_the_reference_encoder() {
        // barcoders picks the code set with 'Ɓ'; ESC/POS with "{B".
        let (ours, hri) = code128(b"{BRF457985").unwrap();
        let reference = Code128::new("ƁRF457985").unwrap().encode();
        assert_eq!(ours, reference);
        assert_eq!(hri, "RF457985");
    }

    #[test]
    fn code128_set_c_takes_values_not_ascii_digits() {
        // "{C" then the values 12, 34, 56: the HRI reads 123456.
        let (ours, hri) = code128(&[b'{', b'C', 12, 34, 56]).unwrap();
        let reference = Code128::new("Ć123456").unwrap().encode();
        assert_eq!(ours, reference);
        assert_eq!(hri, "123456");
    }

    #[test]
    fn code128_switches_sets_and_escapes_a_brace() {
        // "AB" in B, then 1234 in C, then "{" in B.
        let (ours, hri) = code128(&[
            b'{', b'B', b'A', b'B', b'{', b'C', 12, 34, b'{', b'B', b'{', b'{',
        ])
        .unwrap();
        assert_eq!(hri, "AB1234{");
        // start B, A, B, CODE C, 12, 34, CODE B, '{', check, stop
        assert_eq!(ours.len(), 9 * 11 + 13);
    }

    #[test]
    fn code128_refuses_what_a_set_cannot_hold() {
        assert!(code128(&[b'{', b'C', 100]).is_err());
        assert!(code128(b"{Aabc").is_err());
    }

    #[test]
    fn codabar_start_and_stop_patterns_follow_the_standard() {
        // A ... B: the B stop must be 1001001011 (barcoders 2.0 draws C there).
        let m = codabar("A1B").unwrap();
        let s: String = m.iter().map(|b| char::from(b'0' + b)).collect();
        assert_eq!(
            s,
            "1011001001".to_owned() + "0" + "101011001" + "0" + "1001001011"
        );
        assert!(codabar("1234").is_err(), "no start/stop characters");
        assert!(codabar("A12XB").is_err());
    }

    #[test]
    fn other_symbologies_encode() {
        for (m, data) in [
            (67u8, &b"4006381333931"[..]),
            (2, b"400638133393"),
            (68, b"96385074"),
            (65, b"03600029145"),
            (69, b"*CODE39*"),
            (70, b"1234567890"),
            (72, b"CODE93"),
            (71, b"A40156B"),
        ] {
            let (name, _, modules) = encode(m, data);
            assert!(modules.is_ok(), "{} {:?}: {:?}", name, data, modules);
        }
        assert!(
            encode(66, b"0123456").2.is_err(),
            "UPC-E is reported, not drawn"
        );
        assert!(encode(99, b"x").2.is_err());
    }
}
