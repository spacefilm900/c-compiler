//! Statements: blocks, declarations, `if`, loops, `switch` and labels.

use super::*;

impl Gen {
    pub(super) fn gen_compound(&mut self, compound: &Compound) -> Result<(), Ice> {
        let depth = self.vla_depth();
        for item in &compound.items {
            self.gen_stmt(item)?;
        }
        if self.vla_depth() > depth {
            self.ensure_block();
            self.leave_vla_to(depth);
        }
        Ok(())
    }

    /// Lower one statement. An internal error ends lowering with `Err`
    /// (see [`Gen::fail`]); one inside an expression is recorded and
    /// lowering goes on with a placeholder value (see [`Gen::ice`]).
    pub(super) fn gen_stmt(&mut self, stmt: &Stmt) -> Result<(), Ice> {
        self.pos = stmt.pos();
        match stmt {
            Stmt::Compound(compound) => return self.gen_compound(compound),
            Stmt::Decl(decl) => {
                self.ensure_block();
                self.gen_decl(decl);
            }
            Stmt::Return { expr, .. } => {
                self.ensure_block();
                if let Some(expr) = expr {
                    if self.ret_ty.unqualified().is_record() {
                        let src_volatile = expr.ty().is_some_and(|ty| ty.is_volatile());
                        let src = self.record_ptr(expr);
                        self.restore_vla_to(0);
                        self.emit_record_return(&src, src_volatile);
                    } else {
                        let value = self.gen_val(expr);
                        let value =
                            self.convert(&value, &expr.ty().expect("typed"), &self.ret_ty.clone());
                        self.restore_vla_to(0);
                        if self.ret_agg.is_some() {
                            // Windows x64: a scalar returned in memory or as
                            // `<2 x i64>` goes through a slot.
                            let ret = self.ret_ty.unqualified();
                            let slot = self.scalar_copy(&ret, &value);
                            self.emit_record_return(&slot, false);
                        } else {
                            self.emit(&format!("  ret {} {value}", self.ret_ty.ir_type()));
                        }
                    }
                } else {
                    self.restore_vla_to(0);
                    self.emit("  ret void");
                }
                self.terminated = true;
            }
            Stmt::If {
                cond,
                then_stmt,
                else_stmt,
                ..
            } => return self.gen_if(cond, then_stmt, else_stmt.as_deref()),
            Stmt::While { cond, body, .. } => return self.gen_while(cond, body),
            Stmt::Do { body, cond, .. } => return self.gen_do(body, cond),
            Stmt::Switch {
                cond,
                body,
                promoted,
                ..
            } => match promoted {
                Some(promoted) => return self.gen_switch(cond, body, *promoted),
                None => return Err(self.fail("switch has no promoted controlling type")),
            },
            Stmt::Case { stmt, .. } | Stmt::Default { stmt, .. } => {
                let label = self.take_switch_label()?;
                self.place_label(&label);
                return self.gen_stmt(stmt);
            }
            Stmt::FuncDecl(_) => {}
            Stmt::For {
                init,
                cond,
                step,
                body,
                ..
            } => return self.gen_for(init, cond.as_ref(), step.as_ref(), body),
            Stmt::Expr { expr, .. } => {
                self.ensure_block();
                if let Some(expr) = expr {
                    let _ = self.gen_expr(expr);
                }
            }
            Stmt::Label { name, stmt, .. } => {
                let block = c_label(name);
                self.place_label(&block);
                return self.gen_stmt(stmt);
            }
            Stmt::Goto { name, .. } => {
                self.ensure_block();
                let depth = self.label_vla_depths.get(name).copied().unwrap_or(0);
                self.restore_vla_to(depth);
                self.br(&c_label(name));
            }
            Stmt::GotoPtr { expr, .. } => {
                // An indirect branch may reach any label whose address the
                // function takes (LLVM LangRef, `indirectbr`).
                self.ensure_block();
                let target = self.gen_val(expr);
                self.restore_vla_to(0);
                let dests: Vec<String> = self
                    .addr_labels
                    .iter()
                    .map(|label| format!("label %{label}"))
                    .collect();
                self.emit(&format!(
                    "  indirectbr ptr {target}, [{}]",
                    dests.join(", ")
                ));
                self.terminated = true;
            }
            Stmt::Break { .. } => {
                self.ensure_block();
                let Some(end) = self.breaks.last().cloned() else {
                    return Err(self.fail("'break' is not inside a loop or switch"));
                };
                let depth = self.break_vla.last().copied().unwrap_or(0);
                self.restore_vla_to(depth);
                self.br(&end);
            }
            Stmt::Continue { .. } => {
                self.ensure_block();
                let Some(cont) = self.continues.last().cloned() else {
                    return Err(self.fail("'continue' is not inside a loop"));
                };
                let depth = self.continue_vla.last().copied().unwrap_or(0);
                self.restore_vla_to(depth);
                self.br(&cont);
            }
        }
        Ok(())
    }

