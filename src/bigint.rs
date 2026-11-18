//! Arbitrary-precision unsigned integers for exact floating-point
//! conversion and constant folding ([`crate::fp80`], [`crate::fp128`]).
//! Only what those modules need: decimal digits, powers of five, shifts,
//! products, and long division.

/// Little-endian base-2^32 integer. No trailing zero limb.
#[derive(Clone)]
pub(crate) struct Big {
    d: Vec<u32>,
}

impl Big {
    pub(crate) fn from_digits(digits: &[u8]) -> Self {
        let mut value = Big { d: Vec::new() };
        for digit in digits {
            value = value.mul_small(10).add_small(u32::from(digit - b'0'));
        }
        value
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.d.is_empty()
    }

    pub(crate) fn pow5(mut exp: u32) -> Self {
        let mut result = Big { d: vec![1] };
        let mut base = Big { d: vec![5] };
        while exp > 0 {
            if exp & 1 == 1 {
                result = result.mul(&base);
            }
            exp >>= 1;
            if exp > 0 {
                base = base.mul(&base);
            }
        }
        result
    }

    pub(crate) fn bit_length(&self) -> u32 {
        let Some(top) = self.d.last() else {
            return 0;
        };
        (self.d.len() as u32 - 1) * 32 + (32 - top.leading_zeros())
    }

    pub(crate) fn mul_small(&self, factor: u32) -> Self {
        if factor == 0 || self.is_zero() {
            return Big { d: Vec::new() };
        }
        let mut d = Vec::with_capacity(self.d.len() + 1);
        let mut carry = 0u64;
        for limb in &self.d {
            let prod = u64::from(*limb) * u64::from(factor) + carry;
            d.push(prod as u32);
            carry = prod >> 32;
        }
        if carry != 0 {
            d.push(carry as u32);
        }
        Big { d }
    }

    pub(crate) fn add_small(&self, add: u32) -> Self {
        if add == 0 {
            return self.clone();
        }
        let mut d = self.d.clone();
        if d.is_empty() {
            d.push(add);
            return Big { d };
        }
        let mut carry = u64::from(add);
        for limb in &mut d {
            let sum = u64::from(*limb) + carry;
            *limb = sum as u32;
            carry = sum >> 32;
            if carry == 0 {
                return Big { d };
            }
        }
        if carry != 0 {
            d.push(carry as u32);
        }
        Big { d }
    }

    pub(crate) fn mul(&self, other: &Self) -> Self {
        if self.is_zero() || other.is_zero() {
            return Big { d: Vec::new() };
        }
        let mut d = vec![0u32; self.d.len() + other.d.len()];
        for (i, &left) in self.d.iter().enumerate() {
            let mut carry = 0u64;
            for (j, &right) in other.d.iter().enumerate() {
                let prod = u64::from(left) * u64::from(right) + u64::from(d[i + j]) + carry;
                d[i + j] = prod as u32;
                carry = prod >> 32;
            }
            let mut k = i + other.d.len();
            while carry != 0 {
                let sum = u64::from(d[k]) + carry;
                d[k] = sum as u32;
                carry = sum >> 32;
                k += 1;
            }
        }
        while d.last() == Some(&0) {
            d.pop();
        }
        Big { d }
    }

    pub(crate) fn shl(&self, bits: u32) -> Self {
        if self.is_zero() || bits == 0 {
            return self.clone();
        }
        let limbs = (bits / 32) as usize;
        let rem = bits % 32;
        let mut d = vec![0u32; limbs];
        if rem == 0 {
            d.extend_from_slice(&self.d);
            return Big { d };
        }
        let mut carry = 0u32;
        for limb in &self.d {
            d.push((limb << rem) | carry);
            carry = limb >> (32 - rem);
        }
        if carry != 0 {
            d.push(carry);
        }
        Big { d }
    }

