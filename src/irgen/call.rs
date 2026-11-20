//! Calls and the calling conventions (System V AMD64, AAPCS64): arguments,
//! coerced records, `sret`, parameters and returns.

use super::*;

impl Gen {
    pub(super) fn materialize_call(&mut self, expr: &Expr) -> String {
        let Expr::Call {
            callee,
            args,
            resolved,
            ty,
            ..
        } = expr
        else {
            unreachable!("call");
        };
        let ty = ty.clone().expect("typed call");
        let resolved = resolved.as_ref().expect("resolved call");
        let sig =
            function_sig(&resolved.ret, &resolved.params).expect("call has a System V signature");
        let saved = self.pos;
        self.pos = (expr.line(), expr.column());
        let slot = self.call_into_slot(callee, args, resolved, &sig, &ty);
        self.pos = saved;
        slot
    }

    /// Call a function whose result comes back in memory or as a coerced
    /// value, and leave the result in a fresh slot of type `ty`. A record
    /// return always takes this path; on Windows x64 so does a scalar
    /// returned by reference or in XMM0 (`win64::function_sig`).
    pub(super) fn call_into_slot(
        &mut self,
        callee: &Expr,
        args: &[Expr],
        resolved: &CallRes,
        sig: &FnSig,
        ty: &CType,
    ) -> String {
        let tmp = self.entry_alloca(ty);
        let sig = sig.clone();
        let ty = ty.clone();
        match sig.ret_agg.clone() {
            Some(AggAbi::Memory) => {
                let rendered = self.render_call_args(args, resolved, &sig, Some(&tmp));
                let _ = self.emit_call(callee, resolved, &sig, &rendered);
            }
            Some(AggAbi::X87) => {
                let rendered = self.render_call_args(args, resolved, &sig, None);
                let value = self
                    .emit_call(callee, resolved, &sig, &rendered)
                    .unwrap_or_else(|| self.ice("call returning a struct gave no value"));
                let align = ty.align().max(16);
                self.emit(&format!(
                    "  store x86_fp80 {value}, ptr {tmp}, align {align}"
                ));
            }
            Some(AggAbi::Direct(pieces)) => {
                let rendered = self.render_call_args(args, resolved, &sig, None);
                let value = self
                    .emit_call(callee, resolved, &sig, &rendered)
                    .unwrap_or_else(|| self.ice("call returning a struct gave no value"));
                self.store_pieces(&tmp, ty.align(), &pieces, &value);
            }
            Some(AggAbi::Coerce { ty: ir, size }) => {
                let rendered = self.render_call_args(args, resolved, &sig, None);
                let value = self
                    .emit_call(callee, resolved, &sig, &rendered)
                    .unwrap_or_else(|| self.ice("call returning a struct gave no value"));
                self.store_coerced(&value, &tmp, &ty, &ir, size);
            }
            Some(AggAbi::Indirect) | None => {
                self.ice("call returning a struct has no return convention");
            }
        }
        tmp
    }

