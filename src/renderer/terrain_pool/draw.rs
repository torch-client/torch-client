use std::mem::size_of;
use std::num::NonZeroU64;
use std::sync::atomic::Ordering;

use bevy::asset::uuid::Uuid;
use bevy::asset::{AssetId, UntypedAssetId};
use bevy::core_pipeline::core_3d::{
    AlphaMask3d, Opaque3d, Opaque3dBatchSetKey, Opaque3dBinKey, Transparent3d,
};
use bevy::core_pipeline::prepass::{OpaqueNoLightmap3dBatchSetKey, OpaqueNoLightmap3dBinKey};
use bevy::ecs::query::Has;
use bevy::ecs::system::lifetimeless::{Read, SRes};
use bevy::ecs::system::{SystemChangeTick, SystemParamItem};
use bevy::mesh::{
    MeshVertexBufferLayout, MeshVertexBufferLayoutRef, MeshVertexBufferLayouts, VertexBufferLayout,
};
use bevy::pbr::{
    DistanceFog, FogFalloff, MeshPipeline, MeshPipelineKey, SetMeshViewBindGroup,
    SetMeshViewBindingArrayBindGroup, ViewKeyCache,
};
use bevy::prelude::*;
use bevy::render::mesh::allocator::SlabId;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_phase::{
    AddRenderCommand, BinnedRenderPhaseType, DrawFunctions, InputUniformIndex, PhaseItem,
    PhaseItemExtraIndex, RenderCommand, RenderCommandResult, SetItemPipeline, TrackedRenderPass,
    ViewBinnedRenderPhases, ViewSortedRenderPhases,
};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer_sized};
use bevy::render::render_resource::{
    BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, Buffer,
    BufferDescriptor, BufferUsages, Face, IndexFormat, PipelineCache, PrimitiveTopology,
    RenderPipelineDescriptor, SamplerBindingType, ShaderStages, SpecializedRenderPipeline,
    SpecializedRenderPipelines, TextureSampleType, TextureViewId, VertexAttribute, VertexFormat,
    VertexStepMode,
};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::sync_world::MainEntity;
use bevy::render::texture::GpuImage;
use bevy::render::view::{ExtractedView, RetainedViewEntity};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::{Shader, ShaderDefVal};

use super::cull::{CullBuffers, DirectLists, TerrainViews};
use super::pools::{SlotTable, TerrainPools};
use super::{
    FACE_RUNS, STREAM_CUTOUT, STREAM_SOLID, STREAM_WATER, STREAMS, SlotMeta, TerrainParams,
    TerrainShaders, TerrainTextures, TerrainTier, TerrainView,
};

#[cfg(feature = "builtin_shaders")]
use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
#[cfg(feature = "builtin_shaders")]
use bevy::pbr::{
    LightEntity, LightKeyCache, PrepassPipeline, SetPrepassViewBindGroup,
    SetPrepassViewEmptyBindGroup, Shadow, ShadowBatchSetKey, ShadowBinKey, ViewLightEntities,
};
#[cfg(feature = "builtin_shaders")]
use bevy::render::render_resource::{
    CompareFunction, DepthStencilState, FragmentState, MultisampleState, PrimitiveState,
    StencilFaceState, StencilState, VertexState,
};

pub const TERRAIN_GROUP: usize = 2;

const VERTEX_STRIDE: u64 = 20;

const TERRAIN_ASSET_TAG: u128 = 0x7e_77a1 << 64;

pub(crate) fn packed_vertex_layout() -> VertexBufferLayout {
    VertexBufferLayout {
        array_stride: VERTEX_STRIDE,
        step_mode: VertexStepMode::Vertex,
        attributes: vec![
            VertexAttribute {
                format: VertexFormat::Sint16x4,
                offset: 0,
                shader_location: 0,
            },
            VertexAttribute {
                format: VertexFormat::Unorm16x2,
                offset: 8,
                shader_location: 1,
            },
            VertexAttribute {
                format: VertexFormat::Unorm8x4,
                offset: 12,
                shader_location: 2,
            },
            VertexAttribute {
                format: VertexFormat::Unorm8x4,
                offset: 16,
                shader_location: 3,
            },
        ],
    }
}

fn item_asset_id(pool: u32, stream: u32) -> UntypedAssetId {
    AssetId::<Mesh>::Uuid {
        uuid: Uuid::from_u128(TERRAIN_ASSET_TAG | (pool * STREAMS + stream) as u128),
    }
    .untyped()
}

