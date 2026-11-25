//! Expressions: operands, operators, casts, `sizeof`, `_Generic`, compound
//! literals, subscripts, members and lvalues (C11 6.5).

use super::*;

impl Sema<'_> {
    pub(super) fn check_expr(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let implicit_func_name = if let Expr::Var {
            name, line, column, ..
        } = expr
        {
            if matches!(
                name.as_str(),
                "__func__" | "__PRETTY_FUNCTION__" | "__FUNCTION__"
            ) && self.lookup(name).is_none()
            {
                if self.fn_name.is_empty() {
                    return self.err(
                        *line,
                        *column,
                        format!("'{name}' is not allowed outside a function"),
                    );
                }
                let bytes = self.fn_name.as_bytes().to_vec();
                let line = *line;
                let column = *column;
                *expr = Expr::str_lit(bytes, StrEnc::Byte, line, column);
                true
            } else {
                false
            }
        } else {
            false
        };
        match expr {
            Expr::Int { value_ty, .. } => {
                let value_ty = *value_ty;
                expr.set_ty(CType::from_name(value_ty));
                Ok(())
            }
            Expr::Float { kind, .. } => {
                let kind = *kind;
                expr.set_ty(CType::Float(kind));
                Ok(())
            }
            Expr::Var { .. } => self.check_var(expr),
            Expr::Bin { .. } => self.check_bin(expr),
            Expr::Unary { .. } => self.check_unary(expr),
            Expr::Assign { .. } => self.check_assign(expr),
            Expr::CompoundAssign { .. } => self.check_compound_assign(expr),
            Expr::IncDec { .. } => self.check_incdec(expr),
            Expr::Cond { .. } => self.check_cond(expr),
            Expr::CondShort { .. } => self.check_cond_short(expr),
            Expr::StmtExpr { .. } => self.check_stmt_expr(expr),
            Expr::LabelAddr {
                name,
                func,
                line,
                column,
                ..
            } => {
                if self.current_symbol.is_empty() {
                    return self.err(
                        *line,
                        *column,
                        "a label address ('&&label') is not allowed outside a function",
                    );
                }
                if !self.labels.contains_key(name.as_str()) {
                    return self.err(*line, *column, self.missing_label(name));
                }
                // Record even inside `sizeof`: a VLA operand is evaluated
                // (C11 6.5.3.4p2) and `end_unevaluated` is skipped for it,
                // so the address must stay in `addr_labels`. A non-VLA
                // `sizeof` still rolls the list back.
                if !self.addr_labels.contains(name) {
                    self.addr_labels.push(name.clone());
                }
                *func = Some(self.current_symbol.clone());
                expr.set_ty(CType::pointer(CType::Void));
                Ok(())
            }
            Expr::Comma { .. } => self.check_comma(expr),
            Expr::Sizeof { .. } => self.check_sizeof(expr),
            Expr::Cast { .. } => self.check_cast(expr),
            Expr::Call { .. } => self.check_call(expr),
            Expr::Index { .. } => self.check_index(expr),
            Expr::Decay { .. } => Ok(()),
            Expr::Str { .. } => {
                self.check_str(expr)?;
                // C11 6.4.2.2: __func__ is `static const char [N]`, not an
                // unqualified string literal (6.4.5p6).
                if implicit_func_name {
                    let ty = expr
                        .ty()
                        .expect("string literal is typed")
                        .with_quals(Quals {
                            is_const: true,
                            ..Quals::default()
                        });
                    expr.set_ty(ty);
                }
                Ok(())
            }
            Expr::Member { .. } => self.check_member(expr),
            Expr::VaArg { .. } => self.check_va_arg(expr),
            Expr::VaOp { .. } => self.check_va_op(expr),
            Expr::Generic { .. } => self.check_generic(expr),
            Expr::CompoundLit { .. } => self.check_compound_lit(expr),
        }
    }

    /// A generic selection (C11 6.5.1.1). The controlling expression is not
    /// evaluated; its type after lvalue conversion (qualifiers dropped, an
    /// array or function becomes a pointer) picks the association whose type
    /// is compatible with it, or `default`. Every association expression is
    /// analyzed for constraints and name resolution. The node is then
    /// replaced by the chosen expression, which keeps its type and whether
    /// it is an lvalue, so unselected expressions are not evaluated and
    /// emit no IR.
    pub(super) fn check_generic(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let Expr::Generic {
            control,
            assocs,
            line,
            column,
            ..
        } = expr
        else {
            unreachable!("generic selection");
        };
        let (line, column) = (*line, *column);
        let mut assocs = std::mem::take(assocs);
        // The controlling expression is not evaluated (C11 6.5.1.1p3): it
        // is checked for its type and constraints, then nothing it recorded
        // for IR lowering is kept.
        let saved = self.begin_unevaluated();
        let checked = self.check_expr(control);
        self.end_unevaluated(saved);
        checked?;
        let raw = control.ty().expect("typed");
        let ty = match raw.peel() {
            CType::Array(elem, _) => CType::pointer((**elem).clone()),
            CType::Func(_) => CType::pointer(raw.clone()),
            _ => raw.unqualified(),
        };
        let mut default = None;
        let mut chosen = None;
        for (index, assoc) in assocs.iter().enumerate() {
            let Some(assoc_ty) = &assoc.ty else {
                if default.is_some() {
                    return self.err(
                        assoc.line,
                        assoc.column,
                        "duplicate default generic association",
                    );
                }
                default = Some(index);
                continue;
            };
            if assoc_ty.is_func() || !assoc_ty.is_complete() || assoc_ty.is_variably_modified() {
                return self.err(
                    assoc.line,
                    assoc.column,
                    format!(
                        "type '{}' in generic association is not a complete object type other than a variably modified type",
                        assoc_ty.display()
                    ),
                );
            }
            for earlier in assocs[..index].iter().filter_map(|a| a.ty.as_ref()) {
                if compatible(earlier, assoc_ty) {
                    return self.err(
                        assoc.line,
                        assoc.column,
                        format!(
                            "type '{}' in generic association compatible with previously specified type '{}'",
                            assoc_ty.display(),
                            earlier.display()
                        ),
                    );
                }
            }
            if chosen.is_none() && compatible(assoc_ty, &ty) {
                chosen = Some(index);
            }
        }
        let Some(index) = chosen.or(default) else {
            return self.err(
                line,
                column,
                format!(
                    "controlling expression type '{}' not compatible with any generic association type",
                    ty.display()
                ),
            );
        };
        for (i, assoc) in assocs.iter_mut().enumerate() {
            if i != index {
                // Constraints are checked, but unselected associations are
                // not evaluated and must not leave IR artifacts behind
                // (C11 6.5.1.1p3).
                let saved = self.begin_unevaluated();
                let checked = self.check_expr(&mut assoc.expr);
                self.end_unevaluated(saved);
                checked?;
            }
        }
        *expr = assocs.swap_remove(index).expr;
        self.check_expr(expr)
    }

    /// A compound literal (C11 6.5.2.5). The type is a complete object type
    /// or an array of unknown size, completed by the initializer. Inside a
    /// function the object is automatic and initialized where the literal is
    /// evaluated. Outside one it has static storage, the initializer must be
    /// constant, and the node becomes a reference to a private global.
    pub(super) fn check_compound_lit(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let Expr::CompoundLit {
            decl, line, column, ..
        } = expr
        else {
            unreachable!("compound literal");
        };
        let (line, column) = (*line, *column);
        if decl.ty.is_func() {
            return self.err(line, column, "compound literal has function type");
        }
        if decl.ty.is_vla() {
            return self.err(
                line,
                column,
                "a variable length array cannot be a compound literal type",
            );
        }
        self.check_vla_sizes(&decl.ty, line, column)?;
        self.check_atomic_types(&decl.ty, line, column)?;
        let unknown_array = matches!(decl.ty.peel(), CType::Array(_, ArrayLen::Incomplete));
        if !decl.ty.is_complete() && !unknown_array {
            return self.err(
                line,
                column,
                format!(
                    "compound literal has incomplete type '{}'",
                    decl.ty.display()
                ),
            );
        }
        let init = decl.init.clone().expect("compound literal initializer");
        let (ty, mut stores, mut overridden) =
            self.shape_initializer(&decl.ty, &init, line, column)?;
        decl.ty = ty.clone();
        self.check_stores(&mut stores)?;
        self.check_stores(&mut overridden)?;
        if !self.fn_name.is_empty() {
            decl.checked = Some(classify_init(&ty, stores));
            expr.set_ty(ty);
            return Ok(());
        }
        let value = self.fold_stores("compound literal", &ty, &stores)?;
        self.fold_overridden("compound literal", &overridden)?;
        self.static_seq += 1;
        let symbol = format!(".compoundliteral.{}", self.static_seq);
        self.static_locals.push(IrGlobal {
            name: symbol.clone(),
            ty: ty.clone(),
            linkage: Linkage::Internal,
            init: Some(value),
            attrs: VarAttrs::default(),
        });
        *expr = Expr::Var {
            name: String::new(),
            line,
            column,
            resolved: Some(VarRes {
                slot: format!("@{symbol}"),
            }),
            ty: Some(ty),
        };
        Ok(())
    }

    /// A string literal has type `char[N]` or `wchar_t[N]`, counting the
    /// terminating NUL. The private global is created when the address is
    /// used, not for `sizeof` and not when the literal only initializes an
    /// array.
    pub(super) fn check_str(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let (len, elem) = match expr {
            Expr::Str { bytes, enc, .. } => {
                let unit = enc.unit() as u64;
                (bytes.len() as u64 / unit + 1, enc.elem())
            }
            _ => unreachable!("string literal"),
        };
        expr.set_ty(CType::array(CType::from_name(elem), Some(len)));
        Ok(())
    }

    /// Record a string literal the first time its address is needed. Identical
    /// contents share one private global.
    pub(super) fn ensure_string(&mut self, expr: &mut Expr) {
        let (bytes, enc) = match expr {
            Expr::Str {
                symbol: None,
                bytes,
                enc,
                ..
            } => (bytes.clone(), *enc),
            _ => return,
        };
        let symbol = self.intern_string(&bytes, enc);
        if let Expr::Str { symbol: slot, .. } = expr {
            *slot = Some(symbol);
        }
    }

    pub(super) fn intern_string(&mut self, bytes: &[u8], enc: StrEnc) -> String {
        let mut key = bytes.to_vec();
        key.extend(std::iter::repeat(0u8).take(enc.unit()));
        if let Some((_, name, _)) = self
            .strings
            .iter()
            .find(|(stored, _, stored_enc)| stored == &key && *stored_enc == enc)
        {
            return name.clone();
        }
        let name = if self.strings.is_empty() {
            ".str".to_string()
        } else {
            format!(".str.{}", self.strings.len())
        };
        self.strings.push((key, name.clone(), enc));
        name
    }

    /// Array expressions become pointers except as the operand of `sizeof` or `&`.
    ///
    /// An lvalue of incomplete type, other than an array, is not converted to
    /// a value (C11 6.3.2.1p2). A void expression is not an object lvalue, so
    /// a discarded call that returns `void` stays allowed. `&*p` does not call
    /// this on the dereference, so a pointer to an incomplete struct may still
    /// be the operand of `&`.
    pub(super) fn decay(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        self.ensure_string(expr);
        let Some(ty) = expr.ty() else {
            return Ok(());
        };
        if let CType::Array(elem, _) = ty.clone() {
            let line = expr.line();
            let column = expr.column();
            let inner = std::mem::replace(expr, Expr::int(0, TypeName::Int, line, column));
            *expr = Expr::Decay {
                expr: Box::new(inner),
                line,
                column,
                ty: Some(CType::pointer(*elem)),
            };
            return Ok(());
        }
        if ty.is_func() {
            let line = expr.line();
            let column = expr.column();
            let ptr = CType::pointer(ty);
            let inner = std::mem::replace(expr, Expr::int(0, TypeName::Int, line, column));
            *expr = Expr::Decay {
                expr: Box::new(inner),
                line,
                column,
                ty: Some(ptr),
            };
            return Ok(());
        }
        if !ty.is_void() && !ty.is_complete() {
            return self.err(
                expr.line(),
                expr.column(),
                format!("cannot use incomplete type '{}' as a value", ty.display()),
            );
        }
        Ok(())
    }

    pub(super) fn check_var(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let (name, line, column) = match expr {
            Expr::Var {
                name, line, column, ..
            } => (name.clone(), *line, *column),
            _ => unreachable!("check_var"),
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
        let (slot, ty) = match sym {
            Symbol::Var(var) => (var.slot, var.ty),
            Symbol::Func(func) => {
                // A function used as a value is a use. A static function that
                // is never defined must still be diagnosed, and the IR must
                // not name it without a declaration.
                self.note_use(&name, line, column);
                (
                    format!("@{}", func.symbol),
                    CType::func_with(func.ret, func.params, func.variadic, func.prototype),
                )
            }
        };
        if let Expr::Var { resolved, .. } = expr {
            *resolved = Some(VarRes { slot });
        }
        expr.set_ty(ty);
        Ok(())
    }

    pub(super) fn check_bin(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let (op, line, column) = match expr {
            Expr::Bin {
                op, line, column, ..
            } => (*op, *line, *column),
            _ => unreachable!("binary expression"),
        };
        let ty = if let Expr::Bin { left, right, .. } = expr {
            self.check_expr(left)?;
            self.check_expr(right)?;
            self.decay(left)?;
            self.decay(right)?;
            if matches!(op, BinOp::And | BinOp::Or) {
                self.require_scalar(left)?;
                self.require_scalar(right)?;
                CType::int_type()
            } else if op.is_cmp() {
                self.check_compare(op, left, right, line, column)?
            } else if matches!(op, BinOp::Add | BinOp::Sub) {
                self.check_additive(op, left, right, line, column)?
            } else if matches!(
                op,
                BinOp::Mod | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr
            ) {
                let left_ty = self.require_integer(left)?;
                let right_ty = self.require_integer(right)?;
                let result = if matches!(op, BinOp::Shl | BinOp::Shr) {
                    left_ty.promote()
                } else {
                    left_ty.usual(right_ty)
                };
                CType::from_name(result)
            } else {
                self.arith_type(op, left, right, line, column)?
            }
        } else {
            unreachable!("binary expression");
        };
        expr.set_ty(ty);
        Ok(())
    }

    /// `*` and `/` on arithmetic types. One floating operand selects the
    /// usual real type (C11 6.3.1.8): `double` beats `float`, and either
    /// beats every integer. `%` and the bitwise operators stay on the
    /// integer path.
    pub(super) fn arith_type(
        &self,
        op: BinOp,
        left: &Expr,
        right: &Expr,
        line: usize,
        column: usize,
    ) -> Result<CType, Diagnostic> {
        let left_ty = left.ty().expect("typed");
        let right_ty = right.ty().expect("typed");
        if let Some(common) = self.float_arith(&left_ty, &right_ty) {
            return Ok(common);
        }
        if let (Some(left), Some(right)) = (left_ty.as_integer(), right_ty.as_integer()) {
            return Ok(CType::from_name(left.usual(right)));
        }
        self.err(
            line,
            column,
            format!(
                "invalid operands to '{}' ({} and {})",
                op.as_str(),
                left_ty.display(),
                right_ty.display()
            ),
        )
    }

    /// `Some` when at least one operand is floating and both are arithmetic.
    /// `None` when neither is floating, so the caller keeps the integer or
    /// pointer rule. `long double` wins the usual arithmetic conversions.
    pub(super) fn float_arith(&self, left: &CType, right: &CType) -> Option<CType> {
        if !(left.is_float() || right.is_float()) {
            return None;
        }
        if left.is_arithmetic() && right.is_arithmetic() {
            return Some(CType::Float(crate::float::usual_real(
                left.float_kind(),
                right.float_kind(),
            )));
        }
        None
    }

    pub(super) fn check_compare(
        &self,
        op: BinOp,
        left: &Expr,
        right: &Expr,
        line: usize,
        column: usize,
    ) -> Result<CType, Diagnostic> {
        let left_ty = left.ty().expect("typed");
        let right_ty = right.ty().expect("typed");
        if (left_ty.is_float() || right_ty.is_float())
            && left_ty.is_arithmetic()
            && right_ty.is_arithmetic()
        {
            return Ok(CType::int_type());
        }
        if left_ty.is_integer() && right_ty.is_integer() {
            return Ok(CType::int_type());
        }
        let equality = matches!(op, BinOp::Eq | BinOp::Ne);
        let function_pointer = left_ty.pointee().is_some_and(|ty| ty.is_func())
            || right_ty.pointee().is_some_and(|ty| ty.is_func());
        if function_pointer && !equality {
            return self.err(
                line,
                column,
                "relational comparison of function pointers is not allowed",
            );
        }
        let pointers_ok = if equality {
            loose_pointers(&left_ty, &right_ty)
                || void_ptr_mix(&left_ty, &right_ty)
                || (left_ty.is_pointer() && is_null_pointer_constant(right))
                || (right_ty.is_pointer() && is_null_pointer_constant(left))
        } else {
            // Relational operators require compatible object pointers. A null
            // pointer constant is not included, and `void *` is not compatible
            // with `int *`. Clang warns for those and still compiles; this
            // compiler rejects them.
            left_ty.is_pointer() && right_ty.is_pointer() && loose_pointers(&left_ty, &right_ty)
        };
        if pointers_ok {
            return Ok(CType::int_type());
        }
        if equality && self.dialect.gnu() && void_function_ptr_mix(&left_ty, &right_ty) {
            self.warn(
                line,
                column,
                format!(
                    "'{}' compares a void pointer with a function pointer ({} and {}); \
                     C11 6.5.9p2 does not allow this; accepted as a GNU extension (-std=gnu11)",
                    op.as_str(),
                    left_ty.display(),
                    right_ty.display()
                ),
            );
            return Ok(CType::int_type());
        }
        if self.dialect.gnu() && gnu_distinct_pointers(&left_ty, &right_ty) {
            self.warn(
                line,
                column,
                format!(
                    "'{}' compares distinct pointer types ({} and {}); \
                     C11 {} does not allow this; accepted as a GNU extension (-std=gnu11)",
                    op.as_str(),
                    left_ty.display(),
                    right_ty.display(),
                    if equality { "6.5.9p2" } else { "6.5.8p2" }
                ),
            );
            return Ok(CType::int_type());
        }
        self.err(
            line,
            column,
            format!(
                "invalid operands to '{}' ({} and {})",
                op.as_str(),
                left_ty.display(),
                right_ty.display()
            ),
        )
    }

    pub(super) fn check_additive(
        &self,
        op: BinOp,
        left: &Expr,
        right: &Expr,
        line: usize,
        column: usize,
    ) -> Result<CType, Diagnostic> {
        let left_ty = left.ty().expect("typed");
        let right_ty = right.ty().expect("typed");
        if let Some(common) = self.float_arith(&left_ty, &right_ty) {
            return Ok(common);
        }
        if let (Some(left), Some(right)) = (left_ty.as_integer(), right_ty.as_integer()) {
            return Ok(CType::from_name(left.usual(right)));
        }
        if op == BinOp::Add {
            if let Some(ptr) = pointer_plus_int(&left_ty, &right_ty) {
                self.require_complete_pointee(&ptr, line, column)?;
                return Ok(ptr);
            }
        }
        if op == BinOp::Sub && left_ty.is_pointer() && right_ty.is_integer() {
            self.require_complete_pointee(&left_ty, line, column)?;
            return Ok(left_ty);
        }
        if op == BinOp::Sub && left_ty.is_pointer() && right_ty.is_pointer() {
            // GNU label addresses (`&&a - &&b`, used for compact dispatch
            // tables) are `void *`; their difference is not implemented.
            if matches!(left, Expr::LabelAddr { .. }) || matches!(right, Expr::LabelAddr { .. }) {
                return self.err(
                    line,
                    column,
                    "differences between label addresses are not supported",
                );
            }
            if !loose_pointers(&left_ty, &right_ty) {
                return self.err(
                    line,
                    column,
                    format!(
                        "invalid operands to '-' ({} and {})",
                        left_ty.display(),
                        right_ty.display()
                    ),
                );
            }
            self.require_complete_pointee(&left_ty, line, column)?;
            return Ok(CType::from_name(TypeName::ptrdiff_t()));
        }
        self.err(
            line,
            column,
            format!(
                "invalid operands to '{}' ({} and {})",
                op.as_str(),
                left_ty.display(),
                right_ty.display()
            ),
        )
    }

    pub(super) fn require_complete_pointee(
        &self,
        ptr: &CType,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        let Some(pointee) = ptr.pointee() else {
            return self.err(line, column, "pointer arithmetic requires a pointer");
        };
        if pointee.is_void() {
            return self.err(
                line,
                column,
                "arithmetic on a pointer to void is not supported",
            );
        }
        if !pointee.is_complete() {
            return self.err(
                line,
                column,
                format!(
                    "arithmetic on a pointer to incomplete type '{}'",
                    pointee.display()
                ),
            );
        }
        Ok(())
    }

    pub(super) fn check_unary(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let ty = if let Expr::Unary {
            op,
            expr,
            line,
            column,
            ..
        } = expr
        {
            let op = *op;
            let line = *line;
            let column = *column;
            self.check_expr(expr)?;
            self.ensure_string(expr);
            match op {
                UnaryOp::Addr => {
                    let inner_ty = expr.ty().expect("typed");
                    if inner_ty.is_func() {
                        CType::pointer(inner_ty)
                    } else if !expr.is_lvalue() {
                        return self.err(
                            line,
                            column,
                            "cannot take the address of this expression",
                        );
                    } else if is_bitfield(expr) {
                        return self.err(line, column, "cannot take the address of a bit-field");
                    } else {
                        CType::pointer(inner_ty)
                    }
                }
                UnaryOp::Deref => {
                    self.decay(expr)?;
                    let ty = expr.ty().expect("typed");
                    let Some(pointee) = ty.pointee().cloned() else {
                        return self.err(
                            line,
                            column,
                            format!("indirection requires a pointer, found '{}'", ty.display()),
                        );
                    };
                    pointee
                }
                UnaryOp::Not => {
                    self.decay(expr)?;
                    self.require_scalar(expr)?;
                    CType::int_type()
                }
                UnaryOp::BitNot => {
                    self.decay(expr)?;
                    CType::from_name(self.require_integer(expr)?.promote())
                }
                UnaryOp::Plus | UnaryOp::Minus => {
                    self.decay(expr)?;
                    let src = expr.ty().expect("typed");
                    if let Some(kind) = src.float_kind() {
                        // Unary `+` and `-` do not promote a floating operand.
                        CType::Float(kind)
                    } else {
                        CType::from_name(self.require_integer(expr)?.promote())
                    }
                }
            }
        } else {
            unreachable!("unary expression");
        };
        expr.set_ty(ty);
        Ok(())
    }

    pub(super) fn check_assign(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let ty = if let Expr::Assign { target, value, .. } = expr {
            self.check_modifiable(target)?;
            self.check_expr(value)?;
            self.decay(value)?;
            let dest = target.ty().expect("lvalue type");
            self.can_assign(&dest, value)?;
            dest.unqualified()
        } else {
            unreachable!("assignment");
        };
        expr.set_ty(ty);
        Ok(())
    }

    pub(super) fn check_compound_assign(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let ty = if let Expr::CompoundAssign {
            op,
            target,
            value,
            line,
            column,
            ..
        } = expr
        {
            let op = *op;
            let line = *line;
            let column = *column;
            self.check_modifiable(target)?;
            self.check_expr(value)?;
            self.decay(value)?;
            let dest = target.ty().expect("lvalue type");
            if !dest.is_scalar() {
                return self.err(
                    line,
                    column,
                    format!(
                        "compound assignment requires a scalar, found '{}'",
                        dest.display()
                    ),
                );
            }
            if dest.is_pointer() {
                if !matches!(op, BinOp::Add | BinOp::Sub) {
                    return self.err(
                        line,
                        column,
                        format!(
                            "pointer compound assignment does not allow '{}'",
                            op.compound_spelling()
                        ),
                    );
                }
                self.require_integer(value)?;
                self.require_complete_pointee(&dest, line, column)?;
            } else if dest.is_float() || value.ty().is_some_and(|ty| ty.is_float()) {
                if !matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div) {
                    return self.err(
                        line,
                        column,
                        format!(
                            "floating compound assignment does not allow '{}'",
                            op.compound_spelling()
                        ),
                    );
                }
                let rhs = value.ty().expect("typed");
                if !dest.is_arithmetic() || !rhs.is_arithmetic() {
                    return self.err(
                        line,
                        column,
                        format!(
                            "invalid operands to '{}' ({} and {})",
                            op.compound_spelling(),
                            dest.display(),
                            rhs.display()
                        ),
                    );
                }
            } else {
                self.require_integer(value)?;
            }
            dest.unqualified()
        } else {
            unreachable!("compound assignment");
        };
        expr.set_ty(ty);
        Ok(())
    }

    pub(super) fn check_incdec(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let ty = if let Expr::IncDec {
            target,
            line,
            column,
            ..
        } = expr
        {
            let line = *line;
            let column = *column;
            self.check_modifiable(target)?;
            let dest = target.ty().expect("lvalue type");
            if !dest.is_scalar() {
                return self.err(
                    line,
                    column,
                    format!("++ and -- require a scalar, found '{}'", dest.display()),
                );
            }
            if dest.is_pointer() {
                self.require_complete_pointee(&dest, line, column)?;
            }
            dest.unqualified()
        } else {
            unreachable!("increment");
        };
        expr.set_ty(ty);
        Ok(())
    }

    /// Arithmetic arms use the usual arithmetic conversions. Both arms may be
    /// `void`. Only the chosen arm is evaluated, which constant folding honors.
    pub(super) fn check_cond(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let ty = if let Expr::Cond {
            cond,
            then_expr,
            else_expr,
            line,
            column,
            ..
        } = expr
        {
            let line = *line;
            let column = *column;
            self.check_expr(cond)?;
            self.decay(cond)?;
            self.require_scalar(cond)?;
            self.check_expr(then_expr)?;
            self.check_expr(else_expr)?;
            self.decay(then_expr)?;
            self.decay(else_expr)?;
            self.cond_type(then_expr, else_expr, line, column)?
        } else {
            unreachable!("conditional expression");
        };
        expr.set_ty(ty);
        Ok(())
    }

    /// `x ?: y` (GCC manual, "Conditionals with Omitted Operands"): the
    /// type is that of `x ? x : y`, and `x` is evaluated once.
    pub(super) fn check_cond_short(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let Expr::CondShort {
            cond,
            else_expr,
            line,
            column,
            ..
        } = expr
        else {
            unreachable!("conditional with omitted operand");
        };
        let (line, column) = (*line, *column);
        self.check_expr(cond)?;
        self.decay(cond)?;
        self.require_scalar(cond)?;
        self.check_expr(else_expr)?;
        self.decay(else_expr)?;
        let ty = self.cond_type(cond, else_expr, line, column)?;
        expr.set_ty(ty);
        Ok(())
    }

    /// `({ ... })` (GCC manual, "Statements and Declarations in
    /// Expressions"). The block is checked in its own scope. The value is
    /// that of the last expression statement after lvalue conversion; a
    /// block that ends in anything else gives `void`.
    pub(super) fn check_stmt_expr(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let Expr::StmtExpr {
            body, line, column, ..
        } = expr
        else {
            unreachable!("statement expression");
        };
        if self.fn_name.is_empty() {
            return self.err(
                *line,
                *column,
                "statement expression not allowed outside a function",
            );
        }
        // Automatic objects in the block get their own slot names; the
        // function's numbering did not reach into expressions.
        self.stmt_expr_seq += 1;
        let mut next = 0;
        number_compound_in(body, &mut next, &format!("%se{}.", self.stmt_expr_seq));
        self.collect_labels(body)?;
        let switches = std::mem::take(&mut self.switches);
        let switch_outside = self.switch_outside;
        self.switch_outside |= !switches.is_empty();
        let checked = self.check_compound(body, true);
        self.switches = switches;
        self.switch_outside = switch_outside;
        checked?;
        let ty = match body.items.last() {
            Some(Stmt::Expr {
                expr: Some(last), ..
            }) => last.ty().expect("typed").unqualified(),
            _ => CType::Void,
        };
        expr.set_ty(ty);
        Ok(())
    }

    /// The left operand is evaluated and discarded. The result has the type
    /// of the right operand. There is a sequence point between the two.
    pub(super) fn check_comma(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let ty = if let Expr::Comma { left, right, .. } = expr {
            self.check_expr(left)?;
            self.decay(left)?;
            self.check_expr(right)?;
            self.decay(right)?;
            right.ty().expect("comma operand is typed")
        } else {
            unreachable!("comma expression");
        };
        expr.set_ty(ty);
        Ok(())
    }

    /// `sizeof` does not evaluate its operand unless the type is a VLA
    /// (C11 6.5.3.4p2). The result type is `unsigned long`, which is
    /// `size_t` on this LP64 target. `void` is incomplete.
    pub(super) fn check_sizeof(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        if let Expr::Sizeof {
            operand,
            line,
            column,
            ..
        } = expr
        {
            let line = *line;
            let column = *column;
            let sized = match operand {
                SizeofOperand::Type(ty) => {
                    self.check_vla_sizes(ty, line, column)?;
                    self.check_atomic_types_allowing(ty, line, column, true)?;
                    ty.clone()
                }
                SizeofOperand::Expr(inner) => {
                    // C11 6.5.3.4p2: the operand is evaluated only when its
                    // type is a VLA. Otherwise only the type is used.
                    let saved = self.begin_unevaluated();
                    let checked = self.check_expr(inner);
                    if checked.is_err()
                        || is_bitfield(inner)
                        || !inner.ty().is_some_and(|ty| ty.is_vla())
                    {
                        self.end_unevaluated(saved);
                    } else {
                        // C11 6.5.3.4p2: a VLA operand is evaluated, so
                        // string literals and other recordings stay.
                        self.unevaluated_depth -= 1;
                    }
                    checked?;
                    if is_bitfield(inner) {
                        return self.err(line, column, "sizeof applied to a bit-field");
                    }
                    inner.ty().expect("sizeof operand is typed")
                }
            };
            if !sized.is_complete() {
                return self.err(
                    line,
                    column,
                    format!("sizeof applied to incomplete type '{}'", sized.display()),
                );
            }
        }
        expr.set_ty(CType::from_name(TypeName::size_t()));
        Ok(())
    }

    pub(super) fn check_cast(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let ty_name = if let Expr::Cast {
            ty_name,
            expr: inner,
            line,
            column,
            ..
        } = expr
        {
            let ty_name = ty_name.clone();
            let line = *line;
            let column = *column;
            self.check_vla_sizes(&ty_name, line, column)?;
            self.check_atomic_types_allowing(&ty_name, line, column, true)?;
            self.check_expr(inner)?;
            self.decay(inner)?;
            if ty_name.is_array() {
                return self.err(line, column, "cast to an array type is not supported");
            }
            if ty_name.is_incomplete_enum() {
                return self.err(
                    line,
                    column,
                    format!("cast to incomplete type '{}'", ty_name.display()),
                );
            }
            if !ty_name.is_void() {
                let src = inner.ty().expect("typed");
                // C11 6.5.4: an arithmetic type converts to any arithmetic
                // type. A pointer converts to an integer and the reverse.
                // There is no conversion between a pointer and a floating type.
                // GNU C also casts a struct or union to its own type (a
                // no-op that yields the value; GCC and Clang accept it with
                // -std=gnu11 and diagnose it in strict ISO C).
                let same_record = self.dialect.gnu()
                    && ty_name.is_record()
                    && src.is_record()
                    && !ty_name.is_atomic()
                    && !src.is_atomic()
                    && crate::ast::compatible(&ty_name.unqualified(), &src.unqualified());
                let ok = (ty_name.is_arithmetic() && src.is_arithmetic())
                    || (ty_name.is_pointer() && (src.is_pointer() || src.is_integer()))
                    || (ty_name.is_integer() && src.is_pointer())
                    || same_record;
                if !ok {
                    return self.err(
                        line,
                        column,
                        format!(
                            "cast from '{}' to '{}' is not supported",
                            src.display(),
                            ty_name.display()
                        ),
                    );
                }
            }
            ty_name
        } else {
            unreachable!("cast");
        };
        expr.set_ty(ty_name);
        Ok(())
    }

    pub(super) fn check_index(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let ty = if let Expr::Index {
            base,
            index,
            line,
            column,
            ..
        } = expr
        {
            let line = *line;
            let column = *column;
            self.check_expr(base)?;
            self.check_expr(index)?;
            self.decay(base)?;
            self.decay(index)?;
            let base_ty = base.ty().expect("typed");
            let index_ty = index.ty().expect("typed");
            let ptr = if base_ty.is_pointer() && index_ty.is_integer() {
                base_ty
            } else if index_ty.is_pointer() && base_ty.is_integer() {
                index_ty
            } else {
                return self.err(line, column, "subscripted value is not an array or pointer");
            };
            let pointee = ptr.pointee().expect("pointer").clone();
            if pointee.is_void() {
                return self.err(
                    line,
                    column,
                    "subscript of a pointer to void is not supported",
                );
            }
            if !pointee.is_complete() {
                return self.err(
                    line,
                    column,
                    format!(
                        "subscript of a pointer to incomplete type '{}'",
                        pointee.display()
                    ),
                );
            }
            pointee
        } else {
            unreachable!("index");
        };
        expr.set_ty(ty);
        Ok(())
    }

    pub(super) fn check_modifiable(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        self.prepare_lvalue(expr)?;
        let ty = expr.ty().expect("lvalue type");
        if ty.is_array() {
            return self.err(expr.line(), expr.column(), "array type is not assignable");
        }
        if !ty.is_complete() {
            return self.err(
                expr.line(),
                expr.column(),
                format!("cannot assign to incomplete type '{}'", ty.display()),
            );
        }
        if ty.is_const() {
            return self.err(
                expr.line(),
                expr.column(),
                format!("cannot assign to const-qualified type '{}'", ty.display()),
            );
        }
        if ty.is_record() {
            if ty.contains_const() {
                return self.err(
                    expr.line(),
                    expr.column(),
                    "cannot assign to a struct with a const member",
                );
            }
            return Ok(());
        }
        if !ty.is_scalar() {
            return self.err(expr.line(), expr.column(), "expression is not assignable");
        }
        Ok(())
    }

    pub(super) fn check_member(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        let ty = if let Expr::Member {
            base,
            field,
            arrow,
            line,
            column,
            index,
            offset,
            bytewise,
            bit_width,
            bit_signed,
            container_bits,
            abs_bit,
            bit_declared,
            ..
        } = expr
        {
            let field_name = field.clone();
            let arrow = *arrow;
            let line = *line;
            let column = *column;
            self.check_expr(base)?;
            let base_ty = if arrow {
                self.decay(base)?;
                let ty = base.ty().expect("typed");
                let Some(pointee) = ty.pointee().cloned() else {
                    return self.err(
                        line,
                        column,
                        format!("member reference type '{}' is not a pointer", ty.display()),
                    );
                };
                pointee
            } else {
                base.ty().expect("typed")
            };
            let Some(handle) = base_ty.struct_ref().cloned() else {
                return self.err(
                    line,
                    column,
                    format!(
                        "member reference base type '{}' is not a struct",
                        base_ty.display()
                    ),
                );
            };
            if !handle.is_complete() {
                return self.err(
                    line,
                    column,
                    format!("member access on incomplete type 'struct {}'", handle.tag()),
                );
            }
            let Some(hit) = handle.lookup_member(&field_name) else {
                return self.err(
                    line,
                    column,
                    format!(
                        "no member named '{field_name}' in 'struct {}'",
                        handle.tag()
                    ),
                );
            };
            let found = hit.field;
            *index = Some(hit.index);
            *offset = Some(found.offset);
            // An anonymous member's fields are not one LLVM index of the
            // outer object. A byte offset from the outer base reaches them.
            *bytewise = hit.indirect || handle.is_union() || handle.has_bitfield();
            // The struct object's qualifiers apply to the member (C11 6.5.2.3).
            let field_ty = found.ty.clone();
            let declared_int = field_ty.as_integer();
            let mut member_ty = field_ty.with_quals(base_ty.quals());
            if let Some(width) = found.bit_width {
                let declared = declared_int.expect("a bit-field has an integer type");
                *bit_width = Some(width);
                *bit_signed = declared.is_signed();
                *bit_declared = declared;
                *container_bits = (found.ty.size().unwrap_or(1) * 8) as u32;
                *abs_bit = found.bit_offset;
                member_ty = CType::from_name(bitfield_value_type(declared, width))
                    .with_quals(member_ty.quals());
            }
            member_ty
        } else {
            unreachable!("member expression");
        };
        expr.set_ty(ty);
        Ok(())
    }

    pub(super) fn prepare_lvalue(&mut self, expr: &mut Expr) -> Result<(), Diagnostic> {
        match expr {
            Expr::Var { .. } => self.check_var(expr),
            Expr::Index { .. } => self.check_index(expr),
            Expr::Unary {
                op: UnaryOp::Deref, ..
            } => self.check_unary(expr),
            Expr::Str { .. } => self.check_str(expr),
            Expr::Generic { .. } | Expr::CompoundLit { .. } => {
                self.check_expr(expr)?;
                if !expr.is_lvalue() {
                    return self.err(expr.line(), expr.column(), "expression is not assignable");
                }
                Ok(())
            }
            Expr::Member { .. } => {
                self.check_member(expr)?;
                if !expr.is_lvalue() {
                    return self.err(expr.line(), expr.column(), "expression is not assignable");
                }
                Ok(())
            }
            _ => self.err(expr.line(), expr.column(), "expression is not assignable"),
        }
    }
}

