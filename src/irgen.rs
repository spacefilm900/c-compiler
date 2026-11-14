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

pub fn render(unit: &TranslationUnit, filename: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("source_filename = {}\n", llvm_string(filename)));
    let target = crate::target::current();
    out.push_str(&format!(
        "target datalayout = \"{}\"\n",
        target.data_layout()
    ));
    out.push_str(&format!("target triple = \"{}\"\n\n", target.triple()));
    let mut wrote_prelude = false;
    for handle in &unit.structs {
        if handle.is_complete() {
            out.push_str(&render_struct(handle));
            out.push('\n');
            wrote_prelude = true;
        }
    }
    let mut opaque_ids = std::collections::HashSet::new();
    for global in &unit.globals {
        let Some(handle) = global.ty.struct_ref() else {
            continue;
        };
        if handle.is_complete() || !opaque_ids.insert(handle.id()) {
            continue;
        }
        out.push_str(&format!("%{} = type opaque\n", handle.ir_name()));
        wrote_prelude = true;
    }
    for literal in &unit.strings {
        out.push_str(&render_string(literal));
        out.push('\n');
        wrote_prelude = true;
    }
    // Function bodies are rendered first because they decide which library
    // functions IR lowering calls. An internal object that shares such a
    // name is then given a private symbol and the bodies are rendered again.
    let (mut functions, mut needs) = render_functions(unit, &Renames::new());
    let renames = library_renames(unit, &needs);
    if !renames.is_empty() {
        (functions, needs) = render_functions(unit, &renames);
    }
    for global in &unit.globals {
        out.push_str(&render_global(global, &renames));
        out.push('\n');
        wrote_prelude = true;
    }
    for decl in &unit.func_decls {
        out.push_str(&render_declare(decl, unit.fn_attrs.get(&decl.name)));
        out.push('\n');
        wrote_prelude = true;
    }
    if needs.va {
        out.push_str("declare void @llvm.va_start(ptr)\n");
        out.push_str("declare void @llvm.va_end(ptr)\n");
        out.push_str("declare void @llvm.va_copy(ptr, ptr)\n");
        wrote_prelude = true;
    }
    if needs.memmove {
        // Overlap is allowed, so `s = s` is defined. The copy is the object
        // representation, padding included. An aggregate load/store is not:
        // LLVM may leave the padding `undef`.
        out.push_str("declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)\n");
        wrote_prelude = true;
    }
    if needs.memcpy {
        out.push_str("declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)\n");
        wrote_prelude = true;
    }
    if needs.memset {
        out.push_str("declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)\n");
        wrote_prelude = true;
    }
    let nanl_decl = format!("declare {} @nanl(ptr)\n", FloatKind::LongDouble.ir_type());
    // A runtime __builtin_nan* with a malformed tag lowers to nan/nanf/nanl
    // (C11 7.12.11.2). Declare the library function if this unit does not
    // already have that symbol.
    for (needed, name, decl) in [
        (needs.nan, "nan", "declare double @nan(ptr)\n"),
        (needs.nanf, "nanf", "declare float @nanf(ptr)\n"),
        (needs.nanl, "nanl", nanl_decl.as_str()),
    ] {
        if needed && !has_func_symbol(unit, name) {
            out.push_str(decl);
            wrote_prelude = true;
        }
    }

    for line in &needs.intrinsics {
        out.push_str(line);
        out.push('\n');
        wrote_prelude = true;
    }
    let used: Vec<String> = unit
        .globals
        .iter()
        .filter(|global| global.attrs.used && global.init.is_some())
        .map(|global| llvm_global(renamed(&global.name, &renames)))
        .chain(
            unit.decls
                .iter()
                .filter_map(|decl| match decl {
                    TopDecl::Func(func) if func.body.is_some() => Some(func),
                    _ => None,
                })
                .filter(|func| {
                    unit.fn_attrs
                        .get(&func.symbol)
                        .is_some_and(|attrs| attrs.used)
                })
                .map(|func| llvm_global(renamed(&func.symbol, &renames))),
        )
        .collect();
    if !used.is_empty() {
        let list: Vec<String> = used.iter().map(|name| format!("ptr {name}")).collect();
        out.push_str(&format!(
            "@llvm.used = appending global [{} x ptr] [{}], section \"llvm.metadata\"\n",
            used.len(),
            list.join(", ")
        ));
        wrote_prelude = true;
    }
    for (kind, ctor) in [("ctors", true), ("dtors", false)] {
        let entries: Vec<String> = unit
            .decls
            .iter()
            .filter_map(|decl| match decl {
                TopDecl::Func(func) if func.body.is_some() => Some(func),
                _ => None,
            })
            .filter(|func| {
                unit.fn_attrs.get(&func.symbol).is_some_and(|attrs| {
                    if ctor {
                        attrs.constructor
                    } else {
                        attrs.destructor
                    }
                })
            })
            .map(|func| {
                format!(
                    "{{ i32, ptr, ptr }} {{ i32 65535, ptr {}, ptr null }}",
                    llvm_global(renamed(&func.symbol, &renames))
                )
            })
            .collect();
        if !entries.is_empty() {
            out.push_str(&format!(
                "@llvm.global_{kind} = appending global [{} x {{ i32, ptr, ptr }}] [{}]\n",
                entries.len(),
                entries.join(", ")
            ));
            wrote_prelude = true;
        }
    }

    if wrote_prelude {
        out.push('\n');
    }
    for text in functions {
        out.push_str(&text);
        out.push_str("\n\n");
    }
    out
}

/// LLVM symbol replacements: C linker name to the private name used in IR.
type Renames = std::collections::HashMap<String, String>;

/// Library functions that IR lowering calls although the source does not
/// name them. LLVM may lower `llvm.memcpy`, `llvm.memmove`, and
/// `llvm.memset` to calls of the C library functions, and its optimizer
/// may introduce such calls too (LLVM LangRef), so those three are always
/// reserved. `nan`, `nanf`, and `nanl` are called for a malformed quiet
/// `__builtin_nan*` tag.
const MEMORY_LIBCALLS: [&str; 3] = ["memcpy", "memmove", "memset"];

fn render_functions(unit: &TranslationUnit, renames: &Renames) -> (Vec<String>, LowerNeeds) {
    let mut functions = Vec::new();
    let mut needs = LowerNeeds::default();
    for decl in &unit.decls {
        if let TopDecl::Func(func) = decl {
            if func.body.is_some() {
                let (text, fn_needs) =
                    render_function(func, renames, unit.fn_attrs.get(&func.symbol));
                needs.merge(fn_needs);
                functions.push(text);
            }
        }
    }
    (functions, needs)
}

/// Private LLVM names for internal-linkage entities whose C name is a
/// library function that the generated code calls.
///
/// LLVM IR has one namespace for globals and functions, and an internal
/// symbol in the object file would also capture a call the backend
/// emits. Internal linkage means the name is not visible outside this
/// translation unit (C11 6.2.2p3), so renaming it in IR does not change
/// the program. Objects named `nan`, `nanf`, or `nanl` are renamed only
/// when that call is emitted. Objects and defined functions named
/// `memcpy`, `memmove`, or `memset` are always renamed. A user-defined
/// `nan` function stays the call target, as it is when declared.
fn library_renames(unit: &TranslationUnit, needs: &LowerNeeds) -> Renames {
    let mut taken: std::collections::HashSet<&str> = unit
        .globals
        .iter()
        .map(|global| global.name.as_str())
        .chain(unit.func_decls.iter().map(|decl| decl.name.as_str()))
        .collect();
    for decl in &unit.decls {
        if let TopDecl::Func(func) = decl {
            taken.insert(func.symbol.as_str());
        }
    }
    let nan_calls = [
        (needs.nan, "nan"),
        (needs.nanf, "nanf"),
        (needs.nanl, "nanl"),
    ];
    let clashes = |name: &str, objects: bool| {
        MEMORY_LIBCALLS.contains(&name)
            || (objects && nan_calls.iter().any(|(used, lib)| *used && *lib == name))
    };
    let mut wanted = Vec::new();
    for global in &unit.globals {
        if global.linkage == Linkage::Internal && clashes(&global.name, true) {
            wanted.push(global.name.clone());
        }
    }
    for decl in &unit.decls {
        if let TopDecl::Func(func) = decl {
            if func.body.is_some()
                && func.linkage == Linkage::Internal
                && clashes(&func.symbol, false)
            {
                wanted.push(func.symbol.clone());
            }
        }
    }
    let mut renames = Renames::new();
    for name in wanted {
        let mut n = 1u32;
        let fresh = loop {
            let candidate = format!("{name}.{n}");
            if !taken.contains(candidate.as_str())
                && !renames.values().any(|used| *used == candidate)
            {
                break candidate;
            }
            n += 1;
        };
        renames.insert(name, fresh);
    }
    renames
}

/// The IR name of a global or function symbol after [`library_renames`].
fn renamed<'a>(name: &'a str, renames: &'a Renames) -> &'a str {
    renames.get(name).map_or(name, String::as_str)
}

fn render_global(global: &IrGlobal, renames: &Renames) -> String {
    let name = llvm_global(renamed(&global.name, renames));
    let ir = global.ty.ir_type();
    let align = global.ty.align().max(global.attrs.align.unwrap_or(0));
    let section = global
        .attrs
        .section
        .as_ref()
        .map(|name| format!(", section {}", llvm_string(name)))
        .unwrap_or_default();
    // A const object is an LLVM `constant`. `const int *` is not const; the
    // pointer can be reseated. An array of const elements is constant.
    let kind = if global.ty.is_const_object() {
        "constant"
    } else {
        "global"
    };
    if let Some(value) = &global.init {
        let linkage = match global.linkage {
            Linkage::Internal => "internal ",
            Linkage::External if global.attrs.weak => "weak ",
            Linkage::External => "",
        };
        // An address inside a union or bit-field image needs a constant of
        // its own shape. The global then has that type instead of `ir`;
        // every access goes through an opaque `ptr`, so only the size and
        // alignment have to agree.
        let (ir, spelled) = render_typed_const(&global.ty, value, renames);
        format!("{name} = {linkage}{kind} {ir} {spelled}{section}, align {align}")
    } else if global.attrs.weak {
        format!("{name} = extern_weak {kind} {ir}, align {align}")
    } else {
        format!("{name} = external {kind} {ir}, align {align}")
    }
}

fn render_struct(handle: &StructRef) -> String {
    if handle.is_union() || handle.has_bitfield() {
        let size = handle.size().unwrap_or(0);
        let align = u64::from(handle.align().max(1));
        let body = if align != 0 && size % align == 0 && matches!(align, 1 | 2 | 4 | 8 | 16) {
            let elem = match align {
                1 => "i8",
                2 => "i16",
                4 => "i32",
                8 => "i64",
                _ => "i128",
            };
            format!("[ {} x {elem} ]", size / align)
        } else {
            format!("[ {size} x i8 ]")
        };
        return format!("%{} = type {{ {body} }}", handle.llvm_name());
    }
    let fields = handle.fields().unwrap_or_default();
    let body = fields
        .iter()
        .map(|field| field.ty.ir_type())
        .collect::<Vec<_>>()
        .join(", ");
    format!("%{} = type {{ {body} }}", handle.llvm_name())
}

fn render_string(literal: &StrConst) -> String {
    let unit = literal.enc.unit();
    if unit == 1 {
        let n = literal.bytes.len();
        return format!(
            "@{} = private unnamed_addr constant [{n} x i8] c{}, align 1",
            literal.name,
            llvm_bytes(&literal.bytes)
        );
    }
    let ir = if unit == 2 { "i16" } else { "i32" };
    let count = literal.bytes.len() / unit;
    let mut parts = Vec::with_capacity(count);
    for chunk in literal.bytes.chunks(unit) {
        let mut acc = 0u32;
        for (index, byte) in chunk.iter().enumerate() {
            acc |= u32::from(*byte) << (8 * index);
        }
        parts.push(format!("{ir} {acc}"));
    }
    format!(
        "@{} = private unnamed_addr constant [{count} x {ir}] [{}], align {unit}",
        literal.name,
        parts.join(", ")
    )
}

fn render_declare(decl: &IrFunc, attrs: Option<&FnAttrs>) -> String {
    // An unused prototype whose aggregate is still incomplete, or whose
    // return would use `%st0`, has no callable signature yet. The symbol
    // still has to exist when its address is taken.
    let Ok(sig) = function_sig(&decl.ret, &decl.params) else {
        return format!("declare void {}()", llvm_global(&decl.name));
    };
    let params = sig_params(&sig, decl.variadic, None);
    let weak = if attrs.is_some_and(|attrs| attrs.weak) {
        "extern_weak "
    } else {
        ""
    };
    format!(
        "declare {weak}{}{} {}({params}){}",
        sig.ret_prefix,
        sig.ret_type,
        llvm_global(&decl.name),
        fn_attr_suffix(attrs, false)
    )
}

/// LLVM function attributes for GNU function attributes (LLVM LangRef,
/// "Function Attributes"), with a leading space, and the section of a
/// definition.
fn fn_attr_suffix(attrs: Option<&FnAttrs>, definition: bool) -> String {
    let target_attrs = crate::target::current().function_attrs();
    let Some(attrs) = attrs else {
        if definition && !target_attrs.is_empty() {
            return format!(" {target_attrs}");
        }
        return String::new();
    };
    let mut parts = Vec::new();
    if attrs.noreturn {
        parts.push("noreturn".to_string());
    }
    if attrs.noinline {
        parts.push("noinline".to_string());
    } else if attrs.always_inline {
        parts.push("alwaysinline".to_string());
    }
    if attrs.cold {
        parts.push("cold".to_string());
    } else if attrs.hot {
        parts.push("hot".to_string());
    }
    if attrs.const_fn {
        parts.push("memory(none)".to_string());
    } else if attrs.pure {
        parts.push("memory(read)".to_string());
    }
    if definition && !target_attrs.is_empty() {
        parts.push(target_attrs.to_string());
    }
    if definition {
        if let Some(section) = &attrs.section {
            parts.push(format!("section {}", llvm_string(section)));
        }
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" {}", parts.join(" "))
    }
}

/// Parameter list of a lowered signature. `names` supplies one LLVM name per
/// C parameter; declarations pass `None`.
fn sig_params(sig: &FnSig, variadic: bool, names: Option<&[Vec<String>]>) -> String {
    let mut parts = Vec::new();
    if let Some(attrs) = &sig.sret_attrs {
        let name = names.map(|_| "%agg.result");
        parts.push(format_part("ptr", attrs, name));
    }
    for (index, group) in sig.params.iter().enumerate() {
        for (piece, part) in group.iter().enumerate() {
            let name = names
                .and_then(|names| names.get(index))
                .and_then(|group| group.get(piece).map(String::as_str));
            parts.push(format_part(&part.ir_type, &part.attrs, name));
        }
    }
    if variadic {
        parts.push("...".to_string());
    }
    parts.join(", ")
}

fn param_ir(ty: &CType, name: Option<&str>) -> String {
    let ir = ty.ir_type();
    match (ty.abi_attr(), name) {
        (Some(attr), Some(name)) => format!("{ir} {attr} {name}"),
        (Some(attr), None) => format!("{ir} {attr}"),
        (None, Some(name)) => format!("{ir} {name}"),
        (None, None) => ir,
    }
}

fn render_function(
    func: &FuncDef,
    renames: &Renames,
    attrs: Option<&FnAttrs>,
) -> (String, LowerNeeds) {
    let body = func.body.as_ref().expect("defined function");
    let param_tys: Vec<CType> = func.params.iter().map(|param| param.ty.clone()).collect();
    // Semantic analysis rejects a definition whose aggregate needs an SSE
    // register or is still incomplete, so the signature is known here.
    let sig = function_sig(&func.ret_type, &param_tys)
        .expect("defined function has a System V signature");
    let names: Vec<Vec<String>> = sig
        .params
        .iter()
        .enumerate()
        .map(|(index, group)| arg_names(index, group.len()))
        .collect();
    let mut gen = Gen::new(func.ret_type.clone(), sig.ret_agg.clone());
    gen.renames = renames.clone();
    gen.addr_labels = func.addr_labels.iter().map(|name| c_label(name)).collect();
    let linkage = match func.linkage {
        Linkage::Internal => "internal ",
        Linkage::External if attrs.is_some_and(|attrs| attrs.weak) => "weak ",
        Linkage::External => "",
    };
    let params = sig_params(&sig, func.variadic, Some(&names));
    gen.emit(&format!(
        "define {linkage}{}{} {}({params}){} {{",
        sig.ret_prefix,
        sig.ret_type,
        llvm_global(renamed(&func.symbol, renames)),
        fn_attr_suffix(attrs, true)
    ));
    gen.emit("entry:");
    gen.cur = "entry".to_string();
    gen.terminated = false;

    for param in &func.params {
        gen.emit(&format!(
            "  {} = alloca {}, align {}",
            param.slot,
            param.ty.ir_type(),
            param.ty.align()
        ));
    }
    for (slot, ty, align) in local_slots(body) {
        gen.emit(&format!(
            "  {slot} = alloca {}, align {align}",
            ty.ir_type()
        ));
    }
    // Temps allocated while the body is emitted are inserted here, still in
    // the entry block, so a call inside a loop does not grow the stack.
    gen.alloca_at = gen.lines.len();
    for ((param, names), parts) in func.params.iter().zip(names.iter()).zip(sig.params.iter()) {
        gen.store_param(param, names, parts);
    }

    gen.gen_compound(body);
    if !gen.terminated {
        gen.emit_implicit_return();
    }
    gen.emit("}");
    (
        gen.lines.join("\n"),
        LowerNeeds {
            memmove: gen.needs_memmove,
            va: gen.needs_va,
            memcpy: gen.needs_memcpy,
            memset: gen.needs_memset,
            nan: gen.needs_nan,
            nanf: gen.needs_nanf,
            nanl: gen.needs_nanl,
            intrinsics: gen.intrinsics,
        },
    )
}

fn local_slots(compound: &Compound) -> Vec<(String, CType, u32)> {
    let mut slots = Vec::new();
    for item in &compound.items {
        collect_slots(item, &mut slots);
    }
    slots
}

fn collect_slots(stmt: &Stmt, slots: &mut Vec<(String, CType, u32)>) {
    match stmt {
        Stmt::Decl(decl) if decl.storage == Storage::None => slots.push(slot_of(decl)),
        Stmt::Decl(_) | Stmt::FuncDecl(_) => {}
        Stmt::Compound(compound) => {
            for item in &compound.items {
                collect_slots(item, slots);
            }
        }
        Stmt::If {
            then_stmt,
            else_stmt,
            ..
        } => {
            collect_slots(then_stmt, slots);
            if let Some(else_stmt) = else_stmt {
                collect_slots(else_stmt, slots);
            }
        }
        Stmt::While { body, .. } | Stmt::Do { body, .. } => collect_slots(body, slots),
        Stmt::Switch { body, .. }
        | Stmt::Case { stmt: body, .. }
        | Stmt::Default { stmt: body, .. } => collect_slots(body, slots),
        Stmt::For { init, body, .. } => {
            if let ForInit::Decl(decls) = init {
                for decl in decls {
                    if decl.storage == Storage::None {
                        slots.push(slot_of(decl));
                    }
                }
            }
            collect_slots(body, slots);
        }
        Stmt::Label { stmt, .. } => collect_slots(stmt, slots),
        Stmt::Return { .. }
        | Stmt::Expr { .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Goto { .. }
        | Stmt::GotoPtr { .. } => {}
    }
}

/// An automatic object's slot, type, and alignment, including a GNU
/// `aligned` attribute.
fn slot_of(decl: &VarDecl) -> (String, CType, u32) {
    let align = decl.ty.align().max(decl.align.unwrap_or(0));
    (decl.slot.clone(), decl.ty.clone(), align)
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
        }
    }
}

