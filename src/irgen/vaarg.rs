//! `va_start`, `va_end`, `va_copy` and `va_arg` for System V AMD64, AAPCS64
//! and Apple ARM64.

use super::*;

impl Gen {
    pub(super) fn gen_va_op(&mut self, op: VaOp, args: &[Expr]) {
        self.needs_va = true;
        match op {
            VaOp::Start | VaOp::End => {
                let ptr = self.gen_val(&args[0]);
                let intrinsic = if op == VaOp::Start {
                    "llvm.va_start"
                } else {
                    "llvm.va_end"
                };
                self.emit(&format!("  call void @{intrinsic}(ptr {ptr})"));
            }
            VaOp::Copy => {
                let dest = self.gen_val(&args[0]);
                let src = self.gen_val(&args[1]);
                self.emit(&format!("  call void @llvm.va_copy(ptr {dest}, ptr {src})"));
            }
        }
    }

    /// Address of the next variadic argument, advanced in `ap`.
    ///
    /// The layout is the System V AMD64 register save area: six general
    /// registers at offsets 0..48 and eight XMM registers at offsets 48..176.
    /// An argument that does not fit in the registers still free is taken
    /// from the overflow area, which stays 8-byte aligned. A two-register
    /// aggregate that does not fit leaves the leftover register unused.
    pub(super) fn va_arg_addr(&mut self, ap: &str, ty: &CType) -> String {
        match va_class(ty) {
            VaClass::Gp {
                slots,
                stack_align,
                stack_size,
            } => self.va_reg_or_stack(
                ap,
                &RegFetch {
                    field: 0,
                    slots,
                    stride: 8,
                    limit: 48,
                    stack_align,
                    stack_size,
                },
            ),
            VaClass::Fp {
                stack_align,
                stack_size,
            } => self.va_reg_or_stack(
                ap,
                &RegFetch {
                    field: 4,
                    slots: 1,
                    stride: 16,
                    limit: 176,
                    stack_align,
                    stack_size,
                },
            ),
            VaClass::Mem { align, size } => self.va_overflow(ap, align, size),
            VaClass::Pieces(_) => unreachable!("split aggregates are copied per eightbyte"),
        }
    }

    pub(super) fn va_reg_or_stack(&mut self, ap: &str, fetch: &RegFetch) -> String {
        let RegFetch {
            field,
            slots,
            stride,
            limit,
            stack_align,
            stack_size,
        } = *fetch;
        let off_ptr = self.offset_ptr(ap, field);
        let off_ty = CType::from_name(TypeName::UInt);
        let offset = self.load(&off_ty, &off_ptr);
        let need = u64::from(slots) * stride;
        let sum = self.fresh();
        self.emit(&format!("  {sum} = add i32 {offset}, {need}"));
        let fits = self.fresh();
        self.emit(&format!("  {fits} = icmp ule i32 {sum}, {limit}"));
        let reg_label = self.label();
        let stack_label = self.label();
        let done = self.label();
        self.br_cond(&fits, &reg_label, &stack_label);

        self.start(reg_label.clone());
        let save_slot = self.offset_ptr(ap, 16);
        let save = self.load(&CType::pointer(CType::Void), &save_slot);
        let reg_addr = self.fresh();
        self.emit(&format!(
            "  {reg_addr} = getelementptr inbounds i8, ptr {save}, i32 {offset}"
        ));
        self.store(&off_ty, &sum, &off_ptr);
        self.br(&done);

        self.start(stack_label.clone());
        let stack_addr = self.va_overflow(ap, stack_align, stack_size);
        self.br(&done);

        self.start(done);
        let addr = self.fresh();
        self.emit(&format!(
            "  {addr} = phi ptr [ {reg_addr}, %{reg_label} ], [ {stack_addr}, %{stack_label} ]"
        ));
        addr
    }

