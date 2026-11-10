use std::sync::{Mutex, OnceLock};

use azalea::Client;
use azalea_core::position::{ChunkPos as AzChunkPos, ChunkSectionBiomePos};

use crate::renderer::environment::{
    BREADTH, Bases, BiomeLayer, Blend, RADIUS, SAMPLE_COUNT, axis_weights, sample_index,
};

const UNRESOLVED: u8 = u8::MAX;

struct Probe {
    key: Option<(i32, i32, i32, u32)>,
    cells: [u8; SAMPLE_COUNT],
    blend: Blend,
}

impl Probe {
    fn new() -> Probe {
        Probe {
            key: None,
            cells: [0; SAMPLE_COUNT],
            blend: Blend::default(),
        }
    }
}

fn probe() -> &'static Mutex<Probe> {
    static PROBE: OnceLock<Mutex<Probe>> = OnceLock::new();
    PROBE.get_or_init(|| Mutex::new(Probe::new()))
}

static CAMERA_BIOME: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(UNRESOLVED);

#[cfg(feature = "audio")]
static CAMERA_BIOME_ID: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(UNRESOLVED_ID);

#[cfg(feature = "audio")]
const UNRESOLVED_ID: u32 = u32::MAX;

pub(crate) fn camera_has_precipitation() -> bool {
    let row = CAMERA_BIOME.load(std::sync::atomic::Ordering::Relaxed);
    crate::util::biome_color::env(row).has_precipitation
}

#[cfg(any(feature = "audio", feature = "shader_support"))]
pub(crate) fn camera_biome() -> Option<u8> {
    let row = CAMERA_BIOME.load(std::sync::atomic::Ordering::Relaxed);
    (row != UNRESOLVED).then_some(row)
}

#[cfg(feature = "audio")]
pub(crate) fn camera_biome_id() -> Option<u32> {
    let id = CAMERA_BIOME_ID.load(std::sync::atomic::Ordering::Relaxed);
    (id != UNRESOLVED_ID).then_some(id)
}

pub(crate) fn reset() {
    if let Ok(mut probe) = probe().lock() {
        probe.key = None;
    }
    CAMERA_BIOME.store(UNRESOLVED, std::sync::atomic::Ordering::Relaxed);
    #[cfg(feature = "audio")]
    CAMERA_BIOME_ID.store(UNRESOLVED_ID, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn sample(bot: &Client, eye: [f64; 3]) -> Option<BiomeLayer> {
    if !crate::util::biome_color::any_biome_env() {
        return None;
    }
    let bases = Bases::for_dimension(crate::renderer::dimension::current());

    let quart = [
        eye[0] * 0.25 - 0.5,
        eye[1] * 0.25 - 0.5,
        eye[2] * 0.25 - 0.5,
    ];
    let integral = [
        quart[0].floor() as i32,
        quart[1].floor() as i32,
        quart[2].floor() as i32,
    ];
    let fraction = [
        (quart[0] - integral[0] as f64) as f32,
        (quart[1] - integral[1] as f64) as f32,
        (quart[2] - integral[2] as f64) as f32,
    ];

    let mut probe = probe().lock().ok()?;
    let key = (
        integral[0],
        integral[1],
        integral[2],
        crate::client::worldsync::chunk_generation(),
    );
    if probe.key != Some(key) {
        resample(bot, &mut probe, integral)?;
        probe.key = Some(key);
    }

    let wx = axis_weights(fraction[0]);
    let wy = axis_weights(fraction[1]);
    let wz = axis_weights(fraction[2]);
    probe.blend.clear_weights();
    for z in 0..BREADTH {
        for x in 0..BREADTH {
            let wxz = wx[x] * wz[z];
            for y in 0..BREADTH {
                let bucket = probe.cells[sample_index(x, y, z)] as usize;
                probe.blend.add_weight(bucket, wxz * wy[y]);
            }
        }
    }
    Some(probe.blend.resolve(&bases))
}

pub(crate) fn quart_biome(
    chunk: &azalea_world::Chunk,
    min_y: i32,
    biomes: &[u8],
    q: [i32; 3],
) -> u8 {
    quart_biome_id(chunk, min_y, q)
        .and_then(|id| biomes.get(id as usize).copied())
        .unwrap_or(UNRESOLVED)
}

pub(crate) fn quart_biome_id(chunk: &azalea_world::Chunk, min_y: i32, q: [i32; 3]) -> Option<u32> {
    use azalea_registry::DataRegistry;

    let section = chunk.sections.get(((q[1] * 4 - min_y) >> 4) as usize)?;
    let biome = section.get_biome(ChunkSectionBiomePos {
        x: (q[0] & 3) as u8,
        y: (q[1] & 3) as u8,
        z: (q[2] & 3) as u8,
    });
    Some(biome.protocol_id())
}

fn resample(bot: &Client, probe: &mut Probe, integral: [i32; 3]) -> Option<()> {
    let world = bot.world().ok()?;
    let world = world.read();
    let biomes = crate::client::worker::biome_table(&world.registries);
    if biomes.is_empty() {
        return None;
    }
    let min_y = world.chunks.min_y();
    let height = world.chunks.height() as i32;
    let quart_lo = min_y >> 2;
    let quart_hi = ((min_y + height) >> 2) - 1;

    probe.blend = Blend::default();
    let mut held_pos: Option<AzChunkPos> = None;
    let mut held = None;

    for z in 0..BREADTH {
        let qz = integral[2] - RADIUS + z as i32;
        for x in 0..BREADTH {
            let qx = integral[0] - RADIUS + x as i32;
            let pos = AzChunkPos {
                x: qx >> 2,
                z: qz >> 2,
            };
            if held_pos != Some(pos) {
                held = world.chunks.get(&pos);
                held_pos = Some(pos);
            }
            let chunk = held.as_ref().map(|chunk| chunk.read());

            for y in 0..BREADTH {
                let qy = (integral[1] - RADIUS + y as i32).clamp(quart_lo, quart_hi);
                let row = match chunk.as_ref() {
                    Some(chunk) => quart_biome(chunk, min_y, &biomes, [qx, qy, qz]),
                    None => UNRESOLVED,
                };
                if x as i32 == RADIUS && y as i32 == RADIUS && z as i32 == RADIUS {
                    CAMERA_BIOME.store(row, std::sync::atomic::Ordering::Relaxed);
                    #[cfg(feature = "audio")]
                    CAMERA_BIOME_ID.store(
                        chunk
                            .as_ref()
                            .and_then(|chunk| quart_biome_id(chunk, min_y, [qx, qy, qz]))
                            .unwrap_or(UNRESOLVED_ID),
                        std::sync::atomic::Ordering::Relaxed,
                    );
                }
                probe.cells[sample_index(x, y, z)] = probe.blend.bucket_for(row) as u8;
            }
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sweep_covers_every_cell_once() {
        let mut seen = [false; SAMPLE_COUNT];
        for z in 0..BREADTH {
            for x in 0..BREADTH {
                for y in 0..BREADTH {
                    let at = sample_index(x, y, z);
                    assert!(!seen[at], "cell {at} written twice");
                    seen[at] = true;
                }
            }
        }
        assert!(seen.iter().all(|hit| *hit));
    }
}
