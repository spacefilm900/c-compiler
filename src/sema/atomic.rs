//! Constraints on `_Atomic` types that lowering can represent (C11 6.7.2.4,
//! 6.7.3). Lock-free widths are 1, 2, 4, 8, and 16 bytes. Larger atomic
//! objects are not implemented (no locking runtime).

use super::*;

impl Sema<'_> {
    /// Diagnose an atomic type this compiler does not lower, including when
    /// it appears as a typedef, a member, a pointer target, or a parameter.
    pub(super) fn check_atomic_types(
        &self,
        ty: &CType,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        if let Some(message) = bad_atomic(ty) {
            return self.err(line, column, message);
        }
        Ok(())
    }
}

fn bad_atomic(ty: &CType) -> Option<String> {
    if ty.is_atomic() {
        match ty.size() {
            Some(size) if CType::is_lock_free_atomic_size(size) => {}
            Some(size) => {
                return Some(format!(
                    "atomic type '{}' ({size} bytes) is not lock-free; this compiler \
                     supports lock-free atomics of 1, 2, 4, 8, or 16 bytes and does \
                     not implement locking atomics",
                    ty.display()
                ));
            }
            None => {
                return Some(format!("atomic type '{}' is incomplete", ty.display()));
            }
        }
    }
    match ty.peel() {
        CType::Pointer(inner) | CType::Array(inner, _) => bad_atomic(inner),
        CType::Func(func) => {
            bad_atomic(&func.ret).or_else(|| func.params.iter().find_map(bad_atomic))
        }
        CType::Struct(handle) => handle
            .fields_ref()
            .and_then(|fields| fields.iter().find_map(|field| bad_atomic(&field.ty))),
        _ => None,
    }
}