    /// `sret_slot` is the temporary the callee writes when the return is in memory.
    pub(super) fn render_call_args(
        &mut self,
        args: &[Expr],
        resolved: &CallRes,
        sig: &FnSig,
        sret_slot: Option<&str>,
    ) -> Vec<String> {
        let mut rendered = Vec::new();
        let mut free_int = sig.free_int;
        let mut free_sse = sig.free_sse;
        if let Some(slot) = sret_slot {
            let attrs = match sig.sret_attrs.as_deref() {
                Some(attrs) => attrs,
                None => {
                    self.ice("memory return has no sret attribute");
                    ""
                }
            };
            rendered.push(format_part("ptr", attrs, Some(slot)));
        }
        for (index, arg) in args.iter().enumerate() {
            if index < resolved.params.len() {
                let parts = &sig.params[index];
                if parts.len() == 1 && matches!(parts[0].kind, PartKind::Byval) {
                    let src = self.record_ptr(arg);
                    rendered.push(format_part("ptr", &parts[0].attrs, Some(&src)));
                    continue;
                }
                if parts.len() == 1
                    && matches!(parts[0].kind, PartKind::Coerce { .. } | PartKind::Indirect)
                    && resolved.params[index].unqualified().is_record()
                {
                    let value = self.aapcs64_record_arg(arg, &parts[0]);
                    rendered.push(format_part(
                        &parts[0].ir_type,
                        &parts[0].attrs,
                        Some(&value),
                    ));
                    continue;
                }
                if parts
                    .iter()
                    .any(|part| matches!(part.kind, PartKind::Piece(_)))
                {
                    let src = self.record_ptr(arg);
                    let align = arg.ty().expect("typed").align();
                    for part in parts {
                        let PartKind::Piece(piece) = &part.kind else {
                            continue;
                        };
                        let loaded = self.load_piece(&src, align, piece);
                        rendered.push(format_part(&part.ir_type, &part.attrs, Some(&loaded)));
                    }
                    continue;
                }
                let value = self.gen_val(arg);
                let src = arg.ty().expect("typed");
                let param_ty = &resolved.params[index];
                let value = self.convert(&value, &src, param_ty);
                if parts.len() == 1 && matches!(parts[0].kind, PartKind::Indirect) {
                    // Windows x64: a scalar wider than 8 bytes is passed as
                    // a pointer to a copy.
                    let copy = self.scalar_copy(param_ty, &value);
                    rendered.push(format_part("ptr", "", Some(&copy)));
                    continue;
                }
                rendered.push(param_ir(param_ty, Some(&value)));
            } else if arg.ty().is_some_and(|ty| ty.is_record()) {
                rendered.extend(self.variadic_record(arg, &mut free_int, &mut free_sse));
            } else {
                // Default argument promotions (C11 6.5.2.2): integer types
                // narrower than `int` become `int`, and `float` becomes
                // `double`. A floating argument uses an XMM register.
                let value = self.gen_val(arg);
                let src = arg.ty().expect("typed");
                let promoted = default_promote(&src);
                if promoted.is_sse_float() {
                    free_sse = free_sse.saturating_sub(1);
                } else if !promoted.is_long_double() {
                    // A 128-bit integer takes two GP registers. If only
                    // one remains, the whole argument goes on the stack.
                    // Only its own registers are given back (System V
                    // AMD64 3.2.3), so a later argument can still use the
                    // leftover register.
                    let slots = if promoted.size() == Some(16) { 2 } else { 1 };
                    if free_int >= slots {
                        free_int -= slots;
                    }
                }
                let value = self.convert(&value, &src, &promoted);
                if crate::target::current().abi() == crate::target::Abi::Win64
                    && crate::win64::scalar_by_reference(&promoted)
                {
                    let copy = self.scalar_copy(&promoted, &value);
                    rendered.push(format_part("ptr", "", Some(&copy)));
                    continue;
                }
                rendered.push(format!("{} {value}", promoted.ir_type()));
            }
        }
        rendered
    }

    pub(super) fn variadic_record(
        &mut self,
        arg: &Expr,
        free_int: &mut usize,
        free_sse: &mut usize,
    ) -> Vec<String> {
        let ty = arg.ty().expect("typed");
        if crate::target::current().abi() == crate::target::Abi::Win64 {
            let parts = crate::win64::arg_parts(&ty).expect("variadic aggregate");
            let value = self.aapcs64_record_arg(arg, &parts[0]);
            return vec![format_part(&parts[0].ir_type, "", Some(&value))];
        }
        if crate::target::current().is_aarch64() {
            let parts = crate::aapcs64::arg_parts(&ty).expect("variadic aggregate");
            let value = self.aapcs64_record_arg(arg, &parts[0]);
            return vec![format_part(&parts[0].ir_type, "", Some(&value))];
        }
        let src = self.record_ptr(arg);
        let agg =
            crate::abi::classify_for(&ty, crate::abi::AggUse::Param).expect("variadic aggregate");
        let parts = match agg {
            AggAbi::Memory | AggAbi::X87 | AggAbi::Coerce { .. } | AggAbi::Indirect => {
                vec![memory_part(&ty)]
            }
            AggAbi::Direct(pieces) => pieces
                .into_iter()
                .map(|piece| Part {
                    ir_type: piece.ir_type(),
                    attrs: String::new(),
                    kind: PartKind::Piece(piece),
                })
                .collect(),
        };
        let parts = assign_regs(parts, &ty, free_int, free_sse);
        parts
            .iter()
            .map(|part| match &part.kind {
                PartKind::Byval => format_part("ptr", &part.attrs, Some(&src)),
                PartKind::Piece(piece) => {
                    let loaded = self.load_piece(&src, ty.align(), piece);
                    format_part(&piece.ir_type(), "", Some(&loaded))
                }
                PartKind::Scalar | PartKind::Coerce { .. } | PartKind::Indirect => {
                    unreachable!("System V aggregate piece")
                }
            })
            .collect()
    }

