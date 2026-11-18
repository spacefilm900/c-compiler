//! x87 80-bit extended precision, the System V representation of `long double`.
//!
//! A value is 1 sign bit, 15 exponent bits with bias 16383, and a 64-bit
//! significand with an explicit integer bit. Normals keep that bit set.
//! Subnormals use a zero exponent field and a clear integer bit; the least
//! subnormal is 2^-16445. Infinities use an all-ones exponent and a
//! significand of 2^63. The low 80 bits of a `u128` hold that pattern, which
//! is also the little-endian memory image of the first 10 bytes. LLVM spells
//! the same bits as `0xK` followed by 20 hex digits.
//!
//! Decimal conversion rounds to nearest, ties to even, which is the rounding
//! this target uses for C11 6.4.4.2. Arithmetic here is the same rounding, so
//! a constant initializer matches a runtime `x86_fp80` operation.

use crate::ast::{sign_extend, TypeName};
use crate::bigint::{scan_decimal, Big, DecimalText};

const BIAS: i32 = 16383;
const EMAX: i32 = 16383;
const EMIN: i32 = -16382;
/// Power of two of the least bit of a subnormal significand.
const SUB_SCALE: i32 = -16445;
const INF: u128 = 0x7FFF_8000_0000_0000_0000;
const QNAN: u128 = 0x7FFF_C000_0000_0000_0000;

pub fn parse_decimal(body: &str) -> Option<u128> {
    Some(match scan_decimal(body)? {
        DecimalText::Zero => 0,
        DecimalText::Huge => INF,
        DecimalText::Digits(digits, exp10) => decimal_to_bits(&digits, exp10),
    })
}

pub fn parse_hex(int_digits: &str, frac_digits: &str, bin_exp: i32) -> u128 {
    if int_digits
        .bytes()
        .chain(frac_digits.bytes())
        .all(|byte| byte == b'0')
    {
        return 0;
    }
    hex_bits(int_digits, frac_digits, bin_exp)
}

/// +infinity: all-ones exponent and a significand of 2^63.
pub fn plus_inf() -> u128 {
    INF
}

/// A quiet or signaling NaN, with `payload` in the low bits of the 64-bit
/// significand. The integer bit is set. A quiet NaN also has bit 62 set; a
/// signaling NaN has it clear, and bit 61 is set when the rest of the
/// field would otherwise be zero (which would encode an infinity).
pub fn nan_bits(payload: u128, signaling: bool) -> u128 {
    const INTEGER: u128 = 1 << 63;
    const QUIET: u128 = 1 << 62;
    const DEFAULT_SIGNAL: u128 = 1 << 61;
    let mut frac = payload & ((1u128 << 64) - 1);
    frac |= INTEGER;
    if signaling {
        frac &= !QUIET;
        if frac & !INTEGER == 0 {
            frac |= DEFAULT_SIGNAL;
        }
    } else {
        frac |= QUIET;
    }
    (0x7FFFu128 << 64) | frac
}

pub fn is_zero(bits: u128) -> bool {
    bits & ((1u128 << 79) - 1) == 0
}

pub fn negate(bits: u128) -> u128 {
    bits ^ (1u128 << 79)
}

pub fn add(left: u128, right: u128) -> u128 {
    arith(left, right, false)
}

pub fn sub(left: u128, right: u128) -> u128 {
    arith(left, negate(right), false)
}

pub fn mul(left: u128, right: u128) -> u128 {
    match classify(left, right) {
        Class::Nan => QNAN,
        Class::InfZero => QNAN,
        Class::Inf(sign) => signed_inf(sign),
        Class::Zero(sign) => signed_zero(sign),
        Class::Finite(left, right) => {
            let sign = left.sign ^ right.sign;
            let prod = u128::from(left.sig) * u128::from(right.sig);
            if prod == 0 {
                return signed_zero(sign);
            }
            let scale0 = left.scale + right.scale;
            finish(sign, scale0, prod, false)
        }
    }
}

