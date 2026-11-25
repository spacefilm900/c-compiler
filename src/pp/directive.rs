//! Directives (C11 6.10.1-6.10.9): conditional inclusion, `#include`,
//! `#define` and `#undef`, `#line`, `#error`, `#pragma`, and the
//! `__has_include`-style queries.

use super::*;

impl Processor {
    pub(super) fn handle_directive(&mut self) -> Result<(), Diagnostic> {
        let hash = self.tokens[self.index].clone();
        self.index += 1;
        if self.line_done() {
            self.consume_newline();
            return Ok(());
        }
        let name_tok = self.tokens[self.index].clone();
        if name_tok.kind != PpKind::Ident {
            return Err(error_at(&hash, "invalid preprocessing directive"));
        }
        self.index += 1;
        match name_tok.spelling.as_str() {
            "define" => self.do_define(&hash),
            "undef" => self.do_undef(&hash),
            "include" => self.do_include(&hash, false),
            "include_next" => self.do_include(&hash, true),
            "if" => self.do_if(&hash),
            "ifdef" => self.do_ifdef(&hash, true),
            "ifndef" => self.do_ifdef(&hash, false),
            "elif" => self.do_elif(&hash),
            "else" => self.do_else(&hash),
            "endif" => self.do_endif(&hash),
            "error" => self.do_error(&hash),
            "line" => self.do_line(&hash),
            "pragma" => self.do_pragma(&hash),
            other => Err(error_at(
                &name_tok,
                format!("unknown preprocessing directive '#{other}'"),
            )),
        }
    }

    pub(super) fn do_define(&mut self, at: &PpToken) -> Result<(), Diagnostic> {
        let line = self.take_line()?;
        if line.is_empty() {
            return Err(error_at(at, "macro name missing"));
        }
        if line[0].kind != PpKind::Ident {
            return Err(error_at(&line[0], "macro name must be an identifier"));
        }
        let name = line[0].spelling.clone();
        if name == "defined" {
            return Err(error_at(
                &line[0],
                "'defined' cannot be used as a macro name",
            ));
        }
        // C11 6.10.8p2: nor are they the subject of a #define (undefined
        // behaviour; diagnosed, as for #undef).
        if PREDEFINED_FIXED.contains(&name.as_str()) && !self.files[at.file_id as usize].system {
            return Err(error_at(
                &line[0],
                format!("redefining builtin macro '{name}'"),
            ));
        }
        let rest = &line[1..];
        let parsed = if rest
            .first()
            .is_some_and(|tok| tok.is_lparen() && !tok.leading_space)
        {
            parse_function_header(rest, self.files[at.file_id as usize].system)?
        } else {
            ParsedFn {
                params: None,
                variadic: false,
                body: rest.to_vec(),
            }
        };
        let mac =
            self.macro_from_parts(&name, parsed.params, parsed.variadic, parsed.body, Some(at))?;
        self.install(mac, at, false)
    }

    pub(super) fn do_undef(&mut self, at: &PpToken) -> Result<(), Diagnostic> {
        let line = self.take_line()?;
        if line.len() != 1 || line[0].kind != PpKind::Ident {
            return Err(error_at(at, "expected one identifier after #undef"));
        }
        if line[0].spelling == "defined" {
            return Err(error_at(
                &line[0],
                "'defined' cannot be used as a macro name",
            ));
        }
        // C11 6.10.8p2: the predefined macro names of 6.10.8.1 are not the
        // subject of a #undef.
        if PREDEFINED_FIXED.contains(&line[0].spelling.as_str())
            && !self.files[at.file_id as usize].system
        {
            return Err(error_at(
                &line[0],
                format!("undefining builtin macro '{}'", line[0].spelling),
            ));
        }
        self.macros
            .insert(line[0].spelling.clone(), MacroEntry::Undefined);
        Ok(())
    }

    pub(super) fn do_include(
        &mut self,
        at: &PpToken,
        include_next: bool,
    ) -> Result<(), Diagnostic> {
        let line = self.take_line()?;
        let tokens = if starts_header(&line) {
            line
        } else {
            self.expand_list(line)?
        };
        let (name, quote, consumed) = header_from(&tokens, at)?;
        if consumed != tokens.len() {
            return Err(error_at(at, "extra tokens after the file name"));
        }
        if name.is_empty() {
            return Err(error_at(at, "empty file name"));
        }
        let after = if include_next {
            self.files[at.file_id as usize].found_dir.clone()
        } else {
            None
        };
        self.include_file(&name, quote, at, after.as_deref())
    }

    pub(super) fn do_if(&mut self, at: &PpToken) -> Result<(), Diagnostic> {
        let line = self.take_line()?;
        let value = self.eval_line(line, at)?;
        self.cond.push(cond_frame(at, value));
        if !value {
            self.skip_false_branch()?;
        }
        Ok(())
    }

