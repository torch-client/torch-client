use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::atomic::Ordering;

use bevy::app::App;
use bevy::asset::embedded_asset;
use bevy::core_pipeline::{Core3d, Core3dSystems};
use bevy::ecs::query::Has;
use bevy::math::primitives::ViewFrustum;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::render::diagnostic::RecordDiagnostics;
use bevy::render::render_resource::binding_types::{
    storage_buffer_read_only_sized, storage_buffer_sized, uniform_buffer_sized,
};
use bevy::render::render_resource::{
    BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, Buffer,
    BufferBinding, BufferDescriptor, BufferUsages, CachedComputePipelineId, ComputePassDescriptor,
    ComputePipelineDescriptor, PipelineCache, ShaderStages,
};
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy::render::view::ExtractedView;
use bevy::render::{Render, RenderApp, RenderSystems};
use bytemuck::{Pod, Zeroable};

use super::pools::{SlotTable, TerrainPools};
use super::{FACE_RUNS, MAX_VIEWS, SLOTS_INITIAL, STREAMS, SlotMeta, TerrainTier, TerrainView};

#[cfg(feature = "builtin_shaders")]
use bevy::pbr::{LightEntity, ViewLightEntities};

pub const MAX_CULL_PLANES: usize = 13;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct CullView {
    pub planes: [[f32; 4]; MAX_CULL_PLANES],
    pub within_min: [f32; 4],
    pub within_max: [f32; 4],
    pub always_min: [f32; 4],
    pub always_max: [f32; 4],
    pub fog: [f32; 4],
    pub eye: [f32; 4],
    pub slot_cap: u32,
    pub view_index: u32,
    pub pool_count: u32,
    pub compact: u32,
    pub flags: u32,
    pub plane_count: u32,
    pub pad: [u32; 2],
    pub origin: [i32; 4],
}

pub const VIEW_WATER: u32 = 1;
pub const VIEW_UNOCCLUDED: u32 = 2;
pub const VIEW_WITHIN: u32 = 4;
pub const VIEW_ALWAYS: u32 = 8;
pub const VIEW_FOG: u32 = 16;
pub const VIEW_FACES: u32 = 32;

const FOG_CULL_MARGIN: f32 = 1.0;

const WATER_RESORT_DISTANCE_SQ: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CullShape {
    pub planes: [[f32; 4]; MAX_CULL_PLANES],
    pub plane_count: usize,
    pub within: Option<[[f32; 3]; 2]>,
    pub always: Option<[[f32; 3]; 2]>,
    pub fog: Option<[f32; 4]>,
}

impl CullShape {
    fn frustum(planes: [[f32; 4]; 6]) -> CullShape {
        let mut all = [[0.0; 4]; MAX_CULL_PLANES];
        all[..6].copy_from_slice(&planes);
        CullShape {
            planes: all,
            plane_count: 6,
            within: None,
            always: None,
            fog: None,
        }
    }

    pub fn keeps(&self, lo: [f32; 3], hi: [f32; 3]) -> bool {
        self.keeps_measured(lo, hi, lo, hi)
    }

    #[cfg(feature = "render_lod")]
    pub fn keeps_section(&self, lo: [f32; 3], hi: [f32; 3], section: [f32; 3]) -> bool {
        self.keeps_measured(lo, hi, section, section.map(|v| v + 16.0))
    }

