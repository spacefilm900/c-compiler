//! Textual LLVM IR for the C subset.
//!
//! Integer widths follow LP64: `char` is i8, `short` is i16, `int` is i32,
//! and `long` / `long long` are i64. `char` is signed. Locals are `alloca`s
//! in the entry block. File-scope objects are LLVM globals. Internal linkage
//! is `internal`; a declaration with no definition in this translation unit
//! is `external global` or `declare`. A const object is `constant`. String
//! literals whose address is used are `private unnamed_addr constant` arrays
//! of `i8`. A variadic call names the function type, as in
//! `call i32 (ptr, ...) @printf(...)`.
//!
//! The System V AMD64 ABI sign-extends signed `char` and `short` parameters
//! and return values to 32 bits, and zero-extends the unsigned ones. Those
//! are the LLVM `signext` and `zeroext` attributes on the definition, the
//! declaration, and the call. `int` and wider integers have no extension
//! attribute.
//!
//! `&&` and `||` are short-circuiting and produce `int` 0 or 1. `?:`
//! evaluates only the chosen arm and converts both arms to the usual
//! arithmetic type. Signed division uses `sdiv` / `srem`. Unsigned division
//! uses `udiv` / `urem`. Both round toward zero. Unsigned comparisons use
//! `icmp u*`. Signed `>>` is `ashr`; unsigned `>>` is `lshr`.
//!
//! A shift count outside `0 .. width`, a signed left shift of a negative
//! value, and a signed left shift whose mathematical result does not fit are
//! undefined. Those counts are not masked. Constant initializers reject
//! them; a runtime expression does not. Conversions between integer types are
//! `sext`, `zext`, or `trunc`. A same-width signed/unsigned conversion is a
//! no-op in the IR. `sizeof` is an `unsigned long` constant and its operand
//! is not emitted.

use crate::abi::{
    arg_names, assign_regs, format_part, function_sig, memory_part, AggAbi, FnSig, Part, PartKind,
    RegPiece,
};
use crate::ast::*;
use crate::diag::Diagnostic;

mod access;
mod call;
mod consts;
mod expr;
mod inline_builtins;
mod module;
mod stmt;
mod vaarg;
mod vla;
use access::*;
use call::*;
use consts::*;
use expr::*;
use module::*;
use vla::*;

/// Render the checked unit as a textual LLVM IR module.
///
/// Semantic analysis has rejected every invalid program, so an error here
/// is an internal compiler error: a case that analysis should have caught.
/// It is reported at the source position being lowered, with phase `irgen`.
pub fn render(unit: &TranslationUnit, filename: &str) -> Result<String, Diagnostic> {
    render_module(unit, filename)
        .map_err(|ice| Diagnostic::internal(filename, ice.line, ice.column, "irgen", ice.message))
}

/// An internal compiler error found while lowering. `line` 0 means that no
/// source position is known, as for a file-scope initializer.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Ice {
    line: usize,
    column: usize,
    message: String,
}

impl Ice {
    fn new(line: usize, column: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            column,
            message: message.into(),
        }
    }

    /// An error with no source position.
    fn global(message: impl Into<String>) -> Self {
        Self::new(0, 0, message)
    }
}

/// Declarations the function body needs inserted in the module prelude.
#[derive(Default)]
struct LowerNeeds {
    memmove: bool,
    va: bool,
    memcpy: bool,
    memset: bool,
    nan: bool,
    nanf: bool,
    nanl: bool,
    intrinsics: std::collections::BTreeSet<String>,
}

impl LowerNeeds {
    fn merge(&mut self, other: Self) {
        self.intrinsics.extend(other.intrinsics);
        self.memmove |= other.memmove;
        self.va |= other.va;
        self.memcpy |= other.memcpy;
        self.memset |= other.memset;
        self.nan |= other.nan;
        self.nanf |= other.nanf;
        self.nanl |= other.nanl;
    }
}

fn has_func_symbol(unit: &TranslationUnit, name: &str) -> bool {
    unit.func_decls.iter().any(|decl| decl.name == name)
        || unit.decls.iter().any(|decl| {
            matches!(
                decl,
                TopDecl::Func(func) if func.body.is_some() && func.symbol == name
            )
        })
}

