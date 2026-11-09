//! Name resolution and type checks for the C subset.
//!
//! Scopes follow C block scope. The parameter list of a function definition
//! shares a scope with the function's outermost block, so a parameter cannot
//! be redeclared there. A name is visible in its own initializer.
//!
//! File-scope objects and functions follow C11 6.2.2 linkage and 6.9.2
//! tentative definitions. A prototype makes a function visible before its
//! body, which is what mutual recursion needs. A global initializer must be
//! an integer constant expression (C11 6.6). Assignment, `++`/`--`, calls,
//! and the comma operator are rejected there, including when they sit in an
//! evaluated operand. An unevaluated operand (`sizeof`, or the skipped arm
//! of `&&`, `||`, or `?:`) may contain them.

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diag::Diagnostic;

pub fn analyze(unit: &mut TranslationUnit, filename: &str) -> Result<(), Diagnostic> {
    for decl in &mut unit.decls {
        if let TopDecl::Func(func) = decl {
            assign_slots(func);
        }
    }
    let mut sema = Sema {
        filename: filename.to_string(),
        scopes: vec![HashMap::new()],
        loop_depth: 0,
        current_ret: TypeName::Int,
        global_order: Vec::new(),
        func_order: Vec::new(),
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

#[derive(Clone, Copy)]
enum GlobalState {
    /// `extern` with no initializer and no tentative definition yet.
    Declared,
    /// `int g;` or `static int g;` with no explicit initializer yet.
    Tentative,
    Defined(i32),
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
    /// `None` for a block-scope variable, which has no linkage.
    global: Option<GlobalInfo>,
}

#[derive(Clone)]
struct FuncSym {
    name: String,
    ret: TypeName,
    arity: usize,
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
    scopes: Vec<HashMap<String, Symbol>>,
    loop_depth: u32,
    current_ret: TypeName,
    global_order: Vec<String>,
    func_order: Vec<String>,
}

impl Sema {
    fn declare_global(&mut self, decl: &mut GlobalDecl) -> Result<(), Diagnostic> {
        let name = decl.name.clone();
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
                global: Some(GlobalInfo { linkage, state }),
            }));
        }
        if decl.init.is_some() {
            if matches!(self.global_state(&name), Some(GlobalState::Defined(_))) {
                return self.err(decl.line, decl.column, format!("redefinition of '{name}'"));
            }
            let init = decl.init.as_mut().expect("initializer");
            self.check_expr(init)?;
            self.require_int(init)?;
            let value = self.fold_initializer(&name, init)?;
            self.set_global_state(&name, GlobalState::Defined(value));
        } else if decl.storage != Storage::Extern {
            self.upgrade_tentative(&name);
        }
        Ok(())
    }

    fn declare_func(&mut self, func: &mut FuncDef) -> Result<(), Diagnostic> {
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
                    ret: func.ret_type,
                    arity: func.params.len(),
                    linkage,
                    defined: is_def,
                    used: false,
                    use_line: 0,
                    use_column: 0,
                }));
            }
            Some(Symbol::Func(existing)) => {
                if existing.ret != func.ret_type || existing.arity != func.params.len() {
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
            }
            Some(Symbol::Var(_)) => unreachable!("variable rejected above"),
        }
        Ok(())
    }

    fn check_function(&mut self, func: &mut FuncDef) -> Result<(), Diagnostic> {
        self.current_ret = func.ret_type;
        self.push();
        for param in &func.params {
            let Some(name) = param.name.clone() else {
                continue;
            };
            self.insert(Symbol::Var(VarSym {
                name,
                slot: param.slot.clone(),
                global: None,
            }));
        }
        let body = func.body.as_mut().expect("defined function");
        self.check_compound(body, false)?;
        self.pop();
        Ok(())
    }

    fn finish(&mut self, unit: &mut TranslationUnit) -> Result<(), Diagnostic> {
        let mut missing_static = None;
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
                        name: func.name.clone(),
                        ret: func.ret,
                        arity: func.arity,
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
                    let init = match info.state {
                        GlobalState::Declared => None,
                        GlobalState::Tentative => Some(0),
                        GlobalState::Defined(value) => Some(value),
                    };
                    globals.push(IrGlobal {
                        name: var.name.clone(),
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
        unit.func_decls = func_decls;
        unit.globals = globals;
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
            Storage::None => Linkage::External,
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
            Storage::Extern | Storage::None => prior.unwrap_or(Linkage::External),
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

    fn fold_initializer(&self, name: &str, expr: &Expr) -> Result<i32, Diagnostic> {
        match eval_const_expr(expr) {
            Ok(value) => Ok(value),
            Err(ConstError::NotConst) => self.err(
                expr.line(),
                expr.column(),
                format!("initializer of '{name}' is not a constant expression"),
            ),
            Err(ConstError::DivZero) => self.err(
                expr.line(),
                expr.column(),
                format!("initializer of '{name}' divides by zero"),
            ),
            Err(ConstError::Overflow) => self.err(
                expr.line(),
                expr.column(),
                format!("initializer of '{name}' overflows 'int'"),
            ),
            Err(ConstError::Shift) => self.err(
                expr.line(),
                expr.column(),
                format!("initializer of '{name}' uses an undefined shift"),
            ),
        }
    }

    fn file_get(&self, name: &str) -> Option<Symbol> {
        self.scopes.first().expect("file scope").get(name).cloned()
    }

    fn global_state(&self, name: &str) -> Option<GlobalState> {
        match self.scopes.first().expect("file scope").get(name) {
            Some(Symbol::Var(var)) => var.global.as_ref().map(|info| info.state),
            _ => None,
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
                        if self.current_ret != TypeName::Void {
                            return self.err(
                                line,
                                column,
                                "return with no value in a function returning 'int'",
                            );
                        }
                        Ok(())
                    }
                    Some(expr) => {
                        self.check_expr(expr)?;
                        if self.current_ret == TypeName::Void {
                            return self.err(
                                line,
                                column,
                                "return with a value in a function returning 'void'",
                            );
                        }
                        self.require_int(expr)
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
                self.require_int(cond)?;
                self.check_stmt(then_stmt)?;
                if let Some(else_stmt) = else_stmt {
                    self.check_stmt(else_stmt)?;
                }
                Ok(())
            }
            Stmt::While { cond, body, .. } => {
                self.check_expr(cond)?;
                self.require_int(cond)?;
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
                    ForInit::Expr(expr) => self.check_expr(expr)?,
                }
                if let Some(cond) = cond {
                    self.check_expr(cond)?;
                    self.require_int(cond)?;
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
        // The declarator is in scope for its initializer (C scope rules).
        self.insert(Symbol::Var(VarSym {
            name: decl.name.clone(),
            slot: decl.slot.clone(),
            global: None,
        }));
        if let Some(init) = &mut decl.init {
            self.check_expr(init)?;
            self.require_int(init)?;
        }
        Ok(())
    }

    fn check_expr(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        match expr {
            Expr::Int { .. } => {}
            Expr::Var { .. } => return self.check_var(expr),
            Expr::Bin { .. } => return self.check_bin(expr),
            Expr::Unary { .. } => return self.check_unary(expr),
            Expr::Assign { .. } => return self.check_assign(expr),
            Expr::CompoundAssign { .. } => return self.check_compound_assign(expr),
            Expr::IncDec { .. } => return self.check_incdec(expr),
            Expr::Cond { .. } => return self.check_cond(expr),
            Expr::Comma { .. } => return self.check_comma(expr),
            Expr::Sizeof { .. } => return self.check_sizeof(expr),
            Expr::Call { .. } => return self.check_call(expr),
        }
        expr.set_ty(TypeName::Int);
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
        let slot = match sym {
            Symbol::Var(var) => var.slot,
            Symbol::Func(_) => {
                return self.err(
                    line,
                    column,
                    format!("function '{name}' used as a value is not supported"),
                );
            }
        };
        if let Expr::Var { resolved, .. } = expr {
            *resolved = Some(VarRes { slot });
        }
        expr.set_ty(TypeName::Int);
        Ok(())
    }

    fn check_bin(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        if let Expr::Bin { left, right, .. } = expr {
            self.check_expr(left)?;
            self.check_expr(right)?;
            self.require_int(left)?;
            self.require_int(right)?;
        }
        expr.set_ty(TypeName::Int);
        Ok(())
    }

    fn check_unary(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        if let Expr::Unary { expr, .. } = expr {
            self.check_expr(expr)?;
            self.require_int(expr)?;
        }
        expr.set_ty(TypeName::Int);
        Ok(())
    }

    fn check_assign(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        if let Expr::Assign { target, value, .. } = expr {
            self.check_lvalue(target)?;
            self.check_expr(value)?;
            self.require_int(value)?;
        }
        expr.set_ty(TypeName::Int);
        Ok(())
    }

    fn check_compound_assign(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        if let Expr::CompoundAssign { target, value, .. } = expr {
            self.check_lvalue(target)?;
            self.check_expr(value)?;
            self.require_int(value)?;
        }
        expr.set_ty(TypeName::Int);
        Ok(())
    }

    fn check_incdec(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        if let Expr::IncDec { target, .. } = expr {
            self.check_lvalue(target)?;
        }
        expr.set_ty(TypeName::Int);
        Ok(())
    }

    /// Both arms are type-checked. They must both be `int` or both be `void`.
    /// Only the chosen arm is evaluated, which constant folding honors.
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
            self.require_int(cond)?;
            self.check_expr(then_expr)?;
            self.check_expr(else_expr)?;
            match (then_expr.ty(), else_expr.ty()) {
                (Some(ty @ TypeName::Int), Some(TypeName::Int)) => ty,
                (Some(ty @ TypeName::Void), Some(TypeName::Void)) => ty,
                _ => {
                    return self.err(
                        line,
                        column,
                        "incompatible operand types in the conditional operator",
                    );
                }
            }
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
            self.check_expr(right)?;
            right.ty().expect("comma operand is typed")
        } else {
            unreachable!("comma expression");
        };
        expr.set_ty(ty);
        Ok(())
    }

    /// `sizeof` does not evaluate its operand. The result is an `int` with
    /// value 4 until this subset has `size_t`. `void` is incomplete.
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
            match operand {
                SizeofOperand::Type(TypeName::Void) => {
                    return self.err(line, column, "sizeof applied to incomplete type 'void'");
                }
                SizeofOperand::Type(TypeName::Int) => {}
                SizeofOperand::Expr(inner) => {
                    self.check_expr(inner)?;
                    if inner.ty() == Some(TypeName::Void) {
                        return self.err(
                            inner.line(),
                            inner.column(),
                            "sizeof applied to incomplete type 'void'",
                        );
                    }
                }
            }
        }
        expr.set_ty(TypeName::Int);
        Ok(())
    }

    fn check_call(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
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
        let (fname, ret, arity) = match sym {
            Symbol::Func(func) => (func.name, func.ret, func.arity),
            Symbol::Var(_) => {
                return self.err(
                    line,
                    column,
                    format!("called object '{name}' is not a function"),
                );
            }
        };
        if argc != arity {
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
            for arg in args {
                self.check_expr(arg)?;
                self.require_int(arg)?;
            }
        }
        self.note_use(&name, line, column);
        if let Expr::Call { resolved, .. } = expr {
            *resolved = Some(CallRes {
                name: fname,
                ret,
                arity,
            });
        }
        expr.set_ty(ret);
        Ok(())
    }

    fn check_lvalue(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let line = expr.line();
        let column = expr.column();
        let name = match expr {
            Expr::Var { name, .. } => name.clone(),
            _ => return self.err(line, column, "expression is not assignable"),
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
        let slot = match sym {
            Symbol::Var(var) => var.slot,
            Symbol::Func(_) => return self.err(line, column, "expression is not assignable"),
        };
        if let Expr::Var { resolved, .. } = expr {
            *resolved = Some(VarRes { slot });
        }
        expr.set_ty(TypeName::Int);
        Ok(())
    }

    fn require_int(&self, expr: &Expr) -> Result<(), Diagnostic> {
        match expr.ty() {
            Some(TypeName::Int) => Ok(()),
            Some(TypeName::Void) => self.err(
                expr.line(),
                expr.column(),
                "void value used where a value is required",
            ),
            None => self.err(
                expr.line(),
                expr.column(),
                "internal compiler error: expression has no type",
            ),
        }
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
        Err(Diagnostic::new(&self.filename, line, column, message))
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
    Overflow,
    Shift,
}

/// Integer constant expression using the operators this subset implements.
///
/// `&&`, `||`, and `?:` skip the operand that is not evaluated, so
/// `0 && (1/0)` and `0 ? 1/0 : 4` fold without diagnosing the division.
/// `sizeof` is not evaluated and yields 4. Signed `+`, `-`, `*`, and unary
/// `-` must stay inside `int` (C11 6.6p4); so must `INT_MIN / -1` and
/// `INT_MIN % -1`. An evaluated division or remainder by zero is rejected.
///
/// A shift count outside `0..31`, a left shift of a negative value, and a
/// left shift whose mathematical result does not fit in signed 32-bit `int`
/// (including `1 << 31`) are rejected. Right shifts are arithmetic.
/// Assignment, `++`/`--`, calls, and comma are not constant expressions when
/// they are evaluated.
fn eval_const_expr(expr: &Expr) -> Result<i32, ConstError> {
    match expr {
        Expr::Int { value, .. } => Ok(*value),
        Expr::Sizeof { .. } => Ok(4),
        Expr::Unary { op, expr, .. } => {
            let value = eval_const_expr(expr)?;
            match op {
                UnaryOp::Plus => Ok(value),
                UnaryOp::Minus => value.checked_neg().ok_or(ConstError::Overflow),
                UnaryOp::Not => Ok(i32::from(value == 0)),
                UnaryOp::BitNot => Ok(!value),
            }
        }
        Expr::Bin {
            op, left, right, ..
        } => eval_binop(*op, left, right),
        Expr::Cond {
            cond,
            then_expr,
            else_expr,
            ..
        } => {
            if eval_const_expr(cond)? != 0 {
                eval_const_expr(then_expr)
            } else {
                eval_const_expr(else_expr)
            }
        }
        Expr::Var { .. }
        | Expr::Assign { .. }
        | Expr::CompoundAssign { .. }
        | Expr::IncDec { .. }
        | Expr::Call { .. }
        | Expr::Comma { .. } => Err(ConstError::NotConst),
    }
}

fn eval_binop(op: BinOp, left: &Expr, right: &Expr) -> Result<i32, ConstError> {
    match op {
        BinOp::And => {
            if eval_const_expr(left)? == 0 {
                return Ok(0);
            }
            Ok(i32::from(eval_const_expr(right)? != 0))
        }
        BinOp::Or => {
            if eval_const_expr(left)? != 0 {
                return Ok(1);
            }
            Ok(i32::from(eval_const_expr(right)? != 0))
        }
        _ => {
            let left = eval_const_expr(left)?;
            let right = eval_const_expr(right)?;
            match op {
                BinOp::Add => left.checked_add(right).ok_or(ConstError::Overflow),
                BinOp::Sub => left.checked_sub(right).ok_or(ConstError::Overflow),
                BinOp::Mul => left.checked_mul(right).ok_or(ConstError::Overflow),
                BinOp::Div => div_const(left, right),
                BinOp::Mod => rem_const(left, right),
                BinOp::Eq => Ok(i32::from(left == right)),
                BinOp::Ne => Ok(i32::from(left != right)),
                BinOp::Lt => Ok(i32::from(left < right)),
                BinOp::Gt => Ok(i32::from(left > right)),
                BinOp::Le => Ok(i32::from(left <= right)),
                BinOp::Ge => Ok(i32::from(left >= right)),
                BinOp::BitAnd => Ok(left & right),
                BinOp::BitOr => Ok(left | right),
                BinOp::BitXor => Ok(left ^ right),
                BinOp::Shl => shl_const(left, right),
                BinOp::Shr => shr_const(left, right),
                BinOp::And | BinOp::Or => unreachable!("logical operators"),
            }
        }
    }
}

/// C11 left shift is defined for a nonnegative `left` when `left * 2^right`
/// fits in signed `int`. `1 << 31` does not fit. The count must be in `0..31`.
fn shl_const(left: i32, right: i32) -> Result<i32, ConstError> {
    if !(0..32).contains(&right) || left < 0 {
        return Err(ConstError::Shift);
    }
    let shifted = i64::from(left) << right;
    if shifted > i64::from(i32::MAX) {
        return Err(ConstError::Overflow);
    }
    Ok(shifted as i32)
}

/// Signed right shift is arithmetic. The count must be in `0..31`.
fn shr_const(left: i32, right: i32) -> Result<i32, ConstError> {
    if !(0..32).contains(&right) {
        return Err(ConstError::Shift);
    }
    Ok(left >> right)
}

fn div_const(left: i32, right: i32) -> Result<i32, ConstError> {
    if right == 0 {
        return Err(ConstError::DivZero);
    }
    if left == i32::MIN && right == -1 {
        return Err(ConstError::Overflow);
    }
    Ok(left / right)
}

fn rem_const(left: i32, right: i32) -> Result<i32, ConstError> {
    if right == 0 {
        return Err(ConstError::DivZero);
    }
    if left == i32::MIN && right == -1 {
        return Err(ConstError::Overflow);
    }
    Ok(left % right)
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
        let mut unit = parse(&tokens, "neg.c").unwrap();
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
            .contains("called object 'x' is not a function"));
        assert!(
            sema_err("int f(void) { return 1; } int main(void) { return f; }")
                .contains("function 'f' used as a value")
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
        .contains("called object 'id' is not a function"));
        assert!(
            sema_err("int add(int a) { return a; } int main(void) { return add(1, 2)(3); }")
                .contains("only direct function calls are supported")
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
                .init
        };
        assert_eq!(value("g"), Some(7));
        assert_eq!(value("zero"), Some(0));
        assert_eq!(value("missing"), None);
        assert_eq!(value("h"), Some(0));
        assert_eq!(value("m"), Some(i32::MIN));
        assert_eq!(value("d"), Some(-2));
        assert_eq!(value("s"), Some(-1));
        assert_eq!(value("quiet"), Some(0));
        let hidden = unit
            .globals
            .iter()
            .find(|global| global.name == "hidden")
            .unwrap();
        assert_eq!(hidden.linkage, Linkage::Internal);
        assert_eq!(hidden.init, Some(4));

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
        assert_eq!(forward.func_decls[0].arity, 1);
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
                .init
        };
        assert_eq!(value("bits"), Some((6 & 3) | (1 ^ 4)));
        assert_eq!(value("tilde"), Some(-1));
        assert_eq!(value("shl"), Some(16));
        assert_eq!(value("shr"), Some(-4));
        assert_eq!(value("wide"), Some(i32::MIN >> 1));
        assert_eq!(value("pick"), Some(7));
        assert_eq!(value("other"), Some(5));
        assert_eq!(value("quiet_or"), Some(1));
        assert_eq!(value("sz"), Some(4));
        assert_eq!(value("sz_expr"), Some(4));
        assert_eq!(value("self_sz"), Some(4));
        assert_eq!(value("nested"), Some(4));
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
}
