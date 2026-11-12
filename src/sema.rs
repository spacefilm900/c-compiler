//! Name resolution and type checks for the C subset.
//!
//! Scopes follow C block scope. The parameter list of a function definition
//! shares a scope with the function's outermost block, so a parameter cannot
//! be redeclared there. A name is visible in its own initializer.
//!
//! File-scope objects and functions follow C11 6.2.2 linkage and 6.9.2
//! tentative definitions. A prototype makes a function visible before its
//! body, which is what mutual recursion needs. A block-scope `extern`
//! declaration, and a block-scope function declaration, name that same
//! entity when the linkage agrees. The entity is not visible outside the
//! block until a file-scope declaration names it. A block-scope `static`
//! object has no linkage and static storage duration; its initializer is a
//! constant, the same rules as a file-scope initializer.
//!
//! A global or `static` initializer must be a constant (C11 6.6): an integer
//! constant, an address constant, or a brace list of those. Assignment,
//! `++`/`--`, calls, and the comma operator are rejected there, including
//! when they sit in an evaluated operand. An unevaluated operand (`sizeof`,
//! or the skipped arm of `&&`, `||`, or `?:`) may contain them.

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diag::Diagnostic;

pub fn analyze(unit: &mut TranslationUnit, filename: &str) -> Result<(), Diagnostic> {
    analyze_tokens(unit, filename, &[])
}

/// Like [`analyze`], but diagnostics use the source file recorded on `tokens`
/// when a line and column match exactly one file.
pub fn analyze_tokens(
    unit: &mut TranslationUnit,
    filename: &str,
    tokens: &[crate::token::Token],
) -> Result<(), Diagnostic> {
    for decl in &mut unit.decls {
        if let TopDecl::Func(func) = decl {
            assign_slots(func);
        }
    }
    let mut locs: HashMap<(usize, usize), String> = HashMap::new();
    for token in tokens {
        if token.kind != crate::token::TokenKind::Eof {
            locs.insert((token.line, token.column), token.filename.clone());
        }
    }
    let mut sema = Sema {
        filename: filename.to_string(),
        locs,
        scopes: vec![HashMap::new()],
        loop_depth: 0,
        switches: Vec::new(),
        current_ret: CType::int_type(),
        fn_name: String::new(),
        current_func: String::new(),
        global_order: Vec::new(),
        func_order: Vec::new(),
        strings: Vec::new(),
        static_locals: Vec::new(),
        static_seq: 0,
        library_nan_calls: Vec::new(),
        object_decls: HashMap::new(),
        sizeof_depth: 0,
        labels: HashMap::new(),
        variadic_fn: false,
        last_param: None,
    };
    for decl in &mut unit.decls {
        match decl {
            TopDecl::Global(global) => sema.declare_global(global)?,
            TopDecl::Func(func) => {
                sema.declare_func(func)?;
                if func.body.is_some() {
                    sema.check_function(func)?;
                }
            }
        }
    }
    sema.finish(unit)
}

#[derive(Clone)]
enum GlobalState {
    /// `extern` with no initializer and no tentative definition yet.
    Declared,
    /// A declaration with no initializer and no `extern` yet.
    Tentative,
    /// Folded initializer, including a zero of the object's type.
    Defined(ConstInit),
}

#[derive(Clone)]
struct GlobalInfo {
    linkage: Linkage,
    state: GlobalState,
}

#[derive(Clone)]
struct VarSym {
    name: String,
    slot: String,
    ty: CType,
    /// `None` means no linkage: a parameter, an automatic variable, or a
    /// block-scope `static` (C11 6.2.2p6). `Some` is a file-scope object or a
    /// block-scope `extern` that refers to one.
    global: Option<GlobalInfo>,
    /// File-scope symbols introduced only by a block declaration stay hidden
    /// from lookup until a file-scope declaration names them. Symbols that
    /// do not live in the file scope leave this `true`.
    file_visible: bool,
}

#[derive(Clone)]
struct FuncSym {
    name: String,
    /// Linker name, including an `__asm__` label.
    symbol: String,
    ret: CType,
    params: Vec<CType>,
    variadic: bool,
    /// `false` for `int f();` (C11 6.7.6.3p14). A call then applies the
    /// default argument promotions and checks no argument count.
    prototype: bool,
    linkage: Linkage,
    defined: bool,
    used: bool,
    use_line: usize,
    use_column: usize,
    /// See [`VarSym::file_visible`].
    file_visible: bool,
}

#[derive(Clone)]
enum Symbol {
    Var(VarSym),
    Func(FuncSym),
}

/// The switch whose body is being checked. Case values are compared after
/// conversion to `promoted` (C11 6.8.4.2).
struct SwitchCtx {
    promoted: TypeName,
    seen: HashSet<u64>,
    has_default: bool,
}

struct Sema {
    filename: String,
    /// Last token at each line and column. Included files override the main
    /// file when they share a coordinate; parser diagnostics resolve this
    /// more precisely from the token cursor.
    locs: HashMap<(usize, usize), String>,
    scopes: Vec<HashMap<String, Symbol>>,
    loop_depth: u32,
    /// Innermost switch first at the end. A `case` or `default` belongs to
    /// `switches.last()`, not to a switch that encloses that one.
    switches: Vec<SwitchCtx>,
    current_ret: CType,
    /// Name of the function whose body is being checked. Empty at file scope.
    /// `__func__` uses it. `current_func` is the same name, kept for the
    /// static-local symbol so the two uses stay obvious at their call sites.
    fn_name: String,
    /// Name of the function whose body is being checked. Static locals use it
    /// in their LLVM symbol so two functions can each have a `static int x`.
    current_func: String,
    global_order: Vec<String>,
    func_order: Vec<String>,
    /// String literals that need a private global, keyed by bytes including NUL
    /// and the encoding. `u8` and ordinary literals share the byte encoding.
    strings: Vec<(Vec<u8>, String, StrEnc)>,
    /// Block-scope `static` objects. They have no linkage, so they are not in
    /// the file-scope symbol table. The name is an LLVM symbol, not the C name.
    static_locals: Vec<IrGlobal>,
    static_seq: u32,
    /// Runtime calls of the C library `nan`, `nanf`, or `nanl` that IR
    /// lowering emits for a malformed quiet `__builtin_nan*` tag: the
    /// library name, the builtin name, and the call position.
    library_nan_calls: Vec<(String, String, usize, usize)>,
    /// Where each file-scope object, or block-scope `extern` object, was
    /// first declared.
    object_decls: HashMap<String, (usize, usize)>,
    /// Nesting depth of `sizeof` operands, which are not evaluated.
    sizeof_depth: u32,
    /// Labels defined in the function being checked. A `goto` names one of them.
    labels: HashMap<String, (usize, usize)>,
    /// The function being checked is variadic. `va_start` requires that.
    variadic_fn: bool,
    /// Name of the last named parameter, which `va_start` must name.
    last_param: Option<String>,
}

impl Sema {
    fn declare_global(&mut self, decl: &mut GlobalDecl) -> Result<(), Diagnostic> {
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
        // A hidden file entity from a block-scope extern is not a visible
        // prior declaration (C11 6.2.7p4), so it is not composed into this
        // declarator's type before the initializer is checked.
        let mut hidden_ty = None;
        if prebound {
            // C11 6.2.1p7: the identifier is in scope after its declarator,
            // including in the initializer.
            let state = if decl.init.is_none() && decl.storage == Storage::Extern {
                GlobalState::Declared
            } else if decl.init.is_none() {
                GlobalState::Tentative
            } else {
                GlobalState::Declared
            };
            self.global_order.push(name.clone());
            self.object_decls
                .entry(name.clone())
                .or_insert((decl.line, decl.column));
            self.insert_file(Symbol::Var(VarSym {
                name: name.clone(),
                slot: format!("@{name}"),
                ty: ty.clone(),
                global: Some(GlobalInfo { linkage, state }),
                file_visible: true,
            }));
        } else if let Some(Symbol::Var(var)) = &prior {
            if !var.file_visible {
                // Reveal the name with this declarator's type (C11 6.2.1p7)
                // so an initializer such as `{ [0 && sizeof a] = 7 }` on
                // `int a[]` sees an incomplete type (6.5.3.4p1).
                if !compatible(&var.ty, &ty) {
                    return self.err(
                        decl.line,
                        decl.column,
                        format!("conflicting types for '{name}'"),
                    );
                }
                hidden_ty = Some(var.ty.clone());
                self.reveal_file(&name);
                self.set_global_type(&name, ty.clone());
            }
        }
        let shaped = if let Some(init) = &decl.init {
            Some(self.shape_initializer(&ty, init, decl.line, decl.column)?)
        } else {
            None
        };
        if let Some((shaped_ty, _)) = &shaped {
            ty = shaped_ty.clone();
        }
        if let Some(Symbol::Var(var)) = self.file_get(&name) {
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
        }
        if let Some((_, mut stores)) = shaped {
            if matches!(self.global_state(&name), Some(GlobalState::Defined(_))) {
                return self.err(decl.line, decl.column, format!("redefinition of '{name}'"));
            }
            self.check_stores(&mut stores)?;
            let value = self.fold_stores(&name, &ty, &stores)?;
            self.set_global_state(&name, GlobalState::Defined(value));
        } else if decl.storage != Storage::Extern {
            self.upgrade_tentative(&name);
        }
        Ok(())
    }

