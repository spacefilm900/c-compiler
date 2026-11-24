//! Assignment conversions (C11 6.5.16.1), operand requirements and the type
//! of a conditional expression.

use super::*;

impl Sema<'_> {
    /// A struct or union passed or returned by value must be complete, and
    /// so must a GNU forward-declared enum. An x87 aggregate is passed in
    /// memory. A return whose class is `X87` uses `%st0`. SSE eightbytes are
    /// passed in XMM registers.
    pub(super) fn require_passed_aggregate(
        &self,
        ty: &CType,
        line: usize,
        column: usize,
        is_return: bool,
    ) -> Result<(), Diagnostic> {
        let bare = ty.unqualified();
        if bare.is_incomplete_enum() {
            let what = if is_return {
                "function returns"
            } else {
                "parameter has"
            };
            return self.err(
                line,
                column,
                format!("{what} incomplete type '{}'", bare.display()),
            );
        }
        if !bare.is_record() {
            return Ok(());
        }
        let use_as = if is_return {
            crate::abi::AggUse::Return
        } else {
            crate::abi::AggUse::Param
        };
        match crate::abi::classify_for(&bare, use_as) {
            Ok(_) => Ok(()),
            Err(crate::abi::AbiFail::Incomplete) => {
                if is_return {
                    self.err(
                        line,
                        column,
                        format!("function returns incomplete type '{}'", bare.display()),
                    )
                } else {
                    self.err(
                        line,
                        column,
                        format!("parameter has incomplete type '{}'", bare.display()),
                    )
                }
            }
            Err(crate::abi::AbiFail::FloatReg) => {
                if is_return {
                    self.err(line, column, "returning an x87 aggregate is not supported")
                } else {
                    self.err(line, column, "passing an x87 aggregate is not supported")
                }
            }
        }
    }

    /// Implicit assignment conversion (C11 6.5.16.1).
    ///
    /// Arithmetic types convert. A pointer may be assigned a pointer to a
    /// qualified or unqualified version of a compatible type when the
    /// destination pointee has every qualifier of the source pointee, so
    /// `char *` may initialize `const char *` and `const char *` may not
    /// initialize `char *`. `void *` follows the same qualifier rule. A null
    /// pointer constant may initialize any pointer. Any other pointer mismatch
    /// is an error. Clang only warns for incompatible pointer types
    /// (`-Wincompatible-pointer-types`) and still emits code; this compiler
    /// rejects the program. A cast may drop qualifiers; that is not diagnosed.
    /// With `-std=gnu11`, a pointee that differs only in integer signedness
    /// or that drops qualifiers is accepted with a warning, as GCC and Clang
    /// do in GNU mode; other pointer mismatches stay errors.
    pub(super) fn can_assign(&self, dest: &CType, src: &Expr) -> Result<(), Diagnostic> {
        let src_ty = src.ty().expect("typed");
        if dest.is_arithmetic() && src_ty.is_arithmetic() {
            return Ok(());
        }
        if src_ty.is_void() {
            return self.err(
                src.line(),
                src.column(),
                "void value used where a value is required",
            );
        }
        if dest.is_integer() && src_ty.is_integer() {
            return Ok(());
        }
        // C11 6.5.16.1: a pointer may be assigned to `_Bool`. The value is
        // 0 when the pointer is null and 1 otherwise.
        if dest.as_integer() == Some(TypeName::Bool) && src_ty.is_pointer() {
            return Ok(());
        }
        if dest.unqualified().is_record()
            && src_ty.unqualified().is_record()
            && compatible(&dest.unqualified(), &src_ty.unqualified())
        {
            return Ok(());
        }
        if dest.is_pointer()
            && ((src_ty.is_pointer() && pointer_assignable(dest, &src_ty))
                || is_null_pointer_constant(src))
        {
            return Ok(());
        }
        if dest.is_pointer() && self.dialect.gnu() && void_function_ptr_mix(dest, &src_ty) {
            self.warn(
                src.line(),
                src.column(),
                format!(
                    "assigning '{}' to '{}' converts between a void pointer and a \
                     function pointer; C11 6.5.16.1p1 does not allow this; accepted \
                     as a GNU extension (-std=gnu11)",
                    src_ty.display(),
                    dest.display()
                ),
            );
            return Ok(());
        }
        if self.dialect.gnu() && gnu_pointer_sign_or_qual_mix(dest, &src_ty) {
            self.warn(
                src.line(),
                src.column(),
                format!(
                    "assigning '{}' to '{}' changes the signedness or drops a qualifier \
                     of the pointed-to type; C11 6.5.16.1p1 does not allow this; \
                     accepted as a GNU extension (-std=gnu11)",
                    src_ty.display(),
                    dest.display()
                ),
            );
            return Ok(());
        }
        self.err(
            src.line(),
            src.column(),
            format!(
                "incompatible types assigning '{}' to '{}'",
                src_ty.display(),
                dest.display()
            ),
        )
    }

    /// The operand's integer type, or an error if it is not an integer.
    pub(super) fn require_integer(&self, expr: &Expr) -> Result<TypeName, Diagnostic> {
        let ty = expr.ty();
        if let Some(int) = ty.as_ref().and_then(CType::as_integer) {
            return Ok(int);
        }
        match ty {
            Some(ty) if ty.is_void() => self.err(
                expr.line(),
                expr.column(),
                "void value used where a value is required",
            ),
            Some(ty) => self.err(
                expr.line(),
                expr.column(),
                format!("expected an integer, found '{}'", ty.display()),
            ),
            None => self.ice(expr.line(), expr.column(), "expression has no type"),
        }
    }

    pub(super) fn require_scalar(&self, expr: &Expr) -> Result<(), Diagnostic> {
        match expr.ty() {
            Some(ty) if ty.is_scalar() => Ok(()),
            Some(ty) if ty.is_void() => self.err(
                expr.line(),
                expr.column(),
                "void value used where a value is required",
            ),
            Some(ty) => self.err(
                expr.line(),
                expr.column(),
                format!("expected a scalar, found '{}'", ty.display()),
            ),
            None => self.ice(expr.line(), expr.column(), "expression has no type"),
        }
    }

    pub(super) fn cond_type(
        &self,
        then_expr: &Expr,
        else_expr: &Expr,
        line: usize,
        column: usize,
    ) -> Result<CType, Diagnostic> {
        let left = then_expr.ty().expect("typed");
        let right = else_expr.ty().expect("typed");
        if left.is_void() && right.is_void() {
            return Ok(CType::Void);
        }
        if left.unqualified().is_record() && compatible(&left.unqualified(), &right.unqualified()) {
            return Ok(left);
        }
        if (left.is_float() || right.is_float()) && left.is_arithmetic() && right.is_arithmetic() {
            return Ok(CType::Float(crate::float::usual_real(
                left.float_kind(),
                right.float_kind(),
            )));
        }
        if left.is_integer() && right.is_integer() {
            return Ok(CType::from_name(
                left.as_integer()
                    .unwrap()
                    .usual(right.as_integer().unwrap()),
            ));
        }
        // C11 6.5.15p6: a null pointer constant takes the other operand's
        // type, so `p ? (int *)q : (void *)0` is `int *`, not `void *`.
        if left.is_pointer() && is_null_pointer_constant(else_expr) {
            return Ok(left);
        }
        if right.is_pointer() && is_null_pointer_constant(then_expr) {
            return Ok(right);
        }
        if left.is_pointer() && right.is_pointer() {
            if let Some(merged) = merge_pointers(&left, &right) {
                return Ok(merged);
            }
        }
        // C11 6.5.15p3 pairs `void *` only with an object pointer (or a
        // null pointer constant, below). GNU C also pairs it with a function
        // pointer, giving `void *`.
        if self.dialect.gnu()
            && void_function_ptr_mix(&left, &right)
            && !is_null_pointer_constant(then_expr)
            && !is_null_pointer_constant(else_expr)
        {
            self.warn(
                line,
                column,
                format!(
                    "conditional operands '{}' and '{}' mix a void pointer and a function \
                     pointer; C11 6.5.15p3 does not allow this; accepted as a GNU \
                     extension (-std=gnu11)",
                    left.display(),
                    right.display()
                ),
            );
            let void = if left.is_void_pointer() {
                &left
            } else {
                &right
            };
            return Ok(void.clone());
        }
        self.err(
            line,
            column,
            "incompatible operand types in the conditional operator",
        )
    }
}

/// Assignment between pointers (C11 6.5.16.1). The destination pointee must
/// have every qualifier of the source pointee. `char *` converts to
/// `const char *`. `char **` does not convert to `const char **`. A `void *`
/// pairs only with a pointer to an object type, not a function pointer.
pub(super) fn pointer_assignable(dest: &CType, src: &CType) -> bool {
    let (Some(dp), Some(sp)) = (dest.pointee(), src.pointee()) else {
        return false;
    };
    if !dp.quals().includes(sp.quals()) {
        return false;
    }
    compatible(&dp.unqualified(), &sp.unqualified())
        || (dp.unqualified().is_void() && !sp.is_func())
        || (sp.unqualified().is_void() && !dp.is_func())
}