    fn keeps_measured(
        &self,
        lo: [f32; 3],
        hi: [f32; 3],
        fog_lo: [f32; 3],
        fog_hi: [f32; 3],
    ) -> bool {
        if let Some([min, max]) = self.within
            && (0..3).any(|a| hi[a] < min[a] || lo[a] > max[a])
        {
            return false;
        }
        if let Some([x, y, z, far]) = self.fog {
            let eye = [x, y, z];
            let d = [0, 1, 2].map(|a| eye[a].max(fog_lo[a]).min(fog_hi[a]) - eye[a]);
            if (d[0] * d[0] + d[2] * d[2]).sqrt().max(d[1].abs()) > far {
                return false;
            }
        }
        if let Some([min, max]) = self.always
            && (0..3).all(|a| hi[a] >= min[a] && lo[a] <= max[a])
        {
            return true;
        }
        self.planes[..self.plane_count].iter().all(|p| {
            let v = [0, 1, 2].map(|a| if p[a] > 0.0 { hi[a] } else { lo[a] });
            !(p[0] * v[0] + p[1] * v[1] + p[2] * v[2] + p[3] < 0.0)
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn view(
        &self,
        eye: Vec3,
        origin: IVec3,
        slot_cap: u32,
        view_index: u32,
        pool_count: u32,
        compact: u32,
        flags: u32,
    ) -> CullView {
        let corner = |b: Option<[[f32; 3]; 2]>, i: usize| {
            b.map_or([0.0; 4], |b| [b[i][0], b[i][1], b[i][2], 0.0])
        };
        CullView {
            planes: self.planes,
            within_min: corner(self.within, 0),
            within_max: corner(self.within, 1),
            always_min: corner(self.always, 0),
            always_max: corner(self.always, 1),
            fog: self.fog.unwrap_or([0.0; 4]),
            eye: eye.extend(0.0).to_array(),
            slot_cap,
            view_index,
            pool_count,
            compact,
            flags: flags
                | if self.within.is_some() {
                    VIEW_WITHIN
                } else {
                    0
                }
                | if self.always.is_some() {
                    VIEW_ALWAYS
                } else {
                    0
                }
                | if self.fog.is_some() { VIEW_FOG } else { 0 },
            plane_count: self.plane_count as u32,
            pad: [0; 2],
            origin: origin.extend(0).to_array(),
        }
    }

    fn same(&self, other: &CullShape) -> bool {
        let bits = |s: &CullShape| {
            (
                s.planes.map(|p| p.map(f32::to_bits)),
                s.plane_count,
                s.within.map(|b| b.map(|c| c.map(f32::to_bits))),
                s.always.map(|b| b.map(|c| c.map(f32::to_bits))),
                s.fog.map(|f| f.map(f32::to_bits)),
            )
        };
        bits(self) == bits(other)
    }
}

fn fog_cull(fog: Option<&DistanceFog>, eye: Vec3) -> Option<[f32; 4]> {
    #[cfg(feature = "render_lod")]
    if fog.is_some() {
        return Some([eye.x, eye.y, eye.z, crate::client::lod::real_radius()]);
    }
    let fog = fog?;
    let FogFalloff::Linear { end, .. } = fog.falloff else {
        return None;
    };
    let edge = fog.directional_light_color.to_linear().blue;
    let end = if edge > 0.0 { edge.min(end) } else { end };
    (fog.color.alpha() >= 1.0 && end > 0.0).then_some([eye.x, eye.y, eye.z, end + FOG_CULL_MARGIN])
}

const CAMERA_FLAGS: u32 = VIEW_WATER | VIEW_FACES;
#[cfg(feature = "shader_support")]
const PACK_SHADOW_FLAGS: u32 = VIEW_WATER | VIEW_UNOCCLUDED;

const CULL_VIEW_STRIDE: u64 = 512;
const _: () = assert!(std::mem::size_of::<CullView>() as u64 <= CULL_VIEW_STRIDE);

const INDIRECT_ARGS_SIZE: u64 = 20;

#[derive(Resource)]
pub struct CullBuffers {
    pub commands: Buffer,
    pub counts: Buffer,
    pub views: Buffer,
    pub regions: u32,
    pub slot_cap: u32,
    pub pool_count: u32,
    pub view_count: u32,
    pub generation: u64,
    view_scratch: [u8; CULL_VIEW_STRIDE as usize],
    water_counts: Vec<u32>,
    water_cache: [Option<WaterCache>; MAX_VIEWS as usize],
    water_args: Vec<[u32; 5]>,
}

struct WaterCache {
    key: bevy::render::view::RetainedViewEntity,
    flags: u32,
    eye: Vec3,
    water_generation: u64,
    vis_generation: u64,
    order: Vec<(u32, f32, u32)>,
    shape: Option<CullShape>,
}

impl CullBuffers {
    fn new(device: &RenderDevice, view_count: u32, pool_count: u32, slot_cap: u32) -> Self {
        let pool_count = pool_count.max(1);
        let view_count = view_count.clamp(1, MAX_VIEWS);
        let regions = view_count * pool_count * STREAMS;
        let commands = device.create_buffer(&BufferDescriptor {
            label: Some("terrain_cull_commands"),
            size: (regions as u64) * (slot_cap as u64) * (FACE_RUNS as u64) * INDIRECT_ARGS_SIZE,
            usage: BufferUsages::INDIRECT | BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let counts = device.create_buffer(&BufferDescriptor {
            label: Some("terrain_cull_counts"),
            size: (regions as u64) * 4,
            usage: BufferUsages::INDIRECT | BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let views = device.create_buffer(&BufferDescriptor {
            label: Some("terrain_cull_views"),
            size: (MAX_VIEWS as u64) * CULL_VIEW_STRIDE,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            commands,
            counts,
            views,
            regions,
            slot_cap,
            pool_count,
            view_count,
            generation: 0,
            view_scratch: [0u8; CULL_VIEW_STRIDE as usize],
            water_counts: vec![0; (view_count * pool_count) as usize],
            water_cache: Default::default(),
            water_args: Vec::new(),
        }
    }

    fn placeholder(device: &RenderDevice) -> Self {
        let plain = |label, size| {
            device.create_buffer(&BufferDescriptor {
                label: Some(label),
                size,
                usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM,
                mapped_at_creation: false,
            })
        };
        Self {
            commands: plain("terrain_cull_commands_unused", 16),
            counts: plain("terrain_cull_counts_unused", 16),
            views: plain("terrain_cull_views_unused", 16),
            regions: 0,
            slot_cap: 0,
            pool_count: 0,
            view_count: 0,
            generation: 0,
            view_scratch: [0u8; CULL_VIEW_STRIDE as usize],
            water_counts: Vec::new(),
            water_cache: Default::default(),
            water_args: Vec::new(),
        }
    }

    pub fn region_commands(&self) -> u64 {
        self.slot_cap as u64 * FACE_RUNS as u64
    }

    fn fits(&self, view_count: u32, pool_count: u32, slot_cap: u32) -> bool {
        self.view_count >= view_count.max(1)
            && self.pool_count >= pool_count.max(1)
            && self.slot_cap >= slot_cap
    }

    fn write_water(
        &mut self,
        queue: &RenderQueue,
        pools: &TerrainPools,
        view: &AssignedView,
        origin: IVec3,
    ) {
        let occluded = view.flags & VIEW_UNOCCLUDED == 0;
        let region_commands = self.region_commands();
        let cache = &mut self.water_cache[view.slot as usize];
        let sorted = cache.as_ref().is_some_and(|c| {
            c.key == view.key
                && c.flags == view.flags
                && c.water_generation == pools.water_generation
                && c.vis_generation == pools.vis_generation
                && c.eye.distance_squared(view.eye) < WATER_RESORT_DISTANCE_SQ
        });
        let cache = cache.get_or_insert_with(|| WaterCache {
            key: view.key,
            flags: view.flags,
            eye: view.eye,
            water_generation: 0,
            vis_generation: 0,
            order: Vec::new(),
            shape: None,
        });
        if !sorted {
            cache.key = view.key;
            cache.flags = view.flags;
            cache.eye = view.eye;
            cache.water_generation = pools.water_generation;
            cache.vis_generation = pools.vis_generation;
            cache.shape = None;
            cache.order.clear();
            for &slot in &pools.water_slots {
                if occluded && !pools.is_visible(slot) {
                    continue;
                }
                let meta = &pools.meta_cpu[slot as usize];
                let (lo, hi) = meta.render_box(origin);
                let centre = (Vec3::from(lo) + Vec3::from(hi)) * 0.5;
                cache
                    .order
                    .push((meta.pool, centre.distance_squared(view.eye), slot));
            }
            cache
                .order
                .sort_unstable_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)));
        }
        if cache.shape.is_some_and(|shape| shape.same(&view.shape)) {
            return;
        }
        cache.shape = Some(view.shape);

        let row = (view.slot * self.pool_count) as usize;
        self.water_counts[row..row + self.pool_count as usize].fill(0);
        for run in cache.order.chunk_by(|a, b| a.0 == b.0) {
            let pool = run[0].0;
            self.water_args.clear();
            for &(_, _, slot) in run {
                let meta = &pools.meta_cpu[slot as usize];
                let (lo, hi) = meta.render_box(origin);
                #[cfg(feature = "render_lod")]
                let keep = view
                    .shape
                    .keeps_section(lo, hi, meta.render_box_of([0.0; 3], origin));
                #[cfg(not(feature = "render_lod"))]
                let keep = view.shape.keeps(lo, hi);
                if !keep {
                    continue;
                }
                self.water_args.push([
                    meta.index_count,
                    1,
                    meta.first_index,
                    meta.base_vertex,
                    slot,
                ]);
            }
            if self.water_args.is_empty() {
                continue;
            }
            let region = (view.slot * self.pool_count + pool) * STREAMS + super::STREAM_WATER;
            queue.write_buffer(
                &self.commands,
                region as u64 * region_commands * INDIRECT_ARGS_SIZE,
                bytemuck::cast_slice(&self.water_args),
            );
            self.water_counts[row + pool as usize] = self.water_args.len() as u32;
        }
    }

    fn forget_water(&mut self, slot: u32) {
        self.water_cache[slot as usize] = None;
        let row = (slot * self.pool_count) as usize;
        self.water_counts[row..row + self.pool_count as usize].fill(0);
    }

    pub fn water_count(&self, view_index: u32, pool: u32) -> u32 {
        self.water_counts
            .get((view_index * self.pool_count + pool) as usize)
            .copied()
            .unwrap_or(0)
    }
}

#[derive(Resource, Default)]
pub struct TerrainViews {
    pub index: HashMap<bevy::render::view::RetainedViewEntity, u32>,
    pub count: u32,
}

#[derive(Resource, Default)]
pub struct DirectLists(
    pub HashMap<bevy::render::view::RetainedViewEntity, [Vec<u32>; STREAMS as usize]>,
);

#[derive(Clone, Copy)]
pub struct AssignedView {
    slot: u32,
    key: bevy::render::view::RetainedViewEntity,
    shape: CullShape,
    flags: u32,
    eye: Vec3,
}

#[cfg(feature = "shader_support")]
#[derive(Resource, Default)]
pub struct PackCullView(pub Option<CullShape>);

#[cfg(feature = "shader_support")]
pub fn pack_shadow_view(
    camera: bevy::render::view::RetainedViewEntity,
) -> bevy::render::view::RetainedViewEntity {
    bevy::render::view::RetainedViewEntity::new(
        camera.main_entity,
        Some(camera.main_entity),
        u32::MAX,
    )
}

#[cfg(feature = "shader_support")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShadowCullMode {
    Distance,
    Advanced,
    SafeZone,
}

#[cfg(feature = "shader_support")]
pub struct ShadowCullInput {
    pub mode: ShadowCullMode,
    pub camera_clip: Mat4,
    pub toward_light: Vec3,
    pub camera: bevy::math::DVec3,
    pub shadow_distance: f32,
    pub voxel_distance: f32,
    pub render_mul: f32,
    pub render_distance: f32,
}

#[cfg(feature = "shader_support")]
pub fn shadow_cull_shape(input: &ShadowCullInput) -> CullShape {
    let around = |distance: f64| -> [[f32; 3]; 2] {
        let c = input.camera;
        [
            [
                (c.x - distance) as f32,
                (c.y - distance) as f32,
                (c.z - distance) as f32,
            ],
            [
                (c.x + distance) as f32,
                (c.y + distance) as f32,
                (c.z + distance) as f32,
            ],
        ]
    };
    let mut shape = CullShape {
        planes: [[0.0; 4]; MAX_CULL_PLANES],
        plane_count: 0,
        within: None,
        always: None,
        fog: None,
    };
    let render_distance = f64::from(input.render_distance);
    if input.mode == ShadowCullMode::Distance {
        let distance = f64::from(input.shadow_distance * input.render_mul);
        if distance > 0.0 && distance <= render_distance {
            shape.within = Some(around(distance));
        }
        return shape;
    }
    let safe = input.mode == ShadowCullMode::SafeZone;
    let mul = if safe && input.render_mul < 0.0 {
        1.0
    } else {
        input.render_mul
    };
    let distance = if mul < 0.0 {
        render_distance
    } else {
        f64::from(
            if safe {
                input.voxel_distance
            } else {
                input.shadow_distance
            } * mul,
        )
    };
    let boxed = (safe || distance < render_distance).then(|| around(distance));
    for plane in advanced_planes(input.camera_clip, input.toward_light) {
        if shape.plane_count == MAX_CULL_PLANES {
            break;
        }
        let normal = plane.truncate().as_dvec3();
        let w = f64::from(plane.w) - normal.dot(input.camera);
        shape.planes[shape.plane_count] = [plane.x, plane.y, plane.z, w as f32];
        shape.plane_count += 1;
    }
    if safe {
        shape.within = Some(around(f64::from(input.shadow_distance * mul)));
        shape.always = boxed;
    } else {
        shape.within = boxed;
    }
    shape
}

#[cfg(feature = "shader_support")]
fn advanced_planes(camera_clip: Mat4, toward_light: Vec3) -> impl Iterator<Item = Vec4> {
    let t = camera_clip.transpose();
    let base: [Vec4; 6] = [
        Vec4::new(-1.0, 0.0, 0.0, 1.0),
        Vec4::new(1.0, 0.0, 0.0, 1.0),
        Vec4::new(0.0, -1.0, 0.0, 1.0),
        Vec4::new(0.0, 1.0, 0.0, 1.0),
        Vec4::new(0.0, 0.0, -1.0, 1.0),
        Vec4::new(0.0, 0.0, 1.0, 1.0),
    ]
    .map(|v| (t * v).normalize_or_zero());
    let light = toward_light.normalize_or_zero();
    let mut out = [Vec4::ZERO; MAX_CULL_PLANES];
    let mut count = 0;
    let mut add = |plane: Vec4| {
        if count < MAX_CULL_PLANES {
            out[count] = plane;
            count += 1;
        }
    };
    let back: [bool; 6] = base.map(|p| p.truncate().dot(light) > 0.0);
    for plane in base.iter().filter(|p| p.truncate().dot(light) >= 0.0) {
        add(*plane);
    }
    let neighbours = |i: usize| -> [usize; 4] {
        match i / 2 {
            0 => [2, 3, 4, 5],
            1 => [0, 1, 4, 5],
            _ => [0, 1, 2, 3],
        }
    };
    for (i, plane) in base.iter().enumerate() {
        if !back[i] {
            continue;
        }
        for n in neighbours(i) {
            if !back[n] {
                add(edge_plane(*plane, base[n], light));
            }
        }
    }
    out.into_iter().take(count)
}

#[cfg(feature = "shader_support")]
fn edge_plane(back: Vec4, front: Vec4, light: Vec3) -> Vec4 {
    let (b, f) = (back.truncate(), front.truncate());
    let intersection = b.cross(f);
    let normal = intersection.cross(light);
    let point = (intersection.cross(b) * -front.w + f.cross(intersection) * -back.w)
        / intersection.length_squared();
    normal.extend(-normal.dot(point))
}

#[derive(Resource)]
pub struct CullPipeline {
    pub layout: BindGroupLayoutDescriptor,
    pub id: CachedComputePipelineId,
}

#[derive(Resource, Default)]
pub struct CullBindGroup {
    pub group: Option<BindGroup>,
    key: Option<(u64, u64)>,
}

impl CullBindGroup {
    pub const fn new_none() -> Self {
        Self {
            group: None,
            key: None,
        }
    }
}

pub fn build(app: &mut App) {
    embedded_asset!(app, "cull.wgsl");
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    #[cfg(feature = "shader_support")]
    render_app.init_resource::<PackCullView>();
    render_app
        .init_resource::<TerrainViews>()
        .init_resource::<DirectLists>()
        .insert_resource(CullBindGroup::new_none())
        .add_systems(
            Render,
            (
                prepare_cull
                    .in_set(RenderSystems::PrepareResources)
                    .after(super::pools::apply_ops),
                prepare_cull_bind_group.in_set(RenderSystems::PrepareBindGroups),
            ),
        );

    let cull = terrain_cull.before(Core3dSystems::MainPass);
    #[cfg(feature = "builtin_shaders")]
    let cull = cull.before(bevy::pbr::per_view_shadow_pass::<{ bevy::pbr::EARLY_SHADOW_PASS }>);
    render_app.add_systems(Core3d, cull);
}

pub fn finish(app: &mut App) {
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    let device = render_app.world().resource::<RenderDevice>().clone();
    let tier = *render_app.world().resource::<TerrainTier>();

    let buffers = if tier.indirect {
        CullBuffers::new(&device, 1, 1, SLOTS_INITIAL)
    } else {
        CullBuffers::placeholder(&device)
    };
    render_app.insert_resource(buffers);
    if !tier.indirect {
        return;
    }

    let layout_entries = BindGroupLayoutEntries::sequential(
        ShaderStages::COMPUTE,
        (
            storage_buffer_read_only_sized(
                false,
                NonZeroU64::new(std::mem::size_of::<SlotMeta>() as u64),
            ),
            storage_buffer_read_only_sized(false, NonZeroU64::new(4)),
            storage_buffer_sized(false, NonZeroU64::new(INDIRECT_ARGS_SIZE)),
            storage_buffer_sized(false, NonZeroU64::new(4)),
            uniform_buffer_sized(
                true,
                NonZeroU64::new(std::mem::size_of::<CullView>() as u64),
            ),
        ),
    );
    let layout = BindGroupLayoutDescriptor::new("terrain_cull", &layout_entries);

    let shader = render_app
        .world()
        .resource::<super::TerrainShaders>()
        .cull
        .clone();
    let pipeline_cache = render_app.world().resource::<PipelineCache>();
    let id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("terrain_cull".into()),
        layout: vec![layout.clone()],
        immediate_size: 0,
        shader,
        shader_defs: if cfg!(feature = "render_lod") {
            vec!["LOD_REAL_CUT".into()]
        } else {
            vec![]
        },
        entry_point: Some("main".into()),
        zero_initialize_workgroup_memory: false,
    });
    render_app.insert_resource(CullPipeline { layout, id });
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_cull(
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    tier: Res<TerrainTier>,
    pools: Res<TerrainPools>,
    mut cull: ResMut<CullBuffers>,
    mut views: ResMut<TerrainViews>,
    mut lists: ResMut<DirectLists>,
    cameras: Query<(
        Entity,
        &ExtractedView,
        Has<TerrainView>,
        Option<&DistanceFog>,
    )>,
    #[cfg(feature = "builtin_shaders")] view_lights: Query<&ViewLightEntities>,
    #[cfg(feature = "builtin_shaders")] lights: Query<(&ExtractedView, &LightEntity)>,
    #[cfg(feature = "shader_support")] pack_view: Option<Res<PackCullView>>,
    takeover: Option<Res<super::draw::PackTakeover>>,
    origin: Res<crate::renderer::RenderOrigin>,
    mut assigned: Local<Vec<AssignedView>>,
) {
    let pack_draws = takeover.is_some_and(|t| t.0);
    let origin = origin.block();
    #[cfg(feature = "builtin_shaders")]
    let cascades = !pack_draws;
    views.index.clear();
    views.count = 0;

    assigned.clear();

    for (entity, view, is_terrain, fog) in cameras.iter() {
        #[cfg(not(feature = "builtin_shaders"))]
        let _ = entity;
        if !is_terrain || views.count >= MAX_VIEWS {
            continue;
        }
        let slot = views.count;
        views.count += 1;
        views.index.insert(view.retained_view_entity, slot);
        let eye = view.world_from_view.translation();
        let mut shape = CullShape::frustum(frustum_planes(view));
        if !pack_draws {
            shape.fog = fog_cull(fog, eye);
        }
        assigned.push(AssignedView {
            slot,
            key: view.retained_view_entity,
            shape,
            flags: CAMERA_FLAGS,
            eye,
        });

        #[cfg(feature = "shader_support")]
        if let Some(shape) = pack_view.as_ref().and_then(|v| v.0)
            && views.count < MAX_VIEWS
        {
            let slot = views.count;
            views.count += 1;
            let key = pack_shadow_view(view.retained_view_entity);
            views.index.insert(key, slot);
            assigned.push(AssignedView {
                slot,
                key,
                shape,
                flags: PACK_SHADOW_FLAGS,
                eye,
            });
        }

        #[cfg(feature = "builtin_shaders")]
        if cascades && let Ok(view_lights) = view_lights.get(entity) {
            for &light_entity in &view_lights.lights {
                if views.count >= MAX_VIEWS {
                    break;
                }
                let Ok((light_view, light_kind)) = lights.get(light_entity) else {
                    continue;
                };
                if !matches!(light_kind, LightEntity::Directional { .. }) {
                    continue;
                }
                let slot = views.count;
                views.count += 1;
                views.index.insert(light_view.retained_view_entity, slot);
                assigned.push(AssignedView {
                    slot,
                    key: light_view.retained_view_entity,
                    shape: CullShape::frustum(frustum_planes(light_view)),
                    flags: 0,
                    eye,
                });
            }
        }
    }

    let pool_count = pools.pools.len().max(1) as u32;
    if tier.indirect {
        if !cull.fits(views.count, pool_count, pools.slot_cap) {
            let generation = cull.generation + 1;
            *cull = CullBuffers::new(&device, views.count, pool_count, pools.slot_cap);
            cull.generation = generation;
        }
        for view in assigned.iter() {
            let cull_view = view.shape.view(
                view.eye,
                origin,
                cull.slot_cap,
                view.slot,
                cull.pool_count,
                tier.count as u32,
                view.flags,
            );
            let bytes = bytemuck::bytes_of(&cull_view);
            cull.view_scratch[..bytes.len()].copy_from_slice(bytes);
            queue.write_buffer(
                &cull.views,
                (view.slot as u64) * CULL_VIEW_STRIDE,
                &cull.view_scratch,
            );
        }

        for view in assigned.iter() {
            if view.flags & VIEW_WATER != 0 {
                cull.write_water(&queue, &pools, view, origin);
            } else {
                cull.forget_water(view.slot);
            }
        }
        return;
    }

    let stats = &pools.stats.0;
    stats.draws.store(
        stats.draws_pending.swap(0, Ordering::Relaxed),
        Ordering::Relaxed,
    );
    let mut camera_drawn = 0usize;
    for per_view in lists.0.values_mut() {
        for list in per_view.iter_mut() {
            list.clear();
        }
    }
    let end = (pools.draw_end as usize).min(pools.meta_cpu.len());
    for view in assigned.iter() {
        let (shape, flags) = (&view.shape, view.flags);
        let per_view = lists.0.entry(view.key).or_default();
        for (index, meta) in pools.meta_cpu[..end].iter().enumerate() {
            if !pools.is_live(index as u32) {
                continue;
            }
            if flags & VIEW_UNOCCLUDED == 0 && !pools.is_visible(index as u32) {
                continue;
            }
            let (lo, hi) = meta.render_box(origin);
            #[cfg(feature = "render_lod")]
            let keep = shape.keeps_section(lo, hi, meta.render_box_of([0.0; 3], origin));
            #[cfg(not(feature = "render_lod"))]
            let keep = shape.keeps(lo, hi);
            if !keep {
                continue;
            }
            if flags == CAMERA_FLAGS {
                camera_drawn += 1;
            }
            let water = meta.flags & super::META_WATER != 0;
            if water {
                if flags & VIEW_WATER != 0 {
                    per_view[super::STREAM_WATER as usize].push(index as u32);
                }
                continue;
            }
            if meta.solid_count > 0 {
                per_view[super::STREAM_SOLID as usize].push(index as u32);
            }
            if meta.index_count > meta.solid_count {
                per_view[super::STREAM_CUTOUT as usize].push(index as u32);
            }
        }
    }

    stats.drawn.store(camera_drawn, Ordering::Relaxed);

    for per_view in lists.0.values_mut() {
        for (stream, list) in (0u32..).zip(per_view.iter_mut()) {
            list.sort_unstable_by_key(|&slot| {
                let meta = &pools.meta_cpu[slot as usize];
                (meta.pool, meta.stream_range(stream).0)
            });
        }
    }
}

fn frustum_planes(view: &ExtractedView) -> [[f32; 4]; 6] {
    let clip_from_world = view
        .clip_from_world
        .unwrap_or_else(|| view.clip_from_view * view.world_from_view.to_matrix().inverse());
    planes_of(&clip_from_world)
}

fn planes_of(clip_from_world: &Mat4) -> [[f32; 4]; 6] {
    let frustum = ViewFrustum::from_clip_from_world(clip_from_world);
    let mut planes = [[0.0f32; 4]; 6];
    for i in 0..6 {
        planes[i] = frustum.half_spaces[i].normal_d().into();
    }
    planes
}

#[cfg(lua_mods)]
pub fn frustum_of(clip_from_world: &Mat4) -> CullShape {
    CullShape::frustum(planes_of(clip_from_world))
}

pub fn prepare_cull_bind_group(
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Option<Res<CullPipeline>>,
    pools: Res<TerrainPools>,
    cull: Res<CullBuffers>,
    mut bind_group: ResMut<CullBindGroup>,
) {
    let Some(pipeline) = pipeline else { return };
    let SlotTable::Indirect { meta, vis } = &pools.table else {
        return;
    };
    let key = (pools.tables_generation, cull.generation);
    if bind_group.key == Some(key) && bind_group.group.is_some() {
        return;
    }
    let layout = pipeline_cache.get_bind_group_layout(&pipeline.layout);
    let group = device.create_bind_group(
        "terrain_cull",
        &layout,
        &BindGroupEntries::sequential((
            meta.as_entire_binding(),
            vis.as_entire_binding(),
            cull.commands.as_entire_binding(),
            cull.counts.as_entire_binding(),
            BufferBinding {
                buffer: &cull.views,
                offset: 0,
                size: Some(NonZeroU64::new(CULL_VIEW_STRIDE).unwrap()),
            },
        )),
    );
    bind_group.group = Some(group);
    bind_group.key = Some(key);
}

pub fn terrain_cull(
    world: &World,
    view: ViewQuery<(&ExtractedView, Has<TerrainView>)>,
    mut ctx: RenderContext,
) {
    let (view, is_terrain) = view.into_inner();
    if !is_terrain || !world.resource::<TerrainTier>().indirect {
        return;
    }
    let terrain_views = world.resource::<TerrainViews>();
    if !terrain_views.index.contains_key(&view.retained_view_entity) {
        return;
    }
    let cull = world.resource::<CullBuffers>();
    ctx.command_encoder()
        .clear_buffer(&cull.counts, 0, Some(cull.regions as u64 * 4));
    let pipeline_cache = world.resource::<PipelineCache>();
    let Some(cull_pipeline) = world.get_resource::<CullPipeline>() else {
        return;
    };
    let Some(pipeline) = pipeline_cache.get_compute_pipeline(cull_pipeline.id) else {
        return;
    };
    let Some(bind_group) = world.resource::<CullBindGroup>().group.as_ref() else {
        return;
    };

    let draw_end = world.resource::<TerrainPools>().draw_end.min(cull.slot_cap);
    if draw_end == 0 {
        return;
    }
    let workgroups = draw_end.div_ceil(64);
    let recorder = ctx.diagnostic_recorder();
    let recorder = recorder.as_deref();
    let mut pass = ctx
        .command_encoder()
        .begin_compute_pass(&ComputePassDescriptor {
            label: Some("terrain_cull"),
            timestamp_writes: None,
        });
    let span = recorder.pass_span(&mut pass, "terrain_cull");
    pass.set_pipeline(pipeline);
    for view_index in 0..terrain_views.count {
        pass.set_bind_group(0, bind_group, &[view_index * (CULL_VIEW_STRIDE as u32)]);
        pass.dispatch_workgroups(workgroups, 1, 1);
    }
    span.end(&mut pass);
}

#[cfg(all(test, feature = "shader_support"))]
mod tests {
    use super::*;
    use bevy::math::Vec3;

    fn wgsl_size(module: &naga::Module, name: &str) -> usize {
        let handle = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("{name} declared"))
            .0;
        let mut layouter = naga::proc::Layouter::default();
        layouter.update(module.to_ctx()).expect("lays out");
        layouter[handle].size as usize
    }

    fn wgsl_offsets(module: &naga::Module, name: &str) -> Vec<(String, u32)> {
        let (_, ty) = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("{name} declared"));
        let naga::TypeInner::Struct { members, .. } = &ty.inner else {
            panic!("{name} is a struct")
        };
        members
            .iter()
            .map(|m| (m.name.clone().unwrap_or_default(), m.offset))
            .collect()
    }

    #[test]
    fn the_cull_shader_validates_and_its_view_matches() {
        let module = naga::front::wgsl::parse_str(include_str!("cull.wgsl")).expect("parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("validates");
        assert_eq!(
            wgsl_size(&module, "CullView"),
            std::mem::size_of::<CullView>()
        );
        let offset = |name: &str| {
            wgsl_offsets(&module, "CullView")
                .into_iter()
                .find(|(n, _)| n == name)
                .expect("member")
                .1 as usize
        };
        assert_eq!(offset("eye"), std::mem::offset_of!(CullView, eye));
        assert_eq!(offset("slot_cap"), std::mem::offset_of!(CullView, slot_cap));
        assert_eq!(
            offset("plane_count"),
            std::mem::offset_of!(CullView, plane_count)
        );
        assert_eq!(offset("origin"), std::mem::offset_of!(CullView, origin));
    }

    #[test]
    fn every_slot_meta_matches() {
        let module = naga::front::wgsl::parse_str(include_str!("cull.wgsl")).expect("parses");
        assert_eq!(
            wgsl_size(&module, "SlotMeta"),
            std::mem::size_of::<SlotMeta>()
        );
        let offsets = wgsl_offsets(&module, "SlotMeta");
        let at = |name: &str| offsets.iter().find(|(n, _)| n == name).expect("member").1 as usize;
        assert_eq!(at("origin"), std::mem::offset_of!(SlotMeta, origin));
        assert_eq!(
            at("cutout_first"),
            std::mem::offset_of!(SlotMeta, cutout_first)
        );
        assert_eq!(at("faces"), std::mem::offset_of!(SlotMeta, faces));
        let declaration = |source: &str| {
            let start = source.find("struct SlotMeta {").expect("declared");
            let end = start + source[start..].find('}').expect("closed");
            source[start..=end].split_whitespace().collect::<String>()
        };
        let cull = declaration(include_str!("cull.wgsl"));
        assert_eq!(
            declaration(include_str!("../terrain_common.wgsl")),
            cull,
            "terrain_common.wgsl"
        );
    }

    fn section(lo: Vec3) -> ([f32; 3], [f32; 3]) {
        (lo.to_array(), (lo + Vec3::splat(16.0)).to_array())
    }

    fn input(mode: ShadowCullMode, camera: Vec3, toward_light: Vec3) -> ShadowCullInput {
        ShadowCullInput {
            mode,
            camera_clip: Mat4::perspective_rh_gl(70f32.to_radians(), 16.0 / 9.0, 0.05, 512.0),
            toward_light,
            camera: camera.as_dvec3(),
            shadow_distance: 192.0,
            voxel_distance: 0.0,
            render_mul: 1.0,
            render_distance: 256.0,
        }
    }

    #[test]
    fn distance_culling_is_a_box_around_the_camera() {
        let camera = Vec3::new(1000.0, 64.0, -5000.0);
        let shape = shadow_cull_shape(&input(ShadowCullMode::Distance, camera, Vec3::Y));
        let (lo, hi) = section(camera + Vec3::new(100.0, 0.0, 100.0));
        assert!(shape.keeps(lo, hi));
        let (lo, hi) = section(camera + Vec3::new(300.0, 0.0, 0.0));
        assert!(!shape.keeps(lo, hi), "past shadowDistance");
    }

    #[test]
    fn advanced_culling_keeps_casters_into_the_view() {
        let camera = Vec3::new(1000.0, 64.0, -5000.0);
        let shape = shadow_cull_shape(&input(ShadowCullMode::Advanced, camera, Vec3::Y));
        let keeps = |offset: Vec3| {
            let (lo, hi) = section(camera + offset);
            shape.keeps(lo, hi)
        };
        assert!(keeps(Vec3::new(-8.0, -8.0, -60.0)), "in view");
        assert!(
            keeps(Vec3::new(-8.0, 80.0, -60.0)),
            "above the view, casting down into it"
        );
        assert!(!keeps(Vec3::new(-8.0, -8.0, 120.0)), "behind the camera");
    }

    #[test]
    fn the_safe_zone_keeps_its_box() {
        let camera = Vec3::new(8.0, 64.0, 8.0);
        let mut safe = input(ShadowCullMode::SafeZone, camera, Vec3::Y);
        safe.voxel_distance = 64.0;
        let shape = shadow_cull_shape(&safe);
        let (lo, hi) = section(camera + Vec3::new(-8.0, -8.0, 30.0));
        assert!(
            shape.keeps(lo, hi),
            "behind the camera but in the voxel box"
        );
        let (lo, hi) = section(camera + Vec3::new(-8.0, -8.0, 120.0));
        assert!(!shape.keeps(lo, hi), "behind the camera past the voxel box");
        let (lo, hi) = section(camera + Vec3::new(300.0, 0.0, -60.0));
        assert!(!shape.keeps(lo, hi), "past shadowDistance");
    }

    #[test]
    fn pack_shadow_view_is_its_own_key() {
        let camera =
            bevy::render::view::RetainedViewEntity::new(Entity::PLACEHOLDER.into(), None, 0);
        assert_ne!(pack_shadow_view(camera), camera);
        assert_eq!(pack_shadow_view(camera), pack_shadow_view(camera));
    }
}
