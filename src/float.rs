//! Decimal and hexadecimal floating constants, and the conversions C11
//! requires between floating types and integers.
//!
//! A constant is rounded to the nearest value of its type, with ties to even.
//! That is the IEEE 754 default and the choice this target makes for the
//! implementation-defined rounding in C11 6.4.4.2. Decimal `float` and
//! `double` text is converted by the Rust standard library. `long double` is
//! the 80-bit extended format, converted in [`crate::fp80`]. Hexadecimal
//! constants are converted here from the binary significand.

use crate::ast::{sign_extend, Expr, FloatKind, StrEnc, TypeName};

/// IEEE-64 bit pattern of `bits`, which are `f32` bits for [`FloatKind::Float`]
/// and `f64` bits for [`FloatKind::Double`].
///
/// LLVM's hexadecimal floating constant is that 64-bit pattern for both
/// `float` and `double`. A `float` value is widened by integer bit
/// operations so a signaling NaN keeps its signaling encoding. Host
/// `f64::from(f32)` would quiet a signaling NaN.
pub fn llvm_bits(kind: FloatKind, bits: u128) -> u128 {
    match kind {
        FloatKind::Float => u128::from(widen_f32_bits(bits as u32)),
        FloatKind::Double => bits & 0xFFFF_FFFF_FFFF_FFFF,
        FloatKind::LongDouble => bits & ((1u128 << 80) - 1),
    }
}

/// Widen IEEE-754 binary32 bits to binary64, preserving NaN payloads
/// and the signaling/quiet bit. The 23-bit significand is left-justified
/// in the 52-bit field. Every finite `f32` value is exactly a `f64`.
fn widen_f32_bits(bits: u32) -> u64 {
    let sign = u64::from(bits >> 31) << 63;
    let exp = (bits >> 23) & 0xff;
    let frac = bits & 0x007F_FFFF;
    if exp == 0xff {
        return sign | (0x7ffu64 << 52) | (u64::from(frac) << 29);
    }
    if exp == 0 {
        if frac == 0 {
            return sign;
        }
        // Subnormal: the highest set bit is `k` (0..=22). The value is
        // 2^(k-149), which is a normal `f64`.
        let k = 31 - frac.leading_zeros();
        let exp64 = 874 + u64::from(k);
        let mant = (u64::from(frac) << (52 - k)) & ((1u64 << 52) - 1);
        return sign | (exp64 << 52) | mant;
    }
    let exp64 = u64::from(exp) + (1023 - 127);
    sign | (exp64 << 52) | (u64::from(frac) << 29)
}

pub fn llvm_const(kind: FloatKind, bits: u128) -> String {
    match kind {
        FloatKind::LongDouble => format!("0xK{:020X}", llvm_bits(kind, bits)),
        _ => format!("0x{:016X}", llvm_bits(kind, bits) as u64),
    }
}

pub fn real_is_zero(kind: FloatKind, bits: u128) -> bool {
    match kind {
        FloatKind::Float => f32::from_bits(bits as u32) == 0.0,
        FloatKind::Double => f64::from_bits(bits as u64) == 0.0,
        FloatKind::LongDouble => crate::fp80::is_zero(bits),
    }
}

/// Parse a decimal floating constant with the suffix already removed.
pub fn parse_decimal(spelling: &str, kind: FloatKind) -> Option<u128> {
    match kind {
        FloatKind::Float => spelling
            .parse::<f32>()
            .ok()
            .map(|v| u128::from(v.to_bits())),
        FloatKind::Double => spelling
            .parse::<f64>()
            .ok()
            .map(|v| u128::from(v.to_bits())),
        FloatKind::LongDouble => crate::fp80::parse_decimal(spelling),
    }
}

/// Parse a hexadecimal floating significand.
///
/// `int_digits` and `frac_digits` are the hex digits on either side of the
/// radix point (either may be empty, but not both). `bin_exp` is the decimal
/// power of two written after `p`/`P`. The value is the integer formed by
/// those digits times `2^(bin_exp - 4 * frac_digits.len())`.
pub fn parse_hex(
    int_digits: &str,
    frac_digits: &str,
    bin_exp: i32,
    kind: FloatKind,
) -> Option<u128> {
    match kind {
        FloatKind::Float => Some(u128::from(hex_float_bits(
            int_digits,
            frac_digits,
            bin_exp,
            true,
        ))),
        FloatKind::Double => Some(u128::from(hex_float_bits(
            int_digits,
            frac_digits,
            bin_exp,
            false,
        ))),
        FloatKind::LongDouble => Some(crate::fp80::parse_hex(int_digits, frac_digits, bin_exp)),
    }
}

