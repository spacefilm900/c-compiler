//! Structure and union types: members, identity, completion and layout
//! (C11 6.7.2.1, System V AMD64 and AAPCS64 bit-field rules).

use super::*;

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
    pub(super) id: u32,
    pub(super) data: Rc<RefCell<StructBody>>,
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

pub(super) struct StructBody {
    pub(super) kind: RecordKind,
    pub(super) tag: String,
    pub(super) fields: Option<Vec<StructField>>,
    pub(super) size: u64,
    pub(super) align: u32,
    pub(super) llvm_name: String,
    /// See [`StructRef::set_irregular`].
    pub(super) irregular: bool,
    /// See [`StructRef::unadjusted_align`]; 0 means the same as `align`.
    pub(super) unadjusted_align: u32,
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
                irregular: false,
                unadjusted_align: 0,
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

    /// Identity of the shared record body, unique among live records even
    /// when ids are reused (as hand-built test types do).
    pub fn body_key(&self) -> usize {
        Rc::as_ptr(&self.data) as usize
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

    /// The members without copying them. The borrow must end before the
    /// record is completed.
    pub fn fields_ref(&self) -> Option<std::cell::Ref<'_, [StructField]>> {
        std::cell::Ref::filter_map(self.data.borrow(), |body| body.fields.as_deref()).ok()
    }

    pub fn size(&self) -> Option<u64> {
        let body = self.data.borrow();
        body.fields.as_ref().map(|_| body.size)
    }

    pub fn align(&self) -> u32 {
        self.data.borrow().align
    }

    /// The alignment without an `aligned` attribute written on the record
    /// itself (members' own alignments still count). AAPCS64 Linux uses it
    /// to pick registers and stack slots for an argument; observed with
    /// Clang as a black box, `struct { long a, b; } __attribute__((aligned
    /// (16)))` goes in any two registers there, but in an even pair on Apple.
    pub fn unadjusted_align(&self) -> u32 {
        let body = self.data.borrow();
        if body.unadjusted_align == 0 {
            body.align
        } else {
            body.unadjusted_align
        }
    }

    /// Raise the alignment of a completed record whose size is already a
    /// multiple of `align`.
    pub fn raise_align(&self, align: u32) {
        let mut body = self.data.borrow_mut();
        debug_assert!(body.size % u64::from(align.max(1)) == 0);
        body.align = body.align.max(align);
    }