pub fn div(left: u128, right: u128) -> u128 {
    if is_nan(left) || is_nan(right) {
        return QNAN;
    }
    let sign = sign_of(left) ^ sign_of(right);
    if is_inf(left) {
        return if is_inf(right) {
            QNAN
        } else {
            signed_inf(sign)
        };
    }
    if is_inf(right) || (is_zero(left) && !is_zero(right)) {
        return signed_zero(sign);
    }
    if is_zero(right) {
        return if is_zero(left) {
            QNAN
        } else {
            signed_inf(sign)
        };
    }
    let left = decode(left);
    let right = decode(right);
    if right.sig == 0 {
        return if left.sig == 0 {
            QNAN
        } else {
            signed_inf(sign)
        };
    }
    // Two 64-bit quotient chunks. One chunk has no guard bit when the
    // quotient is an exact 64-bit integer, so the remainder would only be
    // a sticky flag and ties would not round.
    let denom = u128::from(right.sig);
    let numer = u128::from(left.sig) << 64;
    let q1 = numer / denom;
    let r1 = numer % denom;
    let q2 = (r1 << 64) / denom;
    let sticky = (r1 << 64) % denom != 0;
    let scale_q1 = left.scale - right.scale - 64;
    if q1 < (1u128 << 64) {
        let acc = (q1 << 64) | q2;
        finish(sign, scale_q1 - 64, acc, sticky)
    } else {
        let lost = q2 & 1 == 1;
        let acc = (q1 << 63) | (q2 >> 1);
        finish(sign, scale_q1 - 63, acc, sticky || lost)
    }
}