/// An exponent that does not fit in a modest `i32` is an infinity or a zero.
/// Leading zeros do not count: `0000000001` is 1, not an overflow.
pub fn parse_exponent(digits: &str, neg: bool) -> Result<i32, bool> {
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Ok(0);
    }
    if digits.len() > 9 {
        return Err(neg);
    }
    let mut value: i32 = 0;
    for byte in digits.bytes() {
        let digit = i32::from(byte - b'0');
        value = match value.checked_mul(10).and_then(|v| v.checked_add(digit)) {
            Some(next) => next,
            None => return Err(neg),
        };
    }
    // Far past every IEEE exponent. The sign selects zero or infinity.
    if value > 100_000 {
        return Err(neg);
    }
    Ok(if neg { -value } else { value })
}

pub fn inf_bits(is_f32: bool) -> u64 {
    if is_f32 {
        u64::from(f32::INFINITY.to_bits())
    } else {
        f64::INFINITY.to_bits()
    }
}

/// GCC floating-point format builtins used by glibc `<math.h>` for
/// `NAN`, `INFINITY`, and the `HUGE_VAL` family. Documented in the GCC
/// manual, "Floating-Point Format Builtins".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FpFormatBuiltin {
    /// `__builtin_inf*`, `__builtin_huge_val*`: +infinity of this type.
    Inf(FloatKind),
    /// `__builtin_nan*` / `__builtin_nans*`: a quiet or signaling NaN.
    Nan { kind: FloatKind, signaling: bool },
}

impl FpFormatBuiltin {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "__builtin_inf" | "__builtin_huge_val" => Self::Inf(FloatKind::Double),
            "__builtin_inff" | "__builtin_huge_valf" => Self::Inf(FloatKind::Float),
            "__builtin_infl" | "__builtin_huge_vall" => Self::Inf(FloatKind::LongDouble),
            "__builtin_nan" => Self::Nan {
                kind: FloatKind::Double,
                signaling: false,
            },
            "__builtin_nanf" => Self::Nan {
                kind: FloatKind::Float,
                signaling: false,
            },
            "__builtin_nanl" => Self::Nan {
                kind: FloatKind::LongDouble,
                signaling: false,
            },
            "__builtin_nans" => Self::Nan {
                kind: FloatKind::Double,
                signaling: true,
            },
            "__builtin_nansf" => Self::Nan {
                kind: FloatKind::Float,
                signaling: true,
            },
            "__builtin_nansl" => Self::Nan {
                kind: FloatKind::LongDouble,
                signaling: true,
            },
            _ => return None,
        })
    }

    pub fn kind(self) -> FloatKind {
        match self {
            Self::Inf(kind) | Self::Nan { kind, .. } => kind,
        }
    }

    pub fn arity(self) -> usize {
        match self {
            Self::Inf(_) => 0,
            Self::Nan { .. } => 1,
        }
    }

    /// `__builtin_nan*` / `__builtin_nans*` require an ordinary string
    /// literal (GCC "Floating-Point Format Builtins").
    pub fn requires_string_literal(self) -> bool {
        matches!(self, Self::Nan { .. })
    }

    /// An ordinary (`""` / `u8""`) string literal, possibly after array
    /// decay. Wide literals are not accepted.
    pub fn is_string_literal_arg(expr: &Expr) -> bool {
        match expr {
            Expr::Str {
                enc: StrEnc::Byte, ..
            } => true,
            Expr::Decay { expr, .. } => Self::is_string_literal_arg(expr),
            _ => false,
        }
    }
}

/// +infinity of `kind`, as the type's bit pattern.
pub fn plus_inf(kind: FloatKind) -> u128 {
    match kind {
        FloatKind::Float => u128::from(inf_bits(true)),
        FloatKind::Double => u128::from(inf_bits(false)),
        FloatKind::LongDouble => crate::fp80::plus_inf(),
    }
}

/// Quiet or signaling NaN bits for `kind`.
///
/// The payload is placed in the least significant bits of the significand
/// and truncated to that field. A quiet NaN has the quiet bit set. A
/// signaling NaN has it clear, and a default payload bit is set when the
/// rest of the field would otherwise be zero (which would encode an
/// infinity). This follows the GCC manual's description of `__builtin_nan`
/// and `__builtin_nans`.
pub fn nan_bits(kind: FloatKind, payload: u128, signaling: bool) -> u128 {
    match kind {
        FloatKind::Float => ieee_nan_bits(payload, signaling, true),
        FloatKind::Double => ieee_nan_bits(payload, signaling, false),
        FloatKind::LongDouble => crate::fp80::nan_bits(payload, signaling),
    }
}

