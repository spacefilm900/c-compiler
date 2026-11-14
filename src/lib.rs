//! Clean-room C subset compiler.
//!
//! The frontend is original Rust written from the C standard, the LLVM
//! Language Reference, and the System V ABI. It is not derived from Clang
//! or GCC.

pub mod abi;
pub mod ast;
pub mod builtins;
pub mod diag;
pub mod dialect;
pub mod driver;
pub mod float;
mod fp80;
pub mod irgen;
pub mod lexer;
pub mod parser;
pub mod pp;
pub mod sema;
pub mod token;

pub use diag::{Diagnostic, Warning};
pub use dialect::Dialect;
pub use driver::run;
pub use pp::PpOptions;

/// Lex, parse, check, and render `source` as a textual LLVM IR module.
///
/// Preprocessing uses the default system include directory and no `-I` / `-D`
/// / `-U` options. [`compile_with`] takes those options.
pub fn compile_to_ir(source: &str, filename: &str) -> Result<String, Diagnostic> {
    compile_with(source, filename, &PpOptions::default())
}

/// Preprocess `source`, then parse, check, and render LLVM IR, in strict
/// C11.
pub fn compile_with(
    source: &str,
    filename: &str,
    options: &PpOptions,
) -> Result<String, Diagnostic> {
    compile_dialect(source, filename, options, Dialect::C11).map(|output| output.ir)
}

/// The result of a successful compilation.
pub struct Output {
    /// The textual LLVM IR module.
    pub ir: String,
    /// Warnings: those from parsing (such as an unknown attribute), then
    /// those from semantic analysis, each in source order.
    pub warnings: Vec<Warning>,
}

