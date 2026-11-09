//! Textual LLVM IR for the C subset.
//!
//! Integer widths follow LP64: `char` is i8, `short` is i16, `int` is i32,
//! and `long` / `long long` are i64. `char` is signed. Locals are `alloca`s
//! in the entry block. File-scope objects are LLVM globals. Internal linkage
//! is `internal`; a declaration with no definition in this translation unit
//! is `external global` or `declare`. A const object is `constant`. String
//! literals whose address is used are `private unnamed_addr constant` arrays
//! of `i8`. A variadic call names the function type, as in
//! `call i32 (ptr, ...) @printf(...)`.
//!
//! The System V AMD64 ABI sign-extends signed `char` and `short` parameters
//! and return values to 32 bits, and zero-extends the unsigned ones. Those
//! are the LLVM `signext` and `zeroext` attributes on the definition, the
//! declaration, and the call. `int` and wider integers have no extension
//! attribute.
//!
//! `&&` and `||` are short-circuiting and produce `int` 0 or 1. `?:`
//! evaluates only the chosen arm and converts both arms to the usual
//! arithmetic type. Signed division uses `sdiv` / `srem`. Unsigned division
//! uses `udiv` / `urem`. Both round toward zero. Unsigned comparisons use
//! `icmp u*`. Signed `>>` is `ashr`; unsigned `>>` is `lshr`.
//!
//! A shift count outside `0 .. width`, a signed left shift of a negative
//! value, and a signed left shift whose mathematical result does not fit are
//! undefined. Those counts are not masked. Constant initializers reject
//! them; a runtime expression does not. Conversions between integer types are
//! `sext`, `zext`, or `trunc`. A same-width signed/unsigned conversion is a
//! no-op in the IR. `sizeof` is an `unsigned long` constant and its operand
//! is not emitted.

use crate::ast::*;

/// System V AMD64 layout for `x86_64-pc-linux-gnu`.
///
/// `i64` is aligned to 8 bytes, which is what `long` requires. LLVM's default
/// layout, used when this line is absent, aligns `i64` to 4 bytes. A struct
/// `getelementptr` is then folded with the wrong field offset: `struct { char
/// c; long l; }` puts `l` at 4 instead of 8. The string is the one Clang
/// attaches to this triple.
const DATA_LAYOUT: &str =
    "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128";

pub fn render(unit: &TranslationUnit, filename: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("source_filename = {}\n", llvm_string(filename)));
    out.push_str(&format!("target datalayout = \"{DATA_LAYOUT}\"\n"));
    out.push_str("target triple = \"x86_64-pc-linux-gnu\"\n\n");
    let mut wrote_prelude = false;
    for handle in &unit.structs {
        if handle.is_complete() {
            out.push_str(&render_struct(handle));
            out.push('\n');
            wrote_prelude = true;
        }
    }
    for literal in &unit.strings {
        out.push_str(&render_string(literal));
        out.push('\n');
        wrote_prelude = true;
    }
    for global in &unit.globals {
        out.push_str(&render_global(global));
        out.push('\n');
        wrote_prelude = true;
    }
    for decl in &unit.func_decls {
        out.push_str(&render_declare(decl));
        out.push('\n');
        wrote_prelude = true;
    }
    let mut functions = Vec::new();
    let mut needs_memmove = false;
    for decl in &unit.decls {
        if let TopDecl::Func(func) = decl {
            if func.body.is_some() {
                let (text, copied) = render_function(func);
                needs_memmove |= copied;
                functions.push(text);
            }
        }
    }
    if needs_memmove {
        // Overlap is allowed, so `s = s` is defined. The copy is the object
        // representation, padding included. An aggregate load/store is not:
        // LLVM may leave the padding `undef`.
        out.push_str("declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)\n");
        wrote_prelude = true;
    }
    if wrote_prelude {
        out.push('\n');
    }
    for text in functions {
        out.push_str(&text);
        out.push_str("\n\n");
    }
    out
}

fn render_global(global: &IrGlobal) -> String {
    let name = llvm_global(&global.name);
    let ir = global.ty.ir_type();
    let align = global.ty.align();
    // A const object is an LLVM `constant`. `const int *` is not const; the
    // pointer can be reseated. An array of const elements is constant.
    let kind = if global.ty.is_const_object() {
        "constant"
    } else {
        "global"
    };
    if let Some(value) = &global.init {
        let linkage = match global.linkage {
            Linkage::Internal => "internal ",
            Linkage::External => "",
        };
        let spelled = render_const(&global.ty, value);
        format!("{name} = {linkage}{kind} {ir} {spelled}, align {align}")
    } else {
        format!("{name} = external {kind} {ir}, align {align}")
    }
}

fn render_struct(handle: &StructRef) -> String {
    if handle.is_union() {
        let size = handle.size().unwrap_or(0);
        let align = u64::from(handle.align().max(1));
        let body = if align != 0 && size % align == 0 && matches!(align, 1 | 2 | 4 | 8 | 16) {
            let elem = match align {
                1 => "i8",
                2 => "i16",
                4 => "i32",
                8 => "i64",
                _ => "i128",
            };
            format!("[ {} x {elem} ]", size / align)
        } else {
            format!("[ {size} x i8 ]")
        };
        return format!("%{} = type {{ {body} }}", handle.llvm_name());
    }
    let fields = handle.fields().unwrap_or_default();
    let body = fields
        .iter()
        .map(|field| field.ty.ir_type())
        .collect::<Vec<_>>()
        .join(", ");
    format!("%{} = type {{ {body} }}", handle.llvm_name())
}

fn render_string(literal: &StrConst) -> String {
    let n = literal.bytes.len();
    format!(
        "@{} = private unnamed_addr constant [{n} x i8] c{}, align 1",
        literal.name,
        llvm_bytes(&literal.bytes)
    )
}

fn render_declare(decl: &IrFunc) -> String {
    let mut parts: Vec<String> = decl.params.iter().map(|ty| param_ir(ty, None)).collect();
    if decl.variadic {
        parts.push("...".to_string());
    }
    let params = parts.join(", ");
    format!(
        "declare {}{} {}({params})",
        ret_prefix(&decl.ret),
        decl.ret.ir_type(),
        llvm_global(&decl.name)
    )
}

fn ret_prefix(ty: &CType) -> String {
    match ty.abi_attr() {
        Some(attr) => format!("{attr} "),
        None => String::new(),
    }
}

fn param_ir(ty: &CType, name: Option<&str>) -> String {
    let ir = ty.ir_type();
    match (ty.abi_attr(), name) {
        (Some(attr), Some(name)) => format!("{ir} {attr} {name}"),
        (Some(attr), None) => format!("{ir} {attr}"),
        (None, Some(name)) => format!("{ir} {name}"),
        (None, None) => ir,
    }
}

fn render_function(func: &FuncDef) -> (String, bool) {
    let body = func.body.as_ref().expect("defined function");
    let mut gen = Gen::new(func.ret_type.clone());
    let linkage = match func.linkage {
        Linkage::Internal => "internal ",
        Linkage::External => "",
    };
    let params = func
        .params
        .iter()
        .map(|param| param_ir(&param.ty, Some(&param.arg_reg)))
        .collect::<Vec<_>>()
        .join(", ");
    gen.emit(&format!(
        "define {linkage}{}{} {}({params}) {{",
        ret_prefix(&func.ret_type),
        func.ret_type.ir_type(),
        llvm_global(&func.symbol)
    ));
    gen.emit("entry:");
    gen.cur = "entry".to_string();
    gen.terminated = false;

    for param in &func.params {
        gen.emit(&format!(
            "  {} = alloca {}, align {}",
            param.slot,
            param.ty.ir_type(),
            param.ty.align()
        ));
    }
    for (slot, ty) in local_slots(body) {
        gen.emit(&format!(
            "  {slot} = alloca {}, align {}",
            ty.ir_type(),
            ty.align()
        ));
    }
    for param in &func.params {
        gen.emit(&format!(
            "  store {} {}, ptr {}, align {}",
            param.ty.ir_type(),
            param.arg_reg,
            param.slot,
            param.ty.align()
        ));
    }

    gen.gen_compound(body);
    if !gen.terminated {
        if func.ret_type.is_void() {
            gen.emit("  ret void");
        } else if func.ret_type.is_pointer() {
            gen.emit("  ret ptr null");
        } else {
            gen.emit(&format!("  ret {} 0", func.ret_type.ir_type()));
        }
    }
    gen.emit("}");
    (gen.lines.join("\n"), gen.needs_memmove)
}

fn local_slots(compound: &Compound) -> Vec<(String, CType)> {
    let mut slots = Vec::new();
    for item in &compound.items {
        collect_slots(item, &mut slots);
    }
    slots
}

