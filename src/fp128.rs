//! IEEE 754 binary128, the representation of `long double` on AArch64 Linux
//! (AAPCS64 and the AArch64 ELF ABI give `long double` the 128-bit IEEE
//! format).
//!
//! A value is 1 sign bit (bit 127), 15 exponent bits (bits 112-126) with bias
//! 16383, and a 112-bit fraction with an implicit leading 1 for normals.
//! Subnormals use a zero exponent field; the least subnormal is 2^-16494.
//! LLVM spells the bits as `0xL` followed by the low 64 bits and then the
//! high 64 bits, in hexadecimal.
//!
//! Every operation here computes the exact result and rounds it once, to
//! nearest with ties to even. IEEE 754 requires exactly that for `+`, `-`,
//! `*`, `/` and the conversions, so a folded constant is the value the same
//! operation gives at run time, and a decimal constant is the nearest
//! binary128 value (C11 6.4.4.2 leaves the rounding implementation-defined;
//! this is the IEEE default every format here uses). The same rounding also
//! narrows binary128 to `float` and `double`.

use std::cmp::Ordering;

use crate::ast::{sign_extend, TypeName};
use crate::bigint::{scan_decimal, Big, DecimalText};

const SIGN: u128 = 1 << 127;
const FRAC_MASK: u128 = (1 << 112) - 1;
const INF: u128 = 0x7FFF << 112;
/// The default quiet NaN: positive, quiet bit only.
const QNAN: u128 = INF | (1 << 111);
/// 1.0.
pub const ONE: u128 = 0x3FFF << 112;
/// The least normal value, 2^-16382.
pub const MIN_NORMAL: u128 = 1 << 112;

/// A binary interchange format: precision (with the leading bit), largest
/// exponent, and exponent field width.
#[derive(Clone, Copy)]
struct Format {
    prec: u32,
    emax: i64,
    ebits: u32,
}

const QUAD: Format = Format {
    prec: 113,
    emax: 16383,
    ebits: 15,
};
const DOUBLE: Format = Format {
    prec: 53,
    emax: 1023,
    ebits: 11,
};
const SINGLE: Format = Format {
    prec: 24,
    emax: 127,
    ebits: 8,
};

impl Format {
    fn frac_bits(self) -> u32 {
        self.prec - 1
    }

    fn sign_bit(self) -> u128 {
        1 << (self.frac_bits() + self.ebits)
    }

    fn inf(self) -> u128 {
        ((1u128 << self.ebits) - 1) << self.frac_bits()
    }

    fn quiet_bit(self) -> u128 {
        1 << (self.frac_bits() - 1)
    }

    /// Power of two of the least subnormal bit.
    fn min_lsb(self) -> i64 {
        1 - self.emax - i64::from(self.frac_bits())
    }

    fn of(is_f32: bool) -> Format {
        if is_f32 {
            SINGLE
        } else {
            DOUBLE
        }
    }
}

/// A decoded value. A finite nonzero value is `sig * 2^exp2`.
enum Class {
    Nan,
    Inf(bool),
    Zero(bool),
    Finite(bool, u128, i64),
}

fn class_in(format: Format, bits: u128) -> Class {
    let sign = bits & format.sign_bit() != 0;
    let frac_mask = (1u128 << format.frac_bits()) - 1;
    let exp_max = (1u128 << format.ebits) - 1;
    let exp = (bits >> format.frac_bits()) & exp_max;
    let frac = bits & frac_mask;
    if exp == exp_max {
        if frac == 0 {
            Class::Inf(sign)
        } else {
            Class::Nan
        }
    } else if exp == 0 {
        if frac == 0 {
            Class::Zero(sign)
        } else {
            Class::Finite(sign, frac, format.min_lsb())
        }
    } else {
        let exp2 = exp as i64 - format.emax - i64::from(format.frac_bits());
        Class::Finite(sign, frac | (1 << format.frac_bits()), exp2)
    }
}

fn class(bits: u128) -> Class {
    class_in(QUAD, bits)
}