pub fn cmp(op: Cmp, left: u128, right: u128) -> bool {
    if is_nan(left) || is_nan(right) {
        return op == Cmp::Ne;
    }
    if is_zero(left) && is_zero(right) {
        return matches!(op, Cmp::Eq | Cmp::Le | Cmp::Ge);
    }
    let sign_l = sign_of(left);
    let sign_r = sign_of(right);
    let order = if sign_l != sign_r {
        if sign_l {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    } else {
        let mag = cmp_mag(&decode(left), &decode(right));
        if sign_l {
            mag.reverse()
        } else {
            mag
        }
    };
    match op {
        Cmp::Eq => order.is_eq(),
        Cmp::Ne => order.is_ne(),
        Cmp::Lt => order.is_lt(),
        Cmp::Gt => order.is_gt(),
        Cmp::Le => order.is_le(),
        Cmp::Ge => order.is_ge(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Cmp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

pub fn from_int(bits: u128, src: TypeName) -> u128 {
    let signed = src.is_signed();
    let wide = if signed {
        sign_extend(bits, src.width())
    } else {
        0
    };
    let mag: u128 = if signed {
        wide.unsigned_abs()
    } else {
        bits & src.mask()
    };
    if mag == 0 {
        return 0;
    }
    let sign = signed && wide < 0;
    let msb = 127 - mag.leading_zeros();
    if msb <= 63 {
        let sig = (mag as u64) << (63 - msb);
        return pack(sign, msb as i32, sig);
    }
    let shift = msb - 63;
    let mut sig = (mag >> shift) as u64;
    let rest = mag & ((1u128 << shift) - 1);
    let half = 1u128 << (shift - 1);
    let mut exp = msb as i32;
    if rest > half || (rest == half && (sig & 1) != 0) {
        if sig == u64::MAX {
            sig = 1u64 << 63;
            exp += 1;
        } else {
            sig += 1;
        }
    }
    pack(sign, exp, sig)
}

pub fn to_int(bits: u128, dest: TypeName) -> Option<u128> {
    if dest == TypeName::Bool {
        return Some(u128::from(!is_zero(bits)));
    }
    if is_nan(bits) || is_inf(bits) {
        return None;
    }
    let part = decode(bits);
    if part.sig == 0 {
        return Some(0);
    }
    // value = sig * 2^(exp-63) with a 64-bit significand, integer bit at 63.
    let exp = part.scale + 63;
    if exp < 0 {
        return Some(0);
    }
    let mag: u128 = if exp <= 63 {
        u128::from(part.sig >> (63 - exp))
    } else {
        let up = (exp - 63) as u32;
        if up > 64 {
            return None;
        }
        (part.sig as u128) << up
    };
    if dest.is_signed() {
        let (min, max) = crate::ast::signed_limits(dest.width());
        let value = if part.sign {
            if mag > 1u128 << 127 {
                return None;
            }
            if mag == 1u128 << 127 {
                i128::MIN
            } else {
                -(mag as i128)
            }
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
        if part.sign && mag != 0 {
            return None;
        }
        if mag > dest.mask() {
            return None;
        }
        Some(mag)
    }
}

pub fn from_ieee(bits: u64, is_f32: bool) -> u128 {
    let (sign, exp, frac, prec, bias) = if is_f32 {
        (
            (bits >> 31) & 1 == 1,
            ((bits >> 23) & 0xFF) as i32,
            bits & 0x7F_FFFF,
            24i32,
            127i32,
        )
    } else {
        (
            bits >> 63 == 1,
            ((bits >> 52) & 0x7FF) as i32,
            bits & 0x000F_FFFF_FFFF_FFFF,
            53,
            1023,
        )
    };
    let frac_bits = prec - 1;
    if exp == 0 {
        if frac == 0 {
            return signed_zero(sign);
        }
        let msb = 63 - frac.leading_zeros();
        let unbiased = (1 - bias) - frac_bits + msb as i32;
        let sig = frac << (63 - msb);
        return pack(sign, unbiased, sig);
    }
    let max_exp = if is_f32 { 0xFF } else { 0x7FF };
    if exp == max_exp {
        if frac == 0 {
            return signed_inf(sign);
        }
        return if sign { negate(QNAN) } else { QNAN };
    }
    let sig = (1u64 << 63) | (frac << (63 - frac_bits));
    let unbiased = exp - bias;
    pack(sign, unbiased, sig)
}

pub fn to_ieee(bits: u128, is_f32: bool) -> u64 {
    let prec: i32 = if is_f32 { 24 } else { 53 };
    let emax: i32 = if is_f32 { 127 } else { 1023 };
    let bias = emax;
    let sign = sign_of(bits);
    if is_nan(bits) {
        return if is_f32 {
            u64::from(f32::NAN.to_bits()) | if sign { 0x8000_0000 } else { 0 }
        } else if sign {
            f64::from_bits(f64::NAN.to_bits() | (1u64 << 63)).to_bits()
        } else {
            f64::NAN.to_bits()
        };
    }
    if is_inf(bits) {
        return if is_f32 {
            u64::from(if sign {
                f32::NEG_INFINITY.to_bits()
            } else {
                f32::INFINITY.to_bits()
            })
        } else if sign {
            f64::NEG_INFINITY.to_bits()
        } else {
            f64::INFINITY.to_bits()
        };
    }
    let part = decode(bits);
    if part.sig == 0 {
        return if is_f32 {
            if sign {
                0x8000_0000
            } else {
                0
            }
        } else if sign {
            1u64 << 63
        } else {
            0
        };
    }
    let exp = part.scale + 63;
    // Keep `prec` bits starting at the integer bit.
    let drop = 64 - prec;
    let mut mant = part.sig >> drop;
    let round = drop > 0 && ((part.sig >> (drop - 1)) & 1) == 1;
    let sticky = drop > 1 && part.sig & ((1u64 << (drop - 1)) - 1) != 0;
    if round && (sticky || mant & 1 == 1) {
        mant += 1;
        if mant == (1u64 << prec) {
            mant >>= 1;
            return pack_ieee(sign, exp + 1, mant, prec, emax, bias, is_f32);
        }
    }
    pack_ieee(sign, exp, mant, prec, emax, bias, is_f32)
}

fn pack_ieee(
    sign: bool,
    exp: i32,
    mant: u64,
    prec: i32,
    emax: i32,
    bias: i32,
    is_f32: bool,
) -> u64 {
    let mut exp = exp;
    let mut mant = mant;
    if exp > emax {
        return if is_f32 {
            u64::from(if sign {
                f32::NEG_INFINITY.to_bits()
            } else {
                f32::INFINITY.to_bits()
            })
        } else if sign {
            f64::NEG_INFINITY.to_bits()
        } else {
            f64::INFINITY.to_bits()
        };
    }
    let emin = 1 - bias;
    if exp < emin {
        let shift = emin - exp;
        if shift > prec {
            return if sign {
                if is_f32 {
                    0x8000_0000
                } else {
                    1u64 << 63
                }
            } else {
                0
            };
        }
        let round = if shift == 0 {
            false
        } else {
            ((mant >> (shift - 1)) & 1) == 1
        };
        let sticky = shift > 1 && mant & ((1u64 << (shift - 1)) - 1) != 0;
        mant >>= shift;
        if round && (sticky || mant & 1 == 1) {
            mant += 1;
        }
        let limit = 1u64 << (prec - 1);
        if mant >= limit {
            exp = emin;
        } else {
            return put_ieee(sign, 0, mant, is_f32);
        }
    }
    let frac = mant & ((1u64 << (prec - 1)) - 1);
    put_ieee(sign, (exp + bias) as u32, frac, is_f32)
}

fn put_ieee(sign: bool, exp: u32, frac: u64, is_f32: bool) -> u64 {
    if is_f32 {
        let bits = (exp << 23) | (frac as u32 & 0x007F_FFFF);
        u64::from(if sign { bits | 0x8000_0000 } else { bits })
    } else {
        let bits = (u64::from(exp) << 52) | (frac & 0x000F_FFFF_FFFF_FFFF);
        if sign {
            bits | (1u64 << 63)
        } else {
            bits
        }
    }
}

fn arith(left: u128, right: u128, _sub: bool) -> u128 {
    if is_nan(left) || is_nan(right) {
        return QNAN;
    }
    if is_inf(left) && is_inf(right) {
        return if sign_of(left) == sign_of(right) {
            signed_inf(sign_of(left))
        } else {
            QNAN
        };
    }
    if is_inf(left) {
        return left & !((1u128 << 64) - 1) | (1u128 << 63);
    }
    if is_inf(right) {
        return right & !((1u128 << 64) - 1) | (1u128 << 63);
    }
    if is_zero(left) && is_zero(right) {
        return if sign_of(left) && sign_of(right) {
            signed_zero(true)
        } else {
            0
        };
    }
    if is_zero(left) {
        return right;
    }
    if is_zero(right) {
        return left;
    }
    let left_p = decode(left);
    let right_p = decode(right);
    if left_p.sign == right_p.sign {
        let mag = add_mag(&left_p, &right_p);
        return encode(left_p.sign, mag);
    }
    match cmp_mag(&left_p, &right_p) {
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => encode(left_p.sign, sub_mag(&left_p, &right_p)),
        std::cmp::Ordering::Less => encode(right_p.sign, sub_mag(&right_p, &left_p)),
    }
}

#[derive(Clone, Copy)]
struct Parts {
    sign: bool,
    /// Weight of significand bit 0. `value = sig * 2^scale`.
    scale: i32,
    sig: u64,
}

fn decode(bits: u128) -> Parts {
    let sign = sign_of(bits);
    let exp = exp_field(bits);
    let sig = sig_field(bits);
    if exp == 0 {
        return Parts {
            sign,
            scale: SUB_SCALE,
            sig,
        };
    }
    Parts {
        sign,
        scale: exp as i32 - BIAS - 63,
        sig,
    }
}

fn encode(sign: bool, part: Parts) -> u128 {
    if part.sig == 0 {
        return signed_zero(sign);
    }
    // `part` is already normalized with the integer bit at bit 63, or it is
    // a subnormal whose scale is SUB_SCALE.
    let exp = part.scale + 63;
    if exp > EMAX {
        return signed_inf(sign);
    }
    if exp >= EMIN {
        return pack(sign, exp, part.sig);
    }
    let shift = EMIN - exp;
    shift_subnormal(sign, part.sig, shift, false, false)
}

fn add_mag(big_in: &Parts, small_in: &Parts) -> Parts {
    let (big, small) = if big_in.scale >= small_in.scale {
        (big_in, small_in)
    } else {
        (small_in, big_in)
    };
    let delta = (big.scale - small.scale) as u32;
    let mut acc = u128::from(big.sig) << 64;
    let (addend, lost) = shr_sticky(u128::from(small.sig) << 64, delta);
    let (sum, carry) = acc.overflowing_add(addend);
    acc = sum;
    if carry {
        let round = (acc >> 64) & 1 == 1;
        let sticky = acc & ((1u128 << 64) - 1) != 0 || lost;
        let sig = (1u64 << 63) | ((acc >> 65) as u64);
        return round_parts(big.scale + 1, sig, round, sticky);
    }
    normalize(big.scale - 64, acc, lost)
}

fn sub_mag(big: &Parts, small: &Parts) -> Parts {
    let delta = (big.scale - small.scale) as u32;
    let mut acc = u128::from(big.sig) << 64;
    let (subend, lost) = shr_sticky(u128::from(small.sig) << 64, delta);
    if lost {
        acc = acc.wrapping_sub(subend).wrapping_sub(1);
    } else {
        acc -= subend;
    }
    normalize(big.scale - 64, acc, lost)
}

/// `scale0` is the weight of bit 0 of `acc`.
fn normalize(scale0: i32, acc: u128, sticky_in: bool) -> Parts {
    if acc == 0 {
        return Parts {
            sign: false,
            scale: 0,
            sig: 0,
        };
    }
    let msb = 127 - acc.leading_zeros() as i32;
    if msb >= 63 {
        let shift = (msb - 63) as u32;
        let sig = (acc >> shift) as u64;
        let round = shift > 0 && ((acc >> (shift - 1)) & 1) == 1;
        let sticky = sticky_in || (shift > 1 && acc & ((1u128 << (shift - 1)) - 1) != 0);
        round_parts(scale0 + msb - 63, sig, round, sticky)
    } else {
        let sig = (acc << (63 - msb)) as u64;
        round_parts(scale0 + msb - 63, sig, false, sticky_in)
    }
}

fn finish(sign: bool, scale0: i32, acc: u128, sticky: bool) -> u128 {
    let part = normalize(scale0, acc, sticky);
    encode(sign, part)
}

fn round_parts(scale: i32, mut sig: u64, round: bool, sticky: bool) -> Parts {
    if round && (sticky || sig & 1 == 1) {
        sig = sig.wrapping_add(1);
        if sig == 0 {
            return Parts {
                sign: false,
                scale: scale + 1,
                sig: 1u64 << 63,
            };
        }
    }
    Parts {
        sign: false,
        scale,
        sig,
    }
}

fn shift_subnormal(sign: bool, sig: u64, shift: i32, round: bool, sticky: bool) -> u128 {
    if shift >= 65 {
        return signed_zero(sign);
    }
    if shift == 64 {
        let top = sig >> 63 == 1;
        let rest = sig & !(1u64 << 63) != 0 || round || sticky;
        return if top && rest {
            pack_raw(sign, 0, 1)
        } else {
            signed_zero(sign)
        };
    }
    if shift <= 0 {
        return pack_raw(sign, 0, sig);
    }
    let shift = shift as u32;
    let new_round = ((sig >> (shift - 1)) & 1) == 1;
    let new_sticky = sticky || round || (shift > 1 && sig & ((1u64 << (shift - 1)) - 1) != 0);
    let mut newsig = sig >> shift;
    if new_round && (new_sticky || newsig & 1 == 1) {
        newsig += 1;
    }
    if newsig == 0 {
        return signed_zero(sign);
    }
    if newsig >= (1u64 << 63) {
        return pack(sign, EMIN, 1u64 << 63);
    }
    pack_raw(sign, 0, newsig)
}

fn cmp_mag(left: &Parts, right: &Parts) -> std::cmp::Ordering {
    if left.sig == 0 && right.sig == 0 {
        return std::cmp::Ordering::Equal;
    }
    if left.sig == 0 {
        return std::cmp::Ordering::Less;
    }
    if right.sig == 0 {
        return std::cmp::Ordering::Greater;
    }
    if left.scale >= right.scale {
        let delta = (left.scale - right.scale) as u32;
        if delta >= 64 {
            return std::cmp::Ordering::Greater;
        }
        let av = u128::from(left.sig) << delta;
        let bv = u128::from(right.sig);
        av.cmp(&bv)
    } else {
        cmp_mag(right, left).reverse()
    }
}

fn shr_sticky(value: u128, shift: u32) -> (u128, bool) {
    if shift == 0 {
        return (value, false);
    }
    if shift >= 128 {
        return (0, value != 0);
    }
    let sticky = value & ((1u128 << shift) - 1) != 0;
    (value >> shift, sticky)
}

enum Class {
    Nan,
    InfZero,
    Inf(bool),
    Zero(bool),
    Finite(Parts, Parts),
}

fn classify(left: u128, right: u128) -> Class {
    if is_nan(left) || is_nan(right) {
        return Class::Nan;
    }
    let left_inf = is_inf(left);
    let right_inf = is_inf(right);
    let left_zero = is_zero(left);
    let right_zero = is_zero(right);
    let sign = sign_of(left) ^ sign_of(right);
    if (left_inf && right_zero) || (right_inf && left_zero) {
        return Class::InfZero;
    }
    if left_inf || right_inf {
        return Class::Inf(sign);
    }
    if left_zero || right_zero {
        return Class::Zero(sign);
    }
    Class::Finite(decode(left), decode(right))
}

fn decimal_to_bits(digits: &[u8], exp10: i64) -> u128 {
    if exp10 > 20_000 {
        return INF;
    }
    if exp10 < -20_000 {
        return 0;
    }
    let mant = Big::from_digits(digits);
    if mant.is_zero() {
        return 0;
    }
    if exp10 >= 0 {
        let num = mant.mul(&Big::pow5(exp10 as u32));
        let bitlen = num.bit_length();
        if bitlen == 0 {
            return 0;
        }
        let (sig, round, sticky) = num.top_bits(bitlen);
        let scale = (bitlen as i32 - 1) + exp10 as i32 - 63;
        return encode(false, round_parts(scale, sig, round, sticky));
    }
    let k = (-exp10) as u32;
    let den = Big::pow5(k);
    let den_bits = den.bit_length().max(1);
    let num_bits = mant.bit_length().max(1);
    // Enough quotient bits for a 64-bit significand plus a guard. A huge
    // mantissa already supplies them, so the shift stays non-negative.
    let extra = (den_bits as i32 + 96 - num_bits as i32).max(0) as u32;
    let shifted = mant.shl(extra);
    let (quot, rem) = shifted.div_rem(&den);
    let bitlen = quot.bit_length();
    if bitlen == 0 {
        return 0;
    }
    let (sig, round, mut sticky) = quot.top_bits(bitlen);
    if !rem.is_zero() {
        sticky = true;
    }
    // quot ≈ mant / 5^k * 2^extra, and the value is that times 2^(-k).
    let scale = (bitlen as i32 - 1) - (extra as i32 + k as i32) - 63;
    encode(false, round_parts(scale, sig, round, sticky))
}

fn hex_bits(int_digits: &str, frac_digits: &str, bin_exp: i32) -> u128 {
    const PREC: i32 = 64;
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
    if unbiased > i64::from(EMAX) {
        return INF;
    }
    if unbiased < i64::from(EMIN) {
        return hex_subnormal(sig, exp_lsb, sticky);
    }
    let shift = msb - i64::from(PREC - 1);
    let (mant, round, more) = split_sig(sig, shift, sticky);
    let mut mant = mant;
    let mut exp = unbiased;
    if round && (more || mant & 1 == 1) {
        mant = mant.wrapping_add(1);
        if mant == 0 {
            mant = 1u64 << 63;
            exp += 1;
        }
    }
    if exp > i64::from(EMAX) {
        return INF;
    }
    pack(false, exp as i32, mant)
}

fn hex_subnormal(sig: u128, exp_lsb: i64, sticky: bool) -> u128 {
    let lsb_exp = i64::from(EMIN) - 63;
    let shift = lsb_exp - exp_lsb;
    let (mant, round, more) = if shift <= 0 {
        let shifted = sig << ((-shift) as u32);
        (shifted as u64, false, sticky)
    } else {
        split_sig(sig, shift, sticky)
    };
    let mut mant = mant;
    if round && (more || mant & 1 == 1) {
        mant = mant.wrapping_add(1);
    }
    if mant >= (1u64 << 63) {
        return pack(false, EMIN, 1u64 << 63);
    }
    pack_raw(false, 0, mant)
}

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
        return (0, false, sticky || sig != 0);
    }
    let shift = shift as u32;
    let mag = (sig >> shift) as u64;
    let round = ((sig >> (shift - 1)) & 1) == 1;
    let rest = shift > 1 && sig & ((1u128 << (shift - 1)) - 1) != 0;
    (mag, round, sticky || rest)
}

fn sign_of(bits: u128) -> bool {
    (bits >> 79) & 1 == 1
}

fn exp_field(bits: u128) -> u32 {
    ((bits >> 64) & 0x7FFF) as u32
}

fn sig_field(bits: u128) -> u64 {
    bits as u64
}

fn is_nan(bits: u128) -> bool {
    exp_field(bits) == 0x7FFF && sig_field(bits) != (1u64 << 63)
}

fn is_inf(bits: u128) -> bool {
    exp_field(bits) == 0x7FFF && sig_field(bits) == (1u64 << 63)
}

fn signed_inf(sign: bool) -> u128 {
    if sign {
        negate(INF)
    } else {
        INF
    }
}

fn signed_zero(sign: bool) -> u128 {
    if sign {
        1u128 << 79
    } else {
        0
    }
}

fn pack(sign: bool, unbiased: i32, sig: u64) -> u128 {
    if unbiased > EMAX {
        return signed_inf(sign);
    }
    if unbiased < EMIN {
        let shift = EMIN - unbiased;
        return shift_subnormal(sign, sig, shift, false, false);
    }
    pack_raw(sign, (unbiased + BIAS) as u32, sig)
}

fn pack_raw(sign: bool, exp: u32, sig: u64) -> u128 {
    let mut bits = u128::from(sig) | (u128::from(exp & 0x7FFF) << 64);
    if sign {
        bits |= 1u128 << 79;
    }
    bits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(text: &str) -> u128 {
        parse_decimal(text).unwrap()
    }

    #[test]
    fn known_constants_match_x87_rounding() {
        assert_eq!(dec("1.0"), 0x3FFF_8000_0000_0000_0000);
        assert_eq!(dec("2.0"), 0x4000_8000_0000_0000_0000);
        assert_eq!(dec("0.5"), 0x3FFE_8000_0000_0000_0000);
        assert_eq!(dec("0.1"), 0x3FFB_CCCC_CCCC_CCCC_CCCD);
        assert_eq!(dec("10"), 0x4002_A000_0000_0000_0000);
        assert_eq!(dec("3.14159265358979323846"), 0x4000_C90F_DAA2_2168_C235);
        assert_eq!(
            dec("1.18973149535723176502e+4932"),
            0x7FFE_FFFF_FFFF_FFFF_FFFF
        );
        assert_eq!(
            dec("3.36210314311209350626e-4932"),
            0x0001_8000_0000_0000_0000
        );
        assert_eq!(
            dec("1.08420217248550443401e-19"),
            0x3FC0_8000_0000_0000_0000
        );
        assert_eq!(dec("3.64519953188247460253e-4951"), 1);
        assert_eq!(dec("1e4932"), 0x7FFE_D72C_B2A9_5C7E_F6CD);
        assert_eq!(dec("1e-4951"), 0);
        assert_eq!(dec("0e99999999999"), 0);
        assert_eq!(dec("0.0e2147483648"), 0);
        assert_eq!(dec("0e-99999999999"), 0);
        assert!(is_inf(dec("1e99999999999")));
        assert_eq!(parse_hex("1", "8", 1), 0x4000_C000_0000_0000_0000);
        assert_eq!(parse_hex("1", "", 0), 0x3FFF_8000_0000_0000_0000);
    }

    #[test]
    fn arithmetic_matches_extended_rounding() {
        let one = dec("1.0");
        let two = dec("2.0");
        let three = parse_hex("1", "8", 1);
        assert_eq!(add(one, one), two);
        assert_eq!(add(one, two), three);
        assert_eq!(sub(two, one), one);
        assert_eq!(mul(two, two), dec("4.0"));
        assert_eq!(div(dec("10"), two), dec("5"));
        assert_eq!(add(one, negate(one)), 0);
        assert!(is_inf(div(one, 0)));
        assert!(is_nan(div(0, 0)));
        assert_eq!(add(dec("0.1"), dec("0.2")), 0x3FFD_9999_9999_9999_999A);
        assert_eq!(sub(one, dec("0.1")), 0x3FFE_E666_6666_6666_6666);
        assert_eq!(mul(dec("0.1"), dec("10")), one);
        assert_eq!(div(one, dec("3")), 0x3FFD_AAAA_AAAA_AAAA_AAAB);
        assert_eq!(add(dec("1e20"), one), 0x4041_AD78_EBC5_AC62_0000);
        assert_eq!(add(one, dec("1e-20")), one);
        assert_eq!(negate(dec("0.1")), 0xBFFB_CCCC_CCCC_CCCC_CCCD);
        assert_eq!(mul(dec("3.64519953188247460253e-4951"), dec("3")), 3);
        assert!(is_inf(add(
            dec("1.18973149535723176502e+4932"),
            dec("1.18973149535723176502e+4932")
        )));
        assert_eq!(
            sub(mul(dec("2.5"), dec("4")), dec("0.5")),
            0x4002_9800_0000_0000_0000
        );
        assert_eq!(negate(0), 0x8000_0000_0000_0000_0000);
    }
}