    pub(super) fn va_overflow(&mut self, ap: &str, align: u64, size: u64) -> String {
        let off_ptr = self.offset_ptr(ap, 8);
        let current = self.load(&CType::pointer(CType::Void), &off_ptr);
        let aligned = self.align_ptr(&current, align.max(8));
        let next = self.fresh();
        self.emit(&format!(
            "  {next} = getelementptr inbounds i8, ptr {aligned}, i64 {size}"
        ));
        self.store(&CType::pointer(CType::Void), &next, &off_ptr);
        aligned
    }

    pub(super) fn align_ptr(&mut self, ptr: &str, align: u64) -> String {
        if align <= 1 {
            return ptr.to_string();
        }
        let bits = self.fresh();
        self.emit(&format!("  {bits} = ptrtoint ptr {ptr} to i64"));
        let added = self.fresh();
        self.emit(&format!("  {added} = add i64 {bits}, {}", align - 1));
        let mask = (align as i64).wrapping_neg();
        let masked = self.fresh();
        self.emit(&format!("  {masked} = and i64 {added}, {mask}"));
        let out = self.fresh();
        self.emit(&format!("  {out} = inttoptr i64 {masked} to ptr"));
        out
    }

    /// Address of the next variadic argument of type `ty` on AArch64,
    /// with `ap` (the address of the `va_list` object) advanced past it.
    pub(super) fn aarch64_va_arg_addr(&mut self, ap: &str, ty: &CType) -> String {
        let class = aarch64_va_class(ty);
        if crate::target::current().is_apple() {
            self.apple_va_arg_addr(ap, &class)
        } else {
            self.aapcs64_va_arg_addr(ap, ty, &class)
        }
    }

    /// Windows x64: `va_list` is a `char *` into the argument slots, each
    /// 8 bytes. A struct or union whose size is not 1, 2, 4 or 8, and a
    /// scalar wider than 8 bytes (MinGW's x87 `long double`, `__int128`),
    /// was passed by reference, so its slot holds the address of a copy
    /// (Microsoft "x64 calling convention", varargs).
    pub(super) fn win64_va_arg_addr(&mut self, ap: &str, ty: &CType) -> String {
        let by_ref = if ty.unqualified().is_record() {
            // The same rule as a call: this also sends a record with a
            // flexible array member by reference at any size.
            !matches!(
                crate::win64::classify(ty, crate::abi::AggUse::Param),
                Ok(crate::abi::AggAbi::Coerce { .. })
            )
        } else {
            crate::win64::scalar_by_reference(ty)
        };
        let ptr_ty = CType::pointer(CType::Void);
        let current = self.load(&ptr_ty, ap);
        let next = self.fresh();
        self.emit(&format!(
            "  {next} = getelementptr inbounds i8, ptr {current}, i64 8"
        ));
        self.store(&ptr_ty, &next, ap);
        if by_ref {
            return self.load(&ptr_ty, &current);
        }
        current
    }

    /// Apple arm64: `va_list` is a `char *` into the stack, where every
    /// variadic argument has its own slot of a multiple of 8 bytes (16-byte
    /// aligned for a 16-byte-aligned type). A composite passed by reference
    /// has an 8-byte slot holding its address.
    pub(super) fn apple_va_arg_addr(&mut self, ap: &str, class: &A64Va) -> String {
        let ptr_ty = CType::pointer(CType::Void);
        let current = self.load(&ptr_ty, ap);
        let aligned = if class.align16 {
            self.align_ptr(&current, 16)
        } else {
            current
        };
        let step = if class.by_ref { 8 } else { class.stack_size };
        let next = self.fresh();
        self.emit(&format!(
            "  {next} = getelementptr inbounds i8, ptr {aligned}, i64 {step}"
        ));
        self.store(&ptr_ty, &next, ap);
        if class.by_ref {
            return self.load(&ptr_ty, &aligned);
        }
        aligned
    }