/// Round `mag * 2^exp2` (negated when `sign`) to `format`, to nearest with
/// ties to even, with gradual underflow and overflow to infinity. `sticky`
/// says the exact magnitude is a little more than `mag * 2^exp2`, by less
/// than `2^exp2`; a caller that sets it supplies at least `prec + 2` bits,
/// so the extra amount lies below the rounding bit.
fn round(format: Format, sign: bool, mag: &Big, exp2: i64, sticky: bool) -> u128 {
    let sign_bits = if sign { format.sign_bit() } else { 0 };
    let len = i64::from(mag.bit_length());
    if len == 0 {
        return sign_bits;
    }
    let shift = (len - i64::from(format.prec)).max(format.min_lsb() - exp2);
    let mut lsb = exp2 + shift;
    let mut sig = if shift <= 0 {
        mag.bits_u128(0, len as u32) << (-shift) as u32
    } else {
        let width = (len - shift).max(0) as u32;
        let shift = u32::try_from(shift).unwrap_or(u32::MAX - 1);
        let mut sig = if width > 0 {
            mag.bits_u128(shift, width)
        } else {
            0
        };
        let half = mag.bit(shift - 1);
        let rest = sticky || (shift >= 2 && mag.any_below(shift - 1));
        if half && (rest || sig & 1 == 1) {
            sig += 1;
        }
        sig
    };
    if sig >> format.prec != 0 {
        sig >>= 1;
        lsb += 1;
    }
    if sig == 0 {
        return sign_bits;
    }
    if sig >> format.frac_bits() == 0 {
        // Subnormal: `lsb` is the format's least subnormal bit.
        return sign_bits | sig;
    }
    let biased = lsb + i64::from(format.frac_bits()) + format.emax;
    if biased >= (1i64 << format.ebits) - 1 {
        return sign_bits | format.inf();
    }
    let frac = sig & ((1u128 << format.frac_bits()) - 1);
    sign_bits | ((biased as u128) << format.frac_bits()) | frac
}

fn signed_inf(sign: bool) -> u128 {
    if sign {
        INF | SIGN
    } else {
        INF
    }
}

fn signed_zero(sign: bool) -> u128 {
    if sign {
        SIGN
    } else {
        0
    }
}

pub fn parse_decimal(body: &str) -> Option<u128> {
    Some(match scan_decimal(body)? {
        DecimalText::Zero => 0,
        DecimalText::Huge => INF,
        DecimalText::Digits(digits, exp10) => decimal_to_bits(&digits, exp10),
    })
}

/// `digits * 10^exp10`, rounded. The value lies in
/// `[10^(len - 1 + exp10), 10^(len + exp10))`; past 10^4933 it is
/// infinite, and below 10^-4967 (under half the least subnormal,
/// about 6.5e-4966) it is zero.
fn decimal_to_bits(digits: &[u8], exp10: i64) -> u128 {
    let magnitude = digits.len() as i64 + exp10;
    if magnitude > 4934 {
        return INF;
    }
    if magnitude < -4967 {
        return 0;
    }
    let mant = Big::from_digits(digits);
    if exp10 >= 0 {
        let exact = mant.mul(&Big::pow5(exp10 as u32));
        return round(QUAD, false, &exact, exp10, false);
    }
    let k = (-exp10) as u32;
    let den = Big::pow5(k);
    // Enough quotient bits for the significand, a rounding bit and one
    // more, so the remainder only decides the sticky bit.
    let extra = (i64::from(den.bit_length()) + i64::from(QUAD.prec) + 3
        - i64::from(mant.bit_length()))
    .max(0) as u32;
    let (quot, rem) = mant.shl(extra).div_rem(&den);
    // quot ~ mant / 5^k * 2^extra; the value is that times 2^(-k - extra).
    round(
        QUAD,
        false,
        &quot,
        -i64::from(k) - i64::from(extra),
        !rem.is_zero(),
    )
}

/// A hexadecimal floating constant: the hex digits around the point and the
/// binary exponent after `p` (C11 6.4.4.2).
pub fn parse_hex(int_digits: &str, frac_digits: &str, bin_exp: i32) -> u128 {
    let mut mag = Big::from_u128(0);
    for byte in int_digits.bytes().chain(frac_digits.bytes()) {
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => 0,
        };
        mag = mag.mul_small(16).add_small(u32::from(digit));
    }
    let exp2 = i64::from(bin_exp) - 4 * frac_digits.len() as i64;
    round(QUAD, false, &mag, exp2, false)
}

pub fn plus_inf() -> u128 {
    INF
}

