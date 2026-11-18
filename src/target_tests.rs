//! Whole-pipeline checks for the non-default targets (AArch64 and Intel macOS): the IR each one gets for
//! a few small programs. The expected lowering was read from Clang as a
//! black box (`clang -target ... -S -emit-llvm`); harness/run_arm64_ir.py
//! and harness/run_cross.py check much more of it against Clang directly.

use crate::pp::PpOptions;
use crate::{compile_dialect, Dialect, Target};

const APPLE: Target = Target::Arm64Apple {
    macos_major: 11,
    macos_minor: 0,
};

/// Intel macOS (`x86_64-apple-macos`, minimum macOS 10.15).
const INTEL_MAC: Target = crate::target::APPLE_X86_64_DEFAULT;

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
                 __ENVIRONMENT_OS_VERSION_MIN_REQUIRED__ == 110000 && __LDBL_MANT_DIG__ == 53 && \
                 __LITTLE_ENDIAN__ == 1\n\
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

#[test]
fn macos_sdk_header_constructs() {
    // `defined` produced by a macro in `#if` (the SDK's `__is_modern_darwin`).
    let module = ir(
        APPLE,
        "#define MODERN(v) (__ENVIRONMENT_OS_VERSION_MIN_REQUIRED__ >= (v) || defined(DRIVERKIT))\n\
         #define COMPAT() defined(SWIFT_X) && (!defined(EPOCH) || (EPOCH < 1))\n\
         #if MODERN(100000) && !COMPAT()\nint modern;\n#endif\n",
    );
    assert!(module.contains("@modern"), "{module}");
    // `_Float16` in declarations only (the SDK's <math.h>).
    let module = ir(
        APPLE,
        "extern _Float16 __fabsf16(_Float16);\nint main(void) { return 0; }\n",
    );
    assert!(module.contains("define i32 @main"), "{module}");
    assert!(err(APPLE, "_Float16 h;\n").contains("incomplete type '_Float16'"));
    // The floating characteristics follow the target's long double.
    for (target, ldbl, dig) in [
        (APPLE, "53", "15"),
        (Target::Aarch64Linux, "113", "33"),
        (Target::X86_64Linux, "64", "18"),
    ] {
        let module = ir(
            target,
            "int same = __FLT_MIN__ == 0x1p-126F && __DBL_MIN__ == 0x1p-1022 \
               && __DBL_DENORM_MIN__ == 0x1p-1074 && __FLT_MANT_DIG__ == 24;\n\
             int mant = __LDBL_MANT_DIG__;\nint dig = __LDBL_DIG__;\n",
        );
        assert!(
            module.contains("@same = global i32 1"),
            "{target:?}: {module}"
        );
        assert!(
            module.contains(&format!("@mant = global i32 {ldbl}")),
            "{target:?}"
        );
        assert!(
            module.contains(&format!("@dig = global i32 {dig}")),
            "{target:?}"
        );
    }
}

#[test]
fn binary128_constants_are_folded_exactly() {
    // AArch64 Linux `long double` constants are folded in binary128, not in
    // `double` and widened (step 9). The expected bits are Clang's, read as
    // a black box, and IEEE 754's nearest values.
    let module = ir(
        Target::Aarch64Linux,
        "long double a[] = { 0.1L, 0.1L + 0.2L, 1.0L / 3.0L, 0x1p-16494L,\n\
         1.18973149535723176508575932662800702e+4932L, -0x1p99999L,\n\
         (long double)0.1, 2.5L * 4.0L - 0.5L };\n\
         int big = (long double)9007199254740993.0L > 9007199254740992.0L;\n\
         int fits = (int)1.9L;\n",
    );
    // The `fp128` operands of `@a`, in initializer order.
    let line = module
        .lines()
        .find(|line| line.starts_with("@a = "))
        .unwrap_or_else(|| panic!("no @a: {module}"));
    let got: Vec<&str> = line
        .split("fp128 ")
        .skip(1)
        .map(|rest| rest.split([',', ']']).next().unwrap_or("").trim())
        .collect();
    let expected = [
        "0xL999999999999999A3FFB999999999999",
        "0xL33333333333333343FFD333333333333",
        "0xL55555555555555553FFD555555555555",
        "0xL00000000000000010000000000000000",
        "0xLFFFFFFFFFFFFFFFF7FFEFFFFFFFFFFFF",
        "0xL0000000000000000FFFF000000000000",
        "0xLA0000000000000003FFB999999999999",
        "0xL00000000000000004002300000000000",
    ];
    assert_eq!(got, expected, "{module}");
    for name in ["@big", "@fits"] {
        let prefix = format!("{name} = global i32 ");
        let value = module
            .lines()
            .find_map(|line| line.strip_prefix(prefix.as_str()))
            .and_then(|rest| rest.split(',').next())
            .map(str::trim);
        assert_eq!(value, Some("1"), "{name}: {module}");
    }
}

