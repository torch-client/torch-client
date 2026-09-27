use std::collections::HashMap;
use std::num::NonZeroU64;

use bevy::app::App;
use bevy::asset::embedded_asset;
use bevy::camera::primitives::Frustum;
use bevy::ecs::query::{Has, QueryItem};
use bevy::ecs::system::lifetimeless::SRes;
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

use super::pools::TerrainPools;
use super::{MAX_VIEWS, SLOTS_INITIAL, STREAMS, TerrainTier, TerrainView};

#[cfg(feature = "builtin_shaders")]
use bevy::pbr::{LightEntity, ViewLightEntities};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct CullView {
    pub planes: [[f32; 4]; 6],
    pub slot_cap: u32,
    pub view_index: u32,
    pub pool_count: u32,
    pub compact: u32,
}

const CULL_VIEW_STRIDE: u64 = 256;

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
        }
    }

    fn fits(&self, view_count: u32, pool_count: u32, slot_cap: u32) -> bool {
        self.view_count >= view_count.max(1)
            && self.pool_count >= pool_count.max(1)
            && self.slot_cap >= slot_cap
    }
}

#[derive(Resource, Default)]
pub struct TerrainViews {
    pub index: HashMap<bevy::render::view::RetainedViewEntity, u32>,
    pub count: u32,
}

#[derive(Resource, Default)]
pub struct DirectLists(pub HashMap<(bevy::render::view::RetainedViewEntity, u32), Vec<u32>>);

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
) {
    views.index.clear();
    views.count = 0;

    let mut assigned: Vec<(u32, bevy::render::view::RetainedViewEntity, [[f32; 4]; 6])> =
        Vec::new();

    for (entity, view, is_terrain) in cameras.iter() {
        #[cfg(not(feature = "builtin_shaders"))]
        let _ = entity;
        if !is_terrain || views.count >= MAX_VIEWS {
            continue;
        }
        let slot = views.count;
        views.count += 1;
        views.index.insert(view.retained_view_entity, slot);
        assigned.push((slot, view.retained_view_entity, frustum_planes(view)));

        #[cfg(feature = "builtin_shaders")]
        if let Ok(view_lights) = view_lights.get(entity) {
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
                assigned.push((
                    slot,
                    light_view.retained_view_entity,
                    frustum_planes(light_view),
                ));
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
        for &(slot, _, planes) in &assigned {
            let cull_view = CullView {
                planes,
                slot_cap: cull.slot_cap,
                view_index: slot,
                pool_count: cull.pool_count,
                compact: tier.count as u32,
            };
            let bytes = bytemuck::bytes_of(&cull_view);
            cull.view_scratch[..bytes.len()].copy_from_slice(bytes);
            queue.write_buffer(
                &cull.views,
                (slot as u64) * CULL_VIEW_STRIDE,
                &cull.view_scratch,
            );
        }
        let live_regions = (cull.regions as usize) * 4;
        queue.write_buffer(&cull.counts, 0, &cull.zero_counts[..live_regions]);
        return;
    }

    for lists_vec in lists.0.values_mut() {
        lists_vec.clear();
    }
    for &(slot, retained, planes) in &assigned {
        let is_shadow = slot != 0;
        for (index, meta) in pools.meta_cpu.iter().enumerate() {
            if !pools.is_live(index as u32) {
                continue;
            }
            let vis_word = pools.vis_cpu.get(index / 32).copied().unwrap_or(u32::MAX);
            if (vis_word >> (index % 32)) & 1 == 0 {
                continue;
            }
            let lo = [
                (meta.origin[0] as f32) + meta.min[0],
                (meta.origin[1] as f32) + meta.min[1],
                (meta.origin[2] as f32) + meta.min[2],
            ];
            let hi = [
                (meta.origin[0] as f32) + meta.max[0],
                (meta.origin[1] as f32) + meta.max[1],
                (meta.origin[2] as f32) + meta.max[2],
            ];
            if !aabb_in_frustum(&planes, lo, hi) {
                continue;
            }
            let water = meta.flags & super::META_WATER != 0;
            if water {
                if !is_shadow {
                    lists
                        .0
                        .entry((retained, super::STREAM_WATER))
                        .or_default()
                        .push(index as u32);
                }
                continue;
            }
            if meta.solid_count > 0 {
                lists
                    .0
                    .entry((retained, super::STREAM_SOLID))
                    .or_default()
                    .push(index as u32);
            }
            if meta.index_count > meta.solid_count {
                lists
                    .0
                    .entry((retained, super::STREAM_CUTOUT))
                    .or_default()
                    .push(index as u32);
            }
        }
    }

    for list in lists.0.values_mut() {
        list.sort_unstable_by_key(|&slot| {
            let meta = &pools.meta_cpu[slot as usize];
            (meta.pool, meta.first_index)
        });
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

fn aabb_in_frustum(planes: &[[f32; 4]; 6], lo: [f32; 3], hi: [f32; 3]) -> bool {
    for p in planes {
        let v = [
            if p[0] > 0.0 { hi[0] } else { lo[0] },
            if p[1] > 0.0 { hi[1] } else { lo[1] },
            if p[2] > 0.0 { hi[2] } else { lo[2] },
        ];
        if p[0] * v[0] + p[1] * v[1] + p[2] * v[2] + p[3] < 0.0 {
            return false;
        }
    }
    true
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
    let key = (pools.tables_generation, cull.generation);
    if bind_group.key == Some(key) && bind_group.group.is_some() {
        return;
    }
    let layout = pipeline_cache.get_bind_group_layout(&pipeline.layout);
    let group = device.create_bind_group(
        "terrain_cull",
        &layout,
        &BindGroupEntries::sequential((
            pools.meta.as_entire_binding(),
            pools.vis.as_entire_binding(),
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

        let workgroups = cull.slot_cap.div_ceil(64);
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
