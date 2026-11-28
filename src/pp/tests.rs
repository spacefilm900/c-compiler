use super::*;

fn pp(source: &str) -> String {
    let mut options = PpOptions::default();
    options.system_dirs.clear();
    preprocess(source, "t.c", &options).unwrap()
}

fn pp_err(source: &str) -> String {
    let mut options = PpOptions::default();
    options.system_dirs.clear();
    preprocess(source, "t.c", &options).unwrap_err().to_string()
}

fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn splices_lines_and_drops_comments() {
    let text = pp("int ab\\\nc = 1; // hidden\\\nstill\n/* a \\\nb */ int y;\n");
    assert_eq!(squash(&text), "int abc = 1; int y;");
}

#[test]
fn expansions_longer_than_the_consumed_prefix() {
    // The first token of the file expands to more tokens than have been
    // read, and each of those expands again, so `splice` has to make
    // room in front of the cursor several times over.
    let text = pp("#define A B B B B\n#define B C C C C\n#define C 1\nA\n#define E\nE # x\n#define F(p) p p p p p p p p\nF(F(F(2)))\n");
    let flat = squash(&text);
    assert!(flat.starts_with(&"1 ".repeat(16)), "{flat}");
    // A '#' that comes out of an expansion does not start a directive.
    assert!(flat.contains("# x"), "{flat}");
    assert_eq!(flat.matches('2').count(), 512, "{flat}");
}

#[test]
fn object_and_function_macros() {
    let text = pp("#define A 2\n#define B A+A\n#define C (A+A)\nint x = B*2;\nint y = C*2;\n#define ID(x) x\n#define ADD(a, b) ((a)+(b))\nint z = ADD(ID(1), 2);\n");
    assert!(text.contains("int x = 2+2*2;"), "{text}");
    assert!(text.contains("int y = (2+2)*2;"), "{text}");
    assert!(text.contains("int z = ((1)+(2));"), "{text}");
}

#[test]
fn hides_recursive_macros_and_rescans() {
    let text = pp("#define x 3\n#define f(a) f(x * (a))\n#undef x\n#define x 2\n#define g f\n#define z z[0]\n#define h g(~\n#define m(a) a(w)\n#define w 0,1\n#define t(a) a\nf(y+1)\nh 5)\nm(f)\n");
    let flat = squash(&text);
    assert!(flat.contains("f(2 * (y+1))"), "{flat}");
    assert!(flat.contains("f(2 * (~ 5))"), "{flat}");
    assert!(flat.contains("f(2 * (0,1))"), "{flat}");
}

#[test]
fn rescan_ends_with_the_replacement_list() {
    // C11 6.10.3.5p5: `f(2)(9)` is `2*9*g`. The `f` that `g(9)` produces is
    // replaced, because the `(9)` that ends g's invocation is past the end
    // of f's replacement list.
    let text = pp("#define f(a) a*g\n#define g(a) f(a)\nf(2)(9)\n");
    assert_eq!(squash(&text).trim(), "2*9*g", "{text}");
    // A `)` produced inside the enclosing expansion keeps it painted.
    let text = pp("#define f(a) a g(9)\n#define g(a) f(a)\nf(2)\n");
    assert_eq!(squash(&text).trim(), "2 f(9)", "{text}");
    // The rest of 6.10.3.5p5's EXAMPLE 3.
    let text = pp("#define x 3\n#define f(a) f(x * (a))\n#undef x\n#define x 2\n#define g f\n#define z z[0]\n#define h g(~\n#define m(a) a(w)\n#define w 0,1\n#define t(a) a\n#define p() int\n#define q(x) x\n#define r(x,y) x ## y\n#define str(x) # x\nf(y+1) + f(f(z)) % t(t(g)(0) + t)(1);\ng(x+(3,4)-w) | h 5) & m\n(f)^m(m);\np() i[q()] = { q(1), r(2,3), r(4,), r(,5), r(,) };\nchar c[2][6] = { str(hello), str() };\n");
    let flat = squash(&text);
    for want in [
        "f(2 * (y+1)) + f(2 * (f(2 * (z[0])))) % f(2 * (0)) + t(1);",
        "f(2 * (2+(3,4)-0,1)) | f(2 * (~ 5)) & f(2 * (0,1))^m(0,1);",
        "int i[] = { 1, 23, 4, 5, };",
        "char c[2][6] = { \"hello\", \"\" };",
    ] {
        assert!(flat.contains(want), "{want} not in {flat}");
    }
}

#[test]
fn stringifies_and_pastes() {
    let text = pp("#define STR(x) #x\n#define XSTR(x) STR(x)\n#define N 40\nconst char *a = STR(a + b);\nconst char *b = XSTR(N);\nconst char *c = STR();\n#define CAT(a, b) a ## b\n#define CAT3(a, b, c) a ## b ## c\nint CAT(x, y) = CAT3(1, 2, 3);\n");
    assert!(text.contains("\"a + b\""), "{text}");
    assert!(text.contains("\"40\""), "{text}");
    assert!(text.contains("\"\""), "{text}");
    assert!(text.contains("int xy = 123;"), "{text}");
}

#[test]
fn variadic_macros_and_omitted_comma() {
    let text = pp("#define FIRST(a, ...) a\n#define REST(a, ...) __VA_ARGS__\n#define F(a, ...) [a, ## __VA_ARGS__]\nint a = FIRST(3, 4, 5);\nint b = REST(9, 1, 2);\nF(1)\nF(1,)\nF(1, 2, 3)\n");
    assert!(text.contains("int a = 3;"), "{text}");
    assert!(text.contains("int b = 1, 2;"), "{text}");
    assert!(text.contains("[1]"), "{text}");
    assert!(text.contains("[1,]"), "{text}");
    assert!(text.contains("[1, 2, 3]"), "{text}");
}

