//! Type spelling for diagnostics, compatibility and composite types
//! (C11 6.2.7), and the pointer-mix rules of the GNU dialect.

use super::*;

pub(super) fn render_type(ty: &CType, name: &str) -> String {
    match ty {
        CType::Qualified(quals, inner) => match inner.as_ref() {
            CType::Pointer(pointee) => render_type(pointee, &star_decl(pointee, *quals, name)),
            CType::Qualified(_, _) | CType::Array(_, _) => render_type(inner, name),
            CType::Void => join_spec(&format!("{}void", qual_prefix(*quals)), name),
            CType::Int(scalar) => {
                join_spec(&format!("{}{}", qual_prefix(*quals), scalar.as_str()), name)
            }
            CType::Float(kind) => {
                join_spec(&format!("{}{}", qual_prefix(*quals), kind.as_str()), name)
            }
            CType::Struct(handle) => join_spec(
                &format!("{}{}", qual_prefix(*quals), record_spelling(handle)),
                name,
            ),
            CType::Func(_) => render_type(inner, name),
        },
        CType::Void => join_spec("void", name),
        CType::Int(scalar) => join_spec(scalar.as_str(), name),
        CType::Float(kind) => join_spec(kind.as_str(), name),
        CType::Struct(handle) => join_spec(&record_spelling(handle), name),
        CType::Func(func) => {
            let applied = format!("{name}({})", render_params(func));
            render_type(&func.ret, &applied)
        }
        CType::Pointer(inner) => render_type(inner, &star_decl(inner, Quals::default(), name)),
        CType::Array(elem, len) => {
            let bracket = match len {
                ArrayLen::Const(n) => format!("[{n}]"),
                ArrayLen::Incomplete => "[]".to_string(),
                ArrayLen::Vla(_) | ArrayLen::Star => "[*]".to_string(),
            };
            render_type(elem, &format!("{name}{bracket}"))
        }
    }
}

pub(super) fn record_spelling(handle: &StructRef) -> String {
    let word = handle.kind().word();
    let tag = handle.tag();
    if tag.is_empty() {
        word.to_string()
    } else if tag == "_Float16" {
        // The parser's stand-in for `_Float16` (see `Parser::float16`).
        tag
    } else {
        format!("{word} {tag}")
    }
}

pub(super) fn render_params(func: &FuncTy) -> String {
    if !func.prototype {
        return String::new();
    }
    let mut parts: Vec<String> = func.params.iter().map(|ty| ty.display()).collect();
    if func.variadic {
        parts.push("...".to_string());
    }
    if parts.is_empty() {
        "void".to_string()
    } else {
        parts.join(", ")
    }
}

pub(super) fn qual_words(quals: Quals) -> Vec<&'static str> {
    let mut words = Vec::new();
    // `_Atomic` is written first, as in the standard's
    // `_Atomic volatile int` example (C11 6.7.3p13).
    if quals.is_atomic {
        words.push("_Atomic");
    }
    if quals.is_const {
        words.push("const");
    }
    if quals.is_volatile {
        words.push("volatile");
    }
    if quals.is_restrict {
        words.push("restrict");
    }
    words
}

pub(super) fn qual_prefix(quals: Quals) -> String {
    let words = qual_words(quals);
    if words.is_empty() {
        String::new()
    } else {
        format!("{} ", words.join(" "))
    }
}

/// Text after `*`: empty, or ` const`, ` volatile`, ` restrict`, in that order.
pub(super) fn qual_after_star(quals: Quals) -> String {
    let words = qual_words(quals);
    if words.is_empty() {
        String::new()
    } else {
        format!(" {}", words.join(" "))
    }
}

pub(super) fn star_decl(pointee: &CType, quals: Quals, name: &str) -> String {
    let q = qual_after_star(quals);
    let needs_paren = matches!(pointee.peel(), CType::Array(_, _) | CType::Func(_));
    if needs_paren {
        if q.is_empty() {
            format!("(*{name})")
        } else if name.is_empty() {
            format!("(*{q})")
        } else {
            format!("(*{q} {name})")
        }
    } else if name.is_empty() {
        if q.is_empty() {
            "*".to_string()
        } else {
            format!("*{q}")
        }
    } else if q.is_empty() {
        format!("*{name}")
    } else {
        format!("*{q} {name}")
    }
}

pub(super) fn join_spec(spec: &str, name: &str) -> String {
    if name.is_empty() {
        spec.to_string()
    } else {
        format!("{spec} {name}")
    }
}

/// Pointers to qualified or unqualified versions of compatible types
/// (C11 6.5.8, 6.5.9). One level of pointee qualifiers is ignored.
/// `const int *` may be compared with `int *`. `char **` may not be compared
/// with `const char **`, because `char *` and `const char *` are not
/// compatible. Semantic analysis and the parser's folding of constant
/// expressions share this rule.
pub fn loose_pointers(left: &CType, right: &CType) -> bool {
    match (left.pointee(), right.pointee()) {
        (Some(a), Some(b)) => compatible(&a.unqualified(), &b.unqualified()),
        _ => false,
    }
}

