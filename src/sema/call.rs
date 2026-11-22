//! Function calls, library and inline builtins, and `va_list` operations.

use super::*;

impl Sema<'_> {
    pub(super) fn check_call(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        if let Some(result) = self.check_builtin_call(expr) {
            return result;
        }
        let direct = match expr {
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Var { name, .. } => matches!(self.lookup(name), Some(Symbol::Func(_))),
                _ => false,
            },
            _ => false,
        };
        if !direct {
            return self.check_indirect_call(expr);
        }
        let (name, line, column, argc) = match expr {
            Expr::Call {
                callee,
                args,
                line,
                column,
                ..
            } => {
                let name = match callee.as_ref() {
                    Expr::Var { name, .. } => name.clone(),
                    _ => {
                        return self.err(
                            *line,
                            *column,
                            "only direct function calls are supported",
                        );
                    }
                };
                (name, *line, *column, args.len())
            }
            _ => unreachable!("check_call"),
        };
        let sym = match self.lookup(&name) {
            Some(sym) => sym,
            None => {
                return self.err(
                    line,
                    column,
                    format!("use of undeclared identifier '{name}'"),
                );
            }
        };
        let (fname, ret, params, variadic) = match sym {
            // Without a prototype every argument gets the default argument
            // promotions (C11 6.5.2.2p6), and the call is lowered like a
            // call to a variadic function with no fixed parameters.
            Symbol::Func(func) if !func.prototype => (func.symbol, func.ret, Vec::new(), true),
            Symbol::Func(func) => (func.symbol, func.ret, func.params, func.variadic),
            Symbol::Var(_) => unreachable!("direct call is a function"),
        };
        let unprototyped =
            matches!(self.lookup(&name), Some(Symbol::Func(func)) if !func.prototype);
        self.require_passed_aggregate(&ret, line, column, true)?;
        self.check_call_arity(&name, argc, params.len(), variadic, line, column)?;
        if let Expr::Call { args, .. } = expr {
            self.check_call_args(args, &params)?;
        }
        self.note_use(&name, line, column);
        if let Some(message) = self.deprecated.get(&fname) {
            let text = if message.is_empty() {
                format!("'{name}' is deprecated")
            } else {
                format!("'{name}' is deprecated: {message}")
            };
            self.warn(line, column, text);
        }
        if let Expr::Call { resolved, .. } = expr {
            *resolved = Some(CallRes {
                name: fname,
                ret: ret.clone(),
                params,
                variadic: variadic && !(unprototyped && fixed_unprototyped_calls()),
                indirect: false,
            });
        }
        expr.set_ty(ret);
        Ok(())
    }

    /// A call of a builtin, or of a library function's `__builtin_` name
    /// that the program does not declare itself: its result, or `None`
    /// for an ordinary call.
    fn check_builtin_call(&mut self, expr: &mut Expr) -> Option<Result<(), Diagnostic>> {
        if let Expr::Call { callee, .. } = &*expr {
            if let Expr::Var { name, .. } = callee.as_ref() {
                if matches!(
                    name.as_str(),
                    "__builtin_memcpy" | "__builtin_memmove" | "__builtin_memset"
                ) {
                    let name = name.clone();
                    return Some(self.check_memory_builtin(expr, &name));
                }
                if matches!(
                    name.as_str(),
                    "__builtin_signbit" | "__builtin_signbitf" | "__builtin_signbitl"
                ) {
                    let name = name.clone();
                    return Some(self.check_signbit(expr, &name));
                }
                if crate::float::FpFormatBuiltin::from_name(name).is_some() {
                    let name = name.clone();
                    return Some(self.check_fp_format_builtin(expr, &name));
                }
                if crate::float::is_fp_compare_builtin(name) {
                    let name = name.clone();
                    return Some(self.check_fp_compare(expr, &name));
                }
                if let Some(checked) = crate::builtins::checked_builtin(name) {
                    let name = name.clone();
                    return Some(self.check_checked_builtin(expr, &name, checked));
                }
                if crate::builtins::is_object_size_builtin(name) {
                    let name = name.clone();
                    return Some(self.check_object_size(expr, &name));
                }
                if let Some(inline) = crate::builtins::Inline::from_name(name) {
                    let name = name.clone();
                    return Some(self.check_inline_builtin(expr, &name, inline));
                }
                if let Some(op) = crate::builtins::AtomicOp::from_name(name) {
                    let name = name.clone();
                    return Some(self.check_atomic_builtin(expr, &name, op));
                }
                if self.lookup(name).is_none() {
                    if let Some(sig) = crate::builtins::library(name) {
                        return Some(self.check_library_builtin(expr, sig));
                    }
                }
            }
        }
        None
    }

    /// The number of arguments `argc` against the `arity` fixed parameters
    /// of `name` (C11 6.5.2.2p2).
    fn check_call_arity(
        &self,
        name: &str,
        argc: usize,
        arity: usize,
        variadic: bool,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        if variadic {
            if argc < arity {
                return self.err(
                    line,
                    column,
                    format!(
                        "function '{name}' expects at least {} {}, found {argc}",
                        arity,
                        if arity == 1 { "argument" } else { "arguments" },
                    ),
                );
            }
        } else if argc != arity {
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
        Ok(())
    }

    /// Check each argument of a direct call and convert it as if by
    /// assignment to its parameter's type (C11 6.5.2.2p7).
    fn check_call_args(&mut self, args: &mut [Expr], params: &[CType]) -> Result<(), Diagnostic> {
        for (index, arg) in args.iter_mut().enumerate() {
            self.check_expr(arg)?;
            self.decay(arg)?;
            if index < params.len() {
                self.can_assign(&params[index], arg)?;
                self.require_passed_aggregate(&params[index], arg.line(), arg.column(), false)?;
            } else if arg.ty().is_some_and(|ty| ty.unqualified().is_record()) {
                let ty = arg.ty().expect("typed");
                self.require_passed_aggregate(&ty, arg.line(), arg.column(), false)?;
            } else {
                // Default argument promotions apply in the IR. There is no
                // parameter type to convert to; the argument must be a scalar.
                self.require_scalar(arg)?;
            }
        }
        Ok(())
    }

    /// A `__builtin_` spelling of a C library function: a call of that
    /// function with its ISO C prototype (GCC manual, "Other Built-in
    /// Functions"). The module declares the function if the program does
    /// not.
    pub(super) fn check_library_builtin(
        &mut self,
        expr: &mut Expr,
        sig: crate::builtins::LibSig,
    ) -> Result<(), Diagnostic> {
        let (line, column) = (expr.line(), expr.column());
        let Expr::Call { callee, args, .. } = expr else {
            unreachable!("library builtin");
        };
        let name = match callee.as_ref() {
            Expr::Var { name, .. } => name.clone(),
            _ => unreachable!("library builtin callee"),
        };
        let arity = sig.params.len();
        let argc = args.len();
        if (sig.variadic && argc < arity) || (!sig.variadic && argc != arity) {
            return self.err(
                line,
                column,
                format!(
                    "function '{name}' expects {}{arity} {}, found {argc}",
                    if sig.variadic { "at least " } else { "" },
                    if arity == 1 { "argument" } else { "arguments" },
                ),
            );
        }
        for (index, arg) in args.iter_mut().enumerate() {
            self.check_expr(arg)?;
            self.decay(arg)?;
            if index < arity {
                self.can_assign(&sig.params[index], arg)?;
            } else {
                self.require_scalar(arg)?;
            }
        }
        let ret = sig.ret.clone();
        if let Expr::Call { resolved, .. } = expr {
            *resolved = Some(CallRes {
                name: sig.name.clone(),
                ret: ret.clone(),
                params: sig.params.clone(),
                variadic: sig.variadic,
                indirect: false,
            });
        }
        self.library_calls.insert(sig.name.clone(), sig);
        expr.set_ty(ret);
        Ok(())
    }

    /// `__builtin___NAME_chk` (GCC manual, "Object Size Checking"): the
    /// call becomes `((void)extra, ..., __builtin_NAME(args))`, so the
    /// flag and object size are still evaluated, before the call (the order
    /// of argument evaluation is unspecified, C11 6.5.2.2p10), and no check
    /// is made. See [`crate::builtins::CheckedBuiltin`].
    pub(super) fn check_checked_builtin(
        &mut self,
        expr: &mut Expr,
        name: &str,
        checked: crate::builtins::CheckedBuiltin,
    ) -> Result<(), Diagnostic> {
        let (line, column) = (expr.line(), expr.column());
        let Expr::Call { callee, args, .. } = expr else {
            unreachable!("checked builtin");
        };
        let argc = args.len();
        let arity = checked.arity;
        if (checked.variadic && argc < arity) || (!checked.variadic && argc != arity) {
            return self.err(
                line,
                column,
                format!(
                    "{name} expects {}{arity} arguments, found {argc}",
                    if checked.variadic { "at least " } else { "" },
                ),
            );
        }
        let mut extra = Vec::with_capacity(checked.extra.len());
        for &index in checked.extra.iter().rev() {
            extra.push(args.remove(index));
        }
        extra.reverse();
        for arg in extra.iter_mut() {
            self.check_expr(arg)?;
            self.decay(arg)?;
            if !arg.ty().expect("typed").is_integer() {
                return self.err(
                    arg.line(),
                    arg.column(),
                    format!("{name} flag and object size must be integers"),
                );
            }
        }
        let (callee_line, callee_column) = (callee.line(), callee.column());
        **callee = Expr::Var {
            name: checked.plain.to_string(),
            line: callee_line,
            column: callee_column,
            resolved: None,
            ty: None,
        };
        self.check_call(expr)?;
        let ty = expr.ty();
        for arg in extra.into_iter().rev() {
            let call = std::mem::replace(
                expr,
                Expr::Int {
                    bits: 0,
                    value_ty: TypeName::Int,
                    line,
                    column,
                    ty: None,
                },
            );
            let discarded = Expr::Cast {
                ty_name: CType::Void,
                expr: Box::new(arg),
                line,
                column,
                ty: Some(CType::Void),
            };
            *expr = Expr::Comma {
                left: Box::new(discarded),
                right: Box::new(call),
                line,
                column,
                ty: ty.clone(),
            };
        }
        Ok(())
    }

    /// `__builtin_object_size (ptr, type)` and
    /// `__builtin_dynamic_object_size` (GCC manual, "Object Size
    /// Checking"). `ptr` is not evaluated, and `type` is an integer
    /// constant from 0 to 3. This compiler never determines an object's
    /// size, so the result is the manual's value for an unknown size:
    /// `(size_t)-1` for types 0 and 1, and 0 for types 2 and 3.
    pub(super) fn check_object_size(
        &mut self,
        expr: &mut Expr,
        name: &str,
    ) -> Result<(), Diagnostic> {
        let (line, column) = (expr.line(), expr.column());
        let Expr::Call { args, .. } = expr else {
            unreachable!("object size builtin");
        };
        if args.len() != 2 {
            return self.err(
                line,
                column,
                format!("{name} expects 2 arguments, found {}", args.len()),
            );
        }
        for arg in args.iter_mut() {
            self.check_expr(arg)?;
            self.decay(arg)?;
        }
        // The parameter is `const void *`, so a null pointer constant is
        // accepted too (C11 6.3.2.3p3).
        if !args[0].ty().expect("typed").is_pointer()
            && !super::expr::is_null_pointer_constant(&args[0])
        {
            return self.err(
                args[0].line(),
                args[0].column(),
                format!("{name} operand must be a pointer"),
            );
        }
        let kind = match eval_const_expr(&args[1]) {
            Ok(value) if value.ty.is_integer() => value.bits & value.ty.mask(),
            _ => {
                return self.err(
                    args[1].line(),
                    args[1].column(),
                    format!("{name} type must be an integer constant expression"),
                );
            }
        };
        if kind > 3 {
            return self.err(
                args[1].line(),
                args[1].column(),
                format!("{name} type must be from 0 to 3"),
            );
        }
        let size_t = TypeName::size_t();
        *expr = Expr::Int {
            bits: if kind < 2 { u64::MAX } else { 0 },
            value_ty: size_t,
            line,
            column,
            ty: Some(CType::Int(size_t)),
        };
        Ok(())
    }

    /// A builtin that IR lowering expands in place: bit counting, byte
    /// swapping, branch hints, `__builtin_trap`, `__builtin_alloca`, the
    /// floating classification builtins, and the overflow-checking
    /// arithmetic (GCC manual, "Other Built-in Functions" and "Built-in
    /// Functions to Perform Arithmetic with Overflow Checking").
    pub(super) fn check_inline_builtin(
        &mut self,
        expr: &mut Expr,
        name: &str,
        inline: crate::builtins::Inline,
    ) -> Result<(), Diagnostic> {
        use crate::builtins::Inline;
        let (line, column) = (expr.line(), expr.column());
        let Expr::Call { args, .. } = expr else {
            unreachable!("inline builtin");
        };
        let (min, max) = inline.arity();
        if args.len() < min || args.len() > max {
            let expected = if min == max {
                format!("{min}")
            } else {
                format!("{min} to {max}")
            };
            return self.err(
                line,
                column,
                format!(
                    "{name} expects {expected} {}, found {}",
                    if max == 1 { "argument" } else { "arguments" },
                    args.len()
                ),
            );
        }
        for (index, arg) in args.iter_mut().enumerate() {
            self.check_expr(arg)?;
            self.decay(arg)?;
            if let Some(param) = inline.param_type(index) {
                self.can_assign(&param, arg)?;
            }
        }
        match inline {
            Inline::IsNan | Inline::IsInf | Inline::IsFinite | Inline::IsNormal => {
                let ty = args[0].ty().expect("typed");
                if !ty.is_float() {
                    return self.err(
                        args[0].line(),
                        args[0].column(),
                        format!("{name} requires a floating-point argument"),
                    );
                }
            }
            Inline::ReturnAddress | Inline::FrameAddress => {
                if eval_const_expr(&args[0]).map_or(true, |value| !is_zero(&value)) {
                    return self.err(
                        args[0].line(),
                        args[0].column(),
                        format!("{name} supports only level 0"),
                    );
                }
            }
            Inline::Overflow { fixed: None, .. } => {
                for arg in &args[..2] {
                    if !arg.ty().is_some_and(|ty| ty.is_integer()) {
                        return self.err(
                            arg.line(),
                            arg.column(),
                            format!("operand of {name} must be an integer"),
                        );
                    }
                }
                let target = args[2].ty().expect("typed");
                let ok = target.pointee().is_some_and(|pointee| {
                    pointee.is_integer()
                        && !pointee.is_const()
                        && pointee.as_integer() != Some(TypeName::Bool)
                });
                if !ok {
                    return self.err(
                        args[2].line(),
                        args[2].column(),
                        format!(
                            "result argument of {name} must be a pointer to a non-const integer other than _Bool"
                        ),
                    );
                }
            }
            _ => {}
        }
        let ret = inline.result_type();
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

    /// `__builtin_signbit`, `__builtin_signbitf`, and `__builtin_signbitl`.
    ///
    /// The result is 1 when the sign bit of the floating value is set, and 0
    /// otherwise, including for NaN and negative zero. The argument is
    /// converted to the builtin’s floating type.
    pub(super) fn check_signbit(&mut self, expr: &mut Expr, name: &str) -> Result<(), Diagnostic> {
        let (line, column) = (expr.line(), expr.column());
        let Expr::Call { args, .. } = expr else {
            unreachable!("signbit");
        };
        if args.len() != 1 {
            return self.err(line, column, format!("{name} expects 1 argument"));
        }
        self.check_expr(&mut args[0])?;
        self.decay(&mut args[0])?;
        let ty = args[0].ty().expect("typed");
        if !ty.is_arithmetic() {
            return self.err(
                args[0].line(),
                args[0].column(),
                format!("{name} requires an arithmetic argument"),
            );
        }
        if let Expr::Call { resolved, .. } = expr {
            *resolved = Some(CallRes {
                name: name.to_string(),
                ret: CType::int_type(),
                params: Vec::new(),
                variadic: false,
                indirect: false,
            });
        }
        expr.set_ty(CType::int_type());
        Ok(())
    }

    /// `__builtin_isgreater`, `__builtin_isgreaterequal`,
    /// `__builtin_isless`, `__builtin_islessequal`,
    /// `__builtin_islessgreater`, and `__builtin_isunordered`: the C11
    /// 7.12.14 comparison macros as glibc `<math.h>` spells them (GCC
    /// "Other Builtins").
    ///
    /// Both operands are arithmetic and at least one is a real floating
    /// type; IR lowering converts both by the usual arithmetic
    /// conversions (6.3.1.8). The result is `int` 0 or 1.
    pub(super) fn check_fp_compare(
        &mut self,
        expr: &mut Expr,
        name: &str,
    ) -> Result<(), Diagnostic> {
        let (line, column) = (expr.line(), expr.column());
        let Expr::Call { args, .. } = expr else {
            unreachable!("comparison builtin");
        };
        if args.len() != 2 {
            return self.err(line, column, format!("{name} expects 2 arguments"));
        }
        for arg in args.iter_mut() {
            self.check_expr(arg)?;
            self.decay(arg)?;
        }
        let left = args[0].ty().expect("typed");
        let right = args[1].ty().expect("typed");
        if !left.is_arithmetic() || !right.is_arithmetic() || !(left.is_float() || right.is_float())
        {
            return self.err(
                line,
                column,
                format!(
                    "non-floating-point arguments in call to {name} ('{}' and '{}')",
                    left.display(),
                    right.display()
                ),
            );
        }
        if let Expr::Call { resolved, .. } = expr {
            *resolved = Some(CallRes {
                name: name.to_string(),
                ret: CType::int_type(),
                params: Vec::new(),
                variadic: false,
                indirect: false,
            });
        }
        expr.set_ty(CType::int_type());
        Ok(())
    }

    /// `__builtin_nan*` / `__builtin_nans*` / `__builtin_inf*` /
    /// `__builtin_huge_val*`.
    ///
    /// The result has the documented floating type and is a constant
    /// expression when the argument, if any, is a string literal that
    /// `strtol` would consume in full (GCC "Floating-Point Format
    /// Builtins"). A non-literal argument is a diagnostic.
    pub(super) fn check_fp_format_builtin(
        &mut self,
        expr: &mut Expr,
        name: &str,
    ) -> Result<(), Diagnostic> {
        let fp = crate::float::FpFormatBuiltin::from_name(name).expect("format builtin");
        let (line, column) = (expr.line(), expr.column());
        let Expr::Call { args, .. } = expr else {
            unreachable!("format builtin");
        };
        if args.len() != fp.arity() {
            return self.err(
                line,
                column,
                format!(
                    "{name} expects {} {}",
                    fp.arity(),
                    if fp.arity() == 1 {
                        "argument"
                    } else {
                        "arguments"
                    }
                ),
            );
        }
        if matches!(fp, crate::float::FpFormatBuiltin::Nan { .. })
            && !crate::float::FpFormatBuiltin::is_string_literal_arg(&args[0])
        {
            return self.err(
                args[0].line(),
                args[0].column(),
                format!("{name} requires a string literal argument"),
            );
        }
        for arg in args.iter_mut() {
            self.check_expr(arg)?;
            self.decay(arg)?;
        }
        // A malformed quiet tag is left to constant evaluation in an
        // ICE or static initializer, and to IR lowering in a runtime
        // context (a call of nan/nanf/nanl). __builtin_nans* has no
        // library equivalent, so a tag that is not consumed in full
        // is always a diagnostic.
        if let crate::float::FpFormatBuiltin::Nan {
            signaling: true, ..
        } = fp
        {
            if crate::float::nan_tag_from_expr(&args[0]).is_none() {
                return self.err(
                    args[0].line(),
                    args[0].column(),
                    format!("{name} requires a string literal that strtol would consume in full"),
                );
            }
        }
        // A malformed quiet tag in a function body that is evaluated
        // becomes a call of the C library function (IR lowering). A file
        // object with that name is checked once the whole unit is seen.
        if let Some(lib) = fp.libm_nan() {
            if self.unevaluated_depth == 0
                && !self.current_func.is_empty()
                && crate::float::nan_tag_from_expr(&args[0]).is_none()
            {
                self.library_nan_calls
                    .push((lib.to_string(), name.to_string(), line, column));
            }
        }
        let ret = CType::Float(fp.kind());
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

    /// The three C library memory builtins. They are not declared: a call
    /// names them directly. The result type is `void *`, and the value is
    /// the first argument.
    pub(super) fn check_memory_builtin(
        &mut self,
        expr: &mut Expr,
        name: &str,
    ) -> Result<(), Diagnostic> {
        let (line, column) = (expr.line(), expr.column());
        let Expr::Call { args, .. } = expr else {
            unreachable!("builtin call");
        };
        if args.len() != 3 {
            return self.err(line, column, format!("{name} expects 3 arguments"));
        }
        for arg in args.iter_mut() {
            self.check_expr(arg)?;
            self.decay(arg)?;
        }
        let dest = args[0].ty().expect("typed");
        let mid = args[1].ty().expect("typed");
        let len = args[2].ty().expect("typed");
        if !dest.is_pointer() {
            return self.err(
                args[0].line(),
                args[0].column(),
                format!("{name} destination must be a pointer"),
            );
        }
        if name == "__builtin_memset" {
            if !mid.is_integer() {
                return self.err(
                    args[1].line(),
                    args[1].column(),
                    format!("{name} value must be an integer"),
                );
            }
        } else if !mid.is_pointer() {
            return self.err(
                args[1].line(),
                args[1].column(),
                format!("{name} source must be a pointer"),
            );
        }
        if !len.is_integer() {
            return self.err(
                args[2].line(),
                args[2].column(),
                format!("{name} size must be an integer"),
            );
        }
        if let Expr::Call { resolved, .. } = expr {
            *resolved = Some(CallRes {
                name: name.to_string(),
                ret: CType::pointer(CType::Void),
                params: Vec::new(),
                variadic: false,
                indirect: false,
            });
        }
        expr.set_ty(CType::pointer(CType::Void));
        Ok(())
    }

    pub(super) fn check_indirect_call(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let (line, column) = (expr.line(), expr.column());
        let Expr::Call { callee, args, .. } = expr else {
            unreachable!("call");
        };
        self.check_expr(callee)?;
        self.decay(callee)?;
        let ty = callee.ty().expect("typed");
        let Some(func) = ty.pointee().and_then(CType::func_ty).cloned() else {
            return self.err(
                line,
                column,
                format!("called object of type '{}' is not a function", ty.display()),
            );
        };
        self.require_passed_aggregate(&func.ret, line, column, true)?;
        let argc = args.len();
        let arity = func.params.len();
        if func.variadic || !func.prototype {
            if argc < arity {
                return self.err(
                    line,
                    column,
                    format!(
                        "function expects at least {arity} {}, found {argc}",
                        if arity == 1 { "argument" } else { "arguments" },
                    ),
                );
            }
        } else if argc != arity {
            return self.err(
                line,
                column,
                format!(
                    "function expects {arity} {}, found {argc}",
                    if arity == 1 { "argument" } else { "arguments" },
                ),
            );
        }
        let params = func.params.clone();
        let ret = func.ret.clone();
        if let Expr::Call { args, .. } = expr {
            for (index, arg) in args.iter_mut().enumerate() {
                self.check_expr(arg)?;
                self.decay(arg)?;
                if index < params.len() {
                    self.can_assign(&params[index], arg)?;
                    self.require_passed_aggregate(&params[index], arg.line(), arg.column(), false)?;
                } else if arg.ty().is_some_and(|ty| ty.unqualified().is_record()) {
                    let ty = arg.ty().expect("typed");
                    self.require_passed_aggregate(&ty, arg.line(), arg.column(), false)?;
                } else {
                    self.require_scalar(arg)?;
                }
            }
        }
        if let Expr::Call { resolved, .. } = expr {
            *resolved = Some(CallRes {
                name: String::new(),
                ret: ret.clone(),
                params,
                variadic: func.variadic || (!func.prototype && !fixed_unprototyped_calls()),
                indirect: true,
            });
        }
        expr.set_ty(ret);
        Ok(())
    }

    pub(super) fn check_va_list_arg(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        if crate::target::current().va_list_kind() != crate::target::VaListKind::SysV {
            // The va_list is an object (`char *` on Apple arm64 and
            // Windows, the AAPCS64 structure on AArch64 Linux), not an
            // array, so the builtins take its address explicitly.
            self.check_expr(expr)?;
            let ty = expr.ty().expect("typed");
            if !(va_list_object(&ty) && expr.is_lvalue()) {
                return self.err(
                    expr.line(),
                    expr.column(),
                    format!("expected a va_list, found '{}'", ty.display()),
                );
            }
            let line = expr.line();
            let column = expr.column();
            let inner = std::mem::replace(expr, Expr::int(0, TypeName::Int, line, column));
            *expr = Expr::unary(UnaryOp::Addr, inner, line, column);
            expr.set_ty(CType::pointer(ty));
            return Ok(());
        }
        self.check_expr(expr)?;
        self.decay(expr)?;
        let ty = expr.ty().expect("typed");
        if va_list_pointee(&ty) {
            return Ok(());
        }
        self.err(
            expr.line(),
            expr.column(),
            format!("expected a va_list, found '{}'", ty.display()),
        )
    }

    pub(super) fn check_va_arg(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let (line, column, type_arg) = match expr {
            Expr::VaArg {
                ap,
                type_arg,
                line,
                column,
                ..
            } => {
                let line = *line;
                let column = *column;
                let type_arg = type_arg.clone();
                self.check_va_list_arg(ap)?;
                (line, column, type_arg)
            }
            _ => unreachable!("va_arg"),
        };
        if !type_arg.is_complete() || type_arg.is_array() || type_arg.is_func() {
            return self.err(
                line,
                column,
                format!("va_arg of type '{}' is not supported", type_arg.display()),
            );
        }
        self.require_passed_aggregate(&type_arg, line, column, false)?;
        expr.set_ty(type_arg);
        Ok(())
    }

    pub(super) fn check_va_op(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let (op, line, column) = match expr {
            Expr::VaOp {
                op, line, column, ..
            } => (*op, *line, *column),
            _ => unreachable!("va op"),
        };
        let Expr::VaOp { args, .. } = expr else {
            unreachable!("va op");
        };
        match op {
            VaOp::Start => {
                if !self.variadic_fn {
                    return self.err(
                        line,
                        column,
                        "va_start used in a function that is not variadic",
                    );
                }
                if args.len() != 2 {
                    return self.err(line, column, "va_start expects a va_list and a parameter");
                }
                self.check_va_list_arg(&mut args[0])?;
                self.check_expr(&mut args[1])?;
                let last = self.last_param.clone().unwrap_or_default();
                let names_last = matches!(&args[1], Expr::Var { name, .. } if name == &last);
                if !names_last {
                    return self.err(
                        line,
                        column,
                        "the second argument of va_start must be the last named parameter",
                    );
                }
            }
            VaOp::End => {
                if args.len() != 1 {
                    return self.err(line, column, "va_end expects a va_list");
                }
                self.check_va_list_arg(&mut args[0])?;
            }
            VaOp::Copy => {
                if args.len() != 2 {
                    return self.err(line, column, "va_copy expects two va_list arguments");
                }
                self.check_va_list_arg(&mut args[0])?;
                self.check_va_list_arg(&mut args[1])?;
            }
        }
        expr.set_ty(CType::Void);
        Ok(())
    }
}

/// A `va_list` object: `char *` on Apple arm64 and Windows, `struct
/// __va_list` on AArch64 Linux.
pub(super) fn va_list_object(ty: &CType) -> bool {
    let bare = ty.unqualified();
    if crate::target::current().va_list_kind() == crate::target::VaListKind::CharPtr {
        return bare
            .pointee()
            .is_some_and(|pointee| pointee.unqualified() == CType::from_name(TypeName::Char));
    }
    bare.struct_ref()
        .is_some_and(|handle| handle.tag() == "__va_list")
}

pub(super) fn va_list_pointee(ty: &CType) -> bool {
    ty.pointee()
        .and_then(CType::struct_ref)
        .is_some_and(|handle| handle.tag() == "__va_list_tag")
}
