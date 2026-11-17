//! Byte images of static initializers: packing scalars, bit-fields, unions
//! and relocated addresses into the bytes of an object.

use super::*;

/// An object representation being built for a union or a bit-field
/// record: its bytes, plus the addresses that need a relocation.
///
/// An address cannot be written as bytes, so it is kept beside them at its
/// byte offset with zeros underneath. A later store that overlaps it
/// replaces it, the same way later bytes replace earlier ones.
#[derive(Clone)]
pub(super) struct ByteImage {
    pub(super) bytes: Vec<u8>,
    pub(super) relocs: Vec<(u64, ConstInit)>,
}

impl ByteImage {
    pub(super) fn zeroed(size: usize) -> Self {
        ByteImage {
            bytes: vec![0u8; size],
            relocs: Vec::new(),
        }
    }

    /// Drop every relocation that overlaps `start..start + len`.
    pub(super) fn clear_relocs(&mut self, start: usize, len: usize) {
        let (start, end) = (start as u64, (start + len) as u64);
        self.relocs
            .retain(|(at, value)| at + reloc_width(value) <= start || *at >= end);
    }

    pub(super) fn zero(&mut self, start: usize, len: usize) {
        let end = (start + len).min(self.bytes.len());
        if start < end {
            self.bytes[start..end].fill(0);
        }
    }

    /// Write `value`, of type `ty`, at byte `offset`. `false` means the
    /// value has no object representation here (an aggregate).
    pub(super) fn write(&mut self, ty: &CType, value: &ConstInit, offset: usize) -> bool {
        match value {
            ConstInit::Ptr(PtrConst::Addr { .. } | PtrConst::Label { .. })
            | ConstInit::PtrToInt { .. } => {
                let len = reloc_width(value) as usize;
                self.clear_relocs(offset, len);
                self.zero(offset, len);
                self.relocs.push((offset as u64, value.clone()));
                self.relocs.sort_by_key(|(at, _)| *at);
                true
            }
            ConstInit::RelocBytes { bytes, relocs } => {
                self.clear_relocs(offset, bytes.len());
                for (index, byte) in bytes.iter().enumerate() {
                    if let Some(slot) = self.bytes.get_mut(offset + index) {
                        *slot = *byte;
                    }
                }
                for (at, reloc) in relocs {
                    self.relocs.push((offset as u64 + at, reloc.clone()));
                }
                self.relocs.sort_by_key(|(at, _)| *at);
                true
            }
            ConstInit::Bytes(raw) => {
                self.clear_relocs(offset, raw.len());
                for (index, byte) in raw.iter().enumerate() {
                    if let Some(slot) = self.bytes.get_mut(offset + index) {
                        *slot = *byte;
                    }
                }
                true
            }
            _ => {
                self.clear_relocs(offset, ty.size().unwrap_or(0) as usize);
                write_folded_bytes(ty, value, &mut self.bytes, offset)
            }
        }
    }

    pub(super) fn pack_bits(&mut self, bit: u32, width: u32, value: u128) {
        let first = (bit / 8) as usize;
        let last = ((bit + width).div_ceil(8)) as usize;
        self.clear_relocs(first, last.saturating_sub(first));
        pack_bits(&mut self.bytes, bit, width, value);
    }

    /// The `size` bytes at `start`, with the relocations inside them.
    /// `None` when a relocation starts inside and ends outside: its address
    /// cannot be split across two pieces.
    pub(super) fn slice(&self, start: usize, size: usize) -> Option<ConstInit> {
        let mut bytes = vec![0u8; size];
        if start < self.bytes.len() {
            let end = (start + size).min(self.bytes.len());
            bytes[..end - start].copy_from_slice(&self.bytes[start..end]);
        }
        let (lo, hi) = (start as u64, (start + size) as u64);
        if self.relocs.iter().any(|(at, value)| {
            let end = at + reloc_width(value);
            (*at >= lo && *at < hi && end > hi) || (*at < lo && end > lo)
        }) {
            return None;
        }
        let relocs: Vec<_> = self
            .relocs
            .iter()
            .filter(|(at, _)| *at >= lo && *at < hi)
            .map(|(at, value)| (at - lo, value.clone()))
            .collect();
        if relocs.is_empty() {
            Some(ConstInit::Bytes(bytes))
        } else {
            Some(ConstInit::RelocBytes { bytes, relocs })
        }
    }

