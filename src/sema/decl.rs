//! Declarations: file-scope and block-scope objects and functions, linkage,
//! redeclaration and composite types, and numbering of automatic objects.

use super::*;

impl Sema<'_> {
    pub(super) fn declare_global(&mut self, decl: &mut GlobalDecl) -> Result<(), Diagnostic> {
        let name = decl.name.clone();
        let mut ty = decl.ty.clone();
        let prior = self.file_get(&name);
        let prior_linkage = match &prior {
            None => None,
            Some(Symbol::Func(_)) => {
                return self.err(
                    decl.line,
                    decl.column,
                    format!("redefinition of '{name}' as different kind of symbol"),
                );
            }
            Some(Symbol::Var(var)) => {
                Some(var.global.as_ref().expect("file-scope variable").linkage)
            }
        };
        let linkage =
            self.object_linkage(decl.storage, prior_linkage, decl.line, decl.column, &name)?;
        let prebound = prior.is_none();
        let hidden_ty = self.bind_global_declarator(decl, &name, &ty, &prior, linkage)?;
        let shaped = if let Some(init) = &decl.init {
            Some(self.shape_initializer(&ty, init, decl.line, decl.column)?)
        } else {
            None
        };
        if let Some((shaped_ty, _, _)) = &shaped {
            ty = shaped_ty.clone();
        }
        let ty = self.compose_with_prior(decl, &name, ty, prebound, hidden_ty)?;
        decl.ty = ty.clone();
        // `extern struct S x;` may name an object completed in another
        // translation unit. A definition, including a tentative definition,
        // needs a complete type (C11 6.9.2).
        let extern_only = decl.init.is_none() && decl.storage == Storage::Extern;
        if !ty.is_array() && !ty.is_complete() && !extern_only {
            return self.err(
                decl.line,
                decl.column,
                format!("variable '{name}' has incomplete type '{}'", ty.display()),
            );
        }
        if prebound {
            self.set_global_type(&name, ty.clone());
        } else {
            self.reveal_file(&name);
            self.set_global_type(&name, ty.clone());
            // C11 6.9.2: a tentative declaration is not the object's
            // definition when an initialized definition is present. Only an
            // initialized definition is `is_def` for 6.7.5p7.
            let is_def = decl.init.is_some();
            self.check_and_merge_tls_alignas(
                &name,
                decl.thread_local,
                decl.alignas,
                is_def,
                decl.line,
                decl.column,
            )?;
        }
        if let Some((_, mut stores, mut overridden)) = shaped {
            if matches!(self.global_state(&name), Some(GlobalState::Defined(_))) {
                return self.err(decl.line, decl.column, format!("redefinition of '{name}'"));
            }
            self.check_stores(&mut stores)?;
            self.check_stores(&mut overridden)?;
            let value = self.fold_stores(&name, &ty, &stores)?;
            self.fold_overridden(&name, &overridden)?;
            self.set_global_state(&name, GlobalState::Defined(value));
        } else if decl.storage != Storage::Extern {
            self.upgrade_tentative(&name);
        }
        Ok(())
    }

    /// C11 6.7.1p3: `_Thread_local` shall appear on every declaration of the
    /// object. C11 6.7.5p7: a definition's `_Alignas`, if any, is the
    /// reference other declarations must match or omit, and a definition
    /// with no `_Alignas` forbids the specifier on every other declaration.
    fn check_and_merge_tls_alignas(
        &mut self,
        name: &str,
        decl_tls: bool,
        decl_alignas: Option<u32>,
        is_def: bool,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        let Some(Symbol::Var(var)) = self.file_get(name) else {
            return Ok(());
        };
        let Some(info) = var.global.clone() else {
            return Ok(());
        };
        if info.thread_local != decl_tls {
            return self.err(
                line,
                column,
                format!("'_Thread_local' must be present on every declaration of '{name}'"),
            );
        }
        if info.defined {
            if let Some(def_align) = info.def_alignas {
                if let Some(align) = decl_alignas {
                    if align != def_align {
                        return self.err(
                            line,
                            column,
                            format!("conflicting alignment for '{name}'"),
                        );
                    }
                }
            } else if decl_alignas.is_some() {
                return self.err(
                    line,
                    column,
                    format!(
                        "alignment specifier on a declaration of '{name}' whose definition has none"
                    ),
                );
            }
        } else if is_def {
            if decl_alignas.is_none() {
                if info.alignas.is_some() {
                    return self.err(
                        line,
                        column,
                        format!(
                            "definition of '{name}' has no alignment specifier, but a declaration does"
                        ),
                    );
                }
            } else if let Some(prev) = info.alignas {
                if Some(prev) != decl_alignas {
                    return self.err(line, column, format!("conflicting alignment for '{name}'"));
                }
            }
        } else if let (Some(prev), Some(align)) = (info.alignas, decl_alignas) {
            if prev != align {
                return self.err(line, column, format!("conflicting alignment for '{name}'"));
            }
        }
        if let Some(Symbol::Var(var)) = self.file_get_mut(name) {
            if let Some(info) = &mut var.global {
                if decl_alignas.is_some() {
                    info.alignas = decl_alignas;
                }
                if is_def {
                    info.defined = true;
                    info.def_alignas = decl_alignas;
                }
            }
        }
        Ok(())
    }

    /// `_Alignas` on an incomplete record is checked again when the type
    /// is complete (C11 6.7.5p4). An `extern` that stays incomplete is left
    /// to the other translation unit.
    fn deferred_alignas_error(&self) -> Option<(String, u32, u32, usize, usize)> {
        for name in &self.global_order {
            let Some(Symbol::Var(var)) = self.file_get(name) else {
                continue;
            };
            let Some(info) = var.global.as_ref() else {
                continue;
            };
            let Some(requested) = info.alignas else {
                continue;
            };
            let ready = var.ty.is_complete() || var.ty.is_array();
            if !ready {
                continue;
            }
            let natural = var.ty.member_align();
            if requested < natural {
                let (line, column) = self.object_decls.get(name).copied().unwrap_or((1, 1));
                return Some((var.ty.display(), requested, natural, line, column));
            }
        }
        None
    }

    /// Bind a file-scope object's name before its initializer is checked
    /// (C11 6.2.1p7), or reveal a name hidden by a block-scope `extern`.
    /// Returns the hidden declaration's type in the second case.
    fn bind_global_declarator(
        &mut self,
        decl: &GlobalDecl,
        name: &str,
        ty: &CType,
        prior: &Option<Symbol>,
        linkage: Linkage,
    ) -> Result<Option<CType>, Diagnostic> {
        // A hidden file entity from a block-scope extern is not a visible
        // prior declaration (C11 6.2.7p4), so it is not composed into this
        // declarator's type before the initializer is checked.
        let mut hidden_ty = None;
        if prior.is_none() {
            // C11 6.2.1p7: the identifier is in scope after its declarator,
            // including in the initializer.
            let state = if decl.init.is_none() && decl.storage == Storage::Extern {
                GlobalState::Declared
            } else if decl.init.is_none() {
                GlobalState::Tentative
            } else {
                GlobalState::Declared
            };
            self.global_order.push(name.to_string());
            self.object_decls
                .entry(name.to_string())
                .or_insert((decl.line, decl.column));
            let is_def = decl.init.is_some();
            self.insert_file(Symbol::Var(VarSym {
                name: name.to_string(),
                slot: format!("@{name}"),
                ty: ty.clone(),
                global: Some(GlobalInfo {
                    linkage,
                    state,
                    thread_local: decl.thread_local,
                    alignas: decl.alignas,
                    defined: is_def,
                    def_alignas: if is_def { decl.alignas } else { None },
                }),
                file_visible: true,
            }));
        } else if let Some(Symbol::Var(var)) = prior {
            if !var.file_visible {
                // Reveal the name with this declarator's type (C11 6.2.1p7)
                // so an initializer such as `{ [0 && sizeof a] = 7 }` on
                // `int a[]` sees an incomplete type (6.5.3.4p1).
                if !compatible(&var.ty, ty) {
                    return self.err(
                        decl.line,
                        decl.column,
                        format!("conflicting types for '{name}'"),
                    );
                }
                hidden_ty = Some(var.ty.clone());
                self.reveal_file(name);
                self.set_global_type(name, ty.clone());
            }
        }
        Ok(hidden_ty)
    }

    /// The type of a redeclared file-scope object: the composite with the
    /// visible prior declaration (C11 6.2.7p3), or with a hidden one when
    /// the two are compatible.
    fn compose_with_prior(
        &self,
        decl: &GlobalDecl,
        name: &str,
        mut ty: CType,
        prebound: bool,
        hidden_ty: Option<CType>,
    ) -> Result<CType, Diagnostic> {
        if let Some(Symbol::Var(var)) = self.file_get(name) {
            if !prebound {
                if let Some(hidden) = hidden_ty {
                    // Compatible hidden and later types still form a composite
                    // for the file entity. An initializer that completes an
                    // incomplete array to a different length is not a
                    // same-scope constraint (6.7p4); 6.2.7p2 would make a
                    // mismatch undefined, but the later type is kept.
                    if compatible(&hidden, &ty) {
                        ty = composite(&hidden, &ty);
                    }
                } else if !compatible(&var.ty, &ty) {
                    return self.err(
                        decl.line,
                        decl.column,
                        format!("conflicting types for '{name}'"),
                    );
                } else {
                    ty = composite(&var.ty, &ty);
                }
            }
        }
        Ok(ty)
    }

    pub(super) fn declare_func(&mut self, func: &mut FuncDef) -> Result<(), Diagnostic> {
        // A prototype may name an incomplete struct or union. A definition
        // needs a complete type. An x87 aggregate return uses `%st0`.
        if func.body.is_some() {
            self.require_passed_aggregate(&func.ret_type, func.line, func.column, true)?;
            for param in &func.params {
                self.require_passed_aggregate(&param.ty, param.line, param.column, false)?;
            }
        }
        let mut seen = HashSet::new();
        for param in &func.params {
            let Some(name) = &param.name else {
                continue;
            };
            if !seen.insert(name.clone()) {
                return self.err(
                    param.line,
                    param.column,
                    format!("redefinition of parameter '{name}'"),
                );
            }
        }
        // Top-level parameter qualifiers are not part of the function type
        // (C11 6.7.6.3p15). Pointee qualifiers stay, so `const char *` and
        // `char *` are different types.
        let params: Vec<CType> = func
            .params
            .iter()
            .map(|param| param.ty.unqualified())
            .collect();
        let name = func.name.clone();
        let prior = self.file_get(&name);
        let prior_linkage = match &prior {
            None => None,
            Some(Symbol::Var(_)) => {
                return self.err(
                    func.line,
                    func.column,
                    format!("redefinition of '{name}' as different kind of symbol"),
                );
            }
            Some(Symbol::Func(existing)) => Some(existing.linkage),
        };
        let linkage =
            self.func_linkage(func.storage, prior_linkage, func.line, func.column, &name)?;
        func.linkage = linkage;
        let is_def = func.body.is_some();
        match prior {
            None => {
                self.func_order.push(name.clone());
                self.insert_file(Symbol::Func(FuncSym {
                    name,
                    symbol: func.symbol.clone(),
                    ret: func.ret_type.clone(),
                    params: params.clone(),
                    variadic: func.variadic,
                    prototype: func.prototype,
                    linkage,
                    defined: is_def,
                    used: false,
                    use_line: 0,
                    use_column: 0,
                    file_visible: true,
                }));
            }
            Some(Symbol::Func(existing)) => {
                self.reveal_file(&name);
                let new_ty = def_func_ty(func, &params);
                if !function_types_match(&existing, &new_ty, is_def) {
                    return self.err(
                        func.line,
                        func.column,
                        format!("conflicting types for '{name}'"),
                    );
                }
                if is_def && existing.defined {
                    return self.err(func.line, func.column, format!("redefinition of '{name}'"));
                }
                if is_def {
                    self.mark_defined(&name);
                }
                if let Some(Symbol::Func(entry)) = self.file_get_mut(&name) {
                    entry.absorb(&new_ty);
                }
                if func.symbol != func.name {
                    if let Some(Symbol::Func(existing)) = self.file_get_mut(&name) {
                        existing.symbol = func.symbol.clone();
                    }
                }
            }
            Some(Symbol::Var(_)) => unreachable!("variable rejected above"),
        }
        Ok(())
    }

    pub(super) fn check_function(&mut self, func: &mut FuncDef) -> Result<(), Diagnostic> {
        self.current_ret = func.ret_type.clone();
        self.fn_name = func.name.clone();
        self.current_func = func.name.clone();
        self.push();
        for param in &func.params {
            let Some(name) = param.name.clone() else {
                continue;
            };
            // The parameter is not in scope in its own declarator (C11 6.2.1p7),
            // so the bound is checked before the current name is inserted.
            self.check_vla_sizes(&param.unadjusted, param.line, param.column)?;
            self.insert(Symbol::Var(VarSym {
                name,
                slot: param.slot.clone(),
                ty: param.ty.clone(),
                global: None,
                file_visible: true,
            }));
        }
        let body = func.body.as_mut().expect("defined function");
        self.labels.clear();
        self.dropped_labels.clear();
        self.addr_labels.clear();
        self.current_symbol = func.symbol.clone();
        self.collect_labels(body)?;
        self.variadic_fn = func.variadic;
        self.last_param = func.params.last().and_then(|param| param.name.clone());
        self.check_compound(body, false)?;
        self.check_vm_jumps(body)?;
        self.pop();
        self.fn_name.clear();
        self.current_func.clear();
        self.variadic_fn = false;
        self.last_param = None;
        self.labels.clear();
        self.dropped_labels.clear();
        func.addr_labels = std::mem::take(&mut self.addr_labels);
        self.current_symbol.clear();
        Ok(())
    }

    pub(super) fn finish(&mut self, unit: &mut TranslationUnit) -> Result<(), Diagnostic> {
        self.check_library_nan_clash()?;
        if let Some((name, requested, natural, line, column)) = self.deferred_alignas_error() {
            return self.err(
                line,
                column,
                format!(
                    "requested alignment {requested} is less than the alignment of '{name}' ({natural})"
                ),
            );
        }
        let mut missing_static = None;
        let mut incomplete_array = None;
        let mut func_decls = Vec::new();
        let mut globals = Vec::new();
        {
            let file = self.scopes.first().expect("file scope");
            // Two C names may share one linker name through `__asm__`
            // labels: glibc declares `signal` with the label `__sysv_signal`
            // and also declares `__sysv_signal` itself. The module gets one
            // `declare` per symbol, and none for a symbol defined here.
            let mut declared: HashSet<String> = self
                .func_order
                .iter()
                .filter_map(|name| match file.get(name) {
                    Some(Symbol::Func(func)) if func.defined => Some(func.symbol.clone()),
                    _ => None,
                })
                .collect();
            for name in &self.func_order {
                let Some(Symbol::Func(func)) = file.get(name) else {
                    continue;
                };
                if func.defined {
                    continue;
                }
                if func.linkage == Linkage::Internal {
                    if func.used {
                        missing_static = Some((func.use_line, func.use_column, name.clone()));
                        break;
                    }
                } else if declared.insert(func.symbol.clone()) {
                    func_decls.push(IrFunc {
                        name: func.symbol.clone(),
                        ret: func.ret.clone(),
                        params: func.params.clone(),
                        variadic: func.variadic,
                        linkage: func.linkage,
                    });
                }
            }
            for (symbol, sig) in &self.library_calls {
                if matches!(file.get(symbol), Some(Symbol::Var(_))) {
                    continue;
                }
                if declared.insert(symbol.clone()) {
                    func_decls.push(IrFunc {
                        name: symbol.clone(),
                        ret: sig.ret.clone(),
                        params: sig.params.clone(),
                        variadic: sig.variadic,
                        linkage: Linkage::External,
                    });
                }
            }
            if missing_static.is_none() {
                for name in &self.global_order {
                    let Some(Symbol::Var(var)) = file.get(name) else {
                        continue;
                    };
                    let info = var.global.as_ref().expect("global");
                    if incomplete_array.is_none()
                        && var.ty.is_array()
                        && var.ty.array_len().is_none()
                        && !matches!(info.state, GlobalState::Declared)
                    {
                        incomplete_array = Some(var.name.clone());
                    }
                    let init = match &info.state {
                        GlobalState::Declared => None,
                        GlobalState::Tentative => Some(zero_const(&var.ty)),
                        GlobalState::Defined(value) => Some(value.clone()),
                    };
                    let mut attrs = unit.var_attrs.get(name).cloned().unwrap_or_default();
                    // A block-scope `extern _Thread_local` records TLS on
                    // the file symbol, but the parser only fills var_attrs
                    // for file-scope declarations.
                    attrs.thread_local = attrs.thread_local || info.thread_local;
                    attrs.align = attrs.align.max(info.alignas);
                    globals.push(IrGlobal {
                        name: var.name.clone(),
                        ty: var.ty.clone(),
                        linkage: info.linkage,
                        init,
                        attrs,
                    });
                }
            }
        }
        if let Some((line, column, name)) = missing_static {
            return self.err(
                line,
                column,
                format!("static function '{name}' is used but not defined"),
            );
        }
        if let Some(name) = incomplete_array {
            let (line, column) = unit
                .decls
                .iter()
                .find_map(|decl| match decl {
                    TopDecl::Global(global) if global.name == name => {
                        Some((global.line, global.column))
                    }
                    _ => None,
                })
                .unwrap_or((1, 1));
            return self.err(line, column, format!("array size missing in '{name}'"));
        }
        unit.func_decls = func_decls;
        globals.append(&mut self.static_locals);
        unit.globals = globals;
        unit.strings = self
            .strings
            .iter()
            .map(|(bytes, name, enc)| StrConst {
                name: name.clone(),
                bytes: bytes.clone(),
                enc: *enc,
            })
            .collect();
        Ok(())
    }

    /// A runtime call of `nan`, `nanf`, or `nanl` from a malformed
    /// `__builtin_nan*` tag must reach the C library. An object with
    /// external linkage and the same name, declared anywhere in the
    /// translation unit, is undefined behaviour (C11 7.1.3p1-p2) and would
    /// take the call, so it is reported at the builtin call. An object with
    /// internal linkage gets a private symbol in IR lowering instead.
    pub(super) fn check_library_nan_clash(&self) -> Result<(), Diagnostic> {
        let file = self.scopes.first().expect("file scope");
        for (lib, builtin, line, column) in &self.library_nan_calls {
            let Some(Symbol::Var(var)) = file.get(lib) else {
                continue;
            };
            let external = var
                .global
                .as_ref()
                .is_some_and(|info| info.linkage == Linkage::External);
            if !external {
                continue;
            }
            let (decl_line, decl_column) = self
                .object_decls
                .get(lib)
                .copied()
                .unwrap_or((*line, *column));
            return self.err(
                *line,
                *column,
                format!(
                    "{builtin} with a tag that is not a constant calls the C library function \
                     '{lib}', which conflicts with the object '{lib}' of type '{}' declared with \
                     external linkage at {decl_line}:{decl_column} (C11 7.1.3)",
                    var.ty.display()
                ),
            );
        }
        Ok(())
    }

    pub(super) fn object_linkage(
        &self,
        storage: Storage,
        prior: Option<Linkage>,
        line: usize,
        column: usize,
        name: &str,
    ) -> Result<Linkage, Diagnostic> {
        // A file-scope object with no storage class has external linkage. It
        // does not inherit a prior `static` the way `extern` does (C11 6.2.2p5).
        let linkage = match storage {
            Storage::Static => Linkage::Internal,
            Storage::Extern => prior.unwrap_or(Linkage::External),
            Storage::None | Storage::Typedef => Linkage::External,
        };
        self.same_linkage(linkage, prior, storage, line, column, name)
    }

    pub(super) fn func_linkage(
        &self,
        storage: Storage,
        prior: Option<Linkage>,
        line: usize,
        column: usize,
        name: &str,
    ) -> Result<Linkage, Diagnostic> {
        // A function with no storage class is treated as `extern` (C11 6.2.2p5),
        // so `static int f(void); int f(void) { ... }` keeps internal linkage.
        let linkage = match storage {
            Storage::Static => Linkage::Internal,
            Storage::Extern | Storage::None | Storage::Typedef => {
                prior.unwrap_or(Linkage::External)
            }
        };
        self.same_linkage(linkage, prior, storage, line, column, name)
    }

    pub(super) fn same_linkage(
        &self,
        linkage: Linkage,
        prior: Option<Linkage>,
        storage: Storage,
        line: usize,
        column: usize,
        name: &str,
    ) -> Result<Linkage, Diagnostic> {
        if let Some(prev) = prior {
            if prev != linkage {
                let message = if storage == Storage::Static {
                    format!("static declaration of '{name}' follows a non-static declaration")
                } else {
                    format!("non-static declaration of '{name}' follows a static declaration")
                };
                return self.err(line, column, message);
            }
        }
        Ok(linkage)
    }

    pub(super) fn check_decl(&mut self, decl: &mut VarDecl) -> Result<(), Diagnostic> {
        match decl.storage {
            Storage::Extern => self.declare_block_extern(decl),
            Storage::Static => self.declare_static_local(decl),
            Storage::None => self.declare_auto(decl),
            Storage::Typedef => self.check_typedef(decl),
        }
    }

    /// An automatic local. A second declaration in the same scope is an error
    /// whether or not the earlier one has linkage (C11 6.7p3).
    pub(super) fn declare_auto(&mut self, decl: &mut VarDecl) -> Result<(), Diagnostic> {
        self.check_vla_sizes(&decl.ty, decl.line, decl.column)?;
        if let Some(existing) = self.lookup_local(&decl.name).cloned() {
            let message = match existing {
                Symbol::Func(_) => {
                    format!(
                        "redefinition of '{}' as different kind of symbol",
                        decl.name
                    )
                }
                Symbol::Var(_) => format!("redefinition of '{}'", decl.name),
            };
            return self.err(decl.line, decl.column, message);
        }
        // C11 6.2.1p7: the identifier is in scope after its declarator,
        // including in the initializer.
        self.insert(Symbol::Var(VarSym {
            name: decl.name.clone(),
            slot: decl.slot.clone(),
            ty: decl.ty.clone(),
            global: None,
            file_visible: true,
        }));
        let init = decl.init.clone();
        let (ty, stores, mut overridden) = if let Some(init) = &init {
            self.shape_initializer(&decl.ty, init, decl.line, decl.column)?
        } else {
            (decl.ty.clone(), Vec::new(), Vec::new())
        };
        if !ty.is_complete() {
            return self.err(decl.line, decl.column, incomplete_local(&decl.name, &ty));
        }
        decl.ty = ty.clone();
        self.set_local_var_type(&decl.name, ty.clone());
        if decl.init.is_some() {
            let mut stores = stores;
            self.check_stores(&mut stores)?;
            self.check_stores(&mut overridden)?;
            decl.checked = Some(classify_init(&ty, stores));
        }
        Ok(())
    }

    /// Block-scope `static`. No linkage, so each declaration is a distinct
    /// object, and the initializer is required to be constant (C11 6.7.9).
    /// The object is initialized once, before startup, not on each entry.
    pub(super) fn declare_static_local(&mut self, decl: &mut VarDecl) -> Result<(), Diagnostic> {
        if self.lookup_local(&decl.name).is_some() {
            return self.err(
                decl.line,
                decl.column,
                format!("redefinition of '{}'", decl.name),
            );
        }
        self.check_vla_sizes(&decl.ty, decl.line, decl.column)?;
        let symbol = self.fresh_static_name(&decl.name);
        let slot = format!("@{symbol}");
        decl.slot = slot.clone();
        decl.checked = None;
        // C11 6.2.1p7: the identifier is in scope after its declarator,
        // including in the initializer.
        self.insert(Symbol::Var(VarSym {
            name: decl.name.clone(),
            slot,
            ty: decl.ty.clone(),
            global: None,
            file_visible: true,
        }));
        let init = decl.init.clone();
        let (ty, stores, mut overridden) = if let Some(init) = &init {
            self.shape_initializer(&decl.ty, init, decl.line, decl.column)?
        } else {
            (decl.ty.clone(), Vec::new(), Vec::new())
        };
        if !ty.is_complete() {
            return self.err(decl.line, decl.column, incomplete_local(&decl.name, &ty));
        }
        decl.ty = ty.clone();
        self.set_local_var_type(&decl.name, ty.clone());
        if decl.thread_local {
            self.folding_tls = Some(symbol.clone());
        }
        let folded = if decl.init.is_some() {
            let mut stores = stores;
            self.check_stores(&mut stores)
                .and_then(|()| self.check_stores(&mut overridden))
                .and_then(|()| self.fold_stores(&decl.name, &ty, &stores))
                .and_then(|value| {
                    self.fold_overridden(&decl.name, &overridden)?;
                    Ok(value)
                })
        } else {
            Ok(zero_const(&ty))
        };
        self.folding_tls = None;
        let value = folded?;
        self.static_locals.push(IrGlobal {
            name: symbol,
            ty,
            linkage: Linkage::Internal,
            init: Some(value),
            attrs: VarAttrs {
                align: decl.align,
                thread_local: decl.thread_local,
                ..VarAttrs::default()
            },
        });
        Ok(())
    }

    /// Block-scope `extern`. Linkage comes from the visible prior declaration
    /// (C11 6.2.2p4). A prior with no linkage yields external linkage, and
    /// that is a different object from the no-linkage one. The same scope
    /// cannot mix linkage and no linkage (C11 6.7p3). An initializer is
    /// forbidden (C11 6.7.9p5).
    pub(super) fn declare_block_extern(&mut self, decl: &mut VarDecl) -> Result<(), Diagnostic> {
        if decl.init.is_some() {
            return self.err(
                decl.line,
                decl.column,
                "a block-scope declaration with linkage cannot have an initializer",
            );
        }
        let name = decl.name.clone();
        if self.lookup_local(&name).is_some() {
            return self.redeclare_block_extern(decl);
        }
        let linkage = self.inherited_object_linkage(&name, decl.line, decl.column)?;
        let mut local_ty = decl.ty.clone();
        if let Some(prior) = self.visible_linked_var(&name) {
            if !compatible(&prior.ty, &local_ty) {
                return self.err(
                    decl.line,
                    decl.column,
                    format!("conflicting types for '{name}'"),
                );
            }
            local_ty = composite(&prior.ty, &local_ty);
        }
        // `extern struct S x;` in a block may name an incomplete type. The
        // object is defined in another translation unit.
        let slot = self.bind_file_object(decl, &local_ty, linkage)?;
        decl.ty = local_ty.clone();
        decl.slot = slot.clone();
        decl.checked = None;
        self.insert(Symbol::Var(VarSym {
            name,
            slot,
            ty: local_ty,
            global: Some(GlobalInfo {
                linkage,
                state: GlobalState::Declared,
                thread_local: decl.thread_local,
                alignas: decl.alignas,
                defined: false,
                def_alignas: None,
            }),
            file_visible: true,
        }));
        Ok(())
    }

    pub(super) fn redeclare_block_extern(&mut self, decl: &mut VarDecl) -> Result<(), Diagnostic> {
        let name = decl.name.clone();
        let existing = self.lookup_local(&name).expect("same-scope name").clone();
        let Symbol::Var(var) = existing else {
            return self.err(
                decl.line,
                decl.column,
                format!("redefinition of '{name}' as different kind of symbol"),
            );
        };
        if var.global.is_none() {
            return self.err(decl.line, decl.column, format!("redefinition of '{name}'"));
        }
        if !compatible(&var.ty, &decl.ty) {
            return self.err(
                decl.line,
                decl.column,
                format!("conflicting types for '{name}'"),
            );
        }
        let local_ty = composite(&var.ty, &decl.ty);
        if let Some(Symbol::Var(file_var)) = self.file_get(&name) {
            if !compatible(&file_var.ty, &local_ty) {
                return self.err(
                    decl.line,
                    decl.column,
                    format!("conflicting types for '{name}'"),
                );
            }
            self.set_global_type(&name, composite(&file_var.ty, &local_ty));
            self.check_and_merge_tls_alignas(
                &name,
                decl.thread_local,
                decl.alignas,
                false,
                decl.line,
                decl.column,
            )?;
        }
        self.set_local_var_type(&name, local_ty.clone());
        decl.ty = local_ty;
        decl.slot = var.slot;
        decl.checked = None;
        Ok(())
    }

    /// A function declaration in a block. No storage class means `extern`
    /// (C11 6.2.2p5). `static` is not allowed (C11 6.7.1p7). A nested body is
    /// rejected by the parser, so this is never a definition.
    pub(super) fn declare_block_func(&mut self, func: &mut FuncDef) -> Result<(), Diagnostic> {
        // A block-scope function declaration is a prototype. Returning a
        // struct or union from a prototype is allowed; a call that uses the
        // value is still rejected.
        if func.storage == Storage::Static {
            return self.err(
                func.line,
                func.column,
                "a function declared in block scope cannot have 'static' storage class",
            );
        }
        if let Some(param) = duplicate_param(func) {
            let name = param.name.clone().unwrap_or_default();
            return self.err(
                param.line,
                param.column,
                format!("redefinition of parameter '{name}'"),
            );
        }
        let params: Vec<CType> = func
            .params
            .iter()
            .map(|param| param.ty.unqualified())
            .collect();
        let name = func.name.clone();
        if self.lookup_local(&name).is_some() {
            return self.redeclare_block_func(func, &params);
        }
        let linkage = self.inherited_func_linkage(&name, func.line, func.column)?;
        func.linkage = linkage;
        self.bind_file_func(func, &params, linkage)?;
        let defined = self
            .file_get(&name)
            .map(|sym| matches!(sym, Symbol::Func(func) if func.defined))
            .unwrap_or(false);
        self.insert(Symbol::Func(FuncSym {
            name,
            symbol: func.symbol.clone(),
            ret: func.ret_type.clone(),
            params,
            variadic: func.variadic,
            prototype: func.prototype,
            linkage,
            defined,
            used: false,
            use_line: 0,
            use_column: 0,
            file_visible: true,
        }));
        Ok(())
    }

    pub(super) fn redeclare_block_func(
        &mut self,
        func: &mut FuncDef,
        params: &[CType],
    ) -> Result<(), Diagnostic> {
        let name = func.name.clone();
        let existing = self.lookup_local(&name).expect("same-scope name").clone();
        let Symbol::Func(prior) = existing else {
            return self.err(
                func.line,
                func.column,
                format!("redefinition of '{name}' as different kind of symbol"),
            );
        };
        let new_ty = def_func_ty(func, params);
        if !function_types_match(&prior, &new_ty, false) {
            return self.err(
                func.line,
                func.column,
                format!("conflicting types for '{name}'"),
            );
        }
        func.linkage = prior.linkage;
        // C11 6.2.7p4: the identifier now has the composite type, so
        // `int f(); int f(int); int f(double);` in one block compares the
        // third declaration with `int f(int)`. The file-scope entry for the
        // same function takes the composite type too.
        if let Some(Symbol::Func(entry)) = self.scopes.last_mut().and_then(|s| s.get_mut(&name)) {
            entry.absorb(&new_ty);
        }
        self.bind_file_func(func, params, prior.linkage)
    }

    pub(super) fn inherited_object_linkage(
        &self,
        name: &str,
        line: usize,
        column: usize,
    ) -> Result<Linkage, Diagnostic> {
        match self.lookup(name) {
            None => Ok(Linkage::External),
            Some(Symbol::Var(var)) => Ok(var
                .global
                .map(|info| info.linkage)
                .unwrap_or(Linkage::External)),
            Some(Symbol::Func(_)) => self.err(
                line,
                column,
                format!("redefinition of '{name}' as different kind of symbol"),
            ),
        }
    }

    pub(super) fn inherited_func_linkage(
        &self,
        name: &str,
        line: usize,
        column: usize,
    ) -> Result<Linkage, Diagnostic> {
        match self.lookup(name) {
            None => Ok(Linkage::External),
            Some(Symbol::Func(func)) => Ok(func.linkage),
            Some(Symbol::Var(var)) if var.global.is_none() => Ok(Linkage::External),
            Some(Symbol::Var(_)) => self.err(
                line,
                column,
                format!("redefinition of '{name}' as different kind of symbol"),
            ),
        }
    }

    pub(super) fn visible_linked_var(&self, name: &str) -> Option<VarSym> {
        match self.lookup(name) {
            Some(Symbol::Var(var)) if var.global.is_some() => Some(var),
            _ => None,
        }
    }

    /// Record the file-scope object a block-scope `extern` refers to.
    ///
    /// The returned slot is the LLVM global. The caller's local type is not
    /// replaced by a hidden file-scope type: `extern int a[]` stays incomplete
    /// when the complete `int a[3]` is not visible. The file entity still
    /// keeps the composite type, which is what the global is emitted as.
    pub(super) fn bind_file_object(
        &mut self,
        decl: &VarDecl,
        local_ty: &CType,
        linkage: Linkage,
    ) -> Result<String, Diagnostic> {
        let name = decl.name.as_str();
        let line = decl.line;
        let column = decl.column;
        match self.file_get(name) {
            Some(Symbol::Func(_)) => self.err(
                line,
                column,
                format!("redefinition of '{name}' as different kind of symbol"),
            ),
            Some(Symbol::Var(var)) => {
                let file_linkage = var.global.as_ref().expect("file-scope object").linkage;
                self.same_linkage(
                    linkage,
                    Some(file_linkage),
                    Storage::Extern,
                    line,
                    column,
                    name,
                )?;
                if !compatible(&var.ty, local_ty) {
                    return self.err(line, column, format!("conflicting types for '{name}'"));
                }
                self.set_global_type(name, composite(&var.ty, local_ty));
                self.check_and_merge_tls_alignas(
                    name,
                    decl.thread_local,
                    decl.alignas,
                    false,
                    line,
                    column,
                )?;
                Ok(var.slot)
            }
            None => {
                let slot = format!("@{name}");
                self.global_order.push(name.to_string());
                self.object_decls
                    .entry(name.to_string())
                    .or_insert((line, column));
                self.insert_file(Symbol::Var(VarSym {
                    name: name.to_string(),
                    slot: slot.clone(),
                    ty: local_ty.clone(),
                    global: Some(GlobalInfo {
                        linkage,
                        state: GlobalState::Declared,
                        thread_local: decl.thread_local,
                        alignas: decl.alignas,
                        defined: false,
                        def_alignas: None,
                    }),
                    file_visible: false,
                }));
                Ok(slot)
            }
        }
    }

    pub(super) fn bind_file_func(
        &mut self,
        func: &FuncDef,
        params: &[CType],
        linkage: Linkage,
    ) -> Result<(), Diagnostic> {
        let name = &func.name;
        match self.file_get(name) {
            Some(Symbol::Var(_)) => self.err(
                func.line,
                func.column,
                format!("redefinition of '{name}' as different kind of symbol"),
            ),
            Some(Symbol::Func(existing)) => {
                let file_linkage = existing.linkage;
                self.same_linkage(
                    linkage,
                    Some(file_linkage),
                    Storage::Extern,
                    func.line,
                    func.column,
                    name,
                )?;
                let new_ty = def_func_ty(func, params);
                if !function_types_match(&existing, &new_ty, false) {
                    return self.err(
                        func.line,
                        func.column,
                        format!("conflicting types for '{name}'"),
                    );
                }
                if let Some(Symbol::Func(entry)) = self.file_get_mut(name) {
                    entry.absorb(&new_ty);
                }
                Ok(())
            }
            None => {
                self.func_order.push(name.clone());
                self.insert_file(Symbol::Func(FuncSym {
                    name: name.clone(),
                    symbol: func.symbol.clone(),
                    ret: func.ret_type.clone(),
                    params: params.to_vec(),
                    variadic: func.variadic,
                    prototype: func.prototype,
                    linkage,
                    defined: false,
                    used: false,
                    use_line: 0,
                    use_column: 0,
                    file_visible: false,
                }));
                Ok(())
            }
        }
    }

    pub(super) fn fresh_static_name(&mut self, var: &str) -> String {
        self.static_seq += 1;
        format!("{}.{var}.{}", self.current_func, self.static_seq)
    }

    pub(super) fn set_local_var_type(&mut self, name: &str, ty: CType) {
        if let Some(Symbol::Var(var)) = self.scopes.last_mut().expect("scope").get_mut(name) {
            var.ty = ty;
        }
    }

    pub(super) fn reveal_file(&mut self, name: &str) {
        match self.scopes.first_mut().expect("file scope").get_mut(name) {
            Some(Symbol::Var(var)) => var.file_visible = true,
            Some(Symbol::Func(func)) => func.file_visible = true,
            None => {}
        }
    }

    pub(super) fn check_typedef(&mut self, decl: &mut VarDecl) -> Result<(), Diagnostic> {
        self.check_vla_sizes(&decl.ty, decl.line, decl.column)
    }
}

