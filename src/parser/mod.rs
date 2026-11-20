//! Recursive-descent parser for a C subset.
//!
//! Expression precedence follows the C standard. From loosest to tightest:
//! comma, assignment, `?:`, `||`, `&&`, `|`, `^`, `&`, equality, relational,
//! shifts, additive, multiplicative, then unary and postfix. Assignment and
//! `?:` are right associative. A dangling `else` binds to the nearest `if`.

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diag::{Diagnostic, Warning};
use crate::dialect::Dialect;
use crate::token::{Token, TokenKind};

mod attrs;
mod decl;
mod declarator;
mod expr;
mod ice;
mod record;
mod specs;
mod stmt;
mod uneval;
use declarator::*;
use expr::*;
use ice::*;
use uneval::*;

pub fn parse(tokens: &[Token], filename: &str) -> Result<TranslationUnit, Diagnostic> {
    parse_dialect(tokens, filename, Dialect::C11)
}

/// Like [`parse`], in the given dialect. The dialect selects the GNU
/// syntax extensions the parser accepts (statement expressions, case
/// ranges, computed `goto` and `&&label`, `?:` with an omitted operand,
/// empty `{}` initializers, zero-length arrays) as well as the constraints
/// checked in folded integer constant expressions.
pub fn parse_dialect(
    tokens: &[Token],
    filename: &str,
    dialect: Dialect,
) -> Result<TranslationUnit, Diagnostic> {
    parse_with(tokens, filename, dialect, false)
}

/// Like [`parse_dialect`]; `gnu89_inline` selects GNU89 `inline`
/// semantics (`-fgnu89-inline`), see [`Parser::mark_inline_definitions`].
pub fn parse_with(
    tokens: &[Token],
    filename: &str,
    dialect: Dialect,
    gnu89_inline: bool,
) -> Result<TranslationUnit, Diagnostic> {
    Parser {
        gnu89_inline,
        tokens,
        dialect,
        index: 0,
        filename,
        tags: Vec::new(),
        names: Vec::new(),
        structs: Vec::new(),
        llvm_names: HashSet::new(),
        next_struct: 0,
        defining: HashSet::new(),
        shadow_decl: None,
        va_list: None,
        float16: None,
        attr_sink: Vec::new(),
        warnings: Vec::new(),
        fn_attrs: HashMap::new(),
        inline_seen: HashMap::new(),
        fn_labels: HashMap::new(),
        var_attrs: HashMap::new(),
        stmt_expr_depth: 0,
    }
    .parse_translation_unit()
}

struct Parser<'a> {
    tokens: &'a [Token],
    /// `-fgnu89-inline`: every function has `gnu_inline` semantics.
    gnu89_inline: bool,
    dialect: Dialect,
    index: usize,
    filename: &'a str,
    /// Tag namespace, innermost scope last. Ordinary identifiers are not here.
    /// Structs, unions, and enums share it (C11 6.2.3).
    tags: Vec<HashMap<String, TagEntry>>,
    /// Ordinary identifiers that change how the next token is parsed: a
    /// typedef name starts a declaration, an enumeration constant is an
    /// integer constant. Innermost scope last.
    names: Vec<HashMap<String, NameBinding>>,
    /// Completed structs and unions, in definition order.
    structs: Vec<StructRef>,
    llvm_names: HashSet<String>,
    next_struct: u32,
    /// Records whose member lists are being parsed. A nested definition of
    /// one of these tags is a redefinition, not a completion.
    defining: HashSet<u32>,
    /// The `[1 x struct __va_list_tag]` type, created the first time
    /// `__builtin_va_list` is named.
    va_list: Option<CType>,
    /// See [`Parser::float16`].
    float16: Option<CType>,
    /// `union tag` seen while an outer `struct tag` (or the reverse) is
    /// visible. A lone `union tag;` hides the outer tag. Using it as a type
    /// in the same declaration is an error.
    shadow_decl: Option<(String, RecordKind)>,
    /// GNU attributes parsed since the enclosing specifier list or
    /// declarator started collecting them.
    attr_sink: Vec<Attr>,
    /// Warnings, such as an unknown attribute.
    warnings: Vec<Warning>,
    fn_attrs: HashMap<String, FnAttrs>,
    /// How each function's file-scope declarations use `inline`, by C name.
    inline_seen: HashMap<String, InlineSeen>,
    /// File-scope function `__asm__` labels, by C name.
    fn_labels: HashMap<String, String>,
    var_attrs: HashMap<String, VarAttrs>,
    /// Statement expressions being parsed, innermost last.
    stmt_expr_depth: usize,
}

