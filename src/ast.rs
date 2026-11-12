//! Abstract syntax for the C subset accepted by this milestone.
//!
//! Names are resolved later. Local `slot` fields are filled before semantic
//! analysis so each variable has a stable LLVM alloca name. File-scope objects
//! use the global's LLVM name instead of an alloca.
//!
//! Types are integers, `void`, pointers, arrays, and structs, with `const`
//! and `volatile`. A function type is not a value type: parameters and
//! returns use the object type that remains after the usual parameter
//! adjustment. A struct is one declaration: the same tag in another scope is
//! a different type, and completing the type updates every use of it.

use std::cell::RefCell;
use std::rc::Rc;

/// Integer and `void` types on `x86_64-pc-linux-gnu` (LP64).
///
/// `char` is a distinct type from `signed char` and `unsigned char`. On this
/// target it is signed. `long` and `long long` are both 64 bits; `long long`
/// still has the greater conversion rank (C11 6.3.1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeName {
    Void,
    Char,
    SChar,
    UChar,
    Short,
    UShort,
    Int,
    UInt,
    Long,
    ULong,
    LongLong,
    ULongLong,
    /// `_Bool`. The object representation is one byte, and every stored value
    /// is 0 or 1. Rank is below `char`. Conversion to `_Bool` compares with 0
    /// rather than truncating (C11 6.3.1.2).
    Bool,
}

impl TypeName {
    pub fn as_str(self) -> &'static str {
        match self {
            TypeName::Void => "void",
            TypeName::Char => "char",
            TypeName::SChar => "signed char",
            TypeName::UChar => "unsigned char",
            TypeName::Short => "short",
            TypeName::UShort => "unsigned short",
            TypeName::Int => "int",
            TypeName::UInt => "unsigned int",
            TypeName::Long => "long",
            TypeName::ULong => "unsigned long",
            TypeName::LongLong => "long long",
            TypeName::ULongLong => "unsigned long long",
            TypeName::Bool => "_Bool",
        }
    }

    pub fn is_void(self) -> bool {
        self == TypeName::Void
    }

    pub fn is_integer(self) -> bool {
        !self.is_void()
    }

    /// `char` is signed on this target.
    pub fn is_signed(self) -> bool {
        matches!(
            self,
            TypeName::Char
                | TypeName::SChar
                | TypeName::Short
                | TypeName::Int
                | TypeName::Long
                | TypeName::LongLong
        )
    }

    pub fn width(self) -> u32 {
        match self {
            TypeName::Void => 0,
            // Stored in a byte. The value is still only 0 or 1.
            TypeName::Bool | TypeName::Char | TypeName::SChar | TypeName::UChar => 8,
            TypeName::Short | TypeName::UShort => 16,
            TypeName::Int | TypeName::UInt => 32,
            TypeName::Long | TypeName::ULong | TypeName::LongLong | TypeName::ULongLong => 64,
        }
    }

    /// Size in bytes. `void` is incomplete and has no size.
    pub fn size(self) -> u64 {
        u64::from(self.width() / 8)
    }

    /// Natural alignment, which is the size for every integer type here.
    pub fn align(self) -> u32 {
        self.size() as u32
    }

    pub fn ir_type(self) -> &'static str {
        match self.width() {
            0 => "void",
            8 => "i8",
            16 => "i16",
            32 => "i32",
            _ => "i64",
        }
    }

    /// Integer conversion rank. `char`, `signed char`, and `unsigned char`
    /// share a rank. The unsigned type has the same rank as its signed pair.
    pub fn rank(self) -> u8 {
        match self {
            TypeName::Void | TypeName::Bool => 0,
            TypeName::Char | TypeName::SChar | TypeName::UChar => 1,
            TypeName::Short | TypeName::UShort => 2,
            TypeName::Int | TypeName::UInt => 3,
            TypeName::Long | TypeName::ULong => 4,
            TypeName::LongLong | TypeName::ULongLong => 5,
        }
    }

    pub fn mask(self) -> u64 {
        width_mask(self.width())
    }

    /// Integer promotions (C11 6.3.1.1). Every type narrower than `int` fits
    /// in signed `int` on this target, including `unsigned short`.
    pub fn promote(self) -> TypeName {
        match self {
            TypeName::Bool
            | TypeName::Char
            | TypeName::SChar
            | TypeName::UChar
            | TypeName::Short
            | TypeName::UShort => TypeName::Int,
            other => other,
        }
    }

    /// The unsigned type of the same width. Used when the usual arithmetic
    /// conversions cannot keep a signed type (C11 6.3.1.8).
    pub fn to_unsigned(self) -> TypeName {
        match self {
            TypeName::Char | TypeName::SChar => TypeName::UChar,
            TypeName::Short => TypeName::UShort,
            TypeName::Int => TypeName::UInt,
            TypeName::Long => TypeName::ULong,
            TypeName::LongLong => TypeName::ULongLong,
            other => other,
        }
    }

    /// Usual arithmetic conversions, including the integer promotions.
    pub fn usual(self, other: TypeName) -> TypeName {
        let a = self.promote();
        let b = other.promote();
        if a == b {
            return a;
        }
        if a.is_signed() == b.is_signed() {
            return if a.rank() >= b.rank() { a } else { b };
        }
        let (signed, unsigned) = if a.is_signed() { (a, b) } else { (b, a) };
        if unsigned.rank() >= signed.rank() {
            return unsigned;
        }
        if signed.width() > unsigned.width() {
            return signed;
        }
        signed.to_unsigned()
    }

    /// System V AMD64 requires the caller to extend an integer narrower than
    /// 32 bits to 32 bits: sign-extend signed types, zero-extend unsigned
    /// types. LLVM spells that `signext` / `zeroext` on the parameter or
    /// return. `int` and wider types are passed as themselves.
    pub fn abi_attr(self) -> Option<&'static str> {
        match self {
            TypeName::Char | TypeName::SChar | TypeName::Short => Some("signext"),
            TypeName::Bool | TypeName::UChar | TypeName::UShort => Some("zeroext"),
            _ => None,
        }
    }

    /// Decimal spelling of `bits` interpreted as this type.
    pub fn format_bits(self, bits: u64) -> String {
        if self.is_signed() {
            sign_extend(bits, self.width()).to_string()
        } else {
            (bits & self.mask()).to_string()
        }
    }
}

/// `const`, `volatile`, and `restrict` on one type (C11 6.7.3).
///
/// The same qualifier twice in one specifier or qualifier list is a
/// diagnostic. Qualifiers from different lists combine: `const int * const`.
/// `restrict` is part of the type for compatibility and assignment. It does
/// not change the IR. It is only meaningful on a pointer; the parser rejects
/// it on any other type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Quals {
    pub is_const: bool,
    pub is_volatile: bool,
    pub is_restrict: bool,
}

impl Quals {
    pub fn is_empty(self) -> bool {
        !self.is_const && !self.is_volatile && !self.is_restrict
    }

    /// `self` has every qualifier `other` has.
    pub fn includes(self, other: Self) -> bool {
        (!other.is_const || self.is_const)
            && (!other.is_volatile || self.is_volatile)
            && (!other.is_restrict || self.is_restrict)
    }

    pub fn merge(self, other: Self) -> Self {
        Self {
            is_const: self.is_const || other.is_const,
            is_volatile: self.is_volatile || other.is_volatile,
            is_restrict: self.is_restrict || other.is_restrict,
        }
    }
}

/// One field of a completed struct. `offset` is the byte offset from the
/// start of the object, including the padding the System V AMD64 ABI inserts
/// so the field is aligned to [`CType::member_align`].
#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub ty: CType,
    /// Byte offset of a normal member, or of the storage unit that holds a
    /// bit-field. The unit is the aligned region of the field's declared type
    /// that contains [`StructField::bit_offset`].
    pub offset: u64,
    /// Bit offset from the start of the struct. Meaningful when `bit_width`
    /// is set. Bit 0 is the least significant bit of the first byte.
    pub bit_offset: u32,
    /// `Some` when this member is a bit-field, including a zero-width unnamed
    /// field used only for alignment. `None` is an ordinary member.
    pub bit_width: Option<u32>,
}

impl StructField {
    pub fn plain(name: impl Into<String>, ty: CType, offset: u64) -> Self {
        Self {
            name: name.into(),
            ty,
            offset,
            bit_offset: 0,
            bit_width: None,
        }
    }

    pub fn is_bitfield(&self) -> bool {
        self.bit_width.is_some()
    }

    /// A flexible array member: an incomplete array that is not a bit-field
    /// (C11 6.7.2.1). The array length is unknown and the element type is
    /// complete.
    pub fn is_flexible(&self) -> bool {
        self.bit_width.is_none() && self.ty.is_array() && self.ty.array_len().is_none()
    }

    /// An unnamed struct or union member that is not a bit-field. C11 calls
    /// it anonymous when its type specifier has no tag.
    pub fn is_anonymous_record(&self) -> bool {
        self.name.is_empty() && self.bit_width.is_none() && self.ty.is_record()
    }
}

/// Where [`StructRef::lookup_member`] found a name.
pub struct MemberHit {
    pub index: usize,
    pub field: StructField,
    /// The name lives inside an anonymous member, so it is addressed by a
    /// byte offset from the start of the outer object.
    pub indirect: bool,
}

/// Identity of one struct declaration.
///
/// Copies share the body. Completing an incomplete type is visible through
/// every [`CType::Struct`] that names it, which is what a later
/// `struct node { ... };` does to an earlier `struct node *`. Equality is the
/// declaration, not the list of fields: two structs with the same members and
/// different tags are not compatible (C11 6.2.7).
#[derive(Clone)]
pub struct StructRef {
    id: u32,
    data: Rc<RefCell<StructBody>>,
}

/// A struct and a union share the tag namespace and the member machinery.
/// The kind selects the layout: struct members are sequential, union members
/// all start at offset 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordKind {
    Struct,
    Union,
}

impl RecordKind {
    pub fn word(self) -> &'static str {
        match self {
            RecordKind::Struct => "struct",
            RecordKind::Union => "union",
        }
    }
}

struct StructBody {
    kind: RecordKind,
    tag: String,
    fields: Option<Vec<StructField>>,
    size: u64,
    align: u32,
    llvm_name: String,
}

impl PartialEq for StructRef {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for StructRef {}

impl std::fmt::Debug for StructRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let body = self.data.borrow();
        write!(f, "{} {}#{}", body.kind.word(), body.tag, self.id)
    }
}

impl StructRef {
    pub fn new_incomplete(id: u32, tag: impl Into<String>, kind: RecordKind) -> Self {
        Self {
            id,
            data: Rc::new(RefCell::new(StructBody {
                kind,
                tag: tag.into(),
                fields: None,
                size: 0,
                align: 1,
                llvm_name: String::new(),
            })),
        }
    }

    pub fn kind(&self) -> RecordKind {
        self.data.borrow().kind
    }

