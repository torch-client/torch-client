use std::mem::size_of;
use std::num::NonZeroU64;

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
    MeshPipeline, MeshPipelineKey, SetMeshViewBindGroup, SetMeshViewBindingArrayBindGroup,
    ViewKeyCache,
};
use bevy::prelude::*;
use bevy::render::mesh::allocator::SlabId;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_phase::{
    AddRenderCommand, BinnedRenderPhaseType, DrawFunctions, InputUniformIndex, PhaseItem,
    PhaseItemExtraIndex, RenderCommand, RenderCommandResult, SetItemPipeline, TrackedRenderPass,
    ViewBinnedRenderPhases, ViewSortedRenderPhases,
};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
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
use bevy::render::view::ExtractedView;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::{Shader, ShaderDefVal};

use super::cull::{CullBuffers, DirectLists, TerrainViews};
use super::pools::TerrainPools;
use super::{
    STREAM_CUTOUT, STREAM_SOLID, STREAM_WATER, STREAMS, SlotMeta, TerrainParams, TerrainShaders,
    TerrainTextures, TerrainTier, TerrainView,
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

const TERRAIN_GROUP: usize = 2;

const VERTEX_STRIDE: u64 = 20;

const TERRAIN_ASSET_TAG: u128 = 0x7e_77a1 << 64;

fn packed_vertex_layout() -> VertexBufferLayout {
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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerrainPipelineKey {
    pub view: MeshPipelineKey,
    pub stream: u32,
    pub direct: bool,
    pub shadow: bool,
    pub fancy: bool,
}

fn terrain_layouts() -> (BindGroupLayoutDescriptor, BindGroupLayoutDescriptor) {
    let meta_size = NonZeroU64::new(size_of::<SlotMeta>() as u64);
    let common = (
        texture_2d(TextureSampleType::Float { filterable: true }),
        sampler(SamplerBindingType::Filtering),
        texture_2d(TextureSampleType::Float { filterable: true }),
        sampler(SamplerBindingType::Filtering),
        uniform_buffer::<Vec4>(false),
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
    fn terrain_layout(&self, direct: bool) -> BindGroupLayoutDescriptor {
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
        let pools = pools.into_inner();
        let Some(pool) = pools.pools.get(pool_index as usize) else {
            return RenderCommandResult::Skip;
        };
        pass.set_vertex_buffer(0, pool.vertices.slice(..));
        pass.set_index_buffer(pool.indices.slice(..), IndexFormat::Uint32);

        if tier.indirect {
            let cull = cull.into_inner();
            let Some(&view_index) = views.into_inner().index.get(&view.retained_view_entity) else {
                return RenderCommandResult::Failure("terrain view has no cull slot");
            };
            let region = ((view_index * cull.pool_count) + pool_index) * STREAMS + stream;
            let commands_offset = region as u64 * cull.slot_cap as u64 * INDIRECT_ARGS_SIZE;
            if tier.count {
                pass.multi_draw_indexed_indirect_count(
                    &cull.commands,
                    commands_offset,
                    &cull.counts,
                    region as u64 * 4,
                    cull.slot_cap,
                );
            } else {
                pass.multi_draw_indexed_indirect(&cull.commands, commands_offset, cull.slot_cap);
            }
            return RenderCommandResult::Success;
        }

        let Some(slots) = lists
            .into_inner()
            .0
            .get(&(view.retained_view_entity, stream))
        else {
            return RenderCommandResult::Success;
        };

        let mut run: Option<(u32, u32, i32)> = None;

        for &slot in slots {
            let meta = &pools.meta_cpu[slot as usize];
            if meta.pool != pool_index {
                continue;
            }
            let (first, count) = match stream {
                STREAM_SOLID => (meta.first_index, meta.solid_count),
                STREAM_CUTOUT => (
                    meta.first_index + meta.solid_count,
                    meta.index_count - meta.solid_count,
                ),
                _ => (meta.first_index, meta.index_count),
            };
            if count == 0 {
                continue;
            }

            let base = meta.base_vertex as i32;
            match run {
                Some((start, end, run_base)) if end == first && run_base == base => {
                    run = Some((start, first + count, base));
                }
                Some((start, end, run_base)) => {
                    pass.draw_indexed(start..end, run_base, 0..1);
                    run = Some((first, first + count, base));
                }
                None => {
                    run = Some((first, first + count, base));
                }
            }
        }

        if let Some((start, end, base)) = run {
            pass.draw_indexed(start..end, base, 0..1);
        }

        RenderCommandResult::Success
    }
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
) {
    if pools.pools.is_empty() {
        return;
    }
    let direct = !tier.indirect;
    let fancy = crate::renderer::terrain::builtin_shaders_enabled();
    let change_tick = ticks.this_run();
    let opaque_draw = opaque_draws.read().id::<DrawTerrainMain>();
    let alpha_draw = alpha_draws.read().id::<DrawTerrainMain>();
    let transparent_draw = transparent_draws.read().id::<DrawTerrainMain>();
    #[cfg(feature = "builtin_shaders")]
    let shadow_draw = shadow_draws.read().id::<DrawTerrainShadow>();

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
) {
    let params = params.as_deref().copied().unwrap_or_default();
    let values = [params.cutoff, params.mip_bias, params.shadow_mip, 0.0f32];
    queue.write_buffer(&buffer.0, 0, bytemuck::bytes_of(&values));
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_terrain_bind_group(
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<TerrainPipeline>,
    tier: Res<TerrainTier>,
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

    let layout = pipeline_cache.get_bind_group_layout(if tier.indirect {
        &pipeline.layout_indirect
    } else {
        &pipeline.layout_direct
    });
    let group = if tier.indirect {
        device.create_bind_group(
            "terrain",
            &layout,
            &BindGroupEntries::sequential((
                &atlas.texture_view,
                &atlas.sampler,
                &lightmap.texture_view,
                &lightmap.sampler,
                params.0.as_entire_binding(),
                pools.meta.as_entire_binding(),
            )),
        )
    } else {
        let Some(origins) = pools.origins_view.as_ref() else {
            return;
        };
        device.create_bind_group(
            "terrain",
            &layout,
            &BindGroupEntries::sequential((
                &atlas.texture_view,
                &atlas.sampler,
                &lightmap.texture_view,
                &lightmap.sampler,
                params.0.as_entire_binding(),
                origins,
            )),
        )
    };
    bind_group.group = Some(group);
    bind_group.key = Some(key);
}

fn init_terrain_pipeline(world: &mut World) {
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
        .add_render_command::<Opaque3d, DrawTerrainMain>()
        .add_render_command::<AlphaMask3d, DrawTerrainMain>()
        .add_render_command::<Transparent3d, DrawTerrainMain>()
        .add_systems(
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
        size: 16,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    render_app.insert_resource(TerrainParamsBuffer(buffer));
}
