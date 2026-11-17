//! Internal compiler errors in IR generation. Each test checks a valid
//! program, damages the checked syntax tree the way a bug in semantic
//! analysis could, and expects `render` to report an internal error at the
//! right position instead of panicking.

use super::render;
use crate::ast::*;
use crate::dialect::Dialect;

fn checked(source: &str) -> TranslationUnit {
    let tokens = crate::pp::preprocess_tokens(source, "t.c", &crate::PpOptions::default())
        .unwrap_or_else(|err| panic!("{err}"));
    let mut unit = crate::parser::parse_with(&tokens, "t.c", Dialect::Gnu11, false)
        .unwrap_or_else(|err| panic!("{err}"));
    crate::sema::analyze_dialect(&mut unit, "t.c", &tokens, Dialect::Gnu11)
        .unwrap_or_else(|err| panic!("{err}"));
    unit
}

/// The top-level statements of the body of function `name`.
fn body<'a>(unit: &'a mut TranslationUnit, name: &str) -> &'a mut Vec<Stmt> {
    for decl in &mut unit.decls {
        if let TopDecl::Func(func) = decl {
            if func.name == name {
                if let Some(body) = &mut func.body {
                    return &mut body.items;
                }
            }
        }
    }
    panic!("no function {name}");
}

/// The expression of the expression statement or `return` at `index`.
fn stmt_expr<'a>(unit: &'a mut TranslationUnit, name: &str, index: usize) -> &'a mut Expr {
    match &mut body(unit, name)[index] {
        Stmt::Expr {
            expr: Some(expr), ..
        }
        | Stmt::Return {
            expr: Some(expr), ..
        } => expr,
        other => panic!("statement {index} has no expression: {other:?}"),
    }
}

/// The internal error text, after checking that it is one.
fn ice(unit: &TranslationUnit) -> String {
    match render(unit, "t.c") {
        Ok(ir) => panic!("rendered:\n{ir}"),
        Err(diag) => {
            assert!(diag.is_internal(), "{diag}");
            assert_eq!(diag.phase, Some("irgen"));
            diag.to_string()
        }
    }
}

#[test]
fn valid_program_renders() {
    let unit = checked("int f(int x) { while (x) { if (x > 3) break; x--; } return x; }");
    assert!(render(&unit, "t.c").is_ok());
}

#[test]
fn break_outside_loop_names_its_position() {
    let mut unit = checked("int f(int x) {\n  while (x) {\n    break;\n  }\n  return x;\n}");
    let Stmt::While {
        body: loop_body, ..
    } = &body(&mut unit, "f")[0]
    else {
        panic!("while");
    };
    let Stmt::Compound(inner) = &**loop_body else {
        panic!("loop body");
    };
    let brk = inner.items[0].clone();
    body(&mut unit, "f")[0] = brk;
    assert_eq!(
        ice(&unit),
        "t.c:3:5: internal compiler error: 'break' is not inside a loop or switch (in irgen)"
    );
}

#[test]
fn continue_outside_loop() {
    let mut unit = checked("int f(int x) { switch (x) { default: break; } return x; }");
    body(&mut unit, "f")[0] = Stmt::Continue { line: 7, column: 2 };
    assert_eq!(
        ice(&unit),
        "t.c:7:2: internal compiler error: 'continue' is not inside a loop (in irgen)"
    );
}

#[test]
fn switch_without_promoted_type() {
    let mut unit = checked("int f(int x) { switch (x) { case 1: return 2; } return x; }");
    let Stmt::Switch { promoted, .. } = &mut body(&mut unit, "f")[0] else {
        panic!("switch");
    };
    *promoted = None;
    assert!(ice(&unit).contains("switch has no promoted controlling type"));
}

#[test]
fn case_without_value() {
    let mut unit = checked("int f(int x) { switch (x) {\ncase 1: return 2; } return x; }");
    let Stmt::Switch { body: sw, .. } = &mut body(&mut unit, "f")[0] else {
        panic!("switch");
    };
    let Stmt::Compound(items) = &mut **sw else {
        panic!("switch body");
    };
    let Stmt::Case { value, .. } = &mut items.items[0] else {
        panic!("case");
    };
    *value = None;
    assert_eq!(
        ice(&unit),
        "t.c:2:1: internal compiler error: case label has no value (in irgen)"
    );
}