    pub(super) fn do_ifdef(&mut self, at: &PpToken, want_defined: bool) -> Result<(), Diagnostic> {
        let line = self.take_line()?;
        if line.len() != 1 || line[0].kind != PpKind::Ident {
            return Err(error_at(at, "expected one identifier"));
        }
        let defined = self.is_defined(&line[0].spelling);
        let value = if want_defined { defined } else { !defined };
        self.cond.push(cond_frame(at, value));
        if !value {
            self.skip_false_branch()?;
        }
        Ok(())
    }

    pub(super) fn do_elif(&mut self, at: &PpToken) -> Result<(), Diagnostic> {
        if self.cond.is_empty() {
            return Err(error_at(at, "#elif without #if"));
        }
        self.reject_after_else("elif", at)?;
        self.skip_rest_of_line();
        self.finish_skip_to_endif()
    }

    pub(super) fn do_else(&mut self, at: &PpToken) -> Result<(), Diagnostic> {
        if self.cond.is_empty() {
            return Err(error_at(at, "#else without #if"));
        }
        self.reject_after_else("else", at)?;
        self.expect_end(at)?;
        self.cond.last_mut().unwrap().else_seen = true;
        self.finish_skip_to_endif()
    }

    pub(super) fn do_endif(&mut self, at: &PpToken) -> Result<(), Diagnostic> {
        if self.cond.is_empty() {
            return Err(error_at(at, "#endif without #if"));
        }
        self.expect_end(at)?;
        self.cond.pop();
        Ok(())
    }

    pub(super) fn do_error(&mut self, at: &PpToken) -> Result<(), Diagnostic> {
        let line = self.take_line()?;
        let expanded = self.expand_list(line)?;
        let mut message = String::from("#error");
        for tok in &expanded {
            message.push(' ');
            message.push_str(&tok.spelling);
        }
        Err(error_at(at, message))
    }

    pub(super) fn do_line(&mut self, at: &PpToken) -> Result<(), Diagnostic> {
        let line = self.take_line()?;
        let expanded = self.expand_list(line)?;
        if expanded.is_empty() {
            return Err(error_at(at, "expected a line number"));
        }
        let number = line_number(&expanded[0])?;
        let mut index = 1;
        let new_file = if index < expanded.len() && expanded[index].kind == PpKind::String {
            let name = decode_string(&expanded[index])?;
            index += 1;
            Some(name)
        } else {
            None
        };
        if index != expanded.len() {
            return Err(error_at(&expanded[index], "extra tokens in #line"));
        }
        self.apply_line(at.file_id, number, new_file);
        Ok(())
    }

    pub(super) fn do_pragma(&mut self, at: &PpToken) -> Result<(), Diagnostic> {
        let line = self.take_line()?;
        self.apply_pragma(at, &line)
    }

    /// Carry out a pragma, from a `#pragma` line or a `_Pragma` operator.
    /// `once` is the only pragma this implementation acts on; every other
    /// one, `STDC` pragmas included, is ignored (C11 6.10.6p1 lets an
    /// implementation ignore the ones it does not recognize).
    pub(super) fn apply_pragma(
        &mut self,
        at: &PpToken,
        line: &[PpToken],
    ) -> Result<(), Diagnostic> {
        if line.first().is_some_and(|tok| tok.spelling == "once") {
            if line.len() != 1 {
                return Err(error_at(&line[1], "extra tokens after #pragma once"));
            }
            let canonical = self.files[at.file_id as usize].canonical.clone();
            self.once.insert(canonical);
        }
        Ok(())
    }

    pub(super) fn apply_line(&mut self, file_id: u32, number: usize, new_file: Option<String>) {
        if self.index >= self.tokens.len() {
            return;
        }
        let physical = self.tokens[self.index].line as isize;
        let delta = number as isize - physical;
        for tok in &mut self.tokens[self.index..] {
            if tok.file_id != file_id {
                continue;
            }
            let adjusted = tok.line as isize + delta;
            tok.line = if adjusted < 1 { 1 } else { adjusted as usize };
            if let Some(name) = &new_file {
                tok.display = Rc::from(name.as_str());
            }
        }
    }

