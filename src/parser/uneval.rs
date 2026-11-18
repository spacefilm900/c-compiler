//! Checks on operands that are not evaluated (`sizeof`, `_Alignof`,
//! `_Generic`), using only their types.

use super::*;

/// Check every association expression of a generic selection, and the
/// controlling expression, for constraints and name resolution. The
/// expressions are not evaluated (C11 6.5.1.1p3). This runs in the parser
/// because an integer constant expression is folded before semantic
/// analysis sees the `_Generic` node.
pub(super) fn check_generic_assocs(
    control: &Expr,
    assocs: &[GenericAssoc],
    env: Env,
) -> Result<(), BoundErr> {
    check_uneval(control, env)?;
    let ty = rvalue_type(control, env)?;
    let mut default = false;
    let mut chosen = None;
    for (index, assoc) in assocs.iter().enumerate() {
        let Some(assoc_ty) = &assoc.ty else {
            if default {
                return Err(BoundErr::GenericDupDefault);
            }
            default = true;
            continue;
        };
        if assoc_ty.is_func() || !assoc_ty.is_complete() {
            return Err(BoundErr::GenericBadType);
        }
        for earlier in assocs[..index].iter().filter_map(|a| a.ty.as_ref()) {
            if compatible(earlier, assoc_ty) {
                return Err(BoundErr::GenericDupType);
            }
        }
        if chosen.is_none() && compatible(assoc_ty, &ty) {
            chosen = Some(index);
        }
    }
    if chosen.is_none() && !default {
        return Err(BoundErr::GenericNoMatch);
    }
    for assoc in assocs {
        check_uneval(&assoc.expr, env)?;
    }
    Ok(())
}

/// Name resolution and call-arity constraints in an expression that is not
/// evaluated, including an unselected `_Generic` association.
pub(super) fn check_uneval(expr: &Expr, env: Env) -> Result<(), BoundErr> {
    match expr {
        Expr::Int { .. } | Expr::Float { .. } | Expr::Str { .. } => Ok(()),
        Expr::Var { name, .. } => {
            if env_lookup(env, name).is_some() {
                Ok(())
            } else {
                Err(BoundErr::Undeclared)
            }
        }
        Expr::Bin {
            op, left, right, ..
        } => {
            check_uneval(left, env)?;
            check_uneval(right, env)?;
            if op.is_cmp() {
                check_uneval_compare(*op, left, right, env)?;
            }
            Ok(())
        }
        Expr::Comma { left, right, .. } => {
            check_uneval(left, env)?;
            check_uneval(right, env)
        }
        Expr::Unary { op, expr, .. } => {
            check_uneval(expr, env)?;
            if *op == UnaryOp::Deref && !rvalue_type(expr, env)?.is_pointer() {
                return Err(BoundErr::InvalidOperand);
            }
            Ok(())
        }
        Expr::Cast { expr, .. } | Expr::Decay { expr, .. } => check_uneval(expr, env),
        Expr::Assign { target, value, .. } | Expr::CompoundAssign { target, value, .. } => {
            check_uneval(target, env)?;
            check_uneval(value, env)?;
            if !uneval_lvalue(target, env) {
                return Err(BoundErr::InvalidOperand);
            }
            Ok(())
        }
        Expr::IncDec { target, .. } => {
            check_uneval(target, env)?;
            if uneval_lvalue(target, env) {
                Ok(())
            } else {
                Err(BoundErr::InvalidOperand)
            }
        }
        Expr::Cond {
            cond,
            then_expr,
            else_expr,
            ..
        } => {
            check_uneval(cond, env)?;
            check_uneval(then_expr, env)?;
            check_uneval(else_expr, env)
        }
        Expr::Call { callee, args, .. } => check_uneval_call(callee, args, env),
        Expr::Sizeof { operand, .. } => match operand {
            SizeofOperand::Expr(inner) => check_uneval(inner, env),
            SizeofOperand::Type(_) => Ok(()),
        },
        Expr::Index { base, index, .. } => {
            check_uneval(base, env)?;
            check_uneval(index, env)
        }
        Expr::Member { base, .. } => check_uneval(base, env),
        Expr::VaArg { ap, .. } => check_uneval(ap, env),
        Expr::VaOp { args, .. } => {
            for arg in args {
                check_uneval(arg, env)?;
            }
            Ok(())
        }
        Expr::Generic {
            control, assocs, ..
        } => check_generic_assocs(control, assocs, env),
        Expr::CompoundLit { decl, .. } => match &decl.init {
            Some(init) => check_uneval_init(init, env),
            None => Ok(()),
        },
        // Its names were resolved when the block was parsed.
        Expr::StmtExpr { .. } | Expr::LabelAddr { .. } => Ok(()),
        Expr::CondShort {
            cond, else_expr, ..
        } => {
            check_uneval(cond, env)?;
            check_uneval(else_expr, env)
        }
    }
}