    fn declare_func(&mut self, func: &mut FuncDef) -> Result<(), Diagnostic> {
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

    fn check_function(&mut self, func: &mut FuncDef) -> Result<(), Diagnostic> {
        self.current_ret = func.ret_type.clone();
        self.fn_name = func.name.clone();
        self.current_func = func.name.clone();
        self.push();
        for param in &func.params {
            let Some(name) = param.name.clone() else {
                continue;
            };
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
        self.collect_labels(body)?;
        self.variadic_fn = func.variadic;
        self.last_param = func.params.last().and_then(|param| param.name.clone());
        self.check_compound(body, false)?;
        self.pop();
        self.fn_name.clear();
        self.current_func.clear();
        self.variadic_fn = false;
        self.last_param = None;
        self.labels.clear();
        Ok(())
    }

    fn finish(&mut self, unit: &mut TranslationUnit) -> Result<(), Diagnostic> {
        self.check_library_nan_clash()?;
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
                    globals.push(IrGlobal {
                        name: var.name.clone(),
                        ty: var.ty.clone(),
                        linkage: info.linkage,
                        init,
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
    fn check_library_nan_clash(&self) -> Result<(), Diagnostic> {
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

    fn object_linkage(
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

    fn func_linkage(
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

    fn same_linkage(
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

    fn shape_initializer(
        &mut self,
        ty: &CType,
        init: &Init,
        line: usize,
        column: usize,
    ) -> Result<(CType, Vec<ElemStore>), Diagnostic> {
        let mut ty = ty.clone();
        if let CType::Array(elem, None) = &ty {
            let elem = (**elem).clone();
            if let Some((bytes, enc)) = string_payload(init) {
                if string_elem_ok(&elem, enc) {
                    let unit = enc.unit() as u64;
                    ty = CType::array(elem.clone(), Some(bytes.len() as u64 / unit + 1));
                } else if matches!(init, Init::Expr(_)) {
                    let (line, column) = init_pos(init);
                    return self.err(
                        line,
                        column,
                        "a string literal cannot initialize this array",
                    );
                }
                // A brace list around a string, with a non-character element,
                // falls through to element-list length inference.
            }
            if ty.array_len().is_none() {
                let Init::List {
                    items,
                    line,
                    column,
                } = init
                else {
                    let (line, column) = init_pos(init);
                    return self.err(line, column, "array initializer must be a brace list");
                };
                let count = self.infer_array_len(&elem, items, *line, *column)?;
                ty = CType::array(elem, Some(count));
            }
        }
        let mut stores = Vec::new();
        self.fill_object(&ty, init, 0, &mut stores, line, column)?;
        if stores.is_empty() && !ty.is_array() {
            return self.err(line, column, "empty initializer list");
        }
        Ok((ty, stores))
    }

    /// Fold an array designator after typing it, so `sizeof` operands are
    /// typed before evaluation (C11 6.7.9p6, 6.5.3.4).
    fn designator_index(
        &mut self,
        expr: &Expr,
        line: usize,
        column: usize,
    ) -> Result<u64, Diagnostic> {
        let mut expr = expr.clone();
        self.check_expr(&mut expr)?;
        let value = match eval_const_expr(&expr) {
            Ok(value) if value.ty.is_integer() => value,
            _ => {
                return self.err(line, column, "designator is not an integer constant");
            }
        };
        if value.ty.is_signed() {
            let signed = sign_extend(value.bits, value.ty.width());
            if signed < 0 {
                return self.err(line, column, "array designator is negative");
            }
            Ok(signed as u64)
        } else {
            Ok(value.bits)
        }
    }

    fn infer_array_len(
        &mut self,
        elem: &CType,
        items: &[Init],
        line: usize,
        column: usize,
    ) -> Result<u64, Diagnostic> {
        if items.is_empty() {
            return self.err(line, column, "empty initializer list");
        }
        let mut pos = 0usize;
        let mut index = 0u64;
        let mut count = 0u64;
        let mut scratch = Vec::new();
        while pos < items.len() {
            if let Init::Designated {
                designators,
                line: dline,
                column: dcol,
                ..
            } = &items[pos]
            {
                let Some(Designator::Index { expr, line, column }) = designators.first() else {
                    return self.err(*dline, *dcol, "array designator is required");
                };
                let at = self.designator_index(expr, *line, *column)?;
                if at > 1_000_000 {
                    return self.err(*line, *column, "array designator is too large");
                }
                let Init::Designated { init: first, .. } = &items[pos] else {
                    unreachable!("designated item");
                };
                let elided = designators.len() == 1
                    && !elem.is_scalar()
                    && matches!(first.as_ref(), Init::Expr(_))
                    && !self.opens_subobject(elem, first)
                    && !init_children(elem, false).is_empty();
                if designators.len() > 1 || elided {
                    let rest = designators[1..].to_vec();
                    let dline = *dline;
                    let dcol = *dcol;
                    let first = first.as_ref().clone();
                    let mut scratch = Vec::new();
                    let mut frames = if elided {
                        Vec::new()
                    } else {
                        let (child, frames) = self.seek_designator(elem, 0, &rest, dline, dcol)?;
                        let mut frames = frames;
                        if self.elides_into(&child, &first) {
                            let child_base =
                                frames.last().map(|frame| frame.base).unwrap_or(0) + child.offset;
                            frames.push(InitFrame {
                                children: init_children(&child.ty, false),
                                index: 0,
                                base: child_base,
                                is_union: child.ty.is_union(),
                            });
                            self.positional_one(&first, &mut frames, &mut scratch, elem)?;
                        } else if !advance_frames(&mut frames) {
                            frames.clear();
                        }
                        frames
                    };
                    if elided {
                        frames.push(InitFrame {
                            children: init_children(elem, false),
                            index: 0,
                            base: 0,
                            is_union: elem.is_union(),
                        });
                        self.positional_one(&first, &mut frames, &mut scratch, elem)?;
                    }
                    pos += 1;
                    while pos < items.len()
                        && !frames.is_empty()
                        && !matches!(items[pos], Init::Designated { .. })
                    {
                        self.positional_one(&items[pos], &mut frames, &mut scratch, elem)?;
                        pos += 1;
                    }
                    count = count.max(at + 1);
                    index = at + 1;
                    continue;
                }
                index = at;
                pos += 1;
            } else {
                let start = pos;
                if self.opens_subobject(elem, &items[pos]) {
                    if !elem.is_scalar() {
                        self.fill_object(elem, &items[pos], 0, &mut scratch, line, column)?;
                    }
                    pos += 1;
                } else {
                    self.fill_unbraced(elem, items, &mut pos, 0, &mut scratch, (line, column))?;
                }
                if pos == start {
                    return self.err(line, column, "empty initializer list");
                }
            }
            index += 1;
            count = count.max(index);
        }
        Ok(count)
    }

    fn fill_object(
        &mut self,
        ty: &CType,
        init: &Init,
        base: u64,
        stores: &mut Vec<ElemStore>,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        if let Init::List { items, .. } = init {
            if items
                .iter()
                .any(|item| matches!(item, Init::Designated { .. }))
            {
                return self.fill_mixed(ty, items, base, stores);
            }
        }
        if ty.is_scalar() {
            let expr = self.scalar_expr(init)?;
            stores.push(ElemStore {
                offset: base,
                ty: ty.clone(),
                expr,
                bits: None,
            });
            return Ok(());
        }
        if let Init::Expr(expr) = init {
            if ty.is_record() {
                if string_payload(init).is_some() {
                    return self.err(line, column, "struct initializer must be a brace list");
                }
                stores.push(ElemStore {
                    offset: base,
                    ty: ty.clone(),
                    expr: expr.clone(),
                    bits: None,
                });
                return Ok(());
            }
        }
        if let Some((bytes, enc)) = string_payload(init) {
            let bytes = bytes.to_vec();
            if string_inits(ty, enc) {
                return self.fill_from_string(ty, &bytes, enc, base, stores, init);
            }
            if matches!(init, Init::Expr(_)) {
                let (line, column) = init_pos(init);
                return self.err(
                    line,
                    column,
                    "a string literal cannot initialize this array",
                );
            }
        }
        if ty.is_record() {
            return self.fill_struct(ty, init, base, stores, line, column);
        }
        let CType::Array(elem, Some(len)) = ty else {
            return self.err(line, column, "array initializer must be a brace list");
        };
        let Init::List {
            items,
            line,
            column,
        } = init
        else {
            let (line, column) = init_pos(init);
            return self.err(line, column, "array initializer must be a brace list");
        };
        let elem_size = elem.size().expect("complete array element");
        let mut pos = 0usize;
        let mut index = 0u64;
        while pos < items.len() {
            if index >= *len {
                return self.err(*line, *column, "excess elements in array initializer");
            }
            let off = base + index * elem_size;
            if let Some((bytes, enc)) = string_payload(&items[pos]) {
                let bytes = bytes.to_vec();
                if string_inits(elem, enc) {
                    self.fill_from_string(elem, &bytes, enc, off, stores, &items[pos])?;
                    pos += 1;
                    index += 1;
                    continue;
                }
            }
            if self.opens_subobject(elem, &items[pos]) {
                self.fill_object(elem, &items[pos], off, stores, *line, *column)?;
                pos += 1;
            } else {
                self.fill_unbraced(elem, items, &mut pos, off, stores, (*line, *column))?;
            }
            index += 1;
        }
        Ok(())
    }

    fn fill_unbraced(
        &mut self,
        ty: &CType,
        items: &[Init],
        pos: &mut usize,
        base: u64,
        stores: &mut Vec<ElemStore>,
        at: (usize, usize),
    ) -> Result<(), Diagnostic> {
        let (line, column) = at;
        if ty.is_scalar() {
            if *pos >= items.len() {
                return Ok(());
            }
            self.fill_object(ty, &items[*pos], base, stores, line, column)?;
            *pos += 1;
            return Ok(());
        }
        if ty.is_record() {
            let Some(fields) = ty.struct_ref().and_then(StructRef::fields) else {
                return self.err(
                    line,
                    column,
                    format!("variable has incomplete type '{}'", ty.display()),
                );
            };
            // An unbraced union consumes one initializer, the first member.
            // Every member is at offset 0, so walking all of them would store
            // later items on top of the same bytes and steal them from the
            // enclosing initializer.
            let fields = initializable_fields(fields, ty.is_union());
            for field in &fields {
                if *pos >= items.len() {
                    break;
                }
                if field.bit_width.is_some() || self.opens_subobject(&field.ty, &items[*pos]) {
                    self.init_field(field, base, &items[*pos], stores)?;
                    *pos += 1;
                } else {
                    let off = base + field.offset;
                    self.fill_unbraced(&field.ty, items, pos, off, stores, (line, column))?;
                }
            }
            return Ok(());
        }
        let CType::Array(elem, Some(len)) = ty else {
            return self.err(line, column, "array initializer must be a brace list");
        };
        let elem_size = elem.size().expect("complete array element");
        for index in 0..*len {
            if *pos >= items.len() {
                break;
            }
            let off = base + index * elem_size;
            if self.opens_subobject(elem, &items[*pos]) {
                self.fill_object(elem, &items[*pos], off, stores, line, column)?;
                *pos += 1;
            } else {
                self.fill_unbraced(elem, items, pos, off, stores, (line, column))?;
            }
        }
        Ok(())
    }

    fn fill_struct(
        &mut self,
        ty: &CType,
        init: &Init,
        base: u64,
        stores: &mut Vec<ElemStore>,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        let Some(fields) = ty.struct_ref().and_then(StructRef::fields) else {
            return self.err(
                line,
                column,
                format!("variable has incomplete type '{}'", ty.display()),
            );
        };
        // A brace list for a union initializes the first named member only.
        // Unnamed and zero-width bit-fields are padding, not initializers.
        let fields = initializable_fields(fields, ty.is_union());
        let Init::List {
            items,
            line,
            column,
        } = init
        else {
            let (line, column) = init_pos(init);
            return self.err(line, column, "struct initializer must be a brace list");
        };
        let mut pos = 0usize;
        for field in &fields {
            if pos >= items.len() {
                break;
            }
            if field.bit_width.is_some() || self.opens_subobject(&field.ty, &items[pos]) {
                self.init_field(field, base, &items[pos], stores)?;
                pos += 1;
            } else {
                let off = base + field.offset;
                self.fill_unbraced(&field.ty, items, &mut pos, off, stores, (*line, *column))?;
            }
        }
        if pos < items.len() {
            let message = if ty.struct_ref().is_some_and(StructRef::has_flexible_member) {
                "initialization of a flexible array member"
            } else {
                "excess elements in struct initializer"
            };
            return self.err(*line, *column, message);
        }
        Ok(())
    }

    /// One initializer element covers this subobject: a brace list, a scalar,
    /// a string for a character array, or an expression of the struct type.
    fn opens_subobject(&mut self, ty: &CType, init: &Init) -> bool {
        takes_one_init(ty, init) || self.covers_struct(ty, init)
    }

    /// An expression whose type is this struct initializes the whole field,
    /// instead of the first scalar inside it (`struct Line l = {p, q}`).
    fn covers_struct(&mut self, ty: &CType, init: &Init) -> bool {
        if !ty.unqualified().is_record() {
            return false;
        }
        let Init::Expr(expr) = init else {
            return false;
        };
        if string_payload(init).is_some() {
            return false;
        }
        let mut probe = expr.clone();
        if self.check_expr(&mut probe).is_err() {
            return false;
        }
        if self.decay(&mut probe).is_err() {
            return false;
        }
        probe.ty().is_some_and(|src| {
            src.unqualified().is_record() && compatible(&ty.unqualified(), &src.unqualified())
        })
    }

    fn scalar_expr(&self, init: &Init) -> Result<Expr, Diagnostic> {
        match init {
            Init::Expr(expr) => Ok(expr.clone()),
            Init::Designated { line, column, .. } => {
                self.err(*line, *column, "designator in a scalar initializer")
            }
            Init::List {
                items,
                line,
                column,
            } => {
                if items.len() != 1 {
                    let message = if items.is_empty() {
                        "empty initializer list"
                    } else {
                        "excess elements in scalar initializer"
                    };
                    return self.err(*line, *column, message);
                }
                match &items[0] {
                    Init::Expr(expr) => Ok(expr.clone()),
                    Init::Designated { line, column, .. } => {
                        self.err(*line, *column, "designator in a scalar initializer")
                    }
                    Init::List { line, column, .. } => {
                        self.err(*line, *column, "excess braces around scalar initializer")
                    }
                }
            }
        }
    }

    fn init_field(
        &mut self,
        field: &StructField,
        base: u64,
        init: &Init,
        stores: &mut Vec<ElemStore>,
    ) -> Result<(), Diagnostic> {
        if let Some(width) = field.bit_width.filter(|width| *width > 0) {
            let expr = self.scalar_expr(init)?;
            let declared = field.ty.as_integer().expect("bit-field type is an integer");
            stores.push(ElemStore {
                offset: base + field.offset,
                ty: CType::from_name(bitfield_value_type(declared, width)),
                expr,
                bits: Some(BitStore {
                    abs_bit: (base as u32)
                        .saturating_mul(8)
                        .saturating_add(field.bit_offset),
                    width,
                    container_bits: (field.ty.size().unwrap_or(1) * 8) as u32,
                    signed_field: declared.is_signed(),
                    declared,
                }),
            });
            return Ok(());
        }
        let (line, column) = init_pos(init);
        self.fill_object(&field.ty, init, base + field.offset, stores, line, column)
    }

    /// A brace list that contains at least one designator.
    ///
    /// Designators are relative to this brace's object. After one, the next
    /// positional initializer continues at the following subobject, including
    /// through the elements of an array that a designator stepped into.
    fn fill_mixed(
        &mut self,
        ty: &CType,
        items: &[Init],
        base: u64,
        stores: &mut Vec<ElemStore>,
    ) -> Result<(), Diagnostic> {
        let mut frames = vec![InitFrame {
            children: init_children(ty, false),
            index: 0,
            base,
            is_union: ty.is_union(),
        }];
        for item in items {
            if let Init::Designated {
                designators,
                init,
                line,
                column,
            } = item
            {
                let (child, mut frames_at) =
                    self.seek_designator(ty, base, designators, *line, *column)?;
                if self.elides_into(&child, init) {
                    // C11 6.7.9p17, p20: a designated aggregate whose
                    // initializer has no braces starts brace elision at its
                    // first scalar; later positional items continue there.
                    let child_base =
                        frames_at.last().map(|frame| frame.base).unwrap_or(0) + child.offset;
                    frames_at.push(InitFrame {
                        children: init_children(&child.ty, false),
                        index: 0,
                        base: child_base,
                        is_union: child.ty.is_union(),
                    });
                    self.positional_one(init, &mut frames_at, stores, ty)?;
                    frames = frames_at;
                    continue;
                }
                self.init_child(&child, &frames_at, init, stores)?;
                frames = frames_at;
                if !advance_frames(&mut frames) {
                    // The designated object was the last subobject. A later
                    // positional initializer is excess; a later designator
                    // starts over from this brace.
                }
                continue;
            }
            self.positional_one(item, &mut frames, stores, ty)?;
        }
        Ok(())
    }

    /// Whether a designated subobject is an aggregate initialized without
    /// braces, by its first scalar (`{ .point = 1, 2 }`). A brace list, a
    /// string for a character array, or an expression of the member's own
    /// struct or union type initializes the whole subobject instead.
    fn elides_into(&mut self, child: &InitChild, init: &Init) -> bool {
        if child.ty.is_scalar() || child.field.as_ref().is_some_and(|f| f.is_bitfield()) {
            return false;
        }
        if !matches!(init, Init::Expr(_)) || self.opens_subobject(&child.ty, init) {
            return false;
        }
        !init_children(&child.ty, false).is_empty()
    }

    fn seek_designator(
        &mut self,
        ty: &CType,
        base: u64,
        designators: &[Designator],
        line: usize,
        column: usize,
    ) -> Result<(InitChild, Vec<InitFrame>), Diagnostic> {
        let mut current = ty.clone();
        let mut current_base = base;
        let mut frames = Vec::new();
        if designators.is_empty() {
            return self.err(line, column, "designator does not match this type");
        }
        for (nth, designator) in designators.iter().enumerate() {
            if let Designator::Field { name, line, column } = designator {
                if current.unqualified().is_array() {
                    return self.err(*line, *column, "field designator on an array");
                }
                let Some(path) = field_designator_path(&current, name) else {
                    if flexible_member_named(&current, name) {
                        return self.err(
                            *line,
                            *column,
                            "initialization of a flexible array member",
                        );
                    }
                    return self.err(*line, *column, format!("no member named '{name}'"));
                };
                let mut chosen = None;
                for (step, index) in path.iter().enumerate() {
                    let children = init_children(&current, true);
                    if children.is_empty() || *index >= children.len() {
                        return self.err(*line, *column, "designator does not match this type");
                    }
                    let child = children[*index].clone();
                    let is_union = current.is_union();
                    frames.push(InitFrame {
                        children,
                        index: *index,
                        base: current_base,
                        is_union,
                    });
                    if step + 1 == path.len() {
                        chosen = Some(child);
                        break;
                    }
                    current_base += child.offset;
                    current = child.ty.clone();
                }
                let child = chosen.expect("designator path");
                if nth + 1 == designators.len() {
                    return Ok((child, frames));
                }
                current_base += child.offset;
                current = child.ty;
                continue;
            }
            let children = init_children(&current, true);
            if children.is_empty() {
                return self.err(line, column, "designator does not match this type");
            }
            let index = match designator {
                Designator::Field { .. } => unreachable!("field designators are handled above"),
                Designator::Index { expr, line, column } => {
                    if !current.unqualified().is_array() {
                        return self.err(*line, *column, "array designator on a non-array");
                    }
                    let index = self.designator_index(expr, *line, *column)?;
                    if index as usize >= children.len() {
                        return self.err(*line, *column, "array designator is out of range");
                    }
                    index as usize
                }
            };
            let child = children[index].clone();
            frames.push(InitFrame {
                children,
                index,
                base: current_base,
                is_union: current.is_union(),
            });
            if nth + 1 == designators.len() {
                return Ok((child, frames));
            }
            current_base += child.offset;
            current = child.ty;
        }
        self.err(line, column, "designator does not match this type")
    }

    fn init_child(
        &mut self,
        child: &InitChild,
        frames: &[InitFrame],
        init: &Init,
        stores: &mut Vec<ElemStore>,
    ) -> Result<(), Diagnostic> {
        let base = frames.last().map(|frame| frame.base).unwrap_or(0);
        if let Some(field) = &child.field {
            self.init_field(field, base, init, stores)
        } else {
            let (line, column) = init_pos(init);
            self.fill_object(&child.ty, init, base + child.offset, stores, line, column)
        }
    }

    fn positional_one(
        &mut self,
        item: &Init,
        frames: &mut Vec<InitFrame>,
        stores: &mut Vec<ElemStore>,
        root: &CType,
    ) -> Result<(), Diagnostic> {
        loop {
            let Some(frame) = frames.last() else {
                let (line, column) = init_pos(item);
                return self.err(line, column, "excess elements in initializer");
            };
            if frame.index >= frame.children.len() {
                let (line, column) = init_pos(item);
                return self.err(line, column, "excess elements in initializer");
            }
            let child = frame.children[frame.index].clone();
            let child_base = frame.base + child.offset;
            let scalar =
                child.ty.is_scalar() || child.field.as_ref().is_some_and(|f| f.is_bitfield());
            if scalar || self.opens_subobject(&child.ty, item) {
                if let Some(field) = &child.field {
                    if field.is_bitfield() || scalar {
                        self.init_field(field, frame.base, item, stores)?;
                    } else {
                        let (line, column) = init_pos(item);
                        self.fill_object(&child.ty, item, child_base, stores, line, column)?;
                    }
                } else {
                    let (line, column) = init_pos(item);
                    self.fill_object(&child.ty, item, child_base, stores, line, column)?;
                }
                if !advance_frames(frames) {
                    frames.clear();
                }
                return Ok(());
            }
            let nested = init_children(&child.ty, false);
            if nested.is_empty() {
                let (line, column) = init_pos(item);
                return self.err(line, column, "excess elements in initializer");
            }
            frames.push(InitFrame {
                children: nested,
                index: 0,
                base: child_base,
                is_union: child.ty.is_union(),
            });
            let _ = root;
        }
    }

    /// C11 6.7.9p14. The terminating NUL is stored when the array is larger
    /// than the literal or has unknown size (already made known). An array
    /// whose length equals the literal, not counting that NUL, is initialized
    /// without it. A longer literal is a diagnostic.
    fn fill_from_string(
        &self,
        ty: &CType,
        bytes: &[u8],
        enc: StrEnc,
        base: u64,
        stores: &mut Vec<ElemStore>,
        init: &Init,
    ) -> Result<(), Diagnostic> {
        let (line, column) = init_pos(init);
        let Some(len) = ty.array_len() else {
            return self.err(line, column, "array size missing");
        };
        let unit = enc.unit();
        let units: Vec<u64> = bytes
            .chunks(unit)
            .map(|chunk| {
                let mut acc = 0u64;
                for (index, byte) in chunk.iter().enumerate() {
                    acc |= u64::from(*byte) << (8 * index);
                }
                acc
            })
            .collect();
        if units.len() as u64 > len {
            return self.err(line, column, "initializer string is too long for the array");
        }
        let elem = ty.array_elem().expect("character array").clone();
        let elem_size = elem.size().expect("character element");
        let mut data = units;
        if (data.len() as u64) < len {
            data.push(0);
        }
        for (index, value) in data.iter().enumerate() {
            if index as u64 >= len {
                break;
            }
            stores.push(ElemStore {
                offset: base + index as u64 * elem_size,
                ty: elem.clone(),
                expr: Expr::int(*value, TypeName::Int, line, column),
                bits: None,
            });
        }
        Ok(())
    }

    fn check_stores(&mut self, stores: &mut [ElemStore]) -> Result<(), Diagnostic> {
        for store in stores {
            self.check_expr(&mut store.expr)?;
            self.decay(&mut store.expr)?;
            self.can_assign(&store.ty.clone(), &store.expr)?;
        }
        Ok(())
    }

    fn fold_stores(
        &self,
        name: &str,
        ty: &CType,
        stores: &[ElemStore],
    ) -> Result<ConstInit, Diagnostic> {
        if ty.is_union() || ty.record_has_bitfield() {
            return self.fold_bit_object(name, ty, stores);
        }
        // A plain member of a nested bit-field record has no bit store. The
        // record is still a byte image, so that member has to be written into
        // it. A bit store is also enough on its own: a union's range walk
        // sees only the first member, and a later bit-field member would
        // otherwise be missed.
        let mut nested = Vec::new();
        bit_record_ranges(ty, 0, &mut nested);
        if stores.iter().any(|store| store.bits.is_some()) || !nested.is_empty() {
            return self.fold_nested_bits(name, ty, stores);
        }
        let mut unions = UnionImages::new(ty);
        let mut folded = Vec::new();
        for store in stores {
            let value = self.fold_one(name, &store.expr, &store.ty)?;
            if !unions.absorb(store, &value)? {
                folded.push((store.offset, value));
            }
        }
        unions.finish(name, self, &mut folded)?;
        let value = assemble(ty, &folded, 0);
        if !union_bytes_ok(ty, &value) {
            return self.err(
                stores.first().map(|store| store.expr.line()).unwrap_or(1),
                stores.first().map(|store| store.expr.column()).unwrap_or(1),
                format!("initializer of '{name}' uses an address in a union"),
            );
        }
        Ok(value)
    }

    fn fold_bit_object(
        &self,
        name: &str,
        ty: &CType,
        stores: &[ElemStore],
    ) -> Result<ConstInit, Diagnostic> {
        let size = ty.size().unwrap_or(0) as usize;
        let mut bytes = vec![0u8; size];
        for store in stores {
            if let Some(bits) = &store.bits {
                let fold_ty = if bits.declared == TypeName::Bool {
                    CType::from_name(TypeName::Bool)
                } else {
                    store.ty.clone()
                };
                let value = self.fold_one(name, &store.expr, &fold_ty)?;
                let ConstInit::Int(raw) = value else {
                    return self.err(
                        store.expr.line(),
                        store.expr.column(),
                        format!("initializer of '{name}' is not a constant expression"),
                    );
                };
                pack_bits(&mut bytes, bits.abs_bit, bits.width, raw);
            } else {
                let value = self.fold_one(name, &store.expr, &store.ty)?;
                if !write_folded_bytes(&store.ty, &value, &mut bytes, store.offset as usize) {
                    let kind = if ty.is_union() {
                        "union"
                    } else {
                        "bit-field struct"
                    };
                    return self.err(
                        store.expr.line(),
                        store.expr.column(),
                        format!("initializer of '{name}' uses an address in a {kind}"),
                    );
                }
            }
        }
        Ok(ConstInit::Bytes(bytes))
    }

    /// A file-scope initializer whose top type is an ordinary struct or array,
    /// with a bit-field struct nested inside it. The bit-field subobjects
    /// become raw bytes. A sibling field that is an address stays a relocation.
    fn fold_nested_bits(
        &self,
        name: &str,
        ty: &CType,
        stores: &[ElemStore],
    ) -> Result<ConstInit, Diagnostic> {
        let size = ty.size().unwrap_or(0) as usize;
        let mut image = vec![0u8; size];
        let mut ranges = Vec::new();
        bit_record_ranges(ty, 0, &mut ranges);
        let mut unions = UnionImages::new(ty);
        let mut folded = Vec::new();
        for store in stores {
            if let Some(bits) = &store.bits {
                let fold_ty = if bits.declared == TypeName::Bool {
                    CType::from_name(TypeName::Bool)
                } else {
                    store.ty.clone()
                };
                let value = self.fold_one(name, &store.expr, &fold_ty)?;
                let ConstInit::Int(raw) = value else {
                    return self.err(
                        store.expr.line(),
                        store.expr.column(),
                        format!("initializer of '{name}' is not a constant expression"),
                    );
                };
                pack_bits(&mut image, bits.abs_bit, bits.width, raw);
            } else {
                let value = self.fold_one(name, &store.expr, &store.ty)?;
                let inside = ranges
                    .iter()
                    .any(|(start, end)| store.offset >= *start && store.offset < *end);
                if inside {
                    // The nested record is a byte image. A plain member shares
                    // that image; a relocation cannot.
                    if !write_folded_bytes(&store.ty, &value, &mut image, store.offset as usize) {
                        return self.err(
                            store.expr.line(),
                            store.expr.column(),
                            format!(
                                "initializer of '{name}' uses an address in a bit-field struct"
                            ),
                        );
                    }
                } else if !unions.absorb(store, &value)? {
                    folded.push((store.offset, value));
                }
            }
        }
        unions.finish(name, self, &mut folded)?;
        splice_bit_blobs(ty, 0, &image, &mut folded);
        let value = assemble(ty, &folded, 0);
        if !union_bytes_ok(ty, &value) {
            return self.err(
                stores.first().map(|store| store.expr.line()).unwrap_or(1),
                stores.first().map(|store| store.expr.column()).unwrap_or(1),
                format!("initializer of '{name}' uses an address in a union"),
            );
        }
        Ok(value)
    }

    fn fold_one(&self, name: &str, expr: &Expr, dest: &CType) -> Result<ConstInit, Diagnostic> {
        if let Some(dest_int) = dest.as_integer() {
            match eval_num(expr) {
                Ok(NumConst::Int(value)) => {
                    return Ok(ConstInit::Int(convert_bits(value.bits, value.ty, dest_int)));
                }
                Ok(NumConst::Real { bits, kind }) => {
                    return match crate::float::real_to_int(bits, kind, dest_int) {
                        Some(converted) => Ok(ConstInit::Int(converted)),
                        None => self.err(
                            expr.line(),
                            expr.column(),
                            format!("initializer of '{name}' overflows '{}'", dest_int.as_str()),
                        ),
                    };
                }
                Err(_) => {}
            }
            if let Expr::Cast {
                ty_name,
                expr: inner,
                ..
            } = expr
            {
                if let Some(cast_int) = ty_name.as_integer() {
                    if let Ok(ptr) = eval_ptr(inner) {
                        // A file-scope address is not a number until link time.
                        // Truncating it, in the cast or in the destination,
                        // cannot be folded. A 64-bit cast into a 64-bit
                        // object keeps every bit (`ptrtoint` to i64).
                        if matches!(ptr, PtrConst::Addr { .. })
                            && (cast_int.width() < 64 || dest_int.width() < 64)
                        {
                            return self.err(
                                expr.line(),
                                expr.column(),
                                format!("initializer of '{name}' truncates an address"),
                            );
                        }
                        return Ok(match ptr_as_int(ptr, cast_int) {
                            ConstInit::Int(bits) => {
                                ConstInit::Int(convert_bits(bits, cast_int, dest_int))
                            }
                            other => other,
                        });
                    }
                }
            }
            return self.const_diag(name, expr);
        }
        if let Some(kind) = dest.float_kind() {
            return match eval_num(expr) {
                Ok(value) => match value.to_real(kind) {
                    Some(bits) => Ok(ConstInit::Float(bits)),
                    None => self.err(
                        expr.line(),
                        expr.column(),
                        format!("initializer of '{name}' is not a constant expression"),
                    ),
                },
                Err(_) => self.const_diag(name, expr),
            };
        }
        if dest.is_pointer() {
            if is_null_pointer_constant(expr) {
                return Ok(ConstInit::Ptr(PtrConst::Null));
            }
            return match eval_ptr(expr) {
                Ok(ptr) => Ok(ConstInit::Ptr(ptr)),
                Err(_) => self.const_diag(name, expr),
            };
        }
        if dest.is_record() {
            return self.err(
                expr.line(),
                expr.column(),
                format!("initializer of '{name}' is not a constant expression"),
            );
        }
        self.err(
            expr.line(),
            expr.column(),
            format!("initializer of '{name}' is not a constant expression"),
        )
    }

    fn const_diag(&self, name: &str, expr: &Expr) -> Result<ConstInit, Diagnostic> {
        match eval_const_expr(expr) {
            Err(ConstError::DivZero) => self.err(
                expr.line(),
                expr.column(),
                format!("initializer of '{name}' divides by zero"),
            ),
            Err(ConstError::Overflow(ty)) => self.err(
                expr.line(),
                expr.column(),
                format!("initializer of '{name}' overflows '{}'", ty.as_str()),
            ),
            Err(ConstError::Shift) => self.err(
                expr.line(),
                expr.column(),
                format!("initializer of '{name}' uses an undefined shift"),
            ),
            _ => self.err(
                expr.line(),
                expr.column(),
                format!("initializer of '{name}' is not a constant expression"),
            ),
        }
    }

    fn file_get(&self, name: &str) -> Option<Symbol> {
        self.scopes.first().expect("file scope").get(name).cloned()
    }

    fn file_get_mut(&mut self, name: &str) -> Option<&mut Symbol> {
        self.scopes.first_mut().expect("file scope").get_mut(name)
    }

    fn global_state(&self, name: &str) -> Option<GlobalState> {
        match self.scopes.first().expect("file scope").get(name) {
            Some(Symbol::Var(var)) => var.global.as_ref().map(|info| info.state.clone()),
            _ => None,
        }
    }

    fn set_global_type(&mut self, name: &str, ty: CType) {
        if let Some(Symbol::Var(var)) = self.scopes.first_mut().expect("file scope").get_mut(name) {
            var.ty = ty;
        }
    }

    fn set_global_state(&mut self, name: &str, state: GlobalState) {
        if let Some(Symbol::Var(var)) = self.scopes.first_mut().expect("file scope").get_mut(name) {
            if let Some(info) = &mut var.global {
                info.state = state;
            }
        }
    }

    fn upgrade_tentative(&mut self, name: &str) {
        if let Some(Symbol::Var(var)) = self.scopes.first_mut().expect("file scope").get_mut(name) {
            if let Some(info) = &mut var.global {
                if matches!(info.state, GlobalState::Declared) {
                    info.state = GlobalState::Tentative;
                }
            }
        }
    }

    fn mark_defined(&mut self, name: &str) {
        if let Some(Symbol::Func(func)) = self.scopes.first_mut().expect("file scope").get_mut(name)
        {
            func.defined = true;
        }
    }

    fn note_use(&mut self, name: &str, line: usize, column: usize) {
        if let Some(Symbol::Func(func)) = self.scopes.first_mut().expect("file scope").get_mut(name)
        {
            if !func.used {
                func.used = true;
                func.use_line = line;
                func.use_column = column;
            }
        }
    }

    fn insert_file(&mut self, sym: Symbol) {
        let key = symbol_name(&sym).to_string();
        self.scopes
            .first_mut()
            .expect("file scope")
            .insert(key, sym);
    }

    fn check_compound(
        &mut self,
        compound: &mut Compound,
        new_scope: bool,
    ) -> Result<(), Diagnostic> {
        if new_scope {
            self.push();
        }
        for item in &mut compound.items {
            self.check_stmt(item)?;
        }
        if new_scope {
            self.pop();
        }
        Ok(())
    }

    fn check_stmt(&mut self, stmt: &mut Stmt) -> Result<(), Diagnostic> {
        match stmt {
            Stmt::Compound(compound) => self.check_compound(compound, true),
            Stmt::Decl(decl) => self.check_decl(decl),
            Stmt::Return { expr, line, column } => {
                let line = *line;
                let column = *column;
                match expr {
                    None => {
                        if !self.current_ret.is_void() {
                            return self.err(
                                line,
                                column,
                                format!(
                                    "return with no value in a function returning '{}'",
                                    self.current_ret.display()
                                ),
                            );
                        }
                        Ok(())
                    }
                    Some(expr) => {
                        self.check_expr(expr)?;
                        self.decay(expr)?;
                        if self.current_ret.is_void() {
                            return self.err(
                                line,
                                column,
                                "return with a value in a function returning 'void'",
                            );
                        }
                        self.can_assign(&self.current_ret.clone(), expr)
                    }
                }
            }
            Stmt::If {
                cond,
                then_stmt,
                else_stmt,
                ..
            } => {
                self.check_expr(cond)?;
                self.decay(cond)?;
                self.require_scalar(cond)?;
                self.check_stmt(then_stmt)?;
                if let Some(else_stmt) = else_stmt {
                    self.check_stmt(else_stmt)?;
                }
                Ok(())
            }
            Stmt::While { cond, body, .. } => {
                self.check_expr(cond)?;
                self.decay(cond)?;
                self.require_scalar(cond)?;
                self.loop_depth += 1;
                self.check_stmt(body)?;
                self.loop_depth -= 1;
                Ok(())
            }
            Stmt::Do { body, cond, .. } => {
                self.loop_depth += 1;
                self.check_stmt(body)?;
                self.loop_depth -= 1;
                self.check_expr(cond)?;
                self.decay(cond)?;
                self.require_scalar(cond)
            }
            Stmt::Switch {
                cond,
                body,
                promoted,
                ..
            } => {
                self.check_expr(cond)?;
                self.decay(cond)?;
                self.require_integer(cond)?;
                let prom = cond
                    .ty()
                    .expect("typed switch condition")
                    .as_integer()
                    .expect("integer switch condition")
                    .promote();
                *promoted = Some(prom);
                self.switches.push(SwitchCtx {
                    promoted: prom,
                    seen: HashSet::new(),
                    has_default: false,
                });
                self.check_stmt(body)?;
                self.switches.pop();
                Ok(())
            }
            Stmt::Case {
                expr,
                value,
                stmt,
                line,
                column,
            } => {
                let line = *line;
                let column = *column;
                if self.switches.is_empty() {
                    return self.err(line, column, "'case' label is not inside a switch");
                }
                self.check_expr(expr)?;
                self.decay(expr)?;
                let prom = self.switches.last().expect("switch").promoted;
                let bits = self.case_value(expr, prom)?;
                if !self.switches.last_mut().expect("switch").seen.insert(bits) {
                    return self.err(
                        line,
                        column,
                        format!("duplicate case value '{}'", prom.format_bits(bits)),
                    );
                }
                *value = Some(bits);
                self.check_stmt(stmt)
            }
            Stmt::Default { stmt, line, column } => {
                let line = *line;
                let column = *column;
                if self.switches.is_empty() {
                    return self.err(line, column, "'default' label is not inside a switch");
                }
                if self.switches.last().expect("switch").has_default {
                    return self.err(line, column, "multiple default labels in one switch");
                }
                self.switches.last_mut().expect("switch").has_default = true;
                self.check_stmt(stmt)
            }
            Stmt::FuncDecl(func) => self.declare_block_func(func),
            Stmt::For {
                init,
                cond,
                step,
                body,
                ..
            } => {
                self.push();
                self.loop_depth += 1;
                match init {
                    ForInit::Empty => {}
                    ForInit::Decl(decls) => {
                        for decl in decls {
                            self.check_decl(decl)?;
                        }
                    }
                    ForInit::Expr(expr) => {
                        self.check_expr(expr)?;
                        self.decay(expr)?;
                    }
                }
                if let Some(cond) = cond {
                    self.check_expr(cond)?;
                    self.decay(cond)?;
                    self.require_scalar(cond)?;
                }
                if let Some(step) = step {
                    self.check_expr(step)?;
                }
                self.check_stmt(body)?;
                self.loop_depth -= 1;
                self.pop();
                Ok(())
            }
            Stmt::Expr { expr, .. } => {
                if let Some(expr) = expr {
                    self.check_expr(expr)?;
                    self.decay(expr)?;
                }
                Ok(())
            }
            Stmt::Break { line, column } => {
                if self.loop_depth == 0 && self.switches.is_empty() {
                    return self.err(*line, *column, "'break' is not inside a loop or switch");
                }
                Ok(())
            }
            Stmt::Continue { line, column } => {
                if self.loop_depth == 0 {
                    return self.err(*line, *column, "'continue' is not inside a loop");
                }
                Ok(())
            }
            Stmt::Label { stmt, .. } => self.check_stmt(stmt),
            Stmt::Goto { name, line, column } => {
                if !self.labels.contains_key(name) {
                    return self.err(*line, *column, format!("use of undeclared label '{name}'"));
                }
                Ok(())
            }
        }
    }

    /// Labels have function scope (C11 6.2.1). A name may be used before it
    /// is defined, and two labels with the same name are a constraint violation.
    fn collect_labels(&mut self, compound: &Compound) -> Result<(), Diagnostic> {
        for item in &compound.items {
            self.collect_labels_stmt(item)?;
        }
        Ok(())
    }

    fn collect_labels_stmt(&mut self, stmt: &Stmt) -> Result<(), Diagnostic> {
        match stmt {
            Stmt::Label {
                name,
                stmt,
                line,
                column,
            } => {
                if self.labels.contains_key(name) {
                    return self.err(*line, *column, format!("redefinition of label '{name}'"));
                }
                self.labels.insert(name.clone(), (*line, *column));
                self.collect_labels_stmt(stmt)
            }
            Stmt::Compound(compound) => self.collect_labels(compound),
            Stmt::If {
                then_stmt,
                else_stmt,
                ..
            } => {
                self.collect_labels_stmt(then_stmt)?;
                if let Some(else_stmt) = else_stmt {
                    self.collect_labels_stmt(else_stmt)?;
                }
                Ok(())
            }
            Stmt::While { body, .. } | Stmt::Do { body, .. } => self.collect_labels_stmt(body),
            Stmt::For { init, body, .. } => {
                if let ForInit::Decl(decls) = init {
                    for decl in decls {
                        if let Some(init) = &decl.init {
                            let _ = init;
                        }
                    }
                }
                self.collect_labels_stmt(body)
            }
            Stmt::Switch { body, .. }
            | Stmt::Case { stmt: body, .. }
            | Stmt::Default { stmt: body, .. } => self.collect_labels_stmt(body),
            Stmt::Decl(_)
            | Stmt::FuncDecl(_)
            | Stmt::Return { .. }
            | Stmt::Expr { .. }
            | Stmt::Break { .. }
            | Stmt::Continue { .. }
            | Stmt::Goto { .. } => Ok(()),
        }
    }

    fn check_decl(&mut self, decl: &mut VarDecl) -> Result<(), Diagnostic> {
        match decl.storage {
            Storage::Extern => self.declare_block_extern(decl),
            Storage::Static => self.declare_static_local(decl),
            Storage::None => self.declare_auto(decl),
            Storage::Typedef => unreachable!("a typedef is not a variable declaration"),
        }
    }

    /// An automatic local. A second declaration in the same scope is an error
    /// whether or not the earlier one has linkage (C11 6.7p3).
    fn declare_auto(&mut self, decl: &mut VarDecl) -> Result<(), Diagnostic> {
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
        let (ty, stores) = if let Some(init) = &init {
            self.shape_initializer(&decl.ty, init, decl.line, decl.column)?
        } else {
            (decl.ty.clone(), Vec::new())
        };
        if !ty.is_complete() {
            return self.err(decl.line, decl.column, incomplete_local(&decl.name, &ty));
        }
        decl.ty = ty.clone();
        self.set_local_var_type(&decl.name, ty.clone());
        if decl.init.is_some() {
            let mut stores = stores;
            self.check_stores(&mut stores)?;
            decl.checked = Some(classify_init(&ty, stores));
        }
        Ok(())
    }

    /// Block-scope `static`. No linkage, so each declaration is a distinct
    /// object, and the initializer is required to be constant (C11 6.7.9).
    /// The object is initialized once, before startup, not on each entry.
    fn declare_static_local(&mut self, decl: &mut VarDecl) -> Result<(), Diagnostic> {
        if self.lookup_local(&decl.name).is_some() {
            return self.err(
                decl.line,
                decl.column,
                format!("redefinition of '{}'", decl.name),
            );
        }
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
        let (ty, stores) = if let Some(init) = &init {
            self.shape_initializer(&decl.ty, init, decl.line, decl.column)?
        } else {
            (decl.ty.clone(), Vec::new())
        };
        if !ty.is_complete() {
            return self.err(decl.line, decl.column, incomplete_local(&decl.name, &ty));
        }
        decl.ty = ty.clone();
        self.set_local_var_type(&decl.name, ty.clone());
        let value = if decl.init.is_some() {
            let mut stores = stores;
            self.check_stores(&mut stores)?;
            self.fold_stores(&decl.name, &ty, &stores)?
        } else {
            zero_const(&ty)
        };
        self.static_locals.push(IrGlobal {
            name: symbol,
            ty,
            linkage: Linkage::Internal,
            init: Some(value),
        });
        Ok(())
    }

    /// Block-scope `extern`. Linkage comes from the visible prior declaration
    /// (C11 6.2.2p4). A prior with no linkage yields external linkage, and
    /// that is a different object from the no-linkage one. The same scope
    /// cannot mix linkage and no linkage (C11 6.7p3). An initializer is
    /// forbidden (C11 6.7.9p5).
    fn declare_block_extern(&mut self, decl: &mut VarDecl) -> Result<(), Diagnostic> {
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
        let slot = self.bind_file_object(&name, &local_ty, linkage, decl.line, decl.column)?;
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
            }),
            file_visible: true,
        }));
        Ok(())
    }

    fn redeclare_block_extern(&mut self, decl: &mut VarDecl) -> Result<(), Diagnostic> {
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
    fn declare_block_func(&mut self, func: &mut FuncDef) -> Result<(), Diagnostic> {
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

    fn redeclare_block_func(
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
        if !function_types_match(&prior, &def_func_ty(func, params), false) {
            return self.err(
                func.line,
                func.column,
                format!("conflicting types for '{name}'"),
            );
        }
        func.linkage = prior.linkage;
        Ok(())
    }

    fn inherited_object_linkage(
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

    fn inherited_func_linkage(
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

    fn visible_linked_var(&self, name: &str) -> Option<VarSym> {
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
    fn bind_file_object(
        &mut self,
        name: &str,
        local_ty: &CType,
        linkage: Linkage,
        line: usize,
        column: usize,
    ) -> Result<String, Diagnostic> {
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
                    }),
                    file_visible: false,
                }));
                Ok(slot)
            }
        }
    }

    fn bind_file_func(
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

    fn fresh_static_name(&mut self, var: &str) -> String {
        self.static_seq += 1;
        format!("{}.{var}.{}", self.current_func, self.static_seq)
    }

    fn set_local_var_type(&mut self, name: &str, ty: CType) {
        if let Some(Symbol::Var(var)) = self.scopes.last_mut().expect("scope").get_mut(name) {
            var.ty = ty;
        }
    }

    fn reveal_file(&mut self, name: &str) {
        match self.scopes.first_mut().expect("file scope").get_mut(name) {
            Some(Symbol::Var(var)) => var.file_visible = true,
            Some(Symbol::Func(func)) => func.file_visible = true,
            None => {}
        }
    }

    fn check_expr(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let implicit_func_name = if let Expr::Var {
            name, line, column, ..
        } = expr
        {
            if (name == "__func__" || name == "__PRETTY_FUNCTION__") && self.lookup(name).is_none()
            {
                if self.fn_name.is_empty() {
                    return self.err(
                        *line,
                        *column,
                        format!("'{name}' is not allowed outside a function"),
                    );
                }
                let bytes = self.fn_name.as_bytes().to_vec();
                let line = *line;
                let column = *column;
                *expr = Expr::str_lit(bytes, StrEnc::Byte, line, column);
                true
            } else {
                false
            }
        } else {
            false
        };
        match expr {
            Expr::Int { value_ty, .. } => {
                let value_ty = *value_ty;
                expr.set_ty(CType::from_name(value_ty));
                Ok(())
            }
            Expr::Float { kind, .. } => {
                let kind = *kind;
                expr.set_ty(CType::Float(kind));
                Ok(())
            }
            Expr::Var { .. } => self.check_var(expr),
            Expr::Bin { .. } => self.check_bin(expr),
            Expr::Unary { .. } => self.check_unary(expr),
            Expr::Assign { .. } => self.check_assign(expr),
            Expr::CompoundAssign { .. } => self.check_compound_assign(expr),
            Expr::IncDec { .. } => self.check_incdec(expr),
            Expr::Cond { .. } => self.check_cond(expr),
            Expr::Comma { .. } => self.check_comma(expr),
            Expr::Sizeof { .. } => self.check_sizeof(expr),
            Expr::Cast { .. } => self.check_cast(expr),
            Expr::Call { .. } => self.check_call(expr),
            Expr::Index { .. } => self.check_index(expr),
            Expr::Decay { .. } => Ok(()),
            Expr::Str { .. } => {
                self.check_str(expr)?;
                // C11 6.4.2.2: __func__ is `static const char [N]`, not an
                // unqualified string literal (6.4.5p6).
                if implicit_func_name {
                    let ty = expr
                        .ty()
                        .expect("string literal is typed")
                        .with_quals(Quals {
                            is_const: true,
                            ..Quals::default()
                        });
                    expr.set_ty(ty);
                }
                Ok(())
            }
            Expr::Member { .. } => self.check_member(expr),
            Expr::VaArg { .. } => self.check_va_arg(expr),
            Expr::VaOp { .. } => self.check_va_op(expr),
            Expr::Generic { .. } => self.check_generic(expr),
            Expr::CompoundLit { .. } => self.check_compound_lit(expr),
        }
    }

    /// A generic selection (C11 6.5.1.1). The controlling expression is not
    /// evaluated; its type after lvalue conversion (qualifiers dropped, an
    /// array or function becomes a pointer) picks the association whose type
    /// is compatible with it, or `default`. Every association expression is
    /// analyzed for constraints and name resolution. The node is then
    /// replaced by the chosen expression, which keeps its type and whether
    /// it is an lvalue, so unselected expressions are not evaluated and
    /// emit no IR.
    fn check_generic(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let Expr::Generic {
            control,
            assocs,
            line,
            column,
            ..
        } = expr
        else {
            unreachable!("generic selection");
        };
        let (line, column) = (*line, *column);
        let mut assocs = std::mem::take(assocs);
        // The controlling expression is not evaluated (C11 6.5.1.1p3), so
        // a malformed __builtin_nan tag there makes no library call.
        let control_nan_calls = self.library_nan_calls.len();
        self.check_expr(control)?;
        self.library_nan_calls.truncate(control_nan_calls);
        let raw = control.ty().expect("typed");
        let ty = match raw.peel() {
            CType::Array(elem, _) => CType::pointer((**elem).clone()),
            CType::Func(_) => CType::pointer(raw.clone()),
            _ => raw.unqualified(),
        };
        let mut default = None;
        let mut chosen = None;
        for (index, assoc) in assocs.iter().enumerate() {
            let Some(assoc_ty) = &assoc.ty else {
                if default.is_some() {
                    return self.err(
                        assoc.line,
                        assoc.column,
                        "duplicate default generic association",
                    );
                }
                default = Some(index);
                continue;
            };
            if assoc_ty.is_func() || !assoc_ty.is_complete() {
                return self.err(
                    assoc.line,
                    assoc.column,
                    format!(
                        "type '{}' in generic association is not a complete object type",
                        assoc_ty.display()
                    ),
                );
            }
            for earlier in assocs[..index].iter().filter_map(|a| a.ty.as_ref()) {
                if compatible(earlier, assoc_ty) {
                    return self.err(
                        assoc.line,
                        assoc.column,
                        format!(
                            "type '{}' in generic association compatible with previously specified type '{}'",
                            assoc_ty.display(),
                            earlier.display()
                        ),
                    );
                }
            }
            if chosen.is_none() && compatible(assoc_ty, &ty) {
                chosen = Some(index);
            }
        }
        let Some(index) = chosen.or(default) else {
            return self.err(
                line,
                column,
                format!(
                    "controlling expression type '{}' not compatible with any generic association type",
                    ty.display()
                ),
            );
        };
        for (i, assoc) in assocs.iter_mut().enumerate() {
            if i != index {
                // Constraints are checked, but unselected associations are
                // not evaluated and must not leave IR artifacts behind
                // (C11 6.5.1.1p3).
                let locals = self.static_locals.len();
                let strings = self.strings.len();
                let seq = self.static_seq;
                let nan_calls = self.library_nan_calls.len();
                self.check_expr(&mut assoc.expr)?;
                self.static_locals.truncate(locals);
                self.strings.truncate(strings);
                self.static_seq = seq;
                self.library_nan_calls.truncate(nan_calls);
            }
        }
        *expr = assocs.swap_remove(index).expr;
        self.check_expr(expr)
    }

    /// A compound literal (C11 6.5.2.5). The type is a complete object type
    /// or an array of unknown size, completed by the initializer. Inside a
    /// function the object is automatic and initialized where the literal is
    /// evaluated. Outside one it has static storage, the initializer must be
    /// constant, and the node becomes a reference to a private global.
    fn check_compound_lit(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let Expr::CompoundLit {
            decl, line, column, ..
        } = expr
        else {
            unreachable!("compound literal");
        };
        let (line, column) = (*line, *column);
        if decl.ty.is_func() {
            return self.err(line, column, "compound literal has function type");
        }
        let unknown_array = matches!(decl.ty.peel(), CType::Array(_, None));
        if !decl.ty.is_complete() && !unknown_array {
            return self.err(
                line,
                column,
                format!(
                    "compound literal has incomplete type '{}'",
                    decl.ty.display()
                ),
            );
        }
        let init = decl.init.clone().expect("compound literal initializer");
        let (ty, mut stores) = self.shape_initializer(&decl.ty, &init, line, column)?;
        decl.ty = ty.clone();
        self.check_stores(&mut stores)?;
        if !self.fn_name.is_empty() {
            decl.checked = Some(classify_init(&ty, stores));
            expr.set_ty(ty);
            return Ok(());
        }
        let value = self.fold_stores("compound literal", &ty, &stores)?;
        self.static_seq += 1;
        let symbol = format!(".compoundliteral.{}", self.static_seq);
        self.static_locals.push(IrGlobal {
            name: symbol.clone(),
            ty: ty.clone(),
            linkage: Linkage::Internal,
            init: Some(value),
        });
        *expr = Expr::Var {
            name: String::new(),
            line,
            column,
            resolved: Some(VarRes {
                slot: format!("@{symbol}"),
            }),
            ty: Some(ty),
        };
        Ok(())
    }

    /// A string literal has type `char[N]` or `wchar_t[N]`, counting the
    /// terminating NUL. The private global is created when the address is
    /// used, not for `sizeof` and not when the literal only initializes an
    /// array.
    fn check_str(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let (len, elem) = match expr {
            Expr::Str { bytes, enc, .. } => {
                let unit = enc.unit() as u64;
                (bytes.len() as u64 / unit + 1, enc.elem())
            }
            _ => unreachable!("string literal"),
        };
        expr.set_ty(CType::array(CType::from_name(elem), Some(len)));
        Ok(())
    }

    /// Record a string literal the first time its address is needed. Identical
    /// contents share one private global.
    fn ensure_string(&mut self, expr: &mut Expr) {
        let (bytes, enc) = match expr {
            Expr::Str {
                symbol: None,
                bytes,
                enc,
                ..
            } => (bytes.clone(), *enc),
            _ => return,
        };
        let symbol = self.intern_string(&bytes, enc);
        if let Expr::Str { symbol: slot, .. } = expr {
            *slot = Some(symbol);
        }
    }

    fn intern_string(&mut self, bytes: &[u8], enc: StrEnc) -> String {
        let mut key = bytes.to_vec();
        key.extend(std::iter::repeat(0u8).take(enc.unit()));
        if let Some((_, name, _)) = self
            .strings
            .iter()
            .find(|(stored, _, stored_enc)| stored == &key && *stored_enc == enc)
        {
            return name.clone();
        }
        let name = if self.strings.is_empty() {
            ".str".to_string()
        } else {
            format!(".str.{}", self.strings.len())
        };
        self.strings.push((key, name.clone(), enc));
        name
    }

    /// Array expressions become pointers except as the operand of `sizeof` or `&`.
    ///
    /// An lvalue of incomplete type, other than an array, is not converted to
    /// a value (C11 6.3.2.1p2). A void expression is not an object lvalue, so
    /// a discarded call that returns `void` stays allowed. `&*p` does not call
    /// this on the dereference, so a pointer to an incomplete struct may still
    /// be the operand of `&`.
    fn decay(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        self.ensure_string(expr);
        let Some(ty) = expr.ty() else {
            return Ok(());
        };
        if let CType::Array(elem, _) = ty.clone() {
            let line = expr.line();
            let column = expr.column();
            let inner = std::mem::replace(expr, Expr::int(0, TypeName::Int, line, column));
            *expr = Expr::Decay {
                expr: Box::new(inner),
                line,
                column,
                ty: Some(CType::pointer(*elem)),
            };
            return Ok(());
        }
        if ty.is_func() {
            let line = expr.line();
            let column = expr.column();
            let ptr = CType::pointer(ty);
            let inner = std::mem::replace(expr, Expr::int(0, TypeName::Int, line, column));
            *expr = Expr::Decay {
                expr: Box::new(inner),
                line,
                column,
                ty: Some(ptr),
            };
            return Ok(());
        }
        if !ty.is_void() && !ty.is_complete() {
            return self.err(
                expr.line(),
                expr.column(),
                format!("cannot use incomplete type '{}' as a value", ty.display()),
            );
        }
        Ok(())
    }

    fn check_var(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let (name, line, column) = match expr {
            Expr::Var {
                name, line, column, ..
            } => (name.clone(), *line, *column),
            _ => unreachable!("check_var"),
        };
        let sym = match self.lookup(&name) {
            Some(sym) => sym,
            None => {
                return self.err(
                    line,
                    column,
                    format!("use of undeclared identifier '{name}'"),
                );
            }
        };
        let (slot, ty) = match sym {
            Symbol::Var(var) => (var.slot, var.ty),
            Symbol::Func(func) => {
                // A function used as a value is a use. A static function that
                // is never defined must still be diagnosed, and the IR must
                // not name it without a declaration.
                self.note_use(&name, line, column);
                (
                    format!("@{}", func.symbol),
                    CType::func_with(func.ret, func.params, func.variadic, func.prototype),
                )
            }
        };
        if let Expr::Var { resolved, .. } = expr {
            *resolved = Some(VarRes { slot });
        }
        expr.set_ty(ty);
        Ok(())
    }

    fn check_bin(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let (op, line, column) = match expr {
            Expr::Bin {
                op, line, column, ..
            } => (*op, *line, *column),
            _ => unreachable!("binary expression"),
        };
        let ty = if let Expr::Bin { left, right, .. } = expr {
            self.check_expr(left)?;
            self.check_expr(right)?;
            self.decay(left)?;
            self.decay(right)?;
            if matches!(op, BinOp::And | BinOp::Or) {
                self.require_scalar(left)?;
                self.require_scalar(right)?;
                CType::int_type()
            } else if op.is_cmp() {
                self.check_compare(op, left, right, line, column)?
            } else if matches!(op, BinOp::Add | BinOp::Sub) {
                self.check_additive(op, left, right, line, column)?
            } else if matches!(
                op,
                BinOp::Mod | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr
            ) {
                self.require_integer(left)?;
                self.require_integer(right)?;
                let left_ty = integer_ty(left);
                let right_ty = integer_ty(right);
                let result = if matches!(op, BinOp::Shl | BinOp::Shr) {
                    left_ty.promote()
                } else {
                    left_ty.usual(right_ty)
                };
                CType::from_name(result)
            } else {
                self.arith_type(op, left, right, line, column)?
            }
        } else {
            unreachable!("binary expression");
        };
        expr.set_ty(ty);
        Ok(())
    }

    /// `*` and `/` on arithmetic types. One floating operand selects the
    /// usual real type (C11 6.3.1.8): `double` beats `float`, and either
    /// beats every integer. `%` and the bitwise operators stay on the
    /// integer path.
    fn arith_type(
        &self,
        op: BinOp,
        left: &Expr,
        right: &Expr,
        line: usize,
        column: usize,
    ) -> Result<CType, Diagnostic> {
        let left_ty = left.ty().expect("typed");
        let right_ty = right.ty().expect("typed");
        if let Some(common) = self.float_arith(&left_ty, &right_ty) {
            return Ok(common);
        }
        if left_ty.is_integer() && right_ty.is_integer() {
            return Ok(CType::from_name(integer_ty(left).usual(integer_ty(right))));
        }
        self.err(
            line,
            column,
            format!(
                "invalid operands to '{}' ({} and {})",
                op.as_str(),
                left_ty.display(),
                right_ty.display()
            ),
        )
    }

    /// `Some` when at least one operand is floating and both are arithmetic.
    /// `None` when neither is floating, so the caller keeps the integer or
    /// pointer rule. `long double` wins the usual arithmetic conversions.
    fn float_arith(&self, left: &CType, right: &CType) -> Option<CType> {
        if !(left.is_float() || right.is_float()) {
            return None;
        }
        if left.is_arithmetic() && right.is_arithmetic() {
            return Some(CType::Float(crate::float::usual_real(
                left.float_kind(),
                right.float_kind(),
            )));
        }
        None
    }

    fn check_compare(
        &self,
        op: BinOp,
        left: &Expr,
        right: &Expr,
        line: usize,
        column: usize,
    ) -> Result<CType, Diagnostic> {
        let left_ty = left.ty().expect("typed");
        let right_ty = right.ty().expect("typed");
        if (left_ty.is_float() || right_ty.is_float())
            && left_ty.is_arithmetic()
            && right_ty.is_arithmetic()
        {
            return Ok(CType::int_type());
        }
        if left_ty.is_integer() && right_ty.is_integer() {
            return Ok(CType::int_type());
        }
        let equality = matches!(op, BinOp::Eq | BinOp::Ne);
        let function_pointer = left_ty.pointee().is_some_and(|ty| ty.is_func())
            || right_ty.pointee().is_some_and(|ty| ty.is_func());
        if function_pointer && !equality {
            return self.err(
                line,
                column,
                "relational comparison of function pointers is not allowed",
            );
        }
        let pointers_ok = if equality {
            loose_pointers(&left_ty, &right_ty)
                || void_ptr_mix(&left_ty, &right_ty)
                || (left_ty.is_pointer() && is_null_pointer_constant(right))
                || (right_ty.is_pointer() && is_null_pointer_constant(left))
        } else {
            // Relational operators require compatible object pointers. A null
            // pointer constant is not included, and `void *` is not compatible
            // with `int *`. Clang warns for those and still compiles; this
            // compiler rejects them.
            left_ty.is_pointer() && right_ty.is_pointer() && loose_pointers(&left_ty, &right_ty)
        };
        if pointers_ok {
            return Ok(CType::int_type());
        }
        self.err(
            line,
            column,
            format!(
                "invalid operands to '{}' ({} and {})",
                op.as_str(),
                left_ty.display(),
                right_ty.display()
            ),
        )
    }

    fn check_additive(
        &self,
        op: BinOp,
        left: &Expr,
        right: &Expr,
        line: usize,
        column: usize,
    ) -> Result<CType, Diagnostic> {
        let left_ty = left.ty().expect("typed");
        let right_ty = right.ty().expect("typed");
        if let Some(common) = self.float_arith(&left_ty, &right_ty) {
            return Ok(common);
        }
        if left_ty.is_integer() && right_ty.is_integer() {
            return Ok(CType::from_name(integer_ty(left).usual(integer_ty(right))));
        }
        if op == BinOp::Add {
            if let Some(ptr) = pointer_plus_int(&left_ty, &right_ty) {
                self.require_complete_pointee(&ptr, line, column)?;
                return Ok(ptr);
            }
        }
        if op == BinOp::Sub && left_ty.is_pointer() && right_ty.is_integer() {
            self.require_complete_pointee(&left_ty, line, column)?;
            return Ok(left_ty);
        }
        if op == BinOp::Sub && left_ty.is_pointer() && right_ty.is_pointer() {
            if !loose_pointers(&left_ty, &right_ty) {
                return self.err(
                    line,
                    column,
                    format!(
                        "invalid operands to '-' ({} and {})",
                        left_ty.display(),
                        right_ty.display()
                    ),
                );
            }
            self.require_complete_pointee(&left_ty, line, column)?;
            return Ok(CType::from_name(TypeName::Long));
        }
        self.err(
            line,
            column,
            format!(
                "invalid operands to '{}' ({} and {})",
                op.as_str(),
                left_ty.display(),
                right_ty.display()
            ),
        )
    }

    fn require_complete_pointee(
        &self,
        ptr: &CType,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        let Some(pointee) = ptr.pointee() else {
            return self.err(line, column, "pointer arithmetic requires a pointer");
        };
        if pointee.is_void() {
            return self.err(
                line,
                column,
                "arithmetic on a pointer to void is not supported",
            );
        }
        if !pointee.is_complete() {
            return self.err(
                line,
                column,
                format!(
                    "arithmetic on a pointer to incomplete type '{}'",
                    pointee.display()
                ),
            );
        }
        Ok(())
    }

    fn check_unary(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let ty = if let Expr::Unary {
            op,
            expr,
            line,
            column,
            ..
        } = expr
        {
            let op = *op;
            let line = *line;
            let column = *column;
            self.check_expr(expr)?;
            self.ensure_string(expr);
            match op {
                UnaryOp::Addr => {
                    let inner_ty = expr.ty().expect("typed");
                    if inner_ty.is_func() {
                        CType::pointer(inner_ty)
                    } else if !expr.is_lvalue() {
                        return self.err(
                            line,
                            column,
                            "cannot take the address of this expression",
                        );
                    } else if is_bitfield(expr) {
                        return self.err(line, column, "cannot take the address of a bit-field");
                    } else {
                        CType::pointer(inner_ty)
                    }
                }
                UnaryOp::Deref => {
                    self.decay(expr)?;
                    let ty = expr.ty().expect("typed");
                    let Some(pointee) = ty.pointee().cloned() else {
                        return self.err(
                            line,
                            column,
                            format!("indirection requires a pointer, found '{}'", ty.display()),
                        );
                    };
                    pointee
                }
                UnaryOp::Not => {
                    self.decay(expr)?;
                    self.require_scalar(expr)?;
                    CType::int_type()
                }
                UnaryOp::BitNot => {
                    self.decay(expr)?;
                    self.require_integer(expr)?;
                    CType::from_name(integer_ty(expr).promote())
                }
                UnaryOp::Plus | UnaryOp::Minus => {
                    self.decay(expr)?;
                    let src = expr.ty().expect("typed");
                    if let Some(kind) = src.float_kind() {
                        // Unary `+` and `-` do not promote a floating operand.
                        CType::Float(kind)
                    } else {
                        self.require_integer(expr)?;
                        CType::from_name(integer_ty(expr).promote())
                    }
                }
            }
        } else {
            unreachable!("unary expression");
        };
        expr.set_ty(ty);
        Ok(())
    }

    fn check_assign(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let ty = if let Expr::Assign { target, value, .. } = expr {
            self.check_modifiable(target)?;
            self.check_expr(value)?;
            self.decay(value)?;
            let dest = target.ty().expect("lvalue type");
            self.can_assign(&dest, value)?;
            dest
        } else {
            unreachable!("assignment");
        };
        expr.set_ty(ty);
        Ok(())
    }

    fn check_compound_assign(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let ty = if let Expr::CompoundAssign {
            op,
            target,
            value,
            line,
            column,
            ..
        } = expr
        {
            let op = *op;
            let line = *line;
            let column = *column;
            self.check_modifiable(target)?;
            self.check_expr(value)?;
            self.decay(value)?;
            let dest = target.ty().expect("lvalue type");
            if !dest.is_scalar() {
                return self.err(
                    line,
                    column,
                    format!(
                        "compound assignment requires a scalar, found '{}'",
                        dest.display()
                    ),
                );
            }
            if dest.is_pointer() {
                if !matches!(op, BinOp::Add | BinOp::Sub) {
                    return self.err(
                        line,
                        column,
                        format!(
                            "pointer compound assignment does not allow '{}'",
                            op.compound_spelling()
                        ),
                    );
                }
                self.require_integer(value)?;
                self.require_complete_pointee(&dest, line, column)?;
            } else if dest.is_float() || value.ty().is_some_and(|ty| ty.is_float()) {
                if !matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div) {
                    return self.err(
                        line,
                        column,
                        format!(
                            "floating compound assignment does not allow '{}'",
                            op.compound_spelling()
                        ),
                    );
                }
                let rhs = value.ty().expect("typed");
                if !dest.is_arithmetic() || !rhs.is_arithmetic() {
                    return self.err(
                        line,
                        column,
                        format!(
                            "invalid operands to '{}' ({} and {})",
                            op.compound_spelling(),
                            dest.display(),
                            rhs.display()
                        ),
                    );
                }
            } else {
                self.require_integer(value)?;
            }
            dest
        } else {
            unreachable!("compound assignment");
        };
        expr.set_ty(ty);
        Ok(())
    }

    fn check_incdec(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let ty = if let Expr::IncDec {
            target,
            line,
            column,
            ..
        } = expr
        {
            let line = *line;
            let column = *column;
            self.check_modifiable(target)?;
            let dest = target.ty().expect("lvalue type");
            if !dest.is_scalar() {
                return self.err(
                    line,
                    column,
                    format!("++ and -- require a scalar, found '{}'", dest.display()),
                );
            }
            if dest.is_pointer() {
                self.require_complete_pointee(&dest, line, column)?;
            }
            dest
        } else {
            unreachable!("increment");
        };
        expr.set_ty(ty);
        Ok(())
    }

    /// Arithmetic arms use the usual arithmetic conversions. Both arms may be
    /// `void`. Only the chosen arm is evaluated, which constant folding honors.
    fn check_cond(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let ty = if let Expr::Cond {
            cond,
            then_expr,
            else_expr,
            line,
            column,
            ..
        } = expr
        {
            let line = *line;
            let column = *column;
            self.check_expr(cond)?;
            self.decay(cond)?;
            self.require_scalar(cond)?;
            self.check_expr(then_expr)?;
            self.check_expr(else_expr)?;
            self.decay(then_expr)?;
            self.decay(else_expr)?;
            self.cond_type(then_expr, else_expr, line, column)?
        } else {
            unreachable!("conditional expression");
        };
        expr.set_ty(ty);
        Ok(())
    }

    /// The left operand is evaluated and discarded. The result has the type
    /// of the right operand. There is a sequence point between the two.
    fn check_comma(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let ty = if let Expr::Comma { left, right, .. } = expr {
            self.check_expr(left)?;
            self.decay(left)?;
            self.check_expr(right)?;
            self.decay(right)?;
            right.ty().expect("comma operand is typed")
        } else {
            unreachable!("comma expression");
        };
        expr.set_ty(ty);
        Ok(())
    }

    /// `sizeof` does not evaluate its operand. The result type is
    /// `unsigned long`, which is `size_t` on this LP64 target. `void` is
    /// incomplete.
    fn check_sizeof(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        if let Expr::Sizeof {
            operand,
            line,
            column,
            ..
        } = expr
        {
            let line = *line;
            let column = *column;
            let sized = match operand {
                SizeofOperand::Type(ty) => ty.clone(),
                SizeofOperand::Expr(inner) => {
                    self.sizeof_depth += 1;
                    let checked = self.check_expr(inner);
                    self.sizeof_depth -= 1;
                    checked?;
                    if is_bitfield(inner) {
                        return self.err(line, column, "sizeof applied to a bit-field");
                    }
                    inner.ty().expect("sizeof operand is typed")
                }
            };
            if sized.size().is_none() {
                return self.err(
                    line,
                    column,
                    format!("sizeof applied to incomplete type '{}'", sized.display()),
                );
            }
        }
        expr.set_ty(CType::from_name(TypeName::ULong));
        Ok(())
    }

    fn check_cast(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let ty_name = if let Expr::Cast {
            ty_name,
            expr: inner,
            line,
            column,
            ..
        } = expr
        {
            let ty_name = ty_name.clone();
            let line = *line;
            let column = *column;
            self.check_expr(inner)?;
            self.decay(inner)?;
            if ty_name.is_array() {
                return self.err(line, column, "cast to an array type is not supported");
            }
            if !ty_name.is_void() {
                let src = inner.ty().expect("typed");
                // C11 6.5.4: an arithmetic type converts to any arithmetic
                // type. A pointer converts to an integer and the reverse.
                // There is no conversion between a pointer and a floating type.
                let ok = (ty_name.is_arithmetic() && src.is_arithmetic())
                    || (ty_name.is_pointer() && (src.is_pointer() || src.is_integer()))
                    || (ty_name.is_integer() && src.is_pointer());
                if !ok {
                    return self.err(
                        line,
                        column,
                        format!(
                            "cast from '{}' to '{}' is not supported",
                            src.display(),
                            ty_name.display()
                        ),
                    );
                }
            }
            ty_name
        } else {
            unreachable!("cast");
        };
        expr.set_ty(ty_name);
        Ok(())
    }

    fn check_call(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        if let Expr::Call { callee, .. } = &*expr {
            if let Expr::Var { name, .. } = callee.as_ref() {
                if matches!(
                    name.as_str(),
                    "__builtin_memcpy" | "__builtin_memmove" | "__builtin_memset"
                ) {
                    let name = name.clone();
                    return self.check_memory_builtin(expr, &name);
                }
                if matches!(
                    name.as_str(),
                    "__builtin_signbit" | "__builtin_signbitf" | "__builtin_signbitl"
                ) {
                    let name = name.clone();
                    return self.check_signbit(expr, &name);
                }
                if crate::float::FpFormatBuiltin::from_name(name).is_some() {
                    let name = name.clone();
                    return self.check_fp_format_builtin(expr, &name);
                }
                if crate::float::is_fp_compare_builtin(name) {
                    let name = name.clone();
                    return self.check_fp_compare(expr, &name);
                }
            }
        }
        let direct = match expr {
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Var { name, .. } => matches!(self.lookup(name), Some(Symbol::Func(_))),
                _ => false,
            },
            _ => false,
        };
        if !direct {
            return self.check_indirect_call(expr);
        }
        let (name, line, column, argc) = match expr {
            Expr::Call {
                callee,
                args,
                line,
                column,
                ..
            } => {
                let name = match callee.as_ref() {
                    Expr::Var { name, .. } => name.clone(),
                    _ => {
                        return self.err(
                            *line,
                            *column,
                            "only direct function calls are supported",
                        );
                    }
                };
                (name, *line, *column, args.len())
            }
            _ => unreachable!("check_call"),
        };
        let sym = match self.lookup(&name) {
            Some(sym) => sym,
            None => {
                return self.err(
                    line,
                    column,
                    format!("use of undeclared identifier '{name}'"),
                );
            }
        };
        let (fname, ret, params, variadic) = match sym {
            // Without a prototype every argument gets the default argument
            // promotions (C11 6.5.2.2p6), and the call is lowered like a
            // call to a variadic function with no fixed parameters.
            Symbol::Func(func) if !func.prototype => (func.symbol, func.ret, Vec::new(), true),
            Symbol::Func(func) => (func.symbol, func.ret, func.params, func.variadic),
            Symbol::Var(_) => unreachable!("direct call is a function"),
        };
        self.require_passed_aggregate(&ret, line, column, true)?;
        let arity = params.len();
        if variadic {
            if argc < arity {
                return self.err(
                    line,
                    column,
                    format!(
                        "function '{name}' expects at least {} {}, found {argc}",
                        arity,
                        if arity == 1 { "argument" } else { "arguments" },
                    ),
                );
            }
        } else if argc != arity {
            return self.err(
                line,
                column,
                format!(
                    "function '{name}' expects {} {}, found {argc}",
                    arity,
                    if arity == 1 { "argument" } else { "arguments" },
                ),
            );
        }
        if let Expr::Call { args, .. } = expr {
            for (index, arg) in args.iter_mut().enumerate() {
                self.check_expr(arg)?;
                self.decay(arg)?;
                if index < params.len() {
                    self.can_assign(&params[index], arg)?;
                    self.require_passed_aggregate(&params[index], arg.line(), arg.column(), false)?;
                } else if arg.ty().is_some_and(|ty| ty.unqualified().is_record()) {
                    let ty = arg.ty().expect("typed");
                    self.require_passed_aggregate(&ty, arg.line(), arg.column(), false)?;
                } else {
                    // Default argument promotions apply in the IR. There is no
                    // parameter type to convert to; the argument must be a scalar.
                    self.require_scalar(arg)?;
                }
            }
        }
        self.note_use(&name, line, column);
        if let Expr::Call { resolved, .. } = expr {
            *resolved = Some(CallRes {
                name: fname,
                ret: ret.clone(),
                params,
                variadic,
                indirect: false,
            });
        }
        expr.set_ty(ret);
        Ok(())
    }

    /// `__builtin_signbit`, `__builtin_signbitf`, and `__builtin_signbitl`.
    ///
    /// The result is 1 when the sign bit of the floating value is set, and 0
    /// otherwise, including for NaN and negative zero. The argument is
    /// converted to the builtin’s floating type.
    fn check_signbit(&mut self, expr: &mut Expr, name: &str) -> Result<(), Diagnostic> {
        let (line, column) = (expr.line(), expr.column());
        let Expr::Call { args, .. } = expr else {
            unreachable!("signbit");
        };
        if args.len() != 1 {
            return self.err(line, column, format!("{name} expects 1 argument"));
        }
        self.check_expr(&mut args[0])?;
        self.decay(&mut args[0])?;
        let ty = args[0].ty().expect("typed");
        if !ty.is_arithmetic() {
            return self.err(
                args[0].line(),
                args[0].column(),
                format!("{name} requires an arithmetic argument"),
            );
        }
        if let Expr::Call { resolved, .. } = expr {
            *resolved = Some(CallRes {
                name: name.to_string(),
                ret: CType::int_type(),
                params: Vec::new(),
                variadic: false,
                indirect: false,
            });
        }
        expr.set_ty(CType::int_type());
        Ok(())
    }

    /// `__builtin_isgreater`, `__builtin_isgreaterequal`,
    /// `__builtin_isless`, `__builtin_islessequal`,
    /// `__builtin_islessgreater`, and `__builtin_isunordered`: the C11
    /// 7.12.14 comparison macros as glibc `<math.h>` spells them (GCC
    /// "Other Builtins").
    ///
    /// Both operands are arithmetic and at least one is a real floating
    /// type; IR lowering converts both by the usual arithmetic
    /// conversions (6.3.1.8). The result is `int` 0 or 1.
    fn check_fp_compare(&mut self, expr: &mut Expr, name: &str) -> Result<(), Diagnostic> {
        let (line, column) = (expr.line(), expr.column());
        let Expr::Call { args, .. } = expr else {
            unreachable!("comparison builtin");
        };
        if args.len() != 2 {
            return self.err(line, column, format!("{name} expects 2 arguments"));
        }
        for arg in args.iter_mut() {
            self.check_expr(arg)?;
            self.decay(arg)?;
        }
        let left = args[0].ty().expect("typed");
        let right = args[1].ty().expect("typed");
        if !left.is_arithmetic() || !right.is_arithmetic() || !(left.is_float() || right.is_float())
        {
            return self.err(
                line,
                column,
                format!(
                    "non-floating-point arguments in call to {name} ('{}' and '{}')",
                    left.display(),
                    right.display()
                ),
            );
        }
        if let Expr::Call { resolved, .. } = expr {
            *resolved = Some(CallRes {
                name: name.to_string(),
                ret: CType::int_type(),
                params: Vec::new(),
                variadic: false,
                indirect: false,
            });
        }
        expr.set_ty(CType::int_type());
        Ok(())
    }

    /// `__builtin_nan*` / `__builtin_nans*` / `__builtin_inf*` /
    /// `__builtin_huge_val*`.
    ///
    /// The result has the documented floating type and is a constant
    /// expression when the argument, if any, is a string literal that
    /// `strtol` would consume in full (GCC "Floating-Point Format
    /// Builtins"). A non-literal argument is a diagnostic.
    fn check_fp_format_builtin(&mut self, expr: &mut Expr, name: &str) -> Result<(), Diagnostic> {
        let fp = crate::float::FpFormatBuiltin::from_name(name).expect("format builtin");
        let (line, column) = (expr.line(), expr.column());
        let Expr::Call { args, .. } = expr else {
            unreachable!("format builtin");
        };
        if args.len() != fp.arity() {
            return self.err(
                line,
                column,
                format!(
                    "{name} expects {} {}",
                    fp.arity(),
                    if fp.arity() == 1 {
                        "argument"
                    } else {
                        "arguments"
                    }
                ),
            );
        }
        if matches!(fp, crate::float::FpFormatBuiltin::Nan { .. })
            && !crate::float::FpFormatBuiltin::is_string_literal_arg(&args[0])
        {
            return self.err(
                args[0].line(),
                args[0].column(),
                format!("{name} requires a string literal argument"),
            );
        }
        for arg in args.iter_mut() {
            self.check_expr(arg)?;
            self.decay(arg)?;
        }
        // A malformed quiet tag is left to constant evaluation in an
        // ICE or static initializer, and to IR lowering in a runtime
        // context (a call of nan/nanf/nanl). __builtin_nans* has no
        // library equivalent, so a tag that is not consumed in full
        // is always a diagnostic.
        if let crate::float::FpFormatBuiltin::Nan {
            signaling: true, ..
        } = fp
        {
            if crate::float::nan_tag_from_expr(&args[0]).is_none() {
                return self.err(
                    args[0].line(),
                    args[0].column(),
                    format!("{name} requires a string literal that strtol would consume in full"),
                );
            }
        }
        // A malformed quiet tag in a function body that is evaluated
        // becomes a call of the C library function (IR lowering). A file
        // object with that name is checked once the whole unit is seen.
        if let Some(lib) = fp.libm_nan() {
            if self.sizeof_depth == 0
                && !self.current_func.is_empty()
                && crate::float::nan_tag_from_expr(&args[0]).is_none()
            {
                self.library_nan_calls
                    .push((lib.to_string(), name.to_string(), line, column));
            }
        }
        let ret = CType::Float(fp.kind());
        if let Expr::Call { resolved, .. } = expr {
            *resolved = Some(CallRes {
                name: name.to_string(),
                ret: ret.clone(),
                params: Vec::new(),
                variadic: false,
                indirect: false,
            });
        }
        expr.set_ty(ret);
        Ok(())
    }

    /// The three C library memory builtins. They are not declared: a call
    /// names them directly. The result type is `void *`, and the value is
    /// the first argument.
    fn check_memory_builtin(&mut self, expr: &mut Expr, name: &str) -> Result<(), Diagnostic> {
        let (line, column) = (expr.line(), expr.column());
        let Expr::Call { args, .. } = expr else {
            unreachable!("builtin call");
        };
        if args.len() != 3 {
            return self.err(line, column, format!("{name} expects 3 arguments"));
        }
        for arg in args.iter_mut() {
            self.check_expr(arg)?;
            self.decay(arg)?;
        }
        let dest = args[0].ty().expect("typed");
        let mid = args[1].ty().expect("typed");
        let len = args[2].ty().expect("typed");
        if !dest.is_pointer() {
            return self.err(
                args[0].line(),
                args[0].column(),
                format!("{name} destination must be a pointer"),
            );
        }
        if name == "__builtin_memset" {
            if !mid.is_integer() {
                return self.err(
                    args[1].line(),
                    args[1].column(),
                    format!("{name} value must be an integer"),
                );
            }
        } else if !mid.is_pointer() {
            return self.err(
                args[1].line(),
                args[1].column(),
                format!("{name} source must be a pointer"),
            );
        }
        if !len.is_integer() {
            return self.err(
                args[2].line(),
                args[2].column(),
                format!("{name} size must be an integer"),
            );
        }
        if let Expr::Call { resolved, .. } = expr {
            *resolved = Some(CallRes {
                name: name.to_string(),
                ret: CType::pointer(CType::Void),
                params: Vec::new(),
                variadic: false,
                indirect: false,
            });
        }
        expr.set_ty(CType::pointer(CType::Void));
        Ok(())
    }

    fn check_indirect_call(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let (line, column) = (expr.line(), expr.column());
        let Expr::Call { callee, args, .. } = expr else {
            unreachable!("call");
        };
        self.check_expr(callee)?;
        self.decay(callee)?;
        let ty = callee.ty().expect("typed");
        let Some(func) = ty.pointee().and_then(CType::func_ty).cloned() else {
            return self.err(
                line,
                column,
                format!("called object of type '{}' is not a function", ty.display()),
            );
        };
        self.require_passed_aggregate(&func.ret, line, column, true)?;
        let argc = args.len();
        let arity = func.params.len();
        if func.variadic || !func.prototype {
            if argc < arity {
                return self.err(
                    line,
                    column,
                    format!(
                        "function expects at least {arity} {}, found {argc}",
                        if arity == 1 { "argument" } else { "arguments" },
                    ),
                );
            }
        } else if argc != arity {
            return self.err(
                line,
                column,
                format!(
                    "function expects {arity} {}, found {argc}",
                    if arity == 1 { "argument" } else { "arguments" },
                ),
            );
        }
        let params = func.params.clone();
        let ret = func.ret.clone();
        let variadic = func.variadic || !func.prototype;
        if let Expr::Call { args, .. } = expr {
            for (index, arg) in args.iter_mut().enumerate() {
                self.check_expr(arg)?;
                self.decay(arg)?;
                if index < params.len() {
                    self.can_assign(&params[index], arg)?;
                    self.require_passed_aggregate(&params[index], arg.line(), arg.column(), false)?;
                } else if arg.ty().is_some_and(|ty| ty.unqualified().is_record()) {
                    let ty = arg.ty().expect("typed");
                    self.require_passed_aggregate(&ty, arg.line(), arg.column(), false)?;
                } else {
                    self.require_scalar(arg)?;
                }
            }
        }
        if let Expr::Call { resolved, .. } = expr {
            *resolved = Some(CallRes {
                name: String::new(),
                ret: ret.clone(),
                params,
                variadic,
                indirect: true,
            });
        }
        expr.set_ty(ret);
        Ok(())
    }

    fn check_index(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let ty = if let Expr::Index {
            base,
            index,
            line,
            column,
            ..
        } = expr
        {
            let line = *line;
            let column = *column;
            self.check_expr(base)?;
            self.check_expr(index)?;
            self.decay(base)?;
            self.decay(index)?;
            let base_ty = base.ty().expect("typed");
            let index_ty = index.ty().expect("typed");
            let ptr = if base_ty.is_pointer() && index_ty.is_integer() {
                base_ty
            } else if index_ty.is_pointer() && base_ty.is_integer() {
                index_ty
            } else {
                return self.err(line, column, "subscripted value is not an array or pointer");
            };
            let pointee = ptr.pointee().expect("pointer").clone();
            if pointee.is_void() {
                return self.err(
                    line,
                    column,
                    "subscript of a pointer to void is not supported",
                );
            }
            if !pointee.is_complete() {
                return self.err(
                    line,
                    column,
                    format!(
                        "subscript of a pointer to incomplete type '{}'",
                        pointee.display()
                    ),
                );
            }
            pointee
        } else {
            unreachable!("index");
        };
        expr.set_ty(ty);
        Ok(())
    }

    fn check_modifiable(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        self.prepare_lvalue(expr)?;
        let ty = expr.ty().expect("lvalue type");
        if ty.is_array() {
            return self.err(expr.line(), expr.column(), "array type is not assignable");
        }
        if !ty.is_complete() {
            return self.err(
                expr.line(),
                expr.column(),
                format!("cannot assign to incomplete type '{}'", ty.display()),
            );
        }
        if ty.is_const() {
            return self.err(
                expr.line(),
                expr.column(),
                format!("cannot assign to const-qualified type '{}'", ty.display()),
            );
        }
        if ty.is_record() {
            if ty.contains_const() {
                return self.err(
                    expr.line(),
                    expr.column(),
                    "cannot assign to a struct with a const member",
                );
            }
            return Ok(());
        }
        if !ty.is_scalar() {
            return self.err(expr.line(), expr.column(), "expression is not assignable");
        }
        Ok(())
    }

    fn check_va_list_arg(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        self.check_expr(expr)?;
        self.decay(expr)?;
        let ty = expr.ty().expect("typed");
        if va_list_pointee(&ty) {
            return Ok(());
        }
        self.err(
            expr.line(),
            expr.column(),
            format!("expected a va_list, found '{}'", ty.display()),
        )
    }

    fn check_va_arg(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let (line, column, type_arg) = match expr {
            Expr::VaArg {
                ap,
                type_arg,
                line,
                column,
                ..
            } => {
                let line = *line;
                let column = *column;
                let type_arg = type_arg.clone();
                self.check_va_list_arg(ap)?;
                (line, column, type_arg)
            }
            _ => unreachable!("va_arg"),
        };
        if !type_arg.is_complete() || type_arg.is_array() || type_arg.is_func() {
            return self.err(
                line,
                column,
                format!("va_arg of type '{}' is not supported", type_arg.display()),
            );
        }
        self.require_passed_aggregate(&type_arg, line, column, false)?;
        expr.set_ty(type_arg);
        Ok(())
    }

    fn check_va_op(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let (op, line, column) = match expr {
            Expr::VaOp {
                op, line, column, ..
            } => (*op, *line, *column),
            _ => unreachable!("va op"),
        };
        let Expr::VaOp { args, .. } = expr else {
            unreachable!("va op");
        };
        match op {
            VaOp::Start => {
                if !self.variadic_fn {
                    return self.err(
                        line,
                        column,
                        "va_start used in a function that is not variadic",
                    );
                }
                if args.len() != 2 {
                    return self.err(line, column, "va_start expects a va_list and a parameter");
                }
                self.check_va_list_arg(&mut args[0])?;
                self.check_expr(&mut args[1])?;
                let last = self.last_param.clone().unwrap_or_default();
                let names_last = matches!(&args[1], Expr::Var { name, .. } if name == &last);
                if !names_last {
                    return self.err(
                        line,
                        column,
                        "the second argument of va_start must be the last named parameter",
                    );
                }
            }
            VaOp::End => {
                if args.len() != 1 {
                    return self.err(line, column, "va_end expects a va_list");
                }
                self.check_va_list_arg(&mut args[0])?;
            }
            VaOp::Copy => {
                if args.len() != 2 {
                    return self.err(line, column, "va_copy expects two va_list arguments");
                }
                self.check_va_list_arg(&mut args[0])?;
                self.check_va_list_arg(&mut args[1])?;
            }
        }
        expr.set_ty(CType::Void);
        Ok(())
    }

    fn check_member(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let ty = if let Expr::Member {
            base,
            field,
            arrow,
            line,
            column,
            index,
            offset,
            bytewise,
            bit_width,
            bit_signed,
            container_bits,
            abs_bit,
            bit_declared,
            ..
        } = expr
        {
            let field_name = field.clone();
            let arrow = *arrow;
            let line = *line;
            let column = *column;
            self.check_expr(base)?;
            let base_ty = if arrow {
                self.decay(base)?;
                let ty = base.ty().expect("typed");
                let Some(pointee) = ty.pointee().cloned() else {
                    return self.err(
                        line,
                        column,
                        format!("member reference type '{}' is not a pointer", ty.display()),
                    );
                };
                pointee
            } else {
                base.ty().expect("typed")
            };
            let Some(handle) = base_ty.struct_ref().cloned() else {
                return self.err(
                    line,
                    column,
                    format!(
                        "member reference base type '{}' is not a struct",
                        base_ty.display()
                    ),
                );
            };
            if !handle.is_complete() {
                return self.err(
                    line,
                    column,
                    format!("member access on incomplete type 'struct {}'", handle.tag()),
                );
            }
            let Some(hit) = handle.lookup_member(&field_name) else {
                return self.err(
                    line,
                    column,
                    format!(
                        "no member named '{field_name}' in 'struct {}'",
                        handle.tag()
                    ),
                );
            };
            let found = hit.field;
            *index = Some(hit.index);
            *offset = Some(found.offset);
            // An anonymous member's fields are not one LLVM index of the
            // outer object. A byte offset from the outer base reaches them.
            *bytewise = hit.indirect || handle.is_union() || handle.has_bitfield();
            // The struct object's qualifiers apply to the member (C11 6.5.2.3).
            let field_ty = found.ty.clone();
            let declared_int = field_ty.as_integer();
            let mut member_ty = field_ty.with_quals(base_ty.quals());
            if let Some(width) = found.bit_width {
                let declared = declared_int.expect("a bit-field has an integer type");
                *bit_width = Some(width);
                *bit_signed = declared.is_signed();
                *bit_declared = declared;
                *container_bits = (found.ty.size().unwrap_or(1) * 8) as u32;
                *abs_bit = found.bit_offset;
                member_ty = CType::from_name(bitfield_value_type(declared, width))
                    .with_quals(member_ty.quals());
            }
            member_ty
        } else {
            unreachable!("member expression");
        };
        expr.set_ty(ty);
        Ok(())
    }

    fn prepare_lvalue(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        match expr {
            Expr::Var { .. } => self.check_var(expr),
            Expr::Index { .. } => self.check_index(expr),
            Expr::Unary {
                op: UnaryOp::Deref, ..
            } => self.check_unary(expr),
            Expr::Str { .. } => self.check_str(expr),
            Expr::Generic { .. } | Expr::CompoundLit { .. } => {
                self.check_expr(expr)?;
                if !expr.is_lvalue() {
                    return self.err(expr.line(), expr.column(), "expression is not assignable");
                }
                Ok(())
            }
            Expr::Member { .. } => {
                self.check_member(expr)?;
                if !expr.is_lvalue() {
                    return self.err(expr.line(), expr.column(), "expression is not assignable");
                }
                Ok(())
            }
            _ => self.err(expr.line(), expr.column(), "expression is not assignable"),
        }
    }

    /// A struct or union passed or returned by value must be complete.
    /// An x87 aggregate is passed in memory. A return whose class is `X87`
    /// uses `%st0`. SSE eightbytes are passed in XMM registers.
    fn require_passed_aggregate(
        &self,
        ty: &CType,
        line: usize,
        column: usize,
        is_return: bool,
    ) -> Result<(), Diagnostic> {
        let bare = ty.unqualified();
        if !bare.is_record() {
            return Ok(());
        }
        let use_as = if is_return {
            crate::abi::AggUse::Return
        } else {
            crate::abi::AggUse::Param
        };
        match crate::abi::classify_for(&bare, use_as) {
            Ok(_) => Ok(()),
            Err(crate::abi::AbiFail::Incomplete) => {
                if is_return {
                    self.err(
                        line,
                        column,
                        format!("function returns incomplete type '{}'", bare.display()),
                    )
                } else {
                    self.err(
                        line,
                        column,
                        format!("parameter has incomplete type '{}'", bare.display()),
                    )
                }
            }
            Err(crate::abi::AbiFail::FloatReg) => {
                if is_return {
                    self.err(line, column, "returning an x87 aggregate is not supported")
                } else {
                    self.err(line, column, "passing an x87 aggregate is not supported")
                }
            }
        }
    }

    /// Implicit assignment conversion (C11 6.5.16.1).
    ///
    /// Arithmetic types convert. A pointer may be assigned a pointer to a
    /// qualified or unqualified version of a compatible type when the
    /// destination pointee has every qualifier of the source pointee, so
    /// `char *` may initialize `const char *` and `const char *` may not
    /// initialize `char *`. `void *` follows the same qualifier rule. A null
    /// pointer constant may initialize any pointer. Any other pointer mismatch
    /// is an error. Clang only warns for incompatible pointer types
    /// (`-Wincompatible-pointer-types`) and still emits code; this compiler
    /// rejects the program. A cast may drop qualifiers; that is not diagnosed.
    fn can_assign(&self, dest: &CType, src: &Expr) -> Result<(), Diagnostic> {
        let src_ty = src.ty().expect("typed");
        if dest.is_arithmetic() && src_ty.is_arithmetic() {
            return Ok(());
        }
        if src_ty.is_void() {
            return self.err(
                src.line(),
                src.column(),
                "void value used where a value is required",
            );
        }
        if dest.is_integer() && src_ty.is_integer() {
            return Ok(());
        }
        // C11 6.5.16.1: a pointer may be assigned to `_Bool`. The value is
        // 0 when the pointer is null and 1 otherwise.
        if dest.as_integer() == Some(TypeName::Bool) && src_ty.is_pointer() {
            return Ok(());
        }
        if dest.unqualified().is_record()
            && src_ty.unqualified().is_record()
            && compatible(&dest.unqualified(), &src_ty.unqualified())
        {
            return Ok(());
        }
        if dest.is_pointer()
            && ((src_ty.is_pointer() && pointer_assignable(dest, &src_ty))
                || is_null_pointer_constant(src))
        {
            return Ok(());
        }
        self.err(
            src.line(),
            src.column(),
            format!(
                "incompatible types assigning '{}' to '{}'",
                src_ty.display(),
                dest.display()
            ),
        )
    }

    /// Integer constant expression of a `case` label, converted to `promoted`.
    ///
    /// The conversion is C11 6.3.1.3: unsigned destinations wrap, and a signed
    /// destination that cannot represent the value keeps the low bits. Two
    /// labels that differ before that conversion and agree after it are
    /// duplicates.
    fn case_value(&self, expr: &Expr, promoted: TypeName) -> Result<u64, Diagnostic> {
        self.require_integer(expr)?;
        match eval_const_expr(expr) {
            Ok(value) => Ok(convert_bits(value.bits, value.ty, promoted)),
            Err(ConstError::DivZero) => {
                self.err(expr.line(), expr.column(), "case label divides by zero")
            }
            Err(ConstError::Overflow(ty)) => self.err(
                expr.line(),
                expr.column(),
                format!("case label overflows '{}'", ty.as_str()),
            ),
            Err(ConstError::Shift) => self.err(
                expr.line(),
                expr.column(),
                "case label uses an undefined shift",
            ),
            Err(ConstError::NotConst) => self.err(
                expr.line(),
                expr.column(),
                "case label is not an integer constant expression",
            ),
        }
    }

    fn require_integer(&self, expr: &Expr) -> Result<(), Diagnostic> {
        match expr.ty() {
            Some(ty) if ty.is_integer() => Ok(()),
            Some(ty) if ty.is_void() => self.err(
                expr.line(),
                expr.column(),
                "void value used where a value is required",
            ),
            Some(ty) => self.err(
                expr.line(),
                expr.column(),
                format!("expected an integer, found '{}'", ty.display()),
            ),
            None => self.err(
                expr.line(),
                expr.column(),
                "internal compiler error: expression has no type",
            ),
        }
    }

    fn require_scalar(&self, expr: &Expr) -> Result<(), Diagnostic> {
        match expr.ty() {
            Some(ty) if ty.is_scalar() => Ok(()),
            Some(ty) if ty.is_void() => self.err(
                expr.line(),
                expr.column(),
                "void value used where a value is required",
            ),
            Some(ty) => self.err(
                expr.line(),
                expr.column(),
                format!("expected a scalar, found '{}'", ty.display()),
            ),
            None => self.err(
                expr.line(),
                expr.column(),
                "internal compiler error: expression has no type",
            ),
        }
    }

    fn cond_type(
        &self,
        then_expr: &Expr,
        else_expr: &Expr,
        line: usize,
        column: usize,
    ) -> Result<CType, Diagnostic> {
        let left = then_expr.ty().expect("typed");
        let right = else_expr.ty().expect("typed");
        if left.is_void() && right.is_void() {
            return Ok(CType::Void);
        }
        if left.unqualified().is_record() && compatible(&left.unqualified(), &right.unqualified()) {
            return Ok(left);
        }
        if (left.is_float() || right.is_float()) && left.is_arithmetic() && right.is_arithmetic() {
            return Ok(CType::Float(crate::float::usual_real(
                left.float_kind(),
                right.float_kind(),
            )));
        }
        if left.is_integer() && right.is_integer() {
            return Ok(CType::from_name(
                left.as_integer()
                    .unwrap()
                    .usual(right.as_integer().unwrap()),
            ));
        }
        if left.is_pointer() && right.is_pointer() {
            if let Some(merged) = merge_pointers(&left, &right) {
                return Ok(merged);
            }
        }
        if left.is_pointer() && is_null_pointer_constant(else_expr) {
            return Ok(left);
        }
        if right.is_pointer() && is_null_pointer_constant(then_expr) {
            return Ok(right);
        }
        self.err(
            line,
            column,
            "incompatible operand types in the conditional operator",
        )
    }

    fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop(&mut self) {
        self.scopes.pop();
    }

    fn insert(&mut self, sym: Symbol) {
        let name = symbol_name(&sym).to_string();
        self.scopes.last_mut().expect("scope").insert(name, sym);
    }

    fn lookup(&self, name: &str) -> Option<Symbol> {
        for (index, scope) in self.scopes.iter().enumerate().rev() {
            if let Some(sym) = scope.get(name) {
                // A block-scope extern creates a file-scope entity so later
                // declarations can link to it. That entity is not in scope
                // until a file-scope declaration says so (C11 6.2.1).
                if index == 0 && !symbol_file_visible(sym) {
                    continue;
                }
                return Some(sym.clone());
            }
        }
        None
    }

    fn lookup_local(&self, name: &str) -> Option<&Symbol> {
        self.scopes.last().and_then(|scope| scope.get(name))
    }

    fn err<T>(
        &self,
        line: usize,
        column: usize,
        message: impl Into<String>,
    ) -> Result<T, Diagnostic> {
        let file = self
            .locs
            .get(&(line, column))
            .map(String::as_str)
            .unwrap_or(self.filename.as_str());
        Err(Diagnostic::new(file, line, column, message))
    }
}

fn is_bitfield(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Member {
            bit_width: Some(_),
            ..
        }
    )
}

