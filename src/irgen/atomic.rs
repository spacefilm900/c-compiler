//! Atomic loads, stores, and read-modify-write operations (C11 6.2.6.1p9,
//! 6.5.2.4, 6.5.16.2). Language accesses use `memory_order_seq_cst`.
//!
//! Lock-free sizes are 1, 2, 4, 8, and 16 bytes on every target this
//! compiler supports. Larger atomic types are diagnosed in semantic
//! analysis; this module does not lower them.

use super::*;

/// Integer width LLVM uses for an atomic access of `size` bytes.
pub(super) fn atomic_int_ir(size: u64) -> Option<&'static str> {
    match size {
        1 => Some("i8"),
        2 => Some("i16"),
        4 => Some("i32"),
        8 => Some("i64"),
        16 => Some("i128"),
        _ => None,
    }
}

/// LLVM type of an atomic memory access for `ty`. Aggregates and
/// 16-byte `long double` are accessed as an integer of the same size,
/// because LLVM does not accept `load atomic` of a struct or `x86_fp80`.
pub(super) fn atomic_mem_ir(ty: &CType) -> String {
    let size = ty.size().expect("complete atomic");
    if ty.is_record() || needs_int_bits(ty) {
        atomic_int_ir(size)
            .expect("lock-free atomic size")
            .to_string()
    } else {
        ty.ir_type()
    }
}

fn needs_int_bits(ty: &CType) -> bool {
    matches!(ty.float_kind(), Some(FloatKind::LongDouble)) && ty.size() == Some(16)
}

impl Gen {
    /// Seq_cst atomic load of an object of type `ty`.
    pub(super) fn atomic_load(&mut self, ty: &CType, slot: &str) -> String {
        let mem = atomic_mem_ir(ty);
        let bits = self.atomic_load_mem(&mem, slot, self.access_align(ty, slot), ty.is_volatile());
        self.bits_to_value(&bits, &mem, ty)
    }

    /// Seq_cst atomic store of `value`, which has type `ty`.
    pub(super) fn atomic_store(&mut self, ty: &CType, value: &str, slot: &str) {
        let mem = atomic_mem_ir(ty);
        let bits = self.value_to_bits(value, ty, &mem);
        self.atomic_store_mem(
            &mem,
            &bits,
            slot,
            self.access_align(ty, slot),
            ty.is_volatile(),
        );
    }