    pub(super) fn include_file(
        &mut self,
        name: &str,
        quote: bool,
        at: &PpToken,
        after: Option<&Path>,
    ) -> Result<(), Diagnostic> {
        let (path, found_dir) = self.search(name, quote, at, after)?;
        let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if self.once.contains(&canonical) {
            return Ok(());
        }
        let depth = self.files[at.file_id as usize].depth + 1;
        if depth > self.max_depth {
            return Err(error_at(at, "include nested too deeply"));
        }
        let text = read_source(&path, at)?;
        let file_id = self.files.len() as u32;
        let display: Rc<str> = Rc::from(path.display().to_string());
        let parent = &self.files[at.file_id as usize];
        let system = self.system_dirs.contains(&found_dir)
            || (quote && parent.system && parent.real_path.parent() == Some(found_dir.as_path()));
        self.files.push(FileInfo {
            real_path: path,
            canonical,
            depth,
            found_dir: Some(found_dir),
            system,
        });
        let text = self.phase_one(&text);
        let mut included = crate::timing::lexing(|| lex_source(&text, &display, file_id))?;
        // A header that does not end in a new-line must not continue into the
        // next line of the file that included it. Clang treats that end of
        // file as the end of the line.
        if let Some(last) = included.last() {
            if last.kind != PpKind::Newline {
                let line = last.line;
                let column = last.column.saturating_add(last.spelling.len());
                included.push(PpToken {
                    kind: PpKind::Newline,
                    spelling: "\n".to_string(),
                    display: Rc::clone(&display),
                    file_id,
                    line,
                    column,
                    leading_space: false,
                    hide: hide_empty(),
                });
            }
        }
        self.splice(included);
        Ok(())
    }

    pub(super) fn search(
        &self,
        name: &str,
        quote: bool,
        at: &PpToken,
        after: Option<&Path>,
    ) -> Result<(PathBuf, PathBuf), Diagnostic> {
        let mut dirs = Vec::new();
        if quote {
            if let Some(dir) = self.files[at.file_id as usize].real_path.parent() {
                dirs.push(dir.to_path_buf());
            }
        }
        dirs.extend(self.include_dirs.iter().cloned());
        dirs.extend(self.system_dirs.iter().cloned());
        // `#include_next` resumes after the last copy of the directory that
        // provided the current file. If that directory is not in this search
        // list, the directive behaves like `#include`.
        let start = match after {
            Some(found) => dirs
                .iter()
                .rposition(|dir| dir == found)
                .map_or(0, |index| index + 1),
            None => 0,
        };
        for dir in dirs.into_iter().skip(start) {
            let path = dir.join(name);
            if path.is_file() {
                return Ok((path, dir));
            }
        }
        Err(error_at(at, format!("'{name}' file not found")))
    }

    pub(super) fn skip_false_branch(&mut self) -> Result<(), Diagnostic> {
        loop {
            match self.skip_until_clause()? {
                Clause::Elif => {
                    let at = self.directive_token();
                    self.reject_after_else("elif", &at)?;
                    let seen = self.cond.last().unwrap().seen_true;
                    if seen {
                        self.skip_rest_of_line();
                        continue;
                    }
                    let line = self.take_line()?;
                    let value = self.eval_line(line, &at)?;
                    let frame = self.cond.last_mut().unwrap();
                    frame.seen_true = value;
                    frame.active = value;
                    if value {
                        return Ok(());
                    }
                }
                Clause::Else => {
                    let at = self.directive_token();
                    self.reject_after_else("else", &at)?;
                    self.expect_end(&at)?;
                    let frame = self.cond.last_mut().unwrap();
                    frame.else_seen = true;
                    if !frame.seen_true {
                        frame.active = true;
                        frame.seen_true = true;
                        return Ok(());
                    }
                }
                Clause::Endif => {
                    let at = cond_token(self.cond.last().unwrap());
                    self.expect_end(&at)?;
                    self.cond.pop();
                    return Ok(());
                }
            }
        }
    }

    pub(super) fn finish_skip_to_endif(&mut self) -> Result<(), Diagnostic> {
        loop {
            match self.skip_until_clause()? {
                Clause::Elif => {
                    let at = self.directive_token();
                    self.reject_after_else("elif", &at)?;
                    self.skip_rest_of_line();
                }
                Clause::Else => {
                    let at = self.directive_token();
                    self.reject_after_else("else", &at)?;
                    self.cond.last_mut().unwrap().else_seen = true;
                    self.skip_rest_of_line();
                }
                Clause::Endif => {
                    let at = cond_token(self.cond.last().unwrap());
                    self.expect_end(&at)?;
                    self.cond.pop();
                    return Ok(());
                }
            }
        }
    }

