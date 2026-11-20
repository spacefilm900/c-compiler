//! Lowering for variable-length arrays: `llvm.stacksave` /
//! `llvm.stackrestore`, dynamic `alloca`, runtime `sizeof`, and GEP
//! that uses a runtime element size.

use super::*;

impl Gen {
    /// Type-check already happened; evaluate every VLA size in `ty` now.
    pub(super) fn eval_vla_sizes(&mut self, ty: &CType) {
        self.eval_vla_sizes_from(ty, None);
    }

    pub(super) fn force_eval_vla_sizes(&mut self, ty: &CType) {
        self.eval_vla_sizes_from(ty, Some(VlaId(0)));
    }

    pub(super) fn force_eval_vla_sizes_from(&mut self, ty: &CType, from: VlaId) {
        self.eval_vla_sizes_from(ty, Some(from));
    }

    fn eval_vla_sizes_from(&mut self, ty: &CType, from: Option<VlaId>) {
        match ty.peel() {
            CType::Array(elem, len) => {
                self.eval_vla_sizes_from(elem, from);
                if let ArrayLen::Vla(id) = *len {
                    self.eval_vla_id(id, elem, from);
                }
            }
            CType::Pointer(inner) => self.eval_vla_sizes_from(inner, from),
            CType::Func(func) => {
                self.eval_vla_sizes_from(&func.ret, from);
                for param in &func.params {
                    self.eval_vla_sizes_from(param, from);
                }
            }
            _ => {}
        }
    }

    fn eval_vla_id(&mut self, id: VlaId, elem: &CType, from: Option<VlaId>) {
        let force = from.is_some_and(|from| id.0 >= from.0);
        if !force && self.vla_bytes.contains_key(&id) {
            return;
        }
        let Some(expr) = self.vla_bounds.get(&id).cloned() else {
            return;
        };
        let value = self.gen_val(&expr);
        let count = self.extend_i64(&value, &expr.ty().expect("typed VLA size"));
        let elem_bytes = self.runtime_size(elem);
        let bytes = self.fresh();
        self.emit(&format!("  {bytes} = mul i64 {count}, {elem_bytes}"));
        self.vla_counts.insert(id, count);
        self.vla_bytes.insert(id, bytes.clone());
        if let Some(slot) = self.vla_byte_slots.get(&id).cloned() {
            self.emit(&format!("  store i64 {bytes}, ptr {slot}"));
        }
    }

    /// Byte size of `ty` as an i64 SSA value or constant.
    pub(super) fn runtime_size(&mut self, ty: &CType) -> String {
        if let Some(size) = ty.size() {
            return format!("{size}");
        }
        match ty.peel() {
            CType::Array(elem, ArrayLen::Vla(id)) => {
                if !self.vla_bytes.contains_key(id) {
                    self.eval_vla_id(*id, elem, None);
                }
                if let Some(slot) = self.vla_byte_slots.get(id).cloned() {
                    let tmp = self.fresh();
                    self.emit(&format!("  {tmp} = load i64, ptr {slot}"));
                    tmp
                } else if let Some(bytes) = self.vla_bytes.get(id).cloned() {
                    bytes
                } else {
                    "0".to_string()
                }
            }
            CType::Array(elem, ArrayLen::Const(n)) => {
                let elem_bytes = self.runtime_size(elem);
                let tmp = self.fresh();
                self.emit(&format!("  {tmp} = mul i64 {n}, {elem_bytes}"));
                tmp
            }
            CType::Array(_, _) => "0".to_string(),
            _ => "0".to_string(),
        }
    }

    /// `sizeof` of a possibly variable-length type.
    pub(super) fn gen_sizeof_value(&mut self, ty: &CType) -> String {
        self.eval_vla_sizes(ty);
        let size = self.runtime_size(ty);
        if size.chars().all(|c| c.is_ascii_digit()) {
            return TypeName::size_t().format_bits(size.parse().unwrap_or(0));
        }
        size
    }

    pub(super) fn emit_vla_alloca(&mut self, decl: &VarDecl) {
        self.eval_vla_sizes(&decl.ty);
        self.enter_vla_scope();
        let bytes = self.runtime_size(&decl.ty);
        let align = decl.ty.align().max(decl.align.unwrap_or(0));
        self.emit(&format!(
            "  {} = alloca i8, i64 {bytes}, align {align}",
            decl.slot
        ));
    }

    pub(super) fn declare_stack_intrinsics(&mut self) {
        self.intrinsics
            .insert("declare ptr @llvm.stacksave()\n".to_string());
        self.intrinsics
            .insert("declare void @llvm.stackrestore(ptr)\n".to_string());
    }

