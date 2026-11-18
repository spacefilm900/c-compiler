//! Macro expansion (C11 6.10.3): rescanning, argument collection,
//! substitution, `#` and `##`, and hide sets.

use super::*;

impl Processor {
    pub(super) fn expand_all(&mut self, isolated: bool) -> Result<Vec<PpToken>, Diagnostic> {
        let mut out = Vec::new();
        while self.index < self.tokens.len() {
            if !isolated && self.at_directive() {
                self.handle_directive()?;
                continue;
            }
            if self.tokens[self.index].kind == PpKind::Newline {
                if !isolated {
                    out.push(self.take_current());
                }
                self.index += 1;
                continue;
            }
            if self.in_if && self.tokens[self.index].spelling == "defined" {
                // `defined` that came out of a macro in an `#if` line. C11
                // 6.10.1p4 leaves this undefined; GCC's manual documents
                // that it is evaluated like a written `defined`, and system
                // headers rely on it (macOS's `__is_modern_darwin`), so the
                // identifier after it, in parentheses or not, is not
                // expanded.
                out.push(self.take_current());
                self.index += 1;
                if self.index < self.tokens.len() && self.tokens[self.index].is_lparen() {
                    out.push(self.take_current());
                    self.index += 1;
                }
                if self.index < self.tokens.len() && self.tokens[self.index].kind == PpKind::Ident {
                    out.push(self.take_current());
                    self.index += 1;
                }
                continue;
            }
            if self.try_expand(isolated)? {
                continue;
            }
            out.push(self.take_current());
            self.index += 1;
        }
        if !isolated && !self.cond.is_empty() {
            let frame = self.cond.last().unwrap();
            return Err(Diagnostic::new(
                &*frame.display,
                frame.line,
                frame.column,
                "unterminated conditional directive",
            ));
        }
        Ok(out)
    }

    /// The token at the cursor, about to be consumed. Its spelling is moved
    /// out rather than copied: a consumed token is read again only for its
    /// kind (`at_line_start`) or its position.
    pub(super) fn take_current(&mut self) -> PpToken {
        let slot = &mut self.tokens[self.index];
        let spelling = std::mem::take(&mut slot.spelling);
        let mut tok = slot.clone();
        tok.spelling = spelling;
        tok
    }

    pub(super) fn at_line_start(&self) -> bool {
        self.index == 0 || self.tokens[self.index - 1].kind == PpKind::Newline
    }

    pub(super) fn at_directive(&self) -> bool {
        self.index < self.tokens.len() && self.at_line_start() && self.tokens[self.index].is_hash()
    }

    pub(super) fn try_expand(&mut self, isolated: bool) -> Result<bool, Diagnostic> {
        let current = &self.tokens[self.index];
        if current.kind != PpKind::Ident || current.hide.contains(&current.spelling) {
            return Ok(false);
        }
        let found = self.lookup(&current.spelling);
        if matches!(found, Found::No) {
            return Ok(false);
        }
        let tok = current.clone();
        match found {
            Found::No => Ok(false),
            Found::Special => {
                self.note_expansion(&tok)?;
                let repl = special_token(&tok);
                self.index += 1;
                self.splice(vec![repl]);
                Ok(true)
            }
            Found::Macro(mac) => {
                if mac.params.is_some() {
                    let Some(lparen) = self.lparen_index(isolated) else {
                        return Ok(false);
                    };
                    self.note_expansion(&tok)?;
                    self.index = lparen;
                    let args = self.collect_args(&mac, &tok)?;
                    let repl = self.substitute(&mac, &args, &tok)?;
                    self.splice(repl);
                    return Ok(true);
                }
                self.note_expansion(&tok)?;
                let repl = self.expand_object(&mac, &tok)?;
                self.index += 1;
                self.splice(repl);
                Ok(true)
            }
        }
    }

    pub(super) fn note_expansion(&mut self, tok: &PpToken) -> Result<(), Diagnostic> {
        self.expansions += 1;
        if self.expansions > MAX_EXPANSIONS {
            return Err(error_at(tok, "macro expansion is too large"));
        }
        Ok(())
    }