    pub(super) fn emit_call(
        &mut self,
        callee: &Expr,
        resolved: &CallRes,
        sig: &FnSig,
        rendered: &[String],
    ) -> Option<String> {
        let arglist = rendered.join(", ");
        let callee = if resolved.indirect {
            self.gen_val(callee)
        } else {
            llvm_global(renamed(&resolved.name, &self.renames))
        };
        // A variadic call names the function type, including `...`
        // (`call i32 (ptr, ...) @printf(...)`). Other calls name only the
        // return type, which is what the existing tests match.
        let ty = if resolved.variadic {
            variadic_sig(sig)
        } else {
            sig.ret_type.clone()
        };
        if sig.ret_type == "void" {
            self.emit(&format!("  call {ty} {callee}({arglist})"));
            None
        } else {
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = call {}{ty} {callee}({arglist})",
                sig.ret_prefix
            ));
            Some(tmp)
        }
    }

    pub(super) fn store_param(&mut self, param: &Param, names: &[String], parts: &[Part]) {
        if parts.len() == 1 && matches!(parts[0].kind, PartKind::Byval | PartKind::Indirect) {
            self.copy_struct(&param.ty, &names[0], &param.slot, false);
            return;
        }
        if let [Part {
            ir_type,
            kind: PartKind::Coerce { size },
            ..
        }] = parts
        {
            let ty = param.ty.clone();
            self.store_coerced(&names[0], &param.slot, &ty, ir_type, *size);
            return;
        }
        if parts
            .iter()
            .any(|part| matches!(part.kind, PartKind::Piece(_)))
        {
            // Pieces cover the bytes the ABI passes. Tail padding that stays
            // in memory is zero, which is a defined object representation.
            self.emit(&format!(
                "  store {} zeroinitializer, ptr {}, align {}",
                param.ty.ir_type(),
                param.slot,
                param.ty.align()
            ));
            for (part, name) in parts.iter().zip(names.iter()) {
                let PartKind::Piece(piece) = &part.kind else {
                    continue;
                };
                self.store_piece(&param.slot, param.ty.align(), piece, name);
            }
            return;
        }
        self.emit(&format!(
            "  store {} {}, ptr {}, align {}",
            param.ty.ir_type(),
            names[0],
            param.slot,
            param.ty.align()
        ));
    }

    /// A fresh entry-block slot of type `ty` holding `value`, for a scalar
    /// passed by reference.
    pub(super) fn scalar_copy(&mut self, ty: &CType, value: &str) -> String {
        let bare = ty.unqualified();
        let slot = self.entry_alloca(&bare);
        self.emit(&format!(
            "  store {} {value}, ptr {slot}, align {}",
            bare.ir_type(),
            bare.align()
        ));
        slot
    }

    /// An entry-block slot of LLVM type `ir`.
    pub(super) fn entry_alloca_ir(&mut self, ir: &str, align: u32) -> String {
        let name = self.fresh();
        let line = format!("  {name} = alloca {ir}, align {align}");
        self.lines.insert(self.alloca_at, line);
        self.alloca_at += 1;
        name
    }

    /// One AAPCS64 aggregate argument: the coerced value, or the address
    /// of a fresh copy for a composite passed by reference.
    pub(super) fn aapcs64_record_arg(&mut self, arg: &Expr, part: &Part) -> String {
        let ty = arg.ty().expect("typed");
        let src = self.record_ptr(arg);
        match part.kind {
            PartKind::Coerce { size } => {
                self.load_coerced(&src, &ty, &part.ir_type, size, ty.is_volatile())
            }
            PartKind::Indirect => {
                // The callee may modify its parameter, so it gets a copy.
                // On Windows x64 that caller-allocated copy must be 16-byte
                // aligned (Microsoft "x64 calling convention", parameter
                // passing).
                let copy = if crate::target::current().abi() == crate::target::Abi::Win64 {
                    let own = ty.unqualified();
                    self.entry_alloca_ir(&own.ir_type(), own.align().max(16))
                } else {
                    self.entry_alloca(&ty.unqualified())
                };
                self.copy_struct(&ty, &src, &copy, ty.is_volatile());
                copy
            }
            _ => unreachable!("AAPCS64 aggregate part"),
        }
    }

    /// Load the coerced LLVM value `ir` (`size` bytes) of the aggregate at
    /// `src`. When `ir` is wider than the object, the object is first
    /// copied into a slot of type `ir` so the load reads no byte past it.
    pub(super) fn load_coerced(
        &mut self,
        src: &str,
        ty: &CType,
        ir: &str,
        size: u64,
        volatile: bool,
    ) -> String {
        let object = ty.size().unwrap_or(0);
        let (from, align) = if size > object {
            let align = coerce_align(ir).max(ty.align());
            let slot = self.entry_alloca_ir(ir, align);
            if object > 0 {
                self.needs_memcpy = true;
                self.emit(&format!(
                    "  call void @llvm.memcpy.p0.p0.i64(ptr {slot}, ptr {src}, i64 {object}, i1 {volatile})"
                ));
            }
            (slot, align)
        } else {
            // A member of a packed record may sit at any byte.
            (src.to_string(), self.access_align(ty, src))
        };
        let vol = if volatile && size <= object {
            " volatile"
        } else {
            ""
        };
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = load{vol} {ir}, ptr {from}, align {align}"
        ));
        tmp
    }

    /// Store the coerced value `value` into the aggregate slot `dest`,
    /// through a slot of type `ir` when `ir` is wider than the object.
    pub(super) fn store_coerced(
        &mut self,
        value: &str,
        dest: &str,
        ty: &CType,
        ir: &str,
        size: u64,
    ) {
        let object = ty.size().unwrap_or(0);
        if size <= object {
            self.emit(&format!(
                "  store {ir} {value}, ptr {dest}, align {}",
                self.access_align(ty, dest)
            ));
            return;
        }
        let align = coerce_align(ir).max(ty.align());
        let slot = self.entry_alloca_ir(ir, align);
        self.emit(&format!("  store {ir} {value}, ptr {slot}, align {align}"));
        if object > 0 {
            self.needs_memcpy = true;
            self.emit(&format!(
                "  call void @llvm.memcpy.p0.p0.i64(ptr {dest}, ptr {slot}, i64 {object}, i1 false)"
            ));
        }
    }

    pub(super) fn entry_alloca(&mut self, ty: &CType) -> String {
        let name = self.fresh();
        let line = format!("  {name} = alloca {}, align {}", ty.ir_type(), ty.align());
        self.lines.insert(self.alloca_at, line);
        self.alloca_at += 1;
        name
    }

    pub(super) fn offset_ptr(&mut self, slot: &str, offset: u64) -> String {
        if offset == 0 {
            return slot.to_string();
        }
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = getelementptr inbounds i8, ptr {slot}, i64 {offset}"
        ));
        tmp
    }

    pub(super) fn load_piece(&mut self, slot: &str, base_align: u32, piece: &RegPiece) -> String {
        let addr = self.offset_ptr(slot, piece.offset());
        let tmp = self.fresh();
        let align = piece_align(base_align, piece.offset());
        self.emit(&format!(
            "  {tmp} = load {}, ptr {addr}, align {align}",
            piece.ir_type()
        ));
        tmp
    }

    pub(super) fn store_piece(
        &mut self,
        slot: &str,
        base_align: u32,
        piece: &RegPiece,
        value: &str,
    ) {
        let addr = self.offset_ptr(slot, piece.offset());
        let align = piece_align(base_align, piece.offset());
        self.emit(&format!(
            "  store {} {value}, ptr {addr}, align {align}",
            piece.ir_type()
        ));
    }

    /// Write a register return into an alloca. One piece is the call result;
    /// two pieces are `extractvalue`d from it.
    pub(super) fn store_pieces(
        &mut self,
        slot: &str,
        align: u32,
        pieces: &[RegPiece],
        value: &str,
    ) {
        if pieces.len() == 1 {
            self.store_piece(slot, align, &pieces[0], value);
            return;
        }
        let agg = crate::abi::pieces_type(pieces);
        for (index, piece) in pieces.iter().enumerate() {
            let tmp = self.fresh();
            self.emit(&format!("  {tmp} = extractvalue {agg} {value}, {index}"));
            self.store_piece(slot, align, piece, &tmp);
        }
    }

    pub(super) fn emit_record_return(&mut self, src: &str, src_volatile: bool) {
        let Some(agg) = self.ret_agg.clone() else {
            self.ice("struct or union return from a function that returns a scalar");
            return;
        };
        let align = self.ret_ty.align();
        match agg {
            AggAbi::Memory => {
                let ty = self.ret_ty.clone();
                self.copy_struct(&ty, src, "%agg.result", src_volatile);
                self.emit("  ret void");
            }
            AggAbi::X87 => {
                let vol = if src_volatile { " volatile" } else { "" };
                let align = align.max(16);
                let tmp = self.fresh();
                self.emit(&format!(
                    "  {tmp} = load{vol} x86_fp80, ptr {src}, align {align}"
                ));
                self.emit(&format!("  ret x86_fp80 {tmp}"));
            }
            AggAbi::Coerce { ty: ir, size } => {
                let ty = self.ret_ty.clone();
                let value = self.load_coerced(src, &ty, &ir, size, src_volatile);
                self.emit(&format!("  ret {ir} {value}"));
            }
            AggAbi::Indirect => unreachable!("AAPCS64 returns use sret, not a pointer argument"),
            AggAbi::Direct(pieces) => {
                if pieces.len() == 1 {
                    let value = self.load_piece(src, align, &pieces[0]);
                    self.emit(&format!("  ret {} {value}", pieces[0].ir_type()));
                    return;
                }
                let agg_ty = crate::abi::pieces_type(&pieces);
                let mut current = String::new();
                for (index, piece) in pieces.iter().enumerate() {
                    let loaded = self.load_piece(src, align, piece);
                    let tmp = self.fresh();
                    if index == 0 {
                        self.emit(&format!(
                            "  {tmp} = insertvalue {agg_ty} poison, {} {loaded}, 0",
                            piece.ir_type()
                        ));
                    } else {
                        self.emit(&format!(
                            "  {tmp} = insertvalue {agg_ty} {current}, {} {loaded}, {index}",
                            piece.ir_type()
                        ));
                    }
                    current = tmp;
                }
                self.emit(&format!("  ret {agg_ty} {current}"));
            }
        }
    }

    pub(super) fn emit_implicit_return(&mut self) {
        if let Some(agg) = self.ret_agg.clone() {
            match agg {
                AggAbi::Memory => self.emit("  ret void"),
                AggAbi::X87 => self.emit("  ret x86_fp80 0xK00000000000000000000"),
                AggAbi::Direct(pieces) => {
                    let ty = crate::abi::pieces_type(&pieces);
                    self.emit(&format!("  ret {ty} zeroinitializer"));
                }
                AggAbi::Coerce { ty, .. } => {
                    self.emit(&format!("  ret {ty} zeroinitializer"));
                }
                AggAbi::Indirect => unreachable!("AAPCS64 returns use sret"),
            }
            self.terminated = true;
            return;
        }
        if self.ret_ty.is_void() {
            self.emit("  ret void");
        } else if self.ret_ty.is_pointer() {
            self.emit("  ret ptr null");
        } else if let Some(kind) = self.ret_ty.float_kind() {
            self.emit(&format!(
                "  ret {} {}",
                self.ret_ty.ir_type(),
                crate::float::llvm_const(kind, 0)
            ));
        } else {
            self.emit(&format!("  ret {} 0", self.ret_ty.ir_type()));
        }
        self.terminated = true;
    }

    pub(super) fn gen_call(
        &mut self,
        callee: &Expr,
        args: &[Expr],
        resolved: &CallRes,
    ) -> Option<String> {
        if let Some(lowered) = self.gen_memory_builtin(&resolved.name, args) {
            return Some(lowered);
        }
        if let Some(lowered) = self.gen_signbit(&resolved.name, args) {
            return Some(lowered);
        }
        if let Some(lowered) = self.gen_fp_format_builtin(&resolved.name, args) {
            return Some(lowered);
        }
        if let Some(lowered) = self.gen_fp_compare(&resolved.name, args) {
            return Some(lowered);
        }
        if let Some(inline) = crate::builtins::Inline::from_name(&resolved.name) {
            return self.gen_inline_builtin(inline, args);
        }
        let sig =
            function_sig(&resolved.ret, &resolved.params).expect("call has a System V signature");
        if sig.ret_agg.is_some() && !resolved.ret.unqualified().is_record() {
            // Windows x64: a scalar returned in memory or as `<2 x i64>`.
            let ret = resolved.ret.unqualified();
            let slot = self.call_into_slot(callee, args, resolved, &sig, &ret);
            return Some(self.load(&ret, &slot));
        }
        let rendered = self.render_call_args(args, resolved, &sig, None);
        self.emit_call(callee, resolved, &sig, &rendered)
    }
}