    pub(super) fn enter_vla_scope(&mut self) {
        self.declare_stack_intrinsics();
        let save = self.fresh();
        self.emit(&format!("  {save} = call ptr @llvm.stacksave()"));
        if let Some(slot) = self.vla_slots.get(self.vla_saves.len()).cloned() {
            self.emit(&format!("  store ptr {save}, ptr {slot}"));
        }
        self.vla_saves.push(save);
        self.store_vla_depth();
    }

    /// Restore and pop VLA object saves down to `depth`.
    pub(super) fn leave_vla_to(&mut self, depth: usize) {
        if self.terminated {
            self.vla_saves.truncate(depth);
            return;
        }
        if self.vla_saves.len() <= depth {
            return;
        }
        self.restore_vla_to(depth);
        self.vla_saves.truncate(depth);
    }

    /// Restore VLA scopes down to `depth` without popping the generator's
    /// stack (for a branch out of the scopes). The current list is kept
    /// because other paths still leave those scopes normally.
    pub(super) fn restore_vla_to(&mut self, depth: usize) {
        if self.vla_depth_slot.is_some() {
            self.restore_if_entered(depth);
            return;
        }
        if self.vla_saves.len() <= depth {
            return;
        }
        self.declare_stack_intrinsics();
        let saves: Vec<String> = self.vla_saves[depth..].iter().rev().cloned().collect();
        for save in saves {
            self.emit(&format!("  call void @llvm.stackrestore(ptr {save})"));
        }
    }

    /// Restore to `depth` only if the runtime depth shows those scopes were
    /// entered. A computed goto can reach a later label without executing
    /// the VLA declaration, so the declaration's stacksave SSA would not
    /// dominate the cleanup; the entry-block slot is used instead, and only
    /// after a depth check so an unstored slot is not loaded.
    fn restore_if_entered(&mut self, depth: usize) {
        if self.terminated {
            return;
        }
        let Some(dslot) = self.vla_depth_slot.clone() else {
            return;
        };
        self.declare_stack_intrinsics();
        let cur = self.fresh();
        self.emit(&format!("  {cur} = load i32, ptr {dslot}"));
        let cmp = self.fresh();
        self.emit(&format!("  {cmp} = icmp sgt i32 {cur}, {depth}"));
        let restore = self.label();
        let cont = self.label();
        self.br_cond(&cmp, &restore, &cont);
        self.start(restore);
        if let Some(slot) = self.vla_slots.get(depth).cloned() {
            let save = self.fresh();
            self.emit(&format!("  {save} = load ptr, ptr {slot}"));
            self.emit(&format!("  call void @llvm.stackrestore(ptr {save})"));
        }
        self.emit(&format!("  store i32 {depth}, ptr {dslot}"));
        self.br(&cont);
        self.start(cont);
    }

    fn store_vla_depth(&mut self) {
        self.store_runtime_vla_depth(self.vla_saves.len());
    }

    fn store_runtime_vla_depth(&mut self, depth: usize) {
        let Some(slot) = self.vla_depth_slot.clone() else {
            return;
        };
        if self.terminated {
            return;
        }
        self.emit(&format!("  store i32 {depth}, ptr {slot}"));
    }

    /// Trampolines for address-taken labels. Emitted at the end of the
    /// function so every entry-block stacksave slot already exists, then
    /// a load of `slots[dest]` restores any VLAs opened after that label.
    pub(super) fn emit_addr_label_trampolines(&mut self) {
        if self.addr_label_names.is_empty() {
            return;
        }
        if !self.terminated {
            self.ice("computed-goto trampolines must follow a terminated block");
            return;
        }
        let names = self.addr_label_names.clone();
        for name in names {
            self.start(c_label_via_ptr(&name));
            let dest = c_label(&name);
            let depth = self.label_vla_depths.get(&name).copied().unwrap_or(0);
            self.restore_if_entered(depth);
            self.br(&dest);
        }
    }

    pub(super) fn vla_depth(&self) -> usize {
        self.vla_saves.len()
    }

    /// GEP when the pointee may be a VLA (runtime element size).
    pub(super) fn gen_vla_gep(
        &mut self,
        ptr: &str,
        idx: &str,
        pointee: &CType,
        unaligned: bool,
    ) -> String {
        let size = self.runtime_size(pointee);
        let offs = self.fresh();
        self.emit(&format!("  {offs} = mul i64 {idx}, {size}"));
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = getelementptr inbounds i8, ptr {ptr}, i64 {offs}"
        ));
        if unaligned {
            self.unaligned.insert(tmp.clone());
        }
        tmp
    }
}

