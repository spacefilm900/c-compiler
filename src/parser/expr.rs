//! Expressions (C11 6.5): precedence climbing, unary and postfix operators,
//! initializers, `_Generic`, builtins folded at parse time and primary
//! expressions.

use super::*;

impl Parser<'_> {
    /// Comma operator. Argument lists and declarators call
    /// [`Self::parse_assignment`] instead, so those commas stay separators.
    pub(super) fn parse_expression(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_assignment()?;
        while self.current().kind == TokenKind::Comma {
            let op = self.advance();
            let right = self.parse_assignment()?;
            left = Expr::comma(left, right, op.line, op.column);
        }
        Ok(left)
    }

    pub(super) fn parse_assignment(&mut self) -> Result<Expr, Diagnostic> {
        let left = self.parse_conditional()?;
        let Some(kind) = assign_kind(self.current().kind) else {
            return Ok(left);
        };
        self.bump();
        let line = left.line();
        let column = left.column();
        let value = self.parse_assignment()?;
        Ok(match kind {
            AssignKind::Plain => Expr::assign(left, value, line, column),
            AssignKind::Compound(binop) => Expr::compound(binop, left, value, line, column),
        })
    }

    /// `?:` is right associative. The middle operand is a full expression,
    /// so it may contain a comma; the third operand may not.
    pub(super) fn parse_conditional(&mut self) -> Result<Expr, Diagnostic> {
        let cond = self.parse_logical_or()?;
        if !self.match_kind(TokenKind::Question) {
            return Ok(cond);
        }
        let line = cond.line();
        let column = cond.column();
        if self.current().kind == TokenKind::Colon {
            // `x ?: y` (GCC manual, "Conditionals with Omitted Operands"):
            // `x` is evaluated once and is the value when it is nonzero.
            let colon = self.advance();
            if !self.dialect.gnu() {
                return Err(self.error_at(
                    colon.line,
                    colon.column,
                    "a conditional with an omitted middle operand is a GNU extension; use -std=gnu11",
                ));
            }
            let else_expr = self.parse_conditional()?;
            return Ok(Expr::CondShort {
                cond: Box::new(cond),
                else_expr: Box::new(else_expr),
                line,
                column,
                ty: None,
            });
        }
        let then_expr = self.parse_expression()?;
        self.expect_skip(TokenKind::Colon, "':'")?;
        let else_expr = self.parse_conditional()?;
        Ok(Expr::cond(cond, then_expr, else_expr, line, column))
    }

    pub(super) fn parse_logical_or(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_logical_and()?;
        while self.current().kind == TokenKind::OrOr {
            let op = self.advance();
            let right = self.parse_logical_and()?;
            left = Expr::bin(BinOp::Or, left, right, op.line, op.column);
        }
        Ok(left)
    }

    pub(super) fn parse_logical_and(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_bit_or()?;
        while self.current().kind == TokenKind::AndAnd {
            let op = self.advance();
            let right = self.parse_bit_or()?;
            left = Expr::bin(BinOp::And, left, right, op.line, op.column);
        }
        Ok(left)
    }

    pub(super) fn parse_bit_or(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_bit_xor()?;
        while self.current().kind == TokenKind::Pipe {
            let op = self.advance();
            let right = self.parse_bit_xor()?;
            left = Expr::bin(BinOp::BitOr, left, right, op.line, op.column);
        }
        Ok(left)
    }

    pub(super) fn parse_bit_xor(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_bit_and()?;
        while self.current().kind == TokenKind::Caret {
            let op = self.advance();
            let right = self.parse_bit_and()?;
            left = Expr::bin(BinOp::BitXor, left, right, op.line, op.column);
        }
        Ok(left)
    }

    pub(super) fn parse_bit_and(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_equality()?;
        while self.current().kind == TokenKind::Amp {
            let op = self.advance();
            let right = self.parse_equality()?;
            left = Expr::bin(BinOp::BitAnd, left, right, op.line, op.column);
        }
        Ok(left)
    }

    pub(super) fn parse_equality(&mut self) -> Result<Expr, Diagnostic> {
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

    pub(super) fn parse_relational(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_shift()?;
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
            let right = self.parse_shift()?;
            left = Expr::bin(kind, left, right, op.line, op.column);
        }
        Ok(left)
    }

    pub(super) fn parse_shift(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_additive()?;
        while matches!(self.current().kind, TokenKind::Shl | TokenKind::Shr) {
            let op = self.advance();
            let kind = if op.kind == TokenKind::Shl {
                BinOp::Shl
            } else {
                BinOp::Shr
            };
            let right = self.parse_additive()?;
            left = Expr::bin(kind, left, right, op.line, op.column);
        }
        Ok(left)
    }

    pub(super) fn parse_additive(&mut self) -> Result<Expr, Diagnostic> {
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

    pub(super) fn parse_multiplicative(&mut self) -> Result<Expr, Diagnostic> {
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

    pub(super) fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        if self.spec_word(&["__extension__"]) {
            self.bump();
            return self.parse_unary();
        }
        let token = self.current().clone();
        match token.kind {
            TokenKind::LParen if self.lookahead_is_type() => self.parse_cast(),
            TokenKind::Plus | TokenKind::Minus | TokenKind::Bang | TokenKind::Tilde => {
                self.bump();
                let op = match token.kind {
                    TokenKind::Plus => UnaryOp::Plus,
                    TokenKind::Minus => UnaryOp::Minus,
                    TokenKind::Bang => UnaryOp::Not,
                    _ => UnaryOp::BitNot,
                };
                let expr = self.parse_unary()?;
                Ok(Expr::unary(op, expr, token.line, token.column))
            }
            TokenKind::PlusPlus | TokenKind::MinusMinus => {
                self.bump();
                let op = if token.kind == TokenKind::PlusPlus {
                    IncOp::PreInc
                } else {
                    IncOp::PreDec
                };
                let expr = self.parse_unary()?;
                Ok(Expr::incdec(op, expr, token.line, token.column))
            }
            TokenKind::KwSizeof => self.parse_sizeof(token.line, token.column),
            TokenKind::KwAlignof => self.parse_alignof(token.line, token.column),
            TokenKind::Amp => {
                self.bump();
                let expr = self.parse_unary()?;
                Ok(Expr::unary(UnaryOp::Addr, expr, token.line, token.column))
            }
            TokenKind::Star => {
                self.bump();
                let expr = self.parse_unary()?;
                Ok(Expr::unary(UnaryOp::Deref, expr, token.line, token.column))
            }
            // `&&label` (GCC manual, "Labels as Values"). As a unary
            // operator `&&` is not C11, so strict mode rejects it.
            TokenKind::AndAnd if self.lookahead() == TokenKind::Ident => {
                if !self.dialect.gnu() {
                    return Err(self.error_at(
                        token.line,
                        token.column,
                        "label addresses ('&&label') are a GNU extension; use -std=gnu11",
                    ));
                }
                self.bump();
                let name = self.advance();
                Ok(Expr::LabelAddr {
                    name: name.spelling,
                    func: None,
                    line: token.line,
                    column: token.column,
                    ty: None,
                })
            }
            _ => self.parse_postfix(),
        }
    }

    /// `=` initializer: an assignment expression, or a brace list. A trailing
    /// comma is allowed. An empty list is not (C11). Designators (`.name` and
    /// `[index]`) are only valid inside a brace list.
    pub(super) fn parse_initializer(&mut self) -> Result<Init, Diagnostic> {
        if self.current().kind != TokenKind::LBrace {
            return Ok(Init::Expr(self.parse_assignment()?));
        }
        let brace = self.advance();
        if self.current().kind == TokenKind::RBrace {
            if self.dialect.gnu() {
                // GNU C: `{}` zero-initializes the object, which `{ 0 }`
                // does for every type that has an initializer.
                let close = self.advance();
                return Ok(Init::List {
                    items: vec![Init::Expr(Expr::int(
                        0,
                        TypeName::Int,
                        close.line,
                        close.column,
                    ))],
                    line: brace.line,
                    column: brace.column,
                });
            }
            return Err(self.error("empty initializer list"));
        }
        let mut items = Vec::new();
        loop {
            items.push(self.parse_init_item()?);
            if self.match_kind(TokenKind::Comma) {
                if self.current().kind == TokenKind::RBrace {
                    break;
                }
                continue;
            }
            break;
        }
        self.expect_skip(TokenKind::RBrace, "'}'")?;
        Ok(Init::List {
            items,
            line: brace.line,
            column: brace.column,
        })
    }

    pub(super) fn parse_init_item(&mut self) -> Result<Init, Diagnostic> {
        if !matches!(self.current().kind, TokenKind::Dot | TokenKind::LBrack) {
            return self.parse_initializer();
        }
        let line = self.current().line;
        let column = self.current().column;
        let mut designators = Vec::new();
        while matches!(self.current().kind, TokenKind::Dot | TokenKind::LBrack) {
            designators.push(self.parse_designator()?);
        }
        self.expect_skip(TokenKind::Assign, "'='")?;
        let init = self.parse_initializer()?;
        Ok(Init::Designated {
            designators,
            init: Box::new(init),
            line,
            column,
        })
    }

    pub(super) fn parse_designator(&mut self) -> Result<Designator, Diagnostic> {
        if self.match_kind(TokenKind::Dot) {
            if self.current().kind != TokenKind::Ident {
                return Err(self.error("expected a member name"));
            }
            let name = self.advance();
            return Ok(Designator::Field {
                name: name.spelling,
                line: name.line,
                column: name.column,
            });
        }
        let bracket = self.advance();
        let expr = self.parse_conditional()?;
        if self.current().kind == TokenKind::Ellipsis {
            return Err(self.error("range designators are not supported"));
        }
        self.expect_skip(TokenKind::RBrack, "']'")?;
        Ok(Designator::Index {
            expr,
            line: bracket.line,
            column: bracket.column,
        })
    }

    /// `sizeof (type-name)` when the parenthesis starts a type. Otherwise
    /// `sizeof unary-expression`. The operand is not evaluated later.
    pub(super) fn parse_sizeof(&mut self, line: usize, column: usize) -> Result<Expr, Diagnostic> {
        self.bump();
        if self.current().kind == TokenKind::LParen && self.lookahead_is_type() {
            let lparen = self.advance();
            let ty = self.parse_type_name()?;
            self.expect_skip(TokenKind::RParen, "')'")?;
            if self.current().kind == TokenKind::LBrace {
                // `sizeof (T){...}` is `sizeof` of a compound literal.
                let literal = self.parse_compound_literal(ty, lparen.line, lparen.column)?;
                let expr = self.parse_postfix_tail(literal)?;
                return Ok(Expr::sizeof_expr(expr, line, column));
            }
            return Ok(Expr::sizeof_type(ty, line, column));
        }
        let expr = self.parse_unary()?;
        Ok(Expr::sizeof_expr(expr, line, column))
    }

    /// `_Alignof ( type-name )` (C11 6.5.3.4) is an integer constant of type
    /// `size_t`. The type shall not be a function type or incomplete. For an
    /// array type the result is the element type's alignment, which is the
    /// alignment a member of that type gets (the ABI's 16-byte rule for
    /// large array objects is not a property of the type).
    pub(super) fn parse_alignof(&mut self, line: usize, column: usize) -> Result<Expr, Diagnostic> {
        let keyword = self.advance();
        if self.current().kind != TokenKind::LParen || !self.lookahead_is_type() {
            if keyword.spelling.starts_with("__") {
                // `__alignof__ expression` (GCC manual, "Determining the
                // Alignment of Functions, Types or Variables"): the
                // alignment of the expression's type.
                let expr = self.parse_unary()?;
                let ty = type_only(&expr, self.env()).map_err(|_| {
                    self.error_at(
                        expr.line(),
                        expr.column(),
                        "cannot determine the type of this '__alignof__' operand",
                    )
                })?;
                if !ty.is_complete() || ty.is_void() || ty.is_func() {
                    return Err(self.error_at(
                        line,
                        column,
                        "invalid application of '__alignof__' to an incomplete type",
                    ));
                }
                return Ok(Expr::int(
                    u64::from(ty.member_align()),
                    TypeName::size_t(),
                    line,
                    column,
                ));
            }
            return Err(self.error("_Alignof requires a parenthesized type name"));
        }
        self.bump();
        let ty = self.parse_type_name()?;
        self.expect_skip(TokenKind::RParen, "')'")?;
        if ty.is_func() {
            return Err(self.error_at(
                line,
                column,
                "invalid application of '_Alignof' to a function type",
            ));
        }
        if !ty.is_complete() || ty.is_void() {
            return Err(self.error_at(
                line,
                column,
                "invalid application of '_Alignof' to an incomplete type",
            ));
        }
        self.check_vla_bound_types(&ty)?;
        Ok(Expr::int(
            u64::from(ty.member_align()),
            TypeName::size_t(),
            line,
            column,
        ))
    }

    pub(super) fn parse_cast(&mut self) -> Result<Expr, Diagnostic> {
        let lparen = self.advance();
        let ty = self.parse_type_name()?;
        self.expect_skip(TokenKind::RParen, "')'")?;
        if self.current().kind == TokenKind::LBrace {
            let literal = self.parse_compound_literal(ty, lparen.line, lparen.column)?;
            return self.parse_postfix_tail(literal);
        }
        let expr = self.parse_unary()?;
        Ok(Expr::cast(ty, expr, lparen.line, lparen.column))
    }

    /// `( type-name ) { initializer-list }` after the `)` (C11 6.5.2.5).
    /// The literal is an unnamed object of that type; an array of unknown
    /// size takes its length from the initializer.
    pub(super) fn parse_compound_literal(
        &mut self,
        ty: CType,
        line: usize,
        column: usize,
    ) -> Result<Expr, Diagnostic> {
        if ty.is_vla() {
            return Err(self.error_at(
                line,
                column,
                "a variable length array cannot be a compound literal type",
            ));
        }
        let init = self.parse_initializer()?;
        Ok(Expr::CompoundLit {
            decl: Box::new(VarDecl {
                name: String::new(),
                ty,
                storage: Storage::None,
                init: Some(init),
                checked: None,
                line,
                column,
                slot: String::new(),
                align: None,
                alignas: None,
                thread_local: false,
                vla_from: None,
            }),
            line,
            column,
            ty: None,
        })
    }

    /// `_Generic ( assignment-expression , generic-assoc-list )`
    /// (C11 6.5.1.1). Each association is `type-name : expr` or
    /// `default : expr`.
    pub(super) fn parse_generic(&mut self) -> Result<Expr, Diagnostic> {
        let keyword = self.advance();
        self.expect_skip(TokenKind::LParen, "'('")?;
        let control = self.parse_assignment()?;
        self.expect_skip(TokenKind::Comma, "','")?;
        let mut assocs = Vec::new();
        loop {
            let token = self.current().clone();
            let ty = if self.match_kind(TokenKind::KwDefault) {
                None
            } else {
                Some(self.parse_type_name()?)
            };
            self.expect_skip(TokenKind::Colon, "':'")?;
            let expr = self.parse_assignment()?;
            assocs.push(GenericAssoc {
                ty,
                expr,
                line: token.line,
                column: token.column,
            });
            if !self.match_kind(TokenKind::Comma) {
                break;
            }
        }
        self.expect_skip(TokenKind::RParen, "')'")?;
        Ok(Expr::Generic {
            control: Box::new(control),
            assocs,
            line: keyword.line,
            column: keyword.column,
            ty: None,
        })
    }

    /// A type name is specifiers plus an optional abstract declarator.
    /// `int *`, `int (*)[3]`, and `int [3]` are accepted. A function type or a
    /// function pointer is rejected.
    pub(super) fn parse_type_name(&mut self) -> Result<CType, Diagnostic> {
        let specs = self.parse_specs(SpecPlace::TypeName)?;
        let base = specs.ty.clone();
        if !self.starts_raw_declarator() {
            return Ok(base);
        }
        let raw = self.parse_raw_declarator(true)?;
        if let Some((_, line, column)) = decl_name(&raw) {
            return Err(self.error_at(line, column, "type name cannot declare an identifier"));
        }
        let (line, column) = decl_pos(&raw);
        let ty = layer_type(self.derive(base, &raw, line, column)?);
        self.reject_flexible_array(&ty, line, column)?;
        Ok(ty)
    }

    pub(super) fn lookahead_is_type(&self) -> bool {
        let Some(token) = self.tokens.get(self.index + 1) else {
            return false;
        };
        self.at_type_specifier(token)
            || matches!(token.kind, TokenKind::KwConst | TokenKind::KwVolatile)
    }

    pub(super) fn parse_builtin_va_arg(
        &mut self,
        line: usize,
        column: usize,
    ) -> Result<Expr, Diagnostic> {
        self.bump();
        let ap = self.parse_assignment()?;
        self.expect_skip(TokenKind::Comma, "','")?;
        let type_arg = self.parse_type_name()?;
        self.expect_skip(TokenKind::RParen, "')'")?;
        Ok(Expr::va_arg(ap, type_arg, line, column))
    }

    /// `__builtin_offsetof ( type-name , member-designator )`, which is what
    /// `<stddef.h>` expands `offsetof` to (C11 7.19p3). The designator is an
    /// identifier followed by any number of `.identifier` and
    /// `[expression]`.
    ///
    /// When every subscript is an integer constant expression the result
    /// is an integer constant of type `size_t`, folded from the record
    /// layout, so it may appear wherever C11 6.6p6 requires an integer
    /// constant expression. A subscript that is not constant (a GNU
    /// extension) leaves `size_t` integer arithmetic, the folded offset
    /// plus each such index times its element size, evaluated at run time
    /// and not a constant expression. No address is formed from a null
    /// pointer, so the IR has no `getelementptr` on a null base.
    pub(super) fn parse_builtin_offsetof(
        &mut self,
        line: usize,
        column: usize,
    ) -> Result<Expr, Diagnostic> {
        self.bump();
        let ty = self.parse_type_name()?;
        self.expect_skip(TokenKind::Comma, "','")?;
        let steps = self.parse_offsetof_designator()?;
        self.expect_skip(TokenKind::RParen, "')'")?;

        let mut cur = ty;
        let mut offset: u64 = 0;
        // Subscripts that are not constant, each already scaled by its
        // element size, as size_t arithmetic evaluated at run time.
        let mut runtime: Vec<Expr> = Vec::new();
        for (position, step) in steps.into_iter().enumerate() {
            match step {
                OffsetStep::Member(name) => {
                    let (field_offset, field_ty) = self.offsetof_member(&cur, &name, position)?;
                    offset = offset.wrapping_add(field_offset);
                    cur = field_ty;
                }
                OffsetStep::Index(index, bracket) => {
                    cur = self.offsetof_index(&cur, index, &bracket, &mut offset, &mut runtime)?;
                }
            }
        }
        // offset + i1 * size1 + i2 * size2 + ... in size_t: plain integer
        // arithmetic, with no address formed from a null pointer.
        Ok(runtime.into_iter().fold(
            Expr::int(offset, TypeName::size_t(), line, column),
            |sum, term| Expr::bin(BinOp::Add, sum, term, line, column),
        ))
    }

    /// The member designator of `offsetof`: a member name followed by
    /// `.member` and `[index]` steps.
    fn parse_offsetof_designator(&mut self) -> Result<Vec<OffsetStep>, Diagnostic> {
        let first = self.expect(TokenKind::Ident, "a member name")?;
        let mut steps = vec![OffsetStep::Member(first)];
        loop {
            match self.current().kind {
                TokenKind::Dot => {
                    self.bump();
                    let name = self.expect(TokenKind::Ident, "a member name")?;
                    steps.push(OffsetStep::Member(name));
                }
                TokenKind::LBrack => {
                    let bracket = self.advance();
                    let index = self.parse_expression()?;
                    self.expect_skip(TokenKind::RBrack, "']'")?;
                    steps.push(OffsetStep::Index(index, bracket));
                }
                _ => break,
            }
        }
        Ok(steps)
    }

    /// The offset and type of member `name` of `cur`, the type reached so
    /// far in an `offsetof` designator (`position` 0 for the first step).
    fn offsetof_member(
        &self,
        cur: &CType,
        name: &Token,
        position: usize,
    ) -> Result<(u64, CType), Diagnostic> {
        let Some(handle) = cur.struct_ref() else {
            let message = if position == 0 {
                format!(
                    "offsetof requires a structure or union type, not '{}'",
                    cur.display()
                )
            } else {
                format!(
                    "member reference base type '{}' is not a structure or union",
                    cur.display()
                )
            };
            return Err(self.error_at(name.line, name.column, message));
        };
        if !handle.is_complete() {
            return Err(self.error_at(
                name.line,
                name.column,
                format!("offsetof of incomplete type '{}'", cur.display()),
            ));
        }
        let Some(hit) = handle.lookup_member(&name.spelling) else {
            return Err(self.error_at(
                name.line,
                name.column,
                format!("no member named '{}' in '{}'", name.spelling, cur.display()),
            ));
        };
        if hit.field.is_bitfield() {
            return Err(self.error_at(
                name.line,
                name.column,
                format!("cannot compute offset of bit-field '{}'", name.spelling),
            ));
        }
        Ok((hit.field.offset, hit.field.ty))
    }

    /// A subscript step `[index]` of an `offsetof` designator applied to
    /// the array type `cur`. A constant index is folded into `offset`;
    /// any other becomes a scaled `size_t` term in `runtime`. Returns the
    /// element type.
    fn offsetof_index(
        &self,
        cur: &CType,
        index: Expr,
        bracket: &Token,
        offset: &mut u64,
        runtime: &mut Vec<Expr>,
    ) -> Result<CType, Diagnostic> {
        let Some(elem) = cur.array_elem().cloned() else {
            return Err(self.error_at(
                bracket.line,
                bracket.column,
                "subscripted value is not an array",
            ));
        };
        let Some(size) = elem.size() else {
            return Err(self.error_at(
                bracket.line,
                bracket.column,
                format!("array has incomplete element type '{}'", elem.display()),
            ));
        };
        check_uneval(&index, self.env())
            .map_err(|err| self.error_at(index.line(), index.column(), ice_message(err)))?;
        let index_ty = rvalue_type(&index, self.env()).ok();
        if let Some(index_ty) = &index_ty {
            if !index_ty.is_integer() {
                return Err(self.error_at(
                    index.line(),
                    index.column(),
                    "array subscript is not an integer",
                ));
            }
        }
        match eval_ice(&index, self.env()) {
            Ok(value) => {
                let scaled = if value.ty.is_signed() {
                    (sign_extend(value.bits, value.ty.width()) as u64).wrapping_mul(size)
                } else {
                    (value.bits as u64).wrapping_mul(size)
                };
                *offset = offset.wrapping_add(scaled);
            }
            Err(_) => {
                let (at_line, at_column) = (index.line(), index.column());
                // Converting the index to size_t sign-extends a
                // signed index, so the wrapping product and sum
                // equal the address difference. When the parser
                // cannot type the index, `| (size_t)0` performs
                // the same conversion and leaves semantic
                // analysis to require an integer operand.
                let as_size = if index_ty.is_some() {
                    Expr::cast(
                        CType::from_name(TypeName::size_t()),
                        index,
                        at_line,
                        at_column,
                    )
                } else {
                    Expr::bin(
                        BinOp::BitOr,
                        index,
                        Expr::int(0, TypeName::size_t(), at_line, at_column),
                        at_line,
                        at_column,
                    )
                };
                runtime.push(Expr::bin(
                    BinOp::Mul,
                    as_size,
                    Expr::int(size, TypeName::size_t(), at_line, at_column),
                    at_line,
                    at_column,
                ));
            }
        }
        Ok(elem)
    }

    /// Builtins the parser answers itself, with their arguments still to
    /// be read: `__builtin_types_compatible_p (type, type)`,
    /// `__builtin_choose_expr (const, e1, e2)`, and
    /// `__builtin_constant_p (e)` (GCC manual, "Other Built-in
    /// Functions"). `None` for any other name.
    pub(super) fn parse_folded_builtin(
        &mut self,
        name: String,
        line: usize,
        column: usize,
    ) -> Result<Option<Expr>, Diagnostic> {
        match name.as_str() {
            "__builtin_types_compatible_p" => {
                self.bump();
                let first = self.parse_type_name()?;
                self.expect_skip(TokenKind::Comma, "','")?;
                let second = self.parse_type_name()?;
                self.expect_skip(TokenKind::RParen, "')'")?;
                // Top-level qualifiers are ignored.
                let same = compatible(&first.unqualified(), &second.unqualified());
                Ok(Some(Expr::int(
                    u64::from(same),
                    TypeName::Int,
                    line,
                    column,
                )))
            }
            "__builtin_choose_expr" => {
                self.bump();
                let cond = self.parse_assignment()?;
                let value = eval_ice(&cond, self.env()).map_err(|_| {
                    self.error_at(
                        cond.line(),
                        cond.column(),
                        "the first argument of __builtin_choose_expr is not a constant",
                    )
                })?;
                self.expect_skip(TokenKind::Comma, "','")?;
                let first = self.parse_assignment()?;
                self.expect_skip(TokenKind::Comma, "','")?;
                let second = self.parse_assignment()?;
                self.expect_skip(TokenKind::RParen, "')'")?;
                Ok(Some(if value.bits & value.ty.mask() != 0 {
                    first
                } else {
                    second
                }))
            }
            "__builtin_constant_p" => {
                self.bump();
                let arg = self.parse_assignment()?;
                self.expect_skip(TokenKind::RParen, "')'")?;
                // 1 for an integer constant expression or a floating or
                // string literal; 0 otherwise. The argument is not
                // evaluated. GCC may also answer 1 after optimization; 0 is
                // always a correct answer.
                let constant = matches!(arg, Expr::Float { .. } | Expr::Str { .. })
                    || eval_ice(&arg, self.env()).is_ok();
                Ok(Some(Expr::int(
                    u64::from(constant),
                    TypeName::Int,
                    line,
                    column,
                )))
            }
            _ => Ok(None),
        }
    }

    /// Fold a bit-counting or byte-swapping builtin, or
    /// `__builtin_expect`, whose arguments are integer constant
    /// expressions, so the call is itself one (GCC folds these too).
    pub(super) fn fold_builtin_call(&self, call: Expr) -> Expr {
        let Expr::Call {
            callee,
            args,
            line,
            column,
            ..
        } = &call
        else {
            return call;
        };
        let Expr::Var { name, .. } = callee.as_ref() else {
            return call;
        };
        let Some(builtin) = crate::builtins::Inline::from_name(name) else {
            return call;
        };
        let env = self.env();
        if builtin == crate::builtins::Inline::Expect && args.len() == 2 {
            if let (Ok(value), Ok(_)) = (eval_ice(&args[0], env), eval_ice(&args[1], env)) {
                let bits = convert_bits(value.bits, value.ty, TypeName::Long);
                return Expr::int(bits as u64, TypeName::Long, *line, *column);
            }
            return call;
        }
        if args.len() != 1 {
            return call;
        }
        let Some(param) = builtin.param_type(0).and_then(|ty| ty.as_integer()) else {
            return call;
        };
        let Ok(value) = eval_ice(&args[0], env) else {
            return call;
        };
        let bits = convert_bits(value.bits, value.ty, param);
        let Some(result) = builtin.fold(bits as u64) else {
            return call;
        };
        let result_ty = builtin
            .result_type()
            .as_integer()
            .expect("bit builtins return an integer");
        Expr::int(result, result_ty, *line, *column)
    }

    pub(super) fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let expr = self.parse_primary()?;
        self.parse_postfix_tail(expr)
    }

    /// Postfix operators applied to `expr`, which is a primary expression
    /// or a compound literal (C11 6.5.2).
    pub(super) fn parse_postfix_tail(&mut self, mut expr: Expr) -> Result<Expr, Diagnostic> {
        loop {
            match self.current().kind {
                TokenKind::LParen => {
                    if let Expr::Var {
                        name, line, column, ..
                    } = &expr
                    {
                        if name == "__builtin_va_arg" {
                            let line = *line;
                            let column = *column;
                            expr = self.parse_builtin_va_arg(line, column)?;
                            continue;
                        }
                        if name == "__builtin_offsetof" {
                            let line = *line;
                            let column = *column;
                            expr = self.parse_builtin_offsetof(line, column)?;
                            continue;
                        }
                        if let Some(folded) =
                            self.parse_folded_builtin(name.clone(), *line, *column)?
                        {
                            expr = folded;
                            continue;
                        }
                    }
                    let line = expr.line();
                    let column = expr.column();
                    let builtin = match &expr {
                        Expr::Var { name, .. } => match name.as_str() {
                            "__builtin_va_start" => Some(VaOp::Start),
                            "__builtin_va_end" => Some(VaOp::End),
                            "__builtin_va_copy" => Some(VaOp::Copy),
                            _ => None,
                        },
                        _ => None,
                    };
                    self.bump();
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
                    self.expect_skip(TokenKind::RParen, "')'")?;
                    expr = if let Some(op) = builtin {
                        Expr::va_op(op, args, line, column)
                    } else {
                        self.fold_builtin_call(Expr::call(expr, args, line, column))
                    };
                }
                TokenKind::LBrack => {
                    let bracket = self.advance();
                    let index = self.parse_expression()?;
                    self.expect_skip(TokenKind::RBrack, "']'")?;
                    expr = Expr::index(expr, index, bracket.line, bracket.column);
                }
                TokenKind::PlusPlus | TokenKind::MinusMinus => {
                    let op_token = self.advance();
                    let op = if op_token.kind == TokenKind::PlusPlus {
                        IncOp::PostInc
                    } else {
                        IncOp::PostDec
                    };
                    expr = Expr::incdec(op, expr, op_token.line, op_token.column);
                }
                TokenKind::Arrow | TokenKind::Dot => {
                    let arrow = self.current().kind == TokenKind::Arrow;
                    let op = self.advance();
                    if self.current().kind != TokenKind::Ident {
                        return Err(self.error("expected a member name"));
                    }
                    let name = self.advance();
                    expr = Expr::member(expr, name.spelling, arrow, op.line, op.column);
                }
                // `case low ... high:` in GNU C.
                TokenKind::Ellipsis if self.dialect.gnu() => break,
                TokenKind::Ellipsis => {
                    return Err(self.error("unexpected '...'"));
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    pub(super) fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.current().clone();
        match token.kind {
            TokenKind::KwGeneric => self.parse_generic(),
            TokenKind::Float => {
                self.bump();
                Ok(Expr::float_lit(
                    (u128::from(token.float_hi) << 64)
                        | u128::from(token.value.expect("float bits")),
                    token.float_kind.expect("float kind"),
                    token.line,
                    token.column,
                ))
            }
            TokenKind::Int => {
                self.bump();
                Ok(Expr::int(
                    token.value.expect("int token"),
                    token.int_ty.expect("int token type"),
                    token.line,
                    token.column,
                ))
            }
            TokenKind::Ident => {
                self.bump();
                if let Some(NameBinding::EnumConst(value, ty)) = self.lookup_name(&token.spelling) {
                    return Ok(Expr::int(
                        (value as u128 & ty.mask()) as u64,
                        ty,
                        token.line,
                        token.column,
                    ));
                }
                if self.is_typedef(&token.spelling) {
                    return Err(self.error_at(
                        token.line,
                        token.column,
                        format!("typedef name '{}' used as a value", token.spelling),
                    ));
                }
                Ok(Expr::var(token.spelling, token.line, token.column))
            }
            TokenKind::String => {
                self.bump();
                // Translation phase 6 (C11 6.4.5). An unprefixed literal adopts
                // the other prefix. Two different non-ordinary prefixes are a
                // diagnostic. `u8` is a non-ordinary prefix; the result is
                // still an array of `char`.
                let mut prefix = lit_prefix(&token.spelling);
                let mut bytes = token.bytes.expect("string token");
                while self.current().kind == TokenKind::String {
                    let next = self.advance();
                    let next_prefix = lit_prefix(&next.spelling);
                    let Some(merged) = merge_prefix(prefix, next_prefix) else {
                        return Err(self.error_at(
                            next.line,
                            next.column,
                            "cannot concatenate string literals with different encoding prefixes",
                        ));
                    };
                    let next_bytes = next.bytes.expect("string token");
                    bytes = concat_encoded(prefix, &bytes, next_prefix, &next_bytes, merged);
                    prefix = merged;
                }
                Ok(Expr::str_lit(bytes, prefix.enc(), token.line, token.column))
            }
            TokenKind::LParen if self.lookahead() == TokenKind::LBrace => self.parse_stmt_expr(),
            TokenKind::LParen => {
                self.bump();
                let expr = self.parse_expression()?;
                self.expect_skip(TokenKind::RParen, "')'")?;
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

    /// `({ block-item-list })` (GCC manual, "Statements and Declarations in
    /// Expressions"). The value is the last expression statement's, after
    /// lvalue conversion; a block ending in anything else is `void`.
    pub(super) fn parse_stmt_expr(&mut self) -> Result<Expr, Diagnostic> {
        let open = self.advance();
        if !self.dialect.gnu() {
            return Err(self.error_at(
                open.line,
                open.column,
                "statement expressions are a GNU extension; use -std=gnu11",
            ));
        }
        if self.names.len() <= 1 {
            return Err(self.error_at(
                open.line,
                open.column,
                "statement expression not allowed at file scope",
            ));
        }
        let brace = self.advance();
        self.push_tags();
        self.stmt_expr_depth += 1;
        let body = self.parse_compound_items(brace.line, brace.column);
        self.stmt_expr_depth -= 1;
        let body = match body {
            Ok(body) => body,
            Err(err) => {
                self.pop_tags();
                return Err(err);
            }
        };
        let value_ty = match body.items.last() {
            Some(Stmt::Expr {
                expr: Some(expr), ..
            }) => rvalue_type(expr, self.env()).ok(),
            _ => Some(CType::Void),
        };
        self.pop_tags();
        self.expect_skip(TokenKind::RParen, "')'")?;
        Ok(Expr::StmtExpr {
            body: Box::new(body),
            value_ty,
            line: open.line,
            column: open.column,
            ty: None,
        })
    }
}

pub(super) fn lit_prefix(spelling: &str) -> LitPrefix {
    if spelling.starts_with("u8") {
        LitPrefix::U8
    } else if spelling.starts_with('u') {
        LitPrefix::U
    } else if spelling.starts_with('U') {
        LitPrefix::UU
    } else if spelling.starts_with('L') {
        LitPrefix::L
    } else {
        LitPrefix::None
    }
}

/// An unprefixed literal adopts the other prefix. Different non-ordinary
/// prefixes do not concatenate (C11 6.4.5).
pub(super) fn merge_prefix(left: LitPrefix, right: LitPrefix) -> Option<LitPrefix> {
    match (left, right) {
        (a, b) if a == b => Some(a),
        (LitPrefix::None, other) | (other, LitPrefix::None) => Some(other),
        _ => None,
    }
}

pub(super) fn concat_encoded(
    left_prefix: LitPrefix,
    left: &[u8],
    right_prefix: LitPrefix,
    right: &[u8],
    merged: LitPrefix,
) -> Vec<u8> {
    let mut out = retarget(left_prefix, left, merged);
    out.extend(retarget(right_prefix, right, merged));
    out
}

impl Parser<'_> {
    /// Type-check VLA size expressions in `ty` without evaluating them.
    /// `_Alignof` does not evaluate its operand (C11 6.5.3.4p3), but the
    /// size still must have integer type (C11 6.7.6.2p1).
    fn check_vla_bound_types(&self, ty: &CType) -> Result<(), Diagnostic> {
        match ty {
            CType::Array(elem, len) => {
                self.check_vla_bound_types(elem)?;
                if let ArrayLen::Vla(id) = *len {
                    self.check_vla_bound_id(id)?;
                }
            }
            CType::Pointer(inner) | CType::Qualified(_, inner) => {
                self.check_vla_bound_types(inner)?;
            }
            CType::Func(func) => {
                self.check_vla_bound_types(&func.ret)?;
                for param in &func.params {
                    self.check_vla_bound_types(param)?;
                }
            }
            CType::Void | CType::Int(_) | CType::Float(_) | CType::Struct(_) => {}
        }
        Ok(())
    }

    fn check_vla_bound_id(&self, id: VlaId) -> Result<(), Diagnostic> {
        let Some(bound) = self.vla_sizes.iter().find(|bound| bound.id == id) else {
            return Ok(());
        };
        if !bound.integer {
            return Err(self.error_at(
                bound.expr.line(),
                bound.expr.column(),
                "array size expression must have integer type",
            ));
        }
        Ok(())
    }
}
