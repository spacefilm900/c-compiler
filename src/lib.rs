//! Clean-room C subset compiler.
//!
//! The frontend is original Rust written from the C standard, the LLVM
//! Language Reference, and the System V ABI. It is not derived from Clang
//! or GCC.

pub mod ast;
pub mod diag;
pub mod driver;
pub mod irgen;
pub mod lexer;
pub mod parser;
pub mod pp;
pub mod sema;
pub mod token;

pub use diag::Diagnostic;
pub use driver::run;
pub use pp::PpOptions;

/// Lex, parse, check, and render `source` as a textual LLVM IR module.
///
/// Preprocessing uses the default system include directory and no `-I` / `-D`
/// / `-U` options. [`compile_with`] takes those options.
pub fn compile_to_ir(source: &str, filename: &str) -> Result<String, Diagnostic> {
    compile_with(source, filename, &PpOptions::default())
}

/// Preprocess `source`, then parse, check, and render LLVM IR.
pub fn compile_with(
    source: &str,
    filename: &str,
    options: &PpOptions,
) -> Result<String, Diagnostic> {
    let tokens = pp::preprocess_tokens(source, filename, options)?;
    let mut unit = parser::parse(&tokens, filename)?;
    sema::analyze_tokens(&mut unit, filename, &tokens)?;
    Ok(irgen::render(&unit, filename))
}
