//! Textual dump of the syntax tree, used by the parser unit tests.

use super::*;

pub fn dump(unit: &TranslationUnit) -> String {
    let mut out = String::new();
    for decl in &unit.decls {
        match decl {
            TopDecl::Func(func) => dump_func(&mut out, func, 0),
            TopDecl::Global(global) => dump_global(&mut out, global, 0),
        }
    }
    out
}

pub(super) fn put(out: &mut String, indent: usize, text: &str) {
    for _ in 0..indent {
        out.push_str("  ");
    }
    out.push_str(text);
    out.push('\n');
}

pub(super) fn storage_prefix(storage: Storage) -> &'static str {
    match storage {
        Storage::None | Storage::Typedef => "",
        Storage::Static => "static ",
        Storage::Extern => "extern ",
    }
}

pub(super) fn dump_global(out: &mut String, global: &GlobalDecl, indent: usize) {
    let prefix = storage_prefix(global.storage);
    let ty = global.ty.display();
    if let Some(init) = &global.init {
        put(
            out,
            indent,
            &format!("{prefix}global {ty} {} =", global.name),
        );
        dump_init(out, init, indent + 1);
    } else {
        put(out, indent, &format!("{prefix}global {ty} {}", global.name));
    }
}

pub(super) fn dump_func(out: &mut String, func: &FuncDef, indent: usize) {
    let kind = if func.body.is_some() { "func" } else { "proto" };
    put(
        out,
        indent,
        &format!(
            "{}{kind} {} {}",
            storage_prefix(func.storage),
            func.ret_type.display(),
            func.name
        ),
    );
    for param in &func.params {
        match &param.name {
            Some(name) => put(
                out,
                indent + 1,
                &format!("param {} {name}", param.ty.display()),
            ),
            None => put(out, indent + 1, &format!("param {}", param.ty.display())),
        }
    }
    if func.variadic {
        put(out, indent + 1, "varargs");
    }
    if let Some(body) = &func.body {
        dump_compound(out, body, indent + 1);
    }
}

pub(super) fn dump_local(out: &mut String, decl: &VarDecl, indent: usize) {
    let prefix = storage_prefix(decl.storage);
    let ty = decl.ty.display();
    if let Some(init) = &decl.init {
        put(out, indent, &format!("{prefix}decl {ty} {} =", decl.name));
        dump_init(out, init, indent + 1);
    } else {
        put(out, indent, &format!("{prefix}decl {ty} {}", decl.name));
    }
}

pub(super) fn dump_compound(out: &mut String, compound: &Compound, indent: usize) {
    put(out, indent, "block");
    for item in &compound.items {
        dump_stmt(out, item, indent + 1);
    }
}

pub(super) fn dump_stmt(out: &mut String, stmt: &Stmt, indent: usize) {
    match stmt {
        Stmt::Compound(compound) => dump_compound(out, compound, indent),
        Stmt::Decl(decl) => dump_local(out, decl, indent),
        Stmt::FuncDecl(func) => dump_func(out, func, indent),
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
                        dump_local(out, decl, indent + 2);
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
        Stmt::Do { body, cond, .. } => {
            put(out, indent, "do");
            put(out, indent + 1, "body");
            dump_stmt(out, body, indent + 2);
            put(out, indent + 1, "cond");
            dump_expr(out, cond, indent + 2);
        }
        Stmt::Switch { cond, body, .. } => {
            put(out, indent, "switch");
            put(out, indent + 1, "cond");
            dump_expr(out, cond, indent + 2);
            put(out, indent + 1, "body");
            dump_stmt(out, body, indent + 2);
        }
        Stmt::Case {
            expr, high, stmt, ..
        } => {
            put(out, indent, "case");
            dump_expr(out, expr, indent + 1);
            if let Some(high) = high {
                put(out, indent, "...");
                dump_expr(out, high, indent + 1);
            }
            dump_stmt(out, stmt, indent + 1);
        }
        Stmt::Default { stmt, .. } => {
            put(out, indent, "default");
            dump_stmt(out, stmt, indent + 1);
        }
        Stmt::Expr { expr, .. } => {
            put(out, indent, "expr");
            if let Some(expr) = expr {
                dump_expr(out, expr, indent + 1);
            }
        }
        Stmt::Break { .. } => put(out, indent, "break"),
        Stmt::Continue { .. } => put(out, indent, "continue"),
        Stmt::Label { name, stmt, .. } => {
            put(out, indent, &format!("label {name}"));
            dump_stmt(out, stmt, indent + 1);
        }
        Stmt::Goto { name, .. } => put(out, indent, &format!("goto {name}")),
        Stmt::GotoPtr { expr, .. } => {
            put(out, indent, "goto *");
            dump_expr(out, expr, indent + 1);
        }
    }
}

