//! Language dialect, selected with `-std=`.
//!
//! Strict ISO C11 (`-std=c11`) is the default. `-std=gnu11` opts in to GNU
//! extensions (PLAN.md, stage 1), with behaviour taken from the public GCC
//! manual and observed from GCC 14 and Clang 19 as black boxes. The
//! predefined macros are the same in both modes.
//!
//! In GNU mode these C11 constraints become warnings:
//!
//! - An equality comparison between a void pointer and a pointer to a
//!   function (C11 6.5.9p2).
//! - A comparison of two pointers to distinct types: equality for any two
//!   pointers, relational for two object pointers (C11 6.5.8p2, 6.5.9p2).
//! - A pointer assignment, initialization, argument, or return whose
//!   pointee differs only in integer signedness, or drops a qualifier of
//!   the source pointee (C11 6.5.16.1p1). Deeper mismatches, such as
//!   `int **` from `unsigned **`, stay errors, as they are in GCC 14.
//!
//! GNU mode also accepts these extensions, which strict C11 rejects with an
//! error that names `-std=gnu11`:
//!
//! - `typeof`, in both spellings (`typeof` and `__typeof__`).
//! - Statement expressions `({ ... })`.
//! - Case ranges `case low ... high:`.
//! - Zero-length arrays `T a[0]`, and empty initializers `{}`.
//! - The conditional with an omitted middle operand, `x ?: y`.
//! - Labels as values: the address of a label `&&label`, of type `void *`,
//!   and the computed `goto *expr;` (GCC manual, "Labels as Values"). A
//!   label address is a constant in a block-scope `static` initializer, as
//!   in Lua's `lvm.c` jump table. Differences of label addresses
//!   (`&&a - &&b`) are not supported.
//!
//! The reserved spellings `__attribute__`, `__extension__`, `__inline__`,
//! `__asm__` labels and the `__builtin_*` functions are
//! accepted in both modes, because they are in the implementation's
//! namespace (C11 7.1.3).

/// The C dialect being compiled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dialect {
    /// ISO C11 with its constraints enforced.
    #[default]
    C11,
    /// C11 plus the GNU extensions listed in the module documentation.
    Gnu11,
}

impl Dialect {
    /// The dialect named by the value of `-std=`, if it is supported.
    pub fn from_std(name: &str) -> Option<Self> {
        match name {
            "c11" => Some(Self::C11),
            "gnu11" => Some(Self::Gnu11),
            _ => None,
        }
    }

    /// GNU extensions are enabled.
    pub fn gnu(self) -> bool {
        self == Self::Gnu11
    }
}