/// The type of a bit-field after the integer promotions (C11 6.3.1.1).
///
/// Only types whose rank is at most `int` are promoted. A signed field, and
/// an unsigned field narrower than 32 bits, become `int`. An unsigned field
/// of 32 bits becomes `unsigned int`. `long` and wider stay as declared.
fn bitfield_value_type(declared: TypeName, width: u32) -> TypeName {
    if declared.rank() > TypeName::Int.rank() {
        return declared;
    }
    let fits_in_int = if declared.is_signed() {
        width <= 32
    } else {
        width < 32
    };
    if fits_in_int {
        TypeName::Int
    } else {
        TypeName::UInt
    }
}

struct InitChild {
    ty: CType,
    offset: u64,
    field: Option<StructField>,
}

impl Clone for InitChild {
    fn clone(&self) -> Self {
        Self {
            ty: self.ty.clone(),
            offset: self.offset,
            field: self.field.clone(),
        }
    }
}

struct InitFrame {
    children: Vec<InitChild>,
    index: usize,
    base: u64,
    /// A union initializes one member. The next positional item continues
    /// after the union, not at another member of it.
    is_union: bool,
}

/// Indexes through `init_children(..., true)` that reach `name`.
///
/// Anonymous struct and union members are not names, so a designator steps
/// through them to the member they promote.
fn field_designator_path(ty: &CType, name: &str) -> Option<Vec<usize>> {
    let children = init_children(ty, true);
    for (index, child) in children.iter().enumerate() {
        if child.field.as_ref().is_some_and(|field| field.name == name) {
            return Some(vec![index]);
        }
        if child
            .field
            .as_ref()
            .is_some_and(StructField::is_anonymous_record)
        {
            if let Some(mut rest) = field_designator_path(&child.ty, name) {
                rest.insert(0, index);
                return Some(rest);
            }
        }
    }
    None
}

