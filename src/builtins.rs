//! GNU builtin functions beyond the handful the C11 headers need.
//!
//! Written from the public GCC manual ("Other Built-in Functions Provided
//! by GCC" and "Built-in Functions to Perform Arithmetic with Overflow
//! Checking"). Three kinds are recognized:
//!
//! * library forwarders: `__builtin_strlen` and friends call the C library
//!   function of the same name with that function's ISO C prototype;
//! * operations lowered inline to LLVM IR (bit counting, byte swapping,
//!   `__builtin_expect`, `__builtin_trap`, `__builtin_alloca`, and the
//!   overflow-checking arithmetic);
//! * builtins folded by the parser (`__builtin_constant_p`,
//!   `__builtin_types_compatible_p`, `__builtin_choose_expr`), which are not
//!   in this table.

use crate::ast::{CType, FloatKind, Quals, TypeName};

/// The operation of an overflow-checking arithmetic builtin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowOp {
    Add,
    Sub,
    Mul,
}

/// An inline-lowered builtin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inline {
    /// Count leading zero bits of an unsigned operand.
    Clz(TypeName),
    /// Count trailing zero bits.
    Ctz(TypeName),
    Popcount(TypeName),
    Parity(TypeName),
    /// One plus the index of the least significant set bit, or 0.
    Ffs(TypeName),
    /// Leading redundant sign bits of a signed operand.
    Clrsb(TypeName),
    /// Reverse the bytes of a 16-, 32-, or 64-bit unsigned value.
    Bswap(TypeName),
    Expect,
    Unreachable,
    Trap,
    Alloca,
    Prefetch,
    AssumeAligned,
    ReturnAddress,
    FrameAddress,
    IsNan,
    IsInf,
    IsFinite,
    IsNormal,
    /// `__builtin_add_overflow` and friends. `fixed` is the operand and
    /// result type of a typed spelling such as `__builtin_sadd_overflow`;
    /// `None` is the type-generic spelling.
    Overflow {
        op: OverflowOp,
        fixed: Option<TypeName>,
    },
}

impl Inline {
    /// Recognize an inline builtin by its full spelling.
    pub fn from_name(name: &str) -> Option<Inline> {
        let rest = name.strip_prefix("__builtin_")?;
        let suffixed = |base: &str| -> Option<TypeName> {
            let tail = rest.strip_prefix(base)?;
            match tail {
                "" => Some(TypeName::UInt),
                "l" => Some(TypeName::ULong),
                "ll" => Some(TypeName::ULongLong),
                _ => None,
            }
        };
        let signed = |ty: TypeName| match ty {
            TypeName::UInt => TypeName::Int,
            TypeName::ULong => TypeName::Long,
            _ => TypeName::LongLong,
        };
        if let Some(ty) = suffixed("clz") {
            return Some(Inline::Clz(ty));
        }
        if let Some(ty) = suffixed("ctz") {
            return Some(Inline::Ctz(ty));
        }
        if let Some(ty) = suffixed("popcount") {
            return Some(Inline::Popcount(ty));
        }
        if let Some(ty) = suffixed("parity") {
            return Some(Inline::Parity(ty));
        }
        if let Some(ty) = suffixed("ffs") {
            return Some(Inline::Ffs(signed(ty)));
        }
        if let Some(ty) = suffixed("clrsb") {
            return Some(Inline::Clrsb(signed(ty)));
        }
        let overflow = |op: OverflowOp, base: &str| -> Option<Inline> {
            if rest == format!("{base}_overflow") {
                return Some(Inline::Overflow { op, fixed: None });
            }
            for (sign, suffix, ty) in [
                ("s", "", TypeName::Int),
                ("s", "l", TypeName::Long),
                ("s", "ll", TypeName::LongLong),
                ("u", "", TypeName::UInt),
                ("u", "l", TypeName::ULong),
                ("u", "ll", TypeName::ULongLong),
            ] {
                if rest == format!("{sign}{base}{suffix}_overflow") {
                    return Some(Inline::Overflow {
                        op,
                        fixed: Some(ty),
                    });
                }
            }
            None
        };
        if let Some(found) = overflow(OverflowOp::Add, "add")
            .or_else(|| overflow(OverflowOp::Sub, "sub"))
            .or_else(|| overflow(OverflowOp::Mul, "mul"))
        {
            return Some(found);
        }
        Some(match rest {
            "bswap16" => Inline::Bswap(TypeName::UShort),
            "bswap32" => Inline::Bswap(TypeName::UInt),
            "bswap64" => Inline::Bswap(TypeName::ULong),
            "expect" => Inline::Expect,
            "unreachable" => Inline::Unreachable,
            "trap" => Inline::Trap,
            "alloca" => Inline::Alloca,
            "prefetch" => Inline::Prefetch,
            "assume_aligned" => Inline::AssumeAligned,
            "return_address" => Inline::ReturnAddress,
            "frame_address" => Inline::FrameAddress,
            "isnan" | "isnanf" | "isnanl" => Inline::IsNan,
            "isinf" | "isinff" | "isinfl" => Inline::IsInf,
            "isfinite" | "finite" | "finitef" | "finitel" => Inline::IsFinite,
            "isnormal" => Inline::IsNormal,
            _ => return None,
        })
    }

