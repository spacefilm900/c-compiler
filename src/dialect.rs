//! Language dialect, selected with `-std=`.
//!
//! Strict ISO C11 (`-std=c11`) is the default. `-std=gnu11` opts in to GNU
//! extensions. It is the hook for the GNU-compatibility milestone (PLAN.md,
//! stage 1). For now it changes exactly one rule and nothing else (the
//! predefined macros are the same in both modes):
//!
//! - An equality comparison between a void pointer and a pointer to a
//!   function, a constraint violation in C11 6.5.9p2, is accepted with a
//!   warning instead of an error.

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
