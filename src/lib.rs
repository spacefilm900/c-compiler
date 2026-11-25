//! Clean-room C subset compiler.
//!
//! The frontend is original Rust written from the C standard, the LLVM
//! Language Reference, and the System V ABI. It is not derived from Clang
//! or GCC.

pub mod aapcs64;
pub mod abi;
pub mod ast;
mod bigint;
pub mod builtins;
pub mod diag;
pub mod dialect;
pub mod driver;
pub mod float;
mod fp128;
mod fp80;
pub(crate) mod fxhash;
pub mod irgen;
pub mod lexer;
mod link;
pub mod parser;
pub mod pp;
pub mod sema;
pub mod target;
#[cfg(test)]
mod target_tests;
pub mod timing;
pub mod token;
pub mod ucn;
pub mod win64;

pub use diag::{Diagnostic, Warning};
pub use dialect::Dialect;
pub use driver::run;
pub use pp::PpOptions;
pub use target::Target;

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
    /// Wall time of each frontend phase (`write` is filled in by the driver).
    pub times: timing::PhaseTimes,
}

/// Like [`compile_with`], in the given dialect.
pub fn compile_dialect(
    source: &str,
    filename: &str,
    options: &PpOptions,
    dialect: Dialect,
) -> Result<Output, Diagnostic> {
    compile_impl(source, filename, options, dialect, false, None)
}

/// Like [`compile_dialect`] for a process that exits right afterwards: the
/// token list and the syntax tree are not freed, because the operating
/// system reclaims them faster than freeing millions of small allocations
/// one by one.
pub fn compile_and_exit(
    source: &str,
    filename: &str,
    options: &PpOptions,
    dialect: Dialect,
) -> Result<Output, Diagnostic> {
    compile_impl(source, filename, options, dialect, true, None)
}

/// A frontend stage whose result `--dump-*` prints instead of compiling on.
/// Preprocessed text (`-E`) is [`pp::preprocess`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// The phase-7 tokens the parser reads, one per line.
    Tokens,
    /// The syntax tree as parsed, before semantic analysis.
    Ast,
    /// The tree after semantic analysis: types, conversions, layouts and
    /// folded constants.
    Sema,
    /// The LLVM IR module, as a normal compilation writes it.
    Ir,
}

/// Run the frontend up to and including `stage` and return its dump in
/// [`Output::ir`]. Warnings found up to that stage are in
/// [`Output::warnings`]. A normal compilation is [`compile_dialect`]; this
/// takes the same path and only stops early.
pub fn dump_stage(
    source: &str,
    filename: &str,
    options: &PpOptions,
    dialect: Dialect,
    stage: Stage,
) -> Result<Output, Diagnostic> {
    compile_impl(source, filename, options, dialect, false, Some(stage))
}

fn compile_impl(
    source: &str,
    filename: &str,
    options: &PpOptions,
    dialect: Dialect,
    leak: bool,
    stop: Option<Stage>,
) -> Result<Output, Diagnostic> {
    use std::time::Instant;
    target::set(options.target);
    let mut times = timing::PhaseTimes::default();
    timing::take_lex();
    let start = Instant::now();
    let options = &PpOptions {
        gnu: dialect.gnu(),
        ..options.clone()
    };
    let tokens = pp::preprocess_tokens(source, filename, options)?;
    times.lex = timing::take_lex();
    times.preprocess = start.elapsed().saturating_sub(times.lex);
    if stop == Some(Stage::Tokens) {
        return Ok(Output {
            ir: token::dump_tokens(&tokens),
            warnings: Vec::new(),
            times,
        });
    }
    let start = Instant::now();
    let mut unit = parser::parse_with(&tokens, filename, dialect, options.gnu89_inline)?;
    times.parse = start.elapsed();
    if stop == Some(Stage::Ast) {
        return Ok(Output {
            ir: ast::dump_parsed(&unit),
            warnings: std::mem::take(&mut unit.warnings),
            times,
        });
    }
    let start = Instant::now();
    let mut warnings = std::mem::take(&mut unit.warnings);
    warnings.extend(sema::analyze_dialect(
        &mut unit, filename, &tokens, dialect,
    )?);
    times.sema = start.elapsed();
    if stop == Some(Stage::Sema) {
        return Ok(Output {
            ir: ast::dump_checked(&unit),
            warnings,
            times,
        });
    }
    let start = Instant::now();
    let ir = irgen::render(&unit, filename).map_err(|diag| in_source_file(diag, &tokens))?;
    times.irgen = start.elapsed();
    if leak {
        std::mem::forget(unit);
        std::mem::forget(tokens);
    }
    Ok(Output {
        ir,
        warnings,
        times,
    })
}

