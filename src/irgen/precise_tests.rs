//! Tests that pin exact IR text or exact program output. The mutation
//! sample in `reports/quality/baseline-2026-11` found code whose output no
//! unit test checked; each test here names what it guards.

use crate::dialect::Dialect;
use crate::pp::PpOptions;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn ir_for(source: &str, options: &PpOptions) -> String {
    crate::compile_dialect(source, "t.c", options, Dialect::Gnu11)
        .unwrap_or_else(|err| panic!("{err}\n{source}"))
        .ir
}

fn ir(source: &str) -> String {
    ir_for(source, &PpOptions::default())
}

/// The lines of `ir` that start with `prefix`.
fn lines_with<'a>(ir: &'a str, prefix: &str) -> Vec<&'a str> {
    ir.lines().filter(|line| line.starts_with(prefix)).collect()
}

/// Compile in GNU C, assemble with Clang, run, and return standard output.
fn run(source: &str) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir();
    let stem = format!("cc-precise-{}-{id}", std::process::id());
    let ll = dir.join(format!("{stem}.ll"));
    let bin = dir.join(format!("{stem}.bin"));
    std::fs::write(&ll, ir(source)).unwrap();
    let built = Command::new("clang")
        .args(["-w", ll.to_str().unwrap(), "-o", bin.to_str().unwrap()])
        .output()
        .expect("clang");
    assert!(
        built.status.success(),
        "clang rejected IR:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let ran = Command::new(&bin).output().expect("run");
    let _ = std::fs::remove_file(&ll);
    let _ = std::fs::remove_file(&bin);
    assert!(ran.status.success(), "exit {:?}", ran.status.code());
    String::from_utf8(ran.stdout).unwrap()
}

#[test]
fn wide_string_literals_used_as_pointers() {
    // `render_string`: element type, element values packed little-endian
    // from the bytes, count and alignment.
    let ir = ir("const unsigned short *p = u\"ab\\u1234\";\n\
                 const unsigned int *q = U\"a\\U0001F600\";\n");
    assert_eq!(
        lines_with(&ir, "@.str"),
        [
            "@.str = private unnamed_addr constant [4 x i16] [i16 97, i16 98, i16 4660, i16 0], align 2",
            "@.str.1 = private unnamed_addr constant [3 x i32] [i32 97, i32 128512, i32 0], align 4",
        ]
    );
}

#[test]
fn union_and_bit_field_types_use_the_alignment_unit() {
    // `render_struct`: a union or a record with bit-fields is an array of
    // its alignment unit, or of bytes when no unit fits.
    let ir = ir("union A { short s; char c[3]; } a;\n\
                 union B { long l; char c[9]; } b;\n\
                 union C { long double d; } c;\n\
                 struct D { int x : 3; } d;\n\
                 union __attribute__((aligned(32))) E { char c; } e;\n");
    assert_eq!(
        lines_with(&ir, "%"),
        [
            "%union.A = type { [ 2 x i16 ] }",
            "%union.B = type { [ 2 x i64 ] }",
            "%union.C = type { [ 1 x i128 ] }",
            "%struct.D = type { [ 1 x i32 ] }",
            "%union.E = type { [ 32 x i8 ] }",
        ]
    );
}