    pub(super) fn into_const(self) -> ConstInit {
        let size = self.bytes.len();
        self.slice(0, size)
            .expect("an address written into an image lies inside it")
    }
}

/// Bytes covered by a relocation in a [`ByteImage`].
pub(super) fn reloc_width(value: &ConstInit) -> u64 {
    match value {
        ConstInit::PtrToInt { width, .. } => u64::from(*width / 8),
        _ => 8,
    }
}

/// A union or bit-field record already rendered as an image.
pub(super) fn is_image_const(value: &ConstInit) -> bool {
    matches!(value, ConstInit::Bytes(_) | ConstInit::RelocBytes { .. })
}

pub(super) fn pack_bits(bytes: &mut [u8], bit: u32, width: u32, value: u128) {
    let mask = if width >= 128 {
        u128::MAX
    } else {
        (1u128 << width) - 1
    };
    let value = value & mask;
    for i in 0..width {
        let pos = bit + i;
        let index = (pos / 8) as usize;
        if index >= bytes.len() {
            break;
        }
        let shift = pos % 8;
        if (value >> i) & 1 == 1 {
            bytes[index] |= 1 << shift;
        } else {
            bytes[index] &= !(1 << shift);
        }
    }
}

pub(super) fn write_folded_bytes(
    ty: &CType,
    value: &ConstInit,
    bytes: &mut [u8],
    offset: usize,
) -> bool {
    match value {
        ConstInit::Zero => true,
        ConstInit::Int(bits) => {
            let width = ty.as_integer().map(TypeName::width).unwrap_or(0) as usize / 8;
            for index in 0..width {
                let at = offset + index;
                if at < bytes.len() {
                    bytes[at] = (*bits >> (8 * index)) as u8;
                }
            }
            true
        }
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
        ConstInit::Ptr(PtrConst::Null) | ConstInit::Ptr(PtrConst::IntToPtr(_)) => {
            let bits = match value {
                ConstInit::Ptr(PtrConst::IntToPtr(bits)) => *bits,
                _ => 0,
            };
            for index in 0..8 {
                let at = offset + index;
                if at < bytes.len() {
                    bytes[at] = (bits >> (8 * index)) as u8;
                }
            }
            true
        }
        _ => false,
    }
}

/// A union constant is rendered as raw bytes. An address in that constant
/// needs a relocation, which those bytes cannot hold.
pub(super) fn union_bytes_ok(ty: &CType, value: &ConstInit) -> bool {
    match value {
        ConstInit::Zero | ConstInit::Int(_) | ConstInit::Float(_) => true,
        // A null pointer and an integer converted to a pointer are bytes.
        // The address of an object needs a relocation.
        ConstInit::Ptr(PtrConst::Null | PtrConst::IntToPtr(_)) => true,
        ConstInit::Ptr(PtrConst::Addr { .. } | PtrConst::Label { .. })
        | ConstInit::PtrToInt { .. } => !ty.is_union(),
        // Bytes are an object representation. Addresses in an image are
        // explicit relocations that the IR generator spells out.
        ConstInit::Bytes(_) | ConstInit::RelocBytes { .. } => true,
        ConstInit::Aggregate(items) => {
            if ty.is_union() {
                return items.first().is_none_or(const_has_no_address);
            }
            if let Some(fields) = ty.struct_ref().and_then(StructRef::fields) {
                return fields
                    .iter()
                    .zip(items)
                    .all(|(field, item)| union_bytes_ok(&field.ty, item));
            }
            if let Some(elem) = ty.array_elem() {
                return items.iter().all(|item| union_bytes_ok(elem, item));
            }
            true
        }
    }
}

pub(super) fn const_has_no_address(value: &ConstInit) -> bool {
    match value {
        ConstInit::Zero | ConstInit::Int(_) | ConstInit::Float(_) => true,
        ConstInit::Ptr(PtrConst::Null | PtrConst::IntToPtr(_)) => true,
        ConstInit::Ptr(PtrConst::Addr { .. } | PtrConst::Label { .. })
        | ConstInit::PtrToInt { .. }
        | ConstInit::RelocBytes { .. } => false,
        ConstInit::Bytes(_) => true,
        ConstInit::Aggregate(items) => items.iter().all(const_has_no_address),
    }
}