#[test]
fn record_expression_without_address() {
    // `x + x` given a struct type has no object to point at.
    let mut unit = checked("struct S { int a; } g;\nvoid f(int x) { x + x; }");
    let record = match &unit.globals[0].ty {
        ty if ty.is_record() => ty.clone(),
        other => panic!("{other:?}"),
    };
    stmt_expr(&mut unit, "f", 0).set_ty(record);
    assert_eq!(
        ice(&unit),
        "t.c:2:19: internal compiler error: struct or union expression has no address (in irgen)"
    );
}

#[test]
fn address_of_a_non_lvalue() {
    let mut unit = checked("int g;\nint *f(void) { return &g; }");
    let Expr::Unary { expr, .. } = stmt_expr(&mut unit, "f", 0) else {
        panic!("&g");
    };
    **expr = Expr::int(1, TypeName::Int, 2, 24);
    assert_eq!(
        ice(&unit),
        "t.c:2:24: internal compiler error: address of an expression that is not an lvalue (in irgen)"
    );
}

#[test]
fn void_expression_used_as_a_value() {
    let mut unit = checked("void g(void);\nint f(void) { return (g(), 1); }");
    let Expr::Comma { left, .. } = stmt_expr(&mut unit, "f", 0).clone() else {
        panic!("comma");
    };
    *stmt_expr(&mut unit, "f", 0) = *left;
    assert_eq!(
        ice(&unit),
        "t.c:2:23: internal compiler error: void expression used as a value (in irgen)"
    );
}

#[test]
fn member_without_index_or_offset() {
    let source = "struct S { int a, b; };\nint f(struct S *p) { return p->b; }";
    let mut unit = checked(source);
    let Expr::Member { index, .. } = stmt_expr(&mut unit, "f", 0) else {
        panic!("member");
    };
    *index = None;
    assert!(ice(&unit).contains("member access has no record type or index"));

    let mut unit = checked(source);
    let Expr::Member {
        bytewise, offset, ..
    } = stmt_expr(&mut unit, "f", 0)
    else {
        panic!("member");
    };
    *bytewise = true;
    *offset = None;
    assert!(ice(&unit).contains("member access has no byte offset"));
}

#[test]
fn string_literal_without_symbol() {
    let mut unit = checked("const char *f(void) { return \"hi\"; }");
    let expr = stmt_expr(&mut unit, "f", 0);
    let Expr::Decay { expr: inner, .. } = expr else {
        panic!("decay: {expr:?}");
    };
    let Expr::Str { symbol, .. } = &mut **inner else {
        panic!("string");
    };
    *symbol = None;
    assert!(ice(&unit).contains("string literal has no symbol"));
}

#[test]
fn sizeof_incomplete_type() {
    let mut unit = checked("unsigned long f(void) { return sizeof(int); }");
    let Expr::Sizeof { operand, .. } = stmt_expr(&mut unit, "f", 0) else {
        panic!("sizeof");
    };
    *operand = SizeofOperand::Type(CType::Void);
    assert!(ice(&unit).contains("sizeof of an incomplete type"));
}

#[test]
fn global_initializer_of_the_wrong_kind_has_no_position() {
    let mut unit = checked("double d = 1.0;\nint main(void) { return 0; }");
    unit.globals[0].init = Some(ConstInit::Int(1));
    assert_eq!(
        ice(&unit),
        "t.c: internal compiler error: integer initializer for an object of another type (in irgen)"
    );
    unit.globals[0].ty = CType::int_type();
    unit.globals[0].init = Some(ConstInit::Float(0));
    assert!(ice(&unit).contains("floating initializer for an object of another type"));
}