    fn atomic_load_mem(&mut self, mem: &str, slot: &str, align: u32, volatile: bool) -> String {
        let vol = if volatile { " volatile" } else { "" };
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = load atomic{vol} {mem}, ptr {slot} seq_cst, align {align}"
        ));
        tmp
    }

    fn atomic_store_mem(&mut self, mem: &str, value: &str, slot: &str, align: u32, volatile: bool) {
        let vol = if volatile { " volatile" } else { "" };
        self.emit(&format!(
            "  store atomic{vol} {mem} {value}, ptr {slot} seq_cst, align {align}"
        ));
    }

    /// Copy `size` bytes from `src` to `dest` as one lock-free atomic
    /// transfer when either side is an atomic object.
    pub(super) fn atomic_copy(
        &mut self,
        ty: &CType,
        src: &str,
        dest: &str,
        src_atomic: bool,
        src_volatile: bool,
    ) {
        let size = ty.size().expect("complete");
        let mem = atomic_int_ir(size).expect("lock-free atomic size");
        let align = ty.align();
        let bits = if src_atomic {
            self.atomic_load_mem(mem, src, align, src_volatile || ty.is_volatile())
        } else {
            let tmp = self.fresh();
            let vol = if src_volatile { " volatile" } else { "" };
            self.emit(&format!(
                "  {tmp} = load{vol} {mem}, ptr {src}, align {align}"
            ));
            tmp
        };
        if ty.is_atomic() {
            self.atomic_store_mem(mem, &bits, dest, align, ty.is_volatile());
        } else {
            let vol = if ty.is_volatile() { " volatile" } else { "" };
            self.emit(&format!(
                "  store{vol} {mem} {bits}, ptr {dest}, align {align}"
            ));
        }
    }

    /// Compound assignment `*target op= value` on an atomic lvalue.
    /// The result is the new value (C11 6.5.16.2 footnote 113).
    pub(super) fn gen_atomic_compound(&mut self, op: BinOp, target: &Expr, value: &Expr) -> String {
        let dest = target.ty().expect("lvalue type");
        let rhs_ty = value.ty().expect("typed");
        let rhs = self.gen_val(value);
        let addr = self.gen_addr(target);
        let (_, new) = self.atomic_rmw_loop(&dest, &addr, dest.is_volatile(), |gen, old| {
            gen.atomic_apply_op(op, &dest, old, &rhs_ty, &rhs, target)
        });
        new
    }

    /// `++` / `--` on an atomic lvalue. Prefix returns the new value;
    /// postfix returns the old value (C11 6.5.2.4 footnote 98; 6.5.3.1).
    pub(super) fn gen_atomic_incdec(&mut self, op: IncOp, target: &Expr) -> String {
        let dest = target.ty().expect("lvalue type");
        let addr = self.gen_addr(target);
        let one_op = if op.is_increment() {
            BinOp::Add
        } else {
            BinOp::Sub
        };
        let (old, new) = self.atomic_rmw_loop(&dest, &addr, dest.is_volatile(), |gen, old| {
            if dest.is_pointer() {
                let one = "1";
                let rhs_ty = CType::from_name(TypeName::Int);
                gen.atomic_apply_op(one_op, &dest, old, &rhs_ty, one, target)
            } else if let Some(kind) = dest.float_kind() {
                let one = crate::float::llvm_const(kind, crate::float::one_bits(kind));
                gen.atomic_apply_op(one_op, &dest, old, &dest.non_atomic(), &one, target)
            } else {
                let one_ty = dest.unqualified();
                let one = "1";
                let rhs_ty = CType::from_name(TypeName::Int);
                let _ = one_ty;
                gen.atomic_apply_op(one_op, &dest, old, &rhs_ty, one, target)
            }
        });
        if op.is_prefix() {
            new
        } else {
            old
        }
    }

    /// Compare-exchange loop implementing `new = old op val` on `addr`.
    /// Returns `(old, new)` of the successful iteration, as values of `ty`.
    fn atomic_rmw_loop(
        &mut self,
        ty: &CType,
        addr: &str,
        volatile: bool,
        mut compute: impl FnMut(&mut Self, &str) -> String,
    ) -> (String, String) {
        let mem = atomic_mem_ir(ty);
        let align = ty.align();
        let first = self.atomic_load_mem(&mem, addr, align, volatile);
        let entry = self.cur.clone();
        let loop_l = self.label();
        let done_l = self.label();
        let old_bits = self.fresh();
        let got = self.fresh();
        self.br(&loop_l);
        self.start(loop_l.clone());
        self.emit(&format!(
            "  {old_bits} = phi {mem} [ {first}, %{entry} ], [ {got}, %{loop_l} ]"
        ));
        let old = self.bits_to_value(&old_bits, &mem, ty);
        let new = compute(self, &old);
        let new_bits = self.value_to_bits(&new, ty, &mem);
        let res = self.fresh();
        let vol = if volatile { " volatile" } else { "" };
        self.emit(&format!(
            "  {res} = cmpxchg{vol} ptr {addr}, {mem} {old_bits}, {mem} {new_bits} seq_cst seq_cst"
        ));
        self.emit(&format!("  {got} = extractvalue {{{mem}, i1}} {res}, 0"));
        let ok = self.fresh();
        self.emit(&format!("  {ok} = extractvalue {{{mem}, i1}} {res}, 1"));
        self.br_cond(&ok, &done_l, &loop_l);
        self.start(done_l);
        (old, new)
    }

    /// `new = old op val` converted back to `dest`'s type, matching
    /// ordinary compound assignment.
    fn atomic_apply_op(
        &mut self,
        op: BinOp,
        dest: &CType,
        old: &str,
        rhs_ty: &CType,
        rhs: &str,
        at: &Expr,
    ) -> String {
        if let Some(pointee) = dest.pointee() {
            let mut idx = self.extend_i64(rhs, rhs_ty);
            if op == BinOp::Sub {
                let tmp = self.fresh();
                self.emit(&format!("  {tmp} = sub i64 0, {idx}"));
                idx = tmp;
            }
            if pointee.size().is_none() {
                return self.gen_vla_gep(old, &idx, pointee, self.unaligned.contains(old));
            }
            let elem = pointee.ir_type();
            let updated = self.fresh();
            self.emit(&format!(
                "  {updated} = getelementptr inbounds {elem}, ptr {old}, i64 {idx}"
            ));
            return updated;
        }
        if dest.is_float() || rhs_ty.is_float() {
            let common = common_real(dest, rhs_ty);
            let old_c = self.convert(old, dest, &common);
            let rhs_c = self.convert(rhs, rhs_ty, &common);
            let updated = self.fresh();
            self.emit(&format!(
                "  {updated} = {} {} {old_c}, {rhs_c}",
                float_inst(op),
                common.ir_type()
            ));
            return self.convert(&updated, &common, dest);
        }
        let (Some(dest_i), Some(rhs_i)) = (dest.as_integer(), rhs_ty.as_integer()) else {
            return self.ice_in(at, "atomic compound assignment operands are not arithmetic");
        };
        let common = if matches!(op, BinOp::Shl | BinOp::Shr) {
            dest_i.promote()
        } else {
            dest_i.usual(rhs_i)
        };
        let old_c = self.convert(old, dest, &CType::from_name(common));
        let rhs_c = self.convert(rhs, rhs_ty, &CType::from_name(common));
        let updated = self.fresh();
        let inst = int_inst(op, common);
        self.emit(&format!(
            "  {updated} = {inst} {} {old_c}, {rhs_c}",
            common.ir_type()
        ));
        self.convert(&updated, &CType::from_name(common), dest)
    }

    fn bits_to_value(&mut self, bits: &str, mem: &str, ty: &CType) -> String {
        if mem == ty.ir_type() {
            return bits.to_string();
        }
        let tmp = self.entry_alloca_ir(mem, ty.align());
        self.emit(&format!(
            "  store {mem} {bits}, ptr {tmp}, align {}",
            ty.align()
        ));
        let val = self.fresh();
        self.emit(&format!(
            "  {val} = load {}, ptr {tmp}, align {}",
            ty.ir_type(),
            ty.align()
        ));
        val
    }

    fn value_to_bits(&mut self, value: &str, ty: &CType, mem: &str) -> String {
        if mem == ty.ir_type() {
            return value.to_string();
        }
        let tmp = self.entry_alloca_ir(mem, ty.align());
        self.emit(&format!(
            "  store {} {value}, ptr {tmp}, align {}",
            ty.ir_type(),
            ty.align()
        ));
        let bits = self.fresh();
        self.emit(&format!(
            "  {bits} = load {mem}, ptr {tmp}, align {}",
            ty.align()
        ));
        bits
    }
}

fn int_inst(op: BinOp, ty: TypeName) -> &'static str {
    match op {
        BinOp::Add => "add",
        BinOp::Sub => "sub",
        BinOp::Mul => "mul",
        BinOp::Div if ty.is_signed() => "sdiv",
        BinOp::Div => "udiv",
        BinOp::Mod if ty.is_signed() => "srem",
        BinOp::Mod => "urem",
        BinOp::Shl => "shl",
        BinOp::Shr if ty.is_signed() => "ashr",
        BinOp::Shr => "lshr",
        BinOp::BitAnd => "and",
        BinOp::BitOr => "or",
        BinOp::BitXor => "xor",
        _ => "add",
    }
}
