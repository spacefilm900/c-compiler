//! Statements and blocks (C11 6.8).

use super::*;

impl Parser<'_> {
    pub(super) fn parse_compound(&mut self) -> Result<Compound, Diagnostic> {
        self.parse_compound_scoped(true)
    }

    /// `new_scope` is false for a function body, which shares its tag scope
    /// with the parameter list.
    pub(super) fn parse_compound_scoped(
        &mut self,
        new_scope: bool,
    ) -> Result<Compound, Diagnostic> {
        let brace = self.expect(TokenKind::LBrace, "'{'")?;
        if new_scope {
            self.push_tags();
        }
        let parsed = self.parse_compound_items(brace.line, brace.column);
        if new_scope {
            self.pop_tags();
        }
        parsed
    }

    pub(super) fn parse_compound_items(
        &mut self,
        line: usize,
        column: usize,
    ) -> Result<Compound, Diagnostic> {
        let mut items = Vec::new();
        while !matches!(self.current().kind, TokenKind::RBrace | TokenKind::Eof) {
            // `T:` is a label when `T` is a typedef (C11 6.2.3). A declaration
            // test would consume `T` as the type and then reject the colon.
            let is_label =
                self.current().kind == TokenKind::Ident && self.lookahead() == TokenKind::Colon;
            if self.current().kind == TokenKind::KwStaticAssert {
                self.parse_static_assert()?;
            } else if let Some(stmt) = self.parse_attribute_statement()? {
                items.push(stmt);
            } else if !is_label && self.starts_declaration() {
                items.extend(self.parse_block_declaration(false)?);
            } else if let Some(message) = unsupported(self.current().kind) {
                return Err(self.error(message));
            } else {
                items.push(self.parse_statement()?);
            }
        }
        self.expect_skip(TokenKind::RBrace, "'}'")?;
        Ok(Compound {
            items,
            line,
            column,
        })
    }

    /// `__attribute__((fallthrough));` and other attributes on a null
    /// statement (GCC manual, "Statement Attributes"). `None` when the
    /// attributes start a declaration instead.
    pub(super) fn parse_attribute_statement(&mut self) -> Result<Option<Stmt>, Diagnostic> {
        if !self.at_attribute() {
            return Ok(None);
        }
        let mut index = self.index;
        while self.tokens.get(index).is_some_and(|tok| {
            tok.kind == TokenKind::Ident
                && matches!(tok.spelling.as_str(), "__attribute__" | "__attribute")
        }) {
            index += 1;
            if self.tokens.get(index).map(|tok| tok.kind) != Some(TokenKind::LParen) {
                return Ok(None);
            }
            let mut depth = 0usize;
            while let Some(tok) = self.tokens.get(index) {
                match tok.kind {
                    TokenKind::LParen => depth += 1,
                    TokenKind::RParen => {
                        depth -= 1;
                        if depth == 0 {
                            index += 1;
                            break;
                        }
                    }
                    TokenKind::Eof => return Ok(None),
                    _ => {}
                }
                index += 1;
            }
        }
        if self.tokens.get(index).map(|tok| tok.kind) != Some(TokenKind::Semi) {
            return Ok(None);
        }
        self.parse_attributes_into()?;
        let semi = self.advance();
        Ok(Some(Stmt::Expr {
            expr: None,
            line: semi.line,
            column: semi.column,
        }))
    }

    pub(super) fn parse_statement(&mut self) -> Result<Stmt, Diagnostic> {
        // `name:` is a label even when `name` is a typedef. The check comes
        // before the declaration test so `typedef int T; T: ;` is a label.
        if self.current().kind == TokenKind::Ident && self.lookahead() == TokenKind::Colon {
            return self.parse_label();
        }
        if let Some(stmt) = self.parse_attribute_statement()? {
            return Ok(stmt);
        }
        // A label's statement is parsed here, including `case 1: int x;`.
        // A declaration is not a statement in C11 (C23 allows the label).
        if self.starts_declaration() {
            return Err(self.error("a declaration is not allowed as a statement"));
        }
        if self.spec_word(&["__asm__", "__asm"]) || self.dialect.gnu() && self.spec_word(&["asm"]) {
            return self.parse_asm_stmt();
        }
        match self.current().kind {
            TokenKind::LBrace => Ok(Stmt::Compound(self.parse_compound()?)),
            TokenKind::KwReturn => self.parse_return(),
            TokenKind::KwIf => self.parse_if(),
            TokenKind::KwWhile => self.parse_while(),
            TokenKind::KwDo => self.parse_do(),
            TokenKind::KwFor => self.parse_for(),
            TokenKind::KwSwitch => self.parse_switch(),
            TokenKind::KwCase => self.parse_case(),
            TokenKind::KwDefault => self.parse_default(),
            TokenKind::KwGoto => self.parse_goto(),
            TokenKind::KwBreak => {
                let token = self.advance();
                self.expect_skip(TokenKind::Semi, "';'")?;
                Ok(Stmt::Break {
                    line: token.line,
                    column: token.column,
                })
            }
            TokenKind::KwContinue => {
                let token = self.advance();
                self.expect_skip(TokenKind::Semi, "';'")?;
                Ok(Stmt::Continue {
                    line: token.line,
                    column: token.column,
                })
            }
            TokenKind::KwElse => Err(self.error("'else' without a matching 'if'")),
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
                self.expect_skip(TokenKind::Semi, "';'")?;
                Ok(Stmt::Expr {
                    expr: Some(expr),
                    line,
                    column,
                })
            }
        }
    }

    pub(super) fn parse_label(&mut self) -> Result<Stmt, Diagnostic> {
        let name = self.advance();
        self.expect_skip(TokenKind::Colon, "':'")?;
        // Label attributes, such as `unused`, are accepted and ignored.
        self.parse_attributes_into()?;
        let stmt = Box::new(self.parse_statement()?);
        Ok(Stmt::Label {
            name: name.spelling,
            stmt,
            line: name.line,
            column: name.column,
        })
    }

    pub(super) fn parse_goto(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        if self.current().kind == TokenKind::Star {
            // `goto *expr;` (GCC manual, "Labels as Values").
            let star = self.advance();
            if !self.dialect.gnu() {
                return Err(self.error_at(
                    star.line,
                    star.column,
                    "computed goto is a GNU extension; use -std=gnu11",
                ));
            }
            let expr = self.parse_expression()?;
            self.expect_skip(TokenKind::Semi, "';'")?;
            return Ok(Stmt::GotoPtr {
                expr,
                line: token.line,
                column: token.column,
            });
        }
        if self.current().kind != TokenKind::Ident {
            return Err(self.error("expected a label name"));
        }
        let name = self.advance();
        self.expect_skip(TokenKind::Semi, "';'")?;
        Ok(Stmt::Goto {
            name: name.spelling,
            line: token.line,
            column: token.column,
        })
    }

    pub(super) fn parse_return(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        let expr = if self.current().kind == TokenKind::Semi {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect_skip(TokenKind::Semi, "';'")?;
        Ok(Stmt::Return {
            expr,
            line: token.line,
            column: token.column,
        })
    }

    pub(super) fn parse_if(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        self.expect_skip(TokenKind::LParen, "'('")?;
        let cond = self.parse_expression()?;
        self.expect_skip(TokenKind::RParen, "')'")?;
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

    pub(super) fn parse_while(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        self.expect_skip(TokenKind::LParen, "'('")?;
        let cond = self.parse_expression()?;
        self.expect_skip(TokenKind::RParen, "')'")?;
        let body = Box::new(self.parse_statement()?);
        Ok(Stmt::While {
            cond,
            body,
            line: token.line,
            column: token.column,
        })
    }

    pub(super) fn parse_do(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        let body = Box::new(self.parse_statement()?);
        self.expect_skip(TokenKind::KwWhile, "'while'")?;
        self.expect_skip(TokenKind::LParen, "'('")?;
        let cond = self.parse_expression()?;
        self.expect_skip(TokenKind::RParen, "')'")?;
        self.expect_skip(TokenKind::Semi, "';'")?;
        Ok(Stmt::Do {
            body,
            cond,
            line: token.line,
            column: token.column,
        })
    }

    pub(super) fn parse_for(&mut self) -> Result<Stmt, Diagnostic> {
        self.push_tags();
        let parsed = self.parse_for_in_scope();
        self.pop_tags();
        parsed
    }

    pub(super) fn parse_for_in_scope(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        self.expect_skip(TokenKind::LParen, "'('")?;
        let init = if self.match_kind(TokenKind::Semi) {
            ForInit::Empty
        } else if self.starts_declaration() {
            let mut decls = Vec::new();
            for item in self.parse_block_declaration(true)? {
                match item {
                    Stmt::Decl(decl) => decls.push(decl),
                    Stmt::FuncDecl(_) => {
                        unreachable!("a function declaration in a for loop is rejected")
                    }
                    _ => unreachable!("a block declaration is a variable or a function"),
                }
            }
            ForInit::Decl(decls)
        } else if let Some(message) = unsupported(self.current().kind) {
            return Err(self.error(message));
        } else {
            let expr = self.parse_expression()?;
            self.expect_skip(TokenKind::Semi, "';'")?;
            ForInit::Expr(expr)
        };
        let cond = if self.current().kind == TokenKind::Semi {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect_skip(TokenKind::Semi, "';'")?;
        let step = if self.current().kind == TokenKind::RParen {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect_skip(TokenKind::RParen, "')'")?;
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

    /// `switch (` expression `)` statement. The controlling expression is a
    /// full expression, so it may contain a comma.
    pub(super) fn parse_switch(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        self.expect_skip(TokenKind::LParen, "'('")?;
        let cond = self.parse_expression()?;
        self.expect_skip(TokenKind::RParen, "')'")?;
        let body = Box::new(self.parse_statement()?);
        Ok(Stmt::Switch {
            cond,
            body,
            promoted: None,
            line: token.line,
            column: token.column,
        })
    }

    /// `case` constant-expression `:` statement.
    ///
    /// The constant is a conditional-expression, not an assignment and not a
    /// comma expression (C11 6.6). `case 1: case 2: stmt` is one labeled
    /// statement whose statement is another labeled statement.
    pub(super) fn parse_case(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        let expr = self.parse_conditional()?;
        // `case low ... high:` (GCC manual, "Case Ranges").
        let high = if self.current().kind == TokenKind::Ellipsis {
            let dots = self.advance();
            if !self.dialect.gnu() {
                return Err(self.error_at(
                    dots.line,
                    dots.column,
                    "case ranges are a GNU extension; use -std=gnu11",
                ));
            }
            Some(self.parse_conditional()?)
        } else {
            None
        };
        self.expect_skip(TokenKind::Colon, "':'")?;
        let stmt = Box::new(self.parse_statement()?);
        Ok(Stmt::Case {
            expr,
            value: None,
            high,
            high_value: None,
            stmt,
            line: token.line,
            column: token.column,
        })
    }

    pub(super) fn parse_default(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        self.expect_skip(TokenKind::Colon, "':'")?;
        let stmt = Box::new(self.parse_statement()?);
        Ok(Stmt::Default {
            stmt,
            line: token.line,
            column: token.column,
        })
    }
}

impl Parser<'_> {
    /// `asm qualifiers ( template [: outputs [: inputs [: clobbers]]] ) ;`
    /// (GCC manual, "Basic Asm" and "Extended Asm"). Operands and `asm goto`
    /// are not supported; a template without operands is passed to LLVM.
    fn parse_asm_stmt(&mut self) -> Result<Stmt, Diagnostic> {
        let kw = self.advance();
        loop {
            if self.current().kind == TokenKind::KwVolatile
                || self.current().kind == TokenKind::KwInline
                || self.spec_word(&["__volatile__", "__volatile", "__inline__", "__inline"])
            {
                self.bump();
                continue;
            }
            if self.current().kind == TokenKind::KwGoto {
                return Err(self.error("asm goto is not supported"));
            }
            break;
        }
        self.expect_skip(TokenKind::LParen, "'('")?;
        let raw = self.parse_asm_string("an asm template")?;
        let mut extended = false;
        let mut clobbers = Vec::new();
        let mut sections = 0;
        while self.current().kind == TokenKind::Colon {
            self.bump();
            extended = true;
            sections += 1;
            if sections <= 2 {
                if !matches!(self.current().kind, TokenKind::Colon | TokenKind::RParen) {
                    return Err(self.error("asm operands are not supported"));
                }
                continue;
            }
            if sections == 3 {
                while self.current().kind == TokenKind::String {
                    clobbers.push(
                        String::from_utf8_lossy(&self.parse_asm_string("a clobber")?).into_owned(),
                    );
                    if !self.match_kind(TokenKind::Comma) {
                        break;
                    }
                }
                continue;
            }
            return Err(self.error("asm goto labels are not supported"));
        }
        self.expect_skip(TokenKind::RParen, "')'")?;
        self.expect_skip(TokenKind::Semi, "';'")?;
        let mut template = Vec::with_capacity(raw.len());
        let mut bytes = raw.iter().copied().peekable();
        while let Some(byte) = bytes.next() {
            match byte {
                b'$' => template.extend_from_slice(b"$$"),
                b'%' if extended => match bytes.next() {
                    Some(b'%') => template.push(b'%'),
                    _ => {
                        return Err(self.error_at(
                            kw.line,
                            kw.column,
                            "asm template operand references are not supported",
                        ))
                    }
                },
                _ => template.push(byte),
            }
        }
        Ok(Stmt::Asm {
            template,
            clobbers,
            line: kw.line,
            column: kw.column,
        })
    }

    fn parse_asm_string(&mut self, what: &str) -> Result<Vec<u8>, Diagnostic> {
        if self.current().kind != TokenKind::String {
            return Err(self.error(format!("expected a string literal for {what}")));
        }
        let mut bytes = Vec::new();
        while self.current().kind == TokenKind::String {
            let tok = self.advance();
            if !matches!(lit_prefix(&tok.spelling), LitPrefix::None | LitPrefix::U8) {
                return Err(self.error_at(
                    tok.line,
                    tok.column,
                    format!("{what} must be a byte string literal"),
                ));
            }
            bytes.extend(tok.bytes.expect("string token"));
        }
        Ok(bytes)
    }
}