fn item_entity(pool: u32, stream: u32) -> (Entity, MainEntity) {
    let entity =
        Entity::from_raw_u32(0xFF00_0000 + pool * STREAMS + stream).expect("below u32::MAX");
    (entity, MainEntity::from(entity))
}

fn decode_asset_id(id: UntypedAssetId) -> (u32, u32) {
    match id {
        UntypedAssetId::Uuid { uuid, .. } => {
            let packed = uuid.as_u128() as u32;
            (packed / STREAMS, packed % STREAMS)
        }
        UntypedAssetId::Index { .. } => (0, STREAM_SOLID),
    }
}

pub trait TerrainItem: PhaseItem {
    fn pool_stream(&self) -> (u32, u32);
}

impl TerrainItem for Opaque3d {
    fn pool_stream(&self) -> (u32, u32) {
        decode_asset_id(self.bin_key.asset_id)
    }
}

impl TerrainItem for AlphaMask3d {
    fn pool_stream(&self) -> (u32, u32) {
        decode_asset_id(self.bin_key.asset_id)
    }
}

impl TerrainItem for Transparent3d {
    fn pool_stream(&self) -> (u32, u32) {
        match self.extra_index {
            PhaseItemExtraIndex::DynamicOffset(packed) => (packed / STREAMS, packed % STREAMS),
            _ => (0, STREAM_WATER),
        }
    }
}

#[cfg(feature = "builtin_shaders")]
impl TerrainItem for Shadow {
    fn pool_stream(&self) -> (u32, u32) {
        decode_asset_id(self.bin_key.asset_id)
    }
}

#[derive(Resource)]
pub struct TerrainParamsBuffer(pub Buffer);

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ParamsUniform {
    misc: [f32; 4],
    fog_color: [f32; 4],
    fog_ramps: [f32; 4],
    origin: [i32; 4],
    wave_phase: [f32; 4],
    wind_plant_phase: [f32; 4],
    wind_leaf_phase: [f32; 4],
}

const PARAMS_SIZE: u64 = size_of::<ParamsUniform>() as u64;

const _: () = assert!(std::mem::offset_of!(ParamsUniform, origin) == 48);

const WIND_FREQ_PLANT: f64 = 0.42;
const WIND_FREQ_LEAF: f64 = 0.05;

fn noise_phases(origin: IVec3) -> ([f32; 4], [f32; 4], [f32; 4]) {
    let (x, z) = (f64::from(origin.x), f64::from(origin.z));
    let turn = |kx: f64, kz: f64| (kx * x + kz * z).rem_euclid(std::f64::consts::TAU) as f32;
    let wind = |f: f64| {
        [
            turn(f, f),
            turn(1.7 * f, -1.1 * f),
            turn(-0.4 * f, 0.9 * f),
            0.0,
        ]
    };
    (
        [turn(0.55, 0.0), turn(0.0, 0.42), turn(0.29, 0.29), 0.0],
        wind(WIND_FREQ_PLANT),
        wind(WIND_FREQ_LEAF),
    )
}

#[derive(Resource, Default)]
pub struct TerrainBindGroup {
    pub group: Option<BindGroup>,
    key: Option<(TextureViewId, TextureViewId, u64)>,
}

#[derive(Resource)]
pub struct TerrainPipeline {
    mesh_pipeline: MeshPipeline,
    #[cfg(feature = "builtin_shaders")]
    prepass: PrepassPipeline,
    shader: Handle<Shader>,
    shadow_shader: Handle<Shader>,
    vertex_layout_ref: MeshVertexBufferLayoutRef,
    layout_indirect: BindGroupLayoutDescriptor,
    layout_direct: BindGroupLayoutDescriptor,
}

#[derive(Resource, Default)]
pub struct PackTakeover(pub bool);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerrainPipelineKey {
    pub view: MeshPipelineKey,
    pub stream: u32,
    pub direct: bool,
    pub shadow: bool,
    pub fancy: bool,
    pub specular: bool,
}

#[cfg(feature = "shader_support")]
pub const SLOT_TABLE_BINDING: u32 = 5;

