#[derive(Clone, Copy, Debug)]
pub struct JavaRandom {
    seed: i64,
    spare_gaussian: Option<f64>,
}

const MULTIPLIER: i64 = 0x5DEEC_E66D;
const ADDEND: i64 = 0xB;
const MASK: i64 = (1 << 48) - 1;

const FLOAT_MULTIPLIER: f32 = 5.960_464_5e-8;

const DOUBLE_MULTIPLIER: f64 = 1.110_223_024_625_156_5e-16;

impl JavaRandom {
    pub fn new(seed: i64) -> JavaRandom {
        JavaRandom {
            seed: (seed ^ MULTIPLIER) & MASK,
            spare_gaussian: None,
        }
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = self.seed.wrapping_mul(MULTIPLIER).wrapping_add(ADDEND) & MASK;
        (self.seed >> (48 - bits)) as i32
    }

    pub fn next_int(&mut self, bound: u32) -> u32 {
        if bound == 0 {
            return 0;
        }
        let bound = bound as i32;
        if bound & -bound == bound {
            return (((bound as i64) * (self.next(31) as i64)) >> 31) as u32;
        }
        loop {
            let bits = self.next(31);
            let value = bits % bound;
            if bits.wrapping_sub(value).wrapping_add(bound - 1) >= 0 {
                return value as u32;
            }
        }
    }

    pub fn next_f32(&mut self) -> f32 {
        self.next(24) as f32 * FLOAT_MULTIPLIER
    }

    pub fn next_f64(&mut self) -> f64 {
        let upper = self.next(26) as i64;
        let lower = self.next(27) as i64;
        ((upper << 27) + lower) as f64 * DOUBLE_MULTIPLIER
    }

    pub fn next_gaussian(&mut self) -> f64 {
        if let Some(spare) = self.spare_gaussian.take() {
            return spare;
        }
        loop {
            let v1 = 2.0 * self.next_f64() - 1.0;
            let v2 = 2.0 * self.next_f64() - 1.0;
            let s = v1 * v1 + v2 * v2;
            if s >= 1.0 || s == 0.0 {
                continue;
            }
            let multiplier = (-2.0 * s.ln() / s).sqrt();
            self.spare_gaussian = Some(v2 * multiplier);
            return v1 * multiplier;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_generator_matches_java() {
        let mut r = JavaRandom::new(0);
        assert!((r.next_f32() - 0.730_967_8).abs() < 1e-6);
        assert!((r.next_f32() - 0.831_441_0).abs() < 1e-6);
    }

    #[test]
    fn next_int_is_deterministic_and_bounded() {
        let mut x = JavaRandom::new(12345);
        let mut y = JavaRandom::new(12345);
        assert_eq!(x.next_int(4), y.next_int(4));
        let mut random = JavaRandom::new(987);
        for bound in [1u32, 2, 3, 4, 5, 7, 8, 16, 100] {
            for _ in 0..64 {
                assert!(random.next_int(bound) < bound);
            }
        }
    }

    #[test]
    fn a_zero_bound_is_zero() {
        assert_eq!(JavaRandom::new(1).next_int(0), 0);
    }

    #[test]
    fn next_f64_draws_two_words() {
        let mut a = JavaRandom::new(7);
        let _ = a.next_f64();
        let mut b = JavaRandom::new(7);
        b.next_f32();
        b.next_f32();
        assert_eq!(a.next_f32(), b.next_f32());
    }
}
