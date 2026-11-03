#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColorMode {
    Default,
    Flower,
    Leaves,
    Glass,
}

#[derive(Default)]
pub struct TileColors {
    tiles: Vec<[u32; 3]>,
}

const FLOWER_COLOR_SCALE: u32 = 5;

impl TileColors {
    pub fn from_atlas(data: &[u8], width: u32, height: u32) -> Self {
        let cols = crate::renderer::ATLAS_COLS;
        let tile_px = (width / cols).max(1);
        let rows = height / tile_px;
        let lut = srgb_to_linear_table();
        let mut tiles = Vec::with_capacity((rows * cols) as usize);
        for tile in 0..rows * cols {
            let (tx, ty) = ((tile % cols) * tile_px, (tile / cols) * tile_px);
            let mut acc = [Acc::default(), Acc::default(), Acc::default()];
            for y in ty..ty + tile_px {
                for x in tx..tx + tile_px {
                    let o = ((y * width + x) * 4) as usize;
                    let (r, g, b, a) = (data[o], data[o + 1], data[o + 2], data[o + 3]);
                    acc[2].add(&lut, r, g, b, a, 1);
                    if a == 0 {
                        continue;
                    }
                    acc[0].add(&lut, r, g, b, a, 1);
                    let flower = if (g as u32) + 25 < b as u32 || (g as u32) + 25 < r as u32 {
                        FLOWER_COLOR_SCALE
                    } else {
                        1
                    };
                    acc[1].add(&lut, r, g, b, a, flower);
                }
            }
            tiles.push([acc[0].finish(), acc[1].finish(), acc[2].finish()]);
        }
        Self { tiles }
    }

    pub(crate) fn get(&self, tile: u32, mode: ColorMode) -> u32 {
        let Some(t) = self.tiles.get(tile as usize) else {
            return 0;
        };
        match mode {
            ColorMode::Default => t[0],
            ColorMode::Flower => t[1],
            ColorMode::Glass => t[2],
            ColorMode::Leaves => t[0] | 0xFF00_0000,
        }
    }
}

#[derive(Default)]
struct Acc {
    count: u32,
    alpha: u64,
    rgb: [f64; 3],
}

impl Acc {
    fn add(&mut self, lut: &[f32; 256], r: u8, g: u8, b: u8, a: u8, scale: u32) {
        self.count += scale;
        self.alpha += a as u64 * scale as u64;
        let w = a as f64 * scale as f64;
        self.rgb[0] += lut[r as usize] as f64 * w;
        self.rgb[1] += lut[g as usize] as f64 * w;
        self.rgb[2] += lut[b as usize] as f64 * w;
    }

    fn finish(&self) -> u32 {
        if self.count == 0 || self.alpha == 0 {
            return 0x00FF_FFFF;
        }
        let a = (self.alpha / self.count as u64) as u32;
        let ch = |c: f64| {
            (linear_to_srgb((c / self.alpha as f64) as f32) * 255.0)
                .round()
                .clamp(0.0, 255.0) as u32
        };
        a << 24 | ch(self.rgb[0]) << 16 | ch(self.rgb[1]) << 8 | ch(self.rgb[2])
    }
}

fn srgb_to_linear_table() -> [f32; 256] {
    std::array::from_fn(|i| crate::util::mth::srgb_byte_to_linear(i as u8))
}

fn linear_to_srgb(c: f32) -> f32 {
    crate::util::mth::linear_to_srgb(c.clamp(0.0, 1.0))
}

pub(crate) fn tile_of_uv(uv: &[[f32; 2]; 4], rows: u32) -> u32 {
    let cols = crate::renderer::ATLAS_COLS;
    let u = uv.iter().map(|c| c[0]).sum::<f32>() / 4.0;
    let v = uv.iter().map(|c| c[1]).sum::<f32>() / 4.0;
    let col = ((u * cols as f32).floor() as i64).clamp(0, cols as i64 - 1) as u32;
    let row = ((v * rows as f32).floor() as i64).clamp(0, rows as i64 - 1) as u32;
    row * cols + col
}
