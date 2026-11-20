use super::*;
use crate::lexer::lex;
use crate::parser::parse;

fn sema_err(source: &str) -> String {
    let tokens = lex(source, "neg.c").unwrap();
    let mut unit = match parse(&tokens, "neg.c") {
        Ok(unit) => unit,
        Err(err) => return err.to_string(),
    };
    analyze(&mut unit, "neg.c").unwrap_err().to_string()
}

fn sema_ok(source: &str) {
    let tokens = lex(source, "ok.c").unwrap();
    let mut unit = parse(&tokens, "ok.c").unwrap();
    analyze(&mut unit, "ok.c").unwrap();
}

#[test]
fn designated_aggregate_members_elide_braces() {
    // C11 6.7.9p17, p20: `.m = 1, 2` starts at the first scalar of `m`.
    sema_ok("struct I { int a, b; }; struct O { struct I in; int z; } o = { .in = 1, 2, 3 };");
    sema_ok("struct I { int a, b; }; union V { int i; struct I in; } v = { .in = 1, 2 };");
    sema_ok("struct I { int a, b; }; struct I a[] = { [1] = 3, 4, 5 };");
    sema_ok("union V { int i; double d; }; struct O { int k; union V v; } o = { .v.d = 2.5 };");
    assert!(sema_err(
        "struct I { int a, b; }; union V { int i; struct I in; } v = { .in = 1, 2, 3 };"
    )
    .contains("excess elements in initializer"));
    assert!(
        sema_err("struct I { int a, b; }; struct O { struct I in; } o = { .in = 1, 2, 3 };")
            .contains("excess elements in initializer")
    );
    assert!(sema_err(
        "struct I { int a, b; }; struct O { struct I in; int z; } o = { .in = 1, 2, .nope = 3 };"
    )
    .contains("no member named 'nope'"));
    sema_ok("int x; union U { int i; int *p; }; struct S { union U u; } s = { .u.p = &x };");
}

#[test]
fn generic_selection_and_compound_literals() {
    sema_ok("int main(void) { const int c = 1; return _Generic(c, int: 0, default: 1); }");
    sema_ok("int main(void) { int x; _Generic(x, int: x) = 1; return x; }");
    sema_ok("int *p = (int[]){1, 2}; struct P { int x; } *q = &(struct P){1};");
    sema_ok("int main(void) { int *p = &(int){1}; (int){2} = 3; return *p; }");
    assert!(sema_err("int a = _Generic(1, long: 1);")
        .contains("not compatible with any generic association type"));
    assert!(sema_err("int a = _Generic(1, int: 1, signed int: 2);")
        .contains("compatible with previously specified type"));
    assert!(sema_err("int a = _Generic(1, default: 1, default: 2);")
        .contains("duplicate default generic association"));
    assert!(sema_err("int a = _Generic(1, void: 1, default: 2);")
        .contains("not a complete object type"));
    assert!(sema_err("int x; int *p = (int[]){x};").contains("not a constant expression"));
    assert!(
        sema_err("int main(void) { static int *q = &(int){2}; return 0; }")
            .contains("not a constant expression")
    );
    assert!(sema_err("struct Q; int a = sizeof((struct Q){0});")
        .contains("compound literal has incomplete type"));
    assert!(sema_err("int main(void) { int (*f)(void) = &(int(void)){0}; }").contains("function"));
}

#[test]
fn generic_unselected_associations_are_checked() {
    // C11 6.5.1.1p3 does not evaluate unselected associations, but they
    // are still assignment-expressions and must satisfy constraints and
    // name resolution (5.1.1.3).
    assert!(sema_err(
        "int main(void) { int n = 2; return _Generic(n, int: 0, default: undeclared_name); }"
    )
    .contains("undeclared identifier 'undeclared_name'"));
    assert!(sema_err(
            "int one(int); int main(void) { int n = 2; return _Generic(n, int: 0, default: one(1, 2)); }"
        )
        .contains("expects 1 argument, found 2"));
    // Nested selection: the outer default is unselected, and its inner
    // default names an undeclared identifier.
    assert!(sema_err(
            "int main(void) { int a = 1, b = 2; return _Generic(a, int: 0, default: _Generic(b, long: 1, default: missing_inner)); }"
        )
        .contains("undeclared identifier 'missing_inner'"));

    // Valid unselected expressions may have a different type from the
    // selected one.
    sema_ok("int main(void) { int n = 1; return _Generic(n, int: 0, default: 3.0); }");
    sema_ok("int main(void) { int n = 1; const char *p = _Generic(n, int: \"ok\", default: 0); return p != 0; }");
    sema_ok("struct S { int m; }; int main(void) { int n = 1; struct S s; return _Generic(n, int: 0, struct S: s.m); }");

    // Unselected side effects are not lowered to IR (C11 6.5.1.1p3).
    let ir = crate::compile_to_ir(
            "int bump(void); int main(void) { int n = 0; return _Generic(n, int: 1, default: bump()); }",
            "generic-unselected.c",
        )
        .unwrap();
    assert!(
        !ir.contains("call") && ir.contains("ret i32 1"),
        "unselected call must not be emitted: {ir}"
    );
    let ir = crate::compile_to_ir(
            "int printf(const char *, ...); int main(void) { int n = 0; return _Generic(n, int: 1, default: printf(\"x\")); }",
            "generic-unselected-printf.c",
        )
        .unwrap();
    assert!(
        !ir.contains("call") && ir.contains("ret i32 1"),
        "unselected printf must not be emitted: {ir}"
    );

    // The parser folds `_Generic` in an integer constant expression
    // before semantic analysis, so these contexts must be checked there.
    assert!(
        sema_err("enum { N = _Generic(0, int: 1, default: missing) };")
            .contains("undeclared identifier")
    );
    assert!(
        sema_err("int a[_Generic(1, int: 2, default: missing)];").contains("undeclared identifier")
    );
    assert!(
        sema_err("struct S { int x : _Generic(0, int: 3, default: missing); };")
            .contains("undeclared identifier")
    );
    assert!(
        sema_err("_Static_assert(_Generic(0, int: 1, default: missing), \"m\");")
            .contains("undeclared identifier")
    );
    assert!(
        sema_err("int one(int); enum { N = _Generic(0, int: 1, default: one(1, 2)) };")
            .contains("argument")
    );
    sema_ok("enum { N = _Generic(0, int: 1, default: 2) };");
    sema_ok("int a[_Generic(1, int: 2, default: sizeof(int))];");
    sema_ok("int one(int); enum { N = _Generic(0, int: 1, default: one(1)) };");
    sema_ok("_Static_assert(_Generic(0, int: 1, default: 0), \"m\");");

    // Constant expressions that short-circuit, or that would be folded
    // away, still need their unselected associations checked.
    assert!(
        sema_err("enum { N = 1 || _Generic(0, int: 1, default: missing) };")
            .contains("undeclared identifier")
    );
    assert!(sema_err("enum { N = _Generic(0, int: 1, int: 2) };")
        .contains("compatible with a previously specified type"));
    assert!(sema_err("enum { N = _Generic(0, int: 1, default: *1) };").contains("invalid operand"));
    sema_ok("enum { N = _Generic(0, int: 1, default: __builtin_signbit(1.0)) };");
    assert!(
        sema_err("enum { N = _Generic(0, int: 1, default: (int[1]){[missing] = 0}) };",)
            .contains("undeclared identifier")
    );

    // __func__ is implicit only in a function (C11 6.4.2.2). A bare
    // supported builtin is not an ordinary identifier.
    assert!(
        sema_err("enum { N = _Generic(0, int: 1, default: __func__) };")
            .contains("undeclared identifier")
    );
    sema_ok("int f(void) { enum { N = _Generic(0, int: 1, default: __func__) }; return 0; }");
    assert!(
        sema_err("enum { N = _Generic(0, int: 1, default: __builtin_signbit) };")
            .contains("undeclared identifier")
    );
    sema_ok("int x; enum { N = _Generic(0, int: 1, default: ++_Generic(0, int: x)) };");
    assert!(
        sema_err("int y; enum { N = _Generic(0, int: 1, default: ++_Generic(0, int: 1)) };")
            .contains("invalid operand")
    );

    // C11 6.4.2.2: __func__ is `static const char [strlen(name)+1]`.
    // An ICE _Generic on its address must use that complete type.
    let ir = crate::compile_to_ir(
        "int f(void) { enum { N = _Generic(&__func__, char (*)[99]: 1, default: 2) }; return N; }",
        "generic-func-wrong.c",
    )
    .unwrap();
    assert!(
        ir.contains("ret i32 2"),
        "char (*)[99] must not match const char (*)[2]: {ir}"
    );
    let ir = crate::compile_to_ir(
            "int f(void) { enum { N = _Generic(&__func__, const char (*)[2]: 1, default: 2) }; return N; }",
            "generic-func-right.c",
        )
        .unwrap();
    assert!(
        ir.contains("ret i32 1"),
        "&__func__ in f must match const char (*)[2]: {ir}"
    );

    // Designator expressions must be typed before they are folded, so
    // `sizeof` in a designator is valid (selected or unselected).
    let ir = crate::compile_to_ir(
        "int main(void) { return (int[1]){[sizeof 0 - sizeof 0] = 7}[0]; }",
        "sizeof-designator.c",
    )
    .unwrap();
    assert!(
        ir.contains("store i32 7"),
        "sizeof in a selected designator must fold: {ir}"
    );
    let ir = crate::compile_to_ir(
            "int main(void) { return _Generic(0, int: 3, default: (int[1]){[sizeof 0 - sizeof 0] = 0}); }",
            "generic-sizeof-designator.c",
        )
        .unwrap();
    assert!(
        ir.contains("ret i32 3"),
        "unselected sizeof designator must not be evaluated: {ir}"
    );

    // ICE and runtime _Generic must agree on the const array type.
    let ir = crate::compile_to_ir(
            "int f(void) { enum { N = _Generic(&__func__, const char (*)[2]: 1, default: 0) }; return N != _Generic(&__func__, const char (*)[2]: 1, default: 0); }",
            "generic-func-agree.c",
        )
        .unwrap();
    assert!(
        !ir.contains("icmp ne i32 1, 0"),
        "ICE and runtime _Generic(&__func__) must agree: {ir}"
    );

    // The declared object is in scope in its initializer (C11 6.2.1p7).
    let ir = crate::compile_to_ir(
        "int main(void) { int a[1] = { [0 && sizeof a] = 7 }; return a[0]; }",
        "designator-self.c",
    )
    .unwrap();
    assert!(
        ir.contains("store i32 7"),
        "designator may name the object being initialized: {ir}"
    );
    let ir = crate::compile_to_ir(
        "int a[1] = { [0 && sizeof a] = 7 }; int main(void) { return a[0]; }",
        "designator-self-global.c",
    )
    .unwrap();
    assert!(
        ir.contains("store i32 7") || ir.contains("[1 x i32] [i32 7]"),
        "file-scope designator may name the object: {ir}"
    );
    // A block-scope extern may already have created the file symbol
    // and left it hidden. The later file-scope declarator still puts
    // the identifier in scope in its initializer (C11 6.2.1p7).
    let ir = crate::compile_to_ir(
            "int f(void) { extern int a[1]; return 0; } int a[1] = { [0 && sizeof a] = 7 }; int main(void) { return a[0]; }",
            "designator-self-global-after-extern.c",
        )
        .unwrap();
    assert!(
        ir.contains("store i32 7") || ir.contains("[1 x i32] [i32 7]"),
        "file-scope designator must see a previously hidden extern: {ir}"
    );
    // C11 6.2.7p4 forms a composite only when a prior declaration is
    // visible. After a block-scope `extern int a[1];`, a later
    // file-scope `int a[]` is visible in its initializer as `int[]`,
    // so `sizeof a` is a diagnostic (6.5.3.4p1). Clang `-std=c11`
    // rejects this the same way. The same designator is valid when
    // the declarator itself is complete (`int a[1]` above).
    assert!(sema_err(
        "int f(void) { extern int a[1]; return 0; } int a[] = { [0 && sizeof a] = 7 };"
    )
    .contains("sizeof applied to incomplete type"));
    // Plain `int a[]` with no prior declaration already diagnoses this.
    assert!(sema_err("int a[] = { [0 && sizeof a] = 7 };")
        .contains("sizeof applied to incomplete type"));
    // The initializer may complete the incomplete declarator. A hidden
    // `int[1]` does not shrink or reject a longer initializer, and a
    // hidden `int[3]` does not enlarge a shorter one.
    let ir = crate::compile_to_ir(
        "int f(void) { extern int a[1]; return 0; } int a[] = {7}; int main(void) { return a[0]; }",
        "hidden-extern-incomplete-init.c",
    )
    .unwrap();
    assert!(
        ir.contains("[1 x i32] [i32 7]"),
        "incomplete array is completed by its initializer: {ir}"
    );
    let ir = crate::compile_to_ir(
            "int f(void) { extern int a[1]; return 0; } int a[] = {1, 2, 3}; int main(void) { return a[2]; }",
            "hidden-extern-longer-init.c",
        )
        .unwrap();
    assert!(
        ir.contains("[3 x i32]"),
        "hidden int[1] must not bound a later int a[] initializer: {ir}"
    );
    let ir = crate::compile_to_ir(
            "int f(void) { extern int a[3]; return 0; } int a[] = {1, 2}; int main(void) { return a[1]; }",
            "hidden-extern-shorter-init.c",
        )
        .unwrap();
    assert!(
        ir.contains("[2 x i32]"),
        "hidden int[3] must not complete a later int a[] initializer: {ir}"
    );

    // An unselected file-scope compound literal must not leave a global.
    let ir = crate::compile_to_ir(
        "int x = _Generic(0, int: 1, default: (int){2});",
        "generic-unselected-compound.c",
    )
    .unwrap();
    assert!(
        !ir.contains("compoundliteral") && ir.contains("@x = global i32 1"),
        "unselected compound literal must not be emitted: {ir}"
    );
}

