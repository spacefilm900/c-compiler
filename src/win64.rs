//! Argument and return lowering for the Microsoft x64 calling convention,
//! used by 64-bit Windows (MSVC and MinGW-w64).
//!
//! Written from Microsoft's published "x64 calling convention" and "x64
//! software conventions". The rules used here:
//!
//! - Each argument takes one 8-byte slot: the first four in RCX, RDX, R8
//!   and R9 (or XMM0 to XMM3 for `float` and `double`), the rest on the
//!   stack above a 32-byte shadow area. The LLVM x86-64 backend assigns the
//!   registers, the slots and the shadow area from the triple, so there is
//!   no register counting here.
//! - A struct or union of exactly 1, 2, 4 or 8 bytes is passed and returned
//!   as the integer of that size, even when its members are floating. Any
//!   other size, including an empty GNU struct, is copied by the caller to
//!   memory it owns, and a pointer to the copy is passed instead. Such a
//!   value is returned through a hidden pointer the caller passes first
//!   (LLVM `sret`).
//! - A scalar that does not fit in one slot is passed the same way, by
//!   reference: the x87 `long double` of MinGW (16 bytes) and `__int128`.
//!   An x87 `long double` is returned through a hidden pointer; `__int128`
//!   is returned in XMM0, which LLVM spells `<2 x i64>` (Clang does both,
//!   observed as a black box).
//! - Integers narrower than 32 bits are not extended by the caller (the
//!   upper bits are unspecified), except `_Bool`, which is a zero-extended
//!   byte ([`crate::ast::TypeName::abi_attr`]).
//! - A variadic argument is lowered like a fixed one after the default
//!   argument promotions. The backend copies a floating variadic argument
//!   into the matching general register as well, as the convention
//!   requires for unprototyped and variadic callees.

use crate::abi::{AbiFail, AggAbi, AggUse, FnSig, Part, PartKind};
use crate::ast::CType;

/// A struct or union whose size is one of these is passed in a register.
fn register_sized(size: u64) -> bool {
    matches!(size, 1 | 2 | 4 | 8)
}

/// Classify a struct or union.
pub fn classify(ty: &CType, use_as: AggUse) -> Result<AggAbi, AbiFail> {
    let bare = ty.unqualified();
    if !bare.is_record() {
        return Ok(AggAbi::Memory);
    }
    let Some(size) = bare.size() else {
        return Err(AbiFail::Incomplete);
    };
    if register_sized(size) {
        return Ok(AggAbi::Coerce {
            ty: format!("i{}", size * 8),
            size,
        });
    }
    Ok(match use_as {
        AggUse::Param => AggAbi::Indirect,
        AggUse::Return => AggAbi::Memory,
    })
}

/// A scalar too large for one 8-byte slot: the x87 `long double` and the
/// 128-bit integers.
pub fn scalar_by_reference(ty: &CType) -> bool {
    let bare = ty.unqualified();
    if bare.is_record() || bare.is_array() {
        return false;
    }
    (bare.is_long_double() && bare.size() == Some(16))
        || (bare.is_integer() && bare.size() == Some(16))
}

fn indirect_part() -> Part {
    Part {
        ir_type: "ptr".to_string(),
        attrs: String::new(),
        kind: PartKind::Indirect,
    }
}

fn lower_part(ty: &CType) -> Result<Part, AbiFail> {
    if ty.unqualified().is_record() {
        return Ok(match classify(ty, AggUse::Param)? {
            AggAbi::Coerce { ty: ir, size } => Part {
                ir_type: ir,
                attrs: String::new(),
                kind: PartKind::Coerce { size },
            },
            _ => indirect_part(),
        });
    }
    if scalar_by_reference(ty) {
        return Ok(indirect_part());
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
        let agg = if ret.unqualified().is_record() {
            Some(classify(ret, AggUse::Return)?)
        } else if scalar_by_reference(ret) {
            // `__int128` comes back in XMM0; an x87 `long double` in memory.
            Some(if ret.is_integer() {
                AggAbi::Coerce {
                    ty: "<2 x i64>".to_string(),
                    size: 16,
                }
            } else {
                AggAbi::Memory
            })
        } else {
            None
        };
        match agg {
            Some(AggAbi::Coerce { ty, size }) => {
                sig.ret_type = ty.clone();
                sig.ret_agg = Some(AggAbi::Coerce { ty, size });
            }
            Some(_) => {
                sig.sret_attrs = Some(format!("sret({}) align {}", ret.ir_type(), ret.align()));
                sig.ret_agg = Some(AggAbi::Memory);
            }
            None => {
                sig.ret_type = ret.ir_type();
                if let Some(attr) = ret.abi_attr() {
                    sig.ret_prefix = format!("{attr} ");
                }
            }
        }
    }
    for param in params {
        sig.params.push(arg_parts(param)?);
    }
    Ok(sig)
}
