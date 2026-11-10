use super::parse_seed;
use super::table::{Biomes, NETHER, OVERWORLD, Row};
use super::vein::{Class, Placed, Terrain, simulate};

const OW_COUNT: usize = 15632;
const OW_DIGEST: u64 = 11544784191425724124;
const NE_COUNT: usize = 9605;
const NE_DIGEST: u64 = 17808801666077471365;

struct Synthetic {
    nether: bool,
    rows: &'static [Row],
}

impl Synthetic {
    fn ground(&self, x: i32, z: i32) -> i32 {
        if self.nether {
            100
        } else {
            40 + ((x >> 4)
                .wrapping_mul(7)
                .wrapping_add((z >> 4).wrapping_mul(13))
                & 127)
        }
    }

    fn biome(&self, x: i32, y: i32, z: i32) -> &'static str {
        const OVERWORLD_BIOMES: [&str; 5] =
            ["plains", "meadow", "badlands", "dripstone_caves", "desert"];
        const NETHER_BIOMES: [&str; 2] = ["nether_wastes", "basalt_deltas"];
        let at = (x >> 4) * 31 + (z >> 4) * 17 + (y >> 6);
        if self.nether {
            NETHER_BIOMES[at.rem_euclid(2) as usize]
        } else {
            OVERWORLD_BIOMES[at.rem_euclid(5) as usize]
        }
    }
}

impl Terrain for Synthetic {
    fn class(&self, x: i32, y: i32, z: i32) -> Class {
        if self.outside(y) {
            return Class::Air;
        }
        let mut h = x.wrapping_mul(73856093) ^ y.wrapping_mul(19349663) ^ z.wrapping_mul(83492791);
        h ^= ((h as u32) >> 13) as i32;
        let v = h & 31;
        if y >= self.ground(x, z) || v == 0 {
            Class::Air
        } else if self.nether {
            if v == 2 {
                Class::NetherBase
            } else {
                Class::Netherrack
            }
        } else if v == 1 {
            Class::Other
        } else if y < 0 {
            Class::Deepslate
        } else {
            Class::Stone
        }
    }

    fn floor(&self, x: i32, z: i32) -> i32 {
        self.ground(x, z)
    }

    fn has_feature(&self, row: usize, x: i32, y: i32, z: i32) -> bool {
        let biome = self.biome(x, y, z);
        match self.rows[row].biomes {
            Biomes::All => true,
            Biomes::Only(names) => names.contains(&biome),
            Biomes::Except(names) => !names.contains(&biome),
        }
    }

    fn outside(&self, y: i32) -> bool {
        if self.nether {
            !(0..256).contains(&y)
        } else {
            !(-64..320).contains(&y)
        }
    }
}

fn digest(seed: i64, world: &Synthetic) -> (usize, u64) {
    let mut count = 0;
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    for cx in -2..=2 {
        for cz in -2..=2 {
            for Placed { x, y, z, kind } in simulate(seed, world.rows, cx, cz, world) {
                count += 1;
                for v in [x as u32, y as u32, z as u32, kind as u32] {
                    h = (h ^ v as u64).wrapping_mul(0x100_0000_01b3);
                }
            }
        }
    }
    (count, h)
}

#[test]
fn overworld_matches_the_java_reference() {
    let world = Synthetic {
        nether: false,
        rows: &OVERWORLD,
    };
    assert_eq!(digest(1234567890123, &world), (OW_COUNT, OW_DIGEST));
}

#[test]
fn nether_matches_the_java_reference() {
    let world = Synthetic {
        nether: true,
        rows: &NETHER,
    };
    assert_eq!(digest(-8913472390128734, &world), (NE_COUNT, NE_DIGEST));
}

#[test]
fn rows_run_in_index_order() {
    for rows in [&OVERWORLD[..], &NETHER[..]] {
        assert!(
            rows.windows(2).all(|w| w[0].index < w[1].index),
            "{}",
            rows[0].name
        );
    }
}

#[test]
fn seed_text_parses_like_world_options() {
    assert_eq!(parse_seed(""), None);
    assert_eq!(parse_seed("   "), None);
    assert_eq!(parse_seed(" -42 "), Some(-42));
    assert_eq!(parse_seed("Hello"), Some(69609650));
    assert!(parse_seed("9223372036854775808").is_some());
}

#[cfg(feature = "click_gui")]
#[test]
fn seed_hash_is_biome_manager_obfuscate_seed() {
    assert_eq!(crate::util::hash::obfuscate_seed(0), 8794265229978523055);
    assert_eq!(
        crate::util::hash::obfuscate_seed(1234567890123),
        5318346877068299412
    );
    assert_eq!(
        crate::util::hash::obfuscate_seed(-8913472390128734),
        1587100174054922628
    );
}
