//! Builtins lowered inline: `signbit`, memory builtins, floating
//! classification and comparison, overflow arithmetic and the other GNU
//! builtins.

use super::*;

impl Gen {
    /// Sign bit of a floating value, as an `int`. `long double`'s sign is
    /// bit 79, the high bit of the exponent word stored at offset 8.
    pub(super) fn gen_signbit(&mut self, name: &str, args: &[Expr]) -> Option<String> {
        let kind = match name {
            "__builtin_signbitf" => FloatKind::Float,
            "__builtin_signbit" => FloatKind::Double,
            "__builtin_signbitl" => FloatKind::LongDouble,
            _ => return None,
        };
        let arg = &args[0];
        let raw = self.gen_val(arg);
        let src = arg.ty().expect("typed");
        let dest = CType::Float(kind);
        let value = self.convert(&raw, &src, &dest);
        let neg = match kind {
            FloatKind::Float => {
                let bits = self.fresh();
                self.emit(&format!("  {bits} = bitcast float {value} to i32"));
                let cmp = self.fresh();
                self.emit(&format!("  {cmp} = icmp slt i32 {bits}, 0"));
                cmp
            }
            FloatKind::Double => {
                let bits = self.fresh();
                self.emit(&format!("  {bits} = bitcast double {value} to i64"));
                let cmp = self.fresh();
                self.emit(&format!("  {cmp} = icmp slt i64 {bits}, 0"));
                cmp
            }
            FloatKind::LongDouble if !kind.is_x87() => {
                // The sign is the top bit of the `double` or binary128
                // representation.
                let ir = dest.ir_type();
                let width = dest.size().unwrap_or(8) * 8;
                let bits = self.fresh();
                self.emit(&format!("  {bits} = bitcast {ir} {value} to i{width}"));
                let cmp = self.fresh();
                self.emit(&format!("  {cmp} = icmp slt i{width} {bits}, 0"));
                cmp
            }
            FloatKind::LongDouble => {
                let slot = self.entry_alloca(&CType::Float(FloatKind::LongDouble));
                self.emit(&format!("  store x86_fp80 {value}, ptr {slot}, align 16"));
                let hi = self.fresh();
                self.emit(&format!(
                    "  {hi} = getelementptr inbounds i8, ptr {slot}, i64 8"
                ));
                let word = self.fresh();
                self.emit(&format!("  {word} = load i16, ptr {hi}, align 2"));
                let cmp = self.fresh();
                self.emit(&format!("  {cmp} = icmp slt i16 {word}, 0"));
                cmp
            }
        };
        let out = self.fresh();
        self.emit(&format!("  {out} = zext i1 {neg} to i32"));
        Some(out)
    }

    /// `__builtin_memcpy`, `__builtin_memmove`, and `__builtin_memset`.
    ///
    /// The copy is `llvm.memcpy` or `llvm.memmove`. The result is the
    /// destination pointer, as the C library functions return `s1`.
    pub(super) fn gen_memory_builtin(&mut self, name: &str, args: &[Expr]) -> Option<String> {
        let which = match name {
            "__builtin_memcpy" => "llvm.memcpy.p0.p0.i64",
            "__builtin_memmove" => "llvm.memmove.p0.p0.i64",
            "__builtin_memset" => "llvm.memset.p0.i64",
            _ => return None,
        };
        let dest = self.gen_val(&args[0]);
        let len_ty = CType::from_name(TypeName::size_t());
        let len_src = args[2].ty().expect("typed");
        let len_val = self.gen_val(&args[2]);
        let len = self.convert(&len_val, &len_src, &len_ty);
        match name {
            "__builtin_memset" => {
                self.needs_memset = true;
                let src_ty = args[1].ty().expect("typed");
                let raw = self.gen_val(&args[1]);
                let as_int = self.convert(&raw, &src_ty, &CType::from_name(TypeName::Int));
                let byte = self.fresh();
                self.emit(&format!("  {byte} = trunc i32 {as_int} to i8"));
                self.emit(&format!(
                    "  call void @{which}(ptr align 1 {dest}, i8 {byte}, i64 {len}, i1 false)"
                ));
            }
            "__builtin_memcpy" => {
                self.needs_memcpy = true;
                let src = self.gen_val(&args[1]);
                self.emit(&format!(
                    "  call void @{which}(ptr align 1 {dest}, ptr align 1 {src}, i64 {len}, i1 false)"
                ));
            }
            _ => {
                self.needs_memmove = true;
                let src = self.gen_val(&args[1]);
                self.emit(&format!(
                    "  call void @{which}(ptr align 1 {dest}, ptr align 1 {src}, i64 {len}, i1 false)"
                ));
            }
        }
        Some(dest)
    }