    pub fn set_unadjusted_align(&self, align: u32) {
        self.data.borrow_mut().unadjusted_align = align.max(1);
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

    pub(super) fn lookup_member_from(
        &self,
        name: &str,
        base: u64,
        quals: Quals,
        indirect: bool,
    ) -> Option<MemberHit> {
        if name.is_empty() {
            return None;
        }
        let fields = self.fields_ref()?;
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
    /// The record has a bit-field, or a GNU `packed` or `aligned` layout.
    /// Either way its LLVM type is a byte array and members are reached by
    /// byte offset.
    pub fn has_bitfield(&self) -> bool {
        self.data.borrow().irregular
            || self
                .fields_ref()
                .is_some_and(|fields| fields.iter().any(StructField::is_bitfield))
    }

    /// Mark a layout that `packed` or `aligned` moved away from the natural
    /// one.
    pub fn set_irregular(&self) {
        self.data.borrow_mut().irregular = true;
    }

    /// The last member is a flexible array (C11 6.7.2.1). `sizeof` excludes it.
    pub fn has_flexible_member(&self) -> bool {
        self.fields_ref()
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    if crate::target::current().ms_layout() {
        return if union_layout {
            ms_layout_union_members(members)
        } else {
            ms_layout_members(members)
        };
    }
    if union_layout {
        return layout_union_members(members);
    }
    let mut places = Vec::with_capacity(members.len());
    let mut bit_pos: u64 = 0;
    let mut align = 1u32;
    let unnamed_aligns = crate::target::current().unnamed_bitfield_aligns();
    for member in members {
        let field_align = member.align.max(1);
        if let Some(width) = member.bit_width {
            let unit_bits = member.size.max(1).saturating_mul(8);
            let align_bits = u64::from(field_align).saturating_mul(8);
            if unnamed_aligns && !member.named {
                align = align.max(field_align);
            }
            if width == 0 {
                bit_pos = align_up(bit_pos, align_bits)?;
                places.push(LayoutPlace {
                    offset: bit_pos / 8,
                    bit_offset: u32::try_from(bit_pos).ok()?,
                    bit_width: Some(0),
                });
            } else {
                let width = u64::from(width);
                let within = bit_pos % unit_bits;
                if within + width > unit_bits {
                    bit_pos = align_up(bit_pos, align_bits)?;
                }
                let unit_start = bit_pos - (bit_pos % unit_bits);
                // A bit-field's position is kept in 32 bits, so a record with
                // a bit-field at or past 512 MiB is reported as too large
                // (an implementation limit, C11 5.2.4.1) rather than
                // truncated.
                places.push(LayoutPlace {
                    offset: unit_start / 8,
                    bit_offset: u32::try_from(bit_pos).ok()?,
                    bit_width: Some(width as u32),
                });
                bit_pos = bit_pos.checked_add(width)?;
                if member.named {
                    align = align.max(field_align);
                }
            }
        } else {
            let byte = align_up(bit_pos.checked_add(7)? / 8, u64::from(field_align))?;
            places.push(LayoutPlace {
                offset: byte,
                bit_offset: 0,
                bit_width: None,
            });
            bit_pos = byte.checked_add(member.size)?.checked_mul(8)?;
            align = align.max(field_align);
        }
    }
    let bytes = bit_pos.checked_add(7)? / 8;
    let size = align_up(bytes, u64::from(align))?;
    Some((places, size, align))
}

/// An open run of bit-fields in the Microsoft layout: the storage unit's
/// first bit, its size in bytes, and the bits used so far.
struct MsUnit {
    start: u64,
    size: u64,
    used: u64,
}

/// The Microsoft record layout used on Windows, as the GCC manual
/// describes it for `-mms-bitfields` (x86 Options), the default for both
/// MSVC and MinGW-w64 targets:
///
/// - Every member, including an unnamed bit-field, has the alignment of
///   its declared type (already reduced by any packing), and the record's
///   alignment is the largest of them.
/// - Adjacent bit-fields share one storage unit of their declared type
///   only when the declared types have the same size and the next field
///   fits in the bits the unit has left. Otherwise the next bit-field opens
///   a new unit at that type's alignment; a field never straddles units.
/// - A zero-width bit-field after a bit-field closes the run, and the next
///   member is aligned to at least the zero-width field's type, which also
///   joins the record's alignment. A zero-width bit-field after any other
///   member is ignored.
pub(super) fn ms_layout_members(members: &[LayoutMember]) -> Option<(Vec<LayoutPlace>, u64, u32)> {
    let mut places = Vec::with_capacity(members.len());
    // The first free byte after everything placed and every closed unit.
    let mut next_byte: u64 = 0;
    let mut align = 1u32;
    let mut unit: Option<MsUnit> = None;
    let mut last_was_bitfield = false;
    // Alignment a zero-width bit-field imposes on the next member.
    let mut pending = 1u64;
    let close = |unit: &mut Option<MsUnit>, next_byte: &mut u64| -> Option<()> {
        if let Some(open) = unit.take() {
            *next_byte = (*next_byte).max(open.start / 8 + open.size);
        }
        Some(())
    };
    for member in members {
        let field_align = member.align.max(1);
        match member.bit_width {
            Some(0) => {
                if last_was_bitfield {
                    close(&mut unit, &mut next_byte)?;
                    pending = pending.max(u64::from(field_align));
                    align = align.max(field_align);
                }
                places.push(LayoutPlace {
                    offset: next_byte,
                    bit_offset: u32::try_from(next_byte.checked_mul(8)?).ok()?,
                    bit_width: Some(0),
                });
                // A second zero-width field is not after a bit-field.
                last_was_bitfield = false;
                continue;
            }
            Some(width) => {
                let width = u64::from(width);
                let size = member.size.max(1);
                let fits = unit
                    .as_ref()
                    .is_some_and(|open| open.size == size && open.used + width <= size * 8);
                if !fits {
                    close(&mut unit, &mut next_byte)?;
                    let at = align_up(next_byte, u64::from(field_align).max(pending))?;
                    pending = 1;
                    unit = Some(MsUnit {
                        start: at.checked_mul(8)?,
                        size,
                        used: 0,
                    });
                }
                let open = unit.as_mut()?;
                let bit = open.start.checked_add(open.used)?;
                places.push(LayoutPlace {
                    offset: open.start / 8,
                    bit_offset: u32::try_from(bit).ok()?,
                    bit_width: Some(width as u32),
                });
                open.used += width;
                align = align.max(field_align);
                last_was_bitfield = true;
            }
            None => {
                close(&mut unit, &mut next_byte)?;
                let at = align_up(next_byte, u64::from(field_align).max(pending))?;
                pending = 1;
                places.push(LayoutPlace {
                    offset: at,
                    bit_offset: 0,
                    bit_width: None,
                });
                next_byte = at.checked_add(member.size)?;
                align = align.max(field_align);
                last_was_bitfield = false;
            }
        }
    }
    close(&mut unit, &mut next_byte)?;
    let size = align_up(next_byte, u64::from(align))?;
    Some((places, size, align))
}

/// A union in the Microsoft layout: every member at offset 0, the size the
/// largest member (a bit-field counts as its declared type, a zero-width
/// one as nothing). Bit-fields do not raise the union's alignment; only
/// ordinary members do (observed with Clang as a black box for both
/// Windows targets).
pub(super) fn ms_layout_union_members(
    members: &[LayoutMember],
) -> Option<(Vec<LayoutPlace>, u64, u32)> {
    let mut places = Vec::with_capacity(members.len());
    let mut align = 1u32;
    let mut size = 0u64;
    for member in members {
        places.push(LayoutPlace {
            offset: 0,
            bit_offset: 0,
            bit_width: member.bit_width,
        });
        match member.bit_width {
            Some(0) => {}
            Some(_) => size = size.max(member.size.max(1)),
            None => {
                size = size.max(member.size);
                align = align.max(member.align.max(1));
            }
        }
    }
    let size = align_up(size, u64::from(align))?;
    Some((places, size, align))
}

pub(super) fn layout_union_members(
    members: &[LayoutMember],
) -> Option<(Vec<LayoutPlace>, u64, u32)> {
    let mut places = Vec::with_capacity(members.len());
    let mut align = 1u32;
    let mut size = 0u64;
    let unnamed_aligns = crate::target::current().unnamed_bitfield_aligns();
    for member in members {
        let field_align = member.align.max(1);
        if let Some(width) = member.bit_width {
            if unnamed_aligns && !member.named {
                align = align.max(field_align);
            }
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

pub(super) fn layout_fields(
    fields: &[(u64, u32)],
    union_layout: bool,
) -> Option<(Vec<u64>, u64, u32)> {
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
