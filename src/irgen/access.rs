//! Memory access: record copies and addresses, loads and stores, bit-fields,
//! and scalar conversions.

use super::*;

impl Gen {
    /// Byte copy of a struct, including padding. `llvm.memmove` allows the
    /// source and destination to be the same object.
    pub(super) fn copy_struct(&mut self, ty: &CType, src: &str, dest: &str, src_volatile: bool) {
        let size = ty.size().expect("complete struct");
        self.needs_memmove = true;
        // A volatile source or destination is one access. LLVM may drop a
        // non-volatile `memmove` of a volatile object.
        let volatile = if ty.is_volatile() || src_volatile {
            "true"
        } else {
            "false"
        };
        self.emit(&format!(
            "  call void @llvm.memmove.p0.p0.i64(ptr {dest}, ptr {src}, i64 {size}, i1 {volatile})"
        ));
    }

    /// Pointer to the object representation of a struct or union expression.
    ///
    /// Lvalues are their address. A call, a conditional, or an assignment is
    /// materialized first, so `f().x` and `s = f()` see the returned bytes.
    pub(super) fn record_ptr(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Var { .. }
            | Expr::Index { .. }
            | Expr::Str { .. }
            | Expr::Unary {
                op: UnaryOp::Deref, ..
            }
            | Expr::Member { .. }
            | Expr::CompoundLit { .. } => self.gen_addr(expr),
            Expr::VaArg { .. } => self.gen_va_record(expr),
            Expr::Assign { target, value, .. } => {
                // The right-hand side is evaluated before the destination is
                // stored, matching scalar assignment.
                let src_volatile = value.ty().is_some_and(|ty| ty.is_volatile());
                let src = self.record_ptr(value);
                let dest = self.gen_addr(target);
                let ty = target.ty().expect("struct assignment");
                self.copy_struct(&ty, &src, &dest, src_volatile);
                dest
            }
            Expr::Call { .. } => self.materialize_call(expr),
            Expr::Cond {
                cond,
                then_expr,
                else_expr,
                ty,
                ..
            } => {
                let ty = ty.clone().expect("typed conditional");
                let tmp = self.entry_alloca(&ty);
                let cond = self.gen_cond(cond);
                let then_label = self.label();
                let else_label = self.label();
                let end_label = self.label();
                self.br_cond(&cond, &then_label, &else_label);
                self.start(then_label);
                let then_volatile = then_expr.ty().is_some_and(|ty| ty.is_volatile());
                let then_src = self.record_ptr(then_expr);
                self.copy_struct(&ty, &then_src, &tmp, then_volatile);
                self.br(&end_label);
                self.start(else_label);
                let else_volatile = else_expr.ty().is_some_and(|ty| ty.is_volatile());
                let else_src = self.record_ptr(else_expr);
                self.copy_struct(&ty, &else_src, &tmp, else_volatile);
                self.br(&end_label);
                self.start(end_label);
                tmp
            }
            Expr::Comma { left, right, .. } => {
                let _ = self.gen_expr(left);
                self.record_ptr(right)
            }
            Expr::StmtExpr { body, ty, .. } => {
                let ty = ty.clone().expect("typed statement expression");
                let Some(last) = self.gen_stmt_expr_body(body) else {
                    return self.ice_in(
                        expr,
                        "a struct or union statement expression does not end in an expression",
                    );
                };
                let volatile = last.ty().is_some_and(|ty| ty.is_volatile());
                let src = self.record_ptr(last);
                self.ensure_block();
                // The block's objects end with it; the value is a copy.
                let tmp = self.entry_alloca(&ty);
                self.copy_struct(&ty, &src, &tmp, volatile);
                tmp
            }
            _ => self.ice_in(expr, "struct or union expression has no address"),
        }
    }

    pub(super) fn load_bitfield(&mut self, expr: &Expr) -> String {
        let addr = self.gen_addr(expr);
        self.load_bitfield_at(&addr, expr)
    }

    pub(super) fn load_bitfield_at(&mut self, addr: &str, expr: &Expr) -> String {
        let Some(info) = bit_info(expr) else {
            return self.ice_in(
                expr,
                "bit-field load from an expression that is not a bit-field",
            );
        };
        let container = container_type(
            info.container_bits,
            info.signed_field,
            expr.ty().expect("typed").quals(),
        );
        let raw = self.load(&container, addr);
        let shift = info.abs_bit - (info.container_off as u32) * 8;
        let shifted = if shift == 0 {
            raw
        } else {
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = lshr {} {raw}, {shift}",
                container.ir_type()
            ));
            tmp
        };
        let mask = if info.width >= 64 {
            u64::MAX
        } else {
            (1u64 << info.width) - 1
        };
        let masked = self.fresh();
        self.emit(&format!(
            "  {masked} = and {} {shifted}, {mask}",
            container.ir_type()
        ));
        let value_ty = expr.ty().expect("bit-field value type");
        self.finish_bitfield(&masked, &info, &container, &value_ty)
    }

    /// The value of a bit-field after it is stored: the written bits, masked
    /// to the field width, sign-extended when the field is signed, then
    /// converted to the promoted type. C11 6.5.16 says an assignment
    /// expression has that value. Reloading the container would be a second
    /// volatile access.
    pub(super) fn finish_bitfield(
        &mut self,
        masked: &str,
        info: &BitInfo,
        container: &CType,
        value_ty: &CType,
    ) -> String {
        let field = if info.signed_field && info.width < info.container_bits {
            let up = info.container_bits - info.width;
            let hi = self.fresh();
            let lo = self.fresh();
            self.emit(&format!(
                "  {hi} = shl {} {masked}, {up}",
                container.ir_type()
            ));
            self.emit(&format!("  {lo} = ashr {} {hi}, {up}", container.ir_type()));
            lo
        } else {
            masked.to_string()
        };
        self.convert(&field, container, value_ty)
    }

    pub(super) fn write_bits(
        &mut self,
        addr: &str,
        info: &BitStore,
        value_ty: &CType,
        value: &str,
    ) {
        let container = container_type(info.container_bits, info.signed_field, value_ty.quals());
        let raw = self.load(&container, addr);
        let narrow = self.convert(value, value_ty, &container);
        let shift = info.abs_bit % info.container_bits;
        let mask = width_mask(info.width);
        let masked = self.fresh();
        self.emit(&format!(
            "  {masked} = and {} {narrow}, {mask}",
            container.ir_type()
        ));
        let placed = if shift == 0 {
            masked
        } else {
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = shl {} {masked}, {shift}",
                container.ir_type()
            ));
            tmp
        };
        let clear_mask = (!((mask as u128) << shift)) & width_mask_u128(info.container_bits);
        let cleared = self.fresh();
        self.emit(&format!(
            "  {cleared} = and {} {raw}, {clear_mask}",
            container.ir_type()
        ));
        let merged = self.fresh();
        self.emit(&format!(
            "  {merged} = or {} {cleared}, {placed}",
            container.ir_type()
        ));
        self.store(&container, &merged, addr);
    }

    pub(super) fn store_bitfield(
        &mut self,
        addr: &str,
        expr: &Expr,
        value: &str,
        from: &CType,
    ) -> String {
        let Some(info) = bit_info(expr) else {
            return self.ice_in(
                expr,
                "bit-field store to an expression that is not a bit-field",
            );
        };
        let container = container_type(
            info.container_bits,
            info.signed_field,
            expr.ty().expect("typed").quals(),
        );
        let raw = self.load(&container, addr);
        let value_ty = expr.ty().expect("bit-field value type");
        // Assignment converts to the declared type. `_Bool` is 0 or 1, so
        // masking the original bits would turn 2 into 0.
        let declared_ty = CType::from_name(info.declared);
        let as_declared = self.convert(value, from, &declared_ty);
        let from_ty = declared_ty;
        let narrow = self.convert(&as_declared, &from_ty, &container);
        let shift = info.abs_bit - (info.container_off as u32) * 8;
        let mask = width_mask(info.width);
        let masked = self.fresh();
        self.emit(&format!(
            "  {masked} = and {} {narrow}, {mask}",
            container.ir_type()
        ));
        let placed = if shift == 0 {
            masked.clone()
        } else {
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = shl {} {masked}, {shift}",
                container.ir_type()
            ));
            tmp
        };
        let clear_mask = (!((mask as u128) << shift)) & width_mask_u128(info.container_bits);
        let cleared = self.fresh();
        self.emit(&format!(
            "  {cleared} = and {} {raw}, {clear_mask}",
            container.ir_type()
        ));
        let merged = self.fresh();
        self.emit(&format!(
            "  {merged} = or {} {cleared}, {placed}",
            container.ir_type()
        ));
        self.store(&container, &merged, addr);
        self.finish_bitfield(&masked, &info, &container, &value_ty)
    }

    pub(super) fn store(&mut self, ty: &CType, value: &str, slot: &str) {
        let align = self.access_align(ty, slot);
        self.emit(&format!(
            "  store{} {} {value}, ptr {slot}, align {align}",
            vol_attr(ty),
            ty.ir_type(),
        ));
    }

    pub(super) fn load(&mut self, ty: &CType, slot: &str) -> String {
        let tmp = self.fresh();
        let align = self.access_align(ty, slot);
        self.emit(&format!(
            "  {tmp} = load{} {}, ptr {slot}, align {align}",
            vol_attr(ty),
            ty.ir_type(),
        ));
        tmp
    }

    /// The alignment of an access through `slot`: the type's, or 1 for a
    /// member of a packed record.
    pub(super) fn access_align(&self, ty: &CType, slot: &str) -> u32 {
        if self.unaligned.contains(slot) {
            1
        } else {
            ty.align()
        }
    }

    /// Convert `value` from `from` to `to`.
    ///
    /// Integer conversions are `sext`, `zext`, or `trunc`. Same-width
    /// signed/unsigned changes need no instruction. Pointer-to-pointer is a
    /// no-op with opaque pointers. Integer-to-pointer sign- or zero-extends
    /// to i64 and uses `inttoptr`; a constant 0 becomes `null`. Pointer-to-
    /// integer is `ptrtoint` to the destination width. That is the
    /// implementation-defined conversion Clang uses on x86_64-pc-linux-gnu.
    pub(super) fn convert(&mut self, value: &str, from: &CType, to: &CType) -> String {
        // C11 6.3.1.2: a pointer converts to `_Bool` as 0 when null and 1
        // otherwise. Truncating the address to a byte is not that conversion.
        if from.is_pointer() && to.as_integer() == Some(TypeName::Bool) {
            let cmp = self.fresh();
            let tmp = self.fresh();
            self.emit(&format!("  {cmp} = icmp ne ptr {value}, null"));
            self.emit(&format!("  {tmp} = zext i1 {cmp} to i8"));
            return tmp;
        }
        if let (Some(from_i), Some(to_i)) = (from.as_integer(), to.as_integer()) {
            // `_Bool` is 0 or 1. A same-width truncation would keep the low
            // byte, so `( _Bool )2` would stay 2.
            if to_i == TypeName::Bool && from_i != TypeName::Bool {
                let cmp = self.fresh();
                let tmp = self.fresh();
                self.emit(&format!(
                    "  {cmp} = icmp ne {} {value}, 0",
                    from_i.ir_type()
                ));
                self.emit(&format!("  {tmp} = zext i1 {cmp} to i8"));
                return tmp;
            }
            if from_i.width() == to_i.width() {
                return value.to_string();
            }
            let op = if to_i.width() < from_i.width() {
                "trunc"
            } else if from_i.is_signed() {
                "sext"
            } else {
                "zext"
            };
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = {op} {} {value} to {}",
                from_i.ir_type(),
                to_i.ir_type()
            ));
            return tmp;
        }
        if let (Some(from_f), Some(to_f)) = (from.as_float(), to.as_float()) {
            if from_f == to_f || from.ir_type() == to.ir_type() {
                return value.to_string();
            }
            let op = if float_rank(to_f) > float_rank(from_f) {
                "fpext"
            } else {
                "fptrunc"
            };
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = {op} {} {value} to {}",
                from.ir_type(),
                to.ir_type()
            ));
            return tmp;
        }
        if from.is_pointer() && to.is_pointer() {
            return value.to_string();
        }
        if from.is_integer() && to.is_pointer() {
            if value == "0" {
                return "null".to_string();
            }
            let wide = self.convert(value, from, &CType::from_name(TypeName::Long));
            let tmp = self.fresh();
            self.emit(&format!("  {tmp} = inttoptr i64 {wide} to ptr"));
            return tmp;
        }
        if from.is_pointer() && to.is_integer() {
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = ptrtoint ptr {value} to {}",
                to.ir_type()
            ));
            return tmp;
        }
        if let (Some(kind), Some(TypeName::Bool)) = (from.float_kind(), to.as_integer()) {
            let cmp = self.fresh();
            let tmp = self.fresh();
            let zero = crate::float::llvm_const(kind, 0);
            self.emit(&format!(
                "  {cmp} = fcmp une {} {value}, {zero}",
                from.ir_type()
            ));
            self.emit(&format!("  {tmp} = zext i1 {cmp} to i8"));
            return tmp;
        }
        if from.is_float() && to.is_integer() {
            let op = if to.as_integer().is_some_and(TypeName::is_signed) {
                "fptosi"
            } else {
                "fptoui"
            };
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = {op} {} {value} to {}",
                from.ir_type(),
                to.ir_type()
            ));
            return tmp;
        }
        if from.is_integer() && to.is_float() {
            let op = if from.as_integer().is_some_and(TypeName::is_signed) {
                "sitofp"
            } else {
                "uitofp"
            };
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = {op} {} {value} to {}",
                from.ir_type(),
                to.ir_type()
            ));
            return tmp;
        }
        value.to_string()
    }

    pub(super) fn extend_i64(&mut self, value: &str, ty: &CType) -> String {
        self.convert(value, ty, &CType::from_name(TypeName::Long))
    }

    /// A pointer operand, or a null pointer constant turned into `ptr null`.
    pub(super) fn as_ptr(&mut self, expr: &Expr) -> String {
        let value = self.gen_val(expr);
        let ty = expr.ty().expect("typed");
        if ty.is_pointer() {
            value
        } else {
            self.convert(&value, &ty, &CType::pointer(CType::Void))
        }
    }
}

