//! External and block-scope declarations: function definitions, file-scope
//! declarations, typedefs, name binding, inline definitions, asm labels and
//! `_Static_assert`.

use super::*;

impl Parser<'_> {
    pub(super) fn finish_definition(
        &mut self,
        specs: Specs,
        declarator: NamedDecl,
    ) -> Result<TopDecl, Diagnostic> {
        let DeclKind::Function {
            ret,
            params,
            prototype,
            variadic,
        } = declarator.kind
        else {
            return Err(self.error_at(
                declarator.line,
                declarator.column,
                "expected a function definition",
            ));
        };
        self.reject_vm_function_ret(&ret, declarator.line, declarator.column)?;
        for param in &params {
            if param.name.is_none() {
                return Err(self.error_at(
                    param.line,
                    param.column,
                    "parameter name omitted in function definition",
                ));
            }
            if param.star_array || type_has_star_param(&param.ty) {
                return Err(self.error_at(
                    param.line,
                    param.column,
                    "star array notation is only allowed in a function prototype",
                ));
            }
        }
        let mut params = params;
        let kr_def = !prototype && !params.is_empty();
        if !kr_def && !prototype && self.starts_declaration() {
            return Err(self.error(
                "a declaration list is not allowed after a function declarator with no identifier list",
            ));
        }
        // C11 6.2.1p4: identifiers in the parameter declaration list have
        // the function's block scope. Open it before that list is parsed
        // so enumerators and earlier parameters are not bound at file scope.
        self.push_tags();
        for (name, entry) in declarator.param_tags {
            self.tags
                .last_mut()
                .expect("function scope")
                .insert(name, entry);
        }
        for (name, binding) in declarator.param_names {
            self.names
                .last_mut()
                .expect("function scope")
                .insert(name, binding);
        }
        if kr_def {
            self.parse_kr_declaration_list(&mut params)?;
        }
        for param in &mut params {
            star_out_nested_fn_vlas(&mut param.ty);
            star_out_nested_fn_vlas(&mut param.unadjusted);
        }
        // C11 6.4.2.2: static const char __func__[] = "function-name";
        {
            let scope = self.names.last_mut().expect("function scope");
            let func_ty = CType::array(
                CType::from_name(TypeName::Char),
                Some(declarator.name.len() as u64 + 1),
            )
            .with_quals(Quals {
                is_const: true,
                ..Quals::default()
            });
            scope.insert(
                "__func__".to_string(),
                NameBinding::Ordinary(Some(func_ty.clone())),
            );
            scope.insert(
                "__PRETTY_FUNCTION__".to_string(),
                NameBinding::Ordinary(Some(func_ty.clone())),
            );
            scope.insert(
                "__FUNCTION__".to_string(),
                NameBinding::Ordinary(Some(func_ty)),
            );
        }
        let symbol = self.file_fn_symbol(&declarator.name, declarator.symbol.as_deref());
        self.note_fn_attrs(&symbol, &specs.attrs, &declarator.attrs);
        self.note_inline(&declarator.name, &symbol, &specs, true);
        let body = self.parse_compound_scoped(false)?;
        self.pop_tags();

        self.reject_thread_local_on(&specs, "a function")?;
        if let Some((line, column)) = specs.alignas_at {
            return Err(self.error_at(
                line,
                column,
                "alignment specifiers are not allowed on a function",
            ));
        }
        Ok(TopDecl::Func(FuncDef {
            ret_type: ret,
            name: declarator.name,
            params,
            variadic,
            prototype,
            kr_def,
            body: Some(body),
            storage: specs.storage,
            linkage: Linkage::External,
            symbol,
            addr_labels: Vec::new(),
            line: declarator.line,
            column: declarator.column,
        }))
    }

    pub(super) fn finish_file_decl(
        &mut self,
        specs: &Specs,
        declarator: NamedDecl,
    ) -> Result<TopDecl, Diagnostic> {
        if declarator.is_function() {
            let DeclKind::Function {
                ret,
                mut params,
                prototype,
                variadic,
            } = declarator.kind
            else {
                unreachable!("function declarator");
            };
            self.reject_vm_function_ret(&ret, declarator.line, declarator.column)?;
            for param in &mut params {
                star_out_vla_types(&mut param.ty);
            }
            let mut ty = decl_kind_type(&DeclKind::Function {
                ret: ret.clone(),
                params: params.clone(),
                prototype,
                variadic,
            });
            star_out_vla_types(&mut ty);
            self.reject_identifier_list_decl(
                prototype,
                &params,
                declarator.line,
                declarator.column,
            )?;
            if self.current().kind == TokenKind::Assign {
                return Err(self.error("function declarations cannot have an initializer"));
            }
            let symbol = self.file_fn_symbol(&declarator.name, declarator.symbol.as_deref());
            self.note_fn_attrs(&symbol, &specs.attrs, &declarator.attrs);
            self.note_inline(&declarator.name, &symbol, specs, false);
            self.bind_ordinary(
                &declarator.name,
                Some(ty),
                declarator.line,
                declarator.column,
            )?;
            self.reject_thread_local_on(specs, "a function")?;
            if let Some((line, column)) = specs.alignas_at {
                return Err(self.error_at(
                    line,
                    column,
                    "alignment specifiers are not allowed on a function",
                ));
            }
            return Ok(TopDecl::Func(FuncDef {
                ret_type: ret,
                name: declarator.name,
                params,
                variadic,
                prototype,
                kr_def: false,
                body: None,
                storage: specs.storage,
                linkage: Linkage::External,
                symbol,
                addr_labels: Vec::new(),
                line: declarator.line,
                column: declarator.column,
            }));
        }
        self.reject_func_spec(specs)?;
        let DeclKind::Object(ty) = declarator.kind else {
            unreachable!("function handled above");
        };
        let attrs: Vec<Attr> = specs
            .attrs
            .iter()
            .chain(&declarator.attrs)
            .cloned()
            .collect();
        let ty = self.apply_mode(ty, &attrs)?;
        if ty.is_void() {
            return Err(self.error_at(
                declarator.line,
                declarator.column,
                "void variables are not supported",
            ));
        }
        self.reject_vm_decl(
            &ty,
            specs.storage,
            specs.thread_local,
            declarator.line,
            declarator.column,
        )?;
        // The identifier's scope starts after its declarator (C11 6.2.1p7),
        // so the initializer already sees it.
        self.bind_ordinary(
            &declarator.name,
            Some(ty.clone()),
            declarator.line,
            declarator.column,
        )?;
        let init = if self.match_kind(TokenKind::Assign) {
            Some(self.parse_initializer()?)
        } else {
            None
        };
        // Checked here, not in semantic analysis: completing this struct tag
        // later updates the shared type, so the later pass would see a
        // complete type. C11 6.7.9p3 requires a complete type at the initializer.
        self.reject_incomplete_initializer(
            &ty,
            init.is_some(),
            declarator.line,
            declarator.column,
        )?;
        self.bind_completed(&declarator.name, &ty, init.as_ref());
        self.reject_bad_restrict(&ty, declarator.line, declarator.column)?;
        let align = self.object_align(specs, &ty, &attrs, false, false, false)?;
        let mut var_attrs = self.var_attrs_of(&attrs)?;
        var_attrs.align = align;
        var_attrs.thread_local = specs.thread_local;
        self.var_attrs
            .entry(declarator.name.clone())
            .or_default()
            .merge(&var_attrs);
        Ok(TopDecl::Global(GlobalDecl {
            name: declarator.name,
            ty,
            storage: specs.storage,
            init,
            line: declarator.line,
            column: declarator.column,
            alignas: specs.alignas,
            thread_local: specs.thread_local,
        }))
    }

    pub(super) fn finish_typedefs(
        &mut self,
        specs: Specs,
        first: NamedDecl,
    ) -> Result<Vec<TopDecl>, Diagnostic> {
        self.reject_func_spec(&specs)?;
        self.reject_typedef_identifier_list(&first)?;
        self.bind_typedef_decl(&specs, &first, true)?;
        while self.match_kind(TokenKind::Comma) {
            let next = self.parse_named_decl(specs.ty.clone())?;
            if next.is_function() && self.current().kind == TokenKind::LBrace {
                return Err(self.error("a function definition cannot follow another declarator"));
            }
            self.reject_typedef_identifier_list(&next)?;
            self.bind_typedef_decl(&specs, &next, false)?;
        }
        self.expect_skip(TokenKind::Semi, "';'")?;
        Ok(Vec::new())
    }

    pub(super) fn parse_typedef_declarators(
        &mut self,
        specs: &Specs,
    ) -> Result<Vec<Stmt>, Diagnostic> {
        self.reject_func_spec(specs)?;
        let mut first = true;
        let mut items = Vec::new();
        loop {
            let declarator = self.parse_named_decl(specs.ty.clone())?;
            if declarator.is_function() && self.current().kind == TokenKind::LBrace {
                return Err(self.error("a typedef cannot have a function body"));
            }
            self.reject_typedef_identifier_list(&declarator)?;
            if let Some(decl) = self.bind_typedef_decl(specs, &declarator, first)? {
                items.push(Stmt::Decl(decl));
            }
            first = false;
            if self.match_kind(TokenKind::Comma) {
                continue;
            }
            break;
        }
        self.expect_skip(TokenKind::Semi, "';'")?;
        Ok(items)
    }

    pub(super) fn bind_typedef_decl(
        &mut self,
        specs: &Specs,
        declarator: &NamedDecl,
        first: bool,
    ) -> Result<Option<VarDecl>, Diagnostic> {
        if self.current().kind == TokenKind::Assign {
            return Err(self.error("a typedef cannot have an initializer"));
        }
        self.reject_thread_local_on(specs, "a typedef")?;
        if let Some((line, column)) = specs.alignas_at {
            return Err(self.error_at(
                line,
                column,
                "alignment specifiers are not allowed on a typedef",
            ));
        }
        let ty = match &declarator.kind {
            DeclKind::Object(ty) => ty.clone(),
            DeclKind::Function { .. } => decl_kind_type(&declarator.kind),
        };
        let attrs: Vec<Attr> = specs
            .attrs
            .iter()
            .chain(&declarator.attrs)
            .cloned()
            .collect();
        let ty = self.apply_mode(ty, &attrs)?;
        // A typedef with its own alignment is a distinct type in GCC. Only
        // the type's own alignment is supported here, plus one form that
        // MinGW-w64's <setjmp.h> uses: `typedef __attribute__((aligned(16)))
        // struct _S { ... } T;`, where the record is defined in this very
        // declaration and its size is already a multiple of the alignment.
        // The record itself takes the alignment. That is exact when the
        // record has no tag. With a tag, `_Alignof(struct _S)` would also be
        // raised where GCC and Clang keep the record's own alignment, so the
        // form is accepted only for a tag reserved to the implementation
        // (C11 7.1.3: `__x` or `_X`), which a program does not name; any
        // other tag is still diagnosed. The typedef must also be the only
        // declarator, so no other typedef name shares the raised record.
        if let Some(align) = self.attrs_align(&attrs)? {
            let only = first && self.current().kind == TokenKind::Semi;
            if let Some(handle) = raisable_record(specs, &ty, align).filter(|_| only) {
                handle.set_unadjusted_align(handle.unadjusted_align());
                handle.raise_align(align);
                handle.set_irregular();
            } else if ty.size().is_none() || ty.member_align() != align {
                return Err(self.error_at(
                    declarator.line,
                    declarator.column,
                    "an 'aligned' attribute on a typedef is only supported on a struct or union definition",
                ));
            }
        }
        self.reject_bad_restrict(&ty, declarator.line, declarator.column)?;
        self.reject_vm_decl(
            &ty,
            Storage::Typedef,
            false,
            declarator.line,
            declarator.column,
        )?;
        self.bind_typedef(
            &declarator.name,
            ty.clone(),
            declarator.line,
            declarator.column,
        )?;
        if self.names.len() <= 1 {
            return Ok(None);
        }
        Ok(Some(VarDecl {
            name: declarator.name.clone(),
            ty,
            storage: Storage::Typedef,
            init: None,
            checked: None,
            line: declarator.line,
            column: declarator.column,
            slot: String::new(),
            align: None,
            alignas: None,
            thread_local: false,
            vla_from: declarator.vla_from,
        }))
    }

    pub(super) fn bind_typedef(
        &mut self,
        name: &str,
        ty: CType,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        let scope = self.names.last_mut().expect("name scope");
        if let Some(existing) = scope.get(name) {
            match existing {
                NameBinding::Typedef(old) if compatible(old, &ty) => {
                    // C11 6.7p3: a typedef may be redefined to the same type
                    // only when that type is not variably modified.
                    if ty.is_variably_modified() || old.is_variably_modified() {
                        return Err(self.error_at(
                            line,
                            column,
                            "a typedef of a variably modified type cannot be redefined",
                        ));
                    }
                    return Ok(());
                }
                _ => {
                    return Err(self.error_at(line, column, format!("redefinition of '{name}'")));
                }
            }
        }
        scope.insert(name.to_string(), NameBinding::Typedef(ty));
        Ok(())
    }

    pub(super) fn bind_ordinary(
        &mut self,
        name: &str,
        ty: Option<CType>,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        let scope = self.names.last_mut().expect("name scope");
        match scope.get(name) {
            Some(NameBinding::Ordinary(old)) => {
                // A redeclaration in the same scope keeps a known array
                // length: `extern int a[]; int a[4];` and the reverse.
                let ty = match (old, ty) {
                    (Some(old), Some(new)) if compatible(old, &new) => Some(composite(&new, old)),
                    (_, new) => new,
                };
                scope.insert(name.to_string(), NameBinding::Ordinary(ty));
                Ok(())
            }
            None => {
                scope.insert(name.to_string(), NameBinding::Ordinary(ty));
                Ok(())
            }
            Some(NameBinding::Typedef(_) | NameBinding::EnumConst(..)) => {
                Err(self.error_at(line, column, format!("redefinition of '{name}'")))
            }
        }
    }

    /// After an initializer, record the length it gives an array of unknown
    /// size (`int a[] = {1, 2, 3}`), so a later `sizeof a` is a constant.
    /// Semantic analysis computes the authoritative length; this mirror only
    /// handles the shapes whose length does not depend on expression types,
    /// and otherwise leaves the array incomplete for `sizeof`.
    pub(super) fn bind_completed(&mut self, name: &str, ty: &CType, init: Option<&Init>) {
        let (Some(init), CType::Array(elem, ArrayLen::Incomplete)) = (init, ty) else {
            return;
        };
        let Some(len) = infer_init_len(elem, init, self.env()) else {
            return;
        };
        if let Some(scope) = self.names.last_mut() {
            if let Some(NameBinding::Ordinary(slot)) = scope.get_mut(name) {
                *slot = Some(CType::array((**elem).clone(), Some(len)));
            }
        }
    }

    pub(super) fn bind_enum(
        &mut self,
        name: &str,
        value: i128,
        ty: TypeName,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        let scope = self.names.last_mut().expect("name scope");
        if scope.contains_key(name) {
            return Err(self.error_at(line, column, format!("redefinition of '{name}'")));
        }
        scope.insert(name.to_string(), NameBinding::EnumConst(value, ty));
        Ok(())
    }

    /// `restrict` only qualifies a pointer whose referenced type is an
    /// object type (C11 6.7.3p2).
    pub(super) fn reject_bad_restrict(
        &self,
        ty: &CType,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        fn walk(ty: &CType) -> Option<&'static str> {
            if ty.quals().is_restrict {
                match ty.unqualified().pointee() {
                    None => return Some("restrict requires a pointer type"),
                    Some(pointee) if pointee.is_func() => {
                        return Some("restrict requires a pointer to an object type");
                    }
                    Some(_) => {}
                }
            }
            match ty.peel() {
                CType::Pointer(inner) | CType::Array(inner, _) => walk(inner),
                CType::Func(func) => walk(&func.ret).or_else(|| func.params.iter().find_map(walk)),
                _ => None,
            }
        }
        match walk(ty) {
            Some(message) => Err(self.error_at(line, column, message)),
            None => Ok(()),
        }
    }

    /// The linker name of a file-scope function declarator. A declarator
    /// without an `__asm__` label keeps the label of an earlier file-scope
    /// declaration of the same function: the GCC manual ("Asm Labels")
    /// renames a definition by giving the label on a declaration before it.
    pub(super) fn file_fn_symbol(&mut self, name: &str, label: Option<&str>) -> String {
        match label {
            Some(label) => {
                self.fn_labels.insert(name.to_string(), label.to_string());
                label.to_string()
            }
            None => self
                .fn_labels
                .get(name)
                .cloned()
                .unwrap_or_else(|| name.to_string()),
        }
    }

    /// Record how a file-scope declaration or definition of a function uses
    /// `inline` and `extern`, for [`Parser::mark_inline_definitions`].
    /// `name` is the C identifier, so every declaration of one function
    /// shares a record whatever its linker `symbol`.
    /// C11 6.7.4p2: a function specifier (`inline`, `_Noreturn`) may appear
    /// only in the declaration of a function, not of an object, a typedef
    /// name, or a tag alone.
    pub(super) fn reject_func_spec(&self, specs: &Specs) -> Result<(), Diagnostic> {
        match specs.func_spec {
            Some((name, line, column)) => Err(self.error_at(
                line,
                column,
                format!("'{name}' can only appear on functions"),
            )),
            None => Ok(()),
        }
    }

    pub(super) fn note_inline(
        &mut self,
        name: &str,
        symbol: &str,
        specs: &Specs,
        definition: bool,
    ) {
        let seen = self.inline_seen.entry(name.to_string()).or_default();
        if !seen.symbols.iter().any(|known| known == symbol) {
            seen.symbols.push(symbol.to_string());
        }
        let is_extern = specs.storage == Storage::Extern;
        if specs.inline_ {
            seen.inline_ = true;
        }
        if !specs.inline_ || is_extern {
            seen.external = true;
        }
        if definition {
            seen.extern_inline_def = specs.inline_ && is_extern;
            seen.def_inline = specs.inline_;
            seen.def_symbol = Some(symbol.to_string());
        }
    }

    /// Mark the functions whose definition here is not an external one.
    /// C11 6.7.4p7: when every file-scope declaration of a function has
    /// `inline` and none has `extern`, its definition is an inline
    /// definition. With GCC's `gnu_inline` attribute (GCC manual, "Common
    /// Function Attributes"), an `extern inline` definition is used only
    /// for inlining, and a plain `inline` one is an external definition.
    /// `-fgnu89-inline` gives every function the `gnu_inline` semantics.
    /// `static` functions have internal linkage and are not affected.
    ///
    /// MSVC instead puts every non-static definition written with `inline`
    /// in a COMDAT, whatever the other declarations say: `linkonce_odr`,
    /// or `weak_odr` for `extern inline` (observed with Clang for
    /// x86_64-pc-windows-msvc as a black box). The UCRT headers rely on it
    /// for `printf` and its relatives.
    pub(super) fn mark_inline_definitions(&mut self) {
        let msvc = crate::target::current().is_msvc();
        for seen in std::mem::take(&mut self.inline_seen).into_values() {
            if msvc {
                if let (true, Some(symbol)) = (seen.def_inline, seen.def_symbol) {
                    self.fn_attrs.entry(symbol).or_default().msvc_comdat =
                        Some(if seen.extern_inline_def {
                            "weak_odr"
                        } else {
                            "linkonce_odr"
                        });
                }
                continue;
            }
            let gnu = self.gnu89_inline
                || seen.symbols.iter().any(|symbol| {
                    self.fn_attrs
                        .get(symbol)
                        .is_some_and(|attrs| attrs.gnu_inline)
                });
            let inline_only = if gnu {
                seen.extern_inline_def
            } else {
                seen.inline_ && !seen.external
            };
            // Only the symbol this C name's own definition is emitted under:
            // another C name may share a linker name through an asm label.
            if let (true, Some(symbol)) = (inline_only, seen.def_symbol) {
                self.fn_attrs
                    .entry(symbol)
                    .or_default()
                    .available_externally = true;
            }
        }
    }

    /// Attributes and an optional `__asm__("name")` after a declarator.
    pub(super) fn parse_decl_suffix(&mut self) -> Result<Option<String>, Diagnostic> {
        let mut symbol = None;
        loop {
            self.skip_attributes()?;
            if self.spec_word(&["__asm__", "__asm", "asm"]) {
                let name = self.parse_asm_label()?;
                symbol = Some(name);
                continue;
            }
            break;
        }
        Ok(symbol)
    }

    pub(super) fn parse_asm_label(&mut self) -> Result<String, Diagnostic> {
        self.bump();
        while self.spec_word(&["__volatile__", "__volatile", "volatile", "goto"])
            || self.current().kind == TokenKind::KwVolatile
        {
            self.bump();
        }
        self.expect_skip(TokenKind::LParen, "'('")?;
        if self.current().kind != TokenKind::String {
            return Err(self.error("expected a string after __asm__"));
        }
        let mut bytes = Vec::new();
        while self.current().kind == TokenKind::String {
            let tok = self.advance();
            if !matches!(lit_prefix(&tok.spelling), LitPrefix::None | LitPrefix::U8) {
                return Err(self.error_at(
                    tok.line,
                    tok.column,
                    "an __asm__ label must use a byte string literal",
                ));
            }
            bytes.extend(tok.bytes.expect("string token"));
        }
        self.expect_skip(TokenKind::RParen, "')'")?;
        let label = String::from_utf8_lossy(&bytes).into_owned();
        // The label is the exact assembler name (GCC manual, "Controlling
        // Names Used in Assembler Code"). Mach-O adds a `_` to every IR
        // name; a leading \01 tells LLVM to use the name as written.
        if crate::target::current().is_apple() {
            return Ok(format!("\u{1}{label}"));
        }
        Ok(label)
    }

    /// `_Static_assert ( constant-expression , string-literal ) ;`
    /// (C11 6.7.10). It may appear at file scope, in a block, and in a
    /// member list. The expression is an integer constant expression; when
    /// it is 0 the message is reported.
    pub(super) fn parse_static_assert(&mut self) -> Result<(), Diagnostic> {
        self.bump();
        self.expect_skip(TokenKind::LParen, "'('")?;
        let expr = self.parse_conditional()?;
        check_uneval(&expr, self.env())
            .map_err(|err| self.error_at(expr.line(), expr.column(), ice_message(err)))?;
        let value = eval_ice(&expr, self.env())
            .map_err(|err| self.error_at(expr.line(), expr.column(), ice_message(err)))?;
        self.expect_skip(TokenKind::Comma, "','")?;
        if self.current().kind != TokenKind::String {
            return Err(self.error(format!(
                "expected a string literal, found {}",
                found(self.current())
            )));
        }
        let mut message = String::new();
        while self.current().kind == TokenKind::String {
            let tok = self.advance();
            match lit_prefix(&tok.spelling) {
                LitPrefix::None | LitPrefix::U8 => {
                    message.push_str(&String::from_utf8_lossy(
                        tok.bytes.as_deref().expect("string token"),
                    ));
                }
                _ => message.push_str(&tok.spelling),
            }
        }
        self.expect_skip(TokenKind::RParen, "')'")?;
        self.expect_skip(TokenKind::Semi, "';'")?;
        if value.bits & value.ty.mask() == 0 {
            return Err(self.error_at(
                expr.line(),
                expr.column(),
                format!("static assertion failed: {message}"),
            ));
        }
        Ok(())
    }

    /// An initializer needs a complete type, except an array whose length comes
    /// from the list (C11 6.7.9p3).
    pub(super) fn reject_incomplete_initializer(
        &self,
        ty: &CType,
        has_init: bool,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        if has_init && ty.is_vla() {
            return Err(self.error_at(
                line,
                column,
                "a variable length array cannot have an initializer",
            ));
        }
        if !has_init || ty.is_complete() || ty.is_incomplete_array() {
            return Ok(());
        }
        Err(self.error_at(
            line,
            column,
            format!("variable has incomplete type '{}'", ty.display()),
        ))
    }

    /// Declarations inside a block, or in the header of a `for` loop.
    ///
    /// A `for` header may not declare a function, and it may not use `static`
    /// or `extern` (the declaration would not have automatic storage). A
    /// function declarator followed by `{` is a nested definition, which this
    /// subset rejects. A function declarator followed by `;` or `,` is a
    /// block-scope declaration; C11 6.2.2 treats it as `extern`.
    pub(super) fn parse_block_declaration(
        &mut self,
        in_for: bool,
    ) -> Result<Vec<Stmt>, Diagnostic> {
        let specs = self.parse_specs(SpecPlace::Declaration)?;
        self.apply_shadow(&specs)?;
        if in_for {
            if let Some((line, column)) = specs.storage_at {
                return Err(self.error_at(
                    line,
                    column,
                    "a for-loop declaration cannot have a storage class",
                ));
            }
        }
        if self.current().kind == TokenKind::Semi {
            self.finish_tag_only(&specs)?;
            self.expect_skip(TokenKind::Semi, "';'")?;
            return Ok(Vec::new());
        }
        if specs.storage == Storage::Typedef {
            return self.parse_typedef_declarators(&specs);
        }
        let base = specs.ty.clone();
        let mut items = Vec::new();
        loop {
            let declarator = self.parse_named_decl(base.clone())?;
            if declarator.is_function() {
                if self.current().kind == TokenKind::LBrace {
                    return Err(self.error_at(
                        declarator.line,
                        declarator.column,
                        "nested functions are not supported",
                    ));
                }
                if in_for {
                    return Err(self.error_at(
                        declarator.line,
                        declarator.column,
                        "a function declaration is not allowed in a for loop",
                    ));
                }
                items.push(Stmt::FuncDecl(self.finish_block_func(&specs, declarator)?));
            } else {
                items.push(Stmt::Decl(self.finish_block_object(&specs, declarator)?));
            }
            if self.match_kind(TokenKind::Comma) {
                continue;
            }
            break;
        }
        self.expect_skip(TokenKind::Semi, "';'")?;
        Ok(items)
    }

    pub(super) fn finish_block_func(
        &mut self,
        specs: &Specs,
        declarator: NamedDecl,
    ) -> Result<FuncDef, Diagnostic> {
        let DeclKind::Function {
            ret,
            mut params,
            prototype,
            variadic,
        } = declarator.kind
        else {
            unreachable!("function declarator");
        };
        self.reject_vm_function_ret(&ret, declarator.line, declarator.column)?;
        for param in &mut params {
            star_out_vla_types(&mut param.ty);
        }
        let mut ty = decl_kind_type(&DeclKind::Function {
            ret: ret.clone(),
            params: params.clone(),
            prototype,
            variadic,
        });
        star_out_vla_types(&mut ty);
        self.reject_identifier_list_decl(prototype, &params, declarator.line, declarator.column)?;
        if self.current().kind == TokenKind::Assign {
            return Err(self.error("function declarations cannot have an initializer"));
        }
        let symbol = declarator
            .symbol
            .clone()
            .unwrap_or_else(|| declarator.name.clone());
        self.note_fn_attrs(&symbol, &specs.attrs, &declarator.attrs);
        self.bind_ordinary(
            &declarator.name,
            Some(ty),
            declarator.line,
            declarator.column,
        )?;
        self.reject_thread_local_on(specs, "a function")?;
        if let Some((line, column)) = specs.alignas_at {
            return Err(self.error_at(
                line,
                column,
                "alignment specifiers are not allowed on a function",
            ));
        }
        Ok(FuncDef {
            ret_type: ret,
            name: declarator.name,
            params,
            variadic,
            prototype,
            kr_def: false,
            body: None,
            storage: specs.storage,
            linkage: Linkage::External,
            symbol,
            addr_labels: Vec::new(),
            line: declarator.line,
            column: declarator.column,
        })
    }

    pub(super) fn finish_block_object(
        &mut self,
        specs: &Specs,
        declarator: NamedDecl,
    ) -> Result<VarDecl, Diagnostic> {
        self.reject_func_spec(specs)?;
        let DeclKind::Object(ty) = declarator.kind else {
            unreachable!("object declarator");
        };
        let attrs: Vec<Attr> = specs
            .attrs
            .iter()
            .chain(&declarator.attrs)
            .cloned()
            .collect();
        let ty = self.apply_mode(ty, &attrs)?;
        let align = self.object_align(specs, &ty, &attrs, false, false, false)?;
        if specs.thread_local && specs.storage == Storage::None {
            let (line, column) = specs
                .thread_local_at
                .unwrap_or((declarator.line, declarator.column));
            return Err(self.error_at(
                line,
                column,
                "block-scope '_Thread_local' requires 'static' or 'extern'",
            ));
        }
        if ty.is_void() {
            return Err(self.error_at(
                declarator.line,
                declarator.column,
                "void variables are not supported",
            ));
        }
        self.reject_vm_decl(
            &ty,
            specs.storage,
            specs.thread_local,
            declarator.line,
            declarator.column,
        )?;
        self.bind_ordinary(
            &declarator.name,
            Some(ty.clone()),
            declarator.line,
            declarator.column,
        )?;
        let init = if self.match_kind(TokenKind::Assign) {
            Some(self.parse_initializer()?)
        } else {
            None
        };
        self.reject_incomplete_initializer(
            &ty,
            init.is_some(),
            declarator.line,
            declarator.column,
        )?;
        self.reject_bad_restrict(&ty, declarator.line, declarator.column)?;
        self.bind_completed(&declarator.name, &ty, init.as_ref());
        Ok(VarDecl {
            name: declarator.name,
            ty,
            storage: specs.storage,
            init,
            checked: None,
            line: declarator.line,
            column: declarator.column,
            slot: String::new(),
            align,
            alignas: specs.alignas,
            thread_local: specs.thread_local,
            vla_from: declarator.vla_from,
        })
    }

    fn reject_typedef_identifier_list(&self, declarator: &NamedDecl) -> Result<(), Diagnostic> {
        if let DeclKind::Function {
            prototype, params, ..
        } = &declarator.kind
        {
            self.reject_identifier_list_decl(
                *prototype,
                params,
                declarator.line,
                declarator.column,
            )?;
        }
        Ok(())
    }

    fn reject_identifier_list_decl(
        &self,
        prototype: bool,
        params: &[Param],
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        if !prototype && !params.is_empty() {
            return Err(self.error_at(
                line,
                column,
                "a function declaration with an identifier list must be a definition",
            ));
        }
        Ok(())
    }

    /// Declaration list of a K&R definition (C11 6.9.1p6).
    fn parse_kr_declaration_list(&mut self, params: &mut [Param]) -> Result<(), Diagnostic> {
        let mut by_name = HashMap::new();
        for (index, param) in params.iter().enumerate() {
            let name = param.name.as_ref().expect("named K&R parameter").clone();
            by_name.insert(name, index);
        }
        let mut declared = HashSet::new();
        while !matches!(self.current().kind, TokenKind::LBrace | TokenKind::Eof) {
            if !self.starts_declaration() {
                return Err(self.error("expected a parameter declaration or '{'"));
            }
            let specs = self.parse_specs(SpecPlace::Declaration)?;
            self.reject_func_spec(&specs)?;
            self.reject_thread_local_on(&specs, "a parameter")?;
            if let Some((line, column)) = specs.alignas_at {
                return Err(self.error_at(
                    line,
                    column,
                    "alignment specifiers are not allowed on a parameter",
                ));
            }
            if specs.storage != Storage::None || (specs.plain_storage && !specs.register) {
                return Err(self.error(
                    "a parameter declaration may only use the 'register' storage-class specifier",
                ));
            }
            if self.current().kind == TokenKind::Semi {
                return Err(self.error(
                    "each declaration in a parameter declaration list shall have a declarator",
                ));
            }
            loop {
                let named = self.parse_named_decl(specs.ty.clone())?;
                if self.current().kind == TokenKind::Assign {
                    return Err(self.error("a parameter declaration cannot have an initializer"));
                }
                if let DeclKind::Function {
                    prototype, params, ..
                } = &named.kind
                {
                    self.reject_identifier_list_decl(*prototype, params, named.line, named.column)?;
                }
                let ty = match named.kind {
                    DeclKind::Object(ty) => ty,
                    DeclKind::Function { .. } => decl_kind_type(&named.kind),
                };
                let unadjusted = ty.clone();
                let ty = self.adjust_parameter(ty, named.line, named.column)?;
                let Some(&index) = by_name.get(&named.name) else {
                    return Err(self.error_at(
                        named.line,
                        named.column,
                        format!("'{}' is not a parameter", named.name),
                    ));
                };
                if !declared.insert(named.name.clone()) {
                    return Err(self.error_at(
                        named.line,
                        named.column,
                        format!("redefinition of parameter '{}'", named.name),
                    ));
                }
                params[index].ty = ty;
                params[index].unadjusted = unadjusted;
                params[index].line = named.line;
                params[index].column = named.column;
                self.bind_ordinary(
                    &named.name,
                    Some(params[index].ty.clone()),
                    named.line,
                    named.column,
                )?;
                if self.match_kind(TokenKind::Comma) {
                    continue;
                }
                break;
            }
            self.expect_skip(TokenKind::Semi, "';'")?;
        }
        if self.current().kind != TokenKind::LBrace {
            return Err(self.error("expected '{'"));
        }
        for param in params.iter() {
            let name = param.name.as_ref().expect("named K&R parameter");
            if !declared.contains(name) {
                return Err(self.error_at(
                    param.line,
                    param.column,
                    format!("parameter '{name}' was not declared"),
                ));
            }
        }
        Ok(())
    }
}