#[test]
fn non_prototype_declarations() {
    // C11 6.7.6.3p15: `int g();` is compatible with a prototype only if
    // no parameter type changes under the default argument promotions
    // and there is no ellipsis.
    sema_ok("int g(); int g(int, double); int main(void) { return g(1, 2.0f); }");
    sema_ok("int g(); int main(void) { return g('a', (short)1, 1.0f); }");
    // C11 6.7.6.3p15 makes a parameter-count mismatch with a `()`
    // definition incompatible. Clang accepts the conversion without a
    // diagnostic, and this compiler has no warnings, so it follows Clang
    // for pointers. A redeclaration with parameters is still rejected.
    sema_ok("int h() { return 1; } int (*p)(int) = h;");
    sema_ok("int d(); int (*q)(int) = d;");
    assert!(sema_err("int h() { return 1; } int h(int);").contains("conflicting types"));
    sema_ok("int k(int); int k(); int main(void) { return k(3); }");
    assert!(sema_err("int g(); int g(char);").contains("conflicting types for 'g'"));
    assert!(sema_err("int g(); int g(float);").contains("conflicting types for 'g'"));
    assert!(sema_err("int g(); int g(int, ...);").contains("conflicting types for 'g'"));
    assert!(sema_err("int g() { return 0; } int g(int);").contains("conflicting types"));
    assert!(sema_err("int g(); long g();").contains("conflicting types for 'g'"));
    // A prototype still checks its arguments after a non-prototype
    // declaration.
    assert!(
        sema_err("int k(); int k(int); int main(void) { return k(); }")
            .contains("expects 1 argument")
    );
}

#[test]
fn checks_overridden_union_initializers() {
    // C11 6.7.9: a store that a later designator for another union
    // member replaces is not part of the value, but its expression is
    // still checked, and in a static initializer it must be constant.
    let union = "union U { int a; int b; };";
    assert!(
        sema_err(&format!("{union} union U u = {{.a = missing, .b = 1}};"))
            .contains("undeclared identifier 'missing'")
    );
    assert!(sema_err(&format!(
        "{union} int main(void) {{ union U u = {{.a = missing, .b = 1}}; return u.b; }}"
    ))
    .contains("undeclared identifier 'missing'"));
    assert!(sema_err(&format!(
        "{union} int main(void) {{ static union U u = {{.a = missing, .b = 1}}; return u.b; }}"
    ))
    .contains("undeclared identifier 'missing'"));
    assert!(
        sema_err(&format!("{union} int g; union U u = {{.a = g, .b = 1}};"))
            .contains("not a constant expression")
    );
    assert!(sema_err(&format!(
        "{union} struct S {{ int x; }}; union U u = {{.a = (struct S){{1}}, .b = 1}};"
    ))
    .contains("incompatible"));
}

#[test]
fn rejects_invalid_programs() {
    assert!(sema_err("int main(void) { return x; }").contains("undeclared identifier 'x'"));
    assert!(sema_err("int main(void) { int x; int x; return 0; }").contains("redefinition of 'x'"));
    assert!(
        sema_err("int f(int x) { int x; return x; } int main(void) { return f(1); }")
            .contains("redefinition of 'x'")
    );
    assert!(sema_err("int f(int a, int a) { return a; }").contains("redefinition of parameter 'a'"));
    assert!(
        sema_err("int f(void) { return 1; } int f(void) { return 2; }")
            .contains("redefinition of 'f'")
    );
    assert!(
        sema_err("int f(int a) { return a; } int main(void) { return f(1, 2); }")
            .contains("expects 1 argument, found 2")
    );
    assert!(sema_err("int main(void) { return foo(1); }").contains("undeclared identifier 'foo'"));
    assert!(sema_err("int main(void) { int x; return x(1); }")
        .contains("called object of type 'int' is not a function"));
    assert!(
        sema_err("int f(void) { return 1; } int main(void) { return f; }")
            .contains("incompatible types")
    );
    assert!(
        sema_err("int main(void) { break; }").contains("'break' is not inside a loop or switch")
    );
    assert!(sema_err("int main(void) { continue; }").contains("'continue' is not inside a loop"));
    assert!(
        sema_err("void f(void) { return 1; } int main(void) { return 0; }")
            .contains("return with a value in a function returning 'void'")
    );
    assert!(sema_err("int main(void) { return; }")
        .contains("return with no value in a function returning 'int'"));
    assert!(sema_err("int main(void) { 1 = 2; }").contains("not assignable"));
    assert!(
        sema_err("void f(void) { return; } int main(void) { return f(); }")
            .contains("void value used where a value is required")
    );
    assert!(
        sema_err("int id(int x) { return x; } int main(void) { int id = 5; return id(1); }")
            .contains("is not a function")
    );
    assert!(
        sema_err("int add(int a) { return a; } int main(void) { return add(1, 2)(3); }")
            .contains("expects 1 argument")
    );
    assert!(sema_err("int f(int a); void f(int a);").contains("conflicting types for 'f'"));
    assert!(sema_err("int f(int a); int f(int a, int b);").contains("conflicting types for 'f'"));
    assert!(sema_err("int f; int f(void);").contains("different kind of symbol"));
    assert!(sema_err("int f(void); int f;").contains("different kind of symbol"));
    assert!(sema_err("int f(void); static int f(void) { return 1; }")
        .contains("static declaration of 'f' follows a non-static declaration"));
    assert!(sema_err("static int g; int g;")
        .contains("non-static declaration of 'g' follows a static declaration"));
    assert!(sema_err("int g = 1; int g = 1;").contains("redefinition of 'g'"));
    assert!(sema_err("extern int g = 1; int g = 2;").contains("redefinition of 'g'"));
    assert!(sema_err("int g = n;").contains("undeclared identifier 'n'"));
    assert!(
        sema_err("int n; int g = n;").contains("initializer of 'g' is not a constant expression")
    );
    assert!(sema_err("int f(void); int g = f();")
        .contains("initializer of 'g' is not a constant expression"));
    assert!(sema_err("int g = 1 / 0;").contains("initializer of 'g' divides by zero"));
    assert!(sema_err("int g = 1 && (1 / 0);").contains("divides by zero"));
    assert!(sema_err("int g = 2147483647 + 1;").contains("initializer of 'g' overflows 'int'"));
    assert!(sema_err("int g = 2147483647 * 2;").contains("overflows 'int'"));
    assert!(
        sema_err("int g = (-2147483647 - 1) / -1;").contains("initializer of 'g' overflows 'int'")
    );
    assert!(sema_err("int g = (-2147483647 - 1) % -1;").contains("overflows 'int'"));
    assert!(sema_err("int g = -(-2147483647 - 1);").contains("overflows 'int'"));
    assert!(
        sema_err("int f(int a, int b); int main(void) { return f(1); }")
            .contains("expects 2 arguments, found 1")
    );
    assert!(sema_err("int f(int a, int a);").contains("redefinition of parameter 'a'"));
    assert!(
        sema_err("static int missing(void); int main(void) { return missing(); }")
            .contains("static function 'missing' is used but not defined")
    );
    assert!(sema_err("int f(int a); int main(void) { return a; }")
        .contains("undeclared identifier 'a'"));
    assert!(
        sema_err("int main(void) { const int x = 1; x = 2; return x; }")
            .contains("cannot assign to const-qualified type 'const int'")
    );
    assert!(
        sema_err("int main(void) { const char *p = \"a\"; *p = 'b'; return 0; }")
            .contains("const-qualified type 'const char'")
    );
    assert!(
        sema_err("int main(void) { char *const p = 0; p = 0; return 0; }")
            .contains("const-qualified type 'char * const'")
    );
    assert!(
        sema_err("int main(void) { const char s[] = \"hi\"; s[0] = 'a'; return 0; }")
            .contains("const-qualified type 'const char'")
    );
    assert!(
        sema_err("void g(char *); int main(void) { const char *p = 0; g(p); return 0; }")
            .contains("incompatible types assigning 'const char *' to 'char *'")
    );
    assert!(
        sema_err("void h(const char **); int main(void) { char **p = 0; h(p); return 0; }")
            .contains("incompatible types assigning 'char **' to 'const char **'")
    );
    assert!(sema_err("char d[1] = \"hi\"; int main(void) { return 0; }")
        .contains("initializer string is too long"));
    assert!(sema_err("int a[2] = \"hi\";").contains("string literal cannot initialize"));
    assert!(sema_err("int main(void) { printf(\"hi\\n\"); return 0; }")
        .contains("undeclared identifier 'printf'"));
    assert!(sema_err("int f(int, ...); int f(int);").contains("conflicting types"));
    assert!(sema_err("const int f(void); int f(void) { return 1; }").contains("conflicting types"));
    assert!(
        sema_err("int printf(const char *, ...); int main(void) { return printf(); }")
            .contains("expects at least 1 argument, found 0")
    );
    assert!(
        sema_err("int main(void) { int a = 1; static int b = a * 2; return b; }")
            .contains("initializer of 'b' is not a constant expression")
    );
    assert!(sema_err("int main(void) { static int a[]; return 0; }")
        .contains("array size missing in 'a'"));
    assert!(
        sema_err("int main(void) { static int x; static int x; return x; }")
            .contains("redefinition of 'x'")
    );
    assert!(
        sema_err("int main(void) { int x; static int x; return x; }")
            .contains("redefinition of 'x'")
    );
    assert!(
        sema_err("int main(void) { static int x; extern int x; return x; }")
            .contains("redefinition of 'x'")
    );
    assert!(
        sema_err("int main(void) { int x; extern int x; return x; }")
            .contains("redefinition of 'x'")
    );
    assert!(
        sema_err("int main(void) { extern int x; int x; return x; }")
            .contains("redefinition of 'x'")
    );
    assert!(sema_err("int f(int i) { extern int i; return i; }").contains("redefinition of 'i'"));
    assert!(sema_err("int main(void) { extern int x = 1; return x; }")
        .contains("cannot have an initializer"));
    assert!(
        sema_err("int main(void) { extern int x; extern long x; return x; }")
            .contains("conflicting types for 'x'")
    );
    assert!(
        sema_err("int x; int main(void) { extern char x; return 0; }")
            .contains("conflicting types for 'x'")
    );
    assert!(
        sema_err("static int x; int main(void) { int x; { extern int x; return x; } }")
            .contains("non-static declaration of 'x' follows a static declaration")
    );
    assert!(
        sema_err("int main(void) { static int foo(void); return foo(); }")
            .contains("cannot have 'static' storage class")
    );
    assert!(
        sema_err("int foo(void); int main(void) { extern int foo; return 0; }")
            .contains("different kind of symbol")
    );
    assert!(
        sema_err("int foo; int main(void) { int foo(void); return 0; }")
            .contains("different kind of symbol")
    );
    assert!(
        sema_err("int main(void) { int foo(void); int foo = 1; return foo; }")
            .contains("different kind of symbol")
    );
    assert!(
        sema_err("int main(void) { int foo = 1; int foo(void); return foo; }")
            .contains("different kind of symbol")
    );
    assert!(sema_err(
        "int main(void) { int foo(int a); return foo(1); } int foo(void) { return 1; }"
    )
    .contains("conflicting types for 'foo'"));
    assert!(sema_err(
        "int main(void) { int foo(void); return foo(); } static int foo(void) { return 1; }"
    )
    .contains("static declaration of 'foo' follows a non-static declaration"));
    assert!(sema_err("int f(void) { { extern int a; } return a; }")
        .contains("undeclared identifier 'a'"));
    assert!(
        sema_err("int main(void) { do { int a = 1; } while (a < 2); return 0; }")
            .contains("undeclared identifier 'a'")
    );
    assert!(
        sema_err("void g(void) { return; } int main(void) { do ; while (g()); return 0; }")
            .contains("void value used where a value is required")
    );
}