/// A call in an unevaluated operand: a builtin's arity and operand rules,
/// or the arity of a call through a prototype.
fn check_uneval_call(callee: &Expr, args: &[Expr], env: Env) -> Result<(), BoundErr> {
    // A supported builtin is recognized only as a call, not as a bare
    // identifier, so it is accepted here before ordinary lookup.
    if let Expr::Var { name, .. } = callee {
        if check_uneval_builtin_call(name, args, env)? {
            return Ok(());
        }
    }
    check_uneval(callee, env)?;
    for arg in args {
        check_uneval(arg, env)?;
    }
    let callee_ty = rvalue_type(callee, env)?;
    let func = callee_ty
        .pointee()
        .and_then(CType::func_ty)
        .or_else(|| callee_ty.func_ty());
    if let Some(func) = func {
        if func.prototype {
            let argc = args.len();
            let arity = func.params.len();
            let bad = if func.variadic {
                argc < arity
            } else {
                argc != arity
            };
            if bad {
                return Err(BoundErr::WrongArity);
            }
        }
    }
    Ok(())
}

/// The checks for a call to the builtin or library function `name`, if it
/// is one; `Ok(false)` when `name` is an ordinary function.
fn check_uneval_builtin_call(name: &str, args: &[Expr], env: Env) -> Result<bool, BoundErr> {
    if crate::builtins::Inline::from_name(name).is_some()
        || (crate::builtins::library(name).is_some() && env_lookup(env, name).is_none())
    {
        for arg in args {
            check_uneval(arg, env)?;
        }
        return Ok(true);
    }
    let Some(arity) = builtin_arity(name) else {
        return Ok(false);
    };
    for arg in args {
        check_uneval(arg, env)?;
    }
    if args.len() != arity {
        return Err(BoundErr::WrongArity);
    }
    if crate::float::is_fp_compare_builtin(name) {
        check_uneval_fp_compare(&args[0], &args[1], env)?;
    }
    if is_signbit_builtin(name) {
        check_uneval_signbit(&args[0], env)?;
    }
    if is_memory_builtin(name) {
        check_uneval_memory_builtin(name, args, env)?;
    }
    if let Some(fp) = crate::float::FpFormatBuiltin::from_name(name) {
        if fp.requires_string_literal()
            && !crate::float::FpFormatBuiltin::is_string_literal_arg(&args[0])
        {
            return Err(BoundErr::NotStringLiteral);
        }
    }
    Ok(true)
}