struct Gen {
    /// IR blocks of the labels whose address `&&` takes (GNU). A computed
    /// `goto` lists all of them as its possible destinations.
    addr_labels: Vec<String>,
    lines: Vec<String>,
    tmp: u32,
    lab: u32,
    cur: String,
    terminated: bool,
    /// `break` targets. A loop and a `switch` each push the block after
    /// themselves. `break` uses the innermost one.
    breaks: Vec<String>,
    /// `continue` targets. Only loops push one. A `switch` does not, so
    /// `continue` inside a switch still belongs to the enclosing loop.
    continues: Vec<String>,
    /// Case and default blocks of the switch currently being emitted, in
    /// the order those statements are visited. A nested switch saves this.
    switch_labels: Vec<String>,
    switch_at: usize,
    ret_ty: CType,
    /// A whole-struct copy used `llvm.memmove`, which keeps padding bytes.
    needs_memmove: bool,
    /// The body calls `llvm.va_start`, `llvm.va_end`, or `llvm.va_copy`.
    needs_va: bool,
    needs_memcpy: bool,
    needs_memset: bool,
    /// Emit a library `nan` / `nanf` / `nanl` declaration if the symbol
    /// is not already in the unit.
    needs_nan: bool,
    needs_nanf: bool,
    needs_nanl: bool,
    /// Index in `lines` where the next entry-block alloca is inserted.
    alloca_at: usize,
    /// Set when the C return type is a struct or union.
    ret_agg: Option<AggAbi>,
    /// Private IR names of globals and functions (see [`library_renames`]).
    renames: Renames,
    /// Pointers that may be less aligned than their pointee type: members
    /// of a GNU `packed` record and addresses derived from them.
    unaligned: std::collections::HashSet<String>,
    /// `declare` lines for LLVM intrinsics the body calls.
    intrinsics: std::collections::BTreeSet<String>,
    /// GNU case ranges of the switch being collected: low and high bits
    /// and the case block.
    switch_ranges: Vec<(u128, u128, String)>,
    /// Line and column of the statement or expression being lowered, for an
    /// internal compiler error.
    pos: (usize, usize),
    /// The first internal compiler error. Lowering goes on with a
    /// placeholder operand, so callers need not check; `render_function`
    /// then returns this instead of the function's IR.
    ice: Option<Ice>,
    /// Typed VLA size expressions for this translation unit.
    vla_bounds: std::collections::HashMap<VlaId, Expr>,
    vla_counts: std::collections::HashMap<VlaId, String>,
    vla_bytes: std::collections::HashMap<VlaId, String>,
    /// Entry-block slots for `vla_bytes`, used when a computed goto can
    /// reach a use of the size along a path that does not go through the
    /// declaration that stored the SSA value.
    vla_byte_slots: std::collections::HashMap<VlaId, String>,
    /// Entry-block pointer slots for VLA objects, used when a computed
    /// goto can reach a use without passing through the dynamic alloca.
    vla_ptr_slots: std::collections::HashSet<String>,
    /// `llvm.stacksave` values for open VLA object scopes, innermost last.
    vla_saves: Vec<String>,
    /// Entry-block slots holding those saves so a computed-goto trampoline
    /// at the end of the function can restore a depth that did not exist
    /// when the destination label was emitted.
    vla_slots: Vec<String>,
    /// Entry-block slot holding the current VLA depth (i32).
    vla_depth_slot: Option<String>,
    /// C names of labels whose address this function takes, in the order
    /// `addr_labels` lists their trampoline blocks.
    addr_label_names: Vec<String>,
    /// Number of open VLA object scopes at each C label in this function.
    label_vla_depths: std::collections::HashMap<String, usize>,
    /// VLA scope depth at each `break` target.
    break_vla: Vec<usize>,
    /// VLA scope depth at each `continue` target.
    continue_vla: Vec<usize>,
}

/// One store of an automatic aggregate initializer, evaluated and waiting
/// for the zero fill: a scalar, a record copy from a source address, or a
/// bit-field write.
enum PendingStore {
    Scalar(u64, CType, String),
    Record(u64, CType, String, bool),
    Bits(u64, BitStore, CType, String),
}

impl Gen {
    fn new(ret_ty: CType, ret_agg: Option<AggAbi>) -> Self {
        Self {
            addr_labels: Vec::new(),
            lines: Vec::new(),
            tmp: 0,
            lab: 0,
            cur: String::new(),
            terminated: false,
            breaks: Vec::new(),
            continues: Vec::new(),
            switch_labels: Vec::new(),
            switch_at: 0,
            ret_ty,
            needs_memmove: false,
            needs_va: false,
            needs_memcpy: false,
            needs_memset: false,
            needs_nan: false,
            needs_nanf: false,
            needs_nanl: false,
            alloca_at: 0,
            ret_agg,
            renames: Renames::new(),
            unaligned: std::collections::HashSet::new(),
            intrinsics: std::collections::BTreeSet::new(),
            switch_ranges: Vec::new(),
            pos: (0, 0),
            ice: None,
            vla_bounds: std::collections::HashMap::new(),
            vla_counts: std::collections::HashMap::new(),
            vla_bytes: std::collections::HashMap::new(),
            vla_byte_slots: std::collections::HashMap::new(),
            vla_ptr_slots: std::collections::HashSet::new(),
            vla_saves: Vec::new(),
            vla_slots: Vec::new(),
            vla_depth_slot: None,
            addr_label_names: Vec::new(),
            label_vla_depths: std::collections::HashMap::new(),
            break_vla: Vec::new(),
            continue_vla: Vec::new(),
        }
    }
}