    pub(super) fn skip_until_clause(&mut self) -> Result<Clause, Diagnostic> {
        let mut depth = 0i32;
        let mut nested_else = Vec::new();
        loop {
            self.skip_to_hash()?;
            self.index += 1;
            if self.line_done() {
                self.consume_newline();
                continue;
            }
            let name_tok = self.tokens[self.index].clone();
            if name_tok.kind != PpKind::Ident {
                self.skip_rest_of_line();
                continue;
            }
            let name = name_tok.spelling.clone();
            self.index += 1;
            match name.as_str() {
                "if" | "ifdef" | "ifndef" => {
                    depth += 1;
                    nested_else.push(false);
                    self.skip_rest_of_line();
                }
                "elif" if depth == 0 => return Ok(Clause::Elif),
                "elif" => {
                    if nested_else.last() == Some(&true) {
                        return Err(error_at(&name_tok, "#elif after #else"));
                    }
                    self.skip_rest_of_line();
                }
                "else" if depth == 0 => return Ok(Clause::Else),
                "else" => {
                    if nested_else.last() == Some(&true) {
                        return Err(error_at(&name_tok, "#else after #else"));
                    }
                    if let Some(seen) = nested_else.last_mut() {
                        *seen = true;
                    }
                    self.skip_rest_of_line();
                }
                "endif" if depth == 0 => return Ok(Clause::Endif),
                "endif" => {
                    depth -= 1;
                    nested_else.pop();
                    self.skip_rest_of_line();
                }
                _ => self.skip_rest_of_line(),
            }
        }
    }

    pub(super) fn directive_token(&self) -> PpToken {
        self.tokens
            .get(self.index.saturating_sub(1))
            .cloned()
            .unwrap_or_else(|| cond_token(self.cond.last().unwrap()))
    }

    pub(super) fn reject_after_else(&self, kind: &str, at: &PpToken) -> Result<(), Diagnostic> {
        if self.cond.last().is_some_and(|frame| frame.else_seen) {
            return Err(error_at(at, format!("#{kind} after #else")));
        }
        Ok(())
    }

    pub(super) fn skip_to_hash(&mut self) -> Result<(), Diagnostic> {
        loop {
            if self.index >= self.tokens.len() {
                let frame = self.cond.last().unwrap();
                return Err(Diagnostic::new(
                    &*frame.display,
                    frame.line,
                    frame.column,
                    "unterminated conditional directive",
                ));
            }
            if self.at_directive() {
                return Ok(());
            }
            self.index += 1;
        }
    }

    pub(super) fn eval_line(
        &mut self,
        tokens: Vec<PpToken>,
        at: &PpToken,
    ) -> Result<bool, Diagnostic> {
        let tokens = self.replace_has_queries(tokens, at)?;
        let replaced = replace_defined(&tokens, |name| self.is_defined(name))?;
        self.in_if = true;
        let expanded = self.expand_list(replaced);
        self.in_if = false;
        let expanded = expanded?;
        // `defined` and the feature queries can also come from a macro
        // (macOS's `__is_modern_darwin`, glibc's `__glibc_has_attribute`).
        let expanded = replace_defined(&expanded, |name| self.is_defined(name))?;
        let expanded = self.replace_has_queries(expanded, at)?;
        let zeroed = replace_idents(expanded);
        if zeroed.is_empty() {
            return Err(error_at(at, "expected an expression"));
        }
        let expr = parse_if_expr(&zeroed)?;
        let value = eval_expr(&expr)?;
        Ok(value.bits != 0)
    }