impl Gen {
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
        assert!(
            self.terminated,
            "internal compiler error: fallthrough into {name}"
        );
        self.emit(&format!("{name}:"));
        self.cur = name;
        self.terminated = false;
    }

    fn br(&mut self, label: &str) {
        assert!(
            !self.terminated,
            "internal compiler error: branch after terminator"
        );
        self.emit(&format!("  br label %{label}"));
        self.terminated = true;
    }

    fn br_cond(&mut self, cond: &str, then_label: &str, else_label: &str) {
        assert!(
            !self.terminated,
            "internal compiler error: branch after terminator"
        );
        self.emit(&format!(
            "  br i1 {cond}, label %{then_label}, label %{else_label}"
        ));
        self.terminated = true;
    }

    fn gen_compound(&mut self, compound: &Compound) {
        for item in &compound.items {
            self.gen_stmt(item);
        }
    }

    fn gen_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Compound(compound) => self.gen_compound(compound),
            Stmt::Decl(decl) => {
                self.ensure_block();
                self.gen_decl(decl);
            }
            Stmt::Return { expr, .. } => {
                self.ensure_block();
                if let Some(expr) = expr {
                    if self.ret_ty.unqualified().is_record() {
                        let src_volatile = expr.ty().is_some_and(|ty| ty.is_volatile());
                        let src = self.record_ptr(expr);
                        self.emit_record_return(&src, src_volatile);
                    } else {
                        let value = self.gen_val(expr);
                        let value =
                            self.convert(&value, &expr.ty().expect("typed"), &self.ret_ty.clone());
                        self.emit(&format!("  ret {} {value}", self.ret_ty.ir_type()));
                    }
                } else {
                    self.emit("  ret void");
                }
                self.terminated = true;
            }
            Stmt::If {
                cond,
                then_stmt,
                else_stmt,
                ..
            } => self.gen_if(cond, then_stmt, else_stmt.as_deref()),
            Stmt::While { cond, body, .. } => self.gen_while(cond, body),
            Stmt::Do { body, cond, .. } => self.gen_do(body, cond),
            Stmt::Switch {
                cond,
                body,
                promoted,
                ..
            } => self.gen_switch(cond, body, promoted.expect("switch type")),
            Stmt::Case { stmt, .. } | Stmt::Default { stmt, .. } => {
                let label = self.take_switch_label();
                self.place_label(&label);
                self.gen_stmt(stmt);
            }
            Stmt::FuncDecl(_) => {}
            Stmt::For {
                init,
                cond,
                step,
                body,
                ..
            } => self.gen_for(init, cond.as_ref(), step.as_ref(), body),
            Stmt::Expr { expr, .. } => {
                self.ensure_block();
                if let Some(expr) = expr {
                    let _ = self.gen_expr(expr);
                }
            }
            Stmt::Label { name, stmt, .. } => {
                let block = c_label(name);
                self.place_label(&block);
                self.gen_stmt(stmt);
            }
            Stmt::Goto { name, .. } => {
                self.ensure_block();
                self.br(&c_label(name));
            }
            Stmt::GotoPtr { expr, .. } => {
                // An indirect branch may reach any label whose address the
                // function takes (LLVM LangRef, `indirectbr`).
                self.ensure_block();
                let target = self.gen_val(expr);
                let dests: Vec<String> = self
                    .addr_labels
                    .iter()
                    .map(|label| format!("label %{label}"))
                    .collect();
                self.emit(&format!(
                    "  indirectbr ptr {target}, [{}]",
                    dests.join(", ")
                ));
                self.terminated = true;
            }
            Stmt::Break { .. } => {
                self.ensure_block();
                let end = self
                    .breaks
                    .last()
                    .expect("break outside loop or switch")
                    .clone();
                self.br(&end);
            }
            Stmt::Continue { .. } => {
                self.ensure_block();
                let cont = self
                    .continues
                    .last()
                    .expect("continue outside loop")
                    .clone();
                self.br(&cont);
            }
        }
    }

    fn gen_decl(&mut self, decl: &VarDecl) {
        let Some(checked) = &decl.checked else {
            return;
        };
        match checked {
            CheckedInit::Scalar(init) => {
                if decl.ty.is_record() {
                    let src = self.record_ptr(init);
                    let src_volatile = init.ty().is_some_and(|ty| ty.is_volatile());
                    self.copy_struct(&decl.ty, &src, &decl.slot, src_volatile);
                    return;
                }
                let value = self.gen_val(init);
                let value = self.convert(&value, &init.ty().expect("typed"), &decl.ty);
                self.store(&decl.ty, &value, &decl.slot);
            }
            CheckedInit::Aggregate(stores) => {
                // Values are evaluated in initializer order. The stores
                // follow the zero fill in that same order, so a later
                // designator overrides part of an earlier record copy.
                let mut pending = Vec::new();
                for store in stores {
                    if let Some(info) = &store.bits {
                        let value = self.gen_val(&store.expr);
                        let dest = if info.declared == TypeName::Bool {
                            CType::from_name(TypeName::Bool)
                        } else {
                            store.ty.clone()
                        };
                        let value = self.convert(&value, &store.expr.ty().expect("typed"), &dest);
                        pending.push(PendingStore::Bits(store.offset, info.clone(), dest, value));
                        continue;
                    }
                    if store.ty.is_record() {
                        let mut src = self.record_ptr(&store.expr);
                        let mut src_volatile = store.expr.ty().is_some_and(|ty| ty.is_volatile());
                        if stores.len() > 1 {
                            // The source may be an object that a later
                            // initializer changes (`.a = (s = x), .b = (s = y)`).
                            // Its value is taken now, into a temporary.
                            let tmp = self.entry_alloca(&store.ty.unqualified());
                            self.copy_struct(&store.ty.unqualified(), &src, &tmp, src_volatile);
                            src = tmp;
                            src_volatile = false;
                        }
                        pending.push(PendingStore::Record(
                            store.offset,
                            store.ty.clone(),
                            src,
                            src_volatile,
                        ));
                        continue;
                    }
                    let value = self.gen_val(&store.expr);
                    let value = self.convert(&value, &store.expr.ty().expect("typed"), &store.ty);
                    pending.push(PendingStore::Scalar(store.offset, store.ty.clone(), value));
                }
                self.emit(&format!(
                    "  store {} zeroinitializer, ptr {}, align {}",
                    decl.ty.ir_type(),
                    decl.slot,
                    decl.ty.align()
                ));
                for store in pending {
                    match store {
                        PendingStore::Scalar(offset, ty, value) => {
                            let addr = self.offset_ptr(&decl.slot, offset);
                            self.store(&ty, &value, &addr);
                        }
                        PendingStore::Record(offset, ty, src, src_volatile) => {
                            let addr = self.offset_ptr(&decl.slot, offset);
                            self.copy_struct(&ty, &src, &addr, src_volatile);
                        }
                        PendingStore::Bits(offset, info, ty, value) => {
                            let addr = self.offset_ptr(&decl.slot, offset);
                            self.write_bits(&addr, &info, &ty, &value);
                        }
                    }
                }
            }
        }
    }

    /// Byte copy of a struct, including padding. `llvm.memmove` allows the
    /// source and destination to be the same object.
    fn copy_struct(&mut self, ty: &CType, src: &str, dest: &str, src_volatile: bool) {
        let size = ty.size().expect("complete struct");
        self.needs_memmove = true;
        // A volatile source or destination is one access. LLVM may drop a
        // non-volatile `memmove` of a volatile object.
        let volatile = if ty.is_volatile() || src_volatile {
            "true"
        } else {
            "false"
        };
        self.emit(&format!(
            "  call void @llvm.memmove.p0.p0.i64(ptr {dest}, ptr {src}, i64 {size}, i1 {volatile})"
        ));
    }

    /// Pointer to the object representation of a struct or union expression.
    ///
    /// Lvalues are their address. A call, a conditional, or an assignment is
    /// materialized first, so `f().x` and `s = f()` see the returned bytes.
    fn record_ptr(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Var { .. }
            | Expr::Index { .. }
            | Expr::Str { .. }
            | Expr::Unary {
                op: UnaryOp::Deref, ..
            }
            | Expr::Member { .. }
            | Expr::CompoundLit { .. } => self.gen_addr(expr),
            Expr::VaArg { .. } => self.gen_va_record(expr),
            Expr::Assign { target, value, .. } => {
                // The right-hand side is evaluated before the destination is
                // stored, matching scalar assignment.
                let src_volatile = value.ty().is_some_and(|ty| ty.is_volatile());
                let src = self.record_ptr(value);
                let dest = self.gen_addr(target);
                let ty = target.ty().expect("struct assignment");
                self.copy_struct(&ty, &src, &dest, src_volatile);
                dest
            }
            Expr::Call { .. } => self.materialize_call(expr),
            Expr::Cond {
                cond,
                then_expr,
                else_expr,
                ty,
                ..
            } => {
                let ty = ty.clone().expect("typed conditional");
                let tmp = self.entry_alloca(&ty);
                let cond = self.gen_cond(cond);
                let then_label = self.label();
                let else_label = self.label();
                let end_label = self.label();
                self.br_cond(&cond, &then_label, &else_label);
                self.start(then_label);
                let then_volatile = then_expr.ty().is_some_and(|ty| ty.is_volatile());
                let then_src = self.record_ptr(then_expr);
                self.copy_struct(&ty, &then_src, &tmp, then_volatile);
                self.br(&end_label);
                self.start(else_label);
                let else_volatile = else_expr.ty().is_some_and(|ty| ty.is_volatile());
                let else_src = self.record_ptr(else_expr);
                self.copy_struct(&ty, &else_src, &tmp, else_volatile);
                self.br(&end_label);
                self.start(end_label);
                tmp
            }
            Expr::Comma { left, right, .. } => {
                let _ = self.gen_expr(left);
                self.record_ptr(right)
            }
            Expr::StmtExpr { body, ty, .. } => {
                let ty = ty.clone().expect("typed statement expression");
                let last = self
                    .gen_stmt_expr_body(body)
                    .expect("a record-valued statement expression ends in an expression");
                let volatile = last.ty().is_some_and(|ty| ty.is_volatile());
                let src = self.record_ptr(last);
                self.ensure_block();
                // The block's objects end with it; the value is a copy.
                let tmp = self.entry_alloca(&ty);
                self.copy_struct(&ty, &src, &tmp, volatile);
                tmp
            }
            _ => panic!("internal compiler error: aggregate expression has no address"),
        }
    }

    fn materialize_call(&mut self, expr: &Expr) -> String {
        let Expr::Call {
            callee,
            args,
            resolved,
            ty,
            ..
        } = expr
        else {
            unreachable!("call");
        };
        let ty = ty.clone().expect("typed call");
        let resolved = resolved.as_ref().expect("resolved call");
        let sig =
            function_sig(&resolved.ret, &resolved.params).expect("call has a System V signature");
        let tmp = self.entry_alloca(&ty);
        match sig.ret_agg.clone() {
            Some(AggAbi::Memory) => {
                let rendered = self.render_call_args(args, resolved, &sig, Some(&tmp));
                let _ = self.emit_call(callee, resolved, &sig, &rendered);
            }
            Some(AggAbi::X87) => {
                let rendered = self.render_call_args(args, resolved, &sig, None);
                let value = self
                    .emit_call(callee, resolved, &sig, &rendered)
                    .expect("x87 aggregate returns a value");
                let align = ty.align().max(16);
                self.emit(&format!(
                    "  store x86_fp80 {value}, ptr {tmp}, align {align}"
                ));
            }
            Some(AggAbi::Direct(pieces)) => {
                let rendered = self.render_call_args(args, resolved, &sig, None);
                let value = self
                    .emit_call(callee, resolved, &sig, &rendered)
                    .expect("register aggregate returns a value");
                self.store_pieces(&tmp, ty.align(), &pieces, &value);
            }
            Some(AggAbi::Coerce { ty: ir, size }) => {
                let rendered = self.render_call_args(args, resolved, &sig, None);
                let value = self
                    .emit_call(callee, resolved, &sig, &rendered)
                    .expect("register aggregate returns a value");
                self.store_coerced(&value, &tmp, &ty, &ir, size);
            }
            Some(AggAbi::Indirect) | None => unreachable!("aggregate call"),
        }
        tmp
    }

    /// `sret_slot` is the temporary the callee writes when the return is in memory.
    fn render_call_args(
        &mut self,
        args: &[Expr],
        resolved: &CallRes,
        sig: &FnSig,
        sret_slot: Option<&str>,
    ) -> Vec<String> {
        let mut rendered = Vec::new();
        let mut free_int = sig.free_int;
        let mut free_sse = sig.free_sse;
        if let Some(slot) = sret_slot {
            let attrs = sig.sret_attrs.as_deref().expect("sret attribute");
            rendered.push(format_part("ptr", attrs, Some(slot)));
        }
        for (index, arg) in args.iter().enumerate() {
            if index < resolved.params.len() {
                let parts = &sig.params[index];
                if parts.len() == 1 && matches!(parts[0].kind, PartKind::Byval) {
                    let src = self.record_ptr(arg);
                    rendered.push(format_part("ptr", &parts[0].attrs, Some(&src)));
                    continue;
                }
                if parts.len() == 1
                    && matches!(parts[0].kind, PartKind::Coerce { .. } | PartKind::Indirect)
                {
                    let value = self.aapcs64_record_arg(arg, &parts[0]);
                    rendered.push(format_part(
                        &parts[0].ir_type,
                        &parts[0].attrs,
                        Some(&value),
                    ));
                    continue;
                }
                if parts
                    .iter()
                    .any(|part| matches!(part.kind, PartKind::Piece(_)))
                {
                    let src = self.record_ptr(arg);
                    let align = arg.ty().expect("typed").align();
                    for part in parts {
                        let PartKind::Piece(piece) = &part.kind else {
                            continue;
                        };
                        let loaded = self.load_piece(&src, align, piece);
                        rendered.push(format_part(&part.ir_type, &part.attrs, Some(&loaded)));
                    }
                    continue;
                }
                let value = self.gen_val(arg);
                let src = arg.ty().expect("typed");
                let param_ty = &resolved.params[index];
                let value = self.convert(&value, &src, param_ty);
                rendered.push(param_ir(param_ty, Some(&value)));
            } else if arg.ty().is_some_and(|ty| ty.is_record()) {
                rendered.extend(self.variadic_record(arg, &mut free_int, &mut free_sse));
            } else {
                // Default argument promotions (C11 6.5.2.2): integer types
                // narrower than `int` become `int`, and `float` becomes
                // `double`. A floating argument uses an XMM register.
                let value = self.gen_val(arg);
                let src = arg.ty().expect("typed");
                let promoted = default_promote(&src);
                if promoted.is_sse_float() {
                    free_sse = free_sse.saturating_sub(1);
                } else if !promoted.is_long_double() {
                    // A 128-bit integer takes two GP registers. If only
                    // one remains, the whole argument goes on the stack.
                    // Only its own registers are given back (System V
                    // AMD64 3.2.3), so a later argument can still use the
                    // leftover register.
                    let slots = if promoted.size() == Some(16) { 2 } else { 1 };
                    if free_int >= slots {
                        free_int -= slots;
                    }
                }
                let value = self.convert(&value, &src, &promoted);
                rendered.push(format!("{} {value}", promoted.ir_type()));
            }
        }
        rendered
    }

    fn variadic_record(
        &mut self,
        arg: &Expr,
        free_int: &mut usize,
        free_sse: &mut usize,
    ) -> Vec<String> {
        let ty = arg.ty().expect("typed");
        if crate::target::current().is_aarch64() {
            let parts = crate::aapcs64::arg_parts(&ty).expect("variadic aggregate");
            let value = self.aapcs64_record_arg(arg, &parts[0]);
            return vec![format_part(&parts[0].ir_type, "", Some(&value))];
        }
        let src = self.record_ptr(arg);
        let agg =
            crate::abi::classify_for(&ty, crate::abi::AggUse::Param).expect("variadic aggregate");
        let parts = match agg {
            AggAbi::Memory | AggAbi::X87 | AggAbi::Coerce { .. } | AggAbi::Indirect => {
                vec![memory_part(&ty)]
            }
            AggAbi::Direct(pieces) => pieces
                .into_iter()
                .map(|piece| Part {
                    ir_type: piece.ir_type(),
                    attrs: String::new(),
                    kind: PartKind::Piece(piece),
                })
                .collect(),
        };
        let parts = assign_regs(parts, &ty, free_int, free_sse);
        parts
            .iter()
            .map(|part| match &part.kind {
                PartKind::Byval => format_part("ptr", &part.attrs, Some(&src)),
                PartKind::Piece(piece) => {
                    let loaded = self.load_piece(&src, ty.align(), piece);
                    format_part(&piece.ir_type(), "", Some(&loaded))
                }
                PartKind::Scalar | PartKind::Coerce { .. } | PartKind::Indirect => {
                    unreachable!("System V aggregate piece")
                }
            })
            .collect()
    }

    fn emit_call(
        &mut self,
        callee: &Expr,
        resolved: &CallRes,
        sig: &FnSig,
        rendered: &[String],
    ) -> Option<String> {
        let arglist = rendered.join(", ");
        let callee = if resolved.indirect {
            self.gen_val(callee)
        } else {
            llvm_global(renamed(&resolved.name, &self.renames))
        };
        // A variadic call names the function type, including `...`
        // (`call i32 (ptr, ...) @printf(...)`). Other calls name only the
        // return type, which is what the existing tests match.
        let ty = if resolved.variadic {
            variadic_sig(sig)
        } else {
            sig.ret_type.clone()
        };
        if sig.ret_type == "void" {
            self.emit(&format!("  call {ty} {callee}({arglist})"));
            None
        } else {
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = call {}{ty} {callee}({arglist})",
                sig.ret_prefix
            ));
            Some(tmp)
        }
    }

    fn store_param(&mut self, param: &Param, names: &[String], parts: &[Part]) {
        if parts.len() == 1 && matches!(parts[0].kind, PartKind::Byval | PartKind::Indirect) {
            self.copy_struct(&param.ty, &names[0], &param.slot, false);
            return;
        }
        if let [Part {
            ir_type,
            kind: PartKind::Coerce { size },
            ..
        }] = parts
        {
            let ty = param.ty.clone();
            self.store_coerced(&names[0], &param.slot, &ty, ir_type, *size);
            return;
        }
        if parts
            .iter()
            .any(|part| matches!(part.kind, PartKind::Piece(_)))
        {
            // Pieces cover the bytes the ABI passes. Tail padding that stays
            // in memory is zero, which is a defined object representation.
            self.emit(&format!(
                "  store {} zeroinitializer, ptr {}, align {}",
                param.ty.ir_type(),
                param.slot,
                param.ty.align()
            ));
            for (part, name) in parts.iter().zip(names.iter()) {
                let PartKind::Piece(piece) = &part.kind else {
                    continue;
                };
                self.store_piece(&param.slot, param.ty.align(), piece, name);
            }
            return;
        }
        self.emit(&format!(
            "  store {} {}, ptr {}, align {}",
            param.ty.ir_type(),
            names[0],
            param.slot,
            param.ty.align()
        ));
    }

    /// An entry-block slot of LLVM type `ir`.
    fn entry_alloca_ir(&mut self, ir: &str, align: u32) -> String {
        let name = self.fresh();
        let line = format!("  {name} = alloca {ir}, align {align}");
        self.lines.insert(self.alloca_at, line);
        self.alloca_at += 1;
        name
    }

    /// One AAPCS64 aggregate argument: the coerced value, or the address
    /// of a fresh copy for a composite passed by reference.
    fn aapcs64_record_arg(&mut self, arg: &Expr, part: &Part) -> String {
        let ty = arg.ty().expect("typed");
        let src = self.record_ptr(arg);
        match part.kind {
            PartKind::Coerce { size } => {
                self.load_coerced(&src, &ty, &part.ir_type, size, ty.is_volatile())
            }
            PartKind::Indirect => {
                // The callee may modify its parameter, so it gets a copy.
                let copy = self.entry_alloca(&ty.unqualified());
                self.copy_struct(&ty, &src, &copy, ty.is_volatile());
                copy
            }
            _ => unreachable!("AAPCS64 aggregate part"),
        }
    }

    /// Load the coerced LLVM value `ir` (`size` bytes) of the aggregate at
    /// `src`. When `ir` is wider than the object, the object is first
    /// copied into a slot of type `ir` so the load reads no byte past it.
    fn load_coerced(
        &mut self,
        src: &str,
        ty: &CType,
        ir: &str,
        size: u64,
        volatile: bool,
    ) -> String {
        let object = ty.size().unwrap_or(0);
        let (from, align) = if size > object {
            let align = coerce_align(ir).max(ty.align());
            let slot = self.entry_alloca_ir(ir, align);
            if object > 0 {
                self.needs_memcpy = true;
                self.emit(&format!(
                    "  call void @llvm.memcpy.p0.p0.i64(ptr {slot}, ptr {src}, i64 {object}, i1 {volatile})"
                ));
            }
            (slot, align)
        } else {
            // A member of a packed record may sit at any byte.
            (src.to_string(), self.access_align(ty, src))
        };
        let vol = if volatile && size <= object {
            " volatile"
        } else {
            ""
        };
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = load{vol} {ir}, ptr {from}, align {align}"
        ));
        tmp
    }

    /// Store the coerced value `value` into the aggregate slot `dest`,
    /// through a slot of type `ir` when `ir` is wider than the object.
    fn store_coerced(&mut self, value: &str, dest: &str, ty: &CType, ir: &str, size: u64) {
        let object = ty.size().unwrap_or(0);
        if size <= object {
            self.emit(&format!(
                "  store {ir} {value}, ptr {dest}, align {}",
                self.access_align(ty, dest)
            ));
            return;
        }
        let align = coerce_align(ir).max(ty.align());
        let slot = self.entry_alloca_ir(ir, align);
        self.emit(&format!("  store {ir} {value}, ptr {slot}, align {align}"));
        if object > 0 {
            self.needs_memcpy = true;
            self.emit(&format!(
                "  call void @llvm.memcpy.p0.p0.i64(ptr {dest}, ptr {slot}, i64 {object}, i1 false)"
            ));
        }
    }

    fn entry_alloca(&mut self, ty: &CType) -> String {
        let name = self.fresh();
        let line = format!("  {name} = alloca {}, align {}", ty.ir_type(), ty.align());
        self.lines.insert(self.alloca_at, line);
        self.alloca_at += 1;
        name
    }

    fn offset_ptr(&mut self, slot: &str, offset: u64) -> String {
        if offset == 0 {
            return slot.to_string();
        }
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = getelementptr inbounds i8, ptr {slot}, i64 {offset}"
        ));
        tmp
    }

    fn load_piece(&mut self, slot: &str, base_align: u32, piece: &RegPiece) -> String {
        let addr = self.offset_ptr(slot, piece.offset());
        let tmp = self.fresh();
        let align = piece_align(base_align, piece.offset());
        self.emit(&format!(
            "  {tmp} = load {}, ptr {addr}, align {align}",
            piece.ir_type()
        ));
        tmp
    }

    fn store_piece(&mut self, slot: &str, base_align: u32, piece: &RegPiece, value: &str) {
        let addr = self.offset_ptr(slot, piece.offset());
        let align = piece_align(base_align, piece.offset());
        self.emit(&format!(
            "  store {} {value}, ptr {addr}, align {align}",
            piece.ir_type()
        ));
    }

    /// Write a register return into an alloca. One piece is the call result;
    /// two pieces are `extractvalue`d from it.
    fn store_pieces(&mut self, slot: &str, align: u32, pieces: &[RegPiece], value: &str) {
        if pieces.len() == 1 {
            self.store_piece(slot, align, &pieces[0], value);
            return;
        }
        let agg = crate::abi::pieces_type(pieces);
        for (index, piece) in pieces.iter().enumerate() {
            let tmp = self.fresh();
            self.emit(&format!("  {tmp} = extractvalue {agg} {value}, {index}"));
            self.store_piece(slot, align, piece, &tmp);
        }
    }

    fn emit_record_return(&mut self, src: &str, src_volatile: bool) {
        let agg = self.ret_agg.clone().expect("record return");
        let align = self.ret_ty.align();
        match agg {
            AggAbi::Memory => {
                let ty = self.ret_ty.clone();
                self.copy_struct(&ty, src, "%agg.result", src_volatile);
                self.emit("  ret void");
            }
            AggAbi::X87 => {
                let vol = if src_volatile { " volatile" } else { "" };
                let align = align.max(16);
                let tmp = self.fresh();
                self.emit(&format!(
                    "  {tmp} = load{vol} x86_fp80, ptr {src}, align {align}"
                ));
                self.emit(&format!("  ret x86_fp80 {tmp}"));
            }
            AggAbi::Coerce { ty: ir, size } => {
                let ty = self.ret_ty.clone();
                let value = self.load_coerced(src, &ty, &ir, size, src_volatile);
                self.emit(&format!("  ret {ir} {value}"));
            }
            AggAbi::Indirect => unreachable!("AAPCS64 returns use sret, not a pointer argument"),
            AggAbi::Direct(pieces) => {
                if pieces.len() == 1 {
                    let value = self.load_piece(src, align, &pieces[0]);
                    self.emit(&format!("  ret {} {value}", pieces[0].ir_type()));
                    return;
                }
                let agg_ty = crate::abi::pieces_type(&pieces);
                let mut current = String::new();
                for (index, piece) in pieces.iter().enumerate() {
                    let loaded = self.load_piece(src, align, piece);
                    let tmp = self.fresh();
                    if index == 0 {
                        self.emit(&format!(
                            "  {tmp} = insertvalue {agg_ty} poison, {} {loaded}, 0",
                            piece.ir_type()
                        ));
                    } else {
                        self.emit(&format!(
                            "  {tmp} = insertvalue {agg_ty} {current}, {} {loaded}, {index}",
                            piece.ir_type()
                        ));
                    }
                    current = tmp;
                }
                self.emit(&format!("  ret {agg_ty} {current}"));
            }
        }
    }

    fn emit_implicit_return(&mut self) {
        if let Some(agg) = self.ret_agg.clone() {
            match agg {
                AggAbi::Memory => self.emit("  ret void"),
                AggAbi::X87 => self.emit("  ret x86_fp80 0xK00000000000000000000"),
                AggAbi::Direct(pieces) => {
                    let ty = crate::abi::pieces_type(&pieces);
                    self.emit(&format!("  ret {ty} zeroinitializer"));
                }
                AggAbi::Coerce { ty, .. } => {
                    self.emit(&format!("  ret {ty} zeroinitializer"));
                }
                AggAbi::Indirect => unreachable!("AAPCS64 returns use sret"),
            }
            return;
        }
        if self.ret_ty.is_void() {
            self.emit("  ret void");
        } else if self.ret_ty.is_pointer() {
            self.emit("  ret ptr null");
        } else if let Some(kind) = self.ret_ty.float_kind() {
            self.emit(&format!(
                "  ret {} {}",
                self.ret_ty.ir_type(),
                crate::float::llvm_const(kind, 0)
            ));
        } else {
            self.emit(&format!("  ret {} 0", self.ret_ty.ir_type()));
        }
    }

    fn load_bitfield(&mut self, expr: &Expr) -> String {
        let addr = self.gen_addr(expr);
        self.load_bitfield_at(&addr, expr)
    }

    fn load_bitfield_at(&mut self, addr: &str, expr: &Expr) -> String {
        let info = bit_info(expr).expect("bit-field");
        let container = container_type(
            info.container_bits,
            info.signed_field,
            expr.ty().expect("typed").quals(),
        );
        let raw = self.load(&container, addr);
        let shift = info.abs_bit - (info.container_off as u32) * 8;
        let shifted = if shift == 0 {
            raw
        } else {
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = lshr {} {raw}, {shift}",
                container.ir_type()
            ));
            tmp
        };
        let mask = if info.width >= 64 {
            u64::MAX
        } else {
            (1u64 << info.width) - 1
        };
        let masked = self.fresh();
        self.emit(&format!(
            "  {masked} = and {} {shifted}, {mask}",
            container.ir_type()
        ));
        let value_ty = expr.ty().expect("bit-field value type");
        self.finish_bitfield(&masked, &info, &container, &value_ty)
    }

    /// The value of a bit-field after it is stored: the written bits, masked
    /// to the field width, sign-extended when the field is signed, then
    /// converted to the promoted type. C11 6.5.16 says an assignment
    /// expression has that value. Reloading the container would be a second
    /// volatile access.
    fn finish_bitfield(
        &mut self,
        masked: &str,
        info: &BitInfo,
        container: &CType,
        value_ty: &CType,
    ) -> String {
        let field = if info.signed_field && info.width < info.container_bits {
            let up = info.container_bits - info.width;
            let hi = self.fresh();
            let lo = self.fresh();
            self.emit(&format!(
                "  {hi} = shl {} {masked}, {up}",
                container.ir_type()
            ));
            self.emit(&format!("  {lo} = ashr {} {hi}, {up}", container.ir_type()));
            lo
        } else {
            masked.to_string()
        };
        self.convert(&field, container, value_ty)
    }

    fn write_bits(&mut self, addr: &str, info: &BitStore, value_ty: &CType, value: &str) {
        let container = container_type(info.container_bits, info.signed_field, value_ty.quals());
        let raw = self.load(&container, addr);
        let narrow = self.convert(value, value_ty, &container);
        let shift = info.abs_bit % info.container_bits;
        let mask = width_mask(info.width);
        let masked = self.fresh();
        self.emit(&format!(
            "  {masked} = and {} {narrow}, {mask}",
            container.ir_type()
        ));
        let placed = if shift == 0 {
            masked
        } else {
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = shl {} {masked}, {shift}",
                container.ir_type()
            ));
            tmp
        };
        let clear_mask = (!((mask as u128) << shift)) & width_mask_u128(info.container_bits);
        let cleared = self.fresh();
        self.emit(&format!(
            "  {cleared} = and {} {raw}, {clear_mask}",
            container.ir_type()
        ));
        let merged = self.fresh();
        self.emit(&format!(
            "  {merged} = or {} {cleared}, {placed}",
            container.ir_type()
        ));
        self.store(&container, &merged, addr);
    }

    fn store_bitfield(&mut self, addr: &str, expr: &Expr, value: &str, from: &CType) -> String {
        let info = bit_info(expr).expect("bit-field");
        let container = container_type(
            info.container_bits,
            info.signed_field,
            expr.ty().expect("typed").quals(),
        );
        let raw = self.load(&container, addr);
        let value_ty = expr.ty().expect("bit-field value type");
        // Assignment converts to the declared type. `_Bool` is 0 or 1, so
        // masking the original bits would turn 2 into 0.
        let declared_ty = CType::from_name(info.declared);
        let as_declared = self.convert(value, from, &declared_ty);
        let from_ty = declared_ty;
        let narrow = self.convert(&as_declared, &from_ty, &container);
        let shift = info.abs_bit - (info.container_off as u32) * 8;
        let mask = width_mask(info.width);
        let masked = self.fresh();
        self.emit(&format!(
            "  {masked} = and {} {narrow}, {mask}",
            container.ir_type()
        ));
        let placed = if shift == 0 {
            masked.clone()
        } else {
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = shl {} {masked}, {shift}",
                container.ir_type()
            ));
            tmp
        };
        let clear_mask = (!((mask as u128) << shift)) & width_mask_u128(info.container_bits);
        let cleared = self.fresh();
        self.emit(&format!(
            "  {cleared} = and {} {raw}, {clear_mask}",
            container.ir_type()
        ));
        let merged = self.fresh();
        self.emit(&format!(
            "  {merged} = or {} {cleared}, {placed}",
            container.ir_type()
        ));
        self.store(&container, &merged, addr);
        self.finish_bitfield(&masked, &info, &container, &value_ty)
    }

    fn store(&mut self, ty: &CType, value: &str, slot: &str) {
        let align = self.access_align(ty, slot);
        self.emit(&format!(
            "  store{} {} {value}, ptr {slot}, align {align}",
            vol_attr(ty),
            ty.ir_type(),
        ));
    }

    fn load(&mut self, ty: &CType, slot: &str) -> String {
        let tmp = self.fresh();
        let align = self.access_align(ty, slot);
        self.emit(&format!(
            "  {tmp} = load{} {}, ptr {slot}, align {align}",
            vol_attr(ty),
            ty.ir_type(),
        ));
        tmp
    }

    /// The alignment of an access through `slot`: the type's, or 1 for a
    /// member of a packed record.
    fn access_align(&self, ty: &CType, slot: &str) -> u32 {
        if self.unaligned.contains(slot) {
            1
        } else {
            ty.align()
        }
    }

    /// Convert `value` from `from` to `to`.
    ///
    /// Integer conversions are `sext`, `zext`, or `trunc`. Same-width
    /// signed/unsigned changes need no instruction. Pointer-to-pointer is a
    /// no-op with opaque pointers. Integer-to-pointer sign- or zero-extends
    /// to i64 and uses `inttoptr`; a constant 0 becomes `null`. Pointer-to-
    /// integer is `ptrtoint` to the destination width. That is the
    /// implementation-defined conversion Clang uses on x86_64-pc-linux-gnu.
    fn convert(&mut self, value: &str, from: &CType, to: &CType) -> String {
        // C11 6.3.1.2: a pointer converts to `_Bool` as 0 when null and 1
        // otherwise. Truncating the address to a byte is not that conversion.
        if from.is_pointer() && to.as_integer() == Some(TypeName::Bool) {
            let cmp = self.fresh();
            let tmp = self.fresh();
            self.emit(&format!("  {cmp} = icmp ne ptr {value}, null"));
            self.emit(&format!("  {tmp} = zext i1 {cmp} to i8"));
            return tmp;
        }
        if let (Some(from_i), Some(to_i)) = (from.as_integer(), to.as_integer()) {
            // `_Bool` is 0 or 1. A same-width truncation would keep the low
            // byte, so `( _Bool )2` would stay 2.
            if to_i == TypeName::Bool && from_i != TypeName::Bool {
                let cmp = self.fresh();
                let tmp = self.fresh();
                self.emit(&format!(
                    "  {cmp} = icmp ne {} {value}, 0",
                    from_i.ir_type()
                ));
                self.emit(&format!("  {tmp} = zext i1 {cmp} to i8"));
                return tmp;
            }
            if from_i.width() == to_i.width() {
                return value.to_string();
            }
            let op = if to_i.width() < from_i.width() {
                "trunc"
            } else if from_i.is_signed() {
                "sext"
            } else {
                "zext"
            };
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = {op} {} {value} to {}",
                from_i.ir_type(),
                to_i.ir_type()
            ));
            return tmp;
        }
        if let (Some(from_f), Some(to_f)) = (from.as_float(), to.as_float()) {
            if from_f == to_f || from.ir_type() == to.ir_type() {
                return value.to_string();
            }
            let op = if float_rank(to_f) > float_rank(from_f) {
                "fpext"
            } else {
                "fptrunc"
            };
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = {op} {} {value} to {}",
                from.ir_type(),
                to.ir_type()
            ));
            return tmp;
        }
        if from.is_pointer() && to.is_pointer() {
            return value.to_string();
        }
        if from.is_integer() && to.is_pointer() {
            if value == "0" {
                return "null".to_string();
            }
            let wide = self.convert(value, from, &CType::from_name(TypeName::Long));
            let tmp = self.fresh();
            self.emit(&format!("  {tmp} = inttoptr i64 {wide} to ptr"));
            return tmp;
        }
        if from.is_pointer() && to.is_integer() {
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = ptrtoint ptr {value} to {}",
                to.ir_type()
            ));
            return tmp;
        }
        if let (Some(from_f), Some(to_f)) = (from.float_kind(), to.float_kind()) {
            if from_f == to_f || from.ir_type() == to.ir_type() {
                return value.to_string();
            }
            let op = if float_rank(to_f) > float_rank(from_f) {
                "fpext"
            } else {
                "fptrunc"
            };
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = {op} {} {value} to {}",
                from.ir_type(),
                to.ir_type()
            ));
            return tmp;
        }
        if from.is_float() && to.as_integer() == Some(TypeName::Bool) {
            let cmp = self.fresh();
            let tmp = self.fresh();
            let zero = crate::float::llvm_const(from.float_kind().expect("float"), 0);
            self.emit(&format!(
                "  {cmp} = fcmp une {} {value}, {zero}",
                from.ir_type()
            ));
            self.emit(&format!("  {tmp} = zext i1 {cmp} to i8"));
            return tmp;
        }
        if from.is_float() && to.is_integer() {
            let op = if to.as_integer().is_some_and(TypeName::is_signed) {
                "fptosi"
            } else {
                "fptoui"
            };
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = {op} {} {value} to {}",
                from.ir_type(),
                to.ir_type()
            ));
            return tmp;
        }
        if from.is_integer() && to.is_float() {
            let op = if from.as_integer().is_some_and(TypeName::is_signed) {
                "sitofp"
            } else {
                "uitofp"
            };
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = {op} {} {value} to {}",
                from.ir_type(),
                to.ir_type()
            ));
            return tmp;
        }
        value.to_string()
    }

    fn extend_i64(&mut self, value: &str, ty: &CType) -> String {
        self.convert(value, ty, &CType::from_name(TypeName::Long))
    }

    /// A pointer operand, or a null pointer constant turned into `ptr null`.
    fn as_ptr(&mut self, expr: &Expr) -> String {
        let value = self.gen_val(expr);
        let ty = expr.ty().expect("typed");
        if ty.is_pointer() {
            value
        } else {
            self.convert(&value, &ty, &CType::pointer(CType::Void))
        }
    }

    fn gen_if(&mut self, cond: &Expr, then_stmt: &Stmt, else_stmt: Option<&Stmt>) {
        self.ensure_block();
        let cond = self.gen_cond(cond);
        let then_label = self.label();
        let else_label = self.label();
        let end_label = self.label();
        if else_stmt.is_some() {
            self.br_cond(&cond, &then_label, &else_label);
        } else {
            self.br_cond(&cond, &then_label, &end_label);
        }
        self.start(then_label);
        self.gen_stmt(then_stmt);
        if !self.terminated {
            self.br(&end_label);
        }
        if let Some(else_stmt) = else_stmt {
            self.start(else_label);
            self.gen_stmt(else_stmt);
            if !self.terminated {
                self.br(&end_label);
            }
        }
        self.start(end_label);
    }

    fn gen_while(&mut self, cond: &Expr, body: &Stmt) {
        self.ensure_block();
        let cond_label = self.label();
        let body_label = self.label();
        let end_label = self.label();
        self.br(&cond_label);
        self.start(cond_label.clone());
        let cond = self.gen_cond(cond);
        self.br_cond(&cond, &body_label, &end_label);
        self.start(body_label);
        self.continues.push(cond_label.clone());
        self.breaks.push(end_label.clone());
        self.gen_stmt(body);
        self.breaks.pop();
        self.continues.pop();
        if !self.terminated {
            self.br(&cond_label);
        }
        self.start(end_label);
    }

    /// The body runs, then the condition. `continue` lands on the condition,
    /// so a `continue` does not skip it and a `break` does not evaluate it.
    fn gen_do(&mut self, body: &Stmt, cond: &Expr) {
        self.ensure_block();
        let body_label = self.label();
        let cond_label = self.label();
        let end_label = self.label();
        self.br(&body_label);
        self.start(body_label.clone());
        self.continues.push(cond_label.clone());
        self.breaks.push(end_label.clone());
        self.gen_stmt(body);
        self.breaks.pop();
        self.continues.pop();
        if !self.terminated {
            self.br(&cond_label);
        }
        self.start(cond_label);
        let cond = self.gen_cond(cond);
        self.br_cond(&cond, &body_label, &end_label);
        self.start(end_label);
    }

    fn gen_for(&mut self, init: &ForInit, cond: Option<&Expr>, step: Option<&Expr>, body: &Stmt) {
        self.ensure_block();
        match init {
            ForInit::Empty => {}
            ForInit::Decl(decls) => {
                for decl in decls {
                    self.gen_decl(decl);
                }
            }
            ForInit::Expr(expr) => {
                let _ = self.gen_expr(expr);
            }
        }
        let cond_label = self.label();
        let body_label = self.label();
        let step_label = self.label();
        let end_label = self.label();
        self.br(&cond_label);
        self.start(cond_label.clone());
        if let Some(cond) = cond {
            let cond = self.gen_cond(cond);
            self.br_cond(&cond, &body_label, &end_label);
        } else {
            self.br(&body_label);
        }
        self.start(body_label);
        self.continues.push(step_label.clone());
        self.breaks.push(end_label.clone());
        self.gen_stmt(body);
        self.breaks.pop();
        self.continues.pop();
        if !self.terminated {
            self.br(&step_label);
        }
        self.start(step_label);
        if let Some(step) = step {
            let _ = self.gen_expr(step);
        }
        self.br(&cond_label);
        self.start(end_label);
    }

    /// `switch` lowers to the LLVM `switch` instruction.
    ///
    /// The controlling value is promoted first (C11 6.8.4.2), so a `char`
    /// or `short` condition is an `i32` and a `long` or `long long` condition
    /// is an `i64`. Each `case` and `default` is its own block. Falling
    /// through branches to the next label. A label inside a loop or an `if`
    /// is still a direct target, which is what Duff's device needs. Code
    /// before the first label is emitted and is not a switch target, so a
    /// jump into the body skips it, including an automatic initializer.
    fn gen_switch(&mut self, cond: &Expr, body: &Stmt, promoted: TypeName) {
        self.ensure_block();
        let raw = self.gen_val(cond);
        let src = cond.ty().expect("typed switch condition");
        let value = self.convert(&raw, &src, &CType::from_name(promoted));

        let mut cases = Vec::new();
        let mut default_label = None;
        let mut labels = Vec::new();
        let saved_ranges = std::mem::take(&mut self.switch_ranges);
        self.collect_switch_targets(body, &mut cases, &mut default_label, &mut labels);
        let ranges = std::mem::replace(&mut self.switch_ranges, saved_ranges);

        let end_label = self.label();
        let default_dest = default_label.unwrap_or_else(|| end_label.clone());
        // A GNU case range `low ... high`: a small one is listed value by
        // value; a large one is tested after the switch's own cases miss,
        // as `value - low <= high - low` unsigned.
        let mask = promoted.mask();
        let mut wide = Vec::new();
        for (low, high, label) in ranges {
            let empty = if promoted.is_signed() {
                sign_extend(low, promoted.width()) > sign_extend(high, promoted.width())
            } else {
                low & mask > high & mask
            };
            if empty {
                continue;
            }
            let span = high.wrapping_sub(low) & mask;
            if span < 256 {
                for step in 0..=span {
                    cases.push((low.wrapping_add(step) & mask, label.clone()));
                }
            } else {
                wide.push((low, span, label));
            }
        }
        if wide.is_empty() {
            self.emit_switch(promoted, &value, &default_dest, &cases);
        } else {
            let check = self.label();
            self.emit_switch(promoted, &value, &check, &cases);
            self.start(check);
            let ir = promoted.ir_type();
            for (low, span, label) in wide {
                let diff = self.fresh();
                self.emit(&format!(
                    "  {diff} = sub {ir} {value}, {}",
                    promoted.format_bits(low)
                ));
                let hit = self.fresh();
                self.emit(&format!(
                    "  {hit} = icmp ule {ir} {diff}, {}",
                    promoted.to_unsigned().format_bits(span)
                ));
                let next = self.label();
                self.br_cond(&hit, &label, &next);
                self.start(next);
            }
            self.br(&default_dest);
        }

        let saved_labels = std::mem::replace(&mut self.switch_labels, labels);
        let saved_at = self.switch_at;
        self.switch_at = 0;
        self.breaks.push(end_label.clone());

        let entry = self.label();
        self.start(entry);
        self.gen_stmt(body);
        assert_eq!(
            self.switch_at,
            self.switch_labels.len(),
            "internal compiler error: switch label was not emitted"
        );
        self.breaks.pop();
        if !self.terminated {
            self.br(&end_label);
        }
        self.switch_labels = saved_labels;
        self.switch_at = saved_at;
        self.start(end_label);
    }

    fn emit_switch(
        &mut self,
        ty: TypeName,
        value: &str,
        default_dest: &str,
        cases: &[(u128, String)],
    ) {
        let ir = ty.ir_type();
        self.emit(&format!("  switch {ir} {value}, label %{default_dest} ["));
        for (bits, label) in cases {
            self.emit(&format!(
                "    {ir} {}, label %{label}",
                ty.format_bits(*bits)
            ));
        }
        self.emit("  ]");
        self.terminated = true;
    }

    /// Case and default labels that belong to this switch, not to a nested one.
    /// The visit order matches [`Self::gen_stmt`].
    fn collect_switch_targets(
        &mut self,
        stmt: &Stmt,
        cases: &mut Vec<(u128, String)>,
        default_label: &mut Option<String>,
        labels: &mut Vec<String>,
    ) {
        match stmt {
            Stmt::Switch { .. } => {}
            Stmt::Case {
                value,
                high_value,
                stmt,
                ..
            } => {
                let label = self.label();
                let bits = value.expect("case value");
                match high_value {
                    None => cases.push((bits, label.clone())),
                    Some(high) => self.switch_ranges.push((bits, *high, label.clone())),
                }
                labels.push(label);
                self.collect_switch_targets(stmt, cases, default_label, labels);
            }
            Stmt::Default { stmt, .. } => {
                let label = self.label();
                *default_label = Some(label.clone());
                labels.push(label);
                self.collect_switch_targets(stmt, cases, default_label, labels);
            }
            Stmt::Compound(compound) => {
                for item in &compound.items {
                    self.collect_switch_targets(item, cases, default_label, labels);
                }
            }
            Stmt::If {
                then_stmt,
                else_stmt,
                ..
            } => {
                self.collect_switch_targets(then_stmt, cases, default_label, labels);
                if let Some(else_stmt) = else_stmt {
                    self.collect_switch_targets(else_stmt, cases, default_label, labels);
                }
            }
            Stmt::While { body, .. } | Stmt::Do { body, .. } => {
                self.collect_switch_targets(body, cases, default_label, labels);
            }
            Stmt::For { body, .. } => {
                self.collect_switch_targets(body, cases, default_label, labels);
            }
            Stmt::Label { stmt, .. } => {
                self.collect_switch_targets(stmt, cases, default_label, labels);
            }
            Stmt::Decl(_)
            | Stmt::FuncDecl(_)
            | Stmt::Return { .. }
            | Stmt::Expr { .. }
            | Stmt::Break { .. }
            | Stmt::Continue { .. }
            | Stmt::Goto { .. }
            | Stmt::GotoPtr { .. } => {}
        }
    }

    fn take_switch_label(&mut self) -> String {
        let label = self
            .switch_labels
            .get(self.switch_at)
            .expect("internal compiler error: case label was not collected")
            .clone();
        self.switch_at += 1;
        label
    }

    fn place_label(&mut self, label: &str) {
        if !self.terminated {
            self.br(label);
        }
        self.start(label.to_string());
    }

    fn gen_va_op(&mut self, op: VaOp, args: &[Expr]) {
        self.needs_va = true;
        match op {
            VaOp::Start | VaOp::End => {
                let ptr = self.gen_val(&args[0]);
                let intrinsic = if op == VaOp::Start {
                    "llvm.va_start"
                } else {
                    "llvm.va_end"
                };
                self.emit(&format!("  call void @{intrinsic}(ptr {ptr})"));
            }
            VaOp::Copy => {
                let dest = self.gen_val(&args[0]);
                let src = self.gen_val(&args[1]);
                self.emit(&format!("  call void @llvm.va_copy(ptr {dest}, ptr {src})"));
            }
        }
    }

    /// Address of the next variadic argument, advanced in `ap`.
    ///
    /// The layout is the System V AMD64 register save area: six general
    /// registers at offsets 0..48 and eight XMM registers at offsets 48..176.
    /// An argument that does not fit in the registers still free is taken
    /// from the overflow area, which stays 8-byte aligned. A two-register
    /// aggregate that does not fit leaves the leftover register unused.
    fn va_arg_addr(&mut self, ap: &str, ty: &CType) -> String {
        match va_class(ty) {
            VaClass::Gp {
                slots,
                stack_align,
                stack_size,
            } => self.va_reg_or_stack(
                ap,
                &RegFetch {
                    field: 0,
                    slots,
                    stride: 8,
                    limit: 48,
                    stack_align,
                    stack_size,
                },
            ),
            VaClass::Fp => self.va_reg_or_stack(
                ap,
                &RegFetch {
                    field: 4,
                    slots: 1,
                    stride: 16,
                    limit: 176,
                    stack_align: 8,
                    stack_size: 8,
                },
            ),
            VaClass::Mem { align, size } => self.va_overflow(ap, align, size),
            VaClass::Pieces(_) => unreachable!("split aggregates are copied per eightbyte"),
        }
    }

    fn va_reg_or_stack(&mut self, ap: &str, fetch: &RegFetch) -> String {
        let RegFetch {
            field,
            slots,
            stride,
            limit,
            stack_align,
            stack_size,
        } = *fetch;
        let off_ptr = self.offset_ptr(ap, field);
        let off_ty = CType::from_name(TypeName::UInt);
        let offset = self.load(&off_ty, &off_ptr);
        let need = u64::from(slots) * stride;
        let sum = self.fresh();
        self.emit(&format!("  {sum} = add i32 {offset}, {need}"));
        let fits = self.fresh();
        self.emit(&format!("  {fits} = icmp ule i32 {sum}, {limit}"));
        let reg_label = self.label();
        let stack_label = self.label();
        let done = self.label();
        self.br_cond(&fits, &reg_label, &stack_label);

        self.start(reg_label.clone());
        let save_slot = self.offset_ptr(ap, 16);
        let save = self.load(&CType::pointer(CType::Void), &save_slot);
        let reg_addr = self.fresh();
        self.emit(&format!(
            "  {reg_addr} = getelementptr inbounds i8, ptr {save}, i32 {offset}"
        ));
        self.store(&off_ty, &sum, &off_ptr);
        self.br(&done);

        self.start(stack_label.clone());
        let stack_addr = self.va_overflow(ap, stack_align, stack_size);
        self.br(&done);

        self.start(done);
        let addr = self.fresh();
        self.emit(&format!(
            "  {addr} = phi ptr [ {reg_addr}, %{reg_label} ], [ {stack_addr}, %{stack_label} ]"
        ));
        addr
    }

    fn va_overflow(&mut self, ap: &str, align: u64, size: u64) -> String {
        let off_ptr = self.offset_ptr(ap, 8);
        let current = self.load(&CType::pointer(CType::Void), &off_ptr);
        let aligned = self.align_ptr(&current, align.max(8));
        let next = self.fresh();
        self.emit(&format!(
            "  {next} = getelementptr inbounds i8, ptr {aligned}, i64 {size}"
        ));
        self.store(&CType::pointer(CType::Void), &next, &off_ptr);
        aligned
    }

    fn align_ptr(&mut self, ptr: &str, align: u64) -> String {
        if align <= 1 {
            return ptr.to_string();
        }
        let bits = self.fresh();
        self.emit(&format!("  {bits} = ptrtoint ptr {ptr} to i64"));
        let added = self.fresh();
        self.emit(&format!("  {added} = add i64 {bits}, {}", align - 1));
        let mask = (align as i64).wrapping_neg();
        let masked = self.fresh();
        self.emit(&format!("  {masked} = and i64 {added}, {mask}"));
        let out = self.fresh();
        self.emit(&format!("  {out} = inttoptr i64 {masked} to ptr"));
        out
    }

    /// Address of the next variadic argument of type `ty` on AArch64,
    /// with `ap` (the address of the `va_list` object) advanced past it.
    fn aarch64_va_arg_addr(&mut self, ap: &str, ty: &CType) -> String {
        let class = aarch64_va_class(ty);
        if crate::target::current().is_apple() {
            self.apple_va_arg_addr(ap, &class)
        } else {
            self.aapcs64_va_arg_addr(ap, ty, &class)
        }
    }

    /// Apple arm64: `va_list` is a `char *` into the stack, where every
    /// variadic argument has its own slot of a multiple of 8 bytes (16-byte
    /// aligned for a 16-byte-aligned type). A composite passed by reference
    /// has an 8-byte slot holding its address.
    fn apple_va_arg_addr(&mut self, ap: &str, class: &A64Va) -> String {
        let ptr_ty = CType::pointer(CType::Void);
        let current = self.load(&ptr_ty, ap);
        let aligned = if class.align16 {
            self.align_ptr(&current, 16)
        } else {
            current
        };
        let step = if class.by_ref { 8 } else { class.stack_size };
        let next = self.fresh();
        self.emit(&format!(
            "  {next} = getelementptr inbounds i8, ptr {aligned}, i64 {step}"
        ));
        self.store(&ptr_ty, &next, ap);
        if class.by_ref {
            return self.load(&ptr_ty, &aligned);
        }
        aligned
    }

    /// AAPCS64 (AArch64 Linux): `va_list` is `{ __stack, __gr_top,
    /// __vr_top, __gr_offs, __vr_offs }`, and `va_arg` follows the
    /// algorithm in the AAPCS64 appendix on variable argument lists. A
    /// negative offset indexes back from the top of the general (8-byte
    /// slots) or FP/SIMD (16-byte slots) register save area; once it is no
    /// longer negative the argument is on the stack.
    fn aapcs64_va_arg_addr(&mut self, ap: &str, ty: &CType, class: &A64Va) -> String {
        let i32_ty = CType::from_name(TypeName::Int);
        let ptr_ty = CType::pointer(CType::Void);
        let (offs_field, top_field, slot) = if class.fp_regs > 0 {
            (28u64, 16u64, 16u64)
        } else {
            (24, 8, 8)
        };
        let nreg = if class.fp_regs > 0 {
            class.fp_regs
        } else {
            class.gp_regs
        };
        let offs_ptr = self.offset_ptr(ap, offs_field);
        let offs = self.load(&i32_ty, &offs_ptr);
        let stack_label = self.label();
        let try_label = self.label();
        let reg_label = self.label();
        let done = self.label();
        let already = self.fresh();
        self.emit(&format!("  {already} = icmp sge i32 {offs}, 0"));
        self.br_cond(&already, &stack_label, &try_label);

        self.start(try_label);
        let offs = if class.fp_regs == 0 && class.align16 {
            let added = self.fresh();
            self.emit(&format!("  {added} = add i32 {offs}, 15"));
            let rounded = self.fresh();
            self.emit(&format!("  {rounded} = and i32 {added}, -16"));
            rounded
        } else {
            offs
        };
        let new_offs = self.fresh();
        self.emit(&format!(
            "  {new_offs} = add i32 {offs}, {}",
            u64::from(nreg) * slot
        ));
        self.store(&i32_ty, &new_offs, &offs_ptr);
        let overflow = self.fresh();
        self.emit(&format!("  {overflow} = icmp sgt i32 {new_offs}, 0"));
        self.br_cond(&overflow, &stack_label, &reg_label);

        self.start(reg_label.clone());
        let top_ptr = self.offset_ptr(ap, top_field);
        let top = self.load(&ptr_ty, &top_ptr);
        let offs64 = self.fresh();
        self.emit(&format!("  {offs64} = sext i32 {offs} to i64"));
        let reg_addr = self.fresh();
        self.emit(&format!(
            "  {reg_addr} = getelementptr inbounds i8, ptr {top}, i64 {offs64}"
        ));
        let reg_addr = if class.fp_regs > 1 || (class.fp_regs == 1 && ty.unqualified().is_record())
        {
            // HFA members sit in successive 16-byte register slots. Gather
            // them into a contiguous object.
            let tmp = self.entry_alloca(&ty.unqualified());
            let unit = class.fp_unit;
            let ir = match unit {
                4 => "float",
                8 => "double",
                _ => "fp128",
            };
            for index in 0..u64::from(class.fp_regs) {
                let from = self.offset_ptr(&reg_addr, index * 16);
                let value = self.fresh();
                self.emit(&format!("  {value} = load {ir}, ptr {from}, align {unit}"));
                let to = self.offset_ptr(&tmp, index * unit);
                self.emit(&format!("  store {ir} {value}, ptr {to}, align {unit}"));
            }
            tmp
        } else {
            reg_addr
        };
        let reg_end = self.cur.clone();
        self.br(&done);

        self.start(stack_label.clone());
        let stack_ptr = self.offset_ptr(ap, 0);
        let current = self.load(&ptr_ty, &stack_ptr);
        let aligned = if class.align16 {
            self.align_ptr(&current, 16)
        } else {
            current
        };
        let step = if class.by_ref { 8 } else { class.stack_size };
        let next = self.fresh();
        self.emit(&format!(
            "  {next} = getelementptr inbounds i8, ptr {aligned}, i64 {step}"
        ));
        self.store(&ptr_ty, &next, &stack_ptr);
        let stack_end = self.cur.clone();
        self.br(&done);

        self.start(done);
        let addr = self.fresh();
        self.emit(&format!(
            "  {addr} = phi ptr [ {reg_addr}, %{reg_end} ], [ {aligned}, %{stack_end} ]"
        ));
        if class.by_ref {
            return self.load(&ptr_ty, &addr);
        }
        addr
    }

    fn gen_va_arg(&mut self, expr: &Expr) -> String {
        let Expr::VaArg { ap, type_arg, .. } = expr else {
            unreachable!("va_arg");
        };
        let ap_ptr = self.gen_val(ap);
        if crate::target::current().is_aarch64() {
            let addr = self.aarch64_va_arg_addr(&ap_ptr, type_arg);
            let tmp = self.fresh();
            self.emit(&format!(
                "  {tmp} = load{} {}, ptr {addr}, align {}",
                vol_attr(type_arg),
                type_arg.ir_type(),
                type_arg.align().min(8)
            ));
            return tmp;
        }
        let addr = self.va_arg_addr(&ap_ptr, type_arg);
        // A 128-bit integer may come from the register save area, whose
        // GP slots are only 8-byte aligned. The address is a phi of that
        // slot and the overflow area, so the load claims 8, not 16.
        if let VaClass::Gp { slots: 2, .. } = va_class(type_arg) {
            if type_arg.align() > 8 {
                let tmp = self.fresh();
                self.emit(&format!(
                    "  {tmp} = load{} {}, ptr {addr}, align 8",
                    vol_attr(type_arg),
                    type_arg.ir_type(),
                ));
                return tmp;
            }
        }
        self.load(type_arg, &addr)
    }

    fn gen_va_record(&mut self, expr: &Expr) -> String {
        let Expr::VaArg { ap, type_arg, .. } = expr else {
            unreachable!("va_arg record");
        };
        let ap_ptr = self.gen_val(ap);
        if crate::target::current().is_aarch64() {
            let addr = self.aarch64_va_arg_addr(&ap_ptr, type_arg);
            let slot = self.entry_alloca(type_arg);
            self.copy_struct(type_arg, &addr, &slot, type_arg.is_volatile());
            return slot;
        }
        if let VaClass::Pieces(pieces) = va_class(type_arg) {
            return self.va_split(&ap_ptr, type_arg, &pieces);
        }
        let addr = self.va_arg_addr(&ap_ptr, type_arg);
        let slot = self.entry_alloca(type_arg);
        self.copy_struct(type_arg, &addr, &slot, type_arg.is_volatile());
        slot
    }

    /// `va_arg` of an aggregate whose eightbytes are not one contiguous
    /// image in the register save area. A `double` pair is two XMM slots
    /// sixteen bytes apart, and a mixed INTEGER/SSE aggregate uses both
    /// register classes. Either class being full sends the whole argument
    /// to the overflow area.
    fn va_split(&mut self, ap: &str, ty: &CType, pieces: &[crate::abi::RegPiece]) -> String {
        let gp_n = pieces.iter().filter(|piece| !piece.is_sse()).count() as u64;
        let fp_n = pieces.len() as u64 - gp_n;
        let gp_ptr = self.offset_ptr(ap, 0);
        let fp_ptr = self.offset_ptr(ap, 4);
        let off_ty = CType::from_name(TypeName::UInt);
        let gp = self.load(&off_ty, &gp_ptr);
        let fp = self.load(&off_ty, &fp_ptr);
        let fits = self.fresh();
        self.emit(&format!(
            "  {fits} = icmp ule i32 {gp}, {}",
            48u64.saturating_sub(gp_n * 8)
        ));
        let fp_fits = self.fresh();
        self.emit(&format!(
            "  {fp_fits} = icmp ule i32 {fp}, {}",
            176u64.saturating_sub(fp_n * 16)
        ));
        let both = self.fresh();
        self.emit(&format!("  {both} = and i1 {fits}, {fp_fits}"));
        let reg_label = self.label();
        let stack_label = self.label();
        let done = self.label();
        self.br_cond(&both, &reg_label, &stack_label);

        let slot = self.entry_alloca(ty);
        self.start(reg_label.clone());
        let save_slot = self.offset_ptr(ap, 16);
        let save = self.load(&CType::pointer(CType::Void), &save_slot);
        let mut gp_at = gp;
        let mut fp_at = fp;
        for piece in pieces {
            let cursor = if piece.is_sse() {
                let at = fp_at.clone();
                let next = self.fresh();
                self.emit(&format!("  {next} = add i32 {at}, 16"));
                fp_at = next;
                at
            } else {
                let at = gp_at.clone();
                let next = self.fresh();
                self.emit(&format!("  {next} = add i32 {at}, 8"));
                gp_at = next;
                at
            };
            let src = self.fresh();
            self.emit(&format!(
                "  {src} = getelementptr inbounds i8, ptr {save}, i32 {cursor}"
            ));
            let dest = self.offset_ptr(&slot, piece.offset());
            let bytes = piece_bytes(piece);
            self.needs_memmove = true;
            self.emit(&format!(
                "  call void @llvm.memmove.p0.p0.i64(ptr {dest}, ptr {src}, i64 {bytes}, i1 false)"
            ));
        }
        self.store(&off_ty, &gp_at, &gp_ptr);
        self.store(&off_ty, &fp_at, &fp_ptr);
        self.br(&done);

        self.start(stack_label.clone());
        let align = u64::from(ty.align()).max(8);
        let size = (ty.size().unwrap_or(8) + 7) & !7;
        let stack_addr = self.va_overflow(ap, align, size.max(8));
        self.copy_struct(ty, &stack_addr, &slot, ty.is_volatile());
        self.br(&done);

        self.start(done);
        slot
    }

    fn gen_val(&mut self, expr: &Expr) -> String {
        self.gen_expr(expr)
            .expect("internal compiler error: void expression used as a value")
    }

    fn gen_expr(&mut self, expr: &Expr) -> Option<String> {
        // A struct or union value is the address of its object representation.
        // Copies use `llvm.memmove`, so padding bytes are kept.
        if expr.ty().is_some_and(|ty| ty.is_record()) {
            return Some(self.record_ptr(expr));
        }
        match expr {
            Expr::Int { bits, value_ty, .. } => Some(value_ty.format_bits(u128::from(*bits))),
            Expr::Float { bits, kind, .. } => Some(crate::float::llvm_const(*kind, *bits)),
            Expr::LabelAddr { name, func, .. } => {
                let func = func.as_deref().expect("label address in a function");
                Some(format!(
                    "blockaddress({}, %{})",
                    llvm_global(renamed(func, &self.renames)),
                    c_label(name)
                ))
            }
            Expr::Var { resolved, ty, .. } => {
                let slot = &self.var_slot(&resolved.as_ref().expect("resolved variable").slot);
                let ty = ty.clone().expect("typed variable");
                if ty.is_array() || ty.is_func() {
                    Some(slot.clone())
                } else {
                    Some(self.load(&ty, slot))
                }
            }
            Expr::Unary {
                op: UnaryOp::Not, ..
            } => Some(self.zext_cond(expr)),
            Expr::Unary {
                op: UnaryOp::Plus,
                expr,
                ..
            } => {
                let value = self.gen_val(expr);
                let src = expr.ty().expect("typed");
                if src.is_float() {
                    Some(value)
                } else {
                    let promoted = CType::from_name(src.as_integer().expect("integer").promote());
                    Some(self.convert(&value, &src, &promoted))
                }
            }
            Expr::Unary {
                op: UnaryOp::Minus,
                expr,
                ..
            } => Some(self.gen_neg_or_not(expr, "sub")),
            Expr::Unary {
                op: UnaryOp::BitNot,
                expr,
                ..
            } => Some(self.gen_neg_or_not(expr, "xor")),
            Expr::Unary {
                op: UnaryOp::Addr,
                expr,
                ..
            } => Some(self.gen_addr(expr)),
            Expr::Unary {
                op: UnaryOp::Deref,
                expr,
                ty,
                ..
            } => {
                let addr = self.gen_val(expr);
                let ty = ty.clone().expect("typed");
                if ty.is_void() {
                    None
                } else if ty.is_array() {
                    Some(addr)
                } else {
                    Some(self.load(&ty, &addr))
                }
            }
            Expr::Bin { op, .. } if op.is_cmp() || matches!(op, BinOp::And | BinOp::Or) => {
                Some(self.zext_cond(expr))
            }
            Expr::Bin {
                op: BinOp::Sub,
                left,
                right,
                ..
            } if left.ty().is_some_and(|ty| ty.is_pointer())
                && right.ty().is_some_and(|ty| ty.is_pointer()) =>
            {
                Some(self.gen_ptr_diff(left, right))
            }
            Expr::Bin {
                op,
                left,
                right,
                ty,
                ..
            } if ty.as_ref().is_some_and(|ty| ty.is_pointer()) => {
                Some(self.gen_ptr_arith(*op, left, right))
            }
            Expr::Bin {
                op, left, right, ..
            } => Some(self.gen_arith(*op, left, right)),
            Expr::Assign { target, value, .. } => {
                let addr = self.gen_addr(target);
                let dest = target.ty().expect("lvalue type");
                let raw = self.gen_val(value);
                let src_ty = value.ty().expect("typed");
                if bit_info(target).is_some() {
                    Some(self.store_bitfield(&addr, target, &raw, &src_ty))
                } else {
                    let stored = self.convert(&raw, &src_ty, &dest);
                    self.store(&dest, &stored, &addr);
                    Some(stored)
                }
            }
            Expr::CompoundAssign {
                op, target, value, ..
            } => Some(self.gen_compound_assign(*op, target, value)),
            Expr::IncDec { op, target, .. } => Some(self.gen_incdec(*op, target)),
            Expr::Cond {
                cond,
                then_expr,
                else_expr,
                ty,
                ..
            } => self.gen_ternary(cond, then_expr, else_expr, ty.clone().expect("typed")),
            Expr::Comma { left, right, .. } => {
                let _ = self.gen_expr(left);
                self.gen_expr(right)
            }
            Expr::Sizeof { operand, .. } => {
                let size = match operand {
                    SizeofOperand::Type(ty) => ty.size(),
                    SizeofOperand::Expr(inner) => inner.ty().expect("typed").size(),
                }
                .expect("sizeof of a complete type");
                Some(TypeName::ULong.format_bits(u128::from(size)))
            }
            Expr::Cast { ty_name, expr, .. } => {
                if ty_name.is_void() {
                    let _ = self.gen_expr(expr);
                    None
                } else {
                    let value = self.gen_val(expr);
                    Some(self.convert(&value, &expr.ty().expect("typed"), ty_name))
                }
            }
            Expr::Index {
                base, index, ty, ..
            } => {
                let addr = self.gen_ptr_index(base, index);
                let ty = ty.clone().expect("typed");
                if ty.is_array() {
                    Some(addr)
                } else {
                    Some(self.load(&ty, &addr))
                }
            }
            Expr::Decay { expr, .. } => Some(self.gen_addr(expr)),
            Expr::Str { .. } => Some(self.gen_addr(expr)),
            Expr::Member { ty, bit_width, .. } => {
                if bit_width.is_some() {
                    Some(self.load_bitfield(expr))
                } else {
                    let addr = self.gen_addr(expr);
                    let ty = ty.clone().expect("typed");
                    if ty.is_array() {
                        Some(addr)
                    } else {
                        Some(self.load(&ty, &addr))
                    }
                }
            }
            Expr::Call {
                callee,
                args,
                resolved,
                ..
            } => self.gen_call(callee, args, resolved.as_ref().expect("resolved call")),
            Expr::VaArg { .. } => Some(self.gen_va_arg(expr)),
            Expr::VaOp { op, args, .. } => {
                self.gen_va_op(*op, args);
                None
            }
            Expr::CompoundLit { decl, .. } => {
                let slot = self.gen_compound_lit(decl);
                if decl.ty.is_array() {
                    Some(slot)
                } else {
                    Some(self.load(&decl.ty, &slot))
                }
            }
            Expr::Generic { .. } => {
                unreachable!("semantic analysis replaces a generic selection")
            }
            Expr::StmtExpr { body, ty, .. } => {
                let last = self.gen_stmt_expr_body(body);
                let value = last.and_then(|last| self.gen_expr(last));
                self.ensure_block();
                if ty.as_ref().is_some_and(CType::is_void) {
                    None
                } else {
                    value
                }
            }
            Expr::CondShort {
                cond,
                else_expr,
                ty,
                ..
            } => self.gen_cond_short(cond, else_expr, ty.clone().expect("typed")),
        }
    }

    /// Emit a statement expression's block up to its last expression
    /// statement, which is returned for the caller to evaluate. Its
    /// automatic objects get entry-block slots.
    fn gen_stmt_expr_body<'b>(&mut self, body: &'b Compound) -> Option<&'b Expr> {
        for (slot, ty, align) in local_slots(body) {
            let line = format!("  {slot} = alloca {}, align {align}", ty.ir_type());
            self.lines.insert(self.alloca_at, line);
            self.alloca_at += 1;
        }
        let (last, rest) = match body.items.split_last() {
            Some((
                Stmt::Expr {
                    expr: Some(last), ..
                },
                rest,
            )) => (Some(last), rest),
            _ => (None, &body.items[..]),
        };
        for item in rest {
            self.gen_stmt(item);
        }
        self.ensure_block();
        last
    }

    /// `x ?: y`: `x` is evaluated once; when it is nonzero it is the
    /// result, otherwise `y` is evaluated.
    fn gen_cond_short(&mut self, cond: &Expr, else_expr: &Expr, result: CType) -> Option<String> {
        let cond_ty = cond.ty().expect("typed");
        let value = self.gen_val(cond);
        let test = self.test_nonzero(&value, &cond_ty);
        let then_label = self.label();
        let else_label = self.label();
        let end_label = self.label();
        self.br_cond(&test, &then_label, &else_label);
        self.start(then_label);
        let then_val = if result.is_void() {
            None
        } else {
            Some(self.convert(&value, &cond_ty, &result))
        };
        let then_block = self.cur.clone();
        self.br(&end_label);
        self.start(else_label);
        let else_val = self
            .gen_expr(else_expr)
            .map(|value| self.convert(&value, &else_expr.ty().expect("typed"), &result));
        self.ensure_block();
        let else_block = self.cur.clone();
        self.br(&end_label);
        self.start(end_label);
        match (then_val, else_val) {
            (Some(then_val), Some(else_val)) => {
                let tmp = self.fresh();
                self.emit(&format!(
                    "  {tmp} = phi {} [ {then_val}, %{then_block} ], [ {else_val}, %{else_block} ]",
                    result.ir_type()
                ));
                Some(tmp)
            }
            _ => None,
        }
    }

    /// `value != 0` as an `i1`, for a scalar of type `ty`.
    fn test_nonzero(&mut self, value: &str, ty: &CType) -> String {
        let tmp = self.fresh();
        if ty.is_pointer() {
            self.emit(&format!("  {tmp} = icmp ne ptr {value}, null"));
        } else if ty.is_float() {
            let zero = crate::float::llvm_const(ty.float_kind().expect("float"), 0);
            self.emit(&format!(
                "  {tmp} = fcmp une {} {value}, {zero}",
                ty.ir_type()
            ));
        } else {
            self.emit(&format!("  {tmp} = icmp ne {} {value}, 0", ty.ir_type()));
        }
        tmp
    }

    fn gen_addr(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Var { resolved, .. } => {
                self.var_slot(&resolved.as_ref().expect("resolved variable").slot)
            }
            Expr::Unary {
                op: UnaryOp::Deref,
                expr,
                ..
            } => self.gen_val(expr),
            Expr::Index { base, index, .. } => self.gen_ptr_index(base, index),
            Expr::Decay { expr, .. } => self.gen_addr(expr),
            Expr::Str { symbol, .. } => llvm_global(symbol.as_deref().expect("string symbol")),
            Expr::Member {
                base,
                arrow,
                index,
                offset,
                bytewise,
                ..
            } => {
                let base_ptr = if *arrow {
                    self.gen_val(base)
                } else if base.is_lvalue() {
                    self.gen_addr(base)
                } else if base.ty().is_some_and(|ty| ty.is_record()) {
                    self.record_ptr(base)
                } else {
                    self.spill(base)
                };
                let tmp = self.fresh();
                let record_ty = if *arrow {
                    base.ty().as_ref().and_then(CType::pointee).cloned()
                } else {
                    base.ty()
                };
                let field_align = expr.ty().map_or(1, |ty| ty.member_align());
                let record_align = record_ty.as_ref().map_or(1, CType::align);
                let field_offset = offset.unwrap_or(0);
                if self.unaligned.contains(&base_ptr)
                    || record_align < field_align
                    || field_offset % u64::from(field_align.max(1)) != 0
                {
                    self.unaligned.insert(tmp.clone());
                }
                if *bytewise {
                    let offset = offset.expect("member offset");
                    self.emit(&format!(
                        "  {tmp} = getelementptr inbounds i8, ptr {base_ptr}, i64 {offset}"
                    ));
                } else {
                    let struct_ty = if *arrow {
                        base.ty()
                            .as_ref()
                            .and_then(CType::pointee)
                            .expect("pointer to struct")
                            .clone()
                    } else {
                        base.ty().expect("struct")
                    };
                    let index = index.expect("member index");
                    self.emit(&format!(
                        "  {tmp} = getelementptr inbounds {}, ptr {base_ptr}, i32 0, i32 {index}",
                        struct_ty.ir_type()
                    ));
                }
                tmp
            }
            Expr::CompoundLit { decl, .. } => self.gen_compound_lit(decl),
            _ => panic!("internal compiler error: address of a non-lvalue"),
        }
    }

    /// The object of a block-scope compound literal (C11 6.5.2.5p5). It is
    /// an entry-block alloca, initialized here each time the literal is
    /// evaluated.
    fn gen_compound_lit(&mut self, decl: &VarDecl) -> String {
        let slot = self.entry_alloca(&decl.ty);
        let mut object = decl.clone();
        object.slot = slot.clone();
        self.gen_decl(&object);
        slot
    }

    /// Materialize a struct rvalue so a member can be addressed.
    fn spill(&mut self, expr: &Expr) -> String {
        let ty = expr.ty().expect("typed");
        let value = self.gen_val(expr);
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = alloca {}, align {}",
            ty.ir_type(),
            ty.align()
        ));
        self.store(&ty, &value, &tmp);
        tmp
    }

    fn gen_ptr_index(&mut self, base: &Expr, index: &Expr) -> String {
        let (ptr_expr, int_expr) = if base.ty().is_some_and(|ty| ty.is_pointer()) {
            (base, index)
        } else {
            (index, base)
        };
        self.gen_gep(ptr_expr, int_expr, false)
    }

    fn gen_ptr_arith(&mut self, op: BinOp, left: &Expr, right: &Expr) -> String {
        let (ptr_expr, int_expr, negate) = if left.ty().is_some_and(|ty| ty.is_pointer()) {
            (left, right, op == BinOp::Sub)
        } else {
            (right, left, false)
        };
        self.gen_gep(ptr_expr, int_expr, negate)
    }

    fn gen_gep(&mut self, ptr_expr: &Expr, int_expr: &Expr, negate: bool) -> String {
        let ptr = self.gen_val(ptr_expr);
        let idx = self.gen_val(int_expr);
        let mut idx = self.extend_i64(&idx, &int_expr.ty().expect("integer index"));
        if negate {
            let tmp = self.fresh();
            self.emit(&format!("  {tmp} = sub i64 0, {idx}"));
            idx = tmp;
        }
        let elem = ptr_expr
            .ty()
            .as_ref()
            .and_then(CType::pointee)
            .expect("pointer")
            .ir_type();
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = getelementptr inbounds {elem}, ptr {ptr}, i64 {idx}"
        ));
        if self.unaligned.contains(&ptr) {
            self.unaligned.insert(tmp.clone());
        }
        tmp
    }

    fn gen_ptr_diff(&mut self, left: &Expr, right: &Expr) -> String {
        let left_val = self.gen_val(left);
        let right_val = self.gen_val(right);
        let left_int = self.fresh();
        let right_int = self.fresh();
        self.emit(&format!("  {left_int} = ptrtoint ptr {left_val} to i64"));
        self.emit(&format!("  {right_int} = ptrtoint ptr {right_val} to i64"));
        let diff = self.fresh();
        self.emit(&format!("  {diff} = sub i64 {left_int}, {right_int}"));
        let size = left
            .ty()
            .as_ref()
            .and_then(CType::pointee)
            .and_then(CType::size)
            .expect("complete pointee");
        if size == 1 {
            return diff;
        }
        let tmp = self.fresh();
        self.emit(&format!("  {tmp} = sdiv exact i64 {diff}, {size}"));
        tmp
    }

    fn gen_neg_or_not(&mut self, expr: &Expr, kind: &str) -> String {
        let src = expr.ty().expect("typed");
        if src.is_float() && kind == "sub" {
            let value = self.gen_val(expr);
            let tmp = self.fresh();
            self.emit(&format!("  {tmp} = fneg {} {value}", src.ir_type()));
            return tmp;
        }
        let dest = CType::from_name(src.as_integer().expect("integer").promote());
        let value = self.gen_val(expr);
        let value = self.convert(&value, &src, &dest);
        let tmp = self.fresh();
        if kind == "sub" {
            self.emit(&format!("  {tmp} = sub {} 0, {value}", dest.ir_type()));
        } else {
            self.emit(&format!("  {tmp} = xor {} {value}, -1", dest.ir_type()));
        }
        tmp
    }

    fn gen_arith(&mut self, op: BinOp, left: &Expr, right: &Expr) -> String {
        let left_ty = left.ty().expect("typed");
        let right_ty = right.ty().expect("typed");
        if left_ty.is_float() || right_ty.is_float() {
            return self.gen_float_arith(op, left, right);
        }
        let left_i = left_ty.as_integer().expect("integer");
        let right_i = right_ty.as_integer().expect("integer");
        let common = if matches!(op, BinOp::Shl | BinOp::Shr) {
            left_i.promote()
        } else {
            left_i.usual(right_i)
        };
        let common_ty = CType::from_name(common);
        let left_val = self.gen_val(left);
        let left_val = self.convert(&left_val, &left_ty, &common_ty);
        let right_val = self.gen_val(right);
        let right_val = self.convert(&right_val, &right_ty, &common_ty);
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = {} {} {left_val}, {right_val}",
            value_inst(op, common.is_signed()),
            common.ir_type()
        ));
        tmp
    }

    fn gen_float_arith(&mut self, op: BinOp, left: &Expr, right: &Expr) -> String {
        let left_ty = left.ty().expect("typed");
        let right_ty = right.ty().expect("typed");
        let common = common_real(&left_ty, &right_ty);
        let left_val = self.gen_val(left);
        let left_val = self.convert(&left_val, &left_ty, &common);
        let right_val = self.gen_val(right);
        let right_val = self.convert(&right_val, &right_ty, &common);
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = {} {} {left_val}, {right_val}",
            float_inst(op),
            common.ir_type()
        ));
        tmp
    }

    fn gen_compound_assign(&mut self, op: BinOp, target: &Expr, value: &Expr) -> String {
        let dest = target.ty().expect("lvalue type");
        let rhs_ty = value.ty().expect("typed");
        // The right-hand side is evaluated before the lvalue is loaded. The
        // address is still computed only once.
        let rhs = self.gen_val(value);
        let addr = self.gen_addr(target);
        let old = if bit_info(target).is_some() {
            self.load_bitfield_at(&addr, target)
        } else {
            self.load(&dest, &addr)
        };
        if dest.is_pointer() {
            let mut idx = self.extend_i64(&rhs, &rhs_ty);
            if op == BinOp::Sub {
                let tmp = self.fresh();
                self.emit(&format!("  {tmp} = sub i64 0, {idx}"));
                idx = tmp;
            }
            let elem = dest.pointee().expect("pointer").ir_type();
            let updated = self.fresh();
            self.emit(&format!(
                "  {updated} = getelementptr inbounds {elem}, ptr {old}, i64 {idx}"
            ));
            self.store(&dest, &updated, &addr);
            return updated;
        }
        if dest.is_float() || rhs_ty.is_float() {
            let common = common_real(&dest, &rhs_ty);
            let old_c = self.convert(&old, &dest, &common);
            let rhs_c = self.convert(&rhs, &rhs_ty, &common);
            let updated = self.fresh();
            self.emit(&format!(
                "  {updated} = {} {} {old_c}, {rhs_c}",
                float_inst(op),
                common.ir_type()
            ));
            let stored = self.convert(&updated, &common, &dest);
            self.store(&dest, &stored, &addr);
            return stored;
        }
        let dest_i = dest.as_integer().expect("integer");
        let rhs_i = rhs_ty.as_integer().expect("integer");
        let common = if matches!(op, BinOp::Shl | BinOp::Shr) {
            dest_i.promote()
        } else {
            dest_i.usual(rhs_i)
        };
        let common_ty = CType::from_name(common);
        let old_c = self.convert(&old, &dest, &common_ty);
        let rhs_c = self.convert(&rhs, &rhs_ty, &common_ty);
        let updated = self.fresh();
        self.emit(&format!(
            "  {updated} = {} {} {old_c}, {rhs_c}",
            value_inst(op, common.is_signed()),
            common.ir_type()
        ));
        let stored = self.convert(&updated, &common_ty, &dest);
        if bit_info(target).is_some() {
            self.store_bitfield(&addr, target, &stored, &dest)
        } else {
            self.store(&dest, &stored, &addr);
            stored
        }
    }

    fn gen_incdec(&mut self, op: IncOp, target: &Expr) -> String {
        let dest = target.ty().expect("lvalue type");
        let addr = self.gen_addr(target);
        let old = if bit_info(target).is_some() {
            self.load_bitfield_at(&addr, target)
        } else {
            self.load(&dest, &addr)
        };
        if dest.is_float() {
            let kind = dest.float_kind().expect("float");
            let one_bits: u128 = match crate::float::fold(kind) {
                FloatKind::Float => 0x3f80_0000,
                FloatKind::Double => 0x3ff0_0000_0000_0000,
                FloatKind::LongDouble => 0x3FFF_8000_0000_0000_0000,
            };
            let one = crate::float::llvm_const(kind, one_bits);
            let updated = self.fresh();
            let inst = if op.is_increment() { "fadd" } else { "fsub" };
            self.emit(&format!(
                "  {updated} = {inst} {} {old}, {one}",
                dest.ir_type()
            ));
            let written = if bit_info(target).is_some() {
                self.store_bitfield(&addr, target, &updated, &dest)
            } else {
                self.store(&dest, &updated, &addr);
                updated
            };
            return if op.is_prefix() { written } else { old };
        }
        if dest.is_pointer() {
            let elem = dest.pointee().expect("pointer").ir_type();
            let step = if op.is_increment() { "1" } else { "-1" };
            let updated = self.fresh();
            self.emit(&format!(
                "  {updated} = getelementptr inbounds {elem}, ptr {old}, i64 {step}"
            ));
            self.store(&dest, &updated, &addr);
            return if op.is_prefix() { updated } else { old };
        }
        let dest_i = dest.as_integer().expect("integer");
        let common = dest_i.usual(TypeName::Int);
        let common_ty = CType::from_name(common);
        let old_c = self.convert(&old, &dest, &common_ty);
        let updated = self.fresh();
        let inst = if op.is_increment() { "add" } else { "sub" };
        self.emit(&format!(
            "  {updated} = {inst} {} {old_c}, 1",
            common.ir_type()
        ));
        let stored = self.convert(&updated, &common_ty, &dest);
        let written = if bit_info(target).is_some() {
            self.store_bitfield(&addr, target, &stored, &dest)
        } else {
            self.store(&dest, &stored, &addr);
            stored
        };
        if op.is_prefix() {
            written
        } else {
            old
        }
    }

    /// Sign bit of a floating value, as an `int`. `long double`'s sign is
    /// bit 79, the high bit of the exponent word stored at offset 8.
    fn gen_signbit(&mut self, name: &str, args: &[Expr]) -> Option<String> {
        let kind = match name {
            "__builtin_signbitf" => FloatKind::Float,
            "__builtin_signbit" => FloatKind::Double,
            "__builtin_signbitl" => FloatKind::LongDouble,
            _ => return None,
        };
        let arg = &args[0];
        let raw = self.gen_val(arg);
        let src = arg.ty().expect("typed");
        let dest = CType::Float(kind);
        let value = self.convert(&raw, &src, &dest);
        let neg = match kind {
            FloatKind::Float => {
                let bits = self.fresh();
                self.emit(&format!("  {bits} = bitcast float {value} to i32"));
                let cmp = self.fresh();
                self.emit(&format!("  {cmp} = icmp slt i32 {bits}, 0"));
                cmp
            }
            FloatKind::Double => {
                let bits = self.fresh();
                self.emit(&format!("  {bits} = bitcast double {value} to i64"));
                let cmp = self.fresh();
                self.emit(&format!("  {cmp} = icmp slt i64 {bits}, 0"));
                cmp
            }
            FloatKind::LongDouble if !kind.is_x87() => {
                // The sign is the top bit of the `double` or binary128
                // representation.
                let ir = dest.ir_type();
                let width = dest.size().unwrap_or(8) * 8;
                let bits = self.fresh();
                self.emit(&format!("  {bits} = bitcast {ir} {value} to i{width}"));
                let cmp = self.fresh();
                self.emit(&format!("  {cmp} = icmp slt i{width} {bits}, 0"));
                cmp
            }
            FloatKind::LongDouble => {
                let slot = self.entry_alloca(&CType::Float(FloatKind::LongDouble));
                self.emit(&format!("  store x86_fp80 {value}, ptr {slot}, align 16"));
                let hi = self.fresh();
                self.emit(&format!(
                    "  {hi} = getelementptr inbounds i8, ptr {slot}, i64 8"
                ));
                let word = self.fresh();
                self.emit(&format!("  {word} = load i16, ptr {hi}, align 2"));
                let cmp = self.fresh();
                self.emit(&format!("  {cmp} = icmp slt i16 {word}, 0"));
                cmp
            }
        };
        let out = self.fresh();
        self.emit(&format!("  {out} = zext i1 {neg} to i32"));
        Some(out)
    }

    /// `__builtin_memcpy`, `__builtin_memmove`, and `__builtin_memset`.
    ///
    /// The copy is `llvm.memcpy` or `llvm.memmove`. The result is the
    /// destination pointer, as the C library functions return `s1`.
    fn gen_memory_builtin(&mut self, name: &str, args: &[Expr]) -> Option<String> {
        let which = match name {
            "__builtin_memcpy" => "llvm.memcpy.p0.p0.i64",
            "__builtin_memmove" => "llvm.memmove.p0.p0.i64",
            "__builtin_memset" => "llvm.memset.p0.i64",
            _ => return None,
        };
        let dest = self.gen_val(&args[0]);
        let len_ty = CType::from_name(TypeName::ULong);
        let len_src = args[2].ty().expect("typed");
        let len_val = self.gen_val(&args[2]);
        let len = self.convert(&len_val, &len_src, &len_ty);
        match name {
            "__builtin_memset" => {
                self.needs_memset = true;
                let src_ty = args[1].ty().expect("typed");
                let raw = self.gen_val(&args[1]);
                let as_int = self.convert(&raw, &src_ty, &CType::from_name(TypeName::Int));
                let byte = self.fresh();
                self.emit(&format!("  {byte} = trunc i32 {as_int} to i8"));
                self.emit(&format!(
                    "  call void @{which}(ptr align 1 {dest}, i8 {byte}, i64 {len}, i1 false)"
                ));
            }
            "__builtin_memcpy" => {
                self.needs_memcpy = true;
                let src = self.gen_val(&args[1]);
                self.emit(&format!(
                    "  call void @{which}(ptr align 1 {dest}, ptr align 1 {src}, i64 {len}, i1 false)"
                ));
            }
            _ => {
                self.needs_memmove = true;
                let src = self.gen_val(&args[1]);
                self.emit(&format!(
                    "  call void @{which}(ptr align 1 {dest}, ptr align 1 {src}, i64 {len}, i1 false)"
                ));
            }
        }
        Some(dest)
    }

    /// `__builtin_nan*` / `__builtin_nans*` / `__builtin_inf*` /
    /// `__builtin_huge_val*` fold to an LLVM floating constant when the
    /// tag, if any, is a compile-time constant. `float` and `double` use
    /// the 16-digit `0x` form; `long double` uses `0xK` and 20 hex digits
    /// (LLVM Language Reference, "Simple Constants").
    ///
    /// A quiet `__builtin_nan*` whose tag is not consumed in full, in a
    /// runtime context, is lowered to a call of the C library `nan` /
    /// `nanf` / `nanl` (C11 7.12.11.2) with the same string literal. The
    /// whole-tag rule is [`crate::float::nan_tag_from_expr`], shared with
    /// constant evaluation. A malformed `__builtin_nans*` tag is diagnosed
    /// in semantic analysis.
    fn gen_fp_format_builtin(&mut self, name: &str, args: &[Expr]) -> Option<String> {
        let fp = crate::float::FpFormatBuiltin::from_name(name)?;
        let bits = match fp {
            crate::float::FpFormatBuiltin::Inf(kind) => crate::float::plus_inf(kind),
            crate::float::FpFormatBuiltin::Nan { kind, signaling } => {
                match crate::float::nan_tag_from_expr(&args[0]) {
                    Some(tag) => crate::float::nan_bits(kind, tag, signaling),
                    None => {
                        let lib = fp
                            .libm_nan()
                            .expect("malformed __builtin_nans* tag should be diagnosed");
                        return Some(self.gen_lib_nan(lib, fp.kind(), &args[0]));
                    }
                }
            }
        };
        Some(crate::float::llvm_const(fp.kind(), bits))
    }

    /// A C11 7.12.14 comparison builtin. Both operands are converted to
    /// their common real type and compared with one quiet `fcmp`; the
    /// `i1` result widens to an `int` 0 or 1.
    fn gen_fp_compare(&mut self, name: &str, args: &[Expr]) -> Option<String> {
        let pred = crate::float::fp_compare_predicate(name)?;
        let left_ty = args[0].ty().expect("typed");
        let right_ty = args[1].ty().expect("typed");
        let common = common_real(&left_ty, &right_ty);
        let left = self.gen_val(&args[0]);
        let left = self.convert(&left, &left_ty, &common);
        let right = self.gen_val(&args[1]);
        let right = self.convert(&right, &right_ty, &common);
        let cmp = self.fresh();
        self.emit(&format!(
            "  {cmp} = fcmp {pred} {} {left}, {right}",
            common.ir_type()
        ));
        let out = self.fresh();
        self.emit(&format!("  {out} = zext i1 {cmp} to i32"));
        Some(out)
    }

    fn gen_lib_nan(&mut self, name: &str, kind: FloatKind, tag: &Expr) -> String {
        match name {
            "nan" => self.needs_nan = true,
            "nanf" => self.needs_nanf = true,
            "nanl" => self.needs_nanl = true,
            _ => unreachable!("library nan name"),
        }
        let arg = self.gen_val(tag);
        let tmp = self.fresh();
        let ret = crate::ast::CType::Float(kind).ir_type();
        self.emit(&format!("  {tmp} = call {ret} @{name}(ptr {arg})"));
        tmp
    }

    fn gen_call(&mut self, callee: &Expr, args: &[Expr], resolved: &CallRes) -> Option<String> {
        if let Some(lowered) = self.gen_memory_builtin(&resolved.name, args) {
            return Some(lowered);
        }
        if let Some(lowered) = self.gen_signbit(&resolved.name, args) {
            return Some(lowered);
        }
        if let Some(lowered) = self.gen_fp_format_builtin(&resolved.name, args) {
            return Some(lowered);
        }
        if let Some(lowered) = self.gen_fp_compare(&resolved.name, args) {
            return Some(lowered);
        }
        if let Some(inline) = crate::builtins::Inline::from_name(&resolved.name) {
            return self.gen_inline_builtin(inline, args);
        }
        let sig =
            function_sig(&resolved.ret, &resolved.params).expect("call has a System V signature");
        let rendered = self.render_call_args(args, resolved, &sig, None);
        self.emit_call(callee, resolved, &sig, &rendered)
    }

    /// Note an intrinsic declaration for the module prelude.
    fn intrinsic(&mut self, declare: &str) {
        self.intrinsics.insert(declare.to_string());
    }

    /// The argument converted to the builtin's parameter type.
    fn builtin_arg(
        &mut self,
        inline: crate::builtins::Inline,
        args: &[Expr],
        index: usize,
    ) -> String {
        let value = self.gen_val(&args[index]);
        let src = args[index].ty().expect("typed");
        match inline.param_type(index) {
            Some(param) => self.convert(&value, &src, &param),
            None => value,
        }
    }

    /// Lower a builtin from [`crate::builtins::Inline`] in place.
    fn gen_inline_builtin(
        &mut self,
        inline: crate::builtins::Inline,
        args: &[Expr],
    ) -> Option<String> {
        use crate::builtins::Inline;
        let int_result = |gen: &mut Self, value: &str, from: TypeName| -> String {
            gen.convert(value, &CType::Int(from), &CType::int_type())
        };
        match inline {
            Inline::Clz(ty) | Inline::Ctz(ty) | Inline::Popcount(ty) | Inline::Parity(ty) => {
                let value = self.builtin_arg(inline, args, 0);
                let ir = ty.ir_type();
                let tmp = self.fresh();
                match inline {
                    Inline::Clz(_) | Inline::Ctz(_) => {
                        let which = if matches!(inline, Inline::Clz(_)) {
                            "ctlz"
                        } else {
                            "cttz"
                        };
                        self.intrinsic(&format!("declare {ir} @llvm.{which}.{ir}({ir}, i1)"));
                        self.emit(&format!(
                            "  {tmp} = call {ir} @llvm.{which}.{ir}({ir} {value}, i1 false)"
                        ));
                    }
                    _ => {
                        self.intrinsic(&format!("declare {ir} @llvm.ctpop.{ir}({ir})"));
                        self.emit(&format!(
                            "  {tmp} = call {ir} @llvm.ctpop.{ir}({ir} {value})"
                        ));
                    }
                }
                let result = if matches!(inline, Inline::Parity(_)) {
                    let bit = self.fresh();
                    self.emit(&format!("  {bit} = and {ir} {tmp}, 1"));
                    bit
                } else {
                    tmp
                };
                Some(int_result(self, &result, ty))
            }
            Inline::Ffs(ty) => {
                let value = self.builtin_arg(inline, args, 0);
                let ir = ty.ir_type();
                self.intrinsic(&format!("declare {ir} @llvm.cttz.{ir}({ir}, i1)"));
                let count = self.fresh();
                self.emit(&format!(
                    "  {count} = call {ir} @llvm.cttz.{ir}({ir} {value}, i1 false)"
                ));
                let plus = self.fresh();
                self.emit(&format!("  {plus} = add {ir} {count}, 1"));
                let zero = self.fresh();
                self.emit(&format!("  {zero} = icmp eq {ir} {value}, 0"));
                let pick = self.fresh();
                self.emit(&format!("  {pick} = select i1 {zero}, {ir} 0, {ir} {plus}"));
                Some(int_result(self, &pick, ty))
            }
            Inline::Clrsb(ty) => {
                let value = self.builtin_arg(inline, args, 0);
                let ir = ty.ir_type();
                let width = ty.width();
                self.intrinsic(&format!("declare {ir} @llvm.ctlz.{ir}({ir}, i1)"));
                let sign = self.fresh();
                self.emit(&format!("  {sign} = ashr {ir} {value}, {}", width - 1));
                let flipped = self.fresh();
                self.emit(&format!("  {flipped} = xor {ir} {value}, {sign}"));
                let count = self.fresh();
                self.emit(&format!(
                    "  {count} = call {ir} @llvm.ctlz.{ir}({ir} {flipped}, i1 false)"
                ));
                let less = self.fresh();
                self.emit(&format!("  {less} = sub {ir} {count}, 1"));
                Some(int_result(self, &less, ty))
            }
            Inline::Bswap(ty) => {
                let value = self.builtin_arg(inline, args, 0);
                let ir = ty.ir_type();
                self.intrinsic(&format!("declare {ir} @llvm.bswap.{ir}({ir})"));
                let tmp = self.fresh();
                self.emit(&format!(
                    "  {tmp} = call {ir} @llvm.bswap.{ir}({ir} {value})"
                ));
                Some(tmp)
            }
            Inline::Expect => {
                let value = self.builtin_arg(inline, args, 0);
                let _ = self.builtin_arg(inline, args, 1);
                Some(value)
            }
            Inline::Unreachable | Inline::Trap => {
                if inline == Inline::Trap {
                    self.intrinsic("declare void @llvm.trap()");
                    self.emit("  call void @llvm.trap()");
                }
                self.emit("  unreachable");
                self.terminated = true;
                // Code after the call is in a block with no predecessor.
                self.ensure_block();
                None
            }
            Inline::Alloca => {
                let size = self.builtin_arg(inline, args, 0);
                let tmp = self.fresh();
                self.emit(&format!("  {tmp} = alloca i8, i64 {size}, align 16"));
                Some(tmp)
            }
            Inline::Prefetch => {
                // A hint only. The address is evaluated for its side effects.
                for index in 0..args.len() {
                    let _ = self.builtin_arg(inline, args, index);
                }
                None
            }
            Inline::AssumeAligned => {
                let ptr = self.builtin_arg(inline, args, 0);
                for index in 1..args.len() {
                    let _ = self.builtin_arg(inline, args, index);
                }
                Some(ptr)
            }
            Inline::ReturnAddress => {
                self.intrinsic("declare ptr @llvm.returnaddress(i32)");
                let tmp = self.fresh();
                self.emit(&format!("  {tmp} = call ptr @llvm.returnaddress(i32 0)"));
                Some(tmp)
            }
            Inline::FrameAddress => {
                self.intrinsic("declare ptr @llvm.frameaddress.p0(i32)");
                let tmp = self.fresh();
                self.emit(&format!("  {tmp} = call ptr @llvm.frameaddress.p0(i32 0)"));
                Some(tmp)
            }
            Inline::IsNan | Inline::IsInf | Inline::IsFinite | Inline::IsNormal => {
                let value = self.gen_val(&args[0]);
                let ty = args[0].ty().expect("typed");
                let kind = ty.float_kind().expect("floating argument");
                let ir = ty.ir_type();
                let cmp = self.fresh();
                if inline == Inline::IsNan {
                    self.emit(&format!("  {cmp} = fcmp uno {ir} {value}, {value}"));
                } else {
                    let suffix = match ir.as_str() {
                        "float" => "f32",
                        "double" => "f64",
                        "fp128" => "f128",
                        _ => "f80",
                    };
                    self.intrinsic(&format!("declare {ir} @llvm.fabs.{suffix}({ir})"));
                    let abs = self.fresh();
                    self.emit(&format!(
                        "  {abs} = call {ir} @llvm.fabs.{suffix}({ir} {value})"
                    ));
                    let (inf, min_normal): (u128, u128) = match crate::float::fold(kind) {
                        FloatKind::Float => (0x7f80_0000, 0x0080_0000),
                        FloatKind::Double => (0x7ff0_0000_0000_0000, 0x0010_0000_0000_0000),
                        FloatKind::LongDouble => {
                            (0x7fff_8000_0000_0000_0000, 0x0001_8000_0000_0000_0000)
                        }
                    };
                    let inf = crate::float::llvm_const(kind, inf);
                    match inline {
                        Inline::IsInf => {
                            self.emit(&format!("  {cmp} = fcmp oeq {ir} {abs}, {inf}"));
                        }
                        Inline::IsFinite => {
                            self.emit(&format!("  {cmp} = fcmp olt {ir} {abs}, {inf}"));
                        }
                        _ => {
                            // The threshold is the run-time format's, not
                            // the folding precision's: binary128 `long
                            // double` is normal down to 2^-16382.
                            let min = if ir == "fp128" {
                                "0xL00000000000000000001000000000000".to_string()
                            } else {
                                crate::float::llvm_const(kind, min_normal)
                            };
                            let below = self.fresh();
                            self.emit(&format!("  {below} = fcmp olt {ir} {abs}, {inf}"));
                            let above = self.fresh();
                            self.emit(&format!("  {above} = fcmp oge {ir} {abs}, {min}"));
                            self.emit(&format!("  {cmp} = and i1 {below}, {above}"));
                        }
                    }
                }
                let tmp = self.fresh();
                self.emit(&format!("  {tmp} = zext i1 {cmp} to i32"));
                Some(tmp)
            }
            Inline::Overflow { op, fixed } => Some(self.gen_overflow(op, fixed, args)),
        }
    }

    /// `__builtin_add_overflow` and friends. Both operands are widened to
    /// `i128`, where the exact result of an addition or subtraction of
    /// 64-bit values fits. A product is formed from the magnitudes, which
    /// fit in 128 bits unsigned, and its sign. The result is stored
    /// truncated to the target type; the return value says whether the
    /// exact result was out of that type's range.
    fn gen_overflow(
        &mut self,
        op: crate::builtins::OverflowOp,
        fixed: Option<TypeName>,
        args: &[Expr],
    ) -> String {
        use crate::builtins::OverflowOp;
        let operand = |gen: &mut Self, index: usize| -> (String, TypeName) {
            let value = gen.gen_val(&args[index]);
            let src = args[index].ty().expect("typed");
            let ty = fixed.unwrap_or_else(|| src.as_integer().expect("integer operand"));
            let value = gen.convert(&value, &src, &CType::Int(ty));
            if ty.width() == 128 {
                return (value, ty);
            }
            let wide = gen.fresh();
            let ext = if ty.is_signed() { "sext" } else { "zext" };
            gen.emit(&format!(
                "  {wide} = {ext} {} {value} to i128",
                ty.ir_type()
            ));
            (wide, ty)
        };
        let (left, left_ty) = operand(self, 0);
        let (right, right_ty) = operand(self, 1);
        let dest_ptr = self.gen_val(&args[2]);
        let target = match fixed {
            Some(ty) => ty,
            None => args[2]
                .ty()
                .and_then(|ty| ty.pointee().and_then(CType::as_integer))
                .expect("pointer to an integer"),
        };
        let tir = target.ir_type();
        let tw = target.width();
        if left_ty.width() == 128 || right_ty.width() == 128 || tw == 128 {
            return self.gen_overflow_i256(
                op,
                [(&left, left_ty), (&right, right_ty)],
                &dest_ptr,
                target,
            );
        }
        let overflow = match op {
            OverflowOp::Add | OverflowOp::Sub => {
                let exact = self.fresh();
                let inst = if op == OverflowOp::Add { "add" } else { "sub" };
                self.emit(&format!("  {exact} = {inst} i128 {left}, {right}"));
                let low = self.fresh();
                self.emit(&format!("  {low} = trunc i128 {exact} to {tir}"));
                self.store(&CType::Int(target), &low, &dest_ptr);
                let back = self.fresh();
                let ext = if target.is_signed() { "sext" } else { "zext" };
                self.emit(&format!("  {back} = {ext} {tir} {low} to i128"));
                let ov = self.fresh();
                self.emit(&format!("  {ov} = icmp ne i128 {back}, {exact}"));
                ov
            }
            OverflowOp::Mul => {
                let magnitude = |gen: &mut Self, value: &str, ty: TypeName| -> (String, String) {
                    if !ty.is_signed() {
                        return (value.to_string(), "false".to_string());
                    }
                    let neg = gen.fresh();
                    gen.emit(&format!("  {neg} = icmp slt i128 {value}, 0"));
                    let flipped = gen.fresh();
                    gen.emit(&format!("  {flipped} = sub i128 0, {value}"));
                    let abs = gen.fresh();
                    gen.emit(&format!(
                        "  {abs} = select i1 {neg}, i128 {flipped}, i128 {value}"
                    ));
                    (abs, neg)
                };
                let (left_abs, left_neg) = magnitude(self, &left, left_ty);
                let (right_abs, right_neg) = magnitude(self, &right, right_ty);
                let mag = self.fresh();
                self.emit(&format!("  {mag} = mul i128 {left_abs}, {right_abs}"));
                let neg = self.fresh();
                self.emit(&format!("  {neg} = xor i1 {left_neg}, {right_neg}"));
                let flipped = self.fresh();
                self.emit(&format!("  {flipped} = sub i128 0, {mag}"));
                let exact = self.fresh();
                self.emit(&format!(
                    "  {exact} = select i1 {neg}, i128 {flipped}, i128 {mag}"
                ));
                let low = self.fresh();
                self.emit(&format!("  {low} = trunc i128 {exact} to {tir}"));
                self.store(&CType::Int(target), &low, &dest_ptr);
                // A zero product is never negative.
                let nonzero = self.fresh();
                self.emit(&format!("  {nonzero} = icmp ne i128 {mag}, 0"));
                let negative = self.fresh();
                self.emit(&format!("  {negative} = and i1 {neg}, {nonzero}"));
                if target.is_signed() {
                    let max = (1u128 << (tw - 1)) - 1;
                    let limit = self.fresh();
                    self.emit(&format!(
                        "  {limit} = select i1 {negative}, i128 {}, i128 {max}",
                        max + 1
                    ));
                    let ov = self.fresh();
                    self.emit(&format!("  {ov} = icmp ugt i128 {mag}, {limit}"));
                    ov
                } else {
                    let max = if tw == 64 {
                        u128::from(u64::MAX)
                    } else {
                        (1u128 << tw) - 1
                    };
                    let big = self.fresh();
                    self.emit(&format!("  {big} = icmp ugt i128 {mag}, {max}"));
                    let ov = self.fresh();
                    self.emit(&format!("  {ov} = or i1 {big}, {negative}"));
                    ov
                }
            }
        };
        let tmp = self.fresh();
        self.emit(&format!("  {tmp} = zext i1 {overflow} to i8"));
        tmp
    }

    /// Overflow checking for a 128-bit operand or destination.
    ///
    /// A same-width `sext`/`trunc` of `i128` is not valid LLVM, and an
    /// `i128` add/sub/mul can wrap, so the overflow flag would be wrong.
    /// Each operand is extended to `i256` (so mixed signs keep infinite
    /// precision, as in `__builtin_add_overflow((__int128)-1, 1, &u128)`),
    /// the operation is done there, and the exact result is tested against
    /// the destination limits. The stored value is the low bits of that
    /// exact result.
    fn gen_overflow_i256(
        &mut self,
        op: crate::builtins::OverflowOp,
        operands: [(&str, TypeName); 2],
        dest_ptr: &str,
        target: TypeName,
    ) -> String {
        use crate::builtins::OverflowOp;
        let widen = |gen: &mut Self, value: &str, ty: TypeName| -> String {
            let ext = if ty.is_signed() { "sext" } else { "zext" };
            let wide = gen.fresh();
            gen.emit(&format!("  {wide} = {ext} i128 {value} to i256"));
            wide
        };
        let left = widen(self, operands[0].0, operands[0].1);
        let right = widen(self, operands[1].0, operands[1].1);
        let inst = match op {
            OverflowOp::Add => "add",
            OverflowOp::Sub => "sub",
            OverflowOp::Mul => "mul",
        };
        let exact = self.fresh();
        self.emit(&format!("  {exact} = {inst} i256 {left}, {right}"));
        let limit = |gen: &mut Self, signed: bool, width: u32, min: bool| -> String {
            if !signed && !min {
                let sh = gen.fresh();
                gen.emit(&format!("  {sh} = shl i256 1, {width}"));
                let max = gen.fresh();
                gen.emit(&format!("  {max} = sub i256 {sh}, 1"));
                return max;
            }
            if signed && !min {
                let sh = gen.fresh();
                gen.emit(&format!("  {sh} = shl nsw i256 1, {}", width - 1));
                let max = gen.fresh();
                gen.emit(&format!("  {max} = sub i256 {sh}, 1"));
                return max;
            }
            if signed && min {
                let sh = gen.fresh();
                gen.emit(&format!("  {sh} = shl nsw i256 1, {}", width - 1));
                let v = gen.fresh();
                gen.emit(&format!("  {v} = sub i256 0, {sh}"));
                return v;
            }
            "0".to_string()
        };
        let tw = target.width();
        let signed = target.is_signed();
        let max = limit(self, signed, tw, false);
        let pred_lo = if signed { "sge" } else { "uge" };
        let pred_hi = if signed { "sle" } else { "ule" };
        let ok_lo = self.fresh();
        if signed {
            let min = limit(self, signed, tw, true);
            self.emit(&format!("  {ok_lo} = icmp {pred_lo} i256 {exact}, {min}"));
        } else {
            self.emit(&format!("  {ok_lo} = icmp {pred_lo} i256 {exact}, 0"));
        }
        let ok_hi = self.fresh();
        self.emit(&format!("  {ok_hi} = icmp {pred_hi} i256 {exact}, {max}"));
        let ok = self.fresh();
        self.emit(&format!("  {ok} = and i1 {ok_lo}, {ok_hi}"));
        let ov = self.fresh();
        self.emit(&format!("  {ov} = xor i1 {ok}, true"));
        let low = self.fresh();
        let tir = target.ir_type();
        self.emit(&format!("  {low} = trunc i256 {exact} to {tir}"));
        self.store(&CType::Int(target), &low, dest_ptr);
        let tmp = self.fresh();
        self.emit(&format!("  {tmp} = zext i1 {ov} to i8"));
        tmp
    }

    fn zext_cond(&mut self, expr: &Expr) -> String {
        let cond = self.gen_cond(expr);
        let tmp = self.fresh();
        self.emit(&format!("  {tmp} = zext i1 {cond} to i32"));
        tmp
    }

    fn gen_cond(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Bin {
                op, left, right, ..
            } if op.is_cmp() => {
                let left_ty = left.ty().expect("typed");
                let right_ty = right.ty().expect("typed");
                if left_ty.is_float() || right_ty.is_float() {
                    let common = common_real(&left_ty, &right_ty);
                    let left_val = self.gen_val(left);
                    let left_val = self.convert(&left_val, &left_ty, &common);
                    let right_val = self.gen_val(right);
                    let right_val = self.convert(&right_val, &right_ty, &common);
                    let tmp = self.fresh();
                    self.emit(&format!(
                        "  {tmp} = fcmp {} {} {left_val}, {right_val}",
                        fcmp_pred(*op),
                        common.ir_type()
                    ));
                    return tmp;
                }
                if left_ty.is_pointer() || right_ty.is_pointer() {
                    let left_val = self.as_ptr(left);
                    let right_val = self.as_ptr(right);
                    let tmp = self.fresh();
                    self.emit(&format!(
                        "  {tmp} = icmp {} ptr {left_val}, {right_val}",
                        ptr_pred(*op)
                    ));
                    return tmp;
                }
                let left_i = left_ty.as_integer().expect("integer");
                let right_i = right_ty.as_integer().expect("integer");
                let common = left_i.usual(right_i);
                let common_ty = CType::from_name(common);
                let left_val = self.gen_val(left);
                let left_val = self.convert(&left_val, &left_ty, &common_ty);
                let right_val = self.gen_val(right);
                let right_val = self.convert(&right_val, &right_ty, &common_ty);
                let tmp = self.fresh();
                self.emit(&format!(
                    "  {tmp} = icmp {} {} {left_val}, {right_val}",
                    cmp_pred(*op, common.is_signed()),
                    common.ir_type()
                ));
                tmp
            }
            Expr::Bin {
                op: BinOp::And,
                left,
                right,
                ..
            } => self.gen_and(left, right),
            Expr::Bin {
                op: BinOp::Or,
                left,
                right,
                ..
            } => self.gen_or(left, right),
            Expr::Unary {
                op: UnaryOp::Not,
                expr,
                ..
            } => {
                let inner = self.gen_cond(expr);
                let tmp = self.fresh();
                self.emit(&format!("  {tmp} = xor i1 {inner}, true"));
                tmp
            }
            _ => {
                let value = self.gen_val(expr);
                let ty = expr.ty().expect("typed");
                let tmp = self.fresh();
                if ty.is_pointer() {
                    self.emit(&format!("  {tmp} = icmp ne ptr {value}, null"));
                } else if ty.is_float() {
                    let zero = crate::float::llvm_const(ty.float_kind().expect("float"), 0);
                    self.emit(&format!(
                        "  {tmp} = fcmp une {} {value}, {zero}",
                        ty.ir_type()
                    ));
                } else {
                    self.emit(&format!("  {tmp} = icmp ne {} {value}, 0", ty.ir_type()));
                }
                tmp
            }
        }
    }

    fn gen_and(&mut self, left: &Expr, right: &Expr) -> String {
        let left = self.gen_cond(left);
        let rhs_label = self.label();
        let false_label = self.label();
        let end_label = self.label();
        self.br_cond(&left, &rhs_label, &false_label);
        self.start(rhs_label);
        let right = self.gen_cond(right);
        let rhs_block = self.cur.clone();
        self.br(&end_label);
        self.start(false_label.clone());
        self.br(&end_label);
        self.start(end_label);
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = phi i1 [ {right}, %{rhs_block} ], [ false, %{false_label} ]"
        ));
        tmp
    }

    /// Only the chosen arm is evaluated. Each arm is converted to `result`
    /// in its own block, so the phi operands are defined in the predecessors.
    fn gen_ternary(
        &mut self,
        cond: &Expr,
        then_expr: &Expr,
        else_expr: &Expr,
        result: CType,
    ) -> Option<String> {
        let cond = self.gen_cond(cond);
        let then_label = self.label();
        let else_label = self.label();
        let end_label = self.label();
        self.br_cond(&cond, &then_label, &else_label);
        self.start(then_label);
        let then_val = self
            .gen_expr(then_expr)
            .map(|value| self.convert(&value, &then_expr.ty().expect("typed"), &result));
        assert!(
            !self.terminated,
            "internal compiler error: expression terminated a block"
        );
        let then_block = self.cur.clone();
        self.br(&end_label);
        self.start(else_label);
        let else_val = self
            .gen_expr(else_expr)
            .map(|value| self.convert(&value, &else_expr.ty().expect("typed"), &result));
        assert!(
            !self.terminated,
            "internal compiler error: expression terminated a block"
        );
        let else_block = self.cur.clone();
        self.br(&end_label);
        self.start(end_label);
        match (then_val, else_val) {
            (Some(then_val), Some(else_val)) => {
                let tmp = self.fresh();
                self.emit(&format!(
                    "  {tmp} = phi {} [ {then_val}, %{then_block} ], [ {else_val}, %{else_block} ]",
                    result.ir_type()
                ));
                Some(tmp)
            }
            (None, None) => None,
            _ => unreachable!("conditional arms have different types"),
        }
    }

    fn gen_or(&mut self, left: &Expr, right: &Expr) -> String {
        let left = self.gen_cond(left);
        let true_label = self.label();
        let rhs_label = self.label();
        let end_label = self.label();
        self.br_cond(&left, &true_label, &rhs_label);
        self.start(rhs_label);
        let right = self.gen_cond(right);
        let rhs_block = self.cur.clone();
        self.br(&end_label);
        self.start(true_label.clone());
        self.br(&end_label);
        self.start(end_label);
        let tmp = self.fresh();
        self.emit(&format!(
            "  {tmp} = phi i1 [ true, %{true_label} ], [ {right}, %{rhs_block} ]"
        ));
        tmp
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

/// `i32 (ptr, ...)` — the function type written on a variadic call.
///
/// Fixed parameters are the lowered types (`i64`, `ptr`, …). `byval` and
/// `sret` stay on the call arguments; they are not part of this type.
fn variadic_sig(sig: &FnSig) -> String {
    let mut parts = Vec::new();
    if sig.sret_attrs.is_some() {
        parts.push("ptr".to_string());
    }
    for group in &sig.params {
        for part in group {
            parts.push(part.ir_type.clone());
        }
    }
    parts.push("...".to_string());
    format!("{} ({})", sig.ret_type, parts.join(", "))
}

/// Alignment of a slot that holds an AAPCS64 coerced value.
fn coerce_align(ir: &str) -> u32 {
    if ir == "i128" || ir.contains("fp128") {
        16
    } else {
        8
    }
}

/// Alignment of an address `offset` bytes into an object of alignment `base`.
fn piece_align(base_align: u32, offset: u64) -> u32 {
    if offset == 0 {
        return base_align.clamp(1, 8);
    }
    let mut align = 1u32;
    let base = base_align.max(1);
    while align < 8 {
        let next = align * 2;
        if next > base || offset % u64::from(next) != 0 {
            break;
        }
        align = next;
    }
    align
}

/// Default argument promotions. Narrow integers become `int`. `float`
/// becomes `double` (C11 6.5.2.2). Pointers and `double` are unchanged.
fn default_promote(ty: &CType) -> CType {
    if ty.float_kind() == Some(FloatKind::Float) {
        return CType::Float(FloatKind::Double);
    }
    match ty.as_integer() {
        Some(int) => CType::from_name(int.promote()),
        None => ty.clone(),
    }
}

struct BitInfo {
    width: u32,
    signed_field: bool,
    container_bits: u32,
    abs_bit: u32,
    container_off: u64,
    declared: TypeName,
}

fn bit_info(expr: &Expr) -> Option<BitInfo> {
    match expr {
        Expr::Member {
            bit_width: Some(width),
            bit_signed,
            container_bits,
            abs_bit,
            bit_declared,
            offset,
            ..
        } => Some(BitInfo {
            width: *width,
            signed_field: *bit_signed,
            container_bits: *container_bits,
            abs_bit: *abs_bit,
            container_off: offset.unwrap_or(0),
            declared: *bit_declared,
        }),
        _ => None,
    }
}

fn container_type(bits: u32, signed_field: bool, quals: Quals) -> CType {
    let name = match (bits, signed_field) {
        (8, true) => TypeName::Char,
        (8, false) => TypeName::UChar,
        (16, true) => TypeName::Short,
        (16, false) => TypeName::UShort,
        (32, true) => TypeName::Int,
        (32, false) => TypeName::UInt,
        (_, true) => TypeName::Long,
        (_, false) => TypeName::ULong,
    };
    CType::from_name(name).with_quals(quals)
}

fn width_mask(bits: u32) -> u64 {
    if bits >= 64 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
    }
}

