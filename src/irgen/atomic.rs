//! Atomic loads, stores, and read-modify-write operations (C11 6.2.6.1p9,
//! 6.5.2.4, 6.5.16.2, 7.17). Language accesses use `memory_order_seq_cst`.
//! Explicit orders come from the `__hcc_atomic_*` builtins in <stdatomic.h>.
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
    // LLVM's x86 backend cannot select `cmpxchg` of `float` or `double`,
    // and `load atomic` of `x86_fp80` is invalid. Every atomic floating
    // access uses an integer of the same size.
    ty.is_float()
}

impl Gen {
    /// Seq_cst atomic load of an object of type `ty`.
    pub(super) fn atomic_load(&mut self, ty: &CType, slot: &str) -> String {
        let mem = atomic_mem_ir(ty);
        let bits = self.atomic_load_mem(
            &mem,
            slot,
            self.access_align(ty, slot),
            ty.is_volatile(),
            "seq_cst",
        );
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
            "seq_cst",
        );
    }

    fn atomic_load_mem(
        &mut self,
        mem: &str,
        slot: &str,
        align: u32,
        volatile: bool,
        order: &str,
    ) -> String {
        self.note_wide_atomic_mem(mem);
        let vol = if volatile { " volatile" } else { "" };
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = load atomic{vol} {mem}, ptr {slot} {order}, align {align}"
        ));
        tmp
    }

    fn atomic_store_mem(
        &mut self,
        mem: &str,
        value: &str,
        slot: &str,
        align: u32,
        volatile: bool,
        order: &str,
    ) {
        self.note_wide_atomic_mem(mem);
        let vol = if volatile { " volatile" } else { "" };
        self.emit(&format!(
            "  store atomic{vol} {mem} {value}, ptr {slot} {order}, align {align}"
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
        dest_atomic: bool,
        src_volatile: bool,
    ) {
        let size = ty.size().expect("complete");
        let mem = atomic_int_ir(size).expect("lock-free atomic size");
        // A non-atomic operand may have a weaker alignment than `ty` when
        // `ty` is the atomic destination (for example `_Atomic struct S x = s`).
        let src_ty = if src_atomic {
            ty.clone()
        } else {
            ty.non_atomic()
        };
        let dest_ty = if dest_atomic {
            ty.clone()
        } else {
            ty.non_atomic()
        };
        let src_align = self.access_align(&src_ty, src);
        let dest_align = self.access_align(&dest_ty, dest);
        if src_atomic && (src_align as u64) < size {
            self.ice("atomic object is not aligned to its size");
        }
        if dest_atomic && (dest_align as u64) < size {
            self.ice("atomic object is not aligned to its size");
        }
        let bits = if src_atomic {
            self.atomic_load_mem(
                mem,
                src,
                src_align,
                src_volatile || ty.is_volatile(),
                "seq_cst",
            )
        } else {
            let tmp = self.fresh();
            let vol = if src_volatile { " volatile" } else { "" };
            self.emit(&format!(
                "  {tmp} = load{vol} {mem}, ptr {src}, align {src_align}"
            ));
            tmp
        };
        if dest_atomic {
            self.atomic_store_mem(mem, &bits, dest, dest_align, ty.is_volatile(), "seq_cst");
        } else {
            let vol = if ty.is_volatile() { " volatile" } else { "" };
            self.emit(&format!(
                "  store{vol} {mem} {bits}, ptr {dest}, align {dest_align}"
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
        if let Some((_, new)) =
            self.try_integer_atomicrmw(op, &dest, &addr, &rhs_ty, &rhs, "seq_cst")
        {
            return new;
        }
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
        if dest.is_integer() && dest.as_integer() != Some(TypeName::Bool) {
            let one_ty = CType::from_name(TypeName::Int);
            if let Some((old, new)) =
                self.try_integer_atomicrmw(one_op, &dest, &addr, &one_ty, "1", "seq_cst")
            {
                return if op.is_prefix() { new } else { old };
            }
        }
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
        compute: impl FnMut(&mut Self, &str) -> String,
    ) -> (String, String) {
        self.atomic_rmw_loop_orders(ty, addr, volatile, "seq_cst", "seq_cst", false, compute)
    }

    #[allow(clippy::too_many_arguments)]
    fn atomic_rmw_loop_orders(
        &mut self,
        ty: &CType,
        addr: &str,
        volatile: bool,
        success: &str,
        failure: &str,
        weak: bool,
        mut compute: impl FnMut(&mut Self, &str) -> String,
    ) -> (String, String) {
        let mem = atomic_mem_ir(ty);
        let align = ty.align();
        let load_ord = load_order_for(success);
        let first = self.atomic_load_mem(&mem, addr, align, volatile, load_ord);
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
        let weak_kw = if weak { " weak" } else { "" };
        self.note_wide_atomic_mem(&mem);
        self.emit(&format!(
            "  {res} = cmpxchg{weak_kw}{vol} ptr {addr}, {mem} {old_bits}, {mem} {new_bits} {success} {failure}"
        ));
        self.emit(&format!("  {got} = extractvalue {{{mem}, i1}} {res}, 0"));
        let ok = self.fresh();
        self.emit(&format!("  {ok} = extractvalue {{{mem}, i1}} {res}, 1"));
        self.br_cond(&ok, &done_l, &loop_l);
        self.start(done_l);
        (old, new)
    }

    /// Scaled pointer offset for atomic `+=`/`-=` and fetch-add/sub.
    /// `inbounds` is true for language arithmetic on a VLA pointee and
    /// false for the fetch builtins, which C11 7.17.7.5 defines as
    /// integer arithmetic on `uintptr_t`.
    fn atomic_ptr_offset(
        &mut self,
        op: BinOp,
        pointee: &CType,
        old: &str,
        rhs_ty: &CType,
        rhs: &str,
        inbounds: bool,
    ) -> String {
        let mut idx = self.extend_i64(rhs, rhs_ty);
        if op == BinOp::Sub {
            let tmp = self.fresh();
            self.emit(&format!("  {tmp} = sub i64 0, {idx}"));
            idx = tmp;
        }
        if pointee.size().is_none() {
            return self.gen_vla_gep(old, &idx, pointee, self.unaligned.contains(old), inbounds);
        }
        let elem = pointee.ir_type();
        let updated = self.fresh();
        self.emit(&format!(
            "  {updated} = getelementptr {elem}, ptr {old}, i64 {idx}"
        ));
        updated
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
            return self.atomic_ptr_offset(op, pointee, old, rhs_ty, rhs, true);
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
        let val = if mem == ty.ir_type() {
            bits.to_string()
        } else {
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
        };
        // Copy the value through a non-atomic slot so a later use is an
        // ordinary load. Passing a `load atomic` result directly as a
        // variadic argument can be mis-lowered by LLVM's
        // arm64-apple-darwin-elf backend (the Apple calling-convention
        // stand-in), which then prints a stale stack word instead of the
        // loaded value.
        self.materialize_scalar(&ty.unqualified(), &val)
    }

    /// Store `value` to a fresh slot of non-atomic type `ty` and load it
    /// back as an ordinary value.
    fn materialize_scalar(&mut self, ty: &CType, value: &str) -> String {
        if ty.is_record() {
            return value.to_string();
        }
        let slot = self.entry_alloca(ty);
        self.store(ty, value, &slot);
        self.load(ty, &slot)
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

    /// Integer `atomicrmw` for add, sub, and, or, xor when both operands
    /// are integers. The result is `(old, new)` in `dest`'s type, or
    /// `None` when a compare-exchange loop must be used instead.
    fn try_integer_atomicrmw(
        &mut self,
        op: BinOp,
        dest: &CType,
        addr: &str,
        rhs_ty: &CType,
        rhs: &str,
        order: &str,
    ) -> Option<(String, String)> {
        let inst = rmw_inst(op)?;
        if !dest.is_integer() || dest.as_integer() == Some(TypeName::Bool) {
            return None;
        }
        if rhs_ty.is_float() || rhs_ty.is_pointer() || !rhs_ty.is_integer() {
            return None;
        }
        let dest_i = dest.as_integer()?;
        let rhs_c = self.convert(rhs, rhs_ty, &dest.unqualified());
        let mem = atomic_mem_ir(dest);
        let vol = if dest.is_volatile() { " volatile" } else { "" };
        let old = self.fresh();
        self.note_wide_atomic_mem(&mem);
        self.emit(&format!(
            "  {old} = atomicrmw{vol} {inst} ptr {addr}, {mem} {rhs_c} {order}"
        ));
        let bin = int_inst(op, dest_i);
        let new = self.fresh();
        self.emit(&format!("  {new} = {bin} {mem} {old}, {rhs_c}"));
        let dest = dest.unqualified();
        Some((
            self.materialize_scalar(&dest, &old),
            self.materialize_scalar(&dest, &new),
        ))
    }

    /// Lower a `__hcc_atomic_*` builtin. `None` is a void result.
    pub(super) fn gen_atomic_builtin(
        &mut self,
        op: crate::builtins::AtomicOp,
        args: &[Expr],
    ) -> Option<String> {
        use crate::builtins::AtomicOp;
        match op {
            AtomicOp::ThreadFence => {
                emit_fence(self, args, false);
                None
            }
            AtomicOp::SignalFence => {
                emit_fence(self, args, true);
                None
            }
            AtomicOp::IsLockFree => {
                let _ = self.gen_val(&args[0]);
                let pointee = args[0]
                    .ty()
                    .and_then(|ty| ty.pointee().cloned())
                    .expect("atomic_is_lock_free pointer");
                let ok = pointee.atomic_is_lock_free();
                Some(if ok { "1" } else { "0" }.to_string())
            }
            AtomicOp::Init => {
                let (obj, addr) = atomic_obj(self, &args[0]);
                let mem = atomic_mem_ir(&obj);
                let bits = self.expr_to_atomic_bits(&args[1], &obj, &mem);
                let vol = if obj.is_volatile() { " volatile" } else { "" };
                self.emit(&format!(
                    "  store{vol} {mem} {bits}, ptr {addr}, align {}",
                    obj.align()
                ));
                None
            }
            AtomicOp::Store => {
                let (obj, addr) = atomic_obj(self, &args[0]);
                let order = self.store_llvm_order(&args[2]);
                let mem = atomic_mem_ir(&obj);
                let bits = self.expr_to_atomic_bits(&args[1], &obj, &mem);
                self.atomic_store_mem(
                    &mem,
                    &bits,
                    &addr,
                    self.access_align(&obj, &addr),
                    obj.is_volatile(),
                    order,
                );
                None
            }
            AtomicOp::FlagClear => {
                let (obj, addr) = atomic_obj(self, &args[0]);
                let order = self.flag_clear_llvm_order(&args[1]);
                let mem = atomic_mem_ir(&obj);
                self.atomic_store_mem(
                    &mem,
                    "0",
                    &addr,
                    self.access_align(&obj, &addr),
                    obj.is_volatile(),
                    order,
                );
                None
            }
            AtomicOp::Load => {
                let (obj, addr) = atomic_obj(self, &args[0]);
                let order = self.load_llvm_order(&args[1]);
                Some(self.atomic_load_ordered(&obj, &addr, order))
            }
            AtomicOp::Exchange => {
                let (obj, addr) = atomic_obj(self, &args[0]);
                let order = self.rmw_llvm_order(&args[2]);
                let mem = atomic_mem_ir(&obj);
                let bits = self.expr_to_atomic_bits(&args[1], &obj, &mem);
                let vol = if obj.is_volatile() { " volatile" } else { "" };
                let old_bits = self.fresh();
                self.note_wide_atomic_mem(&mem);
                self.emit(&format!(
                    "  {old_bits} = atomicrmw{vol} xchg ptr {addr}, {mem} {bits} {order}"
                ));
                Some(self.bits_to_value(&old_bits, &mem, &obj))
            }
            AtomicOp::CompareExchange => Some(self.gen_atomic_cmpxchg(args)),
            AtomicOp::FetchAdd
            | AtomicOp::FetchSub
            | AtomicOp::FetchOr
            | AtomicOp::FetchXor
            | AtomicOp::FetchAnd => Some(self.gen_atomic_fetch(op, args)),
        }
    }

    /// A struct or union result of an atomic builtin, left in a fresh slot.
    pub(super) fn gen_atomic_builtin_record(
        &mut self,
        op: crate::builtins::AtomicOp,
        args: &[Expr],
        ty: &CType,
    ) -> String {
        use crate::builtins::AtomicOp;
        let slot = self.entry_alloca(ty);
        let (obj, addr) = atomic_obj(self, &args[0]);
        let mem = atomic_mem_ir(&obj);
        let bits = match op {
            AtomicOp::Load => {
                let order = self.load_llvm_order(&args[1]);
                self.atomic_load_mem(
                    &mem,
                    &addr,
                    self.access_align(&obj, &addr),
                    obj.is_volatile(),
                    order,
                )
            }
            AtomicOp::Exchange => {
                let order = self.rmw_llvm_order(&args[2]);
                let new_bits = self.expr_to_atomic_bits(&args[1], &obj, &mem);
                let vol = if obj.is_volatile() { " volatile" } else { "" };
                let old_bits = self.fresh();
                self.note_wide_atomic_mem(&mem);
                self.emit(&format!(
                    "  {old_bits} = atomicrmw{vol} xchg ptr {addr}, {mem} {new_bits} {order}"
                ));
                old_bits
            }
            _ => self.ice_in(&args[0], "atomic builtin does not return a record"),
        };
        self.emit(&format!(
            "  store {mem} {bits}, ptr {slot}, align {}",
            ty.align()
        ));
        slot
    }

    fn atomic_load_ordered(&mut self, ty: &CType, slot: &str, order: &str) -> String {
        let mem = atomic_mem_ir(ty);
        let bits = self.atomic_load_mem(&mem, slot, ty.align(), ty.is_volatile(), order);
        self.bits_to_value(&bits, &mem, ty)
    }

    fn expr_to_atomic_bits(&mut self, expr: &Expr, dest: &CType, mem: &str) -> String {
        let dest_u = dest.unqualified();
        if dest_u.is_record() {
            let addr = self.record_ptr(expr);
            let src_ty = expr.ty().expect("typed");
            let align = self.access_align(&src_ty, &addr);
            if is_atomic_object(expr) {
                self.atomic_load_mem(mem, &addr, align, src_ty.is_volatile(), "seq_cst")
            } else {
                let tmp = self.fresh();
                let vol = if src_ty.is_volatile() {
                    " volatile"
                } else {
                    ""
                };
                self.emit(&format!(
                    "  {tmp} = load{vol} {mem}, ptr {addr}, align {align}"
                ));
                tmp
            }
        } else {
            let value = self.gen_val(expr);
            let from = expr.ty().expect("typed");
            let converted = self.convert(&value, &from, &dest_u);
            self.value_to_bits(&converted, &dest_u, mem)
        }
    }

    fn gen_atomic_cmpxchg(&mut self, args: &[Expr]) -> String {
        let (obj, addr) = atomic_obj(self, &args[0]);
        let expected = self.gen_val(&args[1]);
        let success = self.rmw_llvm_order(&args[3]);
        let failure = self.failure_llvm_order(&args[4]);
        let weak = fold_order_code(&args[5]).unwrap_or(0) != 0;
        let mem = atomic_mem_ir(&obj);
        let desired = self.expr_to_atomic_bits(&args[2], &obj, &mem);
        let exp_ty = args[1]
            .ty()
            .and_then(|ty| ty.pointee().cloned())
            .expect("expected pointer");
        let exp_align = exp_ty.align();
        let exp_vol = if exp_ty.is_volatile() {
            " volatile"
        } else {
            ""
        };
        let cmp = self.fresh();
        self.emit(&format!(
            "  {cmp} = load{exp_vol} {mem}, ptr {expected}, align {exp_align}"
        ));
        let vol = if obj.is_volatile() { " volatile" } else { "" };
        let weak_kw = if weak { " weak" } else { "" };
        let res = self.fresh();
        self.note_wide_atomic_mem(&mem);
        self.emit(&format!(
            "  {res} = cmpxchg{weak_kw}{vol} ptr {addr}, {mem} {cmp}, {mem} {desired} {success} {failure}"
        ));
        let old = self.fresh();
        self.emit(&format!("  {old} = extractvalue {{{mem}, i1}} {res}, 0"));
        let ok = self.fresh();
        self.emit(&format!("  {ok} = extractvalue {{{mem}, i1}} {res}, 1"));
        let upd = self.label();
        let done = self.label();
        self.br_cond(&ok, &done, &upd);
        self.start(upd);
        self.emit(&format!(
            "  store{exp_vol} {mem} {old}, ptr {expected}, align {exp_align}"
        ));
        self.br(&done);
        self.start(done);
        let ret = self.fresh();
        self.emit(&format!("  {ret} = zext i1 {ok} to i8"));
        ret
    }

    fn gen_atomic_fetch(&mut self, op: crate::builtins::AtomicOp, args: &[Expr]) -> String {
        use crate::builtins::AtomicOp;
        let (obj, addr) = atomic_obj(self, &args[0]);
        let order = self.rmw_llvm_order(&args[2]);
        let dest = obj.unqualified();
        let bin = match op {
            AtomicOp::FetchAdd => BinOp::Add,
            AtomicOp::FetchSub => BinOp::Sub,
            AtomicOp::FetchOr => BinOp::BitOr,
            AtomicOp::FetchXor => BinOp::BitXor,
            AtomicOp::FetchAnd => BinOp::BitAnd,
            _ => BinOp::Add,
        };
        if dest.is_pointer() {
            let rhs = self.gen_val(&args[1]);
            let rhs_ty = args[1].ty().expect("typed");
            let (old, _) = self.atomic_rmw_loop_orders(
                &obj,
                &addr,
                obj.is_volatile(),
                order,
                load_order_for(order),
                false,
                |gen, old| {
                    let pointee = dest.pointee().expect("pointer dest");
                    gen.atomic_ptr_offset(bin, pointee, old, &rhs_ty, &rhs, false)
                },
            );
            return old;
        }
        let rhs = self.gen_val(&args[1]);
        let rhs_ty = args[1].ty().expect("typed");
        if let Some((old, _)) = self.try_integer_atomicrmw(bin, &obj, &addr, &rhs_ty, &rhs, order) {
            return old;
        }
        let (old, _) = self.atomic_rmw_loop_orders(
            &obj,
            &addr,
            obj.is_volatile(),
            order,
            load_order_for(order),
            false,
            |gen, old| {
                let pointee = dest.pointee().expect("pointer dest");
                gen.atomic_ptr_offset(bin, pointee, old, &rhs_ty, &rhs, false)
            },
        );
        old
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

fn atomic_obj(gen: &mut Gen, expr: &Expr) -> (CType, String) {
    let ptr = expr
        .ty()
        .and_then(|ty| ty.pointee().cloned())
        .expect("atomic pointer");
    let addr = gen.gen_val(expr);
    (ptr, addr)
}

fn emit_fence(gen: &mut Gen, args: &[Expr], signal: bool) {
    let order = gen.fence_llvm_order(&args[0]);
    if order.is_empty() {
        return;
    }
    let scope = if signal {
        " syncscope(\"singlethread\")"
    } else {
        ""
    };
    gen.emit(&format!("  fence{scope} {order}"));
}

fn fold_order_code(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Int { bits, value_ty, .. } => {
            let width = value_ty.width();
            if value_ty.is_signed() {
                let shift = 128 - width;
                Some((((*bits as u128) << shift) as i128 >> shift) as i64)
            } else if *bits <= i64::MAX as u64 {
                Some(*bits as i64)
            } else {
                None
            }
        }
        Expr::Cast { expr, ty_name, .. } => {
            let value = fold_order_code(expr)?;
            convert_order_const(value, ty_name)
        }
        Expr::Unary {
            op: UnaryOp::Plus,
            expr,
            ..
        } => fold_order_code(expr),
        _ => None,
    }
}

fn convert_order_const(value: i64, ty: &CType) -> Option<i64> {
    let ty = ty.unqualified();
    let Some(int) = ty.as_integer() else {
        return Some(value);
    };
    if int == TypeName::Bool {
        return Some(if value != 0 { 1 } else { 0 });
    }
    let width = int.width();
    if width == 0 || width >= 64 {
        return Some(value);
    }
    if int.is_signed() {
        let shift = 64 - width;
        Some((value << shift) >> shift)
    } else {
        let mask = (1i64 << width) - 1;
        Some(value & mask)
    }
}

fn order_from_expr(expr: &Expr) -> i64 {
    match fold_order_code(expr) {
        Some(code) if (0..=5).contains(&code) => code,
        _ => 5,
    }
}

impl Gen {
    fn note_wide_atomic_mem(&mut self, mem: &str) {
        if mem == "i128" && crate::target::current().needs_cx16() {
            self.needs_cx16 = true;
        }
    }

    fn eval_memory_order(&mut self, expr: &Expr) -> i64 {
        if fold_order_code(expr).is_none() {
            let _ = self.gen_val(expr);
        }
        order_from_expr(expr)
    }

    fn load_llvm_order(&mut self, expr: &Expr) -> &'static str {
        llvm_order(self.eval_memory_order(expr), OrderKind::Load)
    }

    fn store_llvm_order(&mut self, expr: &Expr) -> &'static str {
        llvm_order(self.eval_memory_order(expr), OrderKind::Store)
    }

    fn rmw_llvm_order(&mut self, expr: &Expr) -> &'static str {
        llvm_order(self.eval_memory_order(expr), OrderKind::Rmw)
    }

    fn failure_llvm_order(&mut self, expr: &Expr) -> &'static str {
        llvm_order(self.eval_memory_order(expr), OrderKind::Load)
    }

    fn flag_clear_llvm_order(&mut self, expr: &Expr) -> &'static str {
        // consume has no load on which to carry a dependency; use relaxed.
        let code = self.eval_memory_order(expr);
        llvm_order(if code == 1 { 0 } else { code }, OrderKind::Store)
    }

    fn fence_llvm_order(&mut self, expr: &Expr) -> &'static str {
        match self.eval_memory_order(expr) {
            0 => "",
            1 | 2 => "acquire",
            3 => "release",
            4 => "acq_rel",
            _ => "seq_cst",
        }
    }
}

fn llvm_order(code: i64, use_: OrderKind) -> &'static str {
    // memory_order_consume (1) is lowered as acquire.
    match (code, use_) {
        (0, _) => "monotonic",
        (1 | 2, OrderKind::Store) => "monotonic",
        (1 | 2, _) => "acquire",
        (3, OrderKind::Load) => "monotonic",
        (3, _) => "release",
        (4, OrderKind::Load) => "acquire",
        (4, OrderKind::Store) => "release",
        (4, _) => "acq_rel",
        _ => "seq_cst",
    }
}

#[derive(Clone, Copy)]
enum OrderKind {
    Load,
    Store,
    Rmw,
}

fn load_order_for(success: &str) -> &'static str {
    match success {
        "seq_cst" => "seq_cst",
        "acquire" | "acq_rel" => "acquire",
        _ => "monotonic",
    }
}

fn rmw_inst(op: BinOp) -> Option<&'static str> {
    Some(match op {
        BinOp::Add => "add",
        BinOp::Sub => "sub",
        BinOp::BitAnd => "and",
        BinOp::BitOr => "or",
        BinOp::BitXor => "xor",
        _ => return None,
    })
}
