//! Abstract syntax for the C subset accepted by this milestone.
//!
//! Names are resolved later. Local `slot` fields are filled before semantic
//! analysis so each variable has a stable LLVM alloca name. File-scope objects
//! use the global's LLVM name instead of an alloca.
//!
//! Types are integers, `void`, pointers, and arrays, with `const` and
//! `volatile`. A function type is not a value type: parameters and returns
//! use the object type that remains after the usual parameter adjustment.

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
            TypeName::Char | TypeName::SChar | TypeName::UChar => 8,
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
            TypeName::Void => 0,
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
            TypeName::Char
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
            TypeName::UChar | TypeName::UShort => Some("zeroext"),
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

/// `const` and `volatile` on one type (C11 6.7.3). `restrict` is not accepted.
///
/// The same qualifier twice in one specifier or qualifier list is a
/// diagnostic. Qualifiers from different lists combine: `const int * const`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Quals {
    pub is_const: bool,
    pub is_volatile: bool,
}

impl Quals {
    pub fn is_empty(self) -> bool {
        !self.is_const && !self.is_volatile
    }

    /// `self` has every qualifier `other` has.
    pub fn includes(self, other: Self) -> bool {
        (!other.is_const || self.is_const) && (!other.is_volatile || self.is_volatile)
    }

    pub fn merge(self, other: Self) -> Self {
        Self {
            is_const: self.is_const || other.is_const,
            is_volatile: self.is_volatile || other.is_volatile,
        }
    }
}

/// A type this milestone can name: `void`, an integer, a pointer, or an array,
/// each of which may be `const` or `volatile`.
///
/// `char` stays distinct from `signed char`. Pointers use one LLVM `ptr`
/// (opaque pointers). An array with `len == None` is incomplete (`int a[]`).
/// A pointer is complete even when it points at an incomplete type.
///
/// Qualifiers on an array apply to the element type, not the array type
/// (C11 6.7.3p9), so an array is never wrapped in [`CType::Qualified`].
/// `const int *` is a pointer to a qualified `int`. `int * const` is a
/// qualified pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CType {
    Void,
    Int(TypeName),
    Pointer(Box<CType>),
    Array(Box<CType>, Option<u64>),
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
    fn peel(&self) -> &CType {
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

    /// Integer or pointer. Arrays are not scalars; they decay first.
    pub fn is_scalar(&self) -> bool {
        self.is_integer() || self.is_pointer()
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
            CType::Void => false,
            CType::Int(_) | CType::Pointer(_) => true,
            CType::Array(elem, Some(_)) => elem.is_complete(),
            CType::Array(_, None) => false,
            CType::Qualified(_, _) => unreachable!("peel removes qualifiers"),
        }
    }

    /// Size in bytes. Incomplete types have no size.
    pub fn size(&self) -> Option<u64> {
        match self.peel() {
            CType::Void => None,
            CType::Int(ty) => Some(ty.size()),
            CType::Pointer(_) => Some(8),
            CType::Array(elem, Some(len)) => elem.size().map(|elem| elem.saturating_mul(*len)),
            CType::Array(_, None) => None,
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
            CType::Void => 1,
            CType::Int(ty) => ty.align(),
            CType::Pointer(_) => 8,
            CType::Array(elem, _) => {
                let elem_align = elem.align();
                match self.size() {
                    Some(size) if size >= 16 => elem_align.max(16),
                    _ => elem_align,
                }
            }
            CType::Qualified(_, _) => unreachable!("peel removes qualifiers"),
        }
    }

    pub fn ir_type(&self) -> String {
        match self.peel() {
            CType::Void => "void".to_string(),
            CType::Int(ty) => ty.ir_type().to_string(),
            CType::Pointer(_) => "ptr".to_string(),
            CType::Array(elem, len) => {
                let n = len.unwrap_or(0);
                format!("[{n} x {}]", elem.ir_type())
            }
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
        },
        CType::Void => join_spec("void", name),
        CType::Int(scalar) => join_spec(scalar.as_str(), name),
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

fn qual_prefix(quals: Quals) -> String {
    match (quals.is_const, quals.is_volatile) {
        (true, true) => "const volatile ".to_string(),
        (true, false) => "const ".to_string(),
        (false, true) => "volatile ".to_string(),
        (false, false) => String::new(),
    }
}

/// Text after `*`: empty, ` const`, ` volatile`, or ` const volatile`.
fn qual_after_star(quals: Quals) -> String {
    match (quals.is_const, quals.is_volatile) {
        (true, true) => " const volatile".to_string(),
        (true, false) => " const".to_string(),
        (false, true) => " volatile".to_string(),
        (false, false) => String::new(),
    }
}

fn star_decl(pointee: &CType, quals: Quals, name: &str) -> String {
    let q = qual_after_star(quals);
    let needs_paren = matches!(pointee.peel(), CType::Array(_, _));
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
        (CType::Pointer(a), CType::Pointer(b)) => compatible(a, b),
        (CType::Array(a, la), CType::Array(b, lb)) => {
            (la.is_none() || lb.is_none() || la == lb) && compatible(a, b)
        }
        _ => false,
    }
}

