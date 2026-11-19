//! Integer constant expressions (C11 6.6) evaluated while parsing: array
//! bounds, enumerator values and inferred array lengths.

use super::*;

pub(super) enum BoundErr {
    Vla,
    Incomplete,
    NonPositive,
    TooBig,
    DivZero,
    Overflow,
    Shift,
    NotConst,
    Undeclared,
    WrongArity,
    NotStringLiteral,
    InvalidOperand,
    InvalidCompare,
    FpCompareOperands,
    SignbitOperand,
    MemoryDest,
    MemorySource,
    MemsetValue,
    MemorySize,
    GenericDupDefault,
    GenericBadType,
    GenericDupType,
    GenericNoMatch,
    SizeofFunction,
    SizeofBitField,
}

pub(super) fn bound_message(err: BoundErr) -> &'static str {
    match err {
        BoundErr::Vla => "variable-length arrays are not supported",
        BoundErr::Incomplete => "invalid application of 'sizeof' to an incomplete type",
        BoundErr::NonPositive => "array size must be positive",
        BoundErr::TooBig => "array is too large",
        BoundErr::DivZero => "array size divides by zero",
        BoundErr::Overflow => "array size overflows",
        BoundErr::Shift => "array size uses an undefined shift",
        BoundErr::NotConst => "array size is not an integer constant",
        BoundErr::Undeclared => "use of undeclared identifier",
        BoundErr::WrongArity => "wrong number of arguments in function call",
        BoundErr::NotStringLiteral => "requires a string literal argument",
        BoundErr::InvalidOperand => "invalid operand",
        BoundErr::InvalidCompare => "invalid operands to a comparison",
        BoundErr::FpCompareOperands => {
            "non-floating-point arguments in call to a floating-point comparison builtin"
        }
        BoundErr::SignbitOperand => "__builtin_signbit requires an arithmetic argument",
        BoundErr::MemoryDest => "memory builtin destination must be a pointer",
        BoundErr::MemorySource => "memory builtin source must be a pointer",
        BoundErr::MemsetValue => "__builtin_memset value must be an integer",
        BoundErr::MemorySize => "memory builtin size must be an integer",
        BoundErr::GenericDupDefault => "duplicate default generic association",
        BoundErr::GenericBadType => "type in generic association is not a complete object type",
        BoundErr::GenericDupType => {
            "type in generic association compatible with a previously specified type"
        }
        BoundErr::GenericNoMatch => {
            "controlling expression type not compatible with any generic association type"
        }
        BoundErr::SizeofFunction => "invalid application of 'sizeof' to a function type",
        BoundErr::SizeofBitField => "invalid application of 'sizeof' to a bit-field",
    }
}

/// The diagnostic for an expression that must be an integer constant
/// expression but is not a bound, a width, or an enumeration value.
pub(super) fn ice_message(err: BoundErr) -> &'static str {
    match err {
        BoundErr::Vla | BoundErr::NotConst => "expression is not an integer constant expression",
        BoundErr::DivZero => "division by zero in a constant expression",
        BoundErr::Overflow => "constant expression overflows",
        BoundErr::Shift => "constant expression uses an undefined shift",
        other => bound_message(other),
    }
}

pub(super) struct BVal {
    pub(super) bits: u128,
    pub(super) ty: TypeName,
}

/// Integer constant expression used as an array bound. An identifier, a call,
/// or an assignment is reported as a variable-length array.
pub(super) fn fold_array_bound(expr: &Expr, env: Env) -> Result<u64, BoundErr> {
    check_uneval(expr, env)?;
    let value = eval_ice(expr, env)?;
    let count = if value.ty.is_signed() {
        let signed = sign_extend(value.bits, value.ty.width());
        if signed <= 0 {
            return Err(BoundErr::NonPositive);
        }
        u64::try_from(signed).map_err(|_| BoundErr::TooBig)?
    } else {
        let unsigned = value.bits & value.ty.mask();
        if unsigned == 0 {
            return Err(BoundErr::NonPositive);
        }
        u64::try_from(unsigned).map_err(|_| BoundErr::TooBig)?
    };
    if count > u64::MAX / 8 {
        return Err(BoundErr::TooBig);
    }
    Ok(count)
}