pub(super) struct BitInfo {
    pub(super) width: u32,
    pub(super) signed_field: bool,
    pub(super) container_bits: u32,
    pub(super) abs_bit: u32,
    pub(super) container_off: u64,
    pub(super) declared: TypeName,
}

pub(super) fn bit_info(expr: &Expr) -> Option<BitInfo> {
    match expr {
        Expr::Member {
            bit_width: Some(width),
            bit_signed,
            container_bits,
            abs_bit,
            bit_declared,
            offset,
            ..
        } => Some(BitInfo {
            width: *width,
            signed_field: *bit_signed,
            container_bits: *container_bits,
            abs_bit: *abs_bit,
            container_off: offset.unwrap_or(0),
            declared: *bit_declared,
        }),
        _ => None,
    }
}

pub(super) fn container_type(bits: u32, signed_field: bool, quals: Quals) -> CType {
    let name = match (bits, signed_field) {
        (8, true) => TypeName::SChar,
        (8, false) => TypeName::UChar,
        (16, true) => TypeName::Short,
        (16, false) => TypeName::UShort,
        (32, true) => TypeName::Int,
        (32, false) => TypeName::UInt,
        (_, true) => TypeName::Long,
        (_, false) => TypeName::ULong,
    };
    CType::from_name(name).with_quals(quals)
}

pub(super) fn width_mask(bits: u32) -> u64 {
    if bits >= 64 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
    }
}

pub(super) fn width_mask_u128(bits: u32) -> u128 {
    if bits >= 128 {
        u128::MAX
    } else {
        (1u128 << bits) - 1
    }
}
