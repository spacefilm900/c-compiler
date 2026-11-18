//! System V AMD64 classification for aggregates passed or returned by value.
//!
//! The rules are the ones in the psABI (section 3.2.3). Each aggregate is
//! split into eightbyte chunks. Integer and pointer chunks travel in general
//! registers. An SSE chunk travels in an XMM register: one `float`, one
//! `double`, or `<2 x float>` when two floats share the eightbyte. INTEGER
//! wins over SSE in the same eightbyte. An x87 aggregate is passed in memory.
//! Returning one whose eightbytes are `X87` then `X87UP` uses `%st0`
//! (`x86_fp80`). Any other x87 shape is memory. An aggregate larger than two
//! eightbytes, or one with an unaligned field, is passed in memory.
//!
//! LLVM IR for this ABI does not pass the struct type itself. A register
//! aggregate is coerced to integer pieces (or `ptr` when an eightbyte is
//! exactly one pointer). A memory aggregate is a `byval` parameter or an
//! `sret` return slot.

use crate::ast::{align_up, CType, FloatKind};
use crate::target::{Abi, Target};

/// Why an aggregate cannot be lowered yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiFail {
    /// The struct or union has not been completed, so it has no size.
    Incomplete,
    /// Kept so a future target can reject a class this one lowers. No
    /// aggregate on x86_64 System V uses it: x87 returns are [`AggAbi::X87`].
    FloatReg,
}

/// One register-sized piece of an aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegPiece {
    /// Little-endian integer of `bits` bits (8, 16, 24, 32, 40, 48, 56, or 64)
    /// covering `offset` bytes into the object.
    Int { bits: u32, offset: u64 },
    /// A pointer that occupies the whole eightbyte at `offset`.
    Ptr { offset: u64 },
    /// One XMM register at `offset`. Two `float` fields share `<2 x float>`.
    Sse { kind: SsePiece, offset: u64 },
}

/// The LLVM type of one SSE eightbyte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SsePiece {
    Float,
    Double,
    /// Two `float` values in the low 64 bits of one XMM register.
    FloatPair,
}

impl SsePiece {
    fn ir(self) -> &'static str {
        match self {
            SsePiece::Float => "float",
            SsePiece::Double => "double",
            SsePiece::FloatPair => "<2 x float>",
        }
    }
}

impl RegPiece {
    pub fn offset(&self) -> u64 {
        match self {
            RegPiece::Int { offset, .. }
            | RegPiece::Ptr { offset }
            | RegPiece::Sse { offset, .. } => *offset,
        }
    }

    pub fn is_sse(&self) -> bool {
        matches!(self, RegPiece::Sse { .. })
    }

    pub fn ir_type(&self) -> String {
        match self {
            RegPiece::Ptr { .. } => "ptr".to_string(),
            RegPiece::Int { bits, .. } => format!("i{bits}"),
            RegPiece::Sse { kind, .. } => kind.ir().to_string(),
        }
    }
}

/// How an aggregate is passed or returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AggAbi {
    /// One or two register pieces, in eightbyte order.
    Direct(Vec<RegPiece>),
    /// The caller allocates memory. Parameters use `byval`; returns use `sret`.
    Memory,
    /// Returned in `%st0` as `x86_fp80`. The psABI uses this when the
    /// eightbytes are `X87` followed by `X87UP` (a `long double` aggregate).
    /// The same class is passed in memory, so parameters never use this.
    X87,
    /// AAPCS64: one LLVM value of type `ty` whose bytes are the start of
    /// the object's representation (`i64`, `[2 x i64]`, `i128`, an exact
    /// `iN` return, or an HFA `[N x float]`). `size` is the store size of
    /// `ty`, which may exceed the object's size; the extra bytes are
    /// unspecified.
    Coerce { ty: String, size: u64 },
    /// AAPCS64: a parameter larger than 16 bytes is copied by the caller,
    /// and a pointer to the copy is passed (no `byval`).
    Indirect,
}

impl AggAbi {
    /// LLVM type of a register return: one piece, or an anonymous struct of them.
    pub fn direct_type(&self) -> Option<String> {
        match self {
            AggAbi::Direct(pieces) => Some(pieces_type(pieces)),
            AggAbi::Coerce { ty, .. } => Some(ty.clone()),
            _ => None,
        }
    }
}

pub fn pieces_type(pieces: &[RegPiece]) -> String {
    if pieces.len() == 1 {
        pieces[0].ir_type()
    } else {
        let body = pieces
            .iter()
            .map(RegPiece::ir_type)
            .collect::<Vec<_>>()
            .join(", ");
        format!("{{ {body} }}")
    }
}

/// One LLVM parameter that corresponds to part of a C parameter, or to `sret`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    pub ir_type: String,
    pub attrs: String,
    pub kind: PartKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartKind {
    Scalar,
    Piece(RegPiece),
    Byval,
    /// AAPCS64 [`AggAbi::Coerce`]: the whole aggregate as one LLVM value
    /// whose type has `size` bytes.
    Coerce {
        size: u64,
    },
    /// AAPCS64 [`AggAbi::Indirect`]: a pointer to a caller-made copy.
    Indirect,
}