pub(super) fn incomplete_local(name: &str, ty: &CType) -> String {
    if ty.is_array() {
        format!("array size missing in '{name}'")
    } else {
        format!("variable '{name}' has incomplete type '{}'", ty.display())
    }
}

pub(super) fn duplicate_param(func: &FuncDef) -> Option<&Param> {
    let mut seen = HashSet::new();
    for param in &func.params {
        let Some(name) = &param.name else {
            continue;
        };
        if !seen.insert(name.as_str()) {
            return Some(param);
        }
    }
    None
}

pub(super) fn def_func_ty(func: &FuncDef, params: &[CType]) -> FuncTy {
    FuncTy {
        ret: func.ret_type.clone(),
        params: params.to_vec(),
        variadic: func.variadic,
        prototype: func.prototype,
    }
}

/// A redeclaration agrees with what is already known (C11 6.7.6.3p15).
///
/// A definition written `f()` has no parameters, so it does not match a
/// prototype that has any, in either order. A declaration `f()` says nothing
/// about the parameters and matches any prototype whose parameters survive
/// the default argument promotions.
pub(super) fn function_types_match(existing: &FuncSym, new: &FuncTy, new_is_def: bool) -> bool {
    let old = existing.func_ty();
    if !compatible_funcs(&old, new) {
        return false;
    }
    if existing.defined
        && !old.prototype
        && old.params.is_empty()
        && new.prototype
        && !new.params.is_empty()
    {
        return false;
    }
    if new_is_def
        && !new.prototype
        && new.params.is_empty()
        && old.prototype
        && !old.params.is_empty()
    {
        return false;
    }
    true
}