impl Gen {
    /// Record an internal compiler error at the current source position and
    /// return a placeholder operand. Only the first error is kept.
    fn ice(&mut self, message: &str) -> String {
        let (line, column) = self.pos;
        self.ice_at(line, column, message)
    }

    /// [`Gen::ice`] at the position of `expr`.
    fn ice_in(&mut self, expr: &Expr, message: &str) -> String {
        self.ice_at(expr.line(), expr.column(), message)
    }

    fn ice_at(&mut self, line: usize, column: usize, message: &str) -> String {
        if self.ice.is_none() {
            self.ice = Some(Ice::new(line, column, message));
        }
        "poison".to_string()
    }

    /// An internal compiler error that ends lowering, at the current
    /// statement. Statement lowering returns it as `Err`. If an expression
    /// already recorded an error (see [`Gen::ice`]), that one is returned
    /// instead, so the first error found is the one reported.
    fn fail(&mut self, message: &str) -> Ice {
        let (line, column) = self.pos;
        self.fail_at(line, column, message)
    }

    /// [`Gen::fail`] at a given position.
    fn fail_at(&mut self, line: usize, column: usize, message: &str) -> Ice {
        self.ice
            .take()
            .unwrap_or_else(|| Ice::new(line, column, message))
    }

    fn emit(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    /// The IR operand for a variable's slot. A global slot `@name` follows
    /// [`library_renames`]; a local slot is unchanged.
    fn var_slot(&self, slot: &str) -> String {
        match slot
            .strip_prefix('@')
            .and_then(|name| self.renames.get(name))
        {
            Some(fresh) => llvm_global(fresh),
            // An Apple asm label starts with \01 and must be quoted.
            None => match slot.strip_prefix('@') {
                Some(name) if !name.starts_with('"') && !is_unquoted_llvm_name(name) => {
                    llvm_global(name)
                }
                _ => slot.to_string(),
            },
        }
    }

    fn fresh(&mut self) -> String {
        self.tmp += 1;
        format!("%t{}", self.tmp)
    }

    fn label(&mut self) -> String {
        self.lab += 1;
        format!("L{}", self.lab)
    }

    fn ensure_block(&mut self) {
        if self.terminated {
            let label = self.label();
            self.start(label);
        }
    }

    fn start(&mut self, name: String) {
        if !self.terminated {
            self.ice(&format!("block falls through into {name}"));
        }
        self.emit(&format!("{name}:"));
        self.cur = name;
        self.terminated = false;
    }

    fn br(&mut self, label: &str) {
        if self.terminated {
            self.ice("branch after the end of a block");
        }
        self.emit(&format!("  br label %{label}"));
        self.terminated = true;
    }

    fn br_cond(&mut self, cond: &str, then_label: &str, else_label: &str) {
        if self.terminated {
            self.ice("branch after the end of a block");
        }
        self.emit(&format!(
            "  br i1 {cond}, label %{then_label}, label %{else_label}"
        ));
        self.terminated = true;
    }
}

/// LLVM string constant. The Language Reference ends the string at the first
/// `"`. The only escapes are `\\` and `\xx` (two hex digits); a quote is `\22`.
fn llvm_string(text: &str) -> String {
    llvm_bytes(text.as_bytes())
}

fn llvm_bytes(bytes: &[u8]) -> String {
    let mut out = String::from("\"");
    for &byte in bytes {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\22"),
            0x20..=0x7e => out.push(byte as char),
            _ => out.push_str(&format!("\\{byte:02X}")),
        }
    }
    out.push('"');
    out
}

fn vol_attr(ty: &CType) -> &'static str {
    if ty.is_volatile() {
        " volatile"
    } else {
        ""
    }
}

fn c_label(name: &str) -> String {
    format!("C.{name}")
}

fn c_label_via_ptr(name: &str) -> String {
    format!("{}.via_ptr", c_label(name))
}

/// `@name` when `name` matches LLVM's unquoted identifier pattern, otherwise
/// a quoted global. C identifiers already match the unquoted pattern.
fn llvm_global(name: &str) -> String {
    if is_unquoted_llvm_name(name) {
        format!("@{name}")
    } else {
        format!("@{}", llvm_string(name))
    }
}

fn is_unquoted_llvm_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    let start = matches!(first, b'A'..=b'Z' | b'a'..=b'z' | b'$' | b'.' | b'_' | b'-');
    start
        && bytes.all(|byte| {
            matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'$' | b'.' | b'_' | b'-')
        })
}

#[cfg(test)]
mod ice_tests;
#[cfg(test)]
mod precise_tests;

#[cfg(test)]
mod tests;
