//! Hand-written lexer for a C subset.
//!
//! The compiler runs the preprocessor ([`crate::pp`]) over a translation unit
//! first. This lexer is the phase-7 scan: it turns one preprocessing token's
//! spelling, or a raw buffer in the parser unit tests, into tokens. It still
//! rejects a `#` or a backslash-newline that reaches it. String literals are
//! decoded here; adjacent literals are concatenated by the parser.

use crate::ast::{FloatKind, TypeName};
use crate::diag::Diagnostic;
use crate::float::{inf_bits, parse_decimal, parse_exponent, parse_hex};
use crate::token::{keyword, Token, TokenKind};

/// One character of a character constant or string literal, before it is
/// encoded for the literal's prefix.
#[derive(Clone, Copy)]
enum SrcChar {
    /// A source character, a simple escape, or a universal character name:
    /// a code point, encoded as UTF-8, UTF-16, or UTF-32 as the prefix says.
    Code(u32),
    /// An octal escape: one code unit with this value.
    Octal(u64),
    /// A hexadecimal escape: one code unit with this value.
    Hex(u64),
}

pub fn lex(source: &str, filename: &str) -> Result<Vec<Token>, Diagnostic> {
    Lexer {
        source,
        filename,
        bytes: source.as_bytes(),
        index: 0,
        line: 1,
        column: 1,
        tokens: Vec::new(),
    }
    .run()
}

struct Lexer<'a> {
    source: &'a str,
    filename: &'a str,
    bytes: &'a [u8],
    index: usize,
    line: usize,
    column: usize,
    tokens: Vec<Token>,
}