    /// Replace the feature queries that system headers test in `#if`
    /// (Apple's SDK among them; C23 6.10.1 standardises `__has_include`)
    /// with 0 or 1, before macro expansion so that a header name in
    /// `__has_include(<a/b.h>)` is not expanded. A query the program has
    /// defined or undefined as a macro is left to that macro.
    ///
    /// `__has_include` and `__has_include_next` search the include path the
    /// same way `#include` would, and `__has_builtin` answers 1 for the
    /// builtins this compiler implements. The other queries answer 0: this
    /// compiler claims no Clang feature, extension, or attribute, so a
    /// header takes its plain C path (the one a compiler without the query
    /// would take).
    pub(super) fn replace_has_queries(
        &mut self,
        tokens: Vec<PpToken>,
        at: &PpToken,
    ) -> Result<Vec<PpToken>, Diagnostic> {
        let mut out = Vec::with_capacity(tokens.len());
        let mut index = 0;
        while index < tokens.len() {
            let tok = &tokens[index];
            let query = tok.kind == PpKind::Ident
                && is_has_query(&tok.spelling)
                && !self.macros.contains_key(&tok.spelling)
                && !(index > 0 && tokens[index - 1].spelling == "defined")
                && !(index > 1
                    && tokens[index - 1].is_lparen()
                    && tokens[index - 2].spelling == "defined");
            if !query {
                out.push(tok.clone());
                index += 1;
                continue;
            }
            let name = tok.spelling.as_str();
            if index + 1 >= tokens.len() || !tokens[index + 1].is_lparen() {
                return Err(error_at(tok, format!("missing '(' after '{name}'")));
            }
            let open = index + 1;
            let mut depth = 0usize;
            let mut close = None;
            for (offset, inner) in tokens[open..].iter().enumerate() {
                if inner.is_lparen() {
                    depth += 1;
                } else if inner.is_rparen() {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(open + offset);
                        break;
                    }
                }
            }
            let Some(close) = close else {
                return Err(error_at(tok, format!("missing ')' after '{name}'")));
            };
            let inside = &tokens[open + 1..close];
            let value = if name.starts_with("__has_include") {
                // `__has_include(MACRO)`: the file name comes from a macro,
                // as for `#include MACRO` (C11 6.10.2p4).
                let expanded;
                let inside = if starts_header(inside) {
                    inside
                } else {
                    expanded = self.expand_list(inside.to_vec())?;
                    &expanded[..]
                };
                let (header, quote, consumed) = header_from(inside, tok)?;
                if consumed != inside.len() || header.is_empty() {
                    return Err(error_at(tok, format!("'{name}' expects one file name")));
                }
                let after = if name == "__has_include_next" {
                    self.files[at.file_id as usize].found_dir.clone()
                } else {
                    None
                };
                self.search(&header, quote, at, after.as_deref()).is_ok()
            } else if name == "__has_builtin" {
                // 1 for a builtin this compiler implements, so a header
                // uses it instead of a fallback (SDL's byte swaps fall back
                // to inline assembly on x86_64).
                matches!(inside, [arg] if arg.kind == PpKind::Ident
                    && is_implemented_builtin(&arg.spelling))
            } else {
                false
            };
            out.push(number_token(tok, if value { "1" } else { "0" }));
            index = close + 1;
        }
        Ok(out)
    }

    pub(super) fn install(
        &mut self,
        mac: Macro,
        at: &PpToken,
        force: bool,
    ) -> Result<(), Diagnostic> {
        if !force {
            if let Some(MacroEntry::Defined(previous)) = self.macros.get(&mac.name) {
                if !same_macro(previous, &mac) {
                    return Err(error_at(at, format!("macro '{}' redefined", mac.name)));
                }
                return Ok(());
            }
        }
        self.macros
            .insert(mac.name.clone(), MacroEntry::Defined(Rc::new(mac)));
        Ok(())
    }

    pub(super) fn macro_from_parts(
        &self,
        name: &str,
        params: Option<Vec<String>>,
        variadic: bool,
        body: Vec<PpToken>,
        at: Option<&PpToken>,
    ) -> Result<Macro, Diagnostic> {
        let body = normalize_body(body);
        if let Some(at) = at {
            validate_body(&body, params.as_deref(), variadic, at)?;
        } else {
            let marker = command_token();
            validate_body(&body, params.as_deref(), variadic, &marker)?;
        }
        Ok(Macro {
            name: name.to_string(),
            params,
            variadic,
            body,
        })
    }

    pub(super) fn take_line(&mut self) -> Result<Vec<PpToken>, Diagnostic> {
        let mut tokens = Vec::new();
        while self.index < self.tokens.len() && self.tokens[self.index].kind != PpKind::Newline {
            tokens.push(self.tokens[self.index].clone());
            self.index += 1;
        }
        self.consume_newline();
        Ok(tokens)
    }

    pub(super) fn skip_rest_of_line(&mut self) {
        while self.index < self.tokens.len() && self.tokens[self.index].kind != PpKind::Newline {
            self.index += 1;
        }
        self.consume_newline();
    }

    pub(super) fn expect_end(&mut self, at: &PpToken) -> Result<(), Diagnostic> {
        let rest = self.take_line()?;
        if rest.is_empty() {
            Ok(())
        } else {
            Err(error_at(at, "extra tokens at the end of this directive"))
        }
    }

    pub(super) fn line_done(&self) -> bool {
        self.index >= self.tokens.len() || self.tokens[self.index].kind == PpKind::Newline
    }

    pub(super) fn consume_newline(&mut self) {
        if self.index < self.tokens.len() && self.tokens[self.index].kind == PpKind::Newline {
            self.index += 1;
        }
    }
}

pub(super) fn cond_frame(at: &PpToken, value: bool) -> Cond {
    Cond {
        active: value,
        seen_true: value,
        else_seen: false,
        display: at.display.clone(),
        line: at.line,
        column: at.column,
    }
}

pub(super) fn cond_token(frame: &Cond) -> PpToken {
    PpToken {
        kind: PpKind::Punct,
        spelling: "#".to_string(),
        display: frame.display.clone(),
        file_id: 0,
        line: frame.line,
        column: frame.column,
        leading_space: false,
        hide: hide_empty(),
    }
}

