pub mod cull;
pub mod draw;
pub mod pools;

use std::sync::atomic::{AtomicU64, AtomicUsize};
use std::sync::{Arc, OnceLock};

use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::extract_resource::ExtractResource;
use bevy::shader::Shader;
use bytemuck::{Pod, Zeroable};
use parking_lot::Mutex;

use super::mesh::{FACE_GROUPS, MeshBuf};

pub const STREAM_SOLID: u32 = 0;
pub const STREAM_CUTOUT: u32 = 1;
pub const STREAM_WATER: u32 = 2;
pub const STREAMS: u32 = 3;

#[cfg(feature = "shader_support")]
pub fn stream(geometry: crate::shaderpack::programs::Geometry) -> u32 {
    use crate::shaderpack::programs::Geometry;
    match geometry {
        Geometry::TerrainSolid => STREAM_SOLID,
        Geometry::TerrainCutout => STREAM_CUTOUT,
        Geometry::Water => STREAM_WATER,
    }
}

pub const SLOTS_INITIAL: u32 = 8192;

pub const ORIGIN_ROW: u32 = 256;

pub const MAX_VIEWS: u32 = 5;

pub const META_LIVE: u32 = 1;
pub const META_WATER: u32 = 2;

pub const FACE_RUNS: u32 = 4;

pub const FACE_FILL_QUADS: u32 = 32;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct SlotMeta {
    pub min: [f32; 3],
    pub first_index: u32,
    pub max: [f32; 3],
    pub index_count: u32,
    pub origin: [i32; 3],
    pub base_vertex: u32,
    pub solid_count: u32,
    pub flags: u32,
    pub pool: u32,
    pub cutout_first: u32,
    pub faces: [u32; 8],
}

impl SlotMeta {
    pub fn stream_range(&self, stream: u32) -> (u32, u32) {
        match stream {
            STREAM_SOLID => (self.first_index, self.solid_count),
            STREAM_CUTOUT => (self.cutout_first, self.index_count - self.solid_count),
            _ => (self.first_index, self.index_count),
        }
    }

    pub fn pack_faces(faces: &[[u16; FACE_GROUPS]; 2]) -> [u32; 8] {
        let mut out = [0u32; 8];
        for (k, &count) in faces.as_flattened().iter().enumerate() {
            out[k / 2] |= u32::from(count) << ((k % 2) * 16);
        }
        out
    }

    pub fn face_count(&self, stream: u32, group: usize) -> u32 {
        let k = stream as usize * FACE_GROUPS + group;
        (self.faces[k / 2] >> ((k % 2) * 16)) & 0xFFFF
    }

    pub fn render_box(&self, render_origin: IVec3) -> ([f32; 3], [f32; 3]) {
        (
            self.render_box_of(self.min, render_origin),
            self.render_box_of(self.max, render_origin),
        )
    }

    pub fn render_box_of(&self, local: [f32; 3], render_origin: IVec3) -> [f32; 3] {
        [0, 1, 2].map(|a| (self.origin[a] - render_origin[a]) as f32 + local[a])
    }
}

pub enum TerrainOp {
    Upload {
        slot: u32,
        origin: [i32; 3],
        water: bool,
        mesh: MeshBuf,
    },
    Free {
        slot: u32,
    },
    ClearAll,
    Visibility(Arc<[u32]>),
}

#[derive(Resource, Default)]
pub struct TerrainOps(pub Mutex<Vec<TerrainOp>>);

impl TerrainOps {
    pub fn push(&self, op: TerrainOp) {
        self.0.lock().push(op);
    }
}

#[derive(Resource, Default)]
pub struct TerrainOpQueue(pub Vec<TerrainOp>);

#[derive(Resource, Clone, Copy, Debug, ExtractResource)]
pub struct TerrainParams {
    pub cutoff: f32,
    pub mip_bias: f32,
    pub shadow_mip: f32,
}

impl Default for TerrainParams {
    fn default() -> Self {
        Self {
            cutoff: 0.1,
            mip_bias: 0.0,
            shadow_mip: super::terrain::SHADOW_ALPHA_MIP,
        }
    }
}

#[derive(Resource, Clone, ExtractResource)]
pub struct TerrainTextures {
    pub atlas: Handle<Image>,
    pub lightmap: Handle<Image>,
}

#[derive(Component, Clone, Copy, Default, ExtractComponent)]
pub struct TerrainView;