    /// The number of arguments, as a range `(min, max)`.
    pub fn arity(self) -> (usize, usize) {
        match self {
            Inline::Unreachable | Inline::Trap => (0, 0),
            Inline::Expect => (2, 2),
            Inline::Prefetch => (1, 3),
            Inline::AssumeAligned => (2, 3),
            Inline::Overflow { .. } => (3, 3),
            _ => (1, 1),
        }
    }

    /// The result type, when it does not depend on the arguments.
    pub fn result_type(self) -> CType {
        match self {
            Inline::Clz(_)
            | Inline::Ctz(_)
            | Inline::Popcount(_)
            | Inline::Parity(_)
            | Inline::Ffs(_)
            | Inline::Clrsb(_)
            | Inline::IsNan
            | Inline::IsInf
            | Inline::IsFinite
            | Inline::IsNormal => CType::int_type(),
            Inline::Bswap(ty) => CType::Int(ty),
            Inline::Expect => CType::Int(TypeName::Long),
            Inline::Unreachable | Inline::Trap | Inline::Prefetch => CType::Void,
            Inline::Alloca
            | Inline::AssumeAligned
            | Inline::ReturnAddress
            | Inline::FrameAddress => CType::pointer(CType::Void),
            Inline::Overflow { .. } => CType::bool_type(),
        }
    }

    /// The type each argument converts to, where there is one.
    pub fn param_type(self, index: usize) -> Option<CType> {
        match self {
            Inline::Clz(ty)
            | Inline::Ctz(ty)
            | Inline::Popcount(ty)
            | Inline::Parity(ty)
            | Inline::Ffs(ty)
            | Inline::Clrsb(ty)
            | Inline::Bswap(ty) => Some(CType::Int(ty)),
            Inline::Expect => Some(CType::Int(TypeName::Long)),
            Inline::Alloca => Some(CType::Int(TypeName::ULong)),
            Inline::ReturnAddress | Inline::FrameAddress => Some(CType::Int(TypeName::UInt)),
            Inline::AssumeAligned => Some(if index == 0 {
                const_void_ptr()
            } else {
                CType::Int(TypeName::ULong)
            }),
            Inline::Prefetch => Some(if index == 0 {
                const_void_ptr()
            } else {
                CType::int_type()
            }),
            Inline::Overflow {
                fixed: Some(ty), ..
            } => Some(if index == 2 {
                CType::pointer(CType::Int(ty))
            } else {
                CType::Int(ty)
            }),
            _ => None,
        }
    }