    pub fn is_union(&self) -> bool {
        self.kind() == RecordKind::Union
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn tag(&self) -> String {
        self.data.borrow().tag.clone()
    }

    pub fn is_complete(&self) -> bool {
        self.data.borrow().fields.is_some()
    }

    pub fn fields(&self) -> Option<Vec<StructField>> {
        self.data.borrow().fields.clone()
    }

    pub fn size(&self) -> Option<u64> {
        let body = self.data.borrow();
        body.fields.as_ref().map(|_| body.size)
    }

    pub fn align(&self) -> u32 {
        self.data.borrow().align
    }

    /// LLVM type name without the leading `%`, such as `struct.Node`.
    pub fn llvm_name(&self) -> String {
        self.data.borrow().llvm_name.clone()
    }

    /// Name used in IR, including an incomplete struct that has no body yet.
    ///
    /// A completed struct uses the name chosen when it was defined. An
    /// incomplete one is an opaque type. The id keeps two tags in different
    /// scopes from sharing a name.
    pub fn ir_name(&self) -> String {
        let name = self.llvm_name();
        if !name.is_empty() {
            return name;
        }
        let tag = self.tag();
        // `alloc_llvm_name` uses `struct.Tag` and `struct.Tag.N`. An
        // incomplete record needs a different prefix so the two cannot
        // define the same LLVM type.
        if tag.is_empty() {
            format!("opaque.anon.{}", self.id)
        } else {
            format!("opaque.{tag}.{}", self.id)
        }
    }

    pub fn field(&self, name: &str) -> Option<(usize, StructField)> {
        self.lookup_member(name).map(|hit| (hit.index, hit.field))
    }

    /// A member, including one reached through an anonymous struct or union.
    ///
    /// C11 6.7.2.1 makes the members of an untagged, unnamed struct or union
    /// members of the enclosing type. `indirect` means the address is a byte
    /// offset from the outer object, because the name is not one LLVM field.
    pub fn lookup_member(&self, name: &str) -> Option<MemberHit> {
        self.lookup_member_from(name, 0, Quals::default(), false)
    }

    fn lookup_member_from(
        &self,
        name: &str,
        base: u64,
        quals: Quals,
        indirect: bool,
    ) -> Option<MemberHit> {
        if name.is_empty() {
            return None;
        }
        let fields = self.fields()?;
        for (index, field) in fields.iter().enumerate() {
            if field.bit_width == Some(0) {
                continue;
            }
            if field.name == name {
                let mut found = field.clone();
                found.offset = found.offset.saturating_add(base);
                if found.bit_width.is_some() {
                    found.bit_offset = found
                        .bit_offset
                        .saturating_add((base as u32).saturating_mul(8));
                }
                found.ty = found.ty.with_quals(quals);
                return Some(MemberHit {
                    index,
                    field: found,
                    indirect,
                });
            }
            if field.name.is_empty() && field.bit_width.is_none() && field.ty.is_record() {
                let inner = field.ty.struct_ref()?;
                let nested = inner.lookup_member_from(
                    name,
                    base + field.offset,
                    quals.merge(field.ty.quals()),
                    true,
                );
                if nested.is_some() {
                    return nested;
                }
            }
        }
        None
    }

    /// A member is a bit-field, including a zero-width one. The LLVM type is
    /// then a byte array, the same shape as a union, because several C
    /// members share one storage unit.
    pub fn has_bitfield(&self) -> bool {
        self.fields()
            .is_some_and(|fields| fields.iter().any(StructField::is_bitfield))
    }

    /// The last member is a flexible array (C11 6.7.2.1). `sizeof` excludes it.
    pub fn has_flexible_member(&self) -> bool {
        self.fields()
            .is_some_and(|fields| fields.last().is_some_and(StructField::is_flexible))
    }

    pub fn complete(&self, fields: Vec<StructField>, size: u64, align: u32, llvm_name: String) {
        let mut body = self.data.borrow_mut();
        body.fields = Some(fields);
        body.size = size;
        body.align = align.max(1);
        body.llvm_name = llvm_name;
    }
}

/// Round `offset` up to a multiple of `align`. `None` when the sum does not
/// fit in `u64`.
pub fn align_up(offset: u64, align: u64) -> Option<u64> {
    let align = align.max(1);
    let rem = offset % align;
    if rem == 0 {
        Some(offset)
    } else {
        offset.checked_add(align - rem)
    }
}

/// System V AMD64 layout of a struct.
///
/// Each field is placed at the next offset that satisfies its alignment.
/// The struct's alignment is the strictest field alignment, at least 1.
/// Its size is rounded up to a multiple of that alignment, which is the
/// trailing padding. Field alignment is [`CType::member_align`]: an array
/// field uses the element's alignment. The ABI's extra 16-byte alignment
/// applies to a local or global array variable, not to a field and not to
/// `_Alignof` of an array type.
///
/// `fields` is `(size, alignment)` in declaration order. `None` means the
/// size overflowed. `union_layout` places every member at offset 0 and uses
/// the largest member as the size.
pub fn layout_record(fields: &[(u64, u32)]) -> Option<(Vec<u64>, u64, u32)> {
    layout_fields(fields, false)
}

/// Union layout. Every member starts at offset 0. The size is the largest
/// member, rounded up to the strictest alignment.
pub fn layout_union(fields: &[(u64, u32)]) -> Option<(Vec<u64>, u64, u32)> {
    layout_fields(fields, true)
}

/// One member passed to [`layout_members`].
///
/// A bit-field sets `bit_width`. `size` and `align` are then the declared
/// type's size and alignment, which are the storage unit the field must fit
/// in. A normal member leaves `bit_width` as `None` and uses its own size
/// and alignment. `named` is false for an unnamed bit-field; those occupy
/// bits but do not increase the struct's alignment.
pub struct LayoutMember {
    pub size: u64,
    pub align: u32,
    pub bit_width: Option<u32>,
    pub named: bool,
}

/// Where [`layout_members`] put one member.
///
/// `offset` is the byte offset of a normal member, or the start of the
/// storage unit that holds a bit-field. `bit_offset` is the absolute bit
/// position from the start of the object (bit 0 is the least significant bit
/// of byte 0). `bit_width` copies the input.
pub struct LayoutPlace {
    pub offset: u64,
    pub bit_offset: u32,
    pub bit_width: Option<u32>,
}

/// System V AMD64 member layout, including bit-fields.
///
/// Ordinary members are aligned to their alignment. A bit-field is packed
/// from low bits to high bits and must lie entirely inside one storage unit
/// of its declared type: if it does not fit in the unit that contains the
/// next free bit, it starts at the next unit of that alignment. A zero-width
/// bit-field rounds the next free bit up to that alignment. Unnamed
/// bit-fields do not change the struct's alignment. The size is the end of
/// the last byte that was reserved, rounded up to the struct alignment.
///
/// A union places every member at offset 0. Each bit-field starts at bit 0.
/// The size is the largest member, and the alignment is the strictest
/// alignment of a named member.
pub fn layout_members(
    members: &[LayoutMember],
    union_layout: bool,
) -> Option<(Vec<LayoutPlace>, u64, u32)> {
    if union_layout {
        return layout_union_members(members);
    }
    let mut places = Vec::with_capacity(members.len());
    let mut bit_pos: u64 = 0;
    let mut align = 1u32;
    for member in members {
        let field_align = member.align.max(1);
        if let Some(width) = member.bit_width {
            let unit_bits = member.size.max(1).saturating_mul(8);
            let align_bits = u64::from(field_align).saturating_mul(8);
            if width == 0 {
                bit_pos = align_up(bit_pos, align_bits)?;
                places.push(LayoutPlace {
                    offset: bit_pos / 8,
                    bit_offset: bit_pos as u32,
                    bit_width: Some(0),
                });
            } else {
                let width = u64::from(width);
                let within = bit_pos % unit_bits;
                if within + width > unit_bits {
                    bit_pos = align_up(bit_pos, align_bits)?;
                }
                let unit_start = bit_pos - (bit_pos % unit_bits);
                places.push(LayoutPlace {
                    offset: unit_start / 8,
                    bit_offset: bit_pos as u32,
                    bit_width: Some(width as u32),
                });
                bit_pos = bit_pos.checked_add(width)?;
                if member.named {
                    align = align.max(field_align);
                }
            }
        } else {
            let byte = align_up((bit_pos + 7) / 8, u64::from(field_align))?;
            places.push(LayoutPlace {
                offset: byte,
                bit_offset: 0,
                bit_width: None,
            });
            bit_pos = byte.checked_add(member.size)?.saturating_mul(8);
            align = align.max(field_align);
        }
    }
    let bytes = (bit_pos + 7) / 8;
    let size = align_up(bytes, u64::from(align))?;
    Some((places, size, align))
}

fn layout_union_members(members: &[LayoutMember]) -> Option<(Vec<LayoutPlace>, u64, u32)> {
    let mut places = Vec::with_capacity(members.len());
    let mut align = 1u32;
    let mut size = 0u64;
    for member in members {
        let field_align = member.align.max(1);
        if let Some(width) = member.bit_width {
            if width == 0 {
                places.push(LayoutPlace {
                    offset: 0,
                    bit_offset: 0,
                    bit_width: Some(0),
                });
            } else {
                places.push(LayoutPlace {
                    offset: 0,
                    bit_offset: 0,
                    bit_width: Some(width),
                });
                size = size.max(member.size.max(1));
                if member.named {
                    align = align.max(field_align);
                }
            }
        } else {
            places.push(LayoutPlace {
                offset: 0,
                bit_offset: 0,
                bit_width: None,
            });
            size = size.max(member.size);
            align = align.max(field_align);
        }
    }
    let size = align_up(size, u64::from(align))?;
    Some((places, size, align))
}

fn layout_fields(fields: &[(u64, u32)], union_layout: bool) -> Option<(Vec<u64>, u64, u32)> {
    let mut offsets = Vec::with_capacity(fields.len());
    let mut offset = 0u64;
    let mut align = 1u32;
    let mut max_size = 0u64;
    for &(size, field_align) in fields {
        let field_align = field_align.max(1);
        align = align.max(field_align);
        if union_layout {
            offsets.push(0);
            max_size = max_size.max(size);
        } else {
            offset = align_up(offset, u64::from(field_align))?;
            offsets.push(offset);
            offset = offset.checked_add(size)?;
        }
    }
    let size = align_up(
        if union_layout { max_size } else { offset },
        u64::from(align),
    )?;
    Some((offsets, size, align))
}

/// A function type. It is not an object type: a parameter of function type is
/// adjusted to a pointer, and a typedef of a function type names a function
/// designator. `params` are the adjusted parameter types with top-level
/// qualifiers removed.
///
/// `prototype` is false for a function type written with an empty `()`
/// (C11 6.7.6.3p14): the type supplies no information about the number or
/// types of the parameters, so `params` is empty and `variadic` is false.
/// A call through such a type applies the default argument promotions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuncTy {
    pub ret: CType,
    pub params: Vec<CType>,
    pub variadic: bool,
    pub prototype: bool,
}

/// `float`, `double`, and `long double` on this target.
///
/// Sizes follow the System V AMD64 ABI: 4, 8, and 16. `long double` is the
/// 80-bit extended format (`x86_fp80`), stored in 16 bytes with the value
/// in the low 10. All three types are computed. A scalar `long double` is
/// passed in memory and returned in `%st0`; an aggregate that contains one
/// is passed in memory, and returning that aggregate is rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatKind {
    Float,
    Double,
    LongDouble,
}

impl FloatKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FloatKind::Float => "float",
            FloatKind::Double => "double",
            FloatKind::LongDouble => "long double",
        }
    }

    pub fn size(self) -> u64 {
        match self {
            FloatKind::Float => 4,
            FloatKind::Double => 8,
            FloatKind::LongDouble => 16,
        }
    }

    pub fn align(self) -> u32 {
        self.size() as u32
    }

    pub fn ir_type(self) -> &'static str {
        match self {
            FloatKind::Float => "float",
            FloatKind::Double => "double",
            FloatKind::LongDouble => "x86_fp80",
        }
    }
}

