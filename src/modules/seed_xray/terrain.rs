use std::cell::Cell;
use std::sync::{Arc, OnceLock};

use azalea::block::BlockState;
use azalea_core::position::ChunkBlockPos;
use azalea_world::Chunk;
use parking_lot::{RwLock, RwLockReadGuard};

use super::table::{Biomes, Kind, Row};
use super::vein::{Class, Terrain};
use crate::client::envprobe::quart_biome_id;
use crate::modules::esp::blocks::id_of;

pub type Handle = Arc<RwLock<Chunk>>;

const SPAN: i32 = 48;
const UNSEEN: i32 = i32::MIN;

fn ore_kind(name: &str) -> Option<Kind> {
    Some(match name.strip_prefix("deepslate_").unwrap_or(name) {
        "coal_ore" => Kind::Coal,
        "iron_ore" => Kind::Iron,
        "gold_ore" | "nether_gold_ore" => Kind::Gold,
        "redstone_ore" => Kind::Redstone,
        "diamond_ore" => Kind::Diamond,
        "lapis_ore" => Kind::Lapis,
        "copper_ore" => Kind::Copper,
        "emerald_ore" => Kind::Emerald,
        "nether_quartz_ore" => Kind::Quartz,
        "ancient_debris" => Kind::Debris,
        _ => return None,
    })
}

fn classify(name: &str) -> Class {
    if let Some(kind) = ore_kind(name) {
        return Class::Ore(kind);
    }
    match name {
        "air" | "cave_air" | "void_air" => Class::Air,
        "stone" | "granite" | "diorite" | "andesite" => Class::Stone,
        "deepslate" | "tuff" => Class::Deepslate,
        "netherrack" => Class::Netherrack,
        "basalt" | "blackstone" => Class::NetherBase,
        "water" | "lava" | "bubble_column" => Class::Fluid,
        "sculk" | "moss_block" | "rooted_dirt" | "clay" | "dripstone_block" => Class::Buried,
        "glow_lichen" | "vine" | "cave_vines" | "cave_vines_plant" | "hanging_roots"
        | "spore_blossom" | "azalea" | "flowering_azalea" | "moss_carpet" | "short_grass"
        | "tall_grass" | "fern" | "large_fern" | "dead_bush" | "sculk_vein"
        | "pointed_dripstone" | "small_dripleaf" | "big_dripleaf" | "big_dripleaf_stem"
        | "snow" => Class::Decor,
        _ if name.starts_with("infested_") => Class::Buried,
        _ if name.ends_with("_leaves") || name.ends_with("_log") || name.ends_with("_wood") => {
            Class::Tree
        }
        _ => Class::Other,
    }
}

pub fn class_of(state: BlockState) -> Class {
    static CLASSES: OnceLock<Vec<Class>> = OnceLock::new();
    let classes = CLASSES.get_or_init(|| {
        let mut table = vec![Class::Other; BlockState::MAX_STATE as usize + 1];
        for id in 0..=BlockState::MAX_STATE {
            if let Ok(state) = BlockState::try_from(id) {
                table[id as usize] = classify(id_of(state));
            }
        }
        table
    });
    classes[state.id() as usize]
}

fn mask_of(rows: &[Row], biome: &str) -> u32 {
    rows.iter().enumerate().fold(0, |mask, (i, row)| {
        let listed = match row.biomes {
            Biomes::All => true,
            Biomes::Only(names) => names.contains(&biome),
            Biomes::Except(names) => !names.contains(&biome),
        };
        mask | (listed as u32) << i
    })
}

pub fn biome_masks(
    rows: &[Row],
    registries: &azalea_core::registry_holder::RegistryHolder,
) -> Vec<u32> {
    crate::util::biome_color::map_biomes(registries, |name| mask_of(rows, name))
}

fn lcg(rval: i64, c: i64) -> i64 {
    rval.wrapping_mul(
        rval.wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407),
    )
    .wrapping_add(c)
}

fn fiddle(rval: i64) -> f64 {
    ((rval >> 24).rem_euclid(1024) as f64 / 1024.0 - 0.5) * 0.9
}

fn fiddled_distance(seed: i64, corner: [i32; 3], d: [f64; 3]) -> f64 {
    let mut rval = seed;
    for _ in 0..2 {
        for c in corner {
            rval = lcg(rval, c as i64);
        }
    }
    let fx = fiddle(rval);
    rval = lcg(rval, seed);
    let fy = fiddle(rval);
    rval = lcg(rval, seed);
    let fz = fiddle(rval);
    (d[2] + fz).powi(2) + (d[1] + fy).powi(2) + (d[0] + fx).powi(2)
}