    /// `__builtin_nan*` / `__builtin_nans*` / `__builtin_inf*` /
    /// `__builtin_huge_val*` fold to an LLVM floating constant when the
    /// tag, if any, is a compile-time constant. `float` and `double` use
    /// the 16-digit `0x` form; `long double` uses `0xK` and 20 hex digits
    /// (LLVM Language Reference, "Simple Constants").
    ///
    /// A quiet `__builtin_nan*` whose tag is not consumed in full, in a
    /// runtime context, is lowered to a call of the C library `nan` /
    /// `nanf` / `nanl` (C11 7.12.11.2) with the same string literal. The
    /// whole-tag rule is [`crate::float::nan_tag_from_expr`], shared with
    /// constant evaluation. A malformed `__builtin_nans*` tag is diagnosed
    /// in semantic analysis.
    pub(super) fn gen_fp_format_builtin(&mut self, name: &str, args: &[Expr]) -> Option<String> {
        let fp = crate::float::FpFormatBuiltin::from_name(name)?;
        let bits = match fp {
            crate::float::FpFormatBuiltin::Inf(kind) => crate::float::plus_inf(kind),
            crate::float::FpFormatBuiltin::Nan { kind, signaling } => {
                match crate::float::nan_tag_from_expr(&args[0]) {
                    Some(tag) => crate::float::nan_bits(kind, tag, signaling),
                    None => {
                        let Some(lib) = fp.libm_nan() else {
                            return Some(self.ice_in(
                                &args[0],
                                "malformed __builtin_nans tag was not diagnosed",
                            ));
                        };
                        return Some(self.gen_lib_nan(lib, fp.kind(), &args[0]));
                    }
                }
            }
        };
        Some(crate::float::llvm_const(fp.kind(), bits))
    }

    /// A C11 7.12.14 comparison builtin. Both operands are converted to
    /// their common real type and compared with one quiet `fcmp`; the
    /// `i1` result widens to an `int` 0 or 1.
    pub(super) fn gen_fp_compare(&mut self, name: &str, args: &[Expr]) -> Option<String> {
        let pred = crate::float::fp_compare_predicate(name)?;
        let left_ty = args[0].ty().expect("typed");
        let right_ty = args[1].ty().expect("typed");
        let common = common_real(&left_ty, &right_ty);
        let left = self.gen_val(&args[0]);
        let left = self.convert(&left, &left_ty, &common);
        let right = self.gen_val(&args[1]);
        let right = self.convert(&right, &right_ty, &common);
        let cmp = self.fresh();
        self.emit(&format!(
            "  {cmp} = fcmp {pred} {} {left}, {right}",
            common.ir_type()
        ));
        let out = self.fresh();
        self.emit(&format!("  {out} = zext i1 {cmp} to i32"));
        Some(out)
    }

    pub(super) fn gen_lib_nan(&mut self, name: &str, kind: FloatKind, tag: &Expr) -> String {
        match name {
            "nan" => self.needs_nan = true,
            "nanf" => self.needs_nanf = true,
            "nanl" => self.needs_nanl = true,
            _ => unreachable!("library nan name"),
        }
        let arg = self.gen_val(tag);
        let tmp = self.fresh();
        let ret = crate::ast::CType::Float(kind).ir_type();
        self.emit(&format!("  {tmp} = call {ret} @{name}(ptr {arg})"));
        tmp
    }