fn walk_vla_bound_exprs(
    ty: &CType,
    depth: &mut usize,
    depths: &mut std::collections::HashMap<String, usize>,
    max_depth: &mut usize,
    sizes: &[VlaBound],
) {
    match ty.peel() {
        CType::Array(elem, ArrayLen::Vla(id)) => {
            walk_vla_bound_exprs(elem, depth, depths, max_depth, sizes);
            if let Some(bound) = sizes.iter().find(|bound| bound.id == *id) {
                walk_label_depths_expr(&bound.expr, depth, depths, max_depth, sizes);
            }
        }
        CType::Array(elem, _) => walk_vla_bound_exprs(elem, depth, depths, max_depth, sizes),
        CType::Pointer(inner) => walk_vla_bound_exprs(inner, depth, depths, max_depth, sizes),
        CType::Func(func) => {
            walk_vla_bound_exprs(&func.ret, depth, depths, max_depth, sizes);
            for param in &func.params {
                walk_vla_bound_exprs(param, depth, depths, max_depth, sizes);
            }
        }
        _ => {}
    }
}

fn is_vla_object(decl: &VarDecl) -> bool {
    decl.storage == Storage::None && decl.ty.is_vla()
}

fn bump_vla_depth(depth: &mut usize, max_depth: &mut usize, added: usize) {
    *depth += added;
    *max_depth = (*max_depth).max(*depth);
}

/// Walk a function body and record, for each label, the number of open
/// VLA object scopes at that label. `max` is the deepest nesting, used
/// to allocate entry-block stacksave slots.
pub(super) fn label_vla_depths(
    body: &Compound,
    vla_sizes: &[VlaBound],
) -> (std::collections::HashMap<String, usize>, usize) {
    let mut depths = std::collections::HashMap::new();
    let mut depth = 0usize;
    let mut max_depth = 0usize;
    for item in &body.items {
        walk_label_depths(item, &mut depth, &mut depths, &mut max_depth, vla_sizes);
    }
    (depths, max_depth)
}

fn walk_label_depths(
    stmt: &Stmt,
    depth: &mut usize,
    depths: &mut std::collections::HashMap<String, usize>,
    max_depth: &mut usize,
    sizes: &[VlaBound],
) {
    match stmt {
        Stmt::Decl(decl) => {
            if let Some(init) = decl.init.as_ref() {
                walk_label_depths_init(init, depth, depths, max_depth, sizes);
            }
            walk_vla_bound_exprs(&decl.ty, depth, depths, max_depth, sizes);
            if is_vla_object(decl) {
                bump_vla_depth(depth, max_depth, 1);
            }
        }
        Stmt::Compound(compound) => {
            let before = *depth;
            for item in &compound.items {
                walk_label_depths(item, depth, depths, max_depth, sizes);
            }
            *depth = before;
        }
        Stmt::Label { name, stmt, .. } => {
            depths.insert(name.clone(), *depth);
            walk_label_depths(stmt, depth, depths, max_depth, sizes);
        }
        Stmt::If {
            cond,
            then_stmt,
            else_stmt,
            ..
        } => {
            walk_label_depths_expr(cond, depth, depths, max_depth, sizes);
            walk_label_depths(then_stmt, depth, depths, max_depth, sizes);
            if let Some(else_stmt) = else_stmt {
                walk_label_depths(else_stmt, depth, depths, max_depth, sizes);
            }
        }
        Stmt::While { cond, body, .. } => {
            walk_label_depths_expr(cond, depth, depths, max_depth, sizes);
            walk_label_depths(body, depth, depths, max_depth, sizes);
        }
        Stmt::Do { body, cond, .. } => {
            walk_label_depths(body, depth, depths, max_depth, sizes);
            walk_label_depths_expr(cond, depth, depths, max_depth, sizes);
        }
        Stmt::Switch { cond, body, .. } => {
            walk_label_depths_expr(cond, depth, depths, max_depth, sizes);
            walk_label_depths(body, depth, depths, max_depth, sizes);
        }
        Stmt::Case {
            expr,
            high,
            stmt: body,
            ..
        } => {
            walk_label_depths_expr(expr, depth, depths, max_depth, sizes);
            if let Some(high) = high {
                walk_label_depths_expr(high, depth, depths, max_depth, sizes);
            }
            walk_label_depths(body, depth, depths, max_depth, sizes);
        }
        Stmt::Default { stmt: body, .. } => {
            walk_label_depths(body, depth, depths, max_depth, sizes)
        }
        Stmt::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            let added = match init {
                ForInit::Decl(decls) => {
                    let mut added = 0;
                    for decl in decls {
                        if let Some(init) = decl.init.as_ref() {
                            walk_label_depths_init(init, depth, depths, max_depth, sizes);
                        }
                        walk_vla_bound_exprs(&decl.ty, depth, depths, max_depth, sizes);
                        if is_vla_object(decl) {
                            bump_vla_depth(depth, max_depth, 1);
                            added += 1;
                        }
                    }
                    added
                }
                ForInit::Expr(expr) => {
                    walk_label_depths_expr(expr, depth, depths, max_depth, sizes);
                    0
                }
                ForInit::Empty => 0,
            };
            if let Some(cond) = cond {
                walk_label_depths_expr(cond, depth, depths, max_depth, sizes);
            }
            if let Some(step) = step {
                walk_label_depths_expr(step, depth, depths, max_depth, sizes);
            }
            walk_label_depths(body, depth, depths, max_depth, sizes);
            *depth -= added;
        }
        Stmt::Return {
            expr: Some(expr), ..
        }
        | Stmt::Expr {
            expr: Some(expr), ..
        } => {
            walk_label_depths_expr(expr, depth, depths, max_depth, sizes);
        }
        Stmt::GotoPtr { expr, .. } => {
            walk_label_depths_expr(expr, depth, depths, max_depth, sizes);
        }
        _ => {}
    }
}

