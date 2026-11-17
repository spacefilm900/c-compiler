//! Constant expression evaluation: integer and floating arithmetic,
//! comparisons and shifts with C11 overflow rules (C11 6.6).

use super::*;

pub(super) enum ConstError {
    NotConst,
    DivZero,
    Overflow(TypeName),
    Shift,
}

pub(super) struct CVal {
    pub(super) bits: u128,
    pub(super) ty: TypeName,
}

pub(super) fn c_int(value: i32) -> CVal {
    CVal {
        bits: value as u32 as u128,
        ty: TypeName::Int,
    }
}

pub(super) fn is_zero(value: &CVal) -> bool {
    value.bits & value.ty.mask() == 0
}

/// A constant value that may be an integer or a `float`/`double`.
pub(super) enum NumConst {
    Int(CVal),
    Real {
        bits: u128,
        kind: crate::ast::FloatKind,
    },
}

impl NumConst {
    pub(super) fn is_zero(&self) -> bool {
        match self {
            NumConst::Int(value) => is_zero(value),
            NumConst::Real { bits, kind } => crate::float::real_is_zero(*kind, *bits),
        }
    }

    pub(super) fn real_kind(&self) -> Option<crate::ast::FloatKind> {
        match self {
            NumConst::Real { kind, .. } => Some(*kind),
            NumConst::Int(_) => None,
        }
    }

    pub(super) fn to_real(&self, dest: crate::ast::FloatKind) -> Option<u128> {
        match self {
            NumConst::Int(value) => Some(crate::float::int_to_real(value.bits, value.ty, dest)),
            NumConst::Real { bits, kind } => Some(crate::float::real_to_real(*bits, *kind, dest)),
        }
    }

    pub(super) fn to_int(&self, dest: TypeName) -> Result<CVal, ConstError> {
        match self {
            NumConst::Int(value) => Ok(CVal {
                bits: convert_bits(value.bits, value.ty, dest),
                ty: dest,
            }),
            NumConst::Real { bits, kind } => {
                let bits = crate::float::real_to_int(*bits, *kind, dest)
                    .ok_or(ConstError::Overflow(dest))?;
                Ok(CVal { bits, ty: dest })
            }
        }
    }
}

pub(super) fn has_real(expr: &Expr) -> bool {
    match expr {
        Expr::Float { .. } => true,
        Expr::Int { .. } | Expr::Sizeof { .. } => false,
        Expr::Unary { op, expr, .. } => {
            !matches!(op, UnaryOp::Addr | UnaryOp::Deref) && has_real(expr)
        }
        Expr::Bin { left, right, .. } => has_real(left) || has_real(right),
        Expr::Cond {
            cond,
            then_expr,
            else_expr,
            ty,
            ..
        } => {
            ty.as_ref().is_some_and(|ty| ty.is_float())
                || has_real(cond)
                || has_real(then_expr)
                || has_real(else_expr)
        }
        Expr::Cast { ty_name, expr, .. } => ty_name.is_float() || has_real(expr),
        Expr::Call { callee, .. } => matches!(
            callee.as_ref(),
            Expr::Var { name, .. } if crate::float::FpFormatBuiltin::from_name(name).is_some()
        ),
        _ => false,
    }
}

pub(super) fn eval_num(expr: &Expr) -> Result<NumConst, ConstError> {
    if !has_real(expr) {
        return Ok(NumConst::Int(eval_const_expr(expr)?));
    }
    match expr {
        Expr::Float { bits, kind, .. } => Ok(NumConst::Real {
            bits: *bits,
            kind: *kind,
        }),
        Expr::Int { .. } | Expr::Sizeof { .. } => Ok(NumConst::Int(eval_const_expr(expr)?)),
        Expr::Unary {
            op, expr: inner, ..
        } => eval_real_unary(*op, inner),
        Expr::Bin {
            op, left, right, ..
        } => eval_real_binop(*op, left, right),
        Expr::Cond {
            cond,
            then_expr,
            else_expr,
            ty,
            ..
        } => {
            let chosen = if eval_num(cond)?.is_zero() {
                else_expr
            } else {
                then_expr
            };
            let value = eval_num(chosen)?;
            let result = ty.as_ref().ok_or(ConstError::NotConst)?;
            if let Some(kind) = result.float_kind() {
                Ok(NumConst::Real {
                    bits: value.to_real(kind).ok_or(ConstError::NotConst)?,
                    kind,
                })
            } else if let Some(dest) = result.as_integer() {
                Ok(NumConst::Int(value.to_int(dest)?))
            } else {
                Err(ConstError::NotConst)
            }
        }
        Expr::Cast { ty_name, expr, .. } => {
            let value = eval_num(expr)?;
            if let Some(kind) = ty_name.float_kind() {
                Ok(NumConst::Real {
                    bits: value.to_real(kind).ok_or(ConstError::NotConst)?,
                    kind,
                })
            } else if let Some(dest) = ty_name.as_integer() {
                Ok(NumConst::Int(value.to_int(dest)?))
            } else {
                Err(ConstError::NotConst)
            }
        }
        Expr::Call { callee, args, .. } => eval_fp_format_builtin(callee, args),
        _ => Err(ConstError::NotConst),
    }
}

