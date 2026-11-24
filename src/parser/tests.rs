use super::*;
use crate::lexer::lex;

fn parsed(source: &str) -> String {
    let tokens = lex(source, "t.c").unwrap();
    dump(&parse(&tokens, "t.c").unwrap())
}

fn parse_err(source: &str) -> String {
    let tokens = match lex(source, "t.c") {
        Ok(tokens) => tokens,
        Err(err) => return err.to_string(),
    };
    parse(&tokens, "t.c").unwrap_err().to_string()
}

#[test]
fn return_constant_ast() {
    assert_eq!(
        parsed("int main(void) { return 42; }\n"),
        "\
func int main
  block
    return
      int 42
"
    );
}

#[test]
fn precedence_and_associativity() {
    assert_eq!(
        parsed("int main(void) { return 1 + 2 * 3; }\n"),
        "\
func int main
  block
    return
      +
        int 1
        *
          int 2
          int 3
"
    );
    assert_eq!(
        parsed("int main(void) { return 8 - 2 - 1; }\n"),
        "\
func int main
  block
    return
      -
        -
          int 8
          int 2
        int 1
"
    );
    assert_eq!(
        parsed("int main(void) { a = b = 1; }\n"),
        "\
func int main
  block
    expr
      assign
        var a
        assign
          var b
          int 1
"
    );
}

#[test]
fn dangling_else_binds_inward() {
    let source = "\
int main(void) {
    if (c)
        if (d)
            a;
        else
            b;
}
";
    assert_eq!(
        parsed(source),
        "\
func int main
  block
    if
      cond
        var c
      then
        if
          cond
            var d
          then
            expr
              var a
          else
            expr
              var b
"
    );
}

#[test]
fn static_extern_and_do_while_parse() {
    assert_eq!(
        parsed(
            "\
int main(void) {
    static int kept = 1;
    extern int shared;
    int other(int);
    do {
        kept = kept + shared;
    } while (kept < 3);
    return other(kept);
}
"
        ),
        "\
func int main
  block
    static decl int kept =
      int 1
    extern decl int shared
    proto int other
      param int
    do
      body
        block
          expr
            assign
              var kept
              +
                var kept
                var shared
      cond
        <
          var kept
          int 3
    return
      call other
        var kept
"
    );
}

#[test]
fn for_loop_call_and_empty_parameter_list() {
    assert_eq!(
        parsed(
            "\
int add(int a, int b) { return a + b; }
int main() { return add(19, 23); }
"
        ),
        "\
func int add
  param int a
  param int b
  block
    return
      +
        var a
        var b
func int main
  block
    return
      call add
        int 19
        int 23
"
    );
    assert_eq!(
        parsed(
            "\
int main(void) {
    int sum = 0;
    for (int i = 1; i <= 10; i = i + 1)
        sum = sum + i;
    return sum;
}
"
        ),
        "\
func int main
  block
    decl int sum =
      int 0
    for
      init
        decl int i =
          int 1
      cond
        <=
          var i
          int 10
      step
        assign
          var i
          +
            var i
            int 1
      body
        expr
          assign
            var sum
            +
              var sum
              var i
    return
      var sum
"
    );
}

#[test]
fn logical_precedence_and_unary() {
    assert_eq!(
        parsed("int main(void) { return 0 || 1 && 0; }\n"),
        "\
func int main
  block
    return
      ||
        int 0
        &&
          int 1
          int 0
"
    );
    assert_eq!(
        parsed("int main(void) { return -2 + !3; }\n"),
        "\
func int main
  block
    return
      +
        neg
          int 2
        not
          int 3
"
    );
}

#[test]
fn parentheses_do_not_hide_calls() {
    assert_eq!(
        parsed("int main(void) { return (add)(1, 2); }\n"),
        "\
func int main
  block
    return
      call add
        int 1
        int 2
"
    );
}

#[test]
fn pointer_and_array_declarators() {
    assert_eq!(
        parsed(
            "\
int *a[3];
int (*p)[3];
int **pp;
void *vp;
int f(int *a, int b[]);
int *g(int x) { return 0; }
"
        ),
        "\
global int *[3] a
global int (*)[3] p
global int ** pp
global void * vp
proto int f
  param int * a
  param int * b
func int * g
  param int x
  block
    return
      int 0
"
    );
    assert_eq!(
        parsed("int main(void) { return a[i] + *p + &x; }\n"),
        "\
func int main
  block
    return
      +
        +
          index
            var a
            var i
          deref
            var p
        addr
          var x
"
    );
    assert_eq!(
        parsed("int a[] = {1, 2, 3};\n"),
        "\
global int [] a =
  list
    int 1
    int 2
    int 3
"
    );
}