pub(super) fn is_bitfield(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Member {
            bit_width: Some(_),
            ..
        }
    )
}

/// The type of a bit-field after the integer promotions (C11 6.3.1.1).
///
/// Only types whose rank is at most `int` are promoted. A signed field, and
/// an unsigned field narrower than 32 bits, become `int`. An unsigned field
/// of 32 bits becomes `unsigned int`. `long` and wider stay as declared.
pub(super) fn bitfield_value_type(declared: TypeName, width: u32) -> TypeName {
    if declared.rank() > TypeName::Int.rank() {
        return declared;
    }
    let fits_in_int = if declared.is_signed() {
        width <= 32
    } else {
        width < 32
    };
    if fits_in_int {
        TypeName::Int
    } else {
        TypeName::UInt
    }
}

pub(super) fn pointer_plus_int(left: &CType, right: &CType) -> Option<CType> {
    if left.is_pointer() && right.is_integer() {
        Some(left.clone())
    } else if right.is_pointer() && left.is_integer() {
        Some(right.clone())
    } else {
        None
    }
}

/// An integer constant expression with value 0, or that expression cast to
/// `void *` (C11 6.3.2.3p3). A `void *` cast of a pointer, such as
/// `(void *)(void *)0`, is a null pointer but not a null pointer constant.
pub(super) fn is_null_pointer_constant(expr: &Expr) -> bool {
    match expr {
        Expr::Cast { ty_name, expr, .. }
            if ty_name
                .pointee()
                .is_some_and(|pointee| pointee.is_void() && pointee.quals().is_empty()) =>
        {
            is_integer_zero(expr)
        }
        _ => is_integer_zero(expr),
    }
}

/// An integer constant expression with value 0.
pub(super) fn is_integer_zero(expr: &Expr) -> bool {
    match eval_const_expr(expr) {
        Ok(value) => value.ty.is_integer() && (value.bits & value.ty.mask()) == 0,
        Err(_) => false,
    }
}
