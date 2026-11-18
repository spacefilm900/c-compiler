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

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diag::{Diagnostic, Warning};
use crate::dialect::Dialect;

mod call;
mod consteval;
mod convert;
mod decl;
mod expr;
mod image;
mod init;
mod stmt;
use consteval::*;
use decl::*;
use expr::*;
use image::*;
use init::*;

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
    analyze_dialect(unit, filename, tokens, Dialect::C11).map(|_| ())
}

/// Like [`analyze_tokens`], in the given dialect. Returns the warnings, in
/// source order; only GNU extensions produce them.
pub fn analyze_dialect(
    unit: &mut TranslationUnit,
    filename: &str,
    tokens: &[crate::token::Token],
    dialect: Dialect,
) -> Result<Vec<Warning>, Diagnostic> {
    for decl in &mut unit.decls {
        if let TopDecl::Func(func) = decl {
            assign_slots(func);
        }
    }
    let mut sema = Sema {
        filename: filename.to_string(),
        dialect,
        warnings: RefCell::new(Vec::new()),
        tokens,
        locs: std::cell::OnceCell::new(),
        scopes: vec![HashMap::new()],
        loop_depth: 0,
        switches: Vec::new(),
        switch_outside: false,
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
        unevaluated_depth: 0,
        labels: HashMap::new(),
        dropped_labels: HashMap::new(),
        addr_labels: Vec::new(),
        current_symbol: String::new(),
        variadic_fn: false,
        last_param: None,
        stmt_expr_seq: 0,
        union_choice: Vec::new(),
        overridden: Vec::new(),
        store_drops: 0,
        weak_symbols: unit
            .fn_attrs
            .iter()
            .filter(|(_, attrs)| attrs.weak)
            .map(|(symbol, _)| symbol.clone())
            .chain(
                unit.var_attrs
                    .iter()
                    .filter(|(_, attrs)| attrs.weak)
                    .map(|(symbol, _)| symbol.clone()),
            )
            .collect(),
        library_calls: std::collections::BTreeMap::new(),
        deprecated: unit
            .fn_attrs
            .iter()
            .filter_map(|(symbol, attrs)| {
                attrs
                    .deprecated
                    .clone()
                    .map(|message| (symbol.clone(), message))
            })
            .collect(),
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
    sema.finish(unit)?;
    Ok(sema.warnings.into_inner())
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
    seen: HashSet<u128>,
    /// GNU case ranges, as bit patterns of `promoted`.
    ranges: Vec<(u128, u128)>,
    has_default: bool,
}

/// Side state that checking an expression can record for IR lowering, as
/// it stood before an operand that is not evaluated (C11 6.5.1.1p3,
/// 6.5.3.4p2). Such an operand is checked for its constraints and then
/// dropped, so everything it recorded is rolled back
/// ([`Sema::end_unevaluated`]). Symbols it declares (a block-scope
/// `extern` or function declaration) stay declared, as in C, and emit at
/// most an unused external declaration; warnings stay too.
struct Unevaluated {
    static_locals: usize,
    strings: usize,
    static_seq: u32,
    library_nan_calls: usize,
    library_calls: std::collections::BTreeMap<String, crate::builtins::LibSig>,
    addr_labels: usize,
    labels: HashSet<String>,
}

struct Sema<'t> {
    /// Statement expressions checked so far, for their slot names.
    stmt_expr_seq: u32,
    /// The member each union in the initializer being shaped holds so far:
    /// byte offset of the union, its key, and the member index.
    union_choice: Vec<(u64, UnionKey, usize)>,
    /// Stores of union members that a later designator replaced, in the
    /// initializer being shaped.
    overridden: Vec<ElemStore>,
    /// How many times shaping dropped stores (an overridden union member
    /// or subobject), so a [`init::Reach`] knows to count again.
    store_drops: usize,
    /// Symbols declared `weak`. Their address may be null at run time, so
    /// it does not fold to a truth value.
    weak_symbols: std::collections::HashSet<String>,
    /// C library functions that a `__builtin_` forwarder calls, by symbol.
    library_calls: std::collections::BTreeMap<String, crate::builtins::LibSig>,
    /// Functions declared `deprecated`, by symbol, with the message.
    deprecated: HashMap<String, String>,
    filename: String,
    dialect: Dialect,
    /// Warnings for accepted GNU extensions, in the order they were found.
    warnings: RefCell<Vec<Warning>>,
    /// Last token at each line and column. Included files override the main
    /// file when they share a coordinate; parser diagnostics resolve this
    /// more precisely from the token cursor.
    /// Built from `tokens` on the first diagnostic.
    locs: std::cell::OnceCell<HashMap<(usize, usize), std::rc::Rc<str>>>,
    tokens: &'t [crate::token::Token],
    scopes: Vec<HashMap<String, Symbol>>,
    loop_depth: u32,
    /// Innermost switch first at the end. A `case` or `default` belongs to
    /// `switches.last()`, not to a switch that encloses that one.
    switches: Vec<SwitchCtx>,
    /// Set while checking a statement expression inside a switch. The
    /// switch's labels are hidden there: a switch may not jump into a
    /// statement expression (GCC manual, "Statements and Declarations in
    /// Expressions").
    switch_outside: bool,
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
    /// Nesting depth of operands that are not evaluated: `sizeof`, and a
    /// `_Generic` controlling expression or unselected association.
    unevaluated_depth: u32,
    /// Labels defined in the function being checked. A `goto` names one of them.
    labels: HashMap<String, (usize, usize)>,
    /// Labels of the function being checked that are defined inside an
    /// operand that is not evaluated (`sizeof`, a `_Generic` controlling
    /// expression or unselected association). That code is dropped, so
    /// no `goto` may name them; they still have function scope.
    dropped_labels: HashMap<String, (usize, usize)>,
    /// Labels of the function being checked whose address `&&` takes (GNU),
    /// in order of first use.
    addr_labels: Vec<String>,
    /// Linker name of the function being checked, for `blockaddress`.
    current_symbol: String,
    /// The function being checked is variadic. `va_start` requires that.
    variadic_fn: bool,
    /// Name of the last named parameter, which `va_start` must name.
    last_param: Option<String>,
}