fn width_mask_u128(bits: u32) -> u128 {
    if bits >= 128 {
        u128::MAX
    } else {
        (1u128 << bits) - 1
    }
}

struct RegFetch {
    field: u64,
    slots: u32,
    stride: u64,
    limit: u64,
    stack_align: u64,
    stack_size: u64,
}

enum VaClass {
    Gp {
        slots: u32,
        stack_align: u64,
        stack_size: u64,
    },
    Fp,
    Mem {
        align: u64,
        size: u64,
    },
    /// Eightbytes that are not one packed image in the save area.
    Pieces(Vec<crate::abi::RegPiece>),
}

/// How one variadic argument is found on AArch64.
struct A64Va {
    /// Consecutive FP/SIMD registers (an HFA's member count, or 1 for a
    /// floating scalar). Zero for the general-register class.
    fp_regs: u32,
    /// Size of one HFA member, or of the floating scalar.
    fp_unit: u64,
    /// General registers, when `fp_regs` is zero.
    gp_regs: u32,
    /// The argument is a pointer to a caller-made copy.
    by_ref: bool,
    /// 16-byte alignment: an even register pair and a 16-byte-aligned
    /// stack slot.
    align16: bool,
    /// Bytes the argument occupies on the stack, a multiple of 8.
    stack_size: u64,
}

