//! Argument and return lowering for the Arm 64-bit Procedure Call Standard
//! (AAPCS64), as used by Apple arm64 and AArch64 Linux.
//!
//! Written from Arm's AAPCS64 ("Parameter passing" and "Result return") and
//! Apple's "Writing ARM64 code for Apple platforms". The rules used here:
//!
//! - A Homogeneous Floating-point Aggregate (HFA) — a struct, union, or
//!   array whose members are one to four values of the same floating type,
//!   with no other data — travels in consecutive SIMD/FP registers. LLVM
//!   spells that `[N x float]`, `[N x double]`, or `[N x fp128]`, and its
//!   AArch64 backend assigns the registers, or the stack once they run
//!   out, as one unit.
//! - Any other composite larger than 16 bytes is copied to memory the
//!   caller allocates, and a pointer to the copy is passed in its place.
//!   Returned, it is written through the address the caller passes in
//!   x8 (LLVM `sret`).
//! - A composite of at most 16 bytes travels in one or two general
//!   registers, its size rounded up to a multiple of 8. A 16-byte-aligned
//!   composite uses an even-numbered register pair. LLVM spells those
//!   `i64`, `[2 x i64]`, and `i128`. A returned composite of at most eight
//!   bytes is written as the integer of its exact size (`i24` for three
//!   bytes); only those low bytes of x0 are meaningful either way.
//! - Scalars are passed as themselves. Integers narrower than 32 bits
//!   carry `signext` / `zeroext` on Apple, whose ABI makes the caller
//!   extend them; AAPCS64 itself leaves those bits unspecified.
//!
//! The backend, not this module, decides which register or stack slot each
//! LLVM argument uses, including Apple's rule that every variadic argument
//! goes on the stack. So there is no register counting here.

use crate::abi::{AbiFail, AggAbi, AggUse, FnSig, Part, PartKind};
use crate::ast::{CType, FloatKind};

/// Classify a struct or union for AAPCS64.
pub fn classify(ty: &CType, use_as: AggUse) -> Result<AggAbi, AbiFail> {
    let bare = ty.unqualified();
    if !bare.is_record() {
        return Ok(AggAbi::Memory);
    }
    if !bare.is_complete() {
        return Err(AbiFail::Incomplete);
    }
    let size = bare.size().expect("complete aggregate");
    let align = match use_as {
        AggUse::Param => arg_align(&bare),
        AggUse::Return => u64::from(bare.align()),
    };
    if size == 0 {
        // A GNU empty struct carries no data. It is passed as one unused
        // general register so both sides agree on the argument count.
        return Ok(AggAbi::Coerce {
            ty: "i64".to_string(),
            size: 8,
        });
    }
    if let Some((base, count)) = hfa(&bare) {
        let base_size = base_bytes(base);
        return Ok(AggAbi::Coerce {
            ty: format!("[{count} x {base}]"),
            size: base_size * count,
        });
    }
    if size > 16 {
        return Ok(match use_as {
            AggUse::Param => AggAbi::Indirect,
            AggUse::Return => AggAbi::Memory,
        });
    }
    if align >= 16 {
        return Ok(AggAbi::Coerce {
            ty: "i128".to_string(),
            size: 16,
        });
    }
    if size <= 8 {
        return Ok(match use_as {
            AggUse::Param => AggAbi::Coerce {
                ty: "i64".to_string(),
                size: 8,
            },
            AggUse::Return => AggAbi::Coerce {
                ty: format!("i{}", size * 8),
                size,
            },
        });
    }
    Ok(AggAbi::Coerce {
        ty: "[2 x i64]".to_string(),
        size: 16,
    })
}

/// The alignment that places an argument in registers and on the stack.
/// AArch64 Linux ignores an `aligned` attribute written on the record
/// itself; Apple arm64 does not (both observed with Clang as a black box).
pub fn arg_align(ty: &CType) -> u64 {
    let bare = ty.unqualified();
    match bare.struct_ref() {
        Some(handle) if !crate::target::current().is_apple() => {
            u64::from(handle.unadjusted_align())
        }
        _ => u64::from(bare.align()),
    }
}

fn base_bytes(base: &str) -> u64 {
    match base {
        "float" => 4,
        "double" => 8,
        _ => 16,
    }
}

/// The base type and member count of a Homogeneous Floating-point
/// Aggregate, or `None`.
///
/// Every scalar in the object must be the same floating type, there must
/// be no bit-fields, and the object must be exactly one to four of those
/// values with each position covered by some member (a union of two
/// `float[2]` arrays is two floats; padding is not allowed).
pub fn hfa(ty: &CType) -> Option<(&'static str, u64)> {
    let size = ty.size()?;
    let mut leaves: Leaves = Vec::new();
    if !float_leaves(ty, 0, &mut leaves, &mut LeafMemo::new()) || leaves.is_empty() {
        return None;
    }
    let base = leaves[0].1;
    if leaves.iter().any(|(_, kind)| *kind != base) {
        return None;
    }
    let unit = base_bytes(base);
    if size % unit != 0 {
        return None;
    }
    let count = size / unit;
    if !(1..=4).contains(&count) {
        return None;
    }
    let mut covered = [false; 4];
    for (offset, _) in &leaves {
        if offset % unit != 0 {
            return None;
        }
        covered[(offset / unit) as usize] = true;
    }
    if covered[..count as usize].iter().all(|slot| *slot) {
        Some((base, count))
    } else {
        None
    }
}