fn collect_slots(stmt: &Stmt, slots: &mut Vec<(String, CType)>) {
    match stmt {
        Stmt::Decl(decl) => slots.push((decl.slot.clone(), decl.ty.clone())),
        Stmt::Compound(compound) => {
            for item in &compound.items {
                collect_slots(item, slots);
            }
        }
        Stmt::If {
            then_stmt,
            else_stmt,
            ..
        } => {
            collect_slots(then_stmt, slots);
            if let Some(else_stmt) = else_stmt {
                collect_slots(else_stmt, slots);
            }
        }
        Stmt::While { body, .. } => collect_slots(body, slots),
        Stmt::For { init, body, .. } => {
            if let ForInit::Decl(decls) = init {
                for decl in decls {
                    slots.push((decl.slot.clone(), decl.ty.clone()));
                }
            }
            collect_slots(body, slots);
        }
        Stmt::Return { .. } | Stmt::Expr { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => {}
    }
}

struct Gen {
    lines: Vec<String>,
    tmp: u32,
    lab: u32,
    cur: String,
    terminated: bool,
    loops: Vec<(String, String)>,
    ret_ty: CType,
    /// A whole-struct copy used `llvm.memmove`, which keeps padding bytes.
    needs_memmove: bool,
}

impl Gen {
    fn new(ret_ty: CType) -> Self {
        Self {
            lines: Vec::new(),
            tmp: 0,
            lab: 0,
            cur: String::new(),
            terminated: false,
            loops: Vec::new(),
            ret_ty,
            needs_memmove: false,
        }
    }
}

impl Gen {
    fn emit(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    fn fresh(&mut self) -> String {
        self.tmp += 1;
        format!("%t{}", self.tmp)
    }

    fn label(&mut self) -> String {
        self.lab += 1;
        format!("L{}", self.lab)
    }

    fn ensure_block(&mut self) {
        if self.terminated {
            let label = self.label();
            self.start(label);
        }
    }

    fn start(&mut self, name: String) {
        assert!(
            self.terminated,
            "internal compiler error: fallthrough into {name}"
        );
        self.emit(&format!("{name}:"));
        self.cur = name;
        self.terminated = false;
    }

    fn br(&mut self, label: &str) {
        assert!(
            !self.terminated,
            "internal compiler error: branch after terminator"
        );
        self.emit(&format!("  br label %{label}"));
        self.terminated = true;
    }

    fn br_cond(&mut self, cond: &str, then_label: &str, else_label: &str) {
        assert!(
            !self.terminated,
            "internal compiler error: branch after terminator"
        );
        self.emit(&format!(
            "  br i1 {cond}, label %{then_label}, label %{else_label}"
        ));
        self.terminated = true;
    }

    fn gen_compound(&mut self, compound: &Compound) {
        for item in &compound.items {
            self.gen_stmt(item);
        }
    }

    fn gen_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Compound(compound) => self.gen_compound(compound),
            Stmt::Decl(decl) => {
                self.ensure_block();
                self.gen_decl(decl);
            }
            Stmt::Return { expr, .. } => {
                self.ensure_block();
                if let Some(expr) = expr {
                    let value = self.gen_val(expr);
                    let value =
                        self.convert(&value, &expr.ty().expect("typed"), &self.ret_ty.clone());
                    self.emit(&format!("  ret {} {value}", self.ret_ty.ir_type()));
                } else {
                    self.emit("  ret void");
                }
                self.terminated = true;
            }
            Stmt::If {
                cond,
                then_stmt,
                else_stmt,
                ..
            } => self.gen_if(cond, then_stmt, else_stmt.as_deref()),
            Stmt::While { cond, body, .. } => self.gen_while(cond, body),
            Stmt::For {
                init,
                cond,
                step,
                body,
                ..
            } => self.gen_for(init, cond.as_ref(), step.as_ref(), body),
            Stmt::Expr { expr, .. } => {
                self.ensure_block();
                if let Some(expr) = expr {
                    let _ = self.gen_expr(expr);
                }
            }
            Stmt::Break { .. } => {
                self.ensure_block();
                let end = self.loops.last().expect("break outside loop").1.clone();
                self.br(&end);
            }
            Stmt::Continue { .. } => {
                self.ensure_block();
                let cont = self.loops.last().expect("continue outside loop").0.clone();
                self.br(&cont);
            }
        }
    }

    fn gen_decl(&mut self, decl: &VarDecl) {
        let Some(checked) = &decl.checked else {
            return;
        };
        match checked {
            CheckedInit::Scalar(init) => {
                if decl.ty.is_record() {
                    if let Some(src) = self.struct_bytes(init) {
                        self.copy_struct(&decl.ty, &src, &decl.slot);
                        return;
                    }
                }
                let value = self.gen_val(init);
                let value = self.convert(&value, &init.ty().expect("typed"), &decl.ty);
                self.store(&decl.ty, &value, &decl.slot);
            }
            CheckedInit::Aggregate(stores) => {
                let mut ready = Vec::new();
                for store in stores {
                    let value = self.gen_val(&store.expr);
                    let value = self.convert(&value, &store.expr.ty().expect("typed"), &store.ty);
                    ready.push((store.offset, store.ty.clone(), value));
                }
                self.emit(&format!(
                    "  store {} zeroinitializer, ptr {}, align {}",
                    decl.ty.ir_type(),
                    decl.slot,
                    decl.ty.align()
                ));
                for (offset, ty, value) in ready {
                    let addr = if offset == 0 {
                        decl.slot.clone()
                    } else {
                        let tmp = self.fresh();
                        self.emit(&format!(
                            "  {tmp} = getelementptr inbounds i8, ptr {}, i64 {offset}",
                            decl.slot
                        ));
                        tmp
                    };
                    self.store(&ty, &value, &addr);
                }
            }
        }
    }

    /// Byte copy of a struct, including padding. `llvm.memmove` allows the
    /// source and destination to be the same object.
    fn copy_struct(&mut self, ty: &CType, src: &str, dest: &str) {
        let size = ty.size().expect("complete struct");
        self.needs_memmove = true;
        self.emit(&format!(
            "  call void @llvm.memmove.p0.p0.i64(ptr {dest}, ptr {src}, i64 {size}, i1 false)"
        ));
    }

    /// Address of a struct object whose bytes should be copied. An assignment
    /// result is the object just stored, so `u = (s = t)` copies from `s`.
    fn struct_bytes(&mut self, expr: &Expr) -> Option<String> {
        if expr.ty().is_some_and(|ty| ty.is_record()) && expr.is_lvalue() {
            return Some(self.gen_addr(expr));
        }
        if let Expr::Assign { target, .. } = expr {
            if target.ty().is_some_and(|ty| ty.is_record()) {
                let _ = self.gen_expr(expr);
                return Some(self.gen_addr(target));
            }
        }
        None
    }

    fn store(&mut self, ty: &CType, value: &str, slot: &str) {
        self.emit(&format!(
            "  store {} {value}, ptr {slot}, align {}",
            ty.ir_type(),
            ty.align()
        ));
    }

    fn load(&mut self, ty: &CType, slot: &str) -> String {
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = load {}, ptr {slot}, align {}",
            ty.ir_type(),
            ty.align()
        ));
        tmp
    }

    /// Convert `value` from `from` to `to`.
    ///
    /// Integer conversions are `sext`, `zext`, or `trunc`. Same-width
    /// signed/unsigned changes need no instruction. Pointer-to-pointer is a
    /// no-op with opaque pointers. Integer-to-pointer sign- or zero-extends
    /// to i64 and uses `inttoptr`; a constant 0 becomes `null`. Pointer-to-
    /// integer is `ptrtoint` to the destination width. That is the
    /// implementation-defined conversion Clang uses on x86_64-pc-linux-gnu.
    fn convert(&mut self, value: &str, from: &CType, to: &CType) -> String {
        // C11 6.3.1.2: a pointer converts to `_Bool` as 0 when null and 1
        // otherwise. Truncating the address to a byte is not that conversion.
        if from.is_pointer() && to.as_integer() == Some(TypeName::Bool) {
            let cmp = self.fresh();
            let tmp = self.fresh();
            self.emit(&format!("  {cmp} = icmp ne ptr {value}, null"));
            self.emit(&format!("  {tmp} = zext i1 {cmp} to i8"));
            return tmp;
        }
        if let (Some(from_i), Some(to_i)) = (from.as_integer(), to.as_integer()) {
            // `_Bool` is 0 or 1. A same-width truncation would keep the low
            // byte, so `( _Bool )2` would stay 2.
            if to_i == TypeName::Bool && from_i != TypeName::Bool {
                let cmp = self.fresh();
                let tmp = self.fresh();
                self.emit(&format!(
                    "  {cmp} = icmp ne {} {value}, 0",
                    from_i.ir_type()
                ));
                self.emit(&format!("  {tmp} = zext i1 {cmp} to i8"));
                return tmp;
            }
            if from_i.width() == to_i.width() {
                return value.to_string();
            }
            let op = if to_i.width() < from_i.width() {
                "trunc"
            } else if from_i.is_signed() {
                "sext"
            } else {
                "zext"
            };
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = {op} {} {value} to {}",
                from_i.ir_type(),
                to_i.ir_type()
            ));
            return tmp;
        }
        if from.is_pointer() && to.is_pointer() {
            return value.to_string();
        }
        if from.is_integer() && to.is_pointer() {
            if value == "0" {
                return "null".to_string();
            }
            let wide = self.convert(value, from, &CType::from_name(TypeName::Long));
            let tmp = self.fresh();
            self.emit(&format!("  {tmp} = inttoptr i64 {wide} to ptr"));
            return tmp;
        }
        if from.is_pointer() && to.is_integer() {
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = ptrtoint ptr {value} to {}",
                to.ir_type()
            ));
            return tmp;
        }
        value.to_string()
    }

    fn extend_i64(&mut self, value: &str, ty: &CType) -> String {
        self.convert(value, ty, &CType::from_name(TypeName::Long))
    }

    /// A pointer operand, or a null pointer constant turned into `ptr null`.
    fn as_ptr(&mut self, expr: &Expr) -> String {
        let value = self.gen_val(expr);
        let ty = expr.ty().expect("typed");
        if ty.is_pointer() {
            value
        } else {
            self.convert(&value, &ty, &CType::pointer(CType::Void))
        }
    }

    fn gen_if(&mut self, cond: &Expr, then_stmt: &Stmt, else_stmt: Option<&Stmt>) {
        self.ensure_block();
        let cond = self.gen_cond(cond);
        let then_label = self.label();
        let else_label = self.label();
        let end_label = self.label();
        if else_stmt.is_some() {
            self.br_cond(&cond, &then_label, &else_label);
        } else {
            self.br_cond(&cond, &then_label, &end_label);
        }
        self.start(then_label);
        self.gen_stmt(then_stmt);
        if !self.terminated {
            self.br(&end_label);
        }
        if let Some(else_stmt) = else_stmt {
            self.start(else_label);
            self.gen_stmt(else_stmt);
            if !self.terminated {
                self.br(&end_label);
            }
        }
        self.start(end_label);
    }

    fn gen_while(&mut self, cond: &Expr, body: &Stmt) {
        self.ensure_block();
        let cond_label = self.label();
        let body_label = self.label();
        let end_label = self.label();
        self.br(&cond_label);
        self.start(cond_label.clone());
        let cond = self.gen_cond(cond);
        self.br_cond(&cond, &body_label, &end_label);
        self.start(body_label);
        self.loops.push((cond_label.clone(), end_label.clone()));
        self.gen_stmt(body);
        self.loops.pop();
        if !self.terminated {
            self.br(&cond_label);
        }
        self.start(end_label);
    }

    fn gen_for(&mut self, init: &ForInit, cond: Option<&Expr>, step: Option<&Expr>, body: &Stmt) {
        self.ensure_block();
        match init {
            ForInit::Empty => {}
            ForInit::Decl(decls) => {
                for decl in decls {
                    self.gen_decl(decl);
                }
            }
            ForInit::Expr(expr) => {
                let _ = self.gen_expr(expr);
            }
        }
        let cond_label = self.label();
        let body_label = self.label();
        let step_label = self.label();
        let end_label = self.label();
        self.br(&cond_label);
        self.start(cond_label.clone());
        if let Some(cond) = cond {
            let cond = self.gen_cond(cond);
            self.br_cond(&cond, &body_label, &end_label);
        } else {
            self.br(&body_label);
        }
        self.start(body_label);
        self.loops.push((step_label.clone(), end_label.clone()));
        self.gen_stmt(body);
        self.loops.pop();
        if !self.terminated {
            self.br(&step_label);
        }
        self.start(step_label);
        if let Some(step) = step {
            let _ = self.gen_expr(step);
        }
        self.br(&cond_label);
        self.start(end_label);
    }

    fn gen_val(&mut self, expr: &Expr) -> String {
        self.gen_expr(expr)
            .expect("internal compiler error: void expression used as a value")
    }

    fn gen_expr(&mut self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Int { bits, value_ty, .. } => Some(value_ty.format_bits(*bits)),
            Expr::Var { resolved, ty, .. } => {
                let slot = &resolved.as_ref().expect("resolved variable").slot;
                let ty = ty.clone().expect("typed variable");
                if ty.is_array() || ty.is_func() {
                    Some(slot.clone())
                } else {
                    Some(self.load(&ty, slot))
                }
            }
            Expr::Unary {
                op: UnaryOp::Not, ..
            } => Some(self.zext_cond(expr)),
            Expr::Unary {
                op: UnaryOp::Plus,
                expr,
                ..
            } => {
                let value = self.gen_val(expr);
                let src = expr.ty().expect("typed");
                let promoted = CType::from_name(src.as_integer().expect("integer").promote());
                Some(self.convert(&value, &src, &promoted))
            }
            Expr::Unary {
                op: UnaryOp::Minus,
                expr,
                ..
            } => Some(self.gen_neg_or_not(expr, "sub")),
            Expr::Unary {
                op: UnaryOp::BitNot,
                expr,
                ..
            } => Some(self.gen_neg_or_not(expr, "xor")),
            Expr::Unary {
                op: UnaryOp::Addr,
                expr,
                ..
            } => Some(self.gen_addr(expr)),
            Expr::Unary {
                op: UnaryOp::Deref,
                expr,
                ty,
                ..
            } => {
                let addr = self.gen_val(expr);
                let ty = ty.clone().expect("typed");
                if ty.is_void() {
                    None
                } else if ty.is_array() {
                    Some(addr)
                } else {
                    Some(self.load(&ty, &addr))
                }
            }
            Expr::Bin { op, .. } if op.is_cmp() || matches!(op, BinOp::And | BinOp::Or) => {
                Some(self.zext_cond(expr))
            }
            Expr::Bin {
                op: BinOp::Sub,
                left,
                right,
                ..
            } if left.ty().is_some_and(|ty| ty.is_pointer())
                && right.ty().is_some_and(|ty| ty.is_pointer()) =>
            {
                Some(self.gen_ptr_diff(left, right))
            }
            Expr::Bin {
                op,
                left,
                right,
                ty,
                ..
            } if ty.as_ref().is_some_and(|ty| ty.is_pointer()) => {
                Some(self.gen_ptr_arith(*op, left, right))
            }
            Expr::Bin {
                op, left, right, ..
            } => Some(self.gen_arith(*op, left, right)),
            Expr::Assign { target, value, .. } => {
                let addr = self.gen_addr(target);
                let dest = target.ty().expect("lvalue type");
                if dest.is_record() {
                    if let Some(src) = self.struct_bytes(value) {
                        self.copy_struct(&dest, &src, &addr);
                    } else {
                        let raw = self.gen_val(value);
                        self.store(&dest, &raw, &addr);
                    }
                    Some(self.load(&dest, &addr))
                } else {
                    let raw = self.gen_val(value);
                    let stored = self.convert(&raw, &value.ty().expect("typed"), &dest);
                    self.store(&dest, &stored, &addr);
                    Some(stored)
                }
            }
            Expr::CompoundAssign {
                op, target, value, ..
            } => Some(self.gen_compound_assign(*op, target, value)),
            Expr::IncDec { op, target, .. } => Some(self.gen_incdec(*op, target)),
            Expr::Cond {
                cond,
                then_expr,
                else_expr,
                ty,
                ..
            } => self.gen_ternary(cond, then_expr, else_expr, ty.clone().expect("typed")),
            Expr::Comma { left, right, .. } => {
                let _ = self.gen_expr(left);
                self.gen_expr(right)
            }
            Expr::Sizeof { operand, .. } => {
                let size = match operand {
                    SizeofOperand::Type(ty) => ty.size(),
                    SizeofOperand::Expr(inner) => inner.ty().expect("typed").size(),
                }
                .expect("sizeof of a complete type");
                Some(TypeName::ULong.format_bits(size))
            }
            Expr::Cast { ty_name, expr, .. } => {
                if ty_name.is_void() {
                    let _ = self.gen_expr(expr);
                    None
                } else {
                    let value = self.gen_val(expr);
                    Some(self.convert(&value, &expr.ty().expect("typed"), ty_name))
                }
            }
            Expr::Index {
                base, index, ty, ..
            } => {
                let addr = self.gen_ptr_index(base, index);
                let ty = ty.clone().expect("typed");
                if ty.is_array() {
                    Some(addr)
                } else {
                    Some(self.load(&ty, &addr))
                }
            }
            Expr::Decay { expr, .. } => Some(self.gen_addr(expr)),
            Expr::Str { .. } => Some(self.gen_addr(expr)),
            Expr::Member { ty, .. } => {
                let addr = self.gen_addr(expr);
                let ty = ty.clone().expect("typed");
                if ty.is_array() {
                    Some(addr)
                } else {
                    Some(self.load(&ty, &addr))
                }
            }
            Expr::Call {
                callee,
                args,
                resolved,
                ..
            } => self.gen_call(callee, args, resolved.as_ref().expect("resolved call")),
        }
    }

    fn gen_addr(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Var { resolved, .. } => {
                resolved.as_ref().expect("resolved variable").slot.clone()
            }
            Expr::Unary {
                op: UnaryOp::Deref,
                expr,
                ..
            } => self.gen_val(expr),
            Expr::Index { base, index, .. } => self.gen_ptr_index(base, index),
            Expr::Decay { expr, .. } => self.gen_addr(expr),
            Expr::Str { symbol, .. } => llvm_global(symbol.as_deref().expect("string symbol")),
            Expr::Member {
                base,
                arrow,
                index,
                offset,
                bytewise,
                ..
            } => {
                let base_ptr = if *arrow {
                    self.gen_val(base)
                } else if base.is_lvalue() {
                    self.gen_addr(base)
                } else {
                    self.spill(base)
                };
                let tmp = self.fresh();
                if *bytewise {
                    let offset = offset.expect("member offset");
                    self.emit(&format!(
                        "  {tmp} = getelementptr inbounds i8, ptr {base_ptr}, i64 {offset}"
                    ));
                } else {
                    let struct_ty = if *arrow {
                        base.ty()
                            .as_ref()
                            .and_then(CType::pointee)
                            .expect("pointer to struct")
                            .clone()
                    } else {
                        base.ty().expect("struct")
                    };
                    let index = index.expect("member index");
                    self.emit(&format!(
                        "  {tmp} = getelementptr inbounds {}, ptr {base_ptr}, i32 0, i32 {index}",
                        struct_ty.ir_type()
                    ));
                }
                tmp
            }
            _ => panic!("internal compiler error: address of a non-lvalue"),
        }
    }

    /// Materialize a struct rvalue so a member can be addressed.
    fn spill(&mut self, expr: &Expr) -> String {
        let ty = expr.ty().expect("typed");
        let value = self.gen_val(expr);
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = alloca {}, align {}",
            ty.ir_type(),
            ty.align()
        ));
        self.store(&ty, &value, &tmp);
        tmp
    }

    fn gen_ptr_index(&mut self, base: &Expr, index: &Expr) -> String {
        let (ptr_expr, int_expr) = if base.ty().is_some_and(|ty| ty.is_pointer()) {
            (base, index)
        } else {
            (index, base)
        };
        self.gen_gep(ptr_expr, int_expr, false)
    }

    fn gen_ptr_arith(&mut self, op: BinOp, left: &Expr, right: &Expr) -> String {
        let (ptr_expr, int_expr, negate) = if left.ty().is_some_and(|ty| ty.is_pointer()) {
            (left, right, op == BinOp::Sub)
        } else {
            (right, left, false)
        };
        self.gen_gep(ptr_expr, int_expr, negate)
    }

    fn gen_gep(&mut self, ptr_expr: &Expr, int_expr: &Expr, negate: bool) -> String {
        let ptr = self.gen_val(ptr_expr);
        let idx = self.gen_val(int_expr);
        let mut idx = self.extend_i64(&idx, &int_expr.ty().expect("integer index"));
        if negate {
            let tmp = self.fresh();
            self.emit(&format!("  {tmp} = sub i64 0, {idx}"));
            idx = tmp;
        }
        let elem = ptr_expr
            .ty()
            .as_ref()
            .and_then(CType::pointee)
            .expect("pointer")
            .ir_type();
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = getelementptr inbounds {elem}, ptr {ptr}, i64 {idx}"
        ));
        tmp
    }

    fn gen_ptr_diff(&mut self, left: &Expr, right: &Expr) -> String {
        let left_val = self.gen_val(left);
        let right_val = self.gen_val(right);
        let left_int = self.fresh();
        let right_int = self.fresh();
        self.emit(&format!("  {left_int} = ptrtoint ptr {left_val} to i64"));
        self.emit(&format!("  {right_int} = ptrtoint ptr {right_val} to i64"));
        let diff = self.fresh();
        self.emit(&format!("  {diff} = sub i64 {left_int}, {right_int}"));
        let size = left
            .ty()
            .as_ref()
            .and_then(CType::pointee)
            .and_then(CType::size)
            .expect("complete pointee");
        if size == 1 {
            return diff;
        }
        let tmp = self.fresh();
        self.emit(&format!("  {tmp} = sdiv exact i64 {diff}, {size}"));
        tmp
    }

    fn gen_neg_or_not(&mut self, expr: &Expr, kind: &str) -> String {
        let src = expr.ty().expect("typed");
        let dest = CType::from_name(src.as_integer().expect("integer").promote());
        let value = self.gen_val(expr);
        let value = self.convert(&value, &src, &dest);
        let tmp = self.fresh();
        if kind == "sub" {
            self.emit(&format!("  {tmp} = sub {} 0, {value}", dest.ir_type()));
        } else {
            self.emit(&format!("  {tmp} = xor {} {value}, -1", dest.ir_type()));
        }
        tmp
    }

    fn gen_arith(&mut self, op: BinOp, left: &Expr, right: &Expr) -> String {
        let left_ty = left.ty().expect("typed");
        let right_ty = right.ty().expect("typed");
        let left_i = left_ty.as_integer().expect("integer");
        let right_i = right_ty.as_integer().expect("integer");
        let common = if matches!(op, BinOp::Shl | BinOp::Shr) {
            left_i.promote()
        } else {
            left_i.usual(right_i)
        };
        let common_ty = CType::from_name(common);
        let left_val = self.gen_val(left);
        let left_val = self.convert(&left_val, &left_ty, &common_ty);
        let right_val = self.gen_val(right);
        let right_val = self.convert(&right_val, &right_ty, &common_ty);
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = {} {} {left_val}, {right_val}",
            value_inst(op, common.is_signed()),
            common.ir_type()
        ));
        tmp
    }

    fn gen_compound_assign(&mut self, op: BinOp, target: &Expr, value: &Expr) -> String {
        let dest = target.ty().expect("lvalue type");
        let rhs_ty = value.ty().expect("typed");
        // The right-hand side is evaluated before the lvalue is loaded. The
        // address is still computed only once.
        let rhs = self.gen_val(value);
        let addr = self.gen_addr(target);
        let old = self.load(&dest, &addr);
        if dest.is_pointer() {
            let mut idx = self.extend_i64(&rhs, &rhs_ty);
            if op == BinOp::Sub {
                let tmp = self.fresh();
                self.emit(&format!("  {tmp} = sub i64 0, {idx}"));
                idx = tmp;
            }
            let elem = dest.pointee().expect("pointer").ir_type();
            let updated = self.fresh();
            self.emit(&format!(
                "  {updated} = getelementptr inbounds {elem}, ptr {old}, i64 {idx}"
            ));
            self.store(&dest, &updated, &addr);
            return updated;
        }
        let dest_i = dest.as_integer().expect("integer");
        let rhs_i = rhs_ty.as_integer().expect("integer");
        let common = if matches!(op, BinOp::Shl | BinOp::Shr) {
            dest_i.promote()
        } else {
            dest_i.usual(rhs_i)
        };
        let common_ty = CType::from_name(common);
        let old_c = self.convert(&old, &dest, &common_ty);
        let rhs_c = self.convert(&rhs, &rhs_ty, &common_ty);
        let updated = self.fresh();
        self.emit(&format!(
            "  {updated} = {} {} {old_c}, {rhs_c}",
            value_inst(op, common.is_signed()),
            common.ir_type()
        ));
        let stored = self.convert(&updated, &common_ty, &dest);
        self.store(&dest, &stored, &addr);
        stored
    }

    fn gen_incdec(&mut self, op: IncOp, target: &Expr) -> String {
        let dest = target.ty().expect("lvalue type");
        let addr = self.gen_addr(target);
        let old = self.load(&dest, &addr);
        if dest.is_pointer() {
            let elem = dest.pointee().expect("pointer").ir_type();
            let step = if op.is_increment() { "1" } else { "-1" };
            let updated = self.fresh();
            self.emit(&format!(
                "  {updated} = getelementptr inbounds {elem}, ptr {old}, i64 {step}"
            ));
            self.store(&dest, &updated, &addr);
            return if op.is_prefix() { updated } else { old };
        }
        let dest_i = dest.as_integer().expect("integer");
        let common = dest_i.usual(TypeName::Int);
        let common_ty = CType::from_name(common);
        let old_c = self.convert(&old, &dest, &common_ty);
        let updated = self.fresh();
        let inst = if op.is_increment() { "add" } else { "sub" };
        self.emit(&format!(
            "  {updated} = {inst} {} {old_c}, 1",
            common.ir_type()
        ));
        let stored = self.convert(&updated, &common_ty, &dest);
        self.store(&dest, &stored, &addr);
        if op.is_prefix() {
            stored
        } else {
            old
        }
    }

    fn gen_call(&mut self, callee: &Expr, args: &[Expr], resolved: &CallRes) -> Option<String> {
        let mut rendered = Vec::new();
        for (index, arg) in args.iter().enumerate() {
            let value = self.gen_val(arg);
            let src = arg.ty().expect("typed");
            if index < resolved.params.len() {
                let param_ty = &resolved.params[index];
                let value = self.convert(&value, &src, param_ty);
                rendered.push(param_ir(param_ty, Some(&value)));
            } else {
                // Default argument promotions (C11 6.5.2.2): integer types
                // narrower than `int` become `int`. There are no floats yet.
                let promoted = default_promote(&src);
                let value = self.convert(&value, &src, &promoted);
                rendered.push(format!("{} {value}", promoted.ir_type()));
            }
        }
        let arglist = rendered.join(", ");
        let callee = if resolved.indirect {
            self.gen_val(callee)
        } else {
            llvm_global(&resolved.name)
        };
        // A variadic call names the function type, including `...`
        // (`call i32 (ptr, ...) @printf(...)`). Other calls name only the
        // return type, which is what the existing tests match.
        let ty = if resolved.variadic {
            variadic_fn_type(&resolved.ret, &resolved.params)
        } else {
            resolved.ret.ir_type()
        };
        if resolved.ret.is_void() {
            self.emit(&format!("  call {ty} {callee}({arglist})"));
            None
        } else {
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = call {}{ty} {callee}({arglist})",
                ret_prefix(&resolved.ret)
            ));
            Some(tmp)
        }
    }

    fn zext_cond(&mut self, expr: &Expr) -> String {
        let cond = self.gen_cond(expr);
        let tmp = self.fresh();
        self.emit(&format!("  {tmp} = zext i1 {cond} to i32"));
        tmp
    }

    fn gen_cond(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Bin {
                op, left, right, ..
            } if op.is_cmp() => {
                let left_ty = left.ty().expect("typed");
                let right_ty = right.ty().expect("typed");
                if left_ty.is_pointer() || right_ty.is_pointer() {
                    let left_val = self.as_ptr(left);
                    let right_val = self.as_ptr(right);
                    let tmp = self.fresh();
                    self.emit(&format!(
                        "  {tmp} = icmp {} ptr {left_val}, {right_val}",
                        ptr_pred(*op)
                    ));
                    return tmp;
                }
                let left_i = left_ty.as_integer().expect("integer");
                let right_i = right_ty.as_integer().expect("integer");
                let common = left_i.usual(right_i);
                let common_ty = CType::from_name(common);
                let left_val = self.gen_val(left);
                let left_val = self.convert(&left_val, &left_ty, &common_ty);
                let right_val = self.gen_val(right);
                let right_val = self.convert(&right_val, &right_ty, &common_ty);
                let tmp = self.fresh();
                self.emit(&format!(
                    "  {tmp} = icmp {} {} {left_val}, {right_val}",
                    cmp_pred(*op, common.is_signed()),
                    common.ir_type()
                ));
                tmp
            }
            Expr::Bin {
                op: BinOp::And,
                left,
                right,
                ..
            } => self.gen_and(left, right),
            Expr::Bin {
                op: BinOp::Or,
                left,
                right,
                ..
            } => self.gen_or(left, right),
            Expr::Unary {
                op: UnaryOp::Not,
                expr,
                ..
            } => {
                let inner = self.gen_cond(expr);
                let tmp = self.fresh();
                self.emit(&format!("  {tmp} = xor i1 {inner}, true"));
                tmp
            }
            _ => {
                let value = self.gen_val(expr);
                let ty = expr.ty().expect("typed");
                let tmp = self.fresh();
                if ty.is_pointer() {
                    self.emit(&format!("  {tmp} = icmp ne ptr {value}, null"));
                } else {
                    self.emit(&format!("  {tmp} = icmp ne {} {value}, 0", ty.ir_type()));
                }
                tmp
            }
        }
    }

    fn gen_and(&mut self, left: &Expr, right: &Expr) -> String {
        let left = self.gen_cond(left);
        let rhs_label = self.label();
        let false_label = self.label();
        let end_label = self.label();
        self.br_cond(&left, &rhs_label, &false_label);
        self.start(rhs_label);
        let right = self.gen_cond(right);
        let rhs_block = self.cur.clone();
        self.br(&end_label);
        self.start(false_label.clone());
        self.br(&end_label);
        self.start(end_label);
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = phi i1 [ {right}, %{rhs_block} ], [ false, %{false_label} ]"
        ));
        tmp
    }

    /// Only the chosen arm is evaluated. Each arm is converted to `result`
    /// in its own block, so the phi operands are defined in the predecessors.
    fn gen_ternary(
        &mut self,
        cond: &Expr,
        then_expr: &Expr,
        else_expr: &Expr,
        result: CType,
    ) -> Option<String> {
        let cond = self.gen_cond(cond);
        let then_label = self.label();
        let else_label = self.label();
        let end_label = self.label();
        self.br_cond(&cond, &then_label, &else_label);
        self.start(then_label);
        let then_val = self
            .gen_expr(then_expr)
            .map(|value| self.convert(&value, &then_expr.ty().expect("typed"), &result));
        assert!(
            !self.terminated,
            "internal compiler error: expression terminated a block"
        );
        let then_block = self.cur.clone();
        self.br(&end_label);
        self.start(else_label);
        let else_val = self
            .gen_expr(else_expr)
            .map(|value| self.convert(&value, &else_expr.ty().expect("typed"), &result));
        assert!(
            !self.terminated,
            "internal compiler error: expression terminated a block"
        );
        let else_block = self.cur.clone();
        self.br(&end_label);
        self.start(end_label);
        match (then_val, else_val) {
            (Some(then_val), Some(else_val)) => {
                let tmp = self.fresh();
                self.emit(&format!(
                    "  {tmp} = phi {} [ {then_val}, %{then_block} ], [ {else_val}, %{else_block} ]",
                    result.ir_type()
                ));
                Some(tmp)
            }
            (None, None) => None,
            _ => unreachable!("conditional arms have different types"),
        }
    }

    fn gen_or(&mut self, left: &Expr, right: &Expr) -> String {
        let left = self.gen_cond(left);
        let true_label = self.label();
        let rhs_label = self.label();
        let end_label = self.label();
        self.br_cond(&left, &true_label, &rhs_label);
        self.start(rhs_label);
        let right = self.gen_cond(right);
        let rhs_block = self.cur.clone();
        self.br(&end_label);
        self.start(true_label.clone());
        self.br(&end_label);
        self.start(end_label);
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = phi i1 [ true, %{true_label} ], [ {right}, %{rhs_block} ]"
        ));
        tmp
    }
}

