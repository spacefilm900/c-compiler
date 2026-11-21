//! Expressions: values and addresses, pointer arithmetic, arithmetic,
//! assignment, increment and decrement, conditions and the conditional
//! operator.

use super::*;

impl Gen {
    pub(super) fn gen_val(&mut self, expr: &Expr) -> String {
        self.gen_expr(expr)
            .unwrap_or_else(|| self.ice_in(expr, "void expression used as a value"))
    }

    pub(super) fn gen_expr(&mut self, expr: &Expr) -> Option<String> {
        // A struct or union value is the address of its object representation.
        // Copies use `llvm.memmove`, so padding bytes are kept.
        if expr.ty().is_some_and(|ty| ty.is_record()) {
            return Some(self.record_value_ptr(expr));
        }
        match expr {
            Expr::Int { bits, value_ty, .. } => Some(value_ty.format_bits(u128::from(*bits))),
            Expr::Float { bits, kind, .. } => Some(crate::float::llvm_const(*kind, *bits)),
            Expr::LabelAddr { name, func, .. } => {
                let Some(func) = func.as_deref() else {
                    return Some(self.ice_in(expr, "label address outside a function"));
                };
                Some(format!(
                    "blockaddress({}, %{})",
                    llvm_global(renamed(func, &self.renames)),
                    c_label_via_ptr(name)
                ))
            }
            Expr::Var { resolved, ty, .. } => {
                let slot = self.object_addr(&resolved.as_ref().expect("resolved variable").slot);
                let ty = ty.clone().expect("typed variable");
                if ty.is_array() || ty.is_func() {
                    Some(slot)
                } else {
                    Some(self.load(&ty, &slot))
                }
            }
            Expr::Unary {
                op,
                expr: inner,
                ty,
                ..
            } => self.gen_unary(expr, *op, inner, ty.as_ref()),
            Expr::Bin {
                op,
                left,
                right,
                ty,
                ..
            } => Some(self.gen_binary(expr, *op, left, right, ty.as_ref())),
            Expr::Assign { target, value, .. } => Some(self.gen_assign(target, value)),
            Expr::CompoundAssign {
                op, target, value, ..
            } => Some(self.gen_compound_assign(*op, target, value)),
            Expr::IncDec { op, target, .. } => Some(self.gen_incdec(*op, target)),
            Expr::Cond {
                cond,
                then_expr,
                else_expr,
                ty,
                ..
            } => self.gen_ternary(cond, then_expr, else_expr, ty.clone().expect("typed")),
            Expr::Comma { left, right, .. } => {
                let _ = self.gen_expr(left);
                self.gen_expr(right)
            }
            Expr::Sizeof { operand, .. } => Some(self.gen_sizeof(expr, operand)),
            Expr::Cast { ty_name, expr, .. } => {
                self.eval_vla_sizes(ty_name);
                if ty_name.is_void() {
                    let _ = self.gen_expr(expr);
                    None
                } else {
                    let value = self.gen_val(expr);
                    Some(self.convert(&value, &expr.ty().expect("typed"), ty_name))
                }
            }
            Expr::Index {
                base, index, ty, ..
            } => {
                let addr = self.gen_ptr_index(base, index);
                Some(self.value_at(&ty.clone().expect("typed"), addr))
            }
            Expr::Decay { expr, .. } => Some(self.gen_addr(expr)),
            Expr::Str { .. } => Some(self.gen_addr(expr)),
            Expr::Member { ty, bit_width, .. } => {
                if bit_width.is_some() {
                    Some(self.load_bitfield(expr))
                } else {
                    let addr = self.gen_addr(expr);
                    Some(self.value_at(&ty.clone().expect("typed"), addr))
                }
            }
            Expr::Call {
                callee,
                args,
                resolved,
                ..
            } => self.gen_call(callee, args, resolved.as_ref().expect("resolved call")),
            Expr::VaArg { .. } => Some(self.gen_va_arg(expr)),
            Expr::VaOp { op, args, .. } => {
                self.gen_va_op(*op, args);
                None
            }
            Expr::CompoundLit { decl, .. } => {
                let slot = self.gen_compound_lit(decl);
                Some(self.value_at(&decl.ty, slot))
            }
            Expr::Generic { .. } => {
                unreachable!("semantic analysis replaces a generic selection")
            }
            Expr::StmtExpr { body, ty, .. } => {
                let depth = self.vla_depth();
                let last = self.gen_stmt_expr_body(body);
                let value = last.and_then(|last| self.gen_expr(last));
                self.ensure_block();
                self.leave_vla_to(depth);
                if ty.as_ref().is_some_and(CType::is_void) {
                    None
                } else {
                    value
                }
            }
            Expr::CondShort {
                cond,
                else_expr,
                ty,
                ..
            } => self.gen_cond_short(cond, else_expr, ty.clone().expect("typed")),
        }
    }