/// Floating leaves of one record at offset 0, cached per record so that
/// nested unions (whose alternatives repeat the same positions) are walked
/// once per record type instead of once per path.
type Leaves = Vec<(u64, &'static str)>;
type LeafMemo = std::collections::HashMap<usize, Option<Leaves>>;

/// Collect the distinct `(offset, LLVM type)` pairs of the floating
/// scalars. Returns false when the object has any other kind of data, mixes
/// floating types, or has more than four positions; none of those is an
/// HFA, so the list never holds more than four entries.
fn float_leaves(ty: &CType, base: u64, out: &mut Leaves, memo: &mut LeafMemo) -> bool {
    match ty.peel() {
        CType::Float(kind) => add_leaf(out, base, kind_ir(*kind)),
        CType::Array(elem, Some(len)) => {
            let Some(elem_size) = elem.size() else {
                return false;
            };
            // More than four elements are more than four positions.
            if *len > 4 {
                return false;
            }
            for index in 0..*len {
                if !float_leaves(elem, base + index * elem_size, out, memo) {
                    return false;
                }
            }
            true
        }
        CType::Struct(handle) => {
            let id = handle.body_key();
            let leaves = match memo.get(&id) {
                Some(cached) => cached.clone(),
                None => {
                    let mut own = Vec::new();
                    let ok = handle.fields().unwrap_or_default().iter().all(|field| {
                        field.bit_width.is_none()
                            && float_leaves(&field.ty, field.offset, &mut own, memo)
                    });
                    let result = ok.then_some(own);
                    memo.insert(id, result.clone());
                    result
                }
            };
            match leaves {
                Some(leaves) => leaves
                    .iter()
                    .all(|(offset, kind)| add_leaf(out, base + offset, kind)),
                None => false,
            }
        }
        _ => false,
    }
}

/// Add one leaf, skipping a repeat of a position already seen (union
/// alternatives overlap). Fails on a second floating type or a fifth
/// position.
fn add_leaf(out: &mut Leaves, offset: u64, kind: &'static str) -> bool {
    if out.iter().any(|(_, seen)| *seen != kind) {
        return false;
    }
    if out.iter().any(|(seen, _)| *seen == offset) {
        return true;
    }
    out.push((offset, kind));
    out.len() <= 4
}

fn kind_ir(kind: FloatKind) -> &'static str {
    kind.ir_type()
}

fn lower_part(ty: &CType) -> Result<Part, AbiFail> {
    if ty.unqualified().is_record() {
        return Ok(match classify(ty, AggUse::Param)? {
            AggAbi::Coerce { ty: ir, size } => {
                // On AArch64 Linux an HFA that goes to the stack takes a slot
                // aligned to its own alignment, at least 8 and at most 16
                // (AAPCS64 stage C). LLVM needs that as `alignstack`, which
                // Clang also writes; Apple's packed stack slots use the
                // element alignment, the backend's default.
                let attrs = if ir.starts_with('[')
                    && !ir.ends_with("i64]")
                    && !crate::target::current().is_apple()
                {
                    format!("alignstack({})", arg_align(ty).clamp(8, 16))
                } else {
                    String::new()
                };
                Part {
                    ir_type: ir,
                    attrs,
                    kind: PartKind::Coerce { size },
                }
            }
            _ => Part {
                ir_type: "ptr".to_string(),
                attrs: String::new(),
                kind: PartKind::Indirect,
            },
        });
    }
    Ok(Part {
        ir_type: ty.ir_type(),
        attrs: ty.abi_attr().unwrap_or("").to_string(),
        kind: PartKind::Scalar,
    })
}

/// The lowered parts of one argument whose C type is `ty`. Fixed and
/// variadic arguments use the same lowering.
pub fn arg_parts(ty: &CType) -> Result<Vec<Part>, AbiFail> {
    Ok(vec![lower_part(ty)?])
}