fn terrain_layouts() -> (BindGroupLayoutDescriptor, BindGroupLayoutDescriptor) {
    let meta_size = NonZeroU64::new(size_of::<SlotMeta>() as u64);
    let common = (
        texture_2d(TextureSampleType::Float { filterable: true }),
        sampler(SamplerBindingType::Filtering),
        texture_2d(TextureSampleType::Float { filterable: true }),
        sampler(SamplerBindingType::Filtering),
        uniform_buffer_sized(false, NonZeroU64::new(PARAMS_SIZE)),
    );
    let indirect = BindGroupLayoutEntries::sequential(
        ShaderStages::VERTEX_FRAGMENT,
        (
            common.0,
            common.1,
            common.2,
            common.3,
            common.4,
            bevy::render::render_resource::binding_types::storage_buffer_read_only_sized(
                false, meta_size,
            ),
        ),
    );
    let direct = BindGroupLayoutEntries::sequential(
        ShaderStages::VERTEX_FRAGMENT,
        (
            common.0,
            common.1,
            common.2,
            common.3,
            common.4,
            texture_2d(TextureSampleType::Sint),
        ),
    );
    (
        BindGroupLayoutDescriptor::new("terrain_indirect", &indirect),
        BindGroupLayoutDescriptor::new("terrain_direct", &direct),
    )
}

impl FromWorld for TerrainPipeline {
    fn from_world(world: &mut World) -> Self {
        let shaders = world.resource::<TerrainShaders>().clone();
        let (layout_indirect, layout_direct) = terrain_layouts();
        let mut layouts = MeshVertexBufferLayouts::default();
        let vertex_layout_ref = layouts.insert(MeshVertexBufferLayout::new(
            vec![Mesh::ATTRIBUTE_POSITION.id],
            VertexBufferLayout {
                array_stride: 12,
                step_mode: VertexStepMode::Vertex,
                attributes: vec![VertexAttribute {
                    format: VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                }],
            },
        ));
        Self {
            mesh_pipeline: world.resource::<MeshPipeline>().clone(),
            #[cfg(feature = "builtin_shaders")]
            prepass: world.resource::<PrepassPipeline>().clone(),
            shader: shaders.terrain,
            shadow_shader: shaders.prepass,
            vertex_layout_ref,
            layout_indirect,
            layout_direct,
        }
    }
}

impl TerrainPipeline {
    pub fn terrain_layout(&self, direct: bool) -> BindGroupLayoutDescriptor {
        if direct {
            self.layout_direct.clone()
        } else {
            self.layout_indirect.clone()
        }
    }

    fn terrain_defs(key: TerrainPipelineKey) -> Vec<ShaderDefVal> {
        let mut defs = Vec::new();
        if key.fancy {
            defs.push("FANCY_SHADERS".into());
        }
        if key.specular {
            defs.push("SUN_SPECULAR".into());
        }
        if key.stream == STREAM_CUTOUT {
            defs.push("ALPHA_CUTOUT".into());
        }
        if key.stream == STREAM_WATER {
            defs.push("WATER".into());
        }
        if key.direct {
            defs.push("META_TEXTURE".into());
        }
        defs
    }

    fn specialize_main(&self, key: TerrainPipelineKey) -> RenderPipelineDescriptor {
        let mut view_key =
            key.view | MeshPipelineKey::from_primitive_topology(PrimitiveTopology::TriangleList);
        if key.stream == STREAM_WATER {
            view_key |= MeshPipelineKey::BLEND_ALPHA;
        }
        let mut descriptor =
            <MeshPipeline as bevy::render::render_resource::SpecializedMeshPipeline>::specialize(
                &self.mesh_pipeline,
                view_key,
                &self.vertex_layout_ref,
            )
            .expect("terrain vertex layout is missing ATTRIBUTE_POSITION");

        descriptor.label = Some(
            match key.stream {
                STREAM_SOLID => "terrain_solid",
                STREAM_CUTOUT => "terrain_cutout",
                _ => "terrain_water",
            }
            .into(),
        );
        descriptor.vertex.shader = self.shader.clone();
        descriptor.vertex.buffers = vec![packed_vertex_layout()];
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader = self.shader.clone();
        }

        descriptor.layout.truncate(2);
        descriptor.layout.push(self.terrain_layout(key.direct));