#[test]
fn conditionals_skip_groups_and_evaluate() {
    let text = pp("#if (1+2)*3 == 9 && 4 < 5\n#define P 1\n#else\n#define P 0\n#endif\n#if -1 < 1u\n#define Q 1\n#else\n#define Q 0\n#endif\n#if 0\n#error no\n#if 1\n#error still no\n#endif\n#elif 0\n#define S 0\n#else\n#define S 1\n#endif\nint x = P+Q+S;\n");
    assert!(text.contains("int x = 1+0+1;"), "{text}");
    assert!(!text.contains("error"), "{text}");
    // Unary ! in a preprocessor expression (C11 6.10.1, 6.5.3.3p5).
    let text = pp(
            "#if !0\nint t;\n#else\nint f;\n#endif\n#if !1\nint u;\n#else\nint v;\n#endif\n#if !!0\nint w;\n#else\nint x;\n#endif\n#if !!!0\nint y;\n#else\nint z;\n#endif\n",
        );
    assert!(text.contains("int t;"), "{text}");
    assert!(!text.contains("int f;"), "{text}");
    assert!(text.contains("int v;"), "{text}");
    assert!(!text.contains("int u;"), "{text}");
    assert!(text.contains("int x;"), "{text}");
    assert!(!text.contains("int w;"), "{text}");
    assert!(text.contains("int y;"), "{text}");
    assert!(!text.contains("int z;"), "{text}");
}

#[test]
fn defined_operator_and_undefined_identifiers() {
    let text = pp("#define FOO 3\n#if defined FOO && defined(FOO) && !defined(BAR)\nint a = FOO;\n#endif\n#if unknown\nint b;\n#else\nint c;\n#endif\n");
    assert!(text.contains("int a = 3;"), "{text}");
    assert!(text.contains("int c;"), "{text}");
    assert!(!text.contains("int b;"), "{text}");
}

#[test]
fn line_and_file_macros_follow_the_invocation() {
    let text = pp("int a = __LINE__;\nint b = \\\n__LINE__;\n#define L __LINE__\nint c = L;\n");
    assert!(text.contains("int a = 1;"), "{text}");
    assert!(text.contains("int b = 3;"), "{text}");
    assert!(text.contains("int c = 5;"), "{text}");
    let file = pp("__FILE__\n");
    assert!(file.contains("\"t.c\""), "{file}");
}

#[test]
fn reports_directive_errors() {
    assert!(pp_err("#error boom\n").contains("t.c:1:1: error: #error boom"));
    assert!(pp_err("#if 1\n").contains("unterminated conditional"));
    assert!(pp_err("#else\n").contains("#else without #if"));
    assert!(pp_err("#foo\n").contains("unknown preprocessing directive"));
    assert!(pp_err("#define F(a, b) a\nF(1)\n").contains("requires 2 argument"));
    assert!(pp_err("#define A 1\n#define A 2\n").contains("redefined"));
    assert!(pp("#define A 1\n#define A 1\nint x;\n").contains("int x;"));
    assert!(pp_err("#if 1/0\n#endif\n").contains("division by zero"));
}