fn init_children(ty: &CType, union_all: bool) -> Vec<InitChild> {
    if let Some(handle) = ty.struct_ref() {
        let Some(fields) = handle.fields() else {
            return Vec::new();
        };
        let fields = if ty.is_union() && !union_all {
            initializable_fields(fields, true)
        } else {
            initializable_fields(fields, false)
        };
        return fields
            .into_iter()
            .map(|field| InitChild {
                offset: field.offset,
                ty: field.ty.clone(),
                field: Some(field),
            })
            .collect();
    }
    if let CType::Array(elem, Some(len)) = ty.unqualified() {
        let step = elem.size().unwrap_or(0);
        return (0..len)
            .map(|index| InitChild {
                ty: (*elem).clone(),
                offset: index * step,
                field: None,
            })
            .collect();
    }
    Vec::new()
}

fn advance_frames(frames: &mut Vec<InitFrame>) -> bool {
    loop {
        let Some(frame) = frames.last_mut() else {
            return false;
        };
        if frame.is_union {
            frames.pop();
            continue;
        }
        frame.index += 1;
        if frame.index < frame.children.len() {
            return true;
        }
        frames.pop();
    }
}

fn initializable_fields(fields: Vec<StructField>, union_ty: bool) -> Vec<StructField> {
    let fields: Vec<_> = fields
        .into_iter()
        .filter(|field| {
            field.bit_width != Some(0)
                && !field.is_flexible()
                && (!field.name.is_empty() || field.is_anonymous_record())
        })
        .collect();
    if union_ty {
        fields.into_iter().take(1).collect()
    } else {
        fields
    }
}