impl Lexer<'_> {
    fn run(mut self) -> Result<Vec<Token>, Diagnostic> {
        while let Some(byte) = self.peek() {
            if byte >= 128 {
                return self.fail(
                    self.line,
                    self.column,
                    "non-ASCII characters are not supported",
                );
            }
            if is_whitespace(byte) {
                self.bump();
                continue;
            }
            let line = self.line;
            let column = self.column;
            if byte == b'/' && self.peek_at(1) == Some(b'/') {
                self.line_comment()?;
                continue;
            }
            if byte == b'/' && self.peek_at(1) == Some(b'*') {
                self.block_comment(line, column)?;
                continue;
            }
            if is_ident_start(byte) {
                self.ident(line, column)?;
                continue;
            }
            if byte.is_ascii_digit() {
                self.number(line, column)?;
                continue;
            }
            if byte == b'"' {
                self.string_literal(line, column)?;
                continue;
            }
            if byte == b'\'' {
                self.char_constant(line, column)?;
                continue;
            }
            if byte == b'#' {
                return self.fail(line, column, "preprocessor directives are not supported");
            }
            if byte == b'.' {
                self.dot(line, column)?;
                continue;
            }
            if self.three_char(line, column) {
                continue;
            }
            if self.two_char(line, column) {
                continue;
            }
            if self.one_char(byte, line, column) {
                continue;
            }
            return self.fail(
                line,
                column,
                format!("unexpected character '{}'", byte as char),
            );
        }
        self.tokens.push(Token {
            kind: TokenKind::Eof,
            spelling: String::new(),
            line: self.line,
            column: self.column,
            filename: self.filename.to_string(),
            value: None,
            float_hi: 0,
            int_ty: None,
            float_kind: None,
            bytes: None,
        });
        Ok(self.tokens)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.index).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.index + offset).copied()
    }

    fn bump(&mut self) -> u8 {
        let byte = self.bytes[self.index];
        self.index += 1;
        if byte == b'\r' {
            if self.peek() == Some(b'\n') {
                self.index += 1;
            }
            self.line += 1;
            self.column = 1;
        } else if byte == b'\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        byte
    }

    fn push(&mut self, kind: TokenKind, spelling: &str, line: usize, column: usize) {
        self.tokens.push(Token {
            kind,
            spelling: spelling.to_string(),
            line,
            column,
            filename: self.filename.to_string(),
            value: None,
            float_hi: 0,
            int_ty: None,
            float_kind: None,
            bytes: None,
        });
    }

    fn fail<T>(
        &self,
        line: usize,
        column: usize,
        message: impl Into<String>,
    ) -> Result<T, Diagnostic> {
        Err(Diagnostic::new(self.filename, line, column, message))
    }

    fn line_comment(&mut self) -> Result<(), Diagnostic> {
        self.bump();
        self.bump();
        while let Some(byte) = self.peek() {
            if byte == b'\\' && matches!(self.peek_at(1), Some(b'\n' | b'\r')) {
                return self.fail(
                    self.line,
                    self.column,
                    "backslash line splicing is not supported",
                );
            }
            if byte == b'\n' || byte == b'\r' {
                break;
            }
            self.bump();
        }
        Ok(())
    }

    fn block_comment(&mut self, line: usize, column: usize) -> Result<(), Diagnostic> {
        self.bump();
        self.bump();
        while self.peek().is_some() {
            if self.peek() == Some(b'*') && self.peek_at(1) == Some(b'/') {
                self.bump();
                self.bump();
                return Ok(());
            }
            if self.peek() == Some(b'\\') && matches!(self.peek_at(1), Some(b'\n' | b'\r')) {
                return self.fail(
                    self.line,
                    self.column,
                    "backslash line splicing is not supported",
                );
            }
            self.bump();
        }
        self.fail(line, column, "unterminated block comment")
    }

    fn ident(&mut self, line: usize, column: usize) -> Result<(), Diagnostic> {
        let start = self.index;
        while self.peek().is_some_and(is_ident_cont) {
            self.bump();
        }
        let spelling = &self.source[start..self.index];
        // Encoding prefixes (C11 6.4.4.4, 6.4.5). The prefix is not a
        // separate token: it is recognized only when the quote is the next
        // byte. `u8` names a character constant only in C23, so that form
        // is a diagnostic. `wchar_t` is `int`. `char16_t` is the unsigned
        // 16-bit type and `char32_t` is the unsigned 32-bit type.
        if matches!(spelling, "L" | "u" | "U" | "u8") && matches!(self.peek(), Some(b'\'' | b'"')) {
            let quote = self.peek();
            if spelling == "u8" && quote == Some(b'\'') {
                return self.fail(line, column, "u8 character constants are not supported");
            }
            let bits: u32 = match spelling {
                "u" => 16,
                "u8" => 8,
                _ => 32,
            };
            return if quote == Some(b'\'') {
                self.prefixed_char(start, line, column, bits, spelling == "L")
            } else {
                self.prefixed_string(start, line, column, bits)
            };
        }
        let kind = keyword(spelling).unwrap_or(TokenKind::Ident);
        self.push(kind, spelling, line, column);
        Ok(())
    }

    fn number(&mut self, line: usize, column: usize) -> Result<(), Diagnostic> {
        let start = self.index;
        if self.peek() == Some(b'0') && matches!(self.peek_at(1), Some(b'x' | b'X')) {
            return self.hex_constant(start, line, column);
        }
        if self.peek() == Some(b'0') && matches!(self.peek_at(1), Some(b'b' | b'B')) {
            return self.fail(line, column, "binary constants are not supported");
        }
        let digit_start = self.index;
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.bump();
        }
        // `1.5`, `1.`, and `1e2` are floating constants. The exponent marker
        // is part of the constant, so `1e` is not an integer followed by an
        // identifier. A leading zero does not make this octal.
        if matches!(self.peek(), Some(b'.' | b'e' | b'E')) {
            return self.finish_decimal_float(start, line, column);
        }
        let digit_end = self.index;
        if self.source.as_bytes()[digit_start] == b'0' {
            let digits = &self.source[digit_start..digit_end];
            if digits.bytes().any(|byte| byte == b'8' || byte == b'9') {
                return self.fail(line, column, "invalid octal constant");
            }
        }
        let radix = if self.source.as_bytes()[digit_start] == b'0' {
            8
        } else {
            10
        };
        let suffix = self.integer_suffix(line, column)?;
        self.reject_number_tail(line, column)?;
        let spelling = self.source[start..self.index].to_string();
        let digits = self.source[digit_start..digit_end].to_string();
        let value = self.int_value(&digits, radix, line, column)?;
        let ty = self.literal_type(value, suffix, radix != 10, line, column)?;
        self.push_int(&spelling, value as u64, ty, line, column);
        Ok(())
    }

    fn hex_constant(&mut self, start: usize, line: usize, column: usize) -> Result<(), Diagnostic> {
        self.bump();
        self.bump();
        let digit_start = self.index;
        while self.peek().is_some_and(|b| b.is_ascii_hexdigit()) {
            self.bump();
        }
        let digit_end = self.index;
        // `p`/`P` is the binary exponent. A `.` starts the fraction. Neither
        // is part of a hexadecimal integer (`0x1p4` is the double 16).
        if matches!(self.peek(), Some(b'.' | b'p' | b'P')) {
            return self.finish_hex_float(start, digit_start, digit_end, line, column);
        }
        if digit_start == digit_end {
            return self.fail(line, column, "invalid hexadecimal constant");
        }
        let suffix = self.integer_suffix(line, column)?;
        self.reject_number_tail(line, column)?;
        let spelling = self.source[start..self.index].to_string();
        let digits = self.source[digit_start..digit_end].to_string();
        let value = self.int_value(&digits, 16, line, column)?;
        let ty = self.literal_type(value, suffix, true, line, column)?;
        self.push_int(&spelling, value as u64, ty, line, column);
        Ok(())
    }

    /// C11 6.4.4.2. The significand digits are already consumed. A `.` and an
    /// exponent may still follow.
    fn finish_decimal_float(
        &mut self,
        start: usize,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        if self.peek() == Some(b'.') {
            self.bump();
            while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                self.bump();
            }
        }
        self.finish_float_exponent(start, line, column)
    }

    fn finish_float_exponent(
        &mut self,
        start: usize,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.bump();
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.bump();
            }
            let exp_start = self.index;
            while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                self.bump();
            }
            if self.index == exp_start {
                return self.fail(line, column, "invalid floating constant");
            }
        }
        let kind = self.float_suffix(line, column)?;
        let spelling = self.source[start..self.index].to_string();
        let body_len = if kind == FloatKind::Double {
            spelling.len()
        } else {
            spelling.len() - 1
        };
        let body = &spelling[..body_len];
        let Some(bits) = parse_decimal(body, kind) else {
            return self.fail(line, column, "invalid floating constant");
        };
        self.push_float(&spelling, bits, kind, line, column);
        Ok(())
    }

    fn finish_hex_float(
        &mut self,
        start: usize,
        int_start: usize,
        int_end: usize,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        let int_digits = self.source[int_start..int_end].to_string();
        let frac_digits = if self.peek() == Some(b'.') {
            self.bump();
            let frac_start = self.index;
            while self.peek().is_some_and(|b| b.is_ascii_hexdigit()) {
                self.bump();
            }
            self.source[frac_start..self.index].to_string()
        } else {
            String::new()
        };
        if int_digits.is_empty() && frac_digits.is_empty() {
            return self.fail(line, column, "invalid hexadecimal floating constant");
        }
        if !matches!(self.peek(), Some(b'p' | b'P')) {
            return self.fail(
                line,
                column,
                "hexadecimal floating constant requires an exponent",
            );
        }
        self.bump();
        let neg = match self.peek() {
            Some(b'+') => {
                self.bump();
                false
            }
            Some(b'-') => {
                self.bump();
                true
            }
            _ => false,
        };
        let exp_start = self.index;
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.bump();
        }
        if self.index == exp_start {
            return self.fail(line, column, "invalid hexadecimal floating constant");
        }
        let exp_digits = self.source[exp_start..self.index].to_string();
        let kind = self.float_suffix(line, column)?;
        let spelling = self.source[start..self.index].to_string();
        let zero_sig = int_digits
            .bytes()
            .chain(frac_digits.bytes())
            .all(|byte| byte == b'0');
        let bits = if zero_sig {
            0
        } else {
            match parse_exponent(&exp_digits, neg) {
                Ok(exp) => parse_hex(&int_digits, &frac_digits, exp, kind).unwrap_or(0),
                Err(true) => 0,
                Err(false) => {
                    if kind == FloatKind::LongDouble {
                        0x7FFF_8000_0000_0000_0000
                    } else {
                        u128::from(inf_bits(kind == FloatKind::Float))
                    }
                }
            }
        };
        self.push_float(&spelling, bits, kind, line, column);
        Ok(())
    }

    /// `f`/`F` selects `float`. No suffix selects `double`. `l`/`L` selects
    /// `long double`, the 80-bit extended format.
    fn float_suffix(&mut self, line: usize, column: usize) -> Result<FloatKind, Diagnostic> {
        match self.peek() {
            Some(b'f' | b'F') => {
                self.bump();
                if self.float_tail() {
                    return self.fail(line, column, "invalid floating constant");
                }
                Ok(FloatKind::Float)
            }
            Some(b'l' | b'L') => {
                self.bump();
                if self.float_tail() {
                    return self.fail(line, column, "invalid floating constant");
                }
                Ok(FloatKind::LongDouble)
            }
            Some(_) if self.float_tail() => self.fail(line, column, "invalid floating constant"),
            _ => Ok(FloatKind::Double),
        }
    }

    fn float_tail(&self) -> bool {
        self.peek()
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'.')
    }

    fn push_float(
        &mut self,
        spelling: &str,
        bits: u128,
        kind: FloatKind,
        line: usize,
        column: usize,
    ) {
        self.tokens.push(Token {
            kind: TokenKind::Float,
            spelling: spelling.to_string(),
            line,
            column,
            filename: self.filename.to_string(),
            value: Some(bits as u64),
            float_hi: (bits >> 64) as u16,
            int_ty: None,
            float_kind: Some(kind),
            bytes: None,
        });
    }

    /// C11 6.4.4.1. The type is the first in the list for this suffix and
    /// base that can represent the value. `long` and `long long` are both 64
    /// bits on this target, so a value that fits in `long` is `long` rather
    /// than `long long` unless the suffix asks for `long long`.
    fn literal_type(
        &self,
        value: u128,
        suffix: Suffix,
        hex_or_octal: bool,
        line: usize,
        column: usize,
    ) -> Result<TypeName, Diagnostic> {
        let list: &[TypeName] = match (suffix, hex_or_octal) {
            (Suffix::None, false) => &[TypeName::Int, TypeName::Long, TypeName::LongLong],
            (Suffix::None, true) => &[
                TypeName::Int,
                TypeName::UInt,
                TypeName::Long,
                TypeName::ULong,
                TypeName::LongLong,
                TypeName::ULongLong,
            ],
            (Suffix::U, _) => &[TypeName::UInt, TypeName::ULong, TypeName::ULongLong],
            (Suffix::L, false) => &[TypeName::Long, TypeName::LongLong],
            (Suffix::L, true) => &[
                TypeName::Long,
                TypeName::ULong,
                TypeName::LongLong,
                TypeName::ULongLong,
            ],
            (Suffix::Ul, _) => &[TypeName::ULong, TypeName::ULongLong],
            (Suffix::Ll, false) => &[TypeName::LongLong],
            (Suffix::Ll, true) => &[TypeName::LongLong, TypeName::ULongLong],
            (Suffix::Ull, _) => &[TypeName::ULongLong],
        };
        for ty in list {
            if literal_fits(value, *ty) {
                return Ok(*ty);
            }
        }
        self.fail(line, column, "integer constant is too large")
    }

    fn integer_suffix(&mut self, line: usize, column: usize) -> Result<Suffix, Diagnostic> {
        let start = self.index;
        while matches!(self.peek(), Some(b'u' | b'U' | b'l' | b'L')) {
            self.bump();
        }
        if self.index == start {
            return Ok(Suffix::None);
        }
        let text = self.source[start..self.index].to_string();
        match text.as_str() {
            "u" | "U" => Ok(Suffix::U),
            "l" | "L" => Ok(Suffix::L),
            "ll" | "LL" => Ok(Suffix::Ll),
            "ul" | "uL" | "Ul" | "UL" | "lu" | "Lu" | "lU" | "LU" => Ok(Suffix::Ul),
            "ull" | "uLL" | "Ull" | "ULL" | "llu" | "llU" | "LLu" | "LLU" => Ok(Suffix::Ull),
            _ => self.fail(line, column, "invalid integer suffix"),
        }
    }

    /// Parse a digit sequence that the scanner has already checked.
    fn int_value(
        &self,
        digits: &str,
        radix: u32,
        line: usize,
        column: usize,
    ) -> Result<u128, Diagnostic> {
        let mut value: u128 = 0;
        for byte in digits.bytes() {
            let digit = match byte {
                b'0'..=b'9' => u32::from(byte - b'0'),
                b'a'..=b'f' => u32::from(byte - b'a') + 10,
                b'A'..=b'F' => u32::from(byte - b'A') + 10,
                _ => return self.fail(line, column, "invalid numeric literal"),
            };
            if digit >= radix {
                return self.fail(line, column, "invalid numeric literal");
            }
            value = match value
                .checked_mul(u128::from(radix))
                .and_then(|next| next.checked_add(u128::from(digit)))
            {
                Some(next) => next,
                None => return self.fail(line, column, "integer constant is too large"),
            };
        }
        Ok(value)
    }

    fn push_int(&mut self, spelling: &str, bits: u64, ty: TypeName, line: usize, column: usize) {
        self.tokens.push(Token {
            kind: TokenKind::Int,
            spelling: spelling.to_string(),
            line,
            column,
            filename: self.filename.to_string(),
            value: Some(bits),
            float_hi: 0,
            int_ty: Some(ty),
            float_kind: None,
            bytes: None,
        });
    }

    fn reject_number_tail(&self, line: usize, column: usize) -> Result<(), Diagnostic> {
        match self.peek() {
            Some(b) if b.is_ascii_alphabetic() || b == b'_' || b == b'.' => {
                self.fail(line, column, "invalid numeric literal")
            }
            _ => Ok(()),
        }
    }

    /// An ordinary string literal (C11 6.4.5). Escapes are the same set as
    /// character constants. A numeric escape must fit in `unsigned char`. A
    /// source character outside ASCII, or a universal character name, is
    /// stored as its UTF-8 bytes, the execution character set on this target.
    /// The token's bytes do not include the implicit terminating NUL; the type
    /// `char[N]` adds it.
    fn string_literal(&mut self, line: usize, column: usize) -> Result<(), Diagnostic> {
        let start = self.index;
        self.bump();
        let bytes = self.string_body(line, column, 8)?;
        let spelling = self.source[start..self.index].to_string();
        self.push_string(spelling, bytes, line, column);
        Ok(())
    }

    fn push_string(&mut self, spelling: String, bytes: Vec<u8>, line: usize, column: usize) {
        self.tokens.push(Token {
            kind: TokenKind::String,
            spelling,
            line,
            column,
            filename: self.filename.to_string(),
            value: None,
            float_hi: 0,
            int_ty: None,
            float_kind: None,
            bytes: Some(bytes),
        });
    }

    /// The characters of a string literal after the opening quote, encoded
    /// in code units of `bits` bits, little-endian. 8 is UTF-8 (`""` and
    /// `u8""`), 16 is UTF-16 (`u""`), and 32 is UTF-32 (`U""` and `L""`;
    /// `wchar_t` holds ISO 10646 code points on this target).
    fn string_body(
        &mut self,
        line: usize,
        column: usize,
        bits: u32,
    ) -> Result<Vec<u8>, Diagnostic> {
        let mut bytes = Vec::new();
        loop {
            match self.peek() {
                None | Some(b'\n' | b'\r') => {
                    return self.fail(line, column, "unterminated string literal");
                }
                Some(b'"') => {
                    self.bump();
                    return Ok(bytes);
                }
                _ => {
                    let (at_line, at_column) = (self.line, self.column);
                    let ch = self.read_source_char(line, column, true)?;
                    for unit in self.encode_units(ch, bits, at_line, at_column, true)? {
                        match bits {
                            8 => bytes.push(unit as u8),
                            16 => bytes.extend_from_slice(&(unit as u16).to_le_bytes()),
                            _ => bytes.extend_from_slice(&unit.to_le_bytes()),
                        }
                    }
                }
            }
        }
    }

    /// Code units for one character of a literal whose units are `bits`
    /// wide. A numeric escape is one unit and must fit. A character, or a
    /// universal character name, is encoded; in a character constant it must
    /// be a single unit (C11 6.4.4.4p4 and p9 constrain the escapes; a
    /// character that needs several `char`s or `char16_t`s is diagnosed the
    /// same way, as there is no multi-unit character constant here).
    fn encode_units(
        &self,
        ch: SrcChar,
        bits: u32,
        line: usize,
        column: usize,
        string: bool,
    ) -> Result<Vec<u32>, Diagnostic> {
        let limit = if bits >= 32 {
            u64::from(u32::MAX)
        } else {
            (1u64 << bits) - 1
        };
        let units = match ch {
            SrcChar::Octal(value) | SrcChar::Hex(value) => {
                if value > limit {
                    let what = if matches!(ch, SrcChar::Octal(_)) {
                        "octal escape sequence out of range"
                    } else {
                        "hex escape sequence out of range"
                    };
                    return self.fail(line, column, what);
                }
                vec![value as u32]
            }
            SrcChar::Code(code) => match bits {
                8 => {
                    let ch = char::from_u32(code).expect("validated code point");
                    let mut buf = [0u8; 4];
                    ch.encode_utf8(&mut buf)
                        .bytes()
                        .map(u32::from)
                        .collect::<Vec<_>>()
                }
                16 => {
                    let ch = char::from_u32(code).expect("validated code point");
                    let mut buf = [0u16; 2];
                    ch.encode_utf16(&mut buf)
                        .iter()
                        .map(|unit| u32::from(*unit))
                        .collect::<Vec<_>>()
                }
                _ => vec![code],
            },
        };
        if !string && units.len() != 1 {
            return self.fail(
                line,
                column,
                "character too large for enclosing character literal type",
            );
        }
        Ok(units)
    }

    /// A prefixed character constant (C11 6.4.4.4).
    ///
    /// `bits` is 16 for `u` (`char16_t`, unsigned short) and 32 for `U`
    /// (`char32_t`, unsigned int) and `L` (`wchar_t`, which is `int`).
    /// `wchar` selects that signed `int` result. A hex or octal escape that
    /// does not fit in the unsigned type of that width is a diagnostic.
    /// `L` then converts the `unsigned int` value to `int`. A character is
    /// its UTF-16 or UTF-32 code unit; one that needs a surrogate pair does
    /// not fit in `char16_t`.
    fn prefixed_char(
        &mut self,
        start: usize,
        line: usize,
        column: usize,
        bits: u32,
        wchar: bool,
    ) -> Result<(), Diagnostic> {
        self.bump();
        let value = self.char_body(line, column, bits)?;
        let spelling = self.source[start..self.index].to_string();
        let ty = if wchar {
            TypeName::Int
        } else if bits == 16 {
            TypeName::UShort
        } else {
            TypeName::UInt
        };
        self.push_int(&spelling, u64::from(value), ty, line, column);
        Ok(())
    }

    /// The one character of a character constant after the opening quote,
    /// and the closing quote, as a code unit of `bits` bits.
    fn char_body(&mut self, line: usize, column: usize, bits: u32) -> Result<u32, Diagnostic> {
        if matches!(self.peek(), None | Some(b'\n' | b'\r')) {
            return self.fail(line, column, "unterminated character constant");
        }
        if self.peek() == Some(b'\'') {
            return self.fail(line, column, "empty character constant");
        }
        let (at_line, at_column) = (self.line, self.column);
        let ch = self.read_source_char(line, column, false)?;
        if self.peek() != Some(b'\'') {
            if matches!(self.peek(), None | Some(b'\n' | b'\r')) {
                return self.fail(line, column, "unterminated character constant");
            }
            return self.fail(
                line,
                column,
                "multi-character character constants are not supported",
            );
        }
        let units = self.encode_units(ch, bits, at_line, at_column, false)?;
        self.bump();
        Ok(units[0])
    }

    /// A prefixed string literal. `bits` is 8 for `u8` (an array of `char`),
    /// 16 for `u`, and 32 for `U` and `L`. Each code unit is stored
    /// little-endian. The token does not include the terminating NUL.
    fn prefixed_string(
        &mut self,
        start: usize,
        line: usize,
        column: usize,
        bits: u32,
    ) -> Result<(), Diagnostic> {
        self.bump();
        let bytes = self.string_body(line, column, bits)?;
        let spelling = self.source[start..self.index].to_string();
        self.push_string(spelling, bytes, line, column);
        Ok(())
    }

    /// An integer character constant has type `int` (C11 6.4.4.4).
    /// One source character or one escape is accepted. A hex or octal escape
    /// must fit in `unsigned char`; Clang rejects a wider escape on this target.
    /// The value is that byte converted from `char` to `int`. `char` is signed,
    /// so 128..255 are negative. A character outside ASCII needs more than one
    /// UTF-8 byte and is diagnosed.
    fn char_constant(&mut self, line: usize, column: usize) -> Result<(), Diagnostic> {
        let start = self.index;
        self.bump();
        let value = self.char_body(line, column, 8)?;
        let spelling = self.source[start..self.index].to_string();
        let bits = crate::ast::sign_extend(u64::from(value), 8) as u32 as u64;
        self.push_int(&spelling, bits, TypeName::Int, line, column);
        Ok(())
    }

    /// One source character or one escape (C11 6.4.4.4, 6.4.3).
    ///
    /// `string` selects the diagnostic for a missing closer. The escape set is
    /// shared by character constants and string literals. A multibyte source
    /// character is decoded from UTF-8, the source character set here.
    fn read_source_char(
        &mut self,
        line: usize,
        column: usize,
        string: bool,
    ) -> Result<SrcChar, Diagnostic> {
        let unterminated = if string {
            "unterminated string literal"
        } else {
            "unterminated character constant"
        };
        let Some(byte) = self.peek() else {
            return self.fail(line, column, unterminated);
        };
        if byte >= 128 {
            let ch = self.source[self.index..]
                .chars()
                .next()
                .expect("UTF-8 source");
            for _ in 0..ch.len_utf8() {
                self.bump();
            }
            return Ok(SrcChar::Code(u32::from(ch)));
        }
        if byte != b'\\' {
            self.bump();
            return Ok(SrcChar::Code(u32::from(byte)));
        }
        let esc_line = self.line;
        let esc_column = self.column;
        self.bump();
        let Some(next) = self.peek() else {
            return self.fail(line, column, unterminated);
        };
        if next == b'\n' || next == b'\r' {
            return self.fail(
                esc_line,
                esc_column,
                "backslash line splicing is not supported",
            );
        }
        if next >= 128 {
            return self.fail(esc_line, esc_column, "unknown escape sequence");
        }
        self.bump();
        let simple = match next {
            b'a' => 7,
            b'b' => 8,
            b'f' => 12,
            b'n' => 10,
            b'r' => 13,
            b't' => 9,
            b'v' => 11,
            b'\\' | b'\'' | b'"' | b'?' => u32::from(next),
            b'0'..=b'7' => return Ok(SrcChar::Octal(self.octal_digits(next))),
            b'x' => return self.hex_digits(esc_line, esc_column).map(SrcChar::Hex),
            b'u' => return self.universal_name(4, esc_line, esc_column),
            b'U' => return self.universal_name(8, esc_line, esc_column),
            _ => return self.fail(esc_line, esc_column, "unknown escape sequence"),
        };
        Ok(SrcChar::Code(simple))
    }

    /// At most three octal digits (C11 6.4.4.4p1), so the value is below 512.
    fn octal_digits(&mut self, first: u8) -> u64 {
        let mut value = u64::from(first - b'0');
        for _ in 0..2 {
            match self.peek() {
                Some(byte @ b'0'..=b'7') => {
                    self.bump();
                    value = value * 8 + u64::from(byte - b'0');
                }
                _ => break,
            }
        }
        value
    }

    /// Any number of hex digits. A value beyond 32 bits fits no code unit.
    fn hex_digits(&mut self, line: usize, column: usize) -> Result<u64, Diagnostic> {
        let mut value: u64 = 0;
        let mut digits = 0u32;
        while let Some(byte) = self.peek() {
            let Some(digit) = (byte as char).to_digit(16) else {
                break;
            };
            self.bump();
            value = value.saturating_mul(16).saturating_add(u64::from(digit));
            digits += 1;
        }
        if digits == 0 {
            return self.fail(line, column, "invalid hexadecimal escape");
        }
        if value > u64::from(u32::MAX) {
            return self.fail(line, column, "hex escape sequence out of range");
        }
        Ok(value)
    }

    /// `\uXXXX` or `\UXXXXXXXX` (C11 6.4.3). The value is an ISO 10646
    /// code point. It shall not be below U+00A0 other than U+0024, U+0040,
    /// and U+0060, and shall not be a surrogate (D800 through DFFF). A value
    /// above U+10FFFF names no character.
    fn universal_name(
        &mut self,
        digits: u32,
        line: usize,
        column: usize,
    ) -> Result<SrcChar, Diagnostic> {
        let mut value = 0u32;
        for _ in 0..digits {
            let Some(digit) = self.peek().and_then(|byte| (byte as char).to_digit(16)) else {
                return self.fail(line, column, "incomplete universal character name");
            };
            self.bump();
            value = value * 16 + digit;
        }
        let allowed_low = matches!(value, 0x24 | 0x40 | 0x60);
        if (value < 0xA0 && !allowed_low) || (0xD800..=0xDFFF).contains(&value) || value > 0x10FFFF
        {
            return self.fail(line, column, "invalid universal character name");
        }
        Ok(SrcChar::Code(value))
    }

    fn dot(&mut self, line: usize, column: usize) -> Result<(), Diagnostic> {
        if self.peek_at(1) == Some(b'.') && self.peek_at(2) == Some(b'.') {
            self.bump();
            self.bump();
            self.bump();
            self.push(TokenKind::Ellipsis, "...", line, column);
            return Ok(());
        }
        if self.peek_at(1).is_some_and(|b| b.is_ascii_digit()) {
            let start = self.index;
            self.bump();
            while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                self.bump();
            }
            return self.finish_float_exponent(start, line, column);
        }
        self.bump();
        self.push(TokenKind::Dot, ".", line, column);
        Ok(())
    }

    /// `<<=` and `>>=` must be recognized before `<<` and `>>`.
    fn three_char(&mut self, line: usize, column: usize) -> bool {
        let Some(triple) = self.three_bytes() else {
            return false;
        };
        let (kind, spelling) = match &triple {
            b"<<=" => (TokenKind::ShlEq, "<<="),
            b">>=" => (TokenKind::ShrEq, ">>="),
            _ => return false,
        };
        self.bump();
        self.bump();
        self.bump();
        self.push(kind, spelling, line, column);
        true
    }

    fn three_bytes(&self) -> Option<[u8; 3]> {
        Some([self.peek()?, self.peek_at(1)?, self.peek_at(2)?])
    }

    fn two_char(&mut self, line: usize, column: usize) -> bool {
        let Some(pair) = self.two_bytes() else {
            return false;
        };
        let (kind, spelling) = match &pair {
            b"==" => (TokenKind::Eq, "=="),
            b"!=" => (TokenKind::Ne, "!="),
            b"<=" => (TokenKind::Le, "<="),
            b">=" => (TokenKind::Ge, ">="),
            b"&&" => (TokenKind::AndAnd, "&&"),
            b"||" => (TokenKind::OrOr, "||"),
            b"++" => (TokenKind::PlusPlus, "++"),
            b"--" => (TokenKind::MinusMinus, "--"),
            b"<<" => (TokenKind::Shl, "<<"),
            b">>" => (TokenKind::Shr, ">>"),
            b"+=" => (TokenKind::PlusEq, "+="),
            b"-=" => (TokenKind::MinusEq, "-="),
            b"*=" => (TokenKind::StarEq, "*="),
            b"/=" => (TokenKind::SlashEq, "/="),
            b"%=" => (TokenKind::PercentEq, "%="),
            b"&=" => (TokenKind::AmpEq, "&="),
            b"|=" => (TokenKind::PipeEq, "|="),
            b"^=" => (TokenKind::CaretEq, "^="),
            b"->" => (TokenKind::Arrow, "->"),
            _ => return false,
        };
        self.bump();
        self.bump();
        self.push(kind, spelling, line, column);
        true
    }

    fn two_bytes(&self) -> Option<[u8; 2]> {
        Some([self.peek()?, self.peek_at(1)?])
    }

    fn one_char(&mut self, byte: u8, line: usize, column: usize) -> bool {
        let (kind, spelling) = match byte {
            b'+' => (TokenKind::Plus, "+"),
            b'-' => (TokenKind::Minus, "-"),
            b'*' => (TokenKind::Star, "*"),
            b'/' => (TokenKind::Slash, "/"),
            b'%' => (TokenKind::Percent, "%"),
            b'=' => (TokenKind::Assign, "="),
            b'<' => (TokenKind::Lt, "<"),
            b'>' => (TokenKind::Gt, ">"),
            b'!' => (TokenKind::Bang, "!"),
            b'&' => (TokenKind::Amp, "&"),
            b'|' => (TokenKind::Pipe, "|"),
            b'^' => (TokenKind::Caret, "^"),
            b'~' => (TokenKind::Tilde, "~"),
            b'(' => (TokenKind::LParen, "("),
            b')' => (TokenKind::RParen, ")"),
            b'{' => (TokenKind::LBrace, "{"),
            b'}' => (TokenKind::RBrace, "}"),
            b'[' => (TokenKind::LBrack, "["),
            b']' => (TokenKind::RBrack, "]"),
            b';' => (TokenKind::Semi, ";"),
            b',' => (TokenKind::Comma, ","),
            b'?' => (TokenKind::Question, "?"),
            b':' => (TokenKind::Colon, ":"),
            _ => return false,
        };
        self.bump();
        self.push(kind, spelling, line, column);
        true
    }
}

