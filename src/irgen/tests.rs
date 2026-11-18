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
        .args([
            "-w",
            ll.to_str().unwrap(),
            "-o",
            bin.to_str().unwrap(),
            "-lm",
        ])
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
            "-lm",
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
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
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
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
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
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
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
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
fn static_locals_block_linkage_and_do_while() {
    let ir = crate::compile_to_ir(
        "\
static int hidden = 1;
int exposed = 4;
int ext = 2;
int bump(void) {
    static int n = 3;
    n = n + 1;
    return n;
}
int other(void) {
    static int n = 10;
    n = n + 1;
    return n;
}
int read_hidden(void) { return hidden; }
int see_exposed(void) {
    int exposed = 0;
    {
        extern int exposed;
        return exposed;
    }
}
int see_ext(void) {
    int ext = 0;
    {
        extern int ext;
        ext = ext + 5;
        return ext;
    }
}
int call_later(void) {
    int id(int);
    return id(6);
}
int id(int x) { return x; }
int loop(void) {
    int n = 0;
    int i = 0;
    do {
        i = i + 1;
        if (i < 3)
            continue;
        n = n + 1;
    } while (i < 4);
    return n * 10 + i;
}
int main(void) {
    if (bump() != 4) return 1;
    if (bump() != 5) return 2;
    if (other() != 11) return 3;
    if (read_hidden() != 1) return 4;
    if (see_exposed() != 4) return 8;
    if (see_ext() != 7) return 5;
    if (call_later() != 6) return 6;
    if (loop() != 24) return 7;
    return 0;
}
",
        "block.c",
    )
    .unwrap();
    assert!(
        ir.contains("@bump.n.1 = internal global i32 3, align 4"),
        "{ir}"
    );
    assert!(
        ir.contains("@other.n.2 = internal global i32 10, align 4"),
        "{ir}"
    );
    assert!(
        ir.contains("@hidden = internal global i32 1, align 4"),
        "{ir}"
    );
    assert!(ir.contains("@exposed = global i32 4, align 4"), "{ir}");
    assert!(ir.contains("@ext = global i32 2, align 4"), "{ir}");
    assert!(!ir.contains("store i32 3"), "{ir}");
    assert!(!ir.contains("declare i32 @id"), "{ir}");
    assert!(ir.contains("define i32 @id(i32 %p0)"), "{ir}");
    assert_eq!(run_binary(&ir).0, 0);

    let external = crate::compile_to_ir(
        "\
int main(void) {
    extern int missing;
    extern int missing;
    return missing;
}
",
        "ext.c",
    )
    .unwrap();
    assert_eq!(
        external
            .matches("@missing = external global i32, align 4")
            .count(),
        1,
        "{external}"
    );
}