#[test]
fn syntax_errors_name_the_problem() {
    assert!(parse_err("int main(void) { return 1 }").contains("expected ';'"));
    assert!(parse_err("int main(void { return 0; }").contains("expected ')'"));
    assert!(parse_err("struct S { int x: 0; };").contains("zero width"));
    assert!(parse_err("int a[n];").contains("undeclared identifier"));
    assert!(parse_err("int n; int a[n];")
        .contains("variably modified type is not allowed at file scope"));
    assert!(parse_err("int a[0];").contains("array size must be positive"));
    // _Static_assert (C11 6.7.10) and _Alignof (6.5.3.4).
    assert!(parsed("_Static_assert(sizeof(int) == 4, \"ok\");\n").is_empty());
    assert!(parse_err("_Static_assert(sizeof(int) == 8, \"int is 8\");")
        .contains("static assertion failed: int is 8"));
    assert!(parse_err("_Static_assert(1);").contains("expected ','"));
    assert!(parse_err("int x; _Static_assert(x, \"a\");")
        .contains("not an integer constant expression"));
    assert!(parse_err("struct S { int a; _Static_assert(0, \"m\"); };")
        .contains("static assertion failed: m"));
    assert!(parse_err("int main(void) { _Static_assert(0, \"b\"); }")
        .contains("static assertion failed: b"));
    assert!(parsed("int a[_Alignof(double)];\n").contains("global int [8] a"));
    assert!(parse_err("int a = _Alignof(void);").contains("incomplete type"));
    assert!(parse_err("int a = _Alignof(1);").contains("parenthesized type name"));
    assert!(parse_err("struct Q; int a = _Alignof(struct Q);").contains("incomplete type"));
    assert!(parse_err("int a = _Alignof(int(void));").contains("function type"));
    assert!(parsed("_Alignas(8) int a;\n").contains("global int a"));
    assert!(parse_err("_Alignas(3) int a;").contains("not a valid alignment"));
    assert!(parse_err("_Alignas(2) int a;").contains("less than the alignment"));
    assert!(parse_err("typedef _Alignas(8) int T;").contains("not allowed on a typedef"));
    assert!(parse_err("_Alignas(8) int f(void);").contains("not allowed on a function"));
    assert!(parse_err("int f(_Alignas(8) int a);").contains("not allowed on a parameter"));
    assert!(
        parse_err("struct S { _Alignas(8) int x : 3; };").contains("not allowed on a bit-field")
    );
    assert!(parse_err("register _Alignas(16) int a;").contains("not allowed on a register"));
    assert!(parsed("_Thread_local int x;\n").contains("global int x"));
    assert!(parse_err("_Thread_local int f(void);").contains("not allowed on a function"));
    assert!(parse_err("int main(void) { _Thread_local int x; }")
        .contains("requires 'static' or 'extern'"));
    assert!(parse_err("_Thread_local typedef int T;").contains("multiple storage-class"));
    assert!(parse_err("struct S { _Thread_local int x; };")
        .contains("not allowed on a struct or union member"));
    // sizeof in integer constant expressions (C11 6.6p6, 6.5.3.4p1).
    assert!(parsed("int a[4]; int b[sizeof a / sizeof a[0]];\n").contains("global int [4] b"));
    assert!(parse_err("int x[sizeof x];").contains("undeclared identifier"));
    assert!(parse_err("int n; int a[sizeof n * n];")
        .contains("variably modified type is not allowed at file scope"));
    assert!(parse_err("int f(void); int a[sizeof f];").contains("to a function type"));
    assert!(parse_err("struct S { int b : 3; } s; int a[sizeof s.b];").contains("bit-field"));
    assert!(parse_err("enum { E = sizeof(struct Q) };").contains("incomplete type"));
    assert!(parse_err("int a[sizeof(int) - 5];").contains("array is too large"));
    assert!(parse_err("\"hi\";").contains("expected a type"));
    assert!(parsed(";\nint x;\n").contains("global int x"));
    assert!(parsed("__uint128_t x;\n").contains("global unsigned __int128 x"));
    assert!(parsed("unsigned __int128 y;\n").contains("global unsigned __int128 y"));
    assert!(parsed("__int128 z;\n").contains("global __int128 z"));
    assert!(parse_err("int __int128 x;").contains("invalid combination"));
    assert!(parsed(
            "_Static_assert(((__uint128_t)-1 >> 63) != 1, \"ok\");\n_Static_assert(((__uint128_t)1 << 64) >> 64 == 1, \"shift\");\n"
        )
        .is_empty());
    assert!(
        parse_err("_Static_assert(((__uint128_t)-1 >> 63) == 1, \"bad\");")
            .contains("static assertion failed: bad")
    );
    assert!(parse_err("struct S { unsigned __int128 f : 80; };")
        .contains("bit-field of type 'unsigned __int128' is not supported"));
    assert!(parse_err("struct S { __int128 f : 8; };")
        .contains("bit-field of type '__int128' is not supported"));
    assert!(
        parse_err("_Static_assert((((__int128)1 << 126) << 2) == 0, \"z\");").contains("overflows")
    );
    assert!(
        parse_err("_Static_assert(-(__int128)((__uint128_t)1 << 127) != 0, \"n\");")
            .contains("overflows")
    );
    assert!(parse_err("int main(void) { return a ? b; }").contains("expected ':'"));
    assert!(parsed("int add();\n").contains("add"));
    assert!(parse_err("int add(a, b);").contains("must be a definition"));
    assert!(parsed("void f(__builtin_va_list);\nvoid g(__builtin_va_list, int);\n").contains("g"));
    assert!(
        parse_err("int main(void) { int inner(void) { return 1; } }").contains("nested functions")
    );
    assert!(parse_err("int f(int, int) { return 0; }").contains("parameter name omitted"));
    assert!(parse_err("int f(void) = 1;").contains("cannot have an initializer"));
    assert!(parse_err("static extern int x;").contains("multiple storage-class"));
    assert!(parse_err("void g;").contains("void variables"));
    assert!(parse_err("int f(static int a);").contains("not supported on parameters"));
    assert!(parse_err("int main(void) { switch 1; }").contains("expected '('"));
    assert!(parse_err("int main(void) { goto *p; }").contains("computed goto"));
    assert!(
        parse_err("int main(void) { switch (1) { case 1: int x = 0; return x; } }")
            .contains("a declaration is not allowed as a statement")
    );
    assert!(parse_err("int main(void) { else; }").contains("'else' without a matching 'if'"));
    assert!(parsed("int (*fp)(void);\n").contains("int (*)(void)"));
    assert!(
        parse_err("int main(void) { static int foo(void) { return 1; } }")
            .contains("nested functions")
    );
    assert!(
        parse_err("int main(void) { for (static int i = 0; i < 1; i = i + 1) ; }")
            .contains("for-loop declaration cannot have a storage class")
    );
    assert!(
        parse_err("int main(void) { for (extern int i = 0; i < 1; i = i + 1) ; }")
            .contains("for-loop declaration cannot have a storage class")
    );
    assert!(
        parse_err("int main(void) { for (int f(void); ; ) return 0; }")
            .contains("function declaration is not allowed in a for loop")
    );
    assert!(parse_err("int main(void) { int f(a); }").contains("must be a definition"));
    assert!(parse_err("int main(void) { do { } while (0) return 0; }").contains("expected ';'"));
    assert!(parse_err("int main(void) { do { }; while (0); }").contains("expected 'while'"));
    assert!(parse_err("short long x;").contains("invalid combination of type specifiers"));
    assert!(parse_err("unsigned signed x;").contains("invalid combination of type specifiers"));
    assert!(parse_err("long long long x;").contains("invalid combination of type specifiers"));
    assert!(parse_err("int int x;").contains("invalid combination of type specifiers"));
    assert!(parse_err("char int x;").contains("invalid combination of type specifiers"));
    assert!(parse_err("int f(...);").contains("named parameter"));
    assert!(parse_err("int f(...);").contains("named parameter is required"));
    assert!(parse_err("const const int x;").contains("duplicate 'const' qualifier"));
    assert!(parse_err("int * const const p;").contains("duplicate 'const' qualifier"));
    assert!(parsed("int * restrict p;\n").contains("restrict"));
}

#[test]
fn switch_statement_ast() {
    assert_eq!(
        parsed(
            "\
int main(void) {
    switch (1 + 2) {
    case 0: return 1;
    case 1: case 2: return 2;
    default: return 3;
    }
}
"
        ),
        "\
func int main
  block
    switch
      cond
        +
          int 1
          int 2
      body
        block
          case
            int 0
            return
              int 1
          case
            int 1
            case
              int 2
              return
                int 2
          default
            return
              int 3
"
    );
    assert!(
        parse_err("int main(void) { switch (1) { case 1, 2: return 0; } }")
            .contains("expected ':'")
    );
    assert!(
        parse_err("int main(void) { switch (1) { case : return 0; } }")
            .contains("expected an expression")
    );
}

#[test]
fn strings_qualifiers_and_varargs_parse() {
    assert_eq!(
        parsed("const char *p = \"hel\" \"lo\\n\";\n"),
        "\
global const char * p =
  string \"hello\\n\"
"
    );
    assert_eq!(
        parsed("int printf(const char *, ...);\n"),
        "\
proto int printf
  param const char *
  varargs
"
    );
    assert_eq!(
        parsed("char s[2] = \"hi\";\n"),
        "\
global char [2] s =
  string \"hi\"
"
    );
    assert_eq!(
        parsed("int *const p;\n"),
        "\
global int * const p
"
    );
}

