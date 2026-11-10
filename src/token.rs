//! Tokens for a restricted C lexer.
//!
//! Keywords from ISO C11 are recognized so unsupported ones can be rejected
//! with a specific diagnostic instead of being treated as identifiers.

use crate::ast::{FloatKind, TypeName};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Eof,
    Ident,
    Int,
    /// A floating constant. The low bits are on [`Token::value`], the high
    /// 16 bits of a `long double` are on [`Token::float_hi`], and the type
    /// is [`Token::float_kind`].
    Float,
    /// A decoded string literal. The bytes live on [`Token::bytes`] and do
    /// not include the implicit terminating NUL.
    String,

    KwAuto,
    KwBreak,
    KwCase,
    KwChar,
    KwConst,
    KwContinue,
    KwDefault,
    KwDo,
    KwDouble,
    KwElse,
    KwEnum,
    KwExtern,
    KwFloat,
    KwFor,
    KwGoto,
    KwIf,
    KwInline,
    KwInt,
    KwLong,
    KwRegister,
    KwRestrict,
    KwReturn,
    KwShort,
    KwSigned,
    KwSizeof,
    KwStatic,
    KwStruct,
    KwSwitch,
    KwTypedef,
    KwUnion,
    KwUnsigned,
    KwVoid,
    KwVolatile,
    KwWhile,
    KwAlignas,
    KwAlignof,
    KwAtomic,
    KwBool,
    KwComplex,
    KwGeneric,
    KwImaginary,
    KwNoreturn,
    KwStaticAssert,
    KwThreadLocal,

    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Assign,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    AndAnd,
    OrOr,
    Bang,
    Amp,
    Pipe,
    Caret,
    Tilde,
    Shl,
    Shr,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
    AmpEq,
    PipeEq,
    CaretEq,
    ShlEq,
    ShrEq,
    PlusPlus,
    MinusMinus,
    Arrow,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBrack,
    RBrack,
    Semi,
    Comma,
    Question,
    Colon,
    Dot,
    Ellipsis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub spelling: String,
    pub line: usize,
    pub column: usize,
    /// Source file this token came from. The preprocessor sets this for
    /// tokens that originated in a `#include`d file.
    pub filename: String,
    /// Present for [`TokenKind::Int`] and [`TokenKind::Float`]. The low bits
    /// of an integer, a character constant, or a floating constant. The
    /// integer type is [`Token::int_ty`]. A `long double` also stores its
    /// high 16 bits in [`Token::float_hi`].
    pub value: Option<u64>,
    /// Bits 64..79 of a `long double` constant. Zero for every other token.
    pub float_hi: u16,
    /// Type chosen for an integer constant (C11 6.4.4.1) or `int` for a
    /// character constant (C11 6.4.4.4).
    pub int_ty: Option<TypeName>,
    /// `float` or `double` for a [`TokenKind::Float`]. The bits in
    /// [`Token::value`] are `f32` bits in the low 32 for `float`, and `f64`
    /// bits for `double`.
    pub float_kind: Option<FloatKind>,
    /// Decoded bytes of a [`TokenKind::String`], without the implicit NUL.
    pub bytes: Option<Vec<u8>>,
}

pub fn keyword(spelling: &str) -> Option<TokenKind> {
    Some(match spelling {
        "auto" => TokenKind::KwAuto,
        "break" => TokenKind::KwBreak,
        "case" => TokenKind::KwCase,
        "char" => TokenKind::KwChar,
        "const" => TokenKind::KwConst,
        "continue" => TokenKind::KwContinue,
        "default" => TokenKind::KwDefault,
        "do" => TokenKind::KwDo,
        "double" => TokenKind::KwDouble,
        "else" => TokenKind::KwElse,
        "enum" => TokenKind::KwEnum,
        "extern" => TokenKind::KwExtern,
        "float" => TokenKind::KwFloat,
        "for" => TokenKind::KwFor,
        "goto" => TokenKind::KwGoto,
        "if" => TokenKind::KwIf,
        "inline" => TokenKind::KwInline,
        "int" => TokenKind::KwInt,
        "long" => TokenKind::KwLong,
        "register" => TokenKind::KwRegister,
        "restrict" => TokenKind::KwRestrict,
        "return" => TokenKind::KwReturn,
        "short" => TokenKind::KwShort,
        "signed" => TokenKind::KwSigned,
        "sizeof" => TokenKind::KwSizeof,
        "static" => TokenKind::KwStatic,
        "struct" => TokenKind::KwStruct,
        "switch" => TokenKind::KwSwitch,
        "typedef" => TokenKind::KwTypedef,
        "union" => TokenKind::KwUnion,
        "unsigned" => TokenKind::KwUnsigned,
        "void" => TokenKind::KwVoid,
        "volatile" => TokenKind::KwVolatile,
        "while" => TokenKind::KwWhile,
        "_Alignas" => TokenKind::KwAlignas,
        "_Alignof" => TokenKind::KwAlignof,
        "_Atomic" => TokenKind::KwAtomic,
        "_Bool" => TokenKind::KwBool,
        "_Complex" => TokenKind::KwComplex,
        "_Generic" => TokenKind::KwGeneric,
        "_Imaginary" => TokenKind::KwImaginary,
        "_Noreturn" => TokenKind::KwNoreturn,
        "_Static_assert" => TokenKind::KwStaticAssert,
        "_Thread_local" => TokenKind::KwThreadLocal,
        _ => return None,
    })
}