        descriptor.primitive.cull_mode = Some(Face::Back);
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = true;
        }

        for def in Self::terrain_defs(key) {
            descriptor.vertex.shader_defs.push(def.clone());
            if let Some(fragment) = descriptor.fragment.as_mut() {
                fragment.shader_defs.push(def);
            }
        }

        descriptor
    }

    #[cfg(feature = "builtin_shaders")]
    fn specialize_shadow(&self, key: TerrainPipelineKey) -> RenderPipelineDescriptor {
        let mut shader_defs: Vec<ShaderDefVal> = vec![
            "PREPASS_PIPELINE".into(),
            "DEPTH_PREPASS".into(),
            "VERTEX_OUTPUT_INSTANCE_INDEX".into(),
            "MAY_DISCARD".into(),
        ];
        shader_defs.extend(Self::terrain_defs(key));

        let unclipped = key.view.contains(MeshPipelineKey::UNCLIPPED_DEPTH_ORTHO);
        let emulate = unclipped && !self.prepass.depth_clip_control_supported;
        if emulate {
            shader_defs.push("UNCLIPPED_DEPTH_ORTHO_EMULATION".into());
            shader_defs.push("PREPASS_FRAGMENT".into());
        }

        RenderPipelineDescriptor {
            label: Some(
                if key.stream == STREAM_CUTOUT {
                    "terrain_shadow_cutout"
                } else {
                    "terrain_shadow_solid"
                }
                .into(),
            ),
            layout: vec![
                self.prepass.view_layout_no_motion_vectors.clone(),
                self.prepass.empty_layout.clone(),
                self.terrain_layout(key.direct),
            ],
            push_constant_ranges: vec![],
            vertex: VertexState {
                shader: self.shadow_shader.clone(),
                shader_defs: shader_defs.clone(),
                entry_point: None,
                buffers: vec![packed_vertex_layout()],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                cull_mode: Some(Face::Back),
                unclipped_depth: unclipped && self.prepass.depth_clip_control_supported,
                ..default()
            },
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: CompareFunction::GreaterEqual,
                stencil: StencilState {
                    front: StencilFaceState::IGNORE,
                    back: StencilFaceState::IGNORE,
                    read_mask: 0,
                    write_mask: 0,
                },
                bias: super::super::terrain::shadow_depth_bias(),
            }),
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(FragmentState {
                shader: self.shadow_shader.clone(),
                shader_defs,
                entry_point: Some("fragment".into()),
                targets: vec![],
            }),
            zero_initialize_workgroup_memory: false,
        }
    }
}

impl SpecializedRenderPipeline for TerrainPipeline {
    type Key = TerrainPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        #[cfg(feature = "builtin_shaders")]
        if key.shadow {
            return self.specialize_shadow(key);
        }
        self.specialize_main(key)
    }
}

pub struct SetTerrainBindGroup<const I: usize>;

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetTerrainBindGroup<I> {
    type Param = SRes<TerrainBindGroup>;
    type ViewQuery = ();
    type ItemQuery = ();

    fn render<'w>(
        _item: &P,
        _view: (),
        _entity: Option<()>,
        bind_group: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(group) = bind_group.into_inner().group.as_ref() else {
            return RenderCommandResult::Skip;
        };
        pass.set_bind_group(I, group, &[]);
        RenderCommandResult::Success
    }
}

pub struct DrawTerrain;

impl<P: TerrainItem> RenderCommand<P> for DrawTerrain {
    type Param = (
        SRes<TerrainTier>,
        SRes<TerrainPools>,
        SRes<CullBuffers>,
        SRes<TerrainViews>,
        SRes<DirectLists>,
    );
    type ViewQuery = Read<ExtractedView>;
    type ItemQuery = ();

    fn render<'w>(
        item: &P,
        view: &'w ExtractedView,
        _entity: Option<()>,
        (tier, pools, cull, views, lists): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let (pool_index, stream) = item.pool_stream();
        let sources = StreamSources {
            tier: tier.into_inner(),
            pools: pools.into_inner(),
            cull: cull.into_inner(),
            views: views.into_inner(),
            lists: lists.into_inner(),
        };
        match draw_region(
            pass,
            &sources,
            view.retained_view_entity,
            pool_index,
            stream,
        ) {
            Ok(()) => RenderCommandResult::Success,
            Err(DrawSkipped::NoPool) => RenderCommandResult::Skip,
            Err(DrawSkipped::NoCullSlot) => {
                RenderCommandResult::Failure("terrain view has no cull slot")
            }
        }
    }
}

pub struct StreamSources<'w> {
    pub tier: &'w TerrainTier,
    pub pools: &'w TerrainPools,
    pub cull: &'w CullBuffers,
    pub views: &'w TerrainViews,
    pub lists: &'w DirectLists,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawSkipped {
    NoPool,
    NoCullSlot,
}