pub(super) fn eval_ice(expr: &Expr, env: Env) -> Result<BVal, BoundErr> {
    match expr {
        Expr::Int { bits, value_ty, .. } => Ok(BVal {
            bits: u128::from(*bits) & value_ty.mask(),
            ty: *value_ty,
        }),
        Expr::Float { .. } => Err(BoundErr::NotConst),
        Expr::Sizeof { operand, .. } => {
            // C11 6.5.3.4p1: not a function type, an incomplete type, or
            // a bit-field member.
            let ty = match operand {
                SizeofOperand::Type(ty) => ty.clone(),
                SizeofOperand::Expr(inner) => {
                    if is_bitfield_member(inner, env) {
                        return Err(BoundErr::SizeofBitField);
                    }
                    type_only(inner, env)?
                }
            };
            if ty.is_func() {
                return Err(BoundErr::SizeofFunction);
            }
            let size = ty.size().ok_or(BoundErr::Incomplete)?;
            Ok(BVal {
                bits: u128::from(size),
                ty: TypeName::size_t(),
            })
        }
        Expr::Unary { op, expr, .. } => eval_ice_unary(*op, expr, env),
        Expr::Bin {
            op, left, right, ..
        } => eval_ice_bin(*op, left, right, env),
        Expr::Cond {
            cond,
            then_expr,
            else_expr,
            ..
        } => {
            let cond = eval_ice(cond, env)?;
            let chosen = if cond.bits & cond.ty.mask() == 0 {
                else_expr
            } else {
                then_expr
            };
            let value = eval_ice(chosen, env)?;
            let result = match (type_only(then_expr, env)?, type_only(else_expr, env)?) {
                (left, right) if left.is_integer() && right.is_integer() => left
                    .as_integer()
                    .unwrap()
                    .usual(right.as_integer().unwrap()),
                _ => return Err(BoundErr::Vla),
            };
            Ok(BVal {
                bits: convert_bits(value.bits, value.ty, result),
                ty: result,
            })
        }
        Expr::Cast { ty_name, expr, .. } => {
            let dest = ty_name.as_integer().ok_or(BoundErr::Vla)?;
            // C11 6.6 allows a floating constant only as the immediate
            // operand of a cast to an integer type. `(int)3.5` is an
            // integer constant; `1.5` and `(int)(1.5 + 2.5)` are not.
            if let Expr::Float { bits, kind, .. } = expr.as_ref() {
                let bits =
                    crate::float::real_to_int(*bits, *kind, dest).ok_or(BoundErr::Overflow)?;
                return Ok(BVal { bits, ty: dest });
            }
            let value = eval_ice(expr, env)?;
            Ok(BVal {
                bits: convert_bits(value.bits, value.ty, dest),
                ty: dest,
            })
        }
        Expr::Str { .. } => Err(BoundErr::NotConst),
        Expr::Var { .. }
        | Expr::Assign { .. }
        | Expr::Call { .. }
        | Expr::CompoundAssign { .. }
        | Expr::IncDec { .. }
        | Expr::Comma { .. }
        | Expr::Index { .. }
        | Expr::Decay { .. }
        | Expr::Member { .. }
        | Expr::VaArg { .. }
        | Expr::VaOp { .. } => Err(BoundErr::Vla),
        Expr::CompoundLit { .. } | Expr::StmtExpr { .. } | Expr::LabelAddr { .. } => {
            Err(BoundErr::NotConst)
        }
        Expr::Generic {
            control, assocs, ..
        } => {
            check_generic_assocs(control, assocs, env)?;
            eval_ice(select_generic(control, assocs, env)?, env)
        }
        Expr::CondShort {
            cond, else_expr, ..
        } => {
            let first = eval_ice(cond, env)?;
            let value = if first.bits & first.ty.mask() == 0 {
                eval_ice(else_expr, env)?
            } else {
                first
            };
            let result = match (type_only(cond, env)?, type_only(else_expr, env)?) {
                (left, right) if left.is_integer() && right.is_integer() => left
                    .as_integer()
                    .unwrap()
                    .usual(right.as_integer().unwrap()),
                _ => return Err(BoundErr::Vla),
            };
            Ok(BVal {
                bits: convert_bits(value.bits, value.ty, result),
                ty: result,
            })
        }
    }
}