#[test]
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
fn gnu_case_ranges_small_wide_and_empty() {
    // `gen_switch`: a range of up to 256 values is listed case by case; a
    // wider one is tested as `value - low <= span`; an empty range is
    // dropped; a one-value range is not empty. Signed and unsigned bounds, at several widths. The expected
    // output is Clang's.
    let source = r#"
int printf(const char *, ...);
static int sw_int(int x) {
    switch (x) {
    case -300 ... -2: return 1;
    case 5 ... 1: return 2;
    case 10 ... 265: return 3;
    case 300 ... 556: return 4;
    case 600 ... 600: return 5;
    default: return 0;
    }
}
static int sw_uns(unsigned x) {
    switch (x) {
    case 3u ... 1u: return 1;
    case 4000000000u ... 4000000300u: return 2;
    case 7u ... 9u: return 3;
    case 20u ... 20u: return 4;
    default: return 0;
    }
}
static int sw_schar(signed char c) {
    switch (c) {
    case -128 ... -100: return 1;
    case 100 ... 127: return 2;
    default: return 0;
    }
}
static int sw_uchar(unsigned char c) {
    switch (c) {
    case 0 ... 9: return 1;
    case 200 ... 255: return 2;
    default: return 0;
    }
}
static int sw_long(long x) {
    switch (x) {
    case -5000000000L ... -4000000000L: return 1;
    case 4000000000L ... 4000000255L: return 2;
    default: return 0;
    }
}
int main(void) {
    int ints[] = {-301, -300, -150, -2, -1, 0, 1, 3, 5, 9, 10, 100, 265, 266, 299, 300, 400, 556, 557, 600, 601};
    for (unsigned i = 0; i < sizeof ints / sizeof ints[0]; i++) printf("%d", sw_int(ints[i]));
    printf("\n");
    unsigned uns[] = {0, 1, 2, 3, 4, 6, 7, 9, 10, 20, 3999999999u, 4000000000u, 4000000150u, 4000000300u, 4000000301u, 4294967295u};
    for (unsigned i = 0; i < sizeof uns / sizeof uns[0]; i++) printf("%d", sw_uns(uns[i]));
    printf("\n");
    for (int c = -128; c < 128; c += 3) printf("%d", sw_schar((signed char)c));
    printf("\n");
    for (int c = 0; c < 256; c += 5) printf("%d", sw_uchar((unsigned char)c));
    printf("\n");
    long longs[] = {-5000000001L, -5000000000L, -4500000000L, -4000000000L, -3999999999L, 0, 3999999999L, 4000000000L, 4000000255L, 4000000256L};
    for (unsigned i = 0; i < sizeof longs / sizeof longs[0]; i++) printf("%d", sw_long(longs[i]));
    printf("\n");
    return 0;
}
"#;
    assert_eq!(
        run(source),
        "011100000033300444050\n\
         0000003304022200\n\
         11111111110000000000000000000000000000000000000000000000000000000000000000002222222222\n\
         1100000000000000000000000000000000000000222222222222\n\
         0111000220\n"
    );
    // Listed values: 256 for 10 ... 265, 1 for 600, 3 for 7u ... 9u, 1 for
    // 20u, 29
    // and 28 for the signed char ranges, 10 and 56 for the unsigned char
    // ones, and 256 for 4000000000L ... 4000000255L. The four wider ranges
    // are tested after the switch, and the two empty ones are dropped.
    let ir = ir(source);
    let listed = ir
        .lines()
        .filter(|line| line.starts_with("    i") && line.contains(", label %"))
        .count();
    assert_eq!(listed, 256 + 1 + 3 + 1 + 29 + 28 + 10 + 56 + 256);
    assert_eq!(ir.matches("icmp ule").count(), 4);
}

#[test]
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
fn record_values_conversions_and_omitted_operands() {
    // `record_ptr` on a comma and a statement expression, the statement
    // expression's own locals (`gen_stmt_expr_body`), integer to floating
    // conversions (`convert`), and `x ?: y` on integers, pointers and
    // doubles (`gen_cond_short`, `test_nonzero`). Expected output is
    // Clang's.
    let source = r#"
int printf(const char *, ...);
struct S { int x, y; };
static struct S mk(int v) { struct S s = {v, v * 2}; return s; }
int main(void) {
    struct S s = {3, 4};
    int a = 1;
    int r1 = (a++, s).y;
    int r2 = ({ struct S t = mk(5); int k = 1, m = 2; t.x += k + m; t; }).x;
    int i = -7; unsigned u = 4000000000u; long l = -9; unsigned long ul = 18000000000000000000ul;
    double d1 = i, d2 = u, d3 = l, d4 = ul;
    float f1 = i, f2 = u;
    int z = 0; int *p = 0; double dz = 0.0;
    int c1 = z ?: 7, c2 = a ?: 9;
    int *c3 = p ?: &a;
    double c4 = dz ?: 2.5, c5 = d1 ?: 1.0;
    printf("%d %d %d %.1f %.1f %.1f %.1f %.1f %.1f\n", r1, r2, a, d1, d2, d3, d4, (double)f1, (double)f2);
    printf("%d %d %d %.1f %.1f\n", c1, c2, *c3, c4, c5);
    return 0;
}
"#;
    assert_eq!(
        run(source),
        "4 8 2 -7.0 4000000000.0 -9.0 18000000000000000000.0 -7.0 4000000000.0\n\
         7 2 2 2.5 -7.0\n"
    );
    // The statement expression's locals are entry-block allocas, in order,
    // after the function's own.
    let ir = ir(source);
    let allocas: Vec<&str> = ir
        .lines()
        .skip_while(|line| !line.starts_with("define i32 @main"))
        .filter(|line| line.contains(" = alloca "))
        .map(|line| line.trim().split(' ').next().unwrap())
        .collect();
    let first = allocas.iter().position(|slot| slot.starts_with("%se1."));
    let first = first.unwrap_or_else(|| panic!("{allocas:?}"));
    assert_eq!(allocas[first..first + 3], ["%se1.1", "%se1.2", "%se1.3"]);
    // ... and all of them are in the entry block, before its first
    // instruction that is not an alloca.
    let body: Vec<&str> = ir
        .lines()
        .skip_while(|line| !line.starts_with("define i32 @main"))
        .skip(2)
        .collect();
    let first_other = body.iter().position(|line| !line.contains(" = alloca "));
    let last_alloca = body.iter().rposition(|line| line.starts_with("  %se1."));
    assert!(last_alloca < first_other, "{body:?}");
}

