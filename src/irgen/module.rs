//! Module-level output: the module header, named struct types, globals,
//! string constants, declarations, function signatures and attributes,
//! library-name renames, and the per-function driver.

use super::*;

pub(super) fn render_module(unit: &TranslationUnit, filename: &str) -> Result<String, Ice> {
    let mut out = String::new();
    out.push_str(&format!("source_filename = {}\n", llvm_string(filename)));
    let target = crate::target::current();
    out.push_str(&format!(
        "target datalayout = \"{}\"\n",
        target.data_layout()
    ));
    out.push_str(&format!("target triple = \"{}\"\n\n", target.triple()));
    let mut wrote_prelude = render_types_and_strings(unit, &mut out);
    // Function bodies are rendered first because they decide which library
    // functions IR lowering calls. An internal object that shares such a
    // name is then given a private symbol and the bodies are rendered again.
    let (mut functions, mut needs) = render_functions(unit, &Renames::new())?;
    let renames = library_renames(unit, &needs);
    if !renames.is_empty() {
        (functions, needs) = render_functions(unit, &renames)?;
    }
    for global in &unit.globals {
        out.push_str(&render_global(global, &renames)?);
        out.push('\n');
        wrote_prelude = true;
    }
    for decl in &unit.func_decls {
        out.push_str(&render_declare(decl, unit.fn_attrs.get(&decl.name)));
        out.push('\n');
        wrote_prelude = true;
    }
    wrote_prelude |= render_lowering_declares(unit, &needs, &mut out);
    wrote_prelude |= render_special_arrays(unit, &renames, &mut out);
    if wrote_prelude {
        out.push('\n');
    }
    for text in functions {
        out.push_str(&text);
        out.push_str("\n\n");
    }
    Ok(out)
}

/// Named struct types (opaque for an incomplete type a global uses) and
/// string constants. Returns whether anything was written.
fn render_types_and_strings(unit: &TranslationUnit, out: &mut String) -> bool {
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
    wrote_prelude
}

/// Declarations of the intrinsics and library functions that lowering
/// calls. Returns whether anything was written.
fn render_lowering_declares(unit: &TranslationUnit, needs: &LowerNeeds, out: &mut String) -> bool {
    let mut wrote_prelude = false;
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
    wrote_prelude
}

/// `@llvm.used`, `@llvm.global_ctors` and `@llvm.global_dtors` for the
/// `used`, `constructor` and `destructor` attributes. Returns whether
/// anything was written.
fn render_special_arrays(unit: &TranslationUnit, renames: &Renames, out: &mut String) -> bool {
    let mut wrote_prelude = false;
    let used: Vec<String> = unit
        .globals
        .iter()
        .filter(|global| global.attrs.used && global.init.is_some())
        .map(|global| llvm_global(renamed(&global.name, renames)))
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
                .map(|func| llvm_global(renamed(&func.symbol, renames))),
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
                    llvm_global(renamed(&func.symbol, renames))
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
    wrote_prelude
}

/// LLVM symbol replacements: C linker name to the private name used in IR.
pub(super) type Renames = std::collections::HashMap<String, String>;

/// Library functions that IR lowering calls although the source does not
/// name them. LLVM may lower `llvm.memcpy`, `llvm.memmove`, and
/// `llvm.memset` to calls of the C library functions, and its optimizer
/// may introduce such calls too (LLVM LangRef), so those three are always
/// reserved. `nan`, `nanf`, and `nanl` are called for a malformed quiet
/// `__builtin_nan*` tag.
pub(super) const MEMORY_LIBCALLS: [&str; 3] = ["memcpy", "memmove", "memset"];