impl Sema<'_> {
    /// The file of the last token at each line and column.
    fn locs(&self) -> &HashMap<(usize, usize), std::rc::Rc<str>> {
        self.locs.get_or_init(|| {
            let mut locs = HashMap::new();
            for token in self.tokens {
                if token.kind != crate::token::TokenKind::Eof {
                    locs.insert((token.line, token.column), token.filename.clone());
                }
            }
            locs
        })
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

    /// A use in an operand that is not evaluated emits no reference, so a
    /// `static` function used only there needs no definition (C11 6.9p3
    /// names `sizeof`; a `_Generic` operand that is not evaluated is the
    /// same, and Clang 19 and GCC 14 accept it).
    fn note_use(&mut self, name: &str, line: usize, column: usize) {
        if self.unevaluated_depth > 0 {
            return;
        }
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

    fn warn(&self, line: usize, column: usize, message: impl Into<String>) {
        let file = self
            .locs()
            .get(&(line, column))
            .map(|file| &**file)
            .unwrap_or(self.filename.as_str());
        self.warnings
            .borrow_mut()
            .push(Warning(Diagnostic::new(file, line, column, message)));
    }

    fn err<T>(
        &self,
        line: usize,
        column: usize,
        message: impl Into<String>,
    ) -> Result<T, Diagnostic> {
        let file = self
            .locs()
            .get(&(line, column))
            .map(|file| &**file)
            .unwrap_or(self.filename.as_str());
        Err(Diagnostic::new(file, line, column, message))
    }

    /// The error for a `case` or `default` label with no switch of its own.
    fn no_switch_message(&self, label: &str) -> String {
        if self.switch_outside {
            format!("'{label}' label jumps into a statement expression from its switch")
        } else {
            format!("'{label}' label is not inside a switch")
        }
    }

    /// An internal compiler error: a case that an earlier check should have
    /// rejected.
    fn ice<T>(&self, line: usize, column: usize, message: &str) -> Result<T, Diagnostic> {
        self.err(line, column, message).map_err(|diag| Diagnostic {
            phase: Some("sema"),
            ..diag
        })
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

/// A call through a type without a prototype passes the promoted arguments
/// as fixed arguments on AArch64, where variadic arguments travel
/// differently (all on the stack for Apple arm64; in their own register
/// save areas for AAPCS64). The callee is defined with fixed parameters,
/// so the call must match that. On x86_64 the call stays variadic, which
/// System V AMD64 passes identically apart from `%al`.
fn fixed_unprototyped_calls() -> bool {
    crate::target::current().is_aarch64()
}

#[cfg(test)]
mod tests;