#[derive(Clone, Copy)]
enum Suffix {
    None,
    U,
    L,
    Ll,
    Ul,
    Ull,
}

fn literal_fits(value: u128, ty: TypeName) -> bool {
    let width = ty.width();
    if ty.is_signed() {
        let max = (1u128 << (width - 1)) - 1;
        value <= max
    } else if width >= 64 {
        value <= u128::from(u64::MAX)
    } else {
        value < (1u128 << width)
    }
}

fn is_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
}

fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_ident_cont(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(source: &str) -> Vec<Token> {
        let mut toks = lex(source, "t.c").unwrap();
        assert_eq!(toks.pop().unwrap().kind, TokenKind::Eof);
        toks
    }

    fn spellings(source: &str) -> Vec<String> {
        tokens(source).into_iter().map(|t| t.spelling).collect()
    }

    fn kinds(source: &str) -> Vec<TokenKind> {
        tokens(source).into_iter().map(|t| t.kind).collect()
    }

    fn err(source: &str) -> String {
        lex(source, "t.c").unwrap_err().to_string()
    }

    #[test]
    fn return_constant_tokens() {
        assert_eq!(
            kinds("int main(void) { return 42; }"),
            vec![
                TokenKind::KwInt,
                TokenKind::Ident,
                TokenKind::LParen,
                TokenKind::KwVoid,
                TokenKind::RParen,
                TokenKind::LBrace,
                TokenKind::KwReturn,
                TokenKind::Int,
                TokenKind::Semi,
                TokenKind::RBrace,
            ]
        );
        assert_eq!(tokens("return 42;")[1].value, Some(42));
    }

    #[test]
    fn keywords_are_not_identifiers() {
        assert_eq!(
            kinds("int integer _Bool"),
            vec![TokenKind::KwInt, TokenKind::Ident, TokenKind::KwBool,]
        );
        assert_eq!(spellings("int integer"), vec!["int", "integer"]);
    }

    #[test]
    fn comments_are_skipped_and_keep_locations() {
        let toks = tokens("int /*\n*/ x // hidden\n+");
        assert_eq!(
            spellings("int /*\n*/ x // hidden\n+"),
            vec!["int", "x", "+"]
        );
        assert_eq!(toks[1].line, 2);
        assert_eq!(toks[1].column, 4);
        assert_eq!(toks[2].line, 3);
        assert_eq!(toks[2].column, 1);
    }

    #[test]
    fn hash_inside_comment_is_not_a_directive() {
        assert_eq!(spellings("// #include <stdio.h>\nint x"), vec!["int", "x"]);
    }

    #[test]
    fn multi_character_operators() {
        assert_eq!(
            kinds("== != <= >= && || ++ -- << >> += -= *= /= %= &= |= ^= -> <<= >>="),
            vec![
                TokenKind::Eq,
                TokenKind::Ne,
                TokenKind::Le,
                TokenKind::Ge,
                TokenKind::AndAnd,
                TokenKind::OrOr,
                TokenKind::PlusPlus,
                TokenKind::MinusMinus,
                TokenKind::Shl,
                TokenKind::Shr,
                TokenKind::PlusEq,
                TokenKind::MinusEq,
                TokenKind::StarEq,
                TokenKind::SlashEq,
                TokenKind::PercentEq,
                TokenKind::AmpEq,
                TokenKind::PipeEq,
                TokenKind::CaretEq,
                TokenKind::Arrow,
                TokenKind::ShlEq,
                TokenKind::ShrEq,
            ]
        );
        assert_eq!(
            kinds("a << = 1 >> = 2"),
            vec![
                TokenKind::Ident,
                TokenKind::Shl,
                TokenKind::Assign,
                TokenKind::Int,
                TokenKind::Shr,
                TokenKind::Assign,
                TokenKind::Int,
            ]
        );
    }

    #[test]
    fn integer_bases_and_range() {
        assert_eq!(tokens("0").pop().unwrap().value, Some(0));
        assert_eq!(tokens("010").pop().unwrap().value, Some(8));
        assert_eq!(tokens("0x2A").pop().unwrap().value, Some(42));
        assert_eq!(tokens("0X10").pop().unwrap().value, Some(16));
        assert_eq!(tokens("2147483647").pop().unwrap().value, Some(2147483647));
        let big = tokens("2147483648");
        assert_eq!(big[0].value, Some(2147483648));
        assert_eq!(big[0].int_ty, Some(crate::ast::TypeName::Long));
        let hex = tokens("0x80000000");
        assert_eq!(hex[0].value, Some(0x8000_0000));
        assert_eq!(hex[0].int_ty, Some(crate::ast::TypeName::UInt));
        assert_eq!(tokens("1u")[0].int_ty, Some(crate::ast::TypeName::UInt));
        assert_eq!(
            tokens("1ull")[0].int_ty,
            Some(crate::ast::TypeName::ULongLong)
        );
        assert_eq!(
            tokens("1LL")[0].int_ty,
            Some(crate::ast::TypeName::LongLong)
        );
        assert!(err("99999999999999999999").contains("too large"));
        assert!(err("0x1FFFFFFFFFFFFFFFF").contains("too large"));
        let octal_max = tokens("01777777777777777777777");
        assert_eq!(octal_max[0].value, Some(u64::MAX));
        assert_eq!(octal_max[0].int_ty, Some(crate::ast::TypeName::ULong));
        assert!(err("017777777777777777777777").contains("too large"));
        assert!(err("9223372036854775808").contains("too large"));
        assert!(err("08").contains("invalid octal"));
        assert!(err("0x").contains("invalid hexadecimal"));
        assert!(err("0b1").contains("binary constants"));
        assert!(err("1Ll").contains("invalid integer suffix"));
        assert!(err("1uu").contains("invalid integer suffix"));
        assert!(err("123abc").contains("invalid numeric literal"));
    }

    #[test]
    fn floating_constants() {
        use crate::ast::FloatKind;
        let one = tokens("1.5");
        assert_eq!(one[0].kind, TokenKind::Float);
        assert_eq!(one[0].float_kind, Some(FloatKind::Double));
        assert_eq!(one[0].value, Some(0x3ff8_0000_0000_0000));
        assert_eq!(tokens(".5")[0].value, Some(0x3fe0_0000_0000_0000));
        assert_eq!(tokens("1.")[0].value, Some(0x3ff0_0000_0000_0000));
        assert_eq!(tokens("1e2")[0].value, Some(0x4059_0000_0000_0000));
        assert_eq!(tokens("1.5f")[0].float_kind, Some(FloatKind::Float));
        assert_eq!(tokens("1.5f")[0].value, Some(0x3fc0_0000));
        assert_eq!(tokens("1.5F")[0].float_kind, Some(FloatKind::Float));
        assert_eq!(tokens("0x1.8p1")[0].value, Some(0x4008_0000_0000_0000));
        assert_eq!(tokens("0x.8p0")[0].value, Some(0x3fe0_0000_0000_0000));
        assert_eq!(tokens("0x1p-1f")[0].float_kind, Some(FloatKind::Float));
        assert_eq!(
            tokens("0x1p0000000001")[0].value,
            Some(0x4000_0000_0000_0000)
        );
        assert_eq!(tokens("0x0p1000000000")[0].value, Some(0));
        assert_eq!(tokens("08.5")[0].kind, TokenKind::Float);
        assert_eq!(tokens("1.0L")[0].float_kind, Some(FloatKind::LongDouble));
        assert_eq!(tokens("1.0l")[0].float_kind, Some(FloatKind::LongDouble));
        assert!(err("1.0LL").contains("invalid floating constant"));
        assert!(err("1.ex").contains("invalid floating constant"));
        assert!(err("2._").contains("invalid floating constant"));
        assert!(err("1.0e10.0").contains("invalid floating constant"));
        assert!(err("30.e").contains("invalid floating constant"));
        assert!(err("24e-").contains("invalid floating constant"));
        assert!(err("1.e-10x").contains("invalid floating constant"));
        assert!(err("0x1.0").contains("requires an exponent"));
        assert!(err("0xp1").contains("invalid hexadecimal floating constant"));
    }

    #[test]
    fn character_constants() {
        let one = tokens("'a'");
        assert_eq!(one[0].value, Some(u64::from(b'a')));
        assert_eq!(one[0].int_ty, Some(crate::ast::TypeName::Int));
        assert_eq!(tokens(r"'\n'")[0].value, Some(10));
        assert_eq!(tokens(r"'\0'")[0].value, Some(0));
        assert_eq!(tokens(r"'\x41'")[0].value, Some(0x41));
        assert_eq!(tokens(r"'\101'")[0].value, Some(0o101));
        assert_eq!(tokens(r"'\377'")[0].value, Some(u64::from(u32::MAX)));
        assert_eq!(tokens(r"'\xff'")[0].value, Some(u64::from(u32::MAX)));
        assert_eq!(tokens(r"'\''")[0].value, Some(u64::from(b'\'')));
        assert_eq!(tokens(r"'\\'")[0].value, Some(u64::from(b'\\')));
        assert_eq!(tokens(r"'\?'")[0].value, Some(u64::from(b'?')));
        assert!(err("''").contains("empty character constant"));
        assert!(err("'ab'").contains("multi-character"));
        assert!(err(r"'\x100'").contains("hex escape sequence out of range"));
        assert!(err(r"'\400'").contains("octal escape sequence out of range"));
        assert!(err(r"'\q'").contains("unknown escape sequence"));
        assert!(err("'a").contains("unterminated character constant"));
    }

    #[test]
    fn rejects_literals_and_raw_hash() {
        assert!(err("\"hi").contains("unterminated string literal"));
        assert_eq!(tokens("L\"hi\"")[0].kind, TokenKind::String);
        assert_eq!(
            tokens("u8\"hi\"")[0].bytes.as_deref(),
            Some(b"hi".as_slice())
        );
        assert_eq!(tokens("u\"A\"")[0].bytes.as_deref(), Some(&[b'A', 0][..]));
        assert_eq!(
            tokens("U\"A\"")[0].bytes.as_deref(),
            Some(&[b'A', 0, 0, 0][..])
        );
        assert!(err("u8'A'").contains("u8 character constants"));
        let u16 = tokens("u'A'");
        assert_eq!(u16[0].value, Some(u64::from(b'A')));
        assert_eq!(u16[0].int_ty, Some(crate::ast::TypeName::UShort));
        let u32 = tokens("U'\\xFF'");
        assert_eq!(u32[0].value, Some(0xFF));
        assert_eq!(u32[0].int_ty, Some(crate::ast::TypeName::UInt));
        assert!(err("u'\\x10000'").contains("hex escape sequence out of range"));
        assert_eq!(tokens("L'a'")[0].value, Some(u64::from(b'a')));
        assert_eq!(tokens("L'\\0'")[0].value, Some(0));
        assert_eq!(
            tokens("1.0L")[0].float_kind,
            Some(crate::ast::FloatKind::LongDouble)
        );
        assert!(err("1.ex").contains("invalid floating constant"));
        assert!(err("1e").contains("invalid floating constant"));
        assert!(err("24e-").contains("invalid floating constant"));
        assert!(err("0x1.0").contains("requires an exponent"));
        assert!(err("/* unterminated").contains("unterminated block comment"));
        assert!(err("#include <stdio.h>").contains("preprocessor directives"));
        assert!(err("int café").contains("non-ASCII"));
        // UTF-8 source characters and universal character names (C11 6.4.3)
        // are code points, encoded for each prefix.
        assert_eq!(
            tokens("\"\\u00c4\"")[0].bytes.as_deref(),
            Some(&[0xC3, 0x84][..])
        );
        assert_eq!(
            tokens("u8\"€\"")[0].bytes.as_deref(),
            Some(&[0xE2, 0x82, 0xAC][..])
        );
        assert_eq!(
            tokens("u\"\\U0001F600\"")[0].bytes.as_deref(),
            Some(&[0x3D, 0xD8, 0x00, 0xDE][..])
        );
        assert_eq!(
            tokens("L\"Ä\"")[0].bytes.as_deref(),
            Some(&[0xC4, 0, 0, 0][..])
        );
        assert_eq!(tokens("U'😀'")[0].value, Some(0x1F600));
        assert_eq!(tokens("L'\\u20AC'")[0].value, Some(0x20AC));
        assert_eq!(tokens("u'€'")[0].value, Some(0x20AC));
        assert_eq!(tokens("'\\u0040'")[0].value, Some(0x40));
        assert!(err("'Ä'").contains("character too large"));
        assert!(err("u'😀'").contains("character too large"));
        assert!(err("L'\\u0041'").contains("invalid universal character name"));
        assert!(err("L'\\uDC00'").contains("invalid universal character name"));
        assert!(err("U'\\U00110000'").contains("invalid universal character name"));
        assert!(err("L'\\u12'").contains("incomplete universal character name"));
        assert!(err("\"\\x100\"").contains("hex escape sequence out of range"));
        assert!(err("U'\\x100000000'").contains("hex escape sequence out of range"));
        assert!(err("int x = 1 $ 2;").contains("unexpected character '$'"));
    }

    #[test]
    fn string_literals_decode_escapes() {
        let hi = tokens(r#""hi""#);
        assert_eq!(hi[0].kind, TokenKind::String);
        assert_eq!(hi[0].bytes.as_deref(), Some(b"hi".as_slice()));
        assert_eq!(tokens(r#""""#)[0].bytes.as_deref(), Some(b"".as_slice()));
        assert_eq!(
            tokens(r#""\n\t\0\x41\101""#)[0].bytes.as_deref(),
            Some(&[b'\n', b'\t', 0, 0x41, 0o101][..])
        );
        assert_eq!(tokens(r#""\xff""#)[0].bytes.as_deref(), Some(&[0xff][..]));
        assert_eq!(
            tokens(r#""\'\"\\\?""#)[0].bytes.as_deref(),
            Some(b"'\"\\?".as_slice())
        );
        assert_eq!(
            tokens(r#""a\1234""#)[0].bytes.as_deref(),
            Some(&[b'a', 0o123, b'4'][..])
        );
        assert!(err(r#""\x100""#).contains("hex escape sequence out of range"));
        assert!(err(r#""\400""#).contains("octal escape sequence out of range"));
        assert!(err(r#""\q""#).contains("unknown escape sequence"));
        assert!(err("\"a\n\"").contains("unterminated string literal"));
        assert!(err("\"unterminated").contains("unterminated string literal"));
    }

    #[test]
    fn line_splice_is_diagnosed() {
        let line = err("// note \\\nreturn 1;\n");
        assert!(line.contains("backslash line splicing is not supported"));
        assert!(line.contains("1:9:"));
        let block = err("/* note \\\nstill */\n");
        assert!(block.contains("backslash line splicing is not supported"));
        assert_eq!(spellings("// a \\ b\nint x"), vec!["int", "x"]);
        assert_eq!(spellings("/* a \\ b */ int x"), vec!["int", "x"]);
        assert_eq!(spellings("// ends\nint x"), vec!["int", "x"]);
    }

    #[test]
    fn columns_after_keyword() {
        let toks = tokens("int main");
        assert_eq!((toks[0].line, toks[0].column), (1, 1));
        assert_eq!((toks[1].line, toks[1].column), (1, 5));
    }
}