#[test]
fn long_double_conversions_follow_the_target_format() {
    // `convert`: on Apple arm64 `long double` is `double`, so no `fpext`.
    let source = "long double f(double d) { return d; }\n\
                  double g(float x) { return x; }\n";
    let apple = PpOptions {
        target: crate::Target::Arm64Apple {
            macos_major: 11,
            macos_minor: 0,
        },
        ..PpOptions::default()
    };
    let ir = ir_for(source, &apple);
    assert_eq!(ir.matches("fpext").count(), 1, "{ir}");
    assert!(ir.contains("fpext float %"), "{ir}");
    let ir = ir_for(source, &PpOptions::default());
    assert!(ir.contains("fpext double %"), "{ir}");
    assert!(ir.contains("to x86_fp80"), "{ir}");
}

#[test]
fn constant_folding_in_static_initializers() {
    // Semantic analysis folds these (`compare`, `arith`, `eval_shift`).
    let ir = ir(
        "int fold_s[] = { 3 < 3, -2 < -1, 3u < 3u, 2u < 3u, 5u == 5u, 5u == 6u, \
                 (6u & 3u), (12u & 10u), (6 & 3), 1u << 31 >> 31, 0xF0u >> 4 };\n",
    );
    assert_eq!(
        lines_with(&ir, "@fold_s"),
        ["@fold_s = global [11 x i32] [i32 0, i32 1, i32 0, i32 1, i32 1, i32 0, i32 2, i32 8, i32 2, i32 1, i32 15], align 16"]
    );
}