    /// Note an intrinsic declaration for the module prelude.
    pub(super) fn intrinsic(&mut self, declare: &str) {
        self.intrinsics.insert(declare.to_string());
    }

    /// The argument converted to the builtin's parameter type.
    pub(super) fn builtin_arg(
        &mut self,
        inline: crate::builtins::Inline,
        args: &[Expr],
        index: usize,
    ) -> String {
        let value = self.gen_val(&args[index]);
        let src = args[index].ty().expect("typed");
        match inline.param_type(index) {
            Some(param) => self.convert(&value, &src, &param),
            None => value,
        }
    }

    /// Lower a builtin from [`crate::builtins::Inline`] in place.
    pub(super) fn gen_inline_builtin(
        &mut self,
        inline: crate::builtins::Inline,
        args: &[Expr],
    ) -> Option<String> {
        use crate::builtins::Inline;
        match inline {
            Inline::Clz(ty) | Inline::Ctz(ty) | Inline::Popcount(ty) | Inline::Parity(ty) => {
                Some(self.gen_count_bits(inline, ty, args))
            }
            Inline::Ffs(ty) => Some(self.gen_ffs(inline, ty, args)),
            Inline::Clrsb(ty) => Some(self.gen_clrsb(inline, ty, args)),
            Inline::Bswap(ty) => {
                let value = self.builtin_arg(inline, args, 0);
                let ir = ty.ir_type();
                self.intrinsic(&format!("declare {ir} @llvm.bswap.{ir}({ir})"));
                let tmp = self.fresh();
                self.emit(&format!(
                    "  {tmp} = call {ir} @llvm.bswap.{ir}({ir} {value})"
                ));
                Some(tmp)
            }
            Inline::Expect => {
                let value = self.builtin_arg(inline, args, 0);
                let _ = self.builtin_arg(inline, args, 1);
                Some(value)
            }
            Inline::DebugBreak => {
                self.intrinsic("declare void @llvm.debugtrap()");
                self.emit("  call void @llvm.debugtrap()");
                None
            }
            Inline::FastFail => {
                let code = self.builtin_arg(inline, args, 0);
                self.emit(&format!(
                    "  call void asm sideeffect \"int $$0x29\", \"{{cx}},~{{dirflag}},~{{fpsr}},~{{flags}}\"(i32 {code})"
                ));
                self.emit("  unreachable");
                self.terminated = true;
                self.ensure_block();
                None
            }
            Inline::Unreachable | Inline::Trap => {
                if inline == Inline::Trap {
                    self.intrinsic("declare void @llvm.trap()");
                    self.emit("  call void @llvm.trap()");
                }
                self.emit("  unreachable");
                self.terminated = true;
                // Code after the call is in a block with no predecessor.
                self.ensure_block();
                None
            }
            Inline::Alloca => {
                let size = self.builtin_arg(inline, args, 0);
                let tmp = self.fresh();
                self.emit(&format!("  {tmp} = alloca i8, i64 {size}, align 16"));
                Some(tmp)
            }
            Inline::Prefetch => {
                // A hint only. The address is evaluated for its side effects.
                for index in 0..args.len() {
                    let _ = self.builtin_arg(inline, args, index);
                }
                None
            }
            Inline::AssumeAligned => {
                let ptr = self.builtin_arg(inline, args, 0);
                for index in 1..args.len() {
                    let _ = self.builtin_arg(inline, args, index);
                }
                Some(ptr)
            }
            Inline::ReturnAddress => {
                self.intrinsic("declare ptr @llvm.returnaddress(i32)");
                let tmp = self.fresh();
                self.emit(&format!("  {tmp} = call ptr @llvm.returnaddress(i32 0)"));
                Some(tmp)
            }
            Inline::FrameAddress => {
                self.intrinsic("declare ptr @llvm.frameaddress.p0(i32)");
                let tmp = self.fresh();
                self.emit(&format!("  {tmp} = call ptr @llvm.frameaddress.p0(i32 0)"));
                Some(tmp)
            }
            Inline::IsNan | Inline::IsInf | Inline::IsFinite | Inline::IsNormal => {
                Some(self.gen_fp_classify(inline, args))
            }
            Inline::Overflow { op, fixed } => Some(self.gen_overflow(op, fixed, args)),
        }
    }

