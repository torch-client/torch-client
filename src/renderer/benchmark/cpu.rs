use std::time::Duration;

use azalea::block::BlockState;
use azalea_registry::builtin::BlockKind;

use crate::gui::benchmark::CpuStats;
use crate::platform::time::Instant;
use crate::renderer::{
    BlockGeom, MeshTiles, Occupancy, RenderedBlock, build_section_mesh, visgraph,
};

const SECTIONS: usize = 64;
const ROW: usize = 8;
const TIMED_PASSES: usize = 5;
const SLICE: Duration = Duration::from_millis(8);

const PALETTE: [BlockKind; 5] = [
    BlockKind::Stone,
    BlockKind::Dirt,
    BlockKind::GrassBlock,
    BlockKind::OakLeaves,
    BlockKind::Glass,
];
const STONE: u8 = 0;
const DIRT: u8 = 1;
const GRASS: u8 = 2;
const LEAVES: u8 = 3;
const GLASS: u8 = 4;

struct Section {
    x_base: i32,
    z_base: i32,
    blocks: Vec<(i32, i32, i32, u8)>,
    occ: Occupancy,
}

pub(super) struct CpuRun {
    visuals: Vec<RenderedBlock>,
    tiles: MeshTiles,
    sections: Vec<Section>,
    pass: usize,
    next: usize,
    pass_time: Duration,
    pass_quads: u64,
    times: Vec<Duration>,
    quads: u64,
}

impl CpuRun {
    pub fn new() -> Self {
        let blocks = crate::blocks::registry();
        let tiles = blocks.sprites().mesh_tiles();
        let visuals: Vec<RenderedBlock> = PALETTE
            .iter()
            .map(|&kind| crate::util::block_model::block_visual_in(BlockState::from(kind), &blocks))
            .collect();
        let sections = (0..SECTIONS).map(|i| prepare(i, &visuals)).collect();
        Self {
            visuals,
            tiles,
            sections,
            pass: 0,
            next: 0,
            pass_time: Duration::ZERO,
            pass_quads: 0,
            times: Vec::with_capacity(TIMED_PASSES),
            quads: 0,
        }
    }

    pub fn progress(&self) -> f32 {
        let done = self.pass * SECTIONS + self.next;
        done as f32 / ((TIMED_PASSES + 1) * SECTIONS) as f32
    }

    pub fn step(&mut self) -> Option<CpuStats> {
        let start = Instant::now();
        while self.next < SECTIONS {
            let section = &self.sections[self.next];
            let visuals = &self.visuals;
            let (opaque, water) = build_section_mesh(
                section
                    .blocks
                    .iter()
                    .map(|&(x, y, z, b)| (x, y, z, &visuals[usize::from(b)])),
                section.x_base,
                0,
                section.z_base,
                self.tiles,
                &section.occ,
            );
            let occ = &section.occ;
            let (x_base, z_base) = (section.x_base, section.z_base);
            std::hint::black_box(visgraph::resolve(|lx, ly, lz| {
                occ.is_solid(
                    x_base + i32::from(lx),
                    i32::from(ly),
                    z_base + i32::from(lz),
                )
            }));
            self.pass_quads += u64::from(opaque.solid_quads + opaque.cutout_quads)
                + u64::from(water.solid_quads + water.cutout_quads);
            self.next += 1;
            if start.elapsed() >= SLICE {
                break;
            }
        }
        self.pass_time += start.elapsed();
        if self.next < SECTIONS {
            return None;
        }

        if self.pass > 0 {
            self.times.push(self.pass_time);
            self.quads = self.pass_quads;
        }
        self.pass += 1;
        self.next = 0;
        self.pass_time = Duration::ZERO;
        self.pass_quads = 0;
        if self.pass <= TIMED_PASSES {
            return None;
        }
        self.times.sort_unstable();
        let per_section = |d: Duration| d.as_secs_f32() * 1000.0 / SECTIONS as f32;
        Some(CpuStats {
            ms_per_section: per_section(self.times[self.times.len() / 2]),
            best_ms: per_section(self.times[0]),
            worst_ms: per_section(self.times[self.times.len() - 1]),
            sections: SECTIONS as u32,
            passes: TIMED_PASSES as u32,
            quads: self.quads,
        })
    }
}

fn height(x: i32, z: i32) -> i32 {
    6 + (super::hash(x >> 3, 0, z >> 3, 1) % 7) as i32
        + (super::hash(x >> 1, 0, z >> 1, 2) % 3) as i32
}

fn block_at(x: i32, y: i32, z: i32) -> Option<u8> {
    let h = height(x, z);
    if y < h {
        if y == h - 1 {
            return Some(GRASS);
        }
        if super::hash(x, y, z, 3) % 4 == 0 {
            return None;
        }
        return Some(if y >= h - 4 { DIRT } else { STONE });
    }
    if y < h + 3 && super::hash(x >> 1, y, z >> 1, 4) % 9 == 0 {
        return Some(LEAVES);
    }
    if y == h && super::hash(x, y, z, 5) % 31 == 0 {
        return Some(GLASS);
    }
    None
}

fn prepare(i: usize, visuals: &[RenderedBlock]) -> Section {
    let x_base = (i % ROW) as i32 * 16;
    let z_base = (i / ROW) as i32 * 16;
    let mut occ = Occupancy::new(x_base, z_base, -1, 18);
    for y in -1..17 {
        for x in x_base - 1..x_base + 17 {
            for z in z_base - 1..z_base + 17 {
                let Some(b) = block_at(x, y, z) else { continue };
                let rb = &visuals[usize::from(b)];
                occ.set_cull_group(x, y, z, rb.cull_group);
                if rb.full_cube {
                    occ.set_full_cube(x, y, z);
                }
                if rb.emission != 0 {
                    occ.set_emission(x, y, z, rb.emission);
                }
                if rb.emissive {
                    occ.set_emissive(x, y, z);
                }
                if rb.is_solid {
                    occ.set_solid(x, y, z);
                } else if let BlockGeom::Fluid { amount, lava, .. } = rb.geom {
                    occ.set_fluid(x, y, z, amount, lava);
                } else if rb.waterlogged {
                    occ.set_fluid(x, y, z, 8, false);
                }
            }
        }
    }
    occ.fill_light_constant(0, 15);

    let mut blocks = Vec::new();
    for y in 0..16 {
        for x in x_base..x_base + 16 {
            for z in z_base..z_base + 16 {
                let Some(b) = block_at(x, y, z) else { continue };
                if visuals[usize::from(b)].is_solid && occ.enclosed(x, y, z) {
                    continue;
                }
                blocks.push((x, y, z, b));
            }
        }
    }
    Section {
        x_base,
        z_base,
        blocks,
        occ,
    }
}
