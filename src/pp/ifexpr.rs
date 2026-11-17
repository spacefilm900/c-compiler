//! Controlling expressions of `#if` and `#elif` (C11 6.10.1): parsing and
//! evaluation in `intmax_t` / `uintmax_t`.

use super::*;

pub(super) fn literal_val(tok: &PpToken) -> Result<Val, Diagnostic> {
    // A prefixed character constant has the value and type it has in
    // program text (C11 6.10.1p4): `L'\400'` is the `wchar_t` 256, and
    // `U'\xffffffff'` is an unsigned value.
    match crate::lexer::lex_shared(&tok.spelling, &tok.display) {
        Ok(mut tokens) => {
            tokens.pop();
            if tokens.len() == 1 && tokens[0].kind == TokenKind::Float {
                return Err(error_at(
                    tok,
                    "floating constant in preprocessor expression",
                ));
            }
            if tokens.len() != 1 || tokens[0].value.is_none() || tokens[0].int_ty.is_none() {
                return Err(error_at(
                    tok,
                    format!("invalid integer constant '{}'", tok.spelling),
                ));
            }
            Ok(val_from_compiler(&tokens[0]))
        }
        Err(mut err) => {
            err.filename = tok.display.to_string();
            err.line = tok.line;
            err.column = tok.column;
            Err(err)
        }
    }
}

pub(super) fn val_from_compiler(tok: &Token) -> Val {
    let ty = tok.int_ty.unwrap_or(TypeName::Int);
    let raw = tok.value.unwrap_or(0);
    let bits = if ty.is_signed() {
        sign_extend(u128::from(raw), ty.width()) as u64
    } else {
        (u128::from(raw) & ty.mask()) as u64
    };
    // `#if` uses `intmax_t` for every integer constant that fits in it, even
    // when the phase-7 type is a narrower unsigned type (`0xFFFFFFFF` is
    // `unsigned int` and still fits). A `u`/`U` suffix, or a value that does
    // not fit in signed `intmax_t`, stays unsigned. A character constant is
    // not scanned for a suffix.
    let spelling = tok.spelling.as_str();
    let number = spelling.as_bytes().first().is_some_and(u8::is_ascii_digit);
    let u_suffix = number && spelling.bytes().any(|byte| byte == b'u' || byte == b'U');
    let too_big = !ty.is_signed() && ty.width() == 64 && bits > i64::MAX as u64;
    Val {
        bits,
        unsigned: u_suffix || too_big,
    }
}

#[derive(Clone, Copy)]
pub(super) struct Val {
    pub(super) bits: u64,
    pub(super) unsigned: bool,
}

impl Val {
    pub(super) fn signed(value: i64) -> Self {
        Self {
            bits: value as u64,
            unsigned: false,
        }
    }

    pub(super) fn zero(unsigned: bool) -> Self {
        Self { bits: 0, unsigned }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Unary {
    Plus,
    Minus,
    BitNot,
    Not,
}

#[derive(Clone, Copy)]
pub(super) enum Binary {
    Mul,
    Div,
    Mod,
    Add,
    Sub,
    Shl,
    Shr,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    BitAnd,
    BitXor,
    BitOr,
    And,
    Or,
}

pub(super) struct IfLoc {
    pub(super) display: Rc<str>,
    pub(super) line: usize,
    pub(super) column: usize,
}

pub(super) enum IfExpr {
    Num(Val),
    Unary {
        op: Unary,
        expr: Box<IfExpr>,
        loc: IfLoc,
    },
    Binary {
        op: Binary,
        left: Box<IfExpr>,
        right: Box<IfExpr>,
        loc: IfLoc,
    },
    Cond {
        cond: Box<IfExpr>,
        then_expr: Box<IfExpr>,
        else_expr: Box<IfExpr>,
    },
}

pub(super) struct IfParser<'a> {
    pub(super) tokens: &'a [PpToken],
    pub(super) index: usize,
}

pub(super) fn parse_if_expr(tokens: &[PpToken]) -> Result<IfExpr, Diagnostic> {
    if tokens.is_empty() {
        return Err(Diagnostic::new("input", 1, 1, "expected an expression"));
    }
    let mut parser = IfParser { tokens, index: 0 };
    let expr = parser.parse_cond()?;
    if parser.index != parser.tokens.len() {
        return Err(parser.error("extra tokens in preprocessor expression"));
    }
    Ok(expr)
}

impl IfParser<'_> {
    pub(super) fn parse_cond(&mut self) -> Result<IfExpr, Diagnostic> {
        let cond = self.parse_or()?;
        if !self.eat("?") {
            return Ok(cond);
        }
        let then_expr = self.parse_cond()?;
        if !self.eat(":") {
            return Err(self.error("expected ':'"));
        }
        let else_expr = self.parse_cond()?;
        Ok(IfExpr::Cond {
            cond: Box::new(cond),
            then_expr: Box::new(then_expr),
            else_expr: Box::new(else_expr),
        })
    }