#[test]
fn struct_declarators_and_member_access() {
    assert_eq!(
        parsed(
            "\
struct S { int x; char c; };
struct S s;
struct S *p;
"
        ),
        "\
global struct S s
global struct S * p
"
    );
    assert_eq!(
        parsed("int main(void) { return a.b + p->c; }\n"),
        "\
func int main
  block
    return
      +
        member b
          var a
        arrow c
          var p
"
    );
    assert_eq!(
        parsed("struct S { int x; } s = {1, 2};\n"),
        "\
global struct S s =
  list
    int 1
    int 2
"
    );
}

#[test]
fn integer_operator_precedence() {
    assert_eq!(
        parsed("int main(void) { return 1 + 2 << 3; }\n"),
        "\
func int main
  block
    return
      <<
        +
          int 1
          int 2
        int 3
"
    );
    assert_eq!(
        parsed("int main(void) { return 1 << 2 + 3; }\n"),
        "\
func int main
  block
    return
      <<
        int 1
        +
          int 2
          int 3
"
    );
    assert_eq!(
        parsed("int main(void) { return 5 & 3 == 1; }\n"),
        "\
func int main
  block
    return
      &
        int 5
        ==
          int 3
          int 1
"
    );
    assert_eq!(
        parsed("int main(void) { return 1 | 2 & 3 ^ 4; }\n"),
        "\
func int main
  block
    return
      |
        int 1
        ^
          &
            int 2
            int 3
          int 4
"
    );
    assert_eq!(
        parsed("int main(void) { return 1 || 2 | 3 && 4; }\n"),
        "\
func int main
  block
    return
      ||
        int 1
        &&
          |
            int 2
            int 3
          int 4
"
    );
    assert_eq!(
        parsed("int main(void) { return a ? b : c ? d : e; }\n"),
        "\
func int main
  block
    return
      ?:
        var a
        var b
        ?:
          var c
          var d
          var e
"
    );
    assert_eq!(
        parsed("int main(void) { return a ? b, c : d; }\n"),
        "\
func int main
  block
    return
      ?:
        var a
        ,
          var b
          var c
        var d
"
    );
    assert_eq!(
        parsed("int main(void) { return a ? b : c, d; }\n"),
        "\
func int main
  block
    return
      ,
        ?:
          var a
          var b
          var c
        var d
"
    );
    assert_eq!(
        parsed("int main(void) { a = b ? c : d; }\n"),
        "\
func int main
  block
    expr
      assign
        var a
        ?:
          var b
          var c
          var d
"
    );
    assert_eq!(
        parsed("int main(void) { a ? b : c = d; }\n"),
        "\
func int main
  block
    expr
      assign
        ?:
          var a
          var b
          var c
        var d
"
    );
    assert_eq!(
        parsed("int main(void) { a += b += 1; }\n"),
        "\
func int main
  block
    expr
      +=
        var a
        +=
          var b
          int 1
"
    );
    assert_eq!(
        parsed("int main(void) { x <<= 2; y >>= 1; }\n"),
        "\
func int main
  block
    expr
      <<=
        var x
        int 2
    expr
      >>=
        var y
        int 1
"
    );
    assert_eq!(
        parsed("int main(void) { ++x; x--; }\n"),
        "\
func int main
  block
    expr
      pre++
        var x
    expr
      post--
        var x
"
    );
    assert_eq!(
        parsed("int main(void) { return ~1 + !2; }\n"),
        "\
func int main
  block
    return
      +
        bnot
          int 1
        not
          int 2
"
    );
    assert_eq!(
        parsed("int main(void) { return sizeof(int) + sizeof x; }\n"),
        "\
func int main
  block
    return
      +
        sizeof int
        sizeof
          var x
"
    );
    assert_eq!(
        parsed("int main(void) { return (a = 1, b = 2); }\n"),
        "\
func int main
  block
    return
      ,
        assign
          var a
          int 1
        assign
          var b
          int 2
"
    );
    assert_eq!(
        parsed("int add(int a, int b); int main(void) { return add(1, (2, 3)); }\n"),
        "\
proto int add
  param int a
  param int b
func int main
  block
    return
      call add
        int 1
        ,
          int 2
          int 3
"
    );
    assert_eq!(
        parsed("int a = 1 << 4, b = sizeof(int);\n"),
        "\
global int a =
  <<
    int 1
    int 4
global int b =
  sizeof int
"
    );
}

#[test]
fn prototypes_and_globals() {
    assert_eq!(
        parsed(
            "\
static int counter;
extern int shared;
int ready = 1 + 2;
int add(int a, int b);
int sum(int, int);
int add(int a, int b) { return a + b; }
"
        ),
        "\
static global int counter
extern global int shared
global int ready =
  +
    int 1
    int 2
proto int add
  param int a
  param int b
proto int sum
  param int
  param int
func int add
  param int a
  param int b
  block
    return
      +
        var a
        var b
"
    );
}

#[test]
fn integer_types_and_casts() {
    assert_eq!(
        parsed(
            "\
unsigned long long int u;
signed char sc;
short int s;
int f(char, unsigned);
char g(long int x) { return (char)x; }
"
        ),
        "\
global unsigned long long u
global signed char sc
global short s
proto int f
  param char
  param unsigned int
func char g
  param long x
  block
    return
      cast char
        var x
"
    );
    assert_eq!(
        parsed("int main(void) { return (int)1 + 2; }\n"),
        "\
func int main
  block
    return
      +
        cast int
          int 1
        int 2
"
    );
    assert_eq!(
        parsed("int main(void) { return sizeof(unsigned long); }\n"),
        "\
func int main
  block
    return
      sizeof unsigned long
"
    );
}

#[test]
fn logical_not_in_integer_constant_expressions() {
    // C11 6.5.3.3p5: the result of `!` is 1 if the operand compares
    // equal to 0, and 0 otherwise. The result has type `int`. These
    // cases are parse-time integer constant expressions (6.6p3, p6)
    // used by `_Static_assert` (6.7.10), enumerators, and array bounds.
    assert!(parsed("_Static_assert(!0, \"ok\");\n").is_empty());
    assert!(parse_err("_Static_assert(!1, \"error\");").contains("static assertion failed: error"));
    assert!(parsed("enum { E = !0 }; int x = E;\n").contains("global int x =\n  int 1\n"));
    assert!(parsed("int a[!0 + 1];\n").contains("global int [2] a"));

    // Short-circuit `&&` and `||` must not evaluate the unselected
    // operand (6.5.13, 6.5.14), including after `!`.
    assert!(
        parsed("enum { E = !(0 && (1 / 0)) }; int x = E;\n").contains("global int x =\n  int 1\n")
    );
    assert!(
        parsed("enum { E = !(1 || (1 / 0)) }; int x = E;\n").contains("global int x =\n  int 0\n")
    );
    assert!(parsed("int a[!(0 && (1 / 0)) + 1];\n").contains("global int [2] a"));
    assert!(parsed("int a[!(1 || (1 / 0)) + 1];\n").contains("global int [1] a"));

    // Repeated inversion, and a value other than 0 or 1.
    assert!(parsed("int a[!!0 + 1];\n").contains("global int [1] a"));
    assert!(parsed("int a[!!1 + 1];\n").contains("global int [2] a"));
    assert!(parsed("int a[!!!0 + 1];\n").contains("global int [2] a"));
    assert!(parsed("int a[!5 + 1];\n").contains("global int [1] a"));
    assert!(parsed("int a[!!5 + 1];\n").contains("global int [2] a"));

    // The same expressions at each ICE entry point, including nested
    // and multiple `_Static_assert`.
    assert!(parsed(
        "_Static_assert(!0, \"a\"); _Static_assert(!!1, \"b\"); _Static_assert(!!!0, \"c\");\n"
    )
    .is_empty());
    assert!(
        parsed("int main(void) { _Static_assert(!0, \"b\"); return 0; }\n")
            .contains("func int main")
    );
    assert!(
        parsed("struct S { int x; _Static_assert(!0, \"m\"); } s;\n").contains("global struct S s")
    );
    assert!(parse_err("int main(void) { _Static_assert(!1, \"b\"); }")
        .contains("static assertion failed: b"));
    assert!(parse_err("struct S { int a; _Static_assert(!1, \"m\"); };")
        .contains("static assertion failed: m"));
    assert!(parsed(
        "enum { A = !0, B = !!0, C = !!1, D = !!!0, E = !5, F = !!5 }; int x[A + C + D + F];\n"
    )
    .contains("global int [4] x"));
    let arrays = parsed("int a[!0 + 1]; int b[!!1 + 1]; int c[!!!0 + 1];\n");
    assert!(
        arrays.contains("global int [2] a")
            && arrays.contains("global int [2] b")
            && arrays.contains("global int [2] c")
    );
    // `!1` is 0, so it is not a valid array bound.
    assert!(parse_err("enum { E = !1 }; int a[E];").contains("array size must be positive"));

    // Bit-field widths are also parse-time ICEs. `!0` is 1, so the
    // field is valid; `!1` is 0, which is a diagnostic on a named field.
    assert!(parsed("struct S { unsigned f : !0; } s;\n").contains("global struct S s"));
    assert!(parse_err("struct S { unsigned f : !1; };").contains("zero width"));
}

