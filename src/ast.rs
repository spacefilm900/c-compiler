//! Abstract syntax for the C subset accepted by this milestone.
//!
//! Names are resolved later. `slot` fields are filled before semantic analysis
//! so each variable has a stable LLVM alloca name.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeName {
    Int,
    Void,
}

impl TypeName {
    pub fn as_str(self) -> &'static str {
        match self {
            TypeName::Int => "int",
            TypeName::Void => "void",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
}

impl BinOp {
    pub fn as_str(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::Le => "<=",
            BinOp::Ge => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
        }
    }

    pub fn is_cmp(self) -> bool {
        matches!(
            self,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarRes {
    pub slot: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallRes {
    pub name: String,
    pub ret: TypeName,
    pub arity: usize,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Int {
        value: i32,
        line: usize,
        column: usize,
        ty: Option<TypeName>,
    },
    Var {
        name: String,
        line: usize,
        column: usize,
        resolved: Option<VarRes>,
        ty: Option<TypeName>,
    },
    Bin {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<TypeName>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<TypeName>,
    },
    Assign {
        target: Box<Expr>,
        value: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<TypeName>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        line: usize,
        column: usize,
        resolved: Option<CallRes>,
        ty: Option<TypeName>,
    },
}

impl Expr {
    pub fn int(value: i32, line: usize, column: usize) -> Self {
        Expr::Int {
            value,
            line,
            column,
            ty: None,
        }
    }

    pub fn var(name: String, line: usize, column: usize) -> Self {
        Expr::Var {
            name,
            line,
            column,
            resolved: None,
            ty: None,
        }
    }

    pub fn bin(op: BinOp, left: Expr, right: Expr, line: usize, column: usize) -> Self {
        Expr::Bin {
            op,
            left: Box::new(left),
            right: Box::new(right),
            line,
            column,
            ty: None,
        }
    }

    pub fn unary(op: UnaryOp, expr: Expr, line: usize, column: usize) -> Self {
        Expr::Unary {
            op,
            expr: Box::new(expr),
            line,
            column,
            ty: None,
        }
    }

    pub fn assign(target: Expr, value: Expr, line: usize, column: usize) -> Self {
        Expr::Assign {
            target: Box::new(target),
            value: Box::new(value),
            line,
            column,
            ty: None,
        }
    }

    pub fn call(callee: Expr, args: Vec<Expr>, line: usize, column: usize) -> Self {
        Expr::Call {
            callee: Box::new(callee),
            args,
            line,
            column,
            resolved: None,
            ty: None,
        }
    }

    pub fn line(&self) -> usize {
        match self {
            Expr::Int { line, .. }
            | Expr::Var { line, .. }
            | Expr::Bin { line, .. }
            | Expr::Unary { line, .. }
            | Expr::Assign { line, .. }
            | Expr::Call { line, .. } => *line,
        }
    }

    pub fn column(&self) -> usize {
        match self {
            Expr::Int { column, .. }
            | Expr::Var { column, .. }
            | Expr::Bin { column, .. }
            | Expr::Unary { column, .. }
            | Expr::Assign { column, .. }
            | Expr::Call { column, .. } => *column,
        }
    }

    pub fn ty(&self) -> Option<TypeName> {
        match self {
            Expr::Int { ty, .. }
            | Expr::Var { ty, .. }
            | Expr::Bin { ty, .. }
            | Expr::Unary { ty, .. }
            | Expr::Assign { ty, .. }
            | Expr::Call { ty, .. } => *ty,
        }
    }

    pub fn set_ty(&mut self, ty: TypeName) {
        match self {
            Expr::Int { ty: slot, .. }
            | Expr::Var { ty: slot, .. }
            | Expr::Bin { ty: slot, .. }
            | Expr::Unary { ty: slot, .. }
            | Expr::Assign { ty: slot, .. }
            | Expr::Call { ty: slot, .. } => *slot = Some(ty),
        }
    }

    pub fn var_slot(&self) -> Option<&str> {
        match self {
            Expr::Var {
                resolved: Some(res),
                ..
            } => Some(&res.slot),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub name: String,
    pub init: Option<Expr>,
    pub line: usize,
    pub column: usize,
    pub slot: String,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub line: usize,
    pub column: usize,
    pub slot: String,
    pub arg_reg: String,
}

#[derive(Debug, Clone)]
pub enum ForInit {
    Empty,
    Decl(Vec<VarDecl>),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Compound(Compound),
    Decl(VarDecl),
    Return {
        expr: Option<Expr>,
        line: usize,
        column: usize,
    },
    If {
        cond: Expr,
        then_stmt: Box<Stmt>,
        else_stmt: Option<Box<Stmt>>,
        line: usize,
        column: usize,
    },
    While {
        cond: Expr,
        body: Box<Stmt>,
        line: usize,
        column: usize,
    },
    For {
        init: ForInit,
        cond: Option<Expr>,
        step: Option<Expr>,
        body: Box<Stmt>,
        line: usize,
        column: usize,
    },
    Expr {
        expr: Option<Expr>,
        line: usize,
        column: usize,
    },
    Break {
        line: usize,
        column: usize,
    },
    Continue {
        line: usize,
        column: usize,
    },
}

#[derive(Debug, Clone)]
pub struct Compound {
    pub items: Vec<Stmt>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct FuncDef {
    pub ret_type: TypeName,
    pub name: String,
    pub params: Vec<Param>,
    pub body: Compound,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct TranslationUnit {
    pub functions: Vec<FuncDef>,
}

pub fn dump(unit: &TranslationUnit) -> String {
    let mut out = String::new();
    for func in &unit.functions {
        dump_func(&mut out, func, 0);
    }
    out
}

fn put(out: &mut String, indent: usize, text: &str) {
    for _ in 0..indent {
        out.push_str("  ");
    }
    out.push_str(text);
    out.push('\n');
}

fn dump_func(out: &mut String, func: &FuncDef, indent: usize) {
    put(
        out,
        indent,
        &format!("func {} {}", func.ret_type.as_str(), func.name),
    );
    for param in &func.params {
        put(out, indent + 1, &format!("param {}", param.name));
    }
    dump_compound(out, &func.body, indent + 1);
}

fn dump_compound(out: &mut String, compound: &Compound, indent: usize) {
    put(out, indent, "block");
    for item in &compound.items {
        dump_stmt(out, item, indent + 1);
    }
}

fn dump_stmt(out: &mut String, stmt: &Stmt, indent: usize) {
    match stmt {
        Stmt::Compound(compound) => dump_compound(out, compound, indent),
        Stmt::Decl(decl) => {
            if let Some(init) = &decl.init {
                put(out, indent, &format!("decl {} =", decl.name));
                dump_expr(out, init, indent + 1);
            } else {
                put(out, indent, &format!("decl {}", decl.name));
            }
        }
        Stmt::Return { expr, .. } => {
            put(out, indent, "return");
            if let Some(expr) = expr {
                dump_expr(out, expr, indent + 1);
            }
        }
        Stmt::If {
            cond,
            then_stmt,
            else_stmt,
            ..
        } => {
            put(out, indent, "if");
            put(out, indent + 1, "cond");
            dump_expr(out, cond, indent + 2);
            put(out, indent + 1, "then");
            dump_stmt(out, then_stmt, indent + 2);
            if let Some(else_stmt) = else_stmt {
                put(out, indent + 1, "else");
                dump_stmt(out, else_stmt, indent + 2);
            }
        }
        Stmt::While { cond, body, .. } => {
            put(out, indent, "while");
            put(out, indent + 1, "cond");
            dump_expr(out, cond, indent + 2);
            put(out, indent + 1, "body");
            dump_stmt(out, body, indent + 2);
        }
        Stmt::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            put(out, indent, "for");
            match init {
                ForInit::Empty => put(out, indent + 1, "init"),
                ForInit::Expr(expr) => {
                    put(out, indent + 1, "init");
                    dump_expr(out, expr, indent + 2);
                }
                ForInit::Decl(decls) => {
                    put(out, indent + 1, "init");
                    for decl in decls {
                        if let Some(init) = &decl.init {
                            put(out, indent + 2, &format!("decl {} =", decl.name));
                            dump_expr(out, init, indent + 3);
                        } else {
                            put(out, indent + 2, &format!("decl {}", decl.name));
                        }
                    }
                }
            }
            put(out, indent + 1, "cond");
            if let Some(cond) = cond {
                dump_expr(out, cond, indent + 2);
            }
            put(out, indent + 1, "step");
            if let Some(step) = step {
                dump_expr(out, step, indent + 2);
            }
            put(out, indent + 1, "body");
            dump_stmt(out, body, indent + 2);
        }
        Stmt::Expr { expr, .. } => {
            put(out, indent, "expr");
            if let Some(expr) = expr {
                dump_expr(out, expr, indent + 1);
            }
        }
        Stmt::Break { .. } => put(out, indent, "break"),
        Stmt::Continue { .. } => put(out, indent, "continue"),
    }
}

fn dump_expr(out: &mut String, expr: &Expr, indent: usize) {
    match expr {
        Expr::Int { value, .. } => put(out, indent, &format!("int {value}")),
        Expr::Var { name, .. } => put(out, indent, &format!("var {name}")),
        Expr::Bin {
            op, left, right, ..
        } => {
            put(out, indent, op.as_str());
            dump_expr(out, left, indent + 1);
            dump_expr(out, right, indent + 1);
        }
        Expr::Unary { op, expr, .. } => {
            let spelling = match op {
                UnaryOp::Plus => "pos",
                UnaryOp::Minus => "neg",
                UnaryOp::Not => "not",
            };
            put(out, indent, spelling);
            dump_expr(out, expr, indent + 1);
        }
        Expr::Assign { target, value, .. } => {
            put(out, indent, "assign");
            dump_expr(out, target, indent + 1);
            dump_expr(out, value, indent + 1);
        }
        Expr::Call { callee, args, .. } => {
            match callee.as_ref() {
                Expr::Var { name, .. } => put(out, indent, &format!("call {name}")),
                other => {
                    put(out, indent, "call");
                    dump_expr(out, other, indent + 1);
                }
            }
            for arg in args {
                dump_expr(out, arg, indent + 1);
            }
        }
    }
}