pub fn draw_region<'w>(
    pass: &mut TrackedRenderPass<'w>,
    sources: &StreamSources<'w>,
    view: RetainedViewEntity,
    pool_index: u32,
    stream: u32,
) -> Result<(), DrawSkipped> {
    let StreamSources {
        tier,
        pools,
        cull,
        views,
        lists,
    } = *sources;
    let Some(pool) = pools.pools.get(pool_index as usize) else {
        return Err(DrawSkipped::NoPool);
    };
    pass.set_vertex_buffer(0, pool.vertices.slice(..));
    pass.set_index_buffer(pool.indices.slice(..), IndexFormat::Uint32);

    if tier.indirect {
        let Some(&view_index) = views.index.get(&view) else {
            return Err(DrawSkipped::NoCullSlot);
        };
        let region = ((view_index * cull.pool_count) + pool_index) * STREAMS + stream;
        let region_commands = cull.region_commands();
        let commands_offset = region as u64 * region_commands * INDIRECT_ARGS_SIZE;
        if stream == STREAM_WATER {
            let count = cull.water_count(view_index, pool_index);
            if count > 0 {
                pass.multi_draw_indexed_indirect(&cull.commands, commands_offset, count);
            }
        } else if tier.count {
            pass.multi_draw_indexed_indirect_count(
                &cull.commands,
                commands_offset,
                &cull.counts,
                region as u64 * 4,
                region_commands as u32,
            );
        } else {
            let draw_end = pools.draw_end.min(cull.slot_cap);
            if draw_end > 0 {
                pass.multi_draw_indexed_indirect(
                    &cull.commands,
                    commands_offset,
                    draw_end * FACE_RUNS,
                );
            }
        }
        return Ok(());
    }

    let Some(slots) = lists
        .0
        .get(&view)
        .and_then(|per_view| per_view.get(stream as usize))
    else {
        return Ok(());
    };
    let draws = draw_slots(pass, pools, slots, pool_index, stream, false);
    if draws > 0 {
        pools
            .stats
            .0
            .draws_pending
            .fetch_add(draws, Ordering::Relaxed);
    }
    Ok(())
}

pub fn draw_slots(
    pass: &mut TrackedRenderPass<'_>,
    pools: &TerrainPools,
    slots: &[u32],
    pool_index: u32,
    stream: u32,
    instanced: bool,
) -> usize {
    let mut run: Option<(u32, u32)> = None;
    let mut draws = 0usize;

    let pool_of = |slot: u32| pools.meta_cpu[slot as usize].pool;
    let start = slots.partition_point(|&slot| pool_of(slot) < pool_index);
    let end = start + slots[start..].partition_point(|&slot| pool_of(slot) == pool_index);

    for &slot in &slots[start..end] {
        let (first, count) = pools.meta_cpu[slot as usize].stream_range(stream);
        if count == 0 {
            continue;
        }
        if instanced {
            pass.draw_indexed(first..first + count, 0, slot..slot + 1);
            draws += 1;
            continue;
        }
        match run {
            Some((start, end)) if end == first => {
                run = Some((start, first + count));
            }
            Some((start, end)) => {
                pass.draw_indexed(start..end, 0, 0..1);
                draws += 1;
                run = Some((first, first + count));
            }
            None => {
                run = Some((first, first + count));
            }
        }
    }

    if let Some((start, end)) = run {
        pass.draw_indexed(start..end, 0, 0..1);
        draws += 1;
    }
    draws
}

const INDIRECT_ARGS_SIZE: u64 = 20;

pub type DrawTerrainMain = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshViewBindingArrayBindGroup<1>,
    SetTerrainBindGroup<TERRAIN_GROUP>,
    DrawTerrain,
);

#[cfg(feature = "builtin_shaders")]
pub type DrawTerrainShadow = (
    SetItemPipeline,
    SetPrepassViewBindGroup<0>,
    SetPrepassViewEmptyBindGroup<1>,
    SetTerrainBindGroup<TERRAIN_GROUP>,
    DrawTerrain,
);

#[cfg(feature = "builtin_shaders")]
type ViewLights = Option<&'static ViewLightEntities>;
#[cfg(not(feature = "builtin_shaders"))]
type ViewLights = ();