#[test]
fn math_format_builtins_in_constant_expressions() {
    // C11 7.12p4–p5: INFINITY and NAN are constant expressions of type
    // float. glibc spells them as these GCC builtins. The type of each
    // builtin is visible to `_Generic` in an integer constant expression
    // (6.5.1.1, 6.6, 6.7.10).
    assert!(parsed(
        "_Static_assert(_Generic(__builtin_nanf(\"\"), float: 1, default: 0), \"NAN\");\n",
    )
    .is_empty());
    assert!(
        parsed("_Static_assert(_Generic(__builtin_inff(), float: 1, default: 0), \"INF\");\n",)
            .is_empty()
    );
    assert!(parsed(
        "_Static_assert(_Generic(__builtin_huge_val(), double: 1, default: 0), \"HV\");\n",
    )
    .is_empty());
    assert!(parsed(
        "_Static_assert(_Generic(__builtin_huge_valf(), float: 1, default: 0), \"HVF\");\n",
    )
    .is_empty());
    assert!(parsed(
        "_Static_assert(_Generic(__builtin_huge_vall(), long double: 1, default: 0), \"HVL\");\n",
    )
    .is_empty());
    assert!(parsed(
        "_Static_assert(_Generic(__builtin_nan(\"\"), double: 1, default: 0), \"nan\");\n",
    )
    .is_empty());
    assert!(parsed(
        "_Static_assert(_Generic(__builtin_nanl(\"\"), long double: 1, default: 0), \"nanl\");\n",
    )
    .is_empty());
    assert!(
        parsed("_Static_assert(_Generic(__builtin_inf(), double: 1, default: 0), \"inf\");\n",)
            .is_empty()
    );
    assert!(parsed(
        "_Static_assert(_Generic(__builtin_infl(), long double: 1, default: 0), \"infl\");\n",
    )
    .is_empty());
    assert!(parsed("int a[sizeof(__builtin_nanf(\"\"))];\n").contains("global int [4] a"));
    assert!(parsed("int a[sizeof(__builtin_huge_val())];\n").contains("global int [8] a"));
    assert!(parsed("int a[sizeof(__builtin_huge_vall())];\n").contains("global int [16] a"));
    // A non-literal nan argument is a diagnostic at every ICE entry
    // point, not only in semantic analysis. sizeof / _Generic would
    // otherwise see the result type and accept the program.
    assert!(parse_err("_Static_assert(__builtin_nanf(0), \"x\");")
        .contains("requires a string literal argument"));
    assert!(
        parse_err("_Static_assert(_Generic(__builtin_nanf(0), float: 1, default: 0), \"x\");")
            .contains("requires a string literal argument")
    );
    assert!(parse_err("enum { E = sizeof(__builtin_nanf(0)) };")
        .contains("requires a string literal argument"));
    assert!(parse_err("int a[sizeof(__builtin_nanf(0))];")
        .contains("requires a string literal argument"));
    assert!(
        parse_err("enum { E = _Generic(__builtin_nanf(0), float: 1, default: 0) };")
            .contains("requires a string literal argument")
    );
    assert!(
        parse_err("int a[_Generic(__builtin_nanf(0), float: 1, default: 0)];")
            .contains("requires a string literal argument")
    );
}

#[test]
fn smoke_sources_parse() {
    for name in [
        "tests/01_return_const.c",
        "tests/02_add.c",
        "tests/03_loop.c",
        "tests/04_call.c",
    ] {
        let path = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), name);
        let source = std::fs::read_to_string(&path).unwrap();
        let tokens = lex(&source, name).unwrap();
        parse(&tokens, name).unwrap_or_else(|err| panic!("{name}: {err}"));
    }
}

#[test]
fn constant_contexts_check_comparison_operands() {
    // C11 6.5.8p2 and 6.5.9p2 are constraints, so they apply in an
    // operand of sizeof even though it is not evaluated, including a
    // subscript of __builtin_offsetof that folds to a constant.
    for source in [
        "enum { N = sizeof(1 < (int *)0) };",
        "int a[sizeof((int *)0 > 1.0)];",
        "_Static_assert(sizeof((int *)0 == 1), \"\");",
        "_Static_assert(sizeof((int *)0 != 1.0), \"\");",
        "struct T { int x; }; struct T t; enum { N = sizeof(t < t) };",
        "struct B { int x : sizeof(1 >= (char *)0); };",
        "struct S { int a[8]; };\n\
             enum { N = __builtin_offsetof(struct S, a[sizeof(1 < (int *)0)]) };",
    ] {
        let err = parse_err(source);
        assert!(
            err.contains("invalid operands to a comparison"),
            "{source}: {err}"
        );
    }
    // Valid comparisons still fold.
    parsed(
        "int *p;\nenum { A = sizeof(1 < 2.0), B = sizeof(p == 0), C = sizeof(p < p),\n\
             D = sizeof(p != (void *)0), E = sizeof(0 == p), F = sizeof(1 == 2) };",
    );
    parsed(
        "struct S { int a[8]; };\n\
             enum { N = __builtin_offsetof(struct S, a[sizeof((int *)0 == 0)]) };",
    );
}