    /// The value of an object of type `ty` at `addr`; an array is its
    /// address.
    fn value_at(&mut self, ty: &CType, addr: String) -> String {
        if ty.is_array() {
            addr
        } else {
            self.load(ty, &addr)
        }
    }

    /// A unary operator other than `++` / `--`; `whole` is the unary
    /// expression and `operand` its operand.
    fn gen_unary(
        &mut self,
        whole: &Expr,
        op: UnaryOp,
        operand: &Expr,
        ty: Option<&CType>,
    ) -> Option<String> {
        match op {
            UnaryOp::Not => Some(self.zext_cond(whole)),
            UnaryOp::Plus => {
                let value = self.gen_val(operand);
                let src = operand.ty().expect("typed");
                if src.is_float() {
                    Some(value)
                } else if let Some(int) = src.as_integer() {
                    let promoted = CType::from_name(int.promote());
                    Some(self.convert(&value, &src, &promoted))
                } else {
                    Some(self.ice_in(operand, "unary '+' on an operand that is not arithmetic"))
                }
            }
            UnaryOp::Minus => Some(self.gen_neg_or_not(operand, "sub")),
            UnaryOp::BitNot => Some(self.gen_neg_or_not(operand, "xor")),
            UnaryOp::Addr => Some(self.gen_addr(operand)),
            UnaryOp::Deref => {
                let addr = self.gen_val(operand);
                let ty = ty.cloned().expect("typed");
                if ty.is_void() {
                    None
                } else {
                    Some(self.value_at(&ty, addr))
                }
            }
        }
    }

    /// A binary operator; `whole` is the binary expression. Comparisons and
    /// `&&` / `||` give an `int` 0 or 1.
    fn gen_binary(
        &mut self,
        whole: &Expr,
        op: BinOp,
        left: &Expr,
        right: &Expr,
        ty: Option<&CType>,
    ) -> String {
        if op.is_cmp() || matches!(op, BinOp::And | BinOp::Or) {
            self.zext_cond(whole)
        } else if op == BinOp::Sub
            && left.ty().is_some_and(|ty| ty.is_pointer())
            && right.ty().is_some_and(|ty| ty.is_pointer())
        {
            self.gen_ptr_diff(left, right)
        } else if ty.is_some_and(|ty| ty.is_pointer()) {
            self.gen_ptr_arith(op, left, right)
        } else {
            self.gen_arith(op, left, right)
        }
    }

    /// Simple assignment: the value converted to the target's type is
    /// stored and is the result.
    fn gen_assign(&mut self, target: &Expr, value: &Expr) -> String {
        let addr = self.gen_addr(target);
        let dest = target.ty().expect("lvalue type");
        let raw = self.gen_val(value);
        let src_ty = value.ty().expect("typed");
        if bit_info(target).is_some() {
            self.store_bitfield(&addr, target, &raw, &src_ty)
        } else {
            let stored = self.convert(&raw, &src_ty, &dest);
            self.store(&dest, &stored, &addr);
            stored
        }
    }

    /// `sizeof` that reached code generation: a `size_t` constant, or a
    /// runtime value when the operand has a variable length array type.
    fn gen_sizeof(&mut self, whole: &Expr, operand: &SizeofOperand) -> String {
        let ty = match operand {
            SizeofOperand::Type(ty) => ty.clone(),
            SizeofOperand::Expr(inner) => {
                if inner.ty().is_some_and(|ty| ty.is_vla()) {
                    let _ = self.gen_expr(inner);
                }
                inner.ty().expect("typed")
            }
        };
        if !ty.is_vla() {
            let Some(size) = ty.size() else {
                return self.ice_in(whole, "sizeof of an incomplete type");
            };
            return TypeName::size_t().format_bits(u128::from(size));
        }
        self.gen_sizeof_value(&ty)
    }

