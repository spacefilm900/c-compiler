//! Hand-written lexer for a C subset.
//!
//! This is a direct tokenization of source text. It does not implement C
//! translation phases 1–4: no trigraphs, no backslash line splicing, and no
//! preprocessor. Those inputs are diagnosed rather than ignored.

use crate::diag::Diagnostic;
use crate::token::{keyword, Token, TokenKind};

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
                self.ident(line, column);
                continue;
            }
            if byte.is_ascii_digit() {
                self.number(line, column)?;
                continue;
            }
            if byte == b'"' || byte == b'\'' {
                self.quoted(byte, line, column)?;
                continue;
            }
            if byte == b'#' {
                return self.fail(line, column, "preprocessor directives are not supported");
            }
            if byte == b'.' {
                self.dot(line, column)?;
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
            value: None,
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

    fn push(
        &mut self,
        kind: TokenKind,
        spelling: &str,
        line: usize,
        column: usize,
        value: Option<i32>,
    ) {
        self.tokens.push(Token {
            kind,
            spelling: spelling.to_string(),
            line,
            column,
            value,
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

    fn ident(&mut self, line: usize, column: usize) {
        let start = self.index;
        while self.peek().is_some_and(is_ident_cont) {
            self.bump();
        }
        let spelling = &self.source[start..self.index];
        let kind = keyword(spelling).unwrap_or(TokenKind::Ident);
        self.push(kind, spelling, line, column, None);
    }

    fn number(&mut self, line: usize, column: usize) -> Result<(), Diagnostic> {
        let start = self.index;
        if self.peek() == Some(b'0') && matches!(self.peek_at(1), Some(b'x' | b'X')) {
            self.bump();
            self.bump();
            let digits = self.index;
            while self.peek().is_some_and(|b| b.is_ascii_hexdigit()) {
                self.bump();
            }
            if self.index == digits {
                return self.fail(line, column, "invalid hexadecimal constant");
            }
            self.reject_number_tail(line, column)?;
            let spelling = &self.source[start..self.index];
            let value = self.int_value(&self.source[digits..self.index], 16, line, column)?;
            return self.push_int(spelling, value, line, column);
        }
        if self.peek() == Some(b'0') && matches!(self.peek_at(1), Some(b'b' | b'B')) {
            return self.fail(line, column, "binary constants are not supported");
        }
        if self.peek() == Some(b'0') {
            while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                if matches!(self.peek(), Some(b'8' | b'9')) {
                    return self.fail(line, column, "invalid octal constant");
                }
                self.bump();
            }
            self.reject_number_tail(line, column)?;
            let spelling = &self.source[start..self.index];
            let value = self.int_value(spelling, 8, line, column)?;
            return self.push_int(spelling, value, line, column);
        }
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.bump();
        }
        self.reject_number_tail(line, column)?;
        let spelling = &self.source[start..self.index];
        let value = self.int_value(spelling, 10, line, column)?;
        self.push_int(spelling, value, line, column)
    }

    /// Parse a digit sequence that the scanner has already checked.
    ///
    /// C11 6.4.4.1 gives an unsuffixed constant the first type in a list that
    /// can hold it (`int`, then `long`, then `long long`, and unsigned types
    /// for octal and hex). This subset has only signed 32-bit `int`, so a
    /// value that does not fit, including one that does not fit in `i64`, is
    /// a diagnostic. The wider C types are not introduced here.
    fn int_value(
        &self,
        digits: &str,
        radix: u32,
        line: usize,
        column: usize,
    ) -> Result<i64, Diagnostic> {
        let mut value: i64 = 0;
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
                .checked_mul(i64::from(radix))
                .and_then(|next| next.checked_add(i64::from(digit)))
            {
                Some(next) => next,
                None => return self.fail(line, column, "integer constant does not fit in int"),
            };
        }
        Ok(value)
    }

    fn push_int(
        &mut self,
        spelling: &str,
        value: i64,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        if value > i32::MAX as i64 {
            return self.fail(line, column, "integer constant does not fit in int");
        }
        self.push(TokenKind::Int, spelling, line, column, Some(value as i32));
        Ok(())
    }

    fn reject_number_tail(&self, line: usize, column: usize) -> Result<(), Diagnostic> {
        match self.peek() {
            Some(b'.' | b'e' | b'E' | b'p' | b'P') => {
                self.fail(line, column, "floating constants are not supported")
            }
            Some(b'u' | b'U' | b'l' | b'L') => {
                self.fail(line, column, "integer suffixes are not supported")
            }
            Some(b) if b.is_ascii_alphabetic() || b == b'_' => {
                self.fail(line, column, "invalid numeric literal")
            }
            _ => Ok(()),
        }
    }

    fn quoted(&mut self, quote: u8, line: usize, column: usize) -> Result<(), Diagnostic> {
        self.bump();
        while let Some(byte) = self.peek() {
            if byte == b'\n' || byte == b'\r' {
                break;
            }
            if byte == b'\\' {
                self.bump();
                if let Some(next) = self.peek() {
                    if next != b'\n' && next != b'\r' {
                        self.bump();
                    }
                }
                continue;
            }
            self.bump();
            if byte == quote {
                let message = if quote == b'"' {
                    "string literals are not supported"
                } else {
                    "character constants are not supported"
                };
                return self.fail(line, column, message);
            }
        }
        let message = if quote == b'"' {
            "unterminated string literal"
        } else {
            "unterminated character constant"
        };
        self.fail(line, column, message)
    }

    fn dot(&mut self, line: usize, column: usize) -> Result<(), Diagnostic> {
        if self.peek_at(1) == Some(b'.') && self.peek_at(2) == Some(b'.') {
            self.bump();
            self.bump();
            self.bump();
            self.push(TokenKind::Ellipsis, "...", line, column, None);
            return Ok(());
        }
        if self.peek_at(1).is_some_and(|b| b.is_ascii_digit()) {
            return self.fail(line, column, "floating constants are not supported");
        }
        self.bump();
        self.push(TokenKind::Dot, ".", line, column, None);
        Ok(())
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
        self.push(kind, spelling, line, column, None);
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
        self.push(kind, spelling, line, column, None);
        true
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
            kinds("== != <= >= && || ++ -- << >> += -= *= /= %= &= |= ^= ->"),
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
        assert!(err("2147483648").contains("does not fit in int"));
        assert!(err("0x80000000").contains("does not fit in int"));
        assert!(err("99999999999999999999").contains("does not fit in int"));
        assert!(err("0x1FFFFFFFFFFFFFFFF").contains("does not fit in int"));
        assert!(err("01777777777777777777777").contains("does not fit in int"));
        assert!(err("08").contains("invalid octal"));
        assert!(err("0x").contains("invalid hexadecimal"));
        assert!(err("0b1").contains("binary constants"));
        assert!(err("1u").contains("integer suffixes"));
        assert!(err("123abc").contains("invalid numeric literal"));
    }

    #[test]
    fn rejects_literals_and_preprocessor() {
        assert!(err("\"hi\"").contains("string literals are not supported"));
        assert!(err("\"hi").contains("unterminated string literal"));
        assert!(err("'a'").contains("character constants are not supported"));
        assert!(err("'a").contains("unterminated character constant"));
        assert!(err("1.5").contains("floating constants"));
        assert!(err(".5").contains("floating constants"));
        assert!(err("1e2").contains("floating constants"));
        assert!(err("/* unterminated").contains("unterminated block comment"));
        assert!(err("#include <stdio.h>").contains("preprocessor directives"));
        assert!(err("int café").contains("non-ASCII"));
        assert!(err("int x = 1 $ 2;").contains("unexpected character '$'"));
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
