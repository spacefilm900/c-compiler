//! Abstract syntax for the C subset accepted by this milestone.
//!
//! Names are resolved later. Local `slot` fields are filled before semantic
//! analysis so each variable has a stable LLVM alloca name. File-scope objects
//! use the global's LLVM name instead of an alloca.

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
    Type(TypeName),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarRes {
    pub slot: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallRes {
    pub name: String,
    pub ret: TypeName,
    pub params: Vec<TypeName>,
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
        ty: Option<TypeName>,
    },
    Var {
        name: String,
        line: usize,
        column: usize,
        resolved: Option<VarRes>,
        ty: Option<TypeName>,
    },
    Bin {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<TypeName>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<TypeName>,
    },
    Assign {
        target: Box<Expr>,
        value: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<TypeName>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        line: usize,
        column: usize,
        resolved: Option<CallRes>,
        ty: Option<TypeName>,
    },
    /// `E1 op= E2`. `target` is the lvalue, evaluated once.
    CompoundAssign {
        op: BinOp,
        target: Box<Expr>,
        value: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<TypeName>,
    },
    IncDec {
        op: IncOp,
        target: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<TypeName>,
    },
    Cond {
        cond: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<TypeName>,
    },
    Comma {
        left: Box<Expr>,
        right: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<TypeName>,
    },
    Sizeof {
        operand: SizeofOperand,
        line: usize,
        column: usize,
        ty: Option<TypeName>,
    },
    /// `(type)expr`, including `(void)expr`.
    Cast {
        ty_name: TypeName,
        expr: Box<Expr>,
        line: usize,
        column: usize,
        ty: Option<TypeName>,
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

    pub fn cast(ty_name: TypeName, expr: Expr, line: usize, column: usize) -> Self {
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

    pub fn sizeof_type(ty_name: TypeName, line: usize, column: usize) -> Self {
        Expr::Sizeof {
            operand: SizeofOperand::Type(ty_name),
            line,
            column,
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
            | Expr::Cast { line, .. } => *line,
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
            | Expr::Cast { column, .. } => *column,
        }
    }

    pub fn ty(&self) -> Option<TypeName> {
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
            | Expr::Cast { ty, .. } => *ty,
        }
    }

    pub fn set_ty(&mut self, ty: TypeName) {
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
            | Expr::Cast { ty: slot, .. } => *slot = Some(ty),
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
    pub ty: TypeName,
    pub init: Option<Expr>,
    pub line: usize,
    pub column: usize,
    pub slot: String,
}

#[derive(Debug, Clone)]
pub struct Param {
    /// `None` for an unnamed prototype parameter (`int f(int, int)`).
    /// A function definition's parameters always have names.
    pub name: Option<String>,
    pub ty: TypeName,
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
    pub ret_type: TypeName,
    pub name: String,
    pub params: Vec<Param>,
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
    pub ty: TypeName,
    pub storage: Storage,
    pub init: Option<Expr>,
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
    pub ty: TypeName,
    pub linkage: Linkage,
    /// `None` is an external declaration. `Some` is a definition, including
    /// a tentative definition completed as zero. The value is the low
    /// [`TypeName::width`] bits.
    pub init: Option<u64>,
}

/// A function that was declared and not defined. Emitted as `declare`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrFunc {
    pub name: String,
    pub ret: TypeName,
    pub params: Vec<TypeName>,
    pub linkage: Linkage,
}

#[derive(Debug, Clone)]
pub struct TranslationUnit {
    pub decls: Vec<TopDecl>,
    /// Filled by semantic analysis, in order of first declaration.
    pub globals: Vec<IrGlobal>,
    /// Prototypes with no definition in this translation unit.
    pub func_decls: Vec<IrFunc>,
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
    let ty = global.ty.as_str();
    if let Some(init) = &global.init {
        put(
            out,
            indent,
            &format!("{prefix}global {ty} {} =", global.name),
        );
        dump_expr(out, init, indent + 1);
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
            func.ret_type.as_str(),
            func.name
        ),
    );
    for param in &func.params {
        match &param.name {
            Some(name) => put(
                out,
                indent + 1,
                &format!("param {} {name}", param.ty.as_str()),
            ),
            None => put(out, indent + 1, &format!("param {}", param.ty.as_str())),
        }
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
            let ty = decl.ty.as_str();
            if let Some(init) = &decl.init {
                put(out, indent, &format!("decl {ty} {} =", decl.name));
                dump_expr(out, init, indent + 1);
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
                                &format!("decl {} {} =", decl.ty.as_str(), decl.name),
                            );
                            dump_expr(out, init, indent + 3);
                        } else {
                            put(
                                out,
                                indent + 2,
                                &format!("decl {} {}", decl.ty.as_str(), decl.name),
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
            SizeofOperand::Type(ty) => put(out, indent, &format!("sizeof {}", ty.as_str())),
            SizeofOperand::Expr(expr) => {
                put(out, indent, "sizeof");
                dump_expr(out, expr, indent + 1);
            }
        },
        Expr::Cast { ty_name, expr, .. } => {
            put(out, indent, &format!("cast {}", ty_name.as_str()));
            dump_expr(out, expr, indent + 1);
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
