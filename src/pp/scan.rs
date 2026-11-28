//! Translation phases 1-3: line splicing, comments and preprocessing
//! tokens.

use super::*;

pub(super) struct Scanner<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) index: usize,
    pub(super) line: usize,
    pub(super) column: usize,
    pub(super) display: Rc<str>,
    pub(super) file_id: u32,
    pub(super) pending_space: bool,
    pub(super) tokens: Vec<PpToken>,
}

pub(super) fn lex_source(
    source: &str,
    display: &str,
    file_id: u32,
) -> Result<Vec<PpToken>, Diagnostic> {
    let mut scanner = Scanner {
        bytes: source.as_bytes(),
        index: 0,
        line: 1,
        column: 1,
        display: Rc::from(display),
        file_id,
        pending_space: false,
        tokens: Vec::new(),
    };
    scanner.run()?;
    Ok(scanner.tokens)
}

impl Scanner<'_> {
    pub(super) fn run(&mut self) -> Result<(), Diagnostic> {
        while self.skip_splices() {
            if self.index >= self.bytes.len() {
                break;
            }
            let line = self.line;
            let column = self.column;
            let byte = self.bytes[self.index];
            if byte == b' ' || byte == b'\t' || byte == 0x0B || byte == 0x0C {
                self.bump();
                self.pending_space = true;
                continue;
            }
            if byte == b'\n' || byte == b'\r' {
                self.bump();
                self.push_newline(line, column);
                continue;
            }
            if byte == b'/' && self.peek_logical_at(1) == Some(b'/') {
                self.line_comment()?;
                continue;
            }
            if byte == b'/' && self.peek_logical_at(1) == Some(b'*') {
                self.block_comment(line, column)?;
                continue;
            }
            if is_ident_start(byte)
                || byte >= 128
                || (byte == b'\\' && self.extended_char().is_some())
            {
                self.ident(line, column)?;
                continue;
            }
            if byte.is_ascii_digit()
                || (byte == b'.' && self.peek_logical_at(1).is_some_and(|b| b.is_ascii_digit()))
            {
                self.number(line, column);
                continue;
            }
            if byte == b'"' {
                self.string_or_char(line, column, true)?;
                continue;
            }
            if byte == b'\'' {
                self.string_or_char(line, column, false)?;
                continue;
            }
            if let Some(punct) = self.match_punct() {
                let spelling = canonical_punct(punct);
                let len = punct.len();
                for _ in 0..len {
                    self.next_logical();
                }
                self.push(PpKind::Punct, spelling, line, column);
                continue;
            }
            self.bump();
            self.push(PpKind::Other, &(byte as char).to_string(), line, column);
        }
        Ok(())
    }

    pub(super) fn line_comment(&mut self) -> Result<(), Diagnostic> {
        self.next_logical();
        self.next_logical();
        while let Some(byte) = self.peek_byte_logical() {
            if byte == b'\n' || byte == b'\r' {
                break;
            }
            self.next_logical();
        }
        self.pending_space = true;
        Ok(())
    }

    pub(super) fn block_comment(&mut self, line: usize, column: usize) -> Result<(), Diagnostic> {
        self.next_logical();
        self.next_logical();
        loop {
            let Some(byte) = self.peek_byte_logical() else {
                return self.fail(line, column, "unterminated block comment");
            };
            if byte == b'*' && self.peek_logical_at(1) == Some(b'/') {
                self.next_logical();
                self.next_logical();
                self.pending_space = true;
                return Ok(());
            }
            self.next_logical();
        }
    }

    /// An identifier (6.4.2.1). Besides the basic characters it may hold
    /// universal character names and UTF-8 characters from Annex D; the
    /// spelling keeps them as UTF-8, so both forms name one identifier.
    pub(super) fn ident(&mut self, line: usize, column: usize) -> Result<(), Diagnostic> {
        let mut spelling = String::new();
        loop {
            if self.peek_byte_logical().is_some_and(is_ident_continue) {
                spelling.push(self.next_logical().unwrap().0 as char);
                continue;
            }
            let Some((code, len, ucn)) = self.extended_char() else {
                break;
            };
            let initial = spelling.is_empty();
            if (ucn && !crate::ucn::valid_ucn_value(code))
                || !crate::ucn::allowed_in_identifier(code, initial)
            {
                self.skip_splices();
                let (at_line, at_column) = (self.line, self.column);
                if initial && !ucn && !crate::ucn::allowed_in_identifier(code, false) {
                    return self.fail(
                        at_line,
                        at_column,
                        &format!("unexpected character U+{code:04X} outside a string, character constant or comment"),
                    );
                }
                return self.fail(
                    at_line,
                    at_column,
                    &crate::ucn::identifier_error(code, initial, ucn),
                );
            }
            for _ in 0..len {
                self.next_logical();
            }
            spelling.push(char::from_u32(code).expect("checked above"));
        }
        if matches!(spelling.as_str(), "L" | "u" | "U" | "u8") {
            if self.peek_byte_logical() == Some(b'"') {
                let mut tok_spelling = self.read_quoted(line, column, true)?;
                tok_spelling.insert_str(0, &spelling);
                self.push(PpKind::String, &tok_spelling, line, column);
                return Ok(());
            }
            if self.peek_byte_logical() == Some(b'\'') {
                let mut tok_spelling = self.read_quoted(line, column, false)?;
                tok_spelling.insert_str(0, &spelling);
                self.push(PpKind::Char, &tok_spelling, line, column);
                return Ok(());
            }
        }
        self.push(PpKind::Ident, &spelling, line, column);
        Ok(())
    }

    pub(super) fn number(&mut self, line: usize, column: usize) {
        let mut spelling = String::new();
        let (byte, _, _) = self.next_logical().unwrap();
        spelling.push(byte as char);
        loop {
            let Some(byte) = self.peek_byte_logical() else {
                break;
            };
            if byte.is_ascii_digit() || is_ident_continue(byte) || byte == b'.' {
                let (ch, _, _) = self.next_logical().unwrap();
                spelling.push(ch as char);
                if matches!(ch, b'e' | b'E' | b'p' | b'P')
                    && matches!(self.peek_byte_logical(), Some(b'+' | b'-'))
                {
                    spelling.push(self.next_logical().unwrap().0 as char);
                }
                continue;
            }
            // A pp-number may hold identifier characters (6.4.8), so a
            // valid UCN or UTF-8 identifier character stays in the token.
            if let Some((code, len, ucn)) = self.extended_char() {
                if (!ucn || crate::ucn::valid_ucn_value(code))
                    && crate::ucn::allowed_in_identifier(code, false)
                {
                    for _ in 0..len {
                        self.next_logical();
                    }
                    spelling.push(char::from_u32(code).expect("checked above"));
                    continue;
                }
            }
            break;
        }
        self.push(PpKind::Number, &spelling, line, column);
    }

    pub(super) fn string_or_char(
        &mut self,
        line: usize,
        column: usize,
        string: bool,
    ) -> Result<(), Diagnostic> {
        let spelling = self.read_quoted(line, column, string)?;
        let kind = if string { PpKind::String } else { PpKind::Char };
        self.push(kind, &spelling, line, column);
        Ok(())
    }

    pub(super) fn read_quoted(
        &mut self,
        line: usize,
        column: usize,
        string: bool,
    ) -> Result<String, Diagnostic> {
        let closer = if string { b'"' } else { b'\'' };
        // Bytes, not chars: a UTF-8 character inside the literal is copied
        // unchanged and decoded by the phase-7 lexer (C11 5.2.1).
        let mut spelling: Vec<u8> = vec![self.next_logical().unwrap().0];
        loop {
            let Some((byte, _, _)) = self.next_logical() else {
                return self.fail(
                    line,
                    column,
                    if string {
                        "unterminated string literal"
                    } else {
                        "unterminated character constant"
                    },
                );
            };
            if byte == b'\n' || byte == b'\r' {
                return self.fail(
                    line,
                    column,
                    if string {
                        "unterminated string literal"
                    } else {
                        "unterminated character constant"
                    },
                );
            }
            spelling.push(byte);
            if byte == b'\\' {
                let Some((next, _, _)) = self.next_logical() else {
                    return self.fail(
                        line,
                        column,
                        if string {
                            "unterminated string literal"
                        } else {
                            "unterminated character constant"
                        },
                    );
                };
                if next == b'\n' || next == b'\r' {
                    return self.fail(
                        line,
                        column,
                        if string {
                            "unterminated string literal"
                        } else {
                            "unterminated character constant"
                        },
                    );
                }
                spelling.push(next);
                continue;
            }
            if byte == closer {
                break;
            }
        }
        // The source is UTF-8 and only whole characters were copied.
        Ok(String::from_utf8(spelling).expect("UTF-8 literal"))
    }

    pub(super) fn match_punct(&self) -> Option<&'static str> {
        // The longest punctuator is four characters: read that many logical
        // bytes once instead of once per candidate.
        let mut ahead = [0u8; 4];
        let have = self.peek_logical_into(&mut ahead);
        punctuator_prefix(&ahead[..have])
    }

    pub(super) fn push(&mut self, kind: PpKind, spelling: &str, line: usize, column: usize) {
        let leading_space = self.pending_space;
        self.pending_space = false;
        self.tokens.push(PpToken {
            kind,
            spelling: spelling.to_string(),
            display: Rc::clone(&self.display),
            file_id: self.file_id,
            line,
            column,
            leading_space,
            hide: hide_empty(),
        });
    }

    pub(super) fn push_newline(&mut self, line: usize, column: usize) {
        self.pending_space = false;
        self.tokens.push(PpToken {
            kind: PpKind::Newline,
            spelling: "\n".to_string(),
            display: Rc::clone(&self.display),
            file_id: self.file_id,
            line,
            column,
            leading_space: false,
            hide: hide_empty(),
        });
    }

    /// A universal character name or a UTF-8 character at the cursor, as
    /// (code point, logical bytes, written as a UCN), or `None`. Line
    /// splices inside a UCN are skipped (they go in translation phase 2).
    pub(super) fn extended_char(&self) -> Option<(u32, usize, bool)> {
        let mut buf = [0u8; 10];
        let filled = self.peek_logical_into(&mut buf);
        let buf = &buf[..filled];
        if let Some((code, len)) = crate::ucn::parse_ucn(buf) {
            return Some((code, len, true));
        }
        let first = *buf.first()?;
        if first < 0x80 {
            return None;
        }
        let len = match first {
            0xC0..=0xDF => 2,
            0xE0..=0xEF => 3,
            _ => 4,
        };
        let ch = std::str::from_utf8(buf.get(..len)?).ok()?.chars().next()?;
        Some((u32::from(ch), len, false))
    }

    pub(super) fn fail<T>(
        &self,
        line: usize,
        column: usize,
        message: &str,
    ) -> Result<T, Diagnostic> {
        Err(Diagnostic::new(&*self.display, line, column, message))
    }

    pub(super) fn skip_splices(&mut self) -> bool {
        loop {
            if self.index >= self.bytes.len() {
                return false;
            }
            if self.bytes[self.index] == b'\\' && is_newline(self.bytes, self.index + 1) {
                self.bump();
                self.bump();
                continue;
            }
            return true;
        }
    }

    pub(super) fn next_logical(&mut self) -> Option<(u8, usize, usize)> {
        if !self.skip_splices() {
            return None;
        }
        let line = self.line;
        let column = self.column;
        let byte = self.bump();
        Some((byte, line, column))
    }

    pub(super) fn peek_byte_logical(&self) -> Option<u8> {
        self.peek_logical_at(0)
    }

    /// The logical byte `offset` places ahead, skipping line splices.
    pub(super) fn peek_logical_at(&self, offset: usize) -> Option<u8> {
        let mut seen = 0;
        let mut index = self.index;
        loop {
            if index >= self.bytes.len() {
                return None;
            }
            if self.bytes[index] == b'\\' && is_newline(self.bytes, index + 1) {
                index += 1;
                index += newline_width(self.bytes, index);
                continue;
            }
            if seen == offset {
                return Some(self.bytes[index]);
            }
            seen += 1;
            index += 1;
        }
    }

    /// Fill `out` with the next logical bytes, skipping line splices, and
    /// return how many were available.
    pub(super) fn peek_logical_into(&self, out: &mut [u8]) -> usize {
        let mut filled = 0;
        let mut index = self.index;
        while filled < out.len() {
            if index >= self.bytes.len() {
                break;
            }
            if self.bytes[index] == b'\\' && is_newline(self.bytes, index + 1) {
                index += 1;
                index += newline_width(self.bytes, index);
                continue;
            }
            out[filled] = self.bytes[index];
            filled += 1;
            index += 1;
        }
        filled
    }

    pub(super) fn bump(&mut self) -> u8 {
        let byte = self.bytes[self.index];
        self.index += 1;
        if byte == b'\n' {
            self.line += 1;
            self.column = 1;
        } else if byte == b'\r' {
            if self.index < self.bytes.len() && self.bytes[self.index] == b'\n' {
                self.index += 1;
            }
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        byte
    }
}

