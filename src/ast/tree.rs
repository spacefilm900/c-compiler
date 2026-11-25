//! Syntax tree nodes: storage classes, operators, initializers, expressions,
//! statements, declarations and the translation unit.

use super::*;

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
        /// Expanded from a GNU range designator `[first ... last]`: the
        /// same `init` is repeated for every index, so sema rejects a value
        /// that reads a volatile object (GCC evaluates it once).
        ranged: bool,
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
    pub abs_bit: u64,
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
    /// The address of label `label` in the function whose linker name is
    /// `func` (GNU `&&label`), an LLVM `blockaddress`.
    Label {
        func: String,
        label: String,
    },
}

/// Constant initializer value. Integers keep the bit pattern of their type
/// (up to 128 bits). `Zero` is the all-zero value of whatever type it is attached to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstInit {
    Zero,
    Int(u128),
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
    /// The object representation of a union or bit-field record that also
    /// holds addresses. Each relocation is a byte offset and a `Ptr` (eight
    /// bytes) or `PtrToInt` (its width) value; the bytes under it are zero.
    /// Relocations are sorted by offset and do not overlap.
    RelocBytes {
        bytes: Vec<u8>,
        relocs: Vec<(u64, ConstInit)>,
    },
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
        /// Low bits of the value. A C integer constant fits in 64 bits; a
        /// 128-bit value arises from a conversion or an operation, and is
        /// stored in the constant folders as [`u128`], not here.
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
    /// A GNU statement expression `({ ... })` (GCC manual, "Statements and
    /// Declarations in Expressions"). Its value is that of the last item when
    /// that is an expression statement; otherwise it has type `void`.
    /// `value_ty` is the parser's view of that type, for `typeof` and
    /// `sizeof` before semantic analysis.
    StmtExpr {
        body: Box<Compound>,
        value_ty: Option<CType>,
        line: usize,
        column: usize,
        ty: Option<CType>,
    },
    /// The GNU conditional with the middle operand omitted, `cond ?: else`
    /// (GCC manual, "Conditionals with Omitted Operands"). `cond` is
    /// evaluated once; when it is nonzero it is also the result.
    CondShort {
        cond: Box<Expr>,
        else_expr: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<CType>,
    },
    /// The GNU unary `&&label`, the address of a label in the current
    /// function (GCC manual, "Labels as Values"). Its type is `void *`.
    /// Semantic analysis fills `func` with the linker name of the enclosing
    /// function, which an LLVM `blockaddress` constant names.
    LabelAddr {
        name: String,
        func: Option<String>,
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
            | Expr::CompoundLit { line, .. }
            | Expr::StmtExpr { line, .. }
            | Expr::CondShort { line, .. }
            | Expr::LabelAddr { line, .. } => *line,
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
            | Expr::CompoundLit { column, .. }
            | Expr::StmtExpr { column, .. }
            | Expr::CondShort { column, .. }
            | Expr::LabelAddr { column, .. } => *column,
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
            | Expr::CompoundLit { ty, .. }
            | Expr::StmtExpr { ty, .. }
            | Expr::CondShort { ty, .. }
            | Expr::LabelAddr { ty, .. } => ty.clone(),
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
            | Expr::CompoundLit { ty: slot, .. }
            | Expr::StmtExpr { ty: slot, .. }
            | Expr::CondShort { ty: slot, .. }
            | Expr::LabelAddr { ty: slot, .. } => *slot = Some(ty),
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
    /// Alignment from `_Alignas` or a GNU `aligned` attribute, when it is
    /// stricter than the type's own.
    pub align: Option<u32>,
    /// `_Alignas` on this declaration, if any (C11 6.7.5), for redeclarations.
    pub alignas: Option<u32>,
    /// `_Thread_local` (C11 6.7.1). Block-scope thread-local objects also
    /// have `static` or `extern` storage.
    pub thread_local: bool,
    /// First VLA id this declarator allocated, if any. Only those ids are
    /// re-evaluated when the declaration is reached (C11 6.7.6.2p5); a bound
    /// inherited from a typedef keeps the size recorded at the typedef.
    pub vla_from: Option<VlaId>,
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
    /// The unadjusted type used `[*]` (C11 6.7.6.2p4). After adjustment the
    /// outermost array is a pointer, so this records notation that would
    /// otherwise be lost. Only legal in a function prototype, not a definition.
    pub star_array: bool,
    /// Type before array/function adjustment, so VLA size expressions on a
    /// parameter (C11 6.7.6.3 / 6.9.1) are still evaluated on entry after
    /// the parameter itself has become a pointer.
    pub unadjusted: CType,
    /// Declared with `register`, so its address cannot be taken
    /// (C11 6.5.3.2p1).
    pub register: bool,
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
    /// type. Semantic analysis fills it in. `high` is the upper bound of a
    /// GNU case range `case low ... high:` (GCC manual, "Case Ranges"), and
    /// `high_value` its converted value.
    Case {
        expr: Expr,
        value: Option<u128>,
        high: Option<Expr>,
        high_value: Option<u128>,
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
    /// The GNU computed goto `goto * expression ;` (GCC manual, "Labels as
    /// Values"). The expression is a pointer, normally a label address.
    GotoPtr {
        expr: Expr,
        line: usize,
        column: usize,
    },
    /// A GNU `asm` statement without operands (GCC manual, "Basic Asm" and
    /// "Extended Asm"): `asm("nop");` or `asm volatile("pause" ::: "memory");`.
    /// `template` is already in LLVM inline-asm form (LLVM LangRef, "Inline
    /// Assembler Expressions"): `$` doubled, and for extended asm `%%`
    /// turned into `%`. `clobbers` are the clobber strings as written.
    Asm {
        template: Vec<u8>,
        clobbers: Vec<String>,
        line: usize,
        column: usize,
    },
}

impl Stmt {
    /// Line and column where the statement starts.
    pub fn pos(&self) -> (usize, usize) {
        match self {
            Stmt::Compound(compound) => (compound.line, compound.column),
            Stmt::Decl(decl) => (decl.line, decl.column),
            Stmt::FuncDecl(func) => (func.line, func.column),
            Stmt::Return { line, column, .. }
            | Stmt::If { line, column, .. }
            | Stmt::While { line, column, .. }
            | Stmt::For { line, column, .. }
            | Stmt::Do { line, column, .. }
            | Stmt::Switch { line, column, .. }
            | Stmt::Case { line, column, .. }
            | Stmt::Default { line, column, .. }
            | Stmt::Expr { line, column, .. }
            | Stmt::Break { line, column }
            | Stmt::Continue { line, column }
            | Stmt::Label { line, column, .. }
            | Stmt::Goto { line, column, .. }
            | Stmt::GotoPtr { line, column, .. }
            | Stmt::Asm { line, column, .. } => (*line, *column),
        }
    }
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
    /// This is a definition whose declarator used an identifier list
    /// (C11 6.9.1). The LLVM signature uses default argument promotions.
    pub kr_def: bool,
    /// `None` is a declaration. `Some` is a definition.
    pub body: Option<Compound>,
    pub storage: Storage,
    /// Filled in by semantic analysis. The parser sets [`Linkage::External`].
    pub linkage: Linkage,
    /// Linker name. An `__asm__("...")` label replaces the C name. Calls and
    /// `declare` use this; lookup still uses [`FuncDef::name`].
    pub symbol: String,
    /// Labels whose address is taken with `&&` (GNU), in order of first
    /// use. A computed `goto` may reach any of them. Filled in by semantic
    /// analysis.
    pub addr_labels: Vec<String>,
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
    /// Combined `_Alignas` on this declaration, if any (C11 6.7.5).
    pub alignas: Option<u32>,
    /// `_Thread_local` (C11 6.7.1).
    pub thread_local: bool,
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
    /// GNU variable attributes that change the emitted global.
    pub attrs: VarAttrs,
}

/// GNU function attributes that change the IR (GCC manual, "Common
/// Function Attributes"). Every declaration of the function contributes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FnAttrs {
    pub noreturn: bool,
    pub always_inline: bool,
    pub noinline: bool,
    pub cold: bool,
    pub hot: bool,
    /// `const`: no memory is read or written.
    pub const_fn: bool,
    /// `pure`: memory is only read.
    pub pure: bool,
    pub weak: bool,
    pub used: bool,
    pub section: Option<String>,
    /// `deprecated`, with its optional message. A call warns.
    pub deprecated: Option<String>,
    /// Run before `main` (`constructor`) or after it returns
    /// (`destructor`).
    pub constructor: bool,
    pub destructor: bool,
    /// `gnu_inline`: an `extern inline` definition is for inlining only
    /// (GCC manual, "Common Function Attributes").
    pub gnu_inline: bool,
    /// The definition in this translation unit is not an external
    /// definition: a C11 inline definition (6.7.4p7) or a `gnu_inline`
    /// `extern inline` one. It is emitted `available_externally` (LLVM
    /// LangRef), so calls may inline it and otherwise call the external
    /// definition another translation unit or library provides. Set by the
    /// parser once the whole translation unit is read.
    pub available_externally: bool,
    /// MSVC: the definition was written `inline`, so it is emitted in a
    /// COMDAT with this linkage (`linkonce_odr` or `weak_odr`). Set by the
    /// parser for the MSVC target only.
    pub msvc_comdat: Option<&'static str>,
}