#[test]
fn static_extern_and_block_functions_are_in_scope() {
    sema_ok("int main(void) { static int x = 1; return x; }");
    sema_ok("int main(void) { static int x; return x; }");
    sema_ok("static int x = 1; int main(void) { extern int x; return x; }");
    sema_ok("int x = 1; int main(void) { int x = 2; { extern int x; return x; } }");
    sema_ok("static int f(void) { return 1; } int main(void) { int f(void); return f(); }");
    sema_ok("int main(void) { int g(int); return g(1); } int g(int x) { return x; }");
    sema_ok("int main(void) { extern int g(int); extern int g(int); return 0; }");
    sema_ok("int main(void) { do { break; } while (1); return 0; }");
    sema_ok("int main(void) { static int a[] = {1, 2}; return a[1]; }");
    sema_ok("int main(void) { static int x; static int *p = &x; return *p; }");
}

#[test]
fn strings_const_and_varargs_are_accepted() {
    sema_ok("void f(const char *); int main(void) { char *p = \"hi\"; f(p); return 0; }");
    sema_ok("int f(const int x); int f(int x) { return x; }");
    sema_ok("int f(int *const p); int f(int *p) { return 0; }");
    sema_ok("int main(void) { volatile int x = 1; x = 2; return x; }");
    sema_ok("int main(void) { return sizeof \"abc\" + sizeof \"\" + sizeof \"a\\0b\"; }");
    let unit = analyzed("char s[] = \"hi\"; char t[2] = \"hi\"; char *p = \"ab\" \"c\";");
    let bytes = |name: &str| match &unit.globals.iter().find(|g| g.name == name).unwrap().init {
        Some(ConstInit::Aggregate(items)) => items
            .iter()
            .map(|item| match item {
                ConstInit::Int(bits) => *bits as u8,
                ConstInit::Zero => 0,
                _ => 0xff,
            })
            .collect::<Vec<_>>(),
        _ => panic!("{name}"),
    };
    assert_eq!(bytes("s"), b"hi\0");
    assert_eq!(bytes("t"), b"hi");
    assert!(unit.strings.iter().any(|s| s.bytes == b"abc\0"));
}

#[test]
fn accepts_shadowing_and_recursion() {
    sema_ok(
        "\
int main(void) {
    int x = 1;
    {
        int x = 2;
        return x;
    }
}
",
    );
    sema_ok(
        "\
int f(int n) {
    if (n == 0)
        return 1;
    return n + f(n - 1);
}
int main(void) { return f(1); }
",
    );
    assert!(
        sema_err("int main(void) { return later(1); } int later(int x) { return x; }")
            .contains("undeclared identifier 'later'")
    );
}

fn analyzed(source: &str) -> TranslationUnit {
    let tokens = lex(source, "ok.c").unwrap();
    let mut unit = parse(&tokens, "ok.c").unwrap();
    analyze(&mut unit, "ok.c").unwrap();
    unit
}

#[test]
fn folds_globals_and_records_declarations() {
    let unit = analyzed(
        "\
int g = 1 + 2 * 3;
int zero;
extern int missing;
static int hidden = 4;
int h = 0 && (1 / 0);
int m = -2147483647 - 1;
int d = -5 / 2;
int s = -5 % 2;
int quiet = 0 && (2147483647 + 1);
",
    );
    let value = |name: &str| {
        unit.globals
            .iter()
            .find(|global| global.name == name)
            .unwrap()
            .int_init()
    };
    assert_eq!(value("g"), Some(7));
    assert_eq!(value("zero"), Some(0));
    assert_eq!(value("missing"), None);
    assert_eq!(value("h"), Some(0));
    assert_eq!(value("m"), Some(i32::MIN as u32 as u128));
    assert_eq!(value("d"), Some((-2i32) as u32 as u128));
    assert_eq!(value("s"), Some((-1i32) as u32 as u128));
    assert_eq!(value("quiet"), Some(0));
    let hidden = unit
        .globals
        .iter()
        .find(|global| global.name == "hidden")
        .unwrap();
    assert_eq!(hidden.linkage, Linkage::Internal);
    assert_eq!(hidden.int_init(), Some(4));

    let forward = analyzed(
        "\
int add(int a, int b);
int add(int x, int y) { return x + y; }
static int id(void);
int id(void) { return 1; }
int only(int);
",
    );
    assert_eq!(forward.func_decls.len(), 1);
    assert_eq!(forward.func_decls[0].name, "only");
    assert_eq!(forward.func_decls[0].params, vec![CType::int_type()]);
    match &forward.decls[2] {
        TopDecl::Func(func) => assert_eq!(func.linkage, Linkage::Internal),
        TopDecl::Global(_) => panic!("expected the static function"),
    }
    sema_ok("static int unused(void); int main(void) { return 0; }");
    sema_ok(
        "\
int is_even(int n);
int is_odd(int n);
int is_even(int n) { if (n == 0) return 1; return is_odd(n - 1); }
int is_odd(int n) { if (n == 0) return 0; return is_even(n - 1); }
",
    );
}