fn ieee_nan_bits(payload: u128, signaling: bool, is_f32: bool) -> u128 {
    let (exp, mant, quiet, default_signal) = if is_f32 {
        (0x7F80_0000u128, 0x007F_FFFF, 0x0040_0000, 0x0020_0000)
    } else {
        (
            0x7FF0_0000_0000_0000u128,
            0x000F_FFFF_FFFF_FFFF,
            0x0008_0000_0000_0000,
            0x0004_0000_0000_0000,
        )
    };
    let mut frac = payload & mant;
    if signaling {
        frac &= !quiet;
        if frac == 0 {
            frac = default_signal;
        }
    } else {
        frac |= quiet;
    }
    exp | frac
}

/// Payload of a `__builtin_nan*` / `__builtin_nans*` string-literal
/// argument. Only the bytes before the first NUL are parsed, as in a
/// C string passed to `strtol`.
pub fn nan_tag_from_expr(expr: &Expr) -> Option<u128> {
    match expr {
        Expr::Str {
            bytes,
            enc: StrEnc::Byte,
            ..
        } => parse_nan_tag_bytes(bytes),
        Expr::Decay { expr, .. } => nan_tag_from_expr(expr),
        _ => None,
    }
}

/// Parse a `__builtin_nan` / `__builtin_nans` string as `strtol` would
/// with base 0: a leading `0x`/`0X` is hexadecimal, a leading `0` is
/// octal, otherwise decimal. An empty string is 0. `None` if the text
/// is not a (possibly empty) integer literal consumed in full, without
/// a sign or leading whitespace. A compile-time constant requires that
/// complete consumption, as the GCC manual describes. Bytes after the
/// first NUL are ignored (C string semantics).
pub fn parse_nan_tag(tag: &str) -> Option<u128> {
    parse_nan_tag_bytes(tag.as_bytes())
}

pub fn parse_nan_tag_bytes(bytes: &[u8]) -> Option<u128> {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    parse_nan_tag_text(std::str::from_utf8(&bytes[..end]).ok()?)
}

fn parse_nan_tag_text(tag: &str) -> Option<u128> {
    if tag.is_empty() {
        return Some(0);
    }
    let bytes = tag.as_bytes();
    let (base, digits) = if let Some(rest) = bytes
        .strip_prefix(b"0x")
        .or_else(|| bytes.strip_prefix(b"0X"))
    {
        if rest.is_empty() || !rest.iter().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        (16u32, rest)
    } else if bytes[0] == b'0' {
        // A leading 0 is octal, as in strtol base 0. A digit 8 or 9 means
        // the literal is not consumed in full.
        if !bytes.iter().all(|b| (b'0'..=b'7').contains(b)) {
            return None;
        }
        (8, bytes)
    } else if bytes.iter().all(|b| b.is_ascii_digit()) {
        (10, bytes)
    } else {
        return None;
    };
    let mut value = 0u128;
    for &byte in digits {
        let digit = match byte {
            b'0'..=b'9' => u128::from(byte - b'0'),
            b'a'..=b'f' => u128::from(byte - b'a' + 10),
            b'A'..=b'F' => u128::from(byte - b'A' + 10),
            _ => return None,
        };
        value = value.saturating_mul(u128::from(base)).saturating_add(digit);
    }
    Some(value)
}

fn hex_float_bits(int_digits: &str, frac_digits: &str, bin_exp: i32, is_f32: bool) -> u64 {
    let prec: i32 = if is_f32 { 24 } else { 53 };
    let emax: i64 = if is_f32 { 127 } else { 1023 };
    let emin: i64 = if is_f32 { -126 } else { -1022 };
    let bias: i64 = if is_f32 { 127 } else { 1023 };

    let mut sig: u128 = 0;
    let mut exp_lsb: i64 = i64::from(bin_exp) - 4 * frac_digits.len() as i64;
    let mut sticky = false;
    for byte in int_digits.bytes().chain(frac_digits.bytes()) {
        let digit = match byte {
            b'0'..=b'9' => u128::from(byte - b'0'),
            b'a'..=b'f' => u128::from(byte - b'a') + 10,
            b'A'..=b'F' => u128::from(byte - b'A') + 10,
            _ => 0,
        };
        if sig > (u128::MAX >> 4) {
            if sig & 0xF != 0 {
                sticky = true;
            }
            sig >>= 4;
            exp_lsb += 4;
        }
        sig = (sig << 4) | digit;
    }
    if sig == 0 {
        return 0;
    }
    let msb = 127 - sig.leading_zeros() as i64;
    let unbiased = exp_lsb + msb;
    if unbiased > emax {
        return inf_bits(is_f32);
    }
    if unbiased < emin {
        return subnormal(sig, exp_lsb, sticky, prec, emin, is_f32);
    }
    let shift = msb - i64::from(prec - 1);
    let (mant, round, more) = split_sig(sig, shift, sticky);
    let mut mant = mant;
    let mut exp = unbiased;
    if round && (more || mant & 1 == 1) {
        mant += 1;
        if mant == 1u64 << prec {
            mant >>= 1;
            exp += 1;
        }
    }
    if exp > emax {
        return inf_bits(is_f32);
    }
    let frac_bits = prec - 1;
    let frac = mant & ((1u64 << frac_bits) - 1);
    pack((exp + bias) as u32, frac, is_f32)
}