    /// Fold a bit operation on a constant operand. `bits` is the operand
    /// already converted to the parameter type. `None` when the result is
    /// undefined (a zero operand of `clz` or `ctz`) or the builtin is not a
    /// bit operation.
    pub fn fold(self, bits: u64) -> Option<u64> {
        let width = |ty: TypeName| ty.width();
        let mask = |ty: TypeName| ty.mask() as u64;
        Some(match self {
            Inline::Clz(ty) => {
                let value = bits & mask(ty);
                if value == 0 {
                    return None;
                }
                u64::from(value.leading_zeros()) - (64 - u64::from(width(ty)))
            }
            Inline::Ctz(ty) => {
                let value = bits & mask(ty);
                if value == 0 {
                    return None;
                }
                u64::from(value.trailing_zeros())
            }
            Inline::Popcount(ty) => u64::from((bits & mask(ty)).count_ones()),
            Inline::Parity(ty) => u64::from((bits & mask(ty)).count_ones() & 1),
            Inline::Ffs(ty) => {
                let value = bits & mask(ty);
                if value == 0 {
                    0
                } else {
                    u64::from(value.trailing_zeros()) + 1
                }
            }
            Inline::Clrsb(ty) => {
                let w = width(ty);
                let value = bits & mask(ty);
                let sign = if (value >> (w - 1)) & 1 == 1 {
                    mask(ty)
                } else {
                    0
                };
                let x = (value ^ sign) & mask(ty);
                let lz = u64::from(x.leading_zeros()) - (64 - u64::from(w));
                lz - 1
            }
            Inline::Bswap(ty) => {
                let w = width(ty);
                (bits & mask(ty)).swap_bytes() >> (64 - w)
            }
            _ => return None,
        })
    }
}

fn const_void_ptr() -> CType {
    CType::pointer(CType::Void.with_quals(Quals {
        is_const: true,
        ..Quals::default()
    }))
}

/// A C library function that `__builtin_NAME` forwards to.
#[derive(Debug, Clone)]
pub struct LibSig {
    pub name: String,
    pub ret: CType,
    pub params: Vec<CType>,
    pub variadic: bool,
}

/// Library functions, each as (name, return, parameters). A parameter
/// list ending in `.` is variadic. Codes: `v` void, `i` int, `u` unsigned
/// int, `l` long, `x` long long, `z` size_t, `d` double, `f` float, `D`
/// long double, `F` the function's floating type (see `MATH`), `p` void
/// pointer, `P` pointer to const void, `s` char pointer, `S` pointer to
/// const char, `I` int pointer.
const LIBRARY: &[(&str, &str, &str)] = &[
    ("abort", "v", ""),
    ("exit", "v", "i"),
    ("_exit", "v", "i"),
    ("_Exit", "v", "i"),
    ("abs", "i", "i"),
    ("labs", "l", "l"),
    ("llabs", "x", "x"),
    ("imaxabs", "l", "l"),
    ("malloc", "p", "z"),
    ("calloc", "p", "zz"),
    ("realloc", "p", "pz"),
    ("free", "v", "p"),
    ("printf", "i", "S."),
    ("sprintf", "i", "sS."),
    ("snprintf", "i", "szS."),
    ("fprintf", "i", "pS."),
    ("vprintf", "i", "Sp"),
    ("vsprintf", "i", "sSp"),
    ("vsnprintf", "i", "szSp"),
    ("vfprintf", "i", "pSp"),
    ("scanf", "i", "S."),
    ("sscanf", "i", "SS."),
    ("fscanf", "i", "pS."),
    ("puts", "i", "S"),
    ("putchar", "i", "i"),
    ("fputs", "i", "Sp"),
    ("fputc", "i", "ip"),
    ("putc", "i", "ip"),
    ("fwrite", "z", "Pzzp"),
    ("memcmp", "i", "PPz"),
    ("memchr", "p", "Piz"),
    ("bcmp", "i", "PPz"),
    ("bzero", "v", "pz"),
    ("mempcpy", "p", "pPz"),
    ("strlen", "z", "S"),
    ("strnlen", "z", "Sz"),
    ("strcmp", "i", "SS"),
    ("strncmp", "i", "SSz"),
    ("strcasecmp", "i", "SS"),
    ("strncasecmp", "i", "SSz"),
    ("strcpy", "s", "sS"),
    ("strncpy", "s", "sSz"),
    ("stpcpy", "s", "sS"),
    ("stpncpy", "s", "sSz"),
    ("strcat", "s", "sS"),
    ("strncat", "s", "sSz"),
    ("strchr", "s", "Si"),
    ("strrchr", "s", "Si"),
    ("index", "s", "Si"),
    ("rindex", "s", "Si"),
    ("strstr", "s", "SS"),
    ("strpbrk", "s", "SS"),
    ("strspn", "z", "SS"),
    ("strcspn", "z", "SS"),
    ("strdup", "s", "S"),
    ("strndup", "s", "Sz"),
    ("isalnum", "i", "i"),
    ("isalpha", "i", "i"),
    ("isascii", "i", "i"),
    ("isblank", "i", "i"),
    ("iscntrl", "i", "i"),
    ("isdigit", "i", "i"),
    ("isgraph", "i", "i"),
    ("islower", "i", "i"),
    ("isprint", "i", "i"),
    ("ispunct", "i", "i"),
    ("isspace", "i", "i"),
    ("isupper", "i", "i"),
    ("isxdigit", "i", "i"),
    ("tolower", "i", "i"),
    ("toupper", "i", "i"),
    ("toascii", "i", "i"),
];