/// Encoding of a string literal after phase-6 concatenation (C11 6.4.5).
///
/// Ordinary and `u8` literals are [`StrEnc::Byte`]: a UTF-8 string is an
/// array of `char`. `u` is `char16_t`, the target's unsigned 16-bit type.
/// `U` is `char32_t`, the target's unsigned 32-bit type. `L` is `wchar_t`,
/// which is `int` on this target. Code units are stored little-endian and
/// do not include the terminating NUL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrEnc {
    Byte,
    Utf16,
    Wchar,
    Utf32,
}

impl StrEnc {
    pub fn unit(self) -> usize {
        match self {
            StrEnc::Byte => 1,
            StrEnc::Utf16 => 2,
            StrEnc::Wchar | StrEnc::Utf32 => 4,
        }
    }

    /// Element type of the array. `char16_t` and `char32_t` are the target's
    /// unsigned 16-bit and 32-bit types. `wchar_t` is `int`.
    pub fn elem(self) -> TypeName {
        match self {
            StrEnc::Byte => TypeName::Char,
            StrEnc::Utf16 => TypeName::UShort,
            StrEnc::Wchar => TypeName::Int,
            StrEnc::Utf32 => TypeName::UInt,
        }
    }

    pub fn align(self) -> u32 {
        self.unit() as u32
    }

    /// Widen one source byte to this encoding's code unit, little-endian.
    /// An unprefixed literal adopts a wider prefix by this rule.
    pub fn widen_byte(self, byte: u8) -> Vec<u8> {
        let mut value = u32::from(byte);
        let mut out = Vec::with_capacity(self.unit());
        for _ in 0..self.unit() {
            out.push((value & 0xff) as u8);
            value >>= 8;
        }
        out
    }
}

/// A type this milestone can name: `void`, an integer (including `_Bool`), a
/// floating type, a pointer, an array, a function, or a struct or union, each
/// of which may be `const`, `volatile`, or `restrict`.
///
/// `char` stays distinct from `signed char`. Pointers use one LLVM `ptr`
/// (opaque pointers). An array with `len == None` is incomplete (`int a[]`).
/// A pointer is complete even when it points at an incomplete type. A struct
/// or union is incomplete until its member list has been defined in the same
/// scope. A completed enum is [`TypeName::Int`]: the implementation-defined
/// compatible integer type is `int`, and every enumeration constant has type
/// `int`.
///
/// Qualifiers on an array apply to the element type, not the array type
/// (C11 6.7.3p9), so an array is never wrapped in [`CType::Qualified`].
/// `const int *` is a pointer to a qualified `int`. `int * const` is a
/// qualified pointer. `const struct S` is a qualified struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CType {
    Void,
    Int(TypeName),
    Float(FloatKind),
    Pointer(Box<CType>),
    Array(Box<CType>, Option<u64>),
    Struct(StructRef),
    Func(Box<FuncTy>),
    Qualified(Quals, Box<CType>),
}

impl CType {
    pub fn from_name(ty: TypeName) -> Self {
        if ty == TypeName::Void {
            CType::Void
        } else {
            CType::Int(ty)
        }
    }

    pub fn int_type() -> Self {
        CType::Int(TypeName::Int)
    }

    pub fn pointer(pointee: CType) -> Self {
        CType::Pointer(Box::new(pointee))
    }

    pub fn array(elem: CType, len: Option<u64>) -> Self {
        CType::Array(Box::new(elem), len)
    }

    pub fn func(ret: CType, params: Vec<CType>, variadic: bool) -> Self {
        CType::func_with(ret, params, variadic, true)
    }

    /// A function type that may lack a prototype (`int ()`).
    pub fn func_with(ret: CType, params: Vec<CType>, variadic: bool, prototype: bool) -> Self {
        CType::Func(Box::new(FuncTy {
            ret,
            params,
            variadic,
            prototype,
        }))
    }

    pub fn bool_type() -> Self {
        CType::Int(TypeName::Bool)
    }

    /// Drop one layer of qualifiers. Nested qualifiers, including those on a
    /// pointee, stay.
    pub fn unqualified(&self) -> CType {
        match self {
            CType::Qualified(_, inner) => (**inner).clone(),
            other => other.clone(),
        }
    }

    /// Qualifiers on this type itself, not on a pointee or element.
    pub fn quals(&self) -> Quals {
        match self {
            CType::Qualified(quals, _) => *quals,
            _ => Quals::default(),
        }
    }

    pub fn is_const(&self) -> bool {
        self.quals().is_const
    }

    /// Add qualifiers. An array passes them to its element type (C11 6.7.3p9).
    pub fn with_quals(self, extra: Quals) -> CType {
        if extra.is_empty() {
            return self;
        }
        match self {
            CType::Array(elem, len) => CType::array(elem.with_quals(extra), len),
            CType::Qualified(existing, inner) => CType::Qualified(existing.merge(extra), inner),
            other => CType::Qualified(extra, Box::new(other)),
        }
    }

    /// The type with its own qualifiers removed, for queries that do not care
    /// about `const` or `volatile`.
    pub(crate) fn peel(&self) -> &CType {
        match self {
            CType::Qualified(_, inner) => inner.peel(),
            other => other,
        }
    }

    /// A const object, including an array whose elements are const. A pointer
    /// to a const type is not itself const: `const int *` can be reseated.
    pub fn is_const_object(&self) -> bool {
        if self.is_const() {
            return true;
        }
        match self.peel() {
            CType::Array(elem, _) => elem.is_const_object(),
            _ => false,
        }
    }

    pub fn is_void(&self) -> bool {
        matches!(self.peel(), CType::Void)
    }

    pub fn is_integer(&self) -> bool {
        matches!(self.peel(), CType::Int(_))
    }

    pub fn is_pointer(&self) -> bool {
        matches!(self.peel(), CType::Pointer(_))
    }

    pub fn is_array(&self) -> bool {
        matches!(self.peel(), CType::Array(_, _))
    }

    pub fn is_struct(&self) -> bool {
        self.struct_ref().is_some_and(|handle| !handle.is_union())
    }

    pub fn is_union(&self) -> bool {
        self.struct_ref().is_some_and(|handle| handle.is_union())
    }

    /// A struct or a union.
    pub fn is_record(&self) -> bool {
        matches!(self.peel(), CType::Struct(_))
    }

    pub fn is_float(&self) -> bool {
        matches!(self.peel(), CType::Float(_))
    }

    pub fn as_float(&self) -> Option<FloatKind> {
        match self.peel() {
            CType::Float(kind) => Some(*kind),
            _ => None,
        }
    }

    pub fn is_volatile(&self) -> bool {
        self.quals().is_volatile
    }

    pub fn record_has_bitfield(&self) -> bool {
        self.struct_ref().is_some_and(StructRef::has_bitfield)
    }

    /// A struct with a flexible array member, or a union that contains one
    /// at any depth (C11 6.7.2.1p3). Pointers are not walked: a pointer to
    /// such a struct is an ordinary pointer.
    pub fn flexible_carrier(&self) -> bool {
        match self.peel() {
            CType::Array(elem, _) => elem.flexible_carrier(),
            CType::Struct(handle) => {
                if !handle.is_complete() {
                    return false;
                }
                if handle.is_union() {
                    handle
                        .fields()
                        .unwrap_or_default()
                        .iter()
                        .any(|field| field.bit_width.is_none() && field.ty.flexible_carrier())
                } else {
                    handle.has_flexible_member()
                }
            }
            _ => false,
        }
    }

    /// An array whose element is a flexible-array carrier, including through
    /// further arrays and through a pointer (`struct F (*)[2]`).
    pub fn contains_flexible_array(&self) -> bool {
        match self.peel() {
            CType::Array(elem, _) => elem.flexible_carrier() || elem.contains_flexible_array(),
            CType::Pointer(inner) => inner.contains_flexible_array(),
            CType::Func(func) => {
                func.ret.contains_flexible_array()
                    || func.params.iter().any(CType::contains_flexible_array)
            }
            _ => false,
        }
    }

    pub fn is_func(&self) -> bool {
        matches!(self.peel(), CType::Func(_))
    }

    pub fn func_ty(&self) -> Option<&FuncTy> {
        match self.peel() {
            CType::Func(func) => Some(func),
            _ => None,
        }
    }

    pub fn struct_ref(&self) -> Option<&StructRef> {
        match self.peel() {
            CType::Struct(handle) => Some(handle),
            _ => None,
        }
    }

    /// Integer, floating type, or pointer. Arrays and structs are not
    /// scalars; an array decays first, and a struct is copied or accessed
    /// by member.
    pub fn is_scalar(&self) -> bool {
        self.is_integer() || self.is_float() || self.is_pointer()
    }

    /// Integer or floating type. Pointers are scalars but not arithmetic.
    pub fn is_arithmetic(&self) -> bool {
        self.is_integer() || self.is_float()
    }

    pub fn float_kind(&self) -> Option<FloatKind> {
        match self.peel() {
            CType::Float(kind) => Some(*kind),
            _ => None,
        }
    }

    pub fn is_long_double(&self) -> bool {
        self.float_kind() == Some(FloatKind::LongDouble)
    }

    /// `float` or `double`. `long double` uses the x87 registers instead.
    pub fn is_sse_float(&self) -> bool {
        matches!(
            self.float_kind(),
            Some(FloatKind::Float | FloatKind::Double)
        )
    }

    /// `const` anywhere in this object: the type itself, an element, or a
    /// field, recursively. A pointer is not walked, so `const int *` can
    /// still be reseated. A struct with a const field is not a modifiable
    /// lvalue (C11 6.3.2.1).
    pub fn contains_const(&self) -> bool {
        if self.is_const() {
            return true;
        }
        match self.peel() {
            CType::Array(elem, _) => elem.contains_const(),
            CType::Struct(handle) => handle
                .fields()
                .unwrap_or_default()
                .iter()
                .any(|field| field.ty.contains_const()),
            CType::Func(func) => func.ret.contains_const(),
            _ => false,
        }
    }

    pub fn is_void_pointer(&self) -> bool {
        matches!(self.peel(), CType::Pointer(inner) if inner.is_void())
    }

    /// `char`, `signed char`, or `unsigned char`, ignoring qualifiers.
    pub fn is_character(&self) -> bool {
        matches!(
            self.as_integer(),
            Some(TypeName::Char | TypeName::SChar | TypeName::UChar)
        )
    }

    pub fn as_integer(&self) -> Option<TypeName> {
        match self.peel() {
            CType::Int(ty) => Some(*ty),
            _ => None,
        }
    }

    pub fn pointee(&self) -> Option<&CType> {
        match self.peel() {
            CType::Pointer(inner) => Some(inner),
            _ => None,
        }
    }

    pub fn array_elem(&self) -> Option<&CType> {
        match self.peel() {
            CType::Array(elem, _) => Some(elem),
            _ => None,
        }
    }

    pub fn array_len(&self) -> Option<u64> {
        match self.peel() {
            CType::Array(_, len) => *len,
            _ => None,
        }
    }

    pub fn is_complete(&self) -> bool {
        match self.peel() {
            CType::Void | CType::Func(_) => false,
            CType::Int(_) | CType::Float(_) | CType::Pointer(_) => true,
            CType::Array(elem, Some(_)) => elem.is_complete(),
            CType::Array(_, None) => false,
            CType::Struct(handle) => handle.is_complete(),
            CType::Qualified(_, _) => unreachable!("peel removes qualifiers"),
        }
    }