fn subnormal(sig: u128, exp_lsb: i64, sticky: bool, prec: i32, emin: i64, is_f32: bool) -> u64 {
    // Weight of the least fraction bit of a subnormal: 2^(emin - (prec - 1)).
    let lsb_exp = emin - i64::from(prec - 1);
    let shift = lsb_exp - exp_lsb;
    let (mant, round, more) = if shift <= 0 {
        let shifted = sig << ((-shift) as u32);
        (shifted as u64, false, sticky)
    } else {
        split_sig(sig, shift, sticky)
    };
    let mut mant = mant;
    if round && (more || mant & 1 == 1) {
        mant += 1;
    }
    let limit = 1u64 << (prec - 1);
    if mant >= limit {
        // Rounded up to the minimum normal.
        return pack(1, 0, is_f32);
    }
    pack(0, mant, is_f32)
}

/// Shift `sig` so its highest bit that we keep sits at bit 0 of the returned
/// magnitude after a right shift of `shift`. `shift` may be negative.
///
/// Returns `(magnitude, round_bit, sticky)`.
fn split_sig(sig: u128, shift: i64, sticky: bool) -> (u64, bool, bool) {
    if shift <= 0 {
        let mag = if shift == 0 {
            sig
        } else {
            sig << ((-shift) as u32)
        };
        return (mag as u64, false, sticky);
    }
    if shift >= 128 {
        // The whole significand is below the round bit, except when the
        // highest remaining bit is exactly the round bit (shift == 128 and
        // the top bit of sig is set, which cannot happen: sig fits in 128
        // bits and shift == 128 drops every bit, so the round bit is off the
        // bottom). Everything is sticky.
        return (0, false, sticky || sig != 0);
    }
    let shift = shift as u32;
    let mag = (sig >> shift) as u64;
    let round = ((sig >> (shift - 1)) & 1) == 1;
    let rest = if shift == 1 {
        false
    } else {
        let mask = (1u128 << (shift - 1)) - 1;
        sig & mask != 0
    };
    (mag, round, sticky || rest)
}

fn pack(exp_field: u32, frac: u64, is_f32: bool) -> u64 {
    if is_f32 {
        u64::from((exp_field << 23) | (frac as u32 & 0x007F_FFFF))
    } else {
        (u64::from(exp_field) << 52) | (frac & 0x000F_FFFF_FFFF_FFFF)
    }
}

/// Convert a real value to an integer, truncating toward zero.
///
/// `_Bool` is the exception (C11 6.3.1.2): the result is 0 when the value
/// compares equal to 0, and 1 otherwise, including NaN and infinity. For
/// every other integer the result is `None` when the value is not finite or
/// does not fit. C leaves that case undefined; a constant initializer
/// rejects it.
pub fn real_to_int(bits: u128, kind: FloatKind, dest: TypeName) -> Option<u64> {
    if kind == FloatKind::LongDouble {
        return crate::fp80::to_int(bits, dest);
    }
    let value = match kind {
        FloatKind::Float => f64::from(f32::from_bits(bits as u32)),
        FloatKind::Double => f64::from_bits(bits as u64),
        FloatKind::LongDouble => unreachable!("long double is handled above"),
    };
    if dest == TypeName::Bool {
        return Some(u64::from(value != 0.0));
    }
    if !value.is_finite() {
        return None;
    }
    let truncated = value.trunc();
    if dest.width() >= 64 {
        return convert_wide(truncated, dest);
    }
    let min = if dest.is_signed() {
        -((1i64) << (dest.width() - 1)) as f64
    } else {
        0.0
    };
    let max = if dest.is_signed() {
        ((1i64) << (dest.width() - 1)) - 1
    } else {
        (1i64 << dest.width()) - 1
    } as f64;
    // Widths below 64 are exact in f64, including the limits.
    if truncated < min || truncated > max {
        return None;
    }
    let bits = if dest.is_signed() {
        truncated as i64 as u64
    } else {
        truncated as u64
    };
    Some(bits & dest.mask())
}

