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
        self.vla_bytes.insert(id, bytes);
    }

    /// Byte size of `ty` as an i64 SSA value or constant.
    pub(super) fn runtime_size(&mut self, ty: &CType) -> String {
        if let Some(size) = ty.size() {
            return format!("{size}");
        }
        match ty.peel() {
            CType::Array(elem, ArrayLen::Vla(id)) => {
                if let Some(bytes) = self.vla_bytes.get(id).cloned() {
                    bytes
                } else {
                    self.eval_vla_id(*id, elem, None);
                    self.vla_bytes
                        .get(id)
                        .cloned()
                        .unwrap_or_else(|| "0".to_string())
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
        self.vla_saves.push(save);
    }

    pub(super) fn leave_vla_scope(&mut self) {
        if self.terminated {
            self.vla_saves.pop();
            return;
        }
        if let Some(save) = self.vla_saves.pop() {
            self.emit(&format!("  call void @llvm.stackrestore(ptr {save})"));
        }
    }

    /// Restore and pop VLA object saves down to `depth`.
    pub(super) fn leave_vla_to(&mut self, depth: usize) {
        if self.terminated {
            self.vla_saves.truncate(depth);
            return;
        }
        while self.vla_saves.len() > depth {
            self.leave_vla_scope();
        }
    }

    /// Restore VLA scopes down to `depth` without popping the generator's
    /// stack (for a branch out of the scopes). The current list is kept
    /// because other paths still leave those scopes normally.
    pub(super) fn restore_vla_to(&mut self, depth: usize) {
        if self.vla_saves.len() <= depth {
            return;
        }
        self.declare_stack_intrinsics();
        let saves: Vec<String> = self.vla_saves[depth..].iter().rev().cloned().collect();
        for save in saves {
            self.emit(&format!("  call void @llvm.stackrestore(ptr {save})"));
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

/// Walk a function body and record, for each label, the number of open
/// VLA object scopes at that label.
fn is_vla_object(decl: &VarDecl) -> bool {
    decl.storage == Storage::None && decl.ty.is_vla()
}

fn vla_object_count(decls: &[VarDecl]) -> usize {
    decls.iter().filter(|decl| is_vla_object(decl)).count()
}

pub(super) fn label_vla_depths(body: &Compound) -> std::collections::HashMap<String, usize> {
    let mut depths = std::collections::HashMap::new();
    let mut depth = 0usize;
    for item in &body.items {
        walk_label_depths(item, &mut depth, &mut depths);
    }
    depths
}

fn walk_label_depths(
    stmt: &Stmt,
    depth: &mut usize,
    depths: &mut std::collections::HashMap<String, usize>,
) {
    match stmt {
        Stmt::Decl(decl) => {
            if is_vla_object(decl) {
                *depth += 1;
            }
        }
        Stmt::Compound(compound) => {
            let before = *depth;
            for item in &compound.items {
                walk_label_depths(item, depth, depths);
            }
            *depth = before;
        }
        Stmt::Label { name, stmt, .. } => {
            depths.insert(name.clone(), *depth);
            walk_label_depths(stmt, depth, depths);
        }
        Stmt::If {
            then_stmt,
            else_stmt,
            ..
        } => {
            walk_label_depths(then_stmt, depth, depths);
            if let Some(else_stmt) = else_stmt {
                walk_label_depths(else_stmt, depth, depths);
            }
        }
        Stmt::While { body, .. } | Stmt::Do { body, .. } => walk_label_depths(body, depth, depths),
        Stmt::Switch { body, .. }
        | Stmt::Case { stmt: body, .. }
        | Stmt::Default { stmt: body, .. } => walk_label_depths(body, depth, depths),
        Stmt::For { init, body, .. } => {
            let added = match init {
                ForInit::Decl(decls) => vla_object_count(decls),
                _ => 0,
            };
            *depth += added;
            walk_label_depths(body, depth, depths);
            *depth -= added;
        }
        _ => {}
    }
}
