//! Clean-room C subset compiler.
//!
//! The frontend is original Rust written from the C standard, the LLVM
//! Language Reference, and the System V ABI. It is not derived from Clang
//! or GCC.

pub mod abi;
pub mod ast;
pub mod diag;
pub mod dialect;
pub mod driver;
pub mod float;
mod fp80;
pub mod irgen;
pub mod lexer;
pub mod parser;
pub mod pp;
pub mod sema;
pub mod token;

pub use diag::{Diagnostic, Warning};
pub use dialect::Dialect;
pub use driver::run;
pub use pp::PpOptions;

/// Lex, parse, check, and render `source` as a textual LLVM IR module.
///
/// Preprocessing uses the default system include directory and no `-I` / `-D`
/// / `-U` options. [`compile_with`] takes those options.
pub fn compile_to_ir(source: &str, filename: &str) -> Result<String, Diagnostic> {
    compile_with(source, filename, &PpOptions::default())
}

/// Preprocess `source`, then parse, check, and render LLVM IR, in strict
/// C11.
pub fn compile_with(
    source: &str,
    filename: &str,
    options: &PpOptions,
) -> Result<String, Diagnostic> {
    compile_dialect(source, filename, options, Dialect::C11).map(|output| output.ir)
}

/// The result of a successful compilation.
pub struct Output {
    /// The textual LLVM IR module.
    pub ir: String,
    /// Warnings, in source order. Strict C11 produces none.
    pub warnings: Vec<Warning>,
}

/// Like [`compile_with`], in the given dialect.
pub fn compile_dialect(
    source: &str,
    filename: &str,
    options: &PpOptions,
    dialect: Dialect,
) -> Result<Output, Diagnostic> {
    let tokens = pp::preprocess_tokens(source, filename, options)?;
    let mut unit = parser::parse_dialect(&tokens, filename, dialect)?;
    let warnings = sema::analyze_dialect(&mut unit, filename, &tokens, dialect)?;
    Ok(Output {
        ir: irgen::render(&unit, filename),
        warnings,
    })
}
