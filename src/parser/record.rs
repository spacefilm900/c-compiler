//! Structure, union and enumeration specifiers (C11 6.7.2.1-6.7.2.3): member
//! lists, bit-fields, flexible array members, tags and their scopes.

use super::*;

impl Parser<'_> {
    /// `struct tag`, `struct tag { members }`, or the same forms for `union`.
    ///
    /// A brace list defines the type in the current scope. A use with no
    /// braces names the visible tag, or declares an incomplete one here when
    /// none is visible. `forward` is set when the visible tag lives in an
    /// outer scope, so a lone `struct tag;` can hide it (C11 6.7.2.3).
    /// A definition with no tag (`struct { int x; }`) is a distinct type.
    pub(super) fn parse_record_specifier(
        &mut self,
        kind: RecordKind,
    ) -> Result<(CType, Option<String>, usize, usize), Diagnostic> {
        self.shadow_decl = None;
        let kw = self.advance();
        let word = kind.word();
        // `struct __attribute__((packed)) tag { ... }`.
        let pre_attrs = self.parse_attributes_into()?;
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
            let ty = self.define_record(kind, tag.as_deref(), line, column, pre_attrs)?;
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

    /// A struct or union definition. `attrs` are the attributes before the
    /// tag; those after the closing brace join them. `packed` gives every
    /// member alignment 1 and `aligned(N)` raises the record's alignment
    /// (GCC manual, "Common Type Attributes"). A layout that differs from
    /// the natural one makes the LLVM type a byte array.
    pub(super) fn define_record(
        &mut self,
        kind: RecordKind,
        tag: Option<&str>,
        line: usize,
        column: usize,
        mut attrs: Vec<Attr>,
    ) -> Result<CType, Diagnostic> {
        let word = kind.word();
        let handle = self.record_being_defined(kind, tag, line, column)?;
        self.expect_skip(TokenKind::LBrace, "'{'")?;
        let id = handle.id();
        self.defining.insert(id);
        let parsed = self.parse_member_list();
        self.defining.remove(&id);
        let (mut fields, member_attrs) = parsed?;
        self.expect_skip(TokenKind::RBrace, "'}'")?;
        attrs.extend(self.parse_attributes_into()?);
        let label = tag.unwrap_or(word);
        if fields.is_empty() {
            return Err(self.error_at(line, column, format!("{word} '{label}' has no members")));
        }
        self.check_flexible_members(kind, &fields, line, column)?;
        let natural_spec = natural_layout_spec(&fields);
        let packed = attrs.iter().any(|attr| attr.name == "packed");
        let record_align = self.attrs_align(&attrs)?;
        let spec = attributed_layout_spec(&natural_spec, &member_attrs, packed);
        let union_layout = kind == RecordKind::Union;
        let too_large = || self.error_at(line, column, format!("{word} '{label}' is too large"));
        let natural = layout_members(&natural_spec, union_layout).ok_or_else(too_large)?;
        let (places, mut size, mut align) =
            layout_members(&spec, union_layout).ok_or_else(too_large)?;
        let unadjusted_align = align;
        if let Some(extra) = record_align {
            align = align.max(extra);
            size = size.div_ceil(u64::from(align)) * u64::from(align);
        }
        let irregular = places != natural.0 || size != natural.1 || align != natural.2;
        if irregular
            && fields
                .iter()
                .any(|field| field.bit_width.is_some_and(|w| w > 0))
        {
            // A bit-field is read through its declared type's storage unit.
            // That only matches GCC when packing moved nothing and every
            // unit stays inside the record.
            let fits = places.iter().zip(&fields).all(|(place, field)| {
                field.bit_width.is_none() || place.offset + field.ty.size().unwrap_or(0) <= size
            });
            if places != natural.0 || !fits {
                return Err(self.error_at(
                    line,
                    column,
                    format!("bit-fields in a packed or aligned {word} are not supported"),
                ));
            }
        }
        if size == 0 {
            return Err(self.error_at(line, column, format!("{word} '{label}' has size 0")));
        }
        if irregular {
            handle.set_irregular();
        }
        for (field, place) in fields.iter_mut().zip(places) {
            field.offset = place.offset;
            field.bit_offset = place.bit_offset;
            field.bit_width = place.bit_width;
        }
        let llvm_name = self.alloc_llvm_name(kind, tag.unwrap_or("anon"));
        handle.complete(fields, size, align, llvm_name);
        if unadjusted_align != align {
            handle.set_unadjusted_align(unadjusted_align);
        }
        self.structs.push(handle.clone());
        Ok(CType::Struct(handle))
    }

    /// The record a definition with `tag` completes: an incomplete one
    /// declared in the current scope, or a fresh one.
    fn record_being_defined(
        &mut self,
        kind: RecordKind,
        tag: Option<&str>,
        line: usize,
        column: usize,
    ) -> Result<StructRef, Diagnostic> {
        let word = kind.word();
        let Some(tag) = tag else {
            return Ok(self.declare_fresh_tag("", kind));
        };
        if let Some(existing) = self.lookup_record_current(tag) {
            if existing.kind() != kind {
                return Err(self.error_at(
                    line,
                    column,
                    format!("'{tag}' defined as a different tag type"),
                ));
            }
            if existing.is_complete() || self.defining.contains(&existing.id()) {
                return Err(self.error_at(line, column, format!("redefinition of {word} '{tag}'")));
            }
            Ok(existing)
        } else if self.lookup_tag_current(tag).is_some() {
            Err(self.error_at(
                line,
                column,
                format!("'{tag}' defined as a different tag type"),
            ))
        } else {
            Ok(self.declare_fresh_tag(tag, kind))
        }
    }

    /// An enum specifier. Returns where it starts and the enumerated type.
    pub(super) fn parse_enum_specifier(&mut self) -> Result<(usize, usize, TypeName), Diagnostic> {
        let kw = self.advance();
        let (tag, line, column) = if self.current().kind == TokenKind::Ident {
            let tag_tok = self.advance();
            (Some(tag_tok.spelling), tag_tok.line, tag_tok.column)
        } else {
            (None, kw.line, kw.column)
        };
        // GNU mode: a fixed underlying type, `enum E : uint64_t { ... }`
        // (C23 6.7.2.2; Clang accepts it in C as an extension). The macOS
        // SDK's <malloc/malloc.h> uses one unconditionally. The colon starts
        // one only when a type follows: `enum E : 3;` is an unnamed
        // bit-field and `_Generic(x, enum E: 1)` an association.
        let type_follows = self
            .tokens
            .get(self.index + 1)
            .is_some_and(|token| self.at_type_specifier(token));
        let fixed = if self.current().kind == TokenKind::Colon && self.dialect.gnu() && type_follows
        {
            let colon = self.advance();
            let start = self.index;
            let ty = self.parse_specs(SpecPlace::TypeName)?.ty;
            // An enumeration is not an underlying type (C23 6.7.2.2p5).
            // Spelled with `enum` it is caught here; through a typedef it
            // is indistinguishable from its integer type in this compiler.
            if self.tokens[start..self.index]
                .iter()
                .any(|token| token.kind == TokenKind::KwEnum)
            {
                return Err(self.error_at(
                    colon.line,
                    colon.column,
                    "an enumeration's underlying type must be an integer type",
                ));
            }
            match ty.peel() {
                CType::Int(TypeName::Int128 | TypeName::UInt128) => {
                    return Err(self.error_at(
                        colon.line,
                        colon.column,
                        "an underlying type wider than 64 bits is not supported",
                    ))
                }
                CType::Int(name) if *name != TypeName::Bool => Some(*name),
                _ => {
                    return Err(self.error_at(
                        colon.line,
                        colon.column,
                        "an enumeration's underlying type must be an integer type",
                    ))
                }
            }
        } else {
            None
        };
        if fixed.is_some() && self.current().kind != TokenKind::LBrace {
            return Err(self.error("expected '{' after an enumeration's underlying type"));
        }
        if self.current().kind != TokenKind::LBrace {
            let Some(tag) = tag else {
                return Err(self.error_at(kw.line, kw.column, "expected an enum tag or '{'"));
            };
            return match self.lookup_tag(&tag) {
                Some(TagEntry::Enum(base)) => Ok((line, column, base)),
                Some(TagEntry::EnumOpen) => Err(self.error_at(
                    line,
                    column,
                    format!("enum '{tag}' is incomplete until its enumerator list ends"),
                )),
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
                Some(TagEntry::Enum(_) | TagEntry::EnumOpen) => {
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
                    self.tags.last_mut().expect("tag scope").insert(
                        tag.clone(),
                        fixed.map_or(TagEntry::EnumOpen, TagEntry::Enum),
                    );
                }
            }
        }
        self.bump();
        let base = self.parse_enumerators(fixed)?;
        self.expect_skip(TokenKind::RBrace, "'}'")?;
        if let Some(tag) = &tag {
            self.tags
                .last_mut()
                .expect("tag scope")
                .insert(tag.clone(), TagEntry::Enum(base));
        }
        Ok((line, column, base))
    }

    /// The enumerator list; returns the enumerated type.
    ///
    /// C11 6.7.2.2p2 requires every value to be representable as `int`, and
    /// then the type is `int`. In GNU mode, as GCC and Clang allow, a value
    /// may be outside `int`: while the list is read such a constant has the
    /// type of its value (an implicit one, see [`implicit_enum_type`]);
    /// afterwards the enumerated type is `unsigned int`
    /// or `unsigned long` when no value is negative (the first that holds
    /// the largest), else `long`, and every constant of the list has that
    /// type (GCC, and Apple clang 21; Clang 19 keeps the constants that fit
    /// in `int` as `int`). The macOS SDK needs this (`IPC_OTYPE_UNKNOWN = ~0u` in
    /// <mach_debug/ipc_info.h>).
    ///
    /// With a fixed underlying type every value must fit in it, and the
    /// constants and the enumerated type have that type.
    pub(super) fn parse_enumerators(
        &mut self,
        fixed: Option<TypeName>,
    ) -> Result<TypeName, Diagnostic> {
        if self.current().kind == TokenKind::RBrace {
            return Err(self.error("enum has no enumerators"));
        }
        if let Some(base) = fixed {
            return self.parse_fixed_enumerators(base);
        }
        let gnu = self.dialect.gnu();
        let out_of_int = "enumeration constant is not representable as int";
        let mut next: i128 = 0;
        let mut names: Vec<String> = Vec::new();
        let (mut min, mut max) = (0i128, 0i128);
        let mut prev_ty = TypeName::Int;
        loop {
            if self.current().kind != TokenKind::Ident {
                return Err(self.error("expected an enumerator"));
            }
            let name = self.advance();
            let (value, ty) = if self.match_kind(TokenKind::Assign) {
                let expr = self.parse_assignment()?;
                check_uneval(&expr, self.env())
                    .map_err(|err| self.error_at(expr.line(), expr.column(), bound_message(err)))?;
                let folded = eval_ice(&expr, self.env())
                    .map_err(|err| self.error_at(expr.line(), expr.column(), bound_message(err)))?;
                let folded_ty = folded.ty;
                let wide = enum_wide(&folded);
                match enum_int(folded) {
                    Ok(value) => (i128::from(value), TypeName::Int),
                    Err(()) => match (gnu, wide) {
                        (true, Some(value)) => (value, folded_ty),
                        _ => return Err(self.error_at(expr.line(), expr.column(), out_of_int)),
                    },
                }
            } else if (i128::from(i32::MIN)..=i128::from(i32::MAX)).contains(&next) {
                (next, TypeName::Int)
            } else {
                match (gnu, implicit_enum_type(next, prev_ty)) {
                    (true, Some(ty)) => (next, ty),
                    _ => return Err(self.error_at(name.line, name.column, out_of_int)),
                }
            };
            prev_ty = ty;
            self.bind_enum(&name.spelling, value, ty, name.line, name.column)?;
            names.push(name.spelling);
            min = min.min(value);
            max = max.max(value);
            next = value + 1;
            if self.match_kind(TokenKind::Comma) {
                if self.current().kind == TokenKind::RBrace {
                    break;
                }
                continue;
            }
            break;
        }
        if min >= i128::from(i32::MIN) && max <= i128::from(i32::MAX) {
            return Ok(TypeName::Int);
        }
        let base = if min < 0 {
            if min < i128::from(i64::MIN) || max > i128::from(i64::MAX) {
                return Err(self.error("enumerator values do not fit in one integer type"));
            }
            TypeName::int64(true)
        } else if max <= i128::from(u32::MAX) {
            TypeName::UInt
        } else {
            TypeName::int64(false)
        };
        let scope = self.names.last_mut().expect("name scope");
        for name in names {
            if let Some(NameBinding::EnumConst(_, ty)) = scope.get_mut(&name) {
                *ty = base;
            }
        }
        Ok(base)
    }

    /// The enumerator list of `enum : base { ... }`.
    fn parse_fixed_enumerators(&mut self, base: TypeName) -> Result<TypeName, Diagnostic> {
        let width = base.width();
        let (lo, hi) = if base.is_signed() {
            (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1)
        } else {
            (0, (1i128 << width) - 1)
        };
        let too_wide = "enumerator value is not representable in the underlying type";
        let mut next: i128 = 0;
        loop {
            if self.current().kind != TokenKind::Ident {
                return Err(self.error("expected an enumerator"));
            }
            let name = self.advance();
            let (value, line, column) = if self.match_kind(TokenKind::Assign) {
                let expr = self.parse_assignment()?;
                check_uneval(&expr, self.env())
                    .map_err(|err| self.error_at(expr.line(), expr.column(), bound_message(err)))?;
                let folded = eval_ice(&expr, self.env())
                    .map_err(|err| self.error_at(expr.line(), expr.column(), bound_message(err)))?;
                let value = enum_wide(&folded)
                    .ok_or_else(|| self.error_at(expr.line(), expr.column(), too_wide))?;
                (value, expr.line(), expr.column())
            } else {
                (next, name.line, name.column)
            };
            if !(lo..=hi).contains(&value) {
                return Err(self.error_at(line, column, too_wide));
            }
            self.bind_enum(&name.spelling, value, base, name.line, name.column)?;
            next = value + 1;
            if self.match_kind(TokenKind::Comma) {
                if self.current().kind == TokenKind::RBrace {
                    break;
                }
                continue;
            }
            break;
        }
        Ok(base)
    }

    /// The members, and the GNU attributes of each.
    pub(super) fn parse_member_list(
        &mut self,
    ) -> Result<(Vec<StructField>, Vec<MemberAttrs>), Diagnostic> {
        let mut fields = Vec::new();
        let mut extras = Vec::new();
        let mut names = HashSet::new();
        if self.current().kind == TokenKind::RBrace {
            return Ok((fields, extras));
        }
        while !matches!(self.current().kind, TokenKind::RBrace | TokenKind::Eof) {
            if self.current().kind == TokenKind::KwStaticAssert {
                self.parse_static_assert()?;
                continue;
            }
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
                    extras.push(self.member_attrs(&specs, &[], &ty, false)?);
                    fields.push(StructField {
                        name: String::new(),
                        ty,
                        offset: 0,
                        bit_offset: 0,
                        bit_width: None,
                    });
                    self.bump();
                    continue;
                }
                return Err(self.error("expected a declarator"));
            }
            loop {
                if self.current().kind == TokenKind::Colon {
                    let (width, attrs) =
                        self.collect_attrs(|parser| parser.parse_bit_width(&specs.ty, false))?;
                    let (_, more) = self.collect_attrs(Self::skip_attributes)?;
                    let attrs: Vec<Attr> = attrs.into_iter().chain(more).collect();
                    fields.push(StructField {
                        name: String::new(),
                        ty: specs.ty.clone(),
                        offset: 0,
                        bit_offset: 0,
                        bit_width: Some(width),
                    });
                    extras.push(self.member_attrs(&specs, &attrs, &specs.ty, true)?);
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
                    let more = self.parse_attributes_into()?;
                    let member_attrs: Vec<Attr> = named.attrs.iter().cloned().chain(more).collect();
                    let ty = self.apply_mode(ty, &member_attrs)?;
                    if ty.is_variably_modified() {
                        return Err(self.error_at(
                            named.line,
                            named.column,
                            "a structure or union member cannot have a variably modified type",
                        ));
                    }
                    // An incomplete array may be a flexible array member. The
                    // constraint (last member, another named member, not a
                    // union) is checked once the member list is finished.
                    if width.is_none()
                        && !(ty.is_array() && ty.array_len().is_none())
                        && !ty.is_complete()
                    {
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
                    extras.push(self.member_attrs(&specs, &member_attrs, &ty, width.is_some())?);
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
            self.expect_skip(TokenKind::Semi, "';'")?;
        }
        Ok((fields, extras))
    }

    pub(super) fn member_attrs(
        &self,
        specs: &Specs,
        attrs: &[Attr],
        ty: &CType,
        is_bitfield: bool,
    ) -> Result<MemberAttrs, Diagnostic> {
        let all: Vec<Attr> = specs.attrs.iter().chain(attrs).cloned().collect();
        if is_bitfield {
            if let Some((line, column)) = specs.alignas_at {
                return Err(self.error_at(
                    line,
                    column,
                    "alignment specifiers are not allowed on a bit-field",
                ));
            }
        }
        let align = self.object_align(specs, ty, &all, false, false, is_bitfield)?;
        Ok(MemberAttrs {
            align,
            packed: all.iter().any(|attr| attr.name == "packed"),
        })
    }

    /// Member names an anonymous struct or union contributes to its parent.
    pub(super) fn promoted_names_of(ty: &CType, out: &mut Vec<String>) {
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
    pub(super) fn parse_bit_width(&mut self, ty: &CType, named: bool) -> Result<u32, Diagnostic> {
        let colon = self.advance();
        if ty.is_atomic() {
            return Err(self.error_at(
                colon.line,
                colon.column,
                "bit-field cannot have an atomic type",
            ));
        }
        let Some(integer) = ty.unqualified().as_integer() else {
            return Err(self.error_at(
                colon.line,
                colon.column,
                format!("bit-field has non-integral type '{}'", ty.display()),
            ));
        };
        // The backend stores bit-fields in a 64-bit container. A 128-bit
        // declared type would drop bits above 64 and can emit a poison
        // shift on a signed field.
        if integer.width() > 64 {
            return Err(self.error_at(
                colon.line,
                colon.column,
                format!("bit-field of type '{}' is not supported", ty.display()),
            ));
        }
        let expr = self.parse_conditional()?;
        check_uneval(&expr, self.env())
            .map_err(|err| self.error_at(expr.line(), expr.column(), bound_message(err)))?;
        let value = eval_ice(&expr, self.env())
            .map_err(|err| self.error_at(expr.line(), expr.column(), bound_message(err)))?;
        let signed = if value.ty.is_signed() {
            sign_extend(value.bits, value.ty.width())
        } else if value.bits > i64::MAX as u128 {
            return Err(self.error_at(expr.line(), expr.column(), "bit-field width is too large"));
        } else {
            value.bits as i128
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
        if signed > i128::from(max) {
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
    pub(super) fn apply_shadow(&mut self, specs: &Specs) -> Result<(), Diagnostic> {
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
    pub(super) fn finish_tag_only(&mut self, specs: &Specs) -> Result<(), Diagnostic> {
        self.reject_func_spec(specs)?;
        if specs.introduced_tag || specs.ty.unqualified().is_record() {
            if specs.storage != Storage::None || specs.thread_local {
                return Err(self.error(
                    "storage class on a tag declaration with no declarator is not supported",
                ));
            }
            if specs.alignas_at.is_some() {
                return Err(self.error(
                    "alignment specifiers on a tag declaration with no declarator are not supported",
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

    pub(super) fn declare_fresh_tag(&mut self, tag: &str, kind: RecordKind) -> StructRef {
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

    pub(super) fn lookup_tag(&self, tag: &str) -> Option<TagEntry> {
        for scope in self.tags.iter().rev() {
            if let Some(found) = scope.get(tag) {
                return Some(found.clone());
            }
        }
        None
    }

    pub(super) fn lookup_tag_current(&self, tag: &str) -> Option<TagEntry> {
        self.tags.last().and_then(|scope| scope.get(tag).cloned())
    }

    pub(super) fn lookup_record_current(&self, tag: &str) -> Option<StructRef> {
        match self.lookup_tag_current(tag) {
            Some(TagEntry::Record(handle)) => Some(handle),
            _ => None,
        }
    }

    /// `__builtin_va_list` is `[1 x struct __va_list_tag]`, the SysV register
    /// save descriptor. As a parameter the array decays to a pointer, which
    /// is what `vprintf` takes.
    ///
    /// On Apple arm64 and on Windows it is `char *`. On AArch64 Linux it is the AAPCS64
    /// structure `struct __va_list { void *__stack; void *__gr_top;
    /// void *__vr_top; int __gr_offs; int __vr_offs; }`, passed by value.
    /// `_Float16` (ISO/IEC TS 18661-3, a reserved identifier in C11) is
    /// accepted only so that declarations mentioning it parse: the macOS
    /// SDK's <math.h> declares half-precision functions unconditionally.
    /// It is an incomplete type, so a declaration is fine but an object,
    /// a call, or any value of the type is rejected as incomplete.
    pub(super) fn float16(&mut self) -> CType {
        if let Some(ty) = &self.float16 {
            return ty.clone();
        }
        let handle = self.declare_fresh_tag("_Float16", RecordKind::Struct);
        let ty = CType::Struct(handle);
        self.float16 = Some(ty.clone());
        ty
    }

    pub(super) fn builtin_va_list(&mut self) -> CType {
        if let Some(ty) = &self.va_list {
            return ty.clone();
        }
        match crate::target::current().va_list_kind() {
            crate::target::VaListKind::CharPtr => {
                let ty = CType::pointer(CType::from_name(TypeName::Char));
                self.va_list = Some(ty.clone());
                return ty;
            }
            crate::target::VaListKind::Aapcs64 => {
                let handle = self.declare_fresh_tag("__va_list", RecordKind::Struct);
                let void_ptr = || CType::pointer(CType::Void);
                let fields = vec![
                    StructField::plain("__stack", void_ptr(), 0),
                    StructField::plain("__gr_top", void_ptr(), 8),
                    StructField::plain("__vr_top", void_ptr(), 16),
                    StructField::plain("__gr_offs", CType::from_name(TypeName::Int), 24),
                    StructField::plain("__vr_offs", CType::from_name(TypeName::Int), 28),
                ];
                let llvm_name = self.alloc_llvm_name(RecordKind::Struct, "__va_list");
                handle.complete(fields, 32, 8, llvm_name);
                self.structs.push(handle.clone());
                let ty = CType::Struct(handle);
                self.va_list = Some(ty.clone());
                return ty;
            }
            crate::target::VaListKind::SysV => {}
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

    pub(super) fn alloc_llvm_name(&mut self, kind: RecordKind, tag: &str) -> String {
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

    pub(super) fn push_tags(&mut self) {
        self.tags.push(HashMap::new());
        self.names.push(HashMap::new());
    }

    pub(super) fn pop_tags(&mut self) {
        self.tags.pop();
        self.names.pop();
    }

    /// C11 6.7.2.1: a struct with a flexible array member, and any union that
    /// contains one, shall not be an element of an array. A parameter array
    /// has already been adjusted to a pointer, so `void f(struct F a[])`
    /// is a pointer and is allowed.
    pub(super) fn reject_flexible_array(
        &self,
        ty: &CType,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        if ty.contains_flexible_array() {
            return Err(self.error_at(
                line,
                column,
                "type with a flexible array member cannot be an array element",
            ));
        }
        Ok(())
    }

    /// Flexible array member constraints (C11 6.7.2.1).
    ///
    /// The member is the last one, the structure has another named member,
    /// and the structure is not a union. The element type is complete (the
    /// declarator already required that). Such a structure, and any union
    /// that contains one, is not itself a member or an array element.
    pub(super) fn check_flexible_members(
        &self,
        kind: RecordKind,
        fields: &[StructField],
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        let flexible: Vec<usize> = fields
            .iter()
            .enumerate()
            .filter(|(_, field)| field.is_flexible())
            .map(|(index, _)| index)
            .collect();
        if kind == RecordKind::Union && !flexible.is_empty() {
            return Err(self.error_at(
                line,
                column,
                "flexible array member is not allowed in a union",
            ));
        }
        if flexible.len() > 1
            || flexible
                .first()
                .is_some_and(|index| index + 1 != fields.len())
        {
            return Err(self.error_at(
                line,
                column,
                "flexible array member is not the last member",
            ));
        }
        if flexible.len() == 1 && named_members_besides_flexible(fields) == 0 {
            return Err(self.error_at(
                line,
                column,
                "flexible array member requires another named member",
            ));
        }
        for field in fields {
            if field.is_flexible() {
                if field.ty.array_elem().is_some_and(CType::flexible_carrier) {
                    return Err(self.error_at(
                        line,
                        column,
                        "type with a flexible array member cannot be an array element",
                    ));
                }
                continue;
            }
            if kind == RecordKind::Struct && field.ty.flexible_carrier() {
                return Err(self.error_at(
                    line,
                    column,
                    "type with a flexible array member cannot be a struct member or array element",
                ));
            }
            if field.ty.contains_flexible_array() {
                return Err(self.error_at(
                    line,
                    column,
                    "type with a flexible array member cannot be an array element",
                ));
            }
        }
        Ok(())
    }
}

/// Named members other than a flexible array, including names promoted
/// from an anonymous struct or union (C11 6.7.2.1).
pub(super) fn named_members_besides_flexible(fields: &[StructField]) -> usize {
    let mut count = 0usize;
    for field in fields {
        if field.bit_width == Some(0) || field.is_flexible() {
            continue;
        }
        if field.is_anonymous_record() {
            if let Some(inner) = field.ty.struct_ref().and_then(|handle| handle.fields()) {
                count += named_members_besides_flexible(&inner);
            }
        } else if !field.name.is_empty() {
            count += 1;
        }
    }
    count
}

/// The value of an integer constant expression that fits in 64 bits
/// (signed or unsigned), for a GNU enumerator outside `int`.
fn enum_wide(value: &BVal) -> Option<i128> {
    let bits = value.bits & value.ty.mask();
    let v = if value.ty.is_signed() {
        sign_extend(bits, value.ty.width())
    } else if bits > u128::from(u64::MAX) {
        return None;
    } else {
        bits as i128
    };
    (i128::from(i64::MIN)..=i128::from(u64::MAX))
        .contains(&v)
        .then_some(v)
}

/// The reading-time type of an implicit GNU enumerator value outside
/// `int` (GCC and Clang): the previous constant's type when the value fits
/// in it, else `unsigned long` after an unsigned constant and `long` after
/// a signed one (`unsigned long` when it does not fit in `long`).
fn implicit_enum_type(value: i128, prev: TypeName) -> Option<TypeName> {
    let fits = |ty: TypeName| {
        let width = ty.width();
        if ty.is_signed() {
            (-(1i128 << (width - 1))..1i128 << (width - 1)).contains(&value)
        } else {
            (0..1i128 << width).contains(&value)
        }
    };
    if prev.width() <= 64 && fits(prev) {
        Some(prev)
    } else if !prev.is_signed() && fits(TypeName::int64(false)) {
        Some(TypeName::int64(false))
    } else if fits(TypeName::int64(true)) {
        Some(TypeName::int64(true))
    } else if fits(TypeName::int64(false)) {
        Some(TypeName::int64(false))
    } else {
        None
    }
}

pub(super) fn enum_int(value: BVal) -> Result<i32, ()> {
    let bits = value.bits & value.ty.mask();
    let signed = if value.ty.is_signed() {
        sign_extend(bits, value.ty.width())
    } else if bits > i32::MAX as u128 {
        return Err(());
    } else {
        bits as i128
    };
    if !(i32::MIN as i128..=i32::MAX as i128).contains(&signed) {
        return Err(());
    }
    Ok(signed as i32)
}

/// The layout input for `fields` with their natural alignments.
fn natural_layout_spec(fields: &[StructField]) -> Vec<LayoutMember> {
    fields
        .iter()
        .map(|field| LayoutMember {
            size: field.ty.size().unwrap_or(0),
            align: field.ty.member_align(),
            bit_width: field.bit_width,
            named: !field.name.is_empty(),
        })
        .collect()
}

/// The layout input after `packed` (on the record or a member) and a
/// member's `aligned(N)` attribute adjust each member's alignment.
fn attributed_layout_spec(
    natural: &[LayoutMember],
    member_attrs: &[MemberAttrs],
    packed: bool,
) -> Vec<LayoutMember> {
    natural
        .iter()
        .zip(member_attrs)
        .map(|(member, extra)| {
            let base = if packed || extra.packed {
                1
            } else {
                member.align
            };
            LayoutMember {
                align: extra.align.map_or(base, |align| base.max(align)),
                ..*member
            }
        })
        .collect()
}