/// Composite type of two compatible types. A known array length wins.
pub fn composite(left: &CType, right: &CType) -> CType {
    let base = match (left.peel(), right.peel()) {
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

/// Storage-class specifier on a file-scope declaration.
///
/// At most one of `static` and `extern` is allowed. Absence is not the same
/// thing as `extern` for objects: a file-scope object with no storage class
/// has external linkage and is a tentative definition when it has no
/// initializer (C11 6.2.2, 6.9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Storage {
    #[default]
    None,
    Static,
    Extern,
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
    Ptr(PtrConst),
    /// `ptrtoint` of a pointer constant, then truncated to `width` bits.
    PtrToInt {
        ptr: PtrConst,
        width: u32,
    },
    Aggregate(Vec<ConstInit>),
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
    /// A string literal. `bytes` does not include the implicit NUL. The type
    /// is `char[N]` with `N == bytes.len() + 1`. `symbol` is the private
    /// global that holds the literal, filled in when the address is needed.
    Str {
        bytes: Vec<u8>,
        line: usize,
        column: usize,
        symbol: Option<String>,
        ty: Option<CType>,
    },
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

    pub fn str_lit(bytes: Vec<u8>, line: usize, column: usize) -> Self {
        Expr::Str {
            bytes,
            line,
            column,
            symbol: None,
            ty: None,
        }
    }

    pub fn line(&self) -> usize {
        match self {
            Expr::Int { line, .. }
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
            | Expr::Str { line, .. } => *line,
        }
    }

    pub fn column(&self) -> usize {
        match self {
            Expr::Int { column, .. }
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
            | Expr::Str { column, .. } => *column,
        }
    }

    pub fn ty(&self) -> Option<CType> {
        match self {
            Expr::Int { ty, .. }
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
            | Expr::Str { ty, .. } => ty.clone(),
        }
    }

    pub fn set_ty(&mut self, ty: CType) {
        match self {
            Expr::Int { ty: slot, .. }
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
            | Expr::Str { ty: slot, .. } => *slot = Some(ty),
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
    pub init: Option<Init>,
    /// Filled by semantic analysis when `init` is present.
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
    /// The parameter list ends with `...`. A definition of a variadic function
    /// is rejected; only a declaration is kept, so another translation unit
    /// (the C library) can provide it.
    pub variadic: bool,
    /// `None` is a prototype. `Some` is a definition.
    pub body: Option<Compound>,
    pub storage: Storage,
    /// Filled in by semantic analysis. The parser sets [`Linkage::External`].
    pub linkage: Linkage,
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
    pub bytes: Vec<u8>,
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
        Storage::None => "",
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

fn dump_compound(out: &mut String, compound: &Compound, indent: usize) {
    put(out, indent, "block");
    for item in &compound.items {
        dump_stmt(out, item, indent + 1);
    }
}

fn dump_stmt(out: &mut String, stmt: &Stmt, indent: usize) {
    match stmt {
        Stmt::Compound(compound) => dump_compound(out, compound, indent),
        Stmt::Decl(decl) => {
            let ty = decl.ty.display();
            if let Some(init) = &decl.init {
                put(out, indent, &format!("decl {ty} {} =", decl.name));
                dump_init(out, init, indent + 1);
            } else {
                put(out, indent, &format!("decl {ty} {}", decl.name));
            }
        }
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
                        if let Some(init) = &decl.init {
                            put(
                                out,
                                indent + 2,
                                &format!("decl {} {} =", decl.ty.display(), decl.name),
                            );
                            dump_init(out, init, indent + 3);
                        } else {
                            put(
                                out,
                                indent + 2,
                                &format!("decl {} {}", decl.ty.display(), decl.name),
                            );
                        }
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
        Stmt::Expr { expr, .. } => {
            put(out, indent, "expr");
            if let Some(expr) = expr {
                dump_expr(out, expr, indent + 1);
            }
        }
        Stmt::Break { .. } => put(out, indent, "break"),
        Stmt::Continue { .. } => put(out, indent, "continue"),
    }
}

fn dump_expr(out: &mut String, expr: &Expr, indent: usize) {
    match expr {
        Expr::Int { bits, value_ty, .. } => put(
            out,
            indent,
            &format!("{} {}", value_ty.as_str(), value_ty.format_bits(*bits)),
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
    }
}