/// LLVM string constant. The Language Reference ends the string at the first
/// `"`. The only escapes are `\\` and `\xx` (two hex digits); a quote is `\22`.
fn llvm_string(text: &str) -> String {
    llvm_bytes(text.as_bytes())
}

fn llvm_bytes(bytes: &[u8]) -> String {
    let mut out = String::from("\"");
    for &byte in bytes {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\22"),
            0x20..=0x7e => out.push(byte as char),
            _ => out.push_str(&format!("\\{byte:02X}")),
        }
    }
    out.push('"');
    out
}

/// `i32 (ptr, ...)` — the function type written on a variadic call.
fn variadic_fn_type(ret: &CType, params: &[CType]) -> String {
    let mut parts: Vec<String> = params.iter().map(|ty| ty.ir_type()).collect();
    parts.push("...".to_string());
    format!("{} ({})", ret.ir_type(), parts.join(", "))
}

/// Default argument promotions. Narrow integers become `int`. Pointers are
/// unchanged. Floats are not in this subset.
fn default_promote(ty: &CType) -> CType {
    match ty.as_integer() {
        Some(int) => CType::from_name(int.promote()),
        None => ty.clone(),
    }
}

/// `@name` when `name` matches LLVM's unquoted identifier pattern, otherwise
/// a quoted global. C identifiers already match the unquoted pattern.
fn llvm_global(name: &str) -> String {
    if is_unquoted_llvm_name(name) {
        format!("@{name}")
    } else {
        format!("@{}", llvm_string(name))
    }
}