    /// AAPCS64 (AArch64 Linux): `va_list` is `{ __stack, __gr_top,
    /// __vr_top, __gr_offs, __vr_offs }`, and `va_arg` follows the
    /// algorithm in the AAPCS64 appendix on variable argument lists. A
    /// negative offset indexes back from the top of the general (8-byte
    /// slots) or FP/SIMD (16-byte slots) register save area; once it is no
    /// longer negative the argument is on the stack.
    pub(super) fn aapcs64_va_arg_addr(&mut self, ap: &str, ty: &CType, class: &A64Va) -> String {
        let i32_ty = CType::from_name(TypeName::Int);
        let ptr_ty = CType::pointer(CType::Void);
        let (offs_field, top_field, slot) = if class.fp_regs > 0 {
            (28u64, 16u64, 16u64)
        } else {
            (24, 8, 8)
        };
        let nreg = if class.fp_regs > 0 {
            class.fp_regs
        } else {
            class.gp_regs
        };
        let offs_ptr = self.offset_ptr(ap, offs_field);
        let offs = self.load(&i32_ty, &offs_ptr);
        let stack_label = self.label();
        let try_label = self.label();
        let reg_label = self.label();
        let done = self.label();
        let already = self.fresh();
        self.emit(&format!("  {already} = icmp sge i32 {offs}, 0"));
        self.br_cond(&already, &stack_label, &try_label);

        self.start(try_label);
        let offs = if class.fp_regs == 0 && class.align16 {
            let added = self.fresh();
            self.emit(&format!("  {added} = add i32 {offs}, 15"));
            let rounded = self.fresh();
            self.emit(&format!("  {rounded} = and i32 {added}, -16"));
            rounded
        } else {
            offs
        };
        let new_offs = self.fresh();
        self.emit(&format!(
            "  {new_offs} = add i32 {offs}, {}",
            u64::from(nreg) * slot
        ));
        self.store(&i32_ty, &new_offs, &offs_ptr);
        let overflow = self.fresh();
        self.emit(&format!("  {overflow} = icmp sgt i32 {new_offs}, 0"));
        self.br_cond(&overflow, &stack_label, &reg_label);

        self.start(reg_label.clone());
        let top_ptr = self.offset_ptr(ap, top_field);
        let top = self.load(&ptr_ty, &top_ptr);
        let offs64 = self.fresh();
        self.emit(&format!("  {offs64} = sext i32 {offs} to i64"));
        let reg_addr = self.fresh();
        self.emit(&format!(
            "  {reg_addr} = getelementptr inbounds i8, ptr {top}, i64 {offs64}"
        ));
        let reg_addr = if class.fp_regs > 1 || (class.fp_regs == 1 && ty.unqualified().is_record())
        {
            // HFA members sit in successive 16-byte register slots. Gather
            // them into a contiguous object.
            let tmp = self.entry_alloca(&ty.unqualified());
            let unit = class.fp_unit;
            let ir = match unit {
                4 => "float",
                8 => "double",
                _ => "fp128",
            };
            for index in 0..u64::from(class.fp_regs) {
                let from = self.offset_ptr(&reg_addr, index * 16);
                let value = self.fresh();
                self.emit(&format!("  {value} = load {ir}, ptr {from}, align {unit}"));
                let to = self.offset_ptr(&tmp, index * unit);
                self.emit(&format!("  store {ir} {value}, ptr {to}, align {unit}"));
            }
            tmp
        } else {
            reg_addr
        };
        let reg_end = self.cur.clone();
        self.br(&done);

        self.start(stack_label.clone());
        let stack_ptr = self.offset_ptr(ap, 0);
        let current = self.load(&ptr_ty, &stack_ptr);
        let aligned = if class.align16 {
            self.align_ptr(&current, 16)
        } else {
            current
        };
        let step = if class.by_ref { 8 } else { class.stack_size };
        let next = self.fresh();
        self.emit(&format!(
            "  {next} = getelementptr inbounds i8, ptr {aligned}, i64 {step}"
        ));
        self.store(&ptr_ty, &next, &stack_ptr);
        let stack_end = self.cur.clone();
        self.br(&done);

        self.start(done);
        let addr = self.fresh();
        self.emit(&format!(
            "  {addr} = phi ptr [ {reg_addr}, %{reg_end} ], [ {aligned}, %{stack_end} ]"
        ));
        if class.by_ref {
            return self.load(&ptr_ty, &addr);
        }
        addr
    }