    /// Insert `repl` so that it is read next.
    ///
    /// Tokens before `self.index` have been consumed. Only the one just
    /// before the cursor is still read (by `at_line_start` and
    /// `directive_token`), so the replacement is written over the consumed
    /// prefix, with that token copied in front of it. Inserting in the
    /// middle of the vector instead would move the whole rest of the file
    /// on every macro expansion. When the prefix is too short the vector
    /// is rebuilt with free slots in front.
    pub(super) fn splice(&mut self, repl: Vec<PpToken>) {
        let at = self.index;
        let n = repl.len();
        if at == 0 {
            self.tokens.splice(0..0, repl);
            return;
        }
        if n >= at {
            // Room for this replacement and as much again, so a run of
            // nested expansions at the cursor rebuilds only now and then.
            self.headroom = 2 * n + 64;
            let filler = PpToken {
                kind: PpKind::Placemarker,
                spelling: String::new(),
                display: Rc::from(""),
                file_id: 0,
                line: 0,
                column: 0,
                leading_space: false,
                hide: hide_empty(),
            };
            let tail = self.tokens.split_off(at);
            let previous = self.tokens.pop().expect("at > 0");
            let mut rebuilt = Vec::with_capacity(self.headroom + 1 + tail.len());
            rebuilt.resize(self.headroom, filler);
            rebuilt.push(previous);
            rebuilt.extend(tail);
            self.tokens = rebuilt;
            self.index = self.headroom + 1;
        }
        let at = self.index;
        let start = at - n;
        if n > 0 {
            self.tokens[start - 1] = self.tokens[at - 1].clone();
        }
        for (slot, tok) in self.tokens[start..at].iter_mut().zip(repl) {
            *slot = tok;
        }
        self.index = start;
    }

    pub(super) fn lookup(&self, name: &str) -> Found {
        match self.macros.get(name) {
            Some(MacroEntry::Defined(mac)) => Found::Macro(Rc::clone(mac)),
            Some(MacroEntry::Undefined) => Found::No,
            None if name == "__FILE__" || name == "__LINE__" => Found::Special,
            None => Found::No,
        }
    }

    pub(super) fn is_defined(&self, name: &str) -> bool {
        match self.macros.get(name) {
            Some(MacroEntry::Defined(_)) => true,
            Some(MacroEntry::Undefined) => false,
            None => name == "__FILE__" || name == "__LINE__" || is_has_query(name),
        }
    }

    pub(super) fn lparen_index(&self, isolated: bool) -> Option<usize> {
        let mut index = self.index + 1;
        loop {
            if index >= self.tokens.len() {
                return None;
            }
            if self.tokens[index].kind == PpKind::Newline {
                if !isolated {
                    let mut ahead = index + 1;
                    while ahead < self.tokens.len() && self.tokens[ahead].kind == PpKind::Newline {
                        ahead += 1;
                    }
                    if ahead < self.tokens.len() && self.tokens[ahead].is_hash() {
                        return None;
                    }
                }
                index += 1;
                continue;
            }
            if self.tokens[index].is_lparen() {
                if isolated && !paren_closes(&self.tokens[index..]) {
                    return None;
                }
                return Some(index);
            }
            return None;
        }
    }

    pub(super) fn expand_object(
        &mut self,
        mac: &Macro,
        inv: &PpToken,
    ) -> Result<Vec<PpToken>, Diagnostic> {
        let mut body = mac.body.clone();
        // `__LINE__` and `__FILE__` in a replacement list use the invocation,
        // not the line where the macro was defined.
        retarget(&mut body, inv);
        let ops: Vec<bool> = body.iter().map(|tok| tok.spelling == "##").collect();
        let mut repl = paste_tokens(&body, &ops, inv)?;
        repl.retain(|tok| tok.kind != PpKind::Placemarker);
        paint(&mut repl, &inv.hide, &mac.name);
        if let Some(first) = repl.first_mut() {
            first.leading_space |= inv.leading_space;
        }
        Ok(repl)
    }