#[test]
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
fn return_constant_ir_assembles() {
    let ir = crate::compile_to_ir("int main(void) { return 42; }\n", "t.c").unwrap();
    assert!(ir.contains("define i32 @main()"));
    assert!(ir.contains("ret i32 42"));
    assert!(ir.contains("target triple = \"x86_64-pc-linux-gnu\""));
    assert!(ir.contains(&format!(
        "target datalayout = \"{}\"",
        crate::target::Target::X86_64Linux.data_layout()
    )));
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
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
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
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
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
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
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
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
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
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
fn incomplete_extern_keeps_a_distinct_type_name() {
    let source = "\
void f(void) { struct S { int a; }; }
extern struct S x;
void g(void) { struct S { int b; }; }
int main(void) { return &x == 0; }
";
    let ir = crate::compile_to_ir(source, "opaque.c").unwrap();
    let mut names = std::collections::HashSet::new();
    for line in ir.lines() {
        let Some(rest) = line.strip_prefix('%') else {
            continue;
        };
        let Some((name, _)) = rest.split_once(" = type") else {
            continue;
        };
        assert!(
            names.insert(name.to_string()),
            "duplicate LLVM type %{name}\n{ir}"
        );
    }
    assert!(ir.contains("type opaque"), "{ir}");
    let (ll, _) = temp_paths();
    std::fs::write(&ll, &ir).unwrap();
    let obj = ll.with_extension("o");
    let assembled = Command::new("clang")
        .args([
            "-c",
            "-w",
            ll.to_str().unwrap(),
            "-o",
            obj.to_str().unwrap(),
        ])
        .output()
        .expect("clang");
    let _ = std::fs::remove_file(&ll);
    let _ = std::fs::remove_file(&obj);
    assert!(
        assembled.status.success(),
        "clang rejected IR:\n{}\n{ir}",
        String::from_utf8_lossy(&assembled.stderr)
    );
}

#[test]
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
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
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
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

#[test]
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
fn switch_lowers_for_every_integer_width() {
    let source = "\
int on_int(int x) {
    switch (x) {
    case 1: return 1;
    case 2: case 3: return 2;
    default: return 4;
    }
}
int on_char(char c) {
    switch (c) {
    case -1: return 1;
    case 2: return 2;
    default: return 3;
    }
}
int on_uchar(unsigned char c) {
    switch (c) {
    case 255: return 1;
    case -1: return 2;
    default: return 3;
    }
}
int on_long(long x) {
    switch (x) {
    case -1: return 1;
    case 8589934592L: return 2;
    default: return 3;
    }
}
int on_ull(unsigned long long x) {
    switch (x) {
    case -1: return 1;
    case 2: return 2;
    default: return 3;
    }
}
int on_uint(unsigned x) {
    switch (x) {
    case -1: return 1;
    default: return 2;
    }
}
int main(void) {
    if (on_int(1) != 1 || on_int(3) != 2 || on_int(9) != 4) return 1;
    if (on_char(-1) != 1 || on_char(2) != 2 || on_char(0) != 3) return 2;
    if (on_uchar(255) != 1 || on_uchar(0) != 3) return 3;
    if (on_long(-1) != 1 || on_long(8589934592L) != 2) return 4;
    if (on_ull(-1) != 1 || on_ull(2) != 2 || on_ull(0) != 3) return 5;
    if (on_uint(4294967295u) != 1 || on_uint(1) != 2) return 6;
    return 0;
}
";
    let ir = crate::compile_to_ir(source, "switch.c").unwrap();
    assert!(ir.contains("switch i32"), "{ir}");
    assert!(ir.contains("switch i64"), "{ir}");
    assert!(ir.contains("sext i8"), "{ir}");
    assert!(ir.contains("zext i8"), "{ir}");
    assert!(ir.contains("i32 -1, label"), "{ir}");
    assert!(ir.contains("i64 -1, label"), "{ir}");
    assert!(ir.contains("i64 8589934592, label"), "{ir}");
    assert!(ir.contains("i64 18446744073709551615, label"), "{ir}");
    assert!(ir.contains("i32 4294967295, label"), "{ir}");
    assert_eq!(run_binary(&ir), run_reference(source));
}

