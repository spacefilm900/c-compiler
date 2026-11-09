//! Textual LLVM IR for the C subset.
//!
//! `int` is a 32-bit two's complement value, matching the System V x86-64 ABI.
//! Locals are `alloca`s in the entry block so a declaration inside a loop does
//! not allocate on every iteration. File-scope objects are LLVM globals.
//! Internal linkage is `internal`; a declaration with no definition in this
//! translation unit is `external global` or `declare`. `&&` and `||` are
//! short-circuiting and produce 0 or 1. Signed division and remainder use
//! `sdiv` and `srem`, which round toward zero as C requires.

use crate::ast::*;

pub fn render(unit: &TranslationUnit, filename: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("source_filename = {}\n", llvm_string(filename)));
    out.push_str("target triple = \"x86_64-pc-linux-gnu\"\n\n");
    let mut wrote_prelude = false;
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
    if wrote_prelude {
        out.push('\n');
    }
    for decl in &unit.decls {
        if let TopDecl::Func(func) = decl {
            if func.body.is_some() {
                out.push_str(&render_function(func));
                out.push_str("\n\n");
            }
        }
    }
    out
}

fn render_global(global: &IrGlobal) -> String {
    let name = llvm_global(&global.name);
    if let Some(value) = global.init {
        let linkage = match global.linkage {
            Linkage::Internal => "internal ",
            Linkage::External => "",
        };
        format!("{name} = {linkage}global i32 {value}, align 4")
    } else {
        format!("{name} = external global i32, align 4")
    }
}

fn render_declare(decl: &IrFunc) -> String {
    let ret = ir_type(decl.ret);
    let mut params = String::new();
    for index in 0..decl.arity {
        if index > 0 {
            params.push_str(", ");
        }
        params.push_str("i32");
    }
    format!("declare {ret} {}({params})", llvm_global(&decl.name))
}

fn ir_type(ty: TypeName) -> &'static str {
    match ty {
        TypeName::Int => "i32",
        TypeName::Void => "void",
    }
}

fn render_function(func: &FuncDef) -> String {
    let body = func.body.as_ref().expect("defined function");
    let mut gen = Gen::default();
    let ret = ir_type(func.ret_type);
    let linkage = match func.linkage {
        Linkage::Internal => "internal ",
        Linkage::External => "",
    };
    let params = func
        .params
        .iter()
        .map(|param| format!("i32 {}", param.arg_reg))
        .collect::<Vec<_>>()
        .join(", ");
    gen.emit(&format!(
        "define {linkage}{ret} {}({params}) {{",
        llvm_global(&func.name)
    ));
    gen.emit("entry:");
    gen.cur = "entry".to_string();
    gen.terminated = false;

    for param in &func.params {
        gen.emit(&format!("  {} = alloca i32, align 4", param.slot));
    }
    for slot in local_slots(body) {
        gen.emit(&format!("  {slot} = alloca i32, align 4"));
    }
    for param in &func.params {
        gen.emit(&format!(
            "  store i32 {}, ptr {}, align 4",
            param.arg_reg, param.slot
        ));
    }

    gen.gen_compound(body);
    if !gen.terminated {
        match func.ret_type {
            TypeName::Int => gen.emit("  ret i32 0"),
            TypeName::Void => gen.emit("  ret void"),
        }
    }
    gen.emit("}");
    gen.lines.join("\n")
}

fn local_slots(compound: &Compound) -> Vec<String> {
    let mut slots = Vec::new();
    for item in &compound.items {
        collect_slots(item, &mut slots);
    }
    slots
}

fn collect_slots(stmt: &Stmt, slots: &mut Vec<String>) {
    match stmt {
        Stmt::Decl(decl) => slots.push(decl.slot.clone()),
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
                    slots.push(decl.slot.clone());
                }
            }
            collect_slots(body, slots);
        }
        Stmt::Return { .. } | Stmt::Expr { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => {}
    }
}