/// A quiet or signaling NaN with `payload` in the low bits of the 112-bit
/// fraction. A quiet NaN has the top fraction bit set. A signaling NaN has
/// it clear, and the next bit set when the rest of the fraction would
/// otherwise be zero (which would encode an infinity). This is the layout
/// float.rs uses for `float` and `double`.
pub fn nan_bits(payload: u128, signaling: bool) -> u128 {
    let quiet = QUAD.quiet_bit();
    let mut frac = payload & FRAC_MASK;
    if signaling {
        frac &= !quiet;
        if frac == 0 {
            frac = quiet >> 1;
        }
    } else {
        frac |= quiet;
    }
    INF | frac
}

pub fn is_zero(bits: u128) -> bool {
    bits & !SIGN == 0
}

pub fn negate(bits: u128) -> u128 {
    bits ^ SIGN
}

pub fn add(left: u128, right: u128) -> u128 {
    match (class(left), class(right)) {
        (Class::Nan, _) | (_, Class::Nan) => QNAN,
        (Class::Inf(a), Class::Inf(b)) => {
            if a == b {
                signed_inf(a)
            } else {
                QNAN
            }
        }
        (Class::Inf(a), _) => signed_inf(a),
        (_, Class::Inf(b)) => signed_inf(b),
        // Round to nearest: x + -x and +0 + -0 are +0; -0 + -0 is -0.
        (Class::Zero(a), Class::Zero(b)) => signed_zero(a && b),
        (Class::Zero(_), _) => right,
        (_, Class::Zero(_)) => left,
        (Class::Finite(sa, ma, ea), Class::Finite(sb, mb, eb)) => {
            let ((sa, ma, ea), (sb, mb, eb)) = if ea >= eb {
                ((sa, ma, ea), (sb, mb, eb))
            } else {
                ((sb, mb, eb), (sa, ma, ea))
            };
            // An addend more than 240 places below the other is far under
            // a quarter of the result's last place. Any such amount rounds
            // the same way, so a single bit stands in for it and keeps the
            // exact sum small.
            let (mb, eb) = if ea - eb > 240 {
                (1, ea - 240)
            } else {
                (mb, eb)
            };
            let big = Big::from_u128(ma).shl((ea - eb) as u32);
            let small = Big::from_u128(mb);
            if sa == sb {
                return round(QUAD, sa, &big.add(&small), eb, false);
            }
            match big.cmp(&small) {
                Ordering::Equal => 0,
                Ordering::Greater => round(QUAD, sa, &big.sub(&small), eb, false),
                Ordering::Less => round(QUAD, sb, &small.sub(&big), eb, false),
            }
        }
    }
}

pub fn sub(left: u128, right: u128) -> u128 {
    if matches!(class(right), Class::Nan) {
        return QNAN;
    }
    add(left, negate(right))
}

pub fn mul(left: u128, right: u128) -> u128 {
    let sign = (left ^ right) & SIGN != 0;
    match (class(left), class(right)) {
        (Class::Nan, _) | (_, Class::Nan) => QNAN,
        (Class::Inf(_), Class::Zero(_)) | (Class::Zero(_), Class::Inf(_)) => QNAN,
        (Class::Inf(_), _) | (_, Class::Inf(_)) => signed_inf(sign),
        (Class::Zero(_), _) | (_, Class::Zero(_)) => signed_zero(sign),
        (Class::Finite(_, ma, ea), Class::Finite(_, mb, eb)) => {
            let exact = Big::from_u128(ma).mul(&Big::from_u128(mb));
            round(QUAD, sign, &exact, ea + eb, false)
        }
    }
}

pub fn div(left: u128, right: u128) -> u128 {
    let sign = (left ^ right) & SIGN != 0;
    match (class(left), class(right)) {
        (Class::Nan, _) | (_, Class::Nan) => QNAN,
        (Class::Inf(_), Class::Inf(_)) | (Class::Zero(_), Class::Zero(_)) => QNAN,
        (Class::Inf(_), _) | (_, Class::Zero(_)) => signed_inf(sign),
        (_, Class::Inf(_)) | (Class::Zero(_), _) => signed_zero(sign),
        (Class::Finite(_, ma, ea), Class::Finite(_, mb, eb)) => {
            let num = Big::from_u128(ma);
            let den = Big::from_u128(mb);
            // At least prec + 3 quotient bits.
            let extra = (i64::from(den.bit_length()) + i64::from(QUAD.prec) + 3
                - i64::from(num.bit_length()))
            .max(0) as u32;
            let (quot, rem) = num.shl(extra).div_rem(&den);
            round(
                QUAD,
                sign,
                &quot,
                ea - eb - i64::from(extra),
                !rem.is_zero(),
            )
        }
    }
}