#[test]
fn folds_bitwise_shifts_sizeof_and_conditional() {
    let unit = analyzed(
        "\
int bits = (6 & 3) | (1 ^ 4);
int tilde = ~0;
int shl = 1 << 4;
int shr = -8 >> 1;
int wide = (-2147483647 - 1) >> 1;
int pick = 1 ? 7 : 1 / 0;
int other = 0 ? 1 / 0 : 5;
int quiet_or = 1 || (1 / 0);
int sz = sizeof(int);
int sz_expr = sizeof(1 / 0);
int self_sz = sizeof(self_sz);
int nested = sizeof sizeof(int);
int n;
int uneval_inc = 0 ? n++ : 9;
int skipped = 0 && (n = 3);
int foo(void);
int uneval_call = 1 ? 4 : foo();
",
    );
    let value = |name: &str| {
        unit.globals
            .iter()
            .find(|global| global.name == name)
            .unwrap_or_else(|| panic!("missing {name}"))
            .int_init()
    };
    assert_eq!(value("bits"), Some((6 & 3) | (1 ^ 4)));
    assert_eq!(value("tilde"), Some((-1i32) as u32 as u128));
    assert_eq!(value("shl"), Some(16));
    assert_eq!(value("shr"), Some((-4i32) as u32 as u128));
    assert_eq!(value("wide"), Some((i32::MIN >> 1) as u32 as u128));
    assert_eq!(value("pick"), Some(7));
    assert_eq!(value("other"), Some(5));
    assert_eq!(value("quiet_or"), Some(1));
    assert_eq!(value("sz"), Some(4));
    assert_eq!(value("sz_expr"), Some(4));
    assert_eq!(value("self_sz"), Some(4));
    assert_eq!(value("nested"), Some(8));
    assert_eq!(value("uneval_inc"), Some(9));
    assert_eq!(value("skipped"), Some(0));
    assert_eq!(value("uneval_call"), Some(4));

    assert!(sema_err("int g = (1, 2);").contains("not a constant expression"));
    assert!(sema_err("int n; int g = n++;").contains("not a constant expression"));
    assert!(sema_err("int n; int g = ++n;").contains("not a constant expression"));
    assert!(sema_err("int n; int g = n = 1;").contains("not a constant expression"));
    assert!(sema_err("int n; int g = n += 1;").contains("not a constant expression"));
    assert!(sema_err("int f(void); int g = f();").contains("not a constant expression"));
    assert!(sema_err("int n; int g = 1 ? n++ : 2;").contains("not a constant expression"));
    assert!(sema_err("int g = 1 << 31;").contains("overflows 'int'"));
    assert!(sema_err("int g = 1 << 32;").contains("undefined shift"));
    assert!(sema_err("int g = 1 << -1;").contains("undefined shift"));
    assert!(sema_err("int g = -1 << 1;").contains("undefined shift"));
    assert!(sema_err("int g = 8 >> -1;").contains("undefined shift"));
    assert!(sema_err("int g = 8 >> 32;").contains("undefined shift"));
    assert!(sema_err("int g = 0 & (1 / 0);").contains("divides by zero"));
    assert!(sema_err("int g = 0 ? (1 / 0) : 1 / 0;").contains("divides by zero"));
    assert!(sema_err("int main(void) { return ++1; }").contains("not assignable"));
    assert!(sema_err("int main(void) { int x; x++ ++; return x; }").contains("not assignable"));
    assert!(sema_err("int main(void) { int x; ++ ++x; return x; }").contains("not assignable"));
    assert!(
        sema_err("int main(void) { int x; (x + 1) += 2; return x; }").contains("not assignable")
    );
    assert!(
        sema_err("int main(void) { int a; int b; int c; int d; a ? b : c = d; return 0; }")
            .contains("not assignable")
    );
    assert!(
        sema_err("void f(void); int main(void) { return 1 ? f() : 2; }")
            .contains("incompatible operand types")
    );
    assert!(sema_err("int main(void) { return sizeof(void); }").contains("incomplete type 'void'"));
    assert!(
        sema_err("void f(void); int main(void) { return sizeof(f()); }")
            .contains("incomplete type 'void'")
    );
    sema_ok("void f(void); void g(void); void h(int c) { c ? f() : g(); }");
    sema_ok("void f(void); int main(void) { return (f(), 3); }");
    sema_ok("int main(void) { int x; x = 1; x += 2; ++x; x++; return sizeof(x++); }");
}

#[test]
fn integer_types_agree_and_fold() {
    assert!(sema_err("char g; signed char g;").contains("conflicting types for 'g'"));
    assert!(sema_err("int g; unsigned g;").contains("conflicting types for 'g'"));
    assert!(sema_err("int f(long); int f(long long);").contains("conflicting types"));
    assert!(sema_err("int f(char); int f(signed char);").contains("conflicting types"));
    sema_ok("int g; signed int g; signed g;");
    sema_ok("unsigned u; unsigned int u;");
    sema_ok("long l; long int l; signed long l;");
    sema_ok("unsigned long long u; unsigned long long int u;");
    sema_ok("int f(int); int f(signed); int f(signed int x) { return x; }");
    sema_ok("char f(char c); char f(char x) { return x; }");

    let unit = analyzed(
        "\
int cmp = -1 < 1u;
int cmp_long = -1L < 1u;
int cmp_signed = -1 < 1;
char c = 1000;
unsigned char uc = -1;
unsigned u = 1u << 31;
unsigned w = 4294967295u + 1u;
int nested = sizeof(sizeof(char));
unsigned long sz = sizeof(int);
long wide = 1L << 40;
unsigned long cond_ul = 1 ? -1 : 1u;
int cond_cmp = (1 ? -1 : 1u) > 0;
",
    );
    let value = |name: &str| {
        unit.globals
            .iter()
            .find(|global| global.name == name)
            .unwrap_or_else(|| panic!("missing {name}"))
            .int_init()
    };
    assert_eq!(value("cmp"), Some(0));
    assert_eq!(value("cmp_long"), Some(1));
    assert_eq!(value("cmp_signed"), Some(1));
    assert_eq!(value("c"), Some(232));
    assert_eq!(value("uc"), Some(255));
    assert_eq!(value("u"), Some(2147483648));
    assert_eq!(value("w"), Some(0));
    assert_eq!(value("nested"), Some(8));
    assert_eq!(value("sz"), Some(4));
    assert_eq!(value("wide"), Some(1u128 << 40));
    assert_eq!(value("cond_ul"), Some(4294967295));
    assert_eq!(value("cond_cmp"), Some(1));

    assert!(sema_err("long g = 1L << 63;").contains("overflows 'long'"));
    sema_ok("unsigned long g = 1ul << 63;");
    assert!(sema_err("int g = 1 << 40;").contains("undefined shift"));
    sema_ok("long g = 1L << 40;");
    assert!(sema_err("int g = 2147483647 + 1;").contains("overflows 'int'"));
    sema_ok("char g = 1000;");
}

#[test]
fn pointers_and_arrays() {
    sema_ok(
        "\
int g;
int *p = &g;
int a[] = {1, 2, 3};
int *q = &a[1];
int *r = a + 1;
int main(void) {
    int x = 1;
    int *lp = &x;
    *lp = 2;
    return a[0] + *q + (p != 0);
}
",
    );
    assert!(
        sema_err("int main(void) { int *p; char *c; p = c; return 0; }")
            .contains("incompatible types assigning 'char *' to 'int *'")
    );
    assert!(sema_err("int main(void) { void *v; v = v + 1; return 0; }")
        .contains("arithmetic on a pointer to void"));
    assert!(sema_err("int main(void) { int a[]; return 0; }").contains("array size missing"));
    assert!(sema_err("int a[2] = {1, 2, 3};").contains("excess elements in array initializer"));
    assert!(
        sema_err("int main(void) { int *p; return p < 0; }").contains("invalid operands to '<'")
    );
    assert!(sema_err("int *p = 1;").contains("incompatible types"));
    sema_ok("int *p = 0; int *q = (void *)0;");
    sema_ok("void *v; int *p; void id(void) { v = p; p = v; }");
    assert!(sema_err("int main(void) { int a[3]; a = a; return 0; }")
        .contains("array type is not assignable"));
    assert!(sema_err("int main(void) { return &1; }").contains("cannot take the address"));
    assert!(sema_err("int g; int x = (int)&g;").contains("truncates an address"));
    assert!(sema_err("int g; long x = (int)&g;").contains("truncates an address"));
    sema_ok("int g; long x = (long)&g;");
    // `_Bool` from an address constant: 1 unless null, but a weak symbol's
    // address may be null at run time, as Clang and GCC also reject.
    sema_ok("int g; _Bool b = &g; _Bool n = (void *)0; int a[2]; _Bool c = &1[a];");
    assert!(
        sema_err("extern int w __attribute__((weak)); _Bool b = &w;")
            .contains("not a constant expression")
    );
    // An explicit `(_Bool)` cast is the same truth value, not a truncation.
    sema_ok("int g; _Bool b = (_Bool)&g; int i = (_Bool)&g; _Bool n = (_Bool)(void *)0;");
    assert!(
        sema_err("extern int w __attribute__((weak)); _Bool b = (_Bool)&w;")
            .contains("not a constant expression")
    );
    assert!(sema_err("int g; char c = (char)&g;").contains("truncates an address"));
    sema_ok("int g; unsigned long x = (unsigned long)&g;");
    // Wider than a pointer: no `ptrtoint` of matching width can be emitted,
    // and Clang rejects each of these.
    for src in [
        "int g; __int128 x = (__int128)&g;",
        "int g; __int128 x = (long)&g;",
        "int g; unsigned __int128 x = (unsigned long)&g;",
        "int g; long x = (__int128)&g;",
        "int g; struct T { __int128 m; } t = { (long)&g };",
        "int g; union U { __int128 m; } u = { (long)&g };",
    ] {
        assert!(
            sema_err(src).contains("integer wider than a pointer"),
            "{src}"
        );
    }
    assert!(sema_err("int main(void) { int (*p)[]; return p[0][0]; }").contains("incomplete type"));
}

