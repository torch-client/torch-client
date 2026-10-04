use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::atomic::Ordering;

use bevy::app::App;
use bevy::asset::embedded_asset;
use bevy::camera::primitives::Frustum;
use bevy::ecs::query::{Has, QueryItem};
use bevy::prelude::*;
use bevy::render::render_graph::{
    NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
};
use bevy::render::render_resource::binding_types::{
    storage_buffer_read_only_sized, storage_buffer_sized, uniform_buffer_sized,
};
use bevy::render::render_resource::{
    BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, Buffer,
    BufferBinding, BufferDescriptor, BufferUsages, CachedComputePipelineId, ComputePassDescriptor,
    ComputePipelineDescriptor, PipelineCache, ShaderStages,
};
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue};
use bevy::render::view::ExtractedView;
use bevy::render::{Render, RenderApp, RenderSystems};
use bytemuck::{Pod, Zeroable};

use super::pools::{SlotTable, TerrainPools};
use super::{MAX_VIEWS, SLOTS_INITIAL, STREAMS, TerrainTier, TerrainView};

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
    pub slot_cap: u32,
    pub view_index: u32,
    pub pool_count: u32,
    pub compact: u32,
    pub flags: u32,
    pub plane_count: u32,
    pub pad: [u32; 2],
}

pub const VIEW_WATER: u32 = 1;
pub const VIEW_UNOCCLUDED: u32 = 2;
pub const VIEW_WITHIN: u32 = 4;
pub const VIEW_ALWAYS: u32 = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CullShape {
    pub planes: [[f32; 4]; MAX_CULL_PLANES],
    pub plane_count: usize,
    pub within: Option<[[f32; 3]; 2]>,
    pub always: Option<[[f32; 3]; 2]>,
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
        }
    }

    pub fn keeps(&self, lo: [f32; 3], hi: [f32; 3]) -> bool {
        if let Some([min, max]) = self.within
            && (0..3).any(|a| hi[a] < min[a] || lo[a] > max[a])
        {
            return false;
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

    fn view(
        &self,
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
                },
            plane_count: self.plane_count as u32,
            pad: [0; 2],
        }
    }
}

const CAMERA_FLAGS: u32 = VIEW_WATER;
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
    zero_counts: Vec<u8>,
    view_scratch: [u8; CULL_VIEW_STRIDE as usize],
    water_counts: Vec<u32>,
    water_order: Vec<(u32, f32, u32)>,
    water_args: Vec<[u32; 5]>,
}

impl CullBuffers {
    fn new(device: &RenderDevice, view_count: u32, pool_count: u32, slot_cap: u32) -> Self {
        let pool_count = pool_count.max(1);
        let view_count = view_count.clamp(1, MAX_VIEWS);
        let regions = view_count * pool_count * STREAMS;
        let commands = device.create_buffer(&BufferDescriptor {
            label: Some("terrain_cull_commands"),
            size: (regions as u64) * (slot_cap as u64) * INDIRECT_ARGS_SIZE,
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
            zero_counts: vec![0u8; (regions as usize) * 4],
            view_scratch: [0u8; CULL_VIEW_STRIDE as usize],
            water_counts: Vec::new(),
            water_order: Vec::new(),
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
            zero_counts: Vec::new(),
            view_scratch: [0u8; CULL_VIEW_STRIDE as usize],
            water_counts: Vec::new(),
            water_order: Vec::new(),
            water_args: Vec::new(),
        }
    }

    fn fits(&self, view_count: u32, pool_count: u32, slot_cap: u32) -> bool {
        self.view_count >= view_count.max(1)
            && self.pool_count >= pool_count.max(1)
            && self.slot_cap >= slot_cap
    }

    fn write_water(&mut self, queue: &RenderQueue, pools: &TerrainPools, view: &AssignedView) {
        self.water_order.clear();
        for &slot in &pools.water_slots {
            if view.flags & VIEW_UNOCCLUDED == 0 && !pools.is_visible(slot) {
                continue;
            }
            let meta = &pools.meta_cpu[slot as usize];
            let (lo, hi) = meta.world_box();
            if !view.shape.keeps(lo, hi) {
                continue;
            }
            let centre = (Vec3::from(lo) + Vec3::from(hi)) * 0.5;
            self.water_order
                .push((meta.pool, centre.distance_squared(view.eye), slot));
        }
        self.water_order
            .sort_unstable_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)));
        for run in self.water_order.chunk_by(|a, b| a.0 == b.0) {
            let pool = run[0].0;
            self.water_args.clear();
            self.water_args.extend(run.iter().map(|&(_, _, slot)| {
                let meta = &pools.meta_cpu[slot as usize];
                [
                    meta.index_count,
                    1,
                    meta.first_index,
                    meta.base_vertex,
                    slot,
                ]
            }));
            let region = (view.slot * self.pool_count + pool) * STREAMS + super::STREAM_WATER;
            queue.write_buffer(
                &self.commands,
                region as u64 * self.slot_cap as u64 * INDIRECT_ARGS_SIZE,
                bytemuck::cast_slice(&self.water_args),
            );
            self.water_counts[(view.slot * self.pool_count + pool) as usize] = run.len() as u32;
        }
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