    pub(crate) fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.d.len().cmp(&other.d.len()) {
            std::cmp::Ordering::Equal => {}
            order => return order,
        }
        for (left, right) in self.d.iter().rev().zip(other.d.iter().rev()) {
            match left.cmp(right) {
                std::cmp::Ordering::Equal => {}
                order => return order,
            }
        }
        std::cmp::Ordering::Equal
    }

    pub(crate) fn sub(&self, other: &Self) -> Self {
        let mut d = self.d.clone();
        let mut borrow = 0i64;
        for (i, limb) in d.iter_mut().enumerate() {
            let right = i64::from(*other.d.get(i).unwrap_or(&0));
            let cur = i64::from(*limb) - right - borrow;
            if cur < 0 {
                *limb = (cur + (1i64 << 32)) as u32;
                borrow = 1;
            } else {
                *limb = cur as u32;
                borrow = 0;
            }
        }
        while d.last() == Some(&0) {
            d.pop();
        }
        Big { d }
    }

    pub(crate) fn div_rem(&self, den: &Self) -> (Self, Self) {
        if self.cmp(den) == std::cmp::Ordering::Less {
            return (Big { d: Vec::new() }, self.clone());
        }
        let shift = self.bit_length() - den.bit_length();
        let mut den_shift = den.shl(shift);
        let mut quot = Big { d: Vec::new() };
        let mut rem = self.clone();
        for i in (0..=shift).rev() {
            if rem.cmp(&den_shift) != std::cmp::Ordering::Less {
                rem = rem.sub(&den_shift);
                quot.set_bit(i);
            }
            if i > 0 {
                den_shift = den_shift.shr1();
            }
        }
        (quot, rem)
    }

    pub(crate) fn shr1(&self) -> Self {
        if self.is_zero() {
            return Big { d: Vec::new() };
        }
        let mut d = Vec::with_capacity(self.d.len());
        let mut carry = 0u32;
        for limb in self.d.iter().rev() {
            let next = (limb >> 1) | carry;
            carry = (limb & 1) << 31;
            d.push(next);
        }
        d.reverse();
        while d.last() == Some(&0) {
            d.pop();
        }
        Big { d }
    }

    pub(crate) fn set_bit(&mut self, bit: u32) {
        let limb = (bit / 32) as usize;
        if self.d.len() <= limb {
            self.d.resize(limb + 1, 0);
        }
        self.d[limb] |= 1u32 << (bit % 32);
    }

    /// Top 64 bits, plus the following round bit and any lower sticky bit.
    pub(crate) fn top_bits(&self, bitlen: u32) -> (u64, bool, bool) {
        if bitlen <= 64 {
            let sig = self.low_u64() << (64 - bitlen);
            return (sig, false, false);
        }
        let drop = bitlen - 64;
        let sig = self.bits_at(drop, 64);
        let round = self.bit(drop - 1);
        let sticky = drop > 1 && self.any_below(drop - 1);
        (sig, round, sticky)
    }

    pub(crate) fn low_u64(&self) -> u64 {
        let lo = u64::from(*self.d.first().unwrap_or(&0));
        let hi = u64::from(*self.d.get(1).unwrap_or(&0));
        lo | (hi << 32)
    }

    pub(crate) fn bit(&self, index: u32) -> bool {
        let limb = (index / 32) as usize;
        self.d
            .get(limb)
            .is_some_and(|word| word & (1u32 << (index % 32)) != 0)
    }

    pub(crate) fn any_below(&self, index: u32) -> bool {
        let limb = (index / 32) as usize;
        if self.d.iter().take(limb).any(|word| *word != 0) {
            return true;
        }
        let mask = (1u32 << (index % 32)) - 1;
        self.d.get(limb).is_some_and(|word| word & mask != 0)
    }

    pub(crate) fn bits_at(&self, start: u32, width: u32) -> u64 {
        let mut out = 0u64;
        for i in 0..width {
            if self.bit(start + i) {
                out |= 1u64 << i;
            }
        }
        out
    }

    /// Hexadecimal digit values, most significant first, packed eight to a
    /// limb (linear in the number of digits).
    pub(crate) fn from_hex_digits(nibbles: &[u8]) -> Self {
        let mut d = Vec::with_capacity(nibbles.len() / 8 + 1);
        for chunk in nibbles.rchunks(8) {
            let limb = chunk
                .iter()
                .fold(0u32, |acc, &nibble| (acc << 4) | u32::from(nibble & 0xF));
            d.push(limb);
        }
        while d.last() == Some(&0) {
            d.pop();
        }
        Big { d }
    }

    pub(crate) fn from_u128(mut value: u128) -> Self {
        let mut d = Vec::new();
        while value != 0 {
            d.push(value as u32);
            value >>= 32;
        }
        Big { d }
    }

    pub(crate) fn add(&self, other: &Self) -> Self {
        let len = self.d.len().max(other.d.len());
        let mut d = Vec::with_capacity(len + 1);
        let mut carry = 0u64;
        for i in 0..len {
            let sum = u64::from(*self.d.get(i).unwrap_or(&0))
                + u64::from(*other.d.get(i).unwrap_or(&0))
                + carry;
            d.push(sum as u32);
            carry = sum >> 32;
        }
        if carry != 0 {
            d.push(carry as u32);
        }
        Big { d }
    }

    /// `width` bits (at most 128) starting at bit `start`.
    pub(crate) fn bits_u128(&self, start: u32, width: u32) -> u128 {
        let low = self.bits_at(start, width.min(64));
        if width <= 64 {
            return u128::from(low);
        }
        u128::from(low) | (u128::from(self.bits_at(start + 64, width - 64)) << 64)
    }
}