    pub(super) fn gen_decl(&mut self, decl: &VarDecl) {
        if decl.eval_vla {
            self.force_eval_vla_sizes(&decl.ty);
        } else if decl.ty.is_variably_modified() {
            self.eval_vla_sizes(&decl.ty);
        }
        if decl.storage == Storage::Typedef {
            return;
        }
        if decl.storage == Storage::None && decl.ty.is_vla() {
            self.emit_vla_alloca(decl);
            return;
        }
        let Some(checked) = &decl.checked else {
            return;
        };
        match checked {
            CheckedInit::Scalar(init) => {
                if decl.ty.is_record() {
                    let src = self.record_ptr(init);
                    let src_volatile = init.ty().is_some_and(|ty| ty.is_volatile());
                    self.copy_struct(&decl.ty, &src, &decl.slot, src_volatile);
                    return;
                }
                let value = self.gen_val(init);
                let value = self.convert(&value, &init.ty().expect("typed"), &decl.ty);
                self.store(&decl.ty, &value, &decl.slot);
            }
            CheckedInit::Aggregate(stores) => {
                // Values are evaluated in initializer order. The stores
                // follow the zero fill in that same order, so a later
                // designator overrides part of an earlier record copy.
                let mut pending = Vec::new();
                for store in stores {
                    if let Some(info) = &store.bits {
                        let value = self.gen_val(&store.expr);
                        let dest = if info.declared == TypeName::Bool {
                            CType::from_name(TypeName::Bool)
                        } else {
                            store.ty.clone()
                        };
                        let value = self.convert(&value, &store.expr.ty().expect("typed"), &dest);
                        pending.push(PendingStore::Bits(store.offset, info.clone(), dest, value));
                        continue;
                    }
                    if store.ty.is_record() {
                        let mut src = self.record_ptr(&store.expr);
                        let mut src_volatile = store.expr.ty().is_some_and(|ty| ty.is_volatile());
                        if stores.len() > 1 {
                            // The source may be an object that a later
                            // initializer changes (`.a = (s = x), .b = (s = y)`).
                            // Its value is taken now, into a temporary.
                            let tmp = self.entry_alloca(&store.ty.unqualified());
                            self.copy_struct(&store.ty.unqualified(), &src, &tmp, src_volatile);
                            src = tmp;
                            src_volatile = false;
                        }
                        pending.push(PendingStore::Record(
                            store.offset,
                            store.ty.clone(),
                            src,
                            src_volatile,
                        ));
                        continue;
                    }
                    let value = self.gen_val(&store.expr);
                    let value = self.convert(&value, &store.expr.ty().expect("typed"), &store.ty);
                    pending.push(PendingStore::Scalar(store.offset, store.ty.clone(), value));
                }
                self.emit(&format!(
                    "  store {} zeroinitializer, ptr {}, align {}",
                    decl.ty.ir_type(),
                    decl.slot,
                    decl.ty.align()
                ));
                for store in pending {
                    match store {
                        PendingStore::Scalar(offset, ty, value) => {
                            let addr = self.offset_ptr(&decl.slot, offset);
                            self.store(&ty, &value, &addr);
                        }
                        PendingStore::Record(offset, ty, src, src_volatile) => {
                            let addr = self.offset_ptr(&decl.slot, offset);
                            self.copy_struct(&ty, &src, &addr, src_volatile);
                        }
                        PendingStore::Bits(offset, info, ty, value) => {
                            let addr = self.offset_ptr(&decl.slot, offset);
                            self.write_bits(&addr, &info, &ty, &value);
                        }
                    }
                }
            }
        }
    }

