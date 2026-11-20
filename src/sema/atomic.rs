//! Constraints on `_Atomic` types that lowering can represent (C11 6.7.2.4,
//! 6.7.3). Lock-free widths are 1, 2, 4, 8, and 16 bytes. Larger atomic
//! objects are not implemented (no locking runtime).

use super::*;
use std::collections::HashSet;

impl Sema<'_> {
    /// Diagnose an atomic type this compiler does not lower, including when
    /// it appears as a typedef, a member, a pointer target, or a parameter.
    pub(super) fn check_atomic_types(
        &self,
        ty: &CType,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        if let Some(message) = bad_atomic(ty, &mut HashSet::new()) {
            return self.err(line, column, message);
        }
        Ok(())
    }
}

fn bad_atomic(ty: &CType, seen: &mut HashSet<u32>) -> Option<String> {
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
    // Do not walk through pointers: a struct may point at itself. A pointer
    // to a non-lock-free atomic type is still diagnosed. Record types are
    // memoized so a nest of unions (union U { union V x, y; }) stays linear.
    match ty.peel() {
        CType::Pointer(inner) => {
            if inner.is_atomic() {
                bad_atomic(inner, seen)
            } else {
                None
            }
        }
        CType::Array(inner, _) => bad_atomic(inner, seen),
        CType::Func(func) => bad_atomic(&func.ret, seen)
            .or_else(|| func.params.iter().find_map(|param| bad_atomic(param, seen))),
        CType::Struct(handle) => {
            if !seen.insert(handle.id()) {
                return None;
            }
            handle
                .fields_ref()
                .and_then(|fields| fields.iter().find_map(|field| bad_atomic(&field.ty, seen)))
        }
        _ => None,
    }
}