#[allow(clippy::too_many_arguments)]
pub fn queue_terrain(
    tier: Res<TerrainTier>,
    pools: Res<TerrainPools>,
    terrain_pipeline: Res<TerrainPipeline>,
    pipeline_cache: Res<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<TerrainPipeline>>,
    view_key_cache: Res<ViewKeyCache>,
    opaque_draws: Res<DrawFunctions<Opaque3d>>,
    alpha_draws: Res<DrawFunctions<AlphaMask3d>>,
    transparent_draws: Res<DrawFunctions<Transparent3d>>,
    mut opaque_phases: ResMut<ViewBinnedRenderPhases<Opaque3d>>,
    mut alpha_phases: ResMut<ViewBinnedRenderPhases<AlphaMask3d>>,
    mut transparent_phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    views: Query<(&ExtractedView, Has<TerrainView>, ViewLights)>,
    #[cfg(feature = "builtin_shaders")]
    (light_key_cache, shadow_draws, mut shadow_phases, light_views): (
        Res<LightKeyCache>,
        Res<DrawFunctions<Shadow>>,
        ResMut<ViewBinnedRenderPhases<Shadow>>,
        Query<(&ExtractedView, &LightEntity)>,
    ),
    ticks: SystemChangeTick,
    #[cfg(feature = "shader_support")] takeover: Option<Res<PackTakeover>>,
) {
    if pools.pools.is_empty() {
        return;
    }
    let direct = !tier.indirect;
    let fancy = crate::renderer::terrain::builtin_world_shading();
    let specular = fancy && crate::renderer::terrain::sun_specular_enabled();
    let change_tick = ticks.this_run();
    let transparent_draw = transparent_draws.read().id::<DrawTerrainMain>();
    #[cfg(feature = "builtin_shaders")]
    let shadow_draw = shadow_draws.read().id::<DrawTerrainShadow>();

    let opaque_draw = opaque_draws.read().id::<DrawTerrainMain>();
    let alpha_draw = alpha_draws.read().id::<DrawTerrainMain>();

    #[cfg(feature = "shader_support")]
    if takeover.is_some_and(|t| t.0) {
        return;
    }

    for (view, is_terrain, lights) in views.iter() {
        if !is_terrain {
            continue;
        }
        let Some(&view_key) = view_key_cache.get(&view.retained_view_entity) else {
            continue;
        };

        for pool in 0..pools.pools.len() as u32 {
            let mut specialize = |stream: u32, shadow: bool, key: MeshPipelineKey| {
                pipelines.specialize(
                    &pipeline_cache,
                    &terrain_pipeline,
                    TerrainPipelineKey {
                        view: key,
                        stream,
                        direct,
                        shadow,
                        fancy,
                        specular,
                    },
                )
            };

            if let Some(phase) = opaque_phases.get_mut(&view.retained_view_entity) {
                phase.add(
                    Opaque3dBatchSetKey {
                        pipeline: specialize(STREAM_SOLID, false, view_key),
                        draw_function: opaque_draw,
                        material_bind_group_index: None,
                        vertex_slab: SlabId::default(),
                        index_slab: None,
                        lightmap_slab: None,
                    },
                    Opaque3dBinKey {
                        asset_id: item_asset_id(pool, STREAM_SOLID),
                    },
                    item_entity(pool, STREAM_SOLID),
                    InputUniformIndex::default(),
                    BinnedRenderPhaseType::NonMesh,
                    change_tick,
                );
            }

            if let Some(phase) = alpha_phases.get_mut(&view.retained_view_entity) {
                phase.add(
                    OpaqueNoLightmap3dBatchSetKey {
                        pipeline: specialize(STREAM_CUTOUT, false, view_key),
                        draw_function: alpha_draw,
                        material_bind_group_index: None,
                        vertex_slab: SlabId::default(),
                        index_slab: None,
                    },
                    OpaqueNoLightmap3dBinKey {
                        asset_id: item_asset_id(pool, STREAM_CUTOUT),
                    },
                    item_entity(pool, STREAM_CUTOUT),
                    InputUniformIndex::default(),
                    BinnedRenderPhaseType::NonMesh,
                    change_tick,
                );
            }

            if let Some(phase) = transparent_phases.get_mut(&view.retained_view_entity) {
                phase.add(Transparent3d {
                    distance: f32::MIN,
                    pipeline: specialize(STREAM_WATER, false, view_key),
                    entity: item_entity(pool, STREAM_WATER),
                    draw_function: transparent_draw,
                    batch_range: 0..1,
                    extra_index: PhaseItemExtraIndex::DynamicOffset(pool * STREAMS + STREAM_WATER),
                    indexed: true,
                });
            }
        }

        #[cfg(feature = "builtin_shaders")]
        if fancy {
            let Some(lights) = lights else { continue };
            for &light_entity in &lights.lights {
                let Ok((light_view, kind)) = light_views.get(light_entity) else {
                    continue;
                };
                if !matches!(kind, LightEntity::Directional { .. }) {
                    continue;
                }
                let Some(phase) = shadow_phases.get_mut(&light_view.retained_view_entity) else {
                    continue;
                };
                let Some(&light_key) = light_key_cache.get(&light_view.retained_view_entity) else {
                    continue;
                };
                for pool in 0..pools.pools.len() as u32 {
                    for stream in [STREAM_SOLID, STREAM_CUTOUT] {
                        let pipeline = pipelines.specialize(
                            &pipeline_cache,
                            &terrain_pipeline,
                            TerrainPipelineKey {
                                view: light_key,
                                stream,
                                direct,
                                shadow: true,
                                fancy,
                                specular: false,
                            },
                        );
                        phase.add(
                            ShadowBatchSetKey {
                                pipeline,
                                draw_function: shadow_draw,
                                material_bind_group_index: None,
                                vertex_slab: SlabId::default(),
                                index_slab: None,
                            },
                            ShadowBinKey {
                                asset_id: item_asset_id(pool, stream),
                            },
                            item_entity(pool, stream),
                            InputUniformIndex::default(),
                            BinnedRenderPhaseType::NonMesh,
                            change_tick,
                        );
                    }
                }
            }
        }
        #[cfg(not(feature = "builtin_shaders"))]
        let _ = lights;
    }
}