fn value_inst(op: BinOp, signed: bool) -> &'static str {
    match op {
        BinOp::Add => "add",
        BinOp::Sub => "sub",
        BinOp::Mul => "mul",
        BinOp::Div => {
            if signed {
                "sdiv"
            } else {
                "udiv"
            }
        }
        BinOp::Mod => {
            if signed {
                "srem"
            } else {
                "urem"
            }
        }
        BinOp::BitAnd => "and",
        BinOp::BitOr => "or",
        BinOp::BitXor => "xor",
        BinOp::Shl => "shl",
        BinOp::Shr => {
            if signed {
                "ashr"
            } else {
                "lshr"
            }
        }
        BinOp::Eq
        | BinOp::Ne
        | BinOp::Lt
        | BinOp::Gt
        | BinOp::Le
        | BinOp::Ge
        | BinOp::And
        | BinOp::Or => {
            unreachable!("comparison or logical operator")
        }
    }
}

fn ptr_pred(op: BinOp) -> &'static str {
    match op {
        BinOp::Eq => "eq",
        BinOp::Ne => "ne",
        BinOp::Lt => "ult",
        BinOp::Gt => "ugt",
        BinOp::Le => "ule",
        BinOp::Ge => "uge",
        _ => unreachable!("pointer comparison"),
    }
}