impl FnAttrs {
    pub fn merge(&mut self, other: &FnAttrs) {
        self.noreturn |= other.noreturn;
        self.always_inline |= other.always_inline;
        self.noinline |= other.noinline;
        self.cold |= other.cold;
        self.hot |= other.hot;
        self.const_fn |= other.const_fn;
        self.pure |= other.pure;
        self.weak |= other.weak;
        self.used |= other.used;
        if other.section.is_some() {
            self.section = other.section.clone();
        }
        if other.deprecated.is_some() {
            self.deprecated = other.deprecated.clone();
        }
        self.constructor |= other.constructor;
        self.destructor |= other.destructor;
        self.gnu_inline |= other.gnu_inline;
        self.available_externally |= other.available_externally;
        if other.msvc_comdat.is_some() {
            self.msvc_comdat = other.msvc_comdat;
        }
    }
}

/// GNU variable attributes that change the IR (GCC manual, "Common
/// Variable Attributes").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VarAttrs {
    /// `aligned(N)` or `_Alignas`: the object is at least this aligned.
    pub align: Option<u32>,
    pub section: Option<String>,
    pub weak: bool,
    pub used: bool,
    /// Microsoft `__declspec(selectany)`: the definition goes in a COMDAT
    /// (`weak_odr`), so every unit may define it and the linker keeps one.
    pub selectany: bool,
    /// LLVM `thread_local` (C11 `_Thread_local`).
    pub thread_local: bool,
}