/// Like [`compile_with`], in the given dialect.
pub fn compile_dialect(
    source: &str,
    filename: &str,
    options: &PpOptions,
    dialect: Dialect,
) -> Result<Output, Diagnostic> {
    let tokens = pp::preprocess_tokens(source, filename, options)?;
    let mut unit = parser::parse_dialect(&tokens, filename, dialect)?;
    let mut warnings = std::mem::take(&mut unit.warnings);
    warnings.extend(sema::analyze_dialect(
        &mut unit, filename, &tokens, dialect,
    )?);
    Ok(Output {
        ir: irgen::render(&unit, filename),
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compile(source: &str, dialect: Dialect) -> Result<Output, Diagnostic> {
        compile_dialect(source, "t.c", &PpOptions::default(), dialect)
    }

    fn strict_err(source: &str) -> String {
        match compile(source, Dialect::C11) {
            Ok(_) => panic!("strict C11 accepted: {source}"),
            Err(err) => err.to_string(),
        }
    }

    fn gnu_ok(source: &str) -> Output {
        compile(source, Dialect::Gnu11).unwrap_or_else(|err| panic!("{source}: {err}"))
    }

    #[test]
    fn gnu_only_extensions_are_rejected_in_strict_mode() {
        for source in [
            "typeof(1) x;",
            "int main(void) { return ({ 1; }); }",
            "int f(int x) { switch (x) { case 1 ... 3: return 1; } return 0; }",
            "struct S { int n; int a[0]; };",
            "int f(int x) { return x ?: 2; }",
            "int a[2] = {};",
            "void *f(void) { l: return &&l; }",
            "void f(void *p) { goto *p; }",
        ] {
            assert!(strict_err(source).contains("error"), "{source}");
            gnu_ok(source);
        }
        assert!(strict_err("__typeof__(1) x;").contains("-std=gnu11"));
        assert!(strict_err("int f(int x) { return x ?: 2; }").contains("-std=gnu11"));
        assert!(strict_err("int main(void) { return ({ 1; }); }").contains("-std=gnu11"));
        assert!(strict_err("void *f(void) { l: return &&l; }").contains("-std=gnu11"));
        assert!(strict_err("void f(void *p) { goto *p; }").contains("-std=gnu11"));
    }

    #[test]
    fn empty_file_scope_declaration_from_an_empty_macro() {
        // SQLite's shell writes SQLITE_EXTENSION_INIT1; after the macro
        // expands to nothing. The smoke test has no directives, so the
        // preprocessor path is checked here.
        let ir = compile(
            "#define SQLITE_EXTENSION_INIT1\nSQLITE_EXTENSION_INIT1;\nint x;\nint main(void) { return x; }\n",
            Dialect::C11,
        )
        .unwrap_or_else(|err| panic!("{err}"))
        .ir;
        assert!(ir.contains("@x"), "{ir}");
    }

    #[test]
    fn static_function_used_only_unevaluated_needs_no_definition() {
        // tests/141_unevaluated_static_use.c checks the accepted forms
        // against Clang; an evaluated use, including the selected
        // `_Generic` association, still needs the definition (C11 6.9p3).
        for source in [
            "static int g(void); int main(void) { return g(); }",
            "static int g(void); int main(void) { return _Generic(0, int: g(), default: 0); }",
            "static int g(void); int main(void) { return (int)sizeof(g()) + g(); }",
        ] {
            let err = compile(source, Dialect::C11)
                .err()
                .unwrap_or_else(|| panic!("{source}: accepted"))
                .to_string();
            assert!(err.contains("used but not defined"), "{source}: {err}");
        }
    }

    #[test]
    fn labels_as_values_and_computed_goto() {
        // A static table of label addresses inside the function, as Lua's
        // lvm.c builds it, and an indirect branch to every taken label.
        let ir = gnu_ok(
            "int f(int i) { static const void *const t[] = { &&a, &&b }; goto *t[i]; a: return 1; b: return 2; }",
        )
        .ir;
        assert!(ir.contains("blockaddress(@f, %C.a)"), "{ir}");
        assert!(ir.contains("blockaddress(@f, %C.b)"), "{ir}");
        assert!(ir.contains("indirectbr ptr"), "{ir}");
        assert!(ir.contains("[label %C.a, label %C.b]"), "{ir}");
        // A label address only inside `sizeof` is not evaluated, so it is
        // not a destination of the computed `goto`.
        let ir = gnu_ok("void f(void *p) { (void)sizeof(&&x); goto *p; x:; }").ir;
        assert!(ir.contains(", []"), "{ir}");
        // Unselected `_Generic` associations and the controlling expression
        // are not evaluated (C11 6.5.1.1p3); their statement expressions,
        // and the labels in them, are dropped, so a label address there is
        // no destination either. (Clang rejects this program because the
        // indirect goto could enter a statement expression; this compiler
        // accepts it, so it must at least emit valid IR.)
        let ir = gnu_ok(
            "int f(void *p) { int r = _Generic(0, int: 5, default: ({ hidden: (long)&&hidden; })); \
             r += _Generic(({ ctl: &&ctl; }), void *: 1, default: 2); goto *p; one: return r; }",
        )
        .ir;
        assert!(!ir.contains("%C.hidden") && !ir.contains("%C.ctl"), "{ir}");
        assert!(ir.contains("indirectbr ptr") && ir.contains(", []"), "{ir}");
        // A label inside an operand that is not evaluated is dropped with
        // it, so a later `goto` or `&&` naming it is an error (Clang 19
        // rejects these too), and the name still cannot be redefined.
        for source in [
            "int f(void) { (void)_Generic(0, int: 0, default: ({ h:; 1; })); goto h; }",
            "int f(void) { (void)_Generic(({ h:; 1; }), int: 0); goto h; }",
            "int f(void) { (void)sizeof(({ h:; 1; })); goto h; }",
            "int f(void) { (void)_Generic(({ h:; 1; }), int: 0); void *p = &&h; goto *p; }",
        ] {
            let err = compile(source, Dialect::Gnu11)
                .err()
                .unwrap_or_else(|| panic!("{source}: accepted"))
                .to_string();
            assert!(
                err.contains("cannot jump to label 'h'") && err.contains("not evaluated"),
                "{source}: {err}"
            );
        }
        let err = compile(
            "int f(void) { (void)_Generic(({ h:; 1; }), int: 0); return ({ h:; 2; }); }",
            Dialect::Gnu11,
        )
        .err()
        .expect("redefined label")
        .to_string();
        assert!(err.contains("redefinition of label 'h'"), "{err}");
        // A label address is a `void *` value.
        gnu_ok("int f(void) { void *p = &&x; char *q = p; goto *q; x: return q != 0; }");
        for (source, want) in [
            (
                "void f(void) { void *p = &&nowhere; }",
                "undeclared label 'nowhere'",
            ),
            ("void f(void) { goto *1; }", "pointer"),
            ("void *p = &&x;", "outside a function"),
            (
                "int f(void) { static char *t = (char *)&&x + 1; x: return 0; }",
                "constant",
            ),
        ] {
            let err = compile(source, Dialect::Gnu11)
                .err()
                .unwrap_or_else(|| panic!("accepted: {source}"))
                .to_string();
            assert!(err.contains(want), "{source}: {err}");
        }
    }

    #[test]
    fn reserved_gnu_spellings_work_in_both_modes() {
        for dialect in [Dialect::C11, Dialect::Gnu11] {
            for source in [
                "__extension__ int y; static __inline__ int f(void) { return 0; }",
                "int g(int) __asm__(\"g2\"); int h(void) { return __builtin_clz(1u); }",
                "int f(int a, int b) { int r; return __builtin_add_overflow(a, b, &r); }",
                "struct __attribute__((packed)) P { char c; int i; };",
                "_Static_assert(__builtin_types_compatible_p(int, const int), \"\");",
                "_Static_assert(__builtin_choose_expr(1, 2, 3.0) == 2, \"\");",
            ] {
                compile(source, dialect).unwrap_or_else(|err| panic!("{source}: {err}"));
            }
        }
    }

    #[test]
    fn attributes_change_layout_and_warn_when_unknown() {
        gnu_ok("struct __attribute__((packed)) P { char c; int i; }; _Static_assert(sizeof(struct P) == 5, \"\");");
        gnu_ok("struct A { char c; } __attribute__((aligned(8))); _Static_assert(_Alignof(struct A) == 8 && sizeof(struct A) == 8, \"\");");
        gnu_ok("struct M { char c; int i __attribute__((aligned(16))); }; _Static_assert(sizeof(struct M) == 32, \"\");");
        let out = gnu_ok("int x __attribute__((frobnicate));");
        assert!(out.warnings.iter().any(|w| w
            .to_string()
            .contains("unknown attribute 'frobnicate' ignored")));
        let out =
            gnu_ok("void old(void) __attribute__((deprecated(\"no\"))); void f(void) { old(); }");
        assert!(out
            .warnings
            .iter()
            .any(|w| w.to_string().contains("'old' is deprecated: no")));
        let ir = gnu_ok("__attribute__((noreturn, cold)) void die(void); __attribute__((weak, section(\".s\"))) int w = 1; __attribute__((used)) static int u;").ir;
        assert!(ir.contains("noreturn"), "{ir}");
        assert!(ir.contains("weak"), "{ir}");
        assert!(ir.contains("section \".s\""), "{ir}");
        assert!(ir.contains("@llvm.used"), "{ir}");
        assert!(compile("int v __attribute__((vector_size(16)));", Dialect::Gnu11).is_err());
    }

    #[test]
    fn case_ranges_are_checked() {
        let out = gnu_ok("int f(int x) { switch (x) { case 5 ... 1: return 1; } return 0; }");
        assert!(out
            .warnings
            .iter()
            .any(|w| w.to_string().contains("empty case range")));
        let err = compile(
            "int f(int x) { switch (x) { case 1 ... 5: case 3: return 1; } return 0; }",
            Dialect::Gnu11,
        )
        .err()
        .expect("overlap accepted")
        .to_string();
        assert!(err.contains("duplicate case value"), "{err}");
    }

    #[test]
    fn statement_expressions_need_a_function() {
        let err = compile("int x = ({ 1; });", Dialect::Gnu11)
            .err()
            .expect("accepted")
            .to_string();
        assert!(err.contains("statement expression"), "{err}");
    }
}
