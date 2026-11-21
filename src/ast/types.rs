//! C types: basic type names, qualifiers, function types, floating kinds,
//! string encodings and `CType`, and integer bit helpers.

use super::*;

/// Integer and `void` types.
///
/// `char` is a distinct type from `signed char` and `unsigned char`; its
/// signedness is the target's. Under LP64 (every target but Windows) `long`
/// and `long long` are both 64 bits, and `long long` still has the greater
/// conversion rank (C11 6.3.1.1). Under LLP64 (Windows) `long` is 32 bits.
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
    /// GNU 128-bit integers (`__int128`, `unsigned __int128`, `__int128_t`,
    /// `__uint128_t`). Size and alignment are 16. Rank is above `long long`.
    Int128,
    UInt128,
    /// `_Bool`. The object representation is one byte, and every stored value
    /// is 0 or 1. Rank is below `char`. Conversion to `_Bool` compares with 0
    /// rather than truncating (C11 6.3.1.2).
    Bool,
}

impl TypeName {
    /// `size_t` on the current target.
    pub fn size_t() -> TypeName {
        crate::target::current().size_type()
    }

    /// `ptrdiff_t` (and `intptr_t`) on the current target.
    pub fn ptrdiff_t() -> TypeName {
        crate::target::current().ptrdiff_type()
    }

    /// The 64-bit integer type this compiler uses for 64-bit arithmetic
    /// of its own (addresses, wide containers): `long` under LP64, `long
    /// long` under LLP64, where `long` is 32 bits.
    pub fn int64(signed: bool) -> TypeName {
        match (crate::target::current().long_is_64(), signed) {
            (true, true) => TypeName::Long,
            (true, false) => TypeName::ULong,
            (false, true) => TypeName::LongLong,
            (false, false) => TypeName::ULongLong,
        }
    }

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
            TypeName::Int128 => "__int128",
            TypeName::UInt128 => "unsigned __int128",
            TypeName::Bool => "_Bool",
        }
    }

    pub fn is_void(self) -> bool {
        self == TypeName::Void
    }

    pub fn is_integer(self) -> bool {
        !self.is_void()
    }

    /// Plain `char` is signed on x86_64 and Apple arm64, and unsigned on
    /// AArch64 Linux ([`crate::target::Target::char_is_signed`]).
    pub fn is_signed(self) -> bool {
        if self == TypeName::Char {
            return crate::target::current().char_is_signed();
        }
        matches!(
            self,
            TypeName::SChar
                | TypeName::Short
                | TypeName::Int
                | TypeName::Long
                | TypeName::LongLong
                | TypeName::Int128
        )
    }

    pub fn width(self) -> u32 {
        match self {
            TypeName::Void => 0,
            // Stored in a byte. The value is still only 0 or 1.
            TypeName::Bool | TypeName::Char | TypeName::SChar | TypeName::UChar => 8,
            TypeName::Short | TypeName::UShort => 16,
            TypeName::Int | TypeName::UInt => 32,
            TypeName::Long | TypeName::ULong => {
                if crate::target::current().long_is_64() {
                    64
                } else {
                    32
                }
            }
            TypeName::LongLong | TypeName::ULongLong => 64,
            TypeName::Int128 | TypeName::UInt128 => 128,
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
            128 => "i128",
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
            TypeName::Int128 | TypeName::UInt128 => 6,
        }
    }

    pub fn mask(self) -> u128 {
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
            TypeName::Int128 => TypeName::UInt128,
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
    ///
    /// Apple arm64 has the same rule. AAPCS64 itself (AArch64 Linux) leaves
    /// the upper bits unspecified, so nothing is added there.
    pub fn abi_attr(self) -> Option<&'static str> {
        if !crate::target::current().extends_narrow_args() {
            // The Microsoft x64 convention leaves the upper bits of narrow
            // integers unspecified, but a `_Bool` is still passed as a
            // zero-extended byte (Clang does, observed as a black box).
            if self == TypeName::Bool && crate::target::current().is_windows() {
                return Some("zeroext");
            }
            return None;
        }
        match self {
            TypeName::Char if !self.is_signed() => Some("zeroext"),
            TypeName::Char | TypeName::SChar | TypeName::Short => Some("signext"),
            TypeName::Bool | TypeName::UChar | TypeName::UShort => Some("zeroext"),
            _ => None,
        }
    }

    /// Decimal spelling of `bits` interpreted as this type.
    pub fn format_bits(self, bits: u128) -> String {
        if self.is_signed() {
            sign_extend(bits, self.width()).to_string()
        } else {
            (bits & self.mask()).to_string()
        }
    }
}