pub fn function_sig(ret: &CType, params: &[CType]) -> Result<FnSig, AbiFail> {
    let mut sig = FnSig {
        ret_type: "void".to_string(),
        ret_prefix: String::new(),
        ret_agg: None,
        sret_attrs: None,
        params: Vec::with_capacity(params.len()),
        free_int: 0,
        free_sse: 0,
    };
    if !ret.is_void() {
        if ret.unqualified().is_record() {
            let agg = classify(ret, AggUse::Return)?;
            match &agg {
                AggAbi::Coerce { ty, .. } => sig.ret_type = ty.clone(),
                _ => {
                    sig.sret_attrs = Some(format!("sret({}) align {}", ret.ir_type(), ret.align()));
                }
            }
            sig.ret_agg = Some(match agg {
                AggAbi::Coerce { .. } => agg,
                _ => AggAbi::Memory,
            });
        } else {
            sig.ret_type = ret.ir_type();
            if let Some(attr) = ret.abi_attr() {
                sig.ret_prefix = format!("{attr} ");
            }
        }
    }
    for param in params {
        sig.params.push(arg_parts(param)?);
    }
    Ok(sig)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{RecordKind, StructField, StructRef, TypeName};
    use crate::target::{self, Target};

    fn record(fields: Vec<(&str, CType)>, union_ty: bool) -> CType {
        let handle = StructRef::new_incomplete(
            1,
            "T",
            if union_ty {
                RecordKind::Union
            } else {
                RecordKind::Struct
            },
        );
        let mut offset = 0u64;
        let mut size = 0u64;
        let mut align = 1u32;
        let mut out = Vec::new();
        for (name, ty) in fields {
            let field_align = u64::from(ty.align());
            let field_size = ty.size().unwrap();
            if !union_ty {
                offset = offset.div_ceil(field_align) * field_align;
            }
            out.push(StructField::plain(name, ty.clone(), offset));
            align = align.max(ty.align());
            if union_ty {
                size = size.max(field_size);
            } else {
                offset += field_size;
                size = offset;
            }
        }
        let size = size.div_ceil(u64::from(align)) * u64::from(align);
        handle.complete(out, size, align, "struct.T".to_string());
        CType::Struct(handle)
    }

    fn int() -> CType {
        CType::from_name(TypeName::Int)
    }
    fn float() -> CType {
        CType::Float(FloatKind::Float)
    }
    fn double() -> CType {
        CType::Float(FloatKind::Double)
    }

    fn coerce(ty: &CType, use_as: AggUse) -> String {
        match classify(ty, use_as).unwrap() {
            AggAbi::Coerce { ty, .. } => ty,
            AggAbi::Indirect => "indirect".to_string(),
            AggAbi::Memory => "sret".to_string(),
            other => format!("{other:?}"),
        }
    }

    #[test]
    fn classifies_like_the_standard() {
        target::set(crate::target::APPLE_DEFAULT);
        let i2 = record(vec![("a", int()), ("b", int())], false);
        assert_eq!(coerce(&i2, AggUse::Param), "i64");
        assert_eq!(coerce(&i2, AggUse::Return), "i64");
        let i3 = record(vec![("a", int()), ("b", int()), ("c", int())], false);
        assert_eq!(coerce(&i3, AggUse::Param), "[2 x i64]");
        assert_eq!(coerce(&i3, AggUse::Return), "[2 x i64]");
        let c3 = record(
            vec![
                ("a", CType::from_name(TypeName::Char)),
                ("b", CType::from_name(TypeName::Char)),
                ("c", CType::from_name(TypeName::Char)),
            ],
            false,
        );
        assert_eq!(coerce(&c3, AggUse::Param), "i64");
        assert_eq!(coerce(&c3, AggUse::Return), "i24");
        let f3 = record(vec![("a", float()), ("b", float()), ("c", float())], false);
        assert_eq!(coerce(&f3, AggUse::Param), "[3 x float]");
        let d4 = record(
            vec![
                ("a", double()),
                ("b", double()),
                ("c", double()),
                ("d", double()),
            ],
            false,
        );
        assert_eq!(coerce(&d4, AggUse::Param), "[4 x double]");
        let f5 = record(
            vec![
                ("a", float()),
                ("b", float()),
                ("c", float()),
                ("d", float()),
                ("e", float()),
            ],
            false,
        );
        assert_eq!(coerce(&f5, AggUse::Param), "indirect");
        assert_eq!(coerce(&f5, AggUse::Return), "sret");
        let df = record(vec![("a", double()), ("b", float())], false);
        assert_eq!(coerce(&df, AggUse::Param), "[2 x i64]");
        let uf = record(vec![("f", float()), ("g", float())], true);
        assert_eq!(coerce(&uf, AggUse::Param), "[1 x float]");
        let mixed = record(vec![("i", int()), ("f", float())], true);
        assert_eq!(coerce(&mixed, AggUse::Param), "i64");
        let q = record(vec![("x", CType::from_name(TypeName::Int128))], false);
        assert_eq!(coerce(&q, AggUse::Param), "i128");
        let ld = record(
            vec![
                ("a", CType::Float(FloatKind::LongDouble)),
                ("b", CType::Float(FloatKind::LongDouble)),
            ],
            false,
        );
        assert_eq!(coerce(&ld, AggUse::Param), "[2 x double]");
        target::set(Target::Aarch64Linux);
        let ld = record(
            vec![
                ("a", CType::Float(FloatKind::LongDouble)),
                ("b", CType::Float(FloatKind::LongDouble)),
            ],
            false,
        );
        assert_eq!(coerce(&ld, AggUse::Param), "[2 x fp128]");
        assert_eq!(coerce(&ld, AggUse::Return), "[2 x fp128]");
        target::set(Target::host());
    }
}