    /// Size in bytes. Incomplete types have no size.
    pub fn size(&self) -> Option<u64> {
        match self.peel() {
            CType::Void | CType::Func(_) => None,
            CType::Int(ty) => Some(ty.size()),
            CType::Float(kind) => Some(kind.size()),
            CType::Pointer(_) => Some(8),
            CType::Array(elem, Some(len)) => elem.size().map(|elem| elem.saturating_mul(*len)),
            CType::Array(_, None) => None,
            CType::Struct(handle) => handle.size(),
            CType::Qualified(_, _) => unreachable!("peel removes qualifiers"),
        }
    }

    /// Alignment in bytes.
    ///
    /// The System V AMD64 ABI gives an array of 16 bytes or more an alignment
    /// of at least 16. Smaller arrays use the element alignment. Pointers are
    /// 8-byte aligned. Qualifiers do not change the alignment.
    pub fn align(&self) -> u32 {
        match self.peel() {
            CType::Void | CType::Func(_) => 1,
            CType::Int(ty) => ty.align(),
            CType::Float(kind) => kind.align(),
            CType::Pointer(_) => 8,
            CType::Array(elem, _) => {
                let elem_align = elem.align();
                match self.size() {
                    Some(size) if size >= 16 => elem_align.max(16),
                    _ => elem_align,
                }
            }
            CType::Struct(handle) => handle.align(),
            CType::Qualified(_, _) => unreachable!("peel removes qualifiers"),
        }
    }

    /// Alignment of this type when it is a field.
    ///
    /// An array field is aligned as its element is, with no 16-byte boost.
    /// A local or global array variable still uses [`CType::align`], which
    /// applies that boost. A struct field uses the struct's alignment.
    pub fn member_align(&self) -> u32 {
        match self.peel() {
            CType::Array(elem, _) => elem.member_align(),
            other => other.align(),
        }
    }

    pub fn ir_type(&self) -> String {
        match self.peel() {
            CType::Void => "void".to_string(),
            CType::Int(ty) => ty.ir_type().to_string(),
            CType::Float(kind) => kind.ir_type().to_string(),
            CType::Pointer(_) => "ptr".to_string(),
            CType::Func(_) => "void".to_string(),
            CType::Array(elem, len) => {
                let n = len.unwrap_or(0);
                format!("[{n} x {}]", elem.ir_type())
            }
            CType::Struct(handle) => format!("%{}", handle.ir_name()),
            CType::Qualified(_, _) => unreachable!("peel removes qualifiers"),
        }
    }

    /// `signext` / `zeroext` for integer parameters narrower than 32 bits.
    /// Pointers are passed as `ptr` with no extension attribute.
    pub fn abi_attr(&self) -> Option<&'static str> {
        self.as_integer().and_then(TypeName::abi_attr)
    }

    /// C spelling of the type, such as `int *`, `int *[3]`, or `int (*)[3]`.
    pub fn display(&self) -> String {
        render_type(self, "")
    }
}

fn render_type(ty: &CType, name: &str) -> String {
    match ty {
        CType::Qualified(quals, inner) => match inner.as_ref() {
            CType::Pointer(pointee) => render_type(pointee, &star_decl(pointee, *quals, name)),
            CType::Qualified(_, _) | CType::Array(_, _) => render_type(inner, name),
            CType::Void => join_spec(&format!("{}void", qual_prefix(*quals)), name),
            CType::Int(scalar) => {
                join_spec(&format!("{}{}", qual_prefix(*quals), scalar.as_str()), name)
            }
            CType::Float(kind) => {
                join_spec(&format!("{}{}", qual_prefix(*quals), kind.as_str()), name)
            }
            CType::Struct(handle) => join_spec(
                &format!("{}{}", qual_prefix(*quals), record_spelling(handle)),
                name,
            ),
            CType::Func(_) => render_type(inner, name),
        },
        CType::Void => join_spec("void", name),
        CType::Int(scalar) => join_spec(scalar.as_str(), name),
        CType::Float(kind) => join_spec(kind.as_str(), name),
        CType::Struct(handle) => join_spec(&record_spelling(handle), name),
        CType::Func(func) => {
            let applied = format!("{name}({})", render_params(func));
            render_type(&func.ret, &applied)
        }
        CType::Pointer(inner) => render_type(inner, &star_decl(inner, Quals::default(), name)),
        CType::Array(elem, len) => {
            let bracket = match len {
                Some(n) => format!("[{n}]"),
                None => "[]".to_string(),
            };
            render_type(elem, &format!("{name}{bracket}"))
        }
    }
}

fn record_spelling(handle: &StructRef) -> String {
    let word = handle.kind().word();
    let tag = handle.tag();
    if tag.is_empty() {
        word.to_string()
    } else {
        format!("{word} {tag}")
    }
}

fn render_params(func: &FuncTy) -> String {
    if !func.prototype {
        return String::new();
    }
    let mut parts: Vec<String> = func.params.iter().map(|ty| ty.display()).collect();
    if func.variadic {
        parts.push("...".to_string());
    }
    if parts.is_empty() {
        "void".to_string()
    } else {
        parts.join(", ")
    }
}

fn qual_words(quals: Quals) -> Vec<&'static str> {
    let mut words = Vec::new();
    if quals.is_const {
        words.push("const");
    }
    if quals.is_volatile {
        words.push("volatile");
    }
    if quals.is_restrict {
        words.push("restrict");
    }
    words
}

fn qual_prefix(quals: Quals) -> String {
    let words = qual_words(quals);
    if words.is_empty() {
        String::new()
    } else {
        format!("{} ", words.join(" "))
    }
}

/// Text after `*`: empty, or ` const`, ` volatile`, ` restrict`, in that order.
fn qual_after_star(quals: Quals) -> String {
    let words = qual_words(quals);
    if words.is_empty() {
        String::new()
    } else {
        format!(" {}", words.join(" "))
    }
}

fn star_decl(pointee: &CType, quals: Quals, name: &str) -> String {
    let q = qual_after_star(quals);
    let needs_paren = matches!(pointee.peel(), CType::Array(_, _) | CType::Func(_));
    if needs_paren {
        if q.is_empty() {
            format!("(*{name})")
        } else if name.is_empty() {
            format!("(*{q})")
        } else {
            format!("(*{q} {name})")
        }
    } else if name.is_empty() {
        if q.is_empty() {
            "*".to_string()
        } else {
            format!("*{q}")
        }
    } else if q.is_empty() {
        format!("*{name}")
    } else {
        format!("*{q} {name}")
    }
}

fn join_spec(spec: &str, name: &str) -> String {
    if name.is_empty() {
        spec.to_string()
    } else {
        format!("{spec} {name}")
    }
}

/// Pointers to qualified or unqualified versions of compatible types
/// (C11 6.5.8, 6.5.9). One level of pointee qualifiers is ignored.
/// `const int *` may be compared with `int *`. `char **` may not be compared
/// with `const char **`, because `char *` and `const char *` are not
/// compatible. Semantic analysis and the parser's folding of constant
/// expressions share this rule.
pub fn loose_pointers(left: &CType, right: &CType) -> bool {
    match (left.pointee(), right.pointee()) {
        (Some(a), Some(b)) => compatible(&a.unqualified(), &b.unqualified()),
        _ => false,
    }
}

/// Two pointers of which at least one is `void *` (possibly qualified), the
/// other pointer case an equality operator allows (C11 6.5.9p2).
pub fn void_ptr_mix(left: &CType, right: &CType) -> bool {
    left.is_pointer() && right.is_pointer() && (left.is_void_pointer() || right.is_void_pointer())
}

/// Two types are compatible (C11 6.2.7, 6.7.6, 6.7.3).
///
/// Qualifiers must match at every level. Arrays are compatible when their
/// elements are and the lengths are equal or either length is unknown. The
/// composite type keeps a known length. Function parameters strip top-level
/// qualifiers before this check (C11 6.7.6.3p15); that happens at the call
/// site, not here.
pub fn compatible(left: &CType, right: &CType) -> bool {
    if left.quals() != right.quals() {
        return false;
    }
    match (left.peel(), right.peel()) {
        (CType::Void, CType::Void) => true,
        (CType::Int(a), CType::Int(b)) => a == b,
        (CType::Float(a), CType::Float(b)) => a == b,
        (CType::Pointer(a), CType::Pointer(b)) => compatible(a, b),
        (CType::Func(a), CType::Func(b)) => compatible_funcs(a, b),
        (CType::Array(a, la), CType::Array(b, lb)) => {
            (la.is_none() || lb.is_none() || la == lb) && compatible(a, b)
        }
        (CType::Struct(a), CType::Struct(b)) => a == b,
        _ => false,
    }
}

/// Function type compatibility (C11 6.7.6.3p15).
///
/// Two prototypes agree in the number of parameters, in `...`, and in each
/// parameter type. A type without a prototype (`()`) is compatible with a
/// prototype that has no `...` and whose parameter types are each compatible
/// with their default argument promotion, so `int f();` matches
/// `int f(int)` and `int f(double)` but not `int f(char)` or `int f(float)`.
/// Two types without prototypes need only compatible return types.
pub fn compatible_funcs(a: &FuncTy, b: &FuncTy) -> bool {
    if !compatible(&a.ret, &b.ret) {
        return false;
    }
    match (a.prototype, b.prototype) {
        (true, true) => {
            a.variadic == b.variadic
                && a.params.len() == b.params.len()
                && a.params
                    .iter()
                    .zip(b.params.iter())
                    .all(|(left, right)| compatible(left, right))
        }
        (false, false) => true,
        (true, false) => prototype_matches_promotions(a),
        (false, true) => prototype_matches_promotions(b),
    }
}

/// A prototype is compatible with a non-prototype function type only if it
/// has no `...` and every parameter survives the default argument
/// promotions unchanged (C11 6.7.6.3p15, 6.5.2.2p6).
pub fn prototype_matches_promotions(func: &FuncTy) -> bool {
    !func.variadic
        && func
            .params
            .iter()
            .all(|param| compatible(param, &default_promoted(param)))
}

/// The default argument promotions (C11 6.5.2.2p6): the integer promotions,
/// and `float` to `double`. Other types are unchanged, without qualifiers.
pub fn default_promoted(ty: &CType) -> CType {
    match ty.peel() {
        CType::Int(name) => CType::Int(name.promote()),
        CType::Float(FloatKind::Float) => CType::Float(FloatKind::Double),
        other => other.clone(),
    }
}

/// Composite type of two compatible types. A known array length wins.
pub fn composite(left: &CType, right: &CType) -> CType {
    let base = match (left.peel(), right.peel()) {
        (CType::Func(a), CType::Func(b)) => {
            // A prototype wins over a type without one (C11 6.2.7p3).
            let (proto, other) = if a.prototype || !b.prototype {
                (a, b)
            } else {
                (b, a)
            };
            let params = if proto.prototype && other.prototype {
                proto
                    .params
                    .iter()
                    .zip(other.params.iter())
                    .map(|(left, right)| composite(left, right))
                    .collect()
            } else {
                proto.params.clone()
            };
            CType::func_with(
                composite(&a.ret, &b.ret),
                params,
                proto.variadic,
                proto.prototype,
            )
        }
        (CType::Array(a, la), CType::Array(b, lb)) => CType::array(composite(a, b), la.or(*lb)),
        (CType::Pointer(a), CType::Pointer(b)) => CType::pointer(composite(a, b)),
        _ => left.peel().clone(),
    };
    base.with_quals(left.quals())
}

pub fn width_mask(width: u32) -> u64 {
    if width >= 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    }
}

/// Sign-extend the low `width` bits of `bits` to `i64`.
pub fn sign_extend(bits: u64, width: u32) -> i64 {
    let bits = bits & width_mask(width);
    let shift = 64 - width;
    ((bits << shift) as i64) >> shift
}