/// The order of two values, or `None` when either is a NaN (unordered).
/// The two zeros are equal.
pub fn compare(left: u128, right: u128) -> Option<Ordering> {
    if matches!(class(left), Class::Nan) || matches!(class(right), Class::Nan) {
        return None;
    }
    if is_zero(left) && is_zero(right) {
        return Some(Ordering::Equal);
    }
    let (neg_l, neg_r) = (left & SIGN != 0, right & SIGN != 0);
    if neg_l != neg_r {
        return Some(if neg_l {
            Ordering::Less
        } else {
            Ordering::Greater
        });
    }
    // Magnitudes order as their bit patterns do.
    let mag = (left & !SIGN).cmp(&(right & !SIGN));
    Some(if neg_l { mag.reverse() } else { mag })
}

pub fn from_int(bits: u128, src: TypeName) -> u128 {
    let (sign, mag) = if src.is_signed() {
        let wide = sign_extend(bits, src.width());
        (wide < 0, wide.unsigned_abs())
    } else {
        (false, bits & src.mask())
    };
    round(QUAD, sign, &Big::from_u128(mag), 0, false)
}

/// Truncate toward zero. `_Bool` is 0 or 1 (C11 6.3.1.2); for any other
/// type `None` when the value is not finite or does not fit.
pub fn to_int(bits: u128, dest: TypeName) -> Option<u128> {
    if dest == TypeName::Bool {
        return Some(u128::from(!is_zero(bits)));
    }
    let (sign, sig, exp2) = match class(bits) {
        Class::Nan | Class::Inf(_) => return None,
        Class::Zero(_) => return Some(0),
        Class::Finite(sign, sig, exp2) => (sign, sig, exp2),
    };
    let mag: u128 = if exp2 >= 0 {
        if i64::from(128 - sig.leading_zeros()) + exp2 > 128 {
            return None;
        }
        sig << exp2
    } else if exp2 <= -128 {
        0
    } else {
        sig >> (-exp2)
    };
    if dest.is_signed() {
        let (min, max) = crate::ast::signed_limits(dest.width());
        let value = if sign {
            if mag > 1 << 127 {
                return None;
            }
            (mag as i128).wrapping_neg()
        } else if mag > i128::MAX as u128 {
            return None;
        } else {
            mag as i128
        };
        if value < min || value > max {
            return None;
        }
        Some((value as u128) & dest.mask())
    } else {
        if sign && mag != 0 {
            return None;
        }
        if mag > dest.mask() {
            return None;
        }
        Some(mag)
    }
}

/// Widen `float` (`is_f32`) or `double` bits. Every value is exact; a NaN
/// keeps its sign and payload and is quieted.
pub fn from_ieee(bits: u64, is_f32: bool) -> u128 {
    let src = Format::of(is_f32);
    let bits = u128::from(bits);
    match class_in(src, bits) {
        Class::Nan => {
            let sign = if bits & src.sign_bit() != 0 { SIGN } else { 0 };
            let payload = bits & ((1u128 << src.frac_bits()) - 1);
            sign | INF | QUAD.quiet_bit() | (payload << (112 - src.frac_bits()))
        }
        Class::Inf(sign) => signed_inf(sign),
        Class::Zero(sign) => signed_zero(sign),
        Class::Finite(sign, sig, exp2) => round(QUAD, sign, &Big::from_u128(sig), exp2, false),
    }
}

