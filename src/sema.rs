//! Name resolution and type checks for the C subset.
//!
//! Scopes follow C block scope. The parameter list of a function definition
//! shares a scope with the function's outermost block, so a parameter cannot
//! be redeclared there. A name is visible in its own initializer.

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diag::Diagnostic;

pub fn analyze(unit: &mut TranslationUnit, filename: &str) -> Result<(), Diagnostic> {
    for func in &mut unit.functions {
        assign_slots(func);
    }
    let mut sema = Sema {
        filename: filename.to_string(),
        scopes: vec![HashMap::new()],
        loop_depth: 0,
        current_ret: TypeName::Int,
    };
    // A definition is visible only after its declarator, through the end of
    // the file. Calls to functions defined later are undeclared; prototypes
    // are not accepted, so each function must be defined before it is called.
    // The function's own name is in scope for its body, which allows recursion.
    for func in &mut unit.functions {
        sema.declare_function(func)?;
        sema.check_function(func)?;
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SymKind {
    Var,
    Func,
}

#[derive(Clone)]
struct Symbol {
    kind: SymKind,
    name: String,
    slot: String,
    ret: TypeName,
    arity: usize,
}

struct Sema {
    filename: String,
    scopes: Vec<HashMap<String, Symbol>>,
    loop_depth: u32,
    current_ret: TypeName,
}

impl Sema {
    fn declare_function(&mut self, func: &FuncDef) -> Result<(), Diagnostic> {
        if self.lookup_local(&func.name).is_some() {
            return self.err(
                func.line,
                func.column,
                format!("redefinition of '{}'", func.name),
            );
        }
        let mut seen = HashSet::new();
        for param in &func.params {
            if !seen.insert(param.name.clone()) {
                return self.err(
                    param.line,
                    param.column,
                    format!("redefinition of parameter '{}'", param.name),
                );
            }
        }
        self.insert(Symbol {
            kind: SymKind::Func,
            name: func.name.clone(),
            slot: String::new(),
            ret: func.ret_type,
            arity: func.params.len(),
        });
        Ok(())
    }

    fn check_function(&mut self, func: &mut FuncDef) -> Result<(), Diagnostic> {
        self.current_ret = func.ret_type;
        self.push();
        for param in &func.params {
            self.insert(Symbol {
                kind: SymKind::Var,
                name: param.name.clone(),
                slot: param.slot.clone(),
                ret: TypeName::Int,
                arity: 0,
            });
        }
        self.check_compound(&mut func.body, false)?;
        self.pop();
        Ok(())
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
        self.insert(Symbol {
            kind: SymKind::Var,
            name: decl.name.clone(),
            slot: decl.slot.clone(),
            ret: TypeName::Int,
            arity: 0,
        });
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
        if sym.kind != SymKind::Var {
            return self.err(
                line,
                column,
                format!("function '{name}' used as a value is not supported"),
            );
        }
        if let Expr::Var { resolved, .. } = expr {
            *resolved = Some(VarRes { slot: sym.slot });
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
        if sym.kind != SymKind::Func {
            return self.err(
                line,
                column,
                format!("called object '{name}' is not a function"),
            );
        }
        if argc != sym.arity {
            return self.err(
                line,
                column,
                format!(
                    "function '{name}' expects {} {}, found {argc}",
                    sym.arity,
                    if sym.arity == 1 {
                        "argument"
                    } else {
                        "arguments"
                    },
                ),
            );
        }
        if let Expr::Call { args, .. } = expr {
            for arg in args {
                self.check_expr(arg)?;
                self.require_int(arg)?;
            }
        }
        if let Expr::Call { resolved, .. } = expr {
            *resolved = Some(CallRes {
                name: sym.name,
                ret: sym.ret,
                arity: sym.arity,
            });
        }
        expr.set_ty(sym.ret);
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
        if sym.kind != SymKind::Var {
            return self.err(line, column, "expression is not assignable");
        }
        if let Expr::Var { resolved, .. } = expr {
            *resolved = Some(VarRes { slot: sym.slot });
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
        let name = sym.name.clone();
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

fn assign_slots(func: &mut FuncDef) {
    let mut next = 0u32;
    for (index, param) in func.params.iter_mut().enumerate() {
        next += 1;
        param.slot = format!("%v{next}");
        param.arg_reg = format!("%p{index}");
    }
    number_compound(&mut func.body, &mut next);
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
}
