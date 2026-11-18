//! Constant initializers of static objects: byte images, unions,
//! relocations, and typed LLVM constants.

use super::*;

/// Spell raw bytes as the union-shaped LLVM constant for `ty`.
pub(super) fn render_byte_blob(ty: &CType, raw: &[u8]) -> String {
    let mut bytes = vec![0u8; ty.size().unwrap_or(raw.len() as u64) as usize];
    let n = raw.len().min(bytes.len());
    bytes[..n].copy_from_slice(&raw[..n]);
    let align = u64::from(ty.align().max(1));
    let (elem, step) = match align {
        2 => ("i16", 2usize),
        4 => ("i32", 4),
        8 => ("i64", 8),
        16 => ("i128", 16),
        _ => ("i8", 1),
    };
    let step = if bytes.len() % step != 0 { 1 } else { step };
    let elem = if step == 1 { "i8" } else { elem };
    let count = bytes.len() / step;
    if bytes.iter().all(|byte| *byte == 0) {
        return "zeroinitializer".to_string();
    }
    let mut parts = Vec::new();
    for index in 0..count {
        let mut acc: u128 = 0;
        for byte in 0..step {
            acc |= u128::from(bytes[index * step + byte]) << (8 * byte);
        }
        parts.push(format!("{elem} {acc}"));
    }
    if ty.is_union() || ty.record_has_bitfield() {
        format!("{{ [ {count} x {elem} ] [ {} ] }}", parts.join(", "))
    } else {
        format!("[ {} ]", parts.join(", "))
    }
}

/// A union's LLVM type is `{ [N x iK] }` with the union's alignment. The
/// constant is the first member's bytes, little endian, padded with zeros.
pub(super) fn render_union_const(ty: &CType, value: &ConstInit) -> String {
    let size = ty.size().unwrap_or(0) as usize;
    let align = usize::try_from(ty.align().max(1)).unwrap_or(1);
    let mut bytes = vec![0u8; size];
    // Semantic analysis rejects an address constant in a union before this
    // runs. A `false` result here is a bug in that check, not a zero.
    assert!(
        write_const_bytes(ty, value, &mut bytes, 0),
        "union initializer contains a value that is not a byte constant"
    );
    if bytes.iter().all(|byte| *byte == 0) {
        return "zeroinitializer".to_string();
    }
    let (elem, step) = match align {
        2 => ("i16", 2),
        4 => ("i32", 4),
        8 => ("i64", 8),
        16 => ("i128", 16),
        _ => ("i8", 1),
    };
    let count = if step == 0 || bytes.len() % step != 0 {
        bytes.len()
    } else {
        bytes.len() / step
    };
    let step = if bytes.len() % step != 0 { 1 } else { step };
    let elem = if step == 1 { "i8" } else { elem };
    let mut parts = Vec::new();
    for index in 0..count {
        let mut acc: u128 = 0;
        for byte in 0..step {
            let at = index * step + byte;
            if at < bytes.len() {
                acc |= u128::from(bytes[at]) << (8 * byte);
            }
        }
        parts.push(format!("{elem} {acc}"));
    }
    format!("{{ [ {count} x {elem} ] [ {} ] }}", parts.join(", "))
}

pub(super) fn write_const_bytes(
    ty: &CType,
    value: &ConstInit,
    bytes: &mut [u8],
    offset: usize,
) -> bool {
    match value {
        ConstInit::Zero => true,
        ConstInit::Float(bits) => {
            let width = ty.size().unwrap_or(0) as usize;
            let bits = &ty
                .float_kind()
                .map_or(*bits, |kind| crate::float::memory_bits(kind, *bits));
            for index in 0..width {
                let at = offset + index;
                if at < bytes.len() {
                    bytes[at] = (*bits >> (8 * index)) as u8;
                }
            }
            true
        }
        ConstInit::Int(bits) => {
            let Some(int) = ty.as_integer() else {
                return false;
            };
            let width = (int.width() / 8) as usize;
            for index in 0..width {
                let at = offset + index;
                if at < bytes.len() {
                    bytes[at] = (*bits >> (8 * index)) as u8;
                }
            }
            true
        }
        ConstInit::Aggregate(items) => {
            if ty.is_union() {
                let Some(field) = ty.struct_ref().and_then(|handle| {
                    handle.fields().and_then(|fields| fields.into_iter().next())
                }) else {
                    return true;
                };
                let Some(item) = items.first() else {
                    return true;
                };
                return write_const_bytes(&field.ty, item, bytes, offset);
            }
            if let Some(fields) = ty.struct_ref().and_then(StructRef::fields) {
                for (field, item) in fields.iter().zip(items) {
                    let at = offset + field.offset as usize;
                    if !write_const_bytes(&field.ty, item, bytes, at) {
                        return false;
                    }
                }
                return true;
            }
            if let (Some(elem), Some(len)) = (ty.array_elem(), ty.array_len()) {
                let step = elem.size().unwrap_or(0) as usize;
                for index in 0..len as usize {
                    let Some(item) = items.get(index) else {
                        break;
                    };
                    if !write_const_bytes(elem, item, bytes, offset + index * step) {
                        return false;
                    }
                }
                return true;
            }
            false
        }
        ConstInit::Ptr(PtrConst::Null) => true,
        ConstInit::Ptr(PtrConst::IntToPtr(bits)) => {
            for index in 0..8 {
                let at = offset + index;
                if at < bytes.len() {
                    bytes[at] = (*bits >> (8 * index)) as u8;
                }
            }
            true
        }
        ConstInit::Ptr(PtrConst::Addr { .. } | PtrConst::Label { .. })
        | ConstInit::PtrToInt { .. }
        | ConstInit::RelocBytes { .. } => false,
        ConstInit::Bytes(raw) => {
            for (index, byte) in raw.iter().enumerate() {
                let at = offset + index;
                if at < bytes.len() {
                    bytes[at] = *byte;
                }
            }
            true
        }
    }
}