    /// A bit-counting builtin's result, of type `from`, as an `int`.
    fn int_result(&mut self, value: &str, from: TypeName) -> String {
        self.convert(value, &CType::Int(from), &CType::int_type())
    }

    /// `__builtin_clz`, `ctz`, `popcount` and `parity` on an operand of
    /// type `ty`.
    fn gen_count_bits(
        &mut self,
        inline: crate::builtins::Inline,
        ty: TypeName,
        args: &[Expr],
    ) -> String {
        use crate::builtins::Inline;
        let value = self.builtin_arg(inline, args, 0);
        let ir = ty.ir_type();
        let tmp = self.fresh();
        match inline {
            Inline::Clz(_) | Inline::Ctz(_) => {
                let which = if matches!(inline, Inline::Clz(_)) {
                    "ctlz"
                } else {
                    "cttz"
                };
                self.intrinsic(&format!("declare {ir} @llvm.{which}.{ir}({ir}, i1)"));
                self.emit(&format!(
                    "  {tmp} = call {ir} @llvm.{which}.{ir}({ir} {value}, i1 false)"
                ));
            }
            _ => {
                self.intrinsic(&format!("declare {ir} @llvm.ctpop.{ir}({ir})"));
                self.emit(&format!(
                    "  {tmp} = call {ir} @llvm.ctpop.{ir}({ir} {value})"
                ));
            }
        }
        let result = if matches!(inline, Inline::Parity(_)) {
            let bit = self.fresh();
            self.emit(&format!("  {bit} = and {ir} {tmp}, 1"));
            bit
        } else {
            tmp
        };
        self.int_result(&result, ty)
    }

    /// `__builtin_ffs`: one plus the index of the least significant set
    /// bit, or zero for a zero operand.
    fn gen_ffs(&mut self, inline: crate::builtins::Inline, ty: TypeName, args: &[Expr]) -> String {
        let value = self.builtin_arg(inline, args, 0);
        let ir = ty.ir_type();
        self.intrinsic(&format!("declare {ir} @llvm.cttz.{ir}({ir}, i1)"));
        let count = self.fresh();
        self.emit(&format!(
            "  {count} = call {ir} @llvm.cttz.{ir}({ir} {value}, i1 false)"
        ));
        let plus = self.fresh();
        self.emit(&format!("  {plus} = add {ir} {count}, 1"));
        let zero = self.fresh();
        self.emit(&format!("  {zero} = icmp eq {ir} {value}, 0"));
        let pick = self.fresh();
        self.emit(&format!("  {pick} = select i1 {zero}, {ir} 0, {ir} {plus}"));
        self.int_result(&pick, ty)
    }

    /// `__builtin_clrsb`: the number of leading redundant sign bits.
    fn gen_clrsb(
        &mut self,
        inline: crate::builtins::Inline,
        ty: TypeName,
        args: &[Expr],
    ) -> String {
        let value = self.builtin_arg(inline, args, 0);
        let ir = ty.ir_type();
        let width = ty.width();
        self.intrinsic(&format!("declare {ir} @llvm.ctlz.{ir}({ir}, i1)"));
        let sign = self.fresh();
        self.emit(&format!("  {sign} = ashr {ir} {value}, {}", width - 1));
        let flipped = self.fresh();
        self.emit(&format!("  {flipped} = xor {ir} {value}, {sign}"));
        let count = self.fresh();
        self.emit(&format!(
            "  {count} = call {ir} @llvm.ctlz.{ir}({ir} {flipped}, i1 false)"
        ));
        let less = self.fresh();
        self.emit(&format!("  {less} = sub {ir} {count}, 1"));
        self.int_result(&less, ty)
    }