pub(super) fn eval_fp_format_builtin(callee: &Expr, args: &[Expr]) -> Result<NumConst, ConstError> {
    let Expr::Var { name, .. } = callee else {
        return Err(ConstError::NotConst);
    };
    let Some(fp) = crate::float::FpFormatBuiltin::from_name(name) else {
        return Err(ConstError::NotConst);
    };
    let bits = match fp {
        crate::float::FpFormatBuiltin::Inf(kind) => crate::float::plus_inf(kind),
        crate::float::FpFormatBuiltin::Nan { kind, signaling } => {
            let tag = crate::float::nan_tag_from_expr(&args[0]).ok_or(ConstError::NotConst)?;
            crate::float::nan_bits(kind, tag, signaling)
        }
    };
    Ok(NumConst::Real {
        bits,
        kind: fp.kind(),
    })
}

pub(super) fn eval_real_unary(op: UnaryOp, expr: &Expr) -> Result<NumConst, ConstError> {
    if matches!(op, UnaryOp::Addr | UnaryOp::Deref) {
        return Err(ConstError::NotConst);
    }
    let value = eval_num(expr)?;
    match op {
        UnaryOp::Plus => match value {
            NumConst::Int(value) => Ok(NumConst::Int(promote_val(value))),
            real => Ok(real),
        },
        UnaryOp::Minus => match value {
            NumConst::Int(value) => Ok(NumConst::Int(negate(promote_val(value))?)),
            NumConst::Real { bits, kind } => Ok(NumConst::Real {
                bits: crate::float::negate(kind, bits),
                kind,
            }),
        },
        UnaryOp::Not => Ok(NumConst::Int(c_int(i32::from(value.is_zero())))),
        UnaryOp::BitNot => {
            let NumConst::Int(value) = value else {
                return Err(ConstError::NotConst);
            };
            let value = promote_val(value);
            Ok(NumConst::Int(CVal {
                bits: (!value.bits) & value.ty.mask(),
                ty: value.ty,
            }))
        }
        UnaryOp::Addr | UnaryOp::Deref => Err(ConstError::NotConst),
    }
}

pub(super) fn eval_real_binop(
    op: BinOp,
    left: &Expr,
    right: &Expr,
) -> Result<NumConst, ConstError> {
    match op {
        BinOp::And => {
            if eval_num(left)?.is_zero() {
                return Ok(NumConst::Int(c_int(0)));
            }
            Ok(NumConst::Int(c_int(i32::from(!eval_num(right)?.is_zero()))))
        }
        BinOp::Or => {
            if !eval_num(left)?.is_zero() {
                return Ok(NumConst::Int(c_int(1)));
            }
            Ok(NumConst::Int(c_int(i32::from(!eval_num(right)?.is_zero()))))
        }
        _ => {
            let left = eval_num(left)?;
            let right = eval_num(right)?;
            if left.real_kind().is_some() || right.real_kind().is_some() {
                let kind = crate::float::usual_real(left.real_kind(), right.real_kind());
                let left_bits = left.to_real(kind).ok_or(ConstError::NotConst)?;
                let right_bits = right.to_real(kind).ok_or(ConstError::NotConst)?;
                if op.is_cmp() {
                    let cmp = match op {
                        BinOp::Eq => crate::float::FloatCmp::Eq,
                        BinOp::Ne => crate::float::FloatCmp::Ne,
                        BinOp::Lt => crate::float::FloatCmp::Lt,
                        BinOp::Gt => crate::float::FloatCmp::Gt,
                        BinOp::Le => crate::float::FloatCmp::Le,
                        BinOp::Ge => crate::float::FloatCmp::Ge,
                        _ => return Err(ConstError::NotConst),
                    };
                    return Ok(NumConst::Int(c_int(i32::from(crate::float::apply_cmp(
                        cmp, kind, left_bits, right_bits,
                    )))));
                }
                let flop = match op {
                    BinOp::Add => crate::float::FloatOp::Add,
                    BinOp::Sub => crate::float::FloatOp::Sub,
                    BinOp::Mul => crate::float::FloatOp::Mul,
                    BinOp::Div => crate::float::FloatOp::Div,
                    _ => return Err(ConstError::NotConst),
                };
                return Ok(NumConst::Real {
                    bits: crate::float::apply_bin(flop, kind, left_bits, right_bits),
                    kind,
                });
            }
            let NumConst::Int(left) = left else {
                return Err(ConstError::NotConst);
            };
            let NumConst::Int(right) = right else {
                return Err(ConstError::NotConst);
            };
            if matches!(op, BinOp::Shl | BinOp::Shr) {
                return Ok(NumConst::Int(eval_shift(op, left, right)?));
            }
            let ty = left.ty.usual(right.ty);
            let left = convert_bits(left.bits, left.ty, ty);
            let right = convert_bits(right.bits, right.ty, ty);
            if op.is_cmp() {
                return Ok(NumConst::Int(c_int(i32::from(compare(
                    op, left, right, ty,
                )))));
            }
            Ok(NumConst::Int(arith(op, left, right, ty)?))
        }
    }
}