fn pack_bits(bytes: &mut [u8], bit: u32, width: u32, value: u64) {
    let mask = if width >= 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    };
    let value = value & mask;
    for i in 0..width {
        let pos = bit + i;
        let index = (pos / 8) as usize;
        if index >= bytes.len() {
            break;
        }
        let shift = pos % 8;
        if (value >> i) & 1 == 1 {
            bytes[index] |= 1 << shift;
        } else {
            bytes[index] &= !(1 << shift);
        }
    }
}

fn write_folded_bytes(ty: &CType, value: &ConstInit, bytes: &mut [u8], offset: usize) -> bool {
    match value {
        ConstInit::Zero => true,
        ConstInit::Int(bits) => {
            let width = ty.as_integer().map(TypeName::width).unwrap_or(0) as usize / 8;
            for index in 0..width {
                let at = offset + index;
                if at < bytes.len() {
                    bytes[at] = (*bits >> (8 * index)) as u8;
                }
            }
            true
        }
        ConstInit::Float(bits) => {
            let width = ty.size().unwrap_or(0) as usize;
            for index in 0..width {
                let at = offset + index;
                if at < bytes.len() {
                    bytes[at] = (*bits >> (8 * index)) as u8;
                }
            }
            true
        }
        ConstInit::Ptr(PtrConst::Null) | ConstInit::Ptr(PtrConst::IntToPtr(_)) => {
            let bits = match value {
                ConstInit::Ptr(PtrConst::IntToPtr(bits)) => *bits,
                _ => 0,
            };
            for index in 0..8 {
                let at = offset + index;
                if at < bytes.len() {
                    bytes[at] = (bits >> (8 * index)) as u8;
                }
            }
            true
        }
        _ => false,
    }
}

fn va_list_pointee(ty: &CType) -> bool {
    ty.pointee()
        .and_then(CType::struct_ref)
        .is_some_and(|handle| handle.tag() == "__va_list_tag")
}

fn integer_ty(expr: &Expr) -> TypeName {
    expr.ty()
        .and_then(|ty| ty.as_integer())
        .expect("integer expression")
}

fn init_pos(init: &Init) -> (usize, usize) {
    match init {
        Init::Expr(expr) => (expr.line(), expr.column()),
        Init::List { line, column, .. } | Init::Designated { line, column, .. } => (*line, *column),
    }
}

fn zero_const(ty: &CType) -> ConstInit {
    if ty.is_integer() {
        ConstInit::Int(0)
    } else if ty.is_pointer() {
        ConstInit::Ptr(PtrConst::Null)
    } else {
        ConstInit::Zero
    }
}

fn pointer_plus_int(left: &CType, right: &CType) -> Option<CType> {
    if left.is_pointer() && right.is_integer() {
        Some(left.clone())
    } else if right.is_pointer() && left.is_integer() {
        Some(right.clone())
    } else {
        None
    }
}

/// Assignment between pointers (C11 6.5.16.1). The destination pointee must
/// have every qualifier of the source pointee. `char *` converts to
/// `const char *`. `char **` does not convert to `const char **`.
fn pointer_assignable(dest: &CType, src: &CType) -> bool {
    let (Some(dp), Some(sp)) = (dest.pointee(), src.pointee()) else {
        return false;
    };
    if !dp.quals().includes(sp.quals()) {
        return false;
    }
    compatible(&dp.unqualified(), &sp.unqualified())
        || dp.unqualified().is_void()
        || sp.unqualified().is_void()
}

/// `?:` on two pointers. The result pointee carries every qualifier from
/// either side. A `void *` arm produces `void *` with those qualifiers.
fn merge_pointers(left: &CType, right: &CType) -> Option<CType> {
    let lp = left.pointee()?;
    let rp = right.pointee()?;
    let quals = lp.quals().merge(rp.quals());
    if lp.unqualified().is_void() || rp.unqualified().is_void() {
        return Some(CType::pointer(CType::Void.with_quals(quals)));
    }
    if compatible(&lp.unqualified(), &rp.unqualified()) {
        let merged = composite(&lp.unqualified(), &rp.unqualified()).with_quals(quals);
        return Some(CType::pointer(merged));
    }
    None
}

/// Bytes of a string literal used as an initializer, including a single pair
/// of braces around that literal (`char s[4] = {"hi"}`).
fn string_payload(init: &Init) -> Option<(&[u8], StrEnc)> {
    match init {
        Init::Expr(Expr::Str { bytes, enc, .. }) => Some((bytes, *enc)),
        Init::Designated { .. } => None,
        Init::List { items, .. } => match items.as_slice() {
            [Init::Expr(Expr::Str { bytes, enc, .. })] => Some((bytes, *enc)),
            _ => None,
        },
        Init::Expr(_) => None,
    }
}

/// The element type is the one C11 6.7.9 allows for this encoding.
fn string_elem_ok(elem: &CType, enc: StrEnc) -> bool {
    match enc {
        StrEnc::Byte => elem.is_character(),
        StrEnc::Utf16 => elem.as_integer() == Some(TypeName::UShort),
        StrEnc::Wchar => elem.as_integer() == Some(TypeName::Int),
        StrEnc::Utf32 => elem.as_integer() == Some(TypeName::UInt),
    }
}

fn string_inits(ty: &CType, enc: StrEnc) -> bool {
    ty.array_elem()
        .is_some_and(|elem| string_elem_ok(elem, enc))
}

fn flexible_member_named(ty: &CType, name: &str) -> bool {
    ty.struct_ref()
        .and_then(StructRef::fields)
        .is_some_and(|fields| {
            fields
                .iter()
                .any(|field| field.name == name && field.is_flexible())
        })
}

/// An integer constant expression with value 0, or that expression cast to `void *`.
fn is_null_pointer_constant(expr: &Expr) -> bool {
    match expr {
        Expr::Cast { ty_name, expr, .. }
            if ty_name
                .pointee()
                .is_some_and(|pointee| pointee.is_void() && pointee.quals().is_empty()) =>
        {
            is_null_pointer_constant(expr)
        }
        _ => match eval_const_expr(expr) {
            Ok(value) => value.ty.is_integer() && (value.bits & value.ty.mask()) == 0,
            Err(_) => false,
        },
    }
}

fn takes_one_init(ty: &CType, init: &Init) -> bool {
    if matches!(init, Init::List { .. }) || ty.is_scalar() {
        return true;
    }
    string_payload(init).is_some_and(|(_, enc)| string_inits(ty, enc))
}

fn classify_init(ty: &CType, mut stores: Vec<ElemStore>) -> CheckedInit {
    // A single store of the whole object is a scalar or a struct copy.
    // One store of a field, including the only field, stays an aggregate
    // store at that field's offset.
    let whole = stores.len() == 1
        && (ty.is_scalar()
            || (ty.is_record() && compatible(&stores[0].ty.unqualified(), &ty.unqualified())));
    if whole {
        return CheckedInit::Scalar(stores.remove(0).expr);
    }
    CheckedInit::Aggregate(stores)
}

/// A union constant is rendered as raw bytes. An address in that constant
/// needs a relocation, which those bytes cannot hold.
fn union_bytes_ok(ty: &CType, value: &ConstInit) -> bool {
    match value {
        ConstInit::Zero | ConstInit::Int(_) | ConstInit::Float(_) => true,
        // A null pointer and an integer converted to a pointer are bytes.
        // The address of an object needs a relocation.
        ConstInit::Ptr(PtrConst::Null | PtrConst::IntToPtr(_)) => true,
        ConstInit::Ptr(PtrConst::Addr { .. }) | ConstInit::PtrToInt { .. } => !ty.is_union(),
        // Bytes are an object representation. A relocation cannot hide in them.
        ConstInit::Bytes(_) => true,
        ConstInit::Aggregate(items) => {
            if ty.is_union() {
                return items.first().is_none_or(const_has_no_address);
            }
            if let Some(fields) = ty.struct_ref().and_then(StructRef::fields) {
                return fields
                    .iter()
                    .zip(items)
                    .all(|(field, item)| union_bytes_ok(&field.ty, item));
            }
            if let Some(elem) = ty.array_elem() {
                return items.iter().all(|item| union_bytes_ok(elem, item));
            }
            true
        }
    }
}

fn const_has_no_address(value: &ConstInit) -> bool {
    match value {
        ConstInit::Zero | ConstInit::Int(_) | ConstInit::Float(_) => true,
        ConstInit::Ptr(PtrConst::Null | PtrConst::IntToPtr(_)) => true,
        ConstInit::Ptr(PtrConst::Addr { .. }) | ConstInit::PtrToInt { .. } => false,
        ConstInit::Bytes(_) => true,
        ConstInit::Aggregate(items) => items.iter().all(const_has_no_address),
    }
}

/// Byte images of the plain unions nested in a static initializer.
///
/// Every member of a union shares its bytes, and the LLVM type of a union
/// is its first member. A store to another member (`{ .u.d = 2.5 }`) is
/// written into the union's object representation instead, the way a
/// top-level union is folded. A bit-field union is already a byte image.
struct UnionImages {
    /// Sorted and disjoint.
    ranges: Vec<(u64, u64)>,
    /// An image is made when the first store lands in its union.
    images: Vec<Option<Vec<u8>>>,
    bad: Option<(usize, usize)>,
}

impl UnionImages {
    fn new(ty: &CType) -> Self {
        let mut ranges = Vec::new();
        union_ranges(ty, 0, true, &mut ranges);
        let images = vec![None; ranges.len()];
        UnionImages {
            ranges,
            images,
            bad: None,
        }
    }

    /// Write a store that falls in a nested union. `false` means it is
    /// outside every union and is assembled as usual.
    fn absorb(&mut self, store: &ElemStore, value: &ConstInit) -> Result<bool, Diagnostic> {
        let index = self
            .ranges
            .partition_point(|(start, _)| *start <= store.offset);
        if index == 0 || store.offset >= self.ranges[index - 1].1 {
            return Ok(false);
        }
        let (start, end) = self.ranges[index - 1];
        let image = self.images[index - 1].get_or_insert_with(|| vec![0u8; (end - start) as usize]);
        let at = (store.offset - start) as usize;
        if !write_folded_bytes(&store.ty, value, image, at) && self.bad.is_none() {
            self.bad = Some((store.expr.line(), store.expr.column()));
        }
        Ok(true)
    }

    fn finish(
        self,
        name: &str,
        sema: &Sema,
        folded: &mut Vec<(u64, ConstInit)>,
    ) -> Result<(), Diagnostic> {
        if let Some((line, column)) = self.bad {
            return sema.err(
                line,
                column,
                format!("initializer of '{name}' uses an address in a union"),
            );
        }
        for ((start, _), image) in self.ranges.into_iter().zip(self.images) {
            if let Some(image) = image {
                folded.push((start, ConstInit::Bytes(image)));
            }
        }
        Ok(())
    }
}

/// Byte ranges of the outermost plain unions inside `ty`. `top` skips `ty`
/// itself, which the caller folds on its own.
fn union_ranges(ty: &CType, base: u64, top: bool, out: &mut Vec<(u64, u64)>) {
    if is_bit_image(ty) {
        return;
    }
    if ty.is_union() && !top {
        out.push((base, base + ty.size().unwrap_or(0)));
        return;
    }
    if let Some(fields) = ty.struct_ref().and_then(StructRef::fields) {
        for field in fields.iter().filter(|field| field.bit_width.is_none()) {
            union_ranges(&field.ty, base + field.offset, false, out);
        }
        return;
    }
    if let (Some(elem), Some(len)) = (ty.array_elem(), ty.array_len()) {
        let step = elem.size().unwrap_or(0);
        if elem.is_scalar() {
            return;
        }
        for index in 0..len {
            union_ranges(elem, base + index * step, false, out);
        }
    }
}

fn assemble(ty: &CType, stores: &[(u64, ConstInit)], base: u64) -> ConstInit {
    if ty.is_union() {
        if let Some((_, value)) = stores
            .iter()
            .rev()
            .find(|(offset, value)| *offset == base && matches!(value, ConstInit::Bytes(_)))
        {
            return value.clone();
        }
    }
    if is_bit_image(ty) {
        if let Some((_, value)) = stores
            .iter()
            .rev()
            .find(|(offset, value)| *offset == base && matches!(value, ConstInit::Bytes(_)))
        {
            return value.clone();
        }
        let size = ty.size().unwrap_or(0) as usize;
        return ConstInit::Bytes(vec![0u8; size]);
    }
    if ty.is_scalar() {
        return stores
            .iter()
            .rev()
            .find(|(offset, _)| *offset == base)
            .map(|(_, value)| value.clone())
            .unwrap_or_else(|| zero_const(ty));
    }
    if let Some(fields) = ty.struct_ref().and_then(StructRef::fields) {
        // A union's initializer describes the first member. The other members
        // occupy the same bytes; copying the store into each of them would
        // reinterpret those bits as a different type.
        let fields: Vec<_> = if ty.is_union() {
            fields.into_iter().take(1).collect()
        } else {
            fields
        };
        let mut items = Vec::new();
        for field in &fields {
            items.push(assemble(&field.ty, stores, base + field.offset));
        }
        return ConstInit::Aggregate(items);
    }
    let CType::Array(elem, Some(len)) = ty else {
        return ConstInit::Zero;
    };
    let elem_size = elem.size().unwrap_or(0);
    let mut items = Vec::new();
    for index in 0..*len {
        items.push(assemble(elem, stores, base + index * elem_size));
    }
    ConstInit::Aggregate(items)
}

/// A bit-field record, or a union that contains one in any member.
///
/// Union members share one byte image. A bit-field in a member other than
/// the first still occupies those bytes, so the whole union is one image.
/// Each record is examined once: two members of the same union are one type,
/// and searching both would walk that type's graph twice at every level.
fn is_bit_image(ty: &CType) -> bool {
    let mut seen = HashSet::new();
    is_bit_image_seen(ty, &mut seen)
}

fn is_bit_image_seen(ty: &CType, seen: &mut HashSet<u32>) -> bool {
    if ty.record_has_bitfield() {
        return true;
    }
    let Some(handle) = ty.struct_ref() else {
        return false;
    };
    if !handle.is_union() || !seen.insert(handle.id()) {
        return false;
    }
    handle.fields().is_some_and(|fields| {
        fields
            .iter()
            .any(|field| field.bit_width.is_some() || is_bit_image_seen(&field.ty, seen))
    })
}

/// Byte ranges of nested records whose LLVM type is a byte image.
fn bit_record_ranges(ty: &CType, base: u64, out: &mut Vec<(u64, u64)>) {
    if is_bit_image(ty) {
        out.push((base, base + ty.size().unwrap_or(0)));
        return;
    }
    if let Some(fields) = ty.struct_ref().and_then(StructRef::fields) {
        let fields: Vec<_> = if ty.is_union() {
            fields.into_iter().take(1).collect()
        } else {
            fields
        };
        for field in fields {
            if field.bit_width.is_some() {
                continue;
            }
            bit_record_ranges(&field.ty, base + field.offset, out);
        }
        return;
    }
    if let (Some(elem), Some(len)) = (ty.array_elem(), ty.array_len()) {
        let step = elem.size().unwrap_or(0);
        for index in 0..len {
            bit_record_ranges(elem, base + index * step, out);
        }
    }
}

/// Replace each nested bit-field record, including a union that contains
/// one in any member, with the bytes already packed into `image`.
///
/// C11 6.7.9 initializes the designated union member and leaves the rest of
/// that union's object representation zero. Every member occupies the same
/// bytes, so a bit-field in a member other than the first is still this image.
fn splice_bit_blobs(ty: &CType, base: u64, image: &[u8], out: &mut Vec<(u64, ConstInit)>) {
    if is_bit_image(ty) {
        let size = ty.size().unwrap_or(0) as usize;
        let start = base as usize;
        let mut bytes = vec![0u8; size];
        if start < image.len() {
            let end = (start + size).min(image.len());
            bytes[..end - start].copy_from_slice(&image[start..end]);
        }
        out.push((base, ConstInit::Bytes(bytes)));
        return;
    }
    if let Some(fields) = ty.struct_ref().and_then(StructRef::fields) {
        let fields: Vec<_> = if ty.is_union() {
            fields.into_iter().take(1).collect()
        } else {
            fields
        };
        for field in fields {
            if field.bit_width.is_some() {
                continue;
            }
            splice_bit_blobs(&field.ty, base + field.offset, image, out);
        }
        return;
    }
    if let (Some(elem), Some(len)) = (ty.array_elem(), ty.array_len()) {
        let step = elem.size().unwrap_or(0);
        for index in 0..len {
            splice_bit_blobs(elem, base + index * step, image, out);
        }
    }
}

fn ptr_as_int(ptr: PtrConst, dest: TypeName) -> ConstInit {
    match ptr {
        PtrConst::Null => ConstInit::Int(0),
        PtrConst::IntToPtr(bits) => ConstInit::Int(convert_bits(bits, TypeName::ULong, dest)),
        other => ConstInit::PtrToInt {
            ptr: other,
            width: dest.width(),
        },
    }
}

fn eval_ptr(expr: &Expr) -> Result<PtrConst, ConstError> {
    if is_null_pointer_constant(expr)
        && expr
            .ty()
            .is_some_and(|ty| ty.is_integer() || ty.is_pointer())
    {
        return Ok(PtrConst::Null);
    }
    match expr {
        Expr::Unary {
            op: UnaryOp::Addr,
            expr,
            ..
        } => {
            let (name, offset) = designator(expr)?;
            Ok(PtrConst::Addr { name, offset })
        }
        Expr::Decay { expr, .. } => {
            let (name, offset) = designator(expr)?;
            Ok(PtrConst::Addr { name, offset })
        }
        Expr::Bin {
            op: BinOp::Add,
            left,
            right,
            ..
        } => add_ptr_const(left, right, false),
        Expr::Bin {
            op: BinOp::Sub,
            left,
            right,
            ..
        } if left.ty().is_some_and(|ty| ty.is_pointer())
            && right.ty().is_some_and(|ty| ty.is_integer()) =>
        {
            add_ptr_const(left, right, true)
        }
        Expr::Cast { ty_name, expr, .. } if ty_name.is_pointer() => {
            if expr.ty().is_some_and(|ty| ty.is_pointer()) {
                eval_ptr(expr)
            } else {
                int_expr_to_ptr(expr)
            }
        }
        Expr::Cond {
            cond,
            then_expr,
            else_expr,
            ..
        } => {
            let cond = eval_const_expr(cond)?;
            let chosen = if is_zero(&cond) { else_expr } else { then_expr };
            eval_ptr(chosen)
        }
        _ => Err(ConstError::NotConst),
    }
}

fn add_ptr_const(left: &Expr, right: &Expr, negate: bool) -> Result<PtrConst, ConstError> {
    let (ptr_expr, int_expr) = if left.ty().is_some_and(|ty| ty.is_pointer()) {
        (left, right)
    } else {
        (right, left)
    };
    let base = eval_ptr(ptr_expr)?;
    let mut index = const_index(int_expr)?;
    if negate {
        index = index
            .checked_neg()
            .ok_or(ConstError::Overflow(TypeName::Long))?;
    }
    let size = ptr_expr
        .ty()
        .as_ref()
        .and_then(CType::pointee)
        .and_then(CType::size)
        .ok_or(ConstError::NotConst)? as i64;
    let delta = index
        .checked_mul(size)
        .ok_or(ConstError::Overflow(TypeName::Long))?;
    match base {
        PtrConst::Addr { name, offset } => Ok(PtrConst::Addr {
            name,
            offset: offset
                .checked_add(delta)
                .ok_or(ConstError::Overflow(TypeName::Long))?,
        }),
        PtrConst::Null => {
            if delta == 0 {
                Ok(PtrConst::Null)
            } else {
                Ok(PtrConst::IntToPtr(delta as u64))
            }
        }
        PtrConst::IntToPtr(bits) => Ok(PtrConst::IntToPtr(bits.wrapping_add(delta as u64))),
    }
}

/// An integer expression used as a pointer: a constant, or a cast of an
/// address through an integer. A 64-bit integer keeps every address bit.
/// A narrower integer keeps the low bits and extends them back the way the
/// integer type does (sign-extend signed, zero-extend unsigned).
fn int_expr_to_ptr(expr: &Expr) -> Result<PtrConst, ConstError> {
    if let Expr::Cast {
        ty_name,
        expr: inner,
        ..
    } = expr
    {
        if let Some(int_ty) = ty_name.as_integer() {
            if let Ok(ptr) = eval_ptr(inner) {
                return narrow_ptr(ptr, int_ty);
            }
        }
    }
    let value = eval_const_expr(expr)?;
    let bits = convert_bits(value.bits, value.ty, TypeName::ULong);
    if bits == 0 {
        Ok(PtrConst::Null)
    } else {
        Ok(PtrConst::IntToPtr(bits))
    }
}

fn narrow_ptr(ptr: PtrConst, int_ty: TypeName) -> Result<PtrConst, ConstError> {
    if int_ty.width() >= 64 {
        return Ok(ptr);
    }
    match ptr {
        PtrConst::Null => Ok(PtrConst::Null),
        PtrConst::IntToPtr(bits) => {
            let narrowed = convert_bits(bits, TypeName::ULong, int_ty);
            Ok(PtrConst::IntToPtr(convert_bits(
                narrowed,
                int_ty,
                TypeName::ULong,
            )))
        }
        // A file-scope address truncated to 32 bits is still a relocation.
        // Only the low bits survive, so it is not the original address constant.
        PtrConst::Addr { .. } => Err(ConstError::NotConst),
    }
}

fn const_index(expr: &Expr) -> Result<i64, ConstError> {
    let value = eval_const_expr(expr)?;
    if value.ty.is_signed() {
        Ok(sign_extend(value.bits, value.ty.width()))
    } else {
        let bits = value.bits & value.ty.mask();
        if bits > i64::MAX as u64 {
            Err(ConstError::Overflow(value.ty))
        } else {
            Ok(bits as i64)
        }
    }
}

fn designator(expr: &Expr) -> Result<(String, i64), ConstError> {
    match expr {
        Expr::Var { resolved, .. } => {
            let slot = &resolved.as_ref().ok_or(ConstError::NotConst)?.slot;
            let Some(name) = slot.strip_prefix('@') else {
                return Err(ConstError::NotConst);
            };
            Ok((name.to_string(), 0))
        }
        Expr::Str { symbol, .. } => {
            let name = symbol.clone().ok_or(ConstError::NotConst)?;
            Ok((name, 0))
        }
        Expr::Index {
            base, index, ty, ..
        } => {
            let elem = ty
                .as_ref()
                .and_then(CType::size)
                .ok_or(ConstError::NotConst)? as i64;
            let step = const_index(index)?;
            let (name, offset) = pointer_object(base)?;
            Ok((
                name,
                offset
                    .checked_add(
                        step.checked_mul(elem)
                            .ok_or(ConstError::Overflow(TypeName::Long))?,
                    )
                    .ok_or(ConstError::Overflow(TypeName::Long))?,
            ))
        }
        Expr::Unary {
            op: UnaryOp::Deref,
            expr,
            ..
        } => pointer_object(expr),
        Expr::Member {
            base,
            offset,
            arrow,
            ..
        } => {
            let member_off = i64::try_from(offset.ok_or(ConstError::NotConst)?)
                .map_err(|_| ConstError::Overflow(TypeName::Long))?;
            let (name, base_off) = if *arrow {
                pointer_object(base)?
            } else {
                designator(base)?
            };
            Ok((
                name,
                base_off
                    .checked_add(member_off)
                    .ok_or(ConstError::Overflow(TypeName::Long))?,
            ))
        }
        _ => Err(ConstError::NotConst),
    }
}

fn pointer_object(expr: &Expr) -> Result<(String, i64), ConstError> {
    match eval_ptr(expr)? {
        PtrConst::Addr { name, offset } => Ok((name, offset)),
        _ => Err(ConstError::NotConst),
    }
}

fn symbol_name(sym: &Symbol) -> &str {
    match sym {
        Symbol::Var(var) => &var.name,
        Symbol::Func(func) => &func.name,
    }
}

fn symbol_file_visible(sym: &Symbol) -> bool {
    match sym {
        Symbol::Var(var) => var.file_visible,
        Symbol::Func(func) => func.file_visible,
    }
}

fn incomplete_local(name: &str, ty: &CType) -> String {
    if ty.is_array() {
        format!("array size missing in '{name}'")
    } else {
        format!("variable '{name}' has incomplete type '{}'", ty.display())
    }
}