#[test]
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
fn struct_abi_links_with_clang() {
    let defs = r#"
struct s1 { unsigned char a; };
struct s3 { unsigned char a, b, c; };
struct s8 { long a; };
struct s12 { int a, b, c; };
struct s16 { long a; char b; };
struct big { long a, b, c; };
struct nest { char c; int i; };
struct outer { struct nest n; char z; };
struct ptrs { char c; int *p; };
union u9 { long a; unsigned char b[9]; };
struct s1 id1(struct s1 s) { return s; }
struct s3 id3(struct s3 s) { return s; }
struct s8 id8(struct s8 s) { return s; }
struct s12 id12(struct s12 s) { return s; }
struct s16 id16(struct s16 s) { return s; }
struct big idbig(struct big s) { s.b += 1; return s; }
struct outer idouter(struct outer s) { return s; }
int via_ptr(struct ptrs s) { return s.c + *s.p; }
int via_union(union u9 u) { return u.b[8]; }
int apply(int (*f)(int), int x) { return f(x + 1); }
int mix(int a, struct s12 s, int b, struct big g, int c) {
    return a + s.a + s.c + b + (int)g.a + c;
}
int spill(int a, int b, int c, int d, int e, struct s16 s, int z) {
    return a + b + c + d + e + z + (int)s.a + s.b;
}
"#;
    let caller = r#"
struct s1 { unsigned char a; };
struct s3 { unsigned char a, b, c; };
struct s8 { long a; };
struct s12 { int a, b, c; };
struct s16 { long a; char b; };
struct big { long a, b, c; };
struct nest { char c; int i; };
struct outer { struct nest n; char z; };
struct ptrs { char c; int *p; };
union u9 { long a; unsigned char b[9]; };
struct s1 id1(struct s1 s);
struct s3 id3(struct s3 s);
struct s8 id8(struct s8 s);
struct s12 id12(struct s12 s);
struct s16 id16(struct s16 s);
struct big idbig(struct big s);
struct outer idouter(struct outer s);
int via_ptr(struct ptrs s);
int via_union(union u9 u);
int apply(int (*f)(int), int x);
int mix(int a, struct s12 s, int b, struct big g, int c);
int spill(int a, int b, int c, int d, int e, struct s16 s, int z);
int inc(int x) { return x + 1; }
int main(void) {
    struct s1 a = {9};
    if (id1(a).a != 9) return 1;
    struct s3 b = {1, 2, 3};
    if (id3(b).a != 1 || id3(b).c != 3) return 2;
    struct s8 c = {99};
    if (id8(c).a != 99) return 3;
    struct s12 d = {4, 5, 6};
    if (id12(d).a != 4 || id12(d).b != 5 || id12(d).c != 6) return 4;
    struct s16 e = {7, 8};
    if (id16(e).a != 7 || id16(e).b != 8) return 5;
    struct big f = {10, 20, 30};
    struct big g = idbig(f);
    if (f.b != 20 || g.a != 10 || g.b != 21 || g.c != 30) return 6;
    struct outer h = {{1, 2}, 3};
    struct outer i = idouter(h);
    if (i.n.c != 1 || i.n.i != 2 || i.z != 3) return 7;
    int n = 40;
    struct ptrs p = {2, &n};
    if (via_ptr(p) != 42) return 8;
    union u9 u;
    u.a = 0;
    u.b[8] = 5;
    if (via_union(u) != 5) return 9;
    if (apply(inc, 40) != 42) return 10;
    if (mix(1, d, 2, f, 3) != 1 + 4 + 6 + 2 + 10 + 3) return 11;
    struct s16 sp = {100, 7};
    if (spill(1, 2, 3, 4, 5, sp, 6) != 128) return 12;
    return 0;
}
"#;
    let defs_ir = crate::compile_to_ir(defs, "defs.c").unwrap();
    assert!(defs_ir.contains("define i8 @id1(i8 %p0)"), "{defs_ir}");
    assert!(defs_ir.contains("define i24 @id3(i24 %p0)"), "{defs_ir}");
    assert!(
        defs_ir.contains("define { i64, i32 } @id12(i64 %p0, i32 %p0.1)"),
        "{defs_ir}"
    );
    assert!(
        defs_ir.contains("define { i64, i8 } @id16(i64 %p0, i8 %p0.1)"),
        "{defs_ir}"
    );
    assert!(
        defs_ir.contains("ptr sret(%struct.big) align 8 %agg.result"),
        "{defs_ir}"
    );
    assert!(
        defs_ir.contains("ptr byval(%struct.big) align 8 %p0"),
        "{defs_ir}"
    );
    assert!(
        defs_ir.contains("ptr byval(%struct.s16) align 8 %p5"),
        "{defs_ir}"
    );
    link_both_ways(defs, caller, &defs_ir);
}