/// Math functions with `f` and `l` variants: (name, return, parameters),
/// with `F` standing for the variant's floating type.
const MATH: &[(&str, &str, &str)] = &[
    ("acos", "F", "F"),
    ("acosh", "F", "F"),
    ("asin", "F", "F"),
    ("asinh", "F", "F"),
    ("atan", "F", "F"),
    ("atan2", "F", "FF"),
    ("atanh", "F", "F"),
    ("cbrt", "F", "F"),
    ("ceil", "F", "F"),
    ("copysign", "F", "FF"),
    ("cos", "F", "F"),
    ("cosh", "F", "F"),
    ("erf", "F", "F"),
    ("erfc", "F", "F"),
    ("exp", "F", "F"),
    ("exp2", "F", "F"),
    ("expm1", "F", "F"),
    ("fabs", "F", "F"),
    ("fdim", "F", "FF"),
    ("floor", "F", "F"),
    ("fma", "F", "FFF"),
    ("fmax", "F", "FF"),
    ("fmin", "F", "FF"),
    ("fmod", "F", "FF"),
    ("frexp", "F", "FI"),
    ("hypot", "F", "FF"),
    ("ilogb", "i", "F"),
    ("ldexp", "F", "Fi"),
    ("lgamma", "F", "F"),
    ("llrint", "x", "F"),
    ("llround", "x", "F"),
    ("log", "F", "F"),
    ("log10", "F", "F"),
    ("log1p", "F", "F"),
    ("log2", "F", "F"),
    ("logb", "F", "F"),
    ("lrint", "l", "F"),
    ("lround", "l", "F"),
    ("modf", "F", "Fp"),
    ("nearbyint", "F", "F"),
    ("nextafter", "F", "FF"),
    ("pow", "F", "FF"),
    ("remainder", "F", "FF"),
    ("rint", "F", "F"),
    ("round", "F", "F"),
    ("scalbn", "F", "Fi"),
    ("scalbln", "F", "Fl"),
    ("sin", "F", "F"),
    ("sinh", "F", "F"),
    ("sqrt", "F", "F"),
    ("tan", "F", "F"),
    ("tanh", "F", "F"),
    ("tgamma", "F", "F"),
    ("trunc", "F", "F"),
];

fn code_type(code: char, float: FloatKind) -> CType {
    let konst = |ty: CType| {
        ty.with_quals(Quals {
            is_const: true,
            ..Quals::default()
        })
    };
    match code {
        'v' => CType::Void,
        'i' => CType::int_type(),
        'u' => CType::Int(TypeName::UInt),
        'l' => CType::Int(TypeName::Long),
        'x' => CType::Int(TypeName::LongLong),
        'z' => CType::Int(TypeName::ULong),
        'd' => CType::Float(FloatKind::Double),
        'f' => CType::Float(FloatKind::Float),
        'D' => CType::Float(FloatKind::LongDouble),
        'F' => CType::Float(float),
        'p' => CType::pointer(CType::Void),
        'P' => CType::pointer(konst(CType::Void)),
        's' => CType::pointer(CType::Int(TypeName::Char)),
        'S' => CType::pointer(konst(CType::Int(TypeName::Char))),
        'I' => CType::pointer(CType::int_type()),
        other => unreachable!("library signature code {other}"),
    }
}

