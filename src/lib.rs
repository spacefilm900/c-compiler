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
pub mod sema;
pub mod token;

pub use diag::Diagnostic;
pub use driver::run;

/// Lex, parse, check, and render `source` as a textual LLVM IR module.
pub fn compile_to_ir(source: &str, filename: &str) -> Result<String, Diagnostic> {
    let tokens = lexer::lex(source, filename)?;
    let mut unit = parser::parse(&tokens, filename)?;
    sema::analyze(&mut unit, filename)?;
    Ok(irgen::render(&unit, filename))
}
