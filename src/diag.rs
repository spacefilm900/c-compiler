//! Source diagnostics. The compiler reports the first error and stops.
//! Warnings, which only `-std=gnu11` extensions produce, are collected.

use std::fmt;

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub filename: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

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
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}: error: {}", self.filename, self.message)
        } else {
            write!(
                f,
                "{}:{}:{}: error: {}",
                self.filename, self.line, self.column, self.message
            )
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