#[test]
fn struct_layout_members_and_diagnostics() {
    let unit = analyzed(
        "\
struct S { char c; int i; long l; };
struct S g = {1, 2, 3};
struct Node { int val; struct Node *next; };
struct Node n = {4, 0};
",
    );
    let s = unit
        .structs
        .iter()
        .find(|handle| handle.tag() == "S")
        .expect("struct S");
    assert_eq!(s.size(), Some(16));
    assert_eq!(s.align(), 8);
    let fields = s.fields().expect("complete");
    assert_eq!(fields[0].name, "c");
    assert_eq!(fields[0].offset, 0);
    assert_eq!(fields[1].name, "i");
    assert_eq!(fields[1].offset, 4);
    assert_eq!(fields[2].offset, 8);
    assert!(s.field("missing").is_none());
    assert_eq!(s.field("i").unwrap().1.offset, 4);
    let node = unit
        .structs
        .iter()
        .find(|handle| handle.tag() == "Node")
        .expect("struct Node");
    assert_eq!(node.size(), Some(16));
    assert!(node.field("next").unwrap().1.ty.is_pointer());
    let g = unit
        .globals
        .iter()
        .find(|global| global.name == "g")
        .unwrap();
    match &g.init {
        Some(ConstInit::Aggregate(items)) => {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], ConstInit::Int(1));
            assert_eq!(items[1], ConstInit::Int(2));
            assert_eq!(items[2], ConstInit::Int(3));
        }
        other => panic!("expected aggregate, got {other:?}"),
    }

    sema_ok("struct S { int x; }; void f(struct S s) { (void)s; }");
    sema_ok("struct S { int x; }; int f(struct S);");
    assert!(sema_err("struct S; void f(struct S s) { (void)s; }")
        .contains("parameter has incomplete type 'struct S'"));
    assert!(
        sema_err("struct S; struct S f(void) { struct S s; return s; }")
            .contains("incomplete type")
    );
    assert!(sema_err("struct S { struct S { int x; } in; int y; };")
        .contains("redefinition of struct 'S'"));
    assert!(sema_err(
        "struct S { int x; }; int main(void) { struct S s; struct S t; (s = t).x = 1; return 0; }"
    )
    .contains("expression is not assignable"));
    sema_ok("struct S { int x; }; struct S f(void) { struct S s; return s; }");
    assert!(sema_err("struct S; struct S s;").contains("incomplete type 'struct S'"));
    sema_ok("struct S; extern struct S x; int main(void) { return &x == 0; }");
    assert!(
        sema_err("struct S; struct S *p; int main(void) { *p; return 0; }")
            .contains("cannot use incomplete type 'struct S' as a value")
    );
    assert!(
        sema_err("int main(void) { struct S *p = 0; *p; return 0; }")
            .contains("cannot use incomplete type 'struct S' as a value")
    );
    assert!(sema_err(
        "struct S; extern struct S x = {1}; int main(void) { return 0; } struct S { int a; };"
    )
    .contains("incomplete type 'struct S'"));
    sema_ok("struct S; struct S *p; int main(void) { p = 0; return &*p == p; }");
    assert!(
        sema_err("struct S; int main(void) { return sizeof(struct S); }")
            .contains("incomplete type 'struct S'")
    );
    assert!(
        sema_err("struct S; int main(void) { struct S *p; return p->x; }")
            .contains("incomplete type 'struct S'")
    );
    assert!(
        sema_err("struct S { int x; }; int main(void) { struct S s; return s.y; }")
            .contains("no member named 'y'")
    );
    assert!(sema_err("struct S { int x; }; struct S { int y; };")
        .contains("redefinition of struct 'S'"));
    assert!(sema_err("struct S { int x; int x; };").contains("duplicate member"));
    sema_ok("struct S { int n; int a[]; }; int main(void) { return (int)sizeof(struct S); }");
    assert!(sema_err("struct S { int a[]; };").contains("another named member"));
    assert!(sema_err("struct S { int a[]; int n; };").contains("not the last member"));
    assert!(sema_err("union U { int n; int a[]; };").contains("not allowed in a union"));
    assert!(sema_err("struct F { int n; int a[]; }; struct F arr[2];").contains("array element"));
    assert!(
        sema_err("struct F { int n; int a[]; }; struct O { struct F f; };")
            .contains("struct member")
    );
    assert!(
        sema_err("struct S { int n; int a[]; }; struct S s = {1, {2}};")
            .contains("flexible array member")
    );
    assert!(sema_err(
        "struct S { int x:3; }; int main(void) { struct S s; int *p = &s.x; return *p; }"
    )
    .contains("address of a bit-field"));
    sema_ok("struct { int x; } s; int main(void) { s.x = 1; return s.x; }");
    assert!(sema_err("struct S {};").contains("no members"));
    assert!(
        sema_err("struct S { int x; int y; }; struct S s = { .z = 1 };")
            .contains("no member named 'z'")
    );
    assert!(sema_err(
        "struct S { const int x; int y; }; void f(struct S *a, struct S *b) { *a = *b; }"
    )
    .contains("const member"));
    assert!(
        sema_err("struct S { int x; }; int main(void) { struct S s; s++; return 0; }")
            .contains("++ and -- require a scalar")
    );
    assert!(
        sema_err("void f(struct A *p); void f(struct A *p);").contains("conflicting types for 'f'")
    );
    assert!(
        sema_err("struct S { int x; int y; }; struct S s = {1, 2, 3};")
            .contains("excess elements in struct initializer")
    );
    sema_ok(
        "\
struct S { int x; };
void f(struct S *p);
void f(struct S *p) { p->x = 1; }
",
    );
}

#[test]
fn typedef_enum_union_diagnostics() {
    sema_ok(
        "\
typedef int T;
typedef T T;
enum E { A = 1, B };
union U { int x; char c; };
int main(void) { T t = A; union U u = {t}; return u.x + B; }
",
    );
    assert!(sema_err("typedef int T; typedef long T;").contains("redefinition of 'T'"));
    assert!(sema_err("enum E x;").contains("incomplete enum"));
    assert!(sema_err("restrict int x;").contains("restrict requires a pointer"));
    sema_ok("int main(void) { float f; f = 1; return f == 1.0f; }");
    sema_ok("int main(void) { return 1.0L; }");
    sema_ok(
            "enum { N = (int)3.9 }; int main(void) { int a[(int)2.9]; return N == 3 && sizeof(a) == 2 * sizeof(int); }",
        );
    assert!(sema_err("enum { N = 1.5 };").contains("not an integer constant"));
    assert!(sema_err("enum { N = (int)-1.5 };").contains("not an integer constant"));
    sema_ok("extern long double x; void f(void) { for (;; x++) {} }");
    sema_ok("int main(void) { int *p = (int)0.0; switch (1) { case (int)1.5: return p == 0; } return 1; }");
    sema_ok("int main(void) { long double x; x = 1.0; return 0; }");
    sema_ok("long double f(long double x) { return x; }");
    assert!(sema_err("union U { int x; char c; }; union U u = {1, 2};").contains("excess elements"));
    assert!(
        sema_err("struct S; struct S f(void); int main(void) { return f().x; }")
            .contains("incomplete type")
    );
    sema_ok("struct S { int x; }; struct S f(void); int main(void) { return f().x; }");
    sema_ok("struct S { double d; }; void f(struct S s) { (void)s; }");
    sema_ok("struct S { float a; float b; }; struct S f(struct S s) { return s; }");
    sema_ok("struct S { long double x; }; void f(struct S s) { (void)s; }");
    sema_ok("struct S { long double x; }; struct S f(void) { struct S s; return s; }");
    sema_ok("int f(void) __asm__(\"g\"); int f(void) { return 1; }");
    sema_ok("int f(void) __asm__(u8\"g\"); int f(void) { return 1; }");
    assert!(sema_err("int f(void) __asm__(u\"g\");").contains("byte string"));
    let ir = crate::compile_to_ir(
        "struct LD { long double x; }; volatile struct LD v; struct LD f(void) { return v; }\n",
        "volld.c",
    )
    .unwrap();
    assert!(ir.contains("load volatile x86_fp80"), "{ir}");
    assert!(
        sema_err("int f(int); int main(void) { int (*p)(char); p = f; return 0; }")
            .contains("incompatible types")
    );
    assert!(sema_err(
        "int f(int); int main(void) { int (*p)(int) = f; int (*q)(int) = f; return p < q; }"
    )
    .contains("relational comparison of function pointers"));
    sema_ok("int f(int); int main(void) { int (*p)(int) = f; int (*q)(int) = f; return p == q; }");
    assert!(sema_err("int main(void) { int x; return x(1); }").contains("not a function"));
    assert!(sema_err("enum E { A = 1, A = 2 };").contains("redefinition of 'A'"));
    assert!(
        sema_err("static int f(void); int main(void) { int (*p)(void) = f; return 0; }")
            .contains("static function 'f' is used but not defined")
    );
    let ir =
        crate::compile_to_ir("int g; union U { int *p; long l; } u = { &g };\n", "ua.c").unwrap();
    assert!(ir.contains("@u = global <{ ptr }> <{ ptr @g }>"), "{ir}");
    assert!(sema_err("struct S { int *p : 3; };").contains("non-integral"));
    assert!(sema_err("struct S { int x : 4294967297L; };").contains("exceeds the width"));
    sema_ok("struct A { int a : 3; }; struct O { struct A in; } g = {{1}};");
    sema_ok("struct A { int a : 3; int b : 5; }; struct A arr[2] = {{1, 2}, {3, 4}};");
    sema_ok("union U { char b; long a; } g = { .a = 0x1234 };");
    sema_ok("union U { double d; long l; } g = { 1.5 };");
    let ir = crate::compile_to_ir(
        "volatile struct S { int a; } v; struct S x; void f(void) { x = v; }\n",
        "vol.c",
    )
    .unwrap();
    assert!(
        ir.contains("call void @llvm.memmove.p0.p0.i64(ptr @x, ptr @v, i64 4, i1 true)"),
        "{ir}"
    );
    sema_ok("union U { int *p; long l; } u = { 0 };");
    sema_ok("union U { int *p; long l; } u = { (int *)1 };");
    sema_ok("struct S { int a; union { char *s; long n; } v; } g = { 1 };");
    sema_ok("struct T { int a; }; int main(void) { union T; int x = 1; return x; }");
    assert!(sema_err("struct T { int a; }; void g(union T *p);").contains("different tag type"));
    assert!(sema_err("struct T { int a; union T *p; };").contains("different tag type"));
    sema_ok("int main(void) { int *p = 0; _Bool b = p; return b; }");
    let ir = crate::compile_to_ir(
        "#include <stdarg.h>\nint f(va_list ap) { return va_arg(ap, int); }\n",
        "va.c",
    )
    .unwrap();
    assert!(ir.contains("phi ptr"), "{ir}");
}

#[test]
fn deep_union_diamond_still_initializes() {
    // Each union has two members of the previous union. Searching both
    // at every level is exponential; one visit per record stays linear.
    let depth = 16;
    let mut source = String::from("union U0 { int i; };\n");
    for n in 1..=depth {
        source.push_str(&format!(
            "union U{n} {{ union U{} a; union U{} b; }};\n",
            n - 1,
            n - 1
        ));
    }
    source.push_str(&format!(
            "struct O {{ union U{depth} u; int n; }};\nstruct O g = {{ .n = 3 }};\nint main(void) {{ return g.n; }}\n"
        ));
    sema_ok(&source);
}