fn build(name: &str, ret: &str, params: &str, float: FloatKind) -> LibSig {
    let variadic = params.ends_with('.');
    LibSig {
        name: name.to_string(),
        ret: code_type(ret.chars().next().expect("return code"), float),
        params: params
            .chars()
            .filter(|c| *c != '.')
            .map(|c| code_type(c, float))
            .collect(),
        variadic,
    }
}

/// The library function `__builtin_NAME` forwards to, if any.
pub fn library(name: &str) -> Option<LibSig> {
    let base = name.strip_prefix("__builtin_")?;
    for &(lib, ret, params) in LIBRARY {
        if lib == base {
            return Some(build(lib, ret, params, FloatKind::Double));
        }
    }
    for &(lib, ret, params) in MATH {
        if lib == base {
            return Some(build(lib, ret, params, FloatKind::Double));
        }
    }
    for &(lib, ret, params) in MATH {
        for (suffix, kind) in [("f", FloatKind::Float), ("l", FloatKind::LongDouble)] {
            if base.len() == lib.len() + 1 && base.starts_with(lib) && base.ends_with(suffix) {
                return Some(build(base, ret, params, kind));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_bit_builtins_and_variants() {
        assert_eq!(
            Inline::from_name("__builtin_clzll"),
            Some(Inline::Clz(TypeName::ULongLong))
        );
        assert_eq!(
            Inline::from_name("__builtin_ffsl"),
            Some(Inline::Ffs(TypeName::Long))
        );
        assert_eq!(
            Inline::from_name("__builtin_umull_overflow").or(None),
            Some(Inline::Overflow {
                op: OverflowOp::Mul,
                fixed: Some(TypeName::ULong)
            })
        );
        assert_eq!(Inline::from_name("__builtin_clzx"), None);
        assert_eq!(Inline::from_name("clz"), None);
    }

    #[test]
    fn folds_bit_operations() {
        assert_eq!(Inline::Clz(TypeName::UInt).fold(1), Some(31));
        assert_eq!(Inline::Clz(TypeName::UInt).fold(0), None);
        assert_eq!(Inline::Ctz(TypeName::ULong).fold(8), Some(3));
        assert_eq!(Inline::Popcount(TypeName::UInt).fold(0xff), Some(8));
        assert_eq!(Inline::Parity(TypeName::UInt).fold(7), Some(1));
        assert_eq!(Inline::Ffs(TypeName::Int).fold(0), Some(0));
        assert_eq!(Inline::Ffs(TypeName::Int).fold(12), Some(3));
        assert_eq!(Inline::Clrsb(TypeName::Int).fold(1), Some(30));
        assert_eq!(Inline::Clrsb(TypeName::Int).fold(u64::MAX), Some(31));
        assert_eq!(Inline::Bswap(TypeName::UShort).fold(0x1234), Some(0x3412));
        assert_eq!(
            Inline::Bswap(TypeName::UInt).fold(0x1234_5678),
            Some(0x7856_3412)
        );
    }

    #[test]
    fn library_forwarders_have_prototypes() {
        let strlen = library("__builtin_strlen").expect("strlen");
        assert_eq!(strlen.name, "strlen");
        assert_eq!(strlen.params.len(), 1);
        let printf = library("__builtin_printf").expect("printf");
        assert!(printf.variadic);
        let sqrtf = library("__builtin_sqrtf").expect("sqrtf");
        assert_eq!(sqrtf.ret, CType::Float(FloatKind::Float));
        assert!(library("__builtin_nonesuch").is_none());
        assert!(library("strlen").is_none());
    }
}