    pub(super) fn parse_or(&mut self) -> Result<IfExpr, Diagnostic> {
        self.parse_binary(Self::parse_and, &[("||", Binary::Or)])
    }

    pub(super) fn parse_and(&mut self) -> Result<IfExpr, Diagnostic> {
        self.parse_binary(Self::parse_bor, &[("&&", Binary::And)])
    }

    pub(super) fn parse_bor(&mut self) -> Result<IfExpr, Diagnostic> {
        self.parse_binary(Self::parse_bxor, &[("|", Binary::BitOr)])
    }

    pub(super) fn parse_bxor(&mut self) -> Result<IfExpr, Diagnostic> {
        self.parse_binary(Self::parse_band, &[("^", Binary::BitXor)])
    }

    pub(super) fn parse_band(&mut self) -> Result<IfExpr, Diagnostic> {
        self.parse_binary(Self::parse_eq, &[("&", Binary::BitAnd)])
    }

    pub(super) fn parse_eq(&mut self) -> Result<IfExpr, Diagnostic> {
        self.parse_binary(Self::parse_rel, &[("==", Binary::Eq), ("!=", Binary::Ne)])
    }

    pub(super) fn parse_rel(&mut self) -> Result<IfExpr, Diagnostic> {
        self.parse_binary(
            Self::parse_shift,
            &[
                ("<=", Binary::Le),
                (">=", Binary::Ge),
                ("<", Binary::Lt),
                (">", Binary::Gt),
            ],
        )
    }

    pub(super) fn parse_shift(&mut self) -> Result<IfExpr, Diagnostic> {
        self.parse_binary(Self::parse_add, &[("<<", Binary::Shl), (">>", Binary::Shr)])
    }

    pub(super) fn parse_add(&mut self) -> Result<IfExpr, Diagnostic> {
        self.parse_binary(Self::parse_mul, &[("+", Binary::Add), ("-", Binary::Sub)])
    }

    pub(super) fn parse_mul(&mut self) -> Result<IfExpr, Diagnostic> {
        self.parse_binary(
            Self::parse_unary,
            &[("*", Binary::Mul), ("/", Binary::Div), ("%", Binary::Mod)],
        )
    }

    pub(super) fn parse_binary(
        &mut self,
        mut operand: impl FnMut(&mut Self) -> Result<IfExpr, Diagnostic>,
        ops: &[(&str, Binary)],
    ) -> Result<IfExpr, Diagnostic> {
        let mut left = operand(self)?;
        loop {
            let Some(op) = ops.iter().find(|(spelling, _)| self.peek_punct(spelling)) else {
                break;
            };
            let op = *op;
            self.index += 1;
            let loc = self.prev_loc();
            let right = operand(self)?;
            left = IfExpr::Binary {
                op: op.1,
                left: Box::new(left),
                right: Box::new(right),
                loc,
            };
        }
        Ok(left)
    }

    pub(super) fn parse_unary(&mut self) -> Result<IfExpr, Diagnostic> {
        let Some(tok) = self.current().cloned() else {
            return Err(self.error("expected an expression"));
        };
        let op = match tok.spelling.as_str() {
            "+" if tok.kind == PpKind::Punct => Unary::Plus,
            "-" if tok.kind == PpKind::Punct => Unary::Minus,
            "~" if tok.kind == PpKind::Punct => Unary::BitNot,
            "!" if tok.kind == PpKind::Punct => Unary::Not,
            _ => return self.parse_primary(),
        };
        self.index += 1;
        let expr = self.parse_unary()?;
        Ok(IfExpr::Unary {
            op,
            expr: Box::new(expr),
            loc: loc_of(&tok),
        })
    }