/// Byte images of the plain unions nested in a static initializer.
///
/// Every member of a union shares its bytes, and the LLVM type of a union
/// is its first member. A store to another member (`{ .u.d = 2.5 }`) is
/// written into the union's object representation instead, the way a
/// top-level union is folded. A bit-field union is already a byte image.
pub(super) struct UnionImages {
    /// Sorted and disjoint.
    pub(super) ranges: Vec<(u64, u64)>,
    /// An image is made when the first store lands in its union.
    pub(super) images: Vec<Option<ByteImage>>,
    pub(super) bad: Option<(usize, usize)>,
}

impl UnionImages {
    pub(super) fn new(ty: &CType) -> Self {
        let mut ranges = Vec::new();
        union_ranges(ty, 0, true, &mut ranges);
        let images = vec![None; ranges.len()];
        UnionImages {
            ranges,
            images,
            bad: None,
        }
    }

    /// Write a store that falls in a nested union. `false` means it is
    /// outside every union and is assembled as usual.
    pub(super) fn absorb(
        &mut self,
        store: &ElemStore,
        value: &ConstInit,
    ) -> Result<bool, Diagnostic> {
        let index = self
            .ranges
            .partition_point(|(start, _)| *start <= store.offset);
        if index == 0 || store.offset >= self.ranges[index - 1].1 {
            return Ok(false);
        }
        let (start, end) = self.ranges[index - 1];
        let image =
            self.images[index - 1].get_or_insert_with(|| ByteImage::zeroed((end - start) as usize));
        let at = (store.offset - start) as usize;
        if !image.write(&store.ty, value, at) && self.bad.is_none() {
            self.bad = Some((store.expr.line(), store.expr.column()));
        }
        Ok(true)
    }

    pub(super) fn finish(
        self,
        name: &str,
        sema: &Sema<'_>,
        folded: &mut Vec<(u64, ConstInit)>,
    ) -> Result<(), Diagnostic> {
        if let Some((line, column)) = self.bad {
            return sema.err(
                line,
                column,
                format!("initializer of '{name}' uses an address in a union"),
            );
        }
        for ((start, _), image) in self.ranges.into_iter().zip(self.images) {
            if let Some(image) = image {
                folded.push((start, image.into_const()));
            }
        }
        Ok(())
    }
}

/// Byte ranges of the outermost plain unions inside `ty`. `top` skips `ty`
/// itself, which the caller folds on its own.
pub(super) fn union_ranges(ty: &CType, base: u64, top: bool, out: &mut Vec<(u64, u64)>) {
    if is_bit_image(ty) {
        return;
    }
    if ty.is_union() && !top {
        out.push((base, base + ty.size().unwrap_or(0)));
        return;
    }
    if let Some(fields) = ty.struct_ref().and_then(StructRef::fields) {
        for field in fields.iter().filter(|field| field.bit_width.is_none()) {
            union_ranges(&field.ty, base + field.offset, false, out);
        }
        return;
    }
    if let (Some(elem), Some(len)) = (ty.array_elem(), ty.array_len()) {
        let step = elem.size().unwrap_or(0);
        if elem.is_scalar() {
            return;
        }
        for index in 0..len {
            union_ranges(elem, base + index * step, false, out);
        }
    }
}

pub(super) fn assemble(ty: &CType, stores: &[(u64, ConstInit)], base: u64) -> ConstInit {
    if ty.is_union() {
        if let Some((_, value)) = stores
            .iter()
            .rev()
            .find(|(offset, value)| *offset == base && is_image_const(value))
        {
            return value.clone();
        }
    }
    if is_bit_image(ty) {
        if let Some((_, value)) = stores
            .iter()
            .rev()
            .find(|(offset, value)| *offset == base && is_image_const(value))
        {
            return value.clone();
        }
        let size = ty.size().unwrap_or(0) as usize;
        return ConstInit::Bytes(vec![0u8; size]);
    }
    if ty.is_scalar() {
        return stores
            .iter()
            .rev()
            .find(|(offset, _)| *offset == base)
            .map(|(_, value)| value.clone())
            .unwrap_or_else(|| zero_const(ty));
    }
    if let Some(fields) = ty.struct_ref().and_then(StructRef::fields) {
        // A union's initializer describes the first member. The other members
        // occupy the same bytes; copying the store into each of them would
        // reinterpret those bits as a different type.
        let fields: Vec<_> = if ty.is_union() {
            fields.into_iter().take(1).collect()
        } else {
            fields
        };
        let mut items = Vec::new();
        for field in &fields {
            items.push(assemble(&field.ty, stores, base + field.offset));
        }
        return ConstInit::Aggregate(items);
    }
    let CType::Array(elem, Some(len)) = ty else {
        return ConstInit::Zero;
    };
    let elem_size = elem.size().unwrap_or(0);
    let mut items = Vec::new();
    for index in 0..*len {
        items.push(assemble(elem, stores, base + index * elem_size));
    }
    ConstInit::Aggregate(items)
}