#[derive(RenderLabel, Debug, Hash, PartialEq, Eq, Clone)]
pub struct TerrainCullLabel;

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

    render_app
        .add_render_graph_node::<ViewNodeRunner<TerrainCullNode>>(
            bevy::core_pipeline::core_3d::graph::Core3d,
            TerrainCullLabel,
        )
        .add_render_graph_edges(
            bevy::core_pipeline::core_3d::graph::Core3d,
            (
                TerrainCullLabel,
                bevy::core_pipeline::core_3d::graph::Node3d::StartMainPass,
            ),
        );
    #[cfg(feature = "builtin_shaders")]
    render_app.add_render_graph_edges(
        bevy::core_pipeline::core_3d::graph::Core3d,
        (TerrainCullLabel, bevy::pbr::graph::NodePbr::EarlyShadowPass),
    );
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
            storage_buffer_read_only_sized(false, NonZeroU64::new(64)),
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
        push_constant_ranges: vec![],
        shader,
        shader_defs: vec![],
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
    cameras: Query<(Entity, &ExtractedView, Has<TerrainView>)>,
    #[cfg(feature = "builtin_shaders")] view_lights: Query<&ViewLightEntities>,
    #[cfg(feature = "builtin_shaders")] lights: Query<(&ExtractedView, &LightEntity)>,
    #[cfg(feature = "shader_support")] pack_view: Option<Res<PackCullView>>,
    #[cfg(feature = "builtin_shaders")] takeover: Option<Res<super::draw::PackTakeover>>,
    mut assigned: Local<Vec<AssignedView>>,
) {
    #[cfg(feature = "builtin_shaders")]
    let cascades = !takeover.is_some_and(|t| t.0);
    views.index.clear();
    views.count = 0;

    assigned.clear();

    for (entity, view, is_terrain) in cameras.iter() {
        #[cfg(not(feature = "builtin_shaders"))]
        let _ = entity;
        if !is_terrain || views.count >= MAX_VIEWS {
            continue;
        }
        let slot = views.count;
        views.count += 1;
        views.index.insert(view.retained_view_entity, slot);
        let eye = view.world_from_view.translation();
        assigned.push(AssignedView {
            slot,
            key: view.retained_view_entity,
            shape: CullShape::frustum(frustum_planes(view)),
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
        let live_regions = (cull.regions as usize) * 4;
        queue.write_buffer(&cull.counts, 0, &cull.zero_counts[..live_regions]);

        let cull = &mut *cull;
        cull.water_counts.clear();
        cull.water_counts
            .resize((cull.view_count * cull.pool_count) as usize, 0);
        for view in assigned.iter().filter(|view| view.flags & VIEW_WATER != 0) {
            cull.write_water(&queue, &pools, view);
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
            let (lo, hi) = meta.world_box();
            if !shape.keeps(lo, hi) {
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
    let frustum = Frustum::from_clip_from_world(&clip_from_world);
    let mut planes = [[0.0f32; 4]; 6];
    for i in 0..6 {
        planes[i] = frustum.half_spaces[i].normal_d().into();
    }
    planes
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

#[derive(Default)]
pub struct TerrainCullNode;

impl ViewNode for TerrainCullNode {
    type ViewQuery = (Entity, &'static ExtractedView, Has<TerrainView>);

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (_entity, view, is_terrain): QueryItem<'w, '_, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        if !is_terrain || !world.resource::<TerrainTier>().indirect {
            return Ok(());
        }
        let terrain_views = world.resource::<TerrainViews>();
        if !terrain_views.index.contains_key(&view.retained_view_entity) {
            return Ok(());
        }
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(cull_pipeline) = world.get_resource::<CullPipeline>() else {
            return Ok(());
        };
        let Some(pipeline) = pipeline_cache.get_compute_pipeline(cull_pipeline.id) else {
            return Ok(());
        };
        let Some(bind_group) = world.resource::<CullBindGroup>().group.as_ref() else {
            return Ok(());
        };
        let cull = world.resource::<CullBuffers>();

        let draw_end = world.resource::<TerrainPools>().draw_end.min(cull.slot_cap);
        if draw_end == 0 {
            return Ok(());
        }
        let workgroups = draw_end.div_ceil(64);
        let mut pass =
            render_context
                .command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {
                    label: Some("terrain_cull"),
                    timestamp_writes: None,
                });
        pass.set_pipeline(pipeline);
        for view_index in 0..terrain_views.count {
            pass.set_bind_group(0, bind_group, &[view_index * (CULL_VIEW_STRIDE as u32)]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }
        Ok(())
    }
}

#[cfg(all(test, feature = "shader_support"))]
mod tests {
    use super::*;
    use bevy::math::Vec3;

    #[test]
    fn the_cull_shader_validates_and_its_view_matches() {
        let module = naga::front::wgsl::parse_str(include_str!("cull.wgsl")).expect("parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("validates");
        let handle = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("CullView"))
            .expect("declared")
            .0;
        let mut layouter = naga::proc::Layouter::default();
        layouter.update(module.to_ctx()).expect("lays out");
        assert_eq!(
            layouter[handle].size as usize,
            std::mem::size_of::<CullView>()
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