/// Lowered signature of a C function. `params[i]` is the LLVM expansion of
/// C parameter `i`. `sret_attrs` is set when the return value is in memory;
/// that pointer is a hidden first argument and the LLVM return type is `void`.
/// `free_int` is how many of the six general registers remain after the fixed
/// parameters, so a variadic argument can continue the same budget.
/// `free_sse` is the same count for the eight XMM registers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnSig {
    pub ret_type: String,
    pub ret_prefix: String,
    pub ret_agg: Option<AggAbi>,
    pub sret_attrs: Option<String>,
    pub params: Vec<Vec<Part>>,
    pub free_int: usize,
    pub free_sse: usize,
}

/// Whether the aggregate is a parameter or a return value.
///
/// The psABI passes an `X87` / `X87UP` argument in memory, and returns that
/// pair on the x87 stack. A return whose eightbytes are `X87` then `X87UP`
/// is [`AggAbi::X87`]. Every other x87 shape is [`AggAbi::Memory`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggUse {
    Param,
    Return,
}

pub fn function_sig(ret: &CType, params: &[CType]) -> Result<FnSig, AbiFail> {
    if let Abi::Aapcs64 { .. } = crate::target::current().abi() {
        return crate::aapcs64::function_sig(ret, params);
    }
    let (ret_type, ret_prefix, ret_agg, sret_attrs) = lower_return(ret)?;
    // Six general registers. The hidden sret pointer takes the first.
    let mut free_int = if sret_attrs.is_some() { 5 } else { 6 };
    let mut free_sse = 8;
    let mut lowered = Vec::with_capacity(params.len());
    for param in params {
        lowered.push(assign_regs(
            lower_param(param)?,
            param,
            &mut free_int,
            &mut free_sse,
        ));
    }
    Ok(FnSig {
        ret_type,
        ret_prefix,
        ret_agg,
        sret_attrs,
        params: lowered,
        free_int,
        free_sse,
    })
}

/// Place `parts` in the remaining general registers.
///
/// A scalar or a single eightbyte uses one register, or goes on the stack as
/// that same integer once the registers are full. A multi-eightbyte aggregate
/// needs a register for every eightbyte. If even one is missing, the psABI
/// passes the whole argument in memory and does not consume the registers
/// that were still free.
pub fn assign_regs(
    parts: Vec<Part>,
    ty: &CType,
    free_int: &mut usize,
    free_sse: &mut usize,
) -> Vec<Part> {
    let mut int_n = 0usize;
    let mut sse_n = 0usize;
    for part in &parts {
        if let PartKind::Piece(piece) = &part.kind {
            if piece.is_sse() {
                sse_n += 1;
            } else {
                int_n += 1;
            }
        }
    }
    if int_n + sse_n > 0 {
        // A lone integer eightbyte stays an integer when general registers
        // are exhausted; LLVM puts that integer on the stack. An SSE
        // eightbyte, and any aggregate that is short a register of either
        // class, is passed in memory and does not consume the registers
        // that are still free.
        let spill = if sse_n > 0 {
            sse_n > *free_sse || int_n > *free_int
        } else {
            int_n > 1 && int_n > *free_int
        };
        if spill {
            return vec![memory_part(ty)];
        }
        *free_int = free_int.saturating_sub(int_n);
        *free_sse = free_sse.saturating_sub(sse_n);
        return parts;
    }
    if parts
        .first()
        .is_some_and(|part| matches!(part.kind, PartKind::Scalar))
    {
        if ty.is_sse_float() {
            *free_sse = free_sse.saturating_sub(1);
        } else if !ty.is_long_double() {
            *free_int = free_int.saturating_sub(1);
        }
    }
    parts
}

pub fn memory_part(ty: &CType) -> Part {
    let align = ty.align().max(8);
    Part {
        ir_type: "ptr".to_string(),
        attrs: format!("byval({}) align {align}", ty.ir_type()),
        kind: PartKind::Byval,
    }
}

fn lower_return(ty: &CType) -> Result<(String, String, Option<AggAbi>, Option<String>), AbiFail> {
    if ty.is_void() {
        return Ok(("void".to_string(), String::new(), None, None));
    }
    if ty.unqualified().is_record() {
        let agg = classify_for(ty, AggUse::Return)?;
        return match &agg {
            AggAbi::Memory => Ok((
                "void".to_string(),
                String::new(),
                Some(agg),
                Some(format!("sret({}) align {}", ty.ir_type(), ty.align())),
            )),
            AggAbi::Direct(pieces) => Ok((pieces_type(pieces), String::new(), Some(agg), None)),
            AggAbi::X87 => Ok(("x86_fp80".to_string(), String::new(), Some(agg), None)),
            AggAbi::Coerce { .. } | AggAbi::Indirect => unreachable!("AAPCS64 class on System V"),
        };
    }
    let prefix = match ty.abi_attr() {
        Some(attr) => format!("{attr} "),
        None => String::new(),
    };
    Ok((ty.ir_type(), prefix, None, None))
}