fn convert_wide(truncated: f64, dest: TypeName) -> Option<u64> {
    if dest.is_signed() {
        // 2^63 is the first f64 that is not representable as i64.
        if truncated < i64::MIN as f64 || truncated >= 9_223_372_036_854_775_808.0 {
            return None;
        }
        Some(truncated as i64 as u64)
    } else if !(0.0..18_446_744_073_709_551_616.0).contains(&truncated) {
        None
    } else {
        Some(truncated as u64)
    }
}

/// Integer to `float` or `double`, rounding to nearest, ties to even.
///
/// The cast goes straight to the destination format. Rounding through
/// `f64` and then to `f32` can change a value that is not a tie in `f32`.
pub fn int_to_real(bits: u64, src: TypeName, dest: FloatKind) -> u128 {
    if dest == FloatKind::LongDouble {
        return crate::fp80::from_int(bits, src);
    }
    let signed = sign_extend(bits, src.width());
    let unsigned = bits & src.mask();
    match dest {
        FloatKind::Float => {
            let value = if src.is_signed() {
                signed as f32
            } else {
                unsigned as f32
            };
            u128::from(value.to_bits())
        }
        FloatKind::Double => {
            let value = if src.is_signed() {
                signed as f64
            } else {
                unsigned as f64
            };
            u128::from(value.to_bits())
        }
        FloatKind::LongDouble => unreachable!("long double is handled above"),
    }
}

pub fn real_to_real(bits: u128, src: FloatKind, dest: FloatKind) -> u128 {
    if src == dest {
        return bits;
    }
    if src == FloatKind::LongDouble {
        return match dest {
            FloatKind::Float => u128::from(crate::fp80::to_ieee(bits, true)),
            FloatKind::Double => u128::from(crate::fp80::to_ieee(bits, false)),
            FloatKind::LongDouble => bits,
        };
    }
    if dest == FloatKind::LongDouble {
        return match src {
            FloatKind::Float => crate::fp80::from_ieee(bits as u64, true),
            FloatKind::Double => crate::fp80::from_ieee(bits as u64, false),
            FloatKind::LongDouble => bits,
        };
    }
    let value = match src {
        FloatKind::Float => f64::from(f32::from_bits(bits as u32)),
        FloatKind::Double => f64::from_bits(bits as u64),
        FloatKind::LongDouble => unreachable!("long double is handled above"),
    };
    match dest {
        FloatKind::Float => u128::from((value as f32).to_bits()),
        FloatKind::Double => u128::from(value.to_bits()),
        FloatKind::LongDouble => unreachable!("long double is handled above"),
    }
}

pub fn negate(kind: FloatKind, bits: u128) -> u128 {
    match kind {
        FloatKind::Float => u128::from((-f32::from_bits(bits as u32)).to_bits()),
        FloatKind::Double => u128::from((-f64::from_bits(bits as u64)).to_bits()),
        FloatKind::LongDouble => crate::fp80::negate(bits),
    }
}

pub fn apply_bin(op: FloatOp, kind: FloatKind, left: u128, right: u128) -> u128 {
    if kind == FloatKind::LongDouble {
        return match op {
            FloatOp::Add => crate::fp80::add(left, right),
            FloatOp::Sub => crate::fp80::sub(left, right),
            FloatOp::Mul => crate::fp80::mul(left, right),
            FloatOp::Div => crate::fp80::div(left, right),
        };
    }
    match kind {
        FloatKind::Float => {
            let left = f32::from_bits(left as u32);
            let right = f32::from_bits(right as u32);
            let value = match op {
                FloatOp::Add => left + right,
                FloatOp::Sub => left - right,
                FloatOp::Mul => left * right,
                FloatOp::Div => left / right,
            };
            u128::from(value.to_bits())
        }
        FloatKind::Double => {
            let left = f64::from_bits(left as u64);
            let right = f64::from_bits(right as u64);
            let value = match op {
                FloatOp::Add => left + right,
                FloatOp::Sub => left - right,
                FloatOp::Mul => left * right,
                FloatOp::Div => left / right,
            };
            u128::from(value.to_bits())
        }
        FloatKind::LongDouble => unreachable!("long double is handled above"),
    }
}

pub fn apply_cmp(op: FloatCmp, kind: FloatKind, left: u128, right: u128) -> bool {
    if kind == FloatKind::LongDouble {
        let which = match op {
            FloatCmp::Eq => crate::fp80::Cmp::Eq,
            FloatCmp::Ne => crate::fp80::Cmp::Ne,
            FloatCmp::Lt => crate::fp80::Cmp::Lt,
            FloatCmp::Gt => crate::fp80::Cmp::Gt,
            FloatCmp::Le => crate::fp80::Cmp::Le,
            FloatCmp::Ge => crate::fp80::Cmp::Ge,
        };
        return crate::fp80::cmp(which, left, right);
    }
    match kind {
        FloatKind::Float => {
            let left = f32::from_bits(left as u32);
            let right = f32::from_bits(right as u32);
            cmp(op, left, right)
        }
        FloatKind::Double => {
            let left = f64::from_bits(left as u64);
            let right = f64::from_bits(right as u64);
            cmp(op, left, right)
        }
        FloatKind::LongDouble => unreachable!("long double is handled above"),
    }
}