/// The operand constraints of the relational and equality operators
/// (C11 6.5.8p2, 6.5.9p2), as semantic analysis applies them: two
/// arithmetic operands; for a relational operator two pointers to
/// compatible object types; for an equality operator two pointers to
/// compatible types or one `void *`, or a pointer and a null pointer
/// constant. An operand whose type the parser cannot work out is
/// left to semantic analysis.
pub(super) fn check_uneval_compare(
    op: BinOp,
    left: &Expr,
    right: &Expr,
    env: Env,
) -> Result<(), BoundErr> {
    let (Ok(left_ty), Ok(right_ty)) = (rvalue_type(left, env), rvalue_type(right, env)) else {
        return Ok(());
    };
    if left_ty.is_arithmetic() && right_ty.is_arithmetic() {
        return Ok(());
    }
    let ok = if matches!(op, BinOp::Eq | BinOp::Ne) {
        loose_pointers(&left_ty, &right_ty)
            || void_ptr_mix(&left_ty, &right_ty)
            || (env.dialect.gnu() && void_function_ptr_mix(&left_ty, &right_ty))
            || (env.dialect.gnu() && gnu_distinct_pointers(&left_ty, &right_ty))
            || (left_ty.is_pointer() && uneval_null_constant(right, &right_ty, env))
            || (right_ty.is_pointer() && uneval_null_constant(left, &left_ty, env))
    } else {
        let function = |ty: &CType| ty.pointee().is_some_and(CType::is_func);
        left_ty.is_pointer()
            && right_ty.is_pointer()
            && !function(&left_ty)
            && !function(&right_ty)
            && (loose_pointers(&left_ty, &right_ty)
                || (env.dialect.gnu() && gnu_distinct_pointers(&left_ty, &right_ty)))
    };
    if ok {
        Ok(())
    } else {
        Err(BoundErr::InvalidCompare)
    }
}

/// A null pointer constant (C11 6.3.2.3p3): an integer constant expression
/// with the value 0, or such an expression cast to `void *`. A generic
/// selection is classified by its selected expression; a `void *` cast of
/// a pointer, such as `(void *)(void *)0`, is a null pointer but not a null
/// pointer constant.
pub(super) fn uneval_null_constant(expr: &Expr, ty: &CType, env: Env) -> bool {
    match expr {
        Expr::Generic {
            control, assocs, ..
        } => select_generic(control, assocs, env).is_ok_and(|selected| {
            rvalue_type(selected, env)
                .is_ok_and(|selected_ty| uneval_null_constant(selected, &selected_ty, env))
        }),
        Expr::Cast {
            ty_name,
            expr: inner,
            ..
        } if ty_name
            .pointee()
            .is_some_and(|pointee| pointee.is_void() && pointee.quals().is_empty()) =>
        {
            rvalue_type(inner, env).is_ok_and(|inner_ty| uneval_integer_zero(inner, &inner_ty, env))
        }
        _ => uneval_integer_zero(expr, ty, env),
    }
}

/// An integer constant expression with the value 0.
pub(super) fn uneval_integer_zero(expr: &Expr, ty: &CType, env: Env) -> bool {
    ty.is_integer() && eval_ice(expr, env).is_ok_and(|value| value.bits & value.ty.mask() == 0)
}

/// The operands of `__builtin_isgreater` and its companions: both
/// arithmetic and at least one real floating, the rule semantic analysis
/// applies. An operand whose type the parser cannot work out is left to
/// semantic analysis.
pub(super) fn check_uneval_fp_compare(left: &Expr, right: &Expr, env: Env) -> Result<(), BoundErr> {
    let (Ok(left_ty), Ok(right_ty)) = (rvalue_type(left, env), rvalue_type(right, env)) else {
        return Ok(());
    };
    if left_ty.is_arithmetic()
        && right_ty.is_arithmetic()
        && (left_ty.is_float() || right_ty.is_float())
    {
        Ok(())
    } else {
        Err(BoundErr::FpCompareOperands)
    }
}

pub(super) fn check_uneval_init(init: &Init, env: Env) -> Result<(), BoundErr> {
    match init {
        Init::Expr(expr) => check_uneval(expr, env),
        Init::List { items, .. } => {
            for item in items {
                check_uneval_init(item, env)?;
            }
            Ok(())
        }
        Init::Designated {
            designators, init, ..
        } => {
            for designator in designators {
                if let Designator::Index { expr, .. } = designator {
                    check_uneval(expr, env)?;
                }
            }
            check_uneval_init(init, env)
        }
    }
}