fn link_both_ways(defs: &str, caller: &str, defs_ir: &str) {
    let (ll, bin) = temp_paths();
    let obj = ll.with_extension("o");
    let cfile = ll.with_extension("clang.c");
    // Our caller, clang definitions.
    let caller_ir = crate::compile_to_ir(caller, "caller.c").unwrap();
    std::fs::write(&ll, &caller_ir).unwrap();
    std::fs::write(&cfile, defs).unwrap();
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
        "link our caller:\n{}",
        String::from_utf8_lossy(&linked.stderr)
    );
    let ran = Command::new(&bin).output().expect("run");
    assert_eq!(
        ran.status.code(),
        Some(0),
        "our caller: {}",
        String::from_utf8_lossy(&ran.stderr)
    );

    // Our definitions, clang caller.
    let our_ll = ll.with_extension("our.ll");
    let clang_main = ll.with_extension("main.c");
    let clang_obj_path = ll.with_extension("main.o");
    std::fs::write(&our_ll, defs_ir).unwrap();
    std::fs::write(&clang_main, caller).unwrap();
    let compiled = Command::new("clang")
        .args([
            "-w",
            "-std=c11",
            "-c",
            clang_main.to_str().unwrap(),
            "-o",
            clang_obj_path.to_str().unwrap(),
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
            clang_obj_path.to_str().unwrap(),
            "-o",
            bin2.to_str().unwrap(),
        ])
        .output()
        .expect("clang link");
    assert!(
        linked.status.success(),
        "link our defs:\n{}\n{defs_ir}",
        String::from_utf8_lossy(&linked.stderr)
    );
    let ran = Command::new(&bin2).output().expect("run");
    assert_eq!(
        ran.status.code(),
        Some(0),
        "our defs exit {}",
        ran.status.code().unwrap_or(-1)
    );
    let _ = std::fs::remove_file(&ll);
    let _ = std::fs::remove_file(&obj);
    let _ = std::fs::remove_file(&cfile);
    let _ = std::fs::remove_file(&bin);
    let _ = std::fs::remove_file(&our_ll);
    let _ = std::fs::remove_file(&clang_main);
    let _ = std::fs::remove_file(&clang_obj_path);
    let _ = std::fs::remove_file(&bin2);
}
#[test]
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
fn signaling_nan_constants_keep_ieee_encoding() {
    // GCC "Floating-Point Format Builtins": a default `__builtin_nans*`
    // is a signaling NaN. LLVM hex constants for `float` use the
    // widened IEEE-64 pattern (Language Reference, "Simple Constants").
    let source = concat!(
        "float f = __builtin_nansf(\"\");\n",
        "double d = __builtin_nans(\"\");\n",
        "long double l = __builtin_nansl(\"\");\n",
    );
    let ir = crate::compile_to_ir(source, "nans.c").unwrap();
    assert!(
        ir.contains("@f = global float 0x7FF4000000000000"),
        "nansf encoding quieted or missing: {ir}"
    );
    assert!(
        ir.contains("@d = global double 0x7FF4000000000000"),
        "nans encoding missing: {ir}"
    );
    assert!(
        ir.contains("@l = global x86_fp80 0xK7FFFA000000000000000"),
        "nansl encoding missing: {ir}"
    );

    // Clang 19 as a black-box oracle: the same hex constants in its IR.
    let cfile = temp_paths().0.with_extension("c");
    std::fs::write(&cfile, source).unwrap();
    let ll = cfile.with_extension("ll");
    let compiled = Command::new("clang")
        .args([
            "-std=c11",
            "-O0",
            "-S",
            "-emit-llvm",
            "-o",
            ll.to_str().unwrap(),
            cfile.to_str().unwrap(),
        ])
        .output()
        .expect("clang");
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let clang_ir = std::fs::read_to_string(&ll).unwrap();
    assert!(
        clang_ir.contains("float 0x7FF4000000000000"),
        "clang nansf: {clang_ir}"
    );
    assert!(
        clang_ir.contains("double 0x7FF4000000000000"),
        "clang nans: {clang_ir}"
    );
    assert!(
        clang_ir.contains("x86_fp80 0xK7FFFA000000000000000"),
        "clang nansl: {clang_ir}"
    );
    let _ = std::fs::remove_file(&cfile);
    let _ = std::fs::remove_file(&ll);

    // C string semantics: bytes after the first NUL are not part of
    // the payload, so "1\0ignored" matches "1".
    let nul_ir = crate::compile_to_ir(
        "double a = __builtin_nan(\"1\");\ndouble b = __builtin_nan(\"1\\0ignored\");\n",
        "nan-nul.c",
    )
    .unwrap();
    assert_eq!(
        nul_ir.matches("0x7FF8000000000001").count(),
        2,
        "NUL payload should match \"1\": {nul_ir}"
    );
}