    pub(super) fn gen_va_arg(&mut self, expr: &Expr) -> String {
        let Expr::VaArg { ap, type_arg, .. } = expr else {
            unreachable!("va_arg");
        };
        let ap_ptr = self.gen_val(ap);
        if crate::target::current().abi() == crate::target::Abi::Win64 {
            let addr = self.win64_va_arg_addr(&ap_ptr, type_arg);
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = load{} {}, ptr {addr}, align {}",
                vol_attr(type_arg),
                type_arg.ir_type(),
                type_arg.align().min(8)
            ));
            return tmp;
        }
        if crate::target::current().is_aarch64() {
            let addr = self.aarch64_va_arg_addr(&ap_ptr, type_arg);
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = load{} {}, ptr {addr}, align {}",
                vol_attr(type_arg),
                type_arg.ir_type(),
                type_arg.align().min(8)
            ));
            return tmp;
        }
        let addr = self.va_arg_addr(&ap_ptr, type_arg);
        // A 128-bit integer may come from the register save area, whose
        // GP slots are only 8-byte aligned. The address is a phi of that
        // slot and the overflow area, so the load claims 8, not 16.
        if let VaClass::Gp { slots: 2, .. } = va_class(type_arg) {
            if type_arg.align() > 8 {
                let tmp = self.fresh();
                self.emit(&format!(
                    "  {tmp} = load{} {}, ptr {addr}, align 8",
                    vol_attr(type_arg),
                    type_arg.ir_type(),
                ));
                return tmp;
            }
        }
        self.load(type_arg, &addr)
    }

    pub(super) fn gen_va_record(&mut self, expr: &Expr) -> String {
        let Expr::VaArg { ap, type_arg, .. } = expr else {
            unreachable!("va_arg record");
        };
        let ap_ptr = self.gen_val(ap);
        if crate::target::current().abi() == crate::target::Abi::Win64 {
            let addr = self.win64_va_arg_addr(&ap_ptr, type_arg);
            let slot = self.entry_alloca(type_arg);
            self.copy_struct(type_arg, &addr, &slot, type_arg.is_volatile());
            return slot;
        }
        if crate::target::current().is_aarch64() {
            let addr = self.aarch64_va_arg_addr(&ap_ptr, type_arg);
            let slot = self.entry_alloca(type_arg);
            self.copy_struct(type_arg, &addr, &slot, type_arg.is_volatile());
            return slot;
        }
        if let VaClass::Pieces(pieces) = va_class(type_arg) {
            return self.va_split(&ap_ptr, type_arg, &pieces);
        }
        let addr = self.va_arg_addr(&ap_ptr, type_arg);
        let slot = self.entry_alloca(type_arg);
        self.copy_struct(type_arg, &addr, &slot, type_arg.is_volatile());
        slot
    }

    /// `va_arg` of an aggregate whose eightbytes are not one contiguous
    /// image in the register save area. A `double` pair is two XMM slots
    /// sixteen bytes apart, and a mixed INTEGER/SSE aggregate uses both
    /// register classes. Either class being full sends the whole argument
    /// to the overflow area.
    pub(super) fn va_split(
        &mut self,
        ap: &str,
        ty: &CType,
        pieces: &[crate::abi::RegPiece],
    ) -> String {
        let gp_n = pieces.iter().filter(|piece| !piece.is_sse()).count() as u64;
        let fp_n = pieces.len() as u64 - gp_n;
        let gp_ptr = self.offset_ptr(ap, 0);
        let fp_ptr = self.offset_ptr(ap, 4);
        let off_ty = CType::from_name(TypeName::UInt);
        let gp = self.load(&off_ty, &gp_ptr);
        let fp = self.load(&off_ty, &fp_ptr);
        let fits = self.fresh();
        self.emit(&format!(
            "  {fits} = icmp ule i32 {gp}, {}",
            48u64.saturating_sub(gp_n * 8)
        ));
        let fp_fits = self.fresh();
        self.emit(&format!(
            "  {fp_fits} = icmp ule i32 {fp}, {}",
            176u64.saturating_sub(fp_n * 16)
        ));
        let both = self.fresh();
        self.emit(&format!("  {both} = and i1 {fits}, {fp_fits}"));
        let reg_label = self.label();
        let stack_label = self.label();
        let done = self.label();
        self.br_cond(&both, &reg_label, &stack_label);

        let slot = self.entry_alloca(ty);
        self.start(reg_label.clone());
        let save_slot = self.offset_ptr(ap, 16);
        let save = self.load(&CType::pointer(CType::Void), &save_slot);
        let mut gp_at = gp;
        let mut fp_at = fp;
        for piece in pieces {
            let cursor = if piece.is_sse() {
                let at = fp_at.clone();
                let next = self.fresh();
                self.emit(&format!("  {next} = add i32 {at}, 16"));
                fp_at = next;
                at
            } else {
                let at = gp_at.clone();
                let next = self.fresh();
                self.emit(&format!("  {next} = add i32 {at}, 8"));
                gp_at = next;
                at
            };
            let src = self.fresh();
            self.emit(&format!(
                "  {src} = getelementptr inbounds i8, ptr {save}, i32 {cursor}"
            ));
            let dest = self.offset_ptr(&slot, piece.offset());
            let bytes = piece_bytes(piece);
            self.needs_memmove = true;
            self.emit(&format!(
                "  call void @llvm.memmove.p0.p0.i64(ptr {dest}, ptr {src}, i64 {bytes}, i1 false)"
            ));
        }
        self.store(&off_ty, &gp_at, &gp_ptr);
        self.store(&off_ty, &fp_at, &fp_ptr);
        self.br(&done);

        self.start(stack_label.clone());
        let align = u64::from(ty.align()).max(8);
        let size = (ty.size().unwrap_or(8) + 7) & !7;
        let stack_addr = self.va_overflow(ap, align, size.max(8));
        self.copy_struct(ty, &stack_addr, &slot, ty.is_volatile());
        self.br(&done);

        self.start(done);
        slot
    }
}