/// Convert `bits` from `from` to `to` (C11 6.3.1.3).
///
/// Unsigned destinations reduce the value modulo 2^width. A signed
/// destination that cannot represent the value is implementation-defined;
/// this target keeps the low bits and reads them as two's complement, which
/// is what Clang does on `x86_64-pc-linux-gnu`.
pub fn convert_bits(bits: u64, from: TypeName, to: TypeName) -> u64 {
    // C11 6.3.1.2: any nonzero value becomes 1. Truncation would turn 2 into 0.
    if to == TypeName::Bool && from != TypeName::Bool {
        let zero = if from.is_signed() {
            sign_extend(bits, from.width()) == 0
        } else {
            bits & from.mask() == 0
        };
        return u64::from(!zero);
    }
    let to_mask = to.mask();
    if to.width() <= from.width() {
        return bits & to_mask;
    }
    if from.is_signed() {
        (sign_extend(bits, from.width()) as u64) & to_mask
    } else {
        bits & from.mask()
    }
}

/// Storage-class specifier on a declaration.
///
/// At most one of `static` and `extern` is allowed. Absence is not the same
/// thing as `extern` for objects: a file-scope object with no storage class
/// has external linkage and is a tentative definition when it has no
/// initializer (C11 6.2.2, 6.9.2). A block-scope object with no storage class
/// has automatic storage and no linkage. Block-scope `static` has static
/// storage duration and still no linkage. Block-scope `extern` has linkage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Storage {
    #[default]
    None,
    Static,
    Extern,
    /// `typedef`. The declaration introduces an alias, not an object.
    Typedef,
}

/// Linkage after C11 6.2.2 has been applied to a declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Linkage {
    External,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

impl BinOp {
    pub fn as_str(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::Le => "<=",
            BinOp::Ge => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            BinOp::BitXor => "^",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
        }
    }

    /// Spelling of `E1 op= E2` for the operators that have a compound form.
    pub fn compound_spelling(self) -> &'static str {
        match self {
            BinOp::Add => "+=",
            BinOp::Sub => "-=",
            BinOp::Mul => "*=",
            BinOp::Div => "/=",
            BinOp::Mod => "%=",
            BinOp::BitAnd => "&=",
            BinOp::BitOr => "|=",
            BinOp::BitXor => "^=",
            BinOp::Shl => "<<=",
            BinOp::Shr => ">>=",
            BinOp::Eq
            | BinOp::Ne
            | BinOp::Lt
            | BinOp::Gt
            | BinOp::Le
            | BinOp::Ge
            | BinOp::And
            | BinOp::Or => unreachable!("comparisons and && / || have no compound form"),
        }
    }

    pub fn is_cmp(self) -> bool {
        matches!(
            self,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
    BitNot,
    Addr,
    Deref,
}

/// Prefix and postfix `++` / `--`. The result of either form is not an lvalue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncOp {
    PreInc,
    PreDec,
    PostInc,
    PostDec,
}

impl IncOp {
    pub fn is_prefix(self) -> bool {
        matches!(self, IncOp::PreInc | IncOp::PreDec)
    }

    pub fn is_increment(self) -> bool {
        matches!(self, IncOp::PreInc | IncOp::PostInc)
    }
}

/// Operand of `sizeof`. A type name is only accepted in parentheses.
#[derive(Debug, Clone)]
pub enum SizeofOperand {
    Expr(Box<Expr>),
    Type(CType),
}

/// Brace-enclosed initializer, or a single expression.
///
/// `int x = {1}` is a scalar wrapped in one pair of braces. Arrays use a list.
/// A trailing comma is dropped by the parser.
#[derive(Debug, Clone)]
pub enum Init {
    Expr(Expr),
    List {
        items: Vec<Init>,
        line: usize,
        column: usize,
    },
    /// A designated initializer: `.field = ...` or `[index] = ...`, including
    /// a chain such as `.a[1].b`. The designators are relative to the object
    /// of the enclosing brace list.
    Designated {
        designators: Vec<Designator>,
        init: Box<Init>,
        line: usize,
        column: usize,
    },
}

/// One step of a designated initializer.
#[derive(Debug, Clone)]
pub enum Designator {
    Field {
        name: String,
        line: usize,
        column: usize,
    },
    Index {
        expr: Expr,
        line: usize,
        column: usize,
    },
}

/// One scalar or pointer store inside an aggregate initializer.
///
/// `offset` is the byte offset from the start of the object. Semantic analysis
/// fills these after the destination type is known, including a length inferred
/// from `int a[] = {1, 2, 3}`.
#[derive(Debug, Clone)]
pub struct ElemStore {
    pub offset: u64,
    pub ty: CType,
    pub expr: Expr,
    /// Set when the store writes a bit-field. `abs_bit` is from the start of
    /// the object being initialized. The value is converted to `ty` and then
    /// truncated to `width` bits.
    pub bits: Option<BitStore>,
}

/// Where a bit-field initializer is written.
#[derive(Debug, Clone)]
pub struct BitStore {
    pub abs_bit: u32,
    pub width: u32,
    pub container_bits: u32,
    pub signed_field: bool,
    /// Declared type, before the integer promotions. A `_Bool` field converts
    /// to 0 or 1 before the width mask.
    pub declared: TypeName,
}

/// Initializer after type checking. Aggregates are flat element stores; the
/// bytes that are not listed are zero.
#[derive(Debug, Clone)]
pub enum CheckedInit {
    Scalar(Expr),
    Aggregate(Vec<ElemStore>),
}

/// A pointer constant (C11 6.6): null, an address, or an integer converted to
/// a pointer. The address is a file-scope object plus a byte offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PtrConst {
    Null,
    Addr {
        name: String,
        offset: i64,
    },
    /// Low 64 bits of an integer, converted with `inttoptr`.
    IntToPtr(u64),
}

/// Constant initializer value. Integers keep the low bits of their type.
/// `Zero` is the all-zero value of whatever type it is attached to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstInit {
    Zero,
    Int(u64),
    /// IEEE bits of a `float` (low 32), `double` (low 64), or the 80-bit
    /// extended pattern of a `long double`. The destination type selects
    /// the width.
    Float(u128),
    Ptr(PtrConst),
    /// `ptrtoint` of a pointer constant, then truncated to `width` bits.
    PtrToInt {
        ptr: PtrConst,
        width: u32,
    },
    Aggregate(Vec<ConstInit>),
    /// Raw little-endian bytes of a bit-field aggregate.
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarRes {
    pub slot: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallRes {
    pub name: String,
    pub ret: CType,
    pub params: Vec<CType>,
    /// The callee was declared with `...`. Arguments past `params` use the
    /// default argument promotions.
    pub variadic: bool,
    /// The callee is a pointer expression. `name` is empty. A direct call
    /// names the function and leaves this false.
    pub indirect: bool,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Int {
        /// Low bits of the value. The high bits above [`TypeName::width`] are zero.
        bits: u64,
        /// Type of the literal or character constant (C11 6.4.4.1, 6.4.4.4).
        value_ty: TypeName,
        line: usize,
        column: usize,
        ty: Option<CType>,
    },
    /// A floating constant (C11 6.4.4.2). `bits` are `f32` bits in the low
    /// 32 for [`FloatKind::Float`], `f64` bits for `double`, and the 80-bit
    /// extended pattern for `long double`.
    Float {
        bits: u128,
        kind: FloatKind,
        line: usize,
        column: usize,
        ty: Option<CType>,
    },
    Var {
        name: String,
        line: usize,
        column: usize,
        resolved: Option<VarRes>,
        ty: Option<CType>,
    },
    Bin {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<CType>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<CType>,
    },
    Assign {
        target: Box<Expr>,
        value: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<CType>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        line: usize,
        column: usize,
        resolved: Option<CallRes>,
        ty: Option<CType>,
    },
    /// `E1 op= E2`. `target` is the lvalue, evaluated once.
    CompoundAssign {
        op: BinOp,
        target: Box<Expr>,
        value: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<CType>,
    },
    IncDec {
        op: IncOp,
        target: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<CType>,
    },
    Cond {
        cond: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<CType>,
    },
    Comma {
        left: Box<Expr>,
        right: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<CType>,
    },
    Sizeof {
        operand: SizeofOperand,
        line: usize,
        column: usize,
        ty: Option<CType>,
    },
    /// `(type)expr`, including `(void)expr`.
    Cast {
        ty_name: CType,
        expr: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<CType>,
    },
    /// `base[index]`, an lvalue of the element type. The base is decayed.
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<CType>,
    },
    /// Array-to-pointer decay inserted by semantic analysis. The inner
    /// expression keeps its array type; this node's type is the pointer.
    Decay {
        expr: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<CType>,
    },
    /// A string literal. `bytes` does not include the implicit NUL. The
    /// encoding selects the element type and the width of each code unit.
    /// `symbol` is the private global that holds the literal, filled in when
    /// the address is needed.
    Str {
        bytes: Vec<u8>,
        enc: StrEnc,
        line: usize,
        column: usize,
        symbol: Option<String>,
        ty: Option<CType>,
    },
    /// `base.field` or `base->field`. `arrow` selects `->`. Semantic analysis
    /// fills `index` and `offset` from the struct's layout.
    Member {
        base: Box<Expr>,
        field: String,
        arrow: bool,
        line: usize,
        column: usize,
        index: Option<usize>,
        offset: Option<u64>,
        /// Address the member with a byte `getelementptr` from the base.
        /// Unions do this: every member is at offset 0, and the LLVM type is
        /// not a list of those members. A struct that contains a bit-field
        /// does too.
        bytewise: bool,
        /// Set for a bit-field. `ty` is the type of the value after integer
        /// promotion. `bit_width` is the declared width.
        bit_width: Option<u32>,
        /// The declared bit-field type is signed, so a read sign-extends.
        bit_signed: bool,
        /// Storage unit width in bits: 8, 16, 32, or 64.
        container_bits: u32,
        /// Bit position of the field from the start of the struct.
        abs_bit: u32,
        /// Declared type of a bit-field, before integer promotions. `_Bool`
        /// converts a nonzero value to 1 before the field is masked.
        bit_declared: TypeName,
        ty: Option<CType>,
    },
    /// `__builtin_va_arg(ap, type)`.
    VaArg {
        ap: Box<Expr>,
        type_arg: CType,
        line: usize,
        column: usize,
        ty: Option<CType>,
    },
    /// `__builtin_va_start`, `__builtin_va_end`, or `__builtin_va_copy`.
    /// The result type is `void`.
    VaOp {
        op: VaOp,
        args: Vec<Expr>,
        line: usize,
        column: usize,
        ty: Option<CType>,
    },
    /// `_Generic ( assignment-expression , generic-assoc-list )`
    /// (C11 6.5.1.1). Semantic analysis replaces the node with the selected
    /// association's expression, so later passes never see it.
    Generic {
        control: Box<Expr>,
        assocs: Vec<GenericAssoc>,
        line: usize,
        column: usize,
        ty: Option<CType>,
    },
    /// A compound literal `( type-name ) { initializer-list }` (C11 6.5.2.5).
    /// `decl` is the unnamed object it creates. In a function body the
    /// object is automatic and is initialized each time the literal is
    /// evaluated; semantic analysis fills `decl.checked`. Outside a function
    /// it has static storage: semantic analysis makes it a private global
    /// and replaces the node with a reference to that object.
    CompoundLit {
        decl: Box<VarDecl>,
        line: usize,
        column: usize,
        ty: Option<CType>,
    },
}