#[test]
fn switch_diagnostics() {
    sema_ok(
        "\
enum E { A = 1, B = 4 };
int main(void) {
    int x = 1;
    int y = 0;
    char c = 1;
    unsigned char uc = 1;
    unsigned u = 1;
    switch (x) {
        int skipped = (y = 5);
    case 0:
        y = skipped;
        break;
    case A:
    case 'B':
    case sizeof(int):
    case (short)-2:
    case (unsigned char)3:
        return y;
    default:
        break;
    }
    switch (c) {
    case 256:
        return 1;
    case 1:
        return 2;
    }
    switch (uc) {
    case -1:
        return 3;
    case 255:
        return 4;
    }
    switch (u) {
    case -1:
        return 5;
    }
    switch (x) {
    case 1:
        while (0) {
            continue;
            break;
        }
        break;
    }
    do { switch (x) { case 1: continue; } } while (0);
    return 0;
}
",
    );
    assert!(sema_err("int main(void) { case 1: return 0; }")
        .contains("'case' label is not inside a switch"));
    assert!(sema_err("int main(void) { default: return 0; }")
        .contains("'default' label is not inside a switch"));
    assert!(sema_err("int main(void) { while (0) case 1: return 0; }")
        .contains("'case' label is not inside a switch"));
    assert!(sema_err(
        "int main(void) { int x; switch (x) { case 1: return 0; case 1: return 1; } }"
    )
    .contains("duplicate case value '1'"));
    assert!(sema_err(
        "int main(void) { int x; switch (x) { case -1: return 0; case 4294967295u: return 1; } }"
    )
    .contains("duplicate case value '-1'"));
    assert!(sema_err(
        "int main(void) { int x; switch (x) { case 0: return 0; case 17179869184: return 1; } }"
    )
    .contains("duplicate case value '0'"));
    assert!(sema_err(
        "int main(void) { long x; switch (x) { case 100: return 0; case 100l: return 1; } }"
    )
    .contains("duplicate case value '100'"));
    assert!(sema_err(
            "int main(void) { unsigned x; switch (x) { case 4294967295u: return 0; case 1099511627775l: return 1; } }"
        )
        .contains("duplicate case value '4294967295'"));
    assert!(sema_err(
        "int main(void) { int x; switch (x) { default: return 0; default: return 1; } }"
    )
    .contains("multiple default labels"));
    assert!(sema_err(
        "int main(void) { int x; switch (x) { case 1: if (x) { case 1: return 0; } return 1; } }"
    )
    .contains("duplicate case value '1'"));
    assert!(sema_err(
        "int main(void) { int x; switch (x) { default: while (0) default: ; return 0; } }"
    )
    .contains("multiple default labels"));
    assert!(
        sema_err("int main(void) { int *p = 0; switch (p) { default: return 0; } }")
            .contains("expected an integer")
    );
    assert!(sema_err(
        "int main(void) { struct S { int x; } s; s.x = 1; switch (s) { default: return 0; } }"
    )
    .contains("expected an integer"));
    assert!(sema_err(
        "void f(void) { return; } int main(void) { switch (f()) { default: return 0; } }"
    )
    .contains("void value"));
    assert!(
        sema_err("int main(void) { int a[2]; switch (a) { default: return 0; } }")
            .contains("expected an integer")
    );
    assert!(
        sema_err("int main(void) { int x; switch (x) { case x: return 0; } }")
            .contains("not an integer constant expression")
    );
    assert!(
        sema_err("int main(void) { switch (1) { case 1 / 0: return 0; } }")
            .contains("divides by zero")
    );
    assert!(
        sema_err("int main(void) { switch (1) { case 2147483647 + 1: return 0; } }")
            .contains("overflows 'int'")
    );
    assert!(
        sema_err("int main(void) { switch (1) { case 1 << 31: return 0; } }")
            .contains("overflows 'int'")
    );
    assert!(
        sema_err("int main(void) { switch (1) { case 1 << 32: return 0; } }")
            .contains("undefined shift")
    );
    assert!(
        sema_err("int main(void) { switch (1) { case (int *)0: return 0; } }")
            .contains("expected an integer")
    );
    assert!(
        sema_err("int main(void) { switch (1) { case 1: continue; } }")
            .contains("'continue' is not inside a loop")
    );
    assert!(sema_err("int main(void) { break; }").contains("not inside a loop or switch"));
    sema_ok("int main(void) { switch (1) { case 1: break; } return 0; }");
    sema_ok(
            "int main(void) { int x; switch (x) { case 1: return 1; } switch (x) { case 1: return 2; } return 0; }",
        );
}

#[test]
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    ignore = "reads the host glibc <math.h>"
)]
fn math_format_builtins_are_constant_floats() {
    // C11 7.12p3–p5 and GCC "Floating-Point Format Builtins": these
    // names are constant expressions of the documented types, so they
    // may initialize static storage and appear in `_Generic`.
    sema_ok("float n = __builtin_nanf(\"\");");
    sema_ok("float i = __builtin_inff();");
    sema_ok("double h = __builtin_huge_val();");
    sema_ok("float hf = __builtin_huge_valf();");
    sema_ok("long double hl = __builtin_huge_vall();");
    sema_ok("double n = __builtin_nan(\"\");");
    sema_ok("long double n = __builtin_nanl(\"\");");
    sema_ok("double i = __builtin_inf();");
    sema_ok("long double i = __builtin_infl();");
    sema_ok("float n = __builtin_nanf(\"1\");");
    sema_ok("double n = __builtin_nan(\"1\0ignored\");");
    sema_ok("float n = __builtin_nansf(\"\");");
    sema_ok("double n = __builtin_nans(\"\");");
    sema_ok("long double n = __builtin_nansl(\"\");");
    sema_ok("int main(void) { static float n = __builtin_nanf(\"\"); return n != n; }");
    sema_ok("int a = _Generic(__builtin_nanf(\"\"), float: 1, default: 0);");
    sema_ok("int a = _Generic(__builtin_huge_val(), double: 1, default: 0);");
    sema_ok("int a = _Generic(__builtin_huge_vall(), long double: 1, default: 0);");
    // The system <math.h> macros expand to these builtins under
    // __GNUC__ 3.3 or later (C11 7.12p3–p5).
    let ir = crate::compile_to_ir(
        "#include <math.h>\nfloat n = NAN;\nfloat i = INFINITY;\ndouble h = HUGE_VAL;\n",
        "math-macros.c",
    )
    .expect("system math.h macros should compile");
    assert!(
        ir.contains("0x7FF8000000000000") && ir.contains("0x7FF0000000000000"),
        "NAN and INFINITY / HUGE_VAL constants missing: {ir}"
    );

    assert!(sema_err("float n = __builtin_nanf(0);").contains("requires a string literal argument"));
    assert!(sema_err("float n = __builtin_nanf();").contains("expects 1 argument"));
    assert!(sema_err("float n = __builtin_inff(1);").contains("expects 0 arguments"));
    assert!(
        sema_err("const char *p = \"\"; float n = __builtin_nanf(p);")
            .contains("requires a string literal argument")
    );
    // A string that strtol would not consume in full is not a constant
    // (GCC "Floating-Point Format Builtins"). A static initializer
    // and an ICE keep diagnosing. A runtime __builtin_nans* with a
    // malformed tag is also a diagnostic: there is no library nans.
    assert!(sema_err("float n = __builtin_nanf(\"abc\");").contains("not a constant expression"));
    assert!(sema_err("double x = __builtin_nan(\"1xyz\");").contains("not a constant expression"));
    assert!(
        sema_err("int main(void) { static double x = __builtin_nan(\"1xyz\"); return 0; }")
            .contains("not a constant expression")
    );
    assert!(
        sema_err("int main(void) { return (int)__builtin_nans(\"1xyz\"); }")
            .contains("strtol would consume in full")
    );
    // A runtime quiet malformed tag is accepted; IR lowering calls nan().
    sema_ok("int main(void) { return __builtin_nan(\"1xyz\") != 0.0; }");
}