fn cmp<T: PartialEq + PartialOrd>(op: FloatCmp, left: T, right: T) -> bool {
    match op {
        FloatCmp::Eq => left == right,
        FloatCmp::Ne => left != right,
        FloatCmp::Lt => left < right,
        FloatCmp::Gt => left > right,
        FloatCmp::Le => left <= right,
        FloatCmp::Ge => left >= right,
    }
}

#[derive(Clone, Copy)]
pub enum FloatOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Clone, Copy)]
pub enum FloatCmp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

/// The common real type of two arithmetic types (C11 6.3.1.8).
///
/// `long double` wins over `double`, and `double` wins over `float`. An
/// integer operand is `None` and loses to every floating type.
pub fn usual_real(left: Option<FloatKind>, right: Option<FloatKind>) -> FloatKind {
    if left == Some(FloatKind::LongDouble) || right == Some(FloatKind::LongDouble) {
        FloatKind::LongDouble
    } else if left == Some(FloatKind::Double) || right == Some(FloatKind::Double) {
        FloatKind::Double
    } else {
        FloatKind::Float
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(int_digits: &str, frac: &str, exp: i32, kind: FloatKind) -> u128 {
        parse_hex(int_digits, frac, exp, kind).unwrap()
    }

    #[test]
    fn hex_constants_match_ieee_values() {
        assert_eq!(hex("", "0", 0, FloatKind::Double), 0);
        assert_eq!(hex("1", "", 0, FloatKind::Double), 0x3ff0_0000_0000_0000);
        assert_eq!(hex("1", "8", 1, FloatKind::Double), 0x4008_0000_0000_0000);
        assert_eq!(hex("", "8", 0, FloatKind::Double), 0x3fe0_0000_0000_0000);
        assert_eq!(hex("1", "", -1, FloatKind::Double), 0x3fe0_0000_0000_0000);
        assert_eq!(
            hex("1", "0000000000001", 0, FloatKind::Double),
            0x3ff0_0000_0000_0001
        );
        assert_eq!(
            hex("1", "", -1022, FloatKind::Double),
            0x0010_0000_0000_0000
        );
        assert_eq!(hex("1", "", -1074, FloatKind::Double), 1);
        assert_eq!(hex("1", "", -1075, FloatKind::Double), 0);
        assert_eq!(
            hex("1", "fffffffffffff", 1023, FloatKind::Double),
            0x7fef_ffff_ffff_ffff
        );
        assert_eq!(
            hex("1", "", 1024, FloatKind::Double),
            u128::from(f64::INFINITY.to_bits())
        );
        // Tie to even: halfway between 1 and the next double stays at 1.
        assert_eq!(
            hex("1", "00000000000008", 0, FloatKind::Double),
            0x3ff0_0000_0000_0000
        );
        // The next tie rounds away from the odd mantissa.
        assert_eq!(
            hex("1", "00000000000018", 0, FloatKind::Double),
            0x3ff0_0000_0000_0002
        );
        assert_eq!(
            hex("ABC", "DEF", 10, FloatKind::Double),
            0x4145_79bd_e000_0000
        );
        assert_eq!(hex("1", "", 0, FloatKind::Float), 0x3f80_0000);
        assert_eq!(hex("1", "8", 1, FloatKind::Float), 0x4040_0000);
        assert_eq!(hex("1", "fffffe", 127, FloatKind::Float), 0x7f7f_ffff);
        assert_eq!(
            hex("1", "", 128, FloatKind::Float),
            u128::from(f32::INFINITY.to_bits())
        );
        assert_eq!(hex("1", "", -149, FloatKind::Float), 1);
        assert_eq!(hex("1", "000002", 0, FloatKind::Float), 0x3f80_0001);
    }

    #[test]
    fn padded_hex_exponents_keep_their_value() {
        assert_eq!(parse_exponent("0000000001", false), Ok(1));
        assert_eq!(parse_exponent("000", true), Ok(0));
        assert!(parse_exponent("1000000000", false).is_err());
    }

    #[test]
    fn decimal_rounding_matches_f64() {
        let a = parse_decimal(
            "1.00000000000000033306690738754696212708950042724609375",
            FloatKind::Double,
        )
        .unwrap();
        let b = parse_decimal("1.0000000000000004", FloatKind::Double).unwrap();
        assert_eq!(a, b);
        let big = parse_decimal("9223372036854776832.5", FloatKind::Double).unwrap();
        let rounded = parse_decimal("9223372036854777856.0", FloatKind::Double).unwrap();
        assert_eq!(big, rounded);
        assert_eq!(
            parse_decimal("1e309", FloatKind::Double).unwrap(),
            u128::from(f64::INFINITY.to_bits())
        );
        assert_eq!(parse_decimal("1e-400", FloatKind::Double).unwrap(), 0);
        assert_eq!(
            parse_decimal("0.1", FloatKind::Double).unwrap(),
            0x3fb9_9999_9999_999a
        );
        assert_eq!(parse_decimal("0.1", FloatKind::Float).unwrap(), 0x3dcc_cccd);
    }

    #[test]
    fn conversions_truncate_toward_zero() {
        let one_point_nine = parse_decimal("1.9", FloatKind::Double).unwrap();
        assert_eq!(
            real_to_int(one_point_nine, FloatKind::Double, TypeName::Int),
            Some(1)
        );
        // The minus is a unary operator. Constant folding negates the bits.
        let neg_bits = negate(FloatKind::Double, one_point_nine);
        assert_eq!(
            real_to_int(neg_bits, FloatKind::Double, TypeName::Int),
            Some((-1i32) as u32 as u64)
        );
        assert_eq!(
            real_to_int(neg_bits, FloatKind::Double, TypeName::UInt),
            None
        );
        let tiny = negate(
            FloatKind::Double,
            parse_decimal("0.1", FloatKind::Double).unwrap(),
        );
        assert_eq!(
            real_to_int(tiny, FloatKind::Double, TypeName::UInt),
            Some(0)
        );
        assert!(real_to_int(
            u128::from(f64::INFINITY.to_bits()),
            FloatKind::Double,
            TypeName::Int
        )
        .is_none());
        assert!(real_to_int(
            u128::from(f64::NAN.to_bits()),
            FloatKind::Double,
            TypeName::Int
        )
        .is_none());
        assert_eq!(
            int_to_real(u64::MAX, TypeName::ULong, FloatKind::Double),
            u128::from((u64::MAX as f64).to_bits())
        );
        assert_eq!(
            real_to_int(one_point_nine, FloatKind::Double, TypeName::Bool),
            Some(1)
        );
        assert_eq!(real_to_int(0, FloatKind::Double, TypeName::Bool), Some(0));
        assert_eq!(
            real_to_int(
                u128::from(f64::NAN.to_bits()),
                FloatKind::Double,
                TypeName::Bool
            ),
            Some(1)
        );
        assert_eq!(
            real_to_int(
                u128::from(f64::INFINITY.to_bits()),
                FloatKind::Double,
                TypeName::Bool
            ),
            Some(1)
        );
        let wide = 4_611_686_293_305_294_849u64;
        assert_eq!(
            int_to_real(wide, TypeName::LongLong, FloatKind::Float),
            u128::from((wide as i64 as f32).to_bits())
        );
    }

    #[test]
    fn nan_and_inf_bits_match_ieee_and_x87() {
        // Patterns checked against Clang 19 and GCC 14 as oracles.
        assert_eq!(plus_inf(FloatKind::Float), 0x7F80_0000);
        assert_eq!(plus_inf(FloatKind::Double), 0x7FF0_0000_0000_0000);
        assert_eq!(plus_inf(FloatKind::LongDouble), 0x7FFF_8000_0000_0000_0000);
        assert_eq!(nan_bits(FloatKind::Float, 0, false), 0x7FC0_0000);
        assert_eq!(nan_bits(FloatKind::Float, 1, false), 0x7FC0_0001);
        assert_eq!(nan_bits(FloatKind::Float, 123, false), 0x7FC0_007B);
        assert_eq!(nan_bits(FloatKind::Float, 0x40_0000, false), 0x7FC0_0000);
        assert_eq!(nan_bits(FloatKind::Float, 0x7F_FFFF, false), 0x7FFF_FFFF);
        assert_eq!(nan_bits(FloatKind::Double, 0, false), 0x7FF8_0000_0000_0000);
        assert_eq!(nan_bits(FloatKind::Double, 1, false), 0x7FF8_0000_0000_0001);
        assert_eq!(
            nan_bits(FloatKind::LongDouble, 0, false),
            0x7FFF_C000_0000_0000_0000
        );
        assert_eq!(
            nan_bits(FloatKind::LongDouble, 1, false),
            0x7FFF_C000_0000_0000_0001
        );
        assert_eq!(nan_bits(FloatKind::Float, 0, true), 0x7FA0_0000);
        assert_eq!(nan_bits(FloatKind::Float, 1, true), 0x7F80_0001);
        assert_eq!(nan_bits(FloatKind::Double, 0, true), 0x7FF4_0000_0000_0000);
        assert_eq!(nan_bits(FloatKind::Double, 1, true), 0x7FF0_0000_0000_0001);
        assert_eq!(
            nan_bits(FloatKind::LongDouble, 0, true),
            0x7FFF_A000_0000_0000_0000
        );
        assert_eq!(
            nan_bits(FloatKind::LongDouble, 1, true),
            0x7FFF_8000_0000_0000_0001
        );
        assert_eq!(
            llvm_const(FloatKind::Float, nan_bits(FloatKind::Float, 0, false)),
            "0x7FF8000000000000"
        );
        assert_eq!(
            llvm_const(FloatKind::Float, plus_inf(FloatKind::Float)),
            "0x7FF0000000000000"
        );
        assert_eq!(
            llvm_const(
                FloatKind::LongDouble,
                nan_bits(FloatKind::LongDouble, 0, false)
            ),
            "0xK7FFFC000000000000000"
        );
    }

    #[test]
    fn parse_nan_tag_is_strtol_base_zero() {
        assert_eq!(parse_nan_tag(""), Some(0));
        assert_eq!(parse_nan_tag("0"), Some(0));
        assert_eq!(parse_nan_tag("1"), Some(1));
        assert_eq!(parse_nan_tag("123"), Some(123));
        assert_eq!(parse_nan_tag("010"), Some(8));
        assert_eq!(parse_nan_tag("0x1"), Some(1));
        assert_eq!(parse_nan_tag("0X10"), Some(16));
        assert_eq!(parse_nan_tag("0x400000"), Some(0x40_0000));
        assert_eq!(parse_nan_tag("abc"), None);
        assert_eq!(parse_nan_tag("1xyz"), None);
        assert_eq!(parse_nan_tag(" 1"), None);
        assert_eq!(parse_nan_tag("+5"), None);
        assert_eq!(parse_nan_tag("-1"), None);
        assert_eq!(parse_nan_tag("08"), None);
        assert_eq!(parse_nan_tag("0x"), None);
        // C string semantics: only the bytes before the first NUL count.
        assert_eq!(parse_nan_tag("1\0ignored"), Some(1));
        assert_eq!(parse_nan_tag_bytes(b"1\0ignored"), Some(1));
        assert_eq!(parse_nan_tag_bytes(b"\0ignored"), Some(0));
        assert_eq!(parse_nan_tag_bytes(b"1xyz\0ok"), None);
    }

    #[test]
    fn widen_f32_preserves_signaling_nan_encoding() {
        // Finite values, zeros, infinities, and quiet NaNs match the
        // host conversion. Signaling NaNs must not be quieted.
        for bits in [
            0x0000_0000u32,
            0x8000_0000,
            0x3F80_0000, // 1.0
            0x4000_0000, // 2.0
            0x3F00_0000, // 0.5
            0x7F80_0000, // +inf
            0xFF80_0000, // -inf
            0x0000_0001, // smallest subnormal
            0x007F_FFFF, // largest subnormal
            0x0080_0000, // smallest normal
            0x7F7F_FFFF, // largest finite
            0x7FC0_0000, // default quiet NaN
            0x7FC0_0001,
            0xFFC0_0000, // negative quiet NaN
        ] {
            assert_eq!(
                widen_f32_bits(bits),
                f64::from(f32::from_bits(bits)).to_bits(),
                "mismatch for {bits:#010X}"
            );
        }
        let snan = nan_bits(FloatKind::Float, 0, true) as u32;
        assert_eq!(snan, 0x7FA0_0000);
        assert_eq!(widen_f32_bits(snan), 0x7FF4_0000_0000_0000);
        // The host conversion quiets this payload; that is why widening
        // is done with integer bit operations.
        assert_ne!(
            f64::from(f32::from_bits(snan)).to_bits(),
            0x7FF4_0000_0000_0000
        );
        assert_eq!(
            llvm_const(FloatKind::Float, u128::from(snan)),
            "0x7FF4000000000000"
        );
        assert_eq!(
            llvm_const(FloatKind::Double, nan_bits(FloatKind::Double, 0, true)),
            "0x7FF4000000000000"
        );
        assert_eq!(
            llvm_const(
                FloatKind::LongDouble,
                nan_bits(FloatKind::LongDouble, 0, true)
            ),
            "0xK7FFFA000000000000000"
        );
        let neg = snan | 0x8000_0000;
        assert_eq!(widen_f32_bits(neg), 0xFFF4_0000_0000_0000);
    }
}
