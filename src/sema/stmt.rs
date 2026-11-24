//! Statements: blocks, labels, `switch` and `case` checking, loops and
//! unevaluated operands.

use super::*;

pub(super) fn case_le(left: u128, right: u128, ty: TypeName) -> bool {
    if ty.is_signed() {
        sign_extend(left, ty.width()) <= sign_extend(right, ty.width())
    } else {
        (left & ty.mask()) <= (right & ty.mask())
    }
}

pub(super) fn case_in_range(value: u128, low: u128, high: u128, ty: TypeName) -> bool {
    case_le(low, value, ty) && case_le(value, high, ty)
}

pub(super) fn ranges_overlap(
    low: u128,
    high: u128,
    other_low: u128,
    other_high: u128,
    ty: TypeName,
) -> bool {
    case_le(low, other_high, ty) && case_le(other_low, high, ty)
}

impl Sema<'_> {
    pub(super) fn check_compound(
        &mut self,
        compound: &mut Compound,
        new_scope: bool,
    ) -> Result<(), Diagnostic> {
        if new_scope {
            self.push();
        }
        for item in &mut compound.items {
            self.check_stmt(item)?;
        }
        if new_scope {
            self.pop();
        }
        Ok(())
    }

    pub(super) fn check_stmt(&mut self, stmt: &mut Stmt) -> Result<(), Diagnostic> {
        match stmt {
            Stmt::Compound(compound) => self.check_compound(compound, true),
            Stmt::Decl(decl) => self.check_decl(decl),
            Stmt::Return { expr, line, column } => self.check_return(expr.as_mut(), *line, *column),
            Stmt::If {
                cond,
                then_stmt,
                else_stmt,
                ..
            } => {
                self.check_expr(cond)?;
                self.decay(cond)?;
                self.require_scalar(cond)?;
                self.check_stmt(then_stmt)?;
                if let Some(else_stmt) = else_stmt {
                    self.check_stmt(else_stmt)?;
                }
                Ok(())
            }
            Stmt::While { cond, body, .. } => {
                self.check_expr(cond)?;
                self.decay(cond)?;
                self.require_scalar(cond)?;
                self.loop_depth += 1;
                self.check_stmt(body)?;
                self.loop_depth -= 1;
                Ok(())
            }
            Stmt::Do { body, cond, .. } => {
                self.loop_depth += 1;
                self.check_stmt(body)?;
                self.loop_depth -= 1;
                self.check_expr(cond)?;
                self.decay(cond)?;
                self.require_scalar(cond)
            }
            Stmt::Switch {
                cond,
                body,
                promoted,
                ..
            } => self.check_switch(cond, body, promoted),
            Stmt::Case {
                expr,
                value,
                high,
                high_value,
                stmt,
                line,
                column,
            } => self.check_case(
                expr,
                value,
                high.as_mut(),
                high_value,
                stmt,
                (*line, *column),
            ),
            Stmt::Default { stmt, line, column } => {
                let line = *line;
                let column = *column;
                if self.switches.is_empty() {
                    return self.err(line, column, self.no_switch_message("default"));
                }
                if self.switches.last().expect("switch").has_default {
                    return self.err(line, column, "multiple default labels in one switch");
                }
                self.switches.last_mut().expect("switch").has_default = true;
                self.check_stmt(stmt)
            }
            Stmt::FuncDecl(func) => self.declare_block_func(func),
            Stmt::For {
                init,
                cond,
                step,
                body,
                ..
            } => self.check_for(init, cond.as_mut(), step.as_mut(), body),
            Stmt::Expr { expr, .. } => {
                if let Some(expr) = expr {
                    self.check_expr(expr)?;
                    self.decay(expr)?;
                }
                Ok(())
            }
            Stmt::Break { line, column } => {
                if self.loop_depth == 0 && self.switches.is_empty() && !self.switch_outside {
                    return self.err(*line, *column, "'break' is not inside a loop or switch");
                }
                Ok(())
            }
            Stmt::Continue { line, column } => {
                if self.loop_depth == 0 {
                    return self.err(*line, *column, "'continue' is not inside a loop");
                }
                Ok(())
            }
            Stmt::Label { stmt, .. } => self.check_stmt(stmt),
            Stmt::Goto { name, line, column } => {
                if !self.labels.contains_key(name) {
                    return self.err(*line, *column, self.missing_label(name));
                }
                Ok(())
            }
            Stmt::GotoPtr { expr, line, column } => self.check_goto_ptr(expr, *line, *column),
            Stmt::Asm { .. } => Ok(()),
        }
    }

    /// `return` with or without a value (C11 6.8.6.4).
    fn check_return(
        &mut self,
        expr: Option<&mut Expr>,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        match expr {
            None => {
                if !self.current_ret.is_void() {
                    return self.err(
                        line,
                        column,
                        format!(
                            "return with no value in a function returning '{}'",
                            self.current_ret.display()
                        ),
                    );
                }
                Ok(())
            }
            Some(expr) => {
                self.check_expr(expr)?;
                self.decay(expr)?;
                if self.current_ret.is_void() {
                    return self.err(
                        line,
                        column,
                        "return with a value in a function returning 'void'",
                    );
                }
                self.can_assign(&self.current_ret.clone(), expr)
            }
        }
    }

    /// A `switch` statement: the controlling expression is promoted
    /// (C11 6.8.4.2p5) and the body sees a fresh set of case labels.
    fn check_switch(
        &mut self,
        cond: &mut Expr,
        body: &mut Stmt,
        promoted: &mut Option<TypeName>,
    ) -> Result<(), Diagnostic> {
        self.check_expr(cond)?;
        self.decay(cond)?;
        self.require_integer(cond)?;
        let prom = cond
            .ty()
            .expect("typed switch condition")
            .as_integer()
            .expect("integer switch condition")
            .promote();
        *promoted = Some(prom);
        self.switches.push(SwitchCtx {
            promoted: prom,
            seen: HashSet::new(),
            ranges: Vec::new(),
            has_default: false,
        });
        self.check_stmt(body)?;
        self.switches.pop();
        Ok(())
    }

    /// A `case` label, or a GNU `case low ... high` range, in the
    /// innermost switch.
    fn check_case(
        &mut self,
        expr: &mut Expr,
        value: &mut Option<u128>,
        high: Option<&mut Expr>,
        high_value: &mut Option<u128>,
        stmt: &mut Stmt,
        (line, column): (usize, usize),
    ) -> Result<(), Diagnostic> {
        if self.switches.is_empty() {
            return self.err(line, column, self.no_switch_message("case"));
        }
        self.check_expr(expr)?;
        self.decay(expr)?;
        let prom = self.switches.last().expect("switch").promoted;
        let bits = self.case_value(expr, prom)?;
        if let Some(high) = high {
            self.check_expr(high)?;
            self.decay(high)?;
            let high_bits = self.case_value(high, prom)?;
            self.note_case_range(bits, high_bits, prom, line, column)?;
            *value = Some(bits);
            *high_value = Some(high_bits);
            return self.check_stmt(stmt);
        }
        let in_range = self
            .switches
            .last()
            .expect("switch")
            .ranges
            .iter()
            .any(|(lo, hi)| case_in_range(bits, *lo, *hi, prom));
        if in_range || !self.switches.last_mut().expect("switch").seen.insert(bits) {
            return self.err(
                line,
                column,
                format!("duplicate case value '{}'", prom.format_bits(bits)),
            );
        }
        *value = Some(bits);
        self.check_stmt(stmt)
    }

    /// Record the range `bits ... high_bits` in the innermost switch, after
    /// checking that it overlaps no earlier label. An empty range is only
    /// warned about.
    fn note_case_range(
        &mut self,
        bits: u128,
        high_bits: u128,
        prom: TypeName,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        if !case_le(bits, high_bits, prom) {
            // GCC and Clang warn and drop the label.
            self.warn(line, column, "empty case range specified");
            return Ok(());
        }
        let ctx = self.switches.last().expect("switch");
        let clash = ctx
            .seen
            .iter()
            .any(|seen| case_in_range(*seen, bits, high_bits, prom))
            || ctx
                .ranges
                .iter()
                .any(|(lo, hi)| ranges_overlap(bits, high_bits, *lo, *hi, prom));
        if clash {
            return self.err(line, column, "duplicate case value in case range");
        }
        self.switches
            .last_mut()
            .expect("switch")
            .ranges
            .push((bits, high_bits));
        Ok(())
    }

    /// A `for` statement; its clause-1 declarations get their own scope
    /// (C11 6.8.5p5).
    fn check_for(
        &mut self,
        init: &mut ForInit,
        cond: Option<&mut Expr>,
        step: Option<&mut Expr>,
        body: &mut Stmt,
    ) -> Result<(), Diagnostic> {
        self.push();
        self.loop_depth += 1;
        match init {
            ForInit::Empty => {}
            ForInit::Decl(decls) => {
                for decl in decls {
                    self.check_decl(decl)?;
                }
            }
            ForInit::Expr(expr) => {
                self.check_expr(expr)?;
                self.decay(expr)?;
            }
        }
        if let Some(cond) = cond {
            self.check_expr(cond)?;
            self.decay(cond)?;
            self.require_scalar(cond)?;
        }
        if let Some(step) = step {
            self.check_expr(step)?;
        }
        self.check_stmt(body)?;
        self.loop_depth -= 1;
        self.pop();
        Ok(())
    }

    /// GNU computed `goto *expr`: the operand must be a pointer.
    fn check_goto_ptr(
        &mut self,
        expr: &mut Expr,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        self.check_expr(expr)?;
        self.decay(expr)?;
        if !expr.ty().is_some_and(|ty| ty.is_pointer()) {
            return self.err(
                line,
                column,
                "the operand of a computed 'goto' must be a pointer",
            );
        }
        Ok(())
    }

    /// The diagnostic for a `goto` or `&&` naming a label that is not
    /// defined where code is emitted.
    pub(super) fn missing_label(&self, name: &str) -> String {
        if self.dropped_labels.contains_key(name) {
            format!(
                "cannot jump to label '{name}': it is inside an operand that is not evaluated \
                 (sizeof, or a _Generic controlling expression or unselected association)"
            )
        } else {
            format!("use of undeclared label '{name}'")
        }
    }

    /// Start checking an operand that is not evaluated.
    pub(super) fn begin_unevaluated(&mut self) -> Unevaluated {
        self.unevaluated_depth += 1;
        Unevaluated {
            static_locals: self.static_locals.len(),
            strings: self.strings.len(),
            static_seq: self.static_seq,
            library_nan_calls: self.library_nan_calls.len(),
            library_calls: self.library_calls.clone(),
            addr_labels: self.addr_labels.len(),
            labels: self.labels.keys().cloned().collect(),
        }
    }

    /// Roll back what an operand that is not evaluated recorded: private
    /// globals (block-scope `static` objects, file-scope compound literals,
    /// string literals) and their numbering, library calls, label
    /// addresses, and labels. Those labels are dropped with the code, so
    /// they move to `dropped_labels`, where a later `goto` finds them and
    /// is rejected rather than branching to a block that is never emitted.
    pub(super) fn end_unevaluated(&mut self, saved: Unevaluated) {
        self.unevaluated_depth -= 1;
        self.static_locals.truncate(saved.static_locals);
        self.strings.truncate(saved.strings);
        self.static_seq = saved.static_seq;
        self.library_nan_calls.truncate(saved.library_nan_calls);
        self.library_calls = saved.library_calls;
        self.addr_labels.truncate(saved.addr_labels);
        let added: Vec<String> = self
            .labels
            .keys()
            .filter(|name| !saved.labels.contains(*name))
            .cloned()
            .collect();
        for name in added {
            let at = self.labels.remove(&name).expect("label just listed");
            self.dropped_labels.insert(name, at);
        }
    }

    /// Labels have function scope (C11 6.2.1). A name may be used before it
    /// is defined, and two labels with the same name are a constraint violation.
    pub(super) fn collect_labels(&mut self, compound: &Compound) -> Result<(), Diagnostic> {
        for item in &compound.items {
            self.collect_labels_stmt(item)?;
        }
        Ok(())
    }

    pub(super) fn collect_labels_stmt(&mut self, stmt: &Stmt) -> Result<(), Diagnostic> {
        match stmt {
            Stmt::Label {
                name,
                stmt,
                line,
                column,
            } => {
                if self.labels.contains_key(name) || self.dropped_labels.contains_key(name) {
                    return self.err(*line, *column, format!("redefinition of label '{name}'"));
                }
                self.labels.insert(name.clone(), (*line, *column));
                self.collect_labels_stmt(stmt)
            }
            Stmt::Compound(compound) => self.collect_labels(compound),
            Stmt::If {
                then_stmt,
                else_stmt,
                ..
            } => {
                self.collect_labels_stmt(then_stmt)?;
                if let Some(else_stmt) = else_stmt {
                    self.collect_labels_stmt(else_stmt)?;
                }
                Ok(())
            }
            Stmt::While { body, .. } | Stmt::Do { body, .. } => self.collect_labels_stmt(body),
            Stmt::For { init, body, .. } => {
                if let ForInit::Decl(decls) = init {
                    for decl in decls {
                        if let Some(init) = &decl.init {
                            let _ = init;
                        }
                    }
                }
                self.collect_labels_stmt(body)
            }
            Stmt::Switch { body, .. }
            | Stmt::Case { stmt: body, .. }
            | Stmt::Default { stmt: body, .. } => self.collect_labels_stmt(body),
            Stmt::Decl(_)
            | Stmt::FuncDecl(_)
            | Stmt::Return { .. }
            | Stmt::Expr { .. }
            | Stmt::Break { .. }
            | Stmt::Continue { .. }
            | Stmt::Goto { .. }
            | Stmt::GotoPtr { .. }
            | Stmt::Asm { .. } => Ok(()),
        }
    }

    /// Integer constant expression of a `case` label, converted to `promoted`.
    ///
    /// The conversion is C11 6.3.1.3: unsigned destinations wrap, and a signed
    /// destination that cannot represent the value keeps the low bits. Two
    /// labels that differ before that conversion and agree after it are
    /// duplicates.
    pub(super) fn case_value(&self, expr: &Expr, promoted: TypeName) -> Result<u128, Diagnostic> {
        self.require_integer(expr)?;
        match eval_const_expr(expr) {
            Ok(value) => Ok(convert_bits(value.bits, value.ty, promoted)),
            Err(ConstError::DivZero) => {
                self.err(expr.line(), expr.column(), "case label divides by zero")
            }
            Err(ConstError::Overflow(ty)) => self.err(
                expr.line(),
                expr.column(),
                format!("case label overflows '{}'", ty.as_str()),
            ),
            Err(ConstError::Shift) => self.err(
                expr.line(),
                expr.column(),
                "case label uses an undefined shift",
            ),
            Err(ConstError::NotConst) => self.err(
                expr.line(),
                expr.column(),
                "case label is not an integer constant expression",
            ),
        }
    }
}