/// A union's LLVM type is `{ [N x iK] }` with the union's alignment. The
/// constant is the first member's bytes, little endian, padded with zeros.
fn render_union_const(ty: &CType, value: &ConstInit) -> String {
    let size = ty.size().unwrap_or(0) as usize;
    let align = usize::try_from(ty.align().max(1)).unwrap_or(1);
    let mut bytes = vec![0u8; size];
    // Semantic analysis rejects an address constant in a union before this
    // runs. A `false` result here is a bug in that check, not a zero.
    assert!(
        write_const_bytes(ty, value, &mut bytes, 0),
        "union initializer contains a value that is not a byte constant"
    );
    if bytes.iter().all(|byte| *byte == 0) {
        return "zeroinitializer".to_string();
    }
    let (elem, step) = match align {
        2 => ("i16", 2),
        4 => ("i32", 4),
        8 => ("i64", 8),
        16 => ("i128", 16),
        _ => ("i8", 1),
    };
    let count = if step == 0 || bytes.len() % step != 0 {
        bytes.len()
    } else {
        bytes.len() / step
    };
    let step = if bytes.len() % step != 0 { 1 } else { step };
    let elem = if step == 1 { "i8" } else { elem };
    let mut parts = Vec::new();
    for index in 0..count {
        let mut acc: u128 = 0;
        for byte in 0..step {
            let at = index * step + byte;
            if at < bytes.len() {
                acc |= u128::from(bytes[at]) << (8 * byte);
            }
        }
        parts.push(format!("{elem} {acc}"));
    }
    format!("{{ [ {count} x {elem} ] [ {} ] }}", parts.join(", "))
}

