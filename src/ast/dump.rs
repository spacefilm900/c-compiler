//! Textual dumps of the syntax tree.
//!
//! [`dump`] is the plain tree the parser unit tests compare against.
//! [`dump_parsed`] is the same tree with source positions, for
//! `--dump-ast`. [`dump_checked`] is the tree after semantic analysis, for
//! `--dump-sema`: every expression also shows its type, resolved names,
//! member offsets, implicit conversions, folded constants, and, after the
//! tree, record layouts, emitted globals, string literals and external
//! function declarations. The formats are described in `docs/DEBUGGING.md`.
//! Output is deterministic: it follows source order and never iterates a
//! hash map.

use super::*;

/// The plain syntax tree, without positions or types.
pub fn dump(unit: &TranslationUnit) -> String {
    let mut dumper = Dumper::new(false, false);
    dumper.unit(unit);
    dumper.out
}

/// The syntax tree as the parser produced it, with `@line:column` on every
/// node.
pub fn dump_parsed(unit: &TranslationUnit) -> String {
    let mut dumper = Dumper::new(true, false);
    dumper.unit(unit);
    dumper.out
}

/// The tree after semantic analysis, with types, conversions and layouts.
pub fn dump_checked(unit: &TranslationUnit) -> String {
    let mut dumper = Dumper::new(true, true);
    dumper.unit(unit);
    dumper.records(&unit.structs);
    dumper.globals(&unit.globals);
    dumper.strings(&unit.strings);
    dumper.func_decls(&unit.func_decls);
    dumper.out
}

struct Dumper {
    out: String,
    /// Append `@line:column` to nodes that have a position.
    located: bool,
    /// Show what semantic analysis filled in.
    typed: bool,
    /// Return type of the function being dumped, for the conversion of a
    /// `return` operand.
    ret: Option<CType>,
}

fn storage_prefix(storage: Storage) -> &'static str {
    match storage {
        Storage::None | Storage::Typedef => "",
        Storage::Static => "static ",
        Storage::Extern => "extern ",
    }
}

fn linkage_word(linkage: Linkage) -> &'static str {
    match linkage {
        Linkage::External => "external",
        Linkage::Internal => "internal",
    }
}

/// The spelling used to compare two types for an implicit conversion.
fn bare(ty: &CType) -> String {
    ty.unqualified().display()
}

impl Dumper {
    fn new(located: bool, typed: bool) -> Self {
        Self {
            out: String::new(),
            located,
            typed,
            ret: None,
        }
    }