pub(super) fn assign_slots(func: &mut FuncDef) {
    if func.body.is_none() {
        return;
    }
    let mut next = 0u32;
    for (index, param) in func.params.iter_mut().enumerate() {
        next += 1;
        param.slot = format!("%v{next}");
        param.arg_reg = format!("%p{index}");
    }
    let body = func.body.as_mut().expect("defined function");
    number_compound(body, &mut next);
}

pub(super) fn number_compound(compound: &mut Compound, next: &mut u32) {
    number_compound_in(compound, next, "%v");
}

/// Number the automatic objects in `compound` as `{prefix}{n}`.
pub(super) fn number_compound_in(compound: &mut Compound, next: &mut u32, prefix: &str) {
    for item in &mut compound.items {
        number_stmt(item, next, prefix);
    }
}

pub(super) fn number_stmt(stmt: &mut Stmt, next: &mut u32, prefix: &str) {
    match stmt {
        Stmt::Decl(decl) => {
            // `static` and `extern` are globals. Their slots are the LLVM
            // symbol, assigned while checking the declaration.
            if decl.storage == Storage::None {
                *next += 1;
                decl.slot = format!("{prefix}{next}");
            }
        }
        Stmt::Compound(compound) => number_compound_in(compound, next, prefix),
        Stmt::If {
            then_stmt,
            else_stmt,
            ..
        } => {
            number_stmt(then_stmt, next, prefix);
            if let Some(else_stmt) = else_stmt {
                number_stmt(else_stmt, next, prefix);
            }
        }
        Stmt::While { body, .. } | Stmt::Do { body, .. } => number_stmt(body, next, prefix),
        Stmt::Switch { body, .. }
        | Stmt::Case { stmt: body, .. }
        | Stmt::Default { stmt: body, .. } => {
            number_stmt(body, next, prefix);
        }
        Stmt::For { init, body, .. } => {
            if let ForInit::Decl(decls) = init {
                for decl in decls {
                    *next += 1;
                    decl.slot = format!("{prefix}{next}");
                }
            }
            number_stmt(body, next, prefix);
        }
        Stmt::Label { stmt, .. } => number_stmt(stmt, next, prefix),
        Stmt::FuncDecl(_)
        | Stmt::Return { .. }
        | Stmt::Expr { .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Goto { .. }
        | Stmt::GotoPtr { .. } => {}
    }
}