/// A constant that holds an image with addresses somewhere inside it.
pub(super) fn has_reloc_image(value: &ConstInit) -> bool {
    match value {
        ConstInit::RelocBytes { .. } => true,
        ConstInit::Aggregate(items) => items.iter().any(has_reloc_image),
        _ => false,
    }
}

/// The LLVM type and spelling of a static initializer.
///
/// Most constants have the object's own type. A union or bit-field record
/// is a byte array in LLVM, which cannot hold an address, so an image with
/// relocations becomes a packed struct of byte runs and `ptr` (or
/// `ptrtoint`) pieces at their offsets. A struct or array around one is
/// then a packed struct too, laid out from the C offsets, unless every
/// array element still has the same type.
pub(super) fn render_typed_const(
    ty: &CType,
    value: &ConstInit,
    renames: &Renames,
) -> Result<(String, String), Ice> {
    if !has_reloc_image(value) {
        return Ok((ty.ir_type(), render_const(ty, value, renames)?));
    }
    let size = ty.size().unwrap_or(0);
    let mut pieces: Vec<(String, String)> = Vec::new();
    let pad = |pieces: &mut Vec<(String, String)>, from: u64, to: u64| {
        if to > from {
            pieces.push((
                format!("[{} x i8]", to - from),
                "zeroinitializer".to_string(),
            ));
        }
    };
    match value {
        ConstInit::RelocBytes { bytes, relocs } => {
            let mut at = 0u64;
            let run = |pieces: &mut Vec<(String, String)>, from: u64, to: u64| {
                if to > from {
                    let slice = &bytes[from as usize..to as usize];
                    let spelled = if slice.iter().all(|byte| *byte == 0) {
                        "zeroinitializer".to_string()
                    } else {
                        format!("c{}", llvm_bytes(slice))
                    };
                    pieces.push((format!("[{} x i8]", to - from), spelled));
                }
            };
            for (offset, reloc) in relocs {
                run(&mut pieces, at, *offset);
                match reloc {
                    ConstInit::PtrToInt { ptr, width } => {
                        let ir = format!("i{width}");
                        let spelled =
                            format!("ptrtoint ({} to {ir})", render_ptr_operand(ptr, renames));
                        pieces.push((ir, spelled));
                        at = offset + u64::from(*width / 8);
                    }
                    ConstInit::Ptr(ptr) => {
                        pieces.push(("ptr".to_string(), render_ptr(ptr, renames)));
                        at = offset + 8;
                    }
                    other => {
                        return Err(Ice::global(format!(
                            "initializer relocation is not an address: {other:?}"
                        )))
                    }
                }
            }
            run(&mut pieces, at, bytes.len() as u64);
            pad(&mut pieces, bytes.len() as u64, size);
        }
        ConstInit::Aggregate(items) if ty.array_elem().is_some() => {
            let elem = ty.array_elem().expect("array element");
            let parts = items
                .iter()
                .map(|item| render_typed_const(elem, item, renames))
                .collect::<Result<Vec<_>, _>>()?;
            if parts.iter().all(|(ir, _)| *ir == parts[0].0) && !parts.is_empty() {
                let body = parts
                    .iter()
                    .map(|(ir, spelled)| format!("{ir} {spelled}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                return Ok((
                    format!("[{} x {}]", parts.len(), parts[0].0),
                    format!("[{body}]"),
                ));
            }
            let step = elem.size().unwrap_or(0);
            pieces.extend(parts);
            pad(&mut pieces, step * items.len() as u64, size);
        }
        ConstInit::Aggregate(items) => {
            let fields = ty.struct_ref().and_then(StructRef::fields).ok_or_else(|| {
                Ice::global("aggregate initializer with addresses is not a struct or array")
            })?;
            let mut at = 0u64;
            for (field, item) in fields.iter().zip(items) {
                pad(&mut pieces, at, field.offset);
                pieces.push(render_typed_const(&field.ty, item, renames)?);
                at = at.max(field.offset + field.ty.size().unwrap_or(0));
            }
            pad(&mut pieces, at, size);
        }
        other => return Err(Ice::global(format!("constant {other:?} has no byte image"))),
    }
    let types = pieces
        .iter()
        .map(|(ir, _)| ir.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let body = pieces
        .iter()
        .map(|(ir, spelled)| format!("{ir} {spelled}"))
        .collect::<Vec<_>>()
        .join(", ");
    Ok((format!("<{{ {types} }}>"), format!("<{{ {body} }}>")))
}

pub(super) fn render_const(
    ty: &CType,
    value: &ConstInit,
    renames: &Renames,
) -> Result<String, Ice> {
    if ty.is_union() || ty.record_has_bitfield() {
        return Ok(render_union_const(ty, value));
    }
    let spelled = match value {
        ConstInit::Zero => zero_const_spelling(ty),
        ConstInit::Int(bits) => ty
            .as_integer()
            .ok_or_else(|| Ice::global("integer initializer for an object of another type"))?
            .format_bits(*bits),
        ConstInit::Float(bits) => {
            let kind = ty
                .float_kind()
                .ok_or_else(|| Ice::global("floating initializer for an object of another type"))?;
            crate::float::llvm_const(kind, *bits)
        }
        ConstInit::Ptr(ptr) => render_ptr(ptr, renames),
        ConstInit::PtrToInt { ptr, width } => {
            let ir = match width {
                8 => "i8",
                16 => "i16",
                32 => "i32",
                _ => "i64",
            };
            format!("ptrtoint ({} to {ir})", render_ptr_operand(ptr, renames))
        }
        ConstInit::Bytes(raw) => render_byte_blob(ty, raw),
        ConstInit::RelocBytes { .. } => {
            return Err(Ice::global(
                "an initializer with addresses reached the plain constant printer",
            ))
        }
        ConstInit::Aggregate(items) => {
            if let Some(fields) = ty.struct_ref().and_then(StructRef::fields) {
                if fields.len() != items.len() {
                    return Ok("zeroinitializer".to_string());
                }
                let parts = items
                    .iter()
                    .zip(fields.iter())
                    .map(|(item, field)| {
                        Ok(format!(
                            "{} {}",
                            field.ty.ir_type(),
                            render_const(&field.ty, item, renames)?
                        ))
                    })
                    .collect::<Result<Vec<_>, Ice>>()?
                    .join(", ");
                return Ok(format!("{{ {parts} }}"));
            }
            let Some(elem) = ty.array_elem() else {
                return Ok("zeroinitializer".to_string());
            };
            if let Some(bytes) = i8_bytes(ty, items) {
                return Ok(format!("c{}", llvm_bytes(&bytes)));
            }
            let parts = items
                .iter()
                .map(|item| {
                    Ok(format!(
                        "{} {}",
                        elem.ir_type(),
                        render_const(elem, item, renames)?
                    ))
                })
                .collect::<Result<Vec<_>, Ice>>()?
                .join(", ");
            format!("[{parts}]")
        }
    };
    Ok(spelled)
}

/// A flat array of byte-sized integers can be spelled as an LLVM `c"..."`
/// constant. Wider elements keep the `[i32 1, i32 2]` form.
pub(super) fn i8_bytes(ty: &CType, items: &[ConstInit]) -> Option<Vec<u8>> {
    let elem = ty.array_elem()?;
    if elem.as_integer().map(TypeName::width) != Some(8) {
        return None;
    }
    if items.len() != ty.array_len()? as usize {
        return None;
    }
    let mut bytes = Vec::with_capacity(items.len());
    for item in items {
        match item {
            ConstInit::Int(bits) => bytes.push(*bits as u8),
            ConstInit::Zero => bytes.push(0),
            _ => return None,
        }
    }
    Some(bytes)
}

pub(super) fn zero_const_spelling(ty: &CType) -> String {
    if let Some(int) = ty.as_integer() {
        int.format_bits(0)
    } else if ty.is_pointer() {
        "null".to_string()
    } else {
        "zeroinitializer".to_string()
    }
}

pub(super) fn render_ptr(ptr: &PtrConst, renames: &Renames) -> String {
    match ptr {
        PtrConst::Null => "null".to_string(),
        PtrConst::IntToPtr(bits) => format!("inttoptr (i64 {} to ptr)", i64_literal(*bits)),
        PtrConst::Addr { name, offset } if *offset == 0 => llvm_global(renamed(name, renames)),
        PtrConst::Addr { name, offset } => {
            format!(
                "getelementptr (i8, ptr {}, i64 {offset})",
                llvm_global(renamed(name, renames))
            )
        }
        PtrConst::Label { func, label } => format!(
            "blockaddress({}, %{})",
            llvm_global(renamed(func, renames)),
            c_label(label)
        ),
    }
}

pub(super) fn render_ptr_operand(ptr: &PtrConst, renames: &Renames) -> String {
    format!("ptr {}", render_ptr(ptr, renames))
}

pub(super) fn i64_literal(bits: u64) -> String {
    let signed = bits as i64;
    if signed < 0 {
        signed.to_string()
    } else {
        bits.to_string()
    }
}