pub(super) fn command_token() -> PpToken {
    PpToken {
        kind: PpKind::Ident,
        spelling: String::new(),
        display: Rc::from("<command-line>"),
        file_id: 0,
        line: 1,
        column: 1,
        leading_space: false,
        hide: hide_empty(),
    }
}

pub(super) fn special_token(tok: &PpToken) -> PpToken {
    let mut repl = tok.clone();
    repl.hide = hide_empty();
    repl.leading_space = tok.leading_space;
    if tok.spelling == "__LINE__" {
        repl.kind = PpKind::Number;
        repl.spelling = tok.line.to_string();
    } else {
        repl.kind = PpKind::String;
        repl.spelling = format!("\"{}\"", escape_string(&tok.display));
    }
    repl
}

/// Whether `__has_builtin(name)` answers 1: the inline builtins, the
/// builtins that forward to a library function, the floating-format ones
/// (`__builtin_inf`, `__builtin_huge_val`, `__builtin_nan` and their
/// variants), and the ones the parser and semantic analysis handle by name.
pub(super) fn is_implemented_builtin(name: &str) -> bool {
    crate::builtins::Inline::from_name(name).is_some()
        || crate::builtins::library(name).is_some()
        || crate::builtins::AtomicOp::from_name(name).is_some()
        || crate::builtins::checked_builtin(name).is_some()
        || crate::builtins::is_object_size_builtin(name)
        || crate::float::FpFormatBuiltin::from_name(name).is_some()
        || matches!(
            name,
            "__builtin_choose_expr"
                | "__builtin_constant_p"
                | "__builtin_isgreater"
                | "__builtin_isgreaterequal"
                | "__builtin_isless"
                | "__builtin_islessequal"
                | "__builtin_islessgreater"
                | "__builtin_isunordered"
                | "__builtin_memcpy"
                | "__builtin_memmove"
                | "__builtin_memset"
                | "__builtin_offsetof"
                | "__builtin_signbit"
                | "__builtin_signbitf"
                | "__builtin_signbitl"
                | "__builtin_types_compatible_p"
                | "__builtin_va_arg"
                | "__builtin_va_copy"
                | "__builtin_va_end"
                | "__builtin_va_start"
        )
}

/// The `#if` feature queries handled by `replace_has_queries`.
pub(super) fn is_has_query(name: &str) -> bool {
    matches!(
        name,
        "__has_include"
            | "__has_include_next"
            | "__has_feature"
            | "__has_extension"
            | "__has_attribute"
            | "__has_c_attribute"
            | "__has_cpp_attribute"
            | "__has_declspec_attribute"
            | "__has_builtin"
            | "__has_warning"
            | "__building_module"
    )
}

pub(super) fn replace_defined(
    tokens: &[PpToken],
    defined: impl Fn(&str) -> bool,
) -> Result<Vec<PpToken>, Diagnostic> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        if tokens[index].kind == PpKind::Ident && tokens[index].spelling == "defined" {
            let at = &tokens[index];
            index += 1;
            let name = if index < tokens.len() && tokens[index].is_lparen() {
                if index + 2 >= tokens.len()
                    || tokens[index + 1].kind != PpKind::Ident
                    || !tokens[index + 2].is_rparen()
                {
                    return Err(error_at(at, "operator 'defined' requires an identifier"));
                }
                let name = tokens[index + 1].spelling.clone();
                index += 3;
                name
            } else if index < tokens.len() && tokens[index].kind == PpKind::Ident {
                let name = tokens[index].spelling.clone();
                index += 1;
                name
            } else {
                return Err(error_at(at, "operator 'defined' requires an identifier"));
            };
            out.push(number_token(at, if defined(&name) { "1" } else { "0" }));
        } else {
            out.push(tokens[index].clone());
            index += 1;
        }
    }
    Ok(out)
}

pub(super) fn number_token(at: &PpToken, spelling: &str) -> PpToken {
    PpToken {
        kind: PpKind::Number,
        spelling: spelling.to_string(),
        display: at.display.clone(),
        file_id: at.file_id,
        line: at.line,
        column: at.column,
        leading_space: at.leading_space,
        hide: hide_empty(),
    }
}

pub(super) fn replace_idents(tokens: Vec<PpToken>) -> Vec<PpToken> {
    tokens
        .into_iter()
        .map(|mut tok| {
            if tok.kind == PpKind::Ident {
                tok.kind = PpKind::Number;
                tok.spelling = "0".to_string();
                tok.hide = hide_empty();
            }
            tok
        })
        .collect()
}

pub(super) fn starts_header(tokens: &[PpToken]) -> bool {
    tokens
        .first()
        .is_some_and(|tok| tok.kind == PpKind::String || tok.spelling == "<")
}