pub(super) fn dump_expr(out: &mut String, expr: &Expr, indent: usize) {
    match expr {
        Expr::Int { bits, value_ty, .. } => put(
            out,
            indent,
            &format!(
                "{} {}",
                value_ty.as_str(),
                value_ty.format_bits(u128::from(*bits))
            ),
        ),
        Expr::Float { bits, kind, .. } => put(
            out,
            indent,
            &format!(
                "{} {}",
                kind.as_str(),
                crate::float::llvm_const(*kind, *bits)
            ),
        ),
        Expr::Var { name, .. } => put(out, indent, &format!("var {name}")),
        Expr::Bin {
            op, left, right, ..
        } => dump_node(out, indent, op.as_str(), [left.as_ref(), right.as_ref()]),
        Expr::Unary { op, expr, .. } => dump_node(out, indent, unary_label(*op), [expr.as_ref()]),
        Expr::Assign { target, value, .. } => {
            dump_node(out, indent, "assign", [target.as_ref(), value.as_ref()]);
        }
        Expr::CompoundAssign {
            op, target, value, ..
        } => dump_node(
            out,
            indent,
            op.compound_spelling(),
            [target.as_ref(), value.as_ref()],
        ),
        Expr::IncDec { op, target, .. } => {
            dump_node(out, indent, incdec_label(*op), [target.as_ref()]);
        }
        Expr::Cond {
            cond,
            then_expr,
            else_expr,
            ..
        } => dump_node(
            out,
            indent,
            "?:",
            [cond.as_ref(), then_expr.as_ref(), else_expr.as_ref()],
        ),
        Expr::CondShort {
            cond, else_expr, ..
        } => dump_node(out, indent, "?:", [cond.as_ref(), else_expr.as_ref()]),
        Expr::LabelAddr { name, .. } => put(out, indent, &format!("&&{name}")),
        Expr::StmtExpr { body, .. } => {
            put(out, indent, "({})");
            dump_compound(out, body, indent + 1);
        }
        Expr::Comma { left, right, .. } => {
            dump_node(out, indent, ",", [left.as_ref(), right.as_ref()]);
        }
        Expr::Sizeof { operand, .. } => match operand {
            SizeofOperand::Type(ty) => put(out, indent, &format!("sizeof {}", ty.display())),
            SizeofOperand::Expr(expr) => dump_node(out, indent, "sizeof", [expr.as_ref()]),
        },
        Expr::Cast { ty_name, expr, .. } => dump_node(
            out,
            indent,
            &format!("cast {}", ty_name.display()),
            [expr.as_ref()],
        ),
        Expr::Index { base, index, .. } => {
            dump_node(out, indent, "index", [base.as_ref(), index.as_ref()]);
        }
        Expr::Decay { expr, .. } => dump_node(out, indent, "decay", [expr.as_ref()]),
        Expr::Str { bytes, .. } => put(out, indent, &dump_string(bytes)),
        Expr::Member {
            base, field, arrow, ..
        } => {
            let kind = if *arrow { "arrow" } else { "member" };
            dump_node(out, indent, &format!("{kind} {field}"), [base.as_ref()]);
        }
        Expr::VaArg { ap, .. } => dump_node(out, indent, "va_arg", [ap.as_ref()]),
        Expr::Generic {
            control, assocs, ..
        } => dump_generic(out, control, assocs, indent),
        Expr::CompoundLit { decl, .. } => {
            put(
                out,
                indent,
                &format!("compound literal {}", decl.ty.display()),
            );
            if let Some(init) = &decl.init {
                dump_init(out, init, indent + 1);
            }
        }
        Expr::VaOp { op, args, .. } => {
            let name = match op {
                VaOp::Start => "va_start",
                VaOp::End => "va_end",
                VaOp::Copy => "va_copy",
            };
            dump_node(out, indent, name, args);
        }
        Expr::Call { callee, args, .. } => dump_call(out, callee, args, indent),
    }
}

fn dump_call(out: &mut String, callee: &Expr, args: &[Expr], indent: usize) {
    match callee {
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

fn unary_label(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Plus => "pos",
        UnaryOp::Minus => "neg",
        UnaryOp::Not => "not",
        UnaryOp::BitNot => "bnot",
        UnaryOp::Addr => "addr",
        UnaryOp::Deref => "deref",
    }
}

fn incdec_label(op: IncOp) -> &'static str {
    match op {
        IncOp::PreInc => "pre++",
        IncOp::PreDec => "pre--",
        IncOp::PostInc => "post++",
        IncOp::PostDec => "post--",
    }
}

/// A line `label` followed by `children`, one level deeper.
fn dump_node<'e>(
    out: &mut String,
    indent: usize,
    label: &str,
    children: impl IntoIterator<Item = &'e Expr>,
) {
    put(out, indent, label);
    for child in children {
        dump_expr(out, child, indent + 1);
    }
}

fn dump_generic(out: &mut String, control: &Expr, assocs: &[GenericAssoc], indent: usize) {
    put(out, indent, "generic");
    dump_expr(out, control, indent + 1);
    for assoc in assocs {
        let label = match &assoc.ty {
            Some(ty) => format!("assoc {}", ty.display()),
            None => "default".to_string(),
        };
        put(out, indent + 1, &label);
        dump_expr(out, &assoc.expr, indent + 2);
    }
}

pub(super) fn dump_string(bytes: &[u8]) -> String {
    let mut text = String::from("string \"");
    for &byte in bytes {
        match byte {
            b'\n' => text.push_str("\\n"),
            b'\t' => text.push_str("\\t"),
            b'\r' => text.push_str("\\r"),
            b'\\' => text.push_str("\\\\"),
            b'"' => text.push_str("\\\""),
            0x20..=0x7e => text.push(byte as char),
            _ => text.push_str(&format!("\\x{byte:02x}")),
        }
    }
    text.push('"');
    text
}

pub(super) fn dump_init(out: &mut String, init: &Init, indent: usize) {
    match init {
        Init::Expr(expr) => dump_expr(out, expr, indent),
        Init::List { items, .. } => {
            put(out, indent, "list");
            for item in items {
                dump_init(out, item, indent + 1);
            }
        }
        Init::Designated {
            designators, init, ..
        } => {
            let text = designators
                .iter()
                .map(|designator| match designator {
                    Designator::Field { name, .. } => format!(".{name}"),
                    Designator::Index { .. } => "[index]".to_string(),
                })
                .collect::<String>();
            put(out, indent, &format!("designated {text}"));
            dump_init(out, init, indent + 1);
        }
    }
}