fn aarch64_va_class(ty: &CType) -> A64Va {
    let bare = ty.unqualified();
    let size = bare.size().unwrap_or(8);
    let round8 = ((size + 7) & !7).max(8);
    let align16 = crate::aapcs64::arg_align(&bare) >= 16;
    if bare.is_record() {
        if let Some((base, count)) = crate::aapcs64::hfa(&bare) {
            let unit = match base {
                "float" => 4,
                "double" => 8,
                _ => 16,
            };
            return A64Va {
                fp_regs: count as u32,
                fp_unit: unit,
                gp_regs: 0,
                by_ref: false,
                align16,
                stack_size: round8,
            };
        }
        if size > 16 {
            return A64Va {
                fp_regs: 0,
                fp_unit: 0,
                gp_regs: 1,
                by_ref: true,
                align16: false,
                stack_size: 8,
            };
        }
        return A64Va {
            fp_regs: 0,
            fp_unit: 0,
            gp_regs: (round8 / 8) as u32,
            by_ref: false,
            align16,
            stack_size: round8,
        };
    }
    if let Some(kind) = bare.float_kind() {
        return A64Va {
            fp_regs: 1,
            fp_unit: CType::Float(kind).size().unwrap_or(8),
            gp_regs: 0,
            by_ref: false,
            align16,
            stack_size: round8,
        };
    }
    A64Va {
        fp_regs: 0,
        fp_unit: 0,
        gp_regs: (round8 / 8) as u32,
        by_ref: false,
        align16,
        stack_size: round8,
    }
}