/// Integer constant expression (C11 6.6).
///
/// `&&`, `||`, and `?:` skip the operand that is not evaluated.
/// `sizeof` is not evaluated. Its result is an `unsigned long`.
/// Signed arithmetic that does not fit in the result type is rejected,
/// including the minimum signed value divided or remaindered by `-1`.
/// Unsigned arithmetic wraps. A shift count that is negative or not less
/// than the width of the promoted left operand is rejected. A signed left
/// shift of a negative value, or one whose result does not fit, is rejected.
/// Unsigned left shifts wrap. Signed right shifts are arithmetic; unsigned
/// right shifts are logical.
/// Assignment, `++`/`--`, calls, and comma are not constant expressions when
/// they are evaluated.
pub(super) fn eval_const_expr(expr: &Expr) -> Result<CVal, ConstError> {
    match expr {
        Expr::Int { bits, value_ty, .. } => Ok(CVal {
            bits: u128::from(*bits) & value_ty.mask(),
            ty: *value_ty,
        }),
        Expr::Float { .. } => Err(ConstError::NotConst),
        Expr::Sizeof { operand, .. } => {
            let size = match operand {
                SizeofOperand::Type(ty) => ty.size(),
                SizeofOperand::Expr(inner) => inner.ty().expect("sizeof operand is typed").size(),
            };
            Ok(CVal {
                bits: u128::from(size.ok_or(ConstError::NotConst)?),
                ty: TypeName::ULong,
            })
        }
        Expr::Unary { op, expr, .. } => eval_unary(*op, expr),
        Expr::Bin {
            op, left, right, ..
        } => eval_binop(*op, left, right),
        Expr::Cond {
            cond,
            then_expr,
            else_expr,
            ty,
            ..
        } => {
            // The chosen arm is converted to the conditional's result type,
            // the same conversion the IR does before the phi.
            let chosen = if is_zero(&eval_const_expr(cond)?) {
                else_expr
            } else {
                then_expr
            };
            let value = eval_const_expr(chosen)?;
            let result = ty
                .as_ref()
                .and_then(CType::as_integer)
                .ok_or(ConstError::NotConst)?;
            Ok(CVal {
                bits: convert_bits(value.bits, value.ty, result),
                ty: result,
            })
        }
        Expr::Cast { ty_name, expr, .. } => {
            let Some(dest) = ty_name.as_integer() else {
                return Err(ConstError::NotConst);
            };
            // C11 6.6: a floating constant may appear in an integer constant
            // expression only as the immediate operand of a cast. `(int)1.5`
            // is folded; a bare `1.5` stays non-constant.
            if let Expr::Float { bits, kind, .. } = expr.as_ref() {
                let bits = crate::float::real_to_int(*bits, *kind, dest)
                    .ok_or(ConstError::Overflow(dest))?;
                return Ok(CVal { bits, ty: dest });
            }
            let value = eval_const_expr(expr)?;
            Ok(CVal {
                bits: convert_bits(value.bits, value.ty, dest),
                ty: dest,
            })
        }
        Expr::Var { .. }
        | Expr::Assign { .. }
        | Expr::CompoundAssign { .. }
        | Expr::IncDec { .. }
        | Expr::Call { .. }
        | Expr::Comma { .. }
        | Expr::Index { .. }
        | Expr::Decay { .. }
        | Expr::Str { .. }
        | Expr::Member { .. }
        | Expr::VaArg { .. }
        | Expr::VaOp { .. }
        | Expr::Generic { .. }
        | Expr::StmtExpr { .. }
        | Expr::LabelAddr { .. }
        | Expr::CompoundLit { .. } => Err(ConstError::NotConst),
        Expr::CondShort {
            cond,
            else_expr,
            ty,
            ..
        } => {
            let first = eval_const_expr(cond)?;
            let value = if is_zero(&first) {
                eval_const_expr(else_expr)?
            } else {
                first
            };
            let result = ty
                .as_ref()
                .and_then(CType::as_integer)
                .ok_or(ConstError::NotConst)?;
            Ok(CVal {
                bits: convert_bits(value.bits, value.ty, result),
                ty: result,
            })
        }
    }
}

