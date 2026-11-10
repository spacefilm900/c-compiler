//! Recursive-descent parser for a C subset.
//!
//! Expression precedence follows the C standard. From loosest to tightest:
//! comma, assignment, `?:`, `||`, `&&`, `|`, `^`, `&`, equality, relational,
//! shifts, additive, multiplicative, then unary and postfix. Assignment and
//! `?:` are right associative. A dangling `else` binds to the nearest `if`.

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diag::Diagnostic;
use crate::token::{Token, TokenKind};

pub fn parse(tokens: &[Token], filename: &str) -> Result<TranslationUnit, Diagnostic> {
    Parser {
        tokens,
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
    }
    .parse_translation_unit()
}

struct Parser<'a> {
    tokens: &'a [Token],
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
    /// `union tag` seen while an outer `struct tag` (or the reverse) is
    /// visible. A lone `union tag;` hides the outer tag. Using it as a type
    /// in the same declaration is an error.
    shadow_decl: Option<(String, RecordKind)>,
}

/// One tag in the shared struct/union/enum namespace.
#[derive(Clone)]
enum TagEntry {
    Record(StructRef),
    /// A completed enumeration. Its type is `int`.
    Enum,
}

/// A name the parser has to know before semantic analysis.
#[derive(Clone)]
enum NameBinding {
    Typedef(CType),
    /// Value of an enumeration constant, representable as `int`.
    EnumConst(i32),
    /// An ordinary identifier in this scope. It hides an outer typedef or
    /// enumeration constant.
    Ordinary,
}