/// Length that an initializer gives an array of unknown size, for the
/// shapes whose answer follows from the braces alone (C11 6.7.9p22): a
/// string for a character array, a list of scalars, a list in which every
/// element of an aggregate type has its own braces, and `[N]` designators.
/// `None` means the parser does not know; semantic analysis still does.
pub(super) fn infer_init_len(elem: &CType, init: &Init, env: Env) -> Option<u64> {
    if let Init::Expr(Expr::Str { bytes, enc, .. }) = init {
        if elem.is_integer() && elem.size() == Some(enc.unit() as u64) {
            return Some(bytes.len() as u64 / enc.unit() as u64 + 1);
        }
        return None;
    }
    let Init::List { items, .. } = init else {
        return None;
    };
    if let [Init::Expr(Expr::Str { bytes, enc, .. })] = items.as_slice() {
        if elem.is_integer() && elem.size() == Some(enc.unit() as u64) {
            return Some(bytes.len() as u64 / enc.unit() as u64 + 1);
        }
    }
    let scalar = elem.is_scalar();
    let char_array = elem
        .array_elem()
        .is_some_and(|inner| inner.is_integer() && elem.array_len().is_some());
    let mut index = 0u64;
    let mut count = 0u64;
    for item in items {
        let value = match item {
            Init::Designated {
                designators, init, ..
            } => {
                let Some(Designator::Index { expr, .. }) = designators.first() else {
                    return None;
                };
                let at = eval_ice(expr, env).ok()?;
                if at.ty.is_signed() && sign_extend(at.bits, at.ty.width()) < 0 {
                    return None;
                }
                index = u64::try_from(at.bits & at.ty.mask()).ok()?;
                if designators.len() == 1 && !scalar && !matches!(**init, Init::List { .. }) {
                    // `[2] = x` for an aggregate element may start brace
                    // elision; the next positional items could belong to it.
                    return None;
                }
                init.as_ref()
            }
            other => other,
        };
        let one = scalar
            || matches!(value, Init::List { .. })
            || (char_array && matches!(value, Init::Expr(Expr::Str { .. })));
        if !one {
            return None;
        }
        index = index.checked_add(1)?;
        count = count.max(index);
    }
    (count > 0).then_some(count)
}

pub(super) fn eval_ice_unary(op: UnaryOp, expr: &Expr, env: Env) -> Result<BVal, BoundErr> {
    let value = promote_ice(eval_ice(expr, env)?);
    match op {
        UnaryOp::Plus => Ok(value),
        UnaryOp::Minus => {
            if value.ty.is_signed() {
                let signed = sign_extend(value.bits, value.ty.width());
                bits_ice(signed.checked_neg().ok_or(BoundErr::Overflow)?, value.ty)
            } else {
                Ok(BVal {
                    bits: value.bits.wrapping_neg() & value.ty.mask(),
                    ty: value.ty,
                })
            }
        }
        // C11 6.5.3.3p5: the result is 1 if the operand compares equal
        // to 0, and 0 otherwise. The result has type `int`.
        UnaryOp::Not => Ok(BVal {
            bits: u128::from(value.bits & value.ty.mask() == 0),
            ty: TypeName::Int,
        }),
        UnaryOp::BitNot => Ok(BVal {
            bits: (!value.bits) & value.ty.mask(),
            ty: value.ty,
        }),
        UnaryOp::Addr | UnaryOp::Deref => Err(BoundErr::Vla),
    }
}

pub(super) fn promote_ice(value: BVal) -> BVal {
    let ty = value.ty.promote();
    BVal {
        bits: convert_bits(value.bits, value.ty, ty),
        ty,
    }
}

pub(super) fn bits_ice(value: i128, ty: TypeName) -> Result<BVal, BoundErr> {
    let (min, max) = crate::ast::signed_limits(ty.width());
    if value < min || value > max {
        return Err(BoundErr::Overflow);
    }
    Ok(BVal {
        bits: (value as u128) & ty.mask(),
        ty,
    })
}

pub(super) fn eval_ice_bin(
    op: BinOp,
    left: &Expr,
    right: &Expr,
    env: Env,
) -> Result<BVal, BoundErr> {
    match op {
        BinOp::And => {
            let left = eval_ice(left, env)?;
            if left.bits & left.ty.mask() == 0 {
                return Ok(BVal {
                    bits: 0,
                    ty: TypeName::Int,
                });
            }
            let right = eval_ice(right, env)?;
            Ok(BVal {
                bits: u128::from(right.bits & right.ty.mask() != 0),
                ty: TypeName::Int,
            })
        }
        BinOp::Or => {
            let left = eval_ice(left, env)?;
            if left.bits & left.ty.mask() != 0 {
                return Ok(BVal {
                    bits: 1,
                    ty: TypeName::Int,
                });
            }
            let right = eval_ice(right, env)?;
            Ok(BVal {
                bits: u128::from(right.bits & right.ty.mask() != 0),
                ty: TypeName::Int,
            })
        }
        BinOp::Shl | BinOp::Shr => {
            let left = promote_ice(eval_ice(left, env)?);
            let right = promote_ice(eval_ice(right, env)?);
            let count = ice_shift_count(&right, left.ty.width())?;
            if op == BinOp::Shr {
                let bits = if left.ty.is_signed() {
                    (sign_extend(left.bits, left.ty.width()) >> count) as u128 & left.ty.mask()
                } else {
                    (left.bits & left.ty.mask()) >> count
                };
                return Ok(BVal { bits, ty: left.ty });
            }
            if left.ty.is_signed() {
                let signed = sign_extend(left.bits, left.ty.width());
                if signed < 0 {
                    return Err(BoundErr::Shift);
                }
                if !crate::ast::signed_shl_fits(signed, count, left.ty.width()) {
                    return Err(BoundErr::Overflow);
                }
                bits_ice(signed << count, left.ty)
            } else {
                Ok(BVal {
                    bits: ((left.bits & left.ty.mask()) << count) & left.ty.mask(),
                    ty: left.ty,
                })
            }
        }
        _ => {
            let left = eval_ice(left, env)?;
            let right = eval_ice(right, env)?;
            let ty = left.ty.usual(right.ty);
            let left = convert_bits(left.bits, left.ty, ty);
            let right = convert_bits(right.bits, right.ty, ty);
            if op.is_cmp() {
                let bit = ice_compare(op, left, right, ty);
                return Ok(BVal {
                    bits: u128::from(bit),
                    ty: TypeName::Int,
                });
            }
            ice_arith(op, left, right, ty)
        }
    }
}

