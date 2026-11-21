//! Constraints on `_Atomic` types that lowering can represent (C11 6.7.2.4,
//! 6.7.3). Lock-free widths are 1, 2, 4, 8, and 16 bytes. Larger atomic
//! objects are not implemented (no locking runtime).

use super::*;
use std::collections::HashSet;

impl Sema<'_> {
    /// Diagnose an atomic type this compiler does not lower, including when
    /// it appears as a typedef, a member, a pointer target, or a parameter.
    pub(super) fn check_atomic_types(
        &self,
        ty: &CType,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        self.check_atomic_types_allowing(ty, line, column, false)
    }

    /// Like [`Self::check_atomic_types`], but an incomplete atomic type is
    /// allowed. A prototype may name an incomplete type (C11 6.7.6.3p12);
    /// a known size that is not lock-free is still diagnosed.
    pub(super) fn check_atomic_types_allowing(
        &self,
        ty: &CType,
        line: usize,
        column: usize,
        allow_incomplete: bool,
    ) -> Result<(), Diagnostic> {
        if let Some(message) = bad_atomic(ty, &mut HashSet::new(), allow_incomplete) {
            return self.err(line, column, message);
        }
        Ok(())
    }
}

fn bad_atomic(ty: &CType, seen: &mut HashSet<u32>, allow_incomplete: bool) -> Option<String> {
    if ty.is_atomic() {
        match ty.size() {
            Some(size) if CType::is_lock_free_atomic_size(size) => {}
            Some(size) => {
                return Some(format!(
                    "atomic type '{}' ({size} bytes) is not lock-free; this compiler \
                     supports lock-free atomics of 1, 2, 4, 8, or 16 bytes and does \
                     not implement locking atomics",
                    ty.display()
                ));
            }
            None if allow_incomplete => {}
            None => {
                return Some(format!("atomic type '{}' is incomplete", ty.display()));
            }
        }
    }
    // Recurse through pointers and arrays. Record types are memoized so a
    // self-referential struct, and a nest of unions, stays linear.
    match ty.peel() {
        CType::Pointer(inner) => bad_atomic(inner, seen, allow_incomplete),
        CType::Array(inner, _) => bad_atomic(inner, seen, allow_incomplete),
        CType::Func(func) => bad_atomic(&func.ret, seen, allow_incomplete).or_else(|| {
            func.params
                .iter()
                .find_map(|param| bad_atomic(param, seen, allow_incomplete))
        }),
        CType::Struct(handle) => {
            if !seen.insert(handle.id()) {
                return None;
            }
            handle.fields_ref().and_then(|fields| {
                fields
                    .iter()
                    .find_map(|field| bad_atomic(&field.ty, seen, allow_incomplete))
            })
        }
        _ => None,
    }
}

