const W: i32 = 18;
const LAYER: i32 = W * W;

pub struct Occupancy {
    x_base: i32,
    z_base: i32,
    y_base: i32,
    y_count: i32,
    solid: Vec<u64>,
    full: Vec<u64>,
    light: Vec<u8>,
    emission: Vec<u8>,
    emissive: Vec<u64>,
    fluid: Vec<u8>,
    cull_group: Vec<u32>,
    biome: Vec<u8>,
}

impl Occupancy {
    pub fn new(x_base: i32, z_base: i32, y_base: i32, y_count: i32) -> Self {
        let mut occ = Self {
            x_base: 0,
            z_base: 0,
            y_base: 0,
            y_count: 0,
            solid: Vec::new(),
            full: Vec::new(),
            light: Vec::new(),
            emission: Vec::new(),
            emissive: Vec::new(),
            fluid: Vec::new(),
            cull_group: Vec::new(),
            biome: Vec::new(),
        };
        occ.reset(x_base, z_base, y_base, y_count);
        occ
    }

    pub fn reset(&mut self, x_base: i32, z_base: i32, y_base: i32, y_count: i32) {
        self.x_base = x_base;
        self.z_base = z_base;
        self.y_base = y_base;
        self.y_count = y_count;
        let cells = (LAYER * y_count) as usize;
        self.solid.clear();
        self.solid.resize(cells.div_ceil(64), 0);
        self.full.clear();
        self.full.resize(cells.div_ceil(64), 0);
        self.fluid.clear();
        self.fluid.resize(cells, 0);
        self.emission.clear();
        self.emission.resize(cells, 0);
        self.emissive.clear();
        self.emissive.resize(cells.div_ceil(64), 0);
        self.light.clear();
        self.light.resize(cells, 0xF0);
        self.cull_group.clear();
        self.cull_group.resize(cells, 0);
        self.biome.clear();
        self.biome.resize(cells, u8::MAX);
    }

    #[inline]
    pub fn covers_y(&self, y: i32) -> bool {
        y >= self.y_base && y < self.y_base + self.y_count
    }

    #[inline]
    fn index(&self, x: i32, y: i32, z: i32) -> Option<usize> {
        let px = x - self.x_base + 1;
        let pz = z - self.z_base + 1;
        let py = y - self.y_base;
        if px < 0 || px >= W || pz < 0 || pz >= W || py < 0 || py >= self.y_count {
            return None;
        }
        Some((py * LAYER + px * W + pz) as usize)
    }

    #[inline]
    pub fn set_solid(&mut self, x: i32, y: i32, z: i32) {
        if let Some(i) = self.index(x, y, z) {
            bit_set(&mut self.solid, i);
        }
    }

    #[inline]
    pub fn set_full_cube(&mut self, x: i32, y: i32, z: i32) {
        if let Some(i) = self.index(x, y, z) {
            bit_set(&mut self.full, i);
        }
    }

    #[inline]
    pub fn set_emission(&mut self, x: i32, y: i32, z: i32, emission: u8) {
        if let Some(i) = self.index(x, y, z) {
            self.emission[i] = emission;
        }
    }

    #[inline]
    pub fn set_emissive(&mut self, x: i32, y: i32, z: i32) {
        if let Some(i) = self.index(x, y, z) {
            bit_set(&mut self.emissive, i);
        }
    }

    #[inline]
    pub fn is_emissive(&self, x: i32, y: i32, z: i32) -> bool {
        match self.index(x, y, z) {
            Some(i) => bit_get(&self.emissive, i),
            None => false,
        }
    }

    #[inline]
    pub fn set_cull_group(&mut self, x: i32, y: i32, z: i32, group: u32) {
        if let Some(i) = self.index(x, y, z) {
            self.cull_group[i] = group;
        }
    }

    #[inline]
    pub fn set_biome(&mut self, x: i32, y: i32, z: i32, biome: u8) {
        if let Some(i) = self.index(x, y, z) {
            self.biome[i] = biome;
        }
    }

