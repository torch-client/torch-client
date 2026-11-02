use azalea::block::BlockState;
use azalea_core::position::ChunkSectionBiomePos;
use azalea_registry::DataRegistry;
use azalea_world::Chunk;

use super::blocks::{BlockTable, FULLY_OPAQUE};
use super::point;
use crate::lighting::level::{LightMap, SectionSample};

const SECTION_CELLS: usize = 4096;

#[derive(Default)]
pub struct Scratch {
    states: Vec<BlockState>,
    biomes: Vec<u8>,
}

pub enum LightFrom<'a> {
    Off,
    Unlit,
    Engine(&'a parking_lot::RwLock<LightMap>),
}

struct Light {
    samples: Option<Vec<SectionSample>>,
    lowest: i32,
    fallback: (u8, u8),
}

impl Light {
    #[inline]
    fn at(&self, x: usize, y: i32, z: usize) -> (u8, u8) {
        let Some(samples) = &self.samples else {
            return self.fallback;
        };
        match samples.get(((y >> 4) - self.lowest) as usize) {
            Some(s) => s.at(x, (y & 15) as usize, z),
            None => (0, 15),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn chunk_columns(
    chunk: &Chunk,
    cx: i32,
    cz: i32,
    min_y: i32,
    height: i32,
    biomes: &[u8],
    light: LightFrom<'_>,
    blocks: &mut BlockTable,
    scratch: &mut Scratch,
) -> Vec<Vec<u64>> {
    let mut out = vec![Vec::new(); 256];
    let sec_count = (height / 16) as usize;
    let lowest_sec =
        (0..sec_count).find(|&i| chunk.sections.get(i).is_some_and(|s| s.block_count > 0));
    let highest_sec = (0..sec_count)
        .rev()
        .find(|&i| chunk.sections.get(i).is_some_and(|s| s.block_count > 0));
    let (Some(lowest_sec), Some(highest_sec)) = (lowest_sec, highest_sec) else {
        return out;
    };

    scratch.states.clear();
    scratch
        .states
        .resize(sec_count * SECTION_CELLS, BlockState::AIR);
    scratch.biomes.clear();
    scratch.biomes.resize(sec_count * 64, u8::MAX);
    for (i, section) in chunk.sections.iter().enumerate().take(sec_count) {
        if section.block_count > 0 {
            section
                .states
                .unpack_into(&mut scratch.states[i * SECTION_CELLS..(i + 1) * SECTION_CELLS]);
        }
        if i < lowest_sec {
            continue;
        }
        for cell in 0..64usize {
            let biome = section.get_biome(ChunkSectionBiomePos {
                x: (cell & 3) as u8,
                y: (cell >> 4) as u8,
                z: ((cell >> 2) & 3) as u8,
            });
            scratch.biomes[i * 64 + cell] = biomes
                .get(biome.protocol_id() as usize)
                .copied()
                .unwrap_or(u8::MAX);
        }
    }
    let states = &scratch.states;
    let biome_cells = &scratch.biomes;

    let exclusive_max = min_y + height;
    let min_non_empty = min_y + lowest_sec as i32 * 16;
    let base_sec = min_y >> 4;
    let (samples, fallback) = match light {
        LightFrom::Off => (None, (15, 15)),
        LightFrom::Unlit => (None, (0, 15)),
        LightFrom::Engine(map) => {
            let map = map.read();
            let samples: Vec<SectionSample> = (base_sec + lowest_sec as i32
                ..=base_sec + sec_count as i32)
                .map(|sy| {
                    map.resolve(crate::lighting::SectionPos {
                        x: cx,
                        y: sy,
                        z: cz,
                    })
                })
                .collect();
            (Some(samples), (0, 15))
        }
    };
    let light = Light {
        samples,
        lowest: base_sec + lowest_sec as i32,
        fallback,
    };

    let state_at = |x: usize, y: i32, z: usize| -> BlockState {
        if y < min_y || y >= exclusive_max {
            return BlockState::AIR;
        }
        let rel = (y - min_y) as usize;
        states[(rel >> 4) * SECTION_CELLS + ((rel & 15) << 8 | z << 4 | x)]
    };
    let biome_at = |x: usize, y: i32, z: usize| -> u8 {
        let rel = (y.clamp(min_y, exclusive_max - 1) - min_y) as usize;
        biome_cells[(rel >> 4) * 64 + ((rel & 15) >> 2) * 16 + (z >> 2) * 4 + (x >> 2)]
    };
    let top_of_highest = min_y + (highest_sec as i32 + 1) * 16 - 1;

    for x in 0..16usize {
        for z in 0..16usize {
            let Some(top) = (min_non_empty..=top_of_highest)
                .rev()
                .find(|&y| !state_at(x, y, z).is_air())
            else {
                continue;
            };
            let column = &mut out[x * 16 + z];

            let mut last_y = exclusive_max;
            let mut current_state = BlockState::AIR;
            let mut current_biome = biome_at(x, last_y, z);
            let mut mapped = point::pack_id(current_state.id() as u16, current_biome);
            let mut block_light = 0u8;
            let mut sky_light = 15u8;
            let mut force_single = false;

            let mut y = (top + 1).min(exclusive_max);
            while y >= min_non_empty {
                let new_state = state_at(x, y, z);
                let new_biome = biome_at(x, y, z);
                let (new_block_light, new_sky_light) = light.at(x, y + 1, z);
                if new_biome != current_biome || new_state != current_state || force_single {
                    let info = blocks.info(current_state.id() as u16);
                    force_single = !info.is_air
                        && !info.is_solid
                        && !info.is_liquid
                        && info.opacity != FULLY_OPAQUE;
                    column.push(point::encode(
                        mapped,
                        last_y - y,
                        y + 1 - min_y,
                        block_light,
                        sky_light,
                    ));
                    current_biome = new_biome;
                    current_state = new_state;
                    mapped = point::pack_id(current_state.id() as u16, current_biome);
                    block_light = new_block_light;
                    sky_light = new_sky_light;
                    last_y = y;
                }
                y -= 1;
            }
            column.push(point::encode(
                mapped,
                last_y - y,
                y + 1 - min_y,
                block_light,
                sky_light,
            ));
        }
    }
    out
}