pub(super) fn ice_shift_count(count: &BVal, width: u32) -> Result<u32, BoundErr> {
    if count.ty.is_signed() {
        let signed = sign_extend(count.bits, count.ty.width());
        if signed < 0 || signed >= i128::from(width) {
            return Err(BoundErr::Shift);
        }
        Ok(signed as u32)
    } else {
        let unsigned = count.bits & count.ty.mask();
        if unsigned >= u128::from(width) {
            return Err(BoundErr::Shift);
        }
        Ok(unsigned as u32)
    }
}

pub(super) fn ice_compare(op: BinOp, left: u128, right: u128, ty: TypeName) -> bool {
    if ty.is_signed() {
        let left = sign_extend(left, ty.width());
        let right = sign_extend(right, ty.width());
        match op {
            BinOp::Eq => left == right,
            BinOp::Ne => left != right,
            BinOp::Lt => left < right,
            BinOp::Gt => left > right,
            BinOp::Le => left <= right,
            BinOp::Ge => left >= right,
            _ => false,
        }
    } else {
        let left = left & ty.mask();
        let right = right & ty.mask();
        match op {
            BinOp::Eq => left == right,
            BinOp::Ne => left != right,
            BinOp::Lt => left < right,
            BinOp::Gt => left > right,
            BinOp::Le => left <= right,
            BinOp::Ge => left >= right,
            _ => false,
        }
    }
}

pub(super) fn ice_arith(
    op: BinOp,
    left: u128,
    right: u128,
    ty: TypeName,
) -> Result<BVal, BoundErr> {
    if ty.is_signed() {
        let left = sign_extend(left, ty.width());
        let right = sign_extend(right, ty.width());
        if matches!(op, BinOp::Div | BinOp::Mod) && right == 0 {
            return Err(BoundErr::DivZero);
        }
        if matches!(op, BinOp::Div | BinOp::Mod)
            && left == crate::ast::signed_limits(ty.width()).0
            && right == -1
        {
            return Err(BoundErr::Overflow);
        }
        let value = match op {
            BinOp::Add => left.checked_add(right),
            BinOp::Sub => left.checked_sub(right),
            BinOp::Mul => left.checked_mul(right),
            BinOp::Div => Some(left / right),
            BinOp::Mod => Some(left % right),
            BinOp::BitAnd => Some(left & right),
            BinOp::BitOr => Some(left | right),
            BinOp::BitXor => Some(left ^ right),
            _ => return Err(BoundErr::Vla),
        };
        bits_ice(value.ok_or(BoundErr::Overflow)?, ty)
    } else {
        let mask = ty.mask();
        let left = left & mask;
        let right = right & mask;
        if matches!(op, BinOp::Div | BinOp::Mod) && right == 0 {
            return Err(BoundErr::DivZero);
        }
        let bits = match op {
            BinOp::Add => left.wrapping_add(right),
            BinOp::Sub => left.wrapping_sub(right),
            BinOp::Mul => left.wrapping_mul(right),
            BinOp::Div => left / right,
            BinOp::Mod => left % right,
            BinOp::BitAnd => left & right,
            BinOp::BitOr => left | right,
            BinOp::BitXor => left ^ right,
            _ => return Err(BoundErr::Vla),
        };
        Ok(BVal {
            bits: bits & mask,
            ty,
        })
    }
}
