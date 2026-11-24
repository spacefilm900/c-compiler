//! Abstract syntax for the C subset accepted by this milestone.
//!
//! Names are resolved later. Local `slot` fields are filled before semantic
//! analysis so each variable has a stable LLVM alloca name. File-scope objects
//! use the global's LLVM name instead of an alloca.
//!
//! Types are integers, `void`, pointers, arrays, and structs, with `const`
//! and `volatile`. A function type is not a value type: parameters and
//! returns use the object type that remains after the usual parameter
//! adjustment. A struct is one declaration: the same tag in another scope is
//! a different type, and completing the type updates every use of it.

use crate::target::LongDouble;
use std::cell::RefCell;
use std::rc::Rc;

mod compat;
mod dump;
mod fwd_enum;
mod record;
mod tree;
mod types;
pub use compat::*;
pub use dump::*;
pub use fwd_enum::*;
pub use record::*;
pub use tree::*;
pub use types::*;

#[cfg(test)]
mod tests;
