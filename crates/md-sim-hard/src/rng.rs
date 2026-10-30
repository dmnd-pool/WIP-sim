//! Small deterministic generator. A sequence must replay from its seed, so
//! the generator has no hidden state and no external randomness.

#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// SplitMix64 step.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    pub fn u32(&mut self) -> u32 {
        self.next_u64() as u32
    }

    /// Uniform value in `0..n`. Zero for `n == 0`.
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next_u64() % n
        }
    }

    /// Uniform value in the inclusive range `low..=high`.
    pub fn range(&mut self, low: u64, high: u64) -> u64 {
        low + self.below(high - low + 1)
    }

    pub fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    pub fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u64) as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::Rng;

    #[test]
    fn same_seed_gives_same_values() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        let first: Vec<u64> = (0..8).map(|_| a.next_u64()).collect();
        let second: Vec<u64> = (0..8).map(|_| b.next_u64()).collect();
        assert_eq!(first, second);
        assert_ne!(first[0], first[1]);
    }

    #[test]
    fn range_stays_inside_bounds() {
        let mut r = Rng::new(1);
        for _ in 0..1000 {
            let v = r.range(3, 5);
            assert!((3..=5).contains(&v));
        }
    }
}