fn duplicate_param(func: &FuncDef) -> Option<&Param> {
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

impl FuncSym {
    fn func_ty(&self) -> FuncTy {
        FuncTy {
            ret: self.ret.clone(),
            params: self.params.clone(),
            variadic: self.variadic,
            prototype: self.prototype,
        }
    }

    /// Take the composite type with a compatible later declaration (C11
    /// 6.2.7p4): a prototype replaces a declaration that had none.
    fn absorb(&mut self, new: &FuncTy) {
        if !self.prototype && new.prototype {
            self.params = new.params.clone();
            self.variadic = new.variadic;
            self.prototype = true;
        }
    }
}

fn def_func_ty(func: &FuncDef, params: &[CType]) -> FuncTy {
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
fn function_types_match(existing: &FuncSym, new: &FuncTy, new_is_def: bool) -> bool {
    let old = existing.func_ty();
    if !compatible_funcs(&old, new) {
        return false;
    }
    if existing.defined && !old.prototype && new.prototype && !new.params.is_empty() {
        return false;
    }
    if new_is_def && !new.prototype && old.prototype && !old.params.is_empty() {
        return false;
    }
    true
}

enum ConstError {
    NotConst,
    DivZero,
    Overflow(TypeName),
    Shift,
}

struct CVal {
    bits: u64,
    ty: TypeName,
}

fn c_int(value: i32) -> CVal {
    CVal {
        bits: value as u32 as u64,
        ty: TypeName::Int,
    }
}

fn is_zero(value: &CVal) -> bool {
    value.bits & value.ty.mask() == 0
}

/// A constant value that may be an integer or a `float`/`double`.
enum NumConst {
    Int(CVal),
    Real {
        bits: u128,
        kind: crate::ast::FloatKind,
    },
}

impl NumConst {
    fn is_zero(&self) -> bool {
        match self {
            NumConst::Int(value) => is_zero(value),
            NumConst::Real { bits, kind } => crate::float::real_is_zero(*kind, *bits),
        }
    }

    fn real_kind(&self) -> Option<crate::ast::FloatKind> {
        match self {
            NumConst::Real { kind, .. } => Some(*kind),
            NumConst::Int(_) => None,
        }
    }

    fn to_real(&self, dest: crate::ast::FloatKind) -> Option<u128> {
        match self {
            NumConst::Int(value) => Some(crate::float::int_to_real(value.bits, value.ty, dest)),
            NumConst::Real { bits, kind } => Some(crate::float::real_to_real(*bits, *kind, dest)),
        }
    }

    fn to_int(&self, dest: TypeName) -> Result<CVal, ConstError> {
        match self {
            NumConst::Int(value) => Ok(CVal {
                bits: convert_bits(value.bits, value.ty, dest),
                ty: dest,
            }),
            NumConst::Real { bits, kind } => {
                let bits = crate::float::real_to_int(*bits, *kind, dest)
                    .ok_or(ConstError::Overflow(dest))?;
                Ok(CVal { bits, ty: dest })
            }
        }
    }
}

fn has_real(expr: &Expr) -> bool {
    match expr {
        Expr::Float { .. } => true,
        Expr::Int { .. } | Expr::Sizeof { .. } => false,
        Expr::Unary { op, expr, .. } => {
            !matches!(op, UnaryOp::Addr | UnaryOp::Deref) && has_real(expr)
        }
        Expr::Bin { left, right, .. } => has_real(left) || has_real(right),
        Expr::Cond {
            cond,
            then_expr,
            else_expr,
            ty,
            ..
        } => {
            ty.as_ref().is_some_and(|ty| ty.is_float())
                || has_real(cond)
                || has_real(then_expr)
                || has_real(else_expr)
        }
        Expr::Cast { ty_name, expr, .. } => ty_name.is_float() || has_real(expr),
        Expr::Call { callee, .. } => matches!(
            callee.as_ref(),
            Expr::Var { name, .. } if crate::float::FpFormatBuiltin::from_name(name).is_some()
        ),
        _ => false,
    }
}

fn eval_num(expr: &Expr) -> Result<NumConst, ConstError> {
    if !has_real(expr) {
        return Ok(NumConst::Int(eval_const_expr(expr)?));
    }
    match expr {
        Expr::Float { bits, kind, .. } => Ok(NumConst::Real {
            bits: *bits,
            kind: *kind,
        }),
        Expr::Int { .. } | Expr::Sizeof { .. } => Ok(NumConst::Int(eval_const_expr(expr)?)),
        Expr::Unary {
            op, expr: inner, ..
        } => eval_real_unary(*op, inner),
        Expr::Bin {
            op, left, right, ..
        } => eval_real_binop(*op, left, right),
        Expr::Cond {
            cond,
            then_expr,
            else_expr,
            ty,
            ..
        } => {
            let chosen = if eval_num(cond)?.is_zero() {
                else_expr
            } else {
                then_expr
            };
            let value = eval_num(chosen)?;
            let result = ty.as_ref().ok_or(ConstError::NotConst)?;
            if let Some(kind) = result.float_kind() {
                Ok(NumConst::Real {
                    bits: value.to_real(kind).ok_or(ConstError::NotConst)?,
                    kind,
                })
            } else if let Some(dest) = result.as_integer() {
                Ok(NumConst::Int(value.to_int(dest)?))
            } else {
                Err(ConstError::NotConst)
            }
        }
        Expr::Cast { ty_name, expr, .. } => {
            let value = eval_num(expr)?;
            if let Some(kind) = ty_name.float_kind() {
                Ok(NumConst::Real {
                    bits: value.to_real(kind).ok_or(ConstError::NotConst)?,
                    kind,
                })
            } else if let Some(dest) = ty_name.as_integer() {
                Ok(NumConst::Int(value.to_int(dest)?))
            } else {
                Err(ConstError::NotConst)
            }
        }
        Expr::Call { callee, args, .. } => eval_fp_format_builtin(callee, args),
        _ => Err(ConstError::NotConst),
    }
}

fn eval_fp_format_builtin(callee: &Expr, args: &[Expr]) -> Result<NumConst, ConstError> {
    let Expr::Var { name, .. } = callee else {
        return Err(ConstError::NotConst);
    };
    let Some(fp) = crate::float::FpFormatBuiltin::from_name(name) else {
        return Err(ConstError::NotConst);
    };
    let bits = match fp {
        crate::float::FpFormatBuiltin::Inf(kind) => crate::float::plus_inf(kind),
        crate::float::FpFormatBuiltin::Nan { kind, signaling } => {
            let tag = crate::float::nan_tag_from_expr(&args[0]).ok_or(ConstError::NotConst)?;
            crate::float::nan_bits(kind, tag, signaling)
        }
    };
    Ok(NumConst::Real {
        bits,
        kind: fp.kind(),
    })
}

fn eval_real_unary(op: UnaryOp, expr: &Expr) -> Result<NumConst, ConstError> {
    if matches!(op, UnaryOp::Addr | UnaryOp::Deref) {
        return Err(ConstError::NotConst);
    }
    let value = eval_num(expr)?;
    match op {
        UnaryOp::Plus => match value {
            NumConst::Int(value) => Ok(NumConst::Int(promote_val(value))),
            real => Ok(real),
        },
        UnaryOp::Minus => match value {
            NumConst::Int(value) => Ok(NumConst::Int(negate(promote_val(value))?)),
            NumConst::Real { bits, kind } => Ok(NumConst::Real {
                bits: crate::float::negate(kind, bits),
                kind,
            }),
        },
        UnaryOp::Not => Ok(NumConst::Int(c_int(i32::from(value.is_zero())))),
        UnaryOp::BitNot => {
            let NumConst::Int(value) = value else {
                return Err(ConstError::NotConst);
            };
            let value = promote_val(value);
            Ok(NumConst::Int(CVal {
                bits: (!value.bits) & value.ty.mask(),
                ty: value.ty,
            }))
        }
        UnaryOp::Addr | UnaryOp::Deref => Err(ConstError::NotConst),
    }
}

fn eval_real_binop(op: BinOp, left: &Expr, right: &Expr) -> Result<NumConst, ConstError> {
    match op {
        BinOp::And => {
            if eval_num(left)?.is_zero() {
                return Ok(NumConst::Int(c_int(0)));
            }
            Ok(NumConst::Int(c_int(i32::from(!eval_num(right)?.is_zero()))))
        }
        BinOp::Or => {
            if !eval_num(left)?.is_zero() {
                return Ok(NumConst::Int(c_int(1)));
            }
            Ok(NumConst::Int(c_int(i32::from(!eval_num(right)?.is_zero()))))
        }
        _ => {
            let left = eval_num(left)?;
            let right = eval_num(right)?;
            if left.real_kind().is_some() || right.real_kind().is_some() {
                let kind = crate::float::usual_real(left.real_kind(), right.real_kind());
                let left_bits = left.to_real(kind).ok_or(ConstError::NotConst)?;
                let right_bits = right.to_real(kind).ok_or(ConstError::NotConst)?;
                if op.is_cmp() {
                    let cmp = match op {
                        BinOp::Eq => crate::float::FloatCmp::Eq,
                        BinOp::Ne => crate::float::FloatCmp::Ne,
                        BinOp::Lt => crate::float::FloatCmp::Lt,
                        BinOp::Gt => crate::float::FloatCmp::Gt,
                        BinOp::Le => crate::float::FloatCmp::Le,
                        BinOp::Ge => crate::float::FloatCmp::Ge,
                        _ => return Err(ConstError::NotConst),
                    };
                    return Ok(NumConst::Int(c_int(i32::from(crate::float::apply_cmp(
                        cmp, kind, left_bits, right_bits,
                    )))));
                }
                let flop = match op {
                    BinOp::Add => crate::float::FloatOp::Add,
                    BinOp::Sub => crate::float::FloatOp::Sub,
                    BinOp::Mul => crate::float::FloatOp::Mul,
                    BinOp::Div => crate::float::FloatOp::Div,
                    _ => return Err(ConstError::NotConst),
                };
                return Ok(NumConst::Real {
                    bits: crate::float::apply_bin(flop, kind, left_bits, right_bits),
                    kind,
                });
            }
            let NumConst::Int(left) = left else {
                return Err(ConstError::NotConst);
            };
            let NumConst::Int(right) = right else {
                return Err(ConstError::NotConst);
            };
            if matches!(op, BinOp::Shl | BinOp::Shr) {
                return Ok(NumConst::Int(eval_shift(op, left, right)?));
            }
            let ty = left.ty.usual(right.ty);
            let left = convert_bits(left.bits, left.ty, ty);
            let right = convert_bits(right.bits, right.ty, ty);
            if op.is_cmp() {
                return Ok(NumConst::Int(c_int(i32::from(compare(
                    op, left, right, ty,
                )))));
            }
            Ok(NumConst::Int(arith(op, left, right, ty)?))
        }
    }
}

/// Integer constant expression (C11 6.6).
///
/// `&&`, `||`, and `?:` skip the operand that is not evaluated.
/// `sizeof` is not evaluated. Its result is an `unsigned long`.
/// Signed arithmetic that does not fit in the result type is rejected,
/// including the minimum signed value divided or remaindered by `-1`.
/// Unsigned arithmetic wraps. A shift count that is negative or not less
/// than the width of the promoted left operand is rejected. A signed left
/// shift of a negative value, or one whose result does not fit, is rejected.
/// Unsigned left shifts wrap. Signed right shifts are arithmetic; unsigned
/// right shifts are logical.
/// Assignment, `++`/`--`, calls, and comma are not constant expressions when
/// they are evaluated.
fn eval_const_expr(expr: &Expr) -> Result<CVal, ConstError> {
    match expr {
        Expr::Int { bits, value_ty, .. } => Ok(CVal {
            bits: *bits & value_ty.mask(),
            ty: *value_ty,
        }),
        Expr::Float { .. } => Err(ConstError::NotConst),
        Expr::Sizeof { operand, .. } => {
            let size = match operand {
                SizeofOperand::Type(ty) => ty.size(),
                SizeofOperand::Expr(inner) => inner.ty().expect("sizeof operand is typed").size(),
            };
            Ok(CVal {
                bits: size.ok_or(ConstError::NotConst)?,
                ty: TypeName::ULong,
            })
        }
        Expr::Unary { op, expr, .. } => eval_unary(*op, expr),
        Expr::Bin {
            op, left, right, ..
        } => eval_binop(*op, left, right),
        Expr::Cond {
            cond,
            then_expr,
            else_expr,
            ty,
            ..
        } => {
            // The chosen arm is converted to the conditional's result type,
            // the same conversion the IR does before the phi.
            let chosen = if is_zero(&eval_const_expr(cond)?) {
                else_expr
            } else {
                then_expr
            };
            let value = eval_const_expr(chosen)?;
            let result = ty
                .as_ref()
                .and_then(CType::as_integer)
                .ok_or(ConstError::NotConst)?;
            Ok(CVal {
                bits: convert_bits(value.bits, value.ty, result),
                ty: result,
            })
        }
        Expr::Cast { ty_name, expr, .. } => {
            let Some(dest) = ty_name.as_integer() else {
                return Err(ConstError::NotConst);
            };
            // C11 6.6: a floating constant may appear in an integer constant
            // expression only as the immediate operand of a cast. `(int)1.5`
            // is folded; a bare `1.5` stays non-constant.
            if let Expr::Float { bits, kind, .. } = expr.as_ref() {
                let bits = crate::float::real_to_int(*bits, *kind, dest)
                    .ok_or(ConstError::Overflow(dest))?;
                return Ok(CVal { bits, ty: dest });
            }
            let value = eval_const_expr(expr)?;
            Ok(CVal {
                bits: convert_bits(value.bits, value.ty, dest),
                ty: dest,
            })
        }
        Expr::Var { .. }
        | Expr::Assign { .. }
        | Expr::CompoundAssign { .. }
        | Expr::IncDec { .. }
        | Expr::Call { .. }
        | Expr::Comma { .. }
        | Expr::Index { .. }
        | Expr::Decay { .. }
        | Expr::Str { .. }
        | Expr::Member { .. }
        | Expr::VaArg { .. }
        | Expr::VaOp { .. }
        | Expr::Generic { .. }
        | Expr::CompoundLit { .. } => Err(ConstError::NotConst),
    }
}

fn eval_unary(op: UnaryOp, expr: &Expr) -> Result<CVal, ConstError> {
    if matches!(op, UnaryOp::Addr | UnaryOp::Deref) {
        return Err(ConstError::NotConst);
    }
    let value = promote_val(eval_const_expr(expr)?);
    match op {
        UnaryOp::Plus => Ok(value),
        UnaryOp::Minus => negate(value),
        UnaryOp::Not => Ok(c_int(i32::from(is_zero(&value)))),
        UnaryOp::BitNot => Ok(CVal {
            bits: (!value.bits) & value.ty.mask(),
            ty: value.ty,
        }),
        UnaryOp::Addr | UnaryOp::Deref => Err(ConstError::NotConst),
    }
}

fn promote_val(value: CVal) -> CVal {
    let ty = value.ty.promote();
    CVal {
        bits: convert_bits(value.bits, value.ty, ty),
        ty,
    }
}

fn negate(value: CVal) -> Result<CVal, ConstError> {
    if value.ty.is_signed() {
        let signed = sign_extend(value.bits, value.ty.width()) as i128;
        bits_from_signed(signed.wrapping_neg(), value.ty)
    } else {
        Ok(CVal {
            bits: value.bits.wrapping_neg() & value.ty.mask(),
            ty: value.ty,
        })
    }
}

fn eval_binop(op: BinOp, left: &Expr, right: &Expr) -> Result<CVal, ConstError> {
    match op {
        BinOp::And => {
            if is_zero(&eval_const_expr(left)?) {
                return Ok(c_int(0));
            }
            Ok(c_int(i32::from(!is_zero(&eval_const_expr(right)?))))
        }
        BinOp::Or => {
            if !is_zero(&eval_const_expr(left)?) {
                return Ok(c_int(1));
            }
            Ok(c_int(i32::from(!is_zero(&eval_const_expr(right)?))))
        }
        _ => {
            let left = eval_const_expr(left)?;
            let right = eval_const_expr(right)?;
            if matches!(op, BinOp::Shl | BinOp::Shr) {
                return eval_shift(op, left, right);
            }
            let ty = left.ty.usual(right.ty);
            let left = convert_bits(left.bits, left.ty, ty);
            let right = convert_bits(right.bits, right.ty, ty);
            if op.is_cmp() {
                return Ok(c_int(i32::from(compare(op, left, right, ty))));
            }
            arith(op, left, right, ty)
        }
    }
}

fn compare(op: BinOp, left: u64, right: u64, ty: TypeName) -> bool {
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
            _ => unreachable!("comparison"),
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
            _ => unreachable!("comparison"),
        }
    }
}

fn arith(op: BinOp, left: u64, right: u64, ty: TypeName) -> Result<CVal, ConstError> {
    if ty.is_signed() {
        let left = sign_extend(left, ty.width()) as i128;
        let right = sign_extend(right, ty.width()) as i128;
        let value = match op {
            BinOp::Add => left.checked_add(right),
            BinOp::Sub => left.checked_sub(right),
            BinOp::Mul => left.checked_mul(right),
            BinOp::Div => return div_signed(left, right, ty),
            BinOp::Mod => return rem_signed(left, right, ty),
            BinOp::BitAnd => Some(left & right),
            BinOp::BitOr => Some(left | right),
            BinOp::BitXor => Some(left ^ right),
            _ => unreachable!("arithmetic operator"),
        };
        let Some(value) = value else {
            return Err(ConstError::Overflow(ty));
        };
        bits_from_signed(value, ty)
    } else {
        let mask = ty.mask();
        let left = left & mask;
        let right = right & mask;
        let bits = match op {
            BinOp::Add => left.wrapping_add(right),
            BinOp::Sub => left.wrapping_sub(right),
            BinOp::Mul => left.wrapping_mul(right),
            BinOp::Div => {
                if right == 0 {
                    return Err(ConstError::DivZero);
                }
                left / right
            }
            BinOp::Mod => {
                if right == 0 {
                    return Err(ConstError::DivZero);
                }
                left % right
            }
            BinOp::BitAnd => left & right,
            BinOp::BitOr => left | right,
            BinOp::BitXor => left ^ right,
            _ => unreachable!("arithmetic operator"),
        };
        Ok(CVal {
            bits: bits & mask,
            ty,
        })
    }
}

fn bits_from_signed(value: i128, ty: TypeName) -> Result<CVal, ConstError> {
    let width = ty.width();
    let min = -((1i128) << (width - 1));
    let max = ((1i128) << (width - 1)) - 1;
    if value < min || value > max {
        return Err(ConstError::Overflow(ty));
    }
    Ok(CVal {
        bits: (value as u64) & ty.mask(),
        ty,
    })
}

fn signed_min(ty: TypeName) -> i128 {
    -((1i128) << (ty.width() - 1))
}

/// `MIN / -1` and `MIN % -1` are not representable (C11 6.5.5).
fn div_overflows(left: i128, right: i128, ty: TypeName) -> bool {
    left == signed_min(ty) && right == -1
}

fn div_signed(left: i128, right: i128, ty: TypeName) -> Result<CVal, ConstError> {
    if right == 0 {
        return Err(ConstError::DivZero);
    }
    if div_overflows(left, right, ty) {
        return Err(ConstError::Overflow(ty));
    }
    bits_from_signed(left / right, ty)
}

fn rem_signed(left: i128, right: i128, ty: TypeName) -> Result<CVal, ConstError> {
    if right == 0 {
        return Err(ConstError::DivZero);
    }
    if div_overflows(left, right, ty) {
        return Err(ConstError::Overflow(ty));
    }
    bits_from_signed(left % right, ty)
}

/// The count is the promoted right operand. It must be nonnegative and less
/// than the width of the promoted left operand. Signed left shifts must not
/// lose the sign or any magnitude bit. Unsigned shifts wrap.
fn eval_shift(op: BinOp, left: CVal, right: CVal) -> Result<CVal, ConstError> {
    let left = promote_val(left);
    let right = promote_val(right);
    let count = shift_count(&right, left.ty.width())?;
    let width = left.ty.width();
    if op == BinOp::Shr {
        let bits = if left.ty.is_signed() {
            let shifted = sign_extend(left.bits, width) >> count;
            shifted as u64 & left.ty.mask()
        } else {
            (left.bits & left.ty.mask()) >> count
        };
        return Ok(CVal { bits, ty: left.ty });
    }
    if left.ty.is_signed() {
        let signed = sign_extend(left.bits, width) as i128;
        if signed < 0 {
            return Err(ConstError::Shift);
        }
        bits_from_signed(signed << count, left.ty)
    } else {
        Ok(CVal {
            bits: ((left.bits & left.ty.mask()) << count) & left.ty.mask(),
            ty: left.ty,
        })
    }
}

fn shift_count(count: &CVal, width: u32) -> Result<u32, ConstError> {
    if count.ty.is_signed() {
        let signed = sign_extend(count.bits, count.ty.width());
        if signed < 0 || signed >= i64::from(width) {
            return Err(ConstError::Shift);
        }
        Ok(signed as u32)
    } else {
        let unsigned = count.bits & count.ty.mask();
        if unsigned >= u64::from(width) {
            return Err(ConstError::Shift);
        }
        Ok(unsigned as u32)
    }
}