    /// Emit a statement expression's block up to its last expression
    /// statement, which is returned for the caller to evaluate. Its
    /// automatic objects get entry-block slots.
    pub(super) fn gen_stmt_expr_body<'b>(&mut self, body: &'b Compound) -> Option<&'b Expr> {
        for (slot, ty, align) in local_slots(body) {
            let line = format!("  {slot} = alloca {}, align {align}", ty.ir_type());
            self.lines.insert(self.alloca_at, line);
            self.alloca_at += 1;
        }
        let (last, rest) = match body.items.split_last() {
            Some((
                Stmt::Expr {
                    expr: Some(last), ..
                },
                rest,
            )) => (Some(last), rest),
            _ => (None, &body.items[..]),
        };
        for item in rest {
            if let Err(ice) = self.gen_stmt(item) {
                // Back inside an expression: keep the error, as expression
                // lowering does, and let the caller finish with a placeholder.
                self.ice = Some(ice);
                break;
            }
        }
        self.ensure_block();
        last
    }

    /// `x ?: y`: `x` is evaluated once; when it is nonzero it is the
    /// result, otherwise `y` is evaluated.
    pub(super) fn gen_cond_short(
        &mut self,
        cond: &Expr,
        else_expr: &Expr,
        result: CType,
    ) -> Option<String> {
        let cond_ty = cond.ty().expect("typed");
        let value = self.gen_val(cond);
        let test = self.test_nonzero(&value, &cond_ty);
        let then_label = self.label();
        let else_label = self.label();
        let end_label = self.label();
        self.br_cond(&test, &then_label, &else_label);
        self.start(then_label);
        let then_val = if result.is_void() {
            None
        } else {
            Some(self.convert(&value, &cond_ty, &result))
        };
        let then_block = self.cur.clone();
        self.br(&end_label);
        self.start(else_label);
        let else_val = self
            .gen_expr(else_expr)
            .map(|value| self.convert(&value, &else_expr.ty().expect("typed"), &result));
        self.ensure_block();
        let else_block = self.cur.clone();
        self.br(&end_label);
        self.start(end_label);
        match (then_val, else_val) {
            (Some(then_val), Some(else_val)) => {
                let tmp = self.fresh();
                self.emit(&format!(
                    "  {tmp} = phi {} [ {then_val}, %{then_block} ], [ {else_val}, %{else_block} ]",
                    result.ir_type()
                ));
                Some(tmp)
            }
            _ => None,
        }
    }

    /// `value != 0` as an `i1`, for a scalar of type `ty`.
    pub(super) fn test_nonzero(&mut self, value: &str, ty: &CType) -> String {
        let tmp = self.fresh();
        if ty.is_pointer() {
            self.emit(&format!("  {tmp} = icmp ne ptr {value}, null"));
        } else if ty.is_float() {
            let zero = crate::float::llvm_const(ty.float_kind().expect("float"), 0);
            self.emit(&format!(
                "  {tmp} = fcmp une {} {value}, {zero}",
                ty.ir_type()
            ));
        } else {
            self.emit(&format!("  {tmp} = icmp ne {} {value}, 0", ty.ir_type()));
        }
        tmp
    }

    pub(super) fn gen_addr(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Var { resolved, .. } => {
                self.object_addr(&resolved.as_ref().expect("resolved variable").slot)
            }
            Expr::Unary {
                op: UnaryOp::Deref,
                expr,
                ..
            } => self.gen_val(expr),
            Expr::Index { base, index, .. } => self.gen_ptr_index(base, index),
            Expr::Decay { expr, .. } => self.gen_addr(expr),
            Expr::Str { symbol, .. } => match symbol.as_deref() {
                Some(symbol) => llvm_global(symbol),
                None => self.ice_in(expr, "string literal has no symbol"),
            },
            Expr::Member {
                base,
                arrow,
                index,
                offset,
                bytewise,
                ..
            } => {
                let base_ptr = if *arrow {
                    self.gen_val(base)
                } else if base.is_lvalue() {
                    self.gen_addr(base)
                } else if base.ty().is_some_and(|ty| ty.is_record()) {
                    self.record_ptr(base)
                } else {
                    self.spill(base)
                };
                let tmp = self.fresh();
                let record_ty = if *arrow {
                    base.ty().as_ref().and_then(CType::pointee).cloned()
                } else {
                    base.ty()
                };
                let field_align = expr.ty().map_or(1, |ty| ty.member_align());
                let record_align = record_ty.as_ref().map_or(1, CType::align);
                let field_offset = offset.unwrap_or(0);
                if self.unaligned.contains(&base_ptr)
                    || record_align < field_align
                    || field_offset % u64::from(field_align.max(1)) != 0
                {
                    self.unaligned.insert(tmp.clone());
                }
                if *bytewise {
                    let Some(offset) = *offset else {
                        return self.ice_in(expr, "member access has no byte offset");
                    };
                    self.emit(&format!(
                        "  {tmp} = getelementptr inbounds i8, ptr {base_ptr}, i64 {offset}"
                    ));
                } else {
                    let (Some(struct_ty), Some(index)) = (record_ty, *index) else {
                        return self.ice_in(expr, "member access has no record type or index");
                    };
                    self.emit(&format!(
                        "  {tmp} = getelementptr inbounds {}, ptr {base_ptr}, i32 0, i32 {index}",
                        struct_ty.ir_type()
                    ));
                }
                tmp
            }
            Expr::CompoundLit { decl, .. } => self.gen_compound_lit(decl),
            _ => self.ice_in(expr, "address of an expression that is not an lvalue"),
        }
    }

    /// The object of a block-scope compound literal (C11 6.5.2.5p5). It is
    /// an entry-block alloca, initialized here each time the literal is
    /// evaluated.
    pub(super) fn gen_compound_lit(&mut self, decl: &VarDecl) -> String {
        let slot = self.entry_alloca(&decl.ty);
        let mut object = decl.clone();
        object.slot = slot.clone();
        self.gen_decl(&object);
        slot
    }

    /// Materialize a struct rvalue so a member can be addressed.
    pub(super) fn spill(&mut self, expr: &Expr) -> String {
        let ty = expr.ty().expect("typed");
        let value = self.gen_val(expr);
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = alloca {}, align {}",
            ty.ir_type(),
            ty.align()
        ));
        self.store(&ty, &value, &tmp);
        tmp
    }

    pub(super) fn gen_ptr_index(&mut self, base: &Expr, index: &Expr) -> String {
        let (ptr_expr, int_expr) = if base.ty().is_some_and(|ty| ty.is_pointer()) {
            (base, index)
        } else {
            (index, base)
        };
        self.gen_gep(ptr_expr, int_expr, false)
    }

    pub(super) fn gen_ptr_arith(&mut self, op: BinOp, left: &Expr, right: &Expr) -> String {
        let (ptr_expr, int_expr, negate) = if left.ty().is_some_and(|ty| ty.is_pointer()) {
            (left, right, op == BinOp::Sub)
        } else {
            (right, left, false)
        };
        self.gen_gep(ptr_expr, int_expr, negate)
    }

    pub(super) fn gen_gep(&mut self, ptr_expr: &Expr, int_expr: &Expr, negate: bool) -> String {
        let ptr = self.gen_val(ptr_expr);
        let idx = self.gen_val(int_expr);
        let mut idx = self.extend_i64(&idx, &int_expr.ty().expect("integer index"));
        if negate {
            let tmp = self.fresh();
            self.emit(&format!("  {tmp} = sub i64 0, {idx}"));
            idx = tmp;
        }
        let Some(pointee) = ptr_expr.ty().and_then(|ty| ty.pointee().cloned()) else {
            return self.ice_in(
                ptr_expr,
                "pointer arithmetic on an operand that is not a pointer",
            );
        };
        if pointee.is_vla() {
            return self.gen_vla_gep(&ptr, &idx, &pointee, self.unaligned.contains(&ptr), true);
        }
        if pointee.size().is_none() {
            return self.ice_in(
                ptr_expr,
                "pointer arithmetic on an operand that is not a pointer",
            );
        }
        let elem = pointee.ir_type();
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = getelementptr inbounds {elem}, ptr {ptr}, i64 {idx}"
        ));
        if self.unaligned.contains(&ptr) {
            self.unaligned.insert(tmp.clone());
        }
        tmp
    }

    pub(super) fn gen_ptr_diff(&mut self, left: &Expr, right: &Expr) -> String {
        let left_val = self.gen_val(left);
        let right_val = self.gen_val(right);
        let left_int = self.fresh();
        let right_int = self.fresh();
        self.emit(&format!("  {left_int} = ptrtoint ptr {left_val} to i64"));
        self.emit(&format!("  {right_int} = ptrtoint ptr {right_val} to i64"));
        let diff = self.fresh();
        self.emit(&format!("  {diff} = sub i64 {left_int}, {right_int}"));
        let Some(pointee) = left.ty().and_then(|ty| ty.pointee().cloned()) else {
            return self.ice_in(left, "pointer subtraction with an incomplete pointee type");
        };
        if pointee.is_vla() {
            let size = self.runtime_size(&pointee);
            let tmp = self.fresh();
            self.emit(&format!("  {tmp} = sdiv i64 {diff}, {size}"));
            return tmp;
        }
        let Some(size) = pointee.size() else {
            return self.ice_in(left, "pointer subtraction with an incomplete pointee type");
        };
        if size == 1 {
            return diff;
        }
        let tmp = self.fresh();
        self.emit(&format!("  {tmp} = sdiv exact i64 {diff}, {size}"));
        tmp
    }

    pub(super) fn gen_neg_or_not(&mut self, expr: &Expr, kind: &str) -> String {
        let src = expr.ty().expect("typed");
        if src.is_float() && kind == "sub" {
            let value = self.gen_val(expr);
            let tmp = self.fresh();
            self.emit(&format!("  {tmp} = fneg {} {value}", src.ir_type()));
            return tmp;
        }
        let Some(int) = src.as_integer() else {
            return self.ice_in(
                expr,
                "integer operator on an operand that is not an integer",
            );
        };
        let dest = CType::from_name(int.promote());
        let value = self.gen_val(expr);
        let value = self.convert(&value, &src, &dest);
        let tmp = self.fresh();
        if kind == "sub" {
            self.emit(&format!("  {tmp} = sub {} 0, {value}", dest.ir_type()));
        } else {
            self.emit(&format!("  {tmp} = xor {} {value}, -1", dest.ir_type()));
        }
        tmp
    }

    pub(super) fn gen_arith(&mut self, op: BinOp, left: &Expr, right: &Expr) -> String {
        let left_ty = left.ty().expect("typed");
        let right_ty = right.ty().expect("typed");
        if left_ty.is_float() || right_ty.is_float() {
            return self.gen_float_arith(op, left, right);
        }
        let left_i = left_ty.as_integer().expect("integer");
        let right_i = right_ty.as_integer().expect("integer");
        let common = if matches!(op, BinOp::Shl | BinOp::Shr) {
            left_i.promote()
        } else {
            left_i.usual(right_i)
        };
        let common_ty = CType::from_name(common);
        let left_val = self.gen_val(left);
        let left_val = self.convert(&left_val, &left_ty, &common_ty);
        let right_val = self.gen_val(right);
        let right_val = self.convert(&right_val, &right_ty, &common_ty);
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = {} {} {left_val}, {right_val}",
            value_inst(op, common.is_signed()),
            common.ir_type()
        ));
        tmp
    }

    pub(super) fn gen_float_arith(&mut self, op: BinOp, left: &Expr, right: &Expr) -> String {
        let left_ty = left.ty().expect("typed");
        let right_ty = right.ty().expect("typed");
        let common = common_real(&left_ty, &right_ty);
        let left_val = self.gen_val(left);
        let left_val = self.convert(&left_val, &left_ty, &common);
        let right_val = self.gen_val(right);
        let right_val = self.convert(&right_val, &right_ty, &common);
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = {} {} {left_val}, {right_val}",
            float_inst(op),
            common.ir_type()
        ));
        tmp
    }

    pub(super) fn gen_compound_assign(&mut self, op: BinOp, target: &Expr, value: &Expr) -> String {
        let dest = target.ty().expect("lvalue type");
        if dest.is_atomic() {
            return self.gen_atomic_compound(op, target, value);
        }
        let rhs_ty = value.ty().expect("typed");
        // The right-hand side is evaluated before the lvalue is loaded. The
        // address is still computed only once.
        let rhs = self.gen_val(value);
        let addr = self.gen_addr(target);
        let old = if bit_info(target).is_some() {
            self.load_bitfield_at(&addr, target)
        } else {
            self.load(&dest, &addr)
        };
        if let Some(pointee) = dest.pointee() {
            let mut idx = self.extend_i64(&rhs, &rhs_ty);
            if op == BinOp::Sub {
                let tmp = self.fresh();
                self.emit(&format!("  {tmp} = sub i64 0, {idx}"));
                idx = tmp;
            }
            let updated = if pointee.size().is_none() {
                self.gen_vla_gep(&old, &idx, pointee, self.unaligned.contains(&old), true)
            } else {
                let elem = pointee.ir_type();
                let updated = self.fresh();
                self.emit(&format!(
                    "  {updated} = getelementptr inbounds {elem}, ptr {old}, i64 {idx}"
                ));
                updated
            };
            self.store(&dest, &updated, &addr);
            return updated;
        }
        if dest.is_float() || rhs_ty.is_float() {
            let common = common_real(&dest, &rhs_ty);
            let old_c = self.convert(&old, &dest, &common);
            let rhs_c = self.convert(&rhs, &rhs_ty, &common);
            let updated = self.fresh();
            self.emit(&format!(
                "  {updated} = {} {} {old_c}, {rhs_c}",
                float_inst(op),
                common.ir_type()
            ));
            // A bit-field takes the result in its declared type (C11
            // 6.5.16.2p3, 6.5.16.1p2), and only its own bits are written.
            // For `_Bool f : 1`, `f += 0.25` is 1: the sum converts to
            // `_Bool` directly, not through the promoted `int`.
            if let Some(info) = bit_info(target) {
                let declared = CType::from_name(info.declared);
                let stored = self.convert(&updated, &common, &declared);
                return self.store_bitfield(&addr, target, &stored, &declared);
            }
            let stored = self.convert(&updated, &common, &dest);
            self.store(&dest, &stored, &addr);
            return stored;
        }
        let (Some(dest_i), Some(rhs_i)) = (dest.as_integer(), rhs_ty.as_integer()) else {
            return self.ice_in(target, "compound assignment operands are not arithmetic");
        };
        let common = if matches!(op, BinOp::Shl | BinOp::Shr) {
            dest_i.promote()
        } else {
            dest_i.usual(rhs_i)
        };
        let common_ty = CType::from_name(common);
        let old_c = self.convert(&old, &dest, &common_ty);
        let rhs_c = self.convert(&rhs, &rhs_ty, &common_ty);
        let updated = self.fresh();
        self.emit(&format!(
            "  {updated} = {} {} {old_c}, {rhs_c}",
            value_inst(op, common.is_signed()),
            common.ir_type()
        ));
        if let Some(info) = bit_info(target) {
            // As above: convert straight to the declared type, so a
            // `_Bool` bit-field sees the full-width result.
            let declared = CType::from_name(info.declared);
            let stored = self.convert(&updated, &common_ty, &declared);
            return self.store_bitfield(&addr, target, &stored, &declared);
        }
        let stored = self.convert(&updated, &common_ty, &dest);
        self.store(&dest, &stored, &addr);
        stored
    }

    pub(super) fn gen_incdec(&mut self, op: IncOp, target: &Expr) -> String {
        let dest = target.ty().expect("lvalue type");
        if dest.is_atomic() {
            return self.gen_atomic_incdec(op, target);
        }
        let addr = self.gen_addr(target);
        let old = if bit_info(target).is_some() {
            self.load_bitfield_at(&addr, target)
        } else {
            self.load(&dest, &addr)
        };
        if let Some(kind) = dest.float_kind() {
            let one = crate::float::llvm_const(kind, crate::float::one_bits(kind));
            let updated = self.fresh();
            let inst = if op.is_increment() { "fadd" } else { "fsub" };
            self.emit(&format!(
                "  {updated} = {inst} {} {old}, {one}",
                dest.ir_type()
            ));
            let written = if bit_info(target).is_some() {
                self.store_bitfield(&addr, target, &updated, &dest)
            } else {
                self.store(&dest, &updated, &addr);
                updated
            };
            return if op.is_prefix() { written } else { old };
        }
        if let Some(pointee) = dest.pointee() {
            let step = if op.is_increment() { "1" } else { "-1" };
            let updated = if pointee.size().is_none() {
                self.gen_vla_gep(&old, step, pointee, self.unaligned.contains(&old), true)
            } else {
                let elem = pointee.ir_type();
                let updated = self.fresh();
                self.emit(&format!(
                    "  {updated} = getelementptr inbounds {elem}, ptr {old}, i64 {step}"
                ));
                updated
            };
            self.store(&dest, &updated, &addr);
            return if op.is_prefix() { updated } else { old };
        }
        let Some(dest_i) = dest.as_integer() else {
            return self.ice_in(target, "'++' or '--' on an operand that is not a scalar");
        };
        let common = dest_i.usual(TypeName::Int);
        let common_ty = CType::from_name(common);
        let old_c = self.convert(&old, &dest, &common_ty);
        let updated = self.fresh();
        let inst = if op.is_increment() { "add" } else { "sub" };
        self.emit(&format!(
            "  {updated} = {inst} {} {old_c}, 1",
            common.ir_type()
        ));
        let stored = self.convert(&updated, &common_ty, &dest);
        let written = if bit_info(target).is_some() {
            self.store_bitfield(&addr, target, &stored, &dest)
        } else {
            self.store(&dest, &stored, &addr);
            stored
        };
        if op.is_prefix() {
            written
        } else {
            old
        }
    }

    pub(super) fn gen_cond(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Bin {
                op, left, right, ..
            } if op.is_cmp() => {
                let left_ty = left.ty().expect("typed");
                let right_ty = right.ty().expect("typed");
                if left_ty.is_float() || right_ty.is_float() {
                    let common = common_real(&left_ty, &right_ty);
                    let left_val = self.gen_val(left);
                    let left_val = self.convert(&left_val, &left_ty, &common);
                    let right_val = self.gen_val(right);
                    let right_val = self.convert(&right_val, &right_ty, &common);
                    let tmp = self.fresh();
                    self.emit(&format!(
                        "  {tmp} = fcmp {} {} {left_val}, {right_val}",
                        fcmp_pred(*op),
                        common.ir_type()
                    ));
                    return tmp;
                }
                if left_ty.is_pointer() || right_ty.is_pointer() {
                    let left_val = self.as_ptr(left);
                    let right_val = self.as_ptr(right);
                    let tmp = self.fresh();
                    self.emit(&format!(
                        "  {tmp} = icmp {} ptr {left_val}, {right_val}",
                        ptr_pred(*op)
                    ));
                    return tmp;
                }
                let left_i = left_ty.as_integer().expect("integer");
                let right_i = right_ty.as_integer().expect("integer");
                let common = left_i.usual(right_i);
                let common_ty = CType::from_name(common);
                let left_val = self.gen_val(left);
                let left_val = self.convert(&left_val, &left_ty, &common_ty);
                let right_val = self.gen_val(right);
                let right_val = self.convert(&right_val, &right_ty, &common_ty);
                let tmp = self.fresh();
                self.emit(&format!(
                    "  {tmp} = icmp {} {} {left_val}, {right_val}",
                    cmp_pred(*op, common.is_signed()),
                    common.ir_type()
                ));
                tmp
            }
            Expr::Bin {
                op: BinOp::And,
                left,
                right,
                ..
            } => self.gen_and(left, right),
            Expr::Bin {
                op: BinOp::Or,
                left,
                right,
                ..
            } => self.gen_or(left, right),
            Expr::Unary {
                op: UnaryOp::Not,
                expr,
                ..
            } => {
                let inner = self.gen_cond(expr);
                let tmp = self.fresh();
                self.emit(&format!("  {tmp} = xor i1 {inner}, true"));
                tmp
            }
            _ => {
                let value = self.gen_val(expr);
                let ty = expr.ty().expect("typed");
                let tmp = self.fresh();
                if ty.is_pointer() {
                    self.emit(&format!("  {tmp} = icmp ne ptr {value}, null"));
                } else if ty.is_float() {
                    let zero = crate::float::llvm_const(ty.float_kind().expect("float"), 0);
                    self.emit(&format!(
                        "  {tmp} = fcmp une {} {value}, {zero}",
                        ty.ir_type()
                    ));
                } else {
                    self.emit(&format!("  {tmp} = icmp ne {} {value}, 0", ty.ir_type()));
                }
                tmp
            }
        }
    }

    pub(super) fn gen_and(&mut self, left: &Expr, right: &Expr) -> String {
        let left = self.gen_cond(left);
        let rhs_label = self.label();
        let false_label = self.label();
        let end_label = self.label();
        self.br_cond(&left, &rhs_label, &false_label);
        self.start(rhs_label);
        let right = self.gen_cond(right);
        let rhs_block = self.cur.clone();
        self.br(&end_label);
        self.start(false_label.clone());
        self.br(&end_label);
        self.start(end_label);
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = phi i1 [ {right}, %{rhs_block} ], [ false, %{false_label} ]"
        ));
        tmp
    }

    /// Only the chosen arm is evaluated. Each arm is converted to `result`
    /// in its own block, so the phi operands are defined in the predecessors.
    pub(super) fn gen_ternary(
        &mut self,
        cond: &Expr,
        then_expr: &Expr,
        else_expr: &Expr,
        result: CType,
    ) -> Option<String> {
        let cond = self.gen_cond(cond);
        let then_label = self.label();
        let else_label = self.label();
        let end_label = self.label();
        self.br_cond(&cond, &then_label, &else_label);
        self.start(then_label);
        let then_val = self
            .gen_expr(then_expr)
            .map(|value| self.convert(&value, &then_expr.ty().expect("typed"), &result));
        if self.terminated {
            self.ice("an expression ended its block");
        }
        let then_block = self.cur.clone();
        self.br(&end_label);
        self.start(else_label);
        let else_val = self
            .gen_expr(else_expr)
            .map(|value| self.convert(&value, &else_expr.ty().expect("typed"), &result));
        if self.terminated {
            self.ice("an expression ended its block");
        }
        let else_block = self.cur.clone();
        self.br(&end_label);
        self.start(end_label);
        match (then_val, else_val) {
            (Some(then_val), Some(else_val)) => {
                let tmp = self.fresh();
                self.emit(&format!(
                    "  {tmp} = phi {} [ {then_val}, %{then_block} ], [ {else_val}, %{else_block} ]",
                    result.ir_type()
                ));
                Some(tmp)
            }
            (None, None) => None,
            // Only after an earlier internal error, which is the one reported.
            _ => Some(self.ice("conditional arms have different types")),
        }
    }

    pub(super) fn gen_or(&mut self, left: &Expr, right: &Expr) -> String {
        let left = self.gen_cond(left);
        let true_label = self.label();
        let rhs_label = self.label();
        let end_label = self.label();
        self.br_cond(&left, &true_label, &rhs_label);
        self.start(rhs_label);
        let right = self.gen_cond(right);
        let rhs_block = self.cur.clone();
        self.br(&end_label);
        self.start(true_label.clone());
        self.br(&end_label);
        self.start(end_label);
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = phi i1 [ true, %{true_label} ], [ {right}, %{rhs_block} ]"
        ));
        tmp
    }
}

