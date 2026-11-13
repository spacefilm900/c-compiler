//! Language dialect, selected with `-std=`.
//!
//! Strict ISO C11 (`-std=c11`) is the default. `-std=gnu11` opts in to GNU
//! extensions. It is the hook for the GNU-compatibility milestone (PLAN.md,
//! stage 1). For now it relaxes these constraints of C11 to warnings, the
//! way GCC 14 and Clang 19 treat them in GNU mode (observed as black boxes),
//! and changes nothing else (the predefined macros are the same in both
//! modes):
//!
//! - An equality comparison between a void pointer and a pointer to a
//!   function (C11 6.5.9p2).
//! - A comparison of two pointers to distinct types: equality for any two
//!   pointers, relational for two object pointers (C11 6.5.8p2, 6.5.9p2).
//! - A pointer assignment, initialization, argument, or return whose
//!   pointee differs only in integer signedness, or drops a qualifier of
//!   the source pointee (C11 6.5.16.1p1). Deeper mismatches, such as
//!   `int **` from `unsigned **`, stay errors, as they are in GCC 14.

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
