pub fn position_seed(x: i32, y: i32, z: i32) -> i64 {
    let mut seed =
        (x.wrapping_mul(3129871) as i64) ^ (z as i64).wrapping_mul(116129781) ^ (y as i64);
    seed = seed
        .wrapping_mul(seed)
        .wrapping_mul(42317861)
        .wrapping_add(seed.wrapping_mul(11));
    seed >> 16
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShapeOffset {
    None,
    Xz {
        max_horizontal: f32,
    },
    Xyz {
        max_horizontal: f32,
        max_vertical: f32,
    },
}

const MAX_HORIZONTAL_OFFSET: f32 = 0.25;
const MAX_VERTICAL_OFFSET: f32 = 0.2;
const DRIPSTONE_MAX_HORIZONTAL_OFFSET: f32 = 2.0 / 16.0;
const SMALL_DRIPLEAF_MAX_VERTICAL_OFFSET: f32 = 0.1;

impl ShapeOffset {
    pub fn for_block(name: &str) -> Self {
        match name {
            "short_grass" | "fern" | "short_dry_grass" | "tall_dry_grass" => Self::Xyz {
                max_horizontal: MAX_HORIZONTAL_OFFSET,
                max_vertical: MAX_VERTICAL_OFFSET,
            },
            "small_dripleaf" => Self::Xyz {
                max_horizontal: MAX_HORIZONTAL_OFFSET,
                max_vertical: SMALL_DRIPLEAF_MAX_VERTICAL_OFFSET,
            },
            "pointed_dripstone" => Self::Xz {
                max_horizontal: DRIPSTONE_MAX_HORIZONTAL_OFFSET,
            },
            "allium" | "azure_bluet" | "bamboo" | "bamboo_sapling" | "blue_orchid"
            | "closed_eyeblossom" | "cornflower" | "crimson_roots" | "dandelion"
            | "golden_dandelion" | "hanging_roots" | "large_fern" | "lilac"
            | "lily_of_the_valley" | "mangrove_propagule" | "nether_sprouts"
            | "open_eyeblossom" | "orange_tulip" | "oxeye_daisy" | "peony" | "pink_tulip"
            | "pitcher_plant" | "poppy" | "red_tulip" | "rose_bush" | "sunflower"
            | "tall_grass" | "tall_seagrass" | "torchflower" | "warped_roots" | "white_tulip"
            | "wither_rose" => Self::Xz {
                max_horizontal: MAX_HORIZONTAL_OFFSET,
            },
            _ => Self::None,
        }
    }

    pub fn at(self, x: i32, z: i32) -> [f32; 3] {
        let (max_h, max_v) = match self {
            Self::None => return [0.0; 3],
            Self::Xz { max_horizontal } => (max_horizontal, 0.0),
            Self::Xyz {
                max_horizontal,
                max_vertical,
            } => (max_horizontal, max_vertical),
        };
        let seed = position_seed(x, 0, z);
        let max_h = max_h as f64;
        let ox = (((seed & 15) as f32 / 15.0) as f64 - 0.5) * 0.5;
        let oz = ((((seed >> 8) & 15) as f32 / 15.0) as f64 - 0.5) * 0.5;
        let oy = if max_v == 0.0 {
            0.0
        } else {
            ((((seed >> 4) & 15) as f32 / 15.0) as f64 - 1.0) * max_v as f64
        };
        [
            ox.clamp(-max_h, max_h) as f32,
            oy as f32,
            oz.clamp(-max_h, max_h) as f32,
        ]
    }
}