#[test]
fn constant_contexts_check_fp_compare_builtin_operands() {
    // Both operands arithmetic and at least one real floating, as
    // semantic analysis requires; parser-folded contexts apply the
    // same rule.
    for source in [
        "enum { N = sizeof(__builtin_isgreater(1, 2)) };",
        "int a[sizeof(__builtin_isless((int *)0, 1.0))];",
        "_Static_assert(sizeof(__builtin_isunordered('a', 2L)) == 4, \"\");",
        "struct B { int x : sizeof(__builtin_islessgreater(1, 2)); };",
        "struct T { double d; }; struct T t; enum { N = sizeof(__builtin_islessequal(t, 1.0)) };",
        "struct S { int a[8]; };\n\
             enum { N = __builtin_offsetof(struct S, a[sizeof(__builtin_isgreaterequal(1, 2))]) };",
    ] {
        let err = parse_err(source);
        assert!(
            err.contains("non-floating-point arguments"),
            "{source}: {err}"
        );
    }
    parsed(
            "enum { A = sizeof(__builtin_isgreater(1, 2.0f)), B = sizeof(__builtin_isless(1.0L, 'a')),\n\
             C = sizeof(__builtin_isunordered(1.0, 2.0)) };",
        );
}

#[test]
fn restrict_function_return_requires_pointer() {
    // C11 6.7.3p2: types other than pointer types whose referenced type
    // is an object type shall not be restrict-qualified. A function
    // return type is no exception.
    for source in [
        "int restrict f(void);",
        "int restrict f(void) { return 0; }",
        "void g(void) { int restrict f(void); }",
        "const int restrict f(void);",
        "int restrict (*f(void))(void);",
        "unsigned long s = sizeof(int restrict (*)(void));",
    ] {
        let err = parse_err(source);
        assert!(
            err.contains("restrict requires a pointer type"),
            "{source}: {err}"
        );
    }
    // A restrict-qualified pointer to an object type is valid as a
    // return type and as a parameter type.
    parsed("int *restrict f(void);");
    parsed("int *restrict f(void) { return 0; }");
    parsed("void g(void) { int *restrict f(void); }");
    parsed("void h(int *restrict p, int *restrict q);");
    parsed("void h(int *restrict p, int *restrict q) { *p = *q; }");
    parsed("typedef int *restrict (*fp)(int *restrict);");
    parsed("unsigned long s = sizeof(int *restrict (*)(void));");
}

#[test]
fn constant_contexts_check_comparison_pointer_compatibility() {
    // C11 6.5.9p2: two pointer operands of an equality operator point
    // to compatible types (ignoring qualifiers), or one of them points
    // to void. C11 6.5.8p2: two pointer operands of a relational
    // operator point to compatible object types. These constraints
    // hold in an unevaluated sizeof operand that the parser folds.
    for source in [
        "enum { N = sizeof((int *)0 == (double *)0) };",
        "int a[sizeof((int *)0 != (long *)0)];",
        "_Static_assert(sizeof((int *)0 < (double *)0), \"\");",
        "struct B { int x : sizeof((char *)0 >= (unsigned char *)0); };",
        "int *p; double *q; enum { N = sizeof(p == q) };",
        "enum { N = sizeof((char **)0 == (const char **)0) };",
        "enum { N = sizeof((int *)0 <= (void *)0) };",
        "int x[2]; long y[2]; enum { N = sizeof(x == y) };",
        "struct S { int a[8]; };\n\
             enum { N = __builtin_offsetof(struct S, a[sizeof((int *)0 == (float *)0)]) };",
    ] {
        let err = parse_err(source);
        assert!(
            err.contains("invalid operands to a comparison"),
            "{source}: {err}"
        );
    }
    // Compatible pointees, differing qualifiers, void pointers in an
    // equality, and null pointer constants still fold.
    parsed(
            "int *p; const int *cp; void *v; int x[2];\n\
             enum { A = sizeof(p == cp), B = sizeof(p < cp), C = sizeof(p == v),\n\
             D = sizeof(v != cp), E = sizeof(p == 0), F = sizeof(x == p),\n\
             G = sizeof((int (*)[2])0 == (int (*)[])0), H = sizeof((char *)0 > (volatile char *)0) };",
        );
}

#[test]
fn constant_contexts_type_nested_builtin_calls() {
    // __builtin_signbit* yields int and __builtin_memcpy, memmove, and
    // memset yield void *, as in semantic analysis. A call nested in a
    // parser-folded constant keeps that type, so the operand rules of
    // an enclosing comparison builtin or operator apply to it.
    for source in [
            "enum { N = sizeof(__builtin_isgreater(__builtin_signbit(1.0), 2)) };",
            "enum { N = sizeof(__builtin_isless(__builtin_signbitf(1.0f), __builtin_signbitl(2.0L))) };",
            "int a[sizeof(__builtin_isgreater(__builtin_signbit(1.0), 2))];",
            "_Static_assert(sizeof(__builtin_islessequal(__builtin_signbit(1.0), 2)), \"\");",
            "struct B { int x : sizeof(__builtin_isunordered(1, __builtin_signbit(2.0))); };",
            "enum { N = sizeof(__builtin_isgreater(__builtin_memcpy((char *)0, (char *)0, 1), 1.0)) };",
            "struct S { int a[8]; };\n\
             enum { N = __builtin_offsetof(struct S, a[sizeof(__builtin_isgreater(__builtin_signbit(1.0), 2))]) };",
        ] {
            let err = parse_err(source);
            assert!(err.contains("non-floating-point arguments"), "{source}: {err}");
        }
    for source in [
        "int *p; double *q; enum { N = sizeof(__builtin_memcpy(p, p, 1) < q) };",
        // A call is not an integer constant expression (C11 6.6p6), so
        // the int result is no null pointer constant. Clang rejects
        // this; GCC folds the call and accepts it.
        "enum { N = sizeof(__builtin_signbit(1.0) == (int *)0) };",
        "enum { N = sizeof(__builtin_memset((char *)0, 0, 1) == 1.0) };",
    ] {
        let err = parse_err(source);
        assert!(
            err.contains("invalid operands to a comparison"),
            "{source}: {err}"
        );
    }
    // The builtins' own operand constraints, as semantic analysis
    // states them.
    for (source, message) in [
        (
            "enum { N = sizeof(__builtin_signbit((int *)0)) };",
            "requires an arithmetic argument",
        ),
        (
            "struct T { double d; } t; enum { N = sizeof(__builtin_signbitl(t)) };",
            "requires an arithmetic argument",
        ),
        (
            "enum { N = sizeof(__builtin_memcpy(1, (char *)0, 1)) };",
            "destination must be a pointer",
        ),
        (
            "enum { N = sizeof(__builtin_memmove((char *)0, 1.0, 1)) };",
            "source must be a pointer",
        ),
        (
            "enum { N = sizeof(__builtin_memset((char *)0, (char *)0, 1)) };",
            "value must be an integer",
        ),
        (
            "enum { N = sizeof(__builtin_memcpy((char *)0, (char *)0, (char *)0)) };",
            "size must be an integer",
        ),
    ] {
        let err = parse_err(source);
        assert!(err.contains(message), "{source}: {err}");
    }
    // Valid uses fold with the right result type.
    parsed(
            "int *p; double *q;\n\
             enum { A = sizeof(__builtin_signbit(1.0)), B = sizeof(__builtin_memcpy(p, p, 1)),\n\
             C = sizeof(__builtin_isgreater(__builtin_signbit(1.0), 2.0)),\n\
             D = sizeof(__builtin_memcpy(p, p, 1) == q), E = sizeof(__builtin_memset(p, 0, 4) == p),\n\
             F = _Generic(__builtin_signbit(1.0), int: 1, default: 2),\n\
             G = _Generic(__builtin_memmove(p, q, 1), void *: 1, default: 2),\n\
             H = sizeof(__builtin_signbit(__builtin_nan(\"\"))) };",
        );
    parsed("_Static_assert(sizeof(__builtin_signbitf(1.0f)) == sizeof(int), \"\");");
}