fn write_const_bytes(ty: &CType, value: &ConstInit, bytes: &mut [u8], offset: usize) -> bool {
    match value {
        ConstInit::Zero => true,
        ConstInit::Int(bits) => {
            let Some(int) = ty.as_integer() else {
                return false;
            };
            let width = (int.width() / 8) as usize;
            for index in 0..width {
                let at = offset + index;
                if at < bytes.len() {
                    bytes[at] = (*bits >> (8 * index)) as u8;
                }
            }
            true
        }
        ConstInit::Aggregate(items) => {
            if ty.is_union() {
                let Some(field) = ty.struct_ref().and_then(|handle| {
                    handle.fields().and_then(|fields| fields.into_iter().next())
                }) else {
                    return true;
                };
                let Some(item) = items.first() else {
                    return true;
                };
                return write_const_bytes(&field.ty, item, bytes, offset);
            }
            if let Some(fields) = ty.struct_ref().and_then(StructRef::fields) {
                for (field, item) in fields.iter().zip(items) {
                    let at = offset + field.offset as usize;
                    if !write_const_bytes(&field.ty, item, bytes, at) {
                        return false;
                    }
                }
                return true;
            }
            if let (Some(elem), Some(len)) = (ty.array_elem(), ty.array_len()) {
                let step = elem.size().unwrap_or(0) as usize;
                for index in 0..len as usize {
                    let Some(item) = items.get(index) else {
                        break;
                    };
                    if !write_const_bytes(elem, item, bytes, offset + index * step) {
                        return false;
                    }
                }
                return true;
            }
            false
        }
        ConstInit::Ptr(_) | ConstInit::PtrToInt { .. } => false,
    }
}