/// One association of a generic selection. `ty` is `None` for `default`.
#[derive(Debug, Clone)]
pub struct GenericAssoc {
    pub ty: Option<CType>,
    pub expr: Expr,
    pub line: usize,
    pub column: usize,
}

/// Which `<stdarg.h>` operation a [`Expr::VaOp`] performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaOp {
    Start,
    End,
    Copy,
}

impl Expr {
    pub fn int(bits: u64, value_ty: TypeName, line: usize, column: usize) -> Self {
        Expr::Int {
            bits,
            value_ty,
            line,
            column,
            ty: None,
        }
    }

    pub fn float_lit(bits: u128, kind: FloatKind, line: usize, column: usize) -> Self {
        Expr::Float {
            bits,
            kind,
            line,
            column,
            ty: None,
        }
    }

    pub fn cast(ty_name: CType, expr: Expr, line: usize, column: usize) -> Self {
        Expr::Cast {
            ty_name,
            expr: Box::new(expr),
            line,
            column,
            ty: None,
        }
    }

    pub fn var(name: String, line: usize, column: usize) -> Self {
        Expr::Var {
            name,
            line,
            column,
            resolved: None,
            ty: None,
        }
    }

    pub fn bin(op: BinOp, left: Expr, right: Expr, line: usize, column: usize) -> Self {
        Expr::Bin {
            op,
            left: Box::new(left),
            right: Box::new(right),
            line,
            column,
            ty: None,
        }
    }

    pub fn unary(op: UnaryOp, expr: Expr, line: usize, column: usize) -> Self {
        Expr::Unary {
            op,
            expr: Box::new(expr),
            line,
            column,
            ty: None,
        }
    }

    pub fn assign(target: Expr, value: Expr, line: usize, column: usize) -> Self {
        Expr::Assign {
            target: Box::new(target),
            value: Box::new(value),
            line,
            column,
            ty: None,
        }
    }

    pub fn call(callee: Expr, args: Vec<Expr>, line: usize, column: usize) -> Self {
        Expr::Call {
            callee: Box::new(callee),
            args,
            line,
            column,
            resolved: None,
            ty: None,
        }
    }

    pub fn compound(op: BinOp, target: Expr, value: Expr, line: usize, column: usize) -> Self {
        Expr::CompoundAssign {
            op,
            target: Box::new(target),
            value: Box::new(value),
            line,
            column,
            ty: None,
        }
    }

    pub fn incdec(op: IncOp, target: Expr, line: usize, column: usize) -> Self {
        Expr::IncDec {
            op,
            target: Box::new(target),
            line,
            column,
            ty: None,
        }
    }

    pub fn cond(cond: Expr, then_expr: Expr, else_expr: Expr, line: usize, column: usize) -> Self {
        Expr::Cond {
            cond: Box::new(cond),
            then_expr: Box::new(then_expr),
            else_expr: Box::new(else_expr),
            line,
            column,
            ty: None,
        }
    }

    pub fn comma(left: Expr, right: Expr, line: usize, column: usize) -> Self {
        Expr::Comma {
            left: Box::new(left),
            right: Box::new(right),
            line,
            column,
            ty: None,
        }
    }

    pub fn sizeof_expr(expr: Expr, line: usize, column: usize) -> Self {
        Expr::Sizeof {
            operand: SizeofOperand::Expr(Box::new(expr)),
            line,
            column,
            ty: None,
        }
    }

    pub fn sizeof_type(ty_name: CType, line: usize, column: usize) -> Self {
        Expr::Sizeof {
            operand: SizeofOperand::Type(ty_name),
            line,
            column,
            ty: None,
        }
    }

    pub fn index(base: Expr, index: Expr, line: usize, column: usize) -> Self {
        Expr::Index {
            base: Box::new(base),
            index: Box::new(index),
            line,
            column,
            ty: None,
        }
    }

    pub fn decay(expr: Expr) -> Self {
        let line = expr.line();
        let column = expr.column();
        Expr::Decay {
            expr: Box::new(expr),
            line,
            column,
            ty: None,
        }
    }

    pub fn member(base: Expr, field: String, arrow: bool, line: usize, column: usize) -> Self {
        Expr::Member {
            base: Box::new(base),
            field,
            arrow,
            line,
            column,
            index: None,
            offset: None,
            bytewise: false,
            bit_width: None,
            bit_signed: false,
            container_bits: 0,
            abs_bit: 0,
            bit_declared: TypeName::Int,
            ty: None,
        }
    }

    pub fn va_arg(ap: Expr, type_arg: CType, line: usize, column: usize) -> Self {
        Expr::VaArg {
            ap: Box::new(ap),
            type_arg,
            line,
            column,
            ty: None,
        }
    }

    pub fn va_op(op: VaOp, args: Vec<Expr>, line: usize, column: usize) -> Self {
        Expr::VaOp {
            op,
            args,
            line,
            column,
            ty: None,
        }
    }

    pub fn str_lit(bytes: Vec<u8>, enc: StrEnc, line: usize, column: usize) -> Self {
        Expr::Str {
            bytes,
            enc,
            line,
            column,
            symbol: None,
            ty: None,
        }
    }

    pub fn line(&self) -> usize {
        match self {
            Expr::Int { line, .. }
            | Expr::Float { line, .. }
            | Expr::Var { line, .. }
            | Expr::Bin { line, .. }
            | Expr::Unary { line, .. }
            | Expr::Assign { line, .. }
            | Expr::Call { line, .. }
            | Expr::CompoundAssign { line, .. }
            | Expr::IncDec { line, .. }
            | Expr::Cond { line, .. }
            | Expr::Comma { line, .. }
            | Expr::Sizeof { line, .. }
            | Expr::Cast { line, .. }
            | Expr::Index { line, .. }
            | Expr::Decay { line, .. }
            | Expr::Str { line, .. }
            | Expr::Member { line, .. }
            | Expr::VaArg { line, .. }
            | Expr::VaOp { line, .. }
            | Expr::Generic { line, .. }
            | Expr::CompoundLit { line, .. } => *line,
        }
    }

    pub fn column(&self) -> usize {
        match self {
            Expr::Int { column, .. }
            | Expr::Float { column, .. }
            | Expr::Var { column, .. }
            | Expr::Bin { column, .. }
            | Expr::Unary { column, .. }
            | Expr::Assign { column, .. }
            | Expr::Call { column, .. }
            | Expr::CompoundAssign { column, .. }
            | Expr::IncDec { column, .. }
            | Expr::Cond { column, .. }
            | Expr::Comma { column, .. }
            | Expr::Sizeof { column, .. }
            | Expr::Cast { column, .. }
            | Expr::Index { column, .. }
            | Expr::Decay { column, .. }
            | Expr::Str { column, .. }
            | Expr::Member { column, .. }
            | Expr::VaArg { column, .. }
            | Expr::VaOp { column, .. }
            | Expr::Generic { column, .. }
            | Expr::CompoundLit { column, .. } => *column,
        }
    }

    pub fn ty(&self) -> Option<CType> {
        match self {
            Expr::Int { ty, .. }
            | Expr::Float { ty, .. }
            | Expr::Var { ty, .. }
            | Expr::Bin { ty, .. }
            | Expr::Unary { ty, .. }
            | Expr::Assign { ty, .. }
            | Expr::Call { ty, .. }
            | Expr::CompoundAssign { ty, .. }
            | Expr::IncDec { ty, .. }
            | Expr::Cond { ty, .. }
            | Expr::Comma { ty, .. }
            | Expr::Sizeof { ty, .. }
            | Expr::Cast { ty, .. }
            | Expr::Index { ty, .. }
            | Expr::Decay { ty, .. }
            | Expr::Str { ty, .. }
            | Expr::Member { ty, .. }
            | Expr::VaArg { ty, .. }
            | Expr::VaOp { ty, .. }
            | Expr::Generic { ty, .. }
            | Expr::CompoundLit { ty, .. } => ty.clone(),
        }
    }

    pub fn set_ty(&mut self, ty: CType) {
        match self {
            Expr::Int { ty: slot, .. }
            | Expr::Float { ty: slot, .. }
            | Expr::Var { ty: slot, .. }
            | Expr::Bin { ty: slot, .. }
            | Expr::Unary { ty: slot, .. }
            | Expr::Assign { ty: slot, .. }
            | Expr::Call { ty: slot, .. }
            | Expr::CompoundAssign { ty: slot, .. }
            | Expr::IncDec { ty: slot, .. }
            | Expr::Cond { ty: slot, .. }
            | Expr::Comma { ty: slot, .. }
            | Expr::Sizeof { ty: slot, .. }
            | Expr::Cast { ty: slot, .. }
            | Expr::Index { ty: slot, .. }
            | Expr::Decay { ty: slot, .. }
            | Expr::Str { ty: slot, .. }
            | Expr::Member { ty: slot, .. }
            | Expr::VaArg { ty: slot, .. }
            | Expr::VaOp { ty: slot, .. }
            | Expr::Generic { ty: slot, .. }
            | Expr::CompoundLit { ty: slot, .. } => *slot = Some(ty),
        }
    }

    /// An lvalue designates an object. `->` always does (`(*p).m` and `*p`
    /// is an lvalue). `.` does when its left operand does. The result of an
    /// assignment or `?:` is not an lvalue.
    pub fn is_lvalue(&self) -> bool {
        match self {
            Expr::Var { ty, .. } => ty.as_ref().is_none_or(|ty| !ty.is_func()),
            Expr::Index { .. } | Expr::Str { .. } | Expr::CompoundLit { .. } => true,
            Expr::Unary {
                op: UnaryOp::Deref,
                ty,
                ..
            } => ty.as_ref().is_none_or(|ty| !ty.is_func()),
            Expr::Member { arrow, base, .. } => *arrow || base.is_lvalue(),
            _ => false,
        }
    }