pub(super) fn builtin_arity(name: &str) -> Option<usize> {
    if let Some(fp) = crate::float::FpFormatBuiltin::from_name(name) {
        return Some(fp.arity());
    }
    match name {
        name if is_signbit_builtin(name) => Some(1),
        name if is_memory_builtin(name) => Some(3),
        name if crate::float::is_fp_compare_builtin(name) => Some(2),
        _ => None,
    }
}

pub(super) fn is_signbit_builtin(name: &str) -> bool {
    matches!(
        name,
        "__builtin_signbit" | "__builtin_signbitf" | "__builtin_signbitl"
    )
}

pub(super) fn is_memory_builtin(name: &str) -> bool {
    matches!(
        name,
        "__builtin_memcpy" | "__builtin_memmove" | "__builtin_memset"
    )
}

/// The result type semantic analysis gives a supported builtin call:
/// the floating type of `__builtin_nan*`, `__builtin_inf*`, and
/// `__builtin_huge_val*`; `int` for `__builtin_signbit*` and the
/// comparison builtins; `void *` for the memory builtins.
pub(super) fn builtin_result_type(name: &str) -> Option<CType> {
    if let Some(fp) = crate::float::FpFormatBuiltin::from_name(name) {
        return Some(CType::Float(fp.kind()));
    }
    if is_signbit_builtin(name) || crate::float::is_fp_compare_builtin(name) {
        return Some(CType::int_type());
    }
    if is_memory_builtin(name) {
        return Some(CType::pointer(CType::Void));
    }
    if let Some(inline) = crate::builtins::Inline::from_name(name) {
        return Some(inline.result_type());
    }
    crate::builtins::library(name).map(|sig| sig.ret)
}

/// The operand of `__builtin_signbit*` is arithmetic, the rule semantic
/// analysis applies. An operand whose type the parser cannot work out is
/// left to semantic analysis.
pub(super) fn check_uneval_signbit(arg: &Expr, env: Env) -> Result<(), BoundErr> {
    match rvalue_type(arg, env) {
        Ok(ty) if !ty.is_arithmetic() => Err(BoundErr::SignbitOperand),
        _ => Ok(()),
    }
}

/// The operands of `__builtin_memcpy`, `__builtin_memmove`, and
/// `__builtin_memset`, as semantic analysis checks them: a pointer
/// destination; a pointer source, or an integer value for memset; an
/// integer size. An operand whose type the parser cannot work out is left
/// to semantic analysis.
pub(super) fn check_uneval_memory_builtin(
    name: &str,
    args: &[Expr],
    env: Env,
) -> Result<(), BoundErr> {
    let known = |arg: &Expr| rvalue_type(arg, env).ok();
    if known(&args[0]).is_some_and(|ty| !ty.is_pointer()) {
        return Err(BoundErr::MemoryDest);
    }
    if name == "__builtin_memset" {
        if known(&args[1]).is_some_and(|ty| !ty.is_integer()) {
            return Err(BoundErr::MemsetValue);
        }
    } else if known(&args[1]).is_some_and(|ty| !ty.is_pointer()) {
        return Err(BoundErr::MemorySource);
    }
    if known(&args[2]).is_some_and(|ty| !ty.is_integer()) {
        return Err(BoundErr::MemorySize);
    }
    Ok(())
}

pub(super) fn uneval_lvalue(expr: &Expr, env: Env) -> bool {
    match expr {
        Expr::Var { .. }
        | Expr::Index { .. }
        | Expr::Member { .. }
        | Expr::CompoundLit { .. }
        | Expr::Str { .. }
        | Expr::Unary {
            op: UnaryOp::Deref, ..
        } => true,
        Expr::Generic {
            control, assocs, ..
        } => select_generic(control, assocs, env)
            .ok()
            .is_some_and(|selected| uneval_lvalue(selected, env)),
        _ => false,
    }
}