#[test]
fn only_the_first_internal_error_is_reported() {
    let mut unit = checked("int f(void) { return 1; }\nint g(void) { return 2; }");
    body(&mut unit, "f")[0] = Stmt::Break {
        line: 1,
        column: 15,
    };
    body(&mut unit, "g")[0] = Stmt::Continue {
        line: 2,
        column: 15,
    };
    assert!(ice(&unit).starts_with("t.c:1:15:"));
}

fn pointer_to(ty: CType) -> CType {
    CType::Pointer(Box::new(ty))
}

/// The operand of the unary or `++`/`--` expression `expr`, or the
/// right-hand side of a compound assignment.
fn operand(expr: &mut Expr) -> &mut Expr {
    match expr {
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => expr,
        Expr::IncDec { target, .. } => target,
        Expr::CompoundAssign { value, .. } => value,
        other => panic!("no operand: {other:?}"),
    }
}

#[test]
fn operators_on_operands_of_the_wrong_type() {
    let mut unit = checked("int f(int x) { return +x; }");
    operand(stmt_expr(&mut unit, "f", 0)).set_ty(pointer_to(CType::int_type()));
    assert!(ice(&unit).contains("unary '+' on an operand that is not arithmetic"));

    let mut unit = checked("int f(int x) { return -x; }");
    operand(stmt_expr(&mut unit, "f", 0)).set_ty(pointer_to(CType::int_type()));
    assert!(ice(&unit).contains("integer operator on an operand that is not an integer"));

    let mut unit = checked("struct S { int a; } g;\nvoid f(int x) { x++; }");
    let record = unit.globals[0].ty.clone();
    operand(stmt_expr(&mut unit, "f", 0)).set_ty(record);
    assert!(ice(&unit).contains("'++' or '--' on an operand that is not a scalar"));

    let mut unit = checked("void f(int x) { x += 1; }");
    operand(stmt_expr(&mut unit, "f", 0)).set_ty(pointer_to(CType::int_type()));
    assert!(ice(&unit).contains("compound assignment operands are not arithmetic"));
}

#[test]
fn pointer_difference_with_an_incomplete_pointee() {
    let mut unit = checked("long f(int *p, int *q) { return p - q; }");
    let Expr::Bin { left, .. } = stmt_expr(&mut unit, "f", 0) else {
        panic!("p - q");
    };
    left.set_ty(pointer_to(CType::Void));
    assert!(ice(&unit).contains("pointer subtraction with an incomplete pointee type"));
}

#[test]
fn label_address_outside_a_function() {
    let mut unit = checked("void *f(void) { l: return &&l; }");
    let Stmt::Label { stmt, .. } = &mut body(&mut unit, "f")[0] else {
        panic!("label");
    };
    let Stmt::Return {
        expr: Some(Expr::LabelAddr { func, .. }),
        ..
    } = &mut **stmt
    else {
        panic!("return &&l");
    };
    *func = None;
    assert!(ice(&unit).contains("label address outside a function"));
}

#[test]
fn overflow_builtin_result_of_the_wrong_type() {
    let mut unit =
        checked("_Bool f(int a, int b, int *r) { return __builtin_add_overflow(a, b, r); }");
    let Expr::Call { args, .. } = stmt_expr(&mut unit, "f", 0) else {
        panic!("call");
    };
    args[2].set_ty(pointer_to(CType::Float(FloatKind::Double)));
    assert!(ice(&unit).contains("overflow result is not a pointer to an integer"));
}

#[test]
fn an_error_inside_one_arm_of_a_conditional_is_reported() {
    // The damaged arm yields no value and the other does; the first error
    // is reported rather than a panic on the mismatch.
    let mut unit = checked("int f(int c) {\n  return c ? ({ 1; }) : 2;\n}");
    let Expr::Cond { then_expr, .. } = stmt_expr(&mut unit, "f", 0) else {
        panic!("conditional");
    };
    let Expr::StmtExpr { body, .. } = &mut **then_expr else {
        panic!("statement expression: {then_expr:?}");
    };
    body.items[0] = Stmt::Break {
        line: 2,
        column: 16,
    };
    assert_eq!(
        ice(&unit),
        "t.c:2:16: internal compiler error: 'break' is not inside a loop or switch (in irgen)"
    );
}
