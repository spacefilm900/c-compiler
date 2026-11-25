//! Universal character names and extended characters in identifiers
//! (C11 6.4.2.1, 6.4.3 and Annex D).
//!
//! An identifier may contain `\uXXXX` or `\UXXXXXXXX`, and, as in GCC and
//! Clang, the same characters written directly in UTF-8. Both spellings
//! name the same identifier: the lexers store every identifier as UTF-8,
//! so `caf\u00e9` and `café` are one name, one macro and one symbol.

/// N1570 D.1: the ranges of characters allowed in an identifier.
const ALLOWED: [(u32, u32); 45] = [
    (0x00A8, 0x00A8),
    (0x00AA, 0x00AA),
    (0x00AD, 0x00AD),
    (0x00AF, 0x00AF),
    (0x00B2, 0x00B5),
    (0x00B7, 0x00BA),
    (0x00BC, 0x00BE),
    (0x00C0, 0x00D6),
    (0x00D8, 0x00F6),
    (0x00F8, 0x00FF),
    (0x0100, 0x167F),
    (0x1681, 0x180D),
    (0x180F, 0x1FFF),
    (0x200B, 0x200D),
    (0x202A, 0x202E),
    (0x203F, 0x2040),
    (0x2054, 0x2054),
    (0x2060, 0x206F),
    (0x2070, 0x218F),
    (0x2460, 0x24FF),
    (0x2776, 0x2793),
    (0x2C00, 0x2DFF),
    (0x2E80, 0x2FFF),
    (0x3004, 0x3007),
    (0x3021, 0x302F),
    (0x3031, 0x303F),
    (0x3040, 0xD7FF),
    (0xF900, 0xFD3D),
    (0xFD40, 0xFDCF),
    (0xFDF0, 0xFE44),
    (0xFE47, 0xFFFD),
    (0x10000, 0x1FFFD),
    (0x20000, 0x2FFFD),
    (0x30000, 0x3FFFD),
    (0x40000, 0x4FFFD),
    (0x50000, 0x5FFFD),
    (0x60000, 0x6FFFD),
    (0x70000, 0x7FFFD),
    (0x80000, 0x8FFFD),
    (0x90000, 0x9FFFD),
    (0xA0000, 0xAFFFD),
    (0xB0000, 0xBFFFD),
    (0xC0000, 0xCFFFD),
    (0xD0000, 0xDFFFD),
    (0xE0000, 0xEFFFD),
];

/// N1570 D.2: the ranges of characters not allowed as the first
/// character of an identifier (combining marks).
const NOT_INITIAL: [(u32, u32); 4] = [
    (0x0300, 0x036F),
    (0x1DC0, 0x1DFF),
    (0x20D0, 0x20FF),
    (0xFE20, 0xFE2F),
];

fn in_ranges(ranges: &[(u32, u32)], code: u32) -> bool {
    ranges
        .iter()
        .any(|&(low, high)| (low..=high).contains(&code))
}

/// 6.4.3p2: a universal character name shall not designate a character
/// below U+00A0 other than `$`, `@` and `` ` ``, nor a surrogate code
/// point. A value above U+10FFFF designates no ISO/IEC 10646 character.
pub fn valid_ucn_value(code: u32) -> bool {
    let low_ok = matches!(code, 0x24 | 0x40 | 0x60);
    (code >= 0xA0 || low_ok) && !(0xD800..=0xDFFF).contains(&code) && code <= 0x10FFFF
}

/// Whether a non-basic character may appear in an identifier (D.1), and,
/// for the first character, is not one of D.2.
pub fn allowed_in_identifier(code: u32, initial: bool) -> bool {
    in_ranges(&ALLOWED, code) && !(initial && in_ranges(&NOT_INITIAL, code))
}

/// Why `code` cannot be in an identifier at this position, for a message.
pub fn identifier_error(code: u32, initial: bool, ucn: bool) -> String {
    let shown = describe(code);
    if ucn && !valid_ucn_value(code) {
        if code < 0xA0 {
            return format!("character {shown} cannot be written as a universal character name");
        }
        return format!("{shown} is not a valid universal character name");
    }
    if in_ranges(&ALLOWED, code) && initial {
        return format!("character {shown} is not allowed at the start of an identifier");
    }
    format!("character {shown} is not allowed in an identifier")
}

fn describe(code: u32) -> String {
    match char::from_u32(code) {
        Some(ch) if (0x20..0x7F).contains(&code) => format!("'{ch}' (U+{code:04X})"),
        _ => format!("U+{code:04X}"),
    }
}

/// The value of the universal character name at the start of `bytes`
/// (`\u` and four hex digits, or `\U` and eight), and its length, or
/// `None` when `bytes` does not start with one. Validity is not checked.
pub fn parse_ucn(bytes: &[u8]) -> Option<(u32, usize)> {
    let digits = match bytes {
        [b'\\', b'u', ..] => 4,
        [b'\\', b'U', ..] => 8,
        _ => return None,
    };
    let hex = bytes.get(2..2 + digits)?;
    let mut value: u32 = 0;
    for &byte in hex {
        value = value * 16 + (byte as char).to_digit(16)?;
    }
    Some((value, 2 + digits))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annex_d_ranges() {
        // Sorted and non-overlapping.
        for pair in ALLOWED.windows(2) {
            assert!(pair[0].1 < pair[1].0, "{pair:?}");
        }
        for code in [
            0xA8, 0xAA, 0xC0, 0xE9, 0xFF, 0x3B1, 0x4E2D, 0x1F600, 0xEFFFD,
        ] {
            assert!(allowed_in_identifier(code, true), "{code:X}");
        }
        for code in [
            0xA0, 0xA9, 0xD7, 0xF7, 0x1680, 0x180E, 0x2000, 0xD800, 0xFFFE, 0x1FFFE, 0xF0000,
        ] {
            assert!(!allowed_in_identifier(code, false), "{code:X}");
        }
        for code in [0x300, 0x36F, 0x1DC0, 0x20D0, 0xFE20, 0xFE2F] {
            assert!(!allowed_in_identifier(code, true), "{code:X}");
            assert!(allowed_in_identifier(code, false), "{code:X}");
        }
    }

    #[test]
    fn ucn_constraints() {
        for code in [0x24, 0x40, 0x60, 0xA0, 0xE9, 0xD7FF, 0xE000, 0x10FFFF] {
            assert!(valid_ucn_value(code), "{code:X}");
        }
        for code in [0, 0x41, 0x7F, 0x9F, 0xD800, 0xDFFF, 0x110000, u32::MAX] {
            assert!(!valid_ucn_value(code), "{code:X}");
        }
        assert_eq!(parse_ucn(b"\\u00e9x"), Some((0xE9, 6)));
        assert_eq!(parse_ucn(b"\\U0001F600"), Some((0x1F600, 10)));
        assert_eq!(parse_ucn(b"\\u12"), None);
        assert_eq!(parse_ucn(b"\\u12g4"), None);
        assert_eq!(parse_ucn(b"\\x41"), None);
        assert!(identifier_error(0x41, false, true).contains("cannot be written"));
        assert!(identifier_error(0x301, true, true).contains("start of an identifier"));
        assert!(identifier_error(0xD7, false, false).contains("not allowed in an identifier"));
        assert!(identifier_error(0xD800, false, true).contains("not a valid"));
    }
}