fn lower_param(ty: &CType) -> Result<Vec<Part>, AbiFail> {
    if ty.unqualified().is_record() {
        let agg = classify_for(ty, AggUse::Param)?;
        return Ok(match agg {
            AggAbi::Memory | AggAbi::X87 | AggAbi::Indirect | AggAbi::Coerce { .. } => {
                vec![memory_part(ty)]
            }
            AggAbi::Direct(pieces) => pieces
                .into_iter()
                .map(|piece| Part {
                    ir_type: piece.ir_type(),
                    attrs: String::new(),
                    kind: PartKind::Piece(piece),
                })
                .collect(),
        });
    }
    Ok(vec![Part {
        ir_type: ty.ir_type(),
        attrs: ty.abi_attr().unwrap_or("").to_string(),
        kind: PartKind::Scalar,
    }])
}

/// Classify a complete struct or union used as a parameter.
///
/// Incomplete types fail. An SSE eightbyte is an XMM piece. An x87 aggregate
/// is passed in memory. A return whose eightbytes are `X87` then `X87UP` is
/// [`AggAbi::X87`] (`x86_fp80` in `%st0`). Any other x87 shape, including a
/// larger aggregate, is memory.
pub fn classify_aggregate(ty: &CType) -> Result<AggAbi, AbiFail> {
    classify_for(ty, AggUse::Param)
}

pub fn classify_for(ty: &CType, use_as: AggUse) -> Result<AggAbi, AbiFail> {
    if let Abi::Aapcs64 { .. } = crate::target::current().abi() {
        return crate::aapcs64::classify(ty, use_as);
    }
    let bare = ty.unqualified();
    if !bare.is_record() {
        return Ok(AggAbi::Memory);
    }
    let Some(size) = bare.size() else {
        return Err(AbiFail::Incomplete);
    };
    if size == 0 {
        return Ok(AggAbi::Memory);
    }
    // More than two eightbytes is memory for every type this compiler can
    // form. A pure SSE/SSEUP vector could stay in registers past 16 bytes;
    // nothing in this subset has that shape, and a later float step can
    // special-case it here without changing the integer path. The size test
    // comes before leaf collection so a large array member does not allocate
    // one leaf per element.
    if size > 16 {
        return Ok(AggAbi::Memory);
    }
    // Intel macOS, whose only C compiler is Clang: a struct with a flexible
    // array member is passed and returned in memory (observed with Clang as
    // a black box). On Linux this compiler keeps GCC's choice, registers.
    if let (Target::X86_64Apple { .. }, CType::Struct(handle)) =
        (crate::target::current(), bare.peel())
    {
        if handle.has_flexible_member() {
            return Ok(AggAbi::Memory);
        }
    }
    let mut leaves = Vec::new();
    collect_leaves(&bare, 0, false, &mut leaves);
    let unaligned = leaves
        .iter()
        .any(|leaf| leaf.offset % u64::from(leaf.align.max(1)) != 0);
    if unaligned {
        return Ok(AggAbi::Memory);
    }
    let eightbytes = ((size + 7) / 8) as usize;
    let mut classes = vec![Class::Vacant; eightbytes];
    for leaf in &leaves {
        apply_leaf(leaf, &mut classes);
    }
    // X87UP belongs with a preceding X87 eightbyte. Anywhere else the
    // aggregate is passed in memory.
    let x87up_orphan = classes.iter().enumerate().any(|(index, class)| {
        *class == Class::X87Up && (index == 0 || classes[index - 1] != Class::X87)
    });
    if classes.iter().any(|class| *class == Class::Memory) || x87up_orphan {
        return Ok(AggAbi::Memory);
    }
    let x87 = classes
        .iter()
        .any(|class| matches!(class, Class::X87 | Class::X87Up));
    if x87 && use_as == AggUse::Return {
        // X87 followed by X87UP is one `long double`, returned in `%st0`.
        // The argument side of that class is memory; see the branch below.
        let x87_pair = classes.len() == 2
            && classes.first() == Some(&Class::X87)
            && classes.get(1) == Some(&Class::X87Up);
        if x87_pair {
            return Ok(AggAbi::X87);
        }
        return Ok(AggAbi::Memory);
    }
    if x87 {
        // The psABI passes X87 and X87UP arguments in memory.
        return Ok(AggAbi::Memory);
    }
    let mut pieces = Vec::new();
    for (index, class) in classes.iter().enumerate() {
        let piece = match class {
            Class::Integer => integer_piece(index, eightbytes, &leaves, size),
            Class::Sse => Some(sse_piece(index, &leaves)),
            _ => None,
        };
        if let Some(piece) = piece {
            pieces.push(piece);
        }
    }
    if pieces.is_empty() {
        return Ok(AggAbi::Memory);
    }
    Ok(AggAbi::Direct(pieces))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Vacant,
    Integer,
    Sse,
    X87,
    X87Up,
    Memory,
}

struct Leaf {
    offset: u64,
    size: u64,
    align: u32,
    class: Class,
    is_ptr: bool,
    /// The C field that produced this leaf also occupies another eightbyte.
    /// A `char[10]` does. Two separate `char` fields do not.
    spans: bool,
}