/// A pointer to an object type and a pointer to a qualified or unqualified
/// version of `void`, the other pointer case an equality operator allows
/// (C11 6.5.9p2). A pointer to a function does not qualify; a null pointer
/// constant is handled separately.
pub fn void_ptr_mix(left: &CType, right: &CType) -> bool {
    let object_pointer = |ty: &CType| ty.pointee().is_some_and(|pointee| !pointee.is_func());
    object_pointer(left)
        && object_pointer(right)
        && (left.is_void_pointer() || right.is_void_pointer())
}

/// A void pointer and a pointer to a function, in either order. C11 6.5.9p2
/// does not allow them as operands of an equality operator; `-std=gnu11`
/// accepts them as an extension. A null pointer constant is handled
/// separately and is valid in both modes.
pub fn void_function_ptr_mix(left: &CType, right: &CType) -> bool {
    let function_pointer = |ty: &CType| ty.pointee().is_some_and(CType::is_func);
    (left.is_void_pointer() && function_pointer(right))
        || (right.is_void_pointer() && function_pointer(left))
}

/// Two pointers that a GNU-mode comparison accepts with a warning when C11
/// does not (C11 6.5.8p2, 6.5.9p2 require compatible pointees). GCC and
/// Clang, used as black-box oracles, compare any two pointers for equality
/// and any two object pointers relationally in GNU mode. The caller keeps
/// its own rule for relational comparison of function pointers.
pub fn gnu_distinct_pointers(left: &CType, right: &CType) -> bool {
    left.is_pointer() && right.is_pointer()
}

/// A pointer assignment that C11 6.5.16.1p1 does not allow but that GCC
/// and Clang accept with a warning in GNU mode: the pointees differ only in
/// the signedness of an integer type (`int *` from `unsigned *`, or plain
/// `char` against `signed char` and `unsigned char`), or the destination
/// pointee lacks a qualifier of the source pointee (`int *` from
/// `const int *`, `void *` from `const int *`), or both. `long *` from
/// `long long *` is not covered: the types differ in more than sign. Only the first
/// level of pointee is relaxed: `int **` from `unsigned **` stays an error,
/// as it is in GCC 14. Function pointers are not covered.
pub fn gnu_pointer_sign_or_qual_mix(dest: &CType, src: &CType) -> bool {
    let (Some(dp), Some(sp)) = (dest.pointee(), src.pointee()) else {
        return false;
    };
    if dp.is_func() || sp.is_func() {
        return false;
    }
    let (du, su) = (dp.unqualified(), sp.unqualified());
    if compatible(&du, &su) || du.is_void() || su.is_void() {
        return true;
    }
    match (du.as_integer(), su.as_integer()) {
        // Signedness is the only difference allowed: the same rank (C11
        // 6.3.1.1) names a signed type and its unsigned counterpart, and
        // plain `char` shares the rank of both character types. `long` and
        // `long long` have the same size but different ranks, so they stay
        // incompatible.
        (Some(a), Some(b)) => a != TypeName::Bool && b != TypeName::Bool && a.rank() == b.rank(),
        _ => false,
    }
}

/// Array lengths are compatible when they are not both integer constant
/// expressions of unequal value (C11 6.7.6.2p6). A missing, `*`, or
/// variable size does not make the types incompatible.
fn array_lens_compatible(left: ArrayLen, right: ArrayLen) -> bool {
    match (left, right) {
        (ArrayLen::Const(a), ArrayLen::Const(b)) => a == b,
        _ => true,
    }
}

/// Composite array length of two compatible array types (C11 6.2.7p3).
/// A known constant size wins; otherwise a specified VLA size; otherwise
/// `[*]`; otherwise an incomplete array.
fn composite_array_len(left: ArrayLen, right: ArrayLen) -> ArrayLen {
    match (left, right) {
        (ArrayLen::Const(n), _) | (_, ArrayLen::Const(n)) => ArrayLen::Const(n),
        (ArrayLen::Vla(id), _) | (_, ArrayLen::Vla(id)) => ArrayLen::Vla(id),
        (ArrayLen::Star, _) | (_, ArrayLen::Star) => ArrayLen::Star,
        (ArrayLen::Incomplete, ArrayLen::Incomplete) => ArrayLen::Incomplete,
    }
}

/// Two types are compatible (C11 6.2.7, 6.7.6, 6.7.3).
///
/// Qualifiers must match at every level. Arrays are compatible when their
/// elements are and the lengths are equal or either length is unknown. The
/// composite type keeps a known length. Function parameters strip top-level
/// qualifiers before this check (C11 6.7.6.3p15); that happens at the call
/// site, not here.
pub fn compatible(left: &CType, right: &CType) -> bool {
    if left.quals() != right.quals() {
        return false;
    }
    match (left.peel(), right.peel()) {
        (CType::Void, CType::Void) => true,
        (CType::Int(a), CType::Int(b)) => a == b,
        (CType::Float(a), CType::Float(b)) => a == b,
        (CType::Pointer(a), CType::Pointer(b)) => compatible(a, b),
        (CType::Func(a), CType::Func(b)) => compatible_funcs(a, b),
        (CType::Array(a, la), CType::Array(b, lb)) => {
            array_lens_compatible(*la, *lb) && compatible(a, b)
        }
        (CType::Struct(a), CType::Struct(b)) => a == b,
        _ => false,
    }
}