impl VarAttrs {
    pub fn merge(&mut self, other: &VarAttrs) {
        if let Some(align) = other.align {
            self.align = Some(self.align.map_or(align, |old| old.max(align)));
        }
        if other.section.is_some() {
            self.section = other.section.clone();
        }
        self.weak |= other.weak;
        self.used |= other.used;
        self.selectany |= other.selectany;
        self.thread_local |= other.thread_local;
    }
}

impl IrGlobal {
    /// Low bits of an integer definition. `None` when the global is not an
    /// integer constant (a pointer, an aggregate, or an external declaration).
    pub fn int_init(&self) -> Option<u128> {
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
    /// GNU function attributes by linker symbol, merged over every
    /// declaration.
    pub fn_attrs: std::collections::HashMap<String, FnAttrs>,
    /// GNU attributes of file-scope objects by name, merged over every
    /// declaration.
    pub var_attrs: std::collections::HashMap<String, VarAttrs>,
    /// Warnings from parsing (unknown attributes, for example).
    pub warnings: Vec<crate::diag::Warning>,
    /// Size expressions for VLA types, in the order they were parsed.
    /// Each [`VlaId`] in a type refers to one of these.
    pub vla_sizes: Vec<VlaBound>,
}

/// One VLA size expression, evaluated once when the declarator, typedef,
/// or type name that created [`VlaBound::id`] is reached.
#[derive(Debug, Clone)]
pub struct VlaBound {
    pub id: VlaId,
    pub expr: Expr,
    /// Whether `expr` has integer type in the scope that introduced this
    /// bound. Recorded then so a later `_Alignof` is not affected by
    /// shadowing (C11 6.7.6.2p1, 6.5.3.4p3).
    pub integer: bool,
}