/// One GNU attribute, `name` or `name(args)` inside `__attribute__((...))`
/// (GCC manual, "Attribute Syntax"). The name has any surrounding `__`
/// removed, so `__packed__` is `packed`.
#[derive(Clone, Debug)]
struct Attr {
    name: String,
    args: Vec<AttrArg>,
    line: usize,
    column: usize,
}

#[derive(Clone, Debug)]
enum AttrArg {
    Ident(String),
    Str(String),
    Expr(Expr),
    /// An argument this compiler does not interpret.
    Other,
}

/// What an attribute does here.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AttrClass {
    /// Has a meaning that is implemented, or is a hint that is safe to drop.
    Known,
    /// Would change the meaning of the program and is not implemented.
    Unsupported,
    /// Not an attribute this compiler knows. It is ignored with a warning.
    Unknown,
}

/// Attributes of one member of a struct or union.
#[derive(Clone, Copy, Default)]
struct MemberAttrs {
    align: Option<u32>,
    packed: bool,
}

/// One tag in the shared struct/union/enum namespace.
#[derive(Clone)]
enum TagEntry {
    Record(StructRef),
    /// A completed enumeration and its type: `int`, or, for the GNU
    /// extension that allows enumerators outside `int`, the wider type
    /// chosen in [`Parser::parse_enumerators`].
    Enum(TypeName),
    /// An enumeration without a fixed underlying type whose list is being
    /// read: incomplete until its `}` (C11 6.7.2.2p4).
    EnumOpen,
}

/// A name the parser has to know before semantic analysis.
#[derive(Clone)]
enum NameBinding {
    Typedef(CType),
    /// Value and type of an enumeration constant. The type is `int` unless
    /// the GNU extension for enumerators outside `int` applies.
    EnumConst(i128, TypeName),
    /// An ordinary identifier in this scope. It hides an outer typedef or
    /// enumeration constant. The declared type is kept so `sizeof` of an
    /// object can be folded where an integer constant expression is needed
    /// (an array bound, an enumeration value, a bit-field width).
    Ordinary(Option<CType>),
}