pub(super) fn render_functions(
    unit: &TranslationUnit,
    renames: &Renames,
) -> Result<(Vec<String>, LowerNeeds), Ice> {
    let mut functions = Vec::new();
    let mut needs = LowerNeeds::default();
    for decl in &unit.decls {
        if let TopDecl::Func(func) = decl {
            if func.body.is_some() {
                let (text, fn_needs) = render_function(
                    func,
                    renames,
                    unit.fn_attrs.get(&func.symbol),
                    &unit.vla_sizes,
                )?;
                needs.merge(fn_needs);
                functions.push(text);
            }
        }
    }
    Ok((functions, needs))
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
pub(super) fn library_renames(unit: &TranslationUnit, needs: &LowerNeeds) -> Renames {
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
pub(super) fn renamed<'a>(name: &'a str, renames: &'a Renames) -> &'a str {
    renames.get(name).map_or(name, String::as_str)
}

pub(super) fn render_global(global: &IrGlobal, renames: &Renames) -> Result<String, Ice> {
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
    let tls = if global.attrs.thread_local {
        "thread_local "
    } else {
        ""
    };
    if let Some(value) = &global.init {
        let selectany = global.linkage == Linkage::External && global.attrs.selectany;
        let linkage = match global.linkage {
            Linkage::Internal => "internal ",
            Linkage::External if selectany => "weak_odr ",
            Linkage::External if global.attrs.weak => "weak ",
            Linkage::External => "",
        };
        // An address inside a union or bit-field image needs a constant of
        // its own shape. The global then has that type instead of `ir`;
        // every access goes through an opaque `ptr`, so only the size and
        // alignment have to agree.
        let (ir, spelled) = render_typed_const(&global.ty, value, renames)?;
        if selectany {
            // Clang (black box) puts a selectany object in a COMDAT of its
            // own name.
            return Ok(format!(
                "${} = comdat any\n{name} = {linkage}{tls}{kind} {ir} {spelled}{section}, comdat, align {align}",
                &name[1..]
            ));
        }
        Ok(format!(
            "{name} = {linkage}{tls}{kind} {ir} {spelled}{section}, align {align}"
        ))
    } else if global.attrs.weak {
        Ok(format!(
            "{name} = extern_weak {tls}{kind} {ir}, align {align}"
        ))
    } else {
        Ok(format!("{name} = external {tls}{kind} {ir}, align {align}"))
    }
}

pub(super) fn render_struct(handle: &StructRef) -> String {
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

pub(super) fn render_string(literal: &StrConst) -> String {
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

pub(super) fn render_declare(decl: &IrFunc, attrs: Option<&FnAttrs>) -> String {
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
pub(super) fn fn_attr_suffix(attrs: Option<&FnAttrs>, definition: bool) -> String {
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
pub(super) fn sig_params(sig: &FnSig, variadic: bool, names: Option<&[Vec<String>]>) -> String {
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

pub(super) fn param_ir(ty: &CType, name: Option<&str>) -> String {
    let ir = ty.ir_type();
    match (ty.abi_attr(), name) {
        (Some(attr), Some(name)) => format!("{ir} {attr} {name}"),
        (Some(attr), None) => format!("{ir} {attr}"),
        (None, Some(name)) => format!("{ir} {name}"),
        (None, None) => ir,
    }
}

pub(super) fn render_function(
    func: &FuncDef,
    renames: &Renames,
    attrs: Option<&FnAttrs>,
    vla_sizes: &[VlaBound],
) -> Result<(String, LowerNeeds), Ice> {
    let ice = |message: &str| Ice::new(func.line, func.column, message);
    let body = func
        .body
        .as_ref()
        .ok_or_else(|| ice("function has no body"))?;
    // A K&R definition receives default-argument-promoted values (C11 6.9.1,
    // 6.5.2.2). The LLVM signature uses those promoted types; the body
    // converts each into the declared type.
    let param_tys: Vec<CType> = if func.kr_def {
        func.params
            .iter()
            .map(|param| default_promoted(&param.ty))
            .collect()
    } else {
        func.params.iter().map(|param| param.ty.clone()).collect()
    };
    // Semantic analysis rejects a definition whose aggregate needs an SSE
    // register or is still incomplete, so the signature is known here.
    let sig = function_sig(&func.ret_type, &param_tys)
        .map_err(|_| ice("defined function has no calling-convention signature"))?;
    let mut names: Vec<Vec<String>> = sig
        .params
        .iter()
        .enumerate()
        .map(|(index, group)| arg_names(index, group.len()))
        .collect();
    let mut gen = Gen::new(func.ret_type.clone(), sig.ret_agg.clone());
    gen.renames = renames.clone();
    gen.addr_label_names = func.addr_labels.clone();
    gen.addr_labels = func
        .addr_labels
        .iter()
        .map(|name| c_label_via_ptr(name))
        .collect();
    gen.vla_bounds = vla_sizes
        .iter()
        .map(|bound| (bound.id, bound.expr.clone()))
        .collect();
    let (depths, max_vla_depth) = label_vla_depths(body);
    gen.label_vla_depths = depths;
    // MSVC gives every C `inline` definition external linkage in a COMDAT
    // that the linker folds (Parser::mark_inline_definitions), so a call
    // that is not inlined still finds a body.
    let msvc_comdat = attrs
        .and_then(|attrs| attrs.msvc_comdat)
        .filter(|_| func.linkage == Linkage::External);
    let msvc_linkage = msvc_comdat.map(|linkage| format!("{linkage} "));
    let linkage = match func.linkage {
        Linkage::Internal => "internal ",
        Linkage::External if msvc_linkage.is_some() => msvc_linkage.as_deref().unwrap_or(""),
        Linkage::External if attrs.is_some_and(|attrs| attrs.available_externally) => {
            "available_externally "
        }
        Linkage::External if attrs.is_some_and(|attrs| attrs.weak) => "weak ",
        Linkage::External => "",
    };
    let params = sig_params(&sig, func.variadic, Some(&names));
    let global = llvm_global(renamed(&func.symbol, renames));
    let comdat = if msvc_comdat.is_some() {
        gen.emit(&format!("${} = comdat any", &global[1..]));
        " comdat"
    } else {
        ""
    };
    gen.emit(&format!(
        "define {linkage}{}{} {global}({params}){}{comdat} {{",
        sig.ret_prefix,
        sig.ret_type,
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
    // Computed goto may land on a label emitted before a later VLA was
    // saved, so stacksave results live in entry-block slots that every
    // trampoline can load.
    if max_vla_depth > 0 && !func.addr_labels.is_empty() {
        gen.declare_stack_intrinsics();
        for index in 0..max_vla_depth {
            let slot = format!("%vla.save.{index}");
            gen.emit(&format!("  {slot} = alloca ptr, align 8"));
            gen.vla_slots.push(slot);
        }
        gen.vla_depth_slot = Some("%vla.cur".to_string());
        gen.emit("  %vla.cur = alloca i32, align 4");
        gen.emit("  store i32 0, ptr %vla.cur");
    }
    // A computed goto can list a label after a VLA as a destination of an
    // earlier `indirectbr`, so the SSA size at the declaration would not
    // dominate that use. Entry-block slots do.
    if !func.addr_labels.is_empty() {
        for bound in vla_sizes {
            let slot = format!("%vla.bytes.{}", bound.id.0);
            gen.emit(&format!("  {slot} = alloca i64, align 8"));
            gen.emit(&format!("  store i64 0, ptr {slot}"));
            gen.vla_byte_slots.insert(bound.id, slot);
        }
    }
    // Temps allocated while the body is emitted are inserted here, still in
    // the entry block, so a call inside a loop does not grow the stack.
    gen.alloca_at = gen.lines.len();
    if func.kr_def {
        for ((param, name_group), parts) in func
            .params
            .iter()
            .zip(names.iter_mut())
            .zip(sig.params.iter())
        {
            if parts.len() == 1 && !name_group.is_empty() {
                let incoming = default_promoted(&param.ty);
                if incoming != param.ty.unqualified() {
                    let converted = gen.convert(&name_group[0], &incoming, &param.ty);
                    name_group[0] = converted;
                }
            }
        }
    }
    for ((param, names), parts) in func.params.iter().zip(names.iter()).zip(sig.params.iter()) {
        gen.store_param(param, names, parts);
    }
    for param in &func.params {
        gen.force_eval_vla_sizes(&param.unadjusted);
    }

    gen.pos = (func.line, func.column);
    gen.gen_compound(body)?;
    if !gen.terminated {
        gen.emit_implicit_return();
    }
    gen.emit_addr_label_trampolines();
    if let Some(ice) = gen.ice.take() {
        return Err(ice);
    }
    gen.emit("}");
    Ok((
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
    ))
}

pub(super) fn local_slots(compound: &Compound) -> Vec<(String, CType, u32)> {
    let mut slots = Vec::new();
    for item in &compound.items {
        collect_slots(item, &mut slots);
    }
    slots
}

pub(super) fn collect_slots(stmt: &Stmt, slots: &mut Vec<(String, CType, u32)>) {
    match stmt {
        Stmt::Decl(decl) if decl.storage == Storage::None && !decl.ty.is_vla() => {
            slots.push(slot_of(decl))
        }
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
                    if decl.storage == Storage::None && !decl.ty.is_vla() {
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
pub(super) fn slot_of(decl: &VarDecl) -> (String, CType, u32) {
    let align = decl.ty.align().max(decl.align.unwrap_or(0));
    (decl.slot.clone(), decl.ty.clone(), align)
}