fn crosses(offset: u64, size: u64) -> bool {
    size > 0 && offset / 8 != (offset + size - 1) / 8
}

fn collect_leaves(ty: &CType, base: u64, inherited: bool, out: &mut Vec<Leaf>) {
    match ty.peel() {
        CType::Int(name) if name.size() == 16 => {
            // `__int128` is two INTEGER eightbytes (System V AMD64 3.2.3),
            // not one leaf that crosses an eightbyte boundary, so a record
            // holding it goes in two registers. Its 16-byte alignment
            // applies at `base`; the high half is part of the same field.
            let spans = inherited || crosses(base, 16);
            for (offset, align) in [(base, 16), (base + 8, 1)] {
                out.push(Leaf {
                    offset,
                    size: 8,
                    align,
                    class: Class::Integer,
                    is_ptr: false,
                    spans,
                });
            }
        }
        CType::Int(name) => {
            let size = name.size().max(1);
            out.push(Leaf {
                offset: base,
                size,
                align: name.align().max(1),
                class: Class::Integer,
                is_ptr: false,
                spans: inherited || crosses(base, size),
            });
        }
        CType::Pointer(_) => out.push(Leaf {
            offset: base,
            size: 8,
            align: 8,
            class: Class::Integer,
            is_ptr: true,
            spans: inherited || crosses(base, 8),
        }),
        CType::Float(FloatKind::Float) => out.push(Leaf {
            offset: base,
            size: 4,
            align: 4,
            class: Class::Sse,
            is_ptr: false,
            spans: inherited || crosses(base, 4),
        }),
        CType::Float(FloatKind::Double) => out.push(Leaf {
            offset: base,
            size: 8,
            align: 8,
            class: Class::Sse,
            is_ptr: false,
            spans: inherited || crosses(base, 8),
        }),
        CType::Float(FloatKind::LongDouble) => {
            // The field's alignment is 16 and applies at `base`. The second
            // eightbyte is the padding half of the same field, not a second
            // member aligned to 16, so offset 8 is not an unaligned field.
            let spans = inherited || crosses(base, 16);
            out.push(Leaf {
                offset: base,
                size: 8,
                align: 16,
                class: Class::X87,
                is_ptr: false,
                spans,
            });
            out.push(Leaf {
                offset: base + 8,
                size: 8,
                align: 1,
                class: Class::X87Up,
                is_ptr: false,
                spans,
            });
        }
        CType::Array(elem, Some(len)) => {
            let Some(elem_size) = elem.size() else {
                return;
            };
            let total = elem_size.saturating_mul(*len);
            let spans = inherited || crosses(base, total);
            for index in 0..*len {
                collect_leaves(elem, base + index * elem_size, spans, out);
            }
        }
        CType::Struct(handle) => {
            // A bit-field whose declared type is wider than a byte occupies
            // its whole eightbyte. Clang passes `struct { int a:3; char *p; }`
            // as `i64, ptr`, while `struct { char a:3; char *p; }` is `i8, ptr`.
            // A one-byte bit-field is just that byte.
            let fields = handle.fields().unwrap_or_default();
            for field in &fields {
                if field.bit_width.is_some() {
                    let ty_size = field.ty.size().unwrap_or(1);
                    let byte = u64::from(field.bit_offset / 8);
                    if ty_size <= 1 && field.bit_width != Some(0) {
                        let offset = base + byte;
                        out.push(Leaf {
                            offset,
                            size: 1,
                            align: 1,
                            class: Class::Integer,
                            is_ptr: false,
                            spans: inherited || crosses(offset, 1),
                        });
                    } else if ty_size > 1 && field.bit_width != Some(0) {
                        // The eightbyte of the whole argument, not of this
                        // struct: a nested struct can start mid-eightbyte.
                        let offset = ((base + byte) / 8) * 8;
                        out.push(Leaf {
                            offset,
                            size: 8,
                            align: 1,
                            class: Class::Integer,
                            is_ptr: false,
                            spans: inherited || crosses(offset, 8),
                        });
                    }
                    continue;
                }
                collect_leaves(&field.ty, base + field.offset, inherited, out);
            }
        }
        CType::Void | CType::Func(_) | CType::Array(_, None) | CType::Qualified(_, _) => {}
    }
}