impl Parser<'_> {
    /// The names in scope and the dialect, for folding constants.
    fn env(&self) -> Env<'_> {
        Env {
            names: &self.names,
            dialect: self.dialect,
        }
    }

    fn parse_translation_unit(&mut self) -> Result<TranslationUnit, Diagnostic> {
        self.push_tags();
        let mut decls = Vec::new();
        while self.current().kind != TokenKind::Eof {
            decls.extend(self.parse_external()?);
        }
        self.pop_tags();
        self.mark_inline_definitions();
        Ok(TranslationUnit {
            decls,
            globals: Vec::new(),
            func_decls: Vec::new(),
            strings: Vec::new(),
            structs: std::mem::take(&mut self.structs),
            fn_attrs: std::mem::take(&mut self.fn_attrs),
            var_attrs: std::mem::take(&mut self.var_attrs),
            warnings: std::mem::take(&mut self.warnings),
        })
    }

    /// A file-scope declaration or function definition.
    ///
    /// `int f(void);` is a prototype. `int f(void) { ... }` is a definition.
    /// `int g;`, `int *p = &g;`, `int a[3] = {1};`, `extern int g;`, and
    /// `static int g;` are object declarations. A definition cannot share its
    /// declarator list with another declarator. Pointer stars bind to each
    /// declarator, so `int *a, b;` declares one pointer and one `int`.
    fn parse_external(&mut self) -> Result<Vec<TopDecl>, Diagnostic> {
        // Empty declaration at file scope. C11 6.7 does not allow a lone
        // `;` here; Clang and GCC accept it (with -Wextra-semi). SQLite's
        // shell writes `SQLITE_EXTENSION_INIT1;`, which expands to this.
        if self.current().kind == TokenKind::Semi {
            self.bump();
            return Ok(Vec::new());
        }
        if self.current().kind == TokenKind::KwStaticAssert {
            self.parse_static_assert()?;
            return Ok(Vec::new());
        }
        let specs = self.parse_specs(SpecPlace::Declaration)?;
        self.apply_shadow(&specs)?;
        if self.current().kind == TokenKind::Semi {
            self.finish_tag_only(&specs)?;
            self.expect_skip(TokenKind::Semi, "';'")?;
            return Ok(Vec::new());
        }
        let base = specs.ty.clone();
        let first = self.parse_named_decl(base.clone())?;
        if specs.storage == Storage::Typedef {
            return self.finish_typedefs(specs, first);
        }
        if self.is_function_definition(&first) {
            let ty = decl_kind_type(&first.kind);
            self.bind_ordinary(&first.name, Some(ty), first.line, first.column)?;
            return Ok(vec![self.finish_definition(specs, first)?]);
        }
        let mut decls = vec![self.finish_file_decl(&specs, first)?];
        while self.match_kind(TokenKind::Comma) {
            let next = self.parse_named_decl(base.clone())?;
            if next.is_function() && self.current().kind == TokenKind::LBrace {
                return Err(self.error("a function definition cannot follow another declarator"));
            }
            decls.push(self.finish_file_decl(&specs, next)?);
        }
        self.expect_skip(TokenKind::Semi, "';'")?;
        Ok(decls)
    }

    fn lookup_name(&self, name: &str) -> Option<NameBinding> {
        for scope in self.names.iter().rev() {
            if let Some(found) = scope.get(name) {
                return Some(found.clone());
            }
        }
        None
    }

    fn is_typedef(&self, name: &str) -> bool {
        matches!(self.lookup_name(name), Some(NameBinding::Typedef(_)))
    }

    /// A function declarator followed by `{`, or by a K&R declaration list
    /// and then `{` (C11 6.9.1).
    fn is_function_definition(&self, decl: &NamedDecl) -> bool {
        if !decl.is_function() {
            return false;
        }
        if self.current().kind == TokenKind::LBrace {
            return true;
        }
        matches!(
            &decl.kind,
            DeclKind::Function {
                prototype: false,
                params,
                ..
            } if !params.is_empty()
        ) && self.starts_declaration()
    }

    fn starts_declaration(&self) -> bool {
        if self.at_type_specifier(self.current()) {
            return true;
        }
        matches!(
            self.current().kind,
            TokenKind::KwStatic
                | TokenKind::KwExtern
                | TokenKind::KwTypedef
                | TokenKind::KwInline
                | TokenKind::KwRegister
                | TokenKind::KwAuto
                | TokenKind::KwNoreturn
                | TokenKind::KwAlignas
                | TokenKind::KwThreadLocal
        ) || self.spec_word(&[
            "__extension__",
            "__inline",
            "__inline__",
            "__attribute__",
            "__attribute",
        ])
    }

    fn at_type_specifier(&self, token: &Token) -> bool {
        is_type_specifier(token.kind)
            || matches!(
                token.kind,
                TokenKind::KwConst
                    | TokenKind::KwVolatile
                    | TokenKind::KwRestrict
                    | TokenKind::KwStruct
                    | TokenKind::KwUnion
                    | TokenKind::KwEnum
                    | TokenKind::KwBool
                    | TokenKind::KwFloat
                    | TokenKind::KwDouble
            )
            || (token.kind == TokenKind::Ident
                && (self.is_typedef(&token.spelling)
                    || token.spelling == "__builtin_va_list"
                    || token.spelling == "_Float16"
                    || matches!(
                        token.spelling.as_str(),
                        "__int128" | "__int128_t" | "__uint128_t"
                    )))
            || self.is_typeof_word(token)
            || matches!(token.spelling.as_str(), "__restrict" | "__restrict__")
                && token.kind == TokenKind::Ident
    }

    fn current(&self) -> &Token {
        &self.tokens[self.index.min(self.tokens.len().saturating_sub(1))]
    }

    fn lookahead(&self) -> TokenKind {
        self.tokens
            .get(self.index + 1)
            .map(|token| token.kind)
            .unwrap_or(TokenKind::Eof)
    }

    fn advance(&mut self) -> Token {
        let token = self.current().clone();
        self.bump();
        token
    }

    /// Like [`Parser::advance`] without copying the token.
    fn bump(&mut self) {
        if self.current().kind != TokenKind::Eof && self.index + 1 < self.tokens.len() {
            self.index += 1;
        }
    }

    fn match_kind(&mut self, kind: TokenKind) -> bool {
        if self.current().kind == kind {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: TokenKind, what: &str) -> Result<Token, Diagnostic> {
        if self.current().kind != kind {
            return Err(self.error(format!("expected {what}, found {}", found(self.current()))));
        }
        Ok(self.advance())
    }

    /// Like [`Parser::expect`] without copying the token.
    fn expect_skip(&mut self, kind: TokenKind, what: &str) -> Result<(), Diagnostic> {
        if self.current().kind != kind {
            return Err(self.error(format!("expected {what}, found {}", found(self.current()))));
        }
        self.bump();
        Ok(())
    }

    fn error(&self, message: impl Into<String>) -> Diagnostic {
        let token = self.current();
        self.error_at(token.line, token.column, message)
    }

    fn error_at(&self, line: usize, column: usize, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(self.file_at(line, column), line, column, message)
    }

    /// The token that introduced this location. Included files share line
    /// numbers with the main file, so the nearest token at or before the
    /// cursor wins.
    fn file_at(&self, line: usize, column: usize) -> &str {
        let end = (self.index + 1).min(self.tokens.len());
        for token in self.tokens[..end].iter().rev() {
            if token.line == line && token.column == column {
                return &token.filename;
            }
        }
        self.filename
    }
}