/// The association a generic selection picks (C11 6.5.1.1p3): the one whose
/// type is compatible with the controlling expression's type after lvalue
/// conversion, else `default`. Association expressions are checked first.
pub(super) fn select_generic<'e>(
    control: &Expr,
    assocs: &'e [GenericAssoc],
    env: Env,
) -> Result<&'e Expr, BoundErr> {
    let ty = rvalue_type(control, env)?;
    assocs
        .iter()
        .find(|assoc| assoc.ty.as_ref().is_some_and(|t| compatible(t, &ty)))
        .or_else(|| assocs.iter().find(|assoc| assoc.ty.is_none()))
        .map(|assoc| &assoc.expr)
        .ok_or(BoundErr::NotConst)
}

/// What constant folding needs from the parser: the ordinary-identifier
/// scopes, innermost last, and the dialect.
#[derive(Clone, Copy)]
pub(super) struct Env<'a> {
    pub(super) names: &'a [HashMap<String, NameBinding>],
    pub(super) dialect: Dialect,
}

pub(super) fn env_lookup<'a>(env: Env<'a>, name: &str) -> Option<&'a NameBinding> {
    env.names.iter().rev().find_map(|scope| scope.get(name))
}

/// Type of an expression, computed from declarations the parser has seen.
///
/// This is only used where C requires an integer constant expression before
/// semantic analysis runs: an array bound, an enumeration value, a bit-field
/// width, or `_Static_assert`. There the only operands whose type matters
/// without their value are those of `sizeof` (C11 6.6p6), and the operand of
/// `sizeof` is not evaluated (C11 6.5.3.4p2), so it may name any object. The
/// result is the type before lvalue conversion: an array stays an array and
/// a string literal is `char[N]`. Anything this cannot type is reported as
/// not constant rather than guessed.
pub(super) fn type_only(expr: &Expr, env: Env) -> Result<CType, BoundErr> {
    match expr {
        Expr::Int { value_ty, .. } => Ok(CType::from_name(*value_ty)),
        Expr::Float { kind, .. } => Ok(CType::Float(*kind)),
        Expr::Str { bytes, enc, .. } => {
            let len = bytes.len() as u64 / enc.unit() as u64 + 1;
            Ok(CType::array(CType::from_name(enc.elem()), Some(len)))
        }
        Expr::Sizeof { .. } => Ok(CType::from_name(TypeName::ULong)),
        Expr::Var { name, .. } => match env_lookup(env, name) {
            Some(NameBinding::Ordinary(Some(ty))) => Ok(ty.clone()),
            // An object's scope starts after its declarator (C11 6.2.1p7),
            // so `int x[sizeof x];` names nothing. `__func__` is implicit.
            None if name != "__func__" => Err(BoundErr::Undeclared),
            _ => Err(BoundErr::Vla),
        },
        Expr::Unary { op, expr, .. } => unary_type_only(*op, expr, env),
        Expr::Bin {
            op, left, right, ..
        } => bin_type_only(*op, left, right, env),
        Expr::Cast { ty_name, .. } => Ok(ty_name.unqualified()),
        Expr::StmtExpr { value_ty, .. } => value_ty.clone().ok_or(BoundErr::NotConst),
        Expr::LabelAddr { .. } => Ok(CType::pointer(CType::Void)),
        Expr::Cond {
            then_expr: left,
            else_expr: right,
            ..
        }
        | Expr::CondShort {
            cond: left,
            else_expr: right,
            ..
        } => cond_type_only(left, right, env),
        Expr::Assign { target, .. }
        | Expr::CompoundAssign { target, .. }
        | Expr::IncDec { target, .. } => Ok(type_only(target, env)?.unqualified()),
        Expr::Comma { right, .. } => rvalue_type(right, env),
        Expr::Call { callee, .. } => call_type_only(callee, env),
        Expr::Index { base, index, .. } => {
            let base = rvalue_type(base, env)?;
            let index = rvalue_type(index, env)?;
            base.pointee()
                .or_else(|| index.pointee())
                .cloned()
                .ok_or(BoundErr::NotConst)
        }
        Expr::Member {
            base, field, arrow, ..
        } => member_type_only(base, field, *arrow, env),
        Expr::VaArg { type_arg, .. } => Ok(type_arg.clone()),
        Expr::VaOp { .. } => Ok(CType::Void),
        Expr::Decay { ty, .. } => ty.clone().ok_or(BoundErr::NotConst),
        Expr::Generic {
            control, assocs, ..
        } => {
            check_generic_assocs(control, assocs, env)?;
            type_only(select_generic(control, assocs, env)?, env)
        }
        Expr::CompoundLit { decl, .. } => match (&decl.ty, &decl.init) {
            (CType::Array(elem, None), Some(init)) => infer_init_len(elem, init, env)
                .map(|len| CType::array((**elem).clone(), Some(len)))
                .ok_or(BoundErr::NotConst),
            (ty, _) => Ok(ty.clone()),
        },
    }
}