fn va_class(ty: &CType) -> VaClass {
    let bare = ty.unqualified();
    if bare.is_record() {
        let size = bare.size().unwrap_or(1);
        let stack = (size + 7) & !7;
        return match crate::abi::classify_for(&bare, crate::abi::AggUse::Param) {
            Ok(AggAbi::Direct(pieces)) => {
                let sse = pieces.iter().filter(|piece| piece.is_sse()).count();
                if sse == 0 {
                    // In the overflow area the record keeps its own
                    // alignment, which `_Alignas(16)` can raise above 8.
                    VaClass::Gp {
                        slots: pieces.len().max(1) as u32,
                        stack_align: u64::from(bare.align()).max(8),
                        stack_size: stack.max(8),
                    }
                } else if sse == pieces.len() && pieces.len() == 1 {
                    // One XMM slot holds the whole object in its low bytes.
                    VaClass::Fp
                } else {
                    VaClass::Pieces(pieces)
                }
            }
            _ => VaClass::Mem {
                align: u64::from(bare.align()).max(8),
                size: stack.max(8),
            },
        };
    }
    // `long double` is an x87 value. The register save area has no x87
    // slots, so a variadic `long double` is taken from the overflow area,
    // 16-byte aligned.
    if bare.float_kind() == Some(FloatKind::LongDouble) {
        return VaClass::Mem {
            align: 16,
            size: 16,
        };
    }
    if bare.as_float().is_some() {
        return VaClass::Fp;
    }
    let size = bare.size().unwrap_or(8);
    // A 128-bit integer uses two general-purpose slots and 16-byte
    // alignment (System V AMD64).
    let (slots, stack_align) = if size == 16 { (2, 16) } else { (1, 8) };
    VaClass::Gp {
        slots,
        stack_align,
        stack_size: ((size + 7) & !7).max(8),
    }
}