pub(super) fn float_rank(kind: FloatKind) -> u8 {
    match kind {
        FloatKind::Float => 0,
        FloatKind::Double => 1,
        FloatKind::LongDouble => 2,
    }
}

pub(super) fn common_real(left: &CType, right: &CType) -> CType {
    CType::Float(crate::float::usual_real(
        left.float_kind(),
        right.float_kind(),
    ))
}

pub(super) fn float_inst(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "fadd",
        BinOp::Sub => "fsub",
        BinOp::Mul => "fmul",
        BinOp::Div => "fdiv",
        _ => unreachable!("floating operator"),
    }
}

pub(super) fn fcmp_pred(op: BinOp) -> &'static str {
    match op {
        BinOp::Eq => "oeq",
        BinOp::Ne => "une",
        BinOp::Lt => "olt",
        BinOp::Gt => "ogt",
        BinOp::Le => "ole",
        BinOp::Ge => "oge",
        _ => unreachable!("floating comparison"),
    }
}

pub(super) fn value_inst(op: BinOp, signed: bool) -> &'static str {
    match op {
        BinOp::Add => "add",
        BinOp::Sub => "sub",
        BinOp::Mul => "mul",
        BinOp::Div => {
            if signed {
                "sdiv"
            } else {
                "udiv"
            }
        }
        BinOp::Mod => {
            if signed {
                "srem"
            } else {
                "urem"
            }
        }
        BinOp::BitAnd => "and",
        BinOp::BitOr => "or",
        BinOp::BitXor => "xor",
        BinOp::Shl => "shl",
        BinOp::Shr => {
            if signed {
                "ashr"
            } else {
                "lshr"
            }
        }
        BinOp::Eq
        | BinOp::Ne
        | BinOp::Lt
        | BinOp::Gt
        | BinOp::Le
        | BinOp::Ge
        | BinOp::And
        | BinOp::Or => {
            unreachable!("comparison or logical operator")
        }
    }
}

pub(super) fn ptr_pred(op: BinOp) -> &'static str {
    match op {
        BinOp::Eq => "eq",
        BinOp::Ne => "ne",
        BinOp::Lt => "ult",
        BinOp::Gt => "ugt",
        BinOp::Le => "ule",
        BinOp::Ge => "uge",
        _ => unreachable!("pointer comparison"),
    }
}

pub(super) fn cmp_pred(op: BinOp, signed: bool) -> &'static str {
    match op {
        BinOp::Eq => "eq",
        BinOp::Ne => "ne",
        BinOp::Lt => {
            if signed {
                "slt"
            } else {
                "ult"
            }
        }
        BinOp::Gt => {
            if signed {
                "sgt"
            } else {
                "ugt"
            }
        }
        BinOp::Le => {
            if signed {
                "sle"
            } else {
                "ule"
            }
        }
        BinOp::Ge => {
            if signed {
                "sge"
            } else {
                "uge"
            }
        }
        _ => unreachable!("comparison"),
    }
}