    #[inline]
    pub fn biome(&self, x: i32, y: i32, z: i32) -> u8 {
        match self.index(x, y, z) {
            Some(i) => self.biome[i],
            None => u8::MAX,
        }
    }

    #[inline]
    pub fn set_light(&mut self, x: i32, y: i32, z: i32, block: u8, sky: u8) {
        if let Some(i) = self.index(x, y, z) {
            self.light[i] = (block & 15) | (sky & 15) << 4;
        }
    }

    #[inline]
    pub fn fill_light_constant(&mut self, block: u8, sky: u8) {
        self.light.fill((block & 15) | (sky & 15) << 4);
    }

    #[inline]
    pub fn y_window(&self) -> (i32, i32) {
        (self.y_base, self.y_count)
    }

    #[inline]
    pub fn origin(&self) -> (i32, i32) {
        (self.x_base, self.z_base)
    }

    #[inline]
    pub fn is_full_cube(&self, x: i32, y: i32, z: i32) -> bool {
        match self.index(x, y, z) {
            Some(i) => bit_get(&self.full, i),
            None => false,
        }
    }

    #[inline]
    pub fn block_light(&self, x: i32, y: i32, z: i32) -> u8 {
        match self.index(x, y, z) {
            Some(i) => self.light[i] & 15,
            None => 0,
        }
    }

    #[inline]
    pub fn sky_light(&self, x: i32, y: i32, z: i32) -> u8 {
        match self.index(x, y, z) {
            Some(i) => self.light[i] >> 4,
            None => 15,
        }
    }

    #[inline]
    pub fn emission(&self, x: i32, y: i32, z: i32) -> u8 {
        match self.index(x, y, z) {
            Some(i) => self.emission[i],
            None => 0,
        }
    }

    #[inline]
    pub fn set_fluid(&mut self, x: i32, y: i32, z: i32, amount: u8) {
        if let Some(i) = self.index(x, y, z) {
            self.fluid[i] = amount;
        }
    }

    #[inline]
    pub fn is_solid(&self, x: i32, y: i32, z: i32) -> bool {
        match self.index(x, y, z) {
            Some(i) => bit_get(&self.solid, i),
            None => false,
        }
    }

    #[inline]
    pub fn same_cull_group(&self, x: i32, y: i32, z: i32, group: u32) -> bool {
        if group == 0 {
            return false;
        }
        match self.index(x, y, z) {
            Some(i) => self.cull_group[i] == group,
            None => false,
        }
    }

    #[inline]
    pub fn fluid_amount(&self, x: i32, y: i32, z: i32) -> u8 {
        match self.index(x, y, z) {
            Some(i) => self.fluid[i],
            None => 0,
        }
    }

    #[inline]
    pub fn is_fluid(&self, x: i32, y: i32, z: i32) -> bool {
        self.fluid_amount(x, y, z) != 0
    }

    #[inline]
    pub fn fluid_top(&self, x: i32, y: i32, z: i32) -> f32 {
        let amount = self.fluid_amount(x, y, z);
        if amount == 0 {
            0.0
        } else if self.fluid_amount(x, y + 1, z) != 0 {
            1.0
        } else {
            super::fluid_surface(amount)
        }
    }

    #[inline]
    pub fn fluid_own_height(&self, x: i32, y: i32, z: i32) -> f32 {
        let amount = self.fluid_amount(x, y, z);
        if amount == 0 {
            0.0
        } else {
            super::fluid_surface(amount)
        }
    }

    #[inline]
    pub fn enclosed(&self, x: i32, y: i32, z: i32) -> bool {
        self.is_solid(x, y + 1, z)
            && self.is_solid(x, y - 1, z)
            && self.is_solid(x + 1, y, z)
            && self.is_solid(x - 1, y, z)
            && self.is_solid(x, y, z + 1)
            && self.is_solid(x, y, z - 1)
    }
}

#[inline]
fn bit_get(bits: &[u64], i: usize) -> bool {
    bits[i >> 6] & (1u64 << (i & 63)) != 0
}

#[inline]
fn bit_set(bits: &mut [u64], i: usize) {
    bits[i >> 6] |= 1u64 << (i & 63);
}