/// The record a typedef's `aligned(align)` can raise: one defined by this
/// declaration's own type specifier, named by the declarator without
/// derivation or qualifiers, whose size is a multiple of `align` and whose
/// alignment is lower.
fn raisable_record<'a>(specs: &'a Specs, ty: &CType, align: u32) -> Option<&'a StructRef> {
    let handle = specs.ty.struct_ref()?;
    let same = matches!(ty, CType::Struct(own) if own.id() == handle.id());
    let size = handle.size()?;
    let tag = handle.tag();
    let reserved = tag.is_empty()
        || tag.starts_with("__")
        || (tag.starts_with('_') && tag[1..].starts_with(|c: char| c.is_ascii_uppercase()));
    (specs.defined_record
        && same
        && reserved
        && handle.align() < align
        && size % u64::from(align) == 0)
        .then_some(handle)
}

impl Parser<'_> {
    /// C11 6.7.6.2p2: a variably modified identifier has no linkage and
    /// block or function prototype scope. A VLA object cannot have static
    /// or thread storage duration.
    fn reject_vm_decl(
        &self,
        ty: &CType,
        storage: Storage,
        thread_local: bool,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        if !ty.is_variably_modified() {
            return Ok(());
        }
        if self.names.len() <= 1 {
            return Err(self.error_at(
                line,
                column,
                "variably modified type is not allowed at file scope",
            ));
        }
        if storage == Storage::Extern {
            return Err(self.error_at(line, column, "variably modified type cannot have linkage"));
        }
        if ty.is_vla() && (storage == Storage::Static || thread_local) {
            return Err(self.error_at(
                line,
                column,
                "a variable length array cannot have static or thread storage duration",
            ));
        }
        Ok(())
    }

    /// C11 6.7.6.2p2: a function identifier cannot have a variably
    /// modified type, including one derived from a variably modified
    /// return type (`int (*f(void))[n]`).
    fn reject_vm_function_ret(
        &self,
        ret: &CType,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        if ret.is_variably_modified() {
            return Err(self.error_at(
                line,
                column,
                "a function cannot have a variably modified return type",
            ));
        }
        Ok(())
    }
}