pub(super) fn eval_unary(op: UnaryOp, expr: &Expr) -> Result<CVal, ConstError> {
    if matches!(op, UnaryOp::Addr | UnaryOp::Deref) {
        return Err(ConstError::NotConst);
    }
    let value = promote_val(eval_const_expr(expr)?);
    match op {
        UnaryOp::Plus => Ok(value),
        UnaryOp::Minus => negate(value),
        UnaryOp::Not => Ok(c_int(i32::from(is_zero(&value)))),
        UnaryOp::BitNot => Ok(CVal {
            bits: (!value.bits) & value.ty.mask(),
            ty: value.ty,
        }),
        UnaryOp::Addr | UnaryOp::Deref => Err(ConstError::NotConst),
    }
}

pub(super) fn promote_val(value: CVal) -> CVal {
    let ty = value.ty.promote();
    CVal {
        bits: convert_bits(value.bits, value.ty, ty),
        ty,
    }
}

pub(super) fn negate(value: CVal) -> Result<CVal, ConstError> {
    if value.ty.is_signed() {
        let signed = sign_extend(value.bits, value.ty.width());
        bits_from_signed(
            signed.checked_neg().ok_or(ConstError::Overflow(value.ty))?,
            value.ty,
        )
    } else {
        Ok(CVal {
            bits: value.bits.wrapping_neg() & value.ty.mask(),
            ty: value.ty,
        })
    }
}

pub(super) fn eval_binop(op: BinOp, left: &Expr, right: &Expr) -> Result<CVal, ConstError> {
    match op {
        BinOp::And => {
            if is_zero(&eval_const_expr(left)?) {
                return Ok(c_int(0));
            }
            Ok(c_int(i32::from(!is_zero(&eval_const_expr(right)?))))
        }
        BinOp::Or => {
            if !is_zero(&eval_const_expr(left)?) {
                return Ok(c_int(1));
            }
            Ok(c_int(i32::from(!is_zero(&eval_const_expr(right)?))))
        }
        _ => {
            let left = eval_const_expr(left)?;
            let right = eval_const_expr(right)?;
            if matches!(op, BinOp::Shl | BinOp::Shr) {
                return eval_shift(op, left, right);
            }
            let ty = left.ty.usual(right.ty);
            let left = convert_bits(left.bits, left.ty, ty);
            let right = convert_bits(right.bits, right.ty, ty);
            if op.is_cmp() {
                return Ok(c_int(i32::from(compare(op, left, right, ty))));
            }
            arith(op, left, right, ty)
        }
    }
}

pub(super) fn compare(op: BinOp, left: u128, right: u128, ty: TypeName) -> bool {
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
            _ => unreachable!("comparison"),
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
            _ => unreachable!("comparison"),
        }
    }
}