#[derive(Resource, Clone, Default)]
pub struct TerrainStats(pub Arc<TerrainStatsInner>);

#[derive(Default)]
pub struct TerrainStatsInner {
    pub live_slots: AtomicUsize,
    pub used_bytes: AtomicU64,
    pub capacity_bytes: AtomicU64,
    pub pools: AtomicUsize,
    pub drawn: AtomicUsize,
    pub draws: AtomicUsize,
    draws_pending: AtomicUsize,
    pub indirect: AtomicUsize,
}

static STATS: OnceLock<TerrainStats> = OnceLock::new();

pub fn stats() -> Option<&'static TerrainStats> {
    STATS.get()
}

#[derive(Resource, Clone)]
pub struct TerrainShaders {
    pub terrain: Handle<Shader>,
    pub prepass: Handle<Shader>,
    pub cull: Handle<Shader>,
}

fn embedded_shader(assets: &AssetServer, file: &str) -> Handle<Shader> {
    assets.load(super::embedded_path(&format!("renderer/{file}")))
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct TerrainTier {
    pub indirect: bool,
    pub count: bool,
    pub base_vertex: bool,
}

pub struct TerrainPoolPlugin;

impl Plugin for TerrainPoolPlugin {
    fn build(&self, app: &mut App) {
        let stats = TerrainStats::default();
        let _ = STATS.set(stats.clone());
        app.init_resource::<TerrainOps>()
            .init_resource::<TerrainParams>()
            .insert_resource(stats.clone())
            .add_plugins((
                bevy::render::extract_resource::ExtractResourcePlugin::<TerrainParams>::default(),
                bevy::render::extract_resource::ExtractResourcePlugin::<TerrainTextures>::default(),
                bevy::render::extract_component::ExtractComponentPlugin::<TerrainView>::default(),
            ));
        pools::build(app, stats);
        cull::build(app);
        draw::build(app);
    }

    fn finish(&self, app: &mut App) {
        let shaders = {
            let assets = app.world().resource::<AssetServer>();
            TerrainShaders {
                terrain: embedded_shader(assets, "terrain.wgsl"),
                prepass: embedded_shader(assets, "terrain_prepass.wgsl"),
                cull: embedded_shader(assets, "terrain_pool/cull.wgsl"),
            }
        };
        let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) else {
            return;
        };
        render_app.insert_resource(shaders);
        let tier = decide_tier(render_app.world());
        crate::log_info!(
            "render",
            "terrain tier: indirect={} count={} base_vertex={}",
            tier.indirect,
            tier.count,
            tier.base_vertex
        );
        render_app.insert_resource(tier);
        app.insert_resource(tier);
        pools::finish(app);
        cull::finish(app);
        draw::finish(app);
    }
}

