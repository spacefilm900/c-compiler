//! Declaration specifiers (C11 6.7.1-6.7.4): storage classes, type
//! specifiers and qualifiers, `typeof`, and resolving them to a type.

use super::*;

impl Parser<'_> {
    /// Declaration specifiers. GNU attributes among them are returned in
    /// [`Specs::attrs`].
    pub(super) fn parse_specs(&mut self, where_: SpecPlace) -> Result<Specs, Diagnostic> {
        let (mut specs, attrs) = self.collect_attrs(|parser| parser.parse_specs_inner(where_))?;
        specs.attrs = attrs;
        Ok(specs)
    }

    pub(super) fn parse_specs_inner(&mut self, where_: SpecPlace) -> Result<Specs, Diagnostic> {
        let mut flags = SpecFlags::default();
        while self.parse_spec_word(&mut flags, where_)? {}
        self.finish_specs(flags, where_)
    }

    /// Take one declaration specifier into `flags`. `Ok(false)` means the
    /// current token does not continue the list.
    fn parse_spec_word(
        &mut self,
        flags: &mut SpecFlags,
        where_: SpecPlace,
    ) -> Result<bool, Diagnostic> {
        let token = self.current().clone();
        match token.kind {
            TokenKind::KwStatic | TokenKind::KwExtern => {
                let storage = if token.kind == TokenKind::KwStatic {
                    Storage::Static
                } else {
                    Storage::Extern
                };
                self.note_storage(flags, storage, &token, where_)?;
            }
            TokenKind::KwVoid
            | TokenKind::KwChar
            | TokenKind::KwShort
            | TokenKind::KwInt
            | TokenKind::KwLong
            | TokenKind::KwSigned
            | TokenKind::KwUnsigned => {
                self.bump();
                self.note_type(flags, &token)?;
            }
            TokenKind::KwConst | TokenKind::KwVolatile => {
                self.bump();
                self.note_qualifier(flags, &token)?;
            }
            TokenKind::KwRestrict => {
                self.bump();
                self.note_restrict(flags, &token)?;
            }
            TokenKind::KwTypedef => {
                self.note_storage(flags, Storage::Typedef, &token, where_)?;
            }
            TokenKind::KwInline => {
                // A function specifier. It does not change the type;
                // see [`Parser::note_inline`] for what it does to a
                // definition's linkage.
                self.bump();
                flags.inline_ = true;
                flags.note_func_spec("inline", &token);
            }
            TokenKind::KwNoreturn => {
                // A function specifier. It does not change the type.
                self.bump();
                flags.note_func_spec("_Noreturn", &token);
            }
            TokenKind::KwRegister | TokenKind::KwAuto => {
                self.note_storage(flags, Storage::None, &token, where_)?;
                flags.plain_storage = true;
            }
            TokenKind::KwBool => {
                self.bump();
                self.note_bool(flags, &token)?;
            }
            TokenKind::KwFloat | TokenKind::KwDouble => {
                self.bump();
                self.note_float(flags, &token)?;
            }
            TokenKind::KwStruct => {
                self.note_record(flags, RecordKind::Struct)?;
            }
            TokenKind::KwUnion => {
                self.note_record(flags, RecordKind::Union)?;
            }
            TokenKind::KwEnum => {
                self.note_enum(flags)?;
            }
            TokenKind::Ident => return self.parse_spec_ident(flags, &token, where_),
            kind => return self.parse_spec_other(kind),
        }
        Ok(true)
    }

    /// An identifier among declaration specifiers: a GNU type word,
    /// `typeof`, a typedef name, or a GNU spelling of a keyword.
    fn parse_spec_ident(
        &mut self,
        flags: &mut SpecFlags,
        token: &Token,
        where_: SpecPlace,
    ) -> Result<bool, Diagnostic> {
        if matches!(
            token.spelling.as_str(),
            "__int128" | "__int128_t" | "__uint128_t"
        ) {
            self.note_int128(flags, token)?;
        } else if token.spelling == "__builtin_va_list" || token.spelling == "_Float16" {
            if flags.has_type_specifier() {
                return Ok(false);
            }
            self.bump();
            flags.any = true;
            flags.struct_ty = Some(if token.spelling == "_Float16" {
                self.float16()
            } else {
                self.builtin_va_list()
            });
            flags.line = token.line;
            flags.column = token.column;
        } else if self.is_typeof_word(token) {
            if flags.has_type_specifier() {
                return Err(self.error("invalid combination of type specifiers"));
            }
            let ty = self.parse_typeof()?;
            flags.any = true;
            flags.struct_ty = Some(ty);
            flags.line = token.line;
            flags.column = token.column;
        } else if self.is_typedef(&token.spelling) {
            // A typedef name is a type specifier only when the list
            // does not already have one. `typedef T T` is a declarator
            // named T, not two type specifiers.
            if flags.has_type_specifier() {
                return Ok(false);
            }
            self.note_typedef_name(flags)?;
        } else if self.spec_word(&[
            "__extension__",
            "__inline",
            "__inline__",
            "__restrict",
            "__restrict__",
        ]) {
            self.note_ident_spec(flags, where_)?;
        } else {
            return self.parse_spec_other(TokenKind::Ident);
        }
        Ok(true)
    }

    /// A token that is no declaration specifier: a GNU attribute is
    /// skipped, a keyword this compiler does not support is an error, and
    /// anything else ends the list.
    fn parse_spec_other(&mut self, kind: TokenKind) -> Result<bool, Diagnostic> {
        if self.spec_word(&["__attribute__", "__attribute"]) {
            self.skip_attribute()?;
            return Ok(true);
        }
        if let Some(message) = unsupported(kind) {
            return Err(self.error(message));
        }
        Ok(false)
    }

    /// The type and storage the specifier list `flags` denotes.
    fn finish_specs(&mut self, flags: SpecFlags, where_: SpecPlace) -> Result<Specs, Diagnostic> {
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
        if let (Some((name, line, column)), false) =
            (flags.func_spec, where_ == SpecPlace::Declaration)
        {
            return Err(self.error_at(
                line,
                column,
                format!("'{name}' can only appear on functions"),
            ));
        }
        Ok(Specs {
            inline_: flags.inline_,
            func_spec: flags.func_spec,
            storage: flags.storage,
            storage_at: flags.storage_at,
            ty,
            forward_tag: flags.forward_tag,
            introduced_tag: flags.introduced_tag,
            anon_record: flags.anon_record,
            shadow_decl,
            line: flags.line,
            column: flags.column,
            attrs: Vec::new(),
        })
    }

    pub(super) fn note_storage(
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
        self.bump();
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

    pub(super) fn note_restrict(
        &self,
        flags: &mut SpecFlags,
        token: &Token,
    ) -> Result<(), Diagnostic> {
        if flags.quals.is_restrict {
            return Err(self.error_at(token.line, token.column, "duplicate 'restrict' qualifier"));
        }
        flags.quals.is_restrict = true;
        Ok(())
    }

    pub(super) fn note_bool(
        &mut self,
        flags: &mut SpecFlags,
        token: &Token,
    ) -> Result<(), Diagnostic> {
        if flags.bool_ty
            || flags.float_ty.is_some()
            || flags.has_tagged_type()
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

    pub(super) fn note_int128(
        &mut self,
        flags: &mut SpecFlags,
        token: &Token,
    ) -> Result<(), Diagnostic> {
        // `__int128` may be signed or unsigned, like `int`. It does not
        // combine with `char`, `short`, `int`, `long`, `_Bool`, or a float.
        if flags.bool_ty
            || flags.float_ty.is_some()
            || flags.has_tagged_type()
            || flags.int128
            || flags.void
            || flags.char_
            || flags.short != 0
            || flags.int_
            || flags.long != 0
        {
            return Err(self.error_at(
                token.line,
                token.column,
                "invalid combination of type specifiers",
            ));
        }
        if token.spelling == "__uint128_t" && flags.signed {
            return Err(self.error_at(
                token.line,
                token.column,
                "invalid combination of type specifiers",
            ));
        }
        self.bump();
        flags.any = true;
        flags.int128 = true;
        if token.spelling == "__uint128_t" {
            flags.unsigned = true;
        }
        flags.line = token.line;
        flags.column = token.column;
        Ok(())
    }

    pub(super) fn note_float(
        &mut self,
        flags: &mut SpecFlags,
        token: &Token,
    ) -> Result<(), Diagnostic> {
        let bad = self.error_at(
            token.line,
            token.column,
            "invalid combination of type specifiers",
        );
        if flags.has_tagged_type() || flags.bool_ty || flags.float_ty.is_some() {
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

    pub(super) fn note_record(
        &mut self,
        flags: &mut SpecFlags,
        kind: RecordKind,
    ) -> Result<(), Diagnostic> {
        if flags.has_tagged_type()
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

    pub(super) fn note_typedef_name(&mut self, flags: &mut SpecFlags) -> Result<(), Diagnostic> {
        if flags.has_tagged_type()
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

    pub(super) fn note_enum(&mut self, flags: &mut SpecFlags) -> Result<(), Diagnostic> {
        if flags.has_tagged_type()
            || flags.has_scalar()
            || flags.bool_ty
            || flags.float_ty.is_some()
        {
            return Err(self.error("invalid combination of type specifiers"));
        }
        let (line, column, base) = self.parse_enum_specifier()?;
        flags.any = true;
        flags.enum_ty = true;
        flags.enum_base = (base != TypeName::Int).then_some(base);
        flags.introduced_tag = true;
        flags.line = line;
        flags.column = column;
        Ok(())
    }

    /// `__typeof__` and `__typeof` in any mode (strict C11 rejects them
    /// with a diagnostic), and `typeof` in GNU mode, where it is a keyword.
    pub(super) fn is_typeof_word(&self, token: &Token) -> bool {
        token.kind == TokenKind::Ident
            && (matches!(token.spelling.as_str(), "__typeof__" | "__typeof")
                || (token.spelling == "typeof" && self.dialect.gnu()))
    }

    /// `typeof ( expression )` or `typeof ( type-name )` (GCC manual,
    /// "Referring to a Type with typeof"). The type of an expression is
    /// taken before lvalue conversion, so an array stays an array and
    /// qualifiers are kept; the expression is not evaluated.
    pub(super) fn parse_typeof(&mut self) -> Result<CType, Diagnostic> {
        let keyword = self.advance();
        if !self.dialect.gnu() {
            return Err(self.error_at(
                keyword.line,
                keyword.column,
                format!("'{}' is a GNU extension; use -std=gnu11", keyword.spelling),
            ));
        }
        self.expect_skip(TokenKind::LParen, "'('")?;
        let ty = if self.at_type_specifier(self.current()) {
            self.parse_type_name()?
        } else {
            let expr = self.parse_expression()?;
            if is_bitfield_member(&expr, self.env()) {
                return Err(self.error_at(
                    expr.line(),
                    expr.column(),
                    "'typeof' applied to a bit-field",
                ));
            }
            check_uneval(&expr, self.env())
                .map_err(|err| self.error_at(expr.line(), expr.column(), bound_message(err)))?;
            type_only(&expr, self.env()).map_err(|_| {
                self.error_at(
                    expr.line(),
                    expr.column(),
                    "cannot determine the type of this 'typeof' operand",
                )
            })?
        };
        self.expect_skip(TokenKind::RParen, "')'")?;
        Ok(ty)
    }

    pub(super) fn spec_word(&self, words: &[&str]) -> bool {
        self.current().kind == TokenKind::Ident && words.contains(&self.current().spelling.as_str())
    }

    pub(super) fn note_ident_spec(
        &mut self,
        flags: &mut SpecFlags,
        _where: SpecPlace,
    ) -> Result<(), Diagnostic> {
        let token = self.current().clone();
        self.bump();
        match token.spelling.as_str() {
            "__inline" | "__inline__" => {
                flags.inline_ = true;
                flags.note_func_spec("inline", &token);
            }
            "__restrict" | "__restrict__" => self.note_restrict(flags, &token)?,
            "__extension__" => {}
            _ => unreachable!("ident specifier"),
        }
        Ok(())
    }

    pub(super) fn note_qualifier(
        &self,
        flags: &mut SpecFlags,
        token: &Token,
    ) -> Result<(), Diagnostic> {
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

    pub(super) fn note_type(&self, flags: &mut SpecFlags, token: &Token) -> Result<(), Diagnostic> {
        if flags.has_tagged_type()
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

    pub(super) fn resolve_type(&self, flags: &SpecFlags) -> Result<CType, Diagnostic> {
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
        if let Some(base) = flags.enum_base {
            return Ok(CType::from_name(base));
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

    pub(super) fn resolve_scalar(&self, flags: &SpecFlags) -> Result<TypeName, Diagnostic> {
        let bad = self.error_at(
            flags.line,
            flags.column,
            "invalid combination of type specifiers",
        );
        if flags.int128 {
            if flags.void || flags.char_ || flags.short != 0 || flags.int_ || flags.long != 0 {
                return Err(bad);
            }
            return Ok(if flags.unsigned {
                TypeName::UInt128
            } else {
                TypeName::Int128
            });
        }
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
}
