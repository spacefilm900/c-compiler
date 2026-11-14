//! A small, fast, non-cryptographic hasher for the compiler's own string
//! tables (macro names, hide sets). Hash-flooding resistance, which the
//! standard SipHash provides, is not needed for these. The mixing step is
//! the multiply-rotate scheme known as FxHash.
//!
//! No output depends on map iteration order, so the hasher choice cannot
//! change the generated IR.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

#[derive(Default, Clone, Copy)]
pub struct FxHasher {
    hash: u64,
}

impl FxHasher {
    #[inline]
    fn add(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            self.add(u64::from_le_bytes(chunk.try_into().unwrap()));
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut buf = [0u8; 8];
            buf[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(buf));
        }
    }

    #[inline]
    fn write_u8(&mut self, value: u8) {
        self.add(u64::from(value));
    }

    #[inline]
    fn write_u32(&mut self, value: u32) {
        self.add(u64::from(value));
    }

    #[inline]
    fn write_u64(&mut self, value: u64) {
        self.add(value);
    }

    #[inline]
    fn write_usize(&mut self, value: usize) {
        self.add(value as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

pub type FxBuild = BuildHasherDefault<FxHasher>;
pub type FxHashMap<K, V> = HashMap<K, V, FxBuild>;
pub type FxHashSet<T> = HashSet<T, FxBuild>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_lengths_and_contents() {
        let mut set = FxHashSet::default();
        for name in ["", "a", "ab", "abcdefgh", "abcdefghi", "abcdefgh\0"] {
            assert!(set.insert(name.to_string()), "{name:?}");
        }
        assert!(set.contains("abcdefghi"));
        assert!(!set.contains("abcdefgx"));
    }
}