impl Sema<'_> {
    /// Type-generic `__hcc_atomic_*` builtins used by <stdatomic.h>.
    pub(super) fn check_atomic_builtin(
        &mut self,
        expr: &mut Expr,
        name: &str,
        op: crate::builtins::AtomicOp,
    ) -> Result<(), Diagnostic> {
        use crate::builtins::AtomicOp;
        let (line, column, argc) = match expr {
            Expr::Call {
                args, line, column, ..
            } => (*line, *column, args.len()),
            _ => unreachable!("atomic builtin"),
        };
        let arity = op.arity();
        if argc != arity {
            return self.err(
                line,
                column,
                format!(
                    "function '{name}' expects {} {}, found {argc}",
                    arity,
                    if arity == 1 { "argument" } else { "arguments" },
                ),
            );
        }
        let Expr::Call { args, .. } = expr else {
            unreachable!("atomic builtin");
        };
        for arg in args.iter_mut() {
            self.check_expr(arg)?;
            self.decay(arg)?;
        }

        let ret = match op {
            AtomicOp::ThreadFence | AtomicOp::SignalFence => {
                require_order_ty(self, &args[0])?;
                check_fence_order(self, &args[0], op, name)?;
                CType::Void
            }
            AtomicOp::IsLockFree => {
                let obj = atomic_object_ty(self, &args[0], name)?;
                let _ = obj.size();
                CType::from_name(TypeName::Bool)
            }
            AtomicOp::Init => {
                let obj = atomic_object_ty(self, &args[0], name)?;
                if obj.is_const() {
                    return self.err(
                        args[0].line(),
                        args[0].column(),
                        format!("argument of {name} must be a pointer to a non-const atomic type"),
                    );
                }
                let dest = obj.unqualified();
                self.can_assign(&dest, &args[1])?;
                CType::Void
            }
            AtomicOp::Load => {
                let obj = atomic_object_ty(self, &args[0], name)?;
                require_order_ty(self, &args[1])?;
                check_named_order(self, &args[1], name, OrderUse::Load)?;
                obj.unqualified()
            }
            AtomicOp::Store => {
                let obj = atomic_object_ty(self, &args[0], name)?;
                if obj.is_const() {
                    return self.err(
                        args[0].line(),
                        args[0].column(),
                        format!("argument of {name} must be a pointer to a non-const atomic type"),
                    );
                }
                self.can_assign(&obj.unqualified(), &args[1])?;
                require_order_ty(self, &args[2])?;
                check_named_order(self, &args[2], name, OrderUse::Store)?;
                CType::Void
            }
            AtomicOp::FlagClear => {
                let obj = atomic_object_ty(self, &args[0], name)?;
                if obj.is_const() {
                    return self.err(
                        args[0].line(),
                        args[0].column(),
                        format!("argument of {name} must be a pointer to a non-const atomic type"),
                    );
                }
                // <stdatomic.h> stores the flag as `_Atomic unsigned char`.
                if obj.unqualified().as_integer() != Some(TypeName::UChar) {
                    return self.err(
                        args[0].line(),
                        args[0].column(),
                        format!("argument of {name} must be a pointer to _Atomic unsigned char"),
                    );
                }
                require_order_ty(self, &args[1])?;
                check_named_order(self, &args[1], name, OrderUse::FlagClear)?;
                CType::Void
            }
            AtomicOp::Exchange => {
                let obj = atomic_object_ty(self, &args[0], name)?;
                if obj.is_const() {
                    return self.err(
                        args[0].line(),
                        args[0].column(),
                        format!("argument of {name} must be a pointer to a non-const atomic type"),
                    );
                }
                let dest = obj.unqualified();
                self.can_assign(&dest, &args[1])?;
                require_order_ty(self, &args[2])?;
                check_named_order(self, &args[2], name, OrderUse::Rmw)?;
                dest
            }
            AtomicOp::CompareExchange => {
                let obj = atomic_object_ty(self, &args[0], name)?;
                if obj.is_const() {
                    return self.err(
                        args[0].line(),
                        args[0].column(),
                        format!("argument of {name} must be a pointer to a non-const atomic type"),
                    );
                }
                let dest = obj.unqualified();
                let expected = args[1].ty().and_then(|ty| ty.pointee().cloned());
                let Some(expected) = expected else {
                    return self.err(
                        args[1].line(),
                        args[1].column(),
                        format!("expected-value argument of {name} must be a pointer"),
                    );
                };
                if expected.is_const() {
                    return self.err(
                        args[1].line(),
                        args[1].column(),
                        format!("expected-value argument of {name} must be a pointer to a non-const type"),
                    );
                }
                if expected.is_atomic() {
                    return self.err(
                        args[1].line(),
                        args[1].column(),
                        format!("expected-value argument of {name} must be a pointer to a non-atomic type"),
                    );
                }
                if !compatible(&expected.unqualified(), &dest) {
                    return self.err(
                        args[1].line(),
                        args[1].column(),
                        format!(
                            "expected-value argument of {name} must be a pointer to '{}'",
                            dest.display()
                        ),
                    );
                }
                self.can_assign(&dest, &args[2])?;
                require_order_ty(self, &args[3])?;
                require_order_ty(self, &args[4])?;
                if !args[5].ty().is_some_and(|ty| ty.is_integer()) {
                    return self.err(
                        args[5].line(),
                        args[5].column(),
                        format!("weak argument of {name} must be an integer"),
                    );
                }
                check_cmpxchg_orders(self, &args[3], &args[4], name)?;
                CType::from_name(TypeName::Bool)
            }
            AtomicOp::FetchAdd | AtomicOp::FetchSub => {
                let obj = atomic_object_ty(self, &args[0], name)?;
                if obj.is_const() {
                    return self.err(
                        args[0].line(),
                        args[0].column(),
                        format!("argument of {name} must be a pointer to a non-const atomic type"),
                    );
                }
                let dest = obj.unqualified();
                if dest.as_integer() == Some(TypeName::Bool) {
                    return self.err(
                        args[0].line(),
                        args[0].column(),
                        format!("{name} is not applicable to _Atomic _Bool"),
                    );
                }
                let operand = if dest.is_pointer() {
                    let pointee = dest.pointee().expect("pointer dest");
                    if pointee.size().is_none() && !pointee.is_vla() {
                        return self.err(
                            args[0].line(),
                            args[0].column(),
                            format!("{name} requires a pointer to a complete object type"),
                        );
                    }
                    CType::from_name(TypeName::ptrdiff_t())
                } else if dest.is_integer() {
                    dest.clone()
                } else {
                    return self.err(
                        args[0].line(),
                        args[0].column(),
                        format!("{name} requires an atomic integer or pointer type"),
                    );
                };
                self.can_assign(&operand, &args[1])?;
                require_order_ty(self, &args[2])?;
                check_named_order(self, &args[2], name, OrderUse::Rmw)?;
                dest
            }
            AtomicOp::FetchOr | AtomicOp::FetchXor | AtomicOp::FetchAnd => {
                let obj = atomic_object_ty(self, &args[0], name)?;
                if obj.is_const() {
                    return self.err(
                        args[0].line(),
                        args[0].column(),
                        format!("argument of {name} must be a pointer to a non-const atomic type"),
                    );
                }
                let dest = obj.unqualified();
                if dest.as_integer() == Some(TypeName::Bool) {
                    return self.err(
                        args[0].line(),
                        args[0].column(),
                        format!("{name} is not applicable to _Atomic _Bool"),
                    );
                }
                if !dest.is_integer() {
                    return self.err(
                        args[0].line(),
                        args[0].column(),
                        format!("{name} requires an atomic integer type"),
                    );
                }
                self.can_assign(&dest, &args[1])?;
                require_order_ty(self, &args[2])?;
                check_named_order(self, &args[2], name, OrderUse::Rmw)?;
                dest
            }
        };

        if let Expr::Call { resolved, .. } = expr {
            *resolved = Some(CallRes {
                name: name.to_string(),
                ret: ret.clone(),
                params: Vec::new(),
                variadic: false,
                indirect: false,
            });
        }
        expr.set_ty(ret);
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OrderUse {
    Load,
    Store,
    FlagClear,
    Rmw,
    Fence,
}

fn require_order_ty(sema: &Sema<'_>, expr: &Expr) -> Result<(), Diagnostic> {
    if expr.ty().is_some_and(|ty| ty.is_integer()) {
        Ok(())
    } else {
        sema.err(
            expr.line(),
            expr.column(),
            "memory_order argument must be an integer",
        )
    }
}

fn const_order(expr: &Expr) -> Option<i64> {
    const_index(expr).ok()
}

fn check_named_order(
    sema: &Sema<'_>,
    expr: &Expr,
    name: &str,
    use_: OrderUse,
) -> Result<(), Diagnostic> {
    let Some(code) = const_order(expr) else {
        return Ok(());
    };
    let Some(order) = memory_order_name(code) else {
        return sema.err(
            expr.line(),
            expr.column(),
            format!("invalid memory_order value {code} in {name}"),
        );
    };
    let ok = match use_ {
        OrderUse::Load => matches!(code, 0 | 1 | 2 | 5),
        OrderUse::Store => matches!(code, 0 | 3 | 5),
        // C11 7.17.8.2: flag clear forbids acquire and acq_rel, not consume.
        OrderUse::FlagClear => matches!(code, 0 | 1 | 3 | 5),
        OrderUse::Rmw | OrderUse::Fence => true,
    };
    if !ok {
        return sema.err(
            expr.line(),
            expr.column(),
            format!("memory_order_{order} is not valid for {name}"),
        );
    }
    Ok(())
}

fn check_fence_order(
    sema: &Sema<'_>,
    expr: &Expr,
    op: crate::builtins::AtomicOp,
    name: &str,
) -> Result<(), Diagnostic> {
    let _ = op;
    check_named_order(sema, expr, name, OrderUse::Fence)
}

fn check_cmpxchg_orders(
    sema: &Sema<'_>,
    success: &Expr,
    failure: &Expr,
    name: &str,
) -> Result<(), Diagnostic> {
    check_named_order(sema, success, name, OrderUse::Rmw)?;
    let Some(fail) = const_order(failure) else {
        return Ok(());
    };
    let Some(fail_name) = memory_order_name(fail) else {
        return sema.err(
            failure.line(),
            failure.column(),
            format!("invalid memory_order value {fail} in {name}"),
        );
    };
    // C11 7.17.7.4p2: failure is not release or acq_rel, and is no
    // stronger than success.
    if fail == 3 || fail == 4 {
        return sema.err(
            failure.line(),
            failure.column(),
            format!("memory_order_{fail_name} is not valid as a compare-exchange failure order"),
        );
    }
    if let Some(succ) = const_order(success) {
        if fail > succ {
            return sema.err(
                failure.line(),
                failure.column(),
                format!(
                    "compare-exchange failure order memory_order_{fail_name} is stronger than the success order"
                ),
            );
        }
    }
    Ok(())
}

fn memory_order_name(code: i64) -> Option<&'static str> {
    Some(match code {
        0 => "relaxed",
        1 => "consume",
        2 => "acquire",
        3 => "release",
        4 => "acq_rel",
        5 => "seq_cst",
        _ => return None,
    })
}

/// The atomic object type pointed to by `arg`, or a diagnostic.
fn atomic_object_ty(sema: &Sema<'_>, arg: &Expr, name: &str) -> Result<CType, Diagnostic> {
    let Some(ty) = arg.ty() else {
        return sema.err(
            arg.line(),
            arg.column(),
            format!("argument of {name} is untyped"),
        );
    };
    let Some(inner) = ty.pointee() else {
        return sema.err(
            arg.line(),
            arg.column(),
            format!("argument of {name} must be a pointer to an atomic type"),
        );
    };
    if !inner.is_atomic() {
        return sema.err(
            arg.line(),
            arg.column(),
            format!("argument of {name} must be a pointer to an atomic type"),
        );
    }
    Ok(inner.clone())
}