#[test]
fn malformed_runtime_nan_tag_calls_libm() {
    // Clang 19 black-box: a runtime __builtin_nan("1xyz") is a call of
    // nan() (C11 7.12.11.2), not a payload-zero constant. The same
    // helper used by constant evaluation decides whether the tag folds.
    let ir = crate::compile_to_ir(
        "double f(void) { return __builtin_nan(\"1xyz\"); }\n",
        "nan-runtime.c",
    )
    .unwrap();
    assert!(
        ir.contains("declare double @nan(ptr)"),
        "missing nan declaration: {ir}"
    );
    assert!(
        ir.contains("call double @nan(ptr") && ir.contains("c\"1xyz\\00\""),
        "should call nan() with the tag string: {ir}"
    );
    assert!(
        !ir.contains("ret double 0x7FF8000000000000"),
        "malformed tag must not become a payload-zero NaN: {ir}"
    );
    assert_eq!(ir.matches("declare double @nan(ptr)").count(), 1, "{ir}");

    let irf = crate::compile_to_ir(
        "float f(void) { return __builtin_nanf(\"1xyz\"); }\n",
        "nanf-runtime.c",
    )
    .unwrap();
    assert!(irf.contains("declare float @nanf(ptr)"), "{irf}");
    assert!(irf.contains("call float @nanf(ptr"), "{irf}");

    let irl = crate::compile_to_ir(
        "long double f(void) { return __builtin_nanl(\"1xyz\"); }\n",
        "nanl-runtime.c",
    )
    .unwrap();
    assert!(irl.contains("declare x86_fp80 @nanl(ptr)"), "{irl}");
    assert!(irl.contains("call x86_fp80 @nanl(ptr"), "{irl}");

    // A valid tag still folds, including in a function body.
    let ok = crate::compile_to_ir(
        "double f(void) { return __builtin_nan(\"1\"); }\n",
        "nan-ok.c",
    )
    .unwrap();
    assert!(ok.contains("ret double 0x7FF8000000000001"), "{ok}");
    assert!(!ok.contains("@nan("), "{ok}");

    // Do not emit a second declare if the unit already names nan.
    let declared = crate::compile_to_ir(
        "double nan(const char *); double f(void) { return __builtin_nan(\"1xyz\"); }\n",
        "nan-declared.c",
    )
    .unwrap();
    assert_eq!(
        declared.matches("declare double @nan(ptr)").count(),
        1,
        "{declared}"
    );
    assert!(declared.contains("call double @nan(ptr"), "{declared}");

    // A static malformed tag is still rejected. So is a runtime
    // __builtin_nans* malformed tag (no library equivalent).
    let err =
        crate::compile_to_ir("double x = __builtin_nan(\"1xyz\");\n", "nan-static.c").unwrap_err();
    assert!(
        err.message.contains("not a constant expression"),
        "{}",
        err.message
    );
    let err = crate::compile_to_ir(
        "double f(void) { return __builtin_nans(\"1xyz\"); }\n",
        "nans-runtime.c",
    )
    .unwrap_err();
    assert!(
        err.message.contains("strtol would consume in full"),
        "{}",
        err.message
    );
}