/// `i32 (ptr, ...)` — the function type written on a variadic call.
///
/// Fixed parameters are the lowered types (`i64`, `ptr`, …). `byval` and
/// `sret` stay on the call arguments; they are not part of this type.
pub(super) fn variadic_sig(sig: &FnSig) -> String {
    let mut parts = Vec::new();
    if sig.sret_attrs.is_some() {
        parts.push("ptr".to_string());
    }
    for group in &sig.params {
        for part in group {
            parts.push(part.ir_type.clone());
        }
    }
    parts.push("...".to_string());
    format!("{} ({})", sig.ret_type, parts.join(", "))
}

/// Alignment of a slot that holds an AAPCS64 coerced value.
pub(super) fn coerce_align(ir: &str) -> u32 {
    if ir == "i128" || ir.contains("fp128") {
        16
    } else {
        8
    }
}

/// Alignment of an address `offset` bytes into an object of alignment `base`.
pub(super) fn piece_align(base_align: u32, offset: u64) -> u32 {
    if offset == 0 {
        return base_align.clamp(1, 8);
    }
    let mut align = 1u32;
    let base = base_align.max(1);
    while align < 8 {
        let next = align * 2;
        if next > base || offset % u64::from(next) != 0 {
            break;
        }
        align = next;
    }
    align
}

/// Default argument promotions. Narrow integers become `int`. `float`
/// becomes `double` (C11 6.5.2.2). Pointers and `double` are unchanged.
pub(super) fn default_promote(ty: &CType) -> CType {
    if ty.float_kind() == Some(FloatKind::Float) {
        return CType::Float(FloatKind::Double);
    }
    match ty.as_integer() {
        Some(int) => CType::from_name(int.promote()),
        None => ty.clone(),
    }
}

pub(super) fn piece_bytes(piece: &crate::abi::RegPiece) -> u64 {
    match piece {
        crate::abi::RegPiece::Ptr { .. } => 8,
        crate::abi::RegPiece::Int { bits, .. } => u64::from(bits / 8),
        crate::abi::RegPiece::Sse { kind, .. } => match kind {
            crate::abi::SsePiece::Float => 4,
            crate::abi::SsePiece::Double | crate::abi::SsePiece::FloatPair => 8,
        },
    }
}