/// Function type compatibility (C11 6.7.6.3p15).
///
/// Two prototypes agree in the number of parameters, in `...`, and in each
/// parameter type. A type without a prototype (`()`) is compatible with a
/// prototype that has no `...` and whose parameter types are each compatible
/// with their default argument promotion, so `int f();` matches
/// `int f(int)` and `int f(double)` but not `int f(char)` or `int f(float)`.
/// Two types without prototypes need only compatible return types.
pub fn compatible_funcs(a: &FuncTy, b: &FuncTy) -> bool {
    if !compatible(&a.ret, &b.ret) {
        return false;
    }
    match (a.prototype, b.prototype) {
        (true, true) => {
            a.variadic == b.variadic
                && a.params.len() == b.params.len()
                && a.params
                    .iter()
                    .zip(b.params.iter())
                    .all(|(left, right)| compatible(left, right))
        }
        (false, false) => true,
        (true, false) => prototype_matches_unprototyped(a, b),
        (false, true) => prototype_matches_unprototyped(b, a),
    }
}

/// A prototype compared with a type that has no parameter type list
/// (C11 6.7.6.3p15). An empty parameter list is a declaration `int f();`
/// or a `f()` definition: the prototype must have no `...` and every
/// parameter must survive the default argument promotions. A non-empty
/// list is a K&R definition's identifier types: the counts must agree and
/// each prototype parameter must be compatible with the promoted identifier
/// type.
fn prototype_matches_unprototyped(proto: &FuncTy, other: &FuncTy) -> bool {
    if other.params.is_empty() {
        prototype_matches_promotions(proto)
    } else {
        !proto.variadic
            && proto.params.len() == other.params.len()
            && proto
                .params
                .iter()
                .zip(other.params.iter())
                .all(|(left, right)| compatible(left, &default_promoted(right)))
    }
}

/// A prototype is compatible with a non-prototype function type that has no
/// identifier types only if it has no `...` and every parameter survives
/// the default argument promotions unchanged (C11 6.7.6.3p15, 6.5.2.2p6).
pub fn prototype_matches_promotions(func: &FuncTy) -> bool {
    !func.variadic
        && func
            .params
            .iter()
            .all(|param| compatible(param, &default_promoted(param)))
}

/// The default argument promotions (C11 6.5.2.2p6): the integer promotions,
/// and `float` to `double`. Other types are unchanged, without qualifiers.
pub fn default_promoted(ty: &CType) -> CType {
    match ty.peel() {
        CType::Int(name) => CType::Int(name.promote()),
        CType::Float(FloatKind::Float) => CType::Float(FloatKind::Double),
        other => other.clone(),
    }
}

/// Composite type of two compatible types. A known array length wins.
pub fn composite(left: &CType, right: &CType) -> CType {
    let base = match (left.peel(), right.peel()) {
        (CType::Func(a), CType::Func(b)) => {
            // A prototype wins over a type without one (C11 6.2.7p3).
            let (proto, other) = if a.prototype || !b.prototype {
                (a, b)
            } else {
                (b, a)
            };
            let params = if proto.prototype && other.prototype {
                proto
                    .params
                    .iter()
                    .zip(other.params.iter())
                    .map(|(left, right)| composite(left, right))
                    .collect()
            } else {
                proto.params.clone()
            };
            CType::func_with(
                composite(&a.ret, &b.ret),
                params,
                proto.variadic,
                proto.prototype,
            )
        }
        (CType::Array(a, la), CType::Array(b, lb)) => {
            CType::array_of(composite(a, b), composite_array_len(*la, *lb))
        }
        (CType::Pointer(a), CType::Pointer(b)) => CType::pointer(composite(a, b)),
        _ => left.peel().clone(),
    };
    base.with_quals(left.quals())
}

/// `?:` on two pointers (C11 6.5.15p3, p6), shared by semantic analysis and
/// the parser's typing of operands that are not evaluated. The result
/// pointee carries every qualifier from either side. A `void *` arm with an
/// object pointer produces `void *` with those qualifiers; with a function
/// pointer it is not allowed. Compatible pointees give their composite.
pub fn merge_pointers(left: &CType, right: &CType) -> Option<CType> {
    let lp = left.pointee()?;
    let rp = right.pointee()?;
    let quals = lp.quals().merge(rp.quals());
    if (lp.unqualified().is_void() && !rp.is_func())
        || (rp.unqualified().is_void() && !lp.is_func())
    {
        return Some(CType::pointer(CType::Void.with_quals(quals)));
    }
    if compatible(&lp.unqualified(), &rp.unqualified()) {
        let merged = composite(&lp.unqualified(), &rp.unqualified()).with_quals(quals);
        return Some(CType::pointer(merged));
    }
    None
}