pub(super) struct RegFetch {
    pub(super) field: u64,
    pub(super) slots: u32,
    pub(super) stride: u64,
    pub(super) limit: u64,
    pub(super) stack_align: u64,
    pub(super) stack_size: u64,
}

pub(super) enum VaClass {
    Gp {
        slots: u32,
        stack_align: u64,
        stack_size: u64,
    },
    /// One XMM slot. In the overflow area a scalar takes 8 bytes; a record
    /// keeps its own alignment and rounded size, as for `Gp`.
    Fp {
        stack_align: u64,
        stack_size: u64,
    },
    Mem {
        align: u64,
        size: u64,
    },
    /// Eightbytes that are not one packed image in the save area.
    Pieces(Vec<crate::abi::RegPiece>),
}

/// How one variadic argument is found on AArch64.
pub(super) struct A64Va {
    /// Consecutive FP/SIMD registers (an HFA's member count, or 1 for a
    /// floating scalar). Zero for the general-register class.
    pub(super) fp_regs: u32,
    /// Size of one HFA member, or of the floating scalar.
    pub(super) fp_unit: u64,
    /// General registers, when `fp_regs` is zero.
    pub(super) gp_regs: u32,
    /// The argument is a pointer to a caller-made copy.
    pub(super) by_ref: bool,
    /// 16-byte alignment: an even register pair and a 16-byte-aligned
    /// stack slot.
    pub(super) align16: bool,
    /// Bytes the argument occupies on the stack, a multiple of 8.
    pub(super) stack_size: u64,
}