    pub(super) fn gen_if(
        &mut self,
        cond: &Expr,
        then_stmt: &Stmt,
        else_stmt: Option<&Stmt>,
    ) -> Result<(), Ice> {
        self.ensure_block();
        let cond = self.gen_cond(cond);
        let then_label = self.label();
        let else_label = self.label();
        let end_label = self.label();
        if else_stmt.is_some() {
            self.br_cond(&cond, &then_label, &else_label);
        } else {
            self.br_cond(&cond, &then_label, &end_label);
        }
        self.start(then_label);
        self.gen_stmt(then_stmt)?;
        if !self.terminated {
            self.br(&end_label);
        }
        if let Some(else_stmt) = else_stmt {
            self.start(else_label);
            self.gen_stmt(else_stmt)?;
            if !self.terminated {
                self.br(&end_label);
            }
        }
        self.start(end_label);
        Ok(())
    }

    pub(super) fn gen_while(&mut self, cond: &Expr, body: &Stmt) -> Result<(), Ice> {
        self.ensure_block();
        let cond_label = self.label();
        let body_label = self.label();
        let end_label = self.label();
        self.br(&cond_label);
        self.start(cond_label.clone());
        let cond = self.gen_cond(cond);
        self.br_cond(&cond, &body_label, &end_label);
        self.start(body_label);
        self.continue_vla.push(self.vla_depth());
        self.continues.push(cond_label.clone());
        self.break_vla.push(self.vla_depth());
        self.breaks.push(end_label.clone());
        self.gen_stmt(body)?;
        self.breaks.pop();
        self.break_vla.pop();
        self.continues.pop();
        self.continue_vla.pop();
        if !self.terminated {
            self.br(&cond_label);
        }
        self.start(end_label);
        Ok(())
    }

    /// The body runs, then the condition. `continue` lands on the condition,
    /// so a `continue` does not skip it and a `break` does not evaluate it.
    pub(super) fn gen_do(&mut self, body: &Stmt, cond: &Expr) -> Result<(), Ice> {
        self.ensure_block();
        let body_label = self.label();
        let cond_label = self.label();
        let end_label = self.label();
        self.br(&body_label);
        self.start(body_label.clone());
        self.continue_vla.push(self.vla_depth());
        self.continues.push(cond_label.clone());
        self.break_vla.push(self.vla_depth());
        self.breaks.push(end_label.clone());
        self.gen_stmt(body)?;
        self.breaks.pop();
        self.break_vla.pop();
        self.continues.pop();
        self.continue_vla.pop();
        if !self.terminated {
            self.br(&cond_label);
        }
        self.start(cond_label);
        let cond = self.gen_cond(cond);
        self.br_cond(&cond, &body_label, &end_label);
        self.start(end_label);
        Ok(())
    }