#[test]
fn integer_constant_expressions_in_enums_and_array_bounds() {
    // The parser folds array bounds and enumerators (`eval_ice_unary`,
    // `eval_ice_bin`, `bits_ice`, `ice_shift_count`, `ice_compare`,
    // `ice_arith`).
    let ir = ir("enum { E_AND = 6u & 3u, E_XOR = 5u ^ 3u, E_NOT = ~0u & 0xFFu, E_LT = 3u < 2u, \
                 E_NE = 4u != 4u, E_DIV = 7u / 2u, E_MOD = 7u % 3u, E_SDIV = -7 / 2, \
                 E_SAND = 6 & -3, E_MINUS = -(-5) };\n\
                 int fold_p[] = { E_AND, E_XOR, E_NOT, E_LT, E_NE, E_DIV, E_MOD, E_SDIV, E_SAND, E_MINUS };\n\
                 char a1[6u & 3u], a2[5u ^ 3u], a3[7u / 2u], a4[(4u != 5u) + 1], a5[1u << 3u], \
                 a6[-(-3)], a7[(6 & 3) + 1], a8[7 % 3 + 1], a9[!0 + 1], a10[(0 && 1) + 1], \
                 a11[(-2147483647 - 1 < 0) + 1], a12[7 / 2], a13[(4 != 5) + 2], a14[1 << 2u], \
                 a15[(2 || 0) + (0 || 0) + 1], a16[(9u % 4u) + (5u - 3u)];\n");
    assert_eq!(
        lines_with(&ir, "@fold_p"),
        ["@fold_p = global [10 x i32] [i32 2, i32 6, i32 255, i32 0, i32 0, i32 3, i32 1, i32 -3, i32 4, i32 5], align 16"]
    );
    let sizes: Vec<String> = lines_with(&ir, "@a")
        .iter()
        .map(|line| {
            line.split(" x ")
                .next()
                .unwrap()
                .rsplit('[')
                .next()
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(
        sizes,
        ["2", "6", "3", "2", "8", "3", "3", "2", "2", "1", "2", "3", "3", "4", "2", "3"]
    );
}

#[test]
fn unsigned_case_ranges_order_by_value() {
    // `case_le` compares unsigned bounds as unsigned, so `1u ... 3u` is
    // not empty and `2u` inside it is a duplicate.
    let checked = crate::compile_dialect(
        "int f(unsigned x) { switch (x) { case 1u ... 3u: return 1; case 0xFFFFFFF0u ... 0xFFFFFFFFu: return 2; } return 0; }",
        "t.c",
        &PpOptions::default(),
        Dialect::Gnu11,
    )
    .unwrap_or_else(|err| panic!("{err}"));
    assert!(checked.warnings.is_empty());
    let err = match crate::compile_dialect(
        "int f(unsigned x) { switch (x) { case 1u ... 3u: return 1; case 2u: return 2; } return 0; }",
        "t.c",
        &PpOptions::default(),
        Dialect::Gnu11,
    ) {
        Ok(_) => panic!("accepted a duplicate case"),
        Err(err) => err,
    };
    assert_eq!(err.message, "duplicate case value '2'");
}

/// The body of the function whose `define` line contains `name`.
fn function_body<'a>(ir: &'a str, name: &str) -> &'a str {
    let start = ir
        .lines()
        .position(|line| line.starts_with("define ") && line.contains(name))
        .unwrap_or_else(|| panic!("no define for {name}:\n{ir}"));
    let begin = ir.lines().take(start).map(|l| l.len() + 1).sum::<usize>();
    let rest = &ir[begin..];
    &rest[..rest.find("\n}").map_or(rest.len(), |end| end + 2)]
}

#[test]
fn struct_parameter_pieces_are_not_followed_by_a_whole_slot_zero_store() {
    // `store_param`. LLVM's AddressSanitizer at -O0 moves each first store
    // of a parameter into its stack slot to the top of the function. A
    // whole-slot `zeroinitializer` store written before the register pieces
    // then runs after them and erases the parameter (the weekly sanitizer
    // run found this in lib/threads.c `thrd_join`). The pieces are stored
    // first, and only bytes no piece covers are zeroed, after them.
    let x86 = PpOptions {
        target: crate::Target::X86_64Linux,
        ..PpOptions::default()
    };
    let ir = ir_for(
        "struct s { long a; void *b; };\nlong get(struct s x) { return x.a; }\n",
        &x86,
    );
    let body = function_body(&ir, "@get(");
    assert!(!body.contains("zeroinitializer"), "{body}");
    assert!(body.contains("store i64 %p0, ptr"), "{body}");
    assert!(body.contains("store ptr %p0.1, ptr"), "{body}");

    // A float in the second eightbyte leaves bytes 12..16 (tail padding)
    // to no piece: they are zeroed, and only they, after the pieces.
    let ir = ir_for(
        "struct t { double d; float f; };\nfloat get(struct t x) { return x.f; }\n",
        &x86,
    );
    let body = function_body(&ir, "@get(");
    let lines: Vec<&str> = body.lines().collect();
    let zero = lines
        .iter()
        .position(|l| l.contains("zeroinitializer"))
        .unwrap_or_else(|| panic!("tail padding is not zeroed:\n{body}"));
    assert!(
        lines[zero].contains("store [4 x i8] zeroinitializer"),
        "{body}"
    );
    // Both pieces are stored, and the zero store follows both.
    let piece = |text: &str| {
        lines
            .iter()
            .position(|l| l.contains(text))
            .unwrap_or_else(|| panic!("no `{text}` store:\n{body}"))
    };
    let first = piece("store double %p0, ptr");
    let second = piece("store float %p0.1, ptr");
    assert!(zero > first && zero > second, "{body}");
    // The zero store is to byte 12 of the parameter's slot, not to `f`.
    let slot = lines[first]
        .split("ptr ")
        .nth(1)
        .and_then(|rest| rest.split(',').next())
        .unwrap_or_else(|| panic!("no slot in {}", lines[first]));
    let address = lines[zero]
        .split("ptr ")
        .nth(1)
        .and_then(|rest| rest.split(',').next())
        .unwrap_or_else(|| panic!("no address in {}", lines[zero]));
    let offset = format!("{address} = getelementptr inbounds i8, ptr {slot}, i64 12");
    assert!(body.contains(&offset), "want `{offset}`:\n{body}");
    assert_eq!(body.matches("zeroinitializer").count(), 1, "{body}");
}