#[test]
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
fn floating_comparison_builtins_use_quiet_fcmp() {
    // LLVM LangRef: fcmp is not signalling, and the `o` predicates are
    // false when either operand is a NaN while `uno` is true. That is
    // the C11 7.12.14 macro behaviour.
    let cases = [
        ("__builtin_isgreater", "ogt"),
        ("__builtin_isgreaterequal", "oge"),
        ("__builtin_isless", "olt"),
        ("__builtin_islessequal", "ole"),
        ("__builtin_islessgreater", "one"),
        ("__builtin_isunordered", "uno"),
    ];
    for (name, pred) in cases {
        let ir = crate::compile_to_ir(
            &format!("int f(double a, double b) {{ return {name}(a, b); }}\n"),
            "fpcmp.c",
        )
        .unwrap();
        assert!(ir.contains(&format!("fcmp {pred} double")), "{name}: {ir}");
        assert!(
            !ir.contains(&format!("@{name}")),
            "{name} is not a call: {ir}"
        );
    }
    // Usual arithmetic conversions: float and long double compare
    // as x86_fp80; int and float compare as float.
    let ir = crate::compile_to_ir(
        "int f(float a, long double b) { return __builtin_isless(a, b); }\n",
        "fpcmp-mixed.c",
    )
    .unwrap();
    assert!(ir.contains("fpext float"), "{ir}");
    assert!(ir.contains("fcmp olt x86_fp80"), "{ir}");
    let ir = crate::compile_to_ir(
        "int f(int a, float b) { return __builtin_isunordered(a, b); }\n",
        "fpcmp-int.c",
    )
    .unwrap();
    assert!(ir.contains("sitofp i32"), "{ir}");
    assert!(ir.contains("fcmp uno float"), "{ir}");

    let source = "\
int main(void) {
    volatile double n = __builtin_nan(\"\"), one = 1.0;
    volatile long double l = 2.0L;
    volatile float f = -0.0f;
    if (__builtin_isless(n, one) || __builtin_isgreater(one, n)) return 1;
    if (!__builtin_isunordered(n, l) || __builtin_islessgreater(n, f)) return 2;
    if (!__builtin_isgreaterequal(l, one) || !__builtin_islessequal(f, 0.0)) return 3;
    if (__builtin_islessgreater(f, 0) || !__builtin_islessgreater(l, one)) return 4;
    return __builtin_isgreater(l, f) + __builtin_isunordered(one, l) * 4;
}
";
    let ir = crate::compile_to_ir(source, "fpcmp-run.c").unwrap();
    assert_eq!(run_binary(&ir), run_reference(source));
    assert_eq!(run_binary(&ir).0, 1);
}

