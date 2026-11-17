//! Declarators (C11 6.7.6): pointers, arrays, function parameter lists and
//! deriving the declared type.

use super::*;

impl Parser<'_> {
    /// A named declarator. `int *a, b` parses two of these against one base type.
    pub(super) fn parse_named_decl(&mut self, base: CType) -> Result<NamedDecl, Diagnostic> {
        let (mut named, attrs) =
            self.collect_attrs(|parser| parser.parse_named_decl_inner(base))?;
        named.attrs = attrs;
        Ok(named)
    }

    pub(super) fn parse_named_decl_inner(&mut self, base: CType) -> Result<NamedDecl, Diagnostic> {
        let raw = self.parse_raw_declarator(false)?;
        let (name, line, column) = match decl_name(&raw) {
            Some(found) => found,
            None => {
                let (line, column) = decl_pos(&raw);
                return Err(self.error_at(line, column, "expected an identifier"));
            }
        };
        let layer = self.derive(base, &raw, line, column)?;
        let (param_tags, param_names) = match &layer {
            Layer::Function {
                introduced,
                introduced_names,
                ..
            } => (
                introduced
                    .iter()
                    .map(|(name, handle)| (name.clone(), handle.clone()))
                    .collect(),
                introduced_names
                    .iter()
                    .map(|(name, binding)| (name.clone(), binding.clone()))
                    .collect(),
            ),
            Layer::Object(_) => (Vec::new(), Vec::new()),
        };
        let kind = self.layer_to_kind(layer, line, column)?;
        let symbol = self.parse_decl_suffix()?;
        Ok(NamedDecl {
            name,
            line,
            column,
            kind,
            param_tags,
            param_names,
            symbol,
            attrs: Vec::new(),
        })
    }

    pub(super) fn layer_to_kind(
        &self,
        layer: Layer,
        line: usize,
        column: usize,
    ) -> Result<DeclKind, Diagnostic> {
        match layer {
            Layer::Object(ty) => {
                self.reject_flexible_array(&ty, line, column)?;
                Ok(DeclKind::Object(ty))
            }
            Layer::Function {
                ret,
                params,
                prototype,
                variadic,
                introduced: _,
                introduced_names: _,
            } => {
                self.reject_flexible_array(&ret, line, column)?;
                for param in &params {
                    self.reject_flexible_array(&param.ty, param.line, param.column)?;
                }
                Ok(DeclKind::Function {
                    ret,
                    params,
                    prototype,
                    variadic,
                })
            }
        }
    }

    pub(super) fn parse_param_list(&mut self) -> Result<ParsedParams, Diagnostic> {
        self.expect_skip(TokenKind::LParen, "'('")?;
        // Tags declared in a parameter list have prototype scope, or the
        // function's block scope when this declarator is a definition. The
        // map is returned either way; the caller keeps it only for a body.
        self.push_tags();
        let parsed = self.parse_param_list_in_scope();
        let introduced = self.tags.last().expect("parameter scope").clone();
        let introduced_names = self.names.last().expect("parameter scope").clone();
        self.pop_tags();
        let mut parsed = parsed?;
        parsed.introduced = introduced;
        parsed.introduced_names = introduced_names;
        Ok(parsed)
    }

    pub(super) fn parse_param_list_in_scope(&mut self) -> Result<ParsedParams, Diagnostic> {
        if self.match_kind(TokenKind::RParen) {
            return Ok(ParsedParams {
                params: Vec::new(),
                prototype: false,
                variadic: false,
                introduced: HashMap::new(),
                introduced_names: HashMap::new(),
            });
        }
        if self.current().kind == TokenKind::KwVoid && self.lookahead() == TokenKind::RParen {
            self.bump();
            self.bump();
            return Ok(ParsedParams {
                params: Vec::new(),
                prototype: true,
                variadic: false,
                introduced: HashMap::new(),
                introduced_names: HashMap::new(),
            });
        }
        // An identifier list (C11 6.7.6.3p3) belongs to a K&R function
        // definition, which is not supported. `int f();` is handled above.
        if self.current().kind == TokenKind::Ident
            && !self.at_type_specifier(self.current())
            && matches!(self.lookahead(), TokenKind::Comma | TokenKind::RParen)
        {
            return Err(self.error("old-style function declarations are not supported"));
        }
        let mut params = Vec::new();
        let mut variadic = false;
        loop {
            if self.current().kind == TokenKind::Ellipsis {
                if params.is_empty() {
                    return Err(self.error("a named parameter is required before '...'"));
                }
                self.bump();
                variadic = true;
                break;
            }
            params.push(self.parse_param()?);
            if self.match_kind(TokenKind::Comma) {
                if self.current().kind == TokenKind::RParen {
                    return Err(self.error("trailing commas in parameter lists are not supported"));
                }
                continue;
            }
            break;
        }
        self.expect_skip(TokenKind::RParen, "')'")?;
        Ok(ParsedParams {
            params,
            prototype: true,
            variadic,
            introduced: HashMap::new(),
            introduced_names: HashMap::new(),
        })
    }

    pub(super) fn parse_param(&mut self) -> Result<Param, Diagnostic> {
        let specs = self.parse_specs(SpecPlace::Parameter)?;
        let base = specs.ty.clone();
        if !self.starts_raw_declarator() {
            if base.is_void() {
                if self.current().kind == TokenKind::Comma {
                    return Err(self.error_at(
                        specs.line,
                        specs.column,
                        "parameter has incomplete type 'void'",
                    ));
                }
                return Err(self.error(format!("expected ')', found {}", found(self.current()))));
            }
            if matches!(self.current().kind, TokenKind::Comma | TokenKind::RParen) {
                let ty = self.adjust_parameter(base, specs.line, specs.column)?;
                return Ok(Param {
                    name: None,
                    ty,
                    line: specs.line,
                    column: specs.column,
                    slot: String::new(),
                    arg_reg: String::new(),
                });
            }
            return Err(self.error(format!(
                "expected an identifier, found {}",
                found(self.current())
            )));
        }
        let raw = self.parse_raw_declarator(true)?;
        // `int x __attribute__((unused))`: parameter attributes are ignored.
        self.parse_attributes_into()?;
        let named = decl_name(&raw);
        let (line, column) = named
            .as_ref()
            .map(|(_, line, column)| (*line, *column))
            .unwrap_or_else(|| decl_pos(&raw));
        let ty = layer_type(self.derive(base, &raw, line, column)?);
        let ty = self.adjust_parameter(ty, line, column)?;
        if let Some((name, line, column)) = &named {
            self.bind_ordinary(name, Some(ty.clone()), *line, *column)?;
        }
        Ok(Param {
            name: named.map(|(name, _, _)| name),
            ty,
            line,
            column,
            slot: String::new(),
            arg_reg: String::new(),
        })
    }

    /// Outermost array parameters become pointers. A parameter of function
    /// type becomes a pointer to that function (C11 6.7.6.3).
    pub(super) fn adjust_parameter(
        &self,
        ty: CType,
        line: usize,
        column: usize,
    ) -> Result<CType, Diagnostic> {
        let ty = match ty {
            CType::Array(elem, _) => CType::pointer(*elem),
            CType::Func(func) => CType::pointer(CType::Func(func)),
            other => other,
        };
        self.reject_bad_restrict(&ty, line, column)?;
        match ty {
            CType::Void => Err(self.error_at(line, column, "parameter has incomplete type 'void'")),
            // A prototype may name an incomplete struct or union (C11 6.7.6.3).
            // The type has to be complete at a definition and at a call.
            // Other incomplete types, such as `void` or an incomplete array
            // that was not adjusted to a pointer, are still rejected.
            ref other if other.unqualified().is_record() => Ok(ty),
            ref other if !other.is_complete() => Err(self.error_at(
                line,
                column,
                format!("parameter has incomplete type '{}'", other.display()),
            )),
            other => Ok(other),
        }
    }

    pub(super) fn starts_raw_declarator(&self) -> bool {
        matches!(
            self.current().kind,
            TokenKind::Star | TokenKind::LParen | TokenKind::LBrack | TokenKind::Ident
        )
    }

    /// Declarator grammar, with the pointer written outside the direct declarator.
    ///
    /// `*` binds inside `[]` and `()`, so `int *a[3]` is an array of pointers
    /// and `int (*p)[3]` is a pointer to an array. An abstract declarator may
    /// omit the identifier (`sizeof(int *)`, `(void *)0`).
    pub(super) fn parse_raw_declarator(
        &mut self,
        abstract_ok: bool,
    ) -> Result<RawDecl, Diagnostic> {
        if self.current().kind == TokenKind::Star {
            self.bump();
            let quals = self.parse_pointer_quals()?;
            if let Some(message) = unsupported(self.current().kind) {
                return Err(self.error(message));
            }
            let inner = self.parse_raw_declarator(abstract_ok)?;
            return Ok(RawDecl::Pointer(quals, Box::new(inner)));
        }
        self.parse_direct(abstract_ok)
    }

    pub(super) fn parse_direct(&mut self, abstract_ok: bool) -> Result<RawDecl, Diagnostic> {
        let mut node = if self.current().kind == TokenKind::LParen {
            if abstract_ok && self.can_start_param() {
                let line = self.current().line;
                let column = self.current().column;
                let params = self.parse_param_list()?;
                RawDecl::Func(Box::new(RawDecl::Abstract(line, column)), params)
            } else {
                self.bump();
                let inner = self.parse_raw_declarator(abstract_ok)?;
                self.expect_skip(TokenKind::RParen, "')'")?;
                inner
            }
        } else if self.current().kind == TokenKind::Ident {
            let name = self.advance();
            RawDecl::Named(name.spelling, name.line, name.column)
        } else if abstract_ok {
            let token = self.current();
            RawDecl::Abstract(token.line, token.column)
        } else if let Some(message) = unsupported(self.current().kind) {
            return Err(self.error(message));
        } else {
            return Err(self.error(format!(
                "expected an identifier, found {}",
                found(self.current())
            )));
        };
        loop {
            match self.current().kind {
                TokenKind::LBrack => {
                    self.bump();
                    let len = self.parse_array_len()?;
                    self.expect_skip(TokenKind::RBrack, "']'")?;
                    node = RawDecl::Array(Box::new(node), len);
                }
                TokenKind::LParen => {
                    let params = self.parse_param_list()?;
                    node = RawDecl::Func(Box::new(node), params);
                }
                _ => break,
            }
        }
        Ok(node)
    }

    /// Qualifiers after `*` (`int * const p`). A repeated qualifier in this
    /// list is a diagnostic. `restrict` stays unsupported.
    pub(super) fn parse_pointer_quals(&mut self) -> Result<Quals, Diagnostic> {
        let mut quals = Quals::default();
        loop {
            let token = self.current().clone();
            match token.kind {
                TokenKind::KwConst | TokenKind::KwVolatile => {
                    let which = if token.kind == TokenKind::KwConst {
                        "const"
                    } else {
                        "volatile"
                    };
                    let duplicate = if token.kind == TokenKind::KwConst {
                        quals.is_const
                    } else {
                        quals.is_volatile
                    };
                    if duplicate {
                        return Err(self.error(format!("duplicate '{which}' qualifier")));
                    }
                    if token.kind == TokenKind::KwConst {
                        quals.is_const = true;
                    } else {
                        quals.is_volatile = true;
                    }
                    self.bump();
                }
                TokenKind::KwRestrict => {
                    if quals.is_restrict {
                        return Err(self.error("duplicate 'restrict' qualifier"));
                    }
                    quals.is_restrict = true;
                    self.bump();
                }
                TokenKind::Ident
                    if matches!(
                        self.current().spelling.as_str(),
                        "__restrict" | "__restrict__"
                    ) =>
                {
                    if quals.is_restrict {
                        return Err(self.error("duplicate 'restrict' qualifier"));
                    }
                    quals.is_restrict = true;
                    self.bump();
                }
                TokenKind::Ident if self.at_attribute() => {
                    // An attribute on a pointer declarator is ignored.
                    self.parse_attributes_into()?;
                }
                _ => break,
            }
        }
        Ok(quals)
    }

    pub(super) fn can_start_param(&self) -> bool {
        let Some(token) = self.tokens.get(self.index + 1) else {
            return false;
        };
        self.at_type_specifier(token)
            || matches!(token.kind, TokenKind::RParen | TokenKind::Ellipsis)
            || unsupported(token.kind).is_some()
    }

    /// `[]` is an unknown bound. Anything else must be an integer constant
    /// greater than zero. An identifier is a variable-length array.
    pub(super) fn parse_array_len(&mut self) -> Result<Option<u64>, Diagnostic> {
        if self.current().kind == TokenKind::Star {
            return Err(self.error("variable-length arrays are not supported"));
        }
        if self.current().kind == TokenKind::KwStatic {
            return Err(self.error("static array sizes are not supported"));
        }
        if self.current().kind == TokenKind::RBrack {
            return Ok(None);
        }
        let expr = self.parse_assignment()?;
        match fold_array_bound(&expr, self.env()) {
            Ok(len) => Ok(Some(len)),
            // GNU C allows a zero-length array (GCC manual, "Arrays of
            // Length Zero").
            Err(BoundErr::NonPositive)
                if self.dialect.gnu()
                    && eval_ice(&expr, self.env())
                        .is_ok_and(|value| value.bits & value.ty.mask() == 0) =>
            {
                Ok(Some(0))
            }
            Err(err) => Err(self.error_at(expr.line(), expr.column(), bound_message(err))),
        }
    }

    /// Build the type by inserting each constructor next to the base, which is
    /// what makes `[]` and `()` bind tighter than `*`.
    pub(super) fn derive(
        &self,
        base: CType,
        raw: &RawDecl,
        line: usize,
        column: usize,
    ) -> Result<Layer, Diagnostic> {
        let layer = if let CType::Func(func) = &base {
            Layer::Function {
                ret: func.ret.clone(),
                params: func
                    .params
                    .iter()
                    .map(|ty| Param {
                        name: None,
                        ty: ty.clone(),
                        line,
                        column,
                        slot: String::new(),
                        arg_reg: String::new(),
                    })
                    .collect(),
                prototype: func.prototype,
                variadic: func.variadic,
                introduced: HashMap::new(),
                introduced_names: HashMap::new(),
            }
        } else {
            Layer::Object(base)
        };
        self.derive_layer(layer, raw, line, column)
    }

    pub(super) fn derive_layer(
        &self,
        base: Layer,
        raw: &RawDecl,
        line: usize,
        column: usize,
    ) -> Result<Layer, Diagnostic> {
        match raw {
            RawDecl::Named(_, _, _) | RawDecl::Abstract(_, _) => Ok(base),
            RawDecl::Pointer(quals, inner) => match base {
                Layer::Object(ty) => {
                    let ptr = CType::pointer(ty).with_quals(*quals);
                    self.derive_layer(Layer::Object(ptr), inner, line, column)
                }
                layer @ Layer::Function { .. } => {
                    let func = layer_type(layer);
                    let ptr = CType::pointer(func).with_quals(*quals);
                    self.derive_layer(Layer::Object(ptr), inner, line, column)
                }
            },
            RawDecl::Array(inner, len) => match base {
                Layer::Object(ty) => {
                    let arr = self.make_array(ty, *len, line, column)?;
                    self.derive_layer(Layer::Object(arr), inner, line, column)
                }
                Layer::Function { .. } => {
                    Err(self.error_at(line, column, "array of functions is not supported"))
                }
            },
            RawDecl::Func(inner, params) => match base {
                Layer::Object(ret) => {
                    if ret.is_array() {
                        return Err(self.error_at(line, column, "function cannot return an array"));
                    }
                    if ret.is_func() {
                        return Err(self.error_at(
                            line,
                            column,
                            "function cannot return a function",
                        ));
                    }
                    // C11 6.7.3p2 applies to a return type as to any other
                    // type: `int restrict f(void)` is a constraint violation
                    // wherever the declarator appears.
                    self.reject_bad_restrict(&ret, line, column)?;
                    if !params.prototype && !params.params.is_empty() {
                        return Err(self.error_at(
                            line,
                            column,
                            "old-style function declarations are not supported",
                        ));
                    }
                    self.derive_layer(
                        Layer::Function {
                            ret,
                            params: params.params.clone(),
                            prototype: params.prototype,
                            variadic: params.variadic,
                            introduced: params.introduced.clone(),
                            introduced_names: params.introduced_names.clone(),
                        },
                        inner,
                        line,
                        column,
                    )
                }
                Layer::Function { .. } => {
                    Err(self.error_at(line, column, "function cannot return a function"))
                }
            },
        }
    }

    pub(super) fn make_array(
        &self,
        elem: CType,
        len: Option<u64>,
        line: usize,
        column: usize,
    ) -> Result<CType, Diagnostic> {
        if !elem.is_complete() {
            return Err(self.error_at(
                line,
                column,
                format!("array has incomplete element type '{}'", elem.display()),
            ));
        }
        if let Some(n) = len {
            if let Some(elem_size) = elem.size() {
                if elem_size != 0 && n > u64::MAX / elem_size {
                    return Err(self.error_at(line, column, "array is too large"));
                }
            }
        }
        Ok(CType::array(elem, len))
    }
}