/// IR generation knows only the line and column. Like semantic analysis,
/// take the file name from the token at that position, so an internal
/// error inside an included header names the header.
fn in_source_file(mut diag: Diagnostic, tokens: &[token::Token]) -> Diagnostic {
    if let Some(token) = tokens
        .iter()
        .find(|token| (token.line, token.column) == (diag.line, diag.column))
    {
        diag.filename = token.filename.to_string();
    }
    diag
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

    /// Every stage dump comes from the same pipeline as a normal
    /// compilation, and `--dump-ir` is exactly the normal IR.
    #[test]
    fn stage_dumps_follow_the_pipeline() {
        let source = "struct P { char c; int n; } p;\n\
                      int f(char c) { long l = c + 1; return (int)l + p.n; }\n";
        let options = PpOptions::default();
        let dump = |stage| {
            dump_stage(source, "t.c", &options, Dialect::C11, stage)
                .expect("dumped")
                .ir
        };
        let tokens = dump(Stage::Tokens);
        assert!(
            tokens.starts_with("t.c:1:1 KwStruct 'struct'\n"),
            "{tokens}"
        );
        assert!(tokens.contains("t.c:2:30 Int '1' int 1\n"), "{tokens}");
        let ast = dump(Stage::Ast);
        assert!(ast.contains("func int f @2:5\n"), "{ast}");
        assert!(!ast.contains(" : "), "the parsed tree has no types: {ast}");
        let sema = dump(Stage::Sema);
        // The usual arithmetic conversions on `c + 1`, and the
        // initializer's conversion of the `int` sum to `long`.
        assert!(sema.contains("+ : int -> long @2:28"), "{sema}");
        assert!(sema.contains("var c [%v1] : char -> int @2:26"), "{sema}");
        assert!(sema.contains("struct P struct.P size 8 align 4"), "{sema}");
        assert!(sema.contains("field int n at 4"), "{sema}");
        let ir = dump(Stage::Ir);
        let normal = compile_dialect(source, "t.c", &options, Dialect::C11)
            .expect("compiled")
            .ir;
        assert_eq!(ir, normal);
        // The dumps are deterministic.
        assert_eq!(dump(Stage::Sema), sema);
    }

    /// Index designators show their expressions and `va_arg` its type.
    #[test]
    fn dumps_show_designator_indexes_and_va_arg_types() {
        let source = "int a[4] = { [1 + 1] = 5 };\n\
                      int f(int n, ...) { __builtin_va_list ap; __builtin_va_start(ap, n);\n\
                      long v = __builtin_va_arg(ap, long); return (int)v; }\n";
        let ast = dump_stage(
            source,
            "t.c",
            &PpOptions::default(),
            Dialect::C11,
            Stage::Ast,
        )
        .expect("dumped")
        .ir;
        assert!(
            ast.contains("designated [] @1:14\n      index\n        + @1:17\n"),
            "{ast}"
        );
        assert!(ast.contains("va_arg long @3:10\n"), "{ast}");
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
        // The reserved spelling is accepted in strict mode too.
        let unit = compile("__typeof__(1) x; __typeof(2.0) y;", Dialect::C11);
        assert!(unit.is_ok(), "{:?}", unit.err());
        assert!(strict_err("int f(int x) { return x ?: 2; }").contains("-std=gnu11"));
        assert!(strict_err("int main(void) { return ({ 1; }); }").contains("-std=gnu11"));
        assert!(strict_err("void *f(void) { l: return &&l; }").contains("-std=gnu11"));
        assert!(strict_err("void f(void *p) { goto *p; }").contains("-std=gnu11"));
    }

    #[test]
    fn switch_labels_do_not_jump_into_statement_expressions() {
        // GCC manual, "Statements and Declarations in Expressions": a switch
        // may not jump into a statement expression.
        for (source, label) in [
            ("int f(int x) { switch (x) { case 0: ({ case 1: x++; }); } return x; }", "case"),
            ("int f(int x) { switch (x) { case 0: ({ default: x++; }); } return x; }", "default"),
            ("int f(int x) { switch (x) { case 0: x = ({ while (x) { case 1: x--; } x; }); } return x; }", "case"),
        ] {
            let err = match compile(source, Dialect::Gnu11) {
                Ok(_) => panic!("accepted: {source}"),
                Err(err) => err,
            };
            assert!(!err.is_internal(), "{err}");
            assert_eq!(
                err.message,
                format!("'{label}' label jumps into a statement expression from its switch"),
                "{source}"
            );
        }
        let err = match compile(
            "int f(int x) { ({ case 1: x++; }); return x; }",
            Dialect::Gnu11,
        ) {
            Ok(_) => panic!("accepted a case with no switch"),
            Err(err) => err,
        };
        assert_eq!(err.message, "'case' label is not inside a switch");
        // A switch of its own inside the statement expression, and break or
        // continue out of one, stay valid.
        gnu_ok(
            "int f(int x) { switch (x) { case 0: x = ({ int t = 0; switch (x) { case 1: t = 2; break; default: t = 3; } t; }); } return x; }",
        );
        gnu_ok(
            "int f(int x) { switch (x) { case 1: ({ if (x) break; x = 2; }); x++; } return x; }",
        );
        gnu_ok("int f(int x) { while (x) { int y = ({ if (x > 2) continue; x; }); x -= y; } return x; }");
        let err = match compile("int f(int x) { ({ break; }); return x; }", Dialect::Gnu11) {
            Ok(_) => panic!("accepted break outside a loop"),
            Err(err) => err,
        };
        assert_eq!(err.message, "'break' is not inside a loop or switch");
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
    fn int128_va_arg_load_assumes_only_save_slot_alignment() {
        // The register save area's GP slots are 8-byte aligned, so a
        // 128-bit va_arg that may come from there must not claim 16.
        let ir = compile(
            "__int128 f(int n, ...) { __builtin_va_list ap; __builtin_va_start(ap, n); \
             __int128 v = __builtin_va_arg(ap, __int128); __builtin_va_end(ap); return v; }",
            Dialect::C11,
        )
        .unwrap_or_else(|err| panic!("{err}"))
        .ir;
        let phi = ir
            .lines()
            .position(|line| line.contains(" = phi ptr"))
            .expect("va_arg phi");
        let load = ir.lines().nth(phi + 1).expect("load after phi");
        assert!(
            load.contains("load i128") && load.ends_with("align 8"),
            "{load}\n{ir}"
        );
    }

    #[test]
    fn int128_constant_overflows_and_bitfields_are_rejected() {
        // Signed overflow in a constant is a constraint violation
        // (C11 6.6, 6.5.7). Clang 19 warns and wraps; this compiler rejects.
        for (source, want) in [
            (
                "static __int128 x = -(__int128)((__uint128_t)1 << 127);",
                "overflows",
            ),
            (
                "static __int128 x = ((__int128)1 << 126) << 2;",
                "overflows",
            ),
            (
                "int f(__int128 x) { switch (x) { case ((__int128)1 << 126) << 2: return 1; } return 0; }",
                "overflows",
            ),
        ] {
            let err = compile(source, Dialect::C11)
                .err()
                .unwrap_or_else(|| panic!("{source}: accepted"))
                .to_string();
            assert!(err.contains(want), "{source}: {err}");
        }
        // `parse_bit_width` is also covered in the parser tests; compile()
        // is the full path, including a 128-bit named field of width 80.
        let err = compile("struct S { unsigned __int128 f : 80; };", Dialect::C11)
            .err()
            .expect("128-bit bit-field accepted")
            .to_string();
        assert!(
            err.contains("bit-field of type 'unsigned __int128' is not supported"),
            "{err}"
        );
        // A representable 128-bit left shift still folds.
        compile(
            "static __int128 x = (__int128)1 << 126; int main(void) { return (int)(x != 0); }",
            Dialect::C11,
        )
        .unwrap_or_else(|err| panic!("{err}"));
        // Overflow IR for 128-bit operands must not use a same-width cast.
        let ir = compile(
            "int f(__uint128_t a, __uint128_t b, __uint128_t *r) { return __builtin_add_overflow(a, b, r); }",
            Dialect::C11,
        )
        .unwrap_or_else(|err| panic!("{err}"))
        .ir;
        assert!(
            !ir.contains("sext i128 to i128")
                && !ir.contains("zext i128 to i128")
                && !ir.contains("trunc i128 to i128")
                && ir.contains("i256"),
            "{ir}"
        );
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
        assert!(ir.contains("blockaddress(@f, %C.a.via_ptr)"), "{ir}");
        assert!(ir.contains("blockaddress(@f, %C.b.via_ptr)"), "{ir}");
        assert!(ir.contains("indirectbr ptr"), "{ir}");
        assert!(
            ir.contains("[label %C.a.via_ptr, label %C.b.via_ptr]"),
            "{ir}"
        );
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
    fn tentative_array_definition_gets_one_element() {
        // C11 6.9.2p5 EXAMPLE 2.
        let out = compile("int i[]; const int c[]; int *p = i;", Dialect::C11).unwrap();
        assert!(
            out.ir.contains("@i = global [1 x i32] zeroinitializer"),
            "{}",
            out.ir
        );
        assert!(
            out.ir.contains("@c = constant [1 x i32] zeroinitializer"),
            "{}",
            out.ir
        );
        let text: Vec<String> = out.warnings.iter().map(|w| w.to_string()).collect();
        assert_eq!(
            text.iter()
                .filter(|w| w.contains("tentative array definition assumed to have one element"))
                .count(),
            2,
            "{text:?}"
        );
        // A later definition still completes it, and internal linkage
        // needs a complete type (6.9.2p3).
        let out = compile("int i[]; int i[3] = {1};", Dialect::C11).unwrap();
        assert!(out.ir.contains("@i = global [3 x i32]"), "{}", out.ir);
        assert!(out.warnings.is_empty());
        assert!(compile("static int s[];", Dialect::C11).is_err());
        assert!(compile("extern int e[]; int *q = e;", Dialect::C11).is_ok());
    }

    #[test]
    fn multi_character_constants_warn() {
        let out = compile("int a = 'ab'; int b = 'abcde'; int c = 'a';", Dialect::C11).unwrap();
        let text: Vec<String> = out.warnings.iter().map(|w| w.to_string()).collect();
        assert_eq!(
            text.iter()
                .filter(|w| w.contains("multi-character character constant"))
                .count(),
            2,
            "{text:?}"
        );
        assert_eq!(
            text.iter()
                .filter(|w| w.contains("too long for its type"))
                .count(),
            1,
            "{text:?}"
        );
        assert!(out.ir.contains("@a = global i32 24930"), "{}", out.ir);
        assert!(out.ir.contains("@b = global i32 1650680933"), "{}", out.ir);
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

    /// C11 6.7.4p7 inline definitions, GCC's `gnu_inline`, and
    /// `-fgnu89-inline`: which definitions are external ones.
    #[test]
    fn inline_definitions_are_not_external() {
        let source = "inline int f(void) { return 1; }\n\
            extern inline int g(void) { return 2; }\n\
            int h(void);\n\
            inline int h(void) { return 3; }\n\
            static inline int s(void) { return 4; }\n\
            extern __inline __attribute__((__gnu_inline__)) __attribute__((__always_inline__))\n\
            int k(void) { return 5; }\n\
            __inline __attribute__((__gnu_inline__)) int m(void) { return 6; }\n\
            inline int n(void);\n\
            int n(void) { return 7; }\n\
            int main(void) { return f() + g() + h() + s() + k() + m() + n(); }\n";
        let linkage = |ir: &str, name: &str| -> String {
            let line = ir
                .lines()
                .find(|line| line.starts_with("define") && line.contains(&format!("@{name}(")))
                .unwrap_or_else(|| panic!("no definition of {name}: {ir}"));
            line.split_whitespace().nth(1).unwrap_or("").to_string()
        };
        let c11 = compile(source, Dialect::C11).expect("accepted").ir;
        for (name, want) in [
            ("f", "available_externally"),
            ("g", "i32"),
            ("h", "i32"),
            ("s", "internal"),
            ("k", "available_externally"),
            ("m", "i32"),
            ("n", "i32"),
        ] {
            assert_eq!(linkage(&c11, name), want, "{name}: {c11}");
        }
        assert!(c11.contains("@k() alwaysinline"), "{c11}");
        let options = PpOptions {
            gnu89_inline: true,
            ..PpOptions::default()
        };
        let gnu89 = compile_dialect(source, "t.c", &options, Dialect::C11)
            .expect("accepted")
            .ir;
        for (name, want) in [
            ("f", "i32"),
            ("g", "available_externally"),
            ("h", "i32"),
            ("s", "internal"),
            ("k", "available_externally"),
            ("m", "i32"),
        ] {
            assert_eq!(linkage(&gnu89, name), want, "{name}: {gnu89}");
        }
        let text = pp::preprocess(
            "#ifdef __GNUC_GNU_INLINE__\ngnu\n#endif\n#ifdef __GNUC_STDC_INLINE__\nstdc\n#endif\n",
            "t.c",
            &options,
        )
        .expect("preprocessed");
        assert!(text.contains("gnu") && !text.contains("stdc"), "{text}");
    }

    /// A definition takes the `__asm__` label of an earlier declaration
    /// (GCC manual, "Asm Labels"), and the inline rules see every
    /// declaration of the function whatever its label.
    #[test]
    fn definition_keeps_an_earlier_asm_label() {
        let ir = compile(
            "int f(void) __asm__(\"renamed\");\n\
             inline int f(void) { return 1; }\n\
             inline int g(void) __asm__(\"gee\");\n\
             inline int g(void) { return 2; }\n\
             int main(void) { return f() + g(); }\n",
            Dialect::C11,
        )
        .expect("accepted")
        .ir;
        assert!(ir.contains("define i32 @renamed()"), "{ir}");
        assert!(ir.contains("call i32 @renamed()"), "{ir}");
        assert!(
            ir.contains("define available_externally i32 @gee()"),
            "{ir}"
        );
        assert!(ir.contains("call i32 @gee()"), "{ir}");
        assert!(!ir.contains("@f(") && !ir.contains("@g("), "{ir}");
        // An inline-only declaration of another C name does not touch the
        // external definition that shares its linker name.
        let ir = compile(
            "inline int alias(void) __asm__(\"impl\");\n\
             int impl(void) { return 42; }\n\
             int main(void) { return impl() != 42; }\n",
            Dialect::C11,
        )
        .expect("accepted")
        .ir;
        assert!(ir.contains("define i32 @impl()"), "{ir}");
    }

    /// GCC manual, "Common Predefined Macros": `__STRICT_ANSI__` is defined
    /// for a strict `-std` only. hcc defined it under `-std=gnu11` as well,
    /// and glibc then hid `fileno` from zlib's minigzip.c. This goes through
    /// `compile_dialect`, which must hand the dialect to the preprocessor.
    #[test]
    fn strict_ansi_follows_the_dialect() {
        let probe = "#ifdef __STRICT_ANSI__\n#error strict\n#endif\nint x;\n";
        gnu_ok(probe);
        assert!(strict_err(probe).contains("strict"));
    }
}
