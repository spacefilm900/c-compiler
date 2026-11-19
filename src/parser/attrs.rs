//! GNU attributes (GCC manual, "Attribute Syntax"): parsing, classifying and
//! applying them to functions, variables and types.

use super::*;

/// An attribute name without surrounding `__` (`__packed__` is `packed`).
pub(super) fn attr_name(spelling: &str) -> String {
    let name = spelling
        .strip_prefix("__")
        .and_then(|rest| rest.strip_suffix("__"))
        .unwrap_or(spelling);
    // On MSVC `__declspec` is an attribute list (see pp/predefined.rs), and
    // its `align(N)` is GCC's `aligned(N)`.
    if name == "align" && crate::target::current().is_msvc() {
        return "aligned".to_string();
    }
    name.to_string()
}

pub(super) fn attr_class(name: &str) -> AttrClass {
    match name {
        "aligned"
        | "packed"
        | "noreturn"
        | "unused"
        | "used"
        | "always_inline"
        | "gnu_inline"
        | "noinline"
        | "section"
        | "weak"
        | "format"
        | "format_arg"
        | "deprecated"
        | "cold"
        | "hot"
        | "const"
        | "pure"
        | "constructor"
        | "destructor"
        | "fallthrough"
        | "mode"
        | "nonnull"
        | "nothrow"
        | "leaf"
        | "returns_nonnull"
        | "warn_unused_result"
        | "malloc"
        | "alloc_size"
        | "alloc_align"
        | "visibility"
        | "may_alias"
        | "sentinel"
        | "no_instrument_function"
        | "artificial"
        | "externally_visible"
        | "flatten"
        | "noclone"
        | "no_icf"
        | "noipa"
        | "optimize"
        | "target"
        | "returns_twice"
        | "access"
        | "nonstring"
        | "warning"
        | "error"
        | "unavailable"
        | "designated_init"
        | "common"
        | "nocommon"
        | "tls_model"
        | "no_reorder"
        | "no_stack_protector"
        | "stack_protect"
        | "no_sanitize"
        | "no_sanitize_address"
        | "no_sanitize_thread"
        | "no_sanitize_undefined"
        | "no_split_stack"
        | "warn_if_not_aligned"
        | "assume_aligned"
        | "noplt"
        | "retain"
        | "uninitialized"
        | "zero_call_used_regs"
        | "counted_by"
        | "strict_flex_array"
        | "fd_arg"
        | "fd_arg_read"
        | "fd_arg_write"
        | "null_terminated_string_arg"
        | "copy"
        | "noinit"
        | "persistent"
        | "symver"
        | "no_profile_instrument_function"
        | "transparent_union"
        | "scalar_storage_order"
        | "regparm"
        | "cdecl"
        | "ms_abi"
        | "sysv_abi"
        | "force_align_arg_pointer"
        | "expected_throw" => AttrClass::Known,
        "vector_size" | "cleanup" | "alias" | "ifunc" | "naked" | "weakref" => {
            AttrClass::Unsupported
        }
        _ => AttrClass::Unknown,
    }
}

/// Attributes whose arguments are expressions, not identifiers.
pub(super) fn attr_takes_exprs(name: &str) -> bool {
    matches!(
        name,
        "aligned"
            | "alloc_size"
            | "alloc_align"
            | "constructor"
            | "destructor"
            | "vector_size"
            | "nonnull"
            | "format_arg"
            | "sentinel"
            | "assume_aligned"
            | "warn_if_not_aligned"
    )
}