    /// `__builtin_isnan`, `isinf`, `isfinite` and `isnormal`, as an `int`
    /// 0 or 1.
    fn gen_fp_classify(&mut self, inline: crate::builtins::Inline, args: &[Expr]) -> String {
        use crate::builtins::Inline;
        let value = self.gen_val(&args[0]);
        let ty = args[0].ty().expect("typed");
        let kind = ty.float_kind().expect("floating argument");
        let ir = ty.ir_type();
        let cmp = self.fresh();
        if inline == Inline::IsNan {
            self.emit(&format!("  {cmp} = fcmp uno {ir} {value}, {value}"));
        } else {
            let suffix = match ir.as_str() {
                "float" => "f32",
                "double" => "f64",
                "fp128" => "f128",
                _ => "f80",
            };
            self.intrinsic(&format!("declare {ir} @llvm.fabs.{suffix}({ir})"));
            let abs = self.fresh();
            self.emit(&format!(
                "  {abs} = call {ir} @llvm.fabs.{suffix}({ir} {value})"
            ));
            let inf = crate::float::llvm_const(kind, crate::float::plus_inf(kind));
            match inline {
                Inline::IsInf => {
                    self.emit(&format!("  {cmp} = fcmp oeq {ir} {abs}, {inf}"));
                }
                Inline::IsFinite => {
                    self.emit(&format!("  {cmp} = fcmp olt {ir} {abs}, {inf}"));
                }
                _ => {
                    let min = crate::float::llvm_const(kind, crate::float::min_normal_bits(kind));
                    let below = self.fresh();
                    self.emit(&format!("  {below} = fcmp olt {ir} {abs}, {inf}"));
                    let above = self.fresh();
                    self.emit(&format!("  {above} = fcmp oge {ir} {abs}, {min}"));
                    self.emit(&format!("  {cmp} = and i1 {below}, {above}"));
                }
            }
        }
        let tmp = self.fresh();
        self.emit(&format!("  {tmp} = zext i1 {cmp} to i32"));
        tmp
    }

    /// `__builtin_add_overflow` and friends. Both operands are widened to
    /// `i128`, where the exact result of an addition or subtraction of
    /// 64-bit values fits. A product is formed from the magnitudes, which
    /// fit in 128 bits unsigned, and its sign. The result is stored
    /// truncated to the target type; the return value says whether the
    /// exact result was out of that type's range.
    pub(super) fn gen_overflow(
        &mut self,
        op: crate::builtins::OverflowOp,
        fixed: Option<TypeName>,
        args: &[Expr],
    ) -> String {
        use crate::builtins::OverflowOp;
        let (left, left_ty) = self.overflow_operand(fixed, args, 0);
        let (right, right_ty) = self.overflow_operand(fixed, args, 1);
        let dest_ptr = self.gen_val(&args[2]);
        let target = match fixed.or_else(|| {
            args[2]
                .ty()
                .and_then(|ty| ty.pointee().and_then(CType::as_integer))
        }) {
            Some(ty) => ty,
            None => return self.ice_in(&args[2], "overflow result is not a pointer to an integer"),
        };
        let tir = target.ir_type();
        if left_ty.width() == 128 || right_ty.width() == 128 || target.width() == 128 {
            return self.gen_overflow_i256(
                op,
                [(&left, left_ty), (&right, right_ty)],
                &dest_ptr,
                target,
            );
        }
        let overflow = match op {
            OverflowOp::Add | OverflowOp::Sub => {
                let exact = self.fresh();
                let inst = if op == OverflowOp::Add { "add" } else { "sub" };
                self.emit(&format!("  {exact} = {inst} i128 {left}, {right}"));
                let low = self.fresh();
                self.emit(&format!("  {low} = trunc i128 {exact} to {tir}"));
                self.store(&CType::Int(target), &low, &dest_ptr);
                let back = self.fresh();
                let ext = if target.is_signed() { "sext" } else { "zext" };
                self.emit(&format!("  {back} = {ext} {tir} {low} to i128"));
                let ov = self.fresh();
                self.emit(&format!("  {ov} = icmp ne i128 {back}, {exact}"));
                ov
            }
            OverflowOp::Mul => {
                self.overflow_mul([(&left, left_ty), (&right, right_ty)], &dest_ptr, target)
            }
        };
        let tmp = self.fresh();
        self.emit(&format!("  {tmp} = zext i1 {overflow} to i8"));
        tmp
    }