/// Encoding prefix of one string-literal token, taken from its spelling.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LitPrefix {
    None,
    U8,
    U,
    UU,
    L,
}

impl LitPrefix {
    fn unit(self) -> usize {
        match self {
            LitPrefix::None | LitPrefix::U8 => 1,
            LitPrefix::U => 2,
            LitPrefix::L => crate::target::current().wchar_bits() as usize / 8,
            LitPrefix::UU => 4,
        }
    }

    fn enc(self) -> StrEnc {
        match self {
            LitPrefix::None | LitPrefix::U8 => StrEnc::Byte,
            LitPrefix::U => StrEnc::Utf16,
            LitPrefix::L => StrEnc::Wchar,
            LitPrefix::UU => StrEnc::Utf32,
        }
    }
}

/// Re-encode `bytes` from `from` into `dest`. A narrower literal is
/// zero-extended one source byte at a time. Same-width pieces are copied.
fn retarget(from: LitPrefix, bytes: &[u8], dest: LitPrefix) -> Vec<u8> {
    if from.unit() == dest.unit() {
        return bytes.to_vec();
    }
    let mut out = Vec::with_capacity(bytes.len().saturating_mul(dest.unit()));
    for byte in bytes {
        out.extend(dest.enc().widen_byte(*byte));
    }
    out
}