/// A decimal floating constant's text (C11 6.4.4.2, suffix removed), read
/// exactly: the value is `digits * 10^exp10`.
pub(crate) enum DecimalText {
    /// The significand is zero, or the exponent underflows every format.
    Zero,
    /// A nonzero significand with an exponent past every format.
    Huge,
    /// Significant digits (ASCII, no leading or trailing zeros) and the
    /// power of ten of the last one.
    Digits(Vec<u8>, i64),
}

/// Scan the text of a decimal floating constant. `None` when it is not one.
pub(crate) fn scan_decimal(body: &str) -> Option<DecimalText> {
    let bytes = body.as_bytes();
    let mut index = 0;
    let mut digits = Vec::new();
    let mut frac: i32 = 0;
    let mut saw = false;
    let mut dot = false;
    while index < bytes.len() {
        match bytes[index] {
            b'0'..=b'9' => {
                digits.push(bytes[index]);
                if dot {
                    frac += 1;
                }
                saw = true;
                index += 1;
            }
            b'.' if !dot => {
                dot = true;
                index += 1;
            }
            _ => break,
        }
    }
    if !saw {
        return None;
    }
    let mut exp: i32 = 0;
    if index < bytes.len() && matches!(bytes[index], b'e' | b'E') {
        index += 1;
        let neg = if index < bytes.len() && matches!(bytes[index], b'+' | b'-') {
            let minus = bytes[index] == b'-';
            index += 1;
            minus
        } else {
            false
        };
        if index >= bytes.len() || !bytes[index].is_ascii_digit() {
            return None;
        }
        let mut overflow = false;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            let digit = i32::from(bytes[index] - b'0');
            exp = match exp
                .checked_mul(10)
                .and_then(|value| value.checked_add(digit))
            {
                Some(value) => value,
                None => {
                    overflow = true;
                    break;
                }
            };
            index += 1;
        }
        if overflow {
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if index != bytes.len() {
                return None;
            }
            // A zero significand is zero for either exponent sign. A nonzero
            // significand overflows to +infinity, or underflows to +0 when
            // the exponent is negative. The literal itself has no sign; unary
            // minus is a separate operator.
            let all_zero = digits.iter().all(|digit| *digit == b'0');
            if all_zero || neg {
                return Some(DecimalText::Zero);
            }
            return Some(DecimalText::Huge);
        }
        if neg {
            exp = -exp;
        }
    }
    if index != bytes.len() {
        return None;
    }
    let mut start = 0;
    while start < digits.len() && digits[start] == b'0' {
        start += 1;
    }
    if start == digits.len() {
        return Some(DecimalText::Zero);
    }
    while digits.len() > start && digits.last() == Some(&b'0') {
        digits.pop();
        frac -= 1;
    }
    let exp10 = i64::from(exp) - i64::from(frac);
    Some(DecimalText::Digits(digits[start..].to_vec(), exp10))
}
