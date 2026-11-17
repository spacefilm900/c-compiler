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
    // dropped. Signed and unsigned bounds, at several widths. The expected
    // output is Clang's.
    let source = r#"
int printf(const char *, ...);
static int sw_int(int x) {
    switch (x) {
    case -300 ... -2: return 1;
    case 5 ... 1: return 2;
    case 10 ... 265: return 3;
    case 300 ... 556: return 4;
    case 600: return 5;
    default: return 0;
    }
}
static int sw_uns(unsigned x) {
    switch (x) {
    case 3u ... 1u: return 1;
    case 4000000000u ... 4000000300u: return 2;
    case 7u ... 9u: return 3;
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
    unsigned uns[] = {0, 1, 2, 3, 4, 6, 7, 9, 10, 3999999999u, 4000000000u, 4000000150u, 4000000300u, 4000000301u, 4294967295u};
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
         000000330022200\n\
         11111111110000000000000000000000000000000000000000000000000000000000000000002222222222\n\
         1100000000000000000000000000000000000000222222222222\n\
         0111000220\n"
    );
    // Listed values: 256 for 10 ... 265, 1 for 600, 3 for 7u ... 9u, 29
    // and 28 for the signed char ranges, 10 and 56 for the unsigned char
    // ones, and 256 for 4000000000L ... 4000000255L. The four wider ranges
    // are tested after the switch, and the two empty ones are dropped.
    let ir = ir(source);
    let listed = ir
        .lines()
        .filter(|line| line.starts_with("    i") && line.contains(", label %"))
        .count();
    assert_eq!(listed, 256 + 1 + 3 + 29 + 28 + 10 + 56 + 256);
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
