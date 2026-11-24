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

fn analyzed_gnu(source: &str) -> TranslationUnit {
    let dialect = crate::Dialect::Gnu11;
    let tokens = lex(source, "gnu.c").unwrap();
    let mut unit = crate::parser::parse_dialect(&tokens, "gnu.c", dialect).unwrap();
    analyze_dialect(&mut unit, "gnu.c", &tokens, dialect).unwrap();
    unit
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
fn folds_every_comparison_and_operator_like_clang() {
    // Mutation testing (reports/hardening/mutation.md) found that no unit
    // test told `<` from `<=`, `|` from `^`, or a real `?:` from an integer
    // one in the folders. Every value below is what Clang prints for the
    // same initializer; the `aN` bounds go through the parser's ICE folder
    // and the `gN` initializers through sema's.
    let unit = analyzed(
        "\
int g0 = 1.5 < 2.0;
int g1 = 2.0 < 1.5;
int g2 = 2.0 < 2.0;
int g3 = 1.5 > 2.0;
int g4 = 2.5 > 2.0;
int g5 = 2.0 > 2.0;
int g6 = 1.5 <= 2.0;
int g7 = 2.0 <= 2.0;
int g8 = 2.5 <= 2.0;
int g9 = 1.5 >= 2.0;
int g10 = 2.0 >= 2.0;
int g11 = 2.5 >= 2.0;
int g12 = 1.5 == 1.5;
int g13 = 1.5 == 2.5;
int g14 = 1.5 != 1.5;
int g15 = 1.5 != 2.5;
int g16 = 1.5f < 2.0f;
int g17 = 2.0f >= 2.5f;
int g18 = 1 < 1.5;
int g19 = 2 <= 1.5f;
int g20 = 0.0 || 2.5;
int g21 = 0.5 || 0;
int g22 = 0.0 || 0.0;
int g23 = 0.0 && 1;
int g24 = 1.5 && 0.0;
int g25 = 1.5 && 2;
int g26 = !0.0;
int g27 = !0.5;
int g28 = (int)-2.5;
int g29 = (int)+2.5;
int g30 = (int)(1.5 ? 2 : 3);
int g31 = (int)(0.0 ? 2 : 3);
int g32 = (int)(1 ? 2.5 : 3);
int g33 = (int)(0 ? 2 : 3.5);
int g34 = (int)(0.5 ? 7.25 : 3);
int g35 = (int)(1 ? 2 : 3.5);
int g36 = (int)((double)7 / 2);
int g37 = (int)(7 / 2.0 * 2);
int g38 = (int)(7.5 - 2);
int g39 = (int)(1.25 + 1.75);
int g40 = (int)(sizeof(int) * 1.5);
int g41 = (int)(3 * 1.5f);
int g42 = -1 < 1;
int g43 = -1 < 1u;
int g44 = 2 > 2;
int g45 = 3 > 2;
int g46 = 2 >= 2;
int g47 = 1 >= 2;
int g48 = 3 <= 2;
int g49 = 2 <= 2;
int g50 = 1 != 1;
int g51 = 1 != 2;
int g52 = 1 == 2;
int g53 = 2u >= 3u;
int g54 = 3u >= 3u;
int g55 = 2u <= 2u;
int g56 = 3u <= 2u;
int g57 = 3u > 3u;
int g58 = 4u > 3u;
int g59 = 1u != 2u;
int g60 = 1u != 1u;
int g61 = 2u < 3u;
int g62 = -7 % 3;
int g63 = 7 % -3;
int g64 = 7 % 3;
int g65 = 6 | 3;
int g66 = 6 ^ 3;
int g67 = 6 & 3;
int g68 = 5u % 3u;
int g69 = 5u | 2u;
int g70 = 5u ^ 1u;
int g71 = 5u & 4u;
int g72 = (int)(-1u / 2);
int g73 = (int)~0u;
int g74 = ~5;
int g75 = -(-5);
int g76 = (int)(-1u >> 28);
char a0[(-1 < 1) + 3]; int s0 = sizeof a0;
char a1[(-1 < 1u) + 3]; int s1 = sizeof a1;
char a2[(2 > 2) + 3]; int s2 = sizeof a2;
char a3[(3 > 2) + 3]; int s3 = sizeof a3;
char a4[(2 >= 2) + 3]; int s4 = sizeof a4;
char a5[(1 >= 2) + 3]; int s5 = sizeof a5;
char a6[(3 <= 2) + 3]; int s6 = sizeof a6;
char a7[(2 <= 2) + 3]; int s7 = sizeof a7;
char a8[(1 != 1) + 3]; int s8 = sizeof a8;
char a9[(1 != 2) + 3]; int s9 = sizeof a9;
char a10[(1 == 2) + 3]; int s10 = sizeof a10;
char a11[(2u >= 3u) + 3]; int s11 = sizeof a11;
char a12[(3u >= 3u) + 3]; int s12 = sizeof a12;
char a13[(2u <= 2u) + 3]; int s13 = sizeof a13;
char a14[(3u <= 2u) + 3]; int s14 = sizeof a14;
char a15[(3u > 3u) + 3]; int s15 = sizeof a15;
char a16[(4u > 3u) + 3]; int s16 = sizeof a16;
char a17[(1u != 2u) + 3]; int s17 = sizeof a17;
char a18[(1u != 1u) + 3]; int s18 = sizeof a18;
char a19[(2u < 3u) + 3]; int s19 = sizeof a19;
char a20[(-7 % 3) + 3]; int s20 = sizeof a20;
char a21[(7 % -3) + 3]; int s21 = sizeof a21;
char a22[(7 % 3) + 3]; int s22 = sizeof a22;
char a23[(6 | 3) + 3]; int s23 = sizeof a23;
char a24[(6 ^ 3) + 3]; int s24 = sizeof a24;
char a25[(6 & 3) + 3]; int s25 = sizeof a25;
char a26[(5u % 3u) + 3]; int s26 = sizeof a26;
char a27[(5u | 2u) + 3]; int s27 = sizeof a27;
char a28[(5u ^ 1u) + 3]; int s28 = sizeof a28;
char a29[(5u & 4u) + 3]; int s29 = sizeof a29;
char a30[((int)~0u) + 3]; int s30 = sizeof a30;
char a31[(-(-5)) + 3]; int s31 = sizeof a31;
char a32[((int)(-1u >> 28)) + 3]; int s32 = sizeof a32;
",
    );
    let value = |name: String| {
        unit.globals
            .iter()
            .find(|global| global.name == name)
            .unwrap()
            .int_init()
            .unwrap()
    };
    let globals: [i32; 77] = [
        1, 0, 0, 0, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1, 0, 1, 0, 1, 1, 0, 0, 0, 1, 1, 0, -2, 2,
        2, 3, 2, 3, 7, 2, 3, 7, 5, 3, 6, 4, 1, 0, 0, 1, 1, 0, 0, 1, 0, 1, 0, 0, 1, 1, 0, 0, 1, 1,
        0, 1, -1, 1, 1, 7, 5, 2, 2, 7, 4, 4, 2147483647, -1, -6, 5, 15,
    ];
    for (i, want) in globals.iter().enumerate() {
        assert_eq!(value(format!("g{i}")), u128::from(*want as u32), "g{i}");
    }
    let bounds: [u32; 33] = [
        4, 3, 3, 4, 4, 3, 3, 4, 3, 4, 3, 3, 4, 4, 3, 3, 4, 4, 3, 4, 2, 4, 4, 10, 8, 5, 5, 10, 7, 7,
        2, 8, 18,
    ];
    for (i, want) in bounds.iter().enumerate() {
        assert_eq!(value(format!("s{i}")), u128::from(*want), "s{i}");
    }
}

#[test]
fn folds_shifts_limits_and_conditionals_like_clang() {
    // A second mutation pass (reports/hardening/mutation.md) found shift
    // counts of 0 and unsigned counts, values at exactly INT_MAX/INT_MIN,
    // unsigned negation and `?:` operand types untested. Every value is what
    // Clang prints; `bN` bounds go through the parser's ICE folder and the
    // `hN` initializers through sema's.
    // `?:` with an omitted middle operand is GNU, so this is gnu11.
    let unit = analyzed_gnu(
        r#"
int h0 = 0 << 3;
int h1 = 1 << 0;
int h2 = 1 << 2u;
int h3 = 8u >> 1u;
int h4 = -8 >> 1;
int h5 = 1u << 31 >> 30;
int h6 = 5 >> 0;
int h7 = (unsigned char)1 << 3;
int h8 = 1L << 40 >> 38;
int h9 = -1 >> 31;
int h10 = 1 << 30 >> 29;
int h11 = 2147483647 + 0;
int h12 = -2147483647 - 1 + 0;
int h13 = 2147483646 + 1;
int h14 = (-2147483647 - 1) / 1;
int h15 = (-2147483647 - 1) / -2;
int h16 = 7 / -1;
int h17 = -7 % -1;
int h18 = (-2147483647 - 1) % 2;
int h19 = -(0u);
int h20 = -1u;
int h21 = -(unsigned char)1;
int h22 = ~0u;
int h23 = ~(unsigned char)0;
int h24 = ~5;
int h25 = - -3;
int h26 = -(unsigned short)2;
int h27 = 5 | 3;
int h28 = 5u | 3u;
int h29 = 6L | 3;
int h30 = 5 ^ 3;
int h31 = -1 < 0u;
int h32 = 1 <= 1;
int h33 = -1 < 1;
int h34 = 2u < 2u;
int h35 = 2u <= 2u;
int h36 = (long long)-1 < 0;
int h37 = -1L < 0u;
int h38 = 1 <= 0;
int h39 = 3 > 3;
int h40 = 3 >= 3;
int h41 = -1 > -2;
int h42 = 4294967295u > 1;
int h43 = (unsigned char)256 ? 1 : 2;
int h44 = (unsigned char)257 ? 1 : 2;
int h45 = (unsigned char)256 ?: 7;
int h46 = (unsigned short)65536 ?: 9;
int h47 = (char)3 ?: 4;
int h48 = !(unsigned char)256;
int h49 = !(unsigned short)1;
int h50 = sizeof(1 ? (char)1 : (char)2);
int h51 = sizeof(1 ? 1 : 2L);
int h52 = sizeof(0 ? 1 : 2.0);
int h53 = sizeof(1 ? 1 : 2.0f);
int h54 = sizeof((char)1 ?: (char)2);
int h55 = sizeof(0 ?: 1L);
int h56 = sizeof(1 ?: 2.0);
int h57 = sizeof(1 ? (void *)0 : 0);
int h58 = (int)(4294967295u + 2u);
int h59 = (unsigned char)300 + 0;
int h60 = (signed char)200 + 0;
int h61 = (short)40000 + 0;
int h62 = (unsigned short)70000 + 0;
int h63 = 3 * -4;
int h64 = -12 / 5;
int h65 = -12 % 5;
int h66 = 12u / 5u;
int h67 = 12u % 5u;
int h68 = 4294967295u * 2u;
int h69 = 0u - 1u;
int h70 = 2147483647 - -1L > 0;
int h71 = 1 ? 2 : (1 / 0);
int h72 = 0 ? (1 / 0) : 3;
int h73 = 0 && (1 / 0);
int h74 = 1 || (1 / 0);
int h75 = 2 < 2;
int h76 = -1 < -1;
int h77 = -2 < -1;
int h78 = -2 <= -3;
int h79 = -2 > -2;
int h80 = -2 >= -1;
int h81 = (2147483646 + 1) - 2147483640;
int h82 = (-2147483647 - 1) + 2147483647 + 2;
int h83 = 1 && 0;
int h84 = 1 && 2;
int h85 = 0 || 0;
int h86 = 0 || 3;
int h87 = !0;
int h88 = !5;
char b0[(0 << 3) + 3]; int t0 = sizeof b0;
char b1[(1 << 0) + 3]; int t1 = sizeof b1;
char b2[(1 << 2u) + 3]; int t2 = sizeof b2;
char b3[(8u >> 1u) + 3]; int t3 = sizeof b3;
char b4[(1u << 31 >> 30) + 3]; int t4 = sizeof b4;
char b5[(5 >> 0) + 3]; int t5 = sizeof b5;
char b6[((unsigned char)1 << 3) + 3]; int t6 = sizeof b6;
char b7[(1L << 40 >> 38) + 3]; int t7 = sizeof b7;
char b8[(-1 >> 31) + 3]; int t8 = sizeof b8;
char b9[(1 << 30 >> 29) + 3]; int t9 = sizeof b9;
char b10[(-7 % -1) + 3]; int t10 = sizeof b10;
char b11[((-2147483647 - 1) % 2) + 3]; int t11 = sizeof b11;
char b12[(-(0u)) + 3]; int t12 = sizeof b12;
char b13[(-1u) + 3]; int t13 = sizeof b13;
char b14[(-(unsigned char)1) + 3]; int t14 = sizeof b14;
char b15[(~0u) + 3]; int t15 = sizeof b15;
char b16[(~(unsigned char)0) + 3]; int t16 = sizeof b16;
char b17[(- -3) + 3]; int t17 = sizeof b17;
char b18[(-(unsigned short)2) + 3]; int t18 = sizeof b18;
char b19[(5 | 3) + 3]; int t19 = sizeof b19;
char b20[(5u | 3u) + 3]; int t20 = sizeof b20;
char b21[(6L | 3) + 3]; int t21 = sizeof b21;
char b22[(5 ^ 3) + 3]; int t22 = sizeof b22;
char b23[(-1 < 0u) + 3]; int t23 = sizeof b23;
char b24[(1 <= 1) + 3]; int t24 = sizeof b24;
char b25[(-1 < 1) + 3]; int t25 = sizeof b25;
char b26[(2u < 2u) + 3]; int t26 = sizeof b26;
char b27[(2u <= 2u) + 3]; int t27 = sizeof b27;
char b28[((long long)-1 < 0) + 3]; int t28 = sizeof b28;
char b29[(-1L < 0u) + 3]; int t29 = sizeof b29;
char b30[(1 <= 0) + 3]; int t30 = sizeof b30;
char b31[(3 > 3) + 3]; int t31 = sizeof b31;
char b32[(3 >= 3) + 3]; int t32 = sizeof b32;
char b33[(-1 > -2) + 3]; int t33 = sizeof b33;
char b34[(4294967295u > 1) + 3]; int t34 = sizeof b34;
char b35[((unsigned char)256 ? 1 : 2) + 3]; int t35 = sizeof b35;
char b36[((unsigned char)257 ? 1 : 2) + 3]; int t36 = sizeof b36;
char b37[((unsigned char)256 ?: 7) + 3]; int t37 = sizeof b37;
char b38[((unsigned short)65536 ?: 9) + 3]; int t38 = sizeof b38;
char b39[((char)3 ?: 4) + 3]; int t39 = sizeof b39;
char b40[(!(unsigned char)256) + 3]; int t40 = sizeof b40;
char b41[(!(unsigned short)1) + 3]; int t41 = sizeof b41;
char b42[(sizeof(1 ? (char)1 : (char)2)) + 3]; int t42 = sizeof b42;
char b43[(sizeof(1 ? 1 : 2L)) + 3]; int t43 = sizeof b43;
char b44[(sizeof(0 ? 1 : 2.0)) + 3]; int t44 = sizeof b44;
char b45[(sizeof(1 ? 1 : 2.0f)) + 3]; int t45 = sizeof b45;
char b46[(sizeof((char)1 ?: (char)2)) + 3]; int t46 = sizeof b46;
char b47[(sizeof(0 ?: 1L)) + 3]; int t47 = sizeof b47;
char b48[(sizeof(1 ?: 2.0)) + 3]; int t48 = sizeof b48;
char b49[(sizeof(1 ? (void *)0 : 0)) + 3]; int t49 = sizeof b49;
char b50[((int)(4294967295u + 2u)) + 3]; int t50 = sizeof b50;
char b51[((unsigned char)300 + 0) + 3]; int t51 = sizeof b51;
char b52[(-12 / 5) + 3]; int t52 = sizeof b52;
char b53[(-12 % 5) + 3]; int t53 = sizeof b53;
char b54[(12u / 5u) + 3]; int t54 = sizeof b54;
char b55[(12u % 5u) + 3]; int t55 = sizeof b55;
char b56[(4294967295u * 2u) + 3]; int t56 = sizeof b56;
char b57[(0u - 1u) + 3]; int t57 = sizeof b57;
char b58[(2147483647 - -1L > 0) + 3]; int t58 = sizeof b58;
char b59[(1 ? 2 : (1 / 0)) + 3]; int t59 = sizeof b59;
char b60[(0 ? (1 / 0) : 3) + 3]; int t60 = sizeof b60;
char b61[(0 && (1 / 0)) + 3]; int t61 = sizeof b61;
char b62[(1 || (1 / 0)) + 3]; int t62 = sizeof b62;
char b63[(2 < 2) + 3]; int t63 = sizeof b63;
char b64[(-1 < -1) + 3]; int t64 = sizeof b64;
char b65[(-2 < -1) + 3]; int t65 = sizeof b65;
char b66[(-2 <= -3) + 3]; int t66 = sizeof b66;
char b67[(-2 > -2) + 3]; int t67 = sizeof b67;
char b68[(-2 >= -1) + 3]; int t68 = sizeof b68;
char b69[((2147483646 + 1) - 2147483640) + 3]; int t69 = sizeof b69;
char b70[((-2147483647 - 1) + 2147483647 + 2) + 3]; int t70 = sizeof b70;
char b71[(1 && 0) + 3]; int t71 = sizeof b71;
char b72[(1 && 2) + 3]; int t72 = sizeof b72;
char b73[(0 || 0) + 3]; int t73 = sizeof b73;
char b74[(0 || 3) + 3]; int t74 = sizeof b74;
char b75[(!0) + 3]; int t75 = sizeof b75;
char b76[(!5) + 3]; int t76 = sizeof b76;
"#,
    );
    let value = |name: String| {
        unit.globals
            .iter()
            .find(|global| global.name == name)
            .unwrap()
            .int_init()
            .unwrap()
    };
    let globals: [i32; 89] = [
        0,
        1,
        4,
        4,
        -4,
        2,
        5,
        8,
        4,
        -1,
        2,
        2147483647,
        -2147483648,
        2147483647,
        -2147483648,
        1073741824,
        -7,
        0,
        0,
        0,
        -1,
        -1,
        -1,
        -1,
        -6,
        3,
        -2,
        7,
        7,
        7,
        6,
        0,
        1,
        1,
        0,
        1,
        1,
        1,
        0,
        0,
        1,
        1,
        1,
        2,
        1,
        7,
        9,
        3,
        1,
        0,
        4,
        8,
        8,
        4,
        4,
        8,
        8,
        8,
        1,
        44,
        -56,
        -25536,
        4464,
        -12,
        -2,
        -2,
        2,
        2,
        -2,
        -1,
        1,
        2,
        3,
        0,
        1,
        0,
        0,
        1,
        0,
        0,
        0,
        7,
        1,
        0,
        1,
        0,
        1,
        1,
        0,
    ];
    for (i, want) in globals.iter().enumerate() {
        assert_eq!(value(format!("h{i}")), u128::from(*want as u32), "h{i}");
    }
    let bounds: [u32; 77] = [
        3, 4, 7, 7, 5, 8, 11, 7, 2, 5, 3, 3, 3, 2, 2, 2, 2, 6, 1, 10, 10, 10, 9, 3, 4, 4, 3, 4, 4,
        4, 3, 3, 4, 4, 4, 5, 4, 10, 12, 6, 4, 3, 7, 11, 11, 7, 7, 11, 11, 11, 4, 47, 1, 1, 5, 5, 1,
        2, 4, 5, 6, 3, 4, 3, 3, 4, 3, 3, 3, 10, 4, 3, 4, 3, 4, 4, 3,
    ];
    for (i, want) in bounds.iter().enumerate() {
        assert_eq!(value(format!("t{i}")), u128::from(*want), "t{i}");
    }
}

#[test]
fn array_bounds_reject_overflow_and_undefined_operations() {
    for bound in [
        "2147483647 + 1",
        "(-2147483647 - 1) / -1",
        "(-2147483647 - 1) % -1",
        "-2147483647 - 2",
        "2147483647 * 2",
        "1 << 31",
        "-1 << 1",
        "1 << 32",
        "1u % 0u",
        "1 % 0",
    ] {
        let err = sema_err(&format!("char a[{bound}];"));
        assert!(err.contains("array size"), "{bound}: {err}");
    }
    // The limits themselves are fine.
    sema_ok("char a[(-2147483647 - 1) / 1 + 2147483647 + 2]; char b[2147483647 / 2147483647];");
    sema_ok("char c[(-2147483647 - 1) % 2 + 1]; char d[0x7fffffff >> 30];");
}

#[test]
fn infers_array_lengths_like_clang() {
    // Sizes as Clang prints them: designators move the position, strings
    // count their units plus the terminator (u"" and U"" units are 2 and 4
    // bytes), and brace elision fills one element's members before the
    // next (C11 6.7.9p20).
    let unit = analyzed(
        r#"struct P { int x, y; };
int i0[] = {1, 2, [7] = 3, 4};
int i1[] = {[2] = 1, [0] = 2};
char i2[] = "abc";
char i3[] = {"abcde"};
int i4[][2] = {{1, 2}, {3}, {4}};
char i5[][4] = {"ab", "cde"};
short i6[] = {[1] = 1};
int i7[] = {1, 2, [0] = 2};
unsigned short i8[] = u"ab";
unsigned int i9[] = U"xyz";
unsigned short i10[] = {u"abcd"};
struct P i11[] = {{1, 2}, {3, 4}, {5}};
struct P i12[] = {1, 2, 3};
struct P i13[] = {[1] = {1, 2}, 3, 4};
struct P i14[] = {[1].y = 2, 3};
char i15[][3] = {[1] = "ab", "c"};
int z_i0 = sizeof i0;
int z_i1 = sizeof i1;
int z_i2 = sizeof i2;
int z_i3 = sizeof i3;
int z_i4 = sizeof i4;
int z_i5 = sizeof i5;
int z_i6 = sizeof i6;
int z_i7 = sizeof i7;
int z_i8 = sizeof i8;
int z_i9 = sizeof i9;
int z_i10 = sizeof i10;
int z_i11 = sizeof i11;
int z_i12 = sizeof i12;
int z_i13 = sizeof i13;
int z_i14 = sizeof i14;
int z_i15 = sizeof i15;
int y_i0 = sizeof(char[sizeof i0]);
int y_i1 = sizeof(char[sizeof i1]);
int y_i2 = sizeof(char[sizeof i2]);
int y_i3 = sizeof(char[sizeof i3]);
int y_i4 = sizeof(char[sizeof i4]);
int y_i5 = sizeof(char[sizeof i5]);
int y_i6 = sizeof(char[sizeof i6]);
int y_i7 = sizeof(char[sizeof i7]);
int y_i8 = sizeof(char[sizeof i8]);
int y_i9 = sizeof(char[sizeof i9]);
int y_i10 = sizeof(char[sizeof i10]);
int y_i11 = sizeof(char[sizeof i11]);
int y_i12 = sizeof(char[sizeof i12]);
int y_i13 = sizeof(char[sizeof i13]);
int y_i15 = sizeof(char[sizeof i15]);
"#,
    );
    for (name, want) in [
        ("i0", 36),
        ("i1", 12),
        ("i2", 4),
        ("i3", 6),
        ("i4", 24),
        ("i5", 8),
        ("i6", 4),
        ("i7", 8),
        ("i8", 6),
        ("i9", 16),
        ("i10", 10),
        ("i11", 24),
        ("i12", 16),
        ("i13", 24),
        ("i14", 24),
        ("i15", 9),
    ] {
        let size = unit
            .globals
            .iter()
            .find(|global| global.name == format!("z_{name}"))
            .unwrap()
            .int_init();
        assert_eq!(size, Some(want), "{name}");
        // The parser folds the same length for `sizeof` in an array bound
        // (C11 6.6p6), except after a designator into an element's member,
        // which it leaves to semantic analysis.
        if name != "i14" {
            let folded = unit
                .globals
                .iter()
                .find(|global| global.name == format!("y_{name}"))
                .unwrap()
                .int_init();
            assert_eq!(folded, Some(want), "{name} in a constant expression");
        }
    }
}

const LAYOUT_PROBES: &str = r#"struct S0 { char a : 4; char b : 4; };
struct S1 { char a : 4; char b : 5; };
struct S2 { int a : 4; char b : 4; };
struct S3 { int a : 30; int b : 2; char c; };
struct S4 { int a : 30; int b : 3; };
struct S5 { short a : 9; short b : 7; int c; };
struct S6 { char a; int b : 1; char c; };
struct S7 { long long a : 40; int b : 20; };
struct S8 { int a : 1; int : 0; int b : 1; };
struct S9 { char a : 1; long long b : 1; };
struct S10 { int a : 16; int b : 16; int c : 1; };
struct S11 { char a : 3; char b : 3; char c : 3; };
struct S12 { int a : 7; int b : 25; char x; };
struct S13 { char a; short b : 3; short c : 13; char x; };
struct S14 { int a : 3; long long b : 61; char x; };
union U0 { char a : 3; int b : 9; };
union U1 { char a; int : 0; short b : 3; };
union U2 { char a; int : 3; };
struct S15 { char a; int : 4; char b; };
union U3 { char a : 2; long long : 7; };
int z_S0 = sizeof(struct S0);
int a_S0 = _Alignof(struct S0);
int z_S1 = sizeof(struct S1);
int a_S1 = _Alignof(struct S1);
int z_S2 = sizeof(struct S2);
int a_S2 = _Alignof(struct S2);
int z_S3 = sizeof(struct S3);
int a_S3 = _Alignof(struct S3);
int z_S4 = sizeof(struct S4);
int a_S4 = _Alignof(struct S4);
int z_S5 = sizeof(struct S5);
int a_S5 = _Alignof(struct S5);
int z_S6 = sizeof(struct S6);
int a_S6 = _Alignof(struct S6);
int z_S7 = sizeof(struct S7);
int a_S7 = _Alignof(struct S7);
int z_S8 = sizeof(struct S8);
int a_S8 = _Alignof(struct S8);
int z_S9 = sizeof(struct S9);
int a_S9 = _Alignof(struct S9);
int z_S10 = sizeof(struct S10);
int a_S10 = _Alignof(struct S10);
int z_S11 = sizeof(struct S11);
int a_S11 = _Alignof(struct S11);
int z_S12 = sizeof(struct S12);
int a_S12 = _Alignof(struct S12);
int z_S13 = sizeof(struct S13);
int a_S13 = _Alignof(struct S13);
int z_S14 = sizeof(struct S14);
int a_S14 = _Alignof(struct S14);
int z_U0 = sizeof(union U0);
int a_U0 = _Alignof(union U0);
int z_U1 = sizeof(union U1);
int a_U1 = _Alignof(union U1);
int z_U2 = sizeof(union U2);
int a_U2 = _Alignof(union U2);
int z_S15 = sizeof(struct S15);
int a_S15 = _Alignof(struct S15);
int z_U3 = sizeof(union U3);
int a_U3 = _Alignof(union U3);
int o_S12 = __builtin_offsetof(struct S12, x);
int o_S13 = __builtin_offsetof(struct S13, x);
int o_S14 = __builtin_offsetof(struct S14, x);
"#;

fn layout_values(target: crate::target::Target) -> Vec<(String, u128)> {
    crate::target::set(target);
    let unit = analyzed(LAYOUT_PROBES);
    crate::target::set(crate::target::Target::build_default());
    unit.globals
        .iter()
        .map(|global| (global.name.clone(), global.int_init().unwrap()))
        .collect()
}

#[test]
fn bit_field_layouts_match_clang_on_sysv_and_msvc() {
    // Mutation testing (reports/hardening/mutation.md) found the MSVC
    // bit-field rules, the exact-fit case of the System V rule and unnamed
    // bit-fields in unions covered only by the cross harness. Sizes,
    // alignments and offsets are what Clang emits for the same records at
    // each target; `U2` and `U3` were wrong on x86-64 and Apple arm64.
    let cases: [(crate::target::Target, &[(&str, u128)]); 4] = [
        (
            crate::target::Target::X86_64Linux,
            &[
                ("z_S0", 1),
                ("a_S0", 1),
                ("z_S1", 2),
                ("a_S1", 1),
                ("z_S2", 4),
                ("a_S2", 4),
                ("z_S3", 8),
                ("a_S3", 4),
                ("z_S4", 8),
                ("a_S4", 4),
                ("z_S5", 8),
                ("a_S5", 4),
                ("z_S6", 4),
                ("a_S6", 4),
                ("z_S7", 8),
                ("a_S7", 8),
                ("z_S8", 8),
                ("a_S8", 4),
                ("z_S9", 8),
                ("a_S9", 8),
                ("z_S10", 8),
                ("a_S10", 4),
                ("z_S11", 2),
                ("a_S11", 1),
                ("z_S12", 8),
                ("a_S12", 4),
                ("z_S13", 6),
                ("a_S13", 2),
                ("z_S14", 16),
                ("a_S14", 8),
                ("z_U0", 4),
                ("a_U0", 4),
                ("z_U1", 2),
                ("a_U1", 2),
                ("z_U2", 1),
                ("a_U2", 1),
                ("z_S15", 3),
                ("a_S15", 1),
                ("z_U3", 1),
                ("a_U3", 1),
                ("o_S12", 4),
                ("o_S13", 4),
                ("o_S14", 8),
            ],
        ),
        (
            crate::target::Target::X86_64Windows { msvc: true },
            &[
                ("z_S0", 1),
                ("a_S0", 1),
                ("z_S1", 2),
                ("a_S1", 1),
                ("z_S2", 8),
                ("a_S2", 4),
                ("z_S3", 8),
                ("a_S3", 4),
                ("z_S4", 8),
                ("a_S4", 4),
                ("z_S5", 8),
                ("a_S5", 4),
                ("z_S6", 12),
                ("a_S6", 4),
                ("z_S7", 16),
                ("a_S7", 8),
                ("z_S8", 8),
                ("a_S8", 4),
                ("z_S9", 16),
                ("a_S9", 8),
                ("z_S10", 8),
                ("a_S10", 4),
                ("z_S11", 2),
                ("a_S11", 1),
                ("z_S12", 8),
                ("a_S12", 4),
                ("z_S13", 6),
                ("a_S13", 2),
                ("z_S14", 24),
                ("a_S14", 8),
                ("z_U0", 4),
                ("a_U0", 1),
                ("z_U1", 2),
                ("a_U1", 1),
                ("z_U2", 4),
                ("a_U2", 1),
                ("z_S15", 12),
                ("a_S15", 4),
                ("z_U3", 8),
                ("a_U3", 1),
                ("o_S12", 4),
                ("o_S13", 4),
                ("o_S14", 16),
            ],
        ),
        (
            crate::target::Target::Aarch64Linux,
            &[
                ("z_S0", 1),
                ("a_S0", 1),
                ("z_S1", 2),
                ("a_S1", 1),
                ("z_S2", 4),
                ("a_S2", 4),
                ("z_S3", 8),
                ("a_S3", 4),
                ("z_S4", 8),
                ("a_S4", 4),
                ("z_S5", 8),
                ("a_S5", 4),
                ("z_S6", 4),
                ("a_S6", 4),
                ("z_S7", 8),
                ("a_S7", 8),
                ("z_S8", 8),
                ("a_S8", 4),
                ("z_S9", 8),
                ("a_S9", 8),
                ("z_S10", 8),
                ("a_S10", 4),
                ("z_S11", 2),
                ("a_S11", 1),
                ("z_S12", 8),
                ("a_S12", 4),
                ("z_S13", 6),
                ("a_S13", 2),
                ("z_S14", 16),
                ("a_S14", 8),
                ("z_U0", 4),
                ("a_U0", 4),
                ("z_U1", 4),
                ("a_U1", 4),
                ("z_U2", 4),
                ("a_U2", 4),
                ("z_S15", 4),
                ("a_S15", 4),
                ("z_U3", 8),
                ("a_U3", 8),
                ("o_S12", 4),
                ("o_S13", 4),
                ("o_S14", 8),
            ],
        ),
        (
            crate::target::APPLE_DEFAULT,
            &[
                ("z_S0", 1),
                ("a_S0", 1),
                ("z_S1", 2),
                ("a_S1", 1),
                ("z_S2", 4),
                ("a_S2", 4),
                ("z_S3", 8),
                ("a_S3", 4),
                ("z_S4", 8),
                ("a_S4", 4),
                ("z_S5", 8),
                ("a_S5", 4),
                ("z_S6", 4),
                ("a_S6", 4),
                ("z_S7", 8),
                ("a_S7", 8),
                ("z_S8", 8),
                ("a_S8", 4),
                ("z_S9", 8),
                ("a_S9", 8),
                ("z_S10", 8),
                ("a_S10", 4),
                ("z_S11", 2),
                ("a_S11", 1),
                ("z_S12", 8),
                ("a_S12", 4),
                ("z_S13", 6),
                ("a_S13", 2),
                ("z_S14", 16),
                ("a_S14", 8),
                ("z_U0", 4),
                ("a_U0", 4),
                ("z_U1", 2),
                ("a_U1", 2),
                ("z_U2", 1),
                ("a_U2", 1),
                ("z_S15", 3),
                ("a_S15", 1),
                ("z_U3", 1),
                ("a_U3", 1),
                ("o_S12", 4),
                ("o_S13", 4),
                ("o_S14", 8),
            ],
        ),
    ];
    for (target, want) in cases {
        let got = layout_values(target);
        for (name, value) in want {
            let found = got.iter().find(|(n, _)| n == name).map(|(_, v)| *v);
            assert_eq!(found, Some(*value), "{target:?} {name}");
        }
    }
}

#[test]
fn union_bit_fields_take_the_bytes_of_their_width() {
    // A bit-field adds the bytes of its width to a union, not its type's
    // size. A packed union whose bit-field unit would then not fit is
    // rejected, like the packed structs above, instead of laid out wrong.
    for source in [
        "union __attribute__((packed)) P1 { int a : 3; };",
        "union __attribute__((packed)) P2 { char c; int a : 12; };",
        "union P3 { char c; int a : 12 __attribute__((packed)); };",
    ] {
        assert!(
            sema_err(source).contains("bit-fields in a packed or aligned union"),
            "{source}"
        );
    }
    sema_ok("union U { char a; long long : 33; }; struct S { char c; union U u; };");
    sema_ok("struct __attribute__((packed)) P4 { char c; union { int a : 3; } u; char d; };");
}

#[test]
fn parser_folds_sizeof_of_brace_elided_arrays() {
    // tests/163: every bound, enumerator and _Static_assert there needs the
    // parser to know the length of an array completed by a brace-elided or
    // designated initializer (C11 6.7.9p20, p22; 6.6p6).
    analyzed(include_str!("../../tests/163_sizeof_brace_elided_array.c"));
    // A struct value's type is not known to the parser; such a length is
    // left to semantic analysis, and `sizeof` of it in a bound is still
    // rejected (a known limitation, reports/hardening/mutation.md).
    let err = sema_err(
        "struct P { int x, y; };\n\
         int f(void) { struct P s = {1, 2}; struct P v[] = {s, s}; char q[sizeof v]; return sizeof q; }\n",
    );
    assert!(err.contains("incomplete type"), "{err}");
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
fn quickjs_gnu_extensions_and_their_limits() {
    use crate::Dialect::{Gnu11, C11};
    let gnu_err = |source: &str| sema_dialect(source, Gnu11).unwrap_err();
    // A struct or union cast to its own type: GNU only.
    let cast = "struct S { int a; }; int f(struct S s) { return ((const struct S)s).a; }";
    assert!(sema_dialect(cast, Gnu11).is_ok());
    assert!(sema_dialect(cast, C11)
        .unwrap_err()
        .contains("is not supported"));
    assert!(gnu_err(
        "struct S { int a; }; struct T { int a; }; \
                     int f(struct S s) { return ((struct T)s).a; }"
    )
    .contains("is not supported"));
    // A forward-named enum must end up with the type it was assumed to have.
    assert!(sema_dialect("typedef enum E E; enum E { A, B }; E e = B;", Gnu11).is_ok());
    assert!(sema_dialect("typedef enum E E; enum E { A };", C11)
        .unwrap_err()
        .contains("incomplete enum"));
    assert!(gnu_err("typedef enum E E; enum E { A = -1 };")
        .contains("was used before its definition as 'unsigned int', but its type is 'int'"));
    // Range designators: constant nonnegative bounds, a nonempty range, and
    // a value without side effects.
    assert!(sema_dialect("int a[4] = { [1 ... 2] = 3 };", Gnu11).is_ok());
    assert!(
        gnu_err("int f(void); void g(void) { int a[3] = { [0 ... 2] = f() }; }")
            .contains("must have no side effects")
    );
    assert!(gnu_err("int a[4] = { [2 ... 1] = 3 };").contains("empty range designator"));
    assert!(gnu_err("int a[4] = { [-1 ... 1] = 3 };").contains("nonnegative integer constant"));
    assert!(gnu_err("void g(int n) { int a[4] = { [0 ... n] = 3 }; }")
        .contains("nonnegative integer constant"));
    assert!(gnu_err("int a[1 << 22] = { [0 ... (1 << 21)] = 1 };").contains("too large"));
    // The budget covers the expanded tree, so nested ranges multiply.
    assert!(
        gnu_err("int a[1048576][1048576] = { [0 ... 1048575] = { [0 ... 1048575] = 1 } };")
            .contains("too large")
    );
    assert!(sema_dialect(
        "struct S { int x[4]; }; struct S a[3] = { [0 ... 2] = { .x = { [0 ... 3] = 5 } } };",
        Gnu11
    )
    .is_ok());
    // A value is evaluated once (GCC); repeating a volatile read would not be.
    for value in [
        "v",
        "*p",
        "v + 1",
        "*(volatile int *)&n",
        "s.m",
        "*********q",
        "r->next->m",
    ] {
        let source = format!(
            "volatile int v; volatile int *p; int n; struct {{ volatile int m; }} s;\n\
             volatile int *********q; struct R {{ struct R *next; volatile int m; }} *r;\n\
             int f(void) {{ int a[2] = {{ [0 ... 1] = {value} }}; return a[1]; }}"
        );
        assert!(
            gnu_err(&source).contains("must not read a volatile object"),
            "{value}"
        );
    }
    assert!(sema_dialect(
        "int n; struct L { struct L *next; int m; } *l;\n\
         int f(void) { int a[2] = { [0 ... 1] = n + l->next->m }; return a[1]; }",
        Gnu11
    )
    .is_ok());
    // An endpoint is checked as a 64-bit value on every target, including
    // the MSVC one, where `long` is 32 bits and would wrap 2^32 to 0.
    for target in [
        crate::target::Target::X86_64Linux,
        crate::target::Target::X86_64Windows { msvc: true },
    ] {
        let saved = crate::target::current();
        crate::target::set(target);
        let err = gnu_err("int a[1] = { [4294967296LL ... 4294967296LL] = 7 };");
        crate::target::set(saved);
        assert!(err.contains("out of range"), "{target:?}: {err}");
    }
    assert!(sema_dialect("int a[4] = { [1 ... 2] = 3 };", C11).is_err());
    // Zero-size records are GNU only.
    assert!(sema_dialect("struct Z { char a[0]; }; struct Z z = {};", Gnu11).is_ok());
    assert!(sema_dialect("struct Z { char a[0]; }; struct Z z = {0};", Gnu11).is_ok());
    for init in ["{1, 2}", "{1}", "{{0}}"] {
        let source = format!("struct Z {{ char a[0]; }}; struct Z z = {init};");
        assert!(gnu_err(&source).contains("excess elements"), "{init}");
    }
    assert!(sema_dialect("struct Z { char a[0]; };", C11)
        .unwrap_err()
        .contains("array size must be positive"));
    // Array parameter qualifiers and `static`, in both dialects.
    sema_ok("int f(int a[static const 2]) { return a[1]; }");
    assert!(sema_err("int f(int a[const 2]) { a = 0; return 0; }").contains("const"));
    assert!(sema_err("int f(int a[static]);").contains("requires a size"));
    assert!(sema_err("int f(int a[2][static 2]);").contains("outermost array type"));
    assert!(sema_err("int a[static 2];").contains("outermost array type"));
    // asm statements: no operands, no goto.
    assert!(sema_dialect(
        "void f(void) { __asm__ volatile(\"\" ::: \"memory\"); }",
        C11
    )
    .is_ok());
    assert!(gnu_err("void f(int x) { asm(\"\" : : \"r\"(x)); }").contains("operands"));
    assert!(gnu_err("void f(void) { asm(\"%0\" :::); }").contains("operand references"));
    assert!(gnu_err("void f(void) { asm goto(\"\" :::: l); l:; }").contains("asm goto"));
    // A comma in the clobber list must be followed by another clobber.
    assert!(gnu_err("void f(void) { asm(\"\" ::: \"memory\",); }").contains("clobber"));
    assert!(sema_dialect(
        "void f(void) { asm(\"\" ::: \"memory\", \"cc\"); asm(\"\" :::); }",
        Gnu11
    )
    .is_ok());
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
    assert!(sema_err(
        "void f(int n, int x) { switch (x) { for (int a[n]; ;) { case 0: a[0] = 1; } } }"
    )
    .contains("variably modified"));
    sema_ok("void f(int n) { (int (*)[n]){0}; }");
    assert!(sema_err("void f(void) { (int (*)[1.0]){0}; }").contains("integer"));
}

#[test]
fn atomic_types_and_lock_free_sizes() {
    crate::compile_to_ir(
        "struct Node { struct Node *next; int v; } n; int main(void) { return 0; }\n",
        "t.c",
    )
    .unwrap();
    crate::compile_to_ir(
        "_Atomic int x; int main(void) { x = 1; return x; }\n",
        "t.c",
    )
    .unwrap();
    crate::compile_to_ir(
        "_Atomic(int *) p; int main(void) { p = 0; return p == 0; }\n",
        "t.c",
    )
    .unwrap();
    let err = sema_err("struct Big { char a[32]; }; _Atomic struct Big x;\n");
    assert!(err.contains("not lock-free"), "{err}");
    let err = sema_err("_Atomic int a[3]; struct S { char a[3]; }; _Atomic struct S x;\n");
    assert!(err.contains("not lock-free"), "{err}");
    let err = sema_err("struct Big { char a[24]; }; _Atomic(struct Big) **p;\n");
    assert!(err.contains("not lock-free"), "{err}");
    let err = sema_err("struct Big { char a[24]; }; void f(void) { (_Atomic(struct Big)){0}; }\n");
    assert!(err.contains("not lock-free"), "{err}");
    let err = sema_err("struct Big { char a[24]; }; void f(_Atomic struct Big x);\n");
    assert!(err.contains("not lock-free"), "{err}");
    crate::compile_to_ir("struct Inc; void f(_Atomic struct Inc *p);\n", "t.c").unwrap();
    let align = crate::compile_to_ir(
        "struct S { char b[8]; }; int main(void) { return _Alignof(_Atomic struct S); }\n",
        "t.c",
    )
    .unwrap();
    assert!(align.contains("i64 8"), "{align}");
    let err = sema_dialect(
        "struct __attribute__((packed)) W { char c; _Atomic int x; };\n",
        crate::Dialect::Gnu11,
    )
    .unwrap_err();
    assert!(
        err.contains("not aligned") || err.contains("lock-free"),
        "{err}"
    );
    crate::compile_dialect(
        "struct __attribute__((packed, aligned(4))) S { _Atomic int x; };\n",
        "t.c",
        &crate::PpOptions::default(),
        crate::Dialect::Gnu11,
    )
    .unwrap();
    let err = sema_dialect(
        "struct __attribute__((packed, aligned(4))) S { char c; _Atomic int x; };\n",
        crate::Dialect::Gnu11,
    )
    .unwrap_err();
    assert!(
        err.contains("not aligned") || err.contains("lock-free"),
        "{err}"
    );
    let err = sema_err("struct Big { char a[24]; }; _Atomic(struct Big) *f(void);\n");
    assert!(err.contains("not lock-free"), "{err}");
    crate::compile_to_ir("struct Inc; _Atomic struct Inc *f(void);\n", "t.c").unwrap();
    let err = sema_err("struct Big { char a[24]; }; int n = sizeof(_Atomic(struct Big));\n");
    assert!(err.contains("not lock-free"), "{err}");
    let err = crate::compile_to_ir(
        "#include <stdatomic.h>\nstruct S; int main(void) { _Atomic(struct S *) p; (void)atomic_fetch_add(&p, 1); }\n",
        "t.c",
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("complete object type") || err.contains("incomplete"),
        "{err}"
    );
    let err = crate::compile_to_ir(
        "_Atomic(int *) p; void f(void) { __hcc_atomic_flag_clear(&p, 0); }\n",
        "t.c",
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("unsigned char") || err.contains("atomic_flag"),
        "{err}"
    );
    let err = crate::compile_to_ir(
        "_Atomic int x; int f(void) { return __hcc_atomic_load(&x, 18446744073709551615ULL); }\n",
        "t.c",
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("invalid memory_order"), "{err}");
    let err = sema_err(
        "struct Big { char a[24]; }; void *p; void f(void) { (void)(_Atomic(struct Big) *)p; }\n",
    );
    assert!(err.contains("not lock-free"), "{err}");
    crate::compile_to_ir(
        "struct Inc; void *p; void f(void) { (void)(_Atomic struct Inc *)p; }\n",
        "t.c",
    )
    .unwrap();
}

#[test]
fn stdatomic_header_and_constraints() {
    crate::compile_to_ir(
        "#include <stdatomic.h>\n         atomic_int x;\n         int main(void) {\n             atomic_init(&x, 1);\n             atomic_store(&x, 2);\n             int v = atomic_load(&x);\n             v += atomic_fetch_add(&x, 3);\n             int e = 5;\n             atomic_compare_exchange_strong(&x, &e, 9);\n             atomic_thread_fence(memory_order_seq_cst);\n             atomic_signal_fence(memory_order_acquire);\n             atomic_flag f = ATOMIC_FLAG_INIT;\n             (void)atomic_flag_test_and_set(&f);\n             atomic_flag_clear(&f);\n             return v + atomic_is_lock_free(&x);\n         }\n",
        "t.c",
    )
    .unwrap();
    let err = crate::compile_to_ir(
        "#include <stdatomic.h>\nint x; int main(void) { return atomic_load(&x); }\n",
        "t.c",
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("atomic type"), "{err}");
    let err = crate::compile_to_ir(
        "#include <stdatomic.h>\natomic_int x; int main(void) { atomic_store_explicit(&x, 1, memory_order_acquire); }\n",
        "t.c",
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("not valid"), "{err}");
    let err = crate::compile_to_ir(
        "#include <stdatomic.h>\natomic_int x; int e; int main(void) {\n         atomic_compare_exchange_strong_explicit(&x, &e, 1, memory_order_relaxed, memory_order_acquire); }\n",
        "t.c",
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("stronger"), "{err}");
    crate::compile_to_ir(
        "#include <stdatomic.h>\natomic_flag f; int main(void) { atomic_flag_clear_explicit(&f, memory_order_consume); return 0; }\n",
        "t.c",
    )
    .unwrap();
    let err = crate::compile_to_ir(
        "#include <stdatomic.h>\natomic_flag f; int main(void) { atomic_flag_clear_explicit(&f, memory_order_acquire); return 0; }\n",
        "t.c",
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("not valid"), "{err}");
    let err = crate::compile_to_ir(
        "#include <stdatomic.h>\natomic_int x, e; int main(void) { return atomic_compare_exchange_strong(&x, &e, 1); }\n",
        "t.c",
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("non-atomic"), "{err}");
}

#[test]
fn bitfield_with_overaligned_member_is_accepted() {
    // C11 6.7.2.1 and 6.7.5: a bit-field may sit next to an _Alignas
    // member. Extra alignment inserts padding after the bit-field unit;
    // the bit-field itself stays at its natural offset. A GNU packed
    // record that would move packing remains unsupported.
    let unit = analyzed(
        "\
struct Inner { unsigned int b : 3; _Alignas(16) int value; };
struct Outer { char lead; struct Inner inner; char tail; };
",
    );
    let inner = unit
        .structs
        .iter()
        .find(|handle| handle.tag() == "Inner")
        .expect("struct Inner");
    assert_eq!(inner.align(), 16);
    assert_eq!(inner.size(), Some(32));
    let ifields = inner.fields().expect("complete");
    assert_eq!(ifields[0].name, "b");
    assert_eq!(ifields[0].offset, 0);
    assert_eq!(ifields[0].bit_width, Some(3));
    assert_eq!(ifields[1].name, "value");
    assert_eq!(ifields[1].offset, 16);
    let outer = unit
        .structs
        .iter()
        .find(|handle| handle.tag() == "Outer")
        .expect("struct Outer");
    assert_eq!(outer.align(), 16);
    assert_eq!(outer.size(), Some(64));
    let ofields = outer.fields().expect("complete");
    assert_eq!(ofields[0].offset, 0);
    assert_eq!(ofields[1].name, "inner");
    assert_eq!(ofields[1].offset, 16);
    assert_eq!(ofields[2].name, "tail");
    assert_eq!(ofields[2].offset, 48);

    sema_ok("struct I { unsigned int b : 3; _Alignas(8) int v; };");
    sema_ok("struct I { unsigned int b : 3; _Alignas(32) int v; };");
    sema_ok("struct I { unsigned int b : 3; _Alignas(64) int v; };");
    assert!(
        sema_err("struct __attribute__((packed)) S { char c; int b : 3; };")
            .contains("bit-fields in a packed or aligned")
    );
    // A trailing zero-width field is only an alignment marker, not a
    // storage unit. It may sit at the end of the record.
    let unit =
        analyzed("struct S { unsigned int b : 3; _Alignas(8) int v[2]; unsigned int : 0; };");
    let s = unit
        .structs
        .iter()
        .find(|handle| handle.tag() == "S")
        .expect("struct S");
    assert_eq!(s.align(), 8);
    assert_eq!(s.size(), Some(16));
}

#[test]
fn bitfields_around_overaligned_member_are_accepted() {
    // C11 6.7.2.1 and 6.7.5: a bit-field may precede and follow an
    // _Alignas member. Extra alignment inserts padding after the first
    // unit; the trailing bit-field starts a new unit after the aligned
    // member. A GNU packed record that would move packing remains
    // unsupported.
    let unit = analyzed(
        "struct Inner { unsigned int a : 3; _Alignas(16) int x; unsigned int b : 5; };
struct Outer { char lead; struct Inner inner; char tail; };
",
    );
    let inner = unit
        .structs
        .iter()
        .find(|handle| handle.tag() == "Inner")
        .expect("struct Inner");
    assert_eq!(inner.align(), 16);
    assert_eq!(inner.size(), Some(32));
    let ifields = inner.fields().expect("complete");
    assert_eq!(ifields[0].name, "a");
    assert_eq!(ifields[0].offset, 0);
    assert_eq!(ifields[0].bit_width, Some(3));
    assert_eq!(ifields[1].name, "x");
    assert_eq!(ifields[1].offset, 16);
    assert_eq!(ifields[2].name, "b");
    assert_eq!(ifields[2].offset, 20);
    assert_eq!(ifields[2].bit_width, Some(5));
    let outer = unit
        .structs
        .iter()
        .find(|handle| handle.tag() == "Outer")
        .expect("struct Outer");
    assert_eq!(outer.align(), 16);
    assert_eq!(outer.size(), Some(64));
    let ofields = outer.fields().expect("complete");
    assert_eq!(ofields[0].offset, 0);
    assert_eq!(ofields[1].name, "inner");
    assert_eq!(ofields[1].offset, 16);
    assert_eq!(ofields[2].name, "tail");
    assert_eq!(ofields[2].offset, 48);

    let unit = analyzed(
        "struct S { unsigned int a : 3; unsigned int : 0; _Alignas(8) int x; unsigned int b : 5; };",
    );
    let s = unit
        .structs
        .iter()
        .find(|handle| handle.tag() == "S")
        .expect("struct S");
    assert_eq!(s.align(), 8);
    assert_eq!(s.size(), Some(16));
    let fields = s.fields().expect("complete");
    let named: Vec<_> = fields.iter().filter(|f| !f.name.is_empty()).collect();
    assert_eq!(named[0].name, "a");
    assert_eq!(named[0].offset, 0);
    assert_eq!(named[1].name, "x");
    assert_eq!(named[1].offset, 8);
    assert_eq!(named[2].name, "b");
    assert_eq!(named[2].offset, 12);
    assert_eq!(named[2].bit_width, Some(5));

    sema_ok("struct I { unsigned int a : 3; _Alignas(8) int x; unsigned int b : 5; };");
    sema_ok("struct I { unsigned int a : 3; _Alignas(32) int x; unsigned int b : 5; };");
    sema_ok("struct I { unsigned int a : 3; _Alignas(64) int x; unsigned int b : 5; };");
    assert!(
        sema_err("struct __attribute__((packed)) S { char c; int b : 3; };")
            .contains("bit-fields in a packed or aligned")
    );
    // A member-level packed attribute can also move a later bit-field.
    // _Alignas padding must not hide that movement.
    assert!(
        sema_err("struct S { char c; int x __attribute__((packed)); unsigned int b : 5; _Alignas(16) int tail; };")
            .contains("bit-fields in a packed or aligned")
    );
}

#[test]
fn address_constants_through_integer_pointers() {
    // C11 6.6p9: `[]`, `->`, `.`, `&` and `*` may build an address constant
    // from an integer constant cast to a pointer, the pre-offsetof idiom.
    // Found by the coverage tests: these were rejected as not constant.
    let ir = crate::compile_to_ir(
        "struct S { int a; short b[4]; struct { char c; long d; } in; };\n\
         long o1 = (long)&((struct S *)0)->b;\n\
         short *o2 = &((struct S *)0)->b[2];\n\
         struct S *o3 = &((struct S *)0x100)[1];\n\
         short *o4 = ((struct S *)0x100)->b;\n\
         char *o5 = &(*(struct S *)0x200).in.c;\n\
         int *o6 = &*(int *)0;\n",
        "t.c",
    )
    .unwrap();
    assert!(ir.contains("@o1 = global i64 4"), "{ir}");
    assert!(
        ir.contains("@o2 = global ptr inttoptr (i64 8 to ptr)"),
        "{ir}"
    );
    assert!(
        ir.contains("@o3 = global ptr inttoptr (i64 288 to ptr)"),
        "{ir}"
    );
    assert!(
        ir.contains("@o4 = global ptr inttoptr (i64 260 to ptr)"),
        "{ir}"
    );
    assert!(
        ir.contains("@o5 = global ptr inttoptr (i64 528 to ptr)"),
        "{ir}"
    );
    assert!(ir.contains("@o6 = global ptr null"), "{ir}");
    // A label address still may not be offset, and an object's value is
    // still not a constant.
    assert!(sema_err("int x; int *p = &*(&x + (x - x));").contains("not a constant"));
}

#[test]
fn atomic_builtin_argument_constraints() {
    // C11 7.17: each generic function needs a pointer to a non-const
    // atomic object; compare-exchange needs a pointer to a non-const,
    // non-atomic object of the same type for `expected`.
    let err = |body: &str| {
        crate::compile_to_ir(
            &format!(
                "#include <stdatomic.h>\nconst atomic_int c; atomic_int x; atomic_bool b; atomic_flag f;\n\
                 const atomic_flag cf; int e; long l; const int ce;\nvoid t(void) {{ {body}; }}\n"
            ),
            "t.c",
        )
        .unwrap_err()
        .to_string()
    };
    let non_const = "must be a pointer to a non-const atomic type";
    assert!(err("atomic_init(&c, 1)").contains(non_const));
    assert!(err("atomic_store(&c, 1)").contains(non_const));
    assert!(err("atomic_exchange(&c, 1)").contains(non_const));
    assert!(err("atomic_compare_exchange_strong(&c, &e, 1)").contains(non_const));
    assert!(err("atomic_fetch_add(&c, 1)").contains(non_const));
    assert!(err("atomic_fetch_or(&c, 1)").contains(non_const));
    assert!(err("atomic_flag_clear(&cf)").contains(non_const));
    assert!(err("atomic_compare_exchange_strong(&x, e, 1)")
        .contains("expected-value argument of __hcc_atomic_compare_exchange must be a pointer"));
    assert!(
        err("atomic_compare_exchange_strong(&x, &ce, 1)").contains("pointer to a non-const type")
    );
    assert!(err("atomic_compare_exchange_strong(&x, &l, 1)").contains("must be a pointer to 'int'"));
    assert!(err("__hcc_atomic_compare_exchange(&x, &e, 1, 5, 5, &e)")
        .contains("weak argument of __hcc_atomic_compare_exchange must be an integer"));
    assert!(err("atomic_fetch_add(&b, 1)").contains("not applicable to _Atomic _Bool"));
    assert!(err("atomic_fetch_sub(&b, 1)").contains("not applicable to _Atomic _Bool"));
    assert!(err("__hcc_atomic_load(&x)").contains("expects 2 arguments, found 1"));
    assert!(err("__hcc_atomic_is_lock_free(&x, 1)").contains("expects 1 argument, found 2"));
}

#[test]
fn call_and_va_start_constraints() {
    assert!(sema_err("int f(int, int); int g(void) { return f(1); }")
        .contains("function 'f' expects 2 arguments, found 1"));
    assert!(sema_err("int f(int, ...); int g(void) { return f(); }")
        .contains("function 'f' expects at least 1 argument, found 0"));
    // Through a function pointer the message has no name.
    assert!(
        sema_err("int (*fp)(int, int); int g(void) { return fp(1); }")
            .contains("function expects 2 arguments, found 1")
    );
    assert!(
        sema_err("int (*fp)(int, ...); int g(void) { return fp(); }")
            .contains("function expects at least 1 argument, found 0")
    );
    assert!(sema_err("int (*fp)(int); int g(void) { return fp(1, 2); }")
        .contains("function expects 1 argument, found 2"));
    let ir_err = |src: &str| crate::compile_to_ir(src, "t.c").unwrap_err().to_string();
    assert!(ir_err(
        "#include <stdarg.h>\nvoid f(int a) { va_list ap; va_start(ap, a); va_end(ap); }\n"
    )
    .contains("va_start used in a function that is not variadic"));
    assert!(ir_err("#include <stdarg.h>\nvoid f(int a, int b, ...) { va_list ap; va_start(ap, a); va_end(ap); }\n")
        .contains("the second argument of va_start must be the last named parameter"));
    assert!(
        ir_err("int main(void) { return __builtin_popcount(1, 2); }\n")
            .contains("expects 1 argument, found 2")
    );
    assert!(
        ir_err("int main(void) { return __builtin_add_overflow(1, 2, 0); }\n")
            .contains("must be a pointer to a non-const integer other than _Bool")
    );
    assert!(
        ir_err("int main(void) { _Bool b; return __builtin_mul_overflow(1, 2, &b); }\n")
            .contains("other than _Bool")
    );
    assert!(
        ir_err("double sqrt(double); int main(void) { return sqrt(1.0, 2.0); }\n")
            .contains("function 'sqrt' expects 1 argument, found 2")
    );
}