pub fn prepare_params(
    queue: Res<RenderQueue>,
    buffer: Res<TerrainParamsBuffer>,
    params: Option<Res<TerrainParams>>,
    origin: Res<crate::renderer::RenderOrigin>,
    fog: Query<&DistanceFog, With<TerrainView>>,
) {
    let params = params.as_deref().copied().unwrap_or_default();
    let origin = origin.block();
    let (color, ramps) = fog.iter().next().map(fog_values).unwrap_or_default();
    let (wave_phase, wind_plant_phase, wind_leaf_phase) = noise_phases(origin);
    let values = ParamsUniform {
        misc: [params.cutoff, params.mip_bias, params.shadow_mip, 0.0],
        fog_color: color,
        fog_ramps: ramps,
        origin: origin.extend(0).to_array(),
        wave_phase,
        wind_plant_phase,
        wind_leaf_phase,
    };
    queue.write_buffer(&buffer.0, 0, bytemuck::bytes_of(&values));
}

fn fog_values(fog: &DistanceFog) -> ([f32; 4], [f32; 4]) {
    let encode = |c: f32| c.max(0.0).powf(1.0 / 2.2);
    let color = LinearRgba::from(fog.color);
    let environmental = LinearRgba::from(fog.directional_light_color);
    let render = match fog.falloff {
        FogFalloff::Linear { start, end } => (start, end),
        FogFalloff::Exponential { density } | FogFalloff::ExponentialSquared { density } => {
            (density, 0.0)
        }
        FogFalloff::Atmospheric { extinction, .. } => (extinction.x, extinction.y),
    };
    let environmental = fog_ramp(environmental.red, environmental.green);
    let render = fog_ramp(render.0, render.1);
    (
        [
            encode(color.red),
            encode(color.green),
            encode(color.blue),
            color.alpha,
        ],
        [environmental.0, environmental.1, render.0, render.1],
    )
}

