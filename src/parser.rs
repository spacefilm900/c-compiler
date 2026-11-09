//! Recursive-descent parser for a C subset.
//!
//! Expression precedence follows the C standard. From loosest to tightest:
//! comma, assignment, `?:`, `||`, `&&`, `|`, `^`, `&`, equality, relational,
//! shifts, additive, multiplicative, then unary and postfix. Assignment and
//! `?:` are right associative. A dangling `else` binds to the nearest `if`.

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
        let mut decls = Vec::new();
        while self.current().kind != TokenKind::Eof {
            decls.extend(self.parse_external()?);
        }
        Ok(TranslationUnit {
            decls,
            globals: Vec::new(),
            func_decls: Vec::new(),
        })
    }

    /// A file-scope declaration or function definition.
    ///
    /// `int f(void);` is a prototype. `int f(void) { ... }` is a definition.
    /// `int g;`, `int *p = &g;`, `int a[3] = {1};`, `extern int g;`, and
    /// `static int g;` are object declarations. A definition cannot share its
    /// declarator list with another declarator. Pointer stars bind to each
    /// declarator, so `int *a, b;` declares one pointer and one `int`.
    fn parse_external(&mut self) -> Result<Vec<TopDecl>, Diagnostic> {
        let specs = self.parse_specs(SpecPlace::Declaration)?;
        let base = CType::from_name(specs.ty);
        let first = self.parse_named_decl(base.clone())?;
        if first.is_function() && self.current().kind == TokenKind::LBrace {
            return Ok(vec![self.finish_definition(specs, first)?]);
        }
        let mut decls = vec![self.finish_file_decl(&specs, first)?];
        while self.match_kind(TokenKind::Comma) {
            let next = self.parse_named_decl(base.clone())?;
            if next.is_function() && self.current().kind == TokenKind::LBrace {
                return Err(self.error("a function definition cannot follow another declarator"));
            }
            decls.push(self.finish_file_decl(&specs, next)?);
        }
        self.expect(TokenKind::Semi, "';'")?;
        Ok(decls)
    }

    fn finish_definition(
        &mut self,
        specs: Specs,
        declarator: NamedDecl,
    ) -> Result<TopDecl, Diagnostic> {
        let DeclKind::Function {
            ret,
            params,
            prototype: _,
        } = declarator.kind
        else {
            return Err(self.error_at(
                declarator.line,
                declarator.column,
                "expected a function definition",
            ));
        };
        for param in &params {
            if param.name.is_none() {
                return Err(self.error_at(
                    param.line,
                    param.column,
                    "parameter name omitted in function definition",
                ));
            }
        }
        let body = self.parse_compound()?;
        Ok(TopDecl::Func(FuncDef {
            ret_type: ret,
            name: declarator.name,
            params,
            body: Some(body),
            storage: specs.storage,
            linkage: Linkage::External,
            line: declarator.line,
            column: declarator.column,
        }))
    }

    fn finish_file_decl(
        &mut self,
        specs: &Specs,
        declarator: NamedDecl,
    ) -> Result<TopDecl, Diagnostic> {
        if let DeclKind::Function {
            ret,
            params,
            prototype,
        } = declarator.kind
        {
            if !prototype {
                return Err(self.error_at(
                    declarator.line,
                    declarator.column,
                    "old-style function declarations are not supported",
                ));
            }
            if self.current().kind == TokenKind::Assign {
                return Err(self.error("function declarations cannot have an initializer"));
            }
            return Ok(TopDecl::Func(FuncDef {
                ret_type: ret,
                name: declarator.name,
                params,
                body: None,
                storage: specs.storage,
                linkage: Linkage::External,
                line: declarator.line,
                column: declarator.column,
            }));
        }
        let DeclKind::Object(ty) = declarator.kind else {
            unreachable!("function handled above");
        };
        if ty.is_void() {
            return Err(self.error_at(
                declarator.line,
                declarator.column,
                "void variables are not supported",
            ));
        }
        let init = if self.match_kind(TokenKind::Assign) {
            Some(self.parse_initializer()?)
        } else {
            None
        };
        Ok(TopDecl::Global(GlobalDecl {
            name: declarator.name,
            ty,
            storage: specs.storage,
            init,
            line: declarator.line,
            column: declarator.column,
        }))
    }

    fn parse_specs(&mut self, where_: SpecPlace) -> Result<Specs, Diagnostic> {
        let mut flags = SpecFlags::default();
        loop {
            let token = self.current().clone();
            match token.kind {
                TokenKind::KwStatic | TokenKind::KwExtern => {
                    if where_ != SpecPlace::Declaration {
                        let message = if where_ == SpecPlace::Parameter {
                            "storage-class specifiers are not supported on parameters"
                        } else {
                            "storage-class specifiers are not allowed in this type name"
                        };
                        return Err(self.error(message));
                    }
                    if flags.storage != Storage::None {
                        return Err(self.error("multiple storage-class specifiers"));
                    }
                    self.advance();
                    flags.storage = if token.kind == TokenKind::KwStatic {
                        Storage::Static
                    } else {
                        Storage::Extern
                    };
                    flags.storage_at = Some((token.line, token.column));
                }
                TokenKind::KwVoid
                | TokenKind::KwChar
                | TokenKind::KwShort
                | TokenKind::KwInt
                | TokenKind::KwLong
                | TokenKind::KwSigned
                | TokenKind::KwUnsigned => {
                    self.advance();
                    self.note_type(&mut flags, &token)?;
                }
                kind => {
                    if let Some(message) = unsupported(kind) {
                        return Err(self.error(message));
                    }
                    break;
                }
            }
        }
        if !flags.any {
            return Err(self.error(format!("expected a type, found {}", found(self.current()))));
        }
        let ty = self.resolve_type(&flags)?;
        Ok(Specs {
            storage: flags.storage,
            storage_at: flags.storage_at,
            ty,
            line: flags.line,
            column: flags.column,
        })
    }

    fn note_type(&self, flags: &mut SpecFlags, token: &Token) -> Result<(), Diagnostic> {
        flags.any = true;
        flags.line = token.line;
        flags.column = token.column;
        let bad = self.error_at(
            token.line,
            token.column,
            "invalid combination of type specifiers",
        );
        match token.kind {
            TokenKind::KwVoid => {
                if flags.void {
                    return Err(bad);
                }
                flags.void = true;
            }
            TokenKind::KwChar => {
                if flags.char_ {
                    return Err(bad);
                }
                flags.char_ = true;
            }
            TokenKind::KwShort => {
                flags.short += 1;
                if flags.short > 1 {
                    return Err(bad);
                }
            }
            TokenKind::KwInt => {
                if flags.int_ {
                    return Err(bad);
                }
                flags.int_ = true;
            }
            TokenKind::KwLong => {
                flags.long += 1;
                if flags.long > 2 {
                    return Err(bad);
                }
            }
            TokenKind::KwSigned => {
                if flags.signed || flags.unsigned {
                    return Err(bad);
                }
                flags.signed = true;
            }
            TokenKind::KwUnsigned => {
                if flags.unsigned || flags.signed {
                    return Err(bad);
                }
                flags.unsigned = true;
            }
            _ => unreachable!("type specifier"),
        }
        Ok(())
    }

    fn resolve_type(&self, flags: &SpecFlags) -> Result<TypeName, Diagnostic> {
        let bad = self.error_at(
            flags.line,
            flags.column,
            "invalid combination of type specifiers",
        );
        if flags.void {
            if flags.char_
                || flags.short != 0
                || flags.int_
                || flags.long != 0
                || flags.signed
                || flags.unsigned
            {
                return Err(bad);
            }
            return Ok(TypeName::Void);
        }
        if flags.char_ {
            if flags.short != 0 || flags.int_ || flags.long != 0 {
                return Err(bad);
            }
            return Ok(if flags.unsigned {
                TypeName::UChar
            } else if flags.signed {
                TypeName::SChar
            } else {
                TypeName::Char
            });
        }
        if flags.short != 0 && flags.long != 0 {
            return Err(bad);
        }
        if flags.short == 1 {
            return Ok(if flags.unsigned {
                TypeName::UShort
            } else {
                TypeName::Short
            });
        }
        if flags.long == 2 {
            return Ok(if flags.unsigned {
                TypeName::ULongLong
            } else {
                TypeName::LongLong
            });
        }
        if flags.long == 1 {
            return Ok(if flags.unsigned {
                TypeName::ULong
            } else {
                TypeName::Long
            });
        }
        if flags.unsigned {
            return Ok(TypeName::UInt);
        }
        Ok(TypeName::Int)
    }

    /// A named declarator. `int *a, b` parses two of these against one base type.
    fn parse_named_decl(&mut self, base: CType) -> Result<NamedDecl, Diagnostic> {
        let raw = self.parse_raw_declarator(false)?;
        let (name, line, column) = match decl_name(&raw) {
            Some(found) => found,
            None => {
                let (line, column) = decl_pos(&raw);
                return Err(self.error_at(line, column, "expected an identifier"));
            }
        };
        let kind = self.layer_to_kind(self.derive(base, &raw, line, column)?)?;
        Ok(NamedDecl {
            name,
            line,
            column,
            kind,
        })
    }

    fn layer_to_kind(&self, layer: Layer) -> Result<DeclKind, Diagnostic> {
        Ok(match layer {
            Layer::Object(ty) => DeclKind::Object(ty),
            Layer::Function {
                ret,
                params,
                prototype,
            } => DeclKind::Function {
                ret,
                params,
                prototype,
            },
        })
    }

    fn parse_param_list(&mut self) -> Result<ParsedParams, Diagnostic> {
        self.expect(TokenKind::LParen, "'('")?;
        if self.match_kind(TokenKind::RParen) {
            return Ok(ParsedParams {
                params: Vec::new(),
                prototype: false,
            });
        }
        if self.current().kind == TokenKind::KwVoid && self.lookahead() == TokenKind::RParen {
            self.advance();
            self.advance();
            return Ok(ParsedParams {
                params: Vec::new(),
                prototype: true,
            });
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
        Ok(ParsedParams {
            params,
            prototype: true,
        })
    }

    fn parse_param(&mut self) -> Result<Param, Diagnostic> {
        if self.current().kind == TokenKind::Ellipsis {
            return Err(self.error("variadic functions are not supported"));
        }
        let specs = self.parse_specs(SpecPlace::Parameter)?;
        let base = CType::from_name(specs.ty);
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
                return Ok(Param {
                    name: None,
                    ty: base,
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
        let named = decl_name(&raw);
        let (line, column) = named
            .as_ref()
            .map(|(_, line, column)| (*line, *column))
            .unwrap_or_else(|| decl_pos(&raw));
        let ty = match self.derive(base, &raw, line, column)? {
            Layer::Object(ty) => self.adjust_parameter(ty, line, column)?,
            Layer::Function { .. } => {
                return Err(self.error_at(line, column, "function pointers are not supported"));
            }
        };
        Ok(Param {
            name: named.map(|(name, _, _)| name),
            ty,
            line,
            column,
            slot: String::new(),
            arg_reg: String::new(),
        })
    }

    /// Outermost array parameters become pointers (C11 6.7.6.3). A parameter
    /// of function type would become a function pointer; this subset rejects it.
    fn adjust_parameter(&self, ty: CType, line: usize, column: usize) -> Result<CType, Diagnostic> {
        match ty {
            CType::Array(elem, _) => Ok(CType::pointer(*elem)),
            CType::Void => Err(self.error_at(line, column, "parameter has incomplete type 'void'")),
            other if !other.is_complete() => Err(self.error_at(
                line,
                column,
                format!("parameter has incomplete type '{}'", other.display()),
            )),
            other => Ok(other),
        }
    }

    fn starts_raw_declarator(&self) -> bool {
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
    fn parse_raw_declarator(&mut self, abstract_ok: bool) -> Result<RawDecl, Diagnostic> {
        if self.current().kind == TokenKind::Star {
            self.advance();
            if let Some(message) = unsupported(self.current().kind) {
                return Err(self.error(message));
            }
            let inner = self.parse_raw_declarator(abstract_ok)?;
            return Ok(RawDecl::Pointer(Box::new(inner)));
        }
        self.parse_direct(abstract_ok)
    }

    fn parse_direct(&mut self, abstract_ok: bool) -> Result<RawDecl, Diagnostic> {
        let mut node = if self.current().kind == TokenKind::LParen {
            let next = self.lookahead();
            if abstract_ok && self.can_start_param(next) {
                let line = self.current().line;
                let column = self.current().column;
                let params = self.parse_param_list()?;
                RawDecl::Func(Box::new(RawDecl::Abstract(line, column)), params)
            } else {
                self.advance();
                let inner = self.parse_raw_declarator(abstract_ok)?;
                self.expect(TokenKind::RParen, "')'")?;
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
                    self.advance();
                    let len = self.parse_array_len()?;
                    self.expect(TokenKind::RBrack, "']'")?;
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

    fn can_start_param(&self, kind: TokenKind) -> bool {
        is_type_specifier(kind)
            || matches!(kind, TokenKind::RParen | TokenKind::Ellipsis)
            || unsupported(kind).is_some()
    }

    /// `[]` is an unknown bound. Anything else must be an integer constant
    /// greater than zero. An identifier is a variable-length array.
    fn parse_array_len(&mut self) -> Result<Option<u64>, Diagnostic> {
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
        match fold_array_bound(&expr) {
            Ok(len) => Ok(Some(len)),
            Err(err) => Err(self.error_at(expr.line(), expr.column(), bound_message(err))),
        }
    }

    /// Build the type by inserting each constructor next to the base, which is
    /// what makes `[]` and `()` bind tighter than `*`.
    fn derive(
        &self,
        base: CType,
        raw: &RawDecl,
        line: usize,
        column: usize,
    ) -> Result<Layer, Diagnostic> {
        self.derive_layer(Layer::Object(base), raw, line, column)
    }

    fn derive_layer(
        &self,
        base: Layer,
        raw: &RawDecl,
        line: usize,
        column: usize,
    ) -> Result<Layer, Diagnostic> {
        match raw {
            RawDecl::Named(_, _, _) | RawDecl::Abstract(_, _) => Ok(base),
            RawDecl::Pointer(inner) => match base {
                Layer::Object(ty) => {
                    self.derive_layer(Layer::Object(CType::pointer(ty)), inner, line, column)
                }
                Layer::Function { .. } => {
                    Err(self.error_at(line, column, "function pointers are not supported"))
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

    fn make_array(
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

    fn parse_compound(&mut self) -> Result<Compound, Diagnostic> {
        let brace = self.expect(TokenKind::LBrace, "'{'")?;
        let mut items = Vec::new();
        while !matches!(self.current().kind, TokenKind::RBrace | TokenKind::Eof) {
            if starts_declaration(self.current().kind) {
                for decl in self.parse_block_declaration()? {
                    items.push(Stmt::Decl(decl));
                }
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

    fn parse_block_declaration(&mut self) -> Result<Vec<VarDecl>, Diagnostic> {
        let specs = self.parse_specs(SpecPlace::Declaration)?;
        if let Some((line, column)) = specs.storage_at {
            let message = match specs.storage {
                Storage::Static => "static local variables are not supported",
                Storage::Extern => "block-scope extern declarations are not supported",
                Storage::None => unreachable!("storage location without a storage class"),
            };
            return Err(self.error_at(line, column, message));
        }
        let base = CType::from_name(specs.ty);
        let mut decls = Vec::new();
        loop {
            let declarator = self.parse_named_decl(base.clone())?;
            if declarator.is_function() {
                return Err(self.error_at(
                    declarator.line,
                    declarator.column,
                    "nested functions are not supported",
                ));
            }
            let DeclKind::Object(ty) = declarator.kind else {
                unreachable!("function rejected above");
            };
            if ty.is_void() {
                return Err(self.error_at(
                    declarator.line,
                    declarator.column,
                    "void variables are not supported",
                ));
            }
            let init = if self.match_kind(TokenKind::Assign) {
                Some(self.parse_initializer()?)
            } else {
                None
            };
            decls.push(VarDecl {
                name: declarator.name,
                ty,
                init,
                checked: None,
                line: declarator.line,
                column: declarator.column,
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
            TokenKind::KwInt
            | TokenKind::KwVoid
            | TokenKind::KwChar
            | TokenKind::KwShort
            | TokenKind::KwLong
            | TokenKind::KwSigned
            | TokenKind::KwUnsigned
            | TokenKind::KwStatic
            | TokenKind::KwExtern => Err(self.error("a declaration is not allowed as a statement")),
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
        } else if starts_declaration(self.current().kind) {
            ForInit::Decl(self.parse_block_declaration()?)
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

    /// Comma operator. Argument lists and declarators call
    /// [`Self::parse_assignment`] instead, so those commas stay separators.
    fn parse_expression(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_assignment()?;
        while self.current().kind == TokenKind::Comma {
            let op = self.advance();
            let right = self.parse_assignment()?;
            left = Expr::comma(left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_assignment(&mut self) -> Result<Expr, Diagnostic> {
        let left = self.parse_conditional()?;
        let Some(kind) = assign_kind(self.current().kind) else {
            return Ok(left);
        };
        self.advance();
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
    fn parse_conditional(&mut self) -> Result<Expr, Diagnostic> {
        let cond = self.parse_logical_or()?;
        if !self.match_kind(TokenKind::Question) {
            return Ok(cond);
        }
        let line = cond.line();
        let column = cond.column();
        let then_expr = self.parse_expression()?;
        self.expect(TokenKind::Colon, "':'")?;
        let else_expr = self.parse_conditional()?;
        Ok(Expr::cond(cond, then_expr, else_expr, line, column))
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
        let mut left = self.parse_bit_or()?;
        while self.current().kind == TokenKind::AndAnd {
            let op = self.advance();
            let right = self.parse_bit_or()?;
            left = Expr::bin(BinOp::And, left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_bit_or(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_bit_xor()?;
        while self.current().kind == TokenKind::Pipe {
            let op = self.advance();
            let right = self.parse_bit_xor()?;
            left = Expr::bin(BinOp::BitOr, left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_bit_xor(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_bit_and()?;
        while self.current().kind == TokenKind::Caret {
            let op = self.advance();
            let right = self.parse_bit_and()?;
            left = Expr::bin(BinOp::BitXor, left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_bit_and(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_equality()?;
        while self.current().kind == TokenKind::Amp {
            let op = self.advance();
            let right = self.parse_equality()?;
            left = Expr::bin(BinOp::BitAnd, left, right, op.line, op.column);
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

    fn parse_shift(&mut self) -> Result<Expr, Diagnostic> {
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
            TokenKind::LParen if looks_like_type(self.lookahead()) => self.parse_cast(),
            TokenKind::Plus | TokenKind::Minus | TokenKind::Bang | TokenKind::Tilde => {
                self.advance();
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
                self.advance();
                let op = if token.kind == TokenKind::PlusPlus {
                    IncOp::PreInc
                } else {
                    IncOp::PreDec
                };
                let expr = self.parse_unary()?;
                Ok(Expr::incdec(op, expr, token.line, token.column))
            }
            TokenKind::KwSizeof => self.parse_sizeof(token.line, token.column),
            TokenKind::Amp => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::unary(UnaryOp::Addr, expr, token.line, token.column))
            }
            TokenKind::Star => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::unary(UnaryOp::Deref, expr, token.line, token.column))
            }
            _ => self.parse_postfix(),
        }
    }

    /// `=` initializer: an assignment expression, or a brace list. A trailing
    /// comma is allowed. An empty list is not (C11).
    fn parse_initializer(&mut self) -> Result<Init, Diagnostic> {
        if self.current().kind != TokenKind::LBrace {
            return Ok(Init::Expr(self.parse_assignment()?));
        }
        let brace = self.advance();
        if self.current().kind == TokenKind::RBrace {
            return Err(self.error("empty initializer list"));
        }
        let mut items = Vec::new();
        loop {
            items.push(self.parse_initializer()?);
            if self.match_kind(TokenKind::Comma) {
                if self.current().kind == TokenKind::RBrace {
                    break;
                }
                continue;
            }
            break;
        }
        self.expect(TokenKind::RBrace, "'}'")?;
        Ok(Init::List {
            items,
            line: brace.line,
            column: brace.column,
        })
    }

    /// `sizeof (type-name)` when the parenthesis starts a type. Otherwise
    /// `sizeof unary-expression`. The operand is not evaluated later.
    fn parse_sizeof(&mut self, line: usize, column: usize) -> Result<Expr, Diagnostic> {
        self.advance();
        if self.current().kind == TokenKind::LParen && looks_like_type(self.lookahead()) {
            self.advance();
            let ty = self.parse_type_name()?;
            self.expect(TokenKind::RParen, "')'")?;
            return Ok(Expr::sizeof_type(ty, line, column));
        }
        let expr = self.parse_unary()?;
        Ok(Expr::sizeof_expr(expr, line, column))
    }

    fn parse_cast(&mut self) -> Result<Expr, Diagnostic> {
        let lparen = self.advance();
        let ty = self.parse_type_name()?;
        self.expect(TokenKind::RParen, "')'")?;
        let expr = self.parse_unary()?;
        Ok(Expr::cast(ty, expr, lparen.line, lparen.column))
    }

    /// A type name is specifiers plus an optional abstract declarator.
    /// `int *`, `int (*)[3]`, and `int [3]` are accepted. A function type or a
    /// function pointer is rejected.
    fn parse_type_name(&mut self) -> Result<CType, Diagnostic> {
        let specs = self.parse_specs(SpecPlace::TypeName)?;
        let base = CType::from_name(specs.ty);
        if !self.starts_raw_declarator() {
            return Ok(base);
        }
        let raw = self.parse_raw_declarator(true)?;
        if let Some((_, line, column)) = decl_name(&raw) {
            return Err(self.error_at(line, column, "type name cannot declare an identifier"));
        }
        let (line, column) = decl_pos(&raw);
        match self.derive(base, &raw, line, column)? {
            Layer::Object(ty) => Ok(ty),
            Layer::Function { .. } => {
                Err(self.error_at(line, column, "function types are not supported here"))
            }
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
                TokenKind::LBrack => {
                    let bracket = self.advance();
                    let index = self.parse_expression()?;
                    self.expect(TokenKind::RBrack, "']'")?;
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
                    token.int_ty.expect("int token type"),
                    token.line,
                    token.column,
                ))
            }
            TokenKind::Ident => {
                self.advance();
                Ok(Expr::var(token.spelling, token.line, token.column))
            }
            TokenKind::LParen => {
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
        self.error_at(token.line, token.column, message)
    }

    fn error_at(&self, line: usize, column: usize, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(self.filename, line, column, message)
    }
}

struct Specs {
    storage: Storage,
    storage_at: Option<(usize, usize)>,
    ty: TypeName,
    line: usize,
    column: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SpecPlace {
    Declaration,
    Parameter,
    TypeName,
}

#[derive(Default)]
struct SpecFlags {
    storage: Storage,
    storage_at: Option<(usize, usize)>,
    void: bool,
    char_: bool,
    short: u8,
    int_: bool,
    long: u8,
    signed: bool,
    unsigned: bool,
    any: bool,
    line: usize,
    column: usize,
}

struct NamedDecl {
    name: String,
    line: usize,
    column: usize,
    kind: DeclKind,
}

impl NamedDecl {
    fn is_function(&self) -> bool {
        matches!(self.kind, DeclKind::Function { .. })
    }
}

enum DeclKind {
    Object(CType),
    Function {
        ret: CType,
        params: Vec<Param>,
        prototype: bool,
    },
}

enum Layer {
    Object(CType),
    Function {
        ret: CType,
        params: Vec<Param>,
        prototype: bool,
    },
}

enum RawDecl {
    Named(String, usize, usize),
    Abstract(usize, usize),
    Pointer(Box<RawDecl>),
    Array(Box<RawDecl>, Option<u64>),
    Func(Box<RawDecl>, ParsedParams),
}

struct ParsedParams {
    params: Vec<Param>,
    /// `false` only for an empty `()` list, which is not a prototype.
    prototype: bool,
}

fn decl_name(raw: &RawDecl) -> Option<(String, usize, usize)> {
    match raw {
        RawDecl::Named(name, line, column) => Some((name.clone(), *line, *column)),
        RawDecl::Abstract(_, _) => None,
        RawDecl::Pointer(inner) | RawDecl::Array(inner, _) | RawDecl::Func(inner, _) => {
            decl_name(inner)
        }
    }
}

fn decl_pos(raw: &RawDecl) -> (usize, usize) {
    match raw {
        RawDecl::Named(_, line, column) | RawDecl::Abstract(line, column) => (*line, *column),
        RawDecl::Pointer(inner) | RawDecl::Array(inner, _) | RawDecl::Func(inner, _) => {
            decl_pos(inner)
        }
    }
}

enum BoundErr {
    Vla,
    Incomplete,
    NonPositive,
    TooBig,
    DivZero,
    Overflow,
    Shift,
}

fn bound_message(err: BoundErr) -> &'static str {
    match err {
        BoundErr::Vla => "variable-length arrays are not supported",
        BoundErr::Incomplete => "array size uses an incomplete type",
        BoundErr::NonPositive => "array size must be positive",
        BoundErr::TooBig => "array is too large",
        BoundErr::DivZero => "array size divides by zero",
        BoundErr::Overflow => "array size overflows",
        BoundErr::Shift => "array size uses an undefined shift",
    }
}

struct BVal {
    bits: u64,
    ty: TypeName,
}

/// Integer constant expression used as an array bound. An identifier, a call,
/// or an assignment is reported as a variable-length array.
fn fold_array_bound(expr: &Expr) -> Result<u64, BoundErr> {
    let value = eval_ice(expr)?;
    let count = if value.ty.is_signed() {
        let signed = sign_extend(value.bits, value.ty.width());
        if signed <= 0 {
            return Err(BoundErr::NonPositive);
        }
        signed as u64
    } else {
        let unsigned = value.bits & value.ty.mask();
        if unsigned == 0 {
            return Err(BoundErr::NonPositive);
        }
        unsigned
    };
    if count > u64::MAX / 8 {
        return Err(BoundErr::TooBig);
    }
    Ok(count)
}

fn eval_ice(expr: &Expr) -> Result<BVal, BoundErr> {
    match expr {
        Expr::Int { bits, value_ty, .. } => Ok(BVal {
            bits: *bits & value_ty.mask(),
            ty: *value_ty,
        }),
        Expr::Sizeof { operand, .. } => {
            let size = match operand {
                SizeofOperand::Type(ty) => ty.size().ok_or(BoundErr::Incomplete)?,
                SizeofOperand::Expr(inner) => {
                    type_only(inner)?.size().ok_or(BoundErr::Incomplete)?
                }
            };
            Ok(BVal {
                bits: size,
                ty: TypeName::ULong,
            })
        }
        Expr::Unary { op, expr, .. } => eval_ice_unary(*op, expr),
        Expr::Bin {
            op, left, right, ..
        } => eval_ice_bin(*op, left, right),
        Expr::Cond {
            cond,
            then_expr,
            else_expr,
            ..
        } => {
            let cond = eval_ice(cond)?;
            let chosen = if cond.bits & cond.ty.mask() == 0 {
                else_expr
            } else {
                then_expr
            };
            let value = eval_ice(chosen)?;
            let result = match (type_only(then_expr)?, type_only(else_expr)?) {
                (left, right) if left.is_integer() && right.is_integer() => left
                    .as_integer()
                    .unwrap()
                    .usual(right.as_integer().unwrap()),
                _ => return Err(BoundErr::Vla),
            };
            Ok(BVal {
                bits: convert_bits(value.bits, value.ty, result),
                ty: result,
            })
        }
        Expr::Cast { ty_name, expr, .. } => {
            let dest = ty_name.as_integer().ok_or(BoundErr::Vla)?;
            let value = eval_ice(expr)?;
            Ok(BVal {
                bits: convert_bits(value.bits, value.ty, dest),
                ty: dest,
            })
        }
        Expr::Var { .. }
        | Expr::Assign { .. }
        | Expr::Call { .. }
        | Expr::CompoundAssign { .. }
        | Expr::IncDec { .. }
        | Expr::Comma { .. }
        | Expr::Index { .. }
        | Expr::Decay { .. } => Err(BoundErr::Vla),
    }
}

fn type_only(expr: &Expr) -> Result<CType, BoundErr> {
    match expr {
        Expr::Int { value_ty, .. } => Ok(CType::from_name(*value_ty)),
        Expr::Sizeof { .. } => Ok(CType::from_name(TypeName::ULong)),
        Expr::Unary { op, expr, .. } => {
            let inner = type_only(expr)?.as_integer().ok_or(BoundErr::Vla)?;
            let ty = match op {
                UnaryOp::Not => TypeName::Int,
                UnaryOp::Plus | UnaryOp::Minus | UnaryOp::BitNot => inner.promote(),
                UnaryOp::Addr | UnaryOp::Deref => return Err(BoundErr::Vla),
            };
            Ok(CType::from_name(ty))
        }
        Expr::Bin {
            op, left, right, ..
        } => {
            let left = type_only(left)?.as_integer().ok_or(BoundErr::Vla)?;
            let right = type_only(right)?.as_integer().ok_or(BoundErr::Vla)?;
            let ty = if op.is_cmp() || matches!(op, BinOp::And | BinOp::Or) {
                TypeName::Int
            } else if matches!(op, BinOp::Shl | BinOp::Shr) {
                left.promote()
            } else {
                left.usual(right)
            };
            Ok(CType::from_name(ty))
        }
        Expr::Cast { ty_name, .. } => {
            if ty_name.is_integer() {
                Ok(ty_name.clone())
            } else {
                Err(BoundErr::Vla)
            }
        }
        Expr::Cond {
            then_expr,
            else_expr,
            ..
        } => {
            let left = type_only(then_expr)?.as_integer().ok_or(BoundErr::Vla)?;
            let right = type_only(else_expr)?.as_integer().ok_or(BoundErr::Vla)?;
            Ok(CType::from_name(left.usual(right)))
        }
        _ => Err(BoundErr::Vla),
    }
}

fn eval_ice_unary(op: UnaryOp, expr: &Expr) -> Result<BVal, BoundErr> {
    let value = promote_ice(eval_ice(expr)?);
    match op {
        UnaryOp::Plus => Ok(value),
        UnaryOp::Minus => {
            if value.ty.is_signed() {
                let signed = sign_extend(value.bits, value.ty.width()) as i128;
                bits_ice(signed.checked_neg().ok_or(BoundErr::Overflow)?, value.ty)
            } else {
                Ok(BVal {
                    bits: value.bits.wrapping_neg() & value.ty.mask(),
                    ty: value.ty,
                })
            }
        }
        UnaryOp::Not => {
            let zero = value.bits & value.ty.mask() == 0;
            Ok(BVal {
                bits: u64::from(!zero),
                ty: TypeName::Int,
            })
        }
        UnaryOp::BitNot => Ok(BVal {
            bits: (!value.bits) & value.ty.mask(),
            ty: value.ty,
        }),
        UnaryOp::Addr | UnaryOp::Deref => Err(BoundErr::Vla),
    }
}

fn promote_ice(value: BVal) -> BVal {
    let ty = value.ty.promote();
    BVal {
        bits: convert_bits(value.bits, value.ty, ty),
        ty,
    }
}

fn bits_ice(value: i128, ty: TypeName) -> Result<BVal, BoundErr> {
    let width = ty.width();
    let min = -((1i128) << (width - 1));
    let max = ((1i128) << (width - 1)) - 1;
    if value < min || value > max {
        return Err(BoundErr::Overflow);
    }
    Ok(BVal {
        bits: (value as u64) & ty.mask(),
        ty,
    })
}

fn eval_ice_bin(op: BinOp, left: &Expr, right: &Expr) -> Result<BVal, BoundErr> {
    match op {
        BinOp::And => {
            let left = eval_ice(left)?;
            if left.bits & left.ty.mask() == 0 {
                return Ok(BVal {
                    bits: 0,
                    ty: TypeName::Int,
                });
            }
            let right = eval_ice(right)?;
            Ok(BVal {
                bits: u64::from(right.bits & right.ty.mask() != 0),
                ty: TypeName::Int,
            })
        }
        BinOp::Or => {
            let left = eval_ice(left)?;
            if left.bits & left.ty.mask() != 0 {
                return Ok(BVal {
                    bits: 1,
                    ty: TypeName::Int,
                });
            }
            let right = eval_ice(right)?;
            Ok(BVal {
                bits: u64::from(right.bits & right.ty.mask() != 0),
                ty: TypeName::Int,
            })
        }
        BinOp::Shl | BinOp::Shr => {
            let left = promote_ice(eval_ice(left)?);
            let right = promote_ice(eval_ice(right)?);
            let count = ice_shift_count(&right, left.ty.width())?;
            if op == BinOp::Shr {
                let bits = if left.ty.is_signed() {
                    (sign_extend(left.bits, left.ty.width()) >> count) as u64 & left.ty.mask()
                } else {
                    (left.bits & left.ty.mask()) >> count
                };
                return Ok(BVal { bits, ty: left.ty });
            }
            if left.ty.is_signed() {
                let signed = sign_extend(left.bits, left.ty.width()) as i128;
                if signed < 0 {
                    return Err(BoundErr::Shift);
                }
                bits_ice(signed << count, left.ty)
            } else {
                Ok(BVal {
                    bits: ((left.bits & left.ty.mask()) << count) & left.ty.mask(),
                    ty: left.ty,
                })
            }
        }
        _ => {
            let left = eval_ice(left)?;
            let right = eval_ice(right)?;
            let ty = left.ty.usual(right.ty);
            let left = convert_bits(left.bits, left.ty, ty);
            let right = convert_bits(right.bits, right.ty, ty);
            if op.is_cmp() {
                let bit = ice_compare(op, left, right, ty);
                return Ok(BVal {
                    bits: u64::from(bit),
                    ty: TypeName::Int,
                });
            }
            ice_arith(op, left, right, ty)
        }
    }
}

fn ice_shift_count(count: &BVal, width: u32) -> Result<u32, BoundErr> {
    if count.ty.is_signed() {
        let signed = sign_extend(count.bits, count.ty.width());
        if signed < 0 || signed >= i64::from(width) {
            return Err(BoundErr::Shift);
        }
        Ok(signed as u32)
    } else {
        let unsigned = count.bits & count.ty.mask();
        if unsigned >= u64::from(width) {
            return Err(BoundErr::Shift);
        }
        Ok(unsigned as u32)
    }
}

fn ice_compare(op: BinOp, left: u64, right: u64, ty: TypeName) -> bool {
    if ty.is_signed() {
        let left = sign_extend(left, ty.width());
        let right = sign_extend(right, ty.width());
        match op {
            BinOp::Eq => left == right,
            BinOp::Ne => left != right,
            BinOp::Lt => left < right,
            BinOp::Gt => left > right,
            BinOp::Le => left <= right,
            BinOp::Ge => left >= right,
            _ => false,
        }
    } else {
        let left = left & ty.mask();
        let right = right & ty.mask();
        match op {
            BinOp::Eq => left == right,
            BinOp::Ne => left != right,
            BinOp::Lt => left < right,
            BinOp::Gt => left > right,
            BinOp::Le => left <= right,
            BinOp::Ge => left >= right,
            _ => false,
        }
    }
}

fn ice_arith(op: BinOp, left: u64, right: u64, ty: TypeName) -> Result<BVal, BoundErr> {
    if ty.is_signed() {
        let left = sign_extend(left, ty.width()) as i128;
        let right = sign_extend(right, ty.width()) as i128;
        if matches!(op, BinOp::Div | BinOp::Mod) && right == 0 {
            return Err(BoundErr::DivZero);
        }
        if matches!(op, BinOp::Div | BinOp::Mod)
            && left == -((1i128) << (ty.width() - 1))
            && right == -1
        {
            return Err(BoundErr::Overflow);
        }
        let value = match op {
            BinOp::Add => left.checked_add(right),
            BinOp::Sub => left.checked_sub(right),
            BinOp::Mul => left.checked_mul(right),
            BinOp::Div => Some(left / right),
            BinOp::Mod => Some(left % right),
            BinOp::BitAnd => Some(left & right),
            BinOp::BitOr => Some(left | right),
            BinOp::BitXor => Some(left ^ right),
            _ => return Err(BoundErr::Vla),
        };
        bits_ice(value.ok_or(BoundErr::Overflow)?, ty)
    } else {
        let mask = ty.mask();
        let left = left & mask;
        let right = right & mask;
        if matches!(op, BinOp::Div | BinOp::Mod) && right == 0 {
            return Err(BoundErr::DivZero);
        }
        let bits = match op {
            BinOp::Add => left.wrapping_add(right),
            BinOp::Sub => left.wrapping_sub(right),
            BinOp::Mul => left.wrapping_mul(right),
            BinOp::Div => left / right,
            BinOp::Mod => left % right,
            BinOp::BitAnd => left & right,
            BinOp::BitOr => left | right,
            BinOp::BitXor => left ^ right,
            _ => return Err(BoundErr::Vla),
        };
        Ok(BVal {
            bits: bits & mask,
            ty,
        })
    }
}

enum AssignKind {
    Plain,
    Compound(BinOp),
}

fn assign_kind(kind: TokenKind) -> Option<AssignKind> {
    Some(match kind {
        TokenKind::Assign => AssignKind::Plain,
        TokenKind::PlusEq => AssignKind::Compound(BinOp::Add),
        TokenKind::MinusEq => AssignKind::Compound(BinOp::Sub),
        TokenKind::StarEq => AssignKind::Compound(BinOp::Mul),
        TokenKind::SlashEq => AssignKind::Compound(BinOp::Div),
        TokenKind::PercentEq => AssignKind::Compound(BinOp::Mod),
        TokenKind::AmpEq => AssignKind::Compound(BinOp::BitAnd),
        TokenKind::PipeEq => AssignKind::Compound(BinOp::BitOr),
        TokenKind::CaretEq => AssignKind::Compound(BinOp::BitXor),
        TokenKind::ShlEq => AssignKind::Compound(BinOp::Shl),
        TokenKind::ShrEq => AssignKind::Compound(BinOp::Shr),
        _ => return None,
    })
}

/// A token that can begin a type name in `sizeof ( type-name )` or a cast.
fn looks_like_type(kind: TokenKind) -> bool {
    is_type_specifier(kind) || unsupported(kind).is_some()
}

fn is_type_specifier(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::KwVoid
            | TokenKind::KwChar
            | TokenKind::KwShort
            | TokenKind::KwInt
            | TokenKind::KwLong
            | TokenKind::KwSigned
            | TokenKind::KwUnsigned
    )
}

fn starts_declaration(kind: TokenKind) -> bool {
    is_type_specifier(kind) || matches!(kind, TokenKind::KwStatic | TokenKind::KwExtern)
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
        TokenKind::KwFloat => "type 'float' is not supported",
        TokenKind::KwDouble => "type 'double' is not supported",
        TokenKind::KwStruct => "structs are not supported",
        TokenKind::KwUnion => "unions are not supported",
        TokenKind::KwEnum => "enums are not supported",
        TokenKind::KwTypedef => "typedef is not supported",
        TokenKind::KwConst | TokenKind::KwVolatile | TokenKind::KwRestrict => {
            "type qualifiers are not supported"
        }
        TokenKind::KwRegister => "register is not supported",
        TokenKind::KwAuto => "auto is not supported",
        TokenKind::KwInline => "inline is not supported",
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
  param int a
  param int b
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
    decl int sum =
      int 0
    for
      init
        decl int i =
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
    fn pointer_and_array_declarators() {
        assert_eq!(
            parsed(
                "\
int *a[3];
int (*p)[3];
int **pp;
void *vp;
int f(int *a, int b[]);
int *g(int x) { return 0; }
"
            ),
            "\
global int *[3] a
global int (*)[3] p
global int ** pp
global void * vp
proto int f
  param int * a
  param int * b
func int * g
  param int x
  block
    return
      int 0
"
        );
        assert_eq!(
            parsed("int main(void) { return a[i] + *p + &x; }\n"),
            "\
func int main
  block
    return
      +
        +
          index
            var a
            var i
          deref
            var p
        addr
          var x
"
        );
        assert_eq!(
            parsed("int a[] = {1, 2, 3};\n"),
            "\
global int [] a =
  list
    int 1
    int 2
    int 3
"
        );
    }

    #[test]
    fn syntax_errors_name_the_problem() {
        assert!(parse_err("int main(void) { return 1 }").contains("expected ';'"));
        assert!(parse_err("int main(void { return 0; }").contains("expected ')'"));
        assert!(parse_err("int main(void) { float c; return 0; }")
            .contains("type 'float' is not supported"));
        assert!(parse_err("struct S { int x; };").contains("structs are not supported"));
        assert!(parse_err("int main(void) { return sizeof(float); }").contains("type 'float'"));
        assert!(parse_err("int (*fp)(void);").contains("function pointers are not supported"));
        assert!(parse_err("int a[n];").contains("variable-length arrays are not supported"));
        assert!(parse_err("int a[0];").contains("array size must be positive"));
        assert!(parse_err("\"hi\";").contains("string literals"));
        assert!(parse_err("int main(void) { return a ? b; }").contains("expected ':'"));
        assert!(parse_err("int add();").contains("old-style function declarations"));
        assert!(
            parse_err("int main(void) { int inner(void) { return 1; } }")
                .contains("nested functions")
        );
        assert!(parse_err("int main(void) { int inner(void); }").contains("nested functions"));
        assert!(parse_err("int f(int, int) { return 0; }").contains("parameter name omitted"));
        assert!(parse_err("int main(void) { static int x; }").contains("static local"));
        assert!(parse_err("int main(void) { extern int x; }").contains("block-scope extern"));
        assert!(parse_err("int f(void) = 1;").contains("cannot have an initializer"));
        assert!(parse_err("static extern int x;").contains("multiple storage-class"));
        assert!(parse_err("void g;").contains("void variables"));
        assert!(parse_err("int f(static int a);").contains("not supported on parameters"));
        assert!(parse_err("int main(void) { switch (1) {} }").contains("switch is not supported"));
        assert!(parse_err("int main(void) { do {} while (0); }").contains("do-while"));
        assert!(parse_err("int main(void) { else; }").contains("'else' without a matching 'if'"));
        assert!(parse_err("int main(void) { return (float)1; }").contains("type 'float'"));
        assert!(parse_err("int main(void) { int (*fp)(int); return 0; }")
            .contains("function pointers are not supported"));
        assert!(parse_err("short long x;").contains("invalid combination of type specifiers"));
        assert!(parse_err("unsigned signed x;").contains("invalid combination of type specifiers"));
        assert!(parse_err("long long long x;").contains("invalid combination of type specifiers"));
        assert!(parse_err("int int x;").contains("invalid combination of type specifiers"));
        assert!(parse_err("char int x;").contains("invalid combination of type specifiers"));
        assert!(parse_err("int f(int a, ...) { return a; }").contains("variadic"));
    }

    #[test]
    fn integer_operator_precedence() {
        assert_eq!(
            parsed("int main(void) { return 1 + 2 << 3; }\n"),
            "\
func int main
  block
    return
      <<
        +
          int 1
          int 2
        int 3
"
        );
        assert_eq!(
            parsed("int main(void) { return 1 << 2 + 3; }\n"),
            "\
func int main
  block
    return
      <<
        int 1
        +
          int 2
          int 3
"
        );
        assert_eq!(
            parsed("int main(void) { return 5 & 3 == 1; }\n"),
            "\
func int main
  block
    return
      &
        int 5
        ==
          int 3
          int 1
"
        );
        assert_eq!(
            parsed("int main(void) { return 1 | 2 & 3 ^ 4; }\n"),
            "\
func int main
  block
    return
      |
        int 1
        ^
          &
            int 2
            int 3
          int 4
"
        );
        assert_eq!(
            parsed("int main(void) { return 1 || 2 | 3 && 4; }\n"),
            "\
func int main
  block
    return
      ||
        int 1
        &&
          |
            int 2
            int 3
          int 4
"
        );
        assert_eq!(
            parsed("int main(void) { return a ? b : c ? d : e; }\n"),
            "\
func int main
  block
    return
      ?:
        var a
        var b
        ?:
          var c
          var d
          var e
"
        );
        assert_eq!(
            parsed("int main(void) { return a ? b, c : d; }\n"),
            "\
func int main
  block
    return
      ?:
        var a
        ,
          var b
          var c
        var d
"
        );
        assert_eq!(
            parsed("int main(void) { return a ? b : c, d; }\n"),
            "\
func int main
  block
    return
      ,
        ?:
          var a
          var b
          var c
        var d
"
        );
        assert_eq!(
            parsed("int main(void) { a = b ? c : d; }\n"),
            "\
func int main
  block
    expr
      assign
        var a
        ?:
          var b
          var c
          var d
"
        );
        assert_eq!(
            parsed("int main(void) { a ? b : c = d; }\n"),
            "\
func int main
  block
    expr
      assign
        ?:
          var a
          var b
          var c
        var d
"
        );
        assert_eq!(
            parsed("int main(void) { a += b += 1; }\n"),
            "\
func int main
  block
    expr
      +=
        var a
        +=
          var b
          int 1
"
        );
        assert_eq!(
            parsed("int main(void) { x <<= 2; y >>= 1; }\n"),
            "\
func int main
  block
    expr
      <<=
        var x
        int 2
    expr
      >>=
        var y
        int 1
"
        );
        assert_eq!(
            parsed("int main(void) { ++x; x--; }\n"),
            "\
func int main
  block
    expr
      pre++
        var x
    expr
      post--
        var x
"
        );
        assert_eq!(
            parsed("int main(void) { return ~1 + !2; }\n"),
            "\
func int main
  block
    return
      +
        bnot
          int 1
        not
          int 2
"
        );
        assert_eq!(
            parsed("int main(void) { return sizeof(int) + sizeof x; }\n"),
            "\
func int main
  block
    return
      +
        sizeof int
        sizeof
          var x
"
        );
        assert_eq!(
            parsed("int main(void) { return (a = 1, b = 2); }\n"),
            "\
func int main
  block
    return
      ,
        assign
          var a
          int 1
        assign
          var b
          int 2
"
        );
        assert_eq!(
            parsed("int add(int a, int b); int main(void) { return add(1, (2, 3)); }\n"),
            "\
proto int add
  param int a
  param int b
func int main
  block
    return
      call add
        int 1
        ,
          int 2
          int 3
"
        );
        assert_eq!(
            parsed("int a = 1 << 4, b = sizeof(int);\n"),
            "\
global int a =
  <<
    int 1
    int 4
global int b =
  sizeof int
"
        );
    }

    #[test]
    fn prototypes_and_globals() {
        assert_eq!(
            parsed(
                "\
static int counter;
extern int shared;
int ready = 1 + 2;
int add(int a, int b);
int sum(int, int);
int add(int a, int b) { return a + b; }
"
            ),
            "\
static global int counter
extern global int shared
global int ready =
  +
    int 1
    int 2
proto int add
  param int a
  param int b
proto int sum
  param int
  param int
func int add
  param int a
  param int b
  block
    return
      +
        var a
        var b
"
        );
    }

    #[test]
    fn integer_types_and_casts() {
        assert_eq!(
            parsed(
                "\
unsigned long long int u;
signed char sc;
short int s;
int f(char, unsigned);
char g(long int x) { return (char)x; }
"
            ),
            "\
global unsigned long long u
global signed char sc
global short s
proto int f
  param char
  param unsigned int
func char g
  param long x
  block
    return
      cast char
        var x
"
        );
        assert_eq!(
            parsed("int main(void) { return (int)1 + 2; }\n"),
            "\
func int main
  block
    return
      +
        cast int
          int 1
        int 2
"
        );
        assert_eq!(
            parsed("int main(void) { return sizeof(unsigned long); }\n"),
            "\
func int main
  block
    return
      sizeof unsigned long
"
        );
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