/// Width of an INTEGER eightbyte.
///
/// A single eightbyte aggregate uses the bytes its fields occupy, rounded
/// up to their alignment (`char[3]` is `i24`, `int` plus `char` is `i64`).
/// In a larger aggregate, a field that continues into another eightbyte
/// uses that same rule, so `unsigned char[10]` is `i64` plus `i16`. A
/// separate field that sits entirely in one eightbyte is that field's width
/// when it is one aligned scalar (`char` before a `double` is `i8`), and
/// otherwise the rest of the object, up to eight bytes: `i64` when the
/// eightbyte is full, `i32` for the tail of a 12-byte struct.
fn integer_piece(index: usize, eightbytes: usize, leaves: &[Leaf], size: u64) -> Option<RegPiece> {
    let start = index as u64 * 8;
    let limit = (start + 8).min(size);
    let overlapping: Vec<&Leaf> = leaves
        .iter()
        .filter(|leaf| leaf.offset < limit && leaf.offset + leaf.size > start)
        .collect();
    if overlapping.is_empty() {
        return None;
    }
    if overlapping.len() == 1
        && overlapping[0].is_ptr
        && overlapping[0].offset == start
        && overlapping[0].size == 8
    {
        return Some(RegPiece::Ptr { offset: start });
    }
    let spans = overlapping.iter().any(|leaf| leaf.spans);
    if eightbytes > 1 && !spans {
        if let [leaf] = overlapping.as_slice() {
            if leaf.offset == start
                && matches!(leaf.size, 1 | 2 | 4 | 8)
                && u64::from(leaf.align) == leaf.size
            {
                return Some(RegPiece::Int {
                    bits: (leaf.size as u32) * 8,
                    offset: start,
                });
            }
        }
        // Several fields, or one that is not an aligned scalar, share the
        // eightbyte. The piece is the whole eightbyte, but only the bytes
        // the object owns: the last eightbyte of a 12-byte struct is `i32`.
        // A wider piece would load and store past the end of the object.
        return Some(RegPiece::Int {
            bits: ((limit - start) as u32) * 8,
            offset: start,
        });
    }
    piece_at(index, leaves, size)
}

fn sse_piece(index: usize, leaves: &[Leaf]) -> RegPiece {
    let start = index as u64 * 8;
    let limit = start + 8;
    let overlapping: Vec<&Leaf> = leaves
        .iter()
        .filter(|leaf| {
            leaf.class == Class::Sse && leaf.offset < limit && leaf.offset + leaf.size > start
        })
        .collect();
    if overlapping.iter().any(|leaf| leaf.size == 8) {
        return RegPiece::Sse {
            kind: SsePiece::Double,
            offset: start,
        };
    }
    // Two `float` members of a union share offset 0. A pair occupies the
    // high half of the eightbyte; counting leaves would treat that union
    // as `<2 x float>` and load past the object.
    let kind = if overlapping.iter().any(|leaf| leaf.offset >= start + 4) {
        SsePiece::FloatPair
    } else {
        SsePiece::Float
    };
    RegPiece::Sse {
        kind,
        offset: start,
    }
}

fn apply_leaf(leaf: &Leaf, classes: &mut [Class]) {
    if leaf.size == 0 {
        return;
    }
    let first = (leaf.offset / 8) as usize;
    let last = ((leaf.offset + leaf.size - 1) / 8) as usize;
    if first != last {
        for class in classes.iter_mut().take(last + 1).skip(first) {
            *class = Class::Memory;
        }
        return;
    }
    if let Some(slot) = classes.get_mut(first) {
        *slot = merge(*slot, leaf.class);
    }
}

fn merge(left: Class, right: Class) -> Class {
    if left == right || right == Class::Vacant {
        return left;
    }
    if left == Class::Vacant {
        return right;
    }
    if left == Class::Memory || right == Class::Memory {
        return Class::Memory;
    }
    if left == Class::Integer || right == Class::Integer {
        return Class::Integer;
    }
    if matches!(left, Class::X87 | Class::X87Up) || matches!(right, Class::X87 | Class::X87Up) {
        return Class::Memory;
    }
    Class::Sse
}

/// Width of the integer that covers one eightbyte.
///
/// The width runs from the start of the eightbyte through the last byte of
/// any field that overlaps it, then rounds up to the strictest alignment of
/// those fields (and never past 8). That reproduces the coerced widths used
/// for this ABI: a `struct { int a; char b; }` is an `i64`, while
/// `struct { long a; char b; }` is `i64` plus `i8`, and a 3-byte array is `i24`.
fn piece_at(index: usize, leaves: &[Leaf], size: u64) -> Option<RegPiece> {
    let start = index as u64 * 8;
    let limit = (start + 8).min(size);
    let mut overlapping = Vec::new();
    for leaf in leaves {
        let leaf_end = leaf.offset + leaf.size;
        if leaf.offset < limit && leaf_end > start {
            overlapping.push(leaf);
        }
    }
    if overlapping.is_empty() {
        return None;
    }
    if overlapping.len() == 1
        && overlapping[0].is_ptr
        && overlapping[0].offset == start
        && overlapping[0].size == 8
    {
        return Some(RegPiece::Ptr { offset: start });
    }
    let mut last = start;
    let mut max_align = 1u32;
    for leaf in &overlapping {
        let covered = (leaf.offset + leaf.size).min(limit);
        if covered > last {
            last = covered;
        }
        max_align = max_align.max(leaf.align);
    }
    let raw = last - start;
    if raw == 0 {
        return None;
    }
    let rounded = align_up(raw, u64::from(max_align.max(1)))
        .unwrap_or(raw)
        .min(8)
        .min(size - start);
    if rounded == 0 {
        return None;
    }
    Some(RegPiece::Int {
        bits: (rounded as u32) * 8,
        offset: start,
    })
}

/// `name` is omitted on a declaration.
pub fn format_part(ir_type: &str, attrs: &str, name: Option<&str>) -> String {
    match (attrs.is_empty(), name) {
        (true, None) => ir_type.to_string(),
        (true, Some(name)) => format!("{ir_type} {name}"),
        (false, None) => format!("{ir_type} {attrs}"),
        (false, Some(name)) => format!("{ir_type} {attrs} {name}"),
    }
}