/// See [`Parser::note_inline`].
#[derive(Debug, Default)]
struct InlineSeen {
    /// Some file-scope declaration has `inline`.
    inline_: bool,
    /// Some file-scope declaration lacks `inline` or has `extern`.
    external: bool,
    /// The definition is declared `extern inline`.
    extern_inline_def: bool,
    /// The linker names the declarations used.
    symbols: Vec<String>,
    /// The linker name the definition is emitted under, if there is one.
    def_symbol: Option<String>,
    /// The definition itself has `inline`.
    def_inline: bool,
}

struct Specs {
    /// The `inline` function specifier.
    inline_: bool,
    /// The first function specifier (`inline` or `_Noreturn`) and where it
    /// was. C11 6.7.4p2 allows one only in a function's declaration.
    func_spec: Option<(&'static str, usize, usize)>,
    storage: Storage,
    storage_at: Option<(usize, usize)>,
    ty: CType,
    /// Tag to redeclare in the current scope when this specifier is a lone
    /// `struct tag;` and the visible tag belongs to an outer scope.
    forward_tag: Option<String>,
    /// An enum was defined here, so `enum { A };` needs no declarator.
    introduced_tag: bool,
    /// The type specifier was an untagged struct or union definition, so a
    /// member declaration with no declarator is an anonymous member.
    anon_record: bool,
    /// The type specifier was a struct or union definition (`struct S {
    /// ... }`), as opposed to a reference to a record declared elsewhere.
    defined_record: bool,
    /// See [`Parser::shadow_decl`].
    shadow_decl: Option<(String, RecordKind)>,
    line: usize,
    column: usize,
    /// GNU attributes among the specifiers. They apply to every declarator.
    attrs: Vec<Attr>,
    /// Combined `_Alignas` requirement, after dropping a specifier of 0
    /// (C11 6.7.5p6). `None` means no alignment specifier was given.
    alignas: Option<u32>,
    alignas_at: Option<(usize, usize)>,
    /// `_Thread_local` (C11 6.7.1), which may appear with `static` or `extern`.
    thread_local: bool,
    thread_local_at: Option<(usize, usize)>,
    /// `register`, which forbids `_Alignas` (C11 6.7.5p2).
    register: bool,
    /// `auto` or `register` was written. K&R parameter declarations
    /// may use `register` but not `auto` (C11 6.9.1p5).
    plain_storage: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SpecPlace {
    Declaration,
    Parameter,
    TypeName,
    Member,
}

#[derive(Default)]
struct SpecFlags {
    storage: Storage,
    storage_at: Option<(usize, usize)>,
    /// `auto` or `register`, which do not change linkage.
    plain_storage: bool,
    void: bool,
    char_: bool,
    short: u8,
    int_: bool,
    long: u8,
    signed: bool,
    unsigned: bool,
    bool_ty: bool,
    /// GNU `__int128` / `__int128_t` / `__uint128_t`.
    int128: bool,
    float_ty: Option<FloatKind>,
    quals: Quals,
    struct_ty: Option<CType>,
    forward_tag: Option<String>,
    /// `enum { ... }` or `enum Tag { ... }` with no declarator is a declaration.
    introduced_tag: bool,
    anon_record: bool,
    defined_record: bool,
    any: bool,
    /// The `inline` function specifier.
    inline_: bool,
    /// See [`Specs::func_spec`].
    func_spec: Option<(&'static str, usize, usize)>,
    /// An enum specifier was read; it is the type specifier (C11 6.7.2p2),
    /// and the enumerated type is compatible with `int` here.
    enum_ty: bool,
    /// The enumerated type when it is not `int` (GNU extension).
    enum_base: Option<TypeName>,
    line: usize,
    column: usize,
    /// Combined `_Alignas` (0 is dropped).
    alignas: Option<u32>,
    alignas_at: Option<(usize, usize)>,
    thread_local: bool,
    thread_local_at: Option<(usize, usize)>,
    register: bool,
}

impl SpecFlags {
    fn has_scalar(&self) -> bool {
        self.void
            || self.char_
            || self.short != 0
            || self.int_
            || self.long != 0
            || self.signed
            || self.unsigned
            || self.int128
    }

    /// Whether the list already has a type specifier, so a typedef name or
    /// another type word cannot start one.
    fn has_type_specifier(&self) -> bool {
        self.has_tagged_type() || self.bool_ty || self.float_ty.is_some() || self.has_scalar()
    }

    /// A struct, union, enum, typedef name or other complete type specifier
    /// that no other type word may join.
    fn has_tagged_type(&self) -> bool {
        self.struct_ty.is_some() || self.enum_ty
    }

    fn note_func_spec(&mut self, name: &'static str, token: &Token) {
        if self.func_spec.is_none() {
            self.func_spec = Some((name, token.line, token.column));
        }
    }
}

struct NamedDecl {
    name: String,
    line: usize,
    column: usize,
    kind: DeclKind,
    /// Tags declared in this function's parameter list. Empty for an object.
    /// A definition installs them in the function's block scope; a prototype
    /// drops them when the declarator ends (C11 6.2.1).
    param_tags: Vec<(String, TagEntry)>,
    /// Enumeration constants and parameter names from that same list.
    param_names: Vec<(String, NameBinding)>,
    /// `__asm__("name")`, if the declarator had one.
    symbol: Option<String>,
    /// GNU attributes in and after the declarator.
    attrs: Vec<Attr>,
}

impl NamedDecl {
    fn is_function(&self) -> bool {
        matches!(self.kind, DeclKind::Function { .. })
    }
}

enum DeclKind {
    Object(CType),
    Function {
        ret: CType,
        params: Vec<Param>,
        prototype: bool,
        variadic: bool,
    },
}

enum Layer {
    Object(CType),
    Function {
        ret: CType,
        params: Vec<Param>,
        prototype: bool,
        variadic: bool,
        introduced: HashMap<String, TagEntry>,
        introduced_names: HashMap<String, NameBinding>,
    },
}

enum RawDecl {
    Named(String, usize, usize),
    Abstract(usize, usize),
    Pointer(Quals, Box<RawDecl>),
    Array(Box<RawDecl>, Option<u64>),
    Func(Box<RawDecl>, ParsedParams),
}

struct ParsedParams {
    params: Vec<Param>,
    /// `false` only for an empty `()` list, which is not a prototype.
    prototype: bool,
    variadic: bool,
    /// Tags whose scope is this parameter list.
    introduced: HashMap<String, TagEntry>,
    /// Names (enumeration constants and parameters) from that scope.
    introduced_names: HashMap<String, NameBinding>,
}

/// One step of an `offsetof` member designator after the type name.
enum OffsetStep {
    Member(Token),
    Index(Expr, Token),
}

fn is_type_specifier(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::KwVoid
            | TokenKind::KwChar
            | TokenKind::KwShort
            | TokenKind::KwInt
            | TokenKind::KwLong
            | TokenKind::KwSigned
            | TokenKind::KwUnsigned
            | TokenKind::KwStruct
            | TokenKind::KwUnion
            | TokenKind::KwEnum
            | TokenKind::KwBool
            | TokenKind::KwFloat
            | TokenKind::KwDouble
    )
}

fn found(token: &Token) -> String {
    if token.kind == TokenKind::Eof {
        "end of file".to_string()
    } else {
        format!("'{}'", token.spelling)
    }
}

fn unsupported(kind: TokenKind) -> Option<&'static str> {
    Some(match kind {
        TokenKind::KwAtomic => "atomic types are not supported",
        TokenKind::KwComplex | TokenKind::KwImaginary => "complex types are not supported",
        TokenKind::KwNoreturn => "_Noreturn is not supported",
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