    pub(super) fn gen_for(
        &mut self,
        init: &ForInit,
        cond: Option<&Expr>,
        step: Option<&Expr>,
        body: &Stmt,
    ) -> Result<(), Ice> {
        self.ensure_block();
        let for_depth = self.vla_depth();
        match init {
            ForInit::Empty => {}
            ForInit::Decl(decls) => {
                for decl in decls {
                    self.gen_decl(decl);
                }
            }
            ForInit::Expr(expr) => {
                let _ = self.gen_expr(expr);
            }
        }
        let cond_label = self.label();
        let body_label = self.label();
        let step_label = self.label();
        let end_label = self.label();
        self.br(&cond_label);
        self.start(cond_label.clone());
        if let Some(cond) = cond {
            let cond = self.gen_cond(cond);
            self.br_cond(&cond, &body_label, &end_label);
        } else {
            self.br(&body_label);
        }
        self.start(body_label);
        // `continue` keeps for-init VLA objects; `break` leaves their scope.
        self.continue_vla.push(self.vla_depth());
        self.continues.push(step_label.clone());
        self.break_vla.push(for_depth);
        self.breaks.push(end_label.clone());
        self.gen_stmt(body)?;
        self.breaks.pop();
        self.break_vla.pop();
        self.continues.pop();
        self.continue_vla.pop();
        if !self.terminated {
            self.br(&step_label);
        }
        self.start(step_label);
        if let Some(step) = step {
            let _ = self.gen_expr(step);
        }
        self.br(&cond_label);
        self.start(end_label);
        self.leave_vla_to(for_depth);
        Ok(())
    }

    /// `switch` lowers to the LLVM `switch` instruction.
    ///
    /// The controlling value is promoted first (C11 6.8.4.2), so a `char`
    /// or `short` condition is an `i32` and a `long` or `long long` condition
    /// is an `i64`. Each `case` and `default` is its own block. Falling
    /// through branches to the next label. A label inside a loop or an `if`
    /// is still a direct target, which is what Duff's device needs. Code
    /// before the first label is emitted and is not a switch target, so a
    /// jump into the body skips it, including an automatic initializer.
    pub(super) fn gen_switch(
        &mut self,
        cond: &Expr,
        body: &Stmt,
        promoted: TypeName,
    ) -> Result<(), Ice> {
        self.ensure_block();
        let raw = self.gen_val(cond);
        let src = cond.ty().expect("typed switch condition");
        let value = self.convert(&raw, &src, &CType::from_name(promoted));

        let mut cases = Vec::new();
        let mut default_label = None;
        let mut labels = Vec::new();
        let saved_ranges = std::mem::take(&mut self.switch_ranges);
        self.collect_switch_targets(body, &mut cases, &mut default_label, &mut labels)?;
        let ranges = std::mem::replace(&mut self.switch_ranges, saved_ranges);

        let end_label = self.label();
        let default_dest = default_label.unwrap_or_else(|| end_label.clone());
        // A GNU case range `low ... high`: a small one is listed value by
        // value; a large one is tested after the switch's own cases miss,
        // as `value - low <= high - low` unsigned.
        let mask = promoted.mask();
        let mut wide = Vec::new();
        for (low, high, label) in ranges {
            let empty = if promoted.is_signed() {
                sign_extend(low, promoted.width()) > sign_extend(high, promoted.width())
            } else {
                low & mask > high & mask
            };
            if empty {
                continue;
            }
            let span = high.wrapping_sub(low) & mask;
            if span < 256 {
                for step in 0..=span {
                    cases.push((low.wrapping_add(step) & mask, label.clone()));
                }
            } else {
                wide.push((low, span, label));
            }
        }
        if wide.is_empty() {
            self.emit_switch(promoted, &value, &default_dest, &cases);
        } else {
            let check = self.label();
            self.emit_switch(promoted, &value, &check, &cases);
            self.start(check);
            let ir = promoted.ir_type();
            for (low, span, label) in wide {
                let diff = self.fresh();
                self.emit(&format!(
                    "  {diff} = sub {ir} {value}, {}",
                    promoted.format_bits(low)
                ));
                let hit = self.fresh();
                self.emit(&format!(
                    "  {hit} = icmp ule {ir} {diff}, {}",
                    promoted.to_unsigned().format_bits(span)
                ));
                let next = self.label();
                self.br_cond(&hit, &label, &next);
                self.start(next);
            }
            self.br(&default_dest);
        }

        let saved_labels = std::mem::replace(&mut self.switch_labels, labels);
        let saved_at = self.switch_at;
        self.switch_at = 0;
        self.break_vla.push(self.vla_depth());
        self.breaks.push(end_label.clone());

        let entry = self.label();
        self.start(entry);
        self.gen_stmt(body)?;
        if self.switch_at != self.switch_labels.len() {
            return Err(self.fail("a case label of this switch was not emitted"));
        }
        self.breaks.pop();
        self.break_vla.pop();
        if !self.terminated {
            self.br(&end_label);
        }
        self.switch_labels = saved_labels;
        self.switch_at = saved_at;
        self.start(end_label);
        Ok(())
    }