/// [`type_only`] for a unary operator other than `++` / `--`.
fn unary_type_only(op: UnaryOp, expr: &Expr, env: Env) -> Result<CType, BoundErr> {
    match op {
        UnaryOp::Deref => {
            let inner = rvalue_type(expr, env)?;
            inner.pointee().cloned().ok_or(BoundErr::NotConst)
        }
        UnaryOp::Addr => Ok(CType::pointer(type_only(expr, env)?)),
        UnaryOp::Not => Ok(CType::int_type()),
        UnaryOp::Plus | UnaryOp::Minus | UnaryOp::BitNot => {
            let inner = rvalue_type(expr, env)?;
            match inner.as_integer() {
                Some(name) => Ok(CType::from_name(name.promote())),
                None if inner.is_float() && op != UnaryOp::BitNot => Ok(inner),
                None => Err(BoundErr::NotConst),
            }
        }
    }
}

/// [`type_only`] for a binary operator.
fn bin_type_only(op: BinOp, left: &Expr, right: &Expr, env: Env) -> Result<CType, BoundErr> {
    let left = rvalue_type(left, env)?;
    let right = rvalue_type(right, env)?;
    if op.is_cmp() || matches!(op, BinOp::And | BinOp::Or) {
        return Ok(CType::int_type());
    }
    match op {
        BinOp::Add | BinOp::Sub if left.is_pointer() && right.is_pointer() => {
            Ok(CType::from_name(TypeName::Long))
        }
        BinOp::Add | BinOp::Sub if left.is_pointer() => Ok(left),
        BinOp::Add if right.is_pointer() => Ok(right),
        BinOp::Shl | BinOp::Shr => left
            .as_integer()
            .map(|name| CType::from_name(name.promote()))
            .ok_or(BoundErr::NotConst),
        _ => arith_common(&left, &right),
    }
}

/// [`type_only`] for a conditional expression with second and third
/// operands `left` and `right`.
fn cond_type_only(left: &Expr, right: &Expr, env: Env) -> Result<CType, BoundErr> {
    let left = rvalue_type(left, env)?;
    let right = rvalue_type(right, env)?;
    if left.is_arithmetic() && right.is_arithmetic() {
        arith_common(&left, &right)
    } else if left.is_void() || right.is_void() {
        Ok(CType::Void)
    } else if left.is_pointer() || right.is_pointer() {
        // Every object pointer has the same size; `sizeof` is all
        // that asks for this type here.
        Ok(if left.is_pointer() { left } else { right })
    } else {
        Ok(left)
    }
}

/// [`type_only`] for a call: a builtin's result type, or the return type
/// of the function the callee points to.
fn call_type_only(callee: &Expr, env: Env) -> Result<CType, BoundErr> {
    if let Expr::Var { name, .. } = callee {
        if let Some(ty) = builtin_result_type(name) {
            return Ok(ty);
        }
    }
    let callee = rvalue_type(callee, env)?;
    let func = callee
        .pointee()
        .and_then(CType::func_ty)
        .ok_or(BoundErr::NotConst)?;
    Ok(func.ret.unqualified())
}