fn zoomed_quart(zoom_seed: i64, x: i32, y: i32, z: i32) -> [i32; 3] {
    let abs = [x - 2, y - 2, z - 2];
    let parent = abs.map(|v| v >> 2);
    let fract = abs.map(|v| (v & 3) as f64 / 4.0);
    let mut best = 0;
    let mut best_distance = f64::INFINITY;
    for i in 0..8 {
        let even = [i & 4 == 0, i & 2 == 0, i & 1 == 0];
        let corner = [0, 1, 2].map(|a| if even[a] { parent[a] } else { parent[a] + 1 });
        let distance = [0, 1, 2].map(|a| if even[a] { fract[a] } else { fract[a] - 1.0 });
        let next = fiddled_distance(zoom_seed, corner, distance);
        if best_distance > next {
            best = i;
            best_distance = next;
        }
    }
    [
        if best & 4 == 0 {
            parent[0]
        } else {
            parent[0] + 1
        },
        if best & 2 == 0 {
            parent[1]
        } else {
            parent[1] + 1
        },
        if best & 1 == 0 {
            parent[2]
        } else {
            parent[2] + 1
        },
    ]
}

pub struct Region<'a> {
    cx: i32,
    cz: i32,
    guards: [RwLockReadGuard<'a, Chunk>; 9],
    min_y: i32,
    height: i32,
    zoom_seed: i64,
    masks: &'a [u32],
    floors: Vec<Cell<i32>>,
}

impl<'a> Region<'a> {
    pub fn new(
        (cx, cz): (i32, i32),
        handles: &'a [Handle; 9],
        (min_y, height): (i32, i32),
        zoom_seed: i64,
        masks: &'a [u32],
    ) -> Self {
        Region {
            cx,
            cz,
            guards: handles.each_ref().map(|h| h.read()),
            min_y,
            height,
            zoom_seed,
            masks,
            floors: vec![Cell::new(UNSEEN); (SPAN * SPAN) as usize],
        }
    }

    fn chunk_at(&self, x: i32, z: i32) -> Option<&Chunk> {
        let (dx, dz) = ((x >> 4) - self.cx + 1, (z >> 4) - self.cz + 1);
        ((0..3).contains(&dx) && (0..3).contains(&dz))
            .then(|| &*self.guards[(dz * 3 + dx) as usize])
    }

    fn state_in(&self, chunk: &Chunk, x: i32, y: i32, z: i32) -> Option<BlockState> {
        let pos = ChunkBlockPos {
            x: (x & 15) as u8,
            y,
            z: (z & 15) as u8,
        };
        chunk.get_block_state(&pos, self.min_y)
    }

    #[cfg_attr(not(feature = "seed_cracker"), allow(dead_code))]
    pub fn state(&self, x: i32, y: i32, z: i32) -> Option<BlockState> {
        if self.outside(y) {
            return None;
        }
        self.state_in(self.chunk_at(x, z)?, x, y, z)
    }

    pub fn quart_biome(&self, [qx, qy, qz]: [i32; 3]) -> Option<u32> {
        let chunk = self.chunk_at(qx << 2, qz << 2)?;
        let qy = qy.clamp(self.min_y >> 2, ((self.min_y + self.height) >> 2) - 1);
        quart_biome_id(chunk, self.min_y, [qx, qy, qz])
    }

    fn scan_floor(&self, chunk: &Chunk, x: i32, z: i32) -> i32 {
        for (i, section) in chunk.sections.iter().enumerate().rev() {
            if section.block_count == 0 {
                continue;
            }
            let base = self.min_y + i as i32 * 16;
            for y in (base..base + 16).rev() {
                if !matches!(
                    self.class(x, y, z),
                    Class::Air | Class::Decor | Class::Fluid | Class::Tree
                ) {
                    return y + 1;
                }
            }
        }
        self.min_y
    }
}

impl Terrain for Region<'_> {
    fn class(&self, x: i32, y: i32, z: i32) -> Class {
        if self.outside(y) {
            return Class::Air;
        }
        let Some(chunk) = self.chunk_at(x, z) else {
            return Class::Other;
        };
        self.state_in(chunk, x, y, z).map_or(Class::Air, class_of)
    }

    fn floor(&self, x: i32, z: i32) -> i32 {
        let Some(chunk) = self.chunk_at(x, z) else {
            return self.min_y + self.height;
        };
        let (ix, iz) = (x - (self.cx - 1) * 16, z - (self.cz - 1) * 16);
        let cell = &self.floors[(iz * SPAN + ix) as usize];
        if cell.get() == UNSEEN {
            cell.set(self.scan_floor(chunk, x, z));
        }
        cell.get()
    }

    fn has_feature(&self, row: usize, x: i32, y: i32, z: i32) -> bool {
        self.quart_biome(zoomed_quart(self.zoom_seed, x, y, z))
            .and_then(|id| self.masks.get(id as usize))
            .is_some_and(|mask| mask >> row & 1 != 0)
    }

    fn outside(&self, y: i32) -> bool {
        y < self.min_y || y >= self.min_y + self.height
    }
}