    fn put(&mut self, indent: usize, text: &str) {
        for _ in 0..indent {
            self.out.push_str("  ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    /// `text`, then ` @line:column` when positions are on.
    fn put_at(&mut self, indent: usize, text: &str, (line, column): (usize, usize)) {
        if self.located {
            self.put(indent, &format!("{text} @{line}:{column}"));
        } else {
            self.put(indent, text);
        }
    }

    fn unit(&mut self, unit: &TranslationUnit) {
        for decl in &unit.decls {
            match decl {
                TopDecl::Func(func) => self.func(func, 0),
                TopDecl::Global(global) => self.global(global, 0),
            }
        }
    }

    fn global(&mut self, global: &GlobalDecl, indent: usize) {
        let prefix = storage_prefix(global.storage);
        let ty = global.ty.display();
        let pos = (global.line, global.column);
        if let Some(init) = &global.init {
            self.put_at(
                indent,
                &format!("{prefix}global {ty} {} =", global.name),
                pos,
            );
            self.init(init, indent + 1, None);
        } else {
            self.put_at(indent, &format!("{prefix}global {ty} {}", global.name), pos);
        }
    }

    fn func(&mut self, func: &FuncDef, indent: usize) {
        let kind = if func.body.is_some() { "func" } else { "proto" };
        let mut head = format!(
            "{}{kind} {} {}",
            storage_prefix(func.storage),
            func.ret_type.display(),
            func.name
        );
        if self.typed {
            head.push_str(&format!(" [{}", linkage_word(func.linkage)));
            if func.symbol != func.name {
                head.push_str(&format!(" symbol {}", func.symbol));
            }
            head.push(']');
        }
        self.put_at(indent, &head, (func.line, func.column));
        for param in &func.params {
            let mut text = match &param.name {
                Some(name) => format!("param {} {name}", param.ty.display()),
                None => format!("param {}", param.ty.display()),
            };
            if self.typed && !param.slot.is_empty() {
                text.push_str(&format!(" [{}]", param.slot));
            }
            self.put_at(indent + 1, &text, (param.line, param.column));
        }
        if func.variadic {
            self.put(indent + 1, "varargs");
        }
        if let Some(body) = &func.body {
            let outer = self.ret.replace(func.ret_type.clone());
            self.compound(body, indent + 1);
            self.ret = outer;
        }
    }

    fn local(&mut self, decl: &VarDecl, indent: usize) {
        let prefix = storage_prefix(decl.storage);
        let ty = decl.ty.display();
        let mut head = format!("{prefix}decl {ty} {}", decl.name);
        if self.typed && !decl.slot.is_empty() {
            head.push_str(&format!(" [{}]", decl.slot));
        }
        if let Some(align) = decl.align.filter(|_| self.typed) {
            head.push_str(&format!(" align {align}"));
        }
        let pos = (decl.line, decl.column);
        match (&decl.checked, &decl.init) {
            (Some(checked), _) if self.typed => {
                self.put_at(indent, &format!("{head} ="), pos);
                self.checked_init(checked, &decl.ty, indent + 1);
            }
            (_, Some(init)) => {
                self.put_at(indent, &format!("{head} ="), pos);
                self.init(init, indent + 1, None);
            }
            (_, None) => self.put_at(indent, &head, pos),
        }
    }

    fn checked_init(&mut self, checked: &CheckedInit, ty: &CType, indent: usize) {
        match checked {
            CheckedInit::Scalar(expr) => self.expr(expr, indent, Some(&ty.unqualified())),
            CheckedInit::Aggregate(stores) => {
                self.put(indent, "stores");
                for store in stores {
                    let mut text = format!("store {} at {}", store.ty.display(), store.offset);
                    if let Some(bits) = &store.bits {
                        text.push_str(&format!(
                            " bits {} at bit {} unit {}{}",
                            bits.width,
                            bits.abs_bit,
                            bits.container_bits,
                            if bits.signed_field { " signed" } else { "" }
                        ));
                    }
                    self.put(indent + 1, &text);
                    let target = store.ty.unqualified();
                    self.expr(&store.expr, indent + 2, Some(&target));
                }
            }
        }
    }

    fn compound(&mut self, compound: &Compound, indent: usize) {
        self.put_at(indent, "block", (compound.line, compound.column));
        for item in &compound.items {
            self.stmt(item, indent + 1);
        }
    }

    fn stmt(&mut self, stmt: &Stmt, indent: usize) {
        let pos = stmt.pos();
        match stmt {
            Stmt::Compound(compound) => self.compound(compound, indent),
            Stmt::Decl(decl) => self.local(decl, indent),
            Stmt::FuncDecl(func) => self.func(func, indent),
            Stmt::Return { expr, .. } => {
                self.put_at(indent, "return", pos);
                if let Some(expr) = expr {
                    let ret = self.ret.as_ref().map(CType::unqualified);
                    self.expr(expr, indent + 1, ret.as_ref());
                }
            }
            Stmt::If {
                cond,
                then_stmt,
                else_stmt,
                ..
            } => {
                self.put_at(indent, "if", pos);
                self.put(indent + 1, "cond");
                self.expr(cond, indent + 2, None);
                self.put(indent + 1, "then");
                self.stmt(then_stmt, indent + 2);
                if let Some(else_stmt) = else_stmt {
                    self.put(indent + 1, "else");
                    self.stmt(else_stmt, indent + 2);
                }
            }
            Stmt::While { .. } | Stmt::For { .. } | Stmt::Do { .. } => self.loop_stmt(stmt, indent),
            Stmt::Switch { cond, body, .. } => {
                self.put_at(indent, "switch", pos);
                self.put(indent + 1, "cond");
                self.expr(cond, indent + 2, None);
                self.put(indent + 1, "body");
                self.stmt(body, indent + 2);
            }
            Stmt::Case { .. } => self.case_stmt(stmt, indent),
            Stmt::Default { stmt, .. } => {
                self.put_at(indent, "default", pos);
                self.stmt(stmt, indent + 1);
            }
            Stmt::Expr { expr, .. } => {
                self.put_at(indent, "expr", pos);
                if let Some(expr) = expr {
                    self.expr(expr, indent + 1, None);
                }
            }
            Stmt::Break { .. } => self.put_at(indent, "break", pos),
            Stmt::Continue { .. } => self.put_at(indent, "continue", pos),
            Stmt::Label { name, stmt, .. } => {
                self.put_at(indent, &format!("label {name}"), pos);
                self.stmt(stmt, indent + 1);
            }
            Stmt::Goto { name, .. } => self.put_at(indent, &format!("goto {name}"), pos),
            Stmt::GotoPtr { expr, .. } => {
                self.put_at(indent, "goto *", pos);
                self.expr(expr, indent + 1, None);
            }
            Stmt::Asm { template, .. } => self.put_at(
                indent,
                &format!("asm {:?}", String::from_utf8_lossy(template)),
                pos,
            ),
        }
    }

    fn loop_stmt(&mut self, stmt: &Stmt, indent: usize) {
        let pos = stmt.pos();
        match stmt {
            Stmt::While { cond, body, .. } => {
                self.put_at(indent, "while", pos);
                self.put(indent + 1, "cond");
                self.expr(cond, indent + 2, None);
                self.put(indent + 1, "body");
                self.stmt(body, indent + 2);
            }
            Stmt::For {
                init,
                cond,
                step,
                body,
                ..
            } => {
                self.put_at(indent, "for", pos);
                self.put(indent + 1, "init");
                match init {
                    ForInit::Empty => {}
                    ForInit::Expr(expr) => self.expr(expr, indent + 2, None),
                    ForInit::Decl(decls) => {
                        for decl in decls {
                            self.local(decl, indent + 2);
                        }
                    }
                }
                self.put(indent + 1, "cond");
                if let Some(cond) = cond {
                    self.expr(cond, indent + 2, None);
                }
                self.put(indent + 1, "step");
                if let Some(step) = step {
                    self.expr(step, indent + 2, None);
                }
                self.put(indent + 1, "body");
                self.stmt(body, indent + 2);
            }
            Stmt::Do { body, cond, .. } => {
                self.put_at(indent, "do", pos);
                self.put(indent + 1, "body");
                self.stmt(body, indent + 2);
                self.put(indent + 1, "cond");
                self.expr(cond, indent + 2, None);
            }
            _ => {}
        }
    }

    fn case_stmt(&mut self, stmt: &Stmt, indent: usize) {
        let pos = stmt.pos();
        let Stmt::Case {
            expr,
            value,
            high,
            high_value,
            stmt: body,
            ..
        } = stmt
        else {
            return;
        };
        let label = match value.filter(|_| self.typed) {
            Some(value) => format!("case = {}", case_value(expr, value)),
            None => "case".to_string(),
        };
        self.put_at(indent, &label, pos);
        self.expr(expr, indent + 1, None);
        if let Some(high) = high {
            let label = match high_value.filter(|_| self.typed) {
                Some(value) => format!("... = {}", case_value(high, value)),
                None => "...".to_string(),
            };
            self.put(indent, &label);
            self.expr(high, indent + 1, None);
        }
        self.stmt(body, indent + 1);
    }

    /// One expression line. In a typed dump the line is
    /// `label [details] : type -> converted`, where `-> converted` is the
    /// implicit conversion the context applies to the value (shown only
    /// when the type changes).
    fn expr_line(&mut self, indent: usize, expr: &Expr, label: &str, conv: Option<&CType>) {
        let mut text = label.to_string();
        if self.typed {
            if let Some(details) = details(expr) {
                text.push(' ');
                text.push_str(&details);
            }
            if let Some(ty) = expr.ty() {
                text.push_str(&format!(" : {}", ty.display()));
                if let Some(target) = conv {
                    if !target.is_void() && bare(target) != bare(&ty) {
                        text.push_str(&format!(" -> {}", target.display()));
                    }
                }
            }
        }
        self.put_at(indent, &text, (expr.line(), expr.column()));
    }

    fn expr(&mut self, expr: &Expr, indent: usize, conv: Option<&CType>) {
        match expr {
            Expr::Int { .. }
            | Expr::Float { .. }
            | Expr::Var { .. }
            | Expr::Str { .. }
            | Expr::LabelAddr { .. } => self.leaf(expr, indent, conv),
            Expr::Bin { .. }
            | Expr::Unary { .. }
            | Expr::Assign { .. }
            | Expr::CompoundAssign { .. }
            | Expr::IncDec { .. }
            | Expr::Cond { .. }
            | Expr::CondShort { .. }
            | Expr::Comma { .. } => self.operator(expr, indent, conv),
            Expr::Call { .. } => self.call(expr, indent, conv),
            _ => self.postfix(expr, indent, conv),
        }
    }

    /// Constants, names, string literals and label addresses.
    fn leaf(&mut self, expr: &Expr, indent: usize, conv: Option<&CType>) {
        match expr {
            Expr::Int { bits, value_ty, .. } => {
                let label = format!(
                    "{} {}",
                    value_ty.as_str(),
                    value_ty.format_bits(u128::from(*bits))
                );
                self.expr_line(indent, expr, &label, conv);
            }
            Expr::Float { bits, kind, .. } => {
                let label = format!(
                    "{} {}",
                    kind.as_str(),
                    crate::float::llvm_const(*kind, *bits)
                );
                self.expr_line(indent, expr, &label, conv);
            }
            Expr::Var { name, .. } => self.expr_line(indent, expr, &format!("var {name}"), conv),
            Expr::Str { bytes, .. } => self.expr_line(indent, expr, &dump_string(bytes), conv),
            Expr::LabelAddr { name, .. } => {
                self.expr_line(indent, expr, &format!("&&{name}"), conv);
            }
            _ => {}
        }
    }

    /// Unary, binary, assignment and conditional operators.
    fn operator(&mut self, expr: &Expr, indent: usize, conv: Option<&CType>) {
        match expr {
            Expr::Bin {
                op, left, right, ..
            } => {
                self.expr_line(indent, expr, op.as_str(), conv);
                let (left_conv, right_conv) = self.binary_operand_types(*op, expr, left, right);
                self.expr(left, indent + 1, left_conv.as_ref());
                self.expr(right, indent + 1, right_conv.as_ref());
            }
            Expr::Unary {
                op, expr: inner, ..
            } => {
                self.expr_line(indent, expr, unary_label(*op), conv);
                let inner_conv = match op {
                    UnaryOp::Plus | UnaryOp::Minus | UnaryOp::BitNot => expr.ty(),
                    UnaryOp::Not | UnaryOp::Addr | UnaryOp::Deref => None,
                };
                self.expr(inner, indent + 1, inner_conv.as_ref());
            }
            Expr::Assign { target, value, .. } => {
                self.expr_line(indent, expr, "assign", conv);
                self.expr(target, indent + 1, None);
                let to = target.ty().map(|ty| ty.unqualified());
                self.expr(value, indent + 1, to.as_ref());
            }
            Expr::CompoundAssign {
                op, target, value, ..
            } => {
                self.expr_line(indent, expr, op.compound_spelling(), conv);
                self.expr(target, indent + 1, None);
                self.expr(value, indent + 1, None);
            }
            Expr::IncDec { op, target, .. } => {
                self.expr_line(indent, expr, incdec_label(*op), conv);
                self.expr(target, indent + 1, None);
            }
            Expr::Cond {
                cond,
                then_expr,
                else_expr,
                ..
            } => {
                self.expr_line(indent, expr, "?:", conv);
                let arms = expr.ty().filter(CType::is_arithmetic);
                self.expr(cond, indent + 1, None);
                self.expr(then_expr, indent + 1, arms.as_ref());
                self.expr(else_expr, indent + 1, arms.as_ref());
            }
            Expr::CondShort {
                cond, else_expr, ..
            } => {
                self.expr_line(indent, expr, "?:", conv);
                let arms = expr.ty().filter(CType::is_arithmetic);
                self.expr(cond, indent + 1, arms.as_ref());
                self.expr(else_expr, indent + 1, arms.as_ref());
            }
            Expr::Comma { left, right, .. } => {
                self.expr_line(indent, expr, ",", conv);
                self.expr(left, indent + 1, None);
                self.expr(right, indent + 1, None);
            }
            _ => {}
        }
    }

    /// Casts, `sizeof`, subscripts, members, and the other forms.
    fn postfix(&mut self, expr: &Expr, indent: usize, conv: Option<&CType>) {
        match expr {
            Expr::StmtExpr { body, .. } => {
                self.expr_line(indent, expr, "({})", conv);
                self.compound(body, indent + 1);
            }
            Expr::Sizeof { operand, .. } => match operand {
                SizeofOperand::Type(ty) => {
                    self.expr_line(indent, expr, &format!("sizeof {}", ty.display()), conv);
                }
                SizeofOperand::Expr(inner) => {
                    self.expr_line(indent, expr, "sizeof", conv);
                    self.expr(inner, indent + 1, None);
                }
            },
            Expr::Cast {
                ty_name,
                expr: inner,
                ..
            } => {
                self.expr_line(indent, expr, &format!("cast {}", ty_name.display()), conv);
                self.expr(inner, indent + 1, None);
            }
            Expr::Index { base, index, .. } => {
                self.expr_line(indent, expr, "index", conv);
                self.expr(base, indent + 1, None);
                self.expr(index, indent + 1, None);
            }
            Expr::Decay { expr: inner, .. } => {
                self.expr_line(indent, expr, "decay", conv);
                self.expr(inner, indent + 1, None);
            }
            Expr::Member {
                base, field, arrow, ..
            } => {
                let kind = if *arrow { "arrow" } else { "member" };
                self.expr_line(indent, expr, &format!("{kind} {field}"), conv);
                self.expr(base, indent + 1, None);
            }
            Expr::VaArg { ap, type_arg, .. } => {
                let label = format!("va_arg {}", type_arg.display());
                self.expr_line(indent, expr, &label, conv);
                self.expr(ap, indent + 1, None);
            }
            Expr::Generic {
                control, assocs, ..
            } => {
                self.expr_line(indent, expr, "generic", conv);
                self.expr(control, indent + 1, None);
                for assoc in assocs {
                    let label = match &assoc.ty {
                        Some(ty) => format!("assoc {}", ty.display()),
                        None => "default".to_string(),
                    };
                    self.put(indent + 1, &label);
                    self.expr(&assoc.expr, indent + 2, None);
                }
            }
            Expr::CompoundLit { decl, .. } => {
                let label = format!("compound literal {}", decl.ty.display());
                self.expr_line(indent, expr, &label, conv);
                match (&decl.checked, &decl.init) {
                    (Some(checked), _) if self.typed => {
                        self.checked_init(checked, &decl.ty, indent + 1);
                    }
                    (_, Some(init)) => self.init(init, indent + 1, None),
                    (_, None) => {}
                }
            }
            Expr::VaOp { op, args, .. } => {
                let name = match op {
                    VaOp::Start => "va_start",
                    VaOp::End => "va_end",
                    VaOp::Copy => "va_copy",
                };
                self.expr_line(indent, expr, name, conv);
                for arg in args {
                    self.expr(arg, indent + 1, None);
                }
            }
            _ => {}
        }
    }

    /// A call, with each argument converted to its parameter type.
    fn call(&mut self, expr: &Expr, indent: usize, conv: Option<&CType>) {
        let Expr::Call {
            callee,
            args,
            resolved,
            ..
        } = expr
        else {
            return;
        };
        match callee.as_ref() {
            Expr::Var { name, .. } => {
                self.expr_line(indent, expr, &format!("call {name}"), conv);
            }
            other => {
                self.expr_line(indent, expr, "call", conv);
                self.expr(other, indent + 1, None);
            }
        }
        for (position, arg) in args.iter().enumerate() {
            let target = resolved.as_ref().and_then(|res| {
                res.params
                    .get(position)
                    .map(CType::unqualified)
                    .or_else(|| {
                        arg.ty()
                            .filter(|_| self.typed)
                            .map(|ty| default_promoted(&ty))
                    })
            });
            self.expr(arg, indent + 1, target.as_ref());
        }
    }

    /// The types the operands of `op` are converted to (C11 6.3.1.8 for the
    /// arithmetic operators and comparisons, the integer promotions for
    /// shifts). `None` where there is no arithmetic conversion, such as
    /// pointer arithmetic and `&&`.
    fn binary_operand_types(
        &self,
        op: BinOp,
        expr: &Expr,
        left: &Expr,
        right: &Expr,
    ) -> (Option<CType>, Option<CType>) {
        if !self.typed {
            return (None, None);
        }
        let (Some(left_ty), Some(right_ty)) = (left.ty(), right.ty()) else {
            return (None, None);
        };
        if !(left_ty.is_arithmetic() && right_ty.is_arithmetic()) {
            return (None, None);
        }
        match op {
            BinOp::And | BinOp::Or => (None, None),
            BinOp::Shl | BinOp::Shr => (
                left_ty
                    .as_integer()
                    .map(|int| CType::from_name(int.promote())),
                right_ty
                    .as_integer()
                    .map(|int| CType::from_name(int.promote())),
            ),
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                let common = usual_arithmetic(&left_ty, &right_ty);
                (common.clone(), common)
            }
            BinOp::Add
            | BinOp::Sub
            | BinOp::Mul
            | BinOp::Div
            | BinOp::Mod
            | BinOp::BitAnd
            | BinOp::BitOr
            | BinOp::BitXor => (expr.ty(), expr.ty()),
        }
    }

    fn init(&mut self, init: &Init, indent: usize, conv: Option<&CType>) {
        match init {
            Init::Expr(expr) => self.expr(expr, indent, conv),
            Init::List {
                items,
                line,
                column,
            } => {
                self.put_at(indent, "list", (*line, *column));
                for item in items {
                    self.init(item, indent + 1, None);
                }
            }
            Init::Designated {
                designators,
                init,
                line,
                column,
                ..
            } => {
                let text = designators
                    .iter()
                    .map(|designator| match designator {
                        Designator::Field { name, .. } => format!(".{name}"),
                        Designator::Index { .. } => "[]".to_string(),
                    })
                    .collect::<String>();
                self.put_at(indent, &format!("designated {text}"), (*line, *column));
                // Each `[]` above, in order: its index expression.
                for designator in designators {
                    if let Designator::Index { expr, .. } = designator {
                        self.put(indent + 1, "index");
                        self.expr(expr, indent + 2, None);
                    }
                }
                self.init(init, indent + 1, None);
            }
        }
    }

    fn records(&mut self, structs: &[StructRef]) {
        if structs.is_empty() {
            return;
        }
        self.put(0, "records");
        for record in structs {
            let tag = record.tag();
            let tag = if tag.is_empty() { "<anonymous>" } else { &tag };
            let size = record
                .size()
                .map_or_else(|| "?".to_string(), |size| size.to_string());
            self.put(
                1,
                &format!(
                    "{} {tag} {} size {size} align {}",
                    record.kind().word(),
                    record.ir_name(),
                    record.align()
                ),
            );
            for field in record.fields().unwrap_or_default() {
                let name = if field.name.is_empty() {
                    "<unnamed>"
                } else {
                    &field.name
                };
                let text = match field.bit_width {
                    Some(width) => format!(
                        "bitfield {} {name} : {width} unit at {} bit {}",
                        field.ty.display(),
                        field.offset,
                        field.bit_offset
                    ),
                    None => format!("field {} {name} at {}", field.ty.display(), field.offset),
                };
                self.put(2, &text);
            }
        }
    }

    fn globals(&mut self, globals: &[IrGlobal]) {
        if globals.is_empty() {
            return;
        }
        self.put(0, "globals");
        for global in globals {
            let head = format!(
                "global {} {} [{}]",
                global.ty.display(),
                global.name,
                linkage_word(global.linkage)
            );
            match &global.init {
                None => self.put(1, &format!("{head} declared")),
                Some(init) => {
                    self.put(1, &format!("{head} ="));
                    self.const_init(init, Some(&global.ty), 2);
                }
            }
        }
    }

    fn const_init(&mut self, value: &ConstInit, ty: Option<&CType>, indent: usize) {
        match value {
            ConstInit::Zero => self.put(indent, "zero"),
            ConstInit::Int(bits) => {
                let text = match ty.and_then(CType::as_integer) {
                    Some(int) => format!("{} {}", int.as_str(), int.format_bits(*bits)),
                    None => format!("int 0x{bits:x}"),
                };
                self.put(indent, &text);
            }
            ConstInit::Float(bits) => {
                let text = match ty.and_then(CType::float_kind) {
                    Some(kind) => {
                        format!(
                            "{} {}",
                            kind.as_str(),
                            crate::float::llvm_const(kind, *bits)
                        )
                    }
                    None => format!("float 0x{bits:x}"),
                };
                self.put(indent, &text);
            }
            ConstInit::Ptr(ptr) => self.put(indent, &ptr_text(ptr)),
            ConstInit::PtrToInt { ptr, width } => {
                self.put(indent, &format!("ptrtoint i{width} {}", ptr_text(ptr)));
            }
            ConstInit::Aggregate(items) => {
                self.put(indent, "aggregate");
                let fields = ty.and_then(CType::struct_ref).and_then(StructRef::fields);
                let elem = ty.and_then(|ty| match ty.unqualified() {
                    CType::Array(elem, _) => Some(*elem),
                    _ => None,
                });
                for (position, item) in items.iter().enumerate() {
                    let item_ty = match (&fields, &elem) {
                        (Some(fields), _) => fields.get(position).map(|field| field.ty.clone()),
                        (None, Some(elem)) => Some(elem.clone()),
                        (None, None) => None,
                    };
                    self.const_init(item, item_ty.as_ref(), indent + 1);
                }
            }
            ConstInit::Bytes(bytes) => self.put(indent, &format!("bytes {}", hex_bytes(bytes))),
            ConstInit::RelocBytes { bytes, relocs } => {
                self.put(indent, &format!("bytes {}", hex_bytes(bytes)));
                for (offset, reloc) in relocs {
                    self.put(indent + 1, &format!("reloc at {offset}"));
                    self.const_init(reloc, None, indent + 2);
                }
            }
        }
    }

    fn strings(&mut self, strings: &[StrConst]) {
        if strings.is_empty() {
            return;
        }
        self.put(0, "strings");
        for string in strings {
            self.put(
                1,
                &format!("@{} = {}", string.name, dump_string(&string.bytes)),
            );
        }
    }

    fn func_decls(&mut self, decls: &[IrFunc]) {
        if decls.is_empty() {
            return;
        }
        self.put(0, "declarations");
        for decl in decls {
            let mut params = decl.params.iter().map(CType::display).collect::<Vec<_>>();
            if decl.variadic {
                params.push("...".to_string());
            }
            self.put(
                1,
                &format!(
                    "declare {} {}({}) [{}]",
                    decl.ret.display(),
                    decl.name,
                    params.join(", "),
                    linkage_word(decl.linkage)
                ),
            );
        }
    }
}

/// What semantic analysis recorded on an expression besides its type.
fn details(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Var {
            resolved: Some(res),
            ..
        } => Some(format!("[{}]", res.slot)),
        Expr::Str {
            symbol: Some(symbol),
            ..
        } => Some(format!("[@{symbol}]")),
        Expr::LabelAddr {
            func: Some(func), ..
        } => Some(format!("[in @{func}]")),
        Expr::Call {
            resolved: Some(res),
            ..
        } => {
            let mut params = res.params.iter().map(CType::display).collect::<Vec<_>>();
            if res.variadic {
                params.push("...".to_string());
            }
            let target = if res.indirect {
                "indirect".to_string()
            } else {
                format!("@{}", res.name)
            };
            Some(format!(
                "[{target} {}({})]",
                res.ret.display(),
                params.join(", ")
            ))
        }
        Expr::Member {
            index,
            offset,
            bytewise,
            bit_width,
            bit_signed,
            container_bits,
            abs_bit,
            ..
        } => {
            let mut parts = Vec::new();
            if let Some(index) = index {
                parts.push(format!("field {index}"));
            }
            if let Some(offset) = offset {
                parts.push(format!("at {offset}"));
            }
            if *bytewise {
                parts.push("bytewise".to_string());
            }
            if let Some(width) = bit_width {
                parts.push(format!(
                    "bits {width} at bit {abs_bit} unit {container_bits}{}",
                    if *bit_signed { " signed" } else { "" }
                ));
            }
            (!parts.is_empty()).then(|| format!("[{}]", parts.join(" ")))
        }
        _ => None,
    }
}

/// The usual arithmetic conversions (C11 6.3.1.8) of two arithmetic types.
fn usual_arithmetic(left: &CType, right: &CType) -> Option<CType> {
    if left.is_float() || right.is_float() {
        return Some(CType::Float(crate::float::usual_real(
            left.float_kind(),
            right.float_kind(),
        )));
    }
    let (left, right) = (left.as_integer()?, right.as_integer()?);
    Some(CType::from_name(left.usual(right)))
}

/// A folded `case` value, spelled in the type of its expression.
fn case_value(expr: &Expr, value: u128) -> String {
    match expr.ty().and_then(|ty| ty.as_integer()) {
        Some(int) => int.format_bits(value),
        None => value.to_string(),
    }
}

fn ptr_text(ptr: &PtrConst) -> String {
    match ptr {
        PtrConst::Null => "null".to_string(),
        PtrConst::Addr { name, offset: 0 } => format!("&{name}"),
        PtrConst::Addr { name, offset } => format!("&{name} + {offset}"),
        PtrConst::IntToPtr(bits) => format!("inttoptr 0x{bits:x}"),
        PtrConst::Label { func, label } => format!("&&{label} in @{func}"),
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
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

fn dump_string(bytes: &[u8]) -> String {
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
