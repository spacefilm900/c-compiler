//! Name resolution and type checks for the C subset.
//!
//! Scopes follow C block scope. The parameter list of a function definition
//! shares a scope with the function's outermost block, so a parameter cannot
//! be redeclared there. A name is visible in its own initializer.
//!
//! File-scope objects and functions follow C11 6.2.2 linkage and 6.9.2
//! tentative definitions. A prototype makes a function visible before its
//! body, which is what mutual recursion needs. A global initializer must be
//! a constant (C11 6.6): an integer constant, an address constant, or a brace
//! list of those. Assignment, `++`/`--`, calls,
//! and the comma operator are rejected there, including when they sit in an
//! evaluated operand. An unevaluated operand (`sizeof`, or the skipped arm
//! of `&&`, `||`, or `?:`) may contain them.

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
        current_ret: CType::int_type(),
        fn_name: String::new(),
        global_order: Vec::new(),
        func_order: Vec::new(),
        strings: Vec::new(),
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
    /// `None` for a block-scope variable, which has no linkage.
    global: Option<GlobalInfo>,
}

#[derive(Clone)]
struct FuncSym {
    name: String,
    /// Linker name, including an `__asm__` label.
    symbol: String,
    ret: CType,
    params: Vec<CType>,
    variadic: bool,
    linkage: Linkage,
    defined: bool,
    used: bool,
    use_line: usize,
    use_column: usize,
}

#[derive(Clone)]
enum Symbol {
    Var(VarSym),
    Func(FuncSym),
}

struct Sema {
    filename: String,
    /// Last token at each line and column. Included files override the main
    /// file when they share a coordinate; parser diagnostics resolve this
    /// more precisely from the token cursor.
    locs: HashMap<(usize, usize), String>,
    scopes: Vec<HashMap<String, Symbol>>,
    loop_depth: u32,
    current_ret: CType,
    /// Name of the function whose body is being checked. Empty at file scope.
    fn_name: String,
    global_order: Vec<String>,
    func_order: Vec<String>,
    /// String literals that need a private global, keyed by bytes including NUL.
    strings: Vec<(Vec<u8>, String)>,
}