const OBJECT_SIZE_PROGRAM: &str = "int printf(const char *, ...);\n\
         int puts(const char *);\n\
         static int sizes;\n\
         static unsigned long size(void) { sizes++; return (unsigned long)-1; }\n\
         static int fmt(char *b, unsigned long n, const char *f, ...) {\n\
             __builtin_va_list ap; __builtin_va_start(ap, f);\n\
             int r = __builtin___vsnprintf_chk(b, n, 1, size(), f, ap);\n\
             __builtin_va_end(ap); return r; }\n\
         static int fmt_all(char *b, const char *f, ...) {\n\
             __builtin_va_list ap; __builtin_va_start(ap, f);\n\
             int r = __builtin___vsprintf_chk(b, 0, __builtin_object_size(b, 0), f, ap);\n\
             __builtin_va_end(ap); return r; }\n\
         static int say(const char *f, ...) {\n\
             __builtin_va_list ap; __builtin_va_start(ap, f);\n\
             int r = __builtin___vprintf_chk(0, f, ap);\n\
             __builtin_va_end(ap); return r; }\n\
         int main(void) {\n\
             char s[32]; char *d = s; unsigned long os = __builtin_object_size(d, 1);\n\
             __builtin___memset_chk(d, 0, sizeof s, os);\n\
             __builtin___strcpy_chk(d, \"ab\", os);\n\
             __builtin___strcat_chk(d, \"cd\", size());\n\
             __builtin___strncat_chk(d, \"efgh\", 2, os);\n\
             puts(d);\n\
             __builtin___strncpy_chk(d, \"xyz\", 2, os);\n\
             char *e = __builtin___stpcpy_chk(d + 2, \"!\", os);\n\
             printf(\"%s %d\\n\", d, (int)(e - d));\n\
             int n = __builtin___sprintf_chk(d, 1, os, \"%d-%s\", 7, \"seven\");\n\
             printf(\"%d %s\\n\", n, d);\n\
             n = __builtin___snprintf_chk(d, 4, 0, os, \"%d\", 123456);\n\
             printf(\"%d %s\\n\", n, d);\n\
             n = fmt(d, 6, \"%s:%d\", \"ab\", 99);\n\
             printf(\"%d %s\\n\", n, d);\n\
             n = fmt_all(d, \"%c%c\", 'o', 'k');\n\
             printf(\"%d %s\\n\", n, d);\n\
             __builtin___printf_chk(1, \"%s\\n\", \"chk\");\n\
             say(\"%d sizes\\n\", sizes);\n\
             return 0; }\n";

#[test]
fn object_size_checking_builtins_call_the_plain_functions() {
    // GCC manual, "Object Size Checking": each `__builtin___NAME_chk`
    // takes the arguments of NAME plus the object size (and, for the
    // formatted-output family, a flag). This compiler performs no check:
    // the call is to NAME with the extra arguments removed. The macOS SDK's
    // <stdio.h> spells `vsnprintf` as `__builtin___vsnprintf_chk`.
    let source = OBJECT_SIZE_PROGRAM;
    let text = ir(source);
    assert!(
        !text.contains("_chk"),
        "a checking entry point was called:\n{text}"
    );
    for plain in [
        "@vsnprintf(",
        "@vsprintf(",
        "@vprintf(",
        "@sprintf(",
        "@snprintf(",
    ] {
        assert!(text.contains(plain), "no call to {plain}:\n{text}");
    }
    // `mempcpy` is a GNU function that macOS lacks, so it is checked in IR.
    let text = ir(
        "void *f(char *d) { return __builtin___mempcpy_chk(d, \"mp\", 2, 8); }\n\
         void g(void *s, int f) { __builtin___fprintf_chk(s, f, \"x\"); }\n",
    );
    assert!(!text.contains("_chk"), "{text}");
    assert!(
        text.contains("@mempcpy(") && text.contains("@fprintf("),
        "{text}"
    );
}