/// The C type a function layer describes, with top-level parameter
/// qualifiers removed (C11 6.7.6.3p15).
pub(super) fn layer_type(layer: Layer) -> CType {
    match layer {
        Layer::Object(ty) => ty,
        Layer::Function {
            ret,
            params,
            prototype,
            variadic,
            ..
        } => CType::func_with(
            ret,
            params.iter().map(|param| param.ty.unqualified()).collect(),
            variadic,
            prototype,
        ),
    }
}

/// The type a declarator declares: the object type, or the function type.
pub(super) fn decl_kind_type(kind: &DeclKind) -> CType {
    match kind {
        DeclKind::Object(ty) => ty.clone(),
        DeclKind::Function {
            ret,
            params,
            prototype,
            variadic,
        } => CType::func_with(
            ret.clone(),
            params.iter().map(|param| param.ty.unqualified()).collect(),
            *variadic,
            *prototype,
        ),
    }
}

pub(super) fn decl_name(raw: &RawDecl) -> Option<(String, usize, usize)> {
    match raw {
        RawDecl::Named(name, line, column) => Some((name.clone(), *line, *column)),
        RawDecl::Abstract(_, _) => None,
        RawDecl::Pointer(_, inner) | RawDecl::Array(inner, _) | RawDecl::Func(inner, _) => {
            decl_name(inner)
        }
    }
}

pub(super) fn decl_pos(raw: &RawDecl) -> (usize, usize) {
    match raw {
        RawDecl::Named(_, line, column) | RawDecl::Abstract(line, column) => (*line, *column),
        RawDecl::Pointer(_, inner) | RawDecl::Array(inner, _) | RawDecl::Func(inner, _) => {
            decl_pos(inner)
        }
    }
}