    /// Operand `index` of an overflow builtin, converted to its type (the
    /// builtin's fixed type, or the operand's own) and widened to `i128`
    /// unless it is 128 bits already.
    fn overflow_operand(
        &mut self,
        fixed: Option<TypeName>,
        args: &[Expr],
        index: usize,
    ) -> (String, TypeName) {
        let value = self.gen_val(&args[index]);
        let src = args[index].ty().expect("typed");
        let ty = fixed.unwrap_or_else(|| src.as_integer().expect("integer operand"));
        let value = self.convert(&value, &src, &CType::Int(ty));
        if ty.width() == 128 {
            return (value, ty);
        }
        let wide = self.fresh();
        let ext = if ty.is_signed() { "sext" } else { "zext" };
        self.emit(&format!(
            "  {wide} = {ext} {} {value} to i128",
            ty.ir_type()
        ));
        (wide, ty)
    }

    /// The magnitude of an `i128` operand of type `ty` and an `i1` that is
    /// set when it is negative (the constant `false` when unsigned).
    fn overflow_magnitude(&mut self, value: &str, ty: TypeName) -> (String, String) {
        if !ty.is_signed() {
            return (value.to_string(), "false".to_string());
        }
        let neg = self.fresh();
        self.emit(&format!("  {neg} = icmp slt i128 {value}, 0"));
        let flipped = self.fresh();
        self.emit(&format!("  {flipped} = sub i128 0, {value}"));
        let abs = self.fresh();
        self.emit(&format!(
            "  {abs} = select i1 {neg}, i128 {flipped}, i128 {value}"
        ));
        (abs, neg)
    }

    /// `__builtin_mul_overflow` on operands widened to `i128`: store the
    /// truncated product through `dest_ptr` and return the `i1` overflow
    /// flag for the `target` type.
    fn overflow_mul(
        &mut self,
        [(left, left_ty), (right, right_ty)]: [(&str, TypeName); 2],
        dest_ptr: &str,
        target: TypeName,
    ) -> String {
        let tir = target.ir_type();
        let tw = target.width();
        let (left_abs, left_neg) = self.overflow_magnitude(left, left_ty);
        let (right_abs, right_neg) = self.overflow_magnitude(right, right_ty);
        let mag = self.fresh();
        self.emit(&format!("  {mag} = mul i128 {left_abs}, {right_abs}"));
        let neg = self.fresh();
        self.emit(&format!("  {neg} = xor i1 {left_neg}, {right_neg}"));
        let flipped = self.fresh();
        self.emit(&format!("  {flipped} = sub i128 0, {mag}"));
        let exact = self.fresh();
        self.emit(&format!(
            "  {exact} = select i1 {neg}, i128 {flipped}, i128 {mag}"
        ));
        let low = self.fresh();
        self.emit(&format!("  {low} = trunc i128 {exact} to {tir}"));
        self.store(&CType::Int(target), &low, dest_ptr);
        // A zero product is never negative.
        let nonzero = self.fresh();
        self.emit(&format!("  {nonzero} = icmp ne i128 {mag}, 0"));
        let negative = self.fresh();
        self.emit(&format!("  {negative} = and i1 {neg}, {nonzero}"));
        if target.is_signed() {
            let max = (1u128 << (tw - 1)) - 1;
            let limit = self.fresh();
            self.emit(&format!(
                "  {limit} = select i1 {negative}, i128 {}, i128 {max}",
                max + 1
            ));
            let ov = self.fresh();
            self.emit(&format!("  {ov} = icmp ugt i128 {mag}, {limit}"));
            ov
        } else {
            let max = if tw == 64 {
                u128::from(u64::MAX)
            } else {
                (1u128 << tw) - 1
            };
            let big = self.fresh();
            self.emit(&format!("  {big} = icmp ugt i128 {mag}, {max}"));
            let ov = self.fresh();
            self.emit(&format!("  {ov} = or i1 {big}, {negative}"));
            ov
        }
    }