impl Parser<'_> {
    fn parse_translation_unit(&mut self) -> Result<TranslationUnit, Diagnostic> {
        self.push_tags();
        let mut decls = Vec::new();
        while self.current().kind != TokenKind::Eof {
            decls.extend(self.parse_external()?);
        }
        self.pop_tags();
        Ok(TranslationUnit {
            decls,
            globals: Vec::new(),
            func_decls: Vec::new(),
            strings: Vec::new(),
            structs: std::mem::take(&mut self.structs),
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
        let specs = self.parse_specs(SpecPlace::Declaration)?;
        self.apply_shadow(&specs)?;
        if self.current().kind == TokenKind::Semi {
            self.finish_tag_only(&specs)?;
            self.expect(TokenKind::Semi, "';'")?;
            return Ok(Vec::new());
        }
        let base = specs.ty.clone();
        let first = self.parse_named_decl(base.clone())?;
        if specs.storage == Storage::Typedef {
            return self.finish_typedefs(specs, first);
        }
        if first.is_function() && self.current().kind == TokenKind::LBrace {
            self.bind_ordinary(&first.name, first.line, first.column)?;
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
        self.expect(TokenKind::Semi, "';'")?;
        Ok(decls)
    }

    fn finish_definition(
        &mut self,
        specs: Specs,
        declarator: NamedDecl,
    ) -> Result<TopDecl, Diagnostic> {
        let DeclKind::Function {
            ret,
            params,
            prototype: _,
            variadic,
        } = declarator.kind
        else {
            return Err(self.error_at(
                declarator.line,
                declarator.column,
                "expected a function definition",
            ));
        };
        for param in &params {
            if param.name.is_none() {
                return Err(self.error_at(
                    param.line,
                    param.column,
                    "parameter name omitted in function definition",
                ));
            }
        }
        self.push_tags();
        for (name, entry) in declarator.param_tags {
            self.tags
                .last_mut()
                .expect("function scope")
                .insert(name, entry);
        }
        for (name, binding) in declarator.param_names {
            self.names
                .last_mut()
                .expect("function scope")
                .insert(name, binding);
        }
        let body = self.parse_compound_scoped(false)?;
        self.pop_tags();
        let symbol = declarator.symbol.unwrap_or_else(|| declarator.name.clone());
        Ok(TopDecl::Func(FuncDef {
            ret_type: ret,
            name: declarator.name,
            params,
            variadic,
            body: Some(body),
            storage: specs.storage,
            linkage: Linkage::External,
            symbol,
            line: declarator.line,
            column: declarator.column,
        }))
    }

    fn finish_file_decl(
        &mut self,
        specs: &Specs,
        declarator: NamedDecl,
    ) -> Result<TopDecl, Diagnostic> {
        if let DeclKind::Function {
            ret,
            params,
            prototype,
            variadic,
        } = declarator.kind
        {
            if !prototype {
                return Err(self.error_at(
                    declarator.line,
                    declarator.column,
                    "old-style function declarations are not supported",
                ));
            }
            if self.current().kind == TokenKind::Assign {
                return Err(self.error("function declarations cannot have an initializer"));
            }
            let symbol = declarator
                .symbol
                .clone()
                .unwrap_or_else(|| declarator.name.clone());
            self.bind_ordinary(&declarator.name, declarator.line, declarator.column)?;
            return Ok(TopDecl::Func(FuncDef {
                ret_type: ret,
                name: declarator.name,
                params,
                variadic,
                body: None,
                storage: specs.storage,
                linkage: Linkage::External,
                symbol,
                line: declarator.line,
                column: declarator.column,
            }));
        }
        let DeclKind::Object(ty) = declarator.kind else {
            unreachable!("function handled above");
        };
        if ty.is_void() {
            return Err(self.error_at(
                declarator.line,
                declarator.column,
                "void variables are not supported",
            ));
        }
        let init = if self.match_kind(TokenKind::Assign) {
            Some(self.parse_initializer()?)
        } else {
            None
        };
        // Checked here, not in semantic analysis: completing this struct tag
        // later updates the shared type, so the later pass would see a
        // complete type. C11 6.7.9p3 requires a complete type at the initializer.
        self.reject_incomplete_initializer(
            &ty,
            init.is_some(),
            declarator.line,
            declarator.column,
        )?;
        self.bind_ordinary(&declarator.name, declarator.line, declarator.column)?;
        self.reject_bad_restrict(&ty, declarator.line, declarator.column)?;
        Ok(TopDecl::Global(GlobalDecl {
            name: declarator.name,
            ty,
            storage: specs.storage,
            init,
            line: declarator.line,
            column: declarator.column,
        }))
    }

    fn parse_specs(&mut self, where_: SpecPlace) -> Result<Specs, Diagnostic> {
        let mut flags = SpecFlags::default();
        loop {
            let token = self.current().clone();
            match token.kind {
                TokenKind::KwStatic | TokenKind::KwExtern => {
                    let storage = if token.kind == TokenKind::KwStatic {
                        Storage::Static
                    } else {
                        Storage::Extern
                    };
                    self.note_storage(&mut flags, storage, &token, where_)?;
                }
                TokenKind::KwVoid
                | TokenKind::KwChar
                | TokenKind::KwShort
                | TokenKind::KwInt
                | TokenKind::KwLong
                | TokenKind::KwSigned
                | TokenKind::KwUnsigned => {
                    self.advance();
                    self.note_type(&mut flags, &token)?;
                }
                TokenKind::KwConst | TokenKind::KwVolatile => {
                    self.advance();
                    self.note_qualifier(&mut flags, &token)?;
                }
                TokenKind::KwRestrict => {
                    self.advance();
                    self.note_restrict(&mut flags, &token)?;
                }
                TokenKind::KwTypedef => {
                    self.note_storage(&mut flags, Storage::Typedef, &token, where_)?;
                }
                TokenKind::KwInline => {
                    // Accepted and ignored. `static inline` stays `static`.
                    self.advance();
                }
                TokenKind::KwNoreturn => {
                    // A function specifier. It does not change the type.
                    self.advance();
                }
                TokenKind::KwRegister | TokenKind::KwAuto => {
                    self.note_storage(&mut flags, Storage::None, &token, where_)?;
                    flags.plain_storage = true;
                }
                TokenKind::KwBool => {
                    self.advance();
                    self.note_bool(&mut flags, &token)?;
                }
                TokenKind::KwFloat | TokenKind::KwDouble => {
                    self.advance();
                    self.note_float(&mut flags, &token)?;
                }
                TokenKind::KwStruct => {
                    self.note_record(&mut flags, RecordKind::Struct)?;
                }
                TokenKind::KwUnion => {
                    self.note_record(&mut flags, RecordKind::Union)?;
                }
                TokenKind::KwEnum => {
                    self.note_enum(&mut flags)?;
                }
                TokenKind::Ident if token.spelling == "__builtin_va_list" => {
                    if flags.struct_ty.is_some()
                        || flags.bool_ty
                        || flags.float_ty.is_some()
                        || flags.has_scalar()
                    {
                        break;
                    }
                    self.advance();
                    flags.any = true;
                    flags.struct_ty = Some(self.builtin_va_list());
                    flags.line = token.line;
                    flags.column = token.column;
                }
                TokenKind::Ident if self.is_typedef(&token.spelling) => {
                    // A typedef name is a type specifier only when the list
                    // does not already have one. `typedef T T` is a declarator
                    // named T, not two type specifiers.
                    if flags.struct_ty.is_some()
                        || flags.bool_ty
                        || flags.float_ty.is_some()
                        || flags.has_scalar()
                    {
                        break;
                    }
                    self.note_typedef_name(&mut flags)?;
                }
                TokenKind::Ident
                    if self.spec_word(&[
                        "__extension__",
                        "__inline",
                        "__inline__",
                        "__restrict",
                        "__restrict__",
                    ]) =>
                {
                    self.note_ident_spec(&mut flags, where_)?;
                }
                kind => {
                    if self.spec_word(&["__attribute__", "__attribute"]) {
                        self.skip_attribute()?;
                        continue;
                    }
                    if let Some(message) = unsupported(kind) {
                        return Err(self.error(message));
                    }
                    break;
                }
            }
        }
        if !flags.any {
            return Err(self.error(format!("expected a type, found {}", found(self.current()))));
        }
        let ty = self.resolve_type(&flags)?.with_quals(flags.quals);
        let shadow_decl = self.shadow_decl.take();
        // A lone `union tag;` is a declaration and may hide an outer struct.
        // A parameter, member, or type name has no such form: the visible tag
        // is the one being named, and the other kind is a constraint violation.
        if let Some((tag, _)) = &shadow_decl {
            if where_ != SpecPlace::Declaration {
                return Err(self.error_at(
                    flags.line,
                    flags.column,
                    format!("'{tag}' defined as a different tag type"),
                ));
            }
        }
        Ok(Specs {
            storage: flags.storage,
            storage_at: flags.storage_at,
            ty,
            forward_tag: flags.forward_tag,
            introduced_tag: flags.introduced_tag,
            anon_record: flags.anon_record,
            shadow_decl,
            line: flags.line,
            column: flags.column,
        })
    }

    fn note_storage(
        &mut self,
        flags: &mut SpecFlags,
        storage: Storage,
        token: &Token,
        where_: SpecPlace,
    ) -> Result<(), Diagnostic> {
        let allowed = match storage {
            Storage::None => matches!(where_, SpecPlace::Declaration | SpecPlace::Parameter),
            Storage::Static | Storage::Extern | Storage::Typedef => {
                where_ == SpecPlace::Declaration
            }
        };
        if !allowed {
            let message = match where_ {
                SpecPlace::Parameter => "storage-class specifiers are not supported on parameters",
                SpecPlace::Member => "storage class is not allowed on a struct member",
                SpecPlace::Declaration | SpecPlace::TypeName => {
                    "storage-class specifiers are not allowed in this type name"
                }
            };
            return Err(self.error(message));
        }
        self.advance();
        if storage == Storage::None {
            if flags.storage != Storage::None || flags.plain_storage {
                return Err(self.error("multiple storage-class specifiers"));
            }
            flags.plain_storage = true;
            return Ok(());
        }
        if flags.storage != Storage::None || flags.plain_storage {
            return Err(self.error("multiple storage-class specifiers"));
        }
        flags.storage = storage;
        flags.storage_at = Some((token.line, token.column));
        Ok(())
    }

    fn note_restrict(&self, flags: &mut SpecFlags, token: &Token) -> Result<(), Diagnostic> {
        if flags.quals.is_restrict {
            return Err(self.error_at(token.line, token.column, "duplicate 'restrict' qualifier"));
        }
        flags.quals.is_restrict = true;
        Ok(())
    }

    fn note_bool(&mut self, flags: &mut SpecFlags, token: &Token) -> Result<(), Diagnostic> {
        if flags.bool_ty
            || flags.float_ty.is_some()
            || flags.struct_ty.is_some()
            || flags.has_scalar()
        {
            return Err(self.error_at(
                token.line,
                token.column,
                "invalid combination of type specifiers",
            ));
        }
        flags.any = true;
        flags.bool_ty = true;
        flags.line = token.line;
        flags.column = token.column;
        Ok(())
    }

    fn note_float(&mut self, flags: &mut SpecFlags, token: &Token) -> Result<(), Diagnostic> {
        let bad = self.error_at(
            token.line,
            token.column,
            "invalid combination of type specifiers",
        );
        if flags.struct_ty.is_some() || flags.bool_ty || flags.float_ty.is_some() {
            return Err(bad);
        }
        flags.any = true;
        flags.line = token.line;
        flags.column = token.column;
        if token.kind == TokenKind::KwFloat {
            if flags.has_scalar() {
                return Err(bad);
            }
            flags.float_ty = Some(FloatKind::Float);
            return Ok(());
        }
        if flags.short != 0
            || flags.char_
            || flags.int_
            || flags.signed
            || flags.unsigned
            || flags.void
            || flags.long > 1
        {
            return Err(bad);
        }
        if flags.long == 1 {
            flags.long = 0;
            flags.float_ty = Some(FloatKind::LongDouble);
        } else {
            flags.float_ty = Some(FloatKind::Double);
        }
        Ok(())
    }

    fn note_record(&mut self, flags: &mut SpecFlags, kind: RecordKind) -> Result<(), Diagnostic> {
        if flags.struct_ty.is_some()
            || flags.has_scalar()
            || flags.bool_ty
            || flags.float_ty.is_some()
        {
            return Err(self.error("invalid combination of type specifiers"));
        }
        let (ty, forward, line, column) = self.parse_record_specifier(kind)?;
        flags.any = true;
        // C11 6.7.2.1: an anonymous member's type specifier is a struct or
        // union specifier with no tag. A typedef name is not that specifier.
        flags.anon_record = ty
            .struct_ref()
            .is_some_and(|handle| handle.tag().is_empty() && handle.is_complete());
        flags.struct_ty = Some(ty);
        flags.forward_tag = forward;
        flags.line = line;
        flags.column = column;
        Ok(())
    }

    fn note_typedef_name(&mut self, flags: &mut SpecFlags) -> Result<(), Diagnostic> {
        if flags.struct_ty.is_some()
            || flags.has_scalar()
            || flags.bool_ty
            || flags.float_ty.is_some()
        {
            return Err(self.error("invalid combination of type specifiers"));
        }
        let token = self.advance();
        let Some(NameBinding::Typedef(ty)) = self.lookup_name(&token.spelling) else {
            return Err(self.error_at(
                token.line,
                token.column,
                format!("unknown type name '{}'", token.spelling),
            ));
        };
        flags.any = true;
        flags.struct_ty = Some(ty);
        flags.line = token.line;
        flags.column = token.column;
        Ok(())
    }

    fn note_enum(&mut self, flags: &mut SpecFlags) -> Result<(), Diagnostic> {
        if flags.struct_ty.is_some()
            || flags.has_scalar()
            || flags.bool_ty
            || flags.float_ty.is_some()
        {
            return Err(self.error("invalid combination of type specifiers"));
        }
        let (line, column) = self.parse_enum_specifier()?;
        flags.any = true;
        flags.introduced_tag = true;
        flags.line = line;
        flags.column = column;
        Ok(())
    }

    fn spec_word(&self, words: &[&str]) -> bool {
        self.current().kind == TokenKind::Ident && words.contains(&self.current().spelling.as_str())
    }

    fn note_ident_spec(
        &mut self,
        flags: &mut SpecFlags,
        _where: SpecPlace,
    ) -> Result<(), Diagnostic> {
        let token = self.current().clone();
        self.advance();
        match token.spelling.as_str() {
            "__inline" | "__inline__" => {}
            "__restrict" | "__restrict__" => self.note_restrict(flags, &token)?,
            "__extension__" => {}
            _ => unreachable!("ident specifier"),
        }
        Ok(())
    }

    fn note_qualifier(&self, flags: &mut SpecFlags, token: &Token) -> Result<(), Diagnostic> {
        // A qualifier does not satisfy "at least one type specifier" (C11 6.7.2p2).
        let which = if token.kind == TokenKind::KwConst {
            "const"
        } else {
            "volatile"
        };
        let duplicate = if token.kind == TokenKind::KwConst {
            flags.quals.is_const
        } else {
            flags.quals.is_volatile
        };
        if duplicate {
            return Err(self.error_at(
                token.line,
                token.column,
                format!("duplicate '{which}' qualifier"),
            ));
        }
        if token.kind == TokenKind::KwConst {
            flags.quals.is_const = true;
        } else {
            flags.quals.is_volatile = true;
        }
        Ok(())
    }

    fn note_type(&self, flags: &mut SpecFlags, token: &Token) -> Result<(), Diagnostic> {
        if flags.struct_ty.is_some()
            || flags.bool_ty
            || (flags.float_ty.is_some() && token.kind != TokenKind::KwLong)
        {
            return Err(self.error_at(
                token.line,
                token.column,
                "invalid combination of type specifiers",
            ));
        }
        flags.any = true;
        flags.line = token.line;
        flags.column = token.column;
        let bad = self.error_at(
            token.line,
            token.column,
            "invalid combination of type specifiers",
        );
        match token.kind {
            TokenKind::KwVoid => {
                if flags.void {
                    return Err(bad);
                }
                flags.void = true;
            }
            TokenKind::KwChar => {
                if flags.char_ {
                    return Err(bad);
                }
                flags.char_ = true;
            }
            TokenKind::KwShort => {
                flags.short += 1;
                if flags.short > 1 {
                    return Err(bad);
                }
            }
            TokenKind::KwInt => {
                if flags.int_ {
                    return Err(bad);
                }
                flags.int_ = true;
            }
            TokenKind::KwLong => {
                if flags.float_ty == Some(FloatKind::Double) && flags.long == 0 {
                    flags.float_ty = Some(FloatKind::LongDouble);
                } else if flags.float_ty.is_some() {
                    return Err(bad);
                } else {
                    flags.long += 1;
                    if flags.long > 2 {
                        return Err(bad);
                    }
                }
            }
            TokenKind::KwSigned => {
                if flags.signed || flags.unsigned {
                    return Err(bad);
                }
                flags.signed = true;
            }
            TokenKind::KwUnsigned => {
                if flags.unsigned || flags.signed {
                    return Err(bad);
                }
                flags.unsigned = true;
            }
            _ => unreachable!("type specifier"),
        }
        Ok(())
    }

    fn resolve_type(&self, flags: &SpecFlags) -> Result<CType, Diagnostic> {
        let bad = || {
            self.error_at(
                flags.line,
                flags.column,
                "invalid combination of type specifiers",
            )
        };
        if let Some(ty) = &flags.struct_ty {
            if flags.has_scalar() || flags.bool_ty || flags.float_ty.is_some() {
                return Err(bad());
            }
            return Ok(ty.clone());
        }
        if flags.bool_ty {
            if flags.has_scalar() || flags.float_ty.is_some() {
                return Err(bad());
            }
            return Ok(CType::bool_type());
        }
        if let Some(kind) = flags.float_ty {
            if flags.has_scalar() || flags.signed || flags.unsigned {
                return Err(bad());
            }
            return Ok(CType::Float(kind));
        }
        Ok(CType::from_name(self.resolve_scalar(flags)?))
    }

    fn resolve_scalar(&self, flags: &SpecFlags) -> Result<TypeName, Diagnostic> {
        let bad = self.error_at(
            flags.line,
            flags.column,
            "invalid combination of type specifiers",
        );
        if flags.void {
            if flags.char_
                || flags.short != 0
                || flags.int_
                || flags.long != 0
                || flags.signed
                || flags.unsigned
            {
                return Err(bad);
            }
            return Ok(TypeName::Void);
        }
        if flags.char_ {
            if flags.short != 0 || flags.int_ || flags.long != 0 {
                return Err(bad);
            }
            return Ok(if flags.unsigned {
                TypeName::UChar
            } else if flags.signed {
                TypeName::SChar
            } else {
                TypeName::Char
            });
        }
        if flags.short != 0 && flags.long != 0 {
            return Err(bad);
        }
        if flags.short == 1 {
            return Ok(if flags.unsigned {
                TypeName::UShort
            } else {
                TypeName::Short
            });
        }
        if flags.long == 2 {
            return Ok(if flags.unsigned {
                TypeName::ULongLong
            } else {
                TypeName::LongLong
            });
        }
        if flags.long == 1 {
            return Ok(if flags.unsigned {
                TypeName::ULong
            } else {
                TypeName::Long
            });
        }
        if flags.unsigned {
            return Ok(TypeName::UInt);
        }
        Ok(TypeName::Int)
    }

    /// `struct tag`, `struct tag { members }`, or the same forms for `union`.
    ///
    /// A brace list defines the type in the current scope. A use with no
    /// braces names the visible tag, or declares an incomplete one here when
    /// none is visible. `forward` is set when the visible tag lives in an
    /// outer scope, so a lone `struct tag;` can hide it (C11 6.7.2.3).
    /// A definition with no tag (`struct { int x; }`) is a distinct type.
    fn parse_record_specifier(
        &mut self,
        kind: RecordKind,
    ) -> Result<(CType, Option<String>, usize, usize), Diagnostic> {
        self.shadow_decl = None;
        let kw = self.advance();
        let word = kind.word();
        let (tag, line, column) = if self.current().kind == TokenKind::Ident {
            let tag_tok = self.advance();
            (Some(tag_tok.spelling), tag_tok.line, tag_tok.column)
        } else if self.current().kind == TokenKind::LBrace {
            (None, kw.line, kw.column)
        } else {
            return Err(self.error_at(
                kw.line,
                kw.column,
                format!("expected a {word} tag or '{{'"),
            ));
        };
        if self.current().kind == TokenKind::LBrace {
            let ty = self.define_record(kind, tag.as_deref(), line, column)?;
            self.skip_attributes()?;
            return Ok((ty, None, line, column));
        }
        let tag = tag.expect("tag");
        match self.lookup_tag_current(&tag) {
            Some(TagEntry::Record(existing)) if existing.kind() == kind => {
                return Ok((CType::Struct(existing), None, line, column));
            }
            Some(_) => {
                return Err(self.error_at(
                    line,
                    column,
                    format!("'{tag}' defined as a different tag type"),
                ));
            }
            None => {}
        }
        if let Some(outer) = self.lookup_tag(&tag) {
            match outer {
                TagEntry::Record(existing) if existing.kind() == kind => {
                    return Ok((CType::Struct(existing), Some(tag), line, column));
                }
                // An outer tag of the other kind. A lone `union tag;` hides it.
                // `union tag *p` while `struct tag` is visible is a conflict.
                _ => {
                    self.shadow_decl = Some((tag.clone(), kind));
                    let dummy = StructRef::new_incomplete(self.next_struct, &tag, kind);
                    self.next_struct += 1;
                    return Ok((CType::Struct(dummy), None, line, column));
                }
            }
        }
        let created = self.declare_fresh_tag(&tag, kind);
        Ok((CType::Struct(created), None, line, column))
    }

    fn define_record(
        &mut self,
        kind: RecordKind,
        tag: Option<&str>,
        line: usize,
        column: usize,
    ) -> Result<CType, Diagnostic> {
        let word = kind.word();
        let handle = if let Some(tag) = tag {
            if let Some(existing) = self.lookup_record_current(tag) {
                if existing.kind() != kind {
                    return Err(self.error_at(
                        line,
                        column,
                        format!("'{tag}' defined as a different tag type"),
                    ));
                }
                if existing.is_complete() || self.defining.contains(&existing.id()) {
                    return Err(self.error_at(
                        line,
                        column,
                        format!("redefinition of {word} '{tag}'"),
                    ));
                }
                existing
            } else if self.lookup_tag_current(tag).is_some() {
                return Err(self.error_at(
                    line,
                    column,
                    format!("'{tag}' defined as a different tag type"),
                ));
            } else {
                self.declare_fresh_tag(tag, kind)
            }
        } else {
            self.declare_fresh_tag("", kind)
        };
        self.expect(TokenKind::LBrace, "'{'")?;
        let id = handle.id();
        self.defining.insert(id);
        let parsed = self.parse_member_list();
        self.defining.remove(&id);
        let mut fields = parsed?;
        self.expect(TokenKind::RBrace, "'}'")?;
        let label = tag.unwrap_or(word);
        if fields.is_empty() {
            return Err(self.error_at(line, column, format!("{word} '{label}' has no members")));
        }
        let spec: Vec<LayoutMember> = fields
            .iter()
            .map(|field| LayoutMember {
                size: field.ty.size().unwrap_or(0),
                align: field.ty.member_align(),
                bit_width: field.bit_width,
                named: !field.name.is_empty(),
            })
            .collect();
        let Some((places, size, align)) = layout_members(&spec, kind == RecordKind::Union) else {
            return Err(self.error_at(line, column, format!("{word} '{label}' is too large")));
        };
        if size == 0 {
            return Err(self.error_at(line, column, format!("{word} '{label}' has size 0")));
        }
        for (field, place) in fields.iter_mut().zip(places) {
            field.offset = place.offset;
            field.bit_offset = place.bit_offset;
            field.bit_width = place.bit_width;
        }
        let llvm_name = self.alloc_llvm_name(kind, tag.unwrap_or("anon"));
        handle.complete(fields, size, align, llvm_name);
        self.structs.push(handle.clone());
        Ok(CType::Struct(handle))
    }

    fn parse_enum_specifier(&mut self) -> Result<(usize, usize), Diagnostic> {
        let kw = self.advance();
        let (tag, line, column) = if self.current().kind == TokenKind::Ident {
            let tag_tok = self.advance();
            (Some(tag_tok.spelling), tag_tok.line, tag_tok.column)
        } else {
            (None, kw.line, kw.column)
        };
        if self.current().kind != TokenKind::LBrace {
            let Some(tag) = tag else {
                return Err(self.error_at(kw.line, kw.column, "expected an enum tag or '{'"));
            };
            return match self.lookup_tag(&tag) {
                Some(TagEntry::Enum) => Ok((line, column)),
                Some(_) => Err(self.error_at(
                    line,
                    column,
                    format!("'{tag}' defined as a different tag type"),
                )),
                None => Err(self.error_at(line, column, format!("incomplete enum '{tag}'"))),
            };
        }
        if let Some(tag) = &tag {
            match self.lookup_tag_current(tag) {
                Some(TagEntry::Enum) => {
                    return Err(self.error_at(
                        line,
                        column,
                        format!("redefinition of enum '{tag}'"),
                    ));
                }
                Some(_) => {
                    return Err(self.error_at(
                        line,
                        column,
                        format!("'{tag}' defined as a different tag type"),
                    ));
                }
                None => {
                    self.tags
                        .last_mut()
                        .expect("tag scope")
                        .insert(tag.clone(), TagEntry::Enum);
                }
            }
        }
        self.advance();
        self.parse_enumerators()?;
        self.expect(TokenKind::RBrace, "'}'")?;
        Ok((line, column))
    }

    fn parse_enumerators(&mut self) -> Result<(), Diagnostic> {
        if self.current().kind == TokenKind::RBrace {
            return Err(self.error("enum has no enumerators"));
        }
        let mut next: i64 = 0;
        loop {
            if self.current().kind != TokenKind::Ident {
                return Err(self.error("expected an enumerator"));
            }
            let name = self.advance();
            let value = if self.match_kind(TokenKind::Assign) {
                let expr = self.parse_assignment()?;
                let folded = eval_ice(&expr)
                    .map_err(|err| self.error_at(expr.line(), expr.column(), bound_message(err)))?;
                enum_int(folded).map_err(|_| {
                    self.error_at(
                        expr.line(),
                        expr.column(),
                        "enumeration constant is not representable as int",
                    )
                })?
            } else if !(i32::MIN as i64..=i32::MAX as i64).contains(&next) {
                return Err(self.error_at(
                    name.line,
                    name.column,
                    "enumeration constant is not representable as int",
                ));
            } else {
                next as i32
            };
            self.bind_enum(&name.spelling, value, name.line, name.column)?;
            next = value as i64 + 1;
            if self.match_kind(TokenKind::Comma) {
                if self.current().kind == TokenKind::RBrace {
                    break;
                }
                continue;
            }
            break;
        }
        Ok(())
    }

    fn finish_typedefs(
        &mut self,
        specs: Specs,
        first: NamedDecl,
    ) -> Result<Vec<TopDecl>, Diagnostic> {
        self.bind_typedef_decl(&first)?;
        while self.match_kind(TokenKind::Comma) {
            let next = self.parse_named_decl(specs.ty.clone())?;
            if next.is_function() && self.current().kind == TokenKind::LBrace {
                return Err(self.error("a function definition cannot follow another declarator"));
            }
            self.bind_typedef_decl(&next)?;
        }
        self.expect(TokenKind::Semi, "';'")?;
        Ok(Vec::new())
    }

    fn parse_typedef_declarators(&mut self, specs: &Specs) -> Result<(), Diagnostic> {
        loop {
            let declarator = self.parse_named_decl(specs.ty.clone())?;
            if declarator.is_function() && self.current().kind == TokenKind::LBrace {
                return Err(self.error("a typedef cannot have a function body"));
            }
            self.bind_typedef_decl(&declarator)?;
            if self.match_kind(TokenKind::Comma) {
                continue;
            }
            break;
        }
        self.expect(TokenKind::Semi, "';'")?;
        Ok(())
    }

    fn bind_typedef_decl(&mut self, declarator: &NamedDecl) -> Result<(), Diagnostic> {
        if self.current().kind == TokenKind::Assign {
            return Err(self.error("a typedef cannot have an initializer"));
        }
        let ty = match &declarator.kind {
            DeclKind::Object(ty) => ty.clone(),
            DeclKind::Function {
                ret,
                params,
                variadic,
                ..
            } => CType::func(
                ret.clone(),
                params.iter().map(|param| param.ty.unqualified()).collect(),
                *variadic,
            ),
        };
        self.reject_bad_restrict(&ty, declarator.line, declarator.column)?;
        self.bind_typedef(&declarator.name, ty, declarator.line, declarator.column)
    }

    fn bind_typedef(
        &mut self,
        name: &str,
        ty: CType,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        let scope = self.names.last_mut().expect("name scope");
        if let Some(existing) = scope.get(name) {
            match existing {
                NameBinding::Typedef(old) if compatible(old, &ty) => return Ok(()),
                _ => {
                    return Err(self.error_at(line, column, format!("redefinition of '{name}'")));
                }
            }
        }
        scope.insert(name.to_string(), NameBinding::Typedef(ty));
        Ok(())
    }

    fn bind_ordinary(&mut self, name: &str, line: usize, column: usize) -> Result<(), Diagnostic> {
        let scope = self.names.last_mut().expect("name scope");
        match scope.get(name) {
            Some(NameBinding::Ordinary) | None => {
                scope.insert(name.to_string(), NameBinding::Ordinary);
                Ok(())
            }
            Some(NameBinding::Typedef(_) | NameBinding::EnumConst(_)) => {
                Err(self.error_at(line, column, format!("redefinition of '{name}'")))
            }
        }
    }

    fn bind_enum(
        &mut self,
        name: &str,
        value: i32,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        let scope = self.names.last_mut().expect("name scope");
        if scope.contains_key(name) {
            return Err(self.error_at(line, column, format!("redefinition of '{name}'")));
        }
        scope.insert(name.to_string(), NameBinding::EnumConst(value));
        Ok(())
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
                && (self.is_typedef(&token.spelling) || token.spelling == "__builtin_va_list"))
            || matches!(token.spelling.as_str(), "__restrict" | "__restrict__")
                && token.kind == TokenKind::Ident
    }

    /// `restrict` only qualifies a pointer (C11 6.7.3p2).
    fn reject_bad_restrict(
        &self,
        ty: &CType,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        fn walk(ty: &CType) -> bool {
            if ty.quals().is_restrict && !ty.unqualified().is_pointer() {
                return true;
            }
            match ty.peel() {
                CType::Pointer(inner) | CType::Array(inner, _) => walk(inner),
                CType::Func(func) => walk(&func.ret) || func.params.iter().any(walk),
                _ => false,
            }
        }
        if walk(ty) {
            Err(self.error_at(line, column, "restrict requires a pointer type"))
        } else {
            Ok(())
        }
    }

    fn skip_attributes(&mut self) -> Result<(), Diagnostic> {
        while self.spec_word(&["__attribute__", "__attribute"]) {
            self.skip_attribute()?;
        }
        Ok(())
    }

    /// `__attribute__ (( balanced ))`. Nested parentheses are skipped.
    fn skip_attribute(&mut self) -> Result<(), Diagnostic> {
        let at = self.advance();
        self.expect(TokenKind::LParen, "'('")?;
        self.expect(TokenKind::LParen, "'('")?;
        let mut depth = 1i32;
        while depth > 0 {
            match self.current().kind {
                TokenKind::Eof => {
                    return Err(self.error_at(at.line, at.column, "unclosed __attribute__"));
                }
                TokenKind::LParen => depth += 1,
                TokenKind::RParen => depth -= 1,
                _ => {}
            }
            self.advance();
        }
        self.expect(TokenKind::RParen, "')'")?;
        Ok(())
    }

    /// Attributes and an optional `__asm__("name")` after a declarator.
    fn parse_decl_suffix(&mut self) -> Result<Option<String>, Diagnostic> {
        let mut symbol = None;
        loop {
            self.skip_attributes()?;
            if self.spec_word(&["__asm__", "__asm", "asm"]) {
                let name = self.parse_asm_label()?;
                symbol = Some(name);
                continue;
            }
            break;
        }
        Ok(symbol)
    }

    fn parse_asm_label(&mut self) -> Result<String, Diagnostic> {
        self.advance();
        while self.spec_word(&["__volatile__", "__volatile", "volatile", "goto"])
            || self.current().kind == TokenKind::KwVolatile
        {
            self.advance();
        }
        self.expect(TokenKind::LParen, "'('")?;
        if self.current().kind != TokenKind::String {
            return Err(self.error("expected a string after __asm__"));
        }
        let mut bytes = self.advance().bytes.expect("string token");
        while self.current().kind == TokenKind::String {
            bytes.extend(self.advance().bytes.expect("string token"));
        }
        self.expect(TokenKind::RParen, "')'")?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    fn parse_member_list(&mut self) -> Result<Vec<StructField>, Diagnostic> {
        let mut fields = Vec::new();
        let mut names = HashSet::new();
        if self.current().kind == TokenKind::RBrace {
            return Ok(fields);
        }
        while !matches!(self.current().kind, TokenKind::RBrace | TokenKind::Eof) {
            if self.current().kind == TokenKind::Colon {
                return Err(self.error("expected a type before ':'"));
            }
            let specs = self.parse_specs(SpecPlace::Member)?;
            if self.current().kind == TokenKind::Semi {
                if specs.anon_record {
                    let ty = specs.ty.clone();
                    let mut promoted = Vec::new();
                    Self::promoted_names_of(&ty, &mut promoted);
                    for name in promoted {
                        if !names.insert(name.clone()) {
                            return Err(self.error(format!("duplicate member '{name}'")));
                        }
                    }
                    fields.push(StructField {
                        name: String::new(),
                        ty,
                        offset: 0,
                        bit_offset: 0,
                        bit_width: None,
                    });
                    self.advance();
                    continue;
                }
                return Err(self.error("expected a declarator"));
            }
            loop {
                if self.current().kind == TokenKind::Colon {
                    let width = self.parse_bit_width(&specs.ty, false)?;
                    fields.push(StructField {
                        name: String::new(),
                        ty: specs.ty.clone(),
                        offset: 0,
                        bit_offset: 0,
                        bit_width: Some(width),
                    });
                } else {
                    let named = self.parse_named_decl(specs.ty.clone())?;
                    let DeclKind::Object(ty) = named.kind else {
                        return Err(self.error_at(
                            named.line,
                            named.column,
                            "function members are not supported",
                        ));
                    };
                    let width = if self.current().kind == TokenKind::Colon {
                        Some(self.parse_bit_width(&ty, true)?)
                    } else {
                        None
                    };
                    if width.is_none() && ty.is_array() && ty.array_len().is_none() {
                        return Err(self.error_at(
                            named.line,
                            named.column,
                            "flexible array members are not supported",
                        ));
                    }
                    if width.is_none() && !ty.is_complete() {
                        return Err(self.error_at(
                            named.line,
                            named.column,
                            format!(
                                "member '{}' has incomplete type '{}'",
                                named.name,
                                ty.display()
                            ),
                        ));
                    }
                    if !names.insert(named.name.clone()) {
                        return Err(self.error_at(
                            named.line,
                            named.column,
                            format!("duplicate member '{}'", named.name),
                        ));
                    }
                    fields.push(StructField {
                        name: named.name,
                        ty,
                        offset: 0,
                        bit_offset: 0,
                        bit_width: width,
                    });
                }
                if self.match_kind(TokenKind::Comma) {
                    if matches!(
                        self.current().kind,
                        TokenKind::Semi | TokenKind::RBrace | TokenKind::Eof
                    ) {
                        return Err(self.error("expected a member declarator"));
                    }
                    continue;
                }
                break;
            }
            self.expect(TokenKind::Semi, "';'")?;
        }
        Ok(fields)
    }

    /// Member names an anonymous struct or union contributes to its parent.
    fn promoted_names_of(ty: &CType, out: &mut Vec<String>) {
        let Some(fields) = ty.struct_ref().and_then(|handle| handle.fields()) else {
            return;
        };
        for field in fields {
            if field.bit_width == Some(0) {
                continue;
            }
            if field.is_anonymous_record() {
                Self::promoted_names_of(&field.ty, out);
            } else if !field.name.is_empty() {
                out.push(field.name.clone());
            }
        }
    }

    /// `: constant-expression` on a struct or union member.
    ///
    /// The width is an integer constant expression. Zero is allowed only on
    /// an unnamed field, where it forces alignment. The width cannot exceed
    /// the width of the declared type. `_Bool` is one bit wide.
    fn parse_bit_width(&mut self, ty: &CType, named: bool) -> Result<u32, Diagnostic> {
        let colon = self.advance();
        let Some(integer) = ty.unqualified().as_integer() else {
            return Err(self.error_at(
                colon.line,
                colon.column,
                format!("bit-field has non-integral type '{}'", ty.display()),
            ));
        };
        let expr = self.parse_conditional()?;
        let value = eval_ice(&expr)
            .map_err(|err| self.error_at(expr.line(), expr.column(), bound_message(err)))?;
        let signed = if value.ty.is_signed() {
            sign_extend(value.bits, value.ty.width())
        } else if value.bits > i64::MAX as u64 {
            return Err(self.error_at(expr.line(), expr.column(), "bit-field width is too large"));
        } else {
            value.bits as i64
        };
        if signed < 0 {
            return Err(self.error_at(
                expr.line(),
                expr.column(),
                format!("bit-field has negative width ({signed})"),
            ));
        }
        if signed == 0 && named {
            return Err(self.error_at(
                expr.line(),
                expr.column(),
                "named bit-field has zero width",
            ));
        }
        let max = if integer == TypeName::Bool {
            1
        } else {
            integer.width()
        };
        if signed > i64::from(max) {
            return Err(self.error_at(
                expr.line(),
                expr.column(),
                format!(
                    "width of bit-field ({signed} bits) exceeds the width of its type ({max} bits)"
                ),
            ));
        }
        Ok(signed as u32)
    }

    /// A lone `union tag;` hides an outer `struct tag`. Using that specifier
    /// as the type of a declarator is an error.
    fn apply_shadow(&mut self, specs: &Specs) -> Result<(), Diagnostic> {
        let Some((tag, kind)) = specs.shadow_decl.clone() else {
            return Ok(());
        };
        if self.current().kind == TokenKind::Semi {
            self.declare_fresh_tag(&tag, kind);
            return Ok(());
        }
        Err(self.error(format!("'{tag}' defined as a different tag type")))
    }

    /// A declaration that is only a tag (`struct node;` or `enum { A };`).
    fn finish_tag_only(&mut self, specs: &Specs) -> Result<(), Diagnostic> {
        if specs.introduced_tag || specs.ty.unqualified().is_record() {
            if specs.storage != Storage::None {
                return Err(self.error(
                    "storage class on a tag declaration with no declarator is not supported",
                ));
            }
            if let Some(tag) = &specs.forward_tag {
                let kind = specs
                    .ty
                    .struct_ref()
                    .map(|handle| handle.kind())
                    .unwrap_or(RecordKind::Struct);
                self.declare_fresh_tag(tag, kind);
            }
            return Ok(());
        }
        Err(self.error("expected a declarator"))
    }

    fn declare_fresh_tag(&mut self, tag: &str, kind: RecordKind) -> StructRef {
        let handle = StructRef::new_incomplete(self.next_struct, tag, kind);
        self.next_struct += 1;
        if !tag.is_empty() {
            self.tags
                .last_mut()
                .expect("tag scope")
                .insert(tag.to_string(), TagEntry::Record(handle.clone()));
        }
        handle
    }

    fn lookup_tag(&self, tag: &str) -> Option<TagEntry> {
        for scope in self.tags.iter().rev() {
            if let Some(found) = scope.get(tag) {
                return Some(found.clone());
            }
        }
        None
    }

    fn lookup_tag_current(&self, tag: &str) -> Option<TagEntry> {
        self.tags.last().and_then(|scope| scope.get(tag).cloned())
    }

    fn lookup_record_current(&self, tag: &str) -> Option<StructRef> {
        match self.lookup_tag_current(tag) {
            Some(TagEntry::Record(handle)) => Some(handle),
            _ => None,
        }
    }

    /// `__builtin_va_list` is `[1 x struct __va_list_tag]`, the SysV register
    /// save descriptor. As a parameter the array decays to a pointer, which
    /// is what `vprintf` takes.
    fn builtin_va_list(&mut self) -> CType {
        if let Some(ty) = &self.va_list {
            return ty.clone();
        }
        let handle = self.declare_fresh_tag("__va_list_tag", RecordKind::Struct);
        let fields = vec![
            StructField::plain("gp_offset", CType::from_name(TypeName::UInt), 0),
            StructField::plain("fp_offset", CType::from_name(TypeName::UInt), 4),
            StructField::plain("overflow_arg_area", CType::pointer(CType::Void), 8),
            StructField::plain("reg_save_area", CType::pointer(CType::Void), 16),
        ];
        let llvm_name = self.alloc_llvm_name(RecordKind::Struct, "__va_list_tag");
        handle.complete(fields, 24, 8, llvm_name);
        self.structs.push(handle.clone());
        let ty = CType::array(CType::Struct(handle), Some(1));
        self.va_list = Some(ty.clone());
        ty
    }

    fn alloc_llvm_name(&mut self, kind: RecordKind, tag: &str) -> String {
        let base = format!("{}.{}", kind.word(), tag);
        if self.llvm_names.insert(base.clone()) {
            return base;
        }
        let mut n = 1u32;
        loop {
            let name = format!("{base}.{n}");
            if self.llvm_names.insert(name.clone()) {
                return name;
            }
            n += 1;
        }
    }

    fn push_tags(&mut self) {
        self.tags.push(HashMap::new());
        self.names.push(HashMap::new());
    }

    fn pop_tags(&mut self) {
        self.tags.pop();
        self.names.pop();
    }

    /// A named declarator. `int *a, b` parses two of these against one base type.
    fn parse_named_decl(&mut self, base: CType) -> Result<NamedDecl, Diagnostic> {
        let raw = self.parse_raw_declarator(false)?;
        let (name, line, column) = match decl_name(&raw) {
            Some(found) => found,
            None => {
                let (line, column) = decl_pos(&raw);
                return Err(self.error_at(line, column, "expected an identifier"));
            }
        };
        let layer = self.derive(base, &raw, line, column)?;
        let (param_tags, param_names) = match &layer {
            Layer::Function {
                introduced,
                introduced_names,
                ..
            } => (
                introduced
                    .iter()
                    .map(|(name, handle)| (name.clone(), handle.clone()))
                    .collect(),
                introduced_names
                    .iter()
                    .map(|(name, binding)| (name.clone(), binding.clone()))
                    .collect(),
            ),
            Layer::Object(_) => (Vec::new(), Vec::new()),
        };
        let kind = self.layer_to_kind(layer)?;
        let symbol = self.parse_decl_suffix()?;
        Ok(NamedDecl {
            name,
            line,
            column,
            kind,
            param_tags,
            param_names,
            symbol,
        })
    }

    fn layer_to_kind(&self, layer: Layer) -> Result<DeclKind, Diagnostic> {
        Ok(match layer {
            Layer::Object(ty) => DeclKind::Object(ty),
            Layer::Function {
                ret,
                params,
                prototype,
                variadic,
                introduced: _,
                introduced_names: _,
            } => DeclKind::Function {
                ret,
                params,
                prototype,
                variadic,
            },
        })
    }

    fn parse_param_list(&mut self) -> Result<ParsedParams, Diagnostic> {
        self.expect(TokenKind::LParen, "'('")?;
        // Tags declared in a parameter list have prototype scope, or the
        // function's block scope when this declarator is a definition. The
        // map is returned either way; the caller keeps it only for a body.
        self.push_tags();
        let parsed = self.parse_param_list_in_scope();
        let introduced = self.tags.last().expect("parameter scope").clone();
        let introduced_names = self.names.last().expect("parameter scope").clone();
        self.pop_tags();
        let mut parsed = parsed?;
        parsed.introduced = introduced;
        parsed.introduced_names = introduced_names;
        Ok(parsed)
    }

    fn parse_param_list_in_scope(&mut self) -> Result<ParsedParams, Diagnostic> {
        if self.match_kind(TokenKind::RParen) {
            return Ok(ParsedParams {
                params: Vec::new(),
                prototype: false,
                variadic: false,
                introduced: HashMap::new(),
                introduced_names: HashMap::new(),
            });
        }
        if self.current().kind == TokenKind::KwVoid && self.lookahead() == TokenKind::RParen {
            self.advance();
            self.advance();
            return Ok(ParsedParams {
                params: Vec::new(),
                prototype: true,
                variadic: false,
                introduced: HashMap::new(),
                introduced_names: HashMap::new(),
            });
        }
        let mut params = Vec::new();
        let mut variadic = false;
        loop {
            if self.current().kind == TokenKind::Ellipsis {
                if params.is_empty() {
                    return Err(self.error("a named parameter is required before '...'"));
                }
                self.advance();
                variadic = true;
                break;
            }
            params.push(self.parse_param()?);
            if self.match_kind(TokenKind::Comma) {
                if self.current().kind == TokenKind::RParen {
                    return Err(self.error("trailing commas in parameter lists are not supported"));
                }
                continue;
            }
            break;
        }
        self.expect(TokenKind::RParen, "')'")?;
        Ok(ParsedParams {
            params,
            prototype: true,
            variadic,
            introduced: HashMap::new(),
            introduced_names: HashMap::new(),
        })
    }

    fn parse_param(&mut self) -> Result<Param, Diagnostic> {
        let specs = self.parse_specs(SpecPlace::Parameter)?;
        let base = specs.ty.clone();
        if !self.starts_raw_declarator() {
            if base.is_void() {
                if self.current().kind == TokenKind::Comma {
                    return Err(self.error_at(
                        specs.line,
                        specs.column,
                        "parameter has incomplete type 'void'",
                    ));
                }
                return Err(self.error(format!("expected ')', found {}", found(self.current()))));
            }
            if matches!(self.current().kind, TokenKind::Comma | TokenKind::RParen) {
                let ty = self.adjust_parameter(base, specs.line, specs.column)?;
                return Ok(Param {
                    name: None,
                    ty,
                    line: specs.line,
                    column: specs.column,
                    slot: String::new(),
                    arg_reg: String::new(),
                });
            }
            return Err(self.error(format!(
                "expected an identifier, found {}",
                found(self.current())
            )));
        }
        let raw = self.parse_raw_declarator(true)?;
        let named = decl_name(&raw);
        let (line, column) = named
            .as_ref()
            .map(|(_, line, column)| (*line, *column))
            .unwrap_or_else(|| decl_pos(&raw));
        let ty = match self.derive(base, &raw, line, column)? {
            Layer::Object(ty) => self.adjust_parameter(ty, line, column)?,
            Layer::Function {
                ret,
                params,
                variadic,
                ..
            } => {
                let func = CType::func(
                    ret,
                    params.iter().map(|param| param.ty.unqualified()).collect(),
                    variadic,
                );
                self.adjust_parameter(func, line, column)?
            }
        };
        if let Some((name, line, column)) = &named {
            self.bind_ordinary(name, *line, *column)?;
        }
        Ok(Param {
            name: named.map(|(name, _, _)| name),
            ty,
            line,
            column,
            slot: String::new(),
            arg_reg: String::new(),
        })
    }

    /// Outermost array parameters become pointers. A parameter of function
    /// type becomes a pointer to that function (C11 6.7.6.3).
    fn adjust_parameter(&self, ty: CType, line: usize, column: usize) -> Result<CType, Diagnostic> {
        let ty = match ty {
            CType::Array(elem, _) => CType::pointer(*elem),
            CType::Func(func) => CType::pointer(CType::Func(func)),
            other => other,
        };
        self.reject_bad_restrict(&ty, line, column)?;
        match ty {
            CType::Void => Err(self.error_at(line, column, "parameter has incomplete type 'void'")),
            // A prototype may name an incomplete struct or union (C11 6.7.6.3).
            // The type has to be complete at a definition and at a call.
            // Other incomplete types, such as `void` or an incomplete array
            // that was not adjusted to a pointer, are still rejected.
            ref other if other.unqualified().is_record() => Ok(ty),
            ref other if !other.is_complete() => Err(self.error_at(
                line,
                column,
                format!("parameter has incomplete type '{}'", other.display()),
            )),
            other => Ok(other),
        }
    }

    fn starts_raw_declarator(&self) -> bool {
        matches!(
            self.current().kind,
            TokenKind::Star | TokenKind::LParen | TokenKind::LBrack | TokenKind::Ident
        )
    }

    /// Declarator grammar, with the pointer written outside the direct declarator.
    ///
    /// `*` binds inside `[]` and `()`, so `int *a[3]` is an array of pointers
    /// and `int (*p)[3]` is a pointer to an array. An abstract declarator may
    /// omit the identifier (`sizeof(int *)`, `(void *)0`).
    fn parse_raw_declarator(&mut self, abstract_ok: bool) -> Result<RawDecl, Diagnostic> {
        if self.current().kind == TokenKind::Star {
            self.advance();
            let quals = self.parse_pointer_quals()?;
            if let Some(message) = unsupported(self.current().kind) {
                return Err(self.error(message));
            }
            let inner = self.parse_raw_declarator(abstract_ok)?;
            return Ok(RawDecl::Pointer(quals, Box::new(inner)));
        }
        self.parse_direct(abstract_ok)
    }

    fn parse_direct(&mut self, abstract_ok: bool) -> Result<RawDecl, Diagnostic> {
        let mut node = if self.current().kind == TokenKind::LParen {
            if abstract_ok && self.can_start_param() {
                let line = self.current().line;
                let column = self.current().column;
                let params = self.parse_param_list()?;
                RawDecl::Func(Box::new(RawDecl::Abstract(line, column)), params)
            } else {
                self.advance();
                let inner = self.parse_raw_declarator(abstract_ok)?;
                self.expect(TokenKind::RParen, "')'")?;
                inner
            }
        } else if self.current().kind == TokenKind::Ident {
            let name = self.advance();
            RawDecl::Named(name.spelling, name.line, name.column)
        } else if abstract_ok {
            let token = self.current();
            RawDecl::Abstract(token.line, token.column)
        } else if let Some(message) = unsupported(self.current().kind) {
            return Err(self.error(message));
        } else {
            return Err(self.error(format!(
                "expected an identifier, found {}",
                found(self.current())
            )));
        };
        loop {
            match self.current().kind {
                TokenKind::LBrack => {
                    self.advance();
                    let len = self.parse_array_len()?;
                    self.expect(TokenKind::RBrack, "']'")?;
                    node = RawDecl::Array(Box::new(node), len);
                }
                TokenKind::LParen => {
                    let params = self.parse_param_list()?;
                    node = RawDecl::Func(Box::new(node), params);
                }
                _ => break,
            }
        }
        Ok(node)
    }

    /// Qualifiers after `*` (`int * const p`). A repeated qualifier in this
    /// list is a diagnostic. `restrict` stays unsupported.
    fn parse_pointer_quals(&mut self) -> Result<Quals, Diagnostic> {
        let mut quals = Quals::default();
        loop {
            let token = self.current().clone();
            match token.kind {
                TokenKind::KwConst | TokenKind::KwVolatile => {
                    let which = if token.kind == TokenKind::KwConst {
                        "const"
                    } else {
                        "volatile"
                    };
                    let duplicate = if token.kind == TokenKind::KwConst {
                        quals.is_const
                    } else {
                        quals.is_volatile
                    };
                    if duplicate {
                        return Err(self.error(format!("duplicate '{which}' qualifier")));
                    }
                    if token.kind == TokenKind::KwConst {
                        quals.is_const = true;
                    } else {
                        quals.is_volatile = true;
                    }
                    self.advance();
                }
                TokenKind::KwRestrict => {
                    if quals.is_restrict {
                        return Err(self.error("duplicate 'restrict' qualifier"));
                    }
                    quals.is_restrict = true;
                    self.advance();
                }
                TokenKind::Ident
                    if matches!(
                        self.current().spelling.as_str(),
                        "__restrict" | "__restrict__"
                    ) =>
                {
                    if quals.is_restrict {
                        return Err(self.error("duplicate 'restrict' qualifier"));
                    }
                    quals.is_restrict = true;
                    self.advance();
                }
                _ => break,
            }
        }
        Ok(quals)
    }

    fn can_start_param(&self) -> bool {
        let Some(token) = self.tokens.get(self.index + 1) else {
            return false;
        };
        self.at_type_specifier(token)
            || matches!(token.kind, TokenKind::RParen | TokenKind::Ellipsis)
            || unsupported(token.kind).is_some()
    }

    /// `[]` is an unknown bound. Anything else must be an integer constant
    /// greater than zero. An identifier is a variable-length array.
    fn parse_array_len(&mut self) -> Result<Option<u64>, Diagnostic> {
        if self.current().kind == TokenKind::Star {
            return Err(self.error("variable-length arrays are not supported"));
        }
        if self.current().kind == TokenKind::KwStatic {
            return Err(self.error("static array sizes are not supported"));
        }
        if self.current().kind == TokenKind::RBrack {
            return Ok(None);
        }
        let expr = self.parse_assignment()?;
        match fold_array_bound(&expr) {
            Ok(len) => Ok(Some(len)),
            Err(err) => Err(self.error_at(expr.line(), expr.column(), bound_message(err))),
        }
    }

    /// Build the type by inserting each constructor next to the base, which is
    /// what makes `[]` and `()` bind tighter than `*`.
    fn derive(
        &self,
        base: CType,
        raw: &RawDecl,
        line: usize,
        column: usize,
    ) -> Result<Layer, Diagnostic> {
        let layer = if let CType::Func(func) = &base {
            Layer::Function {
                ret: func.ret.clone(),
                params: func
                    .params
                    .iter()
                    .map(|ty| Param {
                        name: None,
                        ty: ty.clone(),
                        line,
                        column,
                        slot: String::new(),
                        arg_reg: String::new(),
                    })
                    .collect(),
                prototype: true,
                variadic: func.variadic,
                introduced: HashMap::new(),
                introduced_names: HashMap::new(),
            }
        } else {
            Layer::Object(base)
        };
        self.derive_layer(layer, raw, line, column)
    }

    fn derive_layer(
        &self,
        base: Layer,
        raw: &RawDecl,
        line: usize,
        column: usize,
    ) -> Result<Layer, Diagnostic> {
        match raw {
            RawDecl::Named(_, _, _) | RawDecl::Abstract(_, _) => Ok(base),
            RawDecl::Pointer(quals, inner) => match base {
                Layer::Object(ty) => {
                    let ptr = CType::pointer(ty).with_quals(*quals);
                    self.derive_layer(Layer::Object(ptr), inner, line, column)
                }
                Layer::Function {
                    ret,
                    params,
                    variadic,
                    ..
                } => {
                    let func = CType::func(
                        ret.clone(),
                        params.iter().map(|param| param.ty.unqualified()).collect(),
                        variadic,
                    );
                    let ptr = CType::pointer(func).with_quals(*quals);
                    self.derive_layer(Layer::Object(ptr), inner, line, column)
                }
            },
            RawDecl::Array(inner, len) => match base {
                Layer::Object(ty) => {
                    let arr = self.make_array(ty, *len, line, column)?;
                    self.derive_layer(Layer::Object(arr), inner, line, column)
                }
                Layer::Function { .. } => {
                    Err(self.error_at(line, column, "array of functions is not supported"))
                }
            },
            RawDecl::Func(inner, params) => match base {
                Layer::Object(ret) => {
                    if ret.is_array() {
                        return Err(self.error_at(line, column, "function cannot return an array"));
                    }
                    if ret.is_func() {
                        return Err(self.error_at(
                            line,
                            column,
                            "function cannot return a function",
                        ));
                    }
                    if !params.prototype && !params.params.is_empty() {
                        return Err(self.error_at(
                            line,
                            column,
                            "old-style function declarations are not supported",
                        ));
                    }
                    self.derive_layer(
                        Layer::Function {
                            ret,
                            params: params.params.clone(),
                            prototype: params.prototype,
                            variadic: params.variadic,
                            introduced: params.introduced.clone(),
                            introduced_names: params.introduced_names.clone(),
                        },
                        inner,
                        line,
                        column,
                    )
                }
                Layer::Function { .. } => {
                    Err(self.error_at(line, column, "function cannot return a function"))
                }
            },
        }
    }

    fn make_array(
        &self,
        elem: CType,
        len: Option<u64>,
        line: usize,
        column: usize,
    ) -> Result<CType, Diagnostic> {
        if !elem.is_complete() {
            return Err(self.error_at(
                line,
                column,
                format!("array has incomplete element type '{}'", elem.display()),
            ));
        }
        if let Some(n) = len {
            if let Some(elem_size) = elem.size() {
                if elem_size != 0 && n > u64::MAX / elem_size {
                    return Err(self.error_at(line, column, "array is too large"));
                }
            }
        }
        Ok(CType::array(elem, len))
    }

    fn parse_compound(&mut self) -> Result<Compound, Diagnostic> {
        self.parse_compound_scoped(true)
    }

    /// `new_scope` is false for a function body, which shares its tag scope
    /// with the parameter list.
    fn parse_compound_scoped(&mut self, new_scope: bool) -> Result<Compound, Diagnostic> {
        let brace = self.expect(TokenKind::LBrace, "'{'")?;
        if new_scope {
            self.push_tags();
        }
        let parsed = self.parse_compound_items(brace.line, brace.column);
        if new_scope {
            self.pop_tags();
        }
        parsed
    }

    fn parse_compound_items(&mut self, line: usize, column: usize) -> Result<Compound, Diagnostic> {
        let mut items = Vec::new();
        while !matches!(self.current().kind, TokenKind::RBrace | TokenKind::Eof) {
            // `T:` is a label when `T` is a typedef (C11 6.2.3). A declaration
            // test would consume `T` as the type and then reject the colon.
            let is_label =
                self.current().kind == TokenKind::Ident && self.lookahead() == TokenKind::Colon;
            if !is_label && self.starts_declaration() {
                items.extend(self.parse_block_declaration(false)?);
            } else if let Some(message) = unsupported(self.current().kind) {
                return Err(self.error(message));
            } else {
                items.push(self.parse_statement()?);
            }
        }
        self.expect(TokenKind::RBrace, "'}'")?;
        Ok(Compound {
            items,
            line,
            column,
        })
    }

    /// An initializer needs a complete type, except an array whose length comes
    /// from the list (C11 6.7.9p3).
    fn reject_incomplete_initializer(
        &self,
        ty: &CType,
        has_init: bool,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        if !has_init || ty.is_complete() || matches!(ty, CType::Array(_, None)) {
            return Ok(());
        }
        Err(self.error_at(
            line,
            column,
            format!("variable has incomplete type '{}'", ty.display()),
        ))
    }

    /// Declarations inside a block, or in the header of a `for` loop.
    ///
    /// A `for` header may not declare a function, and it may not use `static`
    /// or `extern` (the declaration would not have automatic storage). A
    /// function declarator followed by `{` is a nested definition, which this
    /// subset rejects. A function declarator followed by `;` or `,` is a
    /// block-scope declaration; C11 6.2.2 treats it as `extern`.
    fn parse_block_declaration(&mut self, in_for: bool) -> Result<Vec<Stmt>, Diagnostic> {
        let specs = self.parse_specs(SpecPlace::Declaration)?;
        self.apply_shadow(&specs)?;
        if in_for {
            if let Some((line, column)) = specs.storage_at {
                return Err(self.error_at(
                    line,
                    column,
                    "a for-loop declaration cannot have a storage class",
                ));
            }
        }
        if self.current().kind == TokenKind::Semi {
            self.finish_tag_only(&specs)?;
            self.expect(TokenKind::Semi, "';'")?;
            return Ok(Vec::new());
        }
        if specs.storage == Storage::Typedef {
            self.parse_typedef_declarators(&specs)?;
            return Ok(Vec::new());
        }
        let base = specs.ty.clone();
        let mut items = Vec::new();
        loop {
            let declarator = self.parse_named_decl(base.clone())?;
            if declarator.is_function() {
                if self.current().kind == TokenKind::LBrace {
                    return Err(self.error_at(
                        declarator.line,
                        declarator.column,
                        "nested functions are not supported",
                    ));
                }
                if in_for {
                    return Err(self.error_at(
                        declarator.line,
                        declarator.column,
                        "a function declaration is not allowed in a for loop",
                    ));
                }
                items.push(Stmt::FuncDecl(self.finish_block_func(&specs, declarator)?));
            } else {
                items.push(Stmt::Decl(self.finish_block_object(&specs, declarator)?));
            }
            if self.match_kind(TokenKind::Comma) {
                continue;
            }
            break;
        }
        self.expect(TokenKind::Semi, "';'")?;
        Ok(items)
    }

    fn finish_block_func(
        &mut self,
        specs: &Specs,
        declarator: NamedDecl,
    ) -> Result<FuncDef, Diagnostic> {
        let DeclKind::Function {
            ret,
            params,
            prototype,
            variadic,
        } = declarator.kind
        else {
            unreachable!("function declarator");
        };
        if !prototype {
            return Err(self.error_at(
                declarator.line,
                declarator.column,
                "old-style function declarations are not supported",
            ));
        }
        if self.current().kind == TokenKind::Assign {
            return Err(self.error("function declarations cannot have an initializer"));
        }
        let symbol = declarator
            .symbol
            .clone()
            .unwrap_or_else(|| declarator.name.clone());
        self.bind_ordinary(&declarator.name, declarator.line, declarator.column)?;
        Ok(FuncDef {
            ret_type: ret,
            name: declarator.name,
            params,
            variadic,
            body: None,
            storage: specs.storage,
            linkage: Linkage::External,
            symbol,
            line: declarator.line,
            column: declarator.column,
        })
    }

    fn finish_block_object(
        &mut self,
        specs: &Specs,
        declarator: NamedDecl,
    ) -> Result<VarDecl, Diagnostic> {
        let DeclKind::Object(ty) = declarator.kind else {
            unreachable!("object declarator");
        };
        if ty.is_void() {
            return Err(self.error_at(
                declarator.line,
                declarator.column,
                "void variables are not supported",
            ));
        }
        let init = if self.match_kind(TokenKind::Assign) {
            Some(self.parse_initializer()?)
        } else {
            None
        };
        self.reject_incomplete_initializer(
            &ty,
            init.is_some(),
            declarator.line,
            declarator.column,
        )?;
        self.reject_bad_restrict(&ty, declarator.line, declarator.column)?;
        self.bind_ordinary(&declarator.name, declarator.line, declarator.column)?;
        Ok(VarDecl {
            name: declarator.name,
            ty,
            storage: specs.storage,
            init,
            checked: None,
            line: declarator.line,
            column: declarator.column,
            slot: String::new(),
        })
    }

    fn parse_statement(&mut self) -> Result<Stmt, Diagnostic> {
        // `name:` is a label even when `name` is a typedef. The check comes
        // before the declaration test so `typedef int T; T: ;` is a label.
        if self.current().kind == TokenKind::Ident && self.lookahead() == TokenKind::Colon {
            return self.parse_label();
        }
        // A label's statement is parsed here, including `case 1: int x;`.
        // A declaration is not a statement in C11 (C23 allows the label).
        if self.starts_declaration() {
            return Err(self.error("a declaration is not allowed as a statement"));
        }
        match self.current().kind {
            TokenKind::LBrace => Ok(Stmt::Compound(self.parse_compound()?)),
            TokenKind::KwReturn => self.parse_return(),
            TokenKind::KwIf => self.parse_if(),
            TokenKind::KwWhile => self.parse_while(),
            TokenKind::KwDo => self.parse_do(),
            TokenKind::KwFor => self.parse_for(),
            TokenKind::KwSwitch => self.parse_switch(),
            TokenKind::KwCase => self.parse_case(),
            TokenKind::KwDefault => self.parse_default(),
            TokenKind::KwGoto => self.parse_goto(),
            TokenKind::KwBreak => {
                let token = self.advance();
                self.expect(TokenKind::Semi, "';'")?;
                Ok(Stmt::Break {
                    line: token.line,
                    column: token.column,
                })
            }
            TokenKind::KwContinue => {
                let token = self.advance();
                self.expect(TokenKind::Semi, "';'")?;
                Ok(Stmt::Continue {
                    line: token.line,
                    column: token.column,
                })
            }
            TokenKind::KwElse => Err(self.error("'else' without a matching 'if'")),
            TokenKind::Semi => {
                let token = self.advance();
                Ok(Stmt::Expr {
                    expr: None,
                    line: token.line,
                    column: token.column,
                })
            }
            kind => {
                if let Some(message) = unsupported(kind) {
                    return Err(self.error(message));
                }
                let line = self.current().line;
                let column = self.current().column;
                let expr = self.parse_expression()?;
                self.expect(TokenKind::Semi, "';'")?;
                Ok(Stmt::Expr {
                    expr: Some(expr),
                    line,
                    column,
                })
            }
        }
    }

    fn parse_label(&mut self) -> Result<Stmt, Diagnostic> {
        let name = self.advance();
        self.expect(TokenKind::Colon, "':'")?;
        let stmt = Box::new(self.parse_statement()?);
        Ok(Stmt::Label {
            name: name.spelling,
            stmt,
            line: name.line,
            column: name.column,
        })
    }

    fn parse_goto(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        if self.current().kind == TokenKind::Star {
            return Err(self.error("computed goto is not supported"));
        }
        if self.current().kind != TokenKind::Ident {
            return Err(self.error("expected a label name"));
        }
        let name = self.advance();
        self.expect(TokenKind::Semi, "';'")?;
        Ok(Stmt::Goto {
            name: name.spelling,
            line: token.line,
            column: token.column,
        })
    }

    fn parse_return(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        let expr = if self.current().kind == TokenKind::Semi {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(TokenKind::Semi, "';'")?;
        Ok(Stmt::Return {
            expr,
            line: token.line,
            column: token.column,
        })
    }

    fn parse_if(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        self.expect(TokenKind::LParen, "'('")?;
        let cond = self.parse_expression()?;
        self.expect(TokenKind::RParen, "')'")?;
        let then_stmt = Box::new(self.parse_statement()?);
        let else_stmt = if self.match_kind(TokenKind::KwElse) {
            Some(Box::new(self.parse_statement()?))
        } else {
            None
        };
        Ok(Stmt::If {
            cond,
            then_stmt,
            else_stmt,
            line: token.line,
            column: token.column,
        })
    }

    fn parse_while(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        self.expect(TokenKind::LParen, "'('")?;
        let cond = self.parse_expression()?;
        self.expect(TokenKind::RParen, "')'")?;
        let body = Box::new(self.parse_statement()?);
        Ok(Stmt::While {
            cond,
            body,
            line: token.line,
            column: token.column,
        })
    }

    fn parse_do(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        let body = Box::new(self.parse_statement()?);
        self.expect(TokenKind::KwWhile, "'while'")?;
        self.expect(TokenKind::LParen, "'('")?;
        let cond = self.parse_expression()?;
        self.expect(TokenKind::RParen, "')'")?;
        self.expect(TokenKind::Semi, "';'")?;
        Ok(Stmt::Do {
            body,
            cond,
            line: token.line,
            column: token.column,
        })
    }

    fn parse_for(&mut self) -> Result<Stmt, Diagnostic> {
        self.push_tags();
        let parsed = self.parse_for_in_scope();
        self.pop_tags();
        parsed
    }

    fn parse_for_in_scope(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        self.expect(TokenKind::LParen, "'('")?;
        let init = if self.match_kind(TokenKind::Semi) {
            ForInit::Empty
        } else if self.starts_declaration() {
            let mut decls = Vec::new();
            for item in self.parse_block_declaration(true)? {
                match item {
                    Stmt::Decl(decl) => decls.push(decl),
                    Stmt::FuncDecl(_) => {
                        unreachable!("a function declaration in a for loop is rejected")
                    }
                    _ => unreachable!("a block declaration is a variable or a function"),
                }
            }
            ForInit::Decl(decls)
        } else if let Some(message) = unsupported(self.current().kind) {
            return Err(self.error(message));
        } else {
            let expr = self.parse_expression()?;
            self.expect(TokenKind::Semi, "';'")?;
            ForInit::Expr(expr)
        };
        let cond = if self.current().kind == TokenKind::Semi {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(TokenKind::Semi, "';'")?;
        let step = if self.current().kind == TokenKind::RParen {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(TokenKind::RParen, "')'")?;
        let body = Box::new(self.parse_statement()?);
        Ok(Stmt::For {
            init,
            cond,
            step,
            body,
            line: token.line,
            column: token.column,
        })
    }

    /// `switch (` expression `)` statement. The controlling expression is a
    /// full expression, so it may contain a comma.
    fn parse_switch(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        self.expect(TokenKind::LParen, "'('")?;
        let cond = self.parse_expression()?;
        self.expect(TokenKind::RParen, "')'")?;
        let body = Box::new(self.parse_statement()?);
        Ok(Stmt::Switch {
            cond,
            body,
            promoted: None,
            line: token.line,
            column: token.column,
        })
    }

    /// `case` constant-expression `:` statement.
    ///
    /// The constant is a conditional-expression, not an assignment and not a
    /// comma expression (C11 6.6). `case 1: case 2: stmt` is one labeled
    /// statement whose statement is another labeled statement.
    fn parse_case(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        let expr = self.parse_conditional()?;
        self.expect(TokenKind::Colon, "':'")?;
        let stmt = Box::new(self.parse_statement()?);
        Ok(Stmt::Case {
            expr,
            value: None,
            stmt,
            line: token.line,
            column: token.column,
        })
    }

    fn parse_default(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.advance();
        self.expect(TokenKind::Colon, "':'")?;
        let stmt = Box::new(self.parse_statement()?);
        Ok(Stmt::Default {
            stmt,
            line: token.line,
            column: token.column,
        })
    }

    /// Comma operator. Argument lists and declarators call
    /// [`Self::parse_assignment`] instead, so those commas stay separators.
    fn parse_expression(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_assignment()?;
        while self.current().kind == TokenKind::Comma {
            let op = self.advance();
            let right = self.parse_assignment()?;
            left = Expr::comma(left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_assignment(&mut self) -> Result<Expr, Diagnostic> {
        let left = self.parse_conditional()?;
        let Some(kind) = assign_kind(self.current().kind) else {
            return Ok(left);
        };
        self.advance();
        let line = left.line();
        let column = left.column();
        let value = self.parse_assignment()?;
        Ok(match kind {
            AssignKind::Plain => Expr::assign(left, value, line, column),
            AssignKind::Compound(binop) => Expr::compound(binop, left, value, line, column),
        })
    }

    /// `?:` is right associative. The middle operand is a full expression,
    /// so it may contain a comma; the third operand may not.
    fn parse_conditional(&mut self) -> Result<Expr, Diagnostic> {
        let cond = self.parse_logical_or()?;
        if !self.match_kind(TokenKind::Question) {
            return Ok(cond);
        }
        let line = cond.line();
        let column = cond.column();
        let then_expr = self.parse_expression()?;
        self.expect(TokenKind::Colon, "':'")?;
        let else_expr = self.parse_conditional()?;
        Ok(Expr::cond(cond, then_expr, else_expr, line, column))
    }

    fn parse_logical_or(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_logical_and()?;
        while self.current().kind == TokenKind::OrOr {
            let op = self.advance();
            let right = self.parse_logical_and()?;
            left = Expr::bin(BinOp::Or, left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_logical_and(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_bit_or()?;
        while self.current().kind == TokenKind::AndAnd {
            let op = self.advance();
            let right = self.parse_bit_or()?;
            left = Expr::bin(BinOp::And, left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_bit_or(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_bit_xor()?;
        while self.current().kind == TokenKind::Pipe {
            let op = self.advance();
            let right = self.parse_bit_xor()?;
            left = Expr::bin(BinOp::BitOr, left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_bit_xor(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_bit_and()?;
        while self.current().kind == TokenKind::Caret {
            let op = self.advance();
            let right = self.parse_bit_and()?;
            left = Expr::bin(BinOp::BitXor, left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_bit_and(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_equality()?;
        while self.current().kind == TokenKind::Amp {
            let op = self.advance();
            let right = self.parse_equality()?;
            left = Expr::bin(BinOp::BitAnd, left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_relational()?;
        while matches!(self.current().kind, TokenKind::Eq | TokenKind::Ne) {
            let op = self.advance();
            let kind = if op.kind == TokenKind::Eq {
                BinOp::Eq
            } else {
                BinOp::Ne
            };
            let right = self.parse_relational()?;
            left = Expr::bin(kind, left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_relational(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_shift()?;
        while matches!(
            self.current().kind,
            TokenKind::Lt | TokenKind::Gt | TokenKind::Le | TokenKind::Ge
        ) {
            let op = self.advance();
            let kind = match op.kind {
                TokenKind::Lt => BinOp::Lt,
                TokenKind::Gt => BinOp::Gt,
                TokenKind::Le => BinOp::Le,
                _ => BinOp::Ge,
            };
            let right = self.parse_shift()?;
            left = Expr::bin(kind, left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_shift(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_additive()?;
        while matches!(self.current().kind, TokenKind::Shl | TokenKind::Shr) {
            let op = self.advance();
            let kind = if op.kind == TokenKind::Shl {
                BinOp::Shl
            } else {
                BinOp::Shr
            };
            let right = self.parse_additive()?;
            left = Expr::bin(kind, left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_multiplicative()?;
        while matches!(self.current().kind, TokenKind::Plus | TokenKind::Minus) {
            let op = self.advance();
            let kind = if op.kind == TokenKind::Plus {
                BinOp::Add
            } else {
                BinOp::Sub
            };
            let right = self.parse_multiplicative()?;
            left = Expr::bin(kind, left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_unary()?;
        while matches!(
            self.current().kind,
            TokenKind::Star | TokenKind::Slash | TokenKind::Percent
        ) {
            let op = self.advance();
            let kind = match op.kind {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                _ => BinOp::Mod,
            };
            let right = self.parse_unary()?;
            left = Expr::bin(kind, left, right, op.line, op.column);
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        if self.spec_word(&["__extension__"]) {
            self.advance();
            return self.parse_unary();
        }
        let token = self.current().clone();
        match token.kind {
            TokenKind::LParen if self.lookahead_is_type() => self.parse_cast(),
            TokenKind::Plus | TokenKind::Minus | TokenKind::Bang | TokenKind::Tilde => {
                self.advance();
                let op = match token.kind {
                    TokenKind::Plus => UnaryOp::Plus,
                    TokenKind::Minus => UnaryOp::Minus,
                    TokenKind::Bang => UnaryOp::Not,
                    _ => UnaryOp::BitNot,
                };
                let expr = self.parse_unary()?;
                Ok(Expr::unary(op, expr, token.line, token.column))
            }
            TokenKind::PlusPlus | TokenKind::MinusMinus => {
                self.advance();
                let op = if token.kind == TokenKind::PlusPlus {
                    IncOp::PreInc
                } else {
                    IncOp::PreDec
                };
                let expr = self.parse_unary()?;
                Ok(Expr::incdec(op, expr, token.line, token.column))
            }
            TokenKind::KwSizeof => self.parse_sizeof(token.line, token.column),
            TokenKind::Amp => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::unary(UnaryOp::Addr, expr, token.line, token.column))
            }
            TokenKind::Star => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::unary(UnaryOp::Deref, expr, token.line, token.column))
            }
            _ => self.parse_postfix(),
        }
    }

    /// `=` initializer: an assignment expression, or a brace list. A trailing
    /// comma is allowed. An empty list is not (C11). Designators (`.name` and
    /// `[index]`) are only valid inside a brace list.
    fn parse_initializer(&mut self) -> Result<Init, Diagnostic> {
        if self.current().kind != TokenKind::LBrace {
            return Ok(Init::Expr(self.parse_assignment()?));
        }
        let brace = self.advance();
        if self.current().kind == TokenKind::RBrace {
            return Err(self.error("empty initializer list"));
        }
        let mut items = Vec::new();
        loop {
            items.push(self.parse_init_item()?);
            if self.match_kind(TokenKind::Comma) {
                if self.current().kind == TokenKind::RBrace {
                    break;
                }
                continue;
            }
            break;
        }
        self.expect(TokenKind::RBrace, "'}'")?;
        Ok(Init::List {
            items,
            line: brace.line,
            column: brace.column,
        })
    }

    fn parse_init_item(&mut self) -> Result<Init, Diagnostic> {
        if !matches!(self.current().kind, TokenKind::Dot | TokenKind::LBrack) {
            return self.parse_initializer();
        }
        let line = self.current().line;
        let column = self.current().column;
        let mut designators = Vec::new();
        while matches!(self.current().kind, TokenKind::Dot | TokenKind::LBrack) {
            designators.push(self.parse_designator()?);
        }
        self.expect(TokenKind::Assign, "'='")?;
        let init = self.parse_initializer()?;
        Ok(Init::Designated {
            designators,
            init: Box::new(init),
            line,
            column,
        })
    }

    fn parse_designator(&mut self) -> Result<Designator, Diagnostic> {
        if self.match_kind(TokenKind::Dot) {
            if self.current().kind != TokenKind::Ident {
                return Err(self.error("expected a member name"));
            }
            let name = self.advance();
            return Ok(Designator::Field {
                name: name.spelling,
                line: name.line,
                column: name.column,
            });
        }
        let bracket = self.advance();
        let expr = self.parse_conditional()?;
        if self.current().kind == TokenKind::Ellipsis {
            return Err(self.error("range designators are not supported"));
        }
        self.expect(TokenKind::RBrack, "']'")?;
        Ok(Designator::Index {
            expr,
            line: bracket.line,
            column: bracket.column,
        })
    }

    /// `sizeof (type-name)` when the parenthesis starts a type. Otherwise
    /// `sizeof unary-expression`. The operand is not evaluated later.
    fn parse_sizeof(&mut self, line: usize, column: usize) -> Result<Expr, Diagnostic> {
        self.advance();
        if self.current().kind == TokenKind::LParen && self.lookahead_is_type() {
            self.advance();
            let ty = self.parse_type_name()?;
            self.expect(TokenKind::RParen, "')'")?;
            return Ok(Expr::sizeof_type(ty, line, column));
        }
        let expr = self.parse_unary()?;
        Ok(Expr::sizeof_expr(expr, line, column))
    }

    fn parse_cast(&mut self) -> Result<Expr, Diagnostic> {
        let lparen = self.advance();
        let ty = self.parse_type_name()?;
        self.expect(TokenKind::RParen, "')'")?;
        let expr = self.parse_unary()?;
        Ok(Expr::cast(ty, expr, lparen.line, lparen.column))
    }

    /// A type name is specifiers plus an optional abstract declarator.
    /// `int *`, `int (*)[3]`, and `int [3]` are accepted. A function type or a
    /// function pointer is rejected.
    fn parse_type_name(&mut self) -> Result<CType, Diagnostic> {
        let specs = self.parse_specs(SpecPlace::TypeName)?;
        let base = specs.ty.clone();
        if !self.starts_raw_declarator() {
            return Ok(base);
        }
        let raw = self.parse_raw_declarator(true)?;
        if let Some((_, line, column)) = decl_name(&raw) {
            return Err(self.error_at(line, column, "type name cannot declare an identifier"));
        }
        let (line, column) = decl_pos(&raw);
        match self.derive(base, &raw, line, column)? {
            Layer::Object(ty) => Ok(ty),
            Layer::Function {
                ret,
                params,
                variadic,
                ..
            } => Ok(CType::func(
                ret,
                params.iter().map(|param| param.ty.unqualified()).collect(),
                variadic,
            )),
        }
    }

    fn lookahead_is_type(&self) -> bool {
        let Some(token) = self.tokens.get(self.index + 1) else {
            return false;
        };
        self.at_type_specifier(token)
            || matches!(token.kind, TokenKind::KwConst | TokenKind::KwVolatile)
    }

    fn parse_builtin_va_arg(&mut self, line: usize, column: usize) -> Result<Expr, Diagnostic> {
        self.advance();
        let ap = self.parse_assignment()?;
        self.expect(TokenKind::Comma, "','")?;
        let type_arg = self.parse_type_name()?;
        self.expect(TokenKind::RParen, "')'")?;
        Ok(Expr::va_arg(ap, type_arg, line, column))
    }

    fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.current().kind {
                TokenKind::LParen => {
                    if let Expr::Var {
                        name, line, column, ..
                    } = &expr
                    {
                        if name == "__builtin_va_arg" {
                            let line = *line;
                            let column = *column;
                            expr = self.parse_builtin_va_arg(line, column)?;
                            continue;
                        }
                    }
                    let line = expr.line();
                    let column = expr.column();
                    let builtin = match &expr {
                        Expr::Var { name, .. } => match name.as_str() {
                            "__builtin_va_start" => Some(VaOp::Start),
                            "__builtin_va_end" => Some(VaOp::End),
                            "__builtin_va_copy" => Some(VaOp::Copy),
                            _ => None,
                        },
                        _ => None,
                    };
                    self.advance();
                    let mut args = Vec::new();
                    if self.current().kind != TokenKind::RParen {
                        loop {
                            args.push(self.parse_assignment()?);
                            if !self.match_kind(TokenKind::Comma) {
                                break;
                            }
                            if self.current().kind == TokenKind::RParen {
                                return Err(self
                                    .error("trailing commas in argument lists are not supported"));
                            }
                        }
                    }
                    self.expect(TokenKind::RParen, "')'")?;
                    expr = if let Some(op) = builtin {
                        Expr::va_op(op, args, line, column)
                    } else {
                        Expr::call(expr, args, line, column)
                    };
                }
                TokenKind::LBrack => {
                    let bracket = self.advance();
                    let index = self.parse_expression()?;
                    self.expect(TokenKind::RBrack, "']'")?;
                    expr = Expr::index(expr, index, bracket.line, bracket.column);
                }
                TokenKind::PlusPlus | TokenKind::MinusMinus => {
                    let op_token = self.advance();
                    let op = if op_token.kind == TokenKind::PlusPlus {
                        IncOp::PostInc
                    } else {
                        IncOp::PostDec
                    };
                    expr = Expr::incdec(op, expr, op_token.line, op_token.column);
                }
                TokenKind::Arrow | TokenKind::Dot => {
                    let arrow = self.current().kind == TokenKind::Arrow;
                    let op = self.advance();
                    if self.current().kind != TokenKind::Ident {
                        return Err(self.error("expected a member name"));
                    }
                    let name = self.advance();
                    expr = Expr::member(expr, name.spelling, arrow, op.line, op.column);
                }
                TokenKind::Ellipsis => {
                    return Err(self.error("unexpected '...'"));
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.current().clone();
        match token.kind {
            TokenKind::Float => {
                self.advance();
                Ok(Expr::float_lit(
                    (u128::from(token.float_hi) << 64)
                        | u128::from(token.value.expect("float bits")),
                    token.float_kind.expect("float kind"),
                    token.line,
                    token.column,
                ))
            }
            TokenKind::Int => {
                self.advance();
                Ok(Expr::int(
                    token.value.expect("int token"),
                    token.int_ty.expect("int token type"),
                    token.line,
                    token.column,
                ))
            }
            TokenKind::Ident => {
                self.advance();
                if let Some(NameBinding::EnumConst(value)) = self.lookup_name(&token.spelling) {
                    return Ok(Expr::int(
                        value as u32 as u64,
                        TypeName::Int,
                        token.line,
                        token.column,
                    ));
                }
                if self.is_typedef(&token.spelling) {
                    return Err(self.error_at(
                        token.line,
                        token.column,
                        format!("typedef name '{}' used as a value", token.spelling),
                    ));
                }
                Ok(Expr::var(token.spelling, token.line, token.column))
            }
            TokenKind::String => {
                self.advance();
                let wide = token.spelling.starts_with('L');
                let mut bytes = token.bytes.expect("string token");
                // Translation phase 6 concatenates adjacent string literals.
                // The encoding prefixes have to agree.
                while self.current().kind == TokenKind::String {
                    let next = self.advance();
                    if next.spelling.starts_with('L') != wide {
                        return Err(
                            self.error("cannot concatenate wide and ordinary string literals")
                        );
                    }
                    bytes.extend(next.bytes.expect("string token"));
                }
                Ok(Expr::str_lit(bytes, wide, token.line, token.column))
            }
            TokenKind::LParen => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect(TokenKind::RParen, "')'")?;
                Ok(expr)
            }
            kind => {
                if let Some(message) = unsupported(kind) {
                    return Err(self.error(message));
                }
                Err(self.error(format!("expected an expression, found {}", found(&token))))
            }
        }
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
        if token.kind != TokenKind::Eof && self.index + 1 < self.tokens.len() {
            self.index += 1;
        }
        token
    }

    fn match_kind(&mut self, kind: TokenKind) -> bool {
        if self.current().kind == kind {
            self.advance();
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

struct Specs {
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
    /// See [`Parser::shadow_decl`].
    shadow_decl: Option<(String, RecordKind)>,
    line: usize,
    column: usize,
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
    float_ty: Option<FloatKind>,
    quals: Quals,
    struct_ty: Option<CType>,
    forward_tag: Option<String>,
    /// `enum { ... }` or `enum Tag { ... }` with no declarator is a declaration.
    introduced_tag: bool,
    anon_record: bool,
    any: bool,
    line: usize,
    column: usize,
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

fn decl_name(raw: &RawDecl) -> Option<(String, usize, usize)> {
    match raw {
        RawDecl::Named(name, line, column) => Some((name.clone(), *line, *column)),
        RawDecl::Abstract(_, _) => None,
        RawDecl::Pointer(_, inner) | RawDecl::Array(inner, _) | RawDecl::Func(inner, _) => {
            decl_name(inner)
        }
    }
}

fn decl_pos(raw: &RawDecl) -> (usize, usize) {
    match raw {
        RawDecl::Named(_, line, column) | RawDecl::Abstract(line, column) => (*line, *column),
        RawDecl::Pointer(_, inner) | RawDecl::Array(inner, _) | RawDecl::Func(inner, _) => {
            decl_pos(inner)
        }
    }
}

enum BoundErr {
    Vla,
    Incomplete,
    NonPositive,
    TooBig,
    DivZero,
    Overflow,
    Shift,
    NotConst,
}

fn bound_message(err: BoundErr) -> &'static str {
    match err {
        BoundErr::Vla => "variable-length arrays are not supported",
        BoundErr::Incomplete => "array size uses an incomplete type",
        BoundErr::NonPositive => "array size must be positive",
        BoundErr::TooBig => "array is too large",
        BoundErr::DivZero => "array size divides by zero",
        BoundErr::Overflow => "array size overflows",
        BoundErr::Shift => "array size uses an undefined shift",
        BoundErr::NotConst => "array size is not an integer constant",
    }
}

struct BVal {
    bits: u64,
    ty: TypeName,
}

/// Integer constant expression used as an array bound. An identifier, a call,
/// or an assignment is reported as a variable-length array.
fn fold_array_bound(expr: &Expr) -> Result<u64, BoundErr> {
    let value = eval_ice(expr)?;
    let count = if value.ty.is_signed() {
        let signed = sign_extend(value.bits, value.ty.width());
        if signed <= 0 {
            return Err(BoundErr::NonPositive);
        }
        signed as u64
    } else {
        let unsigned = value.bits & value.ty.mask();
        if unsigned == 0 {
            return Err(BoundErr::NonPositive);
        }
        unsigned
    };
    if count > u64::MAX / 8 {
        return Err(BoundErr::TooBig);
    }
    Ok(count)
}

fn eval_ice(expr: &Expr) -> Result<BVal, BoundErr> {
    match expr {
        Expr::Int { bits, value_ty, .. } => Ok(BVal {
            bits: *bits & value_ty.mask(),
            ty: *value_ty,
        }),
        Expr::Float { .. } => Err(BoundErr::NotConst),
        Expr::Sizeof { operand, .. } => {
            let size = match operand {
                SizeofOperand::Type(ty) => ty.size().ok_or(BoundErr::Incomplete)?,
                SizeofOperand::Expr(inner) => {
                    type_only(inner)?.size().ok_or(BoundErr::Incomplete)?
                }
            };
            Ok(BVal {
                bits: size,
                ty: TypeName::ULong,
            })
        }
        Expr::Unary { op, expr, .. } => eval_ice_unary(*op, expr),
        Expr::Bin {
            op, left, right, ..
        } => eval_ice_bin(*op, left, right),
        Expr::Cond {
            cond,
            then_expr,
            else_expr,
            ..
        } => {
            let cond = eval_ice(cond)?;
            let chosen = if cond.bits & cond.ty.mask() == 0 {
                else_expr
            } else {
                then_expr
            };
            let value = eval_ice(chosen)?;
            let result = match (type_only(then_expr)?, type_only(else_expr)?) {
                (left, right) if left.is_integer() && right.is_integer() => left
                    .as_integer()
                    .unwrap()
                    .usual(right.as_integer().unwrap()),
                _ => return Err(BoundErr::Vla),
            };
            Ok(BVal {
                bits: convert_bits(value.bits, value.ty, result),
                ty: result,
            })
        }
        Expr::Cast { ty_name, expr, .. } => {
            let dest = ty_name.as_integer().ok_or(BoundErr::Vla)?;
            // C11 6.6 allows a floating constant only as the immediate
            // operand of a cast to an integer type. `(int)3.5` is an
            // integer constant; `1.5` and `(int)(1.5 + 2.5)` are not.
            if let Expr::Float { bits, kind, .. } = expr.as_ref() {
                let bits =
                    crate::float::real_to_int(*bits, *kind, dest).ok_or(BoundErr::Overflow)?;
                return Ok(BVal { bits, ty: dest });
            }
            let value = eval_ice(expr)?;
            Ok(BVal {
                bits: convert_bits(value.bits, value.ty, dest),
                ty: dest,
            })
        }
        Expr::Str { .. } => Err(BoundErr::NotConst),
        Expr::Var { .. }
        | Expr::Assign { .. }
        | Expr::Call { .. }
        | Expr::CompoundAssign { .. }
        | Expr::IncDec { .. }
        | Expr::Comma { .. }
        | Expr::Index { .. }
        | Expr::Decay { .. }
        | Expr::Member { .. }
        | Expr::VaArg { .. }
        | Expr::VaOp { .. } => Err(BoundErr::Vla),
    }
}

fn type_only(expr: &Expr) -> Result<CType, BoundErr> {
    match expr {
        Expr::Int { value_ty, .. } => Ok(CType::from_name(*value_ty)),
        Expr::Float { kind, .. } => Ok(CType::Float(*kind)),
        Expr::Str { bytes, wide, .. } => {
            let elem = if *wide { TypeName::Int } else { TypeName::Char };
            let len = if *wide {
                bytes.len() as u64 / 4 + 1
            } else {
                bytes.len() as u64 + 1
            };
            Ok(CType::array(CType::from_name(elem), Some(len)))
        }
        Expr::Sizeof { .. } => Ok(CType::from_name(TypeName::ULong)),
        Expr::Unary { op, expr, .. } => {
            let inner = type_only(expr)?.as_integer().ok_or(BoundErr::Vla)?;
            let ty = match op {
                UnaryOp::Not => TypeName::Int,
                UnaryOp::Plus | UnaryOp::Minus | UnaryOp::BitNot => inner.promote(),
                UnaryOp::Addr | UnaryOp::Deref => return Err(BoundErr::Vla),
            };
            Ok(CType::from_name(ty))
        }
        Expr::Bin {
            op, left, right, ..
        } => {
            let left = type_only(left)?.as_integer().ok_or(BoundErr::Vla)?;
            let right = type_only(right)?.as_integer().ok_or(BoundErr::Vla)?;
            let ty = if op.is_cmp() || matches!(op, BinOp::And | BinOp::Or) {
                TypeName::Int
            } else if matches!(op, BinOp::Shl | BinOp::Shr) {
                left.promote()
            } else {
                left.usual(right)
            };
            Ok(CType::from_name(ty))
        }
        Expr::Cast { ty_name, .. } => {
            if ty_name.is_integer() {
                Ok(ty_name.clone())
            } else {
                Err(BoundErr::Vla)
            }
        }
        Expr::Cond {
            then_expr,
            else_expr,
            ..
        } => {
            let left = type_only(then_expr)?.as_integer().ok_or(BoundErr::Vla)?;
            let right = type_only(else_expr)?.as_integer().ok_or(BoundErr::Vla)?;
            Ok(CType::from_name(left.usual(right)))
        }
        _ => Err(BoundErr::Vla),
    }
}

fn eval_ice_unary(op: UnaryOp, expr: &Expr) -> Result<BVal, BoundErr> {
    let value = promote_ice(eval_ice(expr)?);
    match op {
        UnaryOp::Plus => Ok(value),
        UnaryOp::Minus => {
            if value.ty.is_signed() {
                let signed = sign_extend(value.bits, value.ty.width()) as i128;
                bits_ice(signed.checked_neg().ok_or(BoundErr::Overflow)?, value.ty)
            } else {
                Ok(BVal {
                    bits: value.bits.wrapping_neg() & value.ty.mask(),
                    ty: value.ty,
                })
            }
        }
        UnaryOp::Not => {
            let zero = value.bits & value.ty.mask() == 0;
            Ok(BVal {
                bits: u64::from(!zero),
                ty: TypeName::Int,
            })
        }
        UnaryOp::BitNot => Ok(BVal {
            bits: (!value.bits) & value.ty.mask(),
            ty: value.ty,
        }),
        UnaryOp::Addr | UnaryOp::Deref => Err(BoundErr::Vla),
    }
}

fn promote_ice(value: BVal) -> BVal {
    let ty = value.ty.promote();
    BVal {
        bits: convert_bits(value.bits, value.ty, ty),
        ty,
    }
}

fn bits_ice(value: i128, ty: TypeName) -> Result<BVal, BoundErr> {
    let width = ty.width();
    let min = -((1i128) << (width - 1));
    let max = ((1i128) << (width - 1)) - 1;
    if value < min || value > max {
        return Err(BoundErr::Overflow);
    }
    Ok(BVal {
        bits: (value as u64) & ty.mask(),
        ty,
    })
}

fn eval_ice_bin(op: BinOp, left: &Expr, right: &Expr) -> Result<BVal, BoundErr> {
    match op {
        BinOp::And => {
            let left = eval_ice(left)?;
            if left.bits & left.ty.mask() == 0 {
                return Ok(BVal {
                    bits: 0,
                    ty: TypeName::Int,
                });
            }
            let right = eval_ice(right)?;
            Ok(BVal {
                bits: u64::from(right.bits & right.ty.mask() != 0),
                ty: TypeName::Int,
            })
        }
        BinOp::Or => {
            let left = eval_ice(left)?;
            if left.bits & left.ty.mask() != 0 {
                return Ok(BVal {
                    bits: 1,
                    ty: TypeName::Int,
                });
            }
            let right = eval_ice(right)?;
            Ok(BVal {
                bits: u64::from(right.bits & right.ty.mask() != 0),
                ty: TypeName::Int,
            })
        }
        BinOp::Shl | BinOp::Shr => {
            let left = promote_ice(eval_ice(left)?);
            let right = promote_ice(eval_ice(right)?);
            let count = ice_shift_count(&right, left.ty.width())?;
            if op == BinOp::Shr {
                let bits = if left.ty.is_signed() {
                    (sign_extend(left.bits, left.ty.width()) >> count) as u64 & left.ty.mask()
                } else {
                    (left.bits & left.ty.mask()) >> count
                };
                return Ok(BVal { bits, ty: left.ty });
            }
            if left.ty.is_signed() {
                let signed = sign_extend(left.bits, left.ty.width()) as i128;
                if signed < 0 {
                    return Err(BoundErr::Shift);
                }
                bits_ice(signed << count, left.ty)
            } else {
                Ok(BVal {
                    bits: ((left.bits & left.ty.mask()) << count) & left.ty.mask(),
                    ty: left.ty,
                })
            }
        }
        _ => {
            let left = eval_ice(left)?;
            let right = eval_ice(right)?;
            let ty = left.ty.usual(right.ty);
            let left = convert_bits(left.bits, left.ty, ty);
            let right = convert_bits(right.bits, right.ty, ty);
            if op.is_cmp() {
                let bit = ice_compare(op, left, right, ty);
                return Ok(BVal {
                    bits: u64::from(bit),
                    ty: TypeName::Int,
                });
            }
            ice_arith(op, left, right, ty)
        }
    }
}

fn ice_shift_count(count: &BVal, width: u32) -> Result<u32, BoundErr> {
    if count.ty.is_signed() {
        let signed = sign_extend(count.bits, count.ty.width());
        if signed < 0 || signed >= i64::from(width) {
            return Err(BoundErr::Shift);
        }
        Ok(signed as u32)
    } else {
        let unsigned = count.bits & count.ty.mask();
        if unsigned >= u64::from(width) {
            return Err(BoundErr::Shift);
        }
        Ok(unsigned as u32)
    }
}

fn ice_compare(op: BinOp, left: u64, right: u64, ty: TypeName) -> bool {
    if ty.is_signed() {
        let left = sign_extend(left, ty.width());
        let right = sign_extend(right, ty.width());
        match op {
            BinOp::Eq => left == right,
            BinOp::Ne => left != right,
            BinOp::Lt => left < right,
            BinOp::Gt => left > right,
            BinOp::Le => left <= right,
            BinOp::Ge => left >= right,
            _ => false,
        }
    } else {
        let left = left & ty.mask();
        let right = right & ty.mask();
        match op {
            BinOp::Eq => left == right,
            BinOp::Ne => left != right,
            BinOp::Lt => left < right,
            BinOp::Gt => left > right,
            BinOp::Le => left <= right,
            BinOp::Ge => left >= right,
            _ => false,
        }
    }
}

fn ice_arith(op: BinOp, left: u64, right: u64, ty: TypeName) -> Result<BVal, BoundErr> {
    if ty.is_signed() {
        let left = sign_extend(left, ty.width()) as i128;
        let right = sign_extend(right, ty.width()) as i128;
        if matches!(op, BinOp::Div | BinOp::Mod) && right == 0 {
            return Err(BoundErr::DivZero);
        }
        if matches!(op, BinOp::Div | BinOp::Mod)
            && left == -((1i128) << (ty.width() - 1))
            && right == -1
        {
            return Err(BoundErr::Overflow);
        }
        let value = match op {
            BinOp::Add => left.checked_add(right),
            BinOp::Sub => left.checked_sub(right),
            BinOp::Mul => left.checked_mul(right),
            BinOp::Div => Some(left / right),
            BinOp::Mod => Some(left % right),
            BinOp::BitAnd => Some(left & right),
            BinOp::BitOr => Some(left | right),
            BinOp::BitXor => Some(left ^ right),
            _ => return Err(BoundErr::Vla),
        };
        bits_ice(value.ok_or(BoundErr::Overflow)?, ty)
    } else {
        let mask = ty.mask();
        let left = left & mask;
        let right = right & mask;
        if matches!(op, BinOp::Div | BinOp::Mod) && right == 0 {
            return Err(BoundErr::DivZero);
        }
        let bits = match op {
            BinOp::Add => left.wrapping_add(right),
            BinOp::Sub => left.wrapping_sub(right),
            BinOp::Mul => left.wrapping_mul(right),
            BinOp::Div => left / right,
            BinOp::Mod => left % right,
            BinOp::BitAnd => left & right,
            BinOp::BitOr => left | right,
            BinOp::BitXor => left ^ right,
            _ => return Err(BoundErr::Vla),
        };
        Ok(BVal {
            bits: bits & mask,
            ty,
        })
    }
}

enum AssignKind {
    Plain,
    Compound(BinOp),
}

fn assign_kind(kind: TokenKind) -> Option<AssignKind> {
    Some(match kind {
        TokenKind::Assign => AssignKind::Plain,
        TokenKind::PlusEq => AssignKind::Compound(BinOp::Add),
        TokenKind::MinusEq => AssignKind::Compound(BinOp::Sub),
        TokenKind::StarEq => AssignKind::Compound(BinOp::Mul),
        TokenKind::SlashEq => AssignKind::Compound(BinOp::Div),
        TokenKind::PercentEq => AssignKind::Compound(BinOp::Mod),
        TokenKind::AmpEq => AssignKind::Compound(BinOp::BitAnd),
        TokenKind::PipeEq => AssignKind::Compound(BinOp::BitOr),
        TokenKind::CaretEq => AssignKind::Compound(BinOp::BitXor),
        TokenKind::ShlEq => AssignKind::Compound(BinOp::Shl),
        TokenKind::ShrEq => AssignKind::Compound(BinOp::Shr),
        _ => return None,
    })
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

fn enum_int(value: BVal) -> Result<i32, ()> {
    let bits = value.bits & value.ty.mask();
    let signed = if value.ty.is_signed() {
        sign_extend(bits, value.ty.width())
    } else if bits > i32::MAX as u64 {
        return Err(());
    } else {
        bits as i64
    };
    if !(i32::MIN as i64..=i32::MAX as i64).contains(&signed) {
        return Err(());
    }
    Ok(signed as i32)
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
        TokenKind::KwAlignas | TokenKind::KwAlignof => "alignment specifiers are not supported",
        TokenKind::KwAtomic => "atomic types are not supported",
        TokenKind::KwComplex | TokenKind::KwImaginary => "complex types are not supported",
        TokenKind::KwGeneric => "_Generic is not supported",
        TokenKind::KwNoreturn => "_Noreturn is not supported",
        TokenKind::KwStaticAssert => "_Static_assert is not supported",
        TokenKind::KwThreadLocal => "thread-local storage is not supported",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn parsed(source: &str) -> String {
        let tokens = lex(source, "t.c").unwrap();
        dump(&parse(&tokens, "t.c").unwrap())
    }

    fn parse_err(source: &str) -> String {
        let tokens = match lex(source, "t.c") {
            Ok(tokens) => tokens,
            Err(err) => return err.to_string(),
        };
        parse(&tokens, "t.c").unwrap_err().to_string()
    }

    #[test]
    fn return_constant_ast() {
        assert_eq!(
            parsed("int main(void) { return 42; }\n"),
            "\
func int main
  block
    return
      int 42
"
        );
    }

    #[test]
    fn precedence_and_associativity() {
        assert_eq!(
            parsed("int main(void) { return 1 + 2 * 3; }\n"),
            "\
func int main
  block
    return
      +
        int 1
        *
          int 2
          int 3
"
        );
        assert_eq!(
            parsed("int main(void) { return 8 - 2 - 1; }\n"),
            "\
func int main
  block
    return
      -
        -
          int 8
          int 2
        int 1
"
        );
        assert_eq!(
            parsed("int main(void) { a = b = 1; }\n"),
            "\
func int main
  block
    expr
      assign
        var a
        assign
          var b
          int 1
"
        );
    }

    #[test]
    fn dangling_else_binds_inward() {
        let source = "\
int main(void) {
    if (c)
        if (d)
            a;
        else
            b;
}
";
        assert_eq!(
            parsed(source),
            "\
func int main
  block
    if
      cond
        var c
      then
        if
          cond
            var d
          then
            expr
              var a
          else
            expr
              var b
"
        );
    }

    #[test]
    fn static_extern_and_do_while_parse() {
        assert_eq!(
            parsed(
                "\
int main(void) {
    static int kept = 1;
    extern int shared;
    int other(int);
    do {
        kept = kept + shared;
    } while (kept < 3);
    return other(kept);
}
"
            ),
            "\
func int main
  block
    static decl int kept =
      int 1
    extern decl int shared
    proto int other
      param int
    do
      body
        block
          expr
            assign
              var kept
              +
                var kept
                var shared
      cond
        <
          var kept
          int 3
    return
      call other
        var kept
"
        );
    }

    #[test]
    fn for_loop_call_and_empty_parameter_list() {
        assert_eq!(
            parsed(
                "\
int add(int a, int b) { return a + b; }
int main() { return add(19, 23); }
"
            ),
            "\
func int add
  param int a
  param int b
  block
    return
      +
        var a
        var b
func int main
  block
    return
      call add
        int 19
        int 23
"
        );
        assert_eq!(
            parsed(
                "\
int main(void) {
    int sum = 0;
    for (int i = 1; i <= 10; i = i + 1)
        sum = sum + i;
    return sum;
}
"
            ),
            "\
func int main
  block
    decl int sum =
      int 0
    for
      init
        decl int i =
          int 1
      cond
        <=
          var i
          int 10
      step
        assign
          var i
          +
            var i
            int 1
      body
        expr
          assign
            var sum
            +
              var sum
              var i
    return
      var sum
"
        );
    }

    #[test]
    fn logical_precedence_and_unary() {
        assert_eq!(
            parsed("int main(void) { return 0 || 1 && 0; }\n"),
            "\
func int main
  block
    return
      ||
        int 0
        &&
          int 1
          int 0
"
        );
        assert_eq!(
            parsed("int main(void) { return -2 + !3; }\n"),
            "\
func int main
  block
    return
      +
        neg
          int 2
        not
          int 3
"
        );
    }

    #[test]
    fn parentheses_do_not_hide_calls() {
        assert_eq!(
            parsed("int main(void) { return (add)(1, 2); }\n"),
            "\
func int main
  block
    return
      call add
        int 1
        int 2
"
        );
    }

    #[test]
    fn pointer_and_array_declarators() {
        assert_eq!(
            parsed(
                "\
int *a[3];
int (*p)[3];
int **pp;
void *vp;
int f(int *a, int b[]);
int *g(int x) { return 0; }
"
            ),
            "\
global int *[3] a
global int (*)[3] p
global int ** pp
global void * vp
proto int f
  param int * a
  param int * b
func int * g
  param int x
  block
    return
      int 0
"
        );
        assert_eq!(
            parsed("int main(void) { return a[i] + *p + &x; }\n"),
            "\
func int main
  block
    return
      +
        +
          index
            var a
            var i
          deref
            var p
        addr
          var x
"
        );
        assert_eq!(
            parsed("int a[] = {1, 2, 3};\n"),
            "\
global int [] a =
  list
    int 1
    int 2
    int 3
"
        );
    }

    #[test]
    fn syntax_errors_name_the_problem() {
        assert!(parse_err("int main(void) { return 1 }").contains("expected ';'"));
        assert!(parse_err("int main(void { return 0; }").contains("expected ')'"));
        assert!(parse_err("struct S { int x: 0; };").contains("zero width"));
        assert!(parse_err("int a[n];").contains("variable-length arrays are not supported"));
        assert!(parse_err("int a[0];").contains("array size must be positive"));
        assert!(parse_err("\"hi\";").contains("expected a type"));
        assert!(parse_err("int main(void) { return a ? b; }").contains("expected ':'"));
        assert!(parse_err("int add();").contains("old-style function declarations"));
        assert!(
            parse_err("int main(void) { int inner(void) { return 1; } }")
                .contains("nested functions")
        );
        assert!(parse_err("int f(int, int) { return 0; }").contains("parameter name omitted"));
        assert!(parse_err("int f(void) = 1;").contains("cannot have an initializer"));
        assert!(parse_err("static extern int x;").contains("multiple storage-class"));
        assert!(parse_err("void g;").contains("void variables"));
        assert!(parse_err("int f(static int a);").contains("not supported on parameters"));
        assert!(parse_err("int main(void) { switch 1; }").contains("expected '('"));
        assert!(parse_err("int main(void) { goto *p; }").contains("computed goto"));
        assert!(
            parse_err("int main(void) { switch (1) { case 1: int x = 0; return x; } }")
                .contains("a declaration is not allowed as a statement")
        );
        assert!(parse_err("int main(void) { else; }").contains("'else' without a matching 'if'"));
        assert!(parsed("int (*fp)(void);\n").contains("int (*)(void)"));
        assert!(
            parse_err("int main(void) { static int foo(void) { return 1; } }")
                .contains("nested functions")
        );
        assert!(
            parse_err("int main(void) { for (static int i = 0; i < 1; i = i + 1) ; }")
                .contains("for-loop declaration cannot have a storage class")
        );
        assert!(
            parse_err("int main(void) { for (extern int i = 0; i < 1; i = i + 1) ; }")
                .contains("for-loop declaration cannot have a storage class")
        );
        assert!(
            parse_err("int main(void) { for (int f(void); ; ) return 0; }")
                .contains("function declaration is not allowed in a for loop")
        );
        assert!(parse_err("int main(void) { int f(); }").contains("old-style function"));
        assert!(parse_err("int main(void) { do { } while (0) return 0; }").contains("expected ';'"));
        assert!(parse_err("int main(void) { do { }; while (0); }").contains("expected 'while'"));
        assert!(parse_err("short long x;").contains("invalid combination of type specifiers"));
        assert!(parse_err("unsigned signed x;").contains("invalid combination of type specifiers"));
        assert!(parse_err("long long long x;").contains("invalid combination of type specifiers"));
        assert!(parse_err("int int x;").contains("invalid combination of type specifiers"));
        assert!(parse_err("char int x;").contains("invalid combination of type specifiers"));
        assert!(parse_err("int f(...);").contains("named parameter"));
        assert!(parse_err("int f(...);").contains("named parameter is required"));
        assert!(parse_err("const const int x;").contains("duplicate 'const' qualifier"));
        assert!(parse_err("int * const const p;").contains("duplicate 'const' qualifier"));
        assert!(parsed("int * restrict p;\n").contains("restrict"));
    }

    #[test]
    fn switch_statement_ast() {
        assert_eq!(
            parsed(
                "\
int main(void) {
    switch (1 + 2) {
    case 0: return 1;
    case 1: case 2: return 2;
    default: return 3;
    }
}
"
            ),
            "\
func int main
  block
    switch
      cond
        +
          int 1
          int 2
      body
        block
          case
            int 0
            return
              int 1
          case
            int 1
            case
              int 2
              return
                int 2
          default
            return
              int 3
"
        );
        assert!(
            parse_err("int main(void) { switch (1) { case 1, 2: return 0; } }")
                .contains("expected ':'")
        );
        assert!(
            parse_err("int main(void) { switch (1) { case : return 0; } }")
                .contains("expected an expression")
        );
    }

    #[test]
    fn strings_qualifiers_and_varargs_parse() {
        assert_eq!(
            parsed("const char *p = \"hel\" \"lo\\n\";\n"),
            "\
global const char * p =
  string \"hello\\n\"
"
        );
        assert_eq!(
            parsed("int printf(const char *, ...);\n"),
            "\
proto int printf
  param const char *
  varargs
"
        );
        assert_eq!(
            parsed("char s[2] = \"hi\";\n"),
            "\
global char [2] s =
  string \"hi\"
"
        );
        assert_eq!(
            parsed("int *const p;\n"),
            "\
global int * const p
"
        );
    }

    #[test]
    fn struct_declarators_and_member_access() {
        assert_eq!(
            parsed(
                "\
struct S { int x; char c; };
struct S s;
struct S *p;
"
            ),
            "\
global struct S s
global struct S * p
"
        );
        assert_eq!(
            parsed("int main(void) { return a.b + p->c; }\n"),
            "\
func int main
  block
    return
      +
        member b
          var a
        arrow c
          var p
"
        );
        assert_eq!(
            parsed("struct S { int x; } s = {1, 2};\n"),
            "\
global struct S s =
  list
    int 1
    int 2
"
        );
    }

    #[test]
    fn integer_operator_precedence() {
        assert_eq!(
            parsed("int main(void) { return 1 + 2 << 3; }\n"),
            "\
func int main
  block
    return
      <<
        +
          int 1
          int 2
        int 3
"
        );
        assert_eq!(
            parsed("int main(void) { return 1 << 2 + 3; }\n"),
            "\
func int main
  block
    return
      <<
        int 1
        +
          int 2
          int 3
"
        );
        assert_eq!(
            parsed("int main(void) { return 5 & 3 == 1; }\n"),
            "\
func int main
  block
    return
      &
        int 5
        ==
          int 3
          int 1
"
        );
        assert_eq!(
            parsed("int main(void) { return 1 | 2 & 3 ^ 4; }\n"),
            "\
func int main
  block
    return
      |
        int 1
        ^
          &
            int 2
            int 3
          int 4
"
        );
        assert_eq!(
            parsed("int main(void) { return 1 || 2 | 3 && 4; }\n"),
            "\
func int main
  block
    return
      ||
        int 1
        &&
          |
            int 2
            int 3
          int 4
"
        );
        assert_eq!(
            parsed("int main(void) { return a ? b : c ? d : e; }\n"),
            "\
func int main
  block
    return
      ?:
        var a
        var b
        ?:
          var c
          var d
          var e
"
        );
        assert_eq!(
            parsed("int main(void) { return a ? b, c : d; }\n"),
            "\
func int main
  block
    return
      ?:
        var a
        ,
          var b
          var c
        var d
"
        );
        assert_eq!(
            parsed("int main(void) { return a ? b : c, d; }\n"),
            "\
func int main
  block
    return
      ,
        ?:
          var a
          var b
          var c
        var d
"
        );
        assert_eq!(
            parsed("int main(void) { a = b ? c : d; }\n"),
            "\
func int main
  block
    expr
      assign
        var a
        ?:
          var b
          var c
          var d
"
        );
        assert_eq!(
            parsed("int main(void) { a ? b : c = d; }\n"),
            "\
func int main
  block
    expr
      assign
        ?:
          var a
          var b
          var c
        var d
"
        );
        assert_eq!(
            parsed("int main(void) { a += b += 1; }\n"),
            "\
func int main
  block
    expr
      +=
        var a
        +=
          var b
          int 1
"
        );
        assert_eq!(
            parsed("int main(void) { x <<= 2; y >>= 1; }\n"),
            "\
func int main
  block
    expr
      <<=
        var x
        int 2
    expr
      >>=
        var y
        int 1
"
        );
        assert_eq!(
            parsed("int main(void) { ++x; x--; }\n"),
            "\
func int main
  block
    expr
      pre++
        var x
    expr
      post--
        var x
"
        );
        assert_eq!(
            parsed("int main(void) { return ~1 + !2; }\n"),
            "\
func int main
  block
    return
      +
        bnot
          int 1
        not
          int 2
"
        );
        assert_eq!(
            parsed("int main(void) { return sizeof(int) + sizeof x; }\n"),
            "\
func int main
  block
    return
      +
        sizeof int
        sizeof
          var x
"
        );
        assert_eq!(
            parsed("int main(void) { return (a = 1, b = 2); }\n"),
            "\
func int main
  block
    return
      ,
        assign
          var a
          int 1
        assign
          var b
          int 2
"
        );
        assert_eq!(
            parsed("int add(int a, int b); int main(void) { return add(1, (2, 3)); }\n"),
            "\
proto int add
  param int a
  param int b
func int main
  block
    return
      call add
        int 1
        ,
          int 2
          int 3
"
        );
        assert_eq!(
            parsed("int a = 1 << 4, b = sizeof(int);\n"),
            "\
global int a =
  <<
    int 1
    int 4
global int b =
  sizeof int
"
        );
    }

    #[test]
    fn prototypes_and_globals() {
        assert_eq!(
            parsed(
                "\
static int counter;
extern int shared;
int ready = 1 + 2;
int add(int a, int b);
int sum(int, int);
int add(int a, int b) { return a + b; }
"
            ),
            "\
static global int counter
extern global int shared
global int ready =
  +
    int 1
    int 2
proto int add
  param int a
  param int b
proto int sum
  param int
  param int
func int add
  param int a
  param int b
  block
    return
      +
        var a
        var b
"
        );
    }

    #[test]
    fn integer_types_and_casts() {
        assert_eq!(
            parsed(
                "\
unsigned long long int u;
signed char sc;
short int s;
int f(char, unsigned);
char g(long int x) { return (char)x; }
"
            ),
            "\
global unsigned long long u
global signed char sc
global short s
proto int f
  param char
  param unsigned int
func char g
  param long x
  block
    return
      cast char
        var x
"
        );
        assert_eq!(
            parsed("int main(void) { return (int)1 + 2; }\n"),
            "\
func int main
  block
    return
      +
        cast int
          int 1
        int 2
"
        );
        assert_eq!(
            parsed("int main(void) { return sizeof(unsigned long); }\n"),
            "\
func int main
  block
    return
      sizeof unsigned long
"
        );
    }

    #[test]
    fn smoke_sources_parse() {
        for name in [
            "tests/01_return_const.c",
            "tests/02_add.c",
            "tests/03_loop.c",
            "tests/04_call.c",
        ] {
            let path = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), name);
            let source = std::fs::read_to_string(&path).unwrap();
            let tokens = lex(&source, name).unwrap();
            parse(&tokens, name).unwrap_or_else(|err| panic!("{name}: {err}"));
        }
    }
}