fn piece_bytes(piece: &crate::abi::RegPiece) -> u64 {
    match piece {
        crate::abi::RegPiece::Ptr { .. } => 8,
        crate::abi::RegPiece::Int { bits, .. } => u64::from(bits / 8),
        crate::abi::RegPiece::Sse { kind, .. } => match kind {
            crate::abi::SsePiece::Float => 4,
            crate::abi::SsePiece::Double | crate::abi::SsePiece::FloatPair => 8,
        },
    }
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

/// Spell raw bytes as the union-shaped LLVM constant for `ty`.
fn render_byte_blob(ty: &CType, raw: &[u8]) -> String {
    let mut bytes = vec![0u8; ty.size().unwrap_or(raw.len() as u64) as usize];
    let n = raw.len().min(bytes.len());
    bytes[..n].copy_from_slice(&raw[..n]);
    let align = u64::from(ty.align().max(1));
    let (elem, step) = match align {
        2 => ("i16", 2usize),
        4 => ("i32", 4),
        8 => ("i64", 8),
        16 => ("i128", 16),
        _ => ("i8", 1),
    };
    let step = if bytes.len() % step != 0 { 1 } else { step };
    let elem = if step == 1 { "i8" } else { elem };
    let count = bytes.len() / step;
    if bytes.iter().all(|byte| *byte == 0) {
        return "zeroinitializer".to_string();
    }
    let mut parts = Vec::new();
    for index in 0..count {
        let mut acc: u128 = 0;
        for byte in 0..step {
            acc |= u128::from(bytes[index * step + byte]) << (8 * byte);
        }
        parts.push(format!("{elem} {acc}"));
    }
    if ty.is_union() || ty.record_has_bitfield() {
        format!("{{ [ {count} x {elem} ] [ {} ] }}", parts.join(", "))
    } else {
        format!("[ {} ]", parts.join(", "))
    }
}

fn float_rank(kind: FloatKind) -> u8 {
    match kind {
        FloatKind::Float => 0,
        FloatKind::Double => 1,
        FloatKind::LongDouble => 2,
    }
}

fn common_real(left: &CType, right: &CType) -> CType {
    CType::Float(crate::float::usual_real(
        left.float_kind(),
        right.float_kind(),
    ))
}

fn float_inst(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "fadd",
        BinOp::Sub => "fsub",
        BinOp::Mul => "fmul",
        BinOp::Div => "fdiv",
        _ => unreachable!("floating operator"),
    }
}

fn fcmp_pred(op: BinOp) -> &'static str {
    match op {
        BinOp::Eq => "oeq",
        BinOp::Ne => "une",
        BinOp::Lt => "olt",
        BinOp::Gt => "ogt",
        BinOp::Le => "ole",
        BinOp::Ge => "oge",
        _ => unreachable!("floating comparison"),
    }
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

fn value_inst(op: BinOp, signed: bool) -> &'static str {
    match op {
        BinOp::Add => "add",
        BinOp::Sub => "sub",
        BinOp::Mul => "mul",
        BinOp::Div => {
            if signed {
                "sdiv"
            } else {
                "udiv"
            }
        }
        BinOp::Mod => {
            if signed {
                "srem"
            } else {
                "urem"
            }
        }
        BinOp::BitAnd => "and",
        BinOp::BitOr => "or",
        BinOp::BitXor => "xor",
        BinOp::Shl => "shl",
        BinOp::Shr => {
            if signed {
                "ashr"
            } else {
                "lshr"
            }
        }
        BinOp::Eq
        | BinOp::Ne
        | BinOp::Lt
        | BinOp::Gt
        | BinOp::Le
        | BinOp::Ge
        | BinOp::And
        | BinOp::Or => {
            unreachable!("comparison or logical operator")
        }
    }
}

fn ptr_pred(op: BinOp) -> &'static str {
    match op {
        BinOp::Eq => "eq",
        BinOp::Ne => "ne",
        BinOp::Lt => "ult",
        BinOp::Gt => "ugt",
        BinOp::Le => "ule",
        BinOp::Ge => "uge",
        _ => unreachable!("pointer comparison"),
    }
}

/// A union's LLVM type is `{ [N x iK] }` with the union's alignment. The
/// constant is the first member's bytes, little endian, padded with zeros.
fn render_union_const(ty: &CType, value: &ConstInit) -> String {
    let size = ty.size().unwrap_or(0) as usize;
    let align = usize::try_from(ty.align().max(1)).unwrap_or(1);
    let mut bytes = vec![0u8; size];
    // Semantic analysis rejects an address constant in a union before this
    // runs. A `false` result here is a bug in that check, not a zero.
    assert!(
        write_const_bytes(ty, value, &mut bytes, 0),
        "union initializer contains a value that is not a byte constant"
    );
    if bytes.iter().all(|byte| *byte == 0) {
        return "zeroinitializer".to_string();
    }
    let (elem, step) = match align {
        2 => ("i16", 2),
        4 => ("i32", 4),
        8 => ("i64", 8),
        16 => ("i128", 16),
        _ => ("i8", 1),
    };
    let count = if step == 0 || bytes.len() % step != 0 {
        bytes.len()
    } else {
        bytes.len() / step
    };
    let step = if bytes.len() % step != 0 { 1 } else { step };
    let elem = if step == 1 { "i8" } else { elem };
    let mut parts = Vec::new();
    for index in 0..count {
        let mut acc: u128 = 0;
        for byte in 0..step {
            let at = index * step + byte;
            if at < bytes.len() {
                acc |= u128::from(bytes[at]) << (8 * byte);
            }
        }
        parts.push(format!("{elem} {acc}"));
    }
    format!("{{ [ {count} x {elem} ] [ {} ] }}", parts.join(", "))
}

fn write_const_bytes(ty: &CType, value: &ConstInit, bytes: &mut [u8], offset: usize) -> bool {
    match value {
        ConstInit::Zero => true,
        ConstInit::Float(bits) => {
            let width = ty.size().unwrap_or(0) as usize;
            let bits = &ty
                .float_kind()
                .map_or(*bits, |kind| crate::float::memory_bits(kind, *bits));
            for index in 0..width {
                let at = offset + index;
                if at < bytes.len() {
                    bytes[at] = (*bits >> (8 * index)) as u8;
                }
            }
            true
        }
        ConstInit::Int(bits) => {
            let Some(int) = ty.as_integer() else {
                return false;
            };
            let width = (int.width() / 8) as usize;
            for index in 0..width {
                let at = offset + index;
                if at < bytes.len() {
                    bytes[at] = (*bits >> (8 * index)) as u8;
                }
            }
            true
        }
        ConstInit::Aggregate(items) => {
            if ty.is_union() {
                let Some(field) = ty.struct_ref().and_then(|handle| {
                    handle.fields().and_then(|fields| fields.into_iter().next())
                }) else {
                    return true;
                };
                let Some(item) = items.first() else {
                    return true;
                };
                return write_const_bytes(&field.ty, item, bytes, offset);
            }
            if let Some(fields) = ty.struct_ref().and_then(StructRef::fields) {
                for (field, item) in fields.iter().zip(items) {
                    let at = offset + field.offset as usize;
                    if !write_const_bytes(&field.ty, item, bytes, at) {
                        return false;
                    }
                }
                return true;
            }
            if let (Some(elem), Some(len)) = (ty.array_elem(), ty.array_len()) {
                let step = elem.size().unwrap_or(0) as usize;
                for index in 0..len as usize {
                    let Some(item) = items.get(index) else {
                        break;
                    };
                    if !write_const_bytes(elem, item, bytes, offset + index * step) {
                        return false;
                    }
                }
                return true;
            }
            false
        }
        ConstInit::Ptr(PtrConst::Null) => true,
        ConstInit::Ptr(PtrConst::IntToPtr(bits)) => {
            for index in 0..8 {
                let at = offset + index;
                if at < bytes.len() {
                    bytes[at] = (*bits >> (8 * index)) as u8;
                }
            }
            true
        }
        ConstInit::Ptr(PtrConst::Addr { .. } | PtrConst::Label { .. })
        | ConstInit::PtrToInt { .. }
        | ConstInit::RelocBytes { .. } => false,
        ConstInit::Bytes(raw) => {
            for (index, byte) in raw.iter().enumerate() {
                let at = offset + index;
                if at < bytes.len() {
                    bytes[at] = *byte;
                }
            }
            true
        }
    }
}

/// A constant that holds an image with addresses somewhere inside it.
fn has_reloc_image(value: &ConstInit) -> bool {
    match value {
        ConstInit::RelocBytes { .. } => true,
        ConstInit::Aggregate(items) => items.iter().any(has_reloc_image),
        _ => false,
    }
}