pub(super) fn aarch64_va_class(ty: &CType) -> A64Va {
    let bare = ty.unqualified();
    let size = bare.size().unwrap_or(8);
    let round8 = ((size + 7) & !7).max(8);
    let align16 = crate::aapcs64::arg_align(&bare) >= 16;
    if bare.is_record() {
        if let Some((base, count)) = crate::aapcs64::hfa(&bare) {
            let unit = match base {
                "float" => 4,
                "double" => 8,
                _ => 16,
            };
            return A64Va {
                fp_regs: count as u32,
                fp_unit: unit,
                gp_regs: 0,
                by_ref: false,
                align16,
                stack_size: round8,
            };
        }
        if size > 16 {
            return A64Va {
                fp_regs: 0,
                fp_unit: 0,
                gp_regs: 1,
                by_ref: true,
                align16: false,
                stack_size: 8,
            };
        }
        return A64Va {
            fp_regs: 0,
            fp_unit: 0,
            gp_regs: (round8 / 8) as u32,
            by_ref: false,
            align16,
            stack_size: round8,
        };
    }
    if let Some(kind) = bare.float_kind() {
        return A64Va {
            fp_regs: 1,
            fp_unit: CType::Float(kind).size().unwrap_or(8),
            gp_regs: 0,
            by_ref: false,
            align16,
            stack_size: round8,
        };
    }
    A64Va {
        fp_regs: 0,
        fp_unit: 0,
        gp_regs: (round8 / 8) as u32,
        by_ref: false,
        align16,
        stack_size: round8,
    }
}

pub(super) fn va_class(ty: &CType) -> VaClass {
    let bare = ty.unqualified();
    if bare.is_record() {
        let size = bare.size().unwrap_or(1);
        let stack = (size + 7) & !7;
        return match crate::abi::classify_for(&bare, crate::abi::AggUse::Param) {
            Ok(AggAbi::Direct(pieces)) => {
                let sse = pieces.iter().filter(|piece| piece.is_sse()).count();
                if sse == 0 {
                    // In the overflow area the record keeps its own
                    // alignment, which `_Alignas(16)` can raise above 8.
                    VaClass::Gp {
                        slots: pieces.len().max(1) as u32,
                        stack_align: u64::from(bare.align()).max(8),
                        stack_size: stack.max(8),
                    }
                } else if sse == pieces.len() && pieces.len() == 1 {
                    // One XMM slot holds the whole object in its low bytes.
                    // On the stack it is the whole record, which an
                    // `aligned(16)` member makes 16 bytes, 16-aligned.
                    VaClass::Fp {
                        stack_align: u64::from(bare.align()).max(8),
                        stack_size: stack.max(8),
                    }
                } else {
                    VaClass::Pieces(pieces)
                }
            }
            _ => VaClass::Mem {
                align: u64::from(bare.align()).max(8),
                size: stack.max(8),
            },
        };
    }
    // `long double` is an x87 value. The register save area has no x87
    // slots, so a variadic `long double` is taken from the overflow area,
    // 16-byte aligned.
    if bare.float_kind() == Some(FloatKind::LongDouble) {
        return VaClass::Mem {
            align: 16,
            size: 16,
        };
    }
    if bare.as_float().is_some() {
        return VaClass::Fp {
            stack_align: 8,
            stack_size: 8,
        };
    }
    let size = bare.size().unwrap_or(8);
    // A 128-bit integer uses two general-purpose slots and 16-byte
    // alignment (System V AMD64).
    let (slots, stack_align) = if size == 16 { (2, 16) } else { (1, 8) };
    VaClass::Gp {
        slots,
        stack_align,
        stack_size: ((size + 7) & !7).max(8),
    }
}