    pub fn var_slot(&self) -> Option<&str> {
        match self {
            Expr::Var {
                resolved: Some(res),
                ..
            } => Some(&res.slot),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub name: String,
    pub ty: CType,
    pub storage: Storage,
    pub init: Option<Init>,
    /// Filled by semantic analysis for an automatic initializer. A `static`
    /// initializer is folded into a global and stays `None` here, so the
    /// function does not store it on every entry.
    pub checked: Option<CheckedInit>,
    pub line: usize,
    pub column: usize,
    pub slot: String,
}

#[derive(Debug, Clone)]
pub struct Param {
    /// `None` for an unnamed prototype parameter (`int f(int, int)`).
    /// A function definition's parameters always have names.
    pub name: Option<String>,
    pub ty: CType,
    pub line: usize,
    pub column: usize,
    pub slot: String,
    pub arg_reg: String,
}

#[derive(Debug, Clone)]
pub enum ForInit {
    Empty,
    Decl(Vec<VarDecl>),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Compound(Compound),
    Decl(VarDecl),
    Return {
        expr: Option<Expr>,
        line: usize,
        column: usize,
    },
    If {
        cond: Expr,
        then_stmt: Box<Stmt>,
        else_stmt: Option<Box<Stmt>>,
        line: usize,
        column: usize,
    },
    While {
        cond: Expr,
        body: Box<Stmt>,
        line: usize,
        column: usize,
    },
    For {
        init: ForInit,
        cond: Option<Expr>,
        step: Option<Expr>,
        body: Box<Stmt>,
        line: usize,
        column: usize,
    },
    /// `do` statement `while` `(` expression `)` `;`.
    ///
    /// `continue` jumps to the condition, not to the top of the body.
    Do {
        body: Box<Stmt>,
        cond: Expr,
        line: usize,
        column: usize,
    },
    /// `switch (` expression `)` statement (C11 6.8.4.2).
    ///
    /// `promoted` is the type of the controlling expression after the integer
    /// promotions. Semantic analysis fills it in. Case labels anywhere in
    /// `body` belong to this switch, except labels in a nested switch.
    Switch {
        cond: Expr,
        body: Box<Stmt>,
        promoted: Option<TypeName>,
        line: usize,
        column: usize,
    },
    /// `case` constant-expression `:` statement.
    ///
    /// `value` is that constant converted to the enclosing switch's promoted
    /// type. Semantic analysis fills it in.
    Case {
        expr: Expr,
        value: Option<u64>,
        stmt: Box<Stmt>,
        line: usize,
        column: usize,
    },
    /// `default :` statement.
    Default {
        stmt: Box<Stmt>,
        line: usize,
        column: usize,
    },
    /// A function declaration inside a block. The body is always absent; a
    /// nested definition is rejected by the parser.
    FuncDecl(FuncDef),
    Expr {
        expr: Option<Expr>,
        line: usize,
        column: usize,
    },
    Break {
        line: usize,
        column: usize,
    },
    Continue {
        line: usize,
        column: usize,
    },
    /// `identifier :` statement. The name has function scope.
    Label {
        name: String,
        stmt: Box<Stmt>,
        line: usize,
        column: usize,
    },
    /// `goto` identifier `;`.
    Goto {
        name: String,
        line: usize,
        column: usize,
    },
}

#[derive(Debug, Clone)]
pub struct Compound {
    pub items: Vec<Stmt>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct FuncDef {
    pub ret_type: CType,
    pub name: String,
    pub params: Vec<Param>,
    /// The parameter list ends with `...`. A definition is accepted. The
    /// body may use `<stdarg.h>`. A declaration with no body is an LLVM
    /// `declare`, so another translation unit can provide it.
    pub variadic: bool,
    /// `false` for an empty `()` list, which declares a function type with
    /// no prototype (C11 6.7.6.3p14). A definition written `f()` has no
    /// parameters, and its type still has no prototype.
    pub prototype: bool,
    /// `None` is a declaration. `Some` is a definition.
    pub body: Option<Compound>,
    pub storage: Storage,
    /// Filled in by semantic analysis. The parser sets [`Linkage::External`].
    pub linkage: Linkage,
    /// Linker name. An `__asm__("...")` label replaces the C name. Calls and
    /// `declare` use this; lookup still uses [`FuncDef::name`].
    pub symbol: String,
    pub line: usize,
    pub column: usize,
}

/// One file-scope object declaration. Several declarations may name the same
/// object; semantic analysis merges them.
#[derive(Debug, Clone)]
pub struct GlobalDecl {
    pub name: String,
    pub ty: CType,
    pub storage: Storage,
    pub init: Option<Init>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub enum TopDecl {
    Func(FuncDef),
    Global(GlobalDecl),
}

/// A file-scope object as it will be emitted. Built by semantic analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrGlobal {
    pub name: String,
    pub ty: CType,
    pub linkage: Linkage,
    /// `None` is an external declaration. `Some` is a definition, including
    /// a tentative definition completed as zero.
    pub init: Option<ConstInit>,
}

impl IrGlobal {
    /// Low bits of an integer definition. `None` when the global is not an
    /// integer constant (a pointer, an aggregate, or an external declaration).
    pub fn int_init(&self) -> Option<u64> {
        match &self.init {
            Some(ConstInit::Int(bits)) => Some(*bits),
            Some(ConstInit::Zero) if self.ty.is_integer() => Some(0),
            _ => None,
        }
    }
}

/// A function that was declared and not defined. Emitted as `declare`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrFunc {
    pub name: String,
    pub ret: CType,
    pub params: Vec<CType>,
    pub variadic: bool,
    pub linkage: Linkage,
}

/// A string literal lowered to a private unnamed_addr constant.
///
/// `bytes` includes the terminating NUL. The name is `.str` or `.str.N`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrConst {
    pub name: String,
    /// Includes the terminating NUL. Wider encodings store little-endian
    /// code units, including the terminating wide NUL.
    pub bytes: Vec<u8>,
    pub enc: StrEnc,
}

#[derive(Debug, Clone)]
pub struct TranslationUnit {
    pub decls: Vec<TopDecl>,
    /// Filled by semantic analysis, in order of first declaration.
    pub globals: Vec<IrGlobal>,
    /// Prototypes with no definition in this translation unit.
    pub func_decls: Vec<IrFunc>,
    /// String literals whose address is used, in order of first use.
    pub strings: Vec<StrConst>,
    /// Completed struct types, in the order they were defined. An incomplete
    /// struct that is never completed is not listed; a pointer to it is `ptr`.
    pub structs: Vec<StructRef>,
}

pub fn dump(unit: &TranslationUnit) -> String {
    let mut out = String::new();
    for decl in &unit.decls {
        match decl {
            TopDecl::Func(func) => dump_func(&mut out, func, 0),
            TopDecl::Global(global) => dump_global(&mut out, global, 0),
        }
    }
    out
}

fn put(out: &mut String, indent: usize, text: &str) {
    for _ in 0..indent {
        out.push_str("  ");
    }
    out.push_str(text);
    out.push('\n');
}

fn storage_prefix(storage: Storage) -> &'static str {
    match storage {
        Storage::None | Storage::Typedef => "",
        Storage::Static => "static ",
        Storage::Extern => "extern ",
    }
}

fn dump_global(out: &mut String, global: &GlobalDecl, indent: usize) {
    let prefix = storage_prefix(global.storage);
    let ty = global.ty.display();
    if let Some(init) = &global.init {
        put(
            out,
            indent,
            &format!("{prefix}global {ty} {} =", global.name),
        );
        dump_init(out, init, indent + 1);
    } else {
        put(out, indent, &format!("{prefix}global {ty} {}", global.name));
    }
}

fn dump_func(out: &mut String, func: &FuncDef, indent: usize) {
    let kind = if func.body.is_some() { "func" } else { "proto" };
    put(
        out,
        indent,
        &format!(
            "{}{kind} {} {}",
            storage_prefix(func.storage),
            func.ret_type.display(),
            func.name
        ),
    );
    for param in &func.params {
        match &param.name {
            Some(name) => put(
                out,
                indent + 1,
                &format!("param {} {name}", param.ty.display()),
            ),
            None => put(out, indent + 1, &format!("param {}", param.ty.display())),
        }
    }
    if func.variadic {
        put(out, indent + 1, "varargs");
    }
    if let Some(body) = &func.body {
        dump_compound(out, body, indent + 1);
    }
}

fn dump_local(out: &mut String, decl: &VarDecl, indent: usize) {
    let prefix = storage_prefix(decl.storage);
    let ty = decl.ty.display();
    if let Some(init) = &decl.init {
        put(out, indent, &format!("{prefix}decl {ty} {} =", decl.name));
        dump_init(out, init, indent + 1);
    } else {
        put(out, indent, &format!("{prefix}decl {ty} {}", decl.name));
    }
}

fn dump_compound(out: &mut String, compound: &Compound, indent: usize) {
    put(out, indent, "block");
    for item in &compound.items {
        dump_stmt(out, item, indent + 1);
    }
}

fn dump_stmt(out: &mut String, stmt: &Stmt, indent: usize) {
    match stmt {
        Stmt::Compound(compound) => dump_compound(out, compound, indent),
        Stmt::Decl(decl) => dump_local(out, decl, indent),
        Stmt::FuncDecl(func) => dump_func(out, func, indent),
        Stmt::Return { expr, .. } => {
            put(out, indent, "return");
            if let Some(expr) = expr {
                dump_expr(out, expr, indent + 1);
            }
        }
        Stmt::If {
            cond,
            then_stmt,
            else_stmt,
            ..
        } => {
            put(out, indent, "if");
            put(out, indent + 1, "cond");
            dump_expr(out, cond, indent + 2);
            put(out, indent + 1, "then");
            dump_stmt(out, then_stmt, indent + 2);
            if let Some(else_stmt) = else_stmt {
                put(out, indent + 1, "else");
                dump_stmt(out, else_stmt, indent + 2);
            }
        }
        Stmt::While { cond, body, .. } => {
            put(out, indent, "while");
            put(out, indent + 1, "cond");
            dump_expr(out, cond, indent + 2);
            put(out, indent + 1, "body");
            dump_stmt(out, body, indent + 2);
        }
        Stmt::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            put(out, indent, "for");
            match init {
                ForInit::Empty => put(out, indent + 1, "init"),
                ForInit::Expr(expr) => {
                    put(out, indent + 1, "init");
                    dump_expr(out, expr, indent + 2);
                }
                ForInit::Decl(decls) => {
                    put(out, indent + 1, "init");
                    for decl in decls {
                        dump_local(out, decl, indent + 2);
                    }
                }
            }
            put(out, indent + 1, "cond");
            if let Some(cond) = cond {
                dump_expr(out, cond, indent + 2);
            }
            put(out, indent + 1, "step");
            if let Some(step) = step {
                dump_expr(out, step, indent + 2);
            }
            put(out, indent + 1, "body");
            dump_stmt(out, body, indent + 2);
        }
        Stmt::Do { body, cond, .. } => {
            put(out, indent, "do");
            put(out, indent + 1, "body");
            dump_stmt(out, body, indent + 2);
            put(out, indent + 1, "cond");
            dump_expr(out, cond, indent + 2);
        }
        Stmt::Switch { cond, body, .. } => {
            put(out, indent, "switch");
            put(out, indent + 1, "cond");
            dump_expr(out, cond, indent + 2);
            put(out, indent + 1, "body");
            dump_stmt(out, body, indent + 2);
        }
        Stmt::Case { expr, stmt, .. } => {
            put(out, indent, "case");
            dump_expr(out, expr, indent + 1);
            dump_stmt(out, stmt, indent + 1);
        }
        Stmt::Default { stmt, .. } => {
            put(out, indent, "default");
            dump_stmt(out, stmt, indent + 1);
        }
        Stmt::Expr { expr, .. } => {
            put(out, indent, "expr");
            if let Some(expr) = expr {
                dump_expr(out, expr, indent + 1);
            }
        }
        Stmt::Break { .. } => put(out, indent, "break"),
        Stmt::Continue { .. } => put(out, indent, "continue"),
        Stmt::Label { name, stmt, .. } => {
            put(out, indent, &format!("label {name}"));
            dump_stmt(out, stmt, indent + 1);
        }
        Stmt::Goto { name, .. } => put(out, indent, &format!("goto {name}")),
    }
}