/// The LLVM type and spelling of a static initializer.
///
/// Most constants have the object's own type. A union or bit-field record
/// is a byte array in LLVM, which cannot hold an address, so an image with
/// relocations becomes a packed struct of byte runs and `ptr` (or
/// `ptrtoint`) pieces at their offsets. A struct or array around one is
/// then a packed struct too, laid out from the C offsets, unless every
/// array element still has the same type.
fn render_typed_const(ty: &CType, value: &ConstInit, renames: &Renames) -> (String, String) {
    if !has_reloc_image(value) {
        return (ty.ir_type(), render_const(ty, value, renames));
    }
    let size = ty.size().unwrap_or(0);
    let mut pieces: Vec<(String, String)> = Vec::new();
    let pad = |pieces: &mut Vec<(String, String)>, from: u64, to: u64| {
        if to > from {
            pieces.push((
                format!("[{} x i8]", to - from),
                "zeroinitializer".to_string(),
            ));
        }
    };
    match value {
        ConstInit::RelocBytes { bytes, relocs } => {
            let mut at = 0u64;
            let run = |pieces: &mut Vec<(String, String)>, from: u64, to: u64| {
                if to > from {
                    let slice = &bytes[from as usize..to as usize];
                    let spelled = if slice.iter().all(|byte| *byte == 0) {
                        "zeroinitializer".to_string()
                    } else {
                        format!("c{}", llvm_bytes(slice))
                    };
                    pieces.push((format!("[{} x i8]", to - from), spelled));
                }
            };
            for (offset, reloc) in relocs {
                run(&mut pieces, at, *offset);
                match reloc {
                    ConstInit::PtrToInt { ptr, width } => {
                        let ir = format!("i{width}");
                        let spelled =
                            format!("ptrtoint ({} to {ir})", render_ptr_operand(ptr, renames));
                        pieces.push((ir, spelled));
                        at = offset + u64::from(*width / 8);
                    }
                    ConstInit::Ptr(ptr) => {
                        pieces.push(("ptr".to_string(), render_ptr(ptr, renames)));
                        at = offset + 8;
                    }
                    other => unreachable!("relocation {other:?}"),
                }
            }
            run(&mut pieces, at, bytes.len() as u64);
            pad(&mut pieces, bytes.len() as u64, size);
        }
        ConstInit::Aggregate(items) if ty.array_elem().is_some() => {
            let elem = ty.array_elem().expect("array element");
            let parts: Vec<_> = items
                .iter()
                .map(|item| render_typed_const(elem, item, renames))
                .collect();
            if parts.iter().all(|(ir, _)| *ir == parts[0].0) && !parts.is_empty() {
                let body = parts
                    .iter()
                    .map(|(ir, spelled)| format!("{ir} {spelled}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                return (
                    format!("[{} x {}]", parts.len(), parts[0].0),
                    format!("[{body}]"),
                );
            }
            let step = elem.size().unwrap_or(0);
            pieces.extend(parts);
            pad(&mut pieces, step * items.len() as u64, size);
        }
        ConstInit::Aggregate(items) => {
            let fields = ty
                .struct_ref()
                .and_then(StructRef::fields)
                .expect("aggregate with an image inside is a struct or array");
            let mut at = 0u64;
            for (field, item) in fields.iter().zip(items) {
                pad(&mut pieces, at, field.offset);
                pieces.push(render_typed_const(&field.ty, item, renames));
                at = at.max(field.offset + field.ty.size().unwrap_or(0));
            }
            pad(&mut pieces, at, size);
        }
        other => unreachable!("constant {other:?} has no image"),
    }
    let types = pieces
        .iter()
        .map(|(ir, _)| ir.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let body = pieces
        .iter()
        .map(|(ir, spelled)| format!("{ir} {spelled}"))
        .collect::<Vec<_>>()
        .join(", ");
    (format!("<{{ {types} }}>"), format!("<{{ {body} }}>"))
}

fn render_const(ty: &CType, value: &ConstInit, renames: &Renames) -> String {
    if ty.is_union() || ty.record_has_bitfield() {
        return render_union_const(ty, value);
    }
    match value {
        ConstInit::Zero => zero_const_spelling(ty),
        ConstInit::Int(bits) => ty
            .as_integer()
            .expect("integer constant")
            .format_bits(*bits),
        ConstInit::Float(bits) => {
            crate::float::llvm_const(ty.float_kind().expect("float constant"), *bits)
        }
        ConstInit::Ptr(ptr) => render_ptr(ptr, renames),
        ConstInit::PtrToInt { ptr, width } => {
            let ir = match width {
                8 => "i8",
                16 => "i16",
                32 => "i32",
                _ => "i64",
            };
            format!("ptrtoint ({} to {ir})", render_ptr_operand(ptr, renames))
        }
        ConstInit::Bytes(raw) => render_byte_blob(ty, raw),
        ConstInit::RelocBytes { .. } => {
            unreachable!("an image with addresses is spelled by render_typed_const")
        }
        ConstInit::Aggregate(items) => {
            if let Some(fields) = ty.struct_ref().and_then(StructRef::fields) {
                if fields.len() != items.len() {
                    return "zeroinitializer".to_string();
                }
                let parts = items
                    .iter()
                    .zip(fields.iter())
                    .map(|(item, field)| {
                        format!(
                            "{} {}",
                            field.ty.ir_type(),
                            render_const(&field.ty, item, renames)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                return format!("{{ {parts} }}");
            }
            let Some(elem) = ty.array_elem() else {
                return "zeroinitializer".to_string();
            };
            if let Some(bytes) = i8_bytes(ty, items) {
                return format!("c{}", llvm_bytes(&bytes));
            }
            let parts = items
                .iter()
                .map(|item| format!("{} {}", elem.ir_type(), render_const(elem, item, renames)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{parts}]")
        }
    }
}

/// A flat array of byte-sized integers can be spelled as an LLVM `c"..."`
/// constant. Wider elements keep the `[i32 1, i32 2]` form.
fn i8_bytes(ty: &CType, items: &[ConstInit]) -> Option<Vec<u8>> {
    let elem = ty.array_elem()?;
    if elem.as_integer().map(TypeName::width) != Some(8) {
        return None;
    }
    if items.len() != ty.array_len()? as usize {
        return None;
    }
    let mut bytes = Vec::with_capacity(items.len());
    for item in items {
        match item {
            ConstInit::Int(bits) => bytes.push(*bits as u8),
            ConstInit::Zero => bytes.push(0),
            _ => return None,
        }
    }
    Some(bytes)
}

fn zero_const_spelling(ty: &CType) -> String {
    if let Some(int) = ty.as_integer() {
        int.format_bits(0)
    } else if ty.is_pointer() {
        "null".to_string()
    } else {
        "zeroinitializer".to_string()
    }
}

fn render_ptr(ptr: &PtrConst, renames: &Renames) -> String {
    match ptr {
        PtrConst::Null => "null".to_string(),
        PtrConst::IntToPtr(bits) => format!("inttoptr (i64 {} to ptr)", i64_literal(*bits)),
        PtrConst::Addr { name, offset } if *offset == 0 => llvm_global(renamed(name, renames)),
        PtrConst::Addr { name, offset } => {
            format!(
                "getelementptr (i8, ptr {}, i64 {offset})",
                llvm_global(renamed(name, renames))
            )
        }
        PtrConst::Label { func, label } => format!(
            "blockaddress({}, %{})",
            llvm_global(renamed(func, renames)),
            c_label(label)
        ),
    }
}

fn render_ptr_operand(ptr: &PtrConst, renames: &Renames) -> String {
    format!("ptr {}", render_ptr(ptr, renames))
}

fn i64_literal(bits: u64) -> String {
    let signed = bits as i64;
    if signed < 0 {
        signed.to_string()
    } else {
        bits.to_string()
    }
}

fn cmp_pred(op: BinOp, signed: bool) -> &'static str {
    match op {
        BinOp::Eq => "eq",
        BinOp::Ne => "ne",
        BinOp::Lt => {
            if signed {
                "slt"
            } else {
                "ult"
            }
        }
        BinOp::Gt => {
            if signed {
                "sgt"
            } else {
                "ugt"
            }
        }
        BinOp::Le => {
            if signed {
                "sle"
            } else {
                "ule"
            }
        }
        BinOp::Ge => {
            if signed {
                "sge"
            } else {
                "uge"
            }
        }
        _ => unreachable!("comparison"),
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
mod tests {
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_paths() -> (PathBuf, PathBuf) {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir();
        let pid = std::process::id();
        (
            dir.join(format!("cc-{pid}-{id}.ll")),
            dir.join(format!("cc-{pid}-{id}.bin")),
        )
    }

    fn run_binary(ir: &str) -> (i32, Vec<u8>, Vec<u8>) {
        let (ll, bin) = temp_paths();
        std::fs::write(&ll, ir).unwrap();
        let assembled = Command::new("clang")
            .args([
                "-w",
                ll.to_str().unwrap(),
                "-o",
                bin.to_str().unwrap(),
                "-lm",
            ])
            .output()
            .expect("clang");
        assert!(
            assembled.status.success(),
            "clang rejected IR:\n{}\n{ir}",
            String::from_utf8_lossy(&assembled.stderr)
        );
        let ran = Command::new(&bin).output().expect("run compiled IR");
        let _ = std::fs::remove_file(&ll);
        let _ = std::fs::remove_file(&bin);
        (ran.status.code().unwrap_or(101), ran.stdout, ran.stderr)
    }

    fn run_reference(source: &str) -> (i32, Vec<u8>, Vec<u8>) {
        let cfile = temp_paths().0.with_extension("c");
        std::fs::write(&cfile, source).unwrap();
        let result = run_reference_file(&cfile);
        let _ = std::fs::remove_file(&cfile);
        result
    }

    fn run_reference_file(path: &std::path::Path) -> (i32, Vec<u8>, Vec<u8>) {
        let (_, bin) = temp_paths();
        let compiled = Command::new("clang")
            .args([
                "-w",
                "-std=c11",
                path.to_str().unwrap(),
                "-o",
                bin.to_str().unwrap(),
                "-lm",
            ])
            .output()
            .expect("clang");
        assert!(
            compiled.status.success(),
            "oracle failed:\n{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let ran = Command::new(&bin).output().expect("run oracle");
        let _ = std::fs::remove_file(&bin);
        (ran.status.code().unwrap_or(101), ran.stdout, ran.stderr)
    }

    #[test]
    fn source_filename_escapes_and_assembles() {
        let name = "a\"b\\c\n\t\u{7f}.c";
        let ir = crate::compile_to_ir("int main(void) { return 7; }\n", name).unwrap();
        let expected = format!("source_filename = {}\n", super::llvm_string(name));
        assert!(ir.starts_with(&expected), "{ir}");
        assert!(expected.contains("\\22"));
        assert!(expected.contains("\\\\"));
        assert!(!expected.contains("\\\""));
        assert_eq!(run_binary(&ir).0, 7);
        assert_eq!(super::llvm_global("main"), "@main");
        assert_eq!(super::llvm_global("a\"b"), "@\"a\\22b\"");
        assert_eq!(super::llvm_global(""), "@\"\"");
    }

    #[test]
    fn globals_and_prototypes_lower() {
        let ir = crate::compile_to_ir(
            "\
static int hidden = 3;
int counter;
extern int missing;
int add(int, int);
static int id(int x) { return x + hidden; }
int main(void) { counter = id(4); return counter; }
",
            "g.c",
        )
        .unwrap();
        assert!(
            ir.contains("@hidden = internal global i32 3, align 4"),
            "{ir}"
        );
        assert!(ir.contains("@counter = global i32 0, align 4"), "{ir}");
        assert!(
            ir.contains("@missing = external global i32, align 4"),
            "{ir}"
        );
        assert!(ir.contains("declare i32 @add(i32, i32)"), "{ir}");
        assert!(ir.contains("define internal i32 @id(i32 %p0)"), "{ir}");
        assert!(ir.contains("define i32 @main()"), "{ir}");
        assert!(!ir.contains("define i32 @add"), "{ir}");
        assert!(!ir.contains("declare i32 @id"), "{ir}");
        assert_eq!(run_binary(&ir).0, 7);
    }

    #[test]
    fn operators_lower_and_sizeof_is_not_emitted() {
        let ir = crate::compile_to_ir(
            "\
int g = 1 << 4;
int h = sizeof(int);
int i = ~0;
int j = 0 ? 1 : 6;
int main(void) {
    int x = 1;
    int y = sizeof(x = 5);
    int z = x++;
    int w = 0 ? y : z;
    g += 1;
    return y + w + (x << 1) + (-8 >> 1) + (~x & 7) + g;
}
",
            "ops.c",
        )
        .unwrap();
        assert!(ir.contains("@g = global i32 16, align 4"), "{ir}");
        assert!(ir.contains("@h = global i32 4, align 4"), "{ir}");
        assert!(ir.contains("@i = global i32 -1, align 4"), "{ir}");
        assert!(ir.contains("@j = global i32 6, align 4"), "{ir}");
        assert!(ir.contains("shl i32"), "{ir}");
        assert!(ir.contains("ashr i32"), "{ir}");
        assert!(ir.contains("xor i32"), "{ir}");
        assert!(ir.contains("phi i32"), "{ir}");
        assert!(!ir.contains("store i32 5"), "{ir}");
        // y = 4, w = z = 1, x becomes 2, 2 << 1 = 4, -8 >> 1 = -4,
        // ~2 & 7 = 5, g becomes 17. 4 + 1 + 4 - 4 + 5 + 17 = 27.
        assert_eq!(run_binary(&ir).0, 27);
    }

    #[test]
    fn static_locals_block_linkage_and_do_while() {
        let ir = crate::compile_to_ir(
            "\
static int hidden = 1;
int exposed = 4;
int ext = 2;
int bump(void) {
    static int n = 3;
    n = n + 1;
    return n;
}
int other(void) {
    static int n = 10;
    n = n + 1;
    return n;
}
int read_hidden(void) { return hidden; }
int see_exposed(void) {
    int exposed = 0;
    {
        extern int exposed;
        return exposed;
    }
}
int see_ext(void) {
    int ext = 0;
    {
        extern int ext;
        ext = ext + 5;
        return ext;
    }
}
int call_later(void) {
    int id(int);
    return id(6);
}
int id(int x) { return x; }
int loop(void) {
    int n = 0;
    int i = 0;
    do {
        i = i + 1;
        if (i < 3)
            continue;
        n = n + 1;
    } while (i < 4);
    return n * 10 + i;
}
int main(void) {
    if (bump() != 4) return 1;
    if (bump() != 5) return 2;
    if (other() != 11) return 3;
    if (read_hidden() != 1) return 4;
    if (see_exposed() != 4) return 8;
    if (see_ext() != 7) return 5;
    if (call_later() != 6) return 6;
    if (loop() != 24) return 7;
    return 0;
}
",
            "block.c",
        )
        .unwrap();
        assert!(
            ir.contains("@bump.n.1 = internal global i32 3, align 4"),
            "{ir}"
        );
        assert!(
            ir.contains("@other.n.2 = internal global i32 10, align 4"),
            "{ir}"
        );
        assert!(
            ir.contains("@hidden = internal global i32 1, align 4"),
            "{ir}"
        );
        assert!(ir.contains("@exposed = global i32 4, align 4"), "{ir}");
        assert!(ir.contains("@ext = global i32 2, align 4"), "{ir}");
        assert!(!ir.contains("store i32 3"), "{ir}");
        assert!(!ir.contains("declare i32 @id"), "{ir}");
        assert!(ir.contains("define i32 @id(i32 %p0)"), "{ir}");
        assert_eq!(run_binary(&ir).0, 0);

        let external = crate::compile_to_ir(
            "\
int main(void) {
    extern int missing;
    extern int missing;
    return missing;
}
",
            "ext.c",
        )
        .unwrap();
        assert_eq!(
            external
                .matches("@missing = external global i32, align 4")
                .count(),
            1,
            "{external}"
        );
    }

    #[test]
    fn return_constant_ir_assembles() {
        let ir = crate::compile_to_ir("int main(void) { return 42; }\n", "t.c").unwrap();
        assert!(ir.contains("define i32 @main()"));
        assert!(ir.contains("ret i32 42"));
        assert!(ir.contains("target triple = \"x86_64-pc-linux-gnu\""));
        assert!(ir.contains(&format!(
            "target datalayout = \"{}\"",
            crate::target::Target::X86_64Linux.data_layout()
        )));
        assert_eq!(run_binary(&ir).0, 42);
    }

    #[test]
    fn integer_abi_and_unsigned_ops() {
        let ir = crate::compile_to_ir(
            "\
char idc(char c) { return c; }
unsigned char idu(unsigned char u) { return u; }
short ids(short s) { return s; }
unsigned short idus(unsigned short s) { return s; }
int widen(char c, unsigned char u, short s, unsigned short us) {
    return c + u + s + us;
}
unsigned divu(unsigned a, unsigned b) { return a / b; }
unsigned shru(unsigned a) { return a >> 1; }
char c = -1;
unsigned char b = 200;
long l = 1;
extern short e;
",
            "abi.c",
        )
        .unwrap();
        assert!(
            ir.contains("define signext i8 @idc(i8 signext %p0)"),
            "{ir}"
        );
        assert!(
            ir.contains("define zeroext i8 @idu(i8 zeroext %p0)"),
            "{ir}"
        );
        assert!(
            ir.contains("define signext i16 @ids(i16 signext %p0)"),
            "{ir}"
        );
        assert!(
            ir.contains("define zeroext i16 @idus(i16 zeroext %p0)"),
            "{ir}"
        );
        assert!(
            ir.contains(
                "define i32 @widen(i8 signext %p0, i8 zeroext %p1, i16 signext %p2, i16 zeroext %p3)"
            ),
            "{ir}"
        );
        assert!(ir.contains("udiv i32"), "{ir}");
        assert!(ir.contains("lshr i32"), "{ir}");
        assert!(ir.contains("sext i8"), "{ir}");
        assert!(ir.contains("@c = global i8 -1, align 1"), "{ir}");
        assert!(ir.contains("@b = global i8 200, align 1"), "{ir}");
        assert!(ir.contains("@l = global i64 1, align 8"), "{ir}");
        assert!(ir.contains("@e = external global i16, align 2"), "{ir}");
        assert!(!ir.contains("signext i32"), "{ir}");
        assert!(!ir.contains("signext i64"), "{ir}");
        assert!(!ir.contains("zeroext i32"), "{ir}");
    }

    #[test]
    fn mixed_link_matches_clang_calling_convention() {
        let callee = "\
char idc(char c) { return c; }
unsigned char idu(unsigned char u) { return u; }
short ids(short s) { return s; }
unsigned short idus(unsigned short s) { return s; }
int widen(char c, unsigned char u, short s, unsigned short us) {
    return c + u + s + us;
}
";
        let caller = "\
extern char idc(char c);
extern unsigned char idu(unsigned char u);
extern short ids(short s);
extern unsigned short idus(unsigned short s);
extern int widen(char c, unsigned char u, short s, unsigned short us);
int main(void) {
    if (idc(-5) != -5) return 1;
    if (idu(200) != 200) return 2;
    if (ids(-300) != -300) return 3;
    if (idus(40000) != 40000) return 4;
    if (widen(-1, 200, -2, 40000) != 40197) return 5;
    return 0;
}
";
        let caller_ir = crate::compile_to_ir(caller, "caller.c").unwrap();
        assert!(
            caller_ir.contains("declare signext i8 @idc(i8 signext)"),
            "{caller_ir}"
        );
        assert!(
            caller_ir.contains("declare zeroext i16 @idus(i16 zeroext)"),
            "{caller_ir}"
        );
        assert!(
            caller_ir.contains("call signext i8 @idc(i8 signext"),
            "{caller_ir}"
        );
        assert!(
            caller_ir.contains("call zeroext i8 @idu(i8 zeroext"),
            "{caller_ir}"
        );

        let (ll, bin) = temp_paths();
        let obj = ll.with_extension("o");
        let cfile = ll.with_extension("clang.c");
        std::fs::write(&ll, &caller_ir).unwrap();
        std::fs::write(&cfile, callee).unwrap();
        let clang_obj = Command::new("clang")
            .args([
                "-w",
                "-std=c11",
                "-c",
                cfile.to_str().unwrap(),
                "-o",
                obj.to_str().unwrap(),
            ])
            .output()
            .expect("clang");
        assert!(
            clang_obj.status.success(),
            "{}",
            String::from_utf8_lossy(&clang_obj.stderr)
        );
        let linked = Command::new("clang")
            .args([
                "-w",
                ll.to_str().unwrap(),
                obj.to_str().unwrap(),
                "-o",
                bin.to_str().unwrap(),
            ])
            .output()
            .expect("clang link");
        assert!(
            linked.status.success(),
            "{}",
            String::from_utf8_lossy(&linked.stderr)
        );
        let ran = Command::new(&bin).output().expect("run");
        assert_eq!(ran.status.code(), Some(0));

        let callee_ir = crate::compile_to_ir(callee, "callee.c").unwrap();
        let our_ll = ll.with_extension("our.ll");
        let clang_main = ll.with_extension("main.c");
        let clang_obj = ll.with_extension("main.o");
        std::fs::write(&our_ll, callee_ir).unwrap();
        std::fs::write(&clang_main, caller).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-w",
                "-std=c11",
                "-c",
                clang_main.to_str().unwrap(),
                "-o",
                clang_obj.to_str().unwrap(),
            ])
            .output()
            .expect("clang");
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let bin2 = ll.with_extension("bin2");
        let linked = Command::new("clang")
            .args([
                "-w",
                our_ll.to_str().unwrap(),
                clang_obj.to_str().unwrap(),
                "-o",
                bin2.to_str().unwrap(),
            ])
            .output()
            .expect("clang link");
        assert!(
            linked.status.success(),
            "{}",
            String::from_utf8_lossy(&linked.stderr)
        );
        let ran = Command::new(&bin2).output().expect("run");
        assert_eq!(ran.status.code(), Some(0));
        let reference = run_reference(&format!("{callee}\n{caller}"));
        assert_eq!(reference.0, 0);

        let _ = std::fs::remove_file(&ll);
        let _ = std::fs::remove_file(&obj);
        let _ = std::fs::remove_file(&cfile);
        let _ = std::fs::remove_file(&bin);
        let _ = std::fs::remove_file(&our_ll);
        let _ = std::fs::remove_file(&clang_main);
        let _ = std::fs::remove_file(&clang_obj);
        let _ = std::fs::remove_file(&bin2);
    }

    #[test]
    fn pointer_and_array_ir() {
        let ir = crate::compile_to_ir(
            "\
int g;
int a[3] = {1, 2};
int *p = &g;
int *q = &a[2];
int *z = 0;
int main(void) {
    int x = 1;
    int *lp = &x;
    *lp = *p + a[1];
    lp = lp + 1;
    return *q + (z == 0);
}
",
            "ptr.c",
        )
        .unwrap();
        assert!(ir.contains("@p = global ptr @g, align 8"), "{ir}");
        assert!(
            ir.contains("@q = global ptr getelementptr (i8, ptr @a, i64 8), align 8"),
            "{ir}"
        );
        assert!(ir.contains("@z = global ptr null, align 8"), "{ir}");
        assert!(
            ir.contains("@a = global [3 x i32] [i32 1, i32 2, i32 0], align 4"),
            "{ir}"
        );
        assert!(ir.contains("getelementptr inbounds i32, ptr"), "{ir}");
        assert!(!ir.contains("inttoptr"), "{ir}");
        let (code, _, _) = run_binary(&ir);
        assert_eq!(
            code,
            run_reference(
                "\
int g;
int a[3] = {1, 2};
int *p = &g;
int *q = &a[2];
int *z = 0;
int main(void) {
    int x = 1;
    int *lp = &x;
    *lp = *p + a[1];
    lp = lp + 1;
    return *q + (z == 0);
}
"
            )
            .0
        );
    }

    #[test]
    fn strings_and_varargs_lower() {
        let hello = "\
int printf(const char *, ...);
int main(void) {
    printf(\"hello, world\\n\");
    return 0;
}
";
        let ir = crate::compile_to_ir(hello, "hello.c").unwrap();
        assert!(
            ir.contains(
                "@.str = private unnamed_addr constant [14 x i8] c\"hello, world\\0A\\00\", align 1"
            ),
            "{ir}"
        );
        assert!(ir.contains("declare i32 @printf(ptr, ...)"), "{ir}");
        assert!(
            ir.contains("call i32 (ptr, ...) @printf(ptr @.str)"),
            "{ir}"
        );
        let (code, out, err) = run_binary(&ir);
        assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
        assert_eq!(out, b"hello, world\n");

        let ir = crate::compile_to_ir(
            "\
int idc(char c, ...);
int main(void) {
    char c = 2;
    short s = 3;
    return idc(c, c, s);
}
char buf[4] = \"ab\";
const int g = 7;
char exact[2] = \"hi\";
",
            "va.c",
        )
        .unwrap();
        assert!(ir.contains("declare i32 @idc(i8 signext, ...)"), "{ir}");
        assert!(ir.contains("call i32 (i8, ...) @idc(i8 signext"), "{ir}");
        assert!(ir.contains(", i32 "), "{ir}");
        assert!(
            ir.contains("@buf = global [4 x i8] c\"ab\\00\\00\", align 1"),
            "{ir}"
        );
        assert!(ir.contains("@g = constant i32 7, align 4"), "{ir}");
        assert!(
            ir.contains("@exact = global [2 x i8] c\"hi\", align 1"),
            "{ir}"
        );
        assert!(!ir.contains("c\"hi\\00\""), "{ir}");
        let sized =
            crate::compile_to_ir("int main(void) { return sizeof \"abc\"; }\n", "sz.c").unwrap();
        assert!(!sized.contains("@.str"), "{sized}");
    }

    #[test]
    fn struct_ir_names_the_type_and_geps_members() {
        let source = "\
struct S { char c; int i; };
struct S g = {1, 2};
int main(void) {
    struct S s;
    s.c = 3;
    s.i = g.i;
    struct S t;
    t = s;
    return t.i + t.c;
}
";
        let ir = crate::compile_to_ir(source, "struct.c").unwrap();
        assert!(ir.contains("%struct.S = type { i8, i32 }"), "{ir}");
        assert!(
            ir.contains("@g = global %struct.S { i8 1, i32 2 }, align 4"),
            "{ir}"
        );
        assert!(ir.contains("getelementptr inbounds %struct.S, ptr"), "{ir}");
        assert!(ir.contains("call void @llvm.memmove.p0.p0.i64("), "{ir}");
        assert!(
            ir.contains("declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)"),
            "{ir}"
        );
        assert_eq!(run_binary(&ir), run_reference(source));
    }

    #[test]
    fn incomplete_extern_keeps_a_distinct_type_name() {
        let source = "\
void f(void) { struct S { int a; }; }
extern struct S x;
void g(void) { struct S { int b; }; }
int main(void) { return &x == 0; }
";
        let ir = crate::compile_to_ir(source, "opaque.c").unwrap();
        let mut names = std::collections::HashSet::new();
        for line in ir.lines() {
            let Some(rest) = line.strip_prefix('%') else {
                continue;
            };
            let Some((name, _)) = rest.split_once(" = type") else {
                continue;
            };
            assert!(
                names.insert(name.to_string()),
                "duplicate LLVM type %{name}\n{ir}"
            );
        }
        assert!(ir.contains("type opaque"), "{ir}");
        let (ll, _) = temp_paths();
        std::fs::write(&ll, &ir).unwrap();
        let obj = ll.with_extension("o");
        let assembled = Command::new("clang")
            .args([
                "-c",
                "-w",
                ll.to_str().unwrap(),
                "-o",
                obj.to_str().unwrap(),
            ])
            .output()
            .expect("clang");
        let _ = std::fs::remove_file(&ll);
        let _ = std::fs::remove_file(&obj);
        assert!(
            assembled.status.success(),
            "clang rejected IR:\n{}\n{ir}",
            String::from_utf8_lossy(&assembled.stderr)
        );
    }

    #[test]
    fn smoke_files_match_clang() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
        let mut files: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("c"))
            .collect();
        files.sort();
        assert!(!files.is_empty());
        for path in files {
            let source = std::fs::read_to_string(&path).unwrap();
            let name = path.to_string_lossy().to_string();
            let ir = crate::compile_to_ir(&source, &name).unwrap_or_else(|err| {
                panic!("{name}: {err}");
            });
            let mine = run_binary(&ir);
            let reference = run_reference_file(&path);
            assert_eq!(mine, reference, "{name}");
        }
    }

    #[test]
    fn union_enum_and_function_pointer_ir() {
        let source = "\
enum E { A = 2, B };
union U { int x; unsigned char b; };
int id(int n) { return n; }
int main(void) {
    union U u = {A};
    int (*p)(int) = id;
    return u.b + p(B);
}
";
        let ir = crate::compile_to_ir(source, "union.c").unwrap();
        assert!(ir.contains("%union.U = type"), "{ir}");
        assert!(ir.contains("getelementptr inbounds i8"), "{ir}");
        assert!(ir.contains("call i32 %"), "{ir}");
        assert_eq!(run_binary(&ir), run_reference(source));
    }

    #[test]
    fn switch_lowers_for_every_integer_width() {
        let source = "\
int on_int(int x) {
    switch (x) {
    case 1: return 1;
    case 2: case 3: return 2;
    default: return 4;
    }
}
int on_char(char c) {
    switch (c) {
    case -1: return 1;
    case 2: return 2;
    default: return 3;
    }
}
int on_uchar(unsigned char c) {
    switch (c) {
    case 255: return 1;
    case -1: return 2;
    default: return 3;
    }
}
int on_long(long x) {
    switch (x) {
    case -1: return 1;
    case 8589934592L: return 2;
    default: return 3;
    }
}
int on_ull(unsigned long long x) {
    switch (x) {
    case -1: return 1;
    case 2: return 2;
    default: return 3;
    }
}
int on_uint(unsigned x) {
    switch (x) {
    case -1: return 1;
    default: return 2;
    }
}
int main(void) {
    if (on_int(1) != 1 || on_int(3) != 2 || on_int(9) != 4) return 1;
    if (on_char(-1) != 1 || on_char(2) != 2 || on_char(0) != 3) return 2;
    if (on_uchar(255) != 1 || on_uchar(0) != 3) return 3;
    if (on_long(-1) != 1 || on_long(8589934592L) != 2) return 4;
    if (on_ull(-1) != 1 || on_ull(2) != 2 || on_ull(0) != 3) return 5;
    if (on_uint(4294967295u) != 1 || on_uint(1) != 2) return 6;
    return 0;
}
";
        let ir = crate::compile_to_ir(source, "switch.c").unwrap();
        assert!(ir.contains("switch i32"), "{ir}");
        assert!(ir.contains("switch i64"), "{ir}");
        assert!(ir.contains("sext i8"), "{ir}");
        assert!(ir.contains("zext i8"), "{ir}");
        assert!(ir.contains("i32 -1, label"), "{ir}");
        assert!(ir.contains("i64 -1, label"), "{ir}");
        assert!(ir.contains("i64 8589934592, label"), "{ir}");
        assert!(ir.contains("i64 18446744073709551615, label"), "{ir}");
        assert!(ir.contains("i32 4294967295, label"), "{ir}");
        assert_eq!(run_binary(&ir), run_reference(source));
    }

    #[test]
    fn struct_abi_links_with_clang() {
        let defs = r#"
struct s1 { unsigned char a; };
struct s3 { unsigned char a, b, c; };
struct s8 { long a; };
struct s12 { int a, b, c; };
struct s16 { long a; char b; };
struct big { long a, b, c; };
struct nest { char c; int i; };
struct outer { struct nest n; char z; };
struct ptrs { char c; int *p; };
union u9 { long a; unsigned char b[9]; };
struct s1 id1(struct s1 s) { return s; }
struct s3 id3(struct s3 s) { return s; }
struct s8 id8(struct s8 s) { return s; }
struct s12 id12(struct s12 s) { return s; }
struct s16 id16(struct s16 s) { return s; }
struct big idbig(struct big s) { s.b += 1; return s; }
struct outer idouter(struct outer s) { return s; }
int via_ptr(struct ptrs s) { return s.c + *s.p; }
int via_union(union u9 u) { return u.b[8]; }
int apply(int (*f)(int), int x) { return f(x + 1); }
int mix(int a, struct s12 s, int b, struct big g, int c) {
    return a + s.a + s.c + b + (int)g.a + c;
}
int spill(int a, int b, int c, int d, int e, struct s16 s, int z) {
    return a + b + c + d + e + z + (int)s.a + s.b;
}
"#;
        let caller = r#"
struct s1 { unsigned char a; };
struct s3 { unsigned char a, b, c; };
struct s8 { long a; };
struct s12 { int a, b, c; };
struct s16 { long a; char b; };
struct big { long a, b, c; };
struct nest { char c; int i; };
struct outer { struct nest n; char z; };
struct ptrs { char c; int *p; };
union u9 { long a; unsigned char b[9]; };
struct s1 id1(struct s1 s);
struct s3 id3(struct s3 s);
struct s8 id8(struct s8 s);
struct s12 id12(struct s12 s);
struct s16 id16(struct s16 s);
struct big idbig(struct big s);
struct outer idouter(struct outer s);
int via_ptr(struct ptrs s);
int via_union(union u9 u);
int apply(int (*f)(int), int x);
int mix(int a, struct s12 s, int b, struct big g, int c);
int spill(int a, int b, int c, int d, int e, struct s16 s, int z);
int inc(int x) { return x + 1; }
int main(void) {
    struct s1 a = {9};
    if (id1(a).a != 9) return 1;
    struct s3 b = {1, 2, 3};
    if (id3(b).a != 1 || id3(b).c != 3) return 2;
    struct s8 c = {99};
    if (id8(c).a != 99) return 3;
    struct s12 d = {4, 5, 6};
    if (id12(d).a != 4 || id12(d).b != 5 || id12(d).c != 6) return 4;
    struct s16 e = {7, 8};
    if (id16(e).a != 7 || id16(e).b != 8) return 5;
    struct big f = {10, 20, 30};
    struct big g = idbig(f);
    if (f.b != 20 || g.a != 10 || g.b != 21 || g.c != 30) return 6;
    struct outer h = {{1, 2}, 3};
    struct outer i = idouter(h);
    if (i.n.c != 1 || i.n.i != 2 || i.z != 3) return 7;
    int n = 40;
    struct ptrs p = {2, &n};
    if (via_ptr(p) != 42) return 8;
    union u9 u;
    u.a = 0;
    u.b[8] = 5;
    if (via_union(u) != 5) return 9;
    if (apply(inc, 40) != 42) return 10;
    if (mix(1, d, 2, f, 3) != 1 + 4 + 6 + 2 + 10 + 3) return 11;
    struct s16 sp = {100, 7};
    if (spill(1, 2, 3, 4, 5, sp, 6) != 128) return 12;
    return 0;
}
"#;
        let defs_ir = crate::compile_to_ir(defs, "defs.c").unwrap();
        assert!(defs_ir.contains("define i8 @id1(i8 %p0)"), "{defs_ir}");
        assert!(defs_ir.contains("define i24 @id3(i24 %p0)"), "{defs_ir}");
        assert!(
            defs_ir.contains("define { i64, i32 } @id12(i64 %p0, i32 %p0.1)"),
            "{defs_ir}"
        );
        assert!(
            defs_ir.contains("define { i64, i8 } @id16(i64 %p0, i8 %p0.1)"),
            "{defs_ir}"
        );
        assert!(
            defs_ir.contains("ptr sret(%struct.big) align 8 %agg.result"),
            "{defs_ir}"
        );
        assert!(
            defs_ir.contains("ptr byval(%struct.big) align 8 %p0"),
            "{defs_ir}"
        );
        assert!(
            defs_ir.contains("ptr byval(%struct.s16) align 8 %p5"),
            "{defs_ir}"
        );
        link_both_ways(defs, caller, &defs_ir);
    }

    fn link_both_ways(defs: &str, caller: &str, defs_ir: &str) {
        let (ll, bin) = temp_paths();
        let obj = ll.with_extension("o");
        let cfile = ll.with_extension("clang.c");
        // Our caller, clang definitions.
        let caller_ir = crate::compile_to_ir(caller, "caller.c").unwrap();
        std::fs::write(&ll, &caller_ir).unwrap();
        std::fs::write(&cfile, defs).unwrap();
        let clang_obj = Command::new("clang")
            .args([
                "-w",
                "-std=c11",
                "-c",
                cfile.to_str().unwrap(),
                "-o",
                obj.to_str().unwrap(),
            ])
            .output()
            .expect("clang");
        assert!(
            clang_obj.status.success(),
            "{}",
            String::from_utf8_lossy(&clang_obj.stderr)
        );
        let linked = Command::new("clang")
            .args([
                "-w",
                ll.to_str().unwrap(),
                obj.to_str().unwrap(),
                "-o",
                bin.to_str().unwrap(),
            ])
            .output()
            .expect("clang link");
        assert!(
            linked.status.success(),
            "link our caller:\n{}",
            String::from_utf8_lossy(&linked.stderr)
        );
        let ran = Command::new(&bin).output().expect("run");
        assert_eq!(
            ran.status.code(),
            Some(0),
            "our caller: {}",
            String::from_utf8_lossy(&ran.stderr)
        );

        // Our definitions, clang caller.
        let our_ll = ll.with_extension("our.ll");
        let clang_main = ll.with_extension("main.c");
        let clang_obj_path = ll.with_extension("main.o");
        std::fs::write(&our_ll, defs_ir).unwrap();
        std::fs::write(&clang_main, caller).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-w",
                "-std=c11",
                "-c",
                clang_main.to_str().unwrap(),
                "-o",
                clang_obj_path.to_str().unwrap(),
            ])
            .output()
            .expect("clang");
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let bin2 = ll.with_extension("bin2");
        let linked = Command::new("clang")
            .args([
                "-w",
                our_ll.to_str().unwrap(),
                clang_obj_path.to_str().unwrap(),
                "-o",
                bin2.to_str().unwrap(),
            ])
            .output()
            .expect("clang link");
        assert!(
            linked.status.success(),
            "link our defs:\n{}\n{defs_ir}",
            String::from_utf8_lossy(&linked.stderr)
        );
        let ran = Command::new(&bin2).output().expect("run");
        assert_eq!(
            ran.status.code(),
            Some(0),
            "our defs exit {}",
            ran.status.code().unwrap_or(-1)
        );
        let _ = std::fs::remove_file(&ll);
        let _ = std::fs::remove_file(&obj);
        let _ = std::fs::remove_file(&cfile);
        let _ = std::fs::remove_file(&bin);
        let _ = std::fs::remove_file(&our_ll);
        let _ = std::fs::remove_file(&clang_main);
        let _ = std::fs::remove_file(&clang_obj_path);
        let _ = std::fs::remove_file(&bin2);
    }
    #[test]
    fn signaling_nan_constants_keep_ieee_encoding() {
        // GCC "Floating-Point Format Builtins": a default `__builtin_nans*`
        // is a signaling NaN. LLVM hex constants for `float` use the
        // widened IEEE-64 pattern (Language Reference, "Simple Constants").
        let source = concat!(
            "float f = __builtin_nansf(\"\");\n",
            "double d = __builtin_nans(\"\");\n",
            "long double l = __builtin_nansl(\"\");\n",
        );
        let ir = crate::compile_to_ir(source, "nans.c").unwrap();
        assert!(
            ir.contains("@f = global float 0x7FF4000000000000"),
            "nansf encoding quieted or missing: {ir}"
        );
        assert!(
            ir.contains("@d = global double 0x7FF4000000000000"),
            "nans encoding missing: {ir}"
        );
        assert!(
            ir.contains("@l = global x86_fp80 0xK7FFFA000000000000000"),
            "nansl encoding missing: {ir}"
        );

        // Clang 19 as a black-box oracle: the same hex constants in its IR.
        let cfile = temp_paths().0.with_extension("c");
        std::fs::write(&cfile, source).unwrap();
        let ll = cfile.with_extension("ll");
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                "-O0",
                "-S",
                "-emit-llvm",
                "-o",
                ll.to_str().unwrap(),
                cfile.to_str().unwrap(),
            ])
            .output()
            .expect("clang");
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let clang_ir = std::fs::read_to_string(&ll).unwrap();
        assert!(
            clang_ir.contains("float 0x7FF4000000000000"),
            "clang nansf: {clang_ir}"
        );
        assert!(
            clang_ir.contains("double 0x7FF4000000000000"),
            "clang nans: {clang_ir}"
        );
        assert!(
            clang_ir.contains("x86_fp80 0xK7FFFA000000000000000"),
            "clang nansl: {clang_ir}"
        );
        let _ = std::fs::remove_file(&cfile);
        let _ = std::fs::remove_file(&ll);

        // C string semantics: bytes after the first NUL are not part of
        // the payload, so "1\0ignored" matches "1".
        let nul_ir = crate::compile_to_ir(
            "double a = __builtin_nan(\"1\");\ndouble b = __builtin_nan(\"1\\0ignored\");\n",
            "nan-nul.c",
        )
        .unwrap();
        assert_eq!(
            nul_ir.matches("0x7FF8000000000001").count(),
            2,
            "NUL payload should match \"1\": {nul_ir}"
        );
    }

    #[test]
    fn malformed_runtime_nan_tag_calls_libm() {
        // Clang 19 black-box: a runtime __builtin_nan("1xyz") is a call of
        // nan() (C11 7.12.11.2), not a payload-zero constant. The same
        // helper used by constant evaluation decides whether the tag folds.
        let ir = crate::compile_to_ir(
            "double f(void) { return __builtin_nan(\"1xyz\"); }\n",
            "nan-runtime.c",
        )
        .unwrap();
        assert!(
            ir.contains("declare double @nan(ptr)"),
            "missing nan declaration: {ir}"
        );
        assert!(
            ir.contains("call double @nan(ptr") && ir.contains("c\"1xyz\\00\""),
            "should call nan() with the tag string: {ir}"
        );
        assert!(
            !ir.contains("ret double 0x7FF8000000000000"),
            "malformed tag must not become a payload-zero NaN: {ir}"
        );
        assert_eq!(ir.matches("declare double @nan(ptr)").count(), 1, "{ir}");

        let irf = crate::compile_to_ir(
            "float f(void) { return __builtin_nanf(\"1xyz\"); }\n",
            "nanf-runtime.c",
        )
        .unwrap();
        assert!(irf.contains("declare float @nanf(ptr)"), "{irf}");
        assert!(irf.contains("call float @nanf(ptr"), "{irf}");

        let irl = crate::compile_to_ir(
            "long double f(void) { return __builtin_nanl(\"1xyz\"); }\n",
            "nanl-runtime.c",
        )
        .unwrap();
        assert!(irl.contains("declare x86_fp80 @nanl(ptr)"), "{irl}");
        assert!(irl.contains("call x86_fp80 @nanl(ptr"), "{irl}");

        // A valid tag still folds, including in a function body.
        let ok = crate::compile_to_ir(
            "double f(void) { return __builtin_nan(\"1\"); }\n",
            "nan-ok.c",
        )
        .unwrap();
        assert!(ok.contains("ret double 0x7FF8000000000001"), "{ok}");
        assert!(!ok.contains("@nan("), "{ok}");

        // Do not emit a second declare if the unit already names nan.
        let declared = crate::compile_to_ir(
            "double nan(const char *); double f(void) { return __builtin_nan(\"1xyz\"); }\n",
            "nan-declared.c",
        )
        .unwrap();
        assert_eq!(
            declared.matches("declare double @nan(ptr)").count(),
            1,
            "{declared}"
        );
        assert!(declared.contains("call double @nan(ptr"), "{declared}");

        // A static malformed tag is still rejected. So is a runtime
        // __builtin_nans* malformed tag (no library equivalent).
        let err = crate::compile_to_ir("double x = __builtin_nan(\"1xyz\");\n", "nan-static.c")
            .unwrap_err();
        assert!(
            err.message.contains("not a constant expression"),
            "{}",
            err.message
        );
        let err = crate::compile_to_ir(
            "double f(void) { return __builtin_nans(\"1xyz\"); }\n",
            "nans-runtime.c",
        )
        .unwrap_err();
        assert!(
            err.message.contains("strtol would consume in full"),
            "{}",
            err.message
        );
    }

    #[test]
    fn floating_comparison_builtins_use_quiet_fcmp() {
        // LLVM LangRef: fcmp is not signalling, and the `o` predicates are
        // false when either operand is a NaN while `uno` is true. That is
        // the C11 7.12.14 macro behaviour.
        let cases = [
            ("__builtin_isgreater", "ogt"),
            ("__builtin_isgreaterequal", "oge"),
            ("__builtin_isless", "olt"),
            ("__builtin_islessequal", "ole"),
            ("__builtin_islessgreater", "one"),
            ("__builtin_isunordered", "uno"),
        ];
        for (name, pred) in cases {
            let ir = crate::compile_to_ir(
                &format!("int f(double a, double b) {{ return {name}(a, b); }}\n"),
                "fpcmp.c",
            )
            .unwrap();
            assert!(ir.contains(&format!("fcmp {pred} double")), "{name}: {ir}");
            assert!(
                !ir.contains(&format!("@{name}")),
                "{name} is not a call: {ir}"
            );
        }
        // Usual arithmetic conversions: float and long double compare
        // as x86_fp80; int and float compare as float.
        let ir = crate::compile_to_ir(
            "int f(float a, long double b) { return __builtin_isless(a, b); }\n",
            "fpcmp-mixed.c",
        )
        .unwrap();
        assert!(ir.contains("fpext float"), "{ir}");
        assert!(ir.contains("fcmp olt x86_fp80"), "{ir}");
        let ir = crate::compile_to_ir(
            "int f(int a, float b) { return __builtin_isunordered(a, b); }\n",
            "fpcmp-int.c",
        )
        .unwrap();
        assert!(ir.contains("sitofp i32"), "{ir}");
        assert!(ir.contains("fcmp uno float"), "{ir}");

        let source = "\
int main(void) {
    volatile double n = __builtin_nan(\"\"), one = 1.0;
    volatile long double l = 2.0L;
    volatile float f = -0.0f;
    if (__builtin_isless(n, one) || __builtin_isgreater(one, n)) return 1;
    if (!__builtin_isunordered(n, l) || __builtin_islessgreater(n, f)) return 2;
    if (!__builtin_isgreaterequal(l, one) || !__builtin_islessequal(f, 0.0)) return 3;
    if (__builtin_islessgreater(f, 0) || !__builtin_islessgreater(l, one)) return 4;
    return __builtin_isgreater(l, f) + __builtin_isunordered(one, l) * 4;
}
";
        let ir = crate::compile_to_ir(source, "fpcmp-run.c").unwrap();
        assert_eq!(run_binary(&ir), run_reference(source));
        assert_eq!(run_binary(&ir).0, 1);
    }

    #[test]
    fn internal_object_named_like_a_hidden_libcall_is_renamed() {
        // LLVM IR has one namespace for global variables and functions.
        // A file-scope `static int nan;` is valid C11 (7.1.3 reserves
        // `nan` at file scope only with <math.h>), and its internal
        // linkage cannot meet the library at link time. When IR lowering
        // itself calls nan/nanf/nanl, the object gets a private symbol
        // so `@nan` names only the library function.
        let source = "\
static int nan;
double f(void) { return __builtin_nan(\"1xyz\"); }
int main(void) { double d = f(); return (d == d) + nan; }
";
        let ir = crate::compile_to_ir(source, "nan-clash.c").unwrap();
        assert!(!ir.contains("@nan = "), "object must not keep @nan: {ir}");
        assert!(ir.contains("declare double @nan(ptr)"), "{ir}");
        assert!(ir.contains("call double @nan(ptr"), "{ir}");
        assert_eq!(run_binary(&ir).0, 0, "{ir}");

        for (decl, call, ty) in [
            (
                "static float nanf = 2.0f;",
                "__builtin_nanf(\"x\")",
                "float",
            ),
            (
                "static long double nanl = 2.0L;",
                "__builtin_nanl(\"x\")",
                "long double",
            ),
        ] {
            let lib = if ty == "float" { "nanf" } else { "nanl" };
            let source = format!(
                "{decl}\n{ty} f(void) {{ return {call}; }}\n\
                 int main(void) {{ {ty} d = f(); return (d == d) + ({lib} != 2.0f); }}\n"
            );
            let ir = crate::compile_to_ir(&source, "nanf-clash.c").unwrap();
            assert!(!ir.contains(&format!("@{lib} = ")), "{ir}");
            assert!(
                ir.contains(&format!(
                    "call {} @{lib}(ptr",
                    if ty == "float" { "float" } else { "x86_fp80" }
                )),
                "{ir}"
            );
            assert_eq!(run_binary(&ir).0, 0, "{ir}");
        }

        // Every reference follows the renamed object: an address constant,
        // a block-scope extern that names the same object, and a pointer
        // initializer in a block-scope static.
        let source = "\
static int nan = 40;
static int *where = &nan;
int get(void) { extern int nan; return nan; }
double f(void) { return __builtin_nan(\"-1\"); }
int main(void) {
    static int *again = &nan;
    double d = f();
    if (d == d) return 1;
    *where += 1;
    return get() + *again - 82;
}
";
        let ir = crate::compile_to_ir(source, "nan-refs.c").unwrap();
        assert!(!ir.contains("@nan = "), "{ir}");
        assert_eq!(run_binary(&ir).0, 0, "{ir}");
    }

    #[test]
    fn no_rename_without_a_hidden_libcall_clash() {
        // A valid tag folds to a constant and makes no call.
        for tag in ["", "0x10"] {
            let ir = crate::compile_to_ir(
                &format!(
                    "static int nan = 3;\ndouble f(void) {{ return __builtin_nan(\"{tag}\"); }}\n\
                     int main(void) {{ return nan; }}\n"
                ),
                "nan-valid.c",
            )
            .unwrap();
            assert!(ir.contains("@nan = internal global i32 3"), "{ir}");
            assert!(!ir.contains("@nan("), "{ir}");
        }
        // No builtin at all.
        let ir = crate::compile_to_ir(
            "static int nan = 3;\nint main(void) { return nan; }\n",
            "nan-plain.c",
        )
        .unwrap();
        assert!(ir.contains("@nan = internal global i32 3"), "{ir}");
        // A block-scope static has its own symbol already; the library
        // call still names @nan.
        let source = "\
int main(void) {
    static int nan = 5;
    double d = __builtin_nan(\"1xyz\");
    return (d == d) + nan - 5;
}
";
        let ir = crate::compile_to_ir(source, "nan-block.c").unwrap();
        assert!(ir.contains("call double @nan(ptr"), "{ir}");
        assert!(ir.contains("@main.nan."), "{ir}");
        assert_eq!(run_binary(&ir).0, 0, "{ir}");
        // A user-defined static function named nan stays the call target.
        let source = "\
static double nan(const char *tag) { return tag[0] == '1' ? 42.0 : 0.0; }
int main(void) { return (int)__builtin_nan(\"1xyz\") - 42; }
";
        let ir = crate::compile_to_ir(source, "nan-user.c").unwrap();
        assert!(ir.contains("define internal double @nan(ptr"), "{ir}");
        assert!(!ir.contains("declare double @nan"), "{ir}");
        assert_eq!(run_binary(&ir).0, 0, "{ir}");
    }

    #[test]
    fn internal_names_of_memory_libcalls_are_renamed() {
        // llvm.memcpy, llvm.memmove, and llvm.memset may be lowered to
        // calls of the C library memcpy, memmove, and memset (LLVM
        // LangRef), and the optimizer may introduce those calls too. An
        // internal object or function with one of those names would
        // capture the call in the object file, so it gets a private name.
        let source = "\
static int memmove = 5;
static int memcpy = 6;
static int memset(int x) { return x + 1; }
struct Big { char b[4096]; };
struct Big g1, g2;
int main(void) {
    struct Big local;
    char buf[4096];
    __builtin_memset(local.b, 1, sizeof local.b);
    __builtin_memcpy(buf, local.b, sizeof buf);
    g1 = local;
    g2 = g1;
    return g2.b[100] + buf[4000] + memmove + memcpy + memset(-1) - 13;
}
";
        let ir = crate::compile_to_ir(source, "mem-clash.c").unwrap();
        for name in ["memmove", "memcpy"] {
            assert!(!ir.contains(&format!("@{name} = ")), "{name}: {ir}");
        }
        assert!(!ir.contains("@memset("), "{ir}");
        assert_eq!(run_binary(&ir).0, 0, "{ir}");
    }

    #[test]
    fn runtime_offsetof_has_no_inbounds_gep_from_null() {
        // A getelementptr inbounds from a null base with a nonzero offset
        // is poison (LLVM LangRef, "getelementptr"), so a subscript that
        // is not constant must not be lowered as an address from null.
        let source = "struct In { char t; short s[3]; };\n\
                      struct S { char c; int a[4]; struct In in[5]; };\n\
                      unsigned long f(int i, unsigned j) {\n\
                        return __builtin_offsetof(struct S, a[i])\n\
                             + __builtin_offsetof(struct S, in[j].s[i]);\n\
                      }\n\
                      int main(void) { return f(2, 3) == 62 ? 0 : 1; }\n";
        let ir = crate::compile_to_ir(source, "offsetof-runtime.c").unwrap();
        assert!(!ir.contains("ptr null"), "{ir}");
        assert_eq!(run_binary(&ir).0, 0, "{ir}");
    }
}
