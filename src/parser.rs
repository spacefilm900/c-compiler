//! Recursive-descent parser for a C subset.
//!
//! Expression precedence follows the C standard: assignment is right
//! associative and looser than `||`, which is looser than `&&`, equality,
//! relational, additive, and multiplicative operators. A dangling `else`
//! binds to the nearest `if`.

use crate::ast::*;
use crate::diag::Diagnostic;
use crate::token::{Token, TokenKind};

pub fn parse(tokens: &[Token], filename: &str) -> Result<TranslationUnit, Diagnostic> {
    Parser {
        tokens,
        index: 0,
        filename,
    }
    .parse_translation_unit()
}

struct Parser<'a> {
    tokens: &'a [Token],
    index: usize,
    filename: &'a str,
}

impl Parser<'_> {
    fn parse_translation_unit(&mut self) -> Result<TranslationUnit, Diagnostic> {
        let mut functions = Vec::new();
        while self.current().kind != TokenKind::Eof {
            functions.push(self.parse_function()?);
        }
        Ok(TranslationUnit { functions })
    }

    fn parse_function(&mut self) -> Result<FuncDef, Diagnostic> {
        if let Some(message) = unsupported(self.current().kind) {
            return Err(self.error(message));
        }
        let ret_type = self.parse_func_type()?;
        if self.current().kind == TokenKind::Star {
            return Err(self.error("pointers are not supported"));
        }
        if let Some(message) = unsupported(self.current().kind) {
            return Err(self.error(message));
        }
        let name = self.expect(TokenKind::Ident, "an identifier")?;
        if self.current().kind != TokenKind::LParen {
            return Err(self.error("global variables are not supported"));
        }
        let params = self.parse_params()?;
        if self.current().kind == TokenKind::Semi {
            return Err(self.error("function declarations without a body are not supported"));
        }
        let body = self.parse_compound()?;
        Ok(FuncDef {
            ret_type,
            name: name.spelling,
            params,
            body,
            line: name.line,
            column: name.column,
        })
    }

    fn parse_func_type(&mut self) -> Result<TypeName, Diagnostic> {
        let token = self.current().clone();
        match token.kind {
            TokenKind::KwInt => {
                self.advance();
                Ok(TypeName::Int)
            }
            TokenKind::KwVoid => {
                self.advance();
                Ok(TypeName::Void)
            }
            kind => {
                if let Some(message) = unsupported(kind) {
                    Err(self.error(message))
                } else {
                    Err(self.error(format!("expected a type, found {}", found(&token))))
                }
            }
        }
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, Diagnostic> {
        self.expect(TokenKind::LParen, "'('")?;
        if self.match_kind(TokenKind::RParen) {
            return Ok(Vec::new());
        }
        if self.current().kind == TokenKind::KwVoid && self.lookahead() == TokenKind::RParen {
            self.advance();
            self.advance();
            return Ok(Vec::new());
        }
        let mut params = Vec::new();
        loop {
            params.push(self.parse_param()?);
            if self.match_kind(TokenKind::Comma) {
                if self.current().kind == TokenKind::RParen {
                    return Err(self.error("trailing commas in parameter lists are not supported"));
                }
                continue;
            }
            break;
        }
        self.expect(TokenKind::RParen, "')'")?;
        Ok(params)
    }

    fn parse_param(&mut self) -> Result<Param, Diagnostic> {
        let token = self.current().clone();
        if token.kind == TokenKind::Ellipsis {
            return Err(self.error("variadic functions are not supported"));
        }
        if token.kind != TokenKind::KwInt {
            if let Some(message) = unsupported(token.kind) {
                return Err(self.error(message));
            }
            if token.kind == TokenKind::KwVoid {
                if matches!(
                    self.lookahead(),
                    TokenKind::Ident | TokenKind::Star | TokenKind::LBrack
                ) {
                    return Err(self.error("'void' must be the only unnamed parameter"));
                }
                return Err(self.error(format!("expected ')', found {}", found(&token))));
            }
            return Err(self.error(format!(
                "expected a parameter type, found {}",
                found(&token)
            )));
        }
        self.advance();
        if self.current().kind == TokenKind::Star {
            return Err(self.error("pointers are not supported"));
        }
        let name = self.expect(TokenKind::Ident, "an identifier")?;
        if self.current().kind == TokenKind::LBrack {
            return Err(self.error("arrays are not supported"));
        }
        Ok(Param {
            name: name.spelling,
            line: name.line,
            column: name.column,
            slot: String::new(),
            arg_reg: String::new(),
        })
    }

    fn parse_compound(&mut self) -> Result<Compound, Diagnostic> {
        let brace = self.expect(TokenKind::LBrace, "'{'")?;
        let mut items = Vec::new();
        while !matches!(self.current().kind, TokenKind::RBrace | TokenKind::Eof) {
            if self.current().kind == TokenKind::KwInt {
                for decl in self.parse_declaration()? {
                    items.push(Stmt::Decl(decl));
                }
            } else if self.current().kind == TokenKind::KwVoid {
                return Err(self.error("void variables are not supported"));
            } else if let Some(message) = unsupported(self.current().kind) {
                return Err(self.error(message));
            } else {
                items.push(self.parse_statement()?);
            }
        }
        self.expect(TokenKind::RBrace, "'}'")?;
        Ok(Compound {
            items,
            line: brace.line,
            column: brace.column,
        })
    }

    fn parse_declaration(&mut self) -> Result<Vec<VarDecl>, Diagnostic> {
        self.expect(TokenKind::KwInt, "'int'")?;
        let mut decls = Vec::new();
        loop {
            if self.current().kind == TokenKind::Star {
                return Err(self.error("pointers are not supported"));
            }
            let name = self.expect(TokenKind::Ident, "an identifier")?;
            if self.current().kind == TokenKind::LParen {
                return Err(self.error("nested functions are not supported"));
            }
            if self.current().kind == TokenKind::LBrack {
                return Err(self.error("arrays are not supported"));
            }
            let init = if self.match_kind(TokenKind::Assign) {
                Some(self.parse_assignment()?)
            } else {
                None
            };
            decls.push(VarDecl {
                name: name.spelling,
                init,
                line: name.line,
                column: name.column,
                slot: String::new(),
            });
            if self.match_kind(TokenKind::Comma) {
                continue;
            }
            break;
        }
        self.expect(TokenKind::Semi, "';'")?;
        Ok(decls)
    }

    fn parse_statement(&mut self) -> Result<Stmt, Diagnostic> {
        match self.current().kind {
            TokenKind::LBrace => Ok(Stmt::Compound(self.parse_compound()?)),
            TokenKind::KwReturn => self.parse_return(),
            TokenKind::KwIf => self.parse_if(),
            TokenKind::KwWhile => self.parse_while(),
            TokenKind::KwFor => self.parse_for(),
            TokenKind::KwBreak => {
                let token = self.advance();
                self.expect(TokenKind::Semi, "';'")?;
                Ok(Stmt::Break {
                    line: token.line,
                    column: token.column,
                })
            }
            TokenKind::KwContinue => {
                let token = self.advance();
                self.expect(TokenKind::Semi, "';'")?;
                Ok(Stmt::Continue {
                    line: token.line,
                    column: token.column,
                })
            }
            TokenKind::KwElse => Err(self.error("'else' without a matching 'if'")),
            TokenKind::KwInt => Err(self.error("a declaration is not allowed as a statement")),
            TokenKind::Semi => {
                let token = self.advance();
                Ok(Stmt::Expr {
                    expr: None,
                    line: token.line,
                    column: token.column,
                })
            }
            kind => {
                if let Some(message) = unsupported(kind) {
                    return Err(self.error(message));
                }
                let line = self.current().line;
                let column = self.current().column;
                let expr = self.parse_expression()?;
                self.expect(TokenKind::Semi, "';'")?;
                Ok(Stmt::Expr {
                    expr: Some(expr),
                    line,
                    column,
                })
            }
        }
    }

    fn parse_return(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        let expr = if self.current().kind == TokenKind::Semi {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(TokenKind::Semi, "';'")?;
        Ok(Stmt::Return {
            expr,
            line: token.line,
            column: token.column,
        })
    }

    fn parse_if(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        self.expect(TokenKind::LParen, "'('")?;
        let cond = self.parse_expression()?;
        self.expect(TokenKind::RParen, "')'")?;
        let then_stmt = Box::new(self.parse_statement()?);
        let else_stmt = if self.match_kind(TokenKind::KwElse) {
            Some(Box::new(self.parse_statement()?))
        } else {
            None
        };
        Ok(Stmt::If {
            cond,
            then_stmt,
            else_stmt,
            line: token.line,
            column: token.column,
        })
    }

    fn parse_while(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        self.expect(TokenKind::LParen, "'('")?;
        let cond = self.parse_expression()?;
        self.expect(TokenKind::RParen, "')'")?;
        let body = Box::new(self.parse_statement()?);
        Ok(Stmt::While {
            cond,
            body,
            line: token.line,
            column: token.column,
        })
    }

    fn parse_for(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        self.expect(TokenKind::LParen, "'('")?;
        let init = if self.match_kind(TokenKind::Semi) {
            ForInit::Empty
        } else if self.current().kind == TokenKind::KwInt {
            ForInit::Decl(self.parse_declaration()?)
        } else if self.current().kind == TokenKind::KwVoid {
            return Err(self.error("void variables are not supported"));
        } else if let Some(message) = unsupported(self.current().kind) {
            return Err(self.error(message));
        } else {
            let expr = self.parse_expression()?;
            self.expect(TokenKind::Semi, "';'")?;
            ForInit::Expr(expr)
        };
        let cond = if self.current().kind == TokenKind::Semi {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(TokenKind::Semi, "';'")?;
        let step = if self.current().kind == TokenKind::RParen {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(TokenKind::RParen, "')'")?;
        let body = Box::new(self.parse_statement()?);
        Ok(Stmt::For {
            init,
            cond,
            step,
            body,
            line: token.line,
            column: token.column,
        })
    }

    fn parse_expression(&mut self) -> Result<Expr, Diagnostic> {
        let expr = self.parse_assignment()?;
        if self.current().kind == TokenKind::Comma {
            return Err(self.error("the comma operator is not supported"));
        }
        Ok(expr)
    }

    fn parse_assignment(&mut self) -> Result<Expr, Diagnostic> {
        let left = self.parse_logical_or()?;
        if self.match_kind(TokenKind::Assign) {
            let line = left.line();
            let column = left.column();
            let value = self.parse_assignment()?;
            return Ok(Expr::assign(left, value, line, column));
        }
        self.reject_unsupported_infix()?;
        Ok(left)
    }

    fn reject_unsupported_infix(&self) -> Result<(), Diagnostic> {
        let message = match self.current().kind {
            TokenKind::Amp => "bitwise AND is not supported",
            TokenKind::Pipe => "bitwise OR is not supported",
            TokenKind::Caret => "bitwise XOR is not supported",
            TokenKind::Tilde => "bitwise complement is not supported",
            TokenKind::Shl | TokenKind::Shr => "shift operators are not supported",
            TokenKind::PlusEq
            | TokenKind::MinusEq
            | TokenKind::StarEq
            | TokenKind::SlashEq
            | TokenKind::PercentEq
            | TokenKind::AmpEq
            | TokenKind::PipeEq
            | TokenKind::CaretEq => "compound assignment is not supported",
            TokenKind::Question => "the conditional operator is not supported",
            _ => return Ok(()),
        };
        Err(self.error(message))
    }

    fn parse_logical_or(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_logical_and()?;
        while self.current().kind == TokenKind::OrOr {
            let op = self.advance();
            let right = self.parse_logical_and()?;
            left = Expr::bin(BinOp::Or, left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_logical_and(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_equality()?;
        while self.current().kind == TokenKind::AndAnd {
            let op = self.advance();
            let right = self.parse_equality()?;
            left = Expr::bin(BinOp::And, left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_relational()?;
        while matches!(self.current().kind, TokenKind::Eq | TokenKind::Ne) {
            let op = self.advance();
            let kind = if op.kind == TokenKind::Eq {
                BinOp::Eq
            } else {
                BinOp::Ne
            };
            let right = self.parse_relational()?;
            left = Expr::bin(kind, left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_relational(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_additive()?;
        while matches!(
            self.current().kind,
            TokenKind::Lt | TokenKind::Gt | TokenKind::Le | TokenKind::Ge
        ) {
            let op = self.advance();
            let kind = match op.kind {
                TokenKind::Lt => BinOp::Lt,
                TokenKind::Gt => BinOp::Gt,
                TokenKind::Le => BinOp::Le,
                _ => BinOp::Ge,
            };
            let right = self.parse_additive()?;
            left = Expr::bin(kind, left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_multiplicative()?;
        while matches!(self.current().kind, TokenKind::Plus | TokenKind::Minus) {
            let op = self.advance();
            let kind = if op.kind == TokenKind::Plus {
                BinOp::Add
            } else {
                BinOp::Sub
            };
            let right = self.parse_multiplicative()?;
            left = Expr::bin(kind, left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_unary()?;
        while matches!(
            self.current().kind,
            TokenKind::Star | TokenKind::Slash | TokenKind::Percent
        ) {
            let op = self.advance();
            let kind = match op.kind {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                _ => BinOp::Mod,
            };
            let right = self.parse_unary()?;
            left = Expr::bin(kind, left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.current().clone();
        match token.kind {
            TokenKind::Plus | TokenKind::Minus | TokenKind::Bang => {
                self.advance();
                let op = match token.kind {
                    TokenKind::Plus => UnaryOp::Plus,
                    TokenKind::Minus => UnaryOp::Minus,
                    _ => UnaryOp::Not,
                };
                let expr = self.parse_unary()?;
                Ok(Expr::unary(op, expr, token.line, token.column))
            }
            TokenKind::PlusPlus | TokenKind::MinusMinus => {
                Err(self.error("increment and decrement operators are not supported"))
            }
            TokenKind::Amp => Err(self.error("the address-of operator is not supported")),
            TokenKind::Star => Err(self.error("pointer dereference is not supported")),
            TokenKind::Tilde => Err(self.error("bitwise complement is not supported")),
            TokenKind::KwSizeof => Err(self.error("sizeof is not supported")),
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.current().kind {
                TokenKind::LParen => {
                    let line = expr.line();
                    let column = expr.column();
                    self.advance();
                    let mut args = Vec::new();
                    if self.current().kind != TokenKind::RParen {
                        loop {
                            args.push(self.parse_assignment()?);
                            if !self.match_kind(TokenKind::Comma) {
                                break;
                            }
                            if self.current().kind == TokenKind::RParen {
                                return Err(self
                                    .error("trailing commas in argument lists are not supported"));
                            }
                        }
                    }
                    self.expect(TokenKind::RParen, "')'")?;
                    expr = Expr::call(expr, args, line, column);
                }
                TokenKind::LBrack => return Err(self.error("arrays are not supported")),
                TokenKind::PlusPlus | TokenKind::MinusMinus => {
                    return Err(self.error("increment and decrement operators are not supported"));
                }
                TokenKind::Arrow => {
                    return Err(self.error("pointer member access is not supported"));
                }
                TokenKind::Dot => return Err(self.error("member access is not supported")),
                TokenKind::Ellipsis => {
                    return Err(self.error("variadic functions are not supported"));
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.current().clone();
        match token.kind {
            TokenKind::Int => {
                self.advance();
                Ok(Expr::int(
                    token.value.expect("int token"),
                    token.line,
                    token.column,
                ))
            }
            TokenKind::Ident => {
                self.advance();
                Ok(Expr::var(token.spelling, token.line, token.column))
            }
            TokenKind::LParen => {
                let next = self.lookahead();
                if matches!(next, TokenKind::KwInt | TokenKind::KwVoid)
                    || unsupported(next).is_some()
                {
                    return Err(self.error("casts are not supported"));
                }
                self.advance();
                let expr = self.parse_expression()?;
                self.expect(TokenKind::RParen, "')'")?;
                Ok(expr)
            }
            kind => {
                if let Some(message) = unsupported(kind) {
                    return Err(self.error(message));
                }
                Err(self.error(format!("expected an expression, found {}", found(&token))))
            }
        }
    }

    fn current(&self) -> &Token {
        &self.tokens[self.index.min(self.tokens.len().saturating_sub(1))]
    }

    fn lookahead(&self) -> TokenKind {
        self.tokens
            .get(self.index + 1)
            .map(|token| token.kind)
            .unwrap_or(TokenKind::Eof)
    }

    fn advance(&mut self) -> Token {
        let token = self.current().clone();
        if token.kind != TokenKind::Eof && self.index + 1 < self.tokens.len() {
            self.index += 1;
        }
        token
    }

    fn match_kind(&mut self, kind: TokenKind) -> bool {
        if self.current().kind == kind {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: TokenKind, what: &str) -> Result<Token, Diagnostic> {
        if self.current().kind != kind {
            return Err(self.error(format!("expected {what}, found {}", found(self.current()))));
        }
        Ok(self.advance())
    }

    fn error(&self, message: impl Into<String>) -> Diagnostic {
        let token = self.current();
        Diagnostic::new(self.filename, token.line, token.column, message)
    }
}

fn found(token: &Token) -> String {
    if token.kind == TokenKind::Eof {
        "end of file".to_string()
    } else {
        format!("'{}'", token.spelling)
    }
}

fn unsupported(kind: TokenKind) -> Option<&'static str> {
    Some(match kind {
        TokenKind::KwChar => "type 'char' is not supported",
        TokenKind::KwShort => "type 'short' is not supported",
        TokenKind::KwLong => "type 'long' is not supported",
        TokenKind::KwFloat => "type 'float' is not supported",
        TokenKind::KwDouble => "type 'double' is not supported",
        TokenKind::KwSigned => "signed type specifiers are not supported",
        TokenKind::KwUnsigned => "unsigned types are not supported",
        TokenKind::KwStruct => "structs are not supported",
        TokenKind::KwUnion => "unions are not supported",
        TokenKind::KwEnum => "enums are not supported",
        TokenKind::KwTypedef => "typedef is not supported",
        TokenKind::KwConst | TokenKind::KwVolatile | TokenKind::KwRestrict => {
            "type qualifiers are not supported"
        }
        TokenKind::KwStatic => "static storage is not supported",
        TokenKind::KwExtern => "extern declarations are not supported",
        TokenKind::KwRegister => "register is not supported",
        TokenKind::KwAuto => "auto is not supported",
        TokenKind::KwInline => "inline is not supported",
        TokenKind::KwSizeof => "sizeof is not supported",
        TokenKind::KwSwitch | TokenKind::KwCase | TokenKind::KwDefault => "switch is not supported",
        TokenKind::KwDo => "do-while loops are not supported",
        TokenKind::KwGoto => "goto is not supported",
        TokenKind::KwBool => "type '_Bool' is not supported",
        TokenKind::KwAlignas | TokenKind::KwAlignof => "alignment specifiers are not supported",
        TokenKind::KwAtomic => "atomic types are not supported",
        TokenKind::KwComplex | TokenKind::KwImaginary => "complex types are not supported",
        TokenKind::KwGeneric => "_Generic is not supported",
        TokenKind::KwNoreturn => "_Noreturn is not supported",
        TokenKind::KwStaticAssert => "_Static_assert is not supported",
        TokenKind::KwThreadLocal => "thread-local storage is not supported",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn parsed(source: &str) -> String {
        let tokens = lex(source, "t.c").unwrap();
        dump(&parse(&tokens, "t.c").unwrap())
    }

    fn parse_err(source: &str) -> String {
        let tokens = match lex(source, "t.c") {
            Ok(tokens) => tokens,
            Err(err) => return err.to_string(),
        };
        parse(&tokens, "t.c").unwrap_err().to_string()
    }

    #[test]
    fn return_constant_ast() {
        assert_eq!(
            parsed("int main(void) { return 42; }\n"),
            "\
func int main
  block
    return
      int 42
"
        );
    }

    #[test]
    fn precedence_and_associativity() {
        assert_eq!(
            parsed("int main(void) { return 1 + 2 * 3; }\n"),
            "\
func int main
  block
    return
      +
        int 1
        *
          int 2
          int 3
"
        );
        assert_eq!(
            parsed("int main(void) { return 8 - 2 - 1; }\n"),
            "\
func int main
  block
    return
      -
        -
          int 8
          int 2
        int 1
"
        );
        assert_eq!(
            parsed("int main(void) { a = b = 1; }\n"),
            "\
func int main
  block
    expr
      assign
        var a
        assign
          var b
          int 1
"
        );
    }

    #[test]
    fn dangling_else_binds_inward() {
        let source = "\
int main(void) {
    if (c)
        if (d)
            a;
        else
            b;
}
";
        assert_eq!(
            parsed(source),
            "\
func int main
  block
    if
      cond
        var c
      then
        if
          cond
            var d
          then
            expr
              var a
          else
            expr
              var b
"
        );
    }

    #[test]
    fn for_loop_call_and_empty_parameter_list() {
        assert_eq!(
            parsed(
                "\
int add(int a, int b) { return a + b; }
int main() { return add(19, 23); }
"
            ),
            "\
func int add
  param a
  param b
  block
    return
      +
        var a
        var b
func int main
  block
    return
      call add
        int 19
        int 23
"
        );
        assert_eq!(
            parsed(
                "\
int main(void) {
    int sum = 0;
    for (int i = 1; i <= 10; i = i + 1)
        sum = sum + i;
    return sum;
}
"
            ),
            "\
func int main
  block
    decl sum =
      int 0
    for
      init
        decl i =
          int 1
      cond
        <=
          var i
          int 10
      step
        assign
          var i
          +
            var i
            int 1
      body
        expr
          assign
            var sum
            +
              var sum
              var i
    return
      var sum
"
        );
    }

    #[test]
    fn logical_precedence_and_unary() {
        assert_eq!(
            parsed("int main(void) { return 0 || 1 && 0; }\n"),
            "\
func int main
  block
    return
      ||
        int 0
        &&
          int 1
          int 0
"
        );
        assert_eq!(
            parsed("int main(void) { return -2 + !3; }\n"),
            "\
func int main
  block
    return
      +
        neg
          int 2
        not
          int 3
"
        );
    }

    #[test]
    fn parentheses_do_not_hide_calls() {
        assert_eq!(
            parsed("int main(void) { return (add)(1, 2); }\n"),
            "\
func int main
  block
    return
      call add
        int 1
        int 2
"
        );
    }

    #[test]
    fn syntax_errors_name_the_problem() {
        assert!(parse_err("int main(void) { return 1 }").contains("expected ';'"));
        assert!(parse_err("int main(void { return 0; }").contains("expected ')'"));
        assert!(parse_err("int g; int main(void) { return 0; }").contains("global variables"));
        assert!(parse_err("int main(void) { int *p; return 0; }")
            .contains("pointers are not supported"));
        assert!(parse_err("int main(void) { int a[2]; return 0; }")
            .contains("arrays are not supported"));
        assert!(parse_err("int main(void) { char c; return 0; }")
            .contains("type 'char' is not supported"));
        assert!(parse_err("struct S { int x; };").contains("structs are not supported"));
        assert!(parse_err("int main(void) { return 1 & 2; }").contains("bitwise AND"));
        assert!(parse_err("int main(void) { x++; }").contains("increment and decrement"));
        assert!(
            parse_err("int main(void) { return sizeof(int); }").contains("sizeof is not supported")
        );
        assert!(parse_err("int main(void) { return 1 ? 2 : 3; }").contains("conditional operator"));
        assert!(parse_err("int main(void) { return (1, 2); }").contains("comma operator"));
        assert!(parse_err("int add(int a, int b);").contains("without a body"));
        assert!(
            parse_err("int main(void) { int inner(void) { return 1; } }")
                .contains("nested functions")
        );
        assert!(parse_err("int main(void) { switch (1) {} }").contains("switch is not supported"));
        assert!(parse_err("int main(void) { do {} while (0); }").contains("do-while"));
        assert!(parse_err("int main(void) { else; }").contains("'else' without a matching 'if'"));
        assert!(parse_err("int *f(void) { return 0; }").contains("pointers are not supported"));
        assert!(parse_err("int main(void) { return (int)1; }").contains("casts are not supported"));
        assert!(parse_err("int f(int a, ...) { return a; }").contains("variadic"));
    }

    #[test]
    fn smoke_sources_parse() {
        for name in [
            "tests/01_return_const.c",
            "tests/02_add.c",
            "tests/03_loop.c",
            "tests/04_call.c",
        ] {
            let path = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), name);
            let source = std::fs::read_to_string(&path).unwrap();
            let tokens = lex(&source, name).unwrap();
            parse(&tokens, name).unwrap_or_else(|err| panic!("{name}: {err}"));
        }
    }
}