impl Parser<'_> {
    /// Merge a function declaration's attributes into those of its symbol.
    pub(super) fn note_fn_attrs(&mut self, symbol: &str, spec_attrs: &[Attr], decl_attrs: &[Attr]) {
        let attrs: Vec<Attr> = spec_attrs.iter().chain(decl_attrs).cloned().collect();
        let found = self.fn_attrs_of(&attrs);
        if found != FnAttrs::default() {
            self.fn_attrs
                .entry(symbol.to_string())
                .or_default()
                .merge(&found);
        }
    }

    pub(super) fn at_attribute(&self) -> bool {
        self.spec_word(&["__attribute__", "__attribute"])
    }

    /// Any number of `__attribute__((...))` groups. Each attribute is added
    /// to [`Parser::attr_sink`] for the declaration being parsed.
    pub(super) fn skip_attributes(&mut self) -> Result<(), Diagnostic> {
        while self.at_attribute() {
            self.skip_attribute()?;
        }
        Ok(())
    }

    /// Attribute groups here, returned instead of joining the enclosing
    /// declaration's attributes.
    pub(super) fn parse_attributes_into(&mut self) -> Result<Vec<Attr>, Diagnostic> {
        self.collect_attrs(Self::skip_attributes)
            .map(|((), attrs)| attrs)
    }

    /// Run `parse`, and return the attributes it parsed separately from the
    /// enclosing declaration's.
    pub(super) fn collect_attrs<T>(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<T, Diagnostic>,
    ) -> Result<(T, Vec<Attr>), Diagnostic> {
        let saved = std::mem::take(&mut self.attr_sink);
        let result = parse(self);
        let attrs = std::mem::replace(&mut self.attr_sink, saved);
        result.map(|value| (value, attrs))
    }

    /// `__attribute__ (( attribute-list ))` (GCC manual, "Attribute
    /// Syntax"). Each attribute is a word, optionally followed by a
    /// parenthesized argument list; empty entries are allowed. An unknown
    /// attribute is ignored with a warning; one that would change the
    /// program and is not implemented is an error.
    pub(super) fn skip_attribute(&mut self) -> Result<(), Diagnostic> {
        let at = self.advance();
        self.expect_skip(TokenKind::LParen, "'('")?;
        self.expect_skip(TokenKind::LParen, "'('")?;
        loop {
            match self.current().kind {
                TokenKind::RParen => break,
                TokenKind::Comma => {
                    self.bump();
                    continue;
                }
                TokenKind::Eof => {
                    return Err(self.error_at(at.line, at.column, "unclosed __attribute__"));
                }
                _ => {}
            }
            let tok = self.current().clone();
            let word = tok
                .spelling
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
            if !word
                || matches!(
                    tok.kind,
                    TokenKind::Int | TokenKind::Float | TokenKind::String
                )
            {
                return Err(self.error("expected an attribute name"));
            }
            self.bump();
            let name = attr_name(&tok.spelling);
            let mut args = Vec::new();
            if self.current().kind == TokenKind::LParen {
                self.bump();
                while self.current().kind != TokenKind::RParen {
                    args.push(self.parse_attr_arg(&name)?);
                    if !self.match_kind(TokenKind::Comma) {
                        break;
                    }
                }
                self.expect_skip(TokenKind::RParen, "')'")?;
            }
            match attr_class(&name) {
                AttrClass::Known => {}
                AttrClass::Unsupported => {
                    return Err(self.error_at(
                        tok.line,
                        tok.column,
                        format!("attribute '{name}' is not supported"),
                    ));
                }
                AttrClass::Unknown => {
                    self.warn_at(
                        tok.line,
                        tok.column,
                        format!("unknown attribute '{name}' ignored"),
                    );
                }
            }
            if name == "section" && crate::target::current().is_apple() {
                let text = args.iter().find_map(|arg| match arg {
                    AttrArg::Str(text) => Some(text.as_str()),
                    _ => None,
                });
                // A Mach-O section is named "segment,section" (LLVM
                // LangRef, "Global Variables"; the backend rejects others).
                if text.is_some_and(|text| !text.contains(',')) {
                    return Err(self.error_at(
                        tok.line,
                        tok.column,
                        "argument to 'section' attribute is not valid for this target: \
                         mach-o section specifier requires a segment and section \
                         separated by a comma",
                    ));
                }
            }
            self.attr_sink.push(Attr {
                name,
                args,
                line: tok.line,
                column: tok.column,
            });
            if !self.match_kind(TokenKind::Comma) {
                break;
            }
        }
        self.expect_skip(TokenKind::RParen, "')'")?;
        self.expect_skip(TokenKind::RParen, "')'")?;
        Ok(())
    }

    pub(super) fn parse_attr_arg(&mut self, name: &str) -> Result<AttrArg, Diagnostic> {
        let cur = self.current().clone();
        if cur.kind == TokenKind::String {
            let mut bytes = Vec::new();
            while self.current().kind == TokenKind::String {
                let tok = self.advance();
                bytes.extend(tok.bytes.unwrap_or_default());
            }
            return Ok(AttrArg::Str(String::from_utf8_lossy(&bytes).into_owned()));
        }
        if attr_takes_exprs(name) {
            return Ok(AttrArg::Expr(self.parse_assignment()?));
        }
        if cur.kind == TokenKind::Ident
            && matches!(self.lookahead(), TokenKind::Comma | TokenKind::RParen)
        {
            self.bump();
            return Ok(AttrArg::Ident(cur.spelling));
        }
        let mut depth = 0usize;
        loop {
            match self.current().kind {
                TokenKind::LParen => depth += 1,
                TokenKind::RParen if depth == 0 => break,
                TokenKind::RParen => depth -= 1,
                TokenKind::Comma if depth == 0 => break,
                TokenKind::Eof => return Err(self.error("unclosed __attribute__")),
                _ => {}
            }
            self.bump();
        }
        Ok(AttrArg::Other)
    }

    /// A warning, except in a system header.
    pub(super) fn warn_at(&mut self, line: usize, column: usize, message: impl Into<String>) {
        let file = self.file_at(line, column).to_string();
        if file != self.filename && file.starts_with('/') {
            return;
        }
        self.warnings
            .push(Warning(Diagnostic::new(file, line, column, message)));
    }

    /// The alignment `aligned` or `aligned(N)` asks for. With no argument it
    /// is the largest alignment of any type on the target, 16.
    pub(super) fn attr_align(&self, attr: &Attr) -> Result<u32, Diagnostic> {
        let Some(arg) = attr.args.first() else {
            return Ok(16);
        };
        let AttrArg::Expr(expr) = arg else {
            return Err(self.error_at(
                attr.line,
                attr.column,
                "requested alignment is not an integer constant",
            ));
        };
        let value = eval_ice(expr, self.env()).map_err(|_| {
            self.error_at(
                attr.line,
                attr.column,
                "requested alignment is not an integer constant",
            )
        })?;
        let bits = if value.ty.is_signed() {
            let signed = sign_extend(value.bits, value.ty.width());
            if signed <= 0 {
                u64::MAX
            } else {
                u64::try_from(signed).unwrap_or(u64::MAX)
            }
        } else {
            u64::try_from(value.bits & value.ty.mask()).unwrap_or(u64::MAX)
        };
        if !bits.is_power_of_two() || bits > (1 << 28) {
            return Err(self.error_at(
                attr.line,
                attr.column,
                "requested alignment is not a positive power of 2",
            ));
        }
        Ok(bits as u32)
    }

    /// The strictest `aligned` in `attrs`, if any.
    pub(super) fn attrs_align(&self, attrs: &[Attr]) -> Result<Option<u32>, Diagnostic> {
        let mut align: Option<u32> = None;
        for attr in attrs.iter().filter(|attr| attr.name == "aligned") {
            let value = self.attr_align(attr)?;
            align = Some(align.map_or(value, |old| old.max(value)));
        }
        Ok(align)
    }

    pub(super) fn attr_string(attr: &Attr) -> Option<String> {
        attr.args.iter().find_map(|arg| match arg {
            AttrArg::Str(text) => Some(text.clone()),
            _ => None,
        })
    }

    /// The function attributes in `attrs` that change the IR.
    pub(super) fn fn_attrs_of(&self, attrs: &[Attr]) -> FnAttrs {
        let mut out = FnAttrs::default();
        for attr in attrs {
            match attr.name.as_str() {
                "noreturn" => out.noreturn = true,
                "always_inline" => out.always_inline = true,
                "gnu_inline" => out.gnu_inline = true,
                "noinline" => out.noinline = true,
                "cold" => out.cold = true,
                "hot" => out.hot = true,
                "const" => out.const_fn = true,
                "pure" => out.pure = true,
                "weak" => out.weak = true,
                "used" => out.used = true,
                "constructor" => out.constructor = true,
                "destructor" => out.destructor = true,
                "section" => out.section = Self::attr_string(attr),
                "deprecated" => {
                    out.deprecated = Some(Self::attr_string(attr).unwrap_or_default());
                }
                _ => {}
            }
        }
        out
    }

    /// The object attributes in `attrs` that change the IR.
    pub(super) fn var_attrs_of(&self, attrs: &[Attr]) -> Result<VarAttrs, Diagnostic> {
        let mut out = VarAttrs {
            align: self.attrs_align(attrs)?,
            ..VarAttrs::default()
        };
        for attr in attrs {
            match attr.name.as_str() {
                "weak" => out.weak = true,
                "used" => out.used = true,
                "section" => out.section = Self::attr_string(attr),
                _ => {}
            }
        }
        Ok(out)
    }

    /// Apply `mode(...)` to a declared integer type (GCC manual, "Common
    /// Variable Attributes"): the integer of that width with the same
    /// signedness.
    pub(super) fn apply_mode(&self, ty: CType, attrs: &[Attr]) -> Result<CType, Diagnostic> {
        let Some(attr) = attrs.iter().rev().find(|attr| attr.name == "mode") else {
            return Ok(ty);
        };
        let mode = match attr.args.first() {
            Some(AttrArg::Ident(word)) => attr_name(word),
            _ => {
                return Err(self.error_at(
                    attr.line,
                    attr.column,
                    "mode attribute needs a mode name",
                ))
            }
        };
        let width = match mode.as_str() {
            "QI" | "byte" => 8,
            "HI" => 16,
            "SI" => 32,
            "DI" | "word" | "pointer" => 64,
            other => {
                return Err(self.error_at(
                    attr.line,
                    attr.column,
                    format!("mode '{other}' is not supported"),
                ));
            }
        };
        let Some(name) = ty.as_integer().filter(|name| *name != TypeName::Bool) else {
            return Err(self.error_at(
                attr.line,
                attr.column,
                "mode attribute requires an integer type",
            ));
        };
        let signed = name.is_signed();
        let chosen = match (width, signed) {
            (8, true) => TypeName::SChar,
            (8, false) => TypeName::UChar,
            (16, true) => TypeName::Short,
            (16, false) => TypeName::UShort,
            (32, true) => TypeName::Int,
            (32, false) => TypeName::UInt,
            (_, signed) => TypeName::int64(signed),
        };
        Ok(CType::Int(chosen).with_quals(ty.quals()))
    }
}