/// A bit-field record, or a union that contains one in any member.
///
/// Union members share one byte image. A bit-field in a member other than
/// the first still occupies those bytes, so the whole union is one image.
/// Each record is examined once: two members of the same union are one type,
/// and searching both would walk that type's graph twice at every level.
pub(super) fn is_bit_image(ty: &CType) -> bool {
    let mut seen = HashSet::new();
    is_bit_image_seen(ty, &mut seen)
}

pub(super) fn is_bit_image_seen(ty: &CType, seen: &mut HashSet<u32>) -> bool {
    if ty.record_has_bitfield() {
        return true;
    }
    let Some(handle) = ty.struct_ref() else {
        return false;
    };
    if !handle.is_union() || !seen.insert(handle.id()) {
        return false;
    }
    handle.fields().is_some_and(|fields| {
        fields
            .iter()
            .any(|field| field.bit_width.is_some() || is_bit_image_seen(&field.ty, seen))
    })
}

/// A bit-field record or a union that holds a bit-field record anywhere in
/// its members, including through a struct or an array member.
///
/// All members of a union share its bytes (C11 6.7.2.1p16), so such a union
/// is assembled as one complete byte image. Slicing out only the bit-field
/// record of its first member would lose a longer member's bytes, and with
/// them the union's size and the C offsets of the fields after it.
pub(super) fn is_image_record(ty: &CType) -> bool {
    if is_bit_image(ty) {
        return true;
    }
    ty.is_union() && holds_bit_record(ty, &mut HashSet::new())
}

pub(super) fn holds_bit_record(ty: &CType, seen: &mut HashSet<u32>) -> bool {
    if ty.record_has_bitfield() {
        return true;
    }
    if let Some(elem) = ty.array_elem() {
        return holds_bit_record(elem, seen);
    }
    let Some(handle) = ty.struct_ref() else {
        return false;
    };
    if !seen.insert(handle.id()) {
        return false;
    }
    handle.fields().is_some_and(|fields| {
        fields
            .iter()
            .any(|field| field.bit_width.is_some() || holds_bit_record(&field.ty, seen))
    })
}

/// Byte ranges of nested records whose LLVM type is a byte image.
pub(super) fn bit_record_ranges(ty: &CType, base: u64, out: &mut Vec<(u64, u64)>) {
    if is_image_record(ty) {
        out.push((base, base + ty.size().unwrap_or(0)));
        return;
    }
    if let Some(fields) = ty.struct_ref().and_then(StructRef::fields) {
        let fields: Vec<_> = if ty.is_union() {
            fields.into_iter().take(1).collect()
        } else {
            fields
        };
        for field in fields {
            if field.bit_width.is_some() {
                continue;
            }
            bit_record_ranges(&field.ty, base + field.offset, out);
        }
        return;
    }
    if let (Some(elem), Some(len)) = (ty.array_elem(), ty.array_len()) {
        let step = elem.size().unwrap_or(0);
        for index in 0..len {
            bit_record_ranges(elem, base + index * step, out);
        }
    }
}

/// Replace each nested bit-field record, including a union that contains
/// one in any member, with the bytes already packed into `image`.
///
/// C11 6.7.9 initializes the designated union member and leaves the rest of
/// that union's object representation zero. Every member occupies the same
/// bytes, so a bit-field in a member other than the first is still this image.
///
/// `false` means an address in `image` crosses the edge of a record, so it
/// has no place in any one record's bytes.
pub(super) fn splice_bit_blobs(
    ty: &CType,
    base: u64,
    image: &ByteImage,
    out: &mut Vec<(u64, ConstInit)>,
) -> bool {
    if is_image_record(ty) {
        let size = ty.size().unwrap_or(0) as usize;
        let Some(blob) = image.slice(base as usize, size) else {
            return false;
        };
        out.push((base, blob));
        return true;
    }
    if let Some(fields) = ty.struct_ref().and_then(StructRef::fields) {
        let fields: Vec<_> = if ty.is_union() {
            fields.into_iter().take(1).collect()
        } else {
            fields
        };
        for field in fields {
            if field.bit_width.is_some() {
                continue;
            }
            if !splice_bit_blobs(&field.ty, base + field.offset, image, out) {
                return false;
            }
        }
        return true;
    }
    if let (Some(elem), Some(len)) = (ty.array_elem(), ty.array_len()) {
        let step = elem.size().unwrap_or(0);
        for index in 0..len {
            if !splice_bit_blobs(elem, base + index * step, image, out) {
                return false;
            }
        }
    }
    true
}