#[derive(Default)]
struct Gen {
    lines: Vec<String>,
    tmp: u32,
    lab: u32,
    cur: String,
    terminated: bool,
    loops: Vec<(String, String)>,
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
                    self.emit(&format!("  ret i32 {value}"));
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
        if let Some(init) = &decl.init {
            let value = self.gen_val(init);
            self.emit(&format!("  store i32 {value}, ptr {}, align 4", decl.slot));
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
            Expr::Int { value, .. } => Some(value.to_string()),
            Expr::Var { resolved, .. } => {
                let slot = &resolved.as_ref().expect("resolved variable").slot;
                let tmp = self.fresh();
                self.emit(&format!("  {tmp} = load i32, ptr {slot}, align 4"));
                Some(tmp)
            }
            Expr::Unary {
                op: UnaryOp::Not, ..
            } => Some(self.zext_cond(expr)),
            Expr::Unary {
                op: UnaryOp::Plus,
                expr,
                ..
            } => self.gen_expr(expr),
            Expr::Unary {
                op: UnaryOp::Minus,
                expr,
                ..
            } => {
                let value = self.gen_val(expr);
                let tmp = self.fresh();
                self.emit(&format!("  {tmp} = sub i32 0, {value}"));
                Some(tmp)
            }
            Expr::Bin { op, .. } if op.is_cmp() || matches!(op, BinOp::And | BinOp::Or) => {
                Some(self.zext_cond(expr))
            }
            Expr::Bin {
                op, left, right, ..
            } => {
                let left = self.gen_val(left);
                let right = self.gen_val(right);
                let tmp = self.fresh();
                let inst = match op {
                    BinOp::Add => "add",
                    BinOp::Sub => "sub",
                    BinOp::Mul => "mul",
                    BinOp::Div => "sdiv",
                    BinOp::Mod => "srem",
                    _ => unreachable!("logical operator"),
                };
                self.emit(&format!("  {tmp} = {inst} i32 {left}, {right}"));
                Some(tmp)
            }
            Expr::Assign { target, value, .. } => {
                let slot = target.var_slot().expect("assignable variable").to_string();
                let value = self.gen_val(value);
                self.emit(&format!("  store i32 {value}, ptr {slot}, align 4"));
                Some(value)
            }
            Expr::Call { args, resolved, .. } => {
                let resolved = resolved.as_ref().expect("resolved call");
                let values: Vec<String> = args.iter().map(|arg| self.gen_val(arg)).collect();
                let arglist = values
                    .iter()
                    .map(|value| format!("i32 {value}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let callee = llvm_global(&resolved.name);
                if resolved.ret == TypeName::Void {
                    self.emit(&format!("  call void {callee}({arglist})"));
                    None
                } else {
                    let tmp = self.fresh();
                    self.emit(&format!("  {tmp} = call i32 {callee}({arglist})"));
                    Some(tmp)
                }
            }
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
                let left = self.gen_val(left);
                let right = self.gen_val(right);
                let tmp = self.fresh();
                let pred = match op {
                    BinOp::Eq => "eq",
                    BinOp::Ne => "ne",
                    BinOp::Lt => "slt",
                    BinOp::Gt => "sgt",
                    BinOp::Le => "sle",
                    BinOp::Ge => "sge",
                    _ => unreachable!("comparison"),
                };
                self.emit(&format!("  {tmp} = icmp {pred} i32 {left}, {right}"));
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
                let tmp = self.fresh();
                self.emit(&format!("  {tmp} = icmp ne i32 {value}, 0"));
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
    let mut out = String::from("\"");
    for byte in text.bytes() {
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

/// `@name` when `name` matches LLVM's unquoted identifier pattern, otherwise
/// a quoted global. C identifiers already match the unquoted pattern.
fn llvm_global(name: &str) -> String {
    if is_unquoted_llvm_name(name) {
        format!("@{name}")
    } else {
        format!("@{}", llvm_string(name))
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
        let (ll, bin) = temp_paths();
        let cfile = ll.with_extension("c");
        std::fs::write(&cfile, source).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-w",
                "-std=c11",
                cfile.to_str().unwrap(),
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
        let _ = std::fs::remove_file(&cfile);
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
    fn return_constant_ir_assembles() {
        let ir = crate::compile_to_ir("int main(void) { return 42; }\n", "t.c").unwrap();
        assert!(ir.contains("define i32 @main()"));
        assert!(ir.contains("ret i32 42"));
        assert!(ir.contains("target triple = \"x86_64-pc-linux-gnu\""));
        assert_eq!(run_binary(&ir).0, 42);
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
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let ir = crate::compile_to_ir(&source, &name).unwrap_or_else(|err| {
                panic!("{name}: {err}");
            });
            let mine = run_binary(&ir);
            let reference = run_reference(&source);
            assert_eq!(mine, reference, "{name}");
        }
    }
}