fn render_const(ty: &CType, value: &ConstInit) -> String {
    if ty.is_union() {
        return render_union_const(ty, value);
    }
    match value {
        ConstInit::Zero => zero_const_spelling(ty),
        ConstInit::Int(bits) => ty
            .as_integer()
            .expect("integer constant")
            .format_bits(*bits),
        ConstInit::Ptr(ptr) => render_ptr(ptr),
        ConstInit::PtrToInt { ptr, width } => {
            let ir = match width {
                8 => "i8",
                16 => "i16",
                32 => "i32",
                _ => "i64",
            };
            format!("ptrtoint ({} to {ir})", render_ptr_operand(ptr))
        }
        ConstInit::Aggregate(items) => {
            if let Some(fields) = ty.struct_ref().and_then(StructRef::fields) {
                if fields.len() != items.len() {
                    return "zeroinitializer".to_string();
                }
                let parts = items
                    .iter()
                    .zip(fields.iter())
                    .map(|(item, field)| {
                        format!("{} {}", field.ty.ir_type(), render_const(&field.ty, item))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                return format!("{{ {parts} }}");
            }
            let Some(elem) = ty.array_elem() else {
                return "zeroinitializer".to_string();
            };
            if let Some(bytes) = i8_bytes(ty, items) {
                return format!("c{}", llvm_bytes(&bytes));
            }
            let parts = items
                .iter()
                .map(|item| format!("{} {}", elem.ir_type(), render_const(elem, item)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{parts}]")
        }
    }
}

/// A flat array of byte-sized integers can be spelled as an LLVM `c"..."`
/// constant. Wider elements keep the `[i32 1, i32 2]` form.
fn i8_bytes(ty: &CType, items: &[ConstInit]) -> Option<Vec<u8>> {
    let elem = ty.array_elem()?;
    if elem.as_integer().map(TypeName::width) != Some(8) {
        return None;
    }
    if items.len() != ty.array_len()? as usize {
        return None;
    }
    let mut bytes = Vec::with_capacity(items.len());
    for item in items {
        match item {
            ConstInit::Int(bits) => bytes.push(*bits as u8),
            ConstInit::Zero => bytes.push(0),
            _ => return None,
        }
    }
    Some(bytes)
}

fn zero_const_spelling(ty: &CType) -> String {
    if let Some(int) = ty.as_integer() {
        int.format_bits(0)
    } else if ty.is_pointer() {
        "null".to_string()
    } else {
        "zeroinitializer".to_string()
    }
}

fn render_ptr(ptr: &PtrConst) -> String {
    match ptr {
        PtrConst::Null => "null".to_string(),
        PtrConst::IntToPtr(bits) => format!("inttoptr (i64 {} to ptr)", i64_literal(*bits)),
        PtrConst::Addr { name, offset } if *offset == 0 => llvm_global(name),
        PtrConst::Addr { name, offset } => {
            format!(
                "getelementptr (i8, ptr {}, i64 {offset})",
                llvm_global(name)
            )
        }
    }
}

fn render_ptr_operand(ptr: &PtrConst) -> String {
    match ptr {
        PtrConst::Addr { name, offset } if *offset == 0 => format!("ptr {}", llvm_global(name)),
        other => format!("ptr {}", render_ptr(other)),
    }
}

fn i64_literal(bits: u64) -> String {
    let signed = bits as i64;
    if signed < 0 {
        signed.to_string()
    } else {
        bits.to_string()
    }
}

fn cmp_pred(op: BinOp, signed: bool) -> &'static str {
    match op {
        BinOp::Eq => "eq",
        BinOp::Ne => "ne",
        BinOp::Lt => {
            if signed {
                "slt"
            } else {
                "ult"
            }
        }
        BinOp::Gt => {
            if signed {
                "sgt"
            } else {
                "ugt"
            }
        }
        BinOp::Le => {
            if signed {
                "sle"
            } else {
                "ule"
            }
        }
        BinOp::Ge => {
            if signed {
                "sge"
            } else {
                "uge"
            }
        }
        _ => unreachable!("comparison"),
    }
}

fn is_unquoted_llvm_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    let start = matches!(first, b'A'..=b'Z' | b'a'..=b'z' | b'$' | b'.' | b'_' | b'-');
    start
        && bytes.all(|byte| {
            matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'$' | b'.' | b'_' | b'-')
        })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_paths() -> (PathBuf, PathBuf) {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir();
        let pid = std::process::id();
        (
            dir.join(format!("cc-{pid}-{id}.ll")),
            dir.join(format!("cc-{pid}-{id}.bin")),
        )
    }

    fn run_binary(ir: &str) -> (i32, Vec<u8>, Vec<u8>) {
        let (ll, bin) = temp_paths();
        std::fs::write(&ll, ir).unwrap();
        let assembled = Command::new("clang")
            .args(["-w", ll.to_str().unwrap(), "-o", bin.to_str().unwrap()])
            .output()
            .expect("clang");
        assert!(
            assembled.status.success(),
            "clang rejected IR:\n{}\n{ir}",
            String::from_utf8_lossy(&assembled.stderr)
        );
        let ran = Command::new(&bin).output().expect("run compiled IR");
        let _ = std::fs::remove_file(&ll);
        let _ = std::fs::remove_file(&bin);
        (ran.status.code().unwrap_or(101), ran.stdout, ran.stderr)
    }

    fn run_reference(source: &str) -> (i32, Vec<u8>, Vec<u8>) {
        let cfile = temp_paths().0.with_extension("c");
        std::fs::write(&cfile, source).unwrap();
        let result = run_reference_file(&cfile);
        let _ = std::fs::remove_file(&cfile);
        result
    }

    fn run_reference_file(path: &std::path::Path) -> (i32, Vec<u8>, Vec<u8>) {
        let (_, bin) = temp_paths();
        let compiled = Command::new("clang")
            .args([
                "-w",
                "-std=c11",
                path.to_str().unwrap(),
                "-o",
                bin.to_str().unwrap(),
            ])
            .output()
            .expect("clang");
        assert!(
            compiled.status.success(),
            "oracle failed:\n{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let ran = Command::new(&bin).output().expect("run oracle");
        let _ = std::fs::remove_file(&bin);
        (ran.status.code().unwrap_or(101), ran.stdout, ran.stderr)
    }

    #[test]
    fn source_filename_escapes_and_assembles() {
        let name = "a\"b\\c\n\t\u{7f}.c";
        let ir = crate::compile_to_ir("int main(void) { return 7; }\n", name).unwrap();
        let expected = format!("source_filename = {}\n", super::llvm_string(name));
        assert!(ir.starts_with(&expected), "{ir}");
        assert!(expected.contains("\\22"));
        assert!(expected.contains("\\\\"));
        assert!(!expected.contains("\\\""));
        assert_eq!(run_binary(&ir).0, 7);
        assert_eq!(super::llvm_global("main"), "@main");
        assert_eq!(super::llvm_global("a\"b"), "@\"a\\22b\"");
        assert_eq!(super::llvm_global(""), "@\"\"");
    }

    #[test]
    fn globals_and_prototypes_lower() {
        let ir = crate::compile_to_ir(
            "\
static int hidden = 3;
int counter;
extern int missing;
int add(int, int);
static int id(int x) { return x + hidden; }
int main(void) { counter = id(4); return counter; }
",
            "g.c",
        )
        .unwrap();
        assert!(
            ir.contains("@hidden = internal global i32 3, align 4"),
            "{ir}"
        );
        assert!(ir.contains("@counter = global i32 0, align 4"), "{ir}");
        assert!(
            ir.contains("@missing = external global i32, align 4"),
            "{ir}"
        );
        assert!(ir.contains("declare i32 @add(i32, i32)"), "{ir}");
        assert!(ir.contains("define internal i32 @id(i32 %p0)"), "{ir}");
        assert!(ir.contains("define i32 @main()"), "{ir}");
        assert!(!ir.contains("define i32 @add"), "{ir}");
        assert!(!ir.contains("declare i32 @id"), "{ir}");
        assert_eq!(run_binary(&ir).0, 7);
    }

    #[test]
    fn operators_lower_and_sizeof_is_not_emitted() {
        let ir = crate::compile_to_ir(
            "\
int g = 1 << 4;
int h = sizeof(int);
int i = ~0;
int j = 0 ? 1 : 6;
int main(void) {
    int x = 1;
    int y = sizeof(x = 5);
    int z = x++;
    int w = 0 ? y : z;
    g += 1;
    return y + w + (x << 1) + (-8 >> 1) + (~x & 7) + g;
}
",
            "ops.c",
        )
        .unwrap();
        assert!(ir.contains("@g = global i32 16, align 4"), "{ir}");
        assert!(ir.contains("@h = global i32 4, align 4"), "{ir}");
        assert!(ir.contains("@i = global i32 -1, align 4"), "{ir}");
        assert!(ir.contains("@j = global i32 6, align 4"), "{ir}");
        assert!(ir.contains("shl i32"), "{ir}");
        assert!(ir.contains("ashr i32"), "{ir}");
        assert!(ir.contains("xor i32"), "{ir}");
        assert!(ir.contains("phi i32"), "{ir}");
        assert!(!ir.contains("store i32 5"), "{ir}");
        // y = 4, w = z = 1, x becomes 2, 2 << 1 = 4, -8 >> 1 = -4,
        // ~2 & 7 = 5, g becomes 17. 4 + 1 + 4 - 4 + 5 + 17 = 27.
        assert_eq!(run_binary(&ir).0, 27);
    }

    #[test]
    fn return_constant_ir_assembles() {
        let ir = crate::compile_to_ir("int main(void) { return 42; }\n", "t.c").unwrap();
        assert!(ir.contains("define i32 @main()"));
        assert!(ir.contains("ret i32 42"));
        assert!(ir.contains("target triple = \"x86_64-pc-linux-gnu\""));
        assert!(ir.contains(&format!("target datalayout = \"{}\"", super::DATA_LAYOUT)));
        assert_eq!(run_binary(&ir).0, 42);
    }

    #[test]
    fn integer_abi_and_unsigned_ops() {
        let ir = crate::compile_to_ir(
            "\
char idc(char c) { return c; }
unsigned char idu(unsigned char u) { return u; }
short ids(short s) { return s; }
unsigned short idus(unsigned short s) { return s; }
int widen(char c, unsigned char u, short s, unsigned short us) {
    return c + u + s + us;
}
unsigned divu(unsigned a, unsigned b) { return a / b; }
unsigned shru(unsigned a) { return a >> 1; }
char c = -1;
unsigned char b = 200;
long l = 1;
extern short e;
",
            "abi.c",
        )
        .unwrap();
        assert!(
            ir.contains("define signext i8 @idc(i8 signext %p0)"),
            "{ir}"
        );
        assert!(
            ir.contains("define zeroext i8 @idu(i8 zeroext %p0)"),
            "{ir}"
        );
        assert!(
            ir.contains("define signext i16 @ids(i16 signext %p0)"),
            "{ir}"
        );
        assert!(
            ir.contains("define zeroext i16 @idus(i16 zeroext %p0)"),
            "{ir}"
        );
        assert!(
            ir.contains(
                "define i32 @widen(i8 signext %p0, i8 zeroext %p1, i16 signext %p2, i16 zeroext %p3)"
            ),
            "{ir}"
        );
        assert!(ir.contains("udiv i32"), "{ir}");
        assert!(ir.contains("lshr i32"), "{ir}");
        assert!(ir.contains("sext i8"), "{ir}");
        assert!(ir.contains("@c = global i8 -1, align 1"), "{ir}");
        assert!(ir.contains("@b = global i8 200, align 1"), "{ir}");
        assert!(ir.contains("@l = global i64 1, align 8"), "{ir}");
        assert!(ir.contains("@e = external global i16, align 2"), "{ir}");
        assert!(!ir.contains("signext i32"), "{ir}");
        assert!(!ir.contains("signext i64"), "{ir}");
        assert!(!ir.contains("zeroext i32"), "{ir}");
    }

    #[test]
    fn mixed_link_matches_clang_calling_convention() {
        let callee = "\
char idc(char c) { return c; }
unsigned char idu(unsigned char u) { return u; }
short ids(short s) { return s; }
unsigned short idus(unsigned short s) { return s; }
int widen(char c, unsigned char u, short s, unsigned short us) {
    return c + u + s + us;
}
";
        let caller = "\
extern char idc(char c);
extern unsigned char idu(unsigned char u);
extern short ids(short s);
extern unsigned short idus(unsigned short s);
extern int widen(char c, unsigned char u, short s, unsigned short us);
int main(void) {
    if (idc(-5) != -5) return 1;
    if (idu(200) != 200) return 2;
    if (ids(-300) != -300) return 3;
    if (idus(40000) != 40000) return 4;
    if (widen(-1, 200, -2, 40000) != 40197) return 5;
    return 0;
}
";
        let caller_ir = crate::compile_to_ir(caller, "caller.c").unwrap();
        assert!(
            caller_ir.contains("declare signext i8 @idc(i8 signext)"),
            "{caller_ir}"
        );
        assert!(
            caller_ir.contains("declare zeroext i16 @idus(i16 zeroext)"),
            "{caller_ir}"
        );
        assert!(
            caller_ir.contains("call signext i8 @idc(i8 signext"),
            "{caller_ir}"
        );
        assert!(
            caller_ir.contains("call zeroext i8 @idu(i8 zeroext"),
            "{caller_ir}"
        );

        let (ll, bin) = temp_paths();
        let obj = ll.with_extension("o");
        let cfile = ll.with_extension("clang.c");
        std::fs::write(&ll, &caller_ir).unwrap();
        std::fs::write(&cfile, callee).unwrap();
        let clang_obj = Command::new("clang")
            .args([
                "-w",
                "-std=c11",
                "-c",
                cfile.to_str().unwrap(),
                "-o",
                obj.to_str().unwrap(),
            ])
            .output()
            .expect("clang");
        assert!(
            clang_obj.status.success(),
            "{}",
            String::from_utf8_lossy(&clang_obj.stderr)
        );
        let linked = Command::new("clang")
            .args([
                "-w",
                ll.to_str().unwrap(),
                obj.to_str().unwrap(),
                "-o",
                bin.to_str().unwrap(),
            ])
            .output()
            .expect("clang link");
        assert!(
            linked.status.success(),
            "{}",
            String::from_utf8_lossy(&linked.stderr)
        );
        let ran = Command::new(&bin).output().expect("run");
        assert_eq!(ran.status.code(), Some(0));

        let callee_ir = crate::compile_to_ir(callee, "callee.c").unwrap();
        let our_ll = ll.with_extension("our.ll");
        let clang_main = ll.with_extension("main.c");
        let clang_obj = ll.with_extension("main.o");
        std::fs::write(&our_ll, callee_ir).unwrap();
        std::fs::write(&clang_main, caller).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-w",
                "-std=c11",
                "-c",
                clang_main.to_str().unwrap(),
                "-o",
                clang_obj.to_str().unwrap(),
            ])
            .output()
            .expect("clang");
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let bin2 = ll.with_extension("bin2");
        let linked = Command::new("clang")
            .args([
                "-w",
                our_ll.to_str().unwrap(),
                clang_obj.to_str().unwrap(),
                "-o",
                bin2.to_str().unwrap(),
            ])
            .output()
            .expect("clang link");
        assert!(
            linked.status.success(),
            "{}",
            String::from_utf8_lossy(&linked.stderr)
        );
        let ran = Command::new(&bin2).output().expect("run");
        assert_eq!(ran.status.code(), Some(0));
        let reference = run_reference(&format!("{callee}\n{caller}"));
        assert_eq!(reference.0, 0);

        let _ = std::fs::remove_file(&ll);
        let _ = std::fs::remove_file(&obj);
        let _ = std::fs::remove_file(&cfile);
        let _ = std::fs::remove_file(&bin);
        let _ = std::fs::remove_file(&our_ll);
        let _ = std::fs::remove_file(&clang_main);
        let _ = std::fs::remove_file(&clang_obj);
        let _ = std::fs::remove_file(&bin2);
    }

    #[test]
    fn pointer_and_array_ir() {
        let ir = crate::compile_to_ir(
            "\
int g;
int a[3] = {1, 2};
int *p = &g;
int *q = &a[2];
int *z = 0;
int main(void) {
    int x = 1;
    int *lp = &x;
    *lp = *p + a[1];
    lp = lp + 1;
    return *q + (z == 0);
}
",
            "ptr.c",
        )
        .unwrap();
        assert!(ir.contains("@p = global ptr @g, align 8"), "{ir}");
        assert!(
            ir.contains("@q = global ptr getelementptr (i8, ptr @a, i64 8), align 8"),
            "{ir}"
        );
        assert!(ir.contains("@z = global ptr null, align 8"), "{ir}");
        assert!(
            ir.contains("@a = global [3 x i32] [i32 1, i32 2, i32 0], align 4"),
            "{ir}"
        );
        assert!(ir.contains("getelementptr inbounds i32, ptr"), "{ir}");
        assert!(!ir.contains("inttoptr"), "{ir}");
        let (code, _, _) = run_binary(&ir);
        assert_eq!(
            code,
            run_reference(
                "\
int g;
int a[3] = {1, 2};
int *p = &g;
int *q = &a[2];
int *z = 0;
int main(void) {
    int x = 1;
    int *lp = &x;
    *lp = *p + a[1];
    lp = lp + 1;
    return *q + (z == 0);
}
"
            )
            .0
        );
    }

    #[test]
    fn strings_and_varargs_lower() {
        let hello = "\
int printf(const char *, ...);
int main(void) {
    printf(\"hello, world\\n\");
    return 0;
}
";
        let ir = crate::compile_to_ir(hello, "hello.c").unwrap();
        assert!(
            ir.contains(
                "@.str = private unnamed_addr constant [14 x i8] c\"hello, world\\0A\\00\", align 1"
            ),
            "{ir}"
        );
        assert!(ir.contains("declare i32 @printf(ptr, ...)"), "{ir}");
        assert!(
            ir.contains("call i32 (ptr, ...) @printf(ptr @.str)"),
            "{ir}"
        );
        let (code, out, err) = run_binary(&ir);
        assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
        assert_eq!(out, b"hello, world\n");

        let ir = crate::compile_to_ir(
            "\
int idc(char c, ...);
int main(void) {
    char c = 2;
    short s = 3;
    return idc(c, c, s);
}
char buf[4] = \"ab\";
const int g = 7;
char exact[2] = \"hi\";
",
            "va.c",
        )
        .unwrap();
        assert!(ir.contains("declare i32 @idc(i8 signext, ...)"), "{ir}");
        assert!(ir.contains("call i32 (i8, ...) @idc(i8 signext"), "{ir}");
        assert!(ir.contains(", i32 "), "{ir}");
        assert!(
            ir.contains("@buf = global [4 x i8] c\"ab\\00\\00\", align 1"),
            "{ir}"
        );
        assert!(ir.contains("@g = constant i32 7, align 4"), "{ir}");
        assert!(
            ir.contains("@exact = global [2 x i8] c\"hi\", align 1"),
            "{ir}"
        );
        assert!(!ir.contains("c\"hi\\00\""), "{ir}");
        let sized =
            crate::compile_to_ir("int main(void) { return sizeof \"abc\"; }\n", "sz.c").unwrap();
        assert!(!sized.contains("@.str"), "{sized}");
    }

    #[test]
    fn struct_ir_names_the_type_and_geps_members() {
        let source = "\
struct S { char c; int i; };
struct S g = {1, 2};
int main(void) {
    struct S s;
    s.c = 3;
    s.i = g.i;
    struct S t;
    t = s;
    return t.i + t.c;
}
";
        let ir = crate::compile_to_ir(source, "struct.c").unwrap();
        assert!(ir.contains("%struct.S = type { i8, i32 }"), "{ir}");
        assert!(
            ir.contains("@g = global %struct.S { i8 1, i32 2 }, align 4"),
            "{ir}"
        );
        assert!(ir.contains("getelementptr inbounds %struct.S, ptr"), "{ir}");
        assert!(ir.contains("call void @llvm.memmove.p0.p0.i64("), "{ir}");
        assert!(
            ir.contains("declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)"),
            "{ir}"
        );
        assert_eq!(run_binary(&ir), run_reference(source));
    }

    #[test]
    fn smoke_files_match_clang() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
        let mut files: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("c"))
            .collect();
        files.sort();
        assert!(!files.is_empty());
        for path in files {
            let source = std::fs::read_to_string(&path).unwrap();
            let name = path.to_string_lossy().to_string();
            let ir = crate::compile_to_ir(&source, &name).unwrap_or_else(|err| {
                panic!("{name}: {err}");
            });
            let mine = run_binary(&ir);
            let reference = run_reference_file(&path);
            assert_eq!(mine, reference, "{name}");
        }
    }

    #[test]
    fn union_enum_and_function_pointer_ir() {
        let source = "\
enum E { A = 2, B };
union U { int x; unsigned char b; };
int id(int n) { return n; }
int main(void) {
    union U u = {A};
    int (*p)(int) = id;
    return u.b + p(B);
}
";
        let ir = crate::compile_to_ir(source, "union.c").unwrap();
        assert!(ir.contains("%union.U = type"), "{ir}");
        assert!(ir.contains("getelementptr inbounds i8"), "{ir}");
        assert!(ir.contains("call i32 %"), "{ir}");
        assert_eq!(run_binary(&ir), run_reference(source));
    }
}