#[test]
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
fn object_size_checking_builtins_run_as_the_plain_functions() {
    let source = OBJECT_SIZE_PROGRAM;
    assert_eq!(
        run(source),
        "abcdef\nxy! 3\n7 7-seven\n6 123\n5 ab:99\n2 ok\nchk\n2 sizes\n"
    );
}

#[test]
fn object_size_is_a_constant_that_does_not_evaluate_its_operand() {
    // GCC manual, "Object Size Checking": the pointer argument is not
    // evaluated, and an unknown size is (size_t)-1 for types 0 and 1 and 0
    // for types 2 and 3. This compiler never knows the size.
    let text = ir("char *next(void);\n\
         unsigned long a(void) { return __builtin_object_size(next(), 0); }\n\
         unsigned long b(void) { return __builtin_dynamic_object_size(next(), 3); }\n");
    assert!(
        !text.contains("call ptr @next"),
        "operand evaluated:\n{text}"
    );
    assert!(
        function_body(&text, "@a(").contains("ret i64 18446744073709551615"),
        "{text}"
    );
    assert!(function_body(&text, "@b(").contains("ret i64 0"), "{text}");
}

#[test]
fn object_size_accepts_a_null_pointer_constant() {
    // The first parameter is `const void *` (GCC manual, "Object Size
    // Checking"), so an integer constant 0 converts to it (C11 6.3.2.3p3).
    let text = ir(
        "unsigned long f(void) { return __builtin_object_size(0, 0); }\n\
         unsigned long g(void) { return __builtin_object_size(0L, 2); }\n",
    );
    assert!(
        function_body(&text, "@f(").contains("ret i64 18446744073709551615"),
        "{text}"
    );
    assert!(function_body(&text, "@g(").contains("ret i64 0"), "{text}");
    // In an unevaluated operand the call has the arity it needs.
    ir("int n = sizeof(__builtin_object_size(0, 1)) + sizeof(__builtin___strcpy_chk(0, 0, 1));\n");
}

#[test]
fn object_size_checking_builtins_report_misuse() {
    for (source, message) in [
        (
            "unsigned long f(char *p, int t) { return __builtin_object_size(p, t); }",
            "integer constant",
        ),
        (
            "unsigned long f(char *p) { return __builtin_object_size(p, 4); }",
            "0 to 3",
        ),
        (
            "unsigned long f(int p) { return __builtin_object_size(p, 0); }",
            "pointer",
        ),
        (
            "void f(char *p) { __builtin___strcpy_chk(p, \"a\"); }",
            "expects 3 arguments",
        ),
        (
            "void f(char *p) { __builtin___strcpy_chk(p, \"a\", p); }",
            "integer",
        ),
        (
            "int f(char *p) { return __builtin___sprintf_chk(p, 0, 1); }",
            "at least 4",
        ),
        // The argument count is checked in an operand that is never
        // evaluated too (C11 6.5.2.2p2), where only the folded size is kept.
        (
            "char a[sizeof(__builtin_object_size())];",
            "wrong number of arguments",
        ),
        (
            "_Static_assert(sizeof(__builtin___strcpy_chk(0)) == 8, \"\");",
            "wrong number of arguments",
        ),
        (
            "int f(void) { char a[sizeof(__builtin___sprintf_chk(0, 0))]; return sizeof a; }",
            "wrong number of arguments",
        ),
        (
            "unsigned long f(void) { return __builtin_object_size(1, 0); }",
            "pointer",
        ),
    ] {
        let err = crate::compile_dialect(source, "t.c", &PpOptions::default(), Dialect::Gnu11)
            .err()
            .unwrap_or_else(|| panic!("accepted: {source}"))
            .to_string();
        assert!(err.contains(message), "{source}: {err}");
    }
}