    /// Overflow checking for a 128-bit operand or destination.
    ///
    /// A same-width `sext`/`trunc` of `i128` is not valid LLVM, and an
    /// `i128` add/sub/mul can wrap, so the overflow flag would be wrong.
    /// Each operand is extended to `i256` (so mixed signs keep infinite
    /// precision, as in `__builtin_add_overflow((__int128)-1, 1, &u128)`),
    /// the operation is done there, and the exact result is tested against
    /// the destination limits. The stored value is the low bits of that
    /// exact result.
    pub(super) fn gen_overflow_i256(
        &mut self,
        op: crate::builtins::OverflowOp,
        operands: [(&str, TypeName); 2],
        dest_ptr: &str,
        target: TypeName,
    ) -> String {
        use crate::builtins::OverflowOp;
        let widen = |gen: &mut Self, value: &str, ty: TypeName| -> String {
            let ext = if ty.is_signed() { "sext" } else { "zext" };
            let wide = gen.fresh();
            gen.emit(&format!("  {wide} = {ext} i128 {value} to i256"));
            wide
        };
        let left = widen(self, operands[0].0, operands[0].1);
        let right = widen(self, operands[1].0, operands[1].1);
        let inst = match op {
            OverflowOp::Add => "add",
            OverflowOp::Sub => "sub",
            OverflowOp::Mul => "mul",
        };
        let exact = self.fresh();
        self.emit(&format!("  {exact} = {inst} i256 {left}, {right}"));
        let limit = |gen: &mut Self, signed: bool, width: u32, min: bool| -> String {
            if !signed && !min {
                let sh = gen.fresh();
                gen.emit(&format!("  {sh} = shl i256 1, {width}"));
                let max = gen.fresh();
                gen.emit(&format!("  {max} = sub i256 {sh}, 1"));
                return max;
            }
            if signed && !min {
                let sh = gen.fresh();
                gen.emit(&format!("  {sh} = shl nsw i256 1, {}", width - 1));
                let max = gen.fresh();
                gen.emit(&format!("  {max} = sub i256 {sh}, 1"));
                return max;
            }
            if signed && min {
                let sh = gen.fresh();
                gen.emit(&format!("  {sh} = shl nsw i256 1, {}", width - 1));
                let v = gen.fresh();
                gen.emit(&format!("  {v} = sub i256 0, {sh}"));
                return v;
            }
            "0".to_string()
        };
        let tw = target.width();
        let signed = target.is_signed();
        let max = limit(self, signed, tw, false);
        let pred_lo = if signed { "sge" } else { "uge" };
        let pred_hi = if signed { "sle" } else { "ule" };
        let ok_lo = self.fresh();
        if signed {
            let min = limit(self, signed, tw, true);
            self.emit(&format!("  {ok_lo} = icmp {pred_lo} i256 {exact}, {min}"));
        } else {
            self.emit(&format!("  {ok_lo} = icmp {pred_lo} i256 {exact}, 0"));
        }
        let ok_hi = self.fresh();
        self.emit(&format!("  {ok_hi} = icmp {pred_hi} i256 {exact}, {max}"));
        let ok = self.fresh();
        self.emit(&format!("  {ok} = and i1 {ok_lo}, {ok_hi}"));
        let ov = self.fresh();
        self.emit(&format!("  {ov} = xor i1 {ok}, true"));
        let low = self.fresh();
        let tir = target.ir_type();
        self.emit(&format!("  {low} = trunc i256 {exact} to {tir}"));
        self.store(&CType::Int(target), &low, dest_ptr);
        let tmp = self.fresh();
        self.emit(&format!("  {tmp} = zext i1 {ov} to i8"));
        tmp
    }

    pub(super) fn zext_cond(&mut self, expr: &Expr) -> String {
        let cond = self.gen_cond(expr);
        let tmp = self.fresh();
        self.emit(&format!("  {tmp} = zext i1 {cond} to i32"));
        tmp
    }
}