#[test]
fn large_arrays_are_aligned_to_16_only_on_x86_64() {
    // System V AMD64 aligns an array object of 16 bytes or more to 16;
    // AAPCS64 and Apple arm64 align it as its element (as Clang does).
    let source = "int g[5];\nint f(void) { int a[5] = {0}; return a[1] + g[0]; }\n";
    let x86 = ir(Target::X86_64Linux, source);
    assert!(
        x86.contains("@g = global [5 x i32] zeroinitializer, align 16"),
        "{x86}"
    );
    assert!(x86.contains("alloca [5 x i32], align 16"), "{x86}");
    for target in [Target::Aarch64Linux, APPLE] {
        let module = ir(target, source);
        assert!(
            module.contains("[5 x i32] zeroinitializer, align 4"),
            "{module}"
        );
        assert!(module.contains("alloca [5 x i32], align 4"), "{module}");
    }
}

#[test]
fn intel_mac_header_and_type_model() {
    let module = ir(INTEL_MAC, "int x;");
    assert!(
        module.contains("target triple = \"x86_64-apple-macosx10.15.0\""),
        "{module}"
    );
    assert!(module.contains(
        "target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\""
    ));
    // The same C type model as x86_64 Linux (values read from Clang for
    // x86_64-apple-macosx10.15 as a black box).
    let probe = "sizeof(long double), _Alignof(long double), sizeof(__builtin_va_list), \
                 (char)-1 < 0, sizeof(L'a'), L'\\xffffffff' < 0, '\\377'";
    assert_eq!(values(INTEL_MAC, probe), "16, 16, 24, 1, 4, 1, -1");
    let probe = "sizeof(struct { char a; int : 0; }), _Alignof(struct { char a : 3; int : 4; }), \
                 _Generic(__builtin_bswap64(1), unsigned long long: 1, default: 0)";
    assert_eq!(values(INTEL_MAC, probe), "4, 1, 1");
}

#[test]
fn intel_mac_predefined_macros() {
    let probe = "#if defined __APPLE__ && defined __MACH__ && defined __x86_64__ && \
                 defined __amd64__ && !defined __aarch64__ && !defined __arm64__ && \
                 !defined __linux__ && !defined __ELF__ && __LDBL_MANT_DIG__ == 64 && \
                 defined __llvm__ && __LITTLE_ENDIAN__ == 1 && \
                 __ENVIRONMENT_MAC_OS_X_VERSION_MIN_REQUIRED__ == 101500 && \
                 __ENVIRONMENT_OS_VERSION_MIN_REQUIRED__ == 101500 && !defined __CHAR_UNSIGNED__\n\
                 int intel_mac;\n#endif\n\
                 #define STR2(x) #x\n#define STR(x) STR2(x)\n\
                 const char prefix[] = STR(__USER_LABEL_PREFIX__);\n\
                 const char wint[] = STR(__WINT_TYPE__);\n";
    let module = ir(INTEL_MAC, probe);
    assert!(module.contains("@intel_mac"), "{module}");
    assert!(module.contains("c\"_\\00\""), "{module}");
    assert!(module.contains("c\"int\\00\""), "{module}");
    // Before 10.10 the version is written in the old four-digit form.
    let old = Target::X86_64Apple {
        macos_major: 10,
        macos_minor: 9,
    };
    let module = ir(
        old,
        "#if __ENVIRONMENT_MAC_OS_X_VERSION_MIN_REQUIRED__ == 1090\nint four_digits;\n#endif\n",
    );
    assert!(module.contains("@four_digits"), "{module}");
    // __llvm__ is only claimed where an SDK header needs it (Intel macOS's
    // <libkern/i386/_OSByteOrder.h> picks __builtin_bswap over extended asm).
    for other in [APPLE, Target::X86_64Linux, Target::Aarch64Linux] {
        let module = ir(other, "#ifdef __llvm__\nint claims_llvm;\n#endif\nint x;\n");
        assert!(!module.contains("@claims_llvm"), "{other:?}: {module}");
    }
    let module = ir(
        Target::X86_64Apple {
            macos_major: 13,
            macos_minor: 4,
        },
        "#if __ENVIRONMENT_OS_VERSION_MIN_REQUIRED__ == 130400\nint six_digits;\n#endif\n",
    );
    assert!(module.contains("@six_digits"), "{module}");
    assert!(module.contains("x86_64-apple-macosx13.4.0"), "{module}");
}