pub(super) fn ptr_as_int(ptr: PtrConst, dest: TypeName) -> ConstInit {
    match ptr {
        PtrConst::Null => ConstInit::Int(0),
        PtrConst::IntToPtr(bits) => {
            ConstInit::Int(convert_bits(u128::from(bits), TypeName::ULong, dest))
        }
        other => ConstInit::PtrToInt {
            ptr: other,
            width: dest.width(),
        },
    }
}

pub(super) fn eval_ptr(expr: &Expr) -> Result<PtrConst, ConstError> {
    if is_null_pointer_constant(expr)
        && expr
            .ty()
            .is_some_and(|ty| ty.is_integer() || ty.is_pointer())
    {
        return Ok(PtrConst::Null);
    }
    match expr {
        Expr::LabelAddr {
            name,
            func: Some(func),
            ..
        } => Ok(PtrConst::Label {
            func: func.clone(),
            label: name.clone(),
        }),
        Expr::Unary {
            op: UnaryOp::Addr,
            expr,
            ..
        } => {
            let (name, offset) = designator(expr)?;
            Ok(PtrConst::Addr { name, offset })
        }
        Expr::Decay { expr, .. } => {
            let (name, offset) = designator(expr)?;
            Ok(PtrConst::Addr { name, offset })
        }
        Expr::Bin {
            op: BinOp::Add,
            left,
            right,
            ..
        } => add_ptr_const(left, right, false),
        Expr::Bin {
            op: BinOp::Sub,
            left,
            right,
            ..
        } if left.ty().is_some_and(|ty| ty.is_pointer())
            && right.ty().is_some_and(|ty| ty.is_integer()) =>
        {
            add_ptr_const(left, right, true)
        }
        Expr::Cast { ty_name, expr, .. } if ty_name.is_pointer() => {
            if expr.ty().is_some_and(|ty| ty.is_pointer()) {
                eval_ptr(expr)
            } else {
                int_expr_to_ptr(expr)
            }
        }
        Expr::Cond {
            cond,
            then_expr,
            else_expr,
            ..
        } => {
            let cond = eval_const_expr(cond)?;
            let chosen = if is_zero(&cond) { else_expr } else { then_expr };
            eval_ptr(chosen)
        }
        _ => Err(ConstError::NotConst),
    }
}

pub(super) fn add_ptr_const(
    left: &Expr,
    right: &Expr,
    negate: bool,
) -> Result<PtrConst, ConstError> {
    let (ptr_expr, int_expr) = if left.ty().is_some_and(|ty| ty.is_pointer()) {
        (left, right)
    } else {
        (right, left)
    };
    let base = eval_ptr(ptr_expr)?;
    let mut index = const_index(int_expr)?;
    if negate {
        index = index
            .checked_neg()
            .ok_or(ConstError::Overflow(TypeName::Long))?;
    }
    let size = ptr_expr
        .ty()
        .as_ref()
        .and_then(CType::pointee)
        .and_then(CType::size)
        .ok_or(ConstError::NotConst)? as i64;
    let delta = index
        .checked_mul(size)
        .ok_or(ConstError::Overflow(TypeName::Long))?;
    match base {
        PtrConst::Addr { name, offset } => Ok(PtrConst::Addr {
            name,
            offset: offset
                .checked_add(delta)
                .ok_or(ConstError::Overflow(TypeName::Long))?,
        }),
        PtrConst::Null => {
            if delta == 0 {
                Ok(PtrConst::Null)
            } else {
                Ok(PtrConst::IntToPtr(delta as u64))
            }
        }
        PtrConst::IntToPtr(bits) => Ok(PtrConst::IntToPtr(bits.wrapping_add(delta as u64))),
        // A label address is only ever used whole.
        PtrConst::Label { .. } if delta == 0 => Ok(base),
        PtrConst::Label { .. } => Err(ConstError::NotConst),
    }
}