    pub(super) fn substitute(
        &mut self,
        mac: &Macro,
        args: &Collected,
        inv: &PpToken,
    ) -> Result<Vec<PpToken>, Diagnostic> {
        let params = mac.params.clone().unwrap_or_default();
        let mut body = mac.body.clone();
        retarget(&mut body, inv);
        let mut raw: HashMap<String, Vec<PpToken>> = HashMap::new();
        for (index, name) in params.iter().enumerate() {
            raw.insert(name.clone(), args.positional[index].clone());
        }
        if mac.variadic {
            raw.insert("__VA_ARGS__".to_string(), args.va.clone());
        }
        let (_need_raw, need_exp) = param_uses(&body, &raw);
        let mut expanded: HashMap<String, Vec<PpToken>> = HashMap::new();
        for name in &need_exp {
            let arg = raw.get(name).cloned().unwrap_or_default();
            expanded.insert(name.clone(), self.expand_list(arg)?);
        }
        let mut subst = Vec::new();
        // C11 6.10.3.3: only a `##` written in the replacement list is the
        // operator. One that came in with an argument is an ordinary token,
        // so `ops[i]` records which tokens of `subst` are the operator.
        let mut ops = Vec::new();
        let mut index = 0;
        while index < body.len() {
            // Tokens pushed by the branches below that `continue` came from
            // an argument, a stringification or a placemarker.
            ops.resize(subst.len(), false);
            if mac.variadic
                && index + 2 < body.len()
                && body[index].is_comma()
                && body[index + 1].spelling == "##"
                && body[index + 2].spelling == "__VA_ARGS__"
            {
                if args.va_omitted {
                    index += 3;
                    continue;
                }
                subst.push(body[index].clone());
                subst.extend(args.va.clone());
                index += 3;
                continue;
            }
            if body[index].spelling == "#"
                && !hashhash_operand(&body, index)
                && index + 1 < body.len()
                && raw.contains_key(&body[index + 1].spelling)
            {
                let mut string = stringify(&raw[&body[index + 1].spelling], &body[index]);
                string.leading_space = body[index].leading_space;
                subst.push(string);
                index += 2;
                continue;
            }
            if raw.contains_key(&body[index].spelling) {
                let name = &body[index].spelling;
                let beside = hashhash_operand(&body, index);
                let source = if beside {
                    raw.get(name).cloned().unwrap_or_default()
                } else {
                    expanded.get(name).cloned().unwrap_or_default()
                };
                if beside && source.is_empty() {
                    subst.push(placemarker(&body[index]));
                } else if !source.is_empty() {
                    let mut copy = source;
                    if let Some(first) = copy.first_mut() {
                        first.leading_space = body[index].leading_space;
                    }
                    subst.extend(copy);
                }
                index += 1;
                continue;
            }
            ops.push(body[index].spelling == "##");
            subst.push(body[index].clone());
            index += 1;
        }
        ops.resize(subst.len(), false);
        let mut repl = paste_tokens(&subst, &ops, inv)?;
        repl.retain(|tok| tok.kind != PpKind::Placemarker);
        paint(&mut repl, &inv.hide, &mac.name);
        if let Some(first) = repl.first_mut() {
            first.leading_space |= inv.leading_space;
        }
        Ok(repl)
    }

    pub(super) fn expand_list(&mut self, list: Vec<PpToken>) -> Result<Vec<PpToken>, Diagnostic> {
        let saved_tokens = std::mem::replace(&mut self.tokens, list);
        let saved_index = self.index;
        self.index = 0;
        let result = self.expand_all(true);
        self.tokens = saved_tokens;
        self.index = saved_index;
        result
    }

    pub(super) fn collect_args(
        &mut self,
        mac: &Macro,
        inv: &PpToken,
    ) -> Result<Collected, Diagnostic> {
        self.index += 1;
        let named = mac.params.as_ref().map(Vec::len).unwrap_or(0);
        let mut parts = vec![Part {
            comma: None,
            tokens: Vec::new(),
        }];
        let mut depth = 0i32;
        loop {
            let tok = self.next_arg_token(inv)?;
            if tok.is_lparen() {
                depth += 1;
                parts.last_mut().unwrap().tokens.push(tok);
            } else if tok.is_rparen() {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                parts.last_mut().unwrap().tokens.push(tok);
            } else if tok.is_comma() && depth == 0 {
                parts.push(Part {
                    comma: Some(tok),
                    tokens: Vec::new(),
                });
            } else {
                parts.last_mut().unwrap().tokens.push(tok);
            }
        }
        self.finish_args(mac, inv, named, parts)
    }

