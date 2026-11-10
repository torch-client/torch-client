const GOLDEN_RATIO_64: u64 = 0x9E37_79B9_7F4A_7C15;
const SILVER_RATIO_64: u64 = 0x6A09_E667_F3BC_C909;

fn mix_stafford13(z: u64) -> u64 {
    let z = (z ^ z >> 30).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let z = (z ^ z >> 27).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ z >> 31
}

pub struct WorldgenRandom {
    lo: u64,
    hi: u64,
}

impl WorldgenRandom {
    pub fn new() -> Self {
        let mut r = WorldgenRandom { lo: 0, hi: 0 };
        r.set_seed(0);
        r
    }

    pub fn set_seed(&mut self, seed: i64) {
        let low = seed as u64 ^ SILVER_RATIO_64;
        let high = low.wrapping_add(GOLDEN_RATIO_64);
        self.lo = mix_stafford13(low);
        self.hi = mix_stafford13(high);
        if self.lo | self.hi == 0 {
            self.lo = GOLDEN_RATIO_64;
            self.hi = SILVER_RATIO_64;
        }
    }

    fn raw(&mut self) -> u64 {
        let (s0, mut s1) = (self.lo, self.hi);
        let result = s0.wrapping_add(s1).rotate_left(17).wrapping_add(s0);
        s1 ^= s0;
        self.lo = s0.rotate_left(49) ^ s1 ^ s1 << 21;
        self.hi = s1.rotate_left(28);
        result
    }

    fn next(&mut self, bits: u32) -> i32 {
        (self.raw() >> (64 - bits)) as i32
    }

    pub fn next_int(&mut self, bound: i32) -> i32 {
        if bound & (bound - 1) == 0 {
            return ((bound as i64 * self.next(31) as i64) >> 31) as i32;
        }
        loop {
            let sample = self.next(31);
            let modulo = sample % bound;
            if sample.wrapping_sub(modulo).wrapping_add(bound - 1) >= 0 {
                return modulo;
            }
        }
    }

    fn next_long(&mut self) -> i64 {
        let upper = self.next(32);
        let lower = self.next(32);
        ((upper as i64) << 32).wrapping_add(lower as i64)
    }

    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 * 5.960_464_5e-8_f32
    }

    pub fn next_double(&mut self) -> f64 {
        let upper = self.next(26) as i64;
        let lower = self.next(27) as i64;
        ((upper << 27) + lower) as f64 * 1.110_223_024_625_156_5e-16
    }

    pub fn between(&mut self, min: i32, max: i32) -> i32 {
        self.next_int(max - min + 1) + min
    }

    pub fn set_decoration_seed(&mut self, seed: i64, x: i32, z: i32) -> i64 {
        self.set_seed(seed);
        let x_scale = self.next_long() | 1;
        let z_scale = self.next_long() | 1;
        let result = (x as i64)
            .wrapping_mul(x_scale)
            .wrapping_add((z as i64).wrapping_mul(z_scale))
            ^ seed;
        self.set_seed(result);
        result
    }

    pub fn set_feature_seed(&mut self, decoration: i64, index: i32, step: i32) {
        self.set_seed(
            decoration
                .wrapping_add(index as i64)
                .wrapping_add(10_000 * step as i64),
        );
    }
}
