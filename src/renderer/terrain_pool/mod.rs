pub mod cull;
pub mod draw;
pub mod pools;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize};

use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::extract_resource::ExtractResource;
use bevy::shader::Shader;
use bytemuck::{Pod, Zeroable};
use parking_lot::Mutex;

use super::mesh::MeshBuf;

pub const STREAM_SOLID: u32 = 0;
pub const STREAM_CUTOUT: u32 = 1;
pub const STREAM_WATER: u32 = 2;
pub const STREAMS: u32 = 3;

pub const SLOTS_INITIAL: u32 = 8192;

pub const ORIGIN_ROW: u32 = 256;

pub const MAX_VIEWS: u32 = 5;

pub const META_LIVE: u32 = 1;
pub const META_WATER: u32 = 2;

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
    pub _pad: u32,
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
    pub indirect: AtomicUsize,
}

#[derive(Resource, Clone)]
pub struct TerrainShaders {
    pub terrain: Handle<Shader>,
    pub prepass: Handle<Shader>,
    pub cull: Handle<Shader>,
}

fn embedded_shader(assets: &AssetServer, file: &str) -> Handle<Shader> {
    let crate_name = module_path!().split(':').next().unwrap_or("torch_client");
    assets.load(format!("embedded://{crate_name}/renderer/{file}"))
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
        pools::finish(app);
        cull::finish(app);
        draw::finish(app);
    }
}

fn decide_tier(world: &World) -> TerrainTier {
    use bevy::render::renderer::RenderDevice;
    let device = world.resource::<RenderDevice>();
    let features = device.features();
    let forced_direct = matches!(
        std::env::var("MC_TERRAIN_DIRECT").as_deref(),
        Ok("1") | Ok("true")
    );
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
    let count = indirect
        && features
            .contains(bevy::render::render_resource::WgpuFeatures::MULTI_DRAW_INDIRECT_COUNT);
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