fn fog_ramp(start: f32, end: f32) -> (f32, f32) {
    let slope = 1.0 / (end - start);
    if end > start && slope.is_finite() {
        (start, slope)
    } else {
        (start, f32::MAX)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_terrain_bind_group(
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<TerrainPipeline>,
    pools: Res<TerrainPools>,
    textures: Option<Res<TerrainTextures>>,
    images: Res<RenderAssets<GpuImage>>,
    params: Res<TerrainParamsBuffer>,
    mut bind_group: ResMut<TerrainBindGroup>,
) {
    let Some(textures) = textures else { return };
    let (Some(atlas), Some(lightmap)) =
        (images.get(&textures.atlas), images.get(&textures.lightmap))
    else {
        return;
    };

    let key = (
        atlas.texture_view.id(),
        lightmap.texture_view.id(),
        pools.tables_generation,
    );
    if bind_group.key == Some(key) && bind_group.group.is_some() {
        return;
    }

    let group = match &pools.table {
        SlotTable::Indirect { meta, .. } => device.create_bind_group(
            "terrain",
            &pipeline_cache.get_bind_group_layout(&pipeline.layout_indirect),
            &BindGroupEntries::sequential((
                &atlas.texture_view,
                &atlas.sampler,
                &lightmap.texture_view,
                &lightmap.sampler,
                params.0.as_entire_binding(),
                meta.as_entire_binding(),
            )),
        ),
        SlotTable::Direct { origins_view, .. } => device.create_bind_group(
            "terrain",
            &pipeline_cache.get_bind_group_layout(&pipeline.layout_direct),
            &BindGroupEntries::sequential((
                &atlas.texture_view,
                &atlas.sampler,
                &lightmap.texture_view,
                &lightmap.sampler,
                params.0.as_entire_binding(),
                origins_view,
            )),
        ),
    };
    bind_group.group = Some(group);
    bind_group.key = Some(key);
}

pub(crate) fn init_terrain_pipeline(world: &mut World) {
    let pipeline = TerrainPipeline::from_world(world);
    world.insert_resource(pipeline);
}

pub fn build(app: &mut App) {
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app
        .init_resource::<SpecializedRenderPipelines<TerrainPipeline>>()
        .init_resource::<TerrainBindGroup>()
        .init_resource::<PackTakeover>()
        .add_render_command::<Opaque3d, DrawTerrainMain>()
        .add_render_command::<AlphaMask3d, DrawTerrainMain>()
        .add_render_command::<Transparent3d, DrawTerrainMain>();
    render_app.add_systems(
        Render,
        (
            prepare_params.in_set(RenderSystems::PrepareResources),
            prepare_terrain_bind_group.in_set(RenderSystems::PrepareBindGroups),
            queue_terrain.in_set(RenderSystems::Queue),
        ),
    );

    #[cfg(feature = "builtin_shaders")]
    render_app
        .add_render_command::<Shadow, DrawTerrainShadow>()
        .add_systems(
            RenderStartup,
            init_terrain_pipeline.after(bevy::pbr::init_prepass_pipeline),
        );
    #[cfg(not(feature = "builtin_shaders"))]
    render_app.add_systems(RenderStartup, init_terrain_pipeline);
}

pub fn finish(app: &mut App) {
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    let device = render_app.world().resource::<RenderDevice>().clone();
    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some("terrain_params"),
        size: PARAMS_SIZE,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    render_app.insert_resource(TerrainParamsBuffer(buffer));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_declaration_matches_the_uniform() {
        let source = include_str!("../terrain_common.wgsl");
        let start = source.find("struct TerrainParams {").expect("declared");
        let end = start + source[start..].find('}').expect("closed");
        let declaration = source[start..=end].split_whitespace().collect::<String>();
        assert_eq!(
            declaration.matches("vec4<").count() * 16,
            size_of::<ParamsUniform>()
        );
        assert!(declaration.contains("origin:vec4<i32>"));
    }

    #[test]
    fn noise_phases_restore_the_absolute_argument() {
        let origin = IVec3::new(29_999_984, 0, -1_234_560);
        let (wave, plant, _) = noise_phases(origin);
        let (rx, rz) = (3.25f64, -7.5f64);
        let (ax, az) = (f64::from(origin.x) + rx, f64::from(origin.z) + rz);
        let same = |relative: f64, phase: f32, absolute: f64| {
            let d = (relative + f64::from(phase) - absolute).rem_euclid(std::f64::consts::TAU);
            d.min(std::f64::consts::TAU - d) < 1e-5
        };
        assert!(same(0.55 * rx, wave[0], 0.55 * ax));
        assert!(same(0.29 * (rx + rz), wave[2], 0.29 * (ax + az)));
        assert!(same(
            0.42 * (1.7 * rx - 1.1 * rz),
            plant[1],
            0.42 * (1.7 * ax - 1.1 * az)
        ));
    }

    #[test]
    fn the_shaders_use_the_coefficients_the_phases_assume() {
        let main = include_str!("../terrain.wgsl");
        let common = include_str!("../terrain_common.wgsl");
        for term in [
            "sin(xz.x * 0.55 + ph.x",
            "sin(xz.y * 0.42 + ph.y",
            "sin((xz.x + xz.y) * 0.29 + ph.z",
            "cos(xz.x * 0.55 + ph.x",
            "cos(xz.y * 0.42 + ph.y",
            "cos((xz.x + xz.y) * 0.29 + ph.z",
        ] {
            assert!(main.contains(term), "terrain.wgsl lost `{term}`");
        }
        let freqs = [
            format!("WIND_FREQ_PLANT: f32 = {WIND_FREQ_PLANT};"),
            format!("WIND_FREQ_LEAF: f32 = {WIND_FREQ_LEAF};"),
        ];
        for term in [
            "sin(p.x + p.z + phase.x",
            "sin(p.x * 1.7 - p.z * 1.1 + phase.y",
            "cos(p.z * 0.9 - p.x * 0.4 + phase.z",
        ] {
            assert!(common.contains(term), "terrain_common.wgsl lost `{term}`");
        }
        for freq in &freqs {
            assert!(
                common.contains(freq.as_str()),
                "terrain_common.wgsl lost `{freq}`"
            );
        }
    }

    #[test]
    fn the_world_origin_adds_no_phase() {
        let (wave, plant, leaf) = noise_phases(IVec3::ZERO);
        assert_eq!([wave, plant, leaf], [[0.0; 4]; 3]);
    }
}