fn decide_tier(world: &World) -> TerrainTier {
    use bevy::render::renderer::RenderDevice;
    let device = world.resource::<RenderDevice>();
    let features = device.features();
    let forced_direct = crate::platform::env::flag("MC_TERRAIN_DIRECT");
    let storage_ok = device.limits().max_storage_buffers_per_shader_stage >= 4;
    let indirect_ok = world
        .resource::<bevy::render::renderer::RenderAdapter>()
        .get_downlevel_capabilities()
        .flags
        .contains(bevy::render::render_resource::DownlevelFlags::INDIRECT_EXECUTION);
    let indirect = !forced_direct
        && storage_ok
        && indirect_ok
        && features.contains(bevy::render::render_resource::WgpuFeatures::INDIRECT_FIRST_INSTANCE);
    let count_ok =
        features.contains(bevy::render::render_resource::WgpuFeatures::MULTI_DRAW_INDIRECT_COUNT);
    let indirect =
        indirect && (count_ok || !(cfg!(target_os = "android") || cfg!(target_arch = "wasm32")));
    let count = indirect && count_ok;
    let base_vertex = world
        .resource::<bevy::render::renderer::RenderAdapter>()
        .get_downlevel_capabilities()
        .flags
        .contains(bevy::render::render_resource::DownlevelFlags::BASE_VERTEX);
    TerrainTier {
        indirect,
        count,
        base_vertex,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::mesh::FACE_STORAGE_ORDER;

    fn face_mask(lo: [f32; 3], hi: [f32; 3], eye: [f32; 3]) -> u32 {
        let mut mask = 1 << crate::renderer::mesh::FACE_UNALIGNED;
        for a in 0..3 {
            if eye[a] < hi[a] + 1.0 {
                mask |= 1 << (a * 2);
            }
            if eye[a] > lo[a] - 1.0 {
                mask |= 1 << (a * 2 + 1);
            }
        }
        mask
    }

    fn face_runs(meta: &SlotMeta, stream: u32, mask: u32) -> Vec<(u32, u32)> {
        let (first, count) = meta.stream_range(stream);
        let mut runs = Vec::new();
        if count == 0 {
            return runs;
        }
        let fill = FACE_FILL_QUADS * 6;
        let total: u32 = (0..FACE_GROUPS)
            .map(|g| meta.face_count(stream, g) * 6)
            .sum();
        if total != count || total < fill {
            runs.push((first, count));
            return runs;
        }
        let mut at = first;
        let mut run: Option<(u32, u32)> = None;
        let mut gap = 0;
        for g in FACE_STORAGE_ORDER {
            let n = meta.face_count(stream, g) * 6;
            if n == 0 {
                continue;
            }
            if mask & (1 << g) != 0 {
                let (start, len) = run.unwrap_or((at, 0));
                run = Some((start, len + gap + n));
                gap = 0;
            } else if run.is_some() {
                if gap + n <= fill {
                    gap += n;
                } else {
                    runs.extend(run.take());
                    gap = 0;
                }
            }
            at += n;
        }
        runs.extend(run);
        runs
    }

    fn covered(meta: &SlotMeta, stream: u32, runs: &[(u32, u32)]) -> u32 {
        let (first, _) = meta.stream_range(stream);
        let mut at = first;
        let mut out = 0;
        for g in FACE_STORAGE_ORDER {
            let n = meta.face_count(stream, g) * 6;
            if n != 0 && runs.iter().any(|&(s, l)| s <= at && at + n <= s + l) {
                out |= 1 << g;
            }
            at += n;
        }
        out
    }

    fn meta(solid: [u16; FACE_GROUPS], cutout: [u16; FACE_GROUPS]) -> SlotMeta {
        let quads = |c: [u16; FACE_GROUPS]| c.iter().map(|&n| u32::from(n) * 6).sum::<u32>();
        SlotMeta {
            min: [0.0; 3],
            max: [16.0; 3],
            first_index: 1000,
            solid_count: quads(solid),
            index_count: quads(solid) + quads(cutout),
            cutout_first: 5000,
            flags: META_LIVE,
            faces: SlotMeta::pack_faces(&[solid, cutout]),
            ..Default::default()
        }
    }

    #[test]
    fn the_slot_meta_is_96_bytes() {
        assert_eq!(std::mem::size_of::<SlotMeta>(), 96);
    }

    #[test]
    fn face_counts_round_trip_through_the_packing() {
        let solid = [1, 2, 3, 4, 5, 6, 65535];
        let cutout = [7, 8, 9, 10, 11, 12, 13];
        let m = meta(solid, cutout);
        for g in 0..FACE_GROUPS {
            assert_eq!(m.face_count(STREAM_SOLID, g), u32::from(solid[g]));
            assert_eq!(m.face_count(STREAM_CUTOUT, g), u32::from(cutout[g]));
        }
        assert_eq!(m.faces[7], 0);
    }

    fn even(n: u16) -> SlotMeta {
        meta([n; FACE_GROUPS], [0; FACE_GROUPS])
    }

    #[test]
    fn an_eye_inside_draws_the_stream_whole() {
        let m = meta([1, 1, 1, 1, 1, 1, 1], [0, 2, 0, 0, 0, 0, 3]);
        let mask = face_mask([0.0; 3], [16.0; 3], [8.0; 3]);
        assert_eq!(mask, 0x7F);
        assert_eq!(face_runs(&m, STREAM_SOLID, mask), [(1000, 42)]);
        assert_eq!(face_runs(&m, STREAM_CUTOUT, mask), [(5000, 30)]);
    }

    #[test]
    fn an_eye_past_a_corner_is_one_run() {
        let m = even(40);
        let mask = face_mask([0.0; 3], [16.0; 3], [-10.0; 3]);
        assert_eq!(mask, 0b1010101);
        assert_eq!(face_runs(&m, STREAM_SOLID, mask), [(1000, 4 * 240)]);
        let mask = face_mask([0.0; 3], [16.0; 3], [30.0; 3]);
        assert_eq!(
            face_runs(&m, STREAM_SOLID, mask),
            [(1000 + 3 * 240, 4 * 240)]
        );
    }

    #[test]
    fn a_mixed_eye_makes_three_runs() {
        let m = even(40);
        let mask = face_mask([0.0; 3], [16.0; 3], [-10.0, -10.0, 30.0]);
        assert_eq!(mask, 0b110_0101);
        assert_eq!(
            face_runs(&m, STREAM_SOLID, mask),
            [
                (1000, 240),
                (1000 + 2 * 240, 2 * 240),
                (1000 + 5 * 240, 240)
            ],
        );
    }

    #[test]
    fn an_empty_group_does_not_break_a_run() {
        let m = meta([40, 0, 0, 40, 0, 0, 0], [0; FACE_GROUPS]);
        let mask = face_mask([0.0; 3], [16.0; 3], [-10.0, 30.0, 8.0]);
        assert_eq!(face_runs(&m, STREAM_SOLID, mask), [(1000, 480)]);
    }

    #[test]
    fn a_small_hidden_group_is_filled_in() {
        let m = meta([40, 0, 40, 0, 10, 0, 40], [0; FACE_GROUPS]);
        let mask = 0b100_0101;
        assert_eq!(face_runs(&m, STREAM_SOLID, mask), [(1000, 130 * 6)]);
        let m = meta(
            [40, 0, 40, 0, FACE_FILL_QUADS as u16 + 1, 0, 40],
            [0; FACE_GROUPS],
        );
        assert_eq!(
            face_runs(&m, STREAM_SOLID, mask),
            [(1000, 240), (1000 + (40 + 33) * 6, 80 * 6)],
        );
        let m = meta([40, 5, 0, 0, 0, 0, 40], [0; FACE_GROUPS]);
        assert_eq!(face_runs(&m, STREAM_SOLID, 0b100_0001), [(1000, 480)]);
    }

    #[test]
    fn a_small_stream_is_one_command() {
        let m = meta([5, 5, 5, 5, 5, 5, 1], [0; FACE_GROUPS]);
        assert_eq!(face_runs(&m, STREAM_SOLID, 0b100_0000), [(1000, 31 * 6)]);
    }

    #[test]
    fn every_axis_mask_covers_its_groups_within_the_run_limit() {
        let shapes: [[u16; FACE_GROUPS]; 5] = [
            [100; FACE_GROUPS],
            [100, 3, 100, 3, 100, 3, 100],
            [3, 100, 3, 100, 3, 100, 100],
            [100, 100, 0, 100, 0, 100, 0],
            [0, 0, 0, 0, 0, 0, 100],
        ];
        for counts in shapes {
            let m = meta(counts, [0; FACE_GROUPS]);
            for axes in 0..64u32 {
                let mask = axes | 1 << crate::renderer::mesh::FACE_UNALIGNED;
                let runs = face_runs(&m, STREAM_SOLID, mask);
                assert!(
                    runs.len() <= FACE_RUNS as usize,
                    "{counts:?} {mask:#09b}: {runs:?}"
                );
                let visible = (0..FACE_GROUPS)
                    .filter(|&g| mask & (1 << g) != 0 && counts[g] != 0)
                    .fold(0u32, |bits, g| bits | 1 << g);
                assert_eq!(
                    covered(&m, STREAM_SOLID, &runs) & visible,
                    visible,
                    "{counts:?} {mask:#09b}"
                );
                let (first, count) = m.stream_range(STREAM_SOLID);
                let mut end = first;
                for &(s, l) in &runs {
                    assert!(s >= end && l > 0 && s + l <= first + count);
                    end = s + l;
                }
            }
        }
    }

    #[test]
    fn an_ungrouped_stream_is_drawn_whole() {
        let mut m = meta([0; FACE_GROUPS], [0; FACE_GROUPS]);
        m.solid_count = 60;
        m.index_count = 60;
        let mask = face_mask([0.0; 3], [16.0; 3], [-10.0; 3]);
        assert_eq!(face_runs(&m, STREAM_SOLID, mask), [(1000, 60)]);
    }

    #[test]
    fn the_margin_keeps_faces_a_block_past_the_box() {
        let mask = face_mask([0.0; 3], [16.0; 3], [-0.5, 8.0, 8.0]);
        assert_ne!(mask & 0b10, 0, "+X within a block of the -X side");
        let mask = face_mask([0.0; 3], [16.0; 3], [-1.5, 8.0, 8.0]);
        assert_eq!(mask & 0b10, 0, "+X more than a block past the -X side");
    }
}