/// An integer expression used as a pointer: a constant, or a cast of an
/// address through an integer. A 64-bit integer keeps every address bit.
/// A narrower integer keeps the low bits and extends them back the way the
/// integer type does (sign-extend signed, zero-extend unsigned).
pub(super) fn int_expr_to_ptr(expr: &Expr) -> Result<PtrConst, ConstError> {
    if let Expr::Cast {
        ty_name,
        expr: inner,
        ..
    } = expr
    {
        if let Some(int_ty) = ty_name.as_integer() {
            if let Ok(ptr) = eval_ptr(inner) {
                return narrow_ptr(ptr, int_ty);
            }
        }
    }
    let value = eval_const_expr(expr)?;
    let bits = convert_bits(value.bits, value.ty, TypeName::ULong);
    if bits == 0 {
        Ok(PtrConst::Null)
    } else {
        Ok(PtrConst::IntToPtr(bits as u64))
    }
}

pub(super) fn narrow_ptr(ptr: PtrConst, int_ty: TypeName) -> Result<PtrConst, ConstError> {
    if int_ty.width() >= 64 {
        return Ok(ptr);
    }
    match ptr {
        PtrConst::Null => Ok(PtrConst::Null),
        PtrConst::IntToPtr(bits) => {
            let narrowed = convert_bits(u128::from(bits), TypeName::ULong, int_ty);
            Ok(PtrConst::IntToPtr(
                convert_bits(narrowed, int_ty, TypeName::ULong) as u64,
            ))
        }
        // A file-scope address truncated to 32 bits is still a relocation.
        // Only the low bits survive, so it is not the original address constant.
        PtrConst::Addr { .. } | PtrConst::Label { .. } => Err(ConstError::NotConst),
    }
}

pub(super) fn const_index(expr: &Expr) -> Result<i64, ConstError> {
    let value = eval_const_expr(expr)?;
    if value.ty.is_signed() {
        i64::try_from(sign_extend(value.bits, value.ty.width()))
            .map_err(|_| ConstError::Overflow(value.ty))
    } else {
        let bits = value.bits & value.ty.mask();
        if bits > i64::MAX as u128 {
            Err(ConstError::Overflow(value.ty))
        } else {
            Ok(bits as i64)
        }
    }
}

pub(super) fn designator(expr: &Expr) -> Result<(String, i64), ConstError> {
    match expr {
        Expr::Var { resolved, .. } => {
            let slot = &resolved.as_ref().ok_or(ConstError::NotConst)?.slot;
            let Some(name) = slot.strip_prefix('@') else {
                return Err(ConstError::NotConst);
            };
            Ok((name.to_string(), 0))
        }
        Expr::Str { symbol, .. } => {
            let name = symbol.clone().ok_or(ConstError::NotConst)?;
            Ok((name, 0))
        }
        Expr::Index {
            base, index, ty, ..
        } => {
            let elem = ty
                .as_ref()
                .and_then(CType::size)
                .ok_or(ConstError::NotConst)? as i64;
            let step = const_index(index)?;
            let (name, offset) = pointer_object(base)?;
            Ok((
                name,
                offset
                    .checked_add(
                        step.checked_mul(elem)
                            .ok_or(ConstError::Overflow(TypeName::Long))?,
                    )
                    .ok_or(ConstError::Overflow(TypeName::Long))?,
            ))
        }
        Expr::Unary {
            op: UnaryOp::Deref,
            expr,
            ..
        } => pointer_object(expr),
        Expr::Member {
            base,
            offset,
            arrow,
            ..
        } => {
            let member_off = i64::try_from(offset.ok_or(ConstError::NotConst)?)
                .map_err(|_| ConstError::Overflow(TypeName::Long))?;
            let (name, base_off) = if *arrow {
                pointer_object(base)?
            } else {
                designator(base)?
            };
            Ok((
                name,
                base_off
                    .checked_add(member_off)
                    .ok_or(ConstError::Overflow(TypeName::Long))?,
            ))
        }
        _ => Err(ConstError::NotConst),
    }
}

pub(super) fn pointer_object(expr: &Expr) -> Result<(String, i64), ConstError> {
    match eval_ptr(expr)? {
        PtrConst::Addr { name, offset } => Ok((name, offset)),
        _ => Err(ConstError::NotConst),
    }
}