/// Narrow to `float` (`is_f32`) or `double` bits, rounding to nearest. A
/// NaN keeps its sign and the top of its payload and is quieted.
pub fn to_ieee(bits: u128, is_f32: bool) -> u64 {
    let dest = Format::of(is_f32);
    let out = match class(bits) {
        Class::Nan => {
            let sign = if bits & SIGN != 0 { dest.sign_bit() } else { 0 };
            let payload = (bits & FRAC_MASK) >> (112 - dest.frac_bits());
            sign | dest.inf() | dest.quiet_bit() | payload
        }
        Class::Inf(sign) => {
            if sign {
                dest.inf() | dest.sign_bit()
            } else {
                dest.inf()
            }
        }
        Class::Zero(sign) => {
            if sign {
                dest.sign_bit()
            } else {
                0
            }
        }
        Class::Finite(sign, sig, exp2) => round(dest, sign, &Big::from_u128(sig), exp2, false),
    };
    out as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(text: &str) -> u128 {
        parse_decimal(text).expect("decimal")
    }

    /// The bits of the double `value`, widened.
    fn wide(value: f64) -> u128 {
        from_ieee(value.to_bits(), false)
    }

    #[test]
    fn exact_values() {
        assert_eq!(dec("1"), ONE);
        assert_eq!(dec("1.0"), ONE);
        assert_eq!(dec("2"), 0x4000 << 112);
        assert_eq!(dec("0.5"), 0x3FFE << 112);
        assert_eq!(dec("0"), 0);
        assert_eq!(dec("3"), (0x4000 << 112) | (1 << 111));
        assert_eq!(wide(1.5), (0x3FFF << 112) | (1 << 111));
        assert_eq!(wide(-2.0), SIGN | (0x4000 << 112));
    }

    #[test]
    fn decimal_rounding() {
        // 0.1 = 0x3FFB999999999999999999999999999A (1.6 * 2^-4, the last
        // place rounds up). Clang, as a black box, writes the same bits.
        assert_eq!(dec("0.1"), 0x3FFB_9999_9999_9999_9999_9999_9999_999A);
        assert_eq!(dec("1e-1"), dec("0.1"));
        assert_eq!(dec("0.2"), 0x3FFC_9999_9999_9999_9999_9999_9999_999A);
        // 1/3 is below the halfway point: truncated.
        assert_eq!(
            div(ONE, dec("3")),
            0x3FFD_5555_5555_5555_5555_5555_5555_5555
        );
    }

    #[test]
    fn limits() {
        let max = 0x7FFE_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF;
        assert_eq!(dec("1.18973149535723176508575932662800702e+4932"), max);
        assert_eq!(dec("1.2e4932"), INF);
        assert_eq!(dec("1e5000"), INF);
        assert_eq!(
            dec("3.36210314311209350626267781732175260e-4932"),
            MIN_NORMAL
        );
        // The least subnormal, 2^-16494, and values that round to it or to 0.
        assert_eq!(dec("6.47517511943802511092443895822764655e-4966"), 1);
        assert_eq!(dec("4e-4966"), 1);
        assert_eq!(dec("3e-4966"), 0);
        assert_eq!(dec("1e-5000"), 0);
        assert_eq!(parse_hex("1", "", -16494), 1);
        assert_eq!(parse_hex("1", "", -16495), 0);
        assert_eq!(parse_hex("3", "", -16496), 1);
        assert_eq!(parse_hex("1", "", 16384), INF);
        assert_eq!(parse_hex("1", "", -16382), MIN_NORMAL);
    }

    #[test]
    fn hexadecimal() {
        assert_eq!(parse_hex("1", "8", 0), wide(1.5));
        assert_eq!(parse_hex("", "8", 1), ONE);
        // 114 significant bits: the last one is a tie, rounded to even.
        let digits = format!("1{}", "0".repeat(28));
        assert_eq!(parse_hex(&digits, "", 0), parse_hex("1", "", 112));
        let tie = format!("2{}1", "0".repeat(27));
        assert_eq!(parse_hex(&tie, "", 0), parse_hex("2", "", 112));
        let above = format!("2{}3", "0".repeat(27));
        assert_eq!(parse_hex(&above, "", 0), parse_hex("2", "", 112) + 2);
    }

    #[test]
    fn arithmetic() {
        let tenth = dec("0.1");
        let sum = add(tenth, dec("0.2"));
        assert_eq!(sum, 0x3FFD_3333_3333_3333_3333_3333_3333_3334);
        assert_eq!(sub(sum, sum), 0);
        assert_eq!(sub(negate(sum), negate(sum)), 0);
        assert_eq!(mul(dec("2.5"), dec("4")), dec("10"));
        assert_eq!(sub(mul(dec("2.5"), dec("4")), dec("0.5")), dec("9.5"));
        assert_eq!(div(dec("1"), dec("4")), dec("0.25"));
        // Far apart: the small addend only matters through rounding.
        assert_eq!(add(ONE, parse_hex("1", "", -300)), ONE);
        assert_eq!(sub(ONE, parse_hex("1", "", -300)), ONE);
        assert_eq!(add(ONE, parse_hex("1", "", -113)), ONE);
        assert_eq!(add(ONE, parse_hex("3", "", -114)), ONE + 1);
        assert_eq!(
            sub(parse_hex("1", "", 1), parse_hex("1", "", -113)),
            parse_hex("1", "", 1)
        );
        // Overflow and underflow.
        let max = 0x7FFE_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF;
        assert_eq!(mul(max, dec("2")), INF);
        assert_eq!(mul(MIN_NORMAL, dec("0.5")), 1 << 111);
        assert_eq!(div(1, dec("2")), 0);
        assert_eq!(div(3, dec("2")), 2);
    }

    #[test]
    fn specials() {
        assert_eq!(add(INF, negate(INF)), QNAN);
        assert_eq!(mul(INF, 0), QNAN);
        assert_eq!(div(0, 0), QNAN);
        assert_eq!(div(ONE, 0), INF);
        assert_eq!(div(negate(ONE), 0), negate(INF));
        assert_eq!(div(ONE, INF), 0);
        assert_eq!(add(negate(0), negate(0)), negate(0));
        assert_eq!(add(0, negate(0)), 0);
        assert_eq!(compare(QNAN, ONE), None);
        assert_eq!(compare(0, negate(0)), Some(Ordering::Equal));
        assert_eq!(compare(negate(ONE), dec("0.5")), Some(Ordering::Less));
        assert_eq!(
            compare(negate(ONE), negate(dec("0.5"))),
            Some(Ordering::Less)
        );
        assert_eq!(nan_bits(0, false), QNAN);
        assert_eq!(nan_bits(5, false), QNAN | 5);
        assert_eq!(nan_bits(0, true), INF | (1 << 110));
        assert_eq!(nan_bits(3, true), INF | 3);
    }

    #[test]
    fn conversions() {
        for value in [0.1f64, -1.0e300, 5e-324, 2.2250738585072014e-308, 123.456] {
            let quad = wide(value);
            assert_eq!(to_ieee(quad, false), value.to_bits(), "{value}");
        }
        assert_eq!(to_ieee(dec("0.1"), false), 0.1f64.to_bits());
        assert_eq!(to_ieee(dec("0.1"), true), u64::from(0.1f32.to_bits()));
        assert_eq!(to_ieee(dec("1e400"), false), f64::INFINITY.to_bits());
        assert_eq!(to_ieee(dec("1e-400"), false), 0);
        assert_eq!(from_ieee(u64::from(1.5f32.to_bits()), true), wide(1.5));
        let nan = from_ieee(f64::NAN.to_bits(), false);
        assert_eq!(nan, QNAN);
        assert_eq!(to_ieee(QNAN, false), f64::NAN.to_bits());
        assert_eq!(from_int(1, TypeName::Int), ONE);
        assert_eq!(from_int(u128::MAX, TypeName::Int128), negate(ONE));
        assert_eq!(
            from_int(u128::MAX, TypeName::UInt128),
            parse_hex("1", "", 128)
        );
        assert_eq!(to_int(dec("1.9"), TypeName::Int), Some(1));
        assert_eq!(
            to_int(negate(dec("1.9")), TypeName::Int),
            Some(u128::from(u32::MAX))
        );
        assert_eq!(to_int(dec("0.5"), TypeName::Int), Some(0));
        assert_eq!(to_int(dec("4294967296"), TypeName::UInt), None);
        assert_eq!(to_int(dec("4294967295"), TypeName::UInt), Some(0xFFFF_FFFF));
        assert_eq!(to_int(negate(ONE), TypeName::UInt), None);
        assert_eq!(to_int(QNAN, TypeName::Bool), Some(1));
        assert_eq!(to_int(INF, TypeName::Long), None);
        assert_eq!(to_int(parse_hex("1", "", 127), TypeName::Int128), None);
        assert_eq!(
            to_int(negate(parse_hex("1", "", 127)), TypeName::Int128),
            Some(1 << 127)
        );
    }
}