#[test]
fn offsetof_is_an_integer_constant_expression() {
    // C11 7.19p3: offsetof(type, member-designator) is an integer
    // constant expression of type size_t. <stddef.h> expands it to
    // __builtin_offsetof, which accepts `.member` and `[constant]`
    // designators after the first member name.
    let unit = analyzed(
        "\
struct In { char a; double b; int c; };
struct Out { short s; struct In items[3]; union { char k; long w; } u; struct { char p; int q; }; };
_Static_assert(__builtin_offsetof(struct Out, items[2].c) == 8 + 2 * 24 + 16, \"nested\");
enum { E = __builtin_offsetof(struct Out, u.w), Q = __builtin_offsetof(struct Out, q) };
unsigned long direct = __builtin_offsetof(struct Out, items);
unsigned long nested = __builtin_offsetof(struct Out, items[1].b);
unsigned long computed = __builtin_offsetof(struct Out, items[1 + 1].c);
unsigned long via_enum = E;
unsigned long anon = Q;
int bound[__builtin_offsetof(struct Out, items[0].c)];
unsigned long bound_size = sizeof bound;
int typed = _Generic(__builtin_offsetof(struct In, c), unsigned long: 1, default: 0);
",
    );
    let value = |name: &str| {
        unit.globals
            .iter()
            .find(|global| global.name == name)
            .unwrap_or_else(|| panic!("missing {name}"))
            .int_init()
    };
    assert_eq!(value("direct"), Some(8));
    assert_eq!(value("nested"), Some(8 + 24 + 8));
    assert_eq!(value("computed"), Some(8 + 2 * 24 + 16));
    assert_eq!(value("via_enum"), Some(80));
    assert_eq!(value("anon"), Some(92));
    assert_eq!(value("bound_size"), Some(4 * 24));
    assert_eq!(value("typed"), Some(1));

    // A non-constant subscript is not an integer constant expression.
    sema_ok(
        "struct S { int a[4]; }; int f(int i) { return (int)__builtin_offsetof(struct S, a[i]); }",
    );
    assert!(sema_err(
            "struct S { int a[4]; }; int i; _Static_assert(__builtin_offsetof(struct S, a[i]) == 0, \"\");"
        )
        .contains("integer constant expression"));

    // Constraints: a complete structure or union type, an existing
    // member, no bit-field (C11 7.19p3), and `[]` only on arrays.
    assert!(sema_err(
        "struct S { int a : 3; }; unsigned long n = __builtin_offsetof(struct S, a);"
    )
    .contains("bit-field"));
    assert!(
        sema_err("struct S { int a; }; unsigned long n = __builtin_offsetof(struct S, b);")
            .contains("no member named 'b'")
    );
    assert!(
        sema_err("struct S; unsigned long n = __builtin_offsetof(struct S, a);")
            .contains("incomplete")
    );
    assert!(
        sema_err("unsigned long n = __builtin_offsetof(int, a);").contains("structure or union")
    );
    assert!(
        sema_err("struct S { int a; }; unsigned long n = __builtin_offsetof(struct S, a[1]);")
            .contains("subscripted value is not an array")
    );
    assert!(sema_err(
        "struct S { int a[2]; }; unsigned long n = __builtin_offsetof(struct S, a.b);"
    )
    .contains("not a structure or union"));
}

#[test]
fn floating_comparison_builtins() {
    // GCC "Other Builtins": __builtin_isgreater and the rest are the
    // C11 7.12.14 comparison macros. Two real floating operands
    // (one may be an integer) are converted by the usual arithmetic
    // conversions; the result is int.
    for name in [
        "__builtin_isgreater",
        "__builtin_isgreaterequal",
        "__builtin_isless",
        "__builtin_islessequal",
        "__builtin_islessgreater",
        "__builtin_isunordered",
    ] {
        sema_ok(&format!(
            "int f(double a, double b) {{ return {name}(a, b); }}"
        ));
        sema_ok(&format!(
            "int f(float a, long double b) {{ return {name}(a, b); }}"
        ));
        sema_ok(&format!(
            "int f(int a, double b) {{ return {name}(a, b) + {name}(b, 2u); }}"
        ));
        sema_ok(&format!(
            "int f(float a, float b) {{ return _Generic({name}(a, b), int: 1, default: 0); }}"
        ));
        sema_ok(&format!(
            "unsigned long n = sizeof({name}(1.0, 2.0)); int a[sizeof {name}(1.0f, 2)];"
        ));
        assert!(
            sema_err(&format!("int f(int a, int b) {{ return {name}(a, b); }}"))
                .contains("non-floating-point arguments")
        );
        assert!(
            sema_err(&format!("int f(double *p) {{ return {name}(p, 1.0); }}"))
                .contains("non-floating-point arguments")
        );
        assert!(sema_err(&format!(
            "struct S {{ double d; }}; int f(struct S s) {{ return {name}(s, 1.0); }}"
        ))
        .contains("non-floating-point arguments"));
        assert!(
            sema_err(&format!("int f(double a) {{ return {name}(a); }}"))
                .contains("expects 2 arguments")
        );
        assert!(
            sema_err(&format!("int f(double a) {{ return {name}(a, a, a); }}"))
                .contains("expects 2 arguments")
        );
        // Not a constant expression (Clang 19 black-box agrees).
        assert!(sema_err(&format!("int x = {name}(1.0, 2.0);")).contains("not a constant"));
    }
}

#[test]
fn external_object_clashing_with_a_runtime_nan_call() {
    // A malformed quiet __builtin_nan* tag at run time calls the C
    // library nan/nanf/nanl. An object with external linkage and the
    // same name is undefined behaviour (C11 7.1.3p1-p2) and would make
    // the call target the object, so it is a diagnostic at the call.
    for source in [
        "extern int nan;\ndouble f(void) { return __builtin_nan(\"1xyz\"); }",
        "int nan;\ndouble f(void) { return __builtin_nan(\"1xyz\"); }",
        "double f(void) { extern int nan; return __builtin_nan(\"1xyz\"); }",
        "double f(void) { return __builtin_nan(\"1xyz\"); }\nint nan;",
        "float nanf;\nfloat f(void) { return __builtin_nanf(\"x\"); }",
        "extern char nanl[];\nlong double f(void) { return __builtin_nanl(\"x\"); }",
    ] {
        let err = sema_err(source);
        assert!(err.contains("conflicts with the object"), "{source}: {err}");
        assert!(err.contains("external linkage"), "{source}: {err}");
        assert!(
            err.contains(":2:") || source.starts_with("double f"),
            "{source}: {err}"
        );
    }
    // The diagnostic points at the builtin call.
    let err = sema_err("int nan;\ndouble f(void) {\n  return __builtin_nan(\"1xyz\");\n}");
    assert!(err.contains(":3:10"), "{err}");

    // No clash: a valid tag folds, the other library name, an
    // unevaluated operand, internal linkage, a block-scope static, a
    // local, and a user-declared function.
    sema_ok(
        "extern int nan;\ndouble f(void) { return __builtin_nan(\"\") + __builtin_nan(\"0x10\"); }",
    );
    sema_ok("int nan;\nfloat f(void) { return __builtin_nanf(\"1xyz\"); }");
    sema_ok("extern int nan;\nunsigned long f(void) { return sizeof(__builtin_nan(\"1xyz\")); }");
    sema_ok("static int nan;\ndouble f(void) { return __builtin_nan(\"1xyz\"); }");
    sema_ok("int f(void) { static int nan; return (int)__builtin_nan(\"1xyz\") + nan; }");
    sema_ok("double f(void) { int nan = 0; return __builtin_nan(\"1xyz\") + nan; }");
    sema_ok("double nan(const char *);\ndouble f(void) { return __builtin_nan(\"1xyz\"); }");
}

#[test]
fn unevaluated_generic_control_records_no_nan_call() {
    // The controlling expression of a generic selection is not
    // evaluated (C11 6.5.1.1p3), so a malformed tag there makes no
    // library call and cannot clash with an object named nan.
    sema_ok(
            "extern int nan;\nint f(void) { return _Generic(__builtin_nan(\"1xyz\"), double: 1, default: 0); }",
        );
    sema_ok(
            "float nanf;\nint f(void) { return _Generic(_Generic(0, int: __builtin_nanf(\"x\")), float: 1); }",
        );
    sema_ok(
            "extern int nan;\nint f(void) { return _Generic(1 + __builtin_nan(\"q\"), double: 2, default: 3) + (int)sizeof(__builtin_nan(\"q\")); }",
        );
    // The selected association is evaluated, so it still clashes.
    let err = sema_err(
            "extern int nan;\ndouble f(void) { return _Generic(0, int: __builtin_nan(\"1xyz\"), default: 0.0); }",
        );
    assert!(err.contains("conflicts with the object"), "{err}");
    let err = sema_err(
            "extern int nan;\ndouble f(void) { return _Generic(__builtin_nan(\"a\"), double: __builtin_nan(\"b\")); }",
        );
    assert!(
        err.contains("conflicts with the object") && err.contains(":2:"),
        "{err}"
    );
}

#[test]
fn void_pointer_equality_excludes_function_pointers() {
    // C11 6.5.9p2: a void pointer may be compared for equality with a
    // pointer to an object type, not with a pointer to a function. A
    // null pointer constant may be compared with either.
    for source in [
        "int f(void); int g(void *v) { return v == f; }",
        "int f(void); int g(const void *v) { return f != v; }",
        "int g(void *v, int (*fp)(int)) { return fp == v; }",
    ] {
        let err = sema_err(source);
        assert!(err.contains("invalid operands to '"), "{source}: {err}");
    }
    sema_ok("int f(void); int g(void) { return (void *)0 == f; }");
    sema_ok("int f(void); int g(void) { return f != 0; }");
    sema_ok("int g(void *v, int *p, const char *c) { return (v == p) + (c != v); }");
}

fn sema_dialect(source: &str, dialect: crate::Dialect) -> Result<Vec<String>, String> {
    let tokens = lex(source, "gnu.c").unwrap();
    let mut unit =
        crate::parser::parse_dialect(&tokens, "gnu.c", dialect).map_err(|err| err.to_string())?;
    analyze_dialect(&mut unit, "gnu.c", &tokens, dialect)
        .map(|warnings| warnings.iter().map(ToString::to_string).collect())
        .map_err(|err| err.to_string())
}

#[test]
fn gnu_dialect_accepts_void_pointer_function_pointer_equality() {
    // C11 6.5.9p2 makes `void *` == pointer-to-function a constraint
    // violation. -std=gnu11 accepts it as an extension, as GCC 14 and
    // Clang 19 do outside their pedantic modes (black-box), with a
    // warning; -std=c11, the default, keeps the error.
    use crate::Dialect;
    for source in [
        "int f(void); int g(void *v) { return v == f; }",
        "int f(void); int g(void *v) { return v != &f; }",
        "int f(void); int g(const void *v) { return f != v; }",
        "int g(void *v, int (*fp)(int)) { return fp == v; }",
    ] {
        let err = sema_dialect(source, Dialect::C11).unwrap_err();
        assert!(err.contains("invalid operands to '"), "{source}: {err}");
        let warnings = sema_dialect(source, Dialect::Gnu11).unwrap();
        assert_eq!(warnings.len(), 1, "{source}: {warnings:?}");
        assert!(
            warnings[0].contains("warning:") && warnings[0].contains("GNU extension"),
            "{source}: {warnings:?}"
        );
    }
    // Valid in both dialects, with no warning: null pointer constants
    // against a function pointer, and void * against object pointers.
    for source in [
        "int f(void); int g(void) { return (void *)0 == f; }",
        "int f(void); int g(void) { return &f != (void *)0; }",
        "int f(void); int g(void) { return f != 0; }",
        "int g(void *v, int *p, const char *c) { return (v == p) + (c != v); }",
    ] {
        for dialect in [Dialect::C11, Dialect::Gnu11] {
            assert_eq!(sema_dialect(source, dialect), Ok(Vec::new()), "{source}");
        }
    }
    // The extension covers only equality: relational comparison of
    // function pointers stays an error in GNU mode too.
    let err = sema_dialect(
        "int f(void); int g(void *v) { return v < f; }",
        Dialect::Gnu11,
    )
    .unwrap_err();
    assert!(err.contains("relational comparison"), "{err}");
}

#[test]
fn gnu_dialect_accepts_distinct_pointer_comparisons() {
    // C11 6.5.8p2 and 6.5.9p2 need compatible pointee types (apart from
    // qualifiers, void, and null pointer constants). GCC 14 and Clang 19
    // accept distinct pointer types in GNU mode with a warning
    // (black-box). -std=c11 keeps the error.
    use crate::Dialect;
    for source in [
            "int g(int *i, unsigned *u) { return i == u; }",
            "int g(int *i, unsigned *u) { return u != i; }",
            "int g(int *i, unsigned *u) { return i < u; }",
            "int g(int *i, unsigned *u) { return u >= i; }",
            "int g(int **a, const int **b) { return a == b; }",
            "int g(int **a, const int **b) { return a > b; }",
            "int g(volatile unsigned char * volatile * volatile *a, unsigned char *** const b) { return a == b; }",
            "struct S; int g(struct S **a, const struct S **b) { return a != b; }",
            "int g(int *a, double *b) { return a == b; }",
            "int g(int *a, int (*f)(void)) { return a == f; }",
            "int g(int (*f)(int), int (*h)(void)) { return f == h; }",
        ] {
            let err = sema_dialect(source, Dialect::C11).unwrap_err();
            assert!(err.contains("invalid operands to '"), "{source}: {err}");
            let warnings = sema_dialect(source, Dialect::Gnu11).unwrap();
            assert_eq!(warnings.len(), 1, "{source}: {warnings:?}");
            assert!(
                warnings[0].contains("warning:") && warnings[0].contains("distinct pointer types"),
                "{source}: {warnings:?}"
            );
        }
    // Relational comparison of function pointers stays an error, and a
    // pointer against a non-null integer is not part of this extension.
    for source in [
        "int g(int (*f)(int), int (*h)(void)) { return f < h; }",
        "int g(int *p) { return p == 5; }",
    ] {
        assert!(sema_dialect(source, Dialect::Gnu11).is_err(), "{source}");
    }
}

#[test]
fn gnu_dialect_accepts_pointer_sign_and_qualifier_assignments() {
    // C11 6.5.16.1p1 needs compatible pointees and every qualifier of the
    // source pointee on the destination pointee. In GNU mode GCC 14 and
    // Clang 19 accept a pointee that differs only in the signedness of
    // an integer type, or that drops qualifiers, with a warning
    // (black-box). The same rule covers initialization, arguments, and
    // return. -std=c11 keeps the error.
    use crate::Dialect;
    for source in [
        "void g(int *a, unsigned *b) { a = b; }",
        "void g(unsigned *a, int *b) { a = b; }",
        "void g(signed char *a, unsigned char *b) { a = b; }",
        "void g(char *a, unsigned char *b) { a = b; }",
        "void g(long *a, unsigned long *b) { a = b; }",
        "void g(const int *a, unsigned *b) { a = b; }",
        "void g(volatile int *a, volatile unsigned *b) { a = b; }",
        "void g(unsigned *b) { int * volatile a = b; (void)a; }",
        "void g(int *a, const int *b) { a = b; }",
        "void g(void *a, const int *b) { a = b; }",
        "void g(int *a, const unsigned *b) { a = b; }",
        "int f(int *); int g(unsigned *b) { return f(b); }",
        "int *g(unsigned *b) { return b; }",
        "unsigned u; int *p = &u;",
        "const char m[] = \"x\"; char *p = m;",
    ] {
        let err = sema_dialect(source, Dialect::C11).unwrap_err();
        assert!(err.contains("incompatible types"), "{source}: {err}");
        let warnings = sema_dialect(source, Dialect::Gnu11).unwrap();
        assert_eq!(warnings.len(), 1, "{source}: {warnings:?}");
        assert!(warnings[0].contains("warning:"), "{source}: {warnings:?}");
    }
    // GCC 14 rejects these even in GNU mode, so this compiler does too.
    for source in [
        "void g(int *a, long *b) { a = b; }",
        // Same size, different types: only signedness is relaxed.
        "void g(long *a, long long *b) { a = b; }",
        "void g(long long *a, unsigned long *b) { a = b; }",
        "void g(unsigned long *a, const long long *b) { a = b; }",
        "void g(_Bool *a, unsigned char *b) { a = b; }",
        "void g(int **a, unsigned **b) { a = b; }",
        "void g(const int **a, int **b) { a = b; }",
        "void g(int *a, double *b) { a = b; }",
        "void g(int *a, int (*f)(void)) { a = f; }",
        "void g(int *a) { a = 5; }",
    ] {
        let err = sema_dialect(source, Dialect::Gnu11).unwrap_err();
        assert!(err.contains("incompatible"), "{source}: {err}");
    }
}

#[test]
fn null_pointer_constant_needs_integer_operand_in_void_cast() {
    // C11 6.3.2.3p3: only an integer constant expression with the value
    // 0, possibly cast to void *, is a null pointer constant.
    // (void *)(void *)0 is a null pointer of type void *, so comparing
    // it with a function pointer violates 6.5.9p2.
    for source in [
        "int f(void); int g(void) { return f == (void *)(void *)0; }",
        "int f(void); int g(void) { return (void *)(void *)(1 - 1) != &f; }",
    ] {
        let err = sema_err(source);
        assert!(err.contains("invalid operands to '"), "{source}: {err}");
    }
    sema_ok("int f(void); int g(void) { return f == _Generic(0, int: (void *)0); }");
    sema_ok("int g(int *p) { return p == (void *)(void *)0; }");
}

#[test]
fn void_pointer_and_function_pointer_assignment_and_conditional() {
    // C11 6.5.16.1p1 and 6.5.15p3 pair `void *` only with object pointers.
    // -std=gnu11 accepts a function pointer with a warning, as GCC 14 and
    // Clang 19 do outside their pedantic modes (black-box); -std=c11 keeps
    // the error, as `-pedantic-errors` does there.
    use crate::Dialect;
    for source in [
        "int f(void); void *p = f;",
        "int f(void); void g(void) { void *p; p = &f; }",
        "void *v; void g(void) { int (*fp)(void); fp = v; }",
        "int f(void); void take(void *); void g(void) { take(f); }",
        "int f(void); const void *g(void) { return f; }",
        "int f(void); void *v; void g(void) { (void)(1 ? f : v); }",
        "int f(void); void *v; void g(void) { (void)(1 ? v : &f); }",
    ] {
        let err = sema_dialect(source, Dialect::C11).unwrap_err();
        assert!(err.contains("incompatible"), "{source}: {err}");
        let warnings = sema_dialect(source, Dialect::Gnu11).unwrap();
        assert_eq!(warnings.len(), 1, "{source}: {warnings:?}");
        assert!(
            warnings[0].contains("GNU extension"),
            "{source}: {warnings:?}"
        );
    }
    // Valid in both dialects with no warning: null pointer constants, and
    // `void *` with object pointers, including incomplete ones.
    for source in [
        "int f(void); int (*fp)(void) = (void *)0; int (*fq)(void) = 0;",
        "int f(void); void g(void) { int (*fp)(void) = 1 ? f : (void *)0; fp = fp; }",
        "struct I; struct I *ip; void *v; void g(void) { v = ip; ip = v; (void)(1 ? ip : v); }",
        "int a[3]; void *v = a; const void *c = &a; void g(void) { (void)(1 ? c : v); }",
    ] {
        for dialect in [Dialect::C11, Dialect::Gnu11] {
            assert_eq!(sema_dialect(source, dialect), Ok(Vec::new()), "{source}");
        }
    }
}

#[test]
fn block_function_redeclaration_keeps_the_composite_type() {
    // C11 6.2.7p4, 6.7p4: after `int f(); int f(int);` in one block, `f`
    // has the prototype, so `int f(double)` there conflicts with it (Clang
    // 19 and GCC 14 reject it), and a call is checked against it.
    assert!(
        sema_err("void g(void) { int f(); int f(int); int f(double); }")
            .contains("conflicting types for 'f'")
    );
    assert!(
        sema_err("void g(void) { extern int f(); extern int f(int); f(1, 2); }")
            .contains("expects 1 argument")
    );
    // The function's type is shared with other blocks, as for a first
    // block-scope declaration (GCC 14 rejects this too).
    assert!(
        sema_err("void g(void) { int f(); int f(int); } void h(void) { int f(double); }")
            .contains("conflicting types for 'f'")
    );
    sema_ok("void g(void) { int f(); int f(int); int f(int); int f(); f(1); }");
    sema_ok(
        "static int f(); void g(void) { int f(int); int f(); } static int f(int x) { return x; }",
    );
}

#[test]
fn bit_positions_past_two_to_the_32_stay_in_place() {
    // A bit-field store's position from the start of a static object is a
    // u64: a position at or past bit 2^32 must not wrap or saturate onto an
    // earlier byte. Here it is past the end of the image, so nothing moves.
    let mut bytes = [0u8; 16];
    super::image::pack_bits(&mut bytes, (1u64 << 32) + 3, 1, 1);
    super::image::pack_bits(&mut bytes, u64::from(u32::MAX), 9, 0x1ff);
    assert_eq!(bytes, [0u8; 16]);
    super::image::pack_bits(&mut bytes, 9, 3, 0b101);
    assert_eq!(bytes[1], 0b1010);
}

#[test]
fn alignas_and_thread_local_redeclarations() {
    sema_ok("_Alignas(16) int x; int x;");
    sema_ok("_Alignas(16) int x; _Alignas(16) int x;");
    sema_ok("extern int x; _Alignas(8) int x;");
    // N1570 6.7.5p7: a declaration may omit the specifier when the
    // definition has one. `int x;` is a tentative declaration, not a
    // definition that forbids a later specifier.
    sema_ok("int x; _Alignas(16) int x;");
    sema_ok("int x; _Alignas(16) int x = 1;");
    sema_ok("_Alignas(32) int x = 1; int x; extern _Alignas(32) int x;");
    assert!(sema_err("int x = 1; _Alignas(16) int x;").contains("alignment specifier"));
    assert!(sema_err("_Alignas(8) int x; _Alignas(16) int x;").contains("conflicting alignment"));
    assert!(
        sema_err("_Alignas(1) struct S x; struct S { int member; };")
            .contains("less than the alignment")
    );
    sema_ok("_Thread_local int t; extern _Thread_local int t;");
    sema_ok("_Thread_local static int t; _Thread_local static int t;");
    assert!(sema_err("_Thread_local int t; int t;")
        .contains("'_Thread_local' must be present on every declaration"));
    assert!(sema_err("int t; _Thread_local int t;")
        .contains("'_Thread_local' must be present on every declaration"));
    sema_ok("int main(void) { static _Thread_local int x; return x; }");
    sema_ok("extern _Thread_local int t; int main(void) { extern _Thread_local int t; return t; }");
    assert!(
        sema_err("extern _Thread_local int t; int main(void) { extern int t; return t; }")
            .contains("'_Thread_local' must be present on every declaration")
    );
    assert!(
        sema_err("void f(void) { extern _Thread_local int x; extern int x; }")
            .contains("'_Thread_local' must be present on every declaration")
    );
    assert!(sema_err("_Thread_local int x; int *p = &x;").contains("not a constant expression"));
    assert!(
        sema_err("_Thread_local int t; unsigned long long x = (unsigned long long)&t;")
            .contains("not a constant expression")
    );
    assert!(
        sema_err("void f(void) { static _Thread_local void *p = &p; }")
            .contains("not a constant expression")
    );
    sema_ok("void f(void) { static void *p = &p; }");
    sema_ok("_Thread_local int x; int *f(void) { return &x; }");
}

#[test]
fn kr_definition_compatible_with_promoted_prototype() {
    sema_ok("int f(int); int f(c) char c; { return c; }");
    sema_ok("int f(c) char c; { return c; } int f(int);");
    sema_ok("void f(double); void f(x) float x; { }");
    assert!(
        sema_err("void f(float); void f(x) float x; { }").contains("incompatible")
            || sema_err("void f(float); void f(x) float x; { }").contains("conflicting")
    );
    assert!(
        sema_err("int f(int, int); int f(a) int a; { return a; }").contains("incompatible")
            || sema_err("int f(int, int); int f(a) int a; { return a; }").contains("conflicting")
    );
    sema_ok("int f(); int f(a) int a; { return a; }");
    sema_ok("int f(); int f(x) float x; { return 0; } int f(double);");
    assert!(
        sema_err("int f(int); int f() { return 0; }").contains("incompatible")
            || sema_err("int f(int); int f() { return 0; }").contains("conflicting")
    );
}

#[test]
fn vla_constraints_and_jumps() {
    sema_ok("void f(int n) { int a[n]; a[0] = 1; }");
    sema_ok("void f(int n) { int a[n]; goto L; L: a[0] = 1; }");
    sema_ok("void f(int n) { goto L; { int a[n]; } L: ; }");
    assert!(
        sema_err("void f(int n) { goto L; int a[n]; L: a[0] = 1; }").contains("variably modified")
    );
    assert!(
        sema_err("void f(int n, int x) { switch (x) { int a[n]; case 0: a[0] = 1; } }")
            .contains("variably modified")
    );
    assert!(
        sema_err("void f(int n) { int x = _Generic(0, int[n]: 1, default: 0); }")
            .contains("variably modified")
    );
    sema_ok("void f(int n) { int a[n]; switch (n) { case 1: a[0] = 1; } }");
    assert!(sema_err(
        "void f(int n, int x) { switch (x) { int a[n]; if (1) { case 0: a[0] = 1; } } }"
    )
    .contains("variably modified"));
    sema_ok("int n = 3; void f(int n[n]) { n[0] = 1; }");
    assert!(sema_err("void f(int n) { static int (*p)[n + 1.0]; }").contains("integer"));
    assert!(sema_err(
        "void f(int n) { _Static_assert(_Generic(0, int (*)[n]: 1, default: 1), \"ok\"); }"
    )
    .contains("generic association"));
}
