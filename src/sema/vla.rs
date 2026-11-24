//! Variable-length arrays and variably modified types (C11 6.7.6, 6.5.3.4,
//! 6.8.6.1, 6.8.4.2).

use super::*;

impl Sema<'_> {
    /// Type-check every size expression in `ty`, in innermost-first order.
    /// Each expression is checked once.
    pub(super) fn check_vla_sizes(
        &mut self,
        ty: &CType,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        match ty.peel() {
            CType::Array(elem, len) => {
                self.check_vla_sizes(elem, line, column)?;
                if let ArrayLen::Vla(id) = *len {
                    self.check_vla_id(id, line, column)?;
                }
            }
            CType::Pointer(inner) => self.check_vla_sizes(inner, line, column)?,
            CType::Func(func) => {
                self.check_vla_sizes(&func.ret, line, column)?;
                for param in &func.params {
                    self.check_vla_sizes(param, line, column)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn check_vla_id(&mut self, id: VlaId, line: usize, column: usize) -> Result<(), Diagnostic> {
        if !self.vla_checked.insert(id) {
            return Ok(());
        }
        let Some(mut expr) = self.vla_bounds.get(&id).cloned() else {
            return self.err(
                line,
                column,
                "internal error: missing size expression for a variable length array",
            );
        };
        self.check_expr(&mut expr)?;
        self.decay(&mut expr)?;
        let ty = expr.ty().expect("typed size expression");
        if !ty.is_integer() {
            return self.err(
                expr.line(),
                expr.column(),
                "array size expression must have integer type",
            );
        }
        self.vla_bounds.insert(id, expr);
        Ok(())
    }

    /// C11 6.8.6.1p1 and 6.8.4.2p2: a jump must not enter the scope of a
    /// variably modified identifier.
    pub(super) fn check_vm_jumps(&mut self, body: &Compound) -> Result<(), Diagnostic> {
        let mut walk = VmWalk::default();
        walk.compound(body);
        for goto in &walk.gotos {
            let Some(label) = walk.labels.get(&goto.name) else {
                continue;
            };
            if let Some(ident) = vm_entered(&walk.idents, &goto.at, label) {
                return self.err(
                    goto.line,
                    goto.column,
                    format!(
                        "goto jumps into the scope of variably modified '{}'",
                        ident.name
                    ),
                );
            }
        }
        for switch in &walk.switches {
            for label in &switch.cases {
                if let Some(ident) = vm_entered(&walk.idents, &switch.at, label) {
                    return self.err(
                        ident.line,
                        ident.column,
                        format!(
                            "switch jumps into the scope of variably modified '{}'",
                            ident.name
                        ),
                    );
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
struct Pos {
    scopes: Vec<u32>,
    time: u32,
}

struct VmIdent {
    name: String,
    scope: u32,
    time: u32,
    line: usize,
    column: usize,
}

struct GotoRec {
    name: String,
    at: Pos,
    line: usize,
    column: usize,
}

struct SwitchRec {
    at: Pos,
    cases: Vec<Pos>,
}

#[derive(Default)]
struct VmWalk {
    scopes: Vec<u32>,
    next_scope: u32,
    time: u32,
    idents: Vec<VmIdent>,
    labels: HashMap<String, Pos>,
    gotos: Vec<GotoRec>,
    switches: Vec<SwitchRec>,
}

impl VmWalk {
    fn pos(&self) -> Pos {
        Pos {
            scopes: self.scopes.clone(),
            time: self.time,
        }
    }

    fn tick(&mut self) {
        self.time += 1;
    }

    fn push_scope(&mut self) -> u32 {
        let id = self.next_scope;
        self.next_scope += 1;
        self.scopes.push(id);
        id
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn compound(&mut self, compound: &Compound) {
        self.push_scope();
        for item in &compound.items {
            self.stmt(item);
        }
        self.pop_scope();
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Compound(compound) => self.compound(compound),
            Stmt::Decl(decl) => {
                self.tick();
                if decl.ty.is_variably_modified() {
                    if let Some(scope) = self.scopes.last().copied() {
                        self.idents.push(VmIdent {
                            name: decl.name.clone(),
                            scope,
                            time: self.time,
                            line: decl.line,
                            column: decl.column,
                        });
                    }
                }
            }
            Stmt::FuncDecl(_) => self.tick(),
            Stmt::If {
                then_stmt,
                else_stmt,
                ..
            } => {
                self.tick();
                self.stmt(then_stmt);
                if let Some(else_stmt) = else_stmt {
                    self.stmt(else_stmt);
                }
            }
            Stmt::While { body, .. } | Stmt::Do { body, .. } => {
                self.tick();
                self.stmt(body);
            }
            Stmt::For { init, body, .. } => {
                self.tick();
                self.push_scope();
                if let ForInit::Decl(decls) = init {
                    for decl in decls {
                        self.tick();
                        if decl.ty.is_variably_modified() {
                            if let Some(scope) = self.scopes.last().copied() {
                                self.idents.push(VmIdent {
                                    name: decl.name.clone(),
                                    scope,
                                    time: self.time,
                                    line: decl.line,
                                    column: decl.column,
                                });
                            }
                        }
                    }
                }
                self.stmt(body);
                self.pop_scope();
            }
            Stmt::Switch { body, .. } => {
                self.tick();
                let mut rec = SwitchRec {
                    at: self.pos(),
                    cases: Vec::new(),
                };
                self.switch_body(body, &mut rec);
                self.switches.push(rec);
            }
            Stmt::Case { stmt, .. } | Stmt::Default { stmt, .. } => {
                // A case not walked through `switch_body` still belongs to
                // an enclosing switch, which records it there.
                self.stmt(stmt);
            }
            Stmt::Label { name, stmt, .. } => {
                self.tick();
                self.labels.insert(name.clone(), self.pos());
                self.stmt(stmt);
            }
            Stmt::Goto {
                name, line, column, ..
            } => {
                self.tick();
                self.gotos.push(GotoRec {
                    name: name.clone(),
                    at: self.pos(),
                    line: *line,
                    column: *column,
                });
            }
            Stmt::GotoPtr { .. }
            | Stmt::Asm { .. }
            | Stmt::Return { .. }
            | Stmt::Expr { .. }
            | Stmt::Break { .. }
            | Stmt::Continue { .. } => {
                self.tick();
            }
        }
    }

    fn switch_body(&mut self, stmt: &Stmt, rec: &mut SwitchRec) {
        match stmt {
            Stmt::Compound(compound) => {
                self.push_scope();
                for item in &compound.items {
                    self.switch_body(item, rec);
                }
                self.pop_scope();
            }
            Stmt::Case { stmt, .. } | Stmt::Default { stmt, .. } => {
                self.tick();
                rec.cases.push(self.pos());
                self.switch_body(stmt, rec);
            }
            Stmt::Label { name, stmt, .. } => {
                self.tick();
                self.labels.insert(name.clone(), self.pos());
                self.switch_body(stmt, rec);
            }
            Stmt::If {
                then_stmt,
                else_stmt,
                ..
            } => {
                self.switch_body(then_stmt, rec);
                if let Some(else_stmt) = else_stmt {
                    self.switch_body(else_stmt, rec);
                }
            }
            Stmt::While { body, .. } | Stmt::Do { body, .. } => self.switch_body(body, rec),
            Stmt::For { init, body, .. } => {
                self.tick();
                self.push_scope();
                if let ForInit::Decl(decls) = init {
                    for decl in decls {
                        self.tick();
                        if decl.ty.is_variably_modified() {
                            if let Some(scope) = self.scopes.last().copied() {
                                self.idents.push(VmIdent {
                                    name: decl.name.clone(),
                                    scope,
                                    time: self.time,
                                    line: decl.line,
                                    column: decl.column,
                                });
                            }
                        }
                    }
                }
                self.switch_body(body, rec);
                self.pop_scope();
            }
            Stmt::Switch { .. } => self.stmt(stmt),
            other => self.stmt(other),
        }
    }
}

/// A VM identifier whose scope contains `into` but not `from`.
fn vm_entered<'a>(idents: &'a [VmIdent], from: &Pos, into: &Pos) -> Option<&'a VmIdent> {
    idents
        .iter()
        .find(|ident| in_scope(ident, into) && !in_scope(ident, from))
}

fn in_scope(ident: &VmIdent, pos: &Pos) -> bool {
    pos.scopes.contains(&ident.scope) && pos.time > ident.time
}