#[test]
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
fn internal_object_named_like_a_hidden_libcall_is_renamed() {
    // LLVM IR has one namespace for global variables and functions.
    // A file-scope `static int nan;` is valid C11 (7.1.3 reserves
    // `nan` at file scope only with <math.h>), and its internal
    // linkage cannot meet the library at link time. When IR lowering
    // itself calls nan/nanf/nanl, the object gets a private symbol
    // so `@nan` names only the library function.
    let source = "\
static int nan;
double f(void) { return __builtin_nan(\"1xyz\"); }
int main(void) { double d = f(); return (d == d) + nan; }
";
    let ir = crate::compile_to_ir(source, "nan-clash.c").unwrap();
    assert!(!ir.contains("@nan = "), "object must not keep @nan: {ir}");
    assert!(ir.contains("declare double @nan(ptr)"), "{ir}");
    assert!(ir.contains("call double @nan(ptr"), "{ir}");
    assert_eq!(run_binary(&ir).0, 0, "{ir}");

    for (decl, call, ty) in [
        (
            "static float nanf = 2.0f;",
            "__builtin_nanf(\"x\")",
            "float",
        ),
        (
            "static long double nanl = 2.0L;",
            "__builtin_nanl(\"x\")",
            "long double",
        ),
    ] {
        let lib = if ty == "float" { "nanf" } else { "nanl" };
        let source = format!(
            "{decl}\n{ty} f(void) {{ return {call}; }}\n\
                 int main(void) {{ {ty} d = f(); return (d == d) + ({lib} != 2.0f); }}\n"
        );
        let ir = crate::compile_to_ir(&source, "nanf-clash.c").unwrap();
        assert!(!ir.contains(&format!("@{lib} = ")), "{ir}");
        assert!(
            ir.contains(&format!(
                "call {} @{lib}(ptr",
                if ty == "float" { "float" } else { "x86_fp80" }
            )),
            "{ir}"
        );
        assert_eq!(run_binary(&ir).0, 0, "{ir}");
    }

    // Every reference follows the renamed object: an address constant,
    // a block-scope extern that names the same object, and a pointer
    // initializer in a block-scope static.
    let source = "\
static int nan = 40;
static int *where = &nan;
int get(void) { extern int nan; return nan; }
double f(void) { return __builtin_nan(\"-1\"); }
int main(void) {
    static int *again = &nan;
    double d = f();
    if (d == d) return 1;
    *where += 1;
    return get() + *again - 82;
}
";
    let ir = crate::compile_to_ir(source, "nan-refs.c").unwrap();
    assert!(!ir.contains("@nan = "), "{ir}");
    assert_eq!(run_binary(&ir).0, 0, "{ir}");
}

#[test]
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
fn no_rename_without_a_hidden_libcall_clash() {
    // A valid tag folds to a constant and makes no call.
    for tag in ["", "0x10"] {
        let ir = crate::compile_to_ir(
            &format!(
                "static int nan = 3;\ndouble f(void) {{ return __builtin_nan(\"{tag}\"); }}\n\
                     int main(void) {{ return nan; }}\n"
            ),
            "nan-valid.c",
        )
        .unwrap();
        assert!(ir.contains("@nan = internal global i32 3"), "{ir}");
        assert!(!ir.contains("@nan("), "{ir}");
    }
    // No builtin at all.
    let ir = crate::compile_to_ir(
        "static int nan = 3;\nint main(void) { return nan; }\n",
        "nan-plain.c",
    )
    .unwrap();
    assert!(ir.contains("@nan = internal global i32 3"), "{ir}");
    // A block-scope static has its own symbol already; the library
    // call still names @nan.
    let source = "\
int main(void) {
    static int nan = 5;
    double d = __builtin_nan(\"1xyz\");
    return (d == d) + nan - 5;
}
";
    let ir = crate::compile_to_ir(source, "nan-block.c").unwrap();
    assert!(ir.contains("call double @nan(ptr"), "{ir}");
    assert!(ir.contains("@main.nan."), "{ir}");
    assert_eq!(run_binary(&ir).0, 0, "{ir}");
    // A user-defined static function named nan stays the call target.
    let source = "\
static double nan(const char *tag) { return tag[0] == '1' ? 42.0 : 0.0; }
int main(void) { return (int)__builtin_nan(\"1xyz\") - 42; }
";
    let ir = crate::compile_to_ir(source, "nan-user.c").unwrap();
    assert!(ir.contains("define internal double @nan(ptr"), "{ir}");
    assert!(!ir.contains("declare double @nan"), "{ir}");
    assert_eq!(run_binary(&ir).0, 0, "{ir}");
}

#[test]
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
fn internal_names_of_memory_libcalls_are_renamed() {
    // llvm.memcpy, llvm.memmove, and llvm.memset may be lowered to
    // calls of the C library memcpy, memmove, and memset (LLVM
    // LangRef), and the optimizer may introduce those calls too. An
    // internal object or function with one of those names would
    // capture the call in the object file, so it gets a private name.
    let source = "\
static int memmove = 5;
static int memcpy = 6;
static int memset(int x) { return x + 1; }
struct Big { char b[4096]; };
struct Big g1, g2;
int main(void) {
    struct Big local;
    char buf[4096];
    __builtin_memset(local.b, 1, sizeof local.b);
    __builtin_memcpy(buf, local.b, sizeof buf);
    g1 = local;
    g2 = g1;
    return g2.b[100] + buf[4000] + memmove + memcpy + memset(-1) - 13;
}
";
    let ir = crate::compile_to_ir(source, "mem-clash.c").unwrap();
    for name in ["memmove", "memcpy"] {
        assert!(!ir.contains(&format!("@{name} = ")), "{name}: {ir}");
    }
    assert!(!ir.contains("@memset("), "{ir}");
    assert_eq!(run_binary(&ir).0, 0, "{ir}");
}

#[test]
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "builds and runs x86_64 Linux code"
)]
fn runtime_offsetof_has_no_inbounds_gep_from_null() {
    // A getelementptr inbounds from a null base with a nonzero offset
    // is poison (LLVM LangRef, "getelementptr"), so a subscript that
    // is not constant must not be lowered as an address from null.
    let source = "struct In { char t; short s[3]; };\n\
                      struct S { char c; int a[4]; struct In in[5]; };\n\
                      unsigned long f(int i, unsigned j) {\n\
                        return __builtin_offsetof(struct S, a[i])\n\
                             + __builtin_offsetof(struct S, in[j].s[i]);\n\
                      }\n\
                      int main(void) { return f(2, 3) == 62 ? 0 : 1; }\n";
    let ir = crate::compile_to_ir(source, "offsetof-runtime.c").unwrap();
    assert!(!ir.contains("ptr null"), "{ir}");
    assert_eq!(run_binary(&ir).0, 0, "{ir}");
}