/// [`type_only`] for `base.field` or `base->field`.
fn member_type_only(base: &Expr, field: &str, arrow: bool, env: Env) -> Result<CType, BoundErr> {
    let base_ty = if arrow {
        rvalue_type(base, env)?
            .pointee()
            .cloned()
            .ok_or(BoundErr::NotConst)?
    } else {
        type_only(base, env)?
    };
    let handle = base_ty.struct_ref().ok_or(BoundErr::NotConst)?;
    let hit = handle.lookup_member(field).ok_or(BoundErr::NotConst)?;
    Ok(hit.field.ty.with_quals(base_ty.quals()))
}

/// Whether `expr`, ignoring parentheses, designates a bit-field member.
pub(super) fn is_bitfield_member(expr: &Expr, env: Env) -> bool {
    let Expr::Member {
        base, field, arrow, ..
    } = expr
    else {
        return false;
    };
    let base_ty = if *arrow {
        rvalue_type(base, env)
            .ok()
            .and_then(|ty| ty.pointee().cloned())
    } else {
        type_only(base, env).ok()
    };
    base_ty
        .as_ref()
        .and_then(CType::struct_ref)
        .and_then(|handle| handle.lookup_member(field))
        .is_some_and(|hit| hit.field.is_bitfield())
}

/// Type after lvalue conversion: arrays and functions become pointers and
/// qualifiers are dropped (C11 6.3.2.1).
pub(super) fn rvalue_type(expr: &Expr, env: Env) -> Result<CType, BoundErr> {
    let ty = type_only(expr, env)?;
    Ok(match ty.peel() {
        CType::Array(elem, _) => CType::pointer((**elem).clone()),
        CType::Func(_) => CType::pointer(ty.clone()),
        _ => ty.unqualified(),
    })
}

/// The usual arithmetic conversions (C11 6.3.1.8) on two types.
pub(super) fn arith_common(left: &CType, right: &CType) -> Result<CType, BoundErr> {
    match (left.peel(), right.peel()) {
        (CType::Int(a), CType::Int(b)) => Ok(CType::from_name(a.usual(*b))),
        (CType::Float(a), CType::Float(b)) => {
            Ok(CType::Float(if float_rank(*a) >= float_rank(*b) {
                *a
            } else {
                *b
            }))
        }
        (CType::Float(a), CType::Int(_)) | (CType::Int(_), CType::Float(a)) => Ok(CType::Float(*a)),
        _ => Err(BoundErr::NotConst),
    }
}

pub(super) fn float_rank(kind: FloatKind) -> u8 {
    match kind {
        FloatKind::Float => 0,
        FloatKind::Double => 1,
        FloatKind::LongDouble => 2,
    }
}

pub(super) enum AssignKind {
    Plain,
    Compound(BinOp),
}

pub(super) fn assign_kind(kind: TokenKind) -> Option<AssignKind> {
    Some(match kind {
        TokenKind::Assign => AssignKind::Plain,
        TokenKind::PlusEq => AssignKind::Compound(BinOp::Add),
        TokenKind::MinusEq => AssignKind::Compound(BinOp::Sub),
        TokenKind::StarEq => AssignKind::Compound(BinOp::Mul),
        TokenKind::SlashEq => AssignKind::Compound(BinOp::Div),
        TokenKind::PercentEq => AssignKind::Compound(BinOp::Mod),
        TokenKind::AmpEq => AssignKind::Compound(BinOp::BitAnd),
        TokenKind::PipeEq => AssignKind::Compound(BinOp::BitOr),
        TokenKind::CaretEq => AssignKind::Compound(BinOp::BitXor),
        TokenKind::ShlEq => AssignKind::Compound(BinOp::Shl),
        TokenKind::ShrEq => AssignKind::Compound(BinOp::Shr),
        _ => return None,
    })
}