#[test]
fn intel_mac_keeps_system_v_with_mach_o_names() {
    let module = ir(INTEL_MAC, AGGREGATES);
    for want in [
        "@f4(<2 x float> %p0, <2 x float> %p0.1)",
        "byval(%struct.L3)",
        "define i32 @narrow(i8 signext %p0, i16 zeroext %p1, i8 zeroext %p2)",
        "\"frame-pointer\"=\"all\"",
    ] {
        assert!(module.contains(want), "missing {want}:\n{module}");
    }
    // Calls without a prototype stay variadic-style, as on Linux.
    let module = ir(INTEL_MAC, "int f();\nint g(void) { return f(1, 2.0f); }\n");
    assert!(
        module.contains("call i32 (...) @f(i32 1, double "),
        "{module}"
    );
    // An asm label is the exact Mach-O symbol, so `$INODE64` variants of
    // the SDK's stat and readdir work.
    let module = ir(
        INTEL_MAC,
        "int f(void) __asm__(\"_stat$INODE64\");\nint h(void) { return f(); }\n",
    );
    assert!(
        module.contains("call i32 @\"\\01_stat$INODE64\"()"),
        "{module}"
    );
    assert!(err(
        INTEL_MAC,
        "int x __attribute__((section(\".data.x\"))) = 1;"
    )
    .contains("mach-o"));
    ir(
        INTEL_MAC,
        "int x __attribute__((section(\"__DATA,__x\"))) = 1;",
    );
    // System V's 16-byte alignment of large arrays applies here too.
    let module = ir(
        INTEL_MAC,
        "int g[5];\nint f(void) { int a[5] = {0}; return a[1] + g[0]; }\n",
    );
    assert!(
        module.contains("@g = global [5 x i32] zeroinitializer, align 16"),
        "{module}"
    );
    assert!(module.contains("alloca [5 x i32], align 16"), "{module}");
}

#[test]
fn intel_mac_passes_flexible_array_records_in_memory() {
    // Clang, the only C compiler on macOS, passes and returns a struct with
    // a flexible array member in memory there (observed as a black box).
    // On Linux this compiler keeps GCC's choice: registers.
    let source = "struct S { int n; int a[]; };\nstruct S id(struct S s) { return s; }\n";
    let mac = ir(INTEL_MAC, source);
    assert!(
        mac.contains("define void @id(ptr sret(%struct.S) align 4 %agg.result, ptr byval(%struct.S) align 8 %p0)"),
        "{mac}"
    );
    let linux = ir(Target::X86_64Linux, source);
    assert!(linux.contains("define i32 @id(i32 %p0)"), "{linux}");
}

#[test]
fn intel_mac_passes_unions_holding_flexible_array_records_in_memory() {
    // As for the struct itself (observed with Clang as a black box).
    let source = "struct V { int n; int a[]; };\nunion F { struct V v; long l; };\n\
                  long take(union F f) { return f.l; }\n";
    let mac = ir(INTEL_MAC, source);
    assert!(
        mac.contains("define i64 @take(ptr byval(%union.F) align 8 %p0)"),
        "{mac}"
    );
    let linux = ir(Target::X86_64Linux, source);
    assert!(linux.contains("define i64 @take(i64 %p0)"), "{linux}");
}

#[test]
fn intel_mac_passes_a_lone_x87up_eightbyte_as_sse() {
    // `union { long double d; void *p; }`: the second eightbyte is X87UP
    // without X87 before it. The psABI passes the union in memory; Intel
    // macOS (Clang, observed as a black box) passes and returns it in a
    // general register and an SSE register.
    let source = "union A { long double d; void *p; };\n\
                  union A id(union A a) { return a; }\n";
    let mac = ir(INTEL_MAC, source);
    assert!(
        mac.contains("define { i64, double } @id(i64 %p0, double %p0.1)"),
        "{mac}"
    );
    let linux = ir(Target::X86_64Linux, source);
    assert!(
        linux.contains("define void @id(ptr sret(%union.A) align 16 %agg.result, ptr byval(%union.A) align 16 %p0)"),
        "{linux}"
    );
}