impl Sema {
    fn declare_global(&mut self, decl: &mut GlobalDecl) -> Result<(), Diagnostic> {
        let name = decl.name.clone();
        let mut ty = decl.ty.clone();
        let shaped = if let Some(init) = &decl.init {
            Some(self.shape_initializer(&ty, init, decl.line, decl.column)?)
        } else {
            None
        };
        if let Some((shaped_ty, _)) = &shaped {
            ty = shaped_ty.clone();
        }
        decl.ty = ty.clone();
        if !ty.is_array() && !ty.is_complete() {
            return self.err(
                decl.line,
                decl.column,
                format!("variable '{name}' has incomplete type '{}'", ty.display()),
            );
        }
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
                if !compatible(&var.ty, &ty) {
                    return self.err(
                        decl.line,
                        decl.column,
                        format!("conflicting types for '{name}'"),
                    );
                }
                ty = composite(&var.ty, &ty);
                decl.ty = ty.clone();
                Some(var.global.as_ref().expect("file-scope variable").linkage)
            }
        };
        let linkage =
            self.object_linkage(decl.storage, prior_linkage, decl.line, decl.column, &name)?;
        if prior.is_none() {
            let state = if decl.init.is_none() && decl.storage == Storage::Extern {
                GlobalState::Declared
            } else if decl.init.is_none() {
                GlobalState::Tentative
            } else {
                GlobalState::Declared
            };
            self.global_order.push(name.clone());
            self.insert_file(Symbol::Var(VarSym {
                name: name.clone(),
                slot: format!("@{name}"),
                ty: ty.clone(),
                global: Some(GlobalInfo { linkage, state }),
            }));
        } else {
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
        // A prototype may return a struct or union so the system headers can
        // declare div, ldiv, and lldiv. A definition, and any call, still
        // cannot: the System V class-ification for aggregates is not done.
        if func.body.is_some() && func.ret_type.unqualified().is_record() {
            let word = func
                .ret_type
                .struct_ref()
                .map(|handle| handle.kind().word())
                .unwrap_or("struct");
            return self.err(
                func.line,
                func.column,
                format!("returning a {word} by value is not supported"),
            );
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
                    linkage,
                    defined: is_def,
                    used: false,
                    use_line: 0,
                    use_column: 0,
                }));
            }
            Some(Symbol::Func(existing)) => {
                if !compatible(&existing.ret, &func.ret_type)
                    || existing.variadic != func.variadic
                    || existing.params.len() != params.len()
                    || existing
                        .params
                        .iter()
                        .zip(&params)
                        .any(|(old, new)| !compatible(old, new))
                {
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
            }));
        }
        let body = func.body.as_mut().expect("defined function");
        self.check_compound(body, false)?;
        self.pop();
        self.fn_name.clear();
        Ok(())
    }

    fn finish(&mut self, unit: &mut TranslationUnit) -> Result<(), Diagnostic> {
        let mut missing_static = None;
        let mut incomplete_array = None;
        let mut func_decls = Vec::new();
        let mut globals = Vec::new();
        {
            let file = self.scopes.first().expect("file scope");
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
                } else {
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
        unit.globals = globals;
        unit.strings = self
            .strings
            .iter()
            .map(|(bytes, name)| StrConst {
                name: name.clone(),
                bytes: bytes.clone(),
            })
            .collect();
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
            if let Some(bytes) = string_payload(init) {
                if elem.is_character() {
                    ty = CType::array(elem.clone(), Some(bytes.len() as u64 + 1));
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
        let mut count = 0u64;
        let mut scratch = Vec::new();
        while pos < items.len() {
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
            count += 1;
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
        if ty.is_scalar() {
            let expr = self.scalar_expr(init)?;
            stores.push(ElemStore {
                offset: base,
                ty: ty.clone(),
                expr,
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
                });
                return Ok(());
            }
        }
        if let Some(bytes) = string_payload(init).map(|bytes| bytes.to_vec()) {
            if is_char_array(ty) {
                return self.fill_from_string(ty, &bytes, base, stores, init);
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
            if is_char_array(elem) {
                if let Some(bytes) = string_payload(&items[pos]).map(|bytes| bytes.to_vec()) {
                    self.fill_from_string(elem, &bytes, off, stores, &items[pos])?;
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
            for field in &fields {
                if *pos >= items.len() {
                    break;
                }
                let off = base + field.offset;
                if self.opens_subobject(&field.ty, &items[*pos]) {
                    self.fill_object(&field.ty, &items[*pos], off, stores, line, column)?;
                    *pos += 1;
                } else {
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
        // A brace list for a union initializes the first member only.
        let fields: Vec<_> = if ty.is_union() {
            fields.into_iter().take(1).collect()
        } else {
            fields
        };
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
            let off = base + field.offset;
            if self.opens_subobject(&field.ty, &items[pos]) {
                self.fill_object(&field.ty, &items[pos], off, stores, *line, *column)?;
                pos += 1;
            } else {
                self.fill_unbraced(&field.ty, items, &mut pos, off, stores, (*line, *column))?;
            }
        }
        if pos < items.len() {
            return self.err(*line, *column, "excess elements in struct initializer");
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
                    Init::List { line, column, .. } => {
                        self.err(*line, *column, "excess braces around scalar initializer")
                    }
                }
            }
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
        base: u64,
        stores: &mut Vec<ElemStore>,
        init: &Init,
    ) -> Result<(), Diagnostic> {
        let (line, column) = init_pos(init);
        let Some(len) = ty.array_len() else {
            return self.err(line, column, "array size missing");
        };
        if bytes.len() as u64 > len {
            return self.err(line, column, "initializer string is too long for the array");
        }
        let elem = ty.array_elem().expect("character array").clone();
        let elem_size = elem.size().expect("character element");
        let mut data = bytes.to_vec();
        if (data.len() as u64) < len {
            data.push(0);
        }
        for (index, byte) in data.iter().enumerate() {
            if index as u64 >= len {
                break;
            }
            stores.push(ElemStore {
                offset: base + index as u64 * elem_size,
                ty: elem.clone(),
                expr: Expr::int(u64::from(*byte), TypeName::Int, line, column),
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
        let mut folded = Vec::new();
        for store in stores {
            folded.push((store.offset, self.fold_one(name, &store.expr, &store.ty)?));
        }
        Ok(assemble(ty, &folded, 0))
    }

    fn fold_one(&self, name: &str, expr: &Expr, dest: &CType) -> Result<ConstInit, Diagnostic> {
        if let Some(dest_int) = dest.as_integer() {
            if let Ok(value) = eval_const_expr(expr) {
                return Ok(ConstInit::Int(convert_bits(value.bits, value.ty, dest_int)));
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
                if self.loop_depth == 0 {
                    return self.err(*line, *column, "'break' is not inside a loop");
                }
                Ok(())
            }
            Stmt::Continue { line, column } => {
                if self.loop_depth == 0 {
                    return self.err(*line, *column, "'continue' is not inside a loop");
                }
                Ok(())
            }
        }
    }

    fn check_decl(&mut self, decl: &mut VarDecl) -> Result<(), Diagnostic> {
        if self.lookup_local(&decl.name).is_some() {
            return self.err(
                decl.line,
                decl.column,
                format!("redefinition of '{}'", decl.name),
            );
        }
        let init = decl.init.clone();
        let (ty, stores) = if let Some(init) = &init {
            self.shape_initializer(&decl.ty, init, decl.line, decl.column)?
        } else {
            (decl.ty.clone(), Vec::new())
        };
        if !ty.is_complete() {
            let message = if ty.is_array() {
                format!("array size missing in '{}'", decl.name)
            } else {
                format!(
                    "variable '{}' has incomplete type '{}'",
                    decl.name,
                    ty.display()
                )
            };
            return self.err(decl.line, decl.column, message);
        }
        decl.ty = ty.clone();
        // The declarator is in scope for its initializer (C scope rules).
        self.insert(Symbol::Var(VarSym {
            name: decl.name.clone(),
            slot: decl.slot.clone(),
            ty: ty.clone(),
            global: None,
        }));
        if decl.init.is_some() {
            let mut stores = stores;
            self.check_stores(&mut stores)?;
            decl.checked = Some(classify_init(&ty, stores));
        }
        Ok(())
    }

    fn check_expr(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        if let Expr::Var {
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
                *expr = Expr::str_lit(bytes, line, column);
            }
        }
        match expr {
            Expr::Int { value_ty, .. } => {
                let value_ty = *value_ty;
                expr.set_ty(CType::from_name(value_ty));
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
            Expr::Str { .. } => self.check_str(expr),
            Expr::Member { .. } => self.check_member(expr),
        }
    }

    /// A string literal has type `char[N]`, counting the terminating NUL.
    /// The private global is created when the address is used, not for
    /// `sizeof` and not when the literal only initializes a character array.
    fn check_str(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let len = match expr {
            Expr::Str { bytes, .. } => bytes.len() as u64 + 1,
            _ => unreachable!("string literal"),
        };
        expr.set_ty(CType::array(CType::from_name(TypeName::Char), Some(len)));
        Ok(())
    }

    /// Record a string literal the first time its address is needed. Identical
    /// contents share one private global.
    fn ensure_string(&mut self, expr: &mut Expr) {
        let bytes = match expr {
            Expr::Str {
                symbol: None,
                bytes,
                ..
            } => bytes.clone(),
            _ => return,
        };
        let symbol = self.intern_string(&bytes);
        if let Expr::Str { symbol: slot, .. } = expr {
            *slot = Some(symbol);
        }
    }

    fn intern_string(&mut self, bytes: &[u8]) -> String {
        let mut key = bytes.to_vec();
        key.push(0);
        if let Some((_, name)) = self.strings.iter().find(|(stored, _)| stored == &key) {
            return name.clone();
        }
        let name = if self.strings.is_empty() {
            ".str".to_string()
        } else {
            format!(".str.{}", self.strings.len())
        };
        self.strings.push((key, name.clone()));
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
            Symbol::Func(func) => (
                format!("@{}", func.symbol),
                CType::func(func.ret, func.params, func.variadic),
            ),
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
            } else {
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
            }
        } else {
            unreachable!("binary expression");
        };
        expr.set_ty(ty);
        Ok(())
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
        if left_ty.is_integer() && right_ty.is_integer() {
            return Ok(CType::int_type());
        }
        let equality = matches!(op, BinOp::Eq | BinOp::Ne);
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
                UnaryOp::Plus | UnaryOp::Minus | UnaryOp::BitNot => {
                    self.decay(expr)?;
                    self.require_integer(expr)?;
                    CType::from_name(integer_ty(expr).promote())
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
                    self.check_expr(inner)?;
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
                let ok = (ty_name.is_integer() && src.is_integer())
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
            Symbol::Func(func) => (func.symbol, func.ret, func.params, func.variadic),
            Symbol::Var(_) => unreachable!("direct call is a function"),
        };
        if ret.unqualified().is_record() {
            return self.err(
                line,
                column,
                "returning a struct or union by value is not supported",
            );
        }
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
        if func.ret.unqualified().is_record() {
            return self.err(
                line,
                column,
                "returning a struct or union by value is not supported",
            );
        }
        let argc = args.len();
        let arity = func.params.len();
        if func.variadic {
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
        let variadic = func.variadic;
        if let Expr::Call { args, .. } = expr {
            for (index, arg) in args.iter_mut().enumerate() {
                self.check_expr(arg)?;
                self.decay(arg)?;
                if index < params.len() {
                    self.can_assign(&params[index], arg)?;
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
        if ty.is_float() {
            return self.err(
                expr.line(),
                expr.column(),
                "floating-point expressions are not supported",
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
            let Some((found_index, found)) = handle.field(&field_name) else {
                return self.err(
                    line,
                    column,
                    format!(
                        "no member named '{field_name}' in 'struct {}'",
                        handle.tag()
                    ),
                );
            };
            *index = Some(found_index);
            *offset = Some(found.offset);
            *bytewise = handle.is_union();
            // The struct object's qualifiers apply to the member (C11 6.5.2.3).
            found.ty.with_quals(base_ty.quals())
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
        if dest.is_float() || src_ty.is_float() {
            return self.err(
                src.line(),
                src.column(),
                "floating-point expressions are not supported",
            );
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
            Some(ty) if ty.is_float() => self.err(
                expr.line(),
                expr.column(),
                "floating-point expressions are not supported",
            ),
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
        for scope in self.scopes.iter().rev() {
            if let Some(sym) = scope.get(name) {
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

fn integer_ty(expr: &Expr) -> TypeName {
    expr.ty()
        .and_then(|ty| ty.as_integer())
        .expect("integer expression")
}

fn init_pos(init: &Init) -> (usize, usize) {
    match init {
        Init::Expr(expr) => (expr.line(), expr.column()),
        Init::List { line, column, .. } => (*line, *column),
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

/// Pointers to qualified or unqualified versions of compatible types
/// (C11 6.5.8, 6.5.9). One level of pointee qualifiers is ignored.
/// `const int *` may be compared with `int *`. `char **` may not be compared
/// with `const char **`, because `char *` and `const char *` are not
/// compatible.
fn loose_pointers(left: &CType, right: &CType) -> bool {
    match (left.pointee(), right.pointee()) {
        (Some(a), Some(b)) => compatible(&a.unqualified(), &b.unqualified()),
        _ => false,
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

fn is_char_array(ty: &CType) -> bool {
    ty.array_elem().is_some_and(CType::is_character)
}

/// Bytes of a string literal used as an initializer, including a single pair
/// of braces around that literal (`char s[4] = {"hi"}`).
fn string_payload(init: &Init) -> Option<&[u8]> {
    match init {
        Init::Expr(Expr::Str { bytes, .. }) => Some(bytes),
        Init::List { items, .. } => match items.as_slice() {
            [Init::Expr(Expr::Str { bytes, .. })] => Some(bytes),
            _ => None,
        },
        Init::Expr(_) => None,
    }
}

fn void_ptr_mix(left: &CType, right: &CType) -> bool {
    left.is_pointer() && right.is_pointer() && (left.is_void_pointer() || right.is_void_pointer())
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
    string_payload(init).is_some() && is_char_array(ty)
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

fn assemble(ty: &CType, stores: &[(u64, ConstInit)], base: u64) -> ConstInit {
    if ty.is_scalar() {
        return stores
            .iter()
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
        | Expr::Member { .. } => Err(ConstError::NotConst),
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
            *next += 1;
            decl.slot = format!("%v{next}");
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
        Stmt::While { body, .. } => number_stmt(body, next),
        Stmt::For { init, body, .. } => {
            if let ForInit::Decl(decls) = init {
                for decl in decls {
                    *next += 1;
                    decl.slot = format!("%v{next}");
                }
            }
            number_stmt(body, next);
        }
        Stmt::Return { .. } | Stmt::Expr { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => {}
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
        assert!(sema_err("int main(void) { break; }").contains("'break' is not inside a loop"));
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

        assert!(
            sema_err("struct S { int x; }; void f(struct S s) { (void)s; }")
                .contains("passing a struct by value is not supported")
        );
        assert!(sema_err("struct S { int x; }; int f(struct S);")
            .contains("passing a struct by value is not supported"));
        assert!(sema_err("struct S { struct S { int x; } in; int y; };")
            .contains("redefinition of struct 'S'"));
        assert!(sema_err(
            "struct S { int x; }; int main(void) { struct S s; struct S t; (s = t).x = 1; return 0; }"
        )
        .contains("expression is not assignable"));
        assert!(
            sema_err("struct S { int x; }; struct S f(void) { struct S s; return s; }")
                .contains("returning a struct by value is not supported")
        );
        assert!(sema_err("struct S; struct S s;").contains("incomplete type 'struct S'"));
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
        assert!(sema_err("struct S { int n; int a[]; };").contains("flexible array"));
        assert!(sema_err("struct S { int x : 3; };").contains("bit-fields"));
        sema_ok("struct { int x; } s; int main(void) { s.x = 1; return s.x; }");
        assert!(sema_err("struct S {};").contains("no members"));
        assert!(
            sema_err("struct S { int x; int y; }; struct S s = { .y = 1 };")
                .contains("designated initializers")
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
        assert!(sema_err("int main(void) { float f; f = 1; return 0; }")
            .contains("floating-point expressions are not supported"));
        assert!(
            sema_err("union U { int x; char c; }; union U u = {1, 2};").contains("excess elements")
        );
        assert!(sema_err(
            "struct S { int x; }; struct S f(void); int main(void) { return f().x; }"
        )
        .contains("returning a struct or union by value"));
        assert!(sema_err("enum E { A = 1, A = 2 };").contains("redefinition of 'A'"));
    }
}