fn walk_label_depths_init(
    init: &Init,
    depth: &mut usize,
    depths: &mut std::collections::HashMap<String, usize>,
    max_depth: &mut usize,
    sizes: &[VlaBound],
) {
    match init {
        Init::Expr(expr) => walk_label_depths_expr(expr, depth, depths, max_depth, sizes),
        Init::List { items, .. } => {
            for item in items {
                walk_label_depths_init(item, depth, depths, max_depth, sizes);
            }
        }
        Init::Designated {
            designators, init, ..
        } => {
            for designator in designators {
                if let Designator::Index { expr, .. } = designator {
                    walk_label_depths_expr(expr, depth, depths, max_depth, sizes);
                }
            }
            walk_label_depths_init(init, depth, depths, max_depth, sizes);
        }
    }
}

/// Statement expressions inherit the enclosing depth; inner VLA objects
/// and labels are recorded, then the depth is restored on exit.
fn walk_label_depths_expr(
    expr: &Expr,
    depth: &mut usize,
    depths: &mut std::collections::HashMap<String, usize>,
    max_depth: &mut usize,
    sizes: &[VlaBound],
) {
    match expr {
        Expr::StmtExpr { body, .. } => {
            let before = *depth;
            for item in &body.items {
                walk_label_depths(item, depth, depths, max_depth, sizes);
            }
            *depth = before;
        }
        Expr::Bin { left, right, .. }
        | Expr::Assign {
            target: left,
            value: right,
            ..
        }
        | Expr::CompoundAssign {
            target: left,
            value: right,
            ..
        }
        | Expr::Comma { left, right, .. }
        | Expr::Index {
            base: left,
            index: right,
            ..
        } => {
            walk_label_depths_expr(left, depth, depths, max_depth, sizes);
            walk_label_depths_expr(right, depth, depths, max_depth, sizes);
        }
        Expr::Unary { expr, .. }
        | Expr::IncDec { target: expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::Decay { expr, .. }
        | Expr::Member { base: expr, .. } => {
            walk_label_depths_expr(expr, depth, depths, max_depth, sizes);
        }
        Expr::VaArg { ap, .. } => {
            walk_label_depths_expr(ap, depth, depths, max_depth, sizes);
        }
        Expr::VaOp { args, .. } => {
            for arg in args {
                walk_label_depths_expr(arg, depth, depths, max_depth, sizes);
            }
        }
        Expr::Call { callee, args, .. } => {
            walk_label_depths_expr(callee, depth, depths, max_depth, sizes);
            for arg in args {
                walk_label_depths_expr(arg, depth, depths, max_depth, sizes);
            }
        }
        Expr::Cond {
            cond,
            then_expr,
            else_expr,
            ..
        } => {
            walk_label_depths_expr(cond, depth, depths, max_depth, sizes);
            walk_label_depths_expr(then_expr, depth, depths, max_depth, sizes);
            walk_label_depths_expr(else_expr, depth, depths, max_depth, sizes);
        }
        Expr::CondShort {
            cond, else_expr, ..
        } => {
            walk_label_depths_expr(cond, depth, depths, max_depth, sizes);
            walk_label_depths_expr(else_expr, depth, depths, max_depth, sizes);
        }
        Expr::Sizeof {
            operand: SizeofOperand::Expr(expr),
            ..
        } => walk_label_depths_expr(expr, depth, depths, max_depth, sizes),
        Expr::Generic {
            control, assocs, ..
        } => {
            walk_label_depths_expr(control, depth, depths, max_depth, sizes);
            for assoc in assocs {
                walk_label_depths_expr(&assoc.expr, depth, depths, max_depth, sizes);
            }
        }
        Expr::CompoundLit { decl, .. } => {
            if let Some(init) = decl.init.as_ref() {
                walk_label_depths_init(init, depth, depths, max_depth, sizes);
            }
        }
        _ => {}
    }
}