/// The longest punctuator (C11 6.4.6, digraphs included) that `ahead`
/// starts with, as spelled, or `None`. Maximal munch (6.4p4): arms are
/// ordered longest first, so `...` wins over `.`, `<<=` over `<<` and `<`,
/// and `%:%:` over `%:` and `%`.
///
/// This is a `match` on the bytes, which the compiler turns into a few
/// comparisons. Trying every punctuator in turn took up to 55 `memcmp`
/// calls for one `(` and was about a tenth of the time spent on files that
/// are mostly headers.
pub(super) fn punctuator_prefix(ahead: &[u8]) -> Option<&'static str> {
    match ahead {
        [b'%', b':', b'%', b':', ..] => Some("%:%:"),
        [b'<', b'<', b'=', ..] => Some("<<="),
        [b'>', b'>', b'=', ..] => Some(">>="),
        [b'.', b'.', b'.', ..] => Some("..."),
        [b'<', b'<', ..] => Some("<<"),
        [b'>', b'>', ..] => Some(">>"),
        [b'<', b'=', ..] => Some("<="),
        [b'>', b'=', ..] => Some(">="),
        [b'=', b'=', ..] => Some("=="),
        [b'!', b'=', ..] => Some("!="),
        [b'&', b'&', ..] => Some("&&"),
        [b'|', b'|', ..] => Some("||"),
        [b'+', b'+', ..] => Some("++"),
        [b'-', b'-', ..] => Some("--"),
        [b'+', b'=', ..] => Some("+="),
        [b'-', b'=', ..] => Some("-="),
        [b'*', b'=', ..] => Some("*="),
        [b'/', b'=', ..] => Some("/="),
        [b'%', b'=', ..] => Some("%="),
        [b'&', b'=', ..] => Some("&="),
        [b'^', b'=', ..] => Some("^="),
        [b'|', b'=', ..] => Some("|="),
        [b'-', b'>', ..] => Some("->"),
        [b'#', b'#', ..] => Some("##"),
        [b'<', b':', ..] => Some("<:"),
        [b':', b'>', ..] => Some(":>"),
        [b'<', b'%', ..] => Some("<%"),
        [b'%', b'>', ..] => Some("%>"),
        [b'%', b':', ..] => Some("%:"),
        [b'#', ..] => Some("#"),
        [b'+', ..] => Some("+"),
        [b'-', ..] => Some("-"),
        [b'*', ..] => Some("*"),
        [b'/', ..] => Some("/"),
        [b'%', ..] => Some("%"),
        [b'=', ..] => Some("="),
        [b'<', ..] => Some("<"),
        [b'>', ..] => Some(">"),
        [b'!', ..] => Some("!"),
        [b'&', ..] => Some("&"),
        [b'|', ..] => Some("|"),
        [b'^', ..] => Some("^"),
        [b'~', ..] => Some("~"),
        [b'(', ..] => Some("("),
        [b')', ..] => Some(")"),
        [b'{', ..] => Some("{"),
        [b'}', ..] => Some("}"),
        [b'[', ..] => Some("["),
        [b']', ..] => Some("]"),
        [b';', ..] => Some(";"),
        [b',', ..] => Some(","),
        [b'?', ..] => Some("?"),
        [b':', ..] => Some(":"),
        [b'.', ..] => Some("."),
        _ => None,
    }
}

pub(super) fn canonical_punct(punct: &str) -> &str {
    match punct {
        "%:%:" => "##",
        "%:" => "#",
        other => other,
    }
}

pub(super) fn is_newline(bytes: &[u8], index: usize) -> bool {
    matches!(bytes.get(index), Some(b'\n' | b'\r'))
}

pub(super) fn newline_width(bytes: &[u8], index: usize) -> usize {
    if bytes.get(index) == Some(&b'\r') && bytes.get(index + 1) == Some(&b'\n') {
        2
    } else {
        1
    }
}

pub(super) fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

pub(super) fn is_ident_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
