/// SplitMix64, selected for its small, fully specified, stable integer algorithm.
///
/// The implementation is owned by this crate so dependency upgrades cannot silently alter replay.
#[derive(Debug, Clone)]
pub(crate) struct StableRng {
    state: u64,
}

impl StableRng {
    pub(crate) fn for_event(seed: u64, sequence: u64) -> Self {
        Self {
            // The odd multiplier is bijective over u64 for a fixed seed, so distinct sequence
            // values receive distinct initial states before SplitMix64's output permutation.
            state: seed.wrapping_add(sequence.wrapping_mul(0xd134_2543_de82_ef95)),
        }
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    pub(crate) fn below(&mut self, upper_exclusive: u64) -> u64 {
        debug_assert!(upper_exclusive > 0);
        let threshold = upper_exclusive.wrapping_neg() % upper_exclusive;
        loop {
            let value = self.next_u64();
            if value >= threshold {
                return value % upper_exclusive;
            }
        }
    }

    pub(crate) fn usize_inclusive(&mut self, min: usize, max: usize) -> usize {
        let width = (max - min) as u64 + 1;
        min + self.below(width) as usize
    }

    pub(crate) fn i32_inclusive(&mut self, min: i32, max: i32) -> i32 {
        let width = (i64::from(max) - i64::from(min) + 1) as u64;
        (i64::from(min) + self.below(width) as i64) as i32
    }

    pub(crate) fn f32_inclusive(&mut self, min: f32, max: f32) -> f32 {
        if min == max {
            return min;
        }
        let unit = (self.next_u64() >> 11) as f64 / ((1_u64 << 53) - 1) as f64;
        (f64::from(min) + unit * (f64::from(max) - f64::from(min))) as f32
    }

    pub(crate) fn fill_bytes(&mut self, bytes: &mut [u8]) {
        for chunk in bytes.chunks_mut(8) {
            let random = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&random[..chunk.len()]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_matches_published_reference_vector() {
        let mut rng = StableRng::for_event(0, 0);
        assert_eq!(rng.next_u64(), 0xe220_a839_7b1d_cdaf);
        assert_eq!(rng.next_u64(), 0x6e78_9e6a_a1b9_65f4);
    }

    #[test]
    fn event_sequences_are_independently_reproducible() {
        let first = StableRng::for_event(42, 7).next_u64();
        let replay = StableRng::for_event(42, 7).next_u64();
        let next = StableRng::for_event(42, 8).next_u64();
        assert_eq!(first, replay);
        assert_ne!(first, next);
    }
}