#[test]
fn include_guard_and_error_in_header() {
    let dir = std::env::temp_dir().join(format!("cc-pp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(
        dir.join("guard.h"),
        "#ifndef GUARD_H\n#define GUARD_H\nint value;\n#endif\n",
    )
    .unwrap();
    std::fs::write(dir.join("sub").join("bad.h"), "#error from header\n").unwrap();
    let main = dir.join("main.c");
    let source = "#include \"guard.h\"\n#include \"guard.h\"\n#include \"sub/bad.h\"\n";
    let err = preprocess(source, main.to_str().unwrap(), &PpOptions::default())
        .unwrap_err()
        .to_string();
    assert!(err.contains("bad.h"), "{err}");
    assert!(err.contains("#error from header"), "{err}");
    let ok = "#include \"guard.h\"\n#include \"guard.h\"\n";
    let text = preprocess(ok, main.to_str().unwrap(), &PpOptions::default()).unwrap();
    assert_eq!(text.matches("int value;").count(), 1, "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn strict_ansi_is_defined_only_for_strict_iso_c() {
    // GCC manual, "Common Predefined Macros": `__STRICT_ANSI__` is defined
    // for -std=c11 and not for -std=gnu11; glibc hides `fileno` and the rest
    // of POSIX behind it.
    let source = "#ifdef __STRICT_ANSI__\nstrict\n#else\ngnu\n#endif\n";
    let strict = preprocess(source, "t.c", &PpOptions::default()).unwrap();
    assert!(
        strict.contains("strict") && !strict.contains("gnu"),
        "{strict}"
    );
    let gnu = PpOptions {
        gnu: true,
        ..PpOptions::default()
    };
    let text = preprocess(source, "t.c", &gnu).unwrap();
    assert!(text.contains("gnu") && !text.contains("strict"), "{text}");
    // Through the whole front end the dialect sets it.
    let probe = "#ifdef __STRICT_ANSI__\n#error strict\n#endif\nint x;\n";
    assert!(
        crate::compile_dialect(probe, "t.c", &PpOptions::default(), crate::Dialect::Gnu11).is_ok()
    );
    assert!(
        crate::compile_dialect(probe, "t.c", &PpOptions::default(), crate::Dialect::C11).is_err()
    );
}

#[test]
fn compiler_headers_provide_stdalign_and_max_align_t() {
    let options = PpOptions {
        system_dirs: vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("include")],
        ..PpOptions::default()
    };
    let source = "#include <stdalign.h>\n#include <stddef.h>\n\
            int a = alignof(max_align_t) + __alignas_is_defined + __alignof_is_defined;\n";
    let text = preprocess(source, "t.c", &options).unwrap();
    assert!(text.contains("} max_align_t;"), "{text}");
    assert!(
        squash(&text).contains("int a = _Alignof(max_align_t) + 1 + 1;"),
        "{text}"
    );
}

#[test]
fn command_line_defines_and_angle_include() {
    let dir = std::env::temp_dir().join(format!("cc-pp-inc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("n.h"), "int from_header = NAME;\n").unwrap();
    let mut options = PpOptions {
        system_dirs: vec![dir.clone()],
        ..PpOptions::default()
    };
    options.definitions.push(CliMacro::Define {
        name: "NAME".to_string(),
        replacement: Some("5".to_string()),
    });
    let text = preprocess("#include <n.h>\n", "t.c", &options).unwrap();
    assert!(text.contains("int from_header = 5;"), "{text}");
    options
        .definitions
        .push(CliMacro::Undef("NAME".to_string()));
    let err = preprocess("#include <n.h>\n", "t.c", &options).unwrap();
    assert!(err.contains("int from_header = NAME;"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn has_include_and_feature_queries() {
    let dir = std::env::temp_dir().join(format!("cc-pp-has-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("a")).unwrap();
    std::fs::create_dir_all(dir.join("b")).unwrap();
    std::fs::write(
        dir.join("a/n.h"),
        "#if __has_include_next(<n.h>)\nint next;\n#endif\n",
    )
    .unwrap();
    std::fs::write(dir.join("b/n.h"), "").unwrap();
    std::fs::write(
        dir.join("a/only.h"),
        "#if !__has_include_next(<only.h>)\nint last;\n#endif\n",
    )
    .unwrap();
    let options = PpOptions {
        system_dirs: vec![dir.join("a"), dir.join("b")],
        ..PpOptions::default()
    };
    let source = "#if __has_include(<n.h>) && !__has_include(<missing.h>)\nint found;\n#endif\n\
            #define HAS(h) __has_include(h)\n#if HAS(<n.h>)\nint via_macro;\n#endif\n\
            #define NAME <n.h>\n#if __has_include(NAME)\nint named;\n#endif\n\
            #if defined(__has_include) && defined __has_feature && !__has_feature(blocks) \\\n\
              && !__has_attribute(availability) && __has_builtin(__builtin_expect) \\\n\
              && __has_builtin(__builtin_bswap16) && __has_builtin(__builtin_memcpy) \\\n\
              && __has_builtin(__builtin_nanf) && __has_builtin(__builtin_huge_val) \\\n\
              && !__has_builtin(__builtin_no_such_thing) && !__has_builtin(memcpy) \\\n\
              && __has_builtin(__builtin_object_size) && __has_builtin(__builtin___vsnprintf_chk) \\\n\
              && !__has_builtin(__builtin___no_such_chk) \\\n\
              && !__has_extension(x) && !__building_module(m)\nint plain_c;\n#endif\n\
            #include <n.h>\n#include <only.h>\n";
    let text = preprocess(source, "t.c", &options).unwrap();
    for want in [
        "int found;",
        "int via_macro;",
        "int plain_c;",
        "int next;",
        "int last;",
    ] {
        assert!(text.contains(want), "{want}: {text}");
    }
    // A program's own definition of a query name wins.
    let text = preprocess(
        "#define __has_feature(x) 1\n#if __has_feature(anything)\nint own;\n#endif\n",
        "t.c",
        &options,
    )
    .unwrap();
    assert!(text.contains("int own;"), "{text}");
    let err = preprocess("#if __has_include(<n.h>\n#endif\n", "t.c", &options).unwrap_err();
    assert!(err.to_string().contains("missing ')'"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn named_variadic_parameter_only_in_system_headers() {
    let dir = std::env::temp_dir().join(format!("cc-pp-namedva-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let header = "#define GROUP(TAG, MEMBERS...) struct TAG { MEMBERS }\n\
                      #define SHOW(fmt, args...) show(fmt, ## args)\n";
    std::fs::write(dir.join("sys.h"), header).unwrap();
    // A header reached with `#include "..."` from a system header in a
    // subdirectory is a system header too.
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("sub/outer.h"), "#include \"inner.h\"\n").unwrap();
    std::fs::write(dir.join("sub/inner.h"), "#define INNER(a...) inner(a)\n").unwrap();
    std::fs::write(dir.join("user.h"), header).unwrap();
    let options = PpOptions {
        system_dirs: vec![dir.clone()],
        ..PpOptions::default()
    };
    let text = preprocess(
        "#include <sys.h>\n#include <sub/outer.h>\nGROUP(g, int a; int b;);\n\
             SHOW(\"x\");\nSHOW(\"y\", 1, 2);\nINNER(3, 4);\n",
        "t.c",
        &options,
    )
    .unwrap();
    let text = squash(&text);
    assert!(text.contains("struct g { int a; int b; };"), "{text}");
    assert!(text.contains("show(\"x\");"), "{text}");
    assert!(text.contains("show(\"y\", 1, 2);"), "{text}");
    assert!(text.contains("inner(3, 4);"), "{text}");
    // In a header found through -I it is still an error.
    let options = PpOptions {
        include_dirs: vec![dir.clone()],
        system_dirs: Vec::new(),
        ..PpOptions::default()
    };
    let err = preprocess("#include \"user.h\"\n", "t.c", &options).unwrap_err();
    assert!(err.to_string().contains("macro parameter list"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn predefined_stdc_and_no_recursion_on_line() {
    let text = pp("#if __STDC__ == 1 && __STDC_VERSION__ == 201112L && __x86_64__ && __linux__ && __LP64__\nint ok;\n#endif\n#define ID(x) x\nID(ID)(1)\n");
    assert!(text.contains("int ok;"), "{text}");
    assert!(squash(&text).contains("ID(1)"), "{text}");
}

#[test]
fn review_fixes_match_clang() {
    let glued = pp("#define NEG -1\n1-NEG\n");
    assert!(glued.contains("1- -1"), "{glued}");
    assert!(!glued.contains("1--1"), "{glued}");

    let empty = pp_err("#if\n#endif\n");
    assert!(empty.contains("t.c:1:"), "{empty}");
    assert!(empty.contains("expected an expression"), "{empty}");

    let after_else = pp_err("#if 0\n#else\n#elif 1\n#endif\n");
    assert!(after_else.contains("#elif after #else"), "{after_else}");
    let nested = pp_err("#if 0\n#if 0\n#else\n#else\n#endif\n#endif\n");
    assert!(nested.contains("#else after #else"), "{nested}");

    let hex = pp("#if -1 < 0xFFFFFFFF\nA\n#else\nB\n#endif\n#if -1 < 0x8000000000000000\nG\n#else\nH\n#endif\n#if (1 ? -1 : (1 << 0u)) < 0\nI\n#else\nJ\n#endif\n");
    assert!(hex.contains('A') && !hex.contains('B'), "{hex}");
    assert!(hex.contains('H') && !hex.contains('G'), "{hex}");
    assert!(hex.contains('I') && !hex.contains('J'), "{hex}");

    let dir = std::env::temp_dir().join(format!("cc-pp-nonl-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("nonl.h"), "#define X 1").unwrap();
    let main = dir.join("main.c");
    let text = preprocess(
        "#include \"nonl.h\"\nint y = X;\n",
        main.to_str().unwrap(),
        &PpOptions::default(),
    )
    .unwrap();
    assert!(text.contains("int y = 1;"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn prefixed_character_constants_in_if() {
    // C11 6.10.1p4: the value and type of `L'\400'` and `U'...'` are those
    // of the constant in program text.
    let text = pp("#if L'\\400' == 256 && u'\\xffff' == 65535 && U'\\xffffffff' > 0\nint ok;\n#endif\n#if L'Ä' == 0xC4 && U'\\U0001F600' == 0x1F600\nint wide;\n#endif\n#if '\\377' < 0\nint neg;\n#endif\n");
    assert_eq!(squash(&text), "int ok; int wide; int neg;");
    assert!(pp_err("#if u'\\x10000'\n#endif\n").contains("out of range"));
    // The character `u` is not an unsigned suffix: `L'u'` is a signed int.
    let text = pp("#if L'u' - 118 < 0 && L'\\u00c4' - 200 < 0\nint signed_wide;\n#endif\n");
    assert_eq!(squash(&text), "int signed_wide;");
    assert!(pp_err("#if 'é'\n#endif\n").contains("character too large"));
}

#[test]
fn utf8_literals_pass_through() {
    let text = pp("const char *s = \"Ä€\"; /* é */ int c = L'é';\n");
    assert_eq!(squash(&text), "const char *s = \"Ä€\"; int c = L'é';");
    // C11 6.4.2.1 and Annex D: `café` is an identifier, and the same one
    // as `caf\u00e9` (stored as UTF-8).
    assert_eq!(squash(&pp("int café;\n")), "int café;");
}

#[test]
fn universal_character_names_in_identifiers() {
    // A UCN and the UTF-8 character name one identifier, for macros too.
    let text = pp("#define caf\\u00e9 1\nint x = café + caf\\U000000E9;\n#ifdef café\nint defined_too;\n#endif\n");
    assert_eq!(squash(&text), "int x = 1 + 1; int defined_too;");
    // A line splice inside a UCN goes in phase 2, before tokens are formed.
    assert_eq!(squash(&pp("int a\\u00\\\ne9;\n")), "int aé;");
    // Pasting and stringizing: the pasted name is the same identifier, and
    // `#` gives a string with the same characters (UTF-8 in the literal).
    let text = pp("#define P(a, b) a ## b\n#define S(x) #x\n#define xé 7\nint y = P(x, \\u00e9); const char *s = S(\\u00e9t\\u00e9);\n");
    assert_eq!(squash(&text), "int y = 7; const char *s = \"été\";");
    // Annex D.2: a combining mark may follow, but not start, an identifier.
    assert_eq!(squash(&pp("int x\\u0301;\n")), "int x\u{301};");
    assert!(pp_err("int \\u0301x;\n").contains("not allowed at the start of an identifier"));
    assert!(pp_err("int \u{301}x;\n").contains("not allowed at the start of an identifier"));
    // 6.4.3p2: no basic characters, controls or surrogates; nothing above
    // U+10FFFF. Annex D.1 also leaves out `$`, `@`, `` ` `` and U+00A0.
    for (source, want) in [
        (
            "int a\\u0041;\n",
            "cannot be written as a universal character name",
        ),
        (
            "int a\\u0009;\n",
            "cannot be written as a universal character name",
        ),
        ("int a\\uD800;\n", "not a valid universal character name"),
        (
            "int a\\U00110000;\n",
            "not a valid universal character name",
        ),
        ("int a\\u0024;\n", "not allowed in an identifier"),
        ("int a\\u00A0;\n", "not allowed in an identifier"),
        ("int a\\u00D7;\n", "not allowed in an identifier"),
        ("int a\u{D7};\n", "not allowed in an identifier"),
        ("int x = 1 \u{D7} 2;\n", "unexpected character U+00D7"),
    ] {
        assert!(
            pp_err(source).contains(want),
            "{source:?}: {}",
            pp_err(source)
        );
    }
    // A backslash not followed by a complete UCN is an ordinary token.
    assert!(squash(&pp("#define S(x) #x\nS(a\\u12)\n")).contains("\"a\\u12\""));
    // `-D` names may be extended identifiers too.
    let mut options = PpOptions::default();
    options.system_dirs.clear();
    options.definitions.push(CliMacro::Define {
        name: "café".to_string(),
        replacement: Some("3".to_string()),
    });
    let text = preprocess("int v = caf\\u00e9;\n", "t.c", &options).unwrap();
    assert_eq!(squash(&text), "int v = 3;");
}

#[test]
fn conditional_feature_macros() {
    let text = pp("#if !defined(__STDC_NO_ATOMICS__) && __STDC_NO_COMPLEX__ && !defined(__STDC_NO_THREADS__) && !defined(__STDC_NO_VLA__) && __STDC_UTF_16__ && __STDC_UTF_32__\nint ok;\n#endif\n");
    assert_eq!(squash(&text), "int ok;");
}

#[test]
fn line_directive_takes_a_decimal_digit_sequence() {
    // C11 6.10.4p3: a digit sequence, read as decimal even with a leading 0.
    assert_eq!(squash(&pp("#line 010\n__LINE__\n")), "10");
    assert_eq!(squash(&pp("#line 2147483647\n__LINE__\n")), "2147483647");
    for bad in ["0x10", "10u", "5L", "'a'", "1e3", "07.5"] {
        let err = pp_err(&format!("#line {bad}\nint x;\n"));
        assert!(err.contains("simple digit sequence"), "{bad}: {err}");
    }
    assert!(pp_err("#line 0\n").contains("out of range"));
    assert!(pp_err("#line 2147483648\n").contains("out of range"));
    assert!(pp_err("#line 99999999999999999999999\n").contains("out of range"));
    // A macro may expand to the digit sequence (6.10.4p5).
    assert_eq!(squash(&pp("#define N 42\n#line N\n__LINE__\n")), "42");
}

#[test]
fn unsigned_character_constants_are_uintmax_in_if() {
    // C11 6.10.1p4: `char16_t` and `char32_t` are unsigned, so they act as
    // `uintmax_t`; `L'a'` (`wchar_t` is `int`) and `'a'` stay signed.
    assert_eq!(
        squash(&pp("#if u'a' - 98 < 0\nbad\n#else\ngood\n#endif\n")),
        "good"
    );
    assert_eq!(
        squash(&pp("#if U'a' - 98 < 0\nbad\n#else\ngood\n#endif\n")),
        "good"
    );
    assert_eq!(
        squash(&pp("#if U'\\xffffffff' > -1\nbad\n#else\ngood\n#endif\n")),
        "good"
    );
    assert_eq!(
        squash(&pp("#if L'a' - 98 < 0\ngood\n#else\nbad\n#endif\n")),
        "good"
    );
    assert_eq!(
        squash(&pp("#if 'a' - 98 < 0\ngood\n#else\nbad\n#endif\n")),
        "good"
    );
    // An integer constant still picks its type by value: 0xFFFFFFFF fits.
    assert_eq!(
        squash(&pp(
            "#if 0xFFFFFFFF - 0x100000000 < 0\ngood\n#else\nbad\n#endif\n"
        )),
        "good"
    );
}

#[test]
fn hashhash_from_an_argument_is_not_the_paste_operator() {
    // C11 6.10.3.3p2: `##` is the operator only in the replacement list.
    let src = "#define S(x) #x\n#define G(x) S(x)\nG(a ## b)\n\
               #define C(p, q) p ## q\n#define X(m) S(m)\nX(C(x ## y, z))\n\
               #define ID(x) x\nID(1 ## 2)\n";
    assert_eq!(squash(&pp(src)), "\"a ## b\" \"x ## yz\" 1 ## 2");
    // Pasting itself still works, including a chain, and the `##` that
    // pasting `#` and `#` makes is an ordinary token (C11 6.10.3.3p4).
    let src = "#define P(a, b, c) a ## b ## c\nP(x, y, z)\n\
               #define OBJ x ## y\nOBJ\n\
               #define hash_hash # ## #\n#define mkstr(a) # a\n\
               #define in_between(a) mkstr(a)\n\
               #define join(c, d) in_between(c hash_hash d)\njoin(x, y)\n";
    assert_eq!(squash(&pp(src)), "xyz xy \"x ## y\"");
}

#[test]
fn only_one_compiler_header_tree_is_searched() {
    // Two copies of the compiler's headers at different paths: only the
    // first is used, so `#include_next` in one of them reaches the C
    // library rather than its twin.
    let base = std::env::temp_dir().join(format!("cc-two-trees-{}", std::process::id()));
    let (first, second) = (base.join("a/include"), base.join("b/include"));
    let partial = base.join("project/include");
    for dir in [&first, &second] {
        std::fs::create_dir_all(dir).unwrap();
        for name in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/include")).unwrap() {
            std::fs::write(dir.join(name.unwrap().file_name()), "").unwrap();
        }
    }
    // A project's include/ with only a stddef.h is not the compiler's.
    std::fs::create_dir_all(&partial).unwrap();
    std::fs::write(partial.join("stddef.h"), "").unwrap();
    let mut dirs = Vec::new();
    predefined::push_compiler(&mut dirs, base.join("missing"));
    predefined::push_compiler(&mut dirs, partial);
    predefined::push_compiler(&mut dirs, first.clone());
    predefined::push_compiler(&mut dirs, second);
    predefined::push_compiler(&mut dirs, first.clone());
    assert_eq!(dirs, vec![first]);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn command_line_function_like_macro() {
    // GCC manual, "Preprocessor Options": -D 'name(args...)=definition'.
    let mut options = PpOptions::default();
    options.definitions.push(CliMacro::Define {
        name: "TWICE(x, y)".to_string(),
        replacement: Some("((x) + (y))".to_string()),
    });
    let text = preprocess("int n = TWICE(1, 2);\n", "t.c", &options).unwrap();
    assert!(text.contains("int n = ((1) + (2));"), "{text}");
    options.definitions[0] = CliMacro::Define {
        name: "BAD(1)".to_string(),
        replacement: None,
    };
    assert!(preprocess("int n;\n", "t.c", &options).is_err());
    for bad in ["F(x,,y)", "F(x", "F(x)y", "F((x))", "F(x,)"] {
        options.definitions[0] = CliMacro::Define {
            name: bad.to_string(),
            replacement: Some("x".to_string()),
        };
        assert!(preprocess("int n;\n", "t.c", &options).is_err(), "{bad}");
    }
    options.definitions[0] = CliMacro::Define {
        name: "ONE()".to_string(),
        replacement: Some("1".to_string()),
    };
    let text = preprocess("int n = ONE();\n", "t.c", &options).unwrap();
    assert!(text.contains("int n = 1;"), "{text}");
}

#[test]
fn pragma_operator_is_applied_and_removed() {
    // C11 6.10.9: `_Pragma("...")` acts as a #pragma line; the pragmas it
    // gives here are ignored, as unknown #pragma lines are.
    let options = PpOptions::default();
    let text = preprocess(
        "#define P(x) _Pragma(#x)\nP(warning(push)) int a; _Pragma(\"pack(1)\") int b;\n",
        "t.c",
        &options,
    )
    .unwrap();
    assert!(
        !text.contains("_Pragma") && !text.contains("warning"),
        "{text}"
    );
    assert!(text.contains("int a;") && text.contains("int b;"), "{text}");
    assert!(preprocess("_Pragma(x) int a;\n", "t.c", &options).is_err());
}

#[test]
fn if_expression_errors_name_the_problem() {
    // Coverage found every one of these #if diagnostics untested.
    assert!(pp_err("#if 1.5\n#endif\n").contains("floating constant in preprocessor expression"));
    assert!(pp_err("#if 1u2\n#endif\n").contains("invalid integer constant"));
    assert!(pp_err("#if 0x\n#endif\n").contains("invalid hexadecimal constant"));
    assert!(pp_err("#if 1 2\n#endif\n").contains("extra tokens in preprocessor expression"));
    assert!(pp_err("#if 1 ? 2\n#endif\n").contains("expected ':'"));
    assert!(pp_err("#if (1\n#endif\n").contains("expected ')'"));
    assert!(pp_err("#if 1 +\n#endif\n").contains("expected an expression"));
    assert!(pp_err("#if\n#endif\n").contains("expected an expression"));
    assert!(pp_err("#if = 1\n#endif\n").contains("invalid token '='"));
    assert!(pp_err("#if 1 = 1\n#endif\n").contains("extra tokens"));
    assert!(pp_err("#if 1 << -1\n#endif\n").contains("negative shift count"));
    assert!(pp_err("#if 1 % 0\n#endif\n").contains("division by zero"));
    // The comma operator is not allowed in a constant expression (C11 6.6p3).
    assert!(pp_err("#if (1, 2)\n#endif\n").contains("expected ')'"));
    // Unsigned arithmetic: one unsigned operand converts the other.
    let text = pp(
        "#if -1 > 0u && ~0u / 2 == 0x7fffffffffffffff && +1 && (6u ^ 3u | 8u) == 13\nyes\n#endif\n",
    );
    assert!(text.contains("yes"), "{text}");
}

#[test]
fn trigraphs_in_strict_c11_only_unless_requested() {
    // C11 5.2.1.1: phase 1 replaces the nine trigraphs, before line splicing,
    // so `??/` at the end of a line splices it.
    let source = "??=define S(x) ??=x\nint a??(2??) = ??< 1, ~0 ??! 2 ??>; char *s = S(a??'b);\n\
                  int sp = 1 + ??/\n2;\nchar *q = \"what???\"; char *r = \"ok??!\";\n";
    let strict = pp(source);
    assert_eq!(
        squash(&strict),
        "int a[2] = { 1, ~0 | 2 }; char *s = \"a^b\"; int sp = 1 + 2; char *q = \"what???\"; char *r = \"ok|\";"
    );
    // GNU mode ignores them, as GCC does by default.
    let gnu = PpOptions {
        gnu: true,
        system_dirs: Vec::new(),
        ..PpOptions::default()
    };
    let text = preprocess("int a = 1 ??! 2;\nchar *q = \"what??!\";\n", "t.c", &gnu).unwrap();
    assert!(
        text.contains("??!") && text.contains("\"what??!\""),
        "{text}"
    );
    // `-trigraphs` turns them on in GNU mode.
    let requested = PpOptions {
        trigraphs: true,
        ..gnu
    };
    let text = preprocess("int a = 1 ??! 2;\n", "t.c", &requested).unwrap();
    assert_eq!(squash(&text), "int a = 1 | 2;");
}

#[test]
fn trigraphs_in_included_headers() {
    let dir = std::env::temp_dir().join(format!("cc-pp-tri-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("t.h"), "??=define FROM_HEADER ??-1\n").unwrap();
    let options = PpOptions {
        system_dirs: vec![dir.clone()],
        ..PpOptions::default()
    };
    let text = preprocess("#include <t.h>\nint x = FROM_HEADER;\n", "t.c", &options).unwrap();
    assert_eq!(squash(&text), "int x = ~1;");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn date_and_time_are_predefined_string_literals() {
    // C11 6.10.8.1: "Mmm dd yyyy" and "hh:mm:ss". (The SOURCE_DATE_EPOCH
    // override is tested on the binary, where the environment is its own.)
    let text = pp("__DATE__ __TIME__\n#if defined(__DATE__) && defined __TIME__\nboth\n#endif\n");
    let mut parts = text.split('"');
    let _ = parts.next();
    let date = parts.next().unwrap();
    let _ = parts.next();
    let time = parts.next().unwrap();
    assert_eq!(date.len(), 11, "{date}");
    let months = "JanFebMarAprMayJunJulAugSepOctNovDec";
    assert!(
        months.contains(&date[..3]) && date.as_bytes()[3] == b' ',
        "{date}"
    );
    assert!(date[4..6].trim_start().parse::<u32>().is_ok(), "{date}");
    assert!(date[7..].parse::<u32>().is_ok(), "{date}");
    assert_eq!(time.len(), 8, "{time}");
    assert_eq!(
        (time.as_bytes()[2], time.as_bytes()[5]),
        (b':', b':'),
        "{time}"
    );
    assert!(text.contains("both"), "{text}");
}

#[test]
fn pragma_operator_destringizes_and_applies_once() {
    use super::expand::destringize;
    assert_eq!(destringize("\"once\""), "once");
    assert_eq!(destringize("L\"once\""), "once");
    assert_eq!(destringize(r#""message(\"a\\b\")""#), r#"message("a\b")"#);
    assert_eq!(destringize(r#""keep \n""#), r#"keep \n"#);
    let dir = std::env::temp_dir().join(format!("cc-pp-pragma-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("o.h"), "_Pragma(\"once\")\nint once_only;\n").unwrap();
    std::fs::write(
        dir.join("m.h"),
        "#define ONCE _Pragma(L\"once\")\nONCE\nint via_macro;\n",
    )
    .unwrap();
    std::fs::write(dir.join("bad.h"), "_Pragma(\"once extra\")\n").unwrap();
    let options = PpOptions {
        system_dirs: vec![dir.clone()],
        ..PpOptions::default()
    };
    let text = preprocess(
        "#include <o.h>\n#include <o.h>\n#include <m.h>\n#include <m.h>\n",
        "t.c",
        &options,
    )
    .unwrap();
    assert_eq!(text.matches("once_only").count(), 1, "{text}");
    assert_eq!(text.matches("via_macro").count(), 1, "{text}");
    let err = preprocess("#include <bad.h>\n", "t.c", &options)
        .unwrap_err()
        .to_string();
    assert!(err.contains("extra tokens after #pragma once"), "{err}");
    // Unknown and STDC pragmas are ignored, as on a #pragma line.
    let text = preprocess(
        "_Pragma(\"STDC FP_CONTRACT ON\") _Pragma(\"GCC diagnostic push\") int z;\n",
        "t.c",
        &options,
    )
    .unwrap();
    assert_eq!(squash(&text), "int z;");
    // The operand is macro replaced before it must be a string literal,
    // directly and from inside another macro.
    std::fs::write(
        dir.join("t.h"),
        "#define TEXT \"once\"\n#define MARK _Pragma(TEXT)\nMARK\nint via_text;\n",
    )
    .unwrap();
    let text = preprocess("#include <t.h>\n#include <t.h>\n", "t.c", &options).unwrap();
    assert_eq!(text.matches("via_text").count(), 1, "{text}");
    assert!(preprocess("#define TWO \"a\" \"b\"\n_Pragma(TWO)\n", "t.c", &options).is_err());
    assert!(preprocess("#define N 1\n_Pragma(N)\n", "t.c", &options).is_err());
    assert!(preprocess("_Pragma\n", "t.c", &options).is_err());
    assert!(preprocess("_Pragma(u8\"once\")\n", "t.c", &options).is_err());
    assert!(preprocess("_Pragma(\"once\"\n", "t.c", &options).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The C11 6.4.6 punctuators, digraphs included, in the order the
/// punctuator matcher used to try them (a linear scan, kept here as the
/// reference the faster `match` is compared against).
const REFERENCE_PUNCTUATORS: &[&str] = &[
    "%:%:", "<<=", ">>=", "...", "<<", ">>", "<=", ">=", "==", "!=", "&&", "||", "++", "--", "+=",
    "-=", "*=", "/=", "%=", "&=", "^=", "|=", "->", "##", "<:", ":>", "<%", "%>", "%:", "#", "+",
    "-", "*", "/", "%", "=", "<", ">", "!", "&", "|", "^", "~", "(", ")", "{", "}", "[", "]", ";",
    ",", "?", ":", ".",
];

fn reference_punctuator(ahead: &[u8]) -> Option<&'static str> {
    let ahead = &ahead[..ahead.len().min(4)];
    REFERENCE_PUNCTUATORS
        .iter()
        .find(|punct| ahead.starts_with(punct.as_bytes()))
        .copied()
}

#[test]
fn every_c11_punctuator_is_matched_alone_and_before_other_text() {
    // 6.4.6p1 lists 47 punctuators; with the six digraphs counted
    // separately and `%:%:` that is the 54 in the table.
    assert_eq!(REFERENCE_PUNCTUATORS.len(), 54);
    for punct in REFERENCE_PUNCTUATORS {
        let bytes = punct.as_bytes();
        assert_eq!(scan::punctuator_prefix(bytes), Some(*punct), "{punct}");
        // Text that cannot extend any punctuator does not change the match.
        for tail in ["a", " ", "9", "\"", "'", "_"] {
            let text = format!("{punct}{tail}");
            assert_eq!(
                scan::punctuator_prefix(text.as_bytes()),
                Some(*punct),
                "{text}"
            );
        }
    }
}

#[test]
fn punctuator_matching_takes_the_longest_match() {
    let cases: &[(&str, Option<&str>)] = &[
        ("...", Some("...")),
        ("....", Some("...")),
        ("..", Some(".")),
        (".a", Some(".")),
        ("<<=", Some("<<=")),
        ("<<", Some("<<")),
        ("<<<", Some("<<")),
        ("<=", Some("<=")),
        ("<:", Some("<:")),
        ("<%", Some("<%")),
        ("<", Some("<")),
        (">>=", Some(">>=")),
        (">>>", Some(">>")),
        (":>", Some(":>")),
        (":", Some(":")),
        ("%:%:", Some("%:%:")),
        ("%:%", Some("%:")),
        ("%:%>", Some("%:")),
        ("%:%:%:", Some("%:%:")),
        ("%:", Some("%:")),
        ("%>", Some("%>")),
        ("%=", Some("%=")),
        ("%", Some("%")),
        ("##", Some("##")),
        ("###", Some("##")),
        ("#", Some("#")),
        ("->", Some("->")),
        ("-->", Some("--")),
        ("&&&", Some("&&")),
        ("||=", Some("||")),
        ("", None),
        ("a", None),
        ("$", None),
        ("@", None),
        ("`", None),
        ("\\", None),
        ("\"", None),
        ("'", None),
        ("0", None),
    ];
    for (text, want) in cases {
        assert_eq!(scan::punctuator_prefix(text.as_bytes()), *want, "{text:?}");
    }
}

#[test]
fn punctuator_matching_agrees_with_the_linear_scan_on_every_short_string() {
    // Every string of up to four characters over the punctuator
    // characters plus a letter and a space: 0.7 million cases.
    let alphabet: Vec<u8> = b"%:<>=!&|+-*/^.#~(){}[];,?a ".to_vec();
    let mut buf = Vec::new();
    fn walk(buf: &mut Vec<u8>, alphabet: &[u8], depth: usize) {
        assert_eq!(
            scan::punctuator_prefix(buf),
            reference_punctuator(buf),
            "{:?}",
            String::from_utf8_lossy(buf)
        );
        if depth == 4 {
            return;
        }
        for &byte in alphabet {
            buf.push(byte);
            walk(buf, alphabet, depth + 1);
            buf.pop();
        }
    }
    walk(&mut buf, &alphabet, 0);
}

#[test]
fn punctuators_survive_the_preprocessor_with_digraphs_and_splices() {
    let text = pp("x <<= y >>= z ... w -> v ## u <: :> <% %> a %:%: b\n");
    // `%:%:` is the digraph for `##`; the preprocessor writes it that way.
    assert_eq!(
        squash(&text),
        "x <<= y >>= z ... w -> v ## u <: :> <% %> a ## b"
    );
    // A line splice inside a punctuator is removed before matching.
    assert_eq!(squash(&pp("a <\\\n<= b .\\\n.. c\n")), "a <<= b ... c");
    // Two dots are two tokens, and three dots are one.
    assert_eq!(squash(&pp("a .. b ... c\n")), "a . . b ... c");
}

#[test]
fn tokens_share_one_empty_hide_set_and_macro_hiding_still_works() {
    // Tokens that have not been through a macro share the empty set.
    let first = hide_empty();
    let second = hide_empty();
    assert!(Rc::ptr_eq(&first, &second));
    assert!(first.is_empty());
    // Painting builds a new set; the shared one stays empty.
    let painted = hide_with(&first, &Rc::from("M"));
    assert!(!Rc::ptr_eq(&painted, &first));
    assert!(painted.contains("M"));
    assert!(hide_empty().is_empty());
    // A self-referential macro is still not expanded again, and the hiding
    // of `f` through `g` stops where C11 6.10.3.4 says (these are the
    // results of the unchanged compiler).
    let text = pp("#define f(a) a*f\n#define g f\n#define M M + 1\nf(2)(9) M g(3)\n");
    assert_eq!(squash(&text), "2*f(9) M + 1 3*f");
}

/// The preprocessed lines of `source`, each with its spacing squashed.
fn pp_lines(source: &str) -> Vec<String> {
    pp(source)
        .lines()
        .map(squash)
        .filter(|line| !line.is_empty())
        .collect()
}

#[test]
fn stringizing_keeps_each_spelling() {
    // C11 6.10.3.2: spacing collapses to one space, and `"` and `\` in
    // string and character literals are escaped. These are the results of
    // the compiler before spellings were shared.
    let lines = pp_lines(concat!(
        "#define S(x) #x\n",
        "#define XS(x) S(x)\n",
        "#define CAT(a, b) a ## b\n",
        "S( a   +  b )\n",
        "S(\"a\\n\" 'c' L\"w\" u8\"v\")\n",
        "S(<: :> <% %> %: %:%:)\n",
        "S(  leading   and trailing  )\n",
        "S(caf\\u00e9 x\\U0001F600 \u{e9})\n",
        "XS(CAT(a, b))\n",
        "XS(__LINE__)\n",
    ));
    assert_eq!(
        lines,
        [
            r#""a + b""#,
            r#""\"a\\n\" 'c' L\"w\" u8\"v\"""#,
            // The scanner stores `%:` and `%:%:` as `#` and `##`.
            r#""<: :> <% %> # ##""#,
            r#""leading and trailing""#,
            // UCNs in identifiers are kept as UTF-8.
            "\"caf\u{e9} x\u{1F600} \u{e9}\"",
            r#""ab""#,
            r#""10""#,
        ]
    );
}

#[test]
fn token_pasting_builds_new_spellings() {
    let lines = pp_lines(concat!(
        "#define CAT(a, b) a ## b\n",
        "#define XCAT(a, b) CAT(a, b)\n",
        "#define FOO 42\n",
        "#define caf\\u00e9 7\n",
        "CAT(a, b) CAT(1, 2) CAT(<, <=) CAT(%:, %:) CAT(L, \"s\") CAT(x, ) CAT(, y)\n",
        "CAT(F, OO) XCAT(F, OO) CAT(1e, +5) CAT(0x, 1p-3)\n",
        // A pasted identifier with a UCN names the same macro as the
        // UTF-8 spelling.
        "CAT(caf, \\u00e9) CAT(caf, \u{e9}) caf\u{e9} caf\\u00e9 caf\\u00E9\n",
    ));
    assert_eq!(
        lines,
        ["ab 12 <<= ## L\"s\" x y", "42 42 1e+ 5 0x1p-3", "7 7 7 7 7",]
    );
    let err = pp_err("#define CAT(a, b) a ## b\nCAT(., .)\n");
    assert!(
        err.contains("pasting '..' does not produce a valid preprocessing token"),
        "{err}"
    );
}

#[test]
fn macro_expansion_spellings_and_shared_tokens() {
    let lines = pp_lines(concat!(
        "#define V(fmt, ...) f(fmt, ## __VA_ARGS__)\n",
        "#define W(...) g(__VA_ARGS__) #__VA_ARGS__\n",
        "#define int int\n",
        "#define TWICE(x) x x\n",
        "V(1) V(1, 2, 3) W() W(a, b ,c)\n",
        "int x; int y; TWICE(TWICE(z))\n",
    ));
    assert_eq!(
        lines,
        [
            r#"f(1) f(1, 2, 3) g() "" g(a, b ,c) "a, b ,c""#,
            "int x; int y; z z z z",
        ]
    );
}

#[test]
fn scanned_spellings_are_shared_within_a_unit() {
    let _scope = Spellings::scope();
    let tokens = lex_source("int x; int y; \"s\" \"s\"\n", "t.c", 0).unwrap();
    let spelled = |text: &str| -> Vec<&PpToken> {
        tokens.iter().filter(|tok| &*tok.spelling == text).collect()
    };
    // Identifiers and punctuators share one allocation per spelling.
    let ints = spelled("int");
    assert_eq!(ints.len(), 2);
    assert!(Rc::ptr_eq(&ints[0].spelling, &ints[1].spelling));
    let semis = spelled(";");
    assert!(Rc::ptr_eq(&semis[0].spelling, &semis[1].spelling));
    // String literals are not looked up.
    let strings = spelled("\"s\"");
    assert_eq!(strings.len(), 2);
    assert!(!Rc::ptr_eq(&strings[0].spelling, &strings[1].spelling));
    drop(_scope);
    // The table is emptied when the unit ends; the tokens keep their text.
    assert!(!Rc::ptr_eq(&intern("int"), &ints[0].spelling));
    assert_eq!(&*ints[0].spelling, "int");
}

#[test]
fn parser_tokens_share_the_preprocessor_spelling() {
    let mut options = PpOptions::default();
    options.system_dirs.clear();
    let tokens = preprocess_tokens("int a <: 2 :>; int b;\n", "t.c", &options).unwrap();
    let spellings: Vec<&str> = tokens.iter().map(|tok| &*tok.spelling).collect();
    // Digraphs still reach the parser as the tokens they stand for.
    assert_eq!(
        spellings,
        ["int", "a", "[", "2", "]", ";", "int", "b", ";", ""]
    );
    assert_eq!(tokens[2].kind, TokenKind::LBrack);
    assert!(Rc::ptr_eq(&tokens[0].spelling, &tokens[6].spelling));
    assert!(Rc::ptr_eq(&tokens[5].spelling, &tokens[8].spelling));
    // Numbers and other tokens that go through the full lexer share it too,
    // and keep their value.
    let tokens = preprocess_tokens("42 + 42;\n", "t.c", &options).unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Int);
    assert_eq!(tokens[2].value, Some(42));
    assert!(Rc::ptr_eq(&tokens[0].spelling, &tokens[2].spelling));
}

#[test]
fn the_spelling_table_is_empty_after_preprocessing() {
    // `-E` output lexes token pairs again while it is written; those
    // spellings are dropped as well.
    pp("int v0;int v1;\n#define M(x) x x\nM(a)\n");
    assert_eq!(interned_count(), 0);
    let mut options = PpOptions::default();
    options.system_dirs.clear();
    preprocess_tokens("int a;\n", "t.c", &options).unwrap();
    assert_eq!(interned_count(), 0);
}