#[test]
fn restrict_requires_pointer_to_object_type() {
    // C11 6.7.3p2: the referenced type of a restrict-qualified pointer
    // is an object type, so a pointer to a function may not be
    // restrict-qualified, whether it is an object, a parameter, a
    // typedef, or a function return type.
    for source in [
        "int (*restrict pick(void))(void);",
        "int (*restrict fp)(void);",
        "void g(void) { int (*restrict fp)(void); }",
        "void h(int (*restrict p)(void));",
        "typedef int (*restrict F)(void);",
        "int (*restrict *pfp)(void);",
    ] {
        let err = parse_err(source);
        assert!(
            err.contains("restrict requires a pointer to an object type"),
            "{source}: {err}"
        );
    }
    // A restrict-qualified pointer to a pointer to a function, to an
    // array, to void, or to a pointer stays valid.
    parsed(
        "int (**restrict fpp)(void); int **restrict pp; int (*restrict ap)[4];\n\
             void *restrict v; int *restrict *restrict q;\n\
             int (**restrict pick(void))(void);",
    );
}

#[test]
fn constant_contexts_void_pointer_equality_excludes_function_pointers() {
    // C11 6.5.9p2 in a parser-folded sizeof operand: a void pointer and
    // a pointer to a function are not comparable, but a null pointer
    // constant, including (void *)0, is comparable with either.
    for source in [
        "int f(void); void *v; enum { N = sizeof(v == f) };",
        "int (*fp)(void); const void *v; int a[sizeof(fp != v)];",
        "int f(void); void *v; _Static_assert(sizeof(f == v), \"\");",
    ] {
        let err = parse_err(source);
        assert!(
            err.contains("invalid operands to a comparison"),
            "{source}: {err}"
        );
    }
    parsed(
        "int f(void); void *v; int *p;\n\
             enum { A = sizeof((void *)0 == f), B = sizeof(f != 0), C = sizeof(v == p),\n\
             D = sizeof(f == (void *)(1 - 1)), E = sizeof((const char *)0 != v) };",
    );
}

#[test]
fn gnu_dialect_folded_void_pointer_function_pointer_equality() {
    // The parser's folded constants follow the dialect: -std=gnu11
    // accepts a void pointer compared with a function pointer.
    use crate::Dialect;
    for source in [
        "int f(void); void *v; enum { N = sizeof(v == f) };",
        "int f(void); void *v; enum { N = sizeof(v != &f) };",
        "int (*fp)(void); const void *v; int a[sizeof(fp != v)];",
    ] {
        let tokens = lex(source, "t.c").unwrap();
        let err = parse_dialect(&tokens, "t.c", Dialect::C11)
            .err()
            .map(|err| err.to_string())
            .unwrap_or_default();
        assert!(
            err.contains("invalid operands to a comparison"),
            "{source}: {err}"
        );
        assert!(
            parse_dialect(&tokens, "t.c", Dialect::Gnu11).is_ok(),
            "{source}"
        );
    }
}

#[test]
fn folded_null_pointer_constants_generic_and_nested_casts() {
    // C11 6.3.2.3p3: a null pointer constant is an integer constant
    // expression with the value 0, or such an expression cast to
    // void *. A generic selection keeps the selected expression's
    // null-pointer-constant status, as Clang 19 and GCC 14 with
    // -pedantic-errors treat it. A void * cast of (void *)0 is a null
    // pointer but not a null pointer constant, so it is not comparable
    // with a function pointer.
    parsed("int f(void); enum { N = sizeof(f == _Generic(0, int: (void *)0)) };");
    parsed("int f(void); enum { N = sizeof(_Generic(1L, long: 0, default: f) != f) };");
    for source in [
        "int f(void); enum { N = sizeof(f == (void *)(void *)0) };",
        "int f(void); enum { N = sizeof((void *)(void *)(1 - 1) != &f) };",
    ] {
        let err = parse_err(source);
        assert!(
            err.contains("invalid operands to a comparison"),
            "{source}: {err}"
        );
    }
}

#[test]
fn enum_specifier_is_the_type_specifier() {
    // C11 6.7.2p2: an enum specifier takes no other type word. Clang 19 and
    // GCC 14 reject each of these.
    for source in [
        "enum { A } unsigned x;",
        "enum E { B } enum E y;",
        "enum F { C } struct S *p;",
        "enum G { D } long z;",
        "struct R { int i; } enum N { O } r;",
        "enum H { E1 } _Bool b;",
        "enum I { E2 } double d;",
    ] {
        assert!(
            parse_err(source).contains("invalid combination of type specifiers"),
            "{source}"
        );
    }
    parsed("enum { A }; enum E { B } const volatile q; enum E y; typedef int T; enum J { K } T2;");
}

#[test]
fn function_specifiers_only_on_functions() {
    // C11 6.7.4p2. Clang 19 rejects every one of these.
    for source in [
        "_Noreturn int x;",
        "inline int y;",
        "void g(void) { _Noreturn int z; }",
        "_Noreturn void (*fp)(void);",
        "_Noreturn int a(void), b;",
        "typedef inline int F(void);",
        "typedef _Noreturn void G(void);",
        "void h(inline int x);",
        "void k(_Noreturn void f(void));",
        "struct T { inline int m; };",
        "inline struct S { int x; };",
        "__inline__ int w;",
    ] {
        assert!(
            parse_err(source).contains("can only appear on functions"),
            "{source}"
        );
    }
    parsed(
        "_Noreturn void stop(void); static inline _Noreturn void halt(void); \
         void g(void) { _Noreturn void h(void); inline int k(void); } \
         static __inline__ int sq(int x) { return x * x; } _Noreturn void stop(void) { for (;;); }",
    );
}

fn parse_gnu(source: &str) -> Result<String, String> {
    let tokens = lex(source, "t.c").map_err(|err| err.to_string())?;
    parse_dialect(&tokens, "t.c", crate::Dialect::Gnu11)
        .map(|unit| dump(&unit))
        .map_err(|err| err.to_string())
}

