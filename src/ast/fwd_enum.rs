//! GNU forward-declared enumerations: `enum E` named before its enumerator
//! list (`typedef enum E E;` ahead of `enum E { ... };`).
//!
//! The GCC manual ("Incomplete `enum` Types") says such a type is
//! incomplete until its definition, like a forward-declared struct. A use
//! that needs the size (`sizeof`, an object, a member, a cast, a function
//! definition, an lvalue conversion) is an error before the definition and
//! fine after it, while pointers, `extern` declarations and prototypes may
//! name it at any point.
//!
//! Every use shares one [`EnumRef`]. The definition completes it in place
//! with the enumerated type, so a typedef or a pointer written earlier sees
//! the integer type afterwards. Semantic analysis runs after the whole
//! translation unit is parsed, so each completion records the index of the
//! file-scope declaration it happened in, and [`completion_horizon`] tells a
//! query which declaration is being checked: a type completed in a later
//! declaration is still incomplete there. The granularity is one
//! file-scope declaration: an enum completed inside a function body counts
//! as complete for the whole function.

use super::*;
use std::cell::{Cell, OnceCell};

thread_local! {
    static HORIZON: Cell<u32> = const { Cell::new(u32::MAX) };
}

/// Restores the previous horizon when dropped.
pub struct HorizonGuard {
    previous: u32,
}

impl Drop for HorizonGuard {
    fn drop(&mut self) {
        HORIZON.with(|horizon| horizon.set(self.previous));
    }
}

/// Treat forward enums completed after file-scope declaration `index` as
/// incomplete until the guard is dropped. Outside such a guard (the parser,
/// IR generation) every completed enum is complete.
pub fn completion_horizon(index: u32) -> HorizonGuard {
    let previous = HORIZON.with(|horizon| horizon.replace(index));
    HorizonGuard { previous }
}

fn horizon() -> u32 {
    HORIZON.with(Cell::get)
}

/// Identity of one forward-declared enumeration. Copies share the body.
#[derive(Clone)]
pub struct EnumRef {
    id: u32,
    data: Rc<EnumBody>,
}

struct EnumBody {
    tag: String,
    done: OnceCell<EnumDone>,
}

struct EnumDone {
    /// Always a [`CType::Int`]: the enumerated type.
    ty: CType,
    /// The file-scope declaration index the definition was parsed in.
    at: u32,
}

impl PartialEq for EnumRef {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for EnumRef {}

impl std::fmt::Debug for EnumRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "enum {}#{}", self.data.tag, self.id)
    }
}

impl EnumRef {
    pub fn new(id: u32, tag: impl Into<String>) -> Self {
        Self {
            id,
            data: Rc::new(EnumBody {
                tag: tag.into(),
                done: OnceCell::new(),
            }),
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn tag(&self) -> &str {
        &self.data.tag
    }

    /// Complete the type with its enumerated type `base`, defined in
    /// file-scope declaration `at`. A second completion is ignored.
    pub fn complete(&self, base: TypeName, at: u32) {
        let _ = self.data.done.set(EnumDone {
            ty: CType::Int(base),
            at,
        });
    }

    /// The enumerated type, when the definition is visible at the current
    /// [`completion_horizon`].
    pub fn resolved(&self) -> Option<&CType> {
        self.data
            .done
            .get()
            .filter(|done| done.at <= horizon())
            .map(|done| &done.ty)
    }

    /// The enumerated type once defined anywhere in the translation unit,
    /// ignoring the horizon.
    pub fn eventually(&self) -> Option<&CType> {
        self.data.done.get().map(|done| &done.ty)
    }
}