/// `const`, `volatile`, `restrict`, and `_Atomic` on one type (C11 6.7.3).
///
/// The same qualifier twice in one specifier or qualifier list is a
/// diagnostic. Qualifiers from different lists combine: `const int * const`.
/// `restrict` is part of the type for compatibility and assignment. It does
/// not change the IR. It is only meaningful on a pointer; the parser rejects
/// it on any other type. `_Atomic` shall not modify an array or function
/// type (C11 6.7.3p3). The size and alignment of an atomic type need not
/// match the unqualified type (C11 6.2.5); this compiler uses the same
/// size and alignment as the non-atomic type for lock-free widths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Quals {
    pub is_const: bool,
    pub is_volatile: bool,
    pub is_restrict: bool,
    pub is_atomic: bool,
}

impl Quals {
    pub fn is_empty(self) -> bool {
        !self.is_const && !self.is_volatile && !self.is_restrict && !self.is_atomic
    }

    /// `self` has every qualifier `other` has.
    pub fn includes(self, other: Self) -> bool {
        (!other.is_const || self.is_const)
            && (!other.is_volatile || self.is_volatile)
            && (!other.is_restrict || self.is_restrict)
            && (!other.is_atomic || self.is_atomic)
    }

    pub fn merge(self, other: Self) -> Self {
        Self {
            is_const: self.is_const || other.is_const,
            is_volatile: self.is_volatile || other.is_volatile,
            is_restrict: self.is_restrict || other.is_restrict,
            is_atomic: self.is_atomic || other.is_atomic,
        }
    }
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
            FloatKind::LongDouble => match crate::target::current().long_double() {
                LongDouble::Double => 8,
                LongDouble::X87 | LongDouble::Quad => 16,
            },
        }
    }

    pub fn align(self) -> u32 {
        self.size() as u32
    }

    pub fn ir_type(self) -> &'static str {
        match self {
            FloatKind::Float => "float",
            FloatKind::Double => "double",
            FloatKind::LongDouble => match crate::target::current().long_double() {
                LongDouble::X87 => "x86_fp80",
                LongDouble::Double => "double",
                LongDouble::Quad => "fp128",
            },
        }
    }

    /// The kind whose format this kind uses on the current target: `long
    /// double` is computed as `double` where the two share a format (Apple
    /// arm64). Constant folding and IR conversions use this, while the C
    /// type system still keeps `long double` distinct from `double`.
    pub fn repr(self) -> FloatKind {
        if self == FloatKind::LongDouble
            && crate::target::current().long_double() == LongDouble::Double
        {
            FloatKind::Double
        } else {
            self
        }
    }

    /// `long double` is the x87 extended format on this target.
    pub fn is_x87(self) -> bool {
        self == FloatKind::LongDouble && crate::target::current().long_double() == LongDouble::X87
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
            StrEnc::Wchar => crate::target::current().wchar_bits() as usize / 8,
            StrEnc::Utf32 => 4,
        }
    }

    /// Element type of the array. `char16_t` and `char32_t` are the target's
    /// unsigned 16-bit and 32-bit types. `wchar_t` is `int`, `unsigned
    /// int` where the target makes it unsigned (AArch64 Linux), or `unsigned
    /// short` on Windows.
    pub fn elem(self) -> TypeName {
        match self {
            StrEnc::Byte => TypeName::Char,
            StrEnc::Utf16 => TypeName::UShort,
            StrEnc::Wchar => crate::target::current().wchar_type(),
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
/// Length of an array type (C11 6.7.6.2).
///
/// `int a[]` is incomplete. `int a[10]` has a known constant size. A
/// variable length array has a runtime size, identified by a [`VlaId`]
/// assigned when the size expression is seen. `int a[*]` is a prototype
/// scope VLA of unspecified size, and is nonetheless a complete type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayLen {
    Incomplete,
    Const(u64),
    Vla(VlaId),
    Star,
}

impl ArrayLen {
    pub fn from_const(len: Option<u64>) -> Self {
        match len {
            Some(n) => ArrayLen::Const(n),
            None => ArrayLen::Incomplete,
        }
    }

    pub fn const_len(self) -> Option<u64> {
        match self {
            ArrayLen::Const(n) => Some(n),
            _ => None,
        }
    }

    /// A size that is not an integer constant expression (C11 6.7.6.2p4).
    pub fn is_variable(self) -> bool {
        matches!(self, ArrayLen::Vla(_) | ArrayLen::Star)
    }
}

/// Identifies one evaluation of a VLA size expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VlaId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CType {
    Void,
    Int(TypeName),
    Float(FloatKind),
    Pointer(Box<CType>),
    Array(Box<CType>, ArrayLen),
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
        CType::array_of(elem, ArrayLen::from_const(len))
    }

    pub fn array_of(elem: CType, len: ArrayLen) -> Self {
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

    pub fn is_atomic(&self) -> bool {
        self.quals().is_atomic
    }

    /// Sizes this compiler lowers with lock-free LLVM atomics, on every
    /// supported target (x86-64, AArch64, and 64-bit Windows).
    pub fn is_lock_free_atomic_size(size: u64) -> bool {
        matches!(size, 1 | 2 | 4 | 8 | 16)
    }

    /// Whether an atomic type of this shape can be lowered. Non-atomic
    /// types are not asked this question.
    pub fn atomic_is_lock_free(&self) -> bool {
        self.size().is_some_and(CType::is_lock_free_atomic_size)
    }

    /// Drop `_Atomic` from the type itself, keeping other qualifiers.
    /// Lvalue conversion also drops `const` / `volatile` / `restrict`
    /// (C11 6.3.2.1p2); that is [`CType::unqualified`].
    pub fn non_atomic(&self) -> CType {
        match self {
            CType::Qualified(quals, inner) if quals.is_atomic => {
                let quals = Quals {
                    is_atomic: false,
                    ..*quals
                };
                if quals.is_empty() {
                    (**inner).clone()
                } else {
                    CType::Qualified(quals, inner.clone())
                }
            }
            other => other.clone(),
        }
    }

    /// Add qualifiers. An array passes them to its element type (C11 6.7.3p9).
    pub fn with_quals(self, extra: Quals) -> CType {
        if extra.is_empty() {
            return self;
        }
        match self {
            CType::Array(elem, len) => CType::array_of(elem.with_quals(extra), len),
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
                    handle.fields_ref().is_some_and(|fields| {
                        fields
                            .iter()
                            .any(|field| field.bit_width.is_none() && field.ty.flexible_carrier())
                    })
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
                .fields_ref()
                .is_some_and(|fields| fields.iter().any(|field| field.ty.contains_const())),
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
            CType::Array(_, len) => len.const_len(),
            _ => None,
        }
    }

    pub fn array_size(&self) -> Option<ArrayLen> {
        match self.peel() {
            CType::Array(_, len) => Some(*len),
            _ => None,
        }
    }

    /// A variable length array type (C11 6.7.6.2p4): the size is not an
    /// integer constant expression, or the element type does not have a
    /// known constant size.
    pub fn is_incomplete_array(&self) -> bool {
        matches!(self.peel(), CType::Array(_, ArrayLen::Incomplete))
    }

    pub fn is_vla(&self) -> bool {
        match self.peel() {
            CType::Array(_, ArrayLen::Vla(_) | ArrayLen::Star) => true,
            CType::Array(elem, ArrayLen::Const(_)) => elem.is_vla(),
            _ => false,
        }
    }

    /// A variably modified type (C11 6.7.6p3): a VLA type, or a type
    /// derived from one by pointer or array construction.
    pub fn is_variably_modified(&self) -> bool {
        match self.peel() {
            CType::Array(elem, len) => len.is_variable() || elem.is_variably_modified(),
            CType::Pointer(inner) => inner.is_variably_modified(),
            // Derived from the return type (C11 6.7.6p3). Parameter VLA
            // types do not make the function type itself variably modified.
            CType::Func(func) => func.ret.is_variably_modified(),
            _ => false,
        }
    }

    pub fn is_complete(&self) -> bool {
        match self.peel() {
            CType::Void | CType::Func(_) => false,
            CType::Int(_) | CType::Float(_) | CType::Pointer(_) => true,
            // A VLA, including `[*]`, is a complete type (C11 6.7.6.2p4).
            CType::Array(_, ArrayLen::Incomplete) => false,
            CType::Array(elem, _) => elem.is_complete(),
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
            CType::Array(elem, ArrayLen::Const(len)) => {
                elem.size().map(|elem| elem.saturating_mul(*len))
            }
            CType::Array(_, _) => None,
            CType::Struct(handle) => handle.size(),
            CType::Qualified(_, _) => unreachable!("peel removes qualifiers"),
        }
    }

    /// Alignment in bytes.
    ///
    /// The System V AMD64 ABI gives an array of 16 bytes or more an alignment
    /// of at least 16. Smaller arrays use the element alignment. AAPCS64 and
    /// Apple's arm64 rules have no such boost: an array is aligned as its
    /// element. Pointers are 8-byte aligned. `const` and `volatile` do not
    /// change the alignment. An `_Atomic` type is aligned to at least its
    /// size so a lock-free LLVM atomic access is well defined.
    pub fn align(&self) -> u32 {
        let align = match self.peel() {
            CType::Void | CType::Func(_) => 1,
            CType::Int(ty) => ty.align(),
            CType::Float(kind) => kind.align(),
            CType::Pointer(_) => 8,
            CType::Array(elem, _) => {
                let elem_align = elem.align();
                match self.size() {
                    Some(size) if size >= 16 && crate::target::current().boosts_large_arrays() => {
                        elem_align.max(16)
                    }
                    _ => elem_align,
                }
            }
            CType::Struct(handle) => handle.align(),
            CType::Qualified(_, _) => unreachable!("peel removes qualifiers"),
        };
        if self.is_atomic() {
            if let Some(size) = self.size() {
                if size > 0 && size <= u32::MAX as u64 {
                    return align.max(size as u32);
                }
            }
        }
        align
    }

    /// Alignment of this type when it is a field.
    ///
    /// An array field is aligned as its element is, with no 16-byte boost.
    /// A local or global array variable still uses [`CType::align`], which
    /// applies that boost. A struct field uses the struct's alignment.
    pub fn member_align(&self) -> u32 {
        match self.peel() {
            CType::Array(elem, _) => elem.member_align(),
            // Use `self.align()` so an `_Atomic` type keeps its size
            // alignment; `peel()` would drop that qualifier.
            _ => self.align(),
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
                let n = len.const_len().unwrap_or(0);
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

pub fn width_mask(width: u32) -> u128 {
    if width >= 128 {
        u128::MAX
    } else {
        (1u128 << width) - 1
    }
}

/// Sign-extend the low `width` bits of `bits` to `i128`.
pub fn sign_extend(bits: u128, width: u32) -> i128 {
    if width >= 128 {
        return bits as i128;
    }
    let bits = bits & width_mask(width);
    let shift = 128 - width;
    ((bits << shift) as i128) >> shift
}

/// Inclusive signed limits of an integer type of `width` bits.
pub fn signed_limits(width: u32) -> (i128, i128) {
    if width >= 128 {
        (i128::MIN, i128::MAX)
    } else {
        let min = -((1i128) << (width - 1));
        let max = ((1i128) << (width - 1)) - 1;
        (min, max)
    }
}

/// Whether `value << count` is representable in a signed integer of
/// `width` bits (C11 6.5.7). `value` must already be in range for that
/// type. A negative value, a count at or above `width`, or a product
/// that does not fit is not representable.
pub fn signed_shl_fits(value: i128, count: u32, width: u32) -> bool {
    if value < 0 || count >= width {
        return false;
    }
    let (_, max) = signed_limits(width);
    value <= (max >> count)
}

/// Convert `bits` from `from` to `to` (C11 6.3.1.3).
///
/// Unsigned destinations reduce the value modulo 2^width. A signed
/// destination that cannot represent the value is implementation-defined;
/// this target keeps the low bits and reads them as two's complement, which
/// is what Clang does on `x86_64-pc-linux-gnu`.
pub fn convert_bits(bits: u128, from: TypeName, to: TypeName) -> u128 {
    // C11 6.3.1.2: any nonzero value becomes 1. Truncation would turn 2 into 0.
    if to == TypeName::Bool && from != TypeName::Bool {
        let zero = if from.is_signed() {
            sign_extend(bits, from.width()) == 0
        } else {
            bits & from.mask() == 0
        };
        return u128::from(!zero);
    }
    let to_mask = to.mask();
    if to.width() <= from.width() {
        return bits & to_mask;
    }
    if from.is_signed() {
        (sign_extend(bits, from.width()) as u128) & to_mask
    } else {
        bits & from.mask()
    }
}