#[test]
fn unevaluated_operand_types_follow_c11() {
    // C11 6.5.6p2, p3: no pointer + pointer, and pointer - pointer only for
    // compatible complete object types, even where the operand is only typed.
    for source in [
        "int a[sizeof((int *)0 + (int *)0)];",
        "int a[sizeof((int *)0 - (long *)0)];",
        "struct I; int a[sizeof((struct I *)0 - (struct I *)0)];",
        "int a[sizeof((void *)0 - (void *)0)];",
    ] {
        assert!(parse_err(source).contains("invalid operand"), "{source}");
    }
    assert!(parse_gnu("__typeof__((int *)0 + (int *)0) x;")
        .unwrap_err()
        .contains("invalid operand"));
    parsed("int a[sizeof((int *)0 - (const int *)0)]; int b[sizeof((int *)0 + 1)];");
    // C11 6.5.15p6: `void *` with an object pointer gives `void *`, and the
    // qualifiers of both pointees are kept; a null pointer constant takes
    // the other operand's type. `_Generic` folded by the parser sees it.
    parsed(
        "_Static_assert(_Generic(1 ? (int *)0 : (void *)1, void *: 1, default: 0), \"v\"); \
         _Static_assert(_Generic(1 ? (const int *)0 : (int *)1, const int *: 1, default: 0), \"q\"); \
         _Static_assert(_Generic(1 ? (int *)1 : 0, int *: 1, default: 0), \"n\"); \
         _Static_assert(_Generic(1 ? (volatile void *)1 : (const char *)1, \
                                 const volatile void *: 1, default: 0), \"cv\");",
    );
    assert!(parse_err("int a[sizeof(1 ? (int *)1 : (long *)1)];").contains("invalid operand"));
    assert!(parse_err("int a[sizeof(1 ? (int *)1 : 5)];").contains("invalid operand"));
    // A null pointer constant `(void *)0` takes the other operand's type
    // before `void *` merging.
    parsed(
        "_Static_assert(_Generic(1 ? (int *)1 : (void *)0, int *: 1, default: 0), \"n\"); \
         _Static_assert(_Generic(1 ? (void *)0 : (const int *)1, const int *: 1, default: 0), \"m\");",
    );
    // C11 6.5.15p3: void only with void, and a record only with the same
    // record type.
    assert!(
        parse_err("int a[_Generic(1 ? (void)0 : 1, int: 1, default: 2)];")
            .contains("invalid operand")
    );
    assert!(parse_gnu(
        "struct A { int x; }; struct B { int y; }; \
         __typeof__(1 ? (struct A){0} : (struct B){0}) x;"
    )
    .is_err());
    parse_gnu("struct A { int x; }; __typeof__(1 ? (struct A){0} : (struct A){1}) x;").unwrap();
    // typeof in GNU mode takes the same type.
    parse_gnu("__typeof__(1 ? (int *)0 : (void *)1) p; double *d; void h(void) { p = d; }")
        .unwrap();
    // A local declaration hides a library forwarder of the same name, as in
    // semantic analysis (the call's type is the local function's).
    parsed(
        "int g(void) { char __builtin_strlen(int); \
         _Static_assert(sizeof(__builtin_strlen(0)) == 1, \"s\"); return 0; }",
    );
}

#[test]
fn oversized_records_are_too_large() {
    for source in [
        "struct S { char pad[536870912]; unsigned int x : 1; };",
        "struct S { char a[0x1000000000000000]; char b[0x1000000000000000]; char c; };",
        "struct S { char a[0x0fffffffffffffff]; char b[0x0fffffffffffffff]; char c[0x0fffffffffffffff]; };",
    ] {
        assert!(parse_err(source).contains("struct 'S' is too large"), "{source}");
    }
    parsed("struct S { char pad[536870911]; unsigned int x : 1; };");
}

#[test]
fn gnu_enum_values_stay_within_64_bits() {
    // Underlying types wider than 64 bits are outside the subset, and a
    // value above 64 bits is rejected however it is spelled.
    for source in [
        "enum E : unsigned __int128 { A };",
        "enum E : __int128 { A = 1 };",
    ] {
        assert!(
            parse_gnu(source)
                .unwrap_err()
                .contains("wider than 64 bits"),
            "{source}"
        );
    }
    for source in [
        "enum E { A = ~(unsigned __int128)0 };",
        "enum E { A = (unsigned __int128)1 << 64 };",
        "enum E { A = 0xffffffffffffffffUL, B };",
        "enum E : unsigned long { A = 0xffffffffffffffffUL, B };",
    ] {
        assert!(parse_gnu(source).is_err(), "{source}");
    }
    parse_gnu("enum E { A = (unsigned __int128)0xffffffffffffffffUL };").unwrap();
}

#[test]
fn gnu_enum_colon_needs_a_type_after_it() {
    // `enum E : 3` is an unnamed bit-field, and `enum E: 1` a _Generic
    // association; only a type after the colon makes it an underlying type.
    parse_gnu(
        "enum E { A }; struct S { enum E : 3; int x; }; \
         int f(enum E e) { return _Generic(e, enum E: 1, default: 2); }",
    )
    .unwrap();
    parse_gnu("enum E : long { A = sizeof(enum E) }; _Static_assert(A == 8, \"base\");").unwrap();
}

#[test]
fn gnu_enum_list_rules() {
    // An enum without a fixed type is incomplete inside its own list; with
    // one it is complete. An enumeration is not an underlying type.
    assert!(
        parse_gnu("enum E { A = sizeof(enum E), B = 0x100000000L };")
            .unwrap_err()
            .contains("incomplete")
    );
    assert!(parse_err("enum E { A = sizeof(enum E) };").contains("incomplete"));
    parse_gnu("enum E : long { A = sizeof(enum E) }; _Static_assert(A == 8, \"size\");").unwrap();
    assert!(
        parse_gnu("enum E : long { A = sizeof(enum E) }; _Static_assert(A == 4, \"size\");")
            .is_err()
    );
    assert!(parse_gnu("enum E { X }; enum F : enum E { B };").is_err());
    assert!(parse_gnu("enum F : enum E { X } { B };")
        .unwrap_err()
        .contains("must be an integer type"));
    parse_gnu("enum F : int { B }; enum F f; int g(void) { return sizeof(enum F); }").unwrap();
}

#[test]
fn kr_definitions_parse_and_reject_bad_lists() {
    assert!(parsed("int add(a, b) int a; int b; { return a + b; }\n").contains("func int add"));
    assert!(parsed("int add(a, b) int a, b; { return a + b; }\n").contains("func int add"));
    assert!(parsed("int inc(x) register int x; { return x + 1; }\n").contains("func int inc"));
    assert!(parse_err("int f(a) { return a; }").contains("was not declared"));
    assert!(parse_err("int f(a, a) int a; { return a; }").contains("redefinition of parameter"));
    assert!(parse_err("int f(a) int a; int a; { return a; }").contains("redefinition of parameter"));
    assert!(parse_err("int f(a) int b; { return 0; }").contains("is not a parameter"));
    assert!(
        parse_err("int f(a) static int a; { return a; }").contains("may only use the 'register'")
    );
    assert!(parse_err("int f(a) int a = 1; { return a; }").contains("initializer"));
    assert!(parse_err("int (*p)(a, b);")
        .contains("identifier list in a function declarator that is not a definition"));
    assert!(parse_err("int outer(int callback(x));")
        .contains("identifier list in a function declarator that is not a definition"));
    assert!(
        parse_err("typedef int F(x);").contains("identifier list")
            || parse_err("typedef int F(x);").contains("must be a definition")
    );
    let _ = parsed("typedef int F(int);\n");
    assert!(parse_err("int f(g) int g(x); { return 0; }").contains("identifier list"));
    assert!(parsed("int f(g) int g(int); { return 0; }\n").contains("func int f"));
    assert!(parse_err("int f(x) auto int x; { return x; }").contains("may only use the 'register'"));
    assert!(
        parsed("int f(a) enum { Value = 7 } a; { return a; }\nint Value;\n").contains("func int f")
    );
    assert!(
        parsed("int f(a, b) int a; int (*b)[sizeof a]; { return sizeof *b; }\n")
            .contains("func int f")
    );
}