    pub(super) fn emit_switch(
        &mut self,
        ty: TypeName,
        value: &str,
        default_dest: &str,
        cases: &[(u128, String)],
    ) {
        let ir = ty.ir_type();
        self.emit(&format!("  switch {ir} {value}, label %{default_dest} ["));
        for (bits, label) in cases {
            self.emit(&format!(
                "    {ir} {}, label %{label}",
                ty.format_bits(*bits)
            ));
        }
        self.emit("  ]");
        self.terminated = true;
    }

    /// Case and default labels that belong to this switch, not to a nested one.
    /// The visit order matches [`Self::gen_stmt`].
    pub(super) fn collect_switch_targets(
        &mut self,
        stmt: &Stmt,
        cases: &mut Vec<(u128, String)>,
        default_label: &mut Option<String>,
        labels: &mut Vec<String>,
    ) -> Result<(), Ice> {
        match stmt {
            Stmt::Switch { .. } => {}
            Stmt::Case {
                value,
                high_value,
                stmt,
                line,
                column,
                ..
            } => {
                let label = self.label();
                let Some(bits) = *value else {
                    return Err(self.fail_at(*line, *column, "case label has no value"));
                };
                match high_value {
                    None => cases.push((bits, label.clone())),
                    Some(high) => self.switch_ranges.push((bits, *high, label.clone())),
                }
                labels.push(label);
                self.collect_switch_targets(stmt, cases, default_label, labels)?;
            }
            Stmt::Default { stmt, .. } => {
                let label = self.label();
                *default_label = Some(label.clone());
                labels.push(label);
                self.collect_switch_targets(stmt, cases, default_label, labels)?;
            }
            Stmt::Compound(compound) => {
                for item in &compound.items {
                    self.collect_switch_targets(item, cases, default_label, labels)?;
                }
            }
            Stmt::If {
                then_stmt,
                else_stmt,
                ..
            } => {
                self.collect_switch_targets(then_stmt, cases, default_label, labels)?;
                if let Some(else_stmt) = else_stmt {
                    self.collect_switch_targets(else_stmt, cases, default_label, labels)?;
                }
            }
            Stmt::While { body, .. } | Stmt::Do { body, .. } => {
                self.collect_switch_targets(body, cases, default_label, labels)?;
            }
            Stmt::For { body, .. } => {
                self.collect_switch_targets(body, cases, default_label, labels)?;
            }
            Stmt::Label { stmt, .. } => {
                self.collect_switch_targets(stmt, cases, default_label, labels)?;
            }
            Stmt::Decl(_)
            | Stmt::FuncDecl(_)
            | Stmt::Return { .. }
            | Stmt::Expr { .. }
            | Stmt::Break { .. }
            | Stmt::Continue { .. }
            | Stmt::Goto { .. }
            | Stmt::GotoPtr { .. } => {}
        }
        Ok(())
    }

    pub(super) fn take_switch_label(&mut self) -> Result<String, Ice> {
        let Some(label) = self.switch_labels.get(self.switch_at).cloned() else {
            return Err(self.fail("case label was not collected for its switch"));
        };
        self.switch_at += 1;
        Ok(label)
    }

    pub(super) fn place_label(&mut self, label: &str) {
        if !self.terminated {
            self.br(label);
        }
        self.start(label.to_string());
    }
}