fn assign_slots(func: &mut FuncDef) {
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

fn number_compound(compound: &mut Compound, next: &mut u32) {
    for item in &mut compound.items {
        number_stmt(item, next);
    }
}

fn number_stmt(stmt: &mut Stmt, next: &mut u32) {
    match stmt {
        Stmt::Decl(decl) => {
            // `static` and `extern` are globals. Their slots are the LLVM
            // symbol, assigned while checking the declaration.
            if decl.storage == Storage::None {
                *next += 1;
                decl.slot = format!("%v{next}");
            }
        }
        Stmt::Compound(compound) => number_compound(compound, next),
        Stmt::If {
            then_stmt,
            else_stmt,
            ..
        } => {
            number_stmt(then_stmt, next);
            if let Some(else_stmt) = else_stmt {
                number_stmt(else_stmt, next);
            }
        }
        Stmt::While { body, .. } | Stmt::Do { body, .. } => number_stmt(body, next),
        Stmt::Switch { body, .. }
        | Stmt::Case { stmt: body, .. }
        | Stmt::Default { stmt: body, .. } => {
            number_stmt(body, next);
        }
        Stmt::For { init, body, .. } => {
            if let ForInit::Decl(decls) = init {
                for decl in decls {
                    *next += 1;
                    decl.slot = format!("%v{next}");
                }
            }
            number_stmt(body, next);
        }
        Stmt::Label { stmt, .. } => number_stmt(stmt, next),
        Stmt::FuncDecl(_)
        | Stmt::Return { .. }
        | Stmt::Expr { .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Goto { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;

    fn sema_err(source: &str) -> String {
        let tokens = lex(source, "neg.c").unwrap();
        let mut unit = match parse(&tokens, "neg.c") {
            Ok(unit) => unit,
            Err(err) => return err.to_string(),
        };
        analyze(&mut unit, "neg.c").unwrap_err().to_string()
    }

    fn sema_ok(source: &str) {
        let tokens = lex(source, "ok.c").unwrap();
        let mut unit = parse(&tokens, "ok.c").unwrap();
        analyze(&mut unit, "ok.c").unwrap();
    }

    #[test]
    fn designated_aggregate_members_elide_braces() {
        // C11 6.7.9p17, p20: `.m = 1, 2` starts at the first scalar of `m`.
        sema_ok("struct I { int a, b; }; struct O { struct I in; int z; } o = { .in = 1, 2, 3 };");
        sema_ok("struct I { int a, b; }; union V { int i; struct I in; } v = { .in = 1, 2 };");
        sema_ok("struct I { int a, b; }; struct I a[] = { [1] = 3, 4, 5 };");
        sema_ok("union V { int i; double d; }; struct O { int k; union V v; } o = { .v.d = 2.5 };");
        assert!(sema_err(
            "struct I { int a, b; }; union V { int i; struct I in; } v = { .in = 1, 2, 3 };"
        )
        .contains("excess elements in initializer"));
        assert!(sema_err(
            "struct I { int a, b; }; struct O { struct I in; } o = { .in = 1, 2, 3 };"
        )
        .contains("excess elements in initializer"));
        assert!(sema_err(
            "struct I { int a, b; }; struct O { struct I in; int z; } o = { .in = 1, 2, .nope = 3 };"
        )
        .contains("no member named 'nope'"));
        assert!(sema_err(
            "int x; union U { int i; int *p; }; struct S { union U u; } s = { .u.p = &x };"
        )
        .contains("uses an address in a union"));
    }

    #[test]
    fn generic_selection_and_compound_literals() {
        sema_ok("int main(void) { const int c = 1; return _Generic(c, int: 0, default: 1); }");
        sema_ok("int main(void) { int x; _Generic(x, int: x) = 1; return x; }");
        sema_ok("int *p = (int[]){1, 2}; struct P { int x; } *q = &(struct P){1};");
        sema_ok("int main(void) { int *p = &(int){1}; (int){2} = 3; return *p; }");
        assert!(sema_err("int a = _Generic(1, long: 1);")
            .contains("not compatible with any generic association type"));
        assert!(sema_err("int a = _Generic(1, int: 1, signed int: 2);")
            .contains("compatible with previously specified type"));
        assert!(sema_err("int a = _Generic(1, default: 1, default: 2);")
            .contains("duplicate default generic association"));
        assert!(sema_err("int a = _Generic(1, void: 1, default: 2);")
            .contains("not a complete object type"));
        assert!(sema_err("int x; int *p = (int[]){x};").contains("not a constant expression"));
        assert!(
            sema_err("int main(void) { static int *q = &(int){2}; return 0; }")
                .contains("not a constant expression")
        );
        assert!(sema_err("struct Q; int a = sizeof((struct Q){0});")
            .contains("compound literal has incomplete type"));
        assert!(
            sema_err("int main(void) { int (*f)(void) = &(int(void)){0}; }").contains("function")
        );
    }

    #[test]
    fn generic_unselected_associations_are_checked() {
        // C11 6.5.1.1p3 does not evaluate unselected associations, but they
        // are still assignment-expressions and must satisfy constraints and
        // name resolution (5.1.1.3).
        assert!(sema_err(
            "int main(void) { int n = 2; return _Generic(n, int: 0, default: undeclared_name); }"
        )
        .contains("undeclared identifier 'undeclared_name'"));
        assert!(sema_err(
            "int one(int); int main(void) { int n = 2; return _Generic(n, int: 0, default: one(1, 2)); }"
        )
        .contains("expects 1 argument, found 2"));
        // Nested selection: the outer default is unselected, and its inner
        // default names an undeclared identifier.
        assert!(sema_err(
            "int main(void) { int a = 1, b = 2; return _Generic(a, int: 0, default: _Generic(b, long: 1, default: missing_inner)); }"
        )
        .contains("undeclared identifier 'missing_inner'"));

        // Valid unselected expressions may have a different type from the
        // selected one.
        sema_ok("int main(void) { int n = 1; return _Generic(n, int: 0, default: 3.0); }");
        sema_ok("int main(void) { int n = 1; const char *p = _Generic(n, int: \"ok\", default: 0); return p != 0; }");
        sema_ok("struct S { int m; }; int main(void) { int n = 1; struct S s; return _Generic(n, int: 0, struct S: s.m); }");

        // Unselected side effects are not lowered to IR (C11 6.5.1.1p3).
        let ir = crate::compile_to_ir(
            "int bump(void); int main(void) { int n = 0; return _Generic(n, int: 1, default: bump()); }",
            "generic-unselected.c",
        )
        .unwrap();
        assert!(
            !ir.contains("call") && ir.contains("ret i32 1"),
            "unselected call must not be emitted: {ir}"
        );
        let ir = crate::compile_to_ir(
            "int printf(const char *, ...); int main(void) { int n = 0; return _Generic(n, int: 1, default: printf(\"x\")); }",
            "generic-unselected-printf.c",
        )
        .unwrap();
        assert!(
            !ir.contains("call") && ir.contains("ret i32 1"),
            "unselected printf must not be emitted: {ir}"
        );

        // The parser folds `_Generic` in an integer constant expression
        // before semantic analysis, so these contexts must be checked there.
        assert!(
            sema_err("enum { N = _Generic(0, int: 1, default: missing) };")
                .contains("undeclared identifier")
        );
        assert!(sema_err("int a[_Generic(1, int: 2, default: missing)];")
            .contains("undeclared identifier"));
        assert!(
            sema_err("struct S { int x : _Generic(0, int: 3, default: missing); };")
                .contains("undeclared identifier")
        );
        assert!(
            sema_err("_Static_assert(_Generic(0, int: 1, default: missing), \"m\");")
                .contains("undeclared identifier")
        );
        assert!(
            sema_err("int one(int); enum { N = _Generic(0, int: 1, default: one(1, 2)) };")
                .contains("argument")
        );
        sema_ok("enum { N = _Generic(0, int: 1, default: 2) };");
        sema_ok("int a[_Generic(1, int: 2, default: sizeof(int))];");
        sema_ok("int one(int); enum { N = _Generic(0, int: 1, default: one(1)) };");
        sema_ok("_Static_assert(_Generic(0, int: 1, default: 0), \"m\");");

        // Constant expressions that short-circuit, or that would be folded
        // away, still need their unselected associations checked.
        assert!(
            sema_err("enum { N = 1 || _Generic(0, int: 1, default: missing) };")
                .contains("undeclared identifier")
        );
        assert!(sema_err("enum { N = _Generic(0, int: 1, int: 2) };")
            .contains("compatible with a previously specified type"));
        assert!(
            sema_err("enum { N = _Generic(0, int: 1, default: *1) };").contains("invalid operand")
        );
        sema_ok("enum { N = _Generic(0, int: 1, default: __builtin_signbit(1.0)) };");
        assert!(
            sema_err("enum { N = _Generic(0, int: 1, default: (int[1]){[missing] = 0}) };",)
                .contains("undeclared identifier")
        );

        // __func__ is implicit only in a function (C11 6.4.2.2). A bare
        // supported builtin is not an ordinary identifier.
        assert!(
            sema_err("enum { N = _Generic(0, int: 1, default: __func__) };")
                .contains("undeclared identifier")
        );
        sema_ok("int f(void) { enum { N = _Generic(0, int: 1, default: __func__) }; return 0; }");
        assert!(
            sema_err("enum { N = _Generic(0, int: 1, default: __builtin_signbit) };")
                .contains("undeclared identifier")
        );
        sema_ok("int x; enum { N = _Generic(0, int: 1, default: ++_Generic(0, int: x)) };");
        assert!(sema_err(
            "int y; enum { N = _Generic(0, int: 1, default: ++_Generic(0, int: 1)) };"
        )
        .contains("invalid operand"));

        // C11 6.4.2.2: __func__ is `static const char [strlen(name)+1]`.
        // An ICE _Generic on its address must use that complete type.
        let ir = crate::compile_to_ir(
            "int f(void) { enum { N = _Generic(&__func__, char (*)[99]: 1, default: 2) }; return N; }",
            "generic-func-wrong.c",
        )
        .unwrap();
        assert!(
            ir.contains("ret i32 2"),
            "char (*)[99] must not match const char (*)[2]: {ir}"
        );
        let ir = crate::compile_to_ir(
            "int f(void) { enum { N = _Generic(&__func__, const char (*)[2]: 1, default: 2) }; return N; }",
            "generic-func-right.c",
        )
        .unwrap();
        assert!(
            ir.contains("ret i32 1"),
            "&__func__ in f must match const char (*)[2]: {ir}"
        );

        // Designator expressions must be typed before they are folded, so
        // `sizeof` in a designator is valid (selected or unselected).
        let ir = crate::compile_to_ir(
            "int main(void) { return (int[1]){[sizeof 0 - sizeof 0] = 7}[0]; }",
            "sizeof-designator.c",
        )
        .unwrap();
        assert!(
            ir.contains("store i32 7"),
            "sizeof in a selected designator must fold: {ir}"
        );
        let ir = crate::compile_to_ir(
            "int main(void) { return _Generic(0, int: 3, default: (int[1]){[sizeof 0 - sizeof 0] = 0}); }",
            "generic-sizeof-designator.c",
        )
        .unwrap();
        assert!(
            ir.contains("ret i32 3"),
            "unselected sizeof designator must not be evaluated: {ir}"
        );

        // ICE and runtime _Generic must agree on the const array type.
        let ir = crate::compile_to_ir(
            "int f(void) { enum { N = _Generic(&__func__, const char (*)[2]: 1, default: 0) }; return N != _Generic(&__func__, const char (*)[2]: 1, default: 0); }",
            "generic-func-agree.c",
        )
        .unwrap();
        assert!(
            !ir.contains("icmp ne i32 1, 0"),
            "ICE and runtime _Generic(&__func__) must agree: {ir}"
        );

        // The declared object is in scope in its initializer (C11 6.2.1p7).
        let ir = crate::compile_to_ir(
            "int main(void) { int a[1] = { [0 && sizeof a] = 7 }; return a[0]; }",
            "designator-self.c",
        )
        .unwrap();
        assert!(
            ir.contains("store i32 7"),
            "designator may name the object being initialized: {ir}"
        );
        let ir = crate::compile_to_ir(
            "int a[1] = { [0 && sizeof a] = 7 }; int main(void) { return a[0]; }",
            "designator-self-global.c",
        )
        .unwrap();
        assert!(
            ir.contains("store i32 7") || ir.contains("[1 x i32] [i32 7]"),
            "file-scope designator may name the object: {ir}"
        );
        // A block-scope extern may already have created the file symbol
        // and left it hidden. The later file-scope declarator still puts
        // the identifier in scope in its initializer (C11 6.2.1p7).
        let ir = crate::compile_to_ir(
            "int f(void) { extern int a[1]; return 0; } int a[1] = { [0 && sizeof a] = 7 }; int main(void) { return a[0]; }",
            "designator-self-global-after-extern.c",
        )
        .unwrap();
        assert!(
            ir.contains("store i32 7") || ir.contains("[1 x i32] [i32 7]"),
            "file-scope designator must see a previously hidden extern: {ir}"
        );
        // C11 6.2.7p4 forms a composite only when a prior declaration is
        // visible. After a block-scope `extern int a[1];`, a later
        // file-scope `int a[]` is visible in its initializer as `int[]`,
        // so `sizeof a` is a diagnostic (6.5.3.4p1). Clang `-std=c11`
        // rejects this the same way. The same designator is valid when
        // the declarator itself is complete (`int a[1]` above).
        assert!(sema_err(
            "int f(void) { extern int a[1]; return 0; } int a[] = { [0 && sizeof a] = 7 };"
        )
        .contains("sizeof applied to incomplete type"));
        // Plain `int a[]` with no prior declaration already diagnoses this.
        assert!(sema_err("int a[] = { [0 && sizeof a] = 7 };")
            .contains("sizeof applied to incomplete type"));
        // The initializer may complete the incomplete declarator. A hidden
        // `int[1]` does not shrink or reject a longer initializer, and a
        // hidden `int[3]` does not enlarge a shorter one.
        let ir = crate::compile_to_ir(
            "int f(void) { extern int a[1]; return 0; } int a[] = {7}; int main(void) { return a[0]; }",
            "hidden-extern-incomplete-init.c",
        )
        .unwrap();
        assert!(
            ir.contains("[1 x i32] [i32 7]"),
            "incomplete array is completed by its initializer: {ir}"
        );
        let ir = crate::compile_to_ir(
            "int f(void) { extern int a[1]; return 0; } int a[] = {1, 2, 3}; int main(void) { return a[2]; }",
            "hidden-extern-longer-init.c",
        )
        .unwrap();
        assert!(
            ir.contains("[3 x i32]"),
            "hidden int[1] must not bound a later int a[] initializer: {ir}"
        );
        let ir = crate::compile_to_ir(
            "int f(void) { extern int a[3]; return 0; } int a[] = {1, 2}; int main(void) { return a[1]; }",
            "hidden-extern-shorter-init.c",
        )
        .unwrap();
        assert!(
            ir.contains("[2 x i32]"),
            "hidden int[3] must not complete a later int a[] initializer: {ir}"
        );

        // An unselected file-scope compound literal must not leave a global.
        let ir = crate::compile_to_ir(
            "int x = _Generic(0, int: 1, default: (int){2});",
            "generic-unselected-compound.c",
        )
        .unwrap();
        assert!(
            !ir.contains("compoundliteral") && ir.contains("@x = global i32 1"),
            "unselected compound literal must not be emitted: {ir}"
        );
    }

    #[test]
    fn non_prototype_declarations() {
        // C11 6.7.6.3p15: `int g();` is compatible with a prototype only if
        // no parameter type changes under the default argument promotions
        // and there is no ellipsis.
        sema_ok("int g(); int g(int, double); int main(void) { return g(1, 2.0f); }");
        sema_ok("int g(); int main(void) { return g('a', (short)1, 1.0f); }");
        // C11 6.7.6.3p15 makes a parameter-count mismatch with a `()`
        // definition incompatible. Clang accepts the conversion without a
        // diagnostic, and this compiler has no warnings, so it follows Clang
        // for pointers. A redeclaration with parameters is still rejected.
        sema_ok("int h() { return 1; } int (*p)(int) = h;");
        sema_ok("int d(); int (*q)(int) = d;");
        assert!(sema_err("int h() { return 1; } int h(int);").contains("conflicting types"));
        sema_ok("int k(int); int k(); int main(void) { return k(3); }");
        assert!(sema_err("int g(); int g(char);").contains("conflicting types for 'g'"));
        assert!(sema_err("int g(); int g(float);").contains("conflicting types for 'g'"));
        assert!(sema_err("int g(); int g(int, ...);").contains("conflicting types for 'g'"));
        assert!(sema_err("int g() { return 0; } int g(int);").contains("conflicting types"));
        assert!(sema_err("int g(); long g();").contains("conflicting types for 'g'"));
        // A prototype still checks its arguments after a non-prototype
        // declaration.
        assert!(
            sema_err("int k(); int k(int); int main(void) { return k(); }")
                .contains("expects 1 argument")
        );
    }

    #[test]
    fn rejects_invalid_programs() {
        assert!(sema_err("int main(void) { return x; }").contains("undeclared identifier 'x'"));
        assert!(
            sema_err("int main(void) { int x; int x; return 0; }").contains("redefinition of 'x'")
        );
        assert!(
            sema_err("int f(int x) { int x; return x; } int main(void) { return f(1); }")
                .contains("redefinition of 'x'")
        );
        assert!(
            sema_err("int f(int a, int a) { return a; }").contains("redefinition of parameter 'a'")
        );
        assert!(
            sema_err("int f(void) { return 1; } int f(void) { return 2; }")
                .contains("redefinition of 'f'")
        );
        assert!(
            sema_err("int f(int a) { return a; } int main(void) { return f(1, 2); }")
                .contains("expects 1 argument, found 2")
        );
        assert!(
            sema_err("int main(void) { return foo(1); }").contains("undeclared identifier 'foo'")
        );
        assert!(sema_err("int main(void) { int x; return x(1); }")
            .contains("called object of type 'int' is not a function"));
        assert!(
            sema_err("int f(void) { return 1; } int main(void) { return f; }")
                .contains("incompatible types")
        );
        assert!(sema_err("int main(void) { break; }")
            .contains("'break' is not inside a loop or switch"));
        assert!(
            sema_err("int main(void) { continue; }").contains("'continue' is not inside a loop")
        );
        assert!(
            sema_err("void f(void) { return 1; } int main(void) { return 0; }")
                .contains("return with a value in a function returning 'void'")
        );
        assert!(sema_err("int main(void) { return; }")
            .contains("return with no value in a function returning 'int'"));
        assert!(sema_err("int main(void) { 1 = 2; }").contains("not assignable"));
        assert!(
            sema_err("void f(void) { return; } int main(void) { return f(); }")
                .contains("void value used where a value is required")
        );
        assert!(sema_err(
            "int id(int x) { return x; } int main(void) { int id = 5; return id(1); }"
        )
        .contains("is not a function"));
        assert!(
            sema_err("int add(int a) { return a; } int main(void) { return add(1, 2)(3); }")
                .contains("expects 1 argument")
        );
        assert!(sema_err("int f(int a); void f(int a);").contains("conflicting types for 'f'"));
        assert!(
            sema_err("int f(int a); int f(int a, int b);").contains("conflicting types for 'f'")
        );
        assert!(sema_err("int f; int f(void);").contains("different kind of symbol"));
        assert!(sema_err("int f(void); int f;").contains("different kind of symbol"));
        assert!(sema_err("int f(void); static int f(void) { return 1; }")
            .contains("static declaration of 'f' follows a non-static declaration"));
        assert!(sema_err("static int g; int g;")
            .contains("non-static declaration of 'g' follows a static declaration"));
        assert!(sema_err("int g = 1; int g = 1;").contains("redefinition of 'g'"));
        assert!(sema_err("extern int g = 1; int g = 2;").contains("redefinition of 'g'"));
        assert!(sema_err("int g = n;").contains("undeclared identifier 'n'"));
        assert!(sema_err("int n; int g = n;")
            .contains("initializer of 'g' is not a constant expression"));
        assert!(sema_err("int f(void); int g = f();")
            .contains("initializer of 'g' is not a constant expression"));
        assert!(sema_err("int g = 1 / 0;").contains("initializer of 'g' divides by zero"));
        assert!(sema_err("int g = 1 && (1 / 0);").contains("divides by zero"));
        assert!(sema_err("int g = 2147483647 + 1;").contains("initializer of 'g' overflows 'int'"));
        assert!(sema_err("int g = 2147483647 * 2;").contains("overflows 'int'"));
        assert!(sema_err("int g = (-2147483647 - 1) / -1;")
            .contains("initializer of 'g' overflows 'int'"));
        assert!(sema_err("int g = (-2147483647 - 1) % -1;").contains("overflows 'int'"));
        assert!(sema_err("int g = -(-2147483647 - 1);").contains("overflows 'int'"));
        assert!(
            sema_err("int f(int a, int b); int main(void) { return f(1); }")
                .contains("expects 2 arguments, found 1")
        );
        assert!(sema_err("int f(int a, int a);").contains("redefinition of parameter 'a'"));
        assert!(
            sema_err("static int missing(void); int main(void) { return missing(); }")
                .contains("static function 'missing' is used but not defined")
        );
        assert!(sema_err("int f(int a); int main(void) { return a; }")
            .contains("undeclared identifier 'a'"));
        assert!(
            sema_err("int main(void) { const int x = 1; x = 2; return x; }")
                .contains("cannot assign to const-qualified type 'const int'")
        );
        assert!(
            sema_err("int main(void) { const char *p = \"a\"; *p = 'b'; return 0; }")
                .contains("const-qualified type 'const char'")
        );
        assert!(
            sema_err("int main(void) { char *const p = 0; p = 0; return 0; }")
                .contains("const-qualified type 'char * const'")
        );
        assert!(
            sema_err("int main(void) { const char s[] = \"hi\"; s[0] = 'a'; return 0; }")
                .contains("const-qualified type 'const char'")
        );
        assert!(
            sema_err("void g(char *); int main(void) { const char *p = 0; g(p); return 0; }")
                .contains("incompatible types assigning 'const char *' to 'char *'")
        );
        assert!(sema_err(
            "void h(const char **); int main(void) { char **p = 0; h(p); return 0; }"
        )
        .contains("incompatible types assigning 'char **' to 'const char **'"));
        assert!(sema_err("char d[1] = \"hi\"; int main(void) { return 0; }")
            .contains("initializer string is too long"));
        assert!(sema_err("int a[2] = \"hi\";").contains("string literal cannot initialize"));
        assert!(sema_err("int main(void) { printf(\"hi\\n\"); return 0; }")
            .contains("undeclared identifier 'printf'"));
        assert!(sema_err("int f(int, ...); int f(int);").contains("conflicting types"));
        assert!(
            sema_err("const int f(void); int f(void) { return 1; }").contains("conflicting types")
        );
        assert!(
            sema_err("int printf(const char *, ...); int main(void) { return printf(); }")
                .contains("expects at least 1 argument, found 0")
        );
        assert!(
            sema_err("int main(void) { int a = 1; static int b = a * 2; return b; }")
                .contains("initializer of 'b' is not a constant expression")
        );
        assert!(sema_err("int main(void) { static int a[]; return 0; }")
            .contains("array size missing in 'a'"));
        assert!(
            sema_err("int main(void) { static int x; static int x; return x; }")
                .contains("redefinition of 'x'")
        );
        assert!(
            sema_err("int main(void) { int x; static int x; return x; }")
                .contains("redefinition of 'x'")
        );
        assert!(
            sema_err("int main(void) { static int x; extern int x; return x; }")
                .contains("redefinition of 'x'")
        );
        assert!(
            sema_err("int main(void) { int x; extern int x; return x; }")
                .contains("redefinition of 'x'")
        );
        assert!(
            sema_err("int main(void) { extern int x; int x; return x; }")
                .contains("redefinition of 'x'")
        );
        assert!(
            sema_err("int f(int i) { extern int i; return i; }").contains("redefinition of 'i'")
        );
        assert!(sema_err("int main(void) { extern int x = 1; return x; }")
            .contains("cannot have an initializer"));
        assert!(
            sema_err("int main(void) { extern int x; extern long x; return x; }")
                .contains("conflicting types for 'x'")
        );
        assert!(
            sema_err("int x; int main(void) { extern char x; return 0; }")
                .contains("conflicting types for 'x'")
        );
        assert!(
            sema_err("static int x; int main(void) { int x; { extern int x; return x; } }")
                .contains("non-static declaration of 'x' follows a static declaration")
        );
        assert!(
            sema_err("int main(void) { static int foo(void); return foo(); }")
                .contains("cannot have 'static' storage class")
        );
        assert!(
            sema_err("int foo(void); int main(void) { extern int foo; return 0; }")
                .contains("different kind of symbol")
        );
        assert!(
            sema_err("int foo; int main(void) { int foo(void); return 0; }")
                .contains("different kind of symbol")
        );
        assert!(
            sema_err("int main(void) { int foo(void); int foo = 1; return foo; }")
                .contains("different kind of symbol")
        );
        assert!(
            sema_err("int main(void) { int foo = 1; int foo(void); return foo; }")
                .contains("different kind of symbol")
        );
        assert!(sema_err(
            "int main(void) { int foo(int a); return foo(1); } int foo(void) { return 1; }"
        )
        .contains("conflicting types for 'foo'"));
        assert!(sema_err(
            "int main(void) { int foo(void); return foo(); } static int foo(void) { return 1; }"
        )
        .contains("static declaration of 'foo' follows a non-static declaration"));
        assert!(sema_err("int f(void) { { extern int a; } return a; }")
            .contains("undeclared identifier 'a'"));
        assert!(
            sema_err("int main(void) { do { int a = 1; } while (a < 2); return 0; }")
                .contains("undeclared identifier 'a'")
        );
        assert!(sema_err(
            "void g(void) { return; } int main(void) { do ; while (g()); return 0; }"
        )
        .contains("void value used where a value is required"));
    }

    #[test]
    fn static_extern_and_block_functions_are_in_scope() {
        sema_ok("int main(void) { static int x = 1; return x; }");
        sema_ok("int main(void) { static int x; return x; }");
        sema_ok("static int x = 1; int main(void) { extern int x; return x; }");
        sema_ok("int x = 1; int main(void) { int x = 2; { extern int x; return x; } }");
        sema_ok("static int f(void) { return 1; } int main(void) { int f(void); return f(); }");
        sema_ok("int main(void) { int g(int); return g(1); } int g(int x) { return x; }");
        sema_ok("int main(void) { extern int g(int); extern int g(int); return 0; }");
        sema_ok("int main(void) { do { break; } while (1); return 0; }");
        sema_ok("int main(void) { static int a[] = {1, 2}; return a[1]; }");
        sema_ok("int main(void) { static int x; static int *p = &x; return *p; }");
    }

    #[test]
    fn strings_const_and_varargs_are_accepted() {
        sema_ok("void f(const char *); int main(void) { char *p = \"hi\"; f(p); return 0; }");
        sema_ok("int f(const int x); int f(int x) { return x; }");
        sema_ok("int f(int *const p); int f(int *p) { return 0; }");
        sema_ok("int main(void) { volatile int x = 1; x = 2; return x; }");
        sema_ok("int main(void) { return sizeof \"abc\" + sizeof \"\" + sizeof \"a\\0b\"; }");
        let unit = analyzed("char s[] = \"hi\"; char t[2] = \"hi\"; char *p = \"ab\" \"c\";");
        let bytes = |name: &str| match &unit.globals.iter().find(|g| g.name == name).unwrap().init {
            Some(ConstInit::Aggregate(items)) => items
                .iter()
                .map(|item| match item {
                    ConstInit::Int(bits) => *bits as u8,
                    ConstInit::Zero => 0,
                    _ => 0xff,
                })
                .collect::<Vec<_>>(),
            _ => panic!("{name}"),
        };
        assert_eq!(bytes("s"), b"hi\0");
        assert_eq!(bytes("t"), b"hi");
        assert!(unit.strings.iter().any(|s| s.bytes == b"abc\0"));
    }

    #[test]
    fn accepts_shadowing_and_recursion() {
        sema_ok(
            "\
int main(void) {
    int x = 1;
    {
        int x = 2;
        return x;
    }
}
",
        );
        sema_ok(
            "\
int f(int n) {
    if (n == 0)
        return 1;
    return n + f(n - 1);
}
int main(void) { return f(1); }
",
        );
        assert!(
            sema_err("int main(void) { return later(1); } int later(int x) { return x; }")
                .contains("undeclared identifier 'later'")
        );
    }

    fn analyzed(source: &str) -> TranslationUnit {
        let tokens = lex(source, "ok.c").unwrap();
        let mut unit = parse(&tokens, "ok.c").unwrap();
        analyze(&mut unit, "ok.c").unwrap();
        unit
    }

    #[test]
    fn folds_globals_and_records_declarations() {
        let unit = analyzed(
            "\
int g = 1 + 2 * 3;
int zero;
extern int missing;
static int hidden = 4;
int h = 0 && (1 / 0);
int m = -2147483647 - 1;
int d = -5 / 2;
int s = -5 % 2;
int quiet = 0 && (2147483647 + 1);
",
        );
        let value = |name: &str| {
            unit.globals
                .iter()
                .find(|global| global.name == name)
                .unwrap()
                .int_init()
        };
        assert_eq!(value("g"), Some(7));
        assert_eq!(value("zero"), Some(0));
        assert_eq!(value("missing"), None);
        assert_eq!(value("h"), Some(0));
        assert_eq!(value("m"), Some(i32::MIN as u32 as u64));
        assert_eq!(value("d"), Some((-2i32) as u32 as u64));
        assert_eq!(value("s"), Some((-1i32) as u32 as u64));
        assert_eq!(value("quiet"), Some(0));
        let hidden = unit
            .globals
            .iter()
            .find(|global| global.name == "hidden")
            .unwrap();
        assert_eq!(hidden.linkage, Linkage::Internal);
        assert_eq!(hidden.int_init(), Some(4));

        let forward = analyzed(
            "\
int add(int a, int b);
int add(int x, int y) { return x + y; }
static int id(void);
int id(void) { return 1; }
int only(int);
",
        );
        assert_eq!(forward.func_decls.len(), 1);
        assert_eq!(forward.func_decls[0].name, "only");
        assert_eq!(forward.func_decls[0].params, vec![CType::int_type()]);
        match &forward.decls[2] {
            TopDecl::Func(func) => assert_eq!(func.linkage, Linkage::Internal),
            TopDecl::Global(_) => panic!("expected the static function"),
        }
        sema_ok("static int unused(void); int main(void) { return 0; }");
        sema_ok(
            "\
int is_even(int n);
int is_odd(int n);
int is_even(int n) { if (n == 0) return 1; return is_odd(n - 1); }
int is_odd(int n) { if (n == 0) return 0; return is_even(n - 1); }
",
        );
    }

    #[test]
    fn folds_bitwise_shifts_sizeof_and_conditional() {
        let unit = analyzed(
            "\
int bits = (6 & 3) | (1 ^ 4);
int tilde = ~0;
int shl = 1 << 4;
int shr = -8 >> 1;
int wide = (-2147483647 - 1) >> 1;
int pick = 1 ? 7 : 1 / 0;
int other = 0 ? 1 / 0 : 5;
int quiet_or = 1 || (1 / 0);
int sz = sizeof(int);
int sz_expr = sizeof(1 / 0);
int self_sz = sizeof(self_sz);
int nested = sizeof sizeof(int);
int n;
int uneval_inc = 0 ? n++ : 9;
int skipped = 0 && (n = 3);
int foo(void);
int uneval_call = 1 ? 4 : foo();
",
        );
        let value = |name: &str| {
            unit.globals
                .iter()
                .find(|global| global.name == name)
                .unwrap_or_else(|| panic!("missing {name}"))
                .int_init()
        };
        assert_eq!(value("bits"), Some((6 & 3) | (1 ^ 4)));
        assert_eq!(value("tilde"), Some((-1i32) as u32 as u64));
        assert_eq!(value("shl"), Some(16));
        assert_eq!(value("shr"), Some((-4i32) as u32 as u64));
        assert_eq!(value("wide"), Some((i32::MIN >> 1) as u32 as u64));
        assert_eq!(value("pick"), Some(7));
        assert_eq!(value("other"), Some(5));
        assert_eq!(value("quiet_or"), Some(1));
        assert_eq!(value("sz"), Some(4));
        assert_eq!(value("sz_expr"), Some(4));
        assert_eq!(value("self_sz"), Some(4));
        assert_eq!(value("nested"), Some(8));
        assert_eq!(value("uneval_inc"), Some(9));
        assert_eq!(value("skipped"), Some(0));
        assert_eq!(value("uneval_call"), Some(4));

        assert!(sema_err("int g = (1, 2);").contains("not a constant expression"));
        assert!(sema_err("int n; int g = n++;").contains("not a constant expression"));
        assert!(sema_err("int n; int g = ++n;").contains("not a constant expression"));
        assert!(sema_err("int n; int g = n = 1;").contains("not a constant expression"));
        assert!(sema_err("int n; int g = n += 1;").contains("not a constant expression"));
        assert!(sema_err("int f(void); int g = f();").contains("not a constant expression"));
        assert!(sema_err("int n; int g = 1 ? n++ : 2;").contains("not a constant expression"));
        assert!(sema_err("int g = 1 << 31;").contains("overflows 'int'"));
        assert!(sema_err("int g = 1 << 32;").contains("undefined shift"));
        assert!(sema_err("int g = 1 << -1;").contains("undefined shift"));
        assert!(sema_err("int g = -1 << 1;").contains("undefined shift"));
        assert!(sema_err("int g = 8 >> -1;").contains("undefined shift"));
        assert!(sema_err("int g = 8 >> 32;").contains("undefined shift"));
        assert!(sema_err("int g = 0 & (1 / 0);").contains("divides by zero"));
        assert!(sema_err("int g = 0 ? (1 / 0) : 1 / 0;").contains("divides by zero"));
        assert!(sema_err("int main(void) { return ++1; }").contains("not assignable"));
        assert!(sema_err("int main(void) { int x; x++ ++; return x; }").contains("not assignable"));
        assert!(sema_err("int main(void) { int x; ++ ++x; return x; }").contains("not assignable"));
        assert!(
            sema_err("int main(void) { int x; (x + 1) += 2; return x; }")
                .contains("not assignable")
        );
        assert!(sema_err(
            "int main(void) { int a; int b; int c; int d; a ? b : c = d; return 0; }"
        )
        .contains("not assignable"));
        assert!(
            sema_err("void f(void); int main(void) { return 1 ? f() : 2; }")
                .contains("incompatible operand types")
        );
        assert!(
            sema_err("int main(void) { return sizeof(void); }").contains("incomplete type 'void'")
        );
        assert!(
            sema_err("void f(void); int main(void) { return sizeof(f()); }")
                .contains("incomplete type 'void'")
        );
        sema_ok("void f(void); void g(void); void h(int c) { c ? f() : g(); }");
        sema_ok("void f(void); int main(void) { return (f(), 3); }");
        sema_ok("int main(void) { int x; x = 1; x += 2; ++x; x++; return sizeof(x++); }");
    }

    #[test]
    fn integer_types_agree_and_fold() {
        assert!(sema_err("char g; signed char g;").contains("conflicting types for 'g'"));
        assert!(sema_err("int g; unsigned g;").contains("conflicting types for 'g'"));
        assert!(sema_err("int f(long); int f(long long);").contains("conflicting types"));
        assert!(sema_err("int f(char); int f(signed char);").contains("conflicting types"));
        sema_ok("int g; signed int g; signed g;");
        sema_ok("unsigned u; unsigned int u;");
        sema_ok("long l; long int l; signed long l;");
        sema_ok("unsigned long long u; unsigned long long int u;");
        sema_ok("int f(int); int f(signed); int f(signed int x) { return x; }");
        sema_ok("char f(char c); char f(char x) { return x; }");

        let unit = analyzed(
            "\
int cmp = -1 < 1u;
int cmp_long = -1L < 1u;
int cmp_signed = -1 < 1;
char c = 1000;
unsigned char uc = -1;
unsigned u = 1u << 31;
unsigned w = 4294967295u + 1u;
int nested = sizeof(sizeof(char));
unsigned long sz = sizeof(int);
long wide = 1L << 40;
unsigned long cond_ul = 1 ? -1 : 1u;
int cond_cmp = (1 ? -1 : 1u) > 0;
",
        );
        let value = |name: &str| {
            unit.globals
                .iter()
                .find(|global| global.name == name)
                .unwrap_or_else(|| panic!("missing {name}"))
                .int_init()
        };
        assert_eq!(value("cmp"), Some(0));
        assert_eq!(value("cmp_long"), Some(1));
        assert_eq!(value("cmp_signed"), Some(1));
        assert_eq!(value("c"), Some(232));
        assert_eq!(value("uc"), Some(255));
        assert_eq!(value("u"), Some(2147483648));
        assert_eq!(value("w"), Some(0));
        assert_eq!(value("nested"), Some(8));
        assert_eq!(value("sz"), Some(4));
        assert_eq!(value("wide"), Some(1u64 << 40));
        assert_eq!(value("cond_ul"), Some(4294967295));
        assert_eq!(value("cond_cmp"), Some(1));

        assert!(sema_err("long g = 1L << 63;").contains("overflows 'long'"));
        sema_ok("unsigned long g = 1ul << 63;");
        assert!(sema_err("int g = 1 << 40;").contains("undefined shift"));
        sema_ok("long g = 1L << 40;");
        assert!(sema_err("int g = 2147483647 + 1;").contains("overflows 'int'"));
        sema_ok("char g = 1000;");
    }

    #[test]
    fn pointers_and_arrays() {
        sema_ok(
            "\
int g;
int *p = &g;
int a[] = {1, 2, 3};
int *q = &a[1];
int *r = a + 1;
int main(void) {
    int x = 1;
    int *lp = &x;
    *lp = 2;
    return a[0] + *q + (p != 0);
}
",
        );
        assert!(
            sema_err("int main(void) { int *p; char *c; p = c; return 0; }")
                .contains("incompatible types assigning 'char *' to 'int *'")
        );
        assert!(sema_err("int main(void) { void *v; v = v + 1; return 0; }")
            .contains("arithmetic on a pointer to void"));
        assert!(sema_err("int main(void) { int a[]; return 0; }").contains("array size missing"));
        assert!(sema_err("int a[2] = {1, 2, 3};").contains("excess elements in array initializer"));
        assert!(sema_err("int main(void) { int *p; return p < 0; }")
            .contains("invalid operands to '<'"));
        assert!(sema_err("int *p = 1;").contains("incompatible types"));
        sema_ok("int *p = 0; int *q = (void *)0;");
        sema_ok("void *v; int *p; void id(void) { v = p; p = v; }");
        assert!(sema_err("int main(void) { int a[3]; a = a; return 0; }")
            .contains("array type is not assignable"));
        assert!(sema_err("int main(void) { return &1; }").contains("cannot take the address"));
        assert!(sema_err("int g; int x = (int)&g;").contains("truncates an address"));
        assert!(sema_err("int g; long x = (int)&g;").contains("truncates an address"));
        sema_ok("int g; long x = (long)&g;");
        assert!(
            sema_err("int main(void) { int (*p)[]; return p[0][0]; }").contains("incomplete type")
        );
    }

    #[test]
    fn struct_layout_members_and_diagnostics() {
        let unit = analyzed(
            "\
struct S { char c; int i; long l; };
struct S g = {1, 2, 3};
struct Node { int val; struct Node *next; };
struct Node n = {4, 0};
",
        );
        let s = unit
            .structs
            .iter()
            .find(|handle| handle.tag() == "S")
            .expect("struct S");
        assert_eq!(s.size(), Some(16));
        assert_eq!(s.align(), 8);
        let fields = s.fields().expect("complete");
        assert_eq!(fields[0].name, "c");
        assert_eq!(fields[0].offset, 0);
        assert_eq!(fields[1].name, "i");
        assert_eq!(fields[1].offset, 4);
        assert_eq!(fields[2].offset, 8);
        assert!(s.field("missing").is_none());
        assert_eq!(s.field("i").unwrap().1.offset, 4);
        let node = unit
            .structs
            .iter()
            .find(|handle| handle.tag() == "Node")
            .expect("struct Node");
        assert_eq!(node.size(), Some(16));
        assert!(node.field("next").unwrap().1.ty.is_pointer());
        let g = unit
            .globals
            .iter()
            .find(|global| global.name == "g")
            .unwrap();
        match &g.init {
            Some(ConstInit::Aggregate(items)) => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], ConstInit::Int(1));
                assert_eq!(items[1], ConstInit::Int(2));
                assert_eq!(items[2], ConstInit::Int(3));
            }
            other => panic!("expected aggregate, got {other:?}"),
        }

        sema_ok("struct S { int x; }; void f(struct S s) { (void)s; }");
        sema_ok("struct S { int x; }; int f(struct S);");
        assert!(sema_err("struct S; void f(struct S s) { (void)s; }")
            .contains("parameter has incomplete type 'struct S'"));
        assert!(
            sema_err("struct S; struct S f(void) { struct S s; return s; }")
                .contains("incomplete type")
        );
        assert!(sema_err("struct S { struct S { int x; } in; int y; };")
            .contains("redefinition of struct 'S'"));
        assert!(sema_err(
            "struct S { int x; }; int main(void) { struct S s; struct S t; (s = t).x = 1; return 0; }"
        )
        .contains("expression is not assignable"));
        sema_ok("struct S { int x; }; struct S f(void) { struct S s; return s; }");
        assert!(sema_err("struct S; struct S s;").contains("incomplete type 'struct S'"));
        sema_ok("struct S; extern struct S x; int main(void) { return &x == 0; }");
        assert!(
            sema_err("struct S; struct S *p; int main(void) { *p; return 0; }")
                .contains("cannot use incomplete type 'struct S' as a value")
        );
        assert!(
            sema_err("int main(void) { struct S *p = 0; *p; return 0; }")
                .contains("cannot use incomplete type 'struct S' as a value")
        );
        assert!(sema_err(
            "struct S; extern struct S x = {1}; int main(void) { return 0; } struct S { int a; };"
        )
        .contains("incomplete type 'struct S'"));
        sema_ok("struct S; struct S *p; int main(void) { p = 0; return &*p == p; }");
        assert!(
            sema_err("struct S; int main(void) { return sizeof(struct S); }")
                .contains("incomplete type 'struct S'")
        );
        assert!(
            sema_err("struct S; int main(void) { struct S *p; return p->x; }")
                .contains("incomplete type 'struct S'")
        );
        assert!(
            sema_err("struct S { int x; }; int main(void) { struct S s; return s.y; }")
                .contains("no member named 'y'")
        );
        assert!(sema_err("struct S { int x; }; struct S { int y; };")
            .contains("redefinition of struct 'S'"));
        assert!(sema_err("struct S { int x; int x; };").contains("duplicate member"));
        sema_ok("struct S { int n; int a[]; }; int main(void) { return (int)sizeof(struct S); }");
        assert!(sema_err("struct S { int a[]; };").contains("another named member"));
        assert!(sema_err("struct S { int a[]; int n; };").contains("not the last member"));
        assert!(sema_err("union U { int n; int a[]; };").contains("not allowed in a union"));
        assert!(
            sema_err("struct F { int n; int a[]; }; struct F arr[2];").contains("array element")
        );
        assert!(
            sema_err("struct F { int n; int a[]; }; struct O { struct F f; };")
                .contains("struct member")
        );
        assert!(
            sema_err("struct S { int n; int a[]; }; struct S s = {1, {2}};")
                .contains("flexible array member")
        );
        assert!(sema_err(
            "struct S { int x:3; }; int main(void) { struct S s; int *p = &s.x; return *p; }"
        )
        .contains("address of a bit-field"));
        sema_ok("struct { int x; } s; int main(void) { s.x = 1; return s.x; }");
        assert!(sema_err("struct S {};").contains("no members"));
        assert!(
            sema_err("struct S { int x; int y; }; struct S s = { .z = 1 };")
                .contains("no member named 'z'")
        );
        assert!(sema_err(
            "struct S { const int x; int y; }; void f(struct S *a, struct S *b) { *a = *b; }"
        )
        .contains("const member"));
        assert!(
            sema_err("struct S { int x; }; int main(void) { struct S s; s++; return 0; }")
                .contains("++ and -- require a scalar")
        );
        assert!(sema_err("void f(struct A *p); void f(struct A *p);")
            .contains("conflicting types for 'f'"));
        assert!(
            sema_err("struct S { int x; int y; }; struct S s = {1, 2, 3};")
                .contains("excess elements in struct initializer")
        );
        sema_ok(
            "\
struct S { int x; };
void f(struct S *p);
void f(struct S *p) { p->x = 1; }
",
        );
    }

    #[test]
    fn typedef_enum_union_diagnostics() {
        sema_ok(
            "\
typedef int T;
typedef T T;
enum E { A = 1, B };
union U { int x; char c; };
int main(void) { T t = A; union U u = {t}; return u.x + B; }
",
        );
        assert!(sema_err("typedef int T; typedef long T;").contains("redefinition of 'T'"));
        assert!(sema_err("enum E x;").contains("incomplete enum"));
        assert!(sema_err("restrict int x;").contains("restrict requires a pointer"));
        sema_ok("int main(void) { float f; f = 1; return f == 1.0f; }");
        sema_ok("int main(void) { return 1.0L; }");
        sema_ok(
            "enum { N = (int)3.9 }; int main(void) { int a[(int)2.9]; return N == 3 && sizeof(a) == 2 * sizeof(int); }",
        );
        assert!(sema_err("enum { N = 1.5 };").contains("not an integer constant"));
        assert!(sema_err("enum { N = (int)-1.5 };").contains("not an integer constant"));
        sema_ok("extern long double x; void f(void) { for (;; x++) {} }");
        sema_ok("int main(void) { int *p = (int)0.0; switch (1) { case (int)1.5: return p == 0; } return 1; }");
        sema_ok("int main(void) { long double x; x = 1.0; return 0; }");
        sema_ok("long double f(long double x) { return x; }");
        assert!(
            sema_err("union U { int x; char c; }; union U u = {1, 2};").contains("excess elements")
        );
        assert!(
            sema_err("struct S; struct S f(void); int main(void) { return f().x; }")
                .contains("incomplete type")
        );
        sema_ok("struct S { int x; }; struct S f(void); int main(void) { return f().x; }");
        sema_ok("struct S { double d; }; void f(struct S s) { (void)s; }");
        sema_ok("struct S { float a; float b; }; struct S f(struct S s) { return s; }");
        sema_ok("struct S { long double x; }; void f(struct S s) { (void)s; }");
        sema_ok("struct S { long double x; }; struct S f(void) { struct S s; return s; }");
        sema_ok("int f(void) __asm__(\"g\"); int f(void) { return 1; }");
        sema_ok("int f(void) __asm__(u8\"g\"); int f(void) { return 1; }");
        assert!(sema_err("int f(void) __asm__(u\"g\");").contains("byte string"));
        let ir = crate::compile_to_ir(
            "struct LD { long double x; }; volatile struct LD v; struct LD f(void) { return v; }\n",
            "volld.c",
        )
        .unwrap();
        assert!(ir.contains("load volatile x86_fp80"), "{ir}");
        assert!(
            sema_err("int f(int); int main(void) { int (*p)(char); p = f; return 0; }")
                .contains("incompatible types")
        );
        assert!(sema_err(
            "int f(int); int main(void) { int (*p)(int) = f; int (*q)(int) = f; return p < q; }"
        )
        .contains("relational comparison of function pointers"));
        sema_ok(
            "int f(int); int main(void) { int (*p)(int) = f; int (*q)(int) = f; return p == q; }",
        );
        assert!(sema_err("int main(void) { int x; return x(1); }").contains("not a function"));
        assert!(sema_err("enum E { A = 1, A = 2 };").contains("redefinition of 'A'"));
        assert!(
            sema_err("static int f(void); int main(void) { int (*p)(void) = f; return 0; }")
                .contains("static function 'f' is used but not defined")
        );
        assert!(sema_err("int g; union U { int *p; long l; } u = { &g };")
            .contains("address in a union"));
        assert!(sema_err("struct S { int *p : 3; };").contains("non-integral"));
        assert!(sema_err("struct S { int x : 4294967297L; };").contains("exceeds the width"));
        sema_ok("struct A { int a : 3; }; struct O { struct A in; } g = {{1}};");
        sema_ok("struct A { int a : 3; int b : 5; }; struct A arr[2] = {{1, 2}, {3, 4}};");
        sema_ok("union U { char b; long a; } g = { .a = 0x1234 };");
        sema_ok("union U { double d; long l; } g = { 1.5 };");
        let ir = crate::compile_to_ir(
            "volatile struct S { int a; } v; struct S x; void f(void) { x = v; }\n",
            "vol.c",
        )
        .unwrap();
        assert!(
            ir.contains("call void @llvm.memmove.p0.p0.i64(ptr @x, ptr @v, i64 4, i1 true)"),
            "{ir}"
        );
        sema_ok("union U { int *p; long l; } u = { 0 };");
        sema_ok("union U { int *p; long l; } u = { (int *)1 };");
        sema_ok("struct S { int a; union { char *s; long n; } v; } g = { 1 };");
        sema_ok("struct T { int a; }; int main(void) { union T; int x = 1; return x; }");
        assert!(sema_err("struct T { int a; }; void g(union T *p);").contains("different tag type"));
        assert!(sema_err("struct T { int a; union T *p; };").contains("different tag type"));
        sema_ok("int main(void) { int *p = 0; _Bool b = p; return b; }");
        let ir = crate::compile_to_ir(
            "#include <stdarg.h>\nint f(va_list ap) { return va_arg(ap, int); }\n",
            "va.c",
        )
        .unwrap();
        assert!(ir.contains("phi ptr"), "{ir}");
    }

    #[test]
    fn deep_union_diamond_still_initializes() {
        // Each union has two members of the previous union. Searching both
        // at every level is exponential; one visit per record stays linear.
        let depth = 16;
        let mut source = String::from("union U0 { int i; };\n");
        for n in 1..=depth {
            source.push_str(&format!(
                "union U{n} {{ union U{} a; union U{} b; }};\n",
                n - 1,
                n - 1
            ));
        }
        source.push_str(&format!(
            "struct O {{ union U{depth} u; int n; }};\nstruct O g = {{ .n = 3 }};\nint main(void) {{ return g.n; }}\n"
        ));
        sema_ok(&source);
    }

    #[test]
    fn switch_diagnostics() {
        sema_ok(
            "\
enum E { A = 1, B = 4 };
int main(void) {
    int x = 1;
    int y = 0;
    char c = 1;
    unsigned char uc = 1;
    unsigned u = 1;
    switch (x) {
        int skipped = (y = 5);
    case 0:
        y = skipped;
        break;
    case A:
    case 'B':
    case sizeof(int):
    case (short)-2:
    case (unsigned char)3:
        return y;
    default:
        break;
    }
    switch (c) {
    case 256:
        return 1;
    case 1:
        return 2;
    }
    switch (uc) {
    case -1:
        return 3;
    case 255:
        return 4;
    }
    switch (u) {
    case -1:
        return 5;
    }
    switch (x) {
    case 1:
        while (0) {
            continue;
            break;
        }
        break;
    }
    do { switch (x) { case 1: continue; } } while (0);
    return 0;
}
",
        );
        assert!(sema_err("int main(void) { case 1: return 0; }")
            .contains("'case' label is not inside a switch"));
        assert!(sema_err("int main(void) { default: return 0; }")
            .contains("'default' label is not inside a switch"));
        assert!(sema_err("int main(void) { while (0) case 1: return 0; }")
            .contains("'case' label is not inside a switch"));
        assert!(sema_err(
            "int main(void) { int x; switch (x) { case 1: return 0; case 1: return 1; } }"
        )
        .contains("duplicate case value '1'"));
        assert!(sema_err(
            "int main(void) { int x; switch (x) { case -1: return 0; case 4294967295u: return 1; } }"
        )
        .contains("duplicate case value '-1'"));
        assert!(sema_err(
            "int main(void) { int x; switch (x) { case 0: return 0; case 17179869184: return 1; } }"
        )
        .contains("duplicate case value '0'"));
        assert!(sema_err(
            "int main(void) { long x; switch (x) { case 100: return 0; case 100l: return 1; } }"
        )
        .contains("duplicate case value '100'"));
        assert!(sema_err(
            "int main(void) { unsigned x; switch (x) { case 4294967295u: return 0; case 1099511627775l: return 1; } }"
        )
        .contains("duplicate case value '4294967295'"));
        assert!(sema_err(
            "int main(void) { int x; switch (x) { default: return 0; default: return 1; } }"
        )
        .contains("multiple default labels"));
        assert!(sema_err(
            "int main(void) { int x; switch (x) { case 1: if (x) { case 1: return 0; } return 1; } }"
        )
        .contains("duplicate case value '1'"));
        assert!(sema_err(
            "int main(void) { int x; switch (x) { default: while (0) default: ; return 0; } }"
        )
        .contains("multiple default labels"));
        assert!(
            sema_err("int main(void) { int *p = 0; switch (p) { default: return 0; } }")
                .contains("expected an integer")
        );
        assert!(sema_err(
            "int main(void) { struct S { int x; } s; s.x = 1; switch (s) { default: return 0; } }"
        )
        .contains("expected an integer"));
        assert!(sema_err(
            "void f(void) { return; } int main(void) { switch (f()) { default: return 0; } }"
        )
        .contains("void value"));
        assert!(
            sema_err("int main(void) { int a[2]; switch (a) { default: return 0; } }")
                .contains("expected an integer")
        );
        assert!(
            sema_err("int main(void) { int x; switch (x) { case x: return 0; } }")
                .contains("not an integer constant expression")
        );
        assert!(
            sema_err("int main(void) { switch (1) { case 1 / 0: return 0; } }")
                .contains("divides by zero")
        );
        assert!(
            sema_err("int main(void) { switch (1) { case 2147483647 + 1: return 0; } }")
                .contains("overflows 'int'")
        );
        assert!(
            sema_err("int main(void) { switch (1) { case 1 << 31: return 0; } }")
                .contains("overflows 'int'")
        );
        assert!(
            sema_err("int main(void) { switch (1) { case 1 << 32: return 0; } }")
                .contains("undefined shift")
        );
        assert!(
            sema_err("int main(void) { switch (1) { case (int *)0: return 0; } }")
                .contains("expected an integer")
        );
        assert!(
            sema_err("int main(void) { switch (1) { case 1: continue; } }")
                .contains("'continue' is not inside a loop")
        );
        assert!(sema_err("int main(void) { break; }").contains("not inside a loop or switch"));
        sema_ok("int main(void) { switch (1) { case 1: break; } return 0; }");
        sema_ok(
            "int main(void) { int x; switch (x) { case 1: return 1; } switch (x) { case 1: return 2; } return 0; }",
        );
    }

    #[test]
    fn math_format_builtins_are_constant_floats() {
        // C11 7.12p3–p5 and GCC "Floating-Point Format Builtins": these
        // names are constant expressions of the documented types, so they
        // may initialize static storage and appear in `_Generic`.
        sema_ok("float n = __builtin_nanf(\"\");");
        sema_ok("float i = __builtin_inff();");
        sema_ok("double h = __builtin_huge_val();");
        sema_ok("float hf = __builtin_huge_valf();");
        sema_ok("long double hl = __builtin_huge_vall();");
        sema_ok("double n = __builtin_nan(\"\");");
        sema_ok("long double n = __builtin_nanl(\"\");");
        sema_ok("double i = __builtin_inf();");
        sema_ok("long double i = __builtin_infl();");
        sema_ok("float n = __builtin_nanf(\"1\");");
        sema_ok("double n = __builtin_nan(\"1\0ignored\");");
        sema_ok("float n = __builtin_nansf(\"\");");
        sema_ok("double n = __builtin_nans(\"\");");
        sema_ok("long double n = __builtin_nansl(\"\");");
        sema_ok("int main(void) { static float n = __builtin_nanf(\"\"); return n != n; }");
        sema_ok("int a = _Generic(__builtin_nanf(\"\"), float: 1, default: 0);");
        sema_ok("int a = _Generic(__builtin_huge_val(), double: 1, default: 0);");
        sema_ok("int a = _Generic(__builtin_huge_vall(), long double: 1, default: 0);");
        // The system <math.h> macros expand to these builtins under
        // __GNUC__ 3.3 or later (C11 7.12p3–p5).
        let ir = crate::compile_to_ir(
            "#include <math.h>\nfloat n = NAN;\nfloat i = INFINITY;\ndouble h = HUGE_VAL;\n",
            "math-macros.c",
        )
        .expect("system math.h macros should compile");
        assert!(
            ir.contains("0x7FF8000000000000") && ir.contains("0x7FF0000000000000"),
            "NAN and INFINITY / HUGE_VAL constants missing: {ir}"
        );

        assert!(
            sema_err("float n = __builtin_nanf(0);").contains("requires a string literal argument")
        );
        assert!(sema_err("float n = __builtin_nanf();").contains("expects 1 argument"));
        assert!(sema_err("float n = __builtin_inff(1);").contains("expects 0 arguments"));
        assert!(
            sema_err("const char *p = \"\"; float n = __builtin_nanf(p);")
                .contains("requires a string literal argument")
        );
        // A string that strtol would not consume in full is not a constant
        // (GCC "Floating-Point Format Builtins"). A static initializer
        // and an ICE keep diagnosing. A runtime __builtin_nans* with a
        // malformed tag is also a diagnostic: there is no library nans.
        assert!(
            sema_err("float n = __builtin_nanf(\"abc\");").contains("not a constant expression")
        );
        assert!(
            sema_err("double x = __builtin_nan(\"1xyz\");").contains("not a constant expression")
        );
        assert!(sema_err(
            "int main(void) { static double x = __builtin_nan(\"1xyz\"); return 0; }"
        )
        .contains("not a constant expression"));
        assert!(
            sema_err("int main(void) { return (int)__builtin_nans(\"1xyz\"); }")
                .contains("strtol would consume in full")
        );
        // A runtime quiet malformed tag is accepted; IR lowering calls nan().
        sema_ok("int main(void) { return __builtin_nan(\"1xyz\") != 0.0; }");
    }

    #[test]
    fn offsetof_is_an_integer_constant_expression() {
        // C11 7.19p3: offsetof(type, member-designator) is an integer
        // constant expression of type size_t. <stddef.h> expands it to
        // __builtin_offsetof, which accepts `.member` and `[constant]`
        // designators after the first member name.
        let unit = analyzed(
            "\
struct In { char a; double b; int c; };
struct Out { short s; struct In items[3]; union { char k; long w; } u; struct { char p; int q; }; };
_Static_assert(__builtin_offsetof(struct Out, items[2].c) == 8 + 2 * 24 + 16, \"nested\");
enum { E = __builtin_offsetof(struct Out, u.w), Q = __builtin_offsetof(struct Out, q) };
unsigned long direct = __builtin_offsetof(struct Out, items);
unsigned long nested = __builtin_offsetof(struct Out, items[1].b);
unsigned long computed = __builtin_offsetof(struct Out, items[1 + 1].c);
unsigned long via_enum = E;
unsigned long anon = Q;
int bound[__builtin_offsetof(struct Out, items[0].c)];
unsigned long bound_size = sizeof bound;
int typed = _Generic(__builtin_offsetof(struct In, c), unsigned long: 1, default: 0);
",
        );
        let value = |name: &str| {
            unit.globals
                .iter()
                .find(|global| global.name == name)
                .unwrap_or_else(|| panic!("missing {name}"))
                .int_init()
        };
        assert_eq!(value("direct"), Some(8));
        assert_eq!(value("nested"), Some(8 + 24 + 8));
        assert_eq!(value("computed"), Some(8 + 2 * 24 + 16));
        assert_eq!(value("via_enum"), Some(80));
        assert_eq!(value("anon"), Some(92));
        assert_eq!(value("bound_size"), Some(4 * 24));
        assert_eq!(value("typed"), Some(1));

        // A non-constant subscript is not an integer constant expression.
        sema_ok(
            "struct S { int a[4]; }; int f(int i) { return (int)__builtin_offsetof(struct S, a[i]); }",
        );
        assert!(sema_err(
            "struct S { int a[4]; }; int i; _Static_assert(__builtin_offsetof(struct S, a[i]) == 0, \"\");"
        )
        .contains("integer constant expression"));

        // Constraints: a complete structure or union type, an existing
        // member, no bit-field (C11 7.19p3), and `[]` only on arrays.
        assert!(sema_err(
            "struct S { int a : 3; }; unsigned long n = __builtin_offsetof(struct S, a);"
        )
        .contains("bit-field"));
        assert!(sema_err(
            "struct S { int a; }; unsigned long n = __builtin_offsetof(struct S, b);"
        )
        .contains("no member named 'b'"));
        assert!(
            sema_err("struct S; unsigned long n = __builtin_offsetof(struct S, a);")
                .contains("incomplete")
        );
        assert!(sema_err("unsigned long n = __builtin_offsetof(int, a);")
            .contains("structure or union"));
        assert!(sema_err(
            "struct S { int a; }; unsigned long n = __builtin_offsetof(struct S, a[1]);"
        )
        .contains("subscripted value is not an array"));
        assert!(sema_err(
            "struct S { int a[2]; }; unsigned long n = __builtin_offsetof(struct S, a.b);"
        )
        .contains("not a structure or union"));
    }

    #[test]
    fn floating_comparison_builtins() {
        // GCC "Other Builtins": __builtin_isgreater and the rest are the
        // C11 7.12.14 comparison macros. Two real floating operands
        // (one may be an integer) are converted by the usual arithmetic
        // conversions; the result is int.
        for name in [
            "__builtin_isgreater",
            "__builtin_isgreaterequal",
            "__builtin_isless",
            "__builtin_islessequal",
            "__builtin_islessgreater",
            "__builtin_isunordered",
        ] {
            sema_ok(&format!(
                "int f(double a, double b) {{ return {name}(a, b); }}"
            ));
            sema_ok(&format!(
                "int f(float a, long double b) {{ return {name}(a, b); }}"
            ));
            sema_ok(&format!(
                "int f(int a, double b) {{ return {name}(a, b) + {name}(b, 2u); }}"
            ));
            sema_ok(&format!(
                "int f(float a, float b) {{ return _Generic({name}(a, b), int: 1, default: 0); }}"
            ));
            sema_ok(&format!(
                "unsigned long n = sizeof({name}(1.0, 2.0)); int a[sizeof {name}(1.0f, 2)];"
            ));
            assert!(
                sema_err(&format!("int f(int a, int b) {{ return {name}(a, b); }}"))
                    .contains("non-floating-point arguments")
            );
            assert!(
                sema_err(&format!("int f(double *p) {{ return {name}(p, 1.0); }}"))
                    .contains("non-floating-point arguments")
            );
            assert!(sema_err(&format!(
                "struct S {{ double d; }}; int f(struct S s) {{ return {name}(s, 1.0); }}"
            ))
            .contains("non-floating-point arguments"));
            assert!(
                sema_err(&format!("int f(double a) {{ return {name}(a); }}"))
                    .contains("expects 2 arguments")
            );
            assert!(
                sema_err(&format!("int f(double a) {{ return {name}(a, a, a); }}"))
                    .contains("expects 2 arguments")
            );
            // Not a constant expression (Clang 19 black-box agrees).
            assert!(sema_err(&format!("int x = {name}(1.0, 2.0);")).contains("not a constant"));
        }
    }

    #[test]
    fn external_object_clashing_with_a_runtime_nan_call() {
        // A malformed quiet __builtin_nan* tag at run time calls the C
        // library nan/nanf/nanl. An object with external linkage and the
        // same name is undefined behaviour (C11 7.1.3p1-p2) and would make
        // the call target the object, so it is a diagnostic at the call.
        for source in [
            "extern int nan;\ndouble f(void) { return __builtin_nan(\"1xyz\"); }",
            "int nan;\ndouble f(void) { return __builtin_nan(\"1xyz\"); }",
            "double f(void) { extern int nan; return __builtin_nan(\"1xyz\"); }",
            "double f(void) { return __builtin_nan(\"1xyz\"); }\nint nan;",
            "float nanf;\nfloat f(void) { return __builtin_nanf(\"x\"); }",
            "extern char nanl[];\nlong double f(void) { return __builtin_nanl(\"x\"); }",
        ] {
            let err = sema_err(source);
            assert!(err.contains("conflicts with the object"), "{source}: {err}");
            assert!(err.contains("external linkage"), "{source}: {err}");
            assert!(
                err.contains(":2:") || source.starts_with("double f"),
                "{source}: {err}"
            );
        }
        // The diagnostic points at the builtin call.
        let err = sema_err("int nan;\ndouble f(void) {\n  return __builtin_nan(\"1xyz\");\n}");
        assert!(err.contains(":3:10"), "{err}");

        // No clash: a valid tag folds, the other library name, an
        // unevaluated operand, internal linkage, a block-scope static, a
        // local, and a user-declared function.
        sema_ok("extern int nan;\ndouble f(void) { return __builtin_nan(\"\") + __builtin_nan(\"0x10\"); }");
        sema_ok("int nan;\nfloat f(void) { return __builtin_nanf(\"1xyz\"); }");
        sema_ok(
            "extern int nan;\nunsigned long f(void) { return sizeof(__builtin_nan(\"1xyz\")); }",
        );
        sema_ok("static int nan;\ndouble f(void) { return __builtin_nan(\"1xyz\"); }");
        sema_ok("int f(void) { static int nan; return (int)__builtin_nan(\"1xyz\") + nan; }");
        sema_ok("double f(void) { int nan = 0; return __builtin_nan(\"1xyz\") + nan; }");
        sema_ok("double nan(const char *);\ndouble f(void) { return __builtin_nan(\"1xyz\"); }");
    }

    #[test]
    fn unevaluated_generic_control_records_no_nan_call() {
        // The controlling expression of a generic selection is not
        // evaluated (C11 6.5.1.1p3), so a malformed tag there makes no
        // library call and cannot clash with an object named nan.
        sema_ok(
            "extern int nan;\nint f(void) { return _Generic(__builtin_nan(\"1xyz\"), double: 1, default: 0); }",
        );
        sema_ok(
            "float nanf;\nint f(void) { return _Generic(_Generic(0, int: __builtin_nanf(\"x\")), float: 1); }",
        );
        sema_ok(
            "extern int nan;\nint f(void) { return _Generic(1 + __builtin_nan(\"q\"), double: 2, default: 3) + (int)sizeof(__builtin_nan(\"q\")); }",
        );
        // The selected association is evaluated, so it still clashes.
        let err = sema_err(
            "extern int nan;\ndouble f(void) { return _Generic(0, int: __builtin_nan(\"1xyz\"), default: 0.0); }",
        );
        assert!(err.contains("conflicts with the object"), "{err}");
        let err = sema_err(
            "extern int nan;\ndouble f(void) { return _Generic(__builtin_nan(\"a\"), double: __builtin_nan(\"b\")); }",
        );
        assert!(
            err.contains("conflicts with the object") && err.contains(":2:"),
            "{err}"
        );
    }
}
