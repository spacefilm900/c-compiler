//! Decimal and hexadecimal floating constants, and the conversions C11
//! requires between floating types and integers.
//!
//! A constant is rounded to the nearest value of its type, with ties to even.
//! That is the IEEE 754 default and the choice this target makes for the
//! implementation-defined rounding in C11 6.4.4.2. Decimal text is converted
//! by the Rust standard library, which uses that rounding. Hexadecimal
//! constants are converted here from the binary significand. `long double`
//! is not produced: the suffix `l`/`L` is rejected by the lexer.

use crate::ast::{sign_extend, FloatKind, TypeName};

/// IEEE-64 bit pattern of `bits`, which are `f32` bits for [`FloatKind::Float`]
/// and `f64` bits for [`FloatKind::Double`].
///
/// LLVM's hexadecimal floating constant is that 64-bit pattern for both
/// `float` and `double`. A `float` value is first widened; every `f32` value
/// is exactly a `f64`.
pub fn llvm_bits(kind: FloatKind, bits: u64) -> u64 {
    match kind {
        FloatKind::Float => f64::from(f32::from_bits(bits as u32)).to_bits(),
        FloatKind::Double => bits,
        FloatKind::LongDouble => bits,
    }
}

pub fn llvm_const(kind: FloatKind, bits: u64) -> String {
    format!("0x{:016X}", llvm_bits(kind, bits))
}

pub fn real_is_zero(kind: FloatKind, bits: u64) -> bool {
    match kind {
        FloatKind::Float => f32::from_bits(bits as u32) == 0.0,
        FloatKind::Double => f64::from_bits(bits) == 0.0,
        FloatKind::LongDouble => false,
    }
}

/// Parse a decimal floating constant with the suffix already removed.
pub fn parse_decimal(spelling: &str, kind: FloatKind) -> Option<u64> {
    match kind {
        FloatKind::Float => spelling.parse::<f32>().ok().map(|v| u64::from(v.to_bits())),
        FloatKind::Double => spelling.parse::<f64>().ok().map(f64::to_bits),
        FloatKind::LongDouble => None,
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
) -> Option<u64> {
    let is_f32 = match kind {
        FloatKind::Float => true,
        FloatKind::Double => false,
        FloatKind::LongDouble => return None,
    };
    Some(hex_float_bits(int_digits, frac_digits, bin_exp, is_f32))
}

/// An exponent that does not fit in a modest `i32` is an infinity or a zero.
pub fn parse_exponent(digits: &str, neg: bool) -> Result<i32, bool> {
    if digits.is_empty() || digits.len() > 9 {
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
/// The result is `None` when the value is not finite or does not fit in
/// `dest`. C leaves that case undefined; a constant initializer rejects it.
pub fn real_to_int(bits: u64, kind: FloatKind, dest: TypeName) -> Option<u64> {
    let value = match kind {
        FloatKind::Float => f64::from(f32::from_bits(bits as u32)),
        FloatKind::Double => f64::from_bits(bits),
        FloatKind::LongDouble => return None,
    };
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
pub fn int_to_real(bits: u64, src: TypeName, dest: FloatKind) -> u64 {
    let value = if src.is_signed() {
        sign_extend(bits, src.width()) as f64
    } else {
        (bits & src.mask()) as f64
    };
    match dest {
        FloatKind::Float => u64::from((value as f32).to_bits()),
        FloatKind::Double => value.to_bits(),
        FloatKind::LongDouble => 0,
    }
}

pub fn real_to_real(bits: u64, src: FloatKind, dest: FloatKind) -> u64 {
    if src == dest {
        return bits;
    }
    let value = match src {
        FloatKind::Float => f64::from(f32::from_bits(bits as u32)),
        FloatKind::Double => f64::from_bits(bits),
        FloatKind::LongDouble => return bits,
    };
    match dest {
        FloatKind::Float => u64::from((value as f32).to_bits()),
        FloatKind::Double => value.to_bits(),
        FloatKind::LongDouble => bits,
    }
}

pub fn negate(kind: FloatKind, bits: u64) -> u64 {
    match kind {
        FloatKind::Float => u64::from((-f32::from_bits(bits as u32)).to_bits()),
        FloatKind::Double => (-f64::from_bits(bits)).to_bits(),
        FloatKind::LongDouble => bits,
    }
}

pub fn apply_bin(op: FloatOp, kind: FloatKind, left: u64, right: u64) -> u64 {
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
            u64::from(value.to_bits())
        }
        FloatKind::Double => {
            let left = f64::from_bits(left);
            let right = f64::from_bits(right);
            let value = match op {
                FloatOp::Add => left + right,
                FloatOp::Sub => left - right,
                FloatOp::Mul => left * right,
                FloatOp::Div => left / right,
            };
            value.to_bits()
        }
        FloatKind::LongDouble => 0,
    }
}

pub fn apply_cmp(op: FloatCmp, kind: FloatKind, left: u64, right: u64) -> bool {
    match kind {
        FloatKind::Float => {
            let left = f32::from_bits(left as u32);
            let right = f32::from_bits(right as u32);
            cmp(op, left, right)
        }
        FloatKind::Double => {
            let left = f64::from_bits(left);
            let right = f64::from_bits(right);
            cmp(op, left, right)
        }
        FloatKind::LongDouble => false,
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

/// The common real type of two arithmetic types, when at least one is
/// `float` or `double`. `long double` is not a result.
pub fn usual_real(left: Option<FloatKind>, right: Option<FloatKind>) -> FloatKind {
    if left == Some(FloatKind::Double) || right == Some(FloatKind::Double) {
        FloatKind::Double
    } else {
        FloatKind::Float
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(int_digits: &str, frac: &str, exp: i32, kind: FloatKind) -> u64 {
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
            f64::INFINITY.to_bits()
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
            u64::from(f32::INFINITY.to_bits())
        );
        assert_eq!(hex("1", "", -149, FloatKind::Float), 1);
        assert_eq!(hex("1", "000002", 0, FloatKind::Float), 0x3f80_0001);
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
            f64::INFINITY.to_bits()
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
        assert!(real_to_int(f64::INFINITY.to_bits(), FloatKind::Double, TypeName::Int).is_none());
        assert!(real_to_int(f64::NAN.to_bits(), FloatKind::Double, TypeName::Int).is_none());
        assert_eq!(
            int_to_real(u64::MAX, TypeName::ULong, FloatKind::Double),
            (u64::MAX as f64).to_bits()
        );
    }
}