pub(super) fn arith(op: BinOp, left: u128, right: u128, ty: TypeName) -> Result<CVal, ConstError> {
    if ty.is_signed() {
        let left = sign_extend(left, ty.width());
        let right = sign_extend(right, ty.width());
        let value = match op {
            BinOp::Add => left.checked_add(right),
            BinOp::Sub => left.checked_sub(right),
            BinOp::Mul => left.checked_mul(right),
            BinOp::Div => return div_signed(left, right, ty),
            BinOp::Mod => return rem_signed(left, right, ty),
            BinOp::BitAnd => Some(left & right),
            BinOp::BitOr => Some(left | right),
            BinOp::BitXor => Some(left ^ right),
            _ => unreachable!("arithmetic operator"),
        };
        let Some(value) = value else {
            return Err(ConstError::Overflow(ty));
        };
        bits_from_signed(value, ty)
    } else {
        let mask = ty.mask();
        let left = left & mask;
        let right = right & mask;
        let bits = match op {
            BinOp::Add => left.wrapping_add(right),
            BinOp::Sub => left.wrapping_sub(right),
            BinOp::Mul => left.wrapping_mul(right),
            BinOp::Div => {
                if right == 0 {
                    return Err(ConstError::DivZero);
                }
                left / right
            }
            BinOp::Mod => {
                if right == 0 {
                    return Err(ConstError::DivZero);
                }
                left % right
            }
            BinOp::BitAnd => left & right,
            BinOp::BitOr => left | right,
            BinOp::BitXor => left ^ right,
            _ => unreachable!("arithmetic operator"),
        };
        Ok(CVal {
            bits: bits & mask,
            ty,
        })
    }
}

pub(super) fn bits_from_signed(value: i128, ty: TypeName) -> Result<CVal, ConstError> {
    let (min, max) = crate::ast::signed_limits(ty.width());
    if value < min || value > max {
        return Err(ConstError::Overflow(ty));
    }
    Ok(CVal {
        bits: (value as u128) & ty.mask(),
        ty,
    })
}

pub(super) fn signed_min(ty: TypeName) -> i128 {
    crate::ast::signed_limits(ty.width()).0
}

/// `MIN / -1` and `MIN % -1` are not representable (C11 6.5.5).
pub(super) fn div_overflows(left: i128, right: i128, ty: TypeName) -> bool {
    left == signed_min(ty) && right == -1
}

pub(super) fn div_signed(left: i128, right: i128, ty: TypeName) -> Result<CVal, ConstError> {
    if right == 0 {
        return Err(ConstError::DivZero);
    }
    if div_overflows(left, right, ty) {
        return Err(ConstError::Overflow(ty));
    }
    bits_from_signed(left / right, ty)
}

pub(super) fn rem_signed(left: i128, right: i128, ty: TypeName) -> Result<CVal, ConstError> {
    if right == 0 {
        return Err(ConstError::DivZero);
    }
    if div_overflows(left, right, ty) {
        return Err(ConstError::Overflow(ty));
    }
    bits_from_signed(left % right, ty)
}

/// The count is the promoted right operand. It must be nonnegative and less
/// than the width of the promoted left operand. Signed left shifts must not
/// lose the sign or any magnitude bit. Unsigned shifts wrap.
pub(super) fn eval_shift(op: BinOp, left: CVal, right: CVal) -> Result<CVal, ConstError> {
    let left = promote_val(left);
    let right = promote_val(right);
    let count = shift_count(&right, left.ty.width())?;
    let width = left.ty.width();
    if op == BinOp::Shr {
        let bits = if left.ty.is_signed() {
            let shifted = sign_extend(left.bits, width) >> count;
            shifted as u128 & left.ty.mask()
        } else {
            (left.bits & left.ty.mask()) >> count
        };
        return Ok(CVal { bits, ty: left.ty });
    }
    if left.ty.is_signed() {
        let signed = sign_extend(left.bits, width);
        if signed < 0 {
            return Err(ConstError::Shift);
        }
        if !crate::ast::signed_shl_fits(signed, count, width) {
            return Err(ConstError::Overflow(left.ty));
        }
        bits_from_signed(signed << count, left.ty)
    } else {
        Ok(CVal {
            bits: ((left.bits & left.ty.mask()) << count) & left.ty.mask(),
            ty: left.ty,
        })
    }
}

pub(super) fn shift_count(count: &CVal, width: u32) -> Result<u32, ConstError> {
    if count.ty.is_signed() {
        let signed = sign_extend(count.bits, count.ty.width());
        if signed < 0 || signed >= i128::from(width) {
            return Err(ConstError::Shift);
        }
        Ok(signed as u32)
    } else {
        let unsigned = count.bits & count.ty.mask();
        if unsigned >= u128::from(width) {
            return Err(ConstError::Shift);
        }
        Ok(unsigned as u32)
    }
}