    pub(super) fn finish_args(
        &self,
        mac: &Macro,
        inv: &PpToken,
        named: usize,
        parts: Vec<Part>,
    ) -> Result<Collected, Diagnostic> {
        let empty_call = parts.len() == 1 && parts[0].tokens.is_empty() && parts[0].comma.is_none();
        if empty_call {
            if named == 0 && !mac.variadic {
                return Ok(Collected {
                    positional: Vec::new(),
                    va: Vec::new(),
                    va_omitted: false,
                });
            }
            if named == 0 && mac.variadic {
                return Ok(Collected {
                    positional: Vec::new(),
                    va: Vec::new(),
                    va_omitted: false,
                });
            }
            if named == 1 && !mac.variadic {
                return Ok(Collected {
                    positional: vec![Vec::new()],
                    va: Vec::new(),
                    va_omitted: false,
                });
            }
            return Err(arity_error(inv, &mac.name, 0, named, mac.variadic));
        }
        if !mac.variadic {
            if parts.len() != named {
                return Err(arity_error(inv, &mac.name, parts.len(), named, false));
            }
            return Ok(Collected {
                positional: parts.into_iter().map(|part| part.tokens).collect(),
                va: Vec::new(),
                va_omitted: false,
            });
        }
        if named == 0 {
            return Ok(Collected {
                positional: Vec::new(),
                va: join_va(&parts, 0),
                va_omitted: false,
            });
        }
        if parts.len() < named {
            return Err(arity_error(inv, &mac.name, parts.len(), named, true));
        }
        let va_omitted = parts.len() == named;
        let positional = parts
            .iter()
            .take(named)
            .map(|part| part.tokens.clone())
            .collect();
        Ok(Collected {
            positional,
            va: join_va(&parts, named),
            va_omitted,
        })
    }

    pub(super) fn next_arg_token(&mut self, inv: &PpToken) -> Result<PpToken, Diagnostic> {
        let mut saw_space = false;
        loop {
            if self.index >= self.tokens.len() {
                return Err(error_at(
                    inv,
                    format!(
                        "unterminated argument list invoking macro '{}'",
                        inv.spelling
                    ),
                ));
            }
            if self.at_directive() {
                self.handle_directive()?;
                saw_space = true;
                continue;
            }
            if self.tokens[self.index].kind == PpKind::Newline {
                self.index += 1;
                saw_space = true;
                continue;
            }
            let mut tok = self.tokens[self.index].clone();
            self.index += 1;
            if saw_space {
                tok.leading_space = true;
            }
            return Ok(tok);
        }
    }
}

