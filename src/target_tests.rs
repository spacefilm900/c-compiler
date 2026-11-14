//! Whole-pipeline checks for the AArch64 targets: the IR each one gets for
//! a few small programs. The expected lowering was read from Clang as a
//! black box (`clang -target ... -S -emit-llvm`); harness/run_arm64_ir.py
//! and harness/run_cross.py check much more of it against Clang directly.

use crate::pp::PpOptions;
use crate::{compile_dialect, Dialect, Target};

const APPLE: Target = Target::Arm64Apple {
    macos_major: 11,
    macos_minor: 0,
};

fn ir(target: Target, source: &str) -> String {
    let options = PpOptions {
        target,
        system_dirs: Vec::new(),
        ..PpOptions::default()
    };
    compile_dialect(source, "t.c", &options, Dialect::Gnu11)
        .unwrap_or_else(|err| panic!("{target:?}: {source}: {err}"))
        .ir
}

fn err(target: Target, source: &str) -> String {
    let options = PpOptions {
        target,
        system_dirs: Vec::new(),
        ..PpOptions::default()
    };
    match compile_dialect(source, "t.c", &options, Dialect::Gnu11) {
        Ok(_) => panic!("{target:?} accepted: {source}"),
        Err(err) => err.to_string(),
    }
}

/// The value of `int v[] = {...};` as written in the IR.
fn values(target: Target, exprs: &str) -> String {
    let module = ir(target, &format!("int v[] = {{ {exprs} }};\n"));
    let line = module
        .lines()
        .find(|line| line.starts_with("@v = "))
        .expect("@v");
    let start = line.find("] [").expect("array") + 3;
    let end = line.rfind(']').expect("array end");
    line[start..end].replace("i32 ", "")
}

#[test]
fn header_names_the_target() {
    let x86 = ir(Target::X86_64Linux, "int x;");
    assert!(
        x86.contains("target triple = \"x86_64-pc-linux-gnu\""),
        "{x86}"
    );
    let apple = ir(APPLE, "int x;");
    assert!(
        apple.contains("target triple = \"arm64-apple-macosx11.0.0\""),
        "{apple}"
    );
    assert!(apple.contains("target datalayout = \"e-m:o-i64:64-i128:128-n32:64-S128\""));
    let linux = ir(Target::Aarch64Linux, "int x;");
    assert!(
        linux.contains("target triple = \"aarch64-unknown-linux-gnu\""),
        "{linux}"
    );
    assert!(linux.contains("e-m:e-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128"));
}

#[test]
fn type_model_per_target() {
    let probe = "sizeof(long double), _Alignof(long double), sizeof(__builtin_va_list), \
                 (char)-1 < 0, sizeof(L'a'), L'\\xffffffff' < 0, '\\377'";
    assert_eq!(
        values(Target::X86_64Linux, probe),
        "16, 16, 24, 1, 4, 1, -1"
    );
    assert_eq!(values(APPLE, probe), "8, 8, 8, 1, 4, 1, -1");
    assert_eq!(
        values(Target::Aarch64Linux, probe),
        "16, 16, 32, 0, 4, 0, 255"
    );
}

#[test]
fn predefined_macros_per_target() {
    let probe = "#if defined __APPLE__ && defined __MACH__ && defined __aarch64__ && \
                 defined __arm64__ && !defined __linux__ && !defined __ELF__ && \
                 __ENVIRONMENT_OS_VERSION_MIN_REQUIRED__ == 110000 && __LDBL_MANT_DIG__ == 53\n\
                 int apple;\n#endif\n\
                 #if defined __linux__ && defined __aarch64__ && defined __CHAR_UNSIGNED__ && \
                 __LDBL_MANT_DIG__ == 113 && !defined __x86_64__\nint arm_linux;\n#endif\n\
                 #if defined __x86_64__ && __LDBL_MANT_DIG__ == 64 && !defined __aarch64__\n\
                 int x86;\n#endif\n\
                 #define STR2(x) #x\n#define STR(x) STR2(x)\n\
                 const char prefix[] = STR(__USER_LABEL_PREFIX__);\n";
    let apple = ir(APPLE, probe);
    assert!(apple.contains("@apple") && !apple.contains("@arm_linux") && !apple.contains("@x86"));
    assert!(apple.contains("c\"_\\00\""), "{apple}");
    let linux = ir(Target::Aarch64Linux, probe);
    assert!(linux.contains("@arm_linux") && !linux.contains("@apple") && !linux.contains("@x86"));
    let x86 = ir(Target::X86_64Linux, probe);
    assert!(x86.contains("@x86") && !x86.contains("@apple") && !x86.contains("@arm_linux"));
}