fn dump_expr(out: &mut String, expr: &Expr, indent: usize) {
    match expr {
        Expr::Int { bits, value_ty, .. } => put(
            out,
            indent,
            &format!("{} {}", value_ty.as_str(), value_ty.format_bits(*bits)),
        ),
        Expr::Float { bits, kind, .. } => put(
            out,
            indent,
            &format!(
                "{} {}",
                kind.as_str(),
                crate::float::llvm_const(*kind, *bits)
            ),
        ),
        Expr::Var { name, .. } => put(out, indent, &format!("var {name}")),
        Expr::Bin {
            op, left, right, ..
        } => {
            put(out, indent, op.as_str());
            dump_expr(out, left, indent + 1);
            dump_expr(out, right, indent + 1);
        }
        Expr::Unary { op, expr, .. } => {
            let spelling = match op {
                UnaryOp::Plus => "pos",
                UnaryOp::Minus => "neg",
                UnaryOp::Not => "not",
                UnaryOp::BitNot => "bnot",
                UnaryOp::Addr => "addr",
                UnaryOp::Deref => "deref",
            };
            put(out, indent, spelling);
            dump_expr(out, expr, indent + 1);
        }
        Expr::Assign { target, value, .. } => {
            put(out, indent, "assign");
            dump_expr(out, target, indent + 1);
            dump_expr(out, value, indent + 1);
        }
        Expr::CompoundAssign {
            op, target, value, ..
        } => {
            put(out, indent, op.compound_spelling());
            dump_expr(out, target, indent + 1);
            dump_expr(out, value, indent + 1);
        }
        Expr::IncDec { op, target, .. } => {
            let spelling = match op {
                IncOp::PreInc => "pre++",
                IncOp::PreDec => "pre--",
                IncOp::PostInc => "post++",
                IncOp::PostDec => "post--",
            };
            put(out, indent, spelling);
            dump_expr(out, target, indent + 1);
        }
        Expr::Cond {
            cond,
            then_expr,
            else_expr,
            ..
        } => {
            put(out, indent, "?:");
            dump_expr(out, cond, indent + 1);
            dump_expr(out, then_expr, indent + 1);
            dump_expr(out, else_expr, indent + 1);
        }
        Expr::Comma { left, right, .. } => {
            put(out, indent, ",");
            dump_expr(out, left, indent + 1);
            dump_expr(out, right, indent + 1);
        }
        Expr::Sizeof { operand, .. } => match operand {
            SizeofOperand::Type(ty) => put(out, indent, &format!("sizeof {}", ty.display())),
            SizeofOperand::Expr(expr) => {
                put(out, indent, "sizeof");
                dump_expr(out, expr, indent + 1);
            }
        },
        Expr::Cast { ty_name, expr, .. } => {
            put(out, indent, &format!("cast {}", ty_name.display()));
            dump_expr(out, expr, indent + 1);
        }
        Expr::Index { base, index, .. } => {
            put(out, indent, "index");
            dump_expr(out, base, indent + 1);
            dump_expr(out, index, indent + 1);
        }
        Expr::Decay { expr, .. } => {
            put(out, indent, "decay");
            dump_expr(out, expr, indent + 1);
        }
        Expr::Str { bytes, .. } => put(out, indent, &dump_string(bytes)),
        Expr::Member {
            base, field, arrow, ..
        } => {
            let kind = if *arrow { "arrow" } else { "member" };
            put(out, indent, &format!("{kind} {field}"));
            dump_expr(out, base, indent + 1);
        }
        Expr::VaArg { ap, .. } => {
            put(out, indent, "va_arg");
            dump_expr(out, ap, indent + 1);
        }
        Expr::Generic {
            control, assocs, ..
        } => {
            put(out, indent, "generic");
            dump_expr(out, control, indent + 1);
            for assoc in assocs {
                let label = match &assoc.ty {
                    Some(ty) => format!("assoc {}", ty.display()),
                    None => "default".to_string(),
                };
                put(out, indent + 1, &label);
                dump_expr(out, &assoc.expr, indent + 2);
            }
        }
        Expr::CompoundLit { decl, .. } => {
            put(
                out,
                indent,
                &format!("compound literal {}", decl.ty.display()),
            );
            if let Some(init) = &decl.init {
                dump_init(out, init, indent + 1);
            }
        }
        Expr::VaOp { op, args, .. } => {
            let name = match op {
                VaOp::Start => "va_start",
                VaOp::End => "va_end",
                VaOp::Copy => "va_copy",
            };
            put(out, indent, name);
            for arg in args {
                dump_expr(out, arg, indent + 1);
            }
        }
        Expr::Call { callee, args, .. } => {
            match callee.as_ref() {
                Expr::Var { name, .. } => put(out, indent, &format!("call {name}")),
                other => {
                    put(out, indent, "call");
                    dump_expr(out, other, indent + 1);
                }
            }
            for arg in args {
                dump_expr(out, arg, indent + 1);
            }
        }
    }
}

fn dump_string(bytes: &[u8]) -> String {
    let mut text = String::from("string \"");
    for &byte in bytes {
        match byte {
            b'\n' => text.push_str("\\n"),
            b'\t' => text.push_str("\\t"),
            b'\r' => text.push_str("\\r"),
            b'\\' => text.push_str("\\\\"),
            b'"' => text.push_str("\\\""),
            0x20..=0x7e => text.push(byte as char),
            _ => text.push_str(&format!("\\x{byte:02x}")),
        }
    }
    text.push('"');
    text
}

fn dump_init(out: &mut String, init: &Init, indent: usize) {
    match init {
        Init::Expr(expr) => dump_expr(out, expr, indent),
        Init::List { items, .. } => {
            put(out, indent, "list");
            for item in items {
                dump_init(out, item, indent + 1);
            }
        }
        Init::Designated {
            designators, init, ..
        } => {
            let text = designators
                .iter()
                .map(|designator| match designator {
                    Designator::Field { name, .. } => format!(".{name}"),
                    Designator::Index { .. } => "[index]".to_string(),
                })
                .collect::<String>();
            put(out, indent, &format!("designated {text}"));
            dump_init(out, init, indent + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_v_record_layout() {
        // char, int: pad to 4, trailing pad to 8.
        let (off, size, align) = layout_record(&[(1, 1), (4, 4)]).unwrap();
        assert_eq!((off.as_slice(), size, align), (&[0, 4][..], 8, 4));
        // int, char: trailing padding.
        let (off, size, align) = layout_record(&[(4, 4), (1, 1)]).unwrap();
        assert_eq!((off.as_slice(), size, align), (&[0, 4][..], 8, 4));
        // char, long.
        let (off, size, align) = layout_record(&[(1, 1), (8, 8)]).unwrap();
        assert_eq!((off.as_slice(), size, align), (&[0, 8][..], 16, 8));
        // char, short.
        let (off, size, align) = layout_record(&[(1, 1), (2, 2)]).unwrap();
        assert_eq!((off.as_slice(), size, align), (&[0, 2][..], 4, 2));
        // long[2] as a field is aligned to 8, not 16, even though it is 16 bytes.
        let (off, size, align) = layout_record(&[(1, 1), (16, 8)]).unwrap();
        assert_eq!((off.as_slice(), size, align), (&[0, 8][..], 24, 8));
        // char[16] as a field stays alignment 1.
        let (off, size, align) = layout_record(&[(1, 1), (16, 1)]).unwrap();
        assert_eq!((off.as_slice(), size, align), (&[0, 1][..], 17, 1));
        // nested struct of align 4 after a char.
        let (off, size, align) = layout_record(&[(1, 1), (8, 4)]).unwrap();
        assert_eq!((off.as_slice(), size, align), (&[0, 4][..], 12, 4));
    }

    fn bits(size: u64, align: u32, width: u32, named: bool) -> LayoutMember {
        LayoutMember {
            size,
            align,
            bit_width: Some(width),
            named,
        }
    }

    fn plain(size: u64, align: u32) -> LayoutMember {
        LayoutMember {
            size,
            align,
            bit_width: None,
            named: true,
        }
    }

    fn laid(members: &[LayoutMember]) -> (u64, u32, Vec<(u64, u32)>) {
        let (places, size, align) = layout_members(members, false).unwrap();
        let bits = places
            .iter()
            .map(|place| (place.offset, place.bit_offset))
            .collect();
        (size, align, bits)
    }

    #[test]
    fn bitfield_layout_matches_the_psabi() {
        // int a:3; int b:5. Both live in the low byte. Alignment comes from int.
        assert_eq!(
            laid(&[bits(4, 4, 3, true), bits(4, 4, 5, true)]),
            (4, 4, vec![(0, 0), (0, 3)])
        );
        // char a:4; int b:4. The int fits in the same 4-byte unit.
        assert_eq!(
            laid(&[bits(1, 1, 4, true), bits(4, 4, 4, true)]),
            (4, 4, vec![(0, 0), (0, 4)])
        );
        // int a:3; char b. The char starts at the next byte.
        assert_eq!(
            laid(&[bits(4, 4, 3, true), plain(1, 1)]),
            (4, 4, vec![(0, 0), (1, 0)])
        );
        // char a; int b:5. The bit-field continues inside the int unit at bit 0.
        assert_eq!(
            laid(&[plain(1, 1), bits(4, 4, 5, true)]),
            (4, 4, vec![(0, 0), (0, 8)])
        );
        // int a:20; int b:20. The second does not fit and starts a new int.
        assert_eq!(
            laid(&[bits(4, 4, 20, true), bits(4, 4, 20, true)]),
            (8, 4, vec![(0, 0), (4, 32)])
        );
        // int a:3; int :0; int b:3.
        assert_eq!(
            laid(&[
                bits(4, 4, 3, true),
                bits(4, 4, 0, false),
                bits(4, 4, 3, true)
            ]),
            (8, 4, vec![(0, 0), (4, 32), (4, 32)])
        );
        // char a:4; char b:5. The second char starts a new byte.
        assert_eq!(
            laid(&[bits(1, 1, 4, true), bits(1, 1, 5, true)]),
            (2, 1, vec![(0, 0), (1, 8)])
        );
        // int a:17; char b:3. The char starts at bit 17.
        assert_eq!(
            laid(&[bits(4, 4, 17, true), bits(1, 1, 3, true)]),
            (4, 4, vec![(0, 0), (2, 17)])
        );
        // char a:3; int :0; char b. The zero-width field aligns, but does not
        // raise the struct alignment, so the size stays 5.
        assert_eq!(
            laid(&[bits(1, 1, 3, true), bits(4, 4, 0, false), plain(1, 1)]),
            (5, 1, vec![(0, 0), (4, 32), (4, 0)])
        );
        // int :3; char a. Unnamed, so the alignment stays 1 and the char is
        // in the next byte.
        assert_eq!(
            laid(&[bits(4, 4, 3, false), plain(1, 1)]),
            (2, 1, vec![(0, 0), (1, 0)])
        );
        // int :3 alone is one byte. The unnamed field does not align the struct.
        assert_eq!(laid(&[bits(4, 4, 3, false)]), (1, 1, vec![(0, 0)]));
        // int :0 alone reserves nothing.
        assert_eq!(laid(&[bits(4, 4, 0, false)]), (0, 1, vec![(0, 0)]));
        // char a:5; int b:30. 30 bits do not fit in the remaining int unit.
        assert_eq!(
            laid(&[bits(1, 1, 5, true), bits(4, 4, 30, true)]),
            (8, 4, vec![(0, 0), (4, 32)])
        );
        // int a:3; long b:3. The long fits in the 8-byte unit that starts at 0.
        assert_eq!(
            laid(&[bits(4, 4, 3, true), bits(8, 8, 3, true)]),
            (8, 8, vec![(0, 0), (0, 3)])
        );
    }

    #[test]
    fn struct_identity_is_the_declaration() {
        let a = StructRef::new_incomplete(1, "S", RecordKind::Struct);
        let b = StructRef::new_incomplete(2, "S", RecordKind::Struct);
        assert_ne!(a, b);
        assert_eq!(a.clone(), a);
        assert!(!a.is_complete());
        a.complete(
            vec![StructField::plain("x", CType::int_type(), 0)],
            4,
            4,
            "struct.S".to_string(),
        );
        assert!(a.is_complete());
        assert_eq!(a.size(), Some(4));
        assert_eq!(a.field("x").unwrap().0, 0);
        assert!(a.field("y").is_none());
        assert_eq!(CType::Struct(a.clone()).display(), "struct S");
        assert_eq!(CType::Struct(a).ir_type(), "%struct.S");
    }
}