pub(super) fn escape_string(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if ch == '\\' || ch == '"' {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

pub(super) fn retarget(tokens: &mut [PpToken], inv: &PpToken) {
    for tok in tokens {
        tok.display = inv.display.clone();
        tok.file_id = inv.file_id;
        tok.line = inv.line;
        tok.column = inv.column;
    }
}

pub(super) fn paint(tokens: &mut [PpToken], base: &Hide, name: &str) {
    let extra = hide_with(base, name);
    // Neighbouring tokens usually share one hide set (they came from the
    // same body or argument), so the last union is reused instead of
    // building an equal set again.
    let mut last: Option<(Hide, Hide)> = None;
    for tok in tokens {
        if let Some((from, to)) = &last {
            if Rc::ptr_eq(from, &tok.hide) {
                tok.hide = Rc::clone(to);
                continue;
            }
        }
        let union = hide_union(&tok.hide, &extra);
        last = Some((std::mem::replace(&mut tok.hide, Rc::clone(&union)), union));
    }
}

pub(super) fn paren_closes(tokens: &[PpToken]) -> bool {
    let mut depth = 0i32;
    for tok in tokens {
        if tok.is_lparen() {
            depth += 1;
        } else if tok.is_rparen() {
            depth -= 1;
            if depth == 0 {
                return true;
            }
        }
    }
    false
}

pub(super) fn param_uses(
    body: &[PpToken],
    raw: &HashMap<String, Vec<PpToken>>,
) -> (HashSet<String>, HashSet<String>) {
    let mut need_raw = HashSet::new();
    let mut need_exp = HashSet::new();
    for (index, tok) in body.iter().enumerate() {
        if tok.kind != PpKind::Ident || !raw.contains_key(&tok.spelling) {
            continue;
        }
        let stringified =
            index > 0 && body[index - 1].spelling == "#" && !hashhash_operand(body, index - 1);
        if hashhash_operand(body, index) || stringified {
            need_raw.insert(tok.spelling.clone());
        } else {
            need_exp.insert(tok.spelling.clone());
        }
    }
    (need_raw, need_exp)
}

pub(super) fn hashhash_operand(body: &[PpToken], index: usize) -> bool {
    (index + 1 < body.len() && body[index + 1].spelling == "##")
        || (index > 0 && body[index - 1].spelling == "##")
}

pub(super) fn stringify(arg: &[PpToken], at: &PpToken) -> PpToken {
    let mut text = String::new();
    let mut first = true;
    for tok in arg {
        if tok.kind == PpKind::Placemarker {
            continue;
        }
        if !first && tok.leading_space {
            text.push(' ');
        }
        first = false;
        for ch in tok.spelling.chars() {
            if ch == '\\' || ch == '"' {
                text.push('\\');
            }
            text.push(ch);
        }
    }
    PpToken {
        kind: PpKind::String,
        spelling: format!("\"{text}\""),
        display: at.display.clone(),
        file_id: at.file_id,
        line: at.line,
        column: at.column,
        leading_space: at.leading_space,
        hide: hide_empty(),
    }
}

pub(super) fn placemarker(at: &PpToken) -> PpToken {
    PpToken {
        kind: PpKind::Placemarker,
        spelling: String::new(),
        display: at.display.clone(),
        file_id: at.file_id,
        line: at.line,
        column: at.column,
        leading_space: at.leading_space,
        hide: hide_empty(),
    }
}

/// Apply the `##` operators of a replacement list. `ops[i]` is true where
/// `tokens[i]` is a `##` from the replacement list itself; a `##` that an
/// argument supplied, or that pasting produced, is an ordinary token.
pub(super) fn paste_tokens(
    tokens: &[PpToken],
    ops: &[bool],
    inv: &PpToken,
) -> Result<Vec<PpToken>, Diagnostic> {
    let is_op = |index: usize| ops.get(index).copied().unwrap_or(false);
    let mut out = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        if index + 1 < tokens.len() && is_op(index + 1) {
            if index + 2 >= tokens.len() {
                return Err(error_at(inv, "'##' at the end of a macro replacement list"));
            }
            let mut merged = paste_one(&tokens[index], &tokens[index + 2], inv)?;
            index += 3;
            while index < tokens.len() && is_op(index) {
                if index + 1 >= tokens.len() {
                    return Err(error_at(inv, "'##' at the end of a macro replacement list"));
                }
                merged = paste_one(&merged, &tokens[index + 1], inv)?;
                index += 2;
            }
            out.push(merged);
            continue;
        }
        if is_op(index) {
            return Err(error_at(
                inv,
                "'##' at the beginning of a macro replacement list",
            ));
        }
        out.push(tokens[index].clone());
        index += 1;
    }
    Ok(out)
}

pub(super) fn paste_one(
    left: &PpToken,
    right: &PpToken,
    inv: &PpToken,
) -> Result<PpToken, Diagnostic> {
    if left.kind == PpKind::Placemarker && right.kind == PpKind::Placemarker {
        return Ok(placemarker(left));
    }
    if left.kind == PpKind::Placemarker {
        let mut tok = right.clone();
        tok.leading_space = left.leading_space;
        return Ok(tok);
    }
    if right.kind == PpKind::Placemarker {
        return Ok(left.clone());
    }
    let spelling = format!("{}{}", left.spelling, right.spelling);
    let mut lexed = lex_source(&spelling, &left.display, left.file_id)?;
    lexed.retain(|tok| tok.kind != PpKind::Newline);
    if lexed.len() != 1 {
        return Err(error_at(
            inv,
            format!("pasting '{spelling}' does not produce a valid preprocessing token"),
        ));
    }
    let mut tok = lexed.remove(0);
    tok.line = left.line;
    tok.column = left.column;
    tok.display = left.display.clone();
    tok.file_id = left.file_id;
    tok.leading_space = left.leading_space;
    tok.hide = hide_union(&left.hide, &right.hide);
    Ok(tok)
}

pub(super) fn join_va(parts: &[Part], named: usize) -> Vec<PpToken> {
    let mut out = Vec::new();
    for (index, part) in parts.iter().enumerate().skip(named) {
        if index > named {
            if let Some(comma) = &part.comma {
                out.push(comma.clone());
            }
        }
        out.extend(part.tokens.iter().cloned());
    }
    out
}

pub(super) fn arity_error(
    inv: &PpToken,
    name: &str,
    got: usize,
    need: usize,
    variadic: bool,
) -> Diagnostic {
    let message = if variadic {
        format!("macro '{name}' requires at least {need} argument(s) but {got} were provided")
    } else if got < need {
        format!("macro '{name}' requires {need} argument(s) but {got} were provided")
    } else {
        format!("macro '{name}' passed {got} arguments but takes {need}")
    };
    error_at(inv, message)
}