/// LLVM names for the expansion of C parameter `index`.
pub fn arg_names(index: usize, count: usize) -> Vec<String> {
    (0..count)
        .map(|piece| {
            if piece == 0 {
                format!("%p{index}")
            } else {
                format!("%p{index}.{piece}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{RecordKind, StructField, StructRef, TypeName};

    fn field(name: &str, ty: CType, offset: u64) -> StructField {
        StructField::plain(name, ty, offset)
    }

    fn record(id: u32, fields: Vec<StructField>, size: u64, align: u32, union_ty: bool) -> CType {
        let kind = if union_ty {
            RecordKind::Union
        } else {
            RecordKind::Struct
        };
        let handle = StructRef::new_incomplete(id, "S", kind);
        handle.complete(fields, size, align, format!("struct.S{id}"));
        CType::Struct(handle)
    }

    fn ints(name: TypeName) -> CType {
        CType::Int(name)
    }

    fn bytes(n: u64) -> CType {
        let arr = CType::array(ints(TypeName::UChar), Some(n));
        record(n as u32, vec![field("a", arr, 0)], n, 1, false)
    }

    fn direct(ty: &CType) -> Vec<String> {
        match classify_aggregate(ty).unwrap() {
            AggAbi::Direct(pieces) => pieces.into_iter().map(|piece| piece.ir_type()).collect(),
            _ => panic!("expected registers"),
        }
    }

    #[test]
    fn int128_record_is_two_integer_eightbytes() {
        // System V AMD64 3.2.3: `__int128` is two INTEGER eightbytes, so
        // a record holding one goes in registers (Clang and GCC agree).
        let q = record(
            400,
            vec![field("q", ints(TypeName::Int128), 0)],
            16,
            16,
            false,
        );
        assert_eq!(direct(&q), ["i64", "i64"]);
        let u = record(
            401,
            vec![
                field("q", ints(TypeName::Int128), 0),
                field("l", ints(TypeName::Long), 0),
            ],
            16,
            16,
            true,
        );
        assert_eq!(direct(&u), ["i64", "i64"]);
        // More than two eightbytes is still memory.
        let wide = record(
            402,
            vec![
                field("a", ints(TypeName::Long), 0),
                field("q", ints(TypeName::Int128), 16),
            ],
            32,
            16,
            false,
        );
        assert!(matches!(classify_aggregate(&wide).unwrap(), AggAbi::Memory));
    }

    #[test]
    fn char_arrays_match_eightbyte_widths() {
        for n in 1..=8 {
            let ty = bytes(n);
            assert_eq!(direct(&ty), vec![format!("i{}", n * 8)], "size {n}");
        }
        for n in 9..=16 {
            let ty = bytes(n);
            assert_eq!(
                direct(&ty),
                vec!["i64".to_string(), format!("i{}", (n - 8) * 8)],
                "size {n}"
            );
        }
        assert!(matches!(
            classify_aggregate(&bytes(17)).unwrap(),
            AggAbi::Memory
        ));
        assert!(matches!(
            classify_aggregate(&bytes(24)).unwrap(),
            AggAbi::Memory
        ));
    }

    #[test]
    fn last_eightbyte_never_extends_past_the_object() {
        // struct { int a; int b; short c; char d; } is 12 bytes: i64, i32.
        // The second eightbyte has two fields, but only four bytes of it
        // belong to the object.
        let twelve = record(
            120,
            vec![
                field("a", ints(TypeName::Int), 0),
                field("b", ints(TypeName::Int), 4),
                field("c", ints(TypeName::Short), 8),
                field("d", ints(TypeName::Char), 10),
            ],
            12,
            4,
            false,
        );
        assert_eq!(direct(&twelve), vec!["i64".to_string(), "i32".to_string()]);

        // struct { short a[4]; char b; char c; } is 10 bytes: i64, i16.
        let ten = record(
            121,
            vec![
                field("a", CType::array(ints(TypeName::Short), Some(4)), 0),
                field("b", ints(TypeName::Char), 8),
                field("c", ints(TypeName::Char), 9),
            ],
            10,
            2,
            false,
        );
        assert_eq!(direct(&ten), vec!["i64".to_string(), "i16".to_string()]);

        // A 12-byte struct whose last storage unit holds an `int`
        // bit-field (`signed f3 : 24` at byte 8) is i64, i32.
        let mut f3 = field("f3", ints(TypeName::Int), 8);
        f3.bit_width = Some(24);
        f3.bit_offset = 64;
        let bits = record(
            122,
            vec![
                field("a", ints(TypeName::Int), 0),
                field("b", ints(TypeName::Int), 4),
                f3,
            ],
            12,
            4,
            false,
        );
        assert_eq!(direct(&bits), vec!["i64".to_string(), "i32".to_string()]);
    }

    #[test]
    fn nested_bit_field_uses_the_outer_eightbyte() {
        // struct { char a[2]; struct { int b : 4; } s; long m; } is 16
        // bytes: i64, i64. The inner struct starts at byte 4, so its
        // bit-field covers eightbyte 0 of the argument, not bytes 4..12
        // (which would cross into eightbyte 1 and force memory).
        let mut b = field("b", ints(TypeName::Int), 0);
        b.bit_width = Some(4);
        b.bit_offset = 0;
        let inner = record(130, vec![b], 4, 4, false);
        let outer = record(
            131,
            vec![
                field("a", CType::array(ints(TypeName::Char), Some(2)), 0),
                field("s", inner, 4),
                field("m", ints(TypeName::Long), 8),
            ],
            16,
            8,
            false,
        );
        assert_eq!(direct(&outer), vec!["i64".to_string(), "i64".to_string()]);
    }

    #[test]
    fn zero_width_bit_field_occupies_no_eightbyte() {
        // struct { float x; struct { int : 0; float f; } s; double y; } is
        // <2 x float>, double. The `int : 0` has no storage, so it must not
        // make eightbyte 0 INTEGER.
        let mut pad = field("", ints(TypeName::Int), 0);
        pad.bit_width = Some(0);
        pad.bit_offset = 0;
        let inner = record(
            132,
            vec![pad, field("f", CType::Float(FloatKind::Float), 0)],
            4,
            4,
            false,
        );
        let outer = record(
            133,
            vec![
                field("x", CType::Float(FloatKind::Float), 0),
                field("s", inner, 4),
                field("y", CType::Float(FloatKind::Double), 8),
            ],
            16,
            8,
            false,
        );
        assert_eq!(
            direct(&outer),
            vec!["<2 x float>".to_string(), "double".to_string()]
        );
    }

    #[test]
    fn mixed_layouts_match_the_abi() {
        // struct { int a; char b; } is 8 bytes, one i64.
        let padded = record(
            100,
            vec![
                field("a", ints(TypeName::Int), 0),
                field("b", ints(TypeName::Char), 4),
            ],
            8,
            4,
            false,
        );
        assert_eq!(direct(&padded), vec!["i64".to_string()]);

        // struct { long a; char b; } is i64, i8.
        let long_char = record(
            101,
            vec![
                field("a", ints(TypeName::Long), 0),
                field("b", ints(TypeName::Char), 8),
            ],
            16,
            8,
            false,
        );
        assert_eq!(
            direct(&long_char),
            vec!["i64".to_string(), "i8".to_string()]
        );

        // struct { int a; long b; } is i32, i64.
        let int_long = record(
            102,
            vec![
                field("a", ints(TypeName::Int), 0),
                field("b", ints(TypeName::Long), 8),
            ],
            16,
            8,
            false,
        );
        assert_eq!(
            direct(&int_long),
            vec!["i32".to_string(), "i64".to_string()]
        );

        // struct { short a; char b; } is 4 bytes, i32.
        let short_char = record(
            103,
            vec![
                field("a", ints(TypeName::Short), 0),
                field("b", ints(TypeName::Char), 2),
            ],
            4,
            2,
            false,
        );
        assert_eq!(direct(&short_char), vec!["i32".to_string()]);

        // struct { char c; int *p; } is i8, ptr.
        let ptrs = record(
            104,
            vec![
                field("c", ints(TypeName::Char), 0),
                field("p", CType::pointer(ints(TypeName::Int)), 8),
            ],
            16,
            8,
            false,
        );
        assert_eq!(direct(&ptrs), vec!["i8".to_string(), "ptr".to_string()]);

        // union { long a; char c[9]; } is 16 bytes, i64 plus i8.
        let uni = record(
            105,
            vec![
                field("a", ints(TypeName::Long), 0),
                field("c", CType::array(ints(TypeName::Char), Some(9)), 0),
            ],
            16,
            8,
            true,
        );
        assert_eq!(direct(&uni), vec!["i64".to_string(), "i8".to_string()]);

        let three_longs = record(
            106,
            vec![
                field("a", ints(TypeName::Long), 0),
                field("b", ints(TypeName::Long), 8),
                field("c", ints(TypeName::Long), 16),
            ],
            24,
            8,
            false,
        );
        assert!(matches!(
            classify_aggregate(&three_longs).unwrap(),
            AggAbi::Memory
        ));
    }

    #[test]
    fn sse_eightbytes_use_xmm_pieces() {
        let only = record(
            200,
            vec![field("d", CType::Float(FloatKind::Double), 0)],
            8,
            8,
            false,
        );
        assert_eq!(direct(&only), vec!["double".to_string()]);

        let one_float = record(
            210,
            vec![field("f", CType::Float(FloatKind::Float), 0)],
            4,
            4,
            false,
        );
        assert_eq!(direct(&one_float), vec!["float".to_string()]);

        let two = record(
            211,
            vec![
                field("a", CType::Float(FloatKind::Float), 0),
                field("b", CType::Float(FloatKind::Float), 4),
            ],
            8,
            4,
            false,
        );
        assert_eq!(direct(&two), vec!["<2 x float>".to_string()]);

        let three = record(
            212,
            vec![
                field("a", CType::Float(FloatKind::Float), 0),
                field("b", CType::Float(FloatKind::Float), 4),
                field("c", CType::Float(FloatKind::Float), 8),
            ],
            12,
            4,
            false,
        );
        assert_eq!(
            direct(&three),
            vec!["<2 x float>".to_string(), "float".to_string()]
        );

        let int_double = record(
            213,
            vec![
                field("i", ints(TypeName::Int), 0),
                field("d", CType::Float(FloatKind::Double), 8),
            ],
            16,
            8,
            false,
        );
        assert_eq!(
            direct(&int_double),
            vec!["i32".to_string(), "double".to_string()]
        );

        let chars = record(
            214,
            vec![
                field("a", CType::array(ints(TypeName::Char), Some(2)), 0),
                field("d", CType::Float(FloatKind::Double), 8),
            ],
            16,
            8,
            false,
        );
        assert_eq!(
            direct(&chars),
            vec!["i64".to_string(), "double".to_string()]
        );

        let uni = record(
            215,
            vec![
                field("f", CType::Float(FloatKind::Float), 0),
                field("d", CType::Float(FloatKind::Double), 0),
            ],
            8,
            8,
            true,
        );
        assert_eq!(direct(&uni), vec!["double".to_string()]);

        // Both members sit at offset 0. The object is one float.
        let two_floats = record(
            216,
            vec![
                field("a", CType::Float(FloatKind::Float), 0),
                field("b", CType::Float(FloatKind::Float), 0),
            ],
            4,
            4,
            true,
        );
        assert_eq!(direct(&two_floats), vec!["float".to_string()]);

        // The array reaches the high half, so the eightbyte is a pair.
        let float_and_pair = record(
            217,
            vec![
                field("a", CType::Float(FloatKind::Float), 0),
                field(
                    "b",
                    CType::array(CType::Float(FloatKind::Float), Some(2)),
                    0,
                ),
            ],
            8,
            4,
            true,
        );
        assert_eq!(direct(&float_and_pair), vec!["<2 x float>".to_string()]);

        // int and float share an eightbyte, so INTEGER wins and the bits
        // travel in a general register.
        let mixed = record(
            201,
            vec![
                field("i", ints(TypeName::Int), 0),
                field("f", CType::Float(FloatKind::Float), 4),
            ],
            8,
            4,
            false,
        );
        assert_eq!(direct(&mixed), vec!["i64".to_string()]);

        // A large aggregate is a memory copy even if it contains a double.
        let big = record(
            202,
            vec![
                field("d", CType::Float(FloatKind::Double), 0),
                field("a", CType::array(ints(TypeName::Char), Some(16)), 8),
            ],
            24,
            8,
            false,
        );
        assert!(matches!(classify_aggregate(&big).unwrap(), AggAbi::Memory));
    }

    #[test]
    fn multi_eightbyte_struct_uses_memory_when_registers_run_out() {
        let s16 = record(
            300,
            vec![
                field("a", ints(TypeName::Long), 0),
                field("b", ints(TypeName::Char), 8),
            ],
            16,
            8,
            false,
        );
        let mut params = vec![CType::int_type(); 5];
        params.push(s16.clone());
        params.push(CType::int_type());
        let sig = function_sig(&CType::int_type(), &params).unwrap();
        assert!(matches!(sig.params[5][0].kind, PartKind::Byval));
        assert!(matches!(sig.params[6][0].kind, PartKind::Scalar));
        // The spilled aggregate does not take the last general register,
        // so the following int still has it.
        assert_eq!(sig.free_int, 0);
        assert_eq!(sig.free_sse, 8);

        let mut fitted = vec![CType::int_type(); 4];
        fitted.push(s16);
        let sig = function_sig(&CType::int_type(), &fitted).unwrap();
        match &sig.params[4][0].kind {
            PartKind::Piece(piece) => assert_eq!(piece.ir_type(), "i64"),
            other => panic!("expected the first eightbyte in a register, got {other:?}"),
        }
        assert!(matches!(sig.params[4][1].kind, PartKind::Piece(_)));
        assert_eq!(sig.free_int, 0);
    }

    #[test]
    fn long_double_aggregate_is_memory_in_and_x87_out() {
        let ld = record(
            301,
            vec![field("x", CType::Float(FloatKind::LongDouble), 0)],
            16,
            16,
            false,
        );
        assert!(matches!(
            classify_for(&ld, AggUse::Param).unwrap(),
            AggAbi::Memory
        ));
        assert!(matches!(
            classify_for(&ld, AggUse::Return).unwrap(),
            AggAbi::X87
        ));

        // X87UP not preceded by X87, because the long shares the first
        // eightbyte and INTEGER wins there. Both directions are memory.
        let mixed = record(
            302,
            vec![
                field("x", CType::Float(FloatKind::LongDouble), 0),
                field("y", ints(TypeName::Long), 0),
            ],
            16,
            16,
            true,
        );
        assert!(matches!(
            classify_for(&mixed, AggUse::Param).unwrap(),
            AggAbi::Memory
        ));
        assert!(matches!(
            classify_for(&mixed, AggUse::Return).unwrap(),
            AggAbi::Memory
        ));
    }
}