    pub(super) fn parse_primary(&mut self) -> Result<IfExpr, Diagnostic> {
        let Some(tok) = self.current().cloned() else {
            return Err(self.error("expected an expression"));
        };
        if tok.is_lparen() {
            self.index += 1;
            let inner = self.parse_cond()?;
            if !self.eat(")") {
                return Err(self.error("expected ')'"));
            }
            return Ok(inner);
        }
        if tok.kind == PpKind::Number || tok.kind == PpKind::Char {
            self.index += 1;
            return Ok(IfExpr::Num(literal_val(&tok)?));
        }
        Err(error_at(
            &tok,
            format!(
                "invalid token '{}' in preprocessor expression",
                tok.spelling
            ),
        ))
    }

    pub(super) fn peek_punct(&self, spelling: &str) -> bool {
        self.current()
            .is_some_and(|tok| tok.kind == PpKind::Punct && tok.spelling == spelling)
    }

    pub(super) fn eat(&mut self, spelling: &str) -> bool {
        if self.peek_punct(spelling) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    pub(super) fn current(&self) -> Option<&PpToken> {
        self.tokens.get(self.index)
    }

    pub(super) fn prev_loc(&self) -> IfLoc {
        let tok = &self.tokens[self.index - 1];
        loc_of(tok)
    }

    pub(super) fn error(&self, message: &str) -> Diagnostic {
        if let Some(tok) = self.current() {
            error_at(tok, message)
        } else if let Some(tok) = self.tokens.last() {
            error_at(tok, message)
        } else {
            Diagnostic::new("input", 1, 1, message)
        }
    }
}

pub(super) fn loc_of(tok: &PpToken) -> IfLoc {
    IfLoc {
        display: tok.display.clone(),
        line: tok.line,
        column: tok.column,
    }
}

pub(super) fn eval_expr(expr: &IfExpr) -> Result<Val, Diagnostic> {
    match expr {
        IfExpr::Num(value) => Ok(*value),
        IfExpr::Unary { op, expr, loc } => {
            let value = eval_expr(expr)?;
            eval_unary(*op, value, loc)
        }
        IfExpr::Binary {
            op,
            left,
            right,
            loc,
        } => match op {
            Binary::And => {
                let left = eval_expr(left)?;
                if left.bits == 0 {
                    Ok(Val::signed(0))
                } else {
                    let right = eval_expr(right)?;
                    Ok(Val::signed(if right.bits != 0 { 1 } else { 0 }))
                }
            }
            Binary::Or => {
                let left = eval_expr(left)?;
                if left.bits != 0 {
                    Ok(Val::signed(1))
                } else {
                    let right = eval_expr(right)?;
                    Ok(Val::signed(if right.bits != 0 { 1 } else { 0 }))
                }
            }
            _ => {
                let left = eval_expr(left)?;
                let right = eval_expr(right)?;
                eval_binary(*op, left, right, loc)
            }
        },
        IfExpr::Cond {
            cond,
            then_expr,
            else_expr,
        } => {
            let cond = eval_expr(cond)?;
            let unsigned = expr_unsigned(then_expr) || expr_unsigned(else_expr);
            let value = if cond.bits != 0 {
                eval_expr(then_expr)?
            } else {
                eval_expr(else_expr)?
            };
            Ok(Val {
                bits: value.bits,
                unsigned,
            })
        }
    }
}

pub(super) fn expr_unsigned(expr: &IfExpr) -> bool {
    match expr {
        IfExpr::Num(value) => value.unsigned,
        IfExpr::Unary { op: Unary::Not, .. } => false,
        IfExpr::Unary { expr, .. } => expr_unsigned(expr),
        IfExpr::Binary {
            op:
                Binary::Lt
                | Binary::Gt
                | Binary::Le
                | Binary::Ge
                | Binary::Eq
                | Binary::Ne
                | Binary::And
                | Binary::Or,
            ..
        } => false,
        // The result type of a shift is the promoted left operand (C11 6.5.7).
        IfExpr::Binary {
            op: Binary::Shl | Binary::Shr,
            left,
            ..
        } => expr_unsigned(left),
        IfExpr::Binary { left, right, .. } => expr_unsigned(left) || expr_unsigned(right),
        IfExpr::Cond {
            then_expr,
            else_expr,
            ..
        } => expr_unsigned(then_expr) || expr_unsigned(else_expr),
    }
}

pub(super) fn eval_unary(op: Unary, value: Val, loc: &IfLoc) -> Result<Val, Diagnostic> {
    let _ = loc;
    Ok(match op {
        Unary::Plus => value,
        Unary::Minus => {
            if value.unsigned {
                Val {
                    bits: value.bits.wrapping_neg(),
                    unsigned: true,
                }
            } else {
                Val::signed((value.bits as i64).wrapping_neg())
            }
        }
        Unary::BitNot => Val {
            bits: !value.bits,
            unsigned: value.unsigned,
        },
        Unary::Not => Val::signed(if value.bits == 0 { 1 } else { 0 }),
    })
}

pub(super) fn eval_binary(
    op: Binary,
    left: Val,
    right: Val,
    loc: &IfLoc,
) -> Result<Val, Diagnostic> {
    if matches!(op, Binary::Shl | Binary::Shr) {
        return eval_shift(op, left, right, loc);
    }
    if matches!(
        op,
        Binary::Lt | Binary::Gt | Binary::Le | Binary::Ge | Binary::Eq | Binary::Ne
    ) {
        return Ok(Val::signed(if compare(op, left, right) { 1 } else { 0 }));
    }
    let unsigned = left.unsigned || right.unsigned;
    if matches!(op, Binary::Div | Binary::Mod) && right.bits == 0 {
        return Err(Diagnostic::new(
            &*loc.display,
            loc.line,
            loc.column,
            "division by zero in preprocessor expression",
        ));
    }
    let bits = if unsigned {
        let (x, y) = (left.bits, right.bits);
        match op {
            Binary::Mul => x.wrapping_mul(y),
            Binary::Div => x / y,
            Binary::Mod => x % y,
            Binary::Add => x.wrapping_add(y),
            Binary::Sub => x.wrapping_sub(y),
            Binary::BitAnd => x & y,
            Binary::BitXor => x ^ y,
            Binary::BitOr => x | y,
            _ => 0,
        }
    } else {
        let (x, y) = (left.bits as i64, right.bits as i64);
        let signed = match op {
            Binary::Mul => x.wrapping_mul(y),
            Binary::Div => x.wrapping_div(y),
            Binary::Mod => x.wrapping_rem(y),
            Binary::Add => x.wrapping_add(y),
            Binary::Sub => x.wrapping_sub(y),
            Binary::BitAnd => x & y,
            Binary::BitXor => x ^ y,
            Binary::BitOr => x | y,
            _ => 0,
        };
        signed as u64
    };
    Ok(Val { bits, unsigned })
}

pub(super) fn compare(op: Binary, left: Val, right: Val) -> bool {
    if left.unsigned || right.unsigned {
        match op {
            Binary::Lt => left.bits < right.bits,
            Binary::Gt => left.bits > right.bits,
            Binary::Le => left.bits <= right.bits,
            Binary::Ge => left.bits >= right.bits,
            Binary::Eq => left.bits == right.bits,
            Binary::Ne => left.bits != right.bits,
            _ => false,
        }
    } else {
        let (x, y) = (left.bits as i64, right.bits as i64);
        match op {
            Binary::Lt => x < y,
            Binary::Gt => x > y,
            Binary::Le => x <= y,
            Binary::Ge => x >= y,
            Binary::Eq => x == y,
            Binary::Ne => x != y,
            _ => false,
        }
    }
}

pub(super) fn eval_shift(
    op: Binary,
    left: Val,
    right: Val,
    loc: &IfLoc,
) -> Result<Val, Diagnostic> {
    let count = if right.unsigned {
        right.bits
    } else {
        let count = right.bits as i64;
        if count < 0 {
            return Err(Diagnostic::new(
                &*loc.display,
                loc.line,
                loc.column,
                "negative shift count in preprocessor expression",
            ));
        }
        count as u64
    };
    if count >= 64 {
        return Ok(Val::zero(left.unsigned));
    }
    let bits = if left.unsigned {
        match op {
            Binary::Shl => left.bits.wrapping_shl(count as u32),
            _ => left.bits.wrapping_shr(count as u32),
        }
    } else {
        let value = left.bits as i64;
        match op {
            Binary::Shl => value.wrapping_shl(count as u32) as u64,
            _ => value.wrapping_shr(count as u32) as u64,
        }
    };
    Ok(Val {
        bits,
        unsigned: left.unsigned,
    })
}
