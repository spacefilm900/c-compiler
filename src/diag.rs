//! Source diagnostics. The compiler reports the first error and stops.
//! Warnings, which only `-std=gnu11` extensions produce, are collected.
//!
//! An internal compiler error is a [`Diagnostic`] whose `phase` is set: a
//! later phase found a case that an earlier phase should have rejected. It
//! names the source position being compiled and the phase, and the driver
//! exits with [`EXIT_INTERNAL_ERROR`] instead of 1.

use std::fmt;

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub filename: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
    /// The compiler phase that found an internal error, such as `irgen`.
    /// `None` for an ordinary error in the program being compiled.
    pub phase: Option<&'static str>,
}

/// Exit status for an internal compiler error. An ordinary error exits 1
/// and a usage error 2.
pub const EXIT_INTERNAL_ERROR: i32 = 3;

impl Diagnostic {
    pub fn new(
        filename: impl Into<String>,
        line: usize,
        column: usize,
        message: impl Into<String>,
    ) -> Self {
        Self {
            filename: filename.into(),
            line,
            column,
            message: message.into(),
            phase: None,
        }
    }

    /// An internal compiler error found in `phase` while compiling the
    /// given source position. Line 0 means no position is known.
    pub fn internal(
        filename: impl Into<String>,
        line: usize,
        column: usize,
        phase: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            phase: Some(phase),
            ..Self::new(filename, line, column, message)
        }
    }

    pub fn is_internal(&self) -> bool {
        self.phase.is_some()
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}: ", self.filename)?;
        } else {
            write!(f, "{}:{}:{}: ", self.filename, self.line, self.column)?;
        }
        match self.phase {
            None => write!(f, "error: {}", self.message),
            Some(phase) => write!(f, "internal compiler error: {} (in {phase})", self.message),
        }
    }
}

impl std::error::Error for Diagnostic {}

/// A diagnostic that does not stop compilation.
#[derive(Debug, Clone)]
pub struct Warning(pub Diagnostic);

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let diag = &self.0;
        if diag.line == 0 {
            write!(f, "{}: warning: {}", diag.filename, diag.message)
        } else {
            write!(
                f,
                "{}:{}:{}: warning: {}",
                diag.filename, diag.line, diag.column, diag.message
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_errors_name_the_phase() {
        let ice = Diagnostic::internal("a.c", 3, 7, "irgen", "case label was lost");
        assert!(ice.is_internal());
        assert_eq!(
            ice.to_string(),
            "a.c:3:7: internal compiler error: case label was lost (in irgen)"
        );
        let ice = Diagnostic::internal("a.c", 0, 0, "sema", "no type");
        assert_eq!(
            ice.to_string(),
            "a.c: internal compiler error: no type (in sema)"
        );
        let err = Diagnostic::new("a.c", 3, 7, "expected ';'");
        assert!(!err.is_internal());
        assert_eq!(err.to_string(), "a.c:3:7: error: expected ';'");
        assert_eq!(
            Diagnostic::new("a.c", 0, 0, "x").to_string(),
            "a.c: error: x"
        );
        let warning = Warning(Diagnostic::new("a.c", 1, 2, "w"));
        assert_eq!(warning.to_string(), "a.c:1:2: warning: w");
        assert_eq!(
            Warning(Diagnostic::new("a.c", 0, 0, "w")).to_string(),
            "a.c: warning: w"
        );
    }
}