pub(super) fn header_from(
    tokens: &[PpToken],
    at: &PpToken,
) -> Result<(String, bool, usize), Diagnostic> {
    if tokens.is_empty() {
        return Err(error_at(at, "expected a file name"));
    }
    if tokens[0].kind == PpKind::String {
        return Ok((decode_string(&tokens[0])?, true, 1));
    }
    if tokens[0].spelling == "<" {
        let mut name = String::new();
        let mut index = 1;
        while index < tokens.len() {
            if tokens[index].kind == PpKind::Punct && tokens[index].spelling == ">" {
                return Ok((name, false, index + 1));
            }
            if tokens[index].leading_space {
                name.push(' ');
            }
            name.push_str(&tokens[index].spelling);
            index += 1;
        }
        return Err(error_at(at, "missing '>' after the file name"));
    }
    Err(error_at(at, "expected \"filename\" or <filename>"))
}

pub(super) fn decode_string(tok: &PpToken) -> Result<String, Diagnostic> {
    match crate::lexer::lex_shared(&tok.spelling, &tok.display) {
        Ok(mut tokens) => {
            tokens.pop();
            if tokens.len() != 1 {
                return Err(error_at(tok, "invalid string literal"));
            }
            let bytes = tokens[0].bytes.clone().unwrap_or_default();
            String::from_utf8(bytes).map_err(|_| error_at(tok, "file name is not valid UTF-8"))
        }
        Err(mut err) => {
            err.filename = tok.display.to_string();
            err.line = tok.line;
            err.column = tok.column;
            Err(err)
        }
    }
}

/// C11 6.10.4p3: the line number is a digit sequence, read as a decimal
/// integer (a leading zero does not make it octal), from 1 to 2147483647.
/// A suffix, a hexadecimal constant, or a character constant is not one.
pub(super) fn line_number(tok: &PpToken) -> Result<usize, Diagnostic> {
    if tok.kind != PpKind::Number || !tok.spelling.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(error_at(
            tok,
            "#line directive requires a simple digit sequence",
        ));
    }
    let mut value: u64 = 0;
    for byte in tok.spelling.bytes() {
        value = value * 10 + u64::from(byte - b'0');
        if value > 2_147_483_647 {
            return Err(error_at(tok, "line number is out of range"));
        }
    }
    if value == 0 {
        return Err(error_at(tok, "line number is out of range"));
    }
    Ok(value as usize)
}

pub(super) fn read_source(path: &Path, at: &PpToken) -> Result<String, Diagnostic> {
    match std::fs::read(path) {
        Ok(bytes) => String::from_utf8(bytes)
            .map_err(|_| error_at(at, format!("{} is not valid UTF-8", path.display()))),
        Err(err) => Err(error_at(
            at,
            format!("cannot read {}: {err}", path.display()),
        )),
    }
}

pub(super) struct ParsedFn {
    pub(super) params: Option<Vec<String>>,
    pub(super) variadic: bool,
    pub(super) body: Vec<PpToken>,
}

/// `system` allows GNU's named variadic parameter (`#define F(args...)`,
/// in GCC's manual under "Variadic Macros") in a system header, where
/// Linux's own headers use it. In a program it stays an error, since C11
/// 6.10.3 has only `...`. The name is rewritten to `__VA_ARGS__` in the
/// body.
pub(super) fn parse_function_header(
    rest: &[PpToken],
    system: bool,
) -> Result<ParsedFn, Diagnostic> {
    let mut index = 1;
    let mut params = Vec::new();
    let mut variadic = false;
    let mut va_name: Option<String> = None;
    if index < rest.len() && rest[index].is_rparen() {
        index += 1;
        return Ok(ParsedFn {
            params: Some(params),
            variadic: false,
            body: rest[index..].to_vec(),
        });
    }
    loop {
        if index >= rest.len() {
            return Err(error_at(
                rest.last().unwrap(),
                "missing ')' in macro parameter list",
            ));
        }
        if rest[index].spelling == "..." && rest[index].kind == PpKind::Punct {
            variadic = true;
            index += 1;
            if index >= rest.len() || !rest[index].is_rparen() {
                return Err(error_at(&rest[index - 1], "expected ')' after '...'"));
            }
            index += 1;
            break;
        }
        if rest[index].kind != PpKind::Ident {
            return Err(error_at(&rest[index], "expected a parameter name"));
        }
        let name = rest[index].spelling.clone();
        if name == "__VA_ARGS__" {
            return Err(error_at(
                &rest[index],
                "'__VA_ARGS__' is not a parameter name",
            ));
        }
        if params.iter().any(|param| param == &name) {
            return Err(error_at(
                &rest[index],
                format!("duplicate macro parameter '{name}'"),
            ));
        }
        params.push(name);
        index += 1;
        if system
            && index < rest.len()
            && rest[index].kind == PpKind::Punct
            && rest[index].spelling == "..."
        {
            va_name = params.pop();
            variadic = true;
            index += 1;
            if index >= rest.len() || !rest[index].is_rparen() {
                return Err(error_at(&rest[index - 1], "expected ')' after '...'"));
            }
            index += 1;
            break;
        }
        if index >= rest.len() {
            return Err(error_at(
                rest.last().unwrap(),
                "missing ')' in macro parameter list",
            ));
        }
        if rest[index].is_rparen() {
            index += 1;
            break;
        }
        if rest[index].is_comma() {
            index += 1;
            if index < rest.len() && rest[index].spelling == "..." {
                continue;
            }
            continue;
        }
        return Err(error_at(
            &rest[index],
            "expected ',' or ')' in macro parameter list",
        ));
    }
    let mut body = rest[index..].to_vec();
    if let Some(va_name) = va_name {
        for tok in &mut body {
            if tok.kind == PpKind::Ident && tok.spelling == va_name {
                tok.spelling = "__VA_ARGS__".to_string();
            }
        }
    }
    Ok(ParsedFn {
        params: Some(params),
        variadic,
        body,
    })
}