const AGGREGATES: &str = "struct F4 { float a, b, c, d; };\n\
    struct C3 { char a, b, c; };\n\
    struct L2 { long a, b; };\n\
    struct L3 { long a, b, c; };\n\
    struct A { long a, b; } __attribute__((aligned(16)));\n\
    struct F4 f4(struct F4 x) { return x; }\n\
    struct C3 c3(struct C3 x) { return x; }\n\
    struct L2 l2(struct L2 x) { return x; }\n\
    struct L3 l3(struct L3 x) { return x; }\n\
    long al(struct A x) { return x.a; }\n\
    int narrow(char c, unsigned short s, _Bool b) { return c + s + b; }\n";

#[test]
fn apple_lowers_aggregates_like_aapcs64() {
    let module = ir(APPLE, AGGREGATES);
    for want in [
        "define [4 x float] @f4([4 x float] %",
        "define i24 @c3(i64 %",
        "define [2 x i64] @l2([2 x i64] %",
        "define void @l3(ptr sret(%struct.L3) align 8 %",
        "define i64 @al(i128 %",
        "define i32 @narrow(i8 signext %p0, i16 zeroext %p1, i8 zeroext %p2)",
    ] {
        assert!(module.contains(want), "missing {want}:\n{module}");
    }
    // A composite larger than 16 bytes goes as a pointer to a caller copy.
    assert!(module.contains("align 8 %agg.result, ptr %p0)"), "{module}");
    assert!(
        module.contains("\"frame-pointer\"=\"non-leaf\""),
        "{module}"
    );
}

#[test]
fn aarch64_linux_differs_from_apple_where_clang_does() {
    let module = ir(Target::Aarch64Linux, AGGREGATES);
    for want in [
        "define [4 x float] @f4([4 x float] alignstack(8) %",
        "define i24 @c3(i64 %",
        // The record's own aligned attribute does not pick a register pair.
        "define i64 @al([2 x i64] %",
        // AAPCS64 leaves narrow integers unextended.
        "define i32 @narrow(i8 %p0, i16 %p1, i8 %p2)",
    ] {
        assert!(module.contains(want), "missing {want}:\n{module}");
    }
    assert!(!module.contains("frame-pointer"), "{module}");
}

#[test]
fn x86_64_keeps_system_v() {
    let module = ir(Target::X86_64Linux, AGGREGATES);
    assert!(
        module.contains("@f4(<2 x float> %p0, <2 x float> %p0.1)"),
        "{module}"
    );
    assert!(module.contains("byval(%struct.L3)"), "{module}");
}

#[test]
fn calls_without_a_prototype_are_fixed_on_aarch64() {
    let source = "int f();\nint g(void) { return f(1, 2.0f); }\n";
    let apple = ir(APPLE, source);
    assert!(apple.contains("call i32 @f(i32 1, double "), "{apple}");
    let linux = ir(Target::Aarch64Linux, source);
    assert!(linux.contains("call i32 @f(i32 1, double "), "{linux}");
    let x86 = ir(Target::X86_64Linux, source);
    assert!(x86.contains("call i32 (...) @f(i32 1, double "), "{x86}");
}

#[test]
fn variadic_arguments_per_target() {
    let source = "#include <stdarg.h>\n\
        double sum(int n, ...) { va_list ap; va_start(ap, n); double s = 0;\n\
        while (n--) s += va_arg(ap, double); va_end(ap); return s; }\n\
        double use(void) { return sum(2, 1.0, 2.0); }\n";
    let options_dirs = |target| {
        let options = PpOptions {
            target,
            system_dirs: crate::pp::system_dirs_for(target, None),
            ..PpOptions::default()
        };
        compile_dialect(source, "t.c", &options, Dialect::C11)
            .unwrap_or_else(|err| panic!("{err}"))
            .ir
    };
    let apple = options_dirs(APPLE);
    // Apple: va_list is a char *; each argument is in an 8-byte stack slot.
    assert!(
        apple.contains("call double (i32, ...) @sum(i32 2, double "),
        "{apple}"
    );
    assert!(!apple.contains("__va_list"), "{apple}");
    let linux = options_dirs(Target::Aarch64Linux);
    // AAPCS64: the va_list structure with its register save area offsets.
    assert!(linux.contains("%struct.__va_list"), "{linux}");
}