#[test]
fn variable_length_arrays_parse_and_reject_constraints() {
    let simple = parsed("void f(int n) { int a[n]; }\n");
    assert!(simple.contains("decl int [*] a"), "{simple}");
    let multi = parsed("void f(int n) { int a[n][n]; int (*p)[n]; }\n");
    assert!(multi.contains("decl int [*][*] a"), "{multi}");
    assert!(multi.contains("decl int (*)[*] p"), "{multi}");
    let proto = parsed("void f(int n, int a[n][n]);\n");
    assert!(proto.contains("param int (*)[*] a"), "{proto}");
    let proto_star = parsed("void f(int n, int a[*][*]);\n");
    assert!(proto_star.contains("param int (*)[*] a"), "{proto_star}");
    let typdef = parsed("void f(int n) { typedef int T[n]; T x; }\n");
    assert!(typdef.contains("decl int [*] T"), "{typdef}");
    assert!(typdef.contains("decl int [*] x"), "{typdef}");
    assert!(parse_err("int n; int a[n];").contains("file scope"));
    assert!(parse_err("int n; extern int (*p)[n];").contains("file scope"));
    assert!(parse_err("void f(int n) { static int a[n]; }").contains("static or thread"));
    assert!(parse_err("void f(int n) { extern int a[n]; }").contains("linkage"));
    assert!(parse_err("void f(int n) { extern int (*p)[n]; }").contains("linkage"));
    assert!(parse_err("void f(int n) { int a[n] = {0}; }").contains("initializer"));
    assert!(parse_err("void f(int n) { struct S { int a[n]; }; }").contains("member"));
    assert!(parse_err("void f(int n) { struct S { int (*p)[n]; }; }").contains("member"));
    assert!(parse_err("void f(int a[*]) { }").contains("star array notation"));
    assert!(parse_err("int a[*];").contains("star array notation"));
    // `[*]` in a nested prototype is allowed on a definition (C11 6.7.6.2p4).
    let _ = parsed("void f(void (*cb)(int a[2][*])) { }\n");
    assert!(
        parse_err("void f(int n) { typedef int T[n]; typedef int T[n]; }").contains("redefined")
    );
    assert!(parse_err("void f(int n) { (int[n]){0}; }").contains("compound literal"));
    assert!(parse_err("int n; int (*f(void))[n];").contains("variably modified return type"));
    assert!(parse_err("int n; typedef int (*F(void))[n];").contains("file scope"));
    assert!(parse_err("void f(void) { (void)_Alignof(int[1.0]); }").contains("integer type"));
    let _ = parsed("void f(int n) { typedef int T[n]; { double n; (void)_Alignof(T); } }\n");
}

#[test]
fn atomic_type_qualifier_and_specifier() {
    let ast = parsed("_Atomic int x; _Atomic(int) y; const _Atomic volatile int z;\n");
    assert!(ast.contains("x"), "{ast}");
    assert!(
        parse_err("_Atomic(const int) x;").contains("qualified type"),
        "{}",
        parse_err("_Atomic(const int) x;")
    );
    assert!(
        parse_err("typedef int A[3]; _Atomic(A) x;").contains("array type"),
        "{}",
        parse_err("typedef int A[3]; _Atomic(A) x;")
    );
    assert!(
        parse_err("typedef int F(void); _Atomic(F) x;").contains("function type"),
        "{}",
        parse_err("typedef int F(void); _Atomic(F) x;")
    );
    assert!(
        parse_err("typedef _Atomic int Ai; _Atomic(Ai) x;").contains("atomic type"),
        "{}",
        parse_err("typedef _Atomic int Ai; _Atomic(Ai) x;")
    );
    assert!(
        parse_err("struct S { _Atomic int x : 3; };").contains("atomic type"),
        "{}",
        parse_err("struct S { _Atomic int x : 3; };")
    );
    let repeated = parsed("_Atomic _Atomic int x; int * _Atomic _Atomic p;\n");
    assert!(repeated.contains("x"), "{repeated}");
    assert!(repeated.contains("p"), "{repeated}");
    // Repeated qualifiers are the same type as a single qualifier (C11 6.7.3p5).
    assert_eq!(parsed("_Atomic int x; int * _Atomic p;\n"), repeated);
    assert!(
        parse_err("int * _Atomic (p);").contains("type specifier"),
        "{}",
        parse_err("int * _Atomic (p);")
    );
    let shadow = parsed("typedef int T; void f(void) { _Atomic(int) T; }\n");
    assert!(shadow.contains("T"), "{shadow}");
}

#[test]
fn atomic_pointer_and_array_element() {
    // `_Atomic int a[3]` is an array of atomic int (C11 6.7.3p9), not an
    // atomic array. `_Atomic(int *)` is an atomic pointer.
    assert!(parsed("_Atomic int a[3];\n").contains("a"));
    assert!(parsed("_Atomic(int *) p;\n").contains("p"));
    assert!(parsed("typedef int F(void); F *_Atomic p;\n").contains("p"));
    assert!(
        parse_err("typedef int F(void); _Atomic F x;").contains("function type"),
        "{}",
        parse_err("typedef int F(void); _Atomic F x;")
    );
    assert!(
        parse_err("typedef int F(void); int k = sizeof(_Atomic F *);").contains("function type"),
        "{}",
        parse_err("typedef int F(void); int k = sizeof(_Atomic F *);")
    );
}

#[test]
fn array_bound_errors_name_the_problem() {
    assert!(parse_err("int a[1 / 0];").contains("array size divides by zero"));
    assert!(parse_err("int a[1 % 0];").contains("array size divides by zero"));
    assert!(parse_err("int a[(-2147483647 - 1) / -1];").contains("array size overflows"));
    assert!(parse_err("int a[2147483647 + 1];").contains("array size overflows"));
    assert!(parse_err("int a[1 << 40];").contains("array size uses an undefined shift"));
    assert!(parse_err("int a[1 << -1];").contains("array size uses an undefined shift"));
    assert!(parse_err("int a[-1];").contains("array size must be positive"));
    assert!(parse_err("int a[0 ? 1 : -1];").contains("array size must be positive"));
    assert!(parse_err("int a[\"x\"[0]];").contains("variably modified type"));
    // The unevaluated arm of ?: and the skipped operand of && and || are
    // not evaluated (C11 6.6p3), so they may divide by zero.
    assert!(parsed("int a[1 ? 2 : 1 / 0];").contains("a"));
    assert!(parsed("int a[0 && 1 / 0 ? 1 : 2];").contains("a"));
    assert!(parsed("int a[1 || 1 / 0];").contains("a"));
    assert!(parse_err("enum { E = 1 / 0 };").contains("division by zero in a constant expression"));
    assert!(parse_err("enum { E = 1 << 64 };").contains("undefined shift"));
    // An enumerator or a bit-field width is not an array size: the
    // message must not say it is.
    assert!(!parse_err("enum { E = 1 / 0 };").contains("array size"));
    assert!(parse_err("struct S { int x : 1 / 0; };")
        .contains("division by zero in a constant expression"));
    assert!(parse_err("struct S { int x : 2147483647 + 1; };")
        .contains("constant expression overflows"));
}