/// The macros of C11 6.10.8.1 that a program cannot #undef.
const PREDEFINED_FIXED: [&str; 7] = [
    "__DATE__",
    "__FILE__",
    "__LINE__",
    "__STDC__",
    "__STDC_HOSTED__",
    "__STDC_VERSION__",
    "__TIME__",
];

pub(super) fn normalize_body(mut body: Vec<PpToken>) -> Vec<PpToken> {
    if let Some(first) = body.first_mut() {
        first.leading_space = false;
    }
    for tok in &mut body {
        tok.hide = hide_empty();
    }
    body
}

pub(super) fn validate_body(
    body: &[PpToken],
    params: Option<&[String]>,
    variadic: bool,
    at: &PpToken,
) -> Result<(), Diagnostic> {
    if body.first().is_some_and(|tok| tok.spelling == "##") {
        return Err(error_at(
            at,
            "'##' at the beginning of a macro replacement list",
        ));
    }
    if body.last().is_some_and(|tok| tok.spelling == "##") {
        return Err(error_at(at, "'##' at the end of a macro replacement list"));
    }
    // C11 6.10.3p5: `__VA_ARGS__` belongs only to a variadic macro.
    if !variadic
        && body
            .iter()
            .any(|tok| tok.kind == PpKind::Ident && tok.spelling == "__VA_ARGS__")
    {
        return Err(error_at(
            at,
            "__VA_ARGS__ can only appear in the expansion of a variadic macro",
        ));
    }
    let Some(params) = params else {
        return Ok(());
    };
    for (index, tok) in body.iter().enumerate() {
        if tok.spelling != "#" || hashhash_operand(body, index) {
            continue;
        }
        let Some(next) = body.get(index + 1) else {
            return Err(error_at(at, "'#' is not followed by a macro parameter"));
        };
        let ok = params.iter().any(|name| name == &next.spelling)
            || (variadic && next.spelling == "__VA_ARGS__");
        if !ok {
            return Err(error_at(at, "'#' is not followed by a macro parameter"));
        }
    }
    Ok(())
}

pub(super) fn same_macro(left: &Macro, right: &Macro) -> bool {
    left.params == right.params
        && left.variadic == right.variadic
        && same_body(&left.body, &right.body)
}

pub(super) fn same_body(left: &[PpToken], right: &[PpToken]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    for (index, (a, b)) in left.iter().zip(right).enumerate() {
        if a.kind != b.kind || a.spelling != b.spelling {
            return false;
        }
        if index > 0 && a.leading_space != b.leading_space {
            return false;
        }
    }
    true
}

pub(super) fn body_tokens(source: &str) -> Result<Vec<PpToken>, Diagnostic> {
    let mut tokens = lex_source(source, "<built-in>", 0)?;
    tokens.retain(|tok| tok.kind != PpKind::Newline);
    Ok(tokens)
}

pub(super) fn is_ident_text(text: &str) -> bool {
    let mut chars = text.chars();
    let ok = |ch: char, initial: bool| {
        ch == '_'
            || if ch.is_ascii() {
                ch.is_ascii_alphabetic() || (!initial && ch.is_ascii_digit())
            } else {
                crate::ucn::allowed_in_identifier(u32::from(ch), initial)
            }
    };
    chars.next().is_some_and(|ch| ok(ch, true)) && chars.all(|ch| ok(ch, false))
}