#[test]
fn unnamed_bit_fields_align_records_only_on_aarch64_linux() {
    let probe = "sizeof(struct { char a; int : 0; }), _Alignof(struct { char a : 3; int : 4; })";
    assert_eq!(values(Target::X86_64Linux, probe), "4, 1");
    assert_eq!(values(APPLE, probe), "4, 1");
    assert_eq!(values(Target::Aarch64Linux, probe), "4, 4");
}

#[test]
fn mach_o_names() {
    let module = ir(
        APPLE,
        "int f(void) __asm__(\"_g\");\nint h(void) { return f(); }\n",
    );
    assert!(module.contains("call i32 @\"\\01_g\"()"), "{module}");
    let x86 = ir(
        Target::X86_64Linux,
        "int f(void) __asm__(\"g\");\nint h(void) { return f(); }\n",
    );
    assert!(x86.contains("call i32 @g()"), "{x86}");
    assert!(err(APPLE, "int x __attribute__((section(\".data.x\"))) = 1;").contains("mach-o"));
    ir(APPLE, "int x __attribute__((section(\"__DATA,__x\"))) = 1;");
}

#[test]
fn binary128_constants_in_byte_images() {
    // A record with a bit-field is written as bytes; the long double in it
    // must hold binary128 bits on AArch64 Linux, not the folded double's.
    let module = ir(
        Target::Aarch64Linux,
        "struct T { long double x; unsigned u : 1; } t = { 2.0, 1 };\n",
    );
    assert!(
        module.contains("i128 85070591730234615865843651857942052864"),
        "{module}"
    );
}

#[test]
fn review_fixes_on_arm64() {
    // Overlapping union alternatives are three float positions: an HFA.
    let union_hfa = "union U { float a[3]; float b[3]; };\nunion U f(union U u) { return u; }\n";
    for target in [APPLE, Target::Aarch64Linux] {
        let module = ir(target, union_hfa);
        assert!(module.contains("define [3 x float] @f("), "{module}");
    }
    // The address of a function with an Apple asm label is quoted.
    let module = ir(
        APPLE,
        "int f(void) __asm__(\"_f\");\nint (*g(void))(void) { return f; }\n",
    );
    assert!(module.contains("ret ptr @\"\\01_f\""), "{module}");
    // binary128 is normal down to 2^-16382.
    let module = ir(
        Target::Aarch64Linux,
        "int n(long double x) { return __builtin_isnormal(x); }\n",
    );
    assert!(
        module.contains("0xL00000000000000000001000000000000"),
        "{module}"
    );
    // A struct member of a packed record is loaded at its real alignment.
    let module = ir(
        APPLE,
        "struct S { long x; };\nstruct __attribute__((packed)) P { char c; struct S s; };\n\
         long h(struct S s);\nlong g(struct P *p) { return h(p->s); }\n",
    );
    let loads: Vec<&str> = module.lines().filter(|l| l.contains("load i64")).collect();
    assert!(
        !loads.is_empty() && loads.iter().all(|l| l.ends_with("align 1")),
        "{module}"
    );
}

#[test]
fn nested_union_hfa_is_linear() {
    // Each level is a union of two copies of the level below: 2^40 paths
    // to the same two float positions. The check must stay fast and still
    // find the HFA.
    let mut source = String::from("union U0 { float a[2]; };\n");
    for level in 1..=40 {
        let below = level - 1;
        source.push_str(&format!(
            "union U{level} {{ union U{below} x; union U{below} y; }};\n"
        ));
    }
    source.push_str("union U40 f(union U40 u) { return u; }\n");
    let start = std::time::Instant::now();
    for target in [APPLE, Target::Aarch64Linux] {
        let module = ir(target, &source);
        assert!(module.contains("define [2 x float] @f("), "{module}");
    }
    assert!(start.elapsed() < std::time::Duration::from_secs(10));
    // A union mixing float and double positions is still not an HFA.
    let module = ir(
        APPLE,
        "union M { float a; double b; };\nunion M m(union M u) { return u; }\n",
    );
    assert!(!module.contains("define [1 x"), "{module}");
}
