use bevy::core_pipeline::core_3d::Transparent3d;
use bevy::ecs::system::SystemParamItem;
use bevy::ecs::system::lifetimeless::SRes;
use bevy::mesh::{
    MeshVertexBufferLayout, MeshVertexBufferLayoutRef, MeshVertexBufferLayouts, VertexBufferLayout,
};
use bevy::pbr::{
    MeshPipeline, MeshPipelineKey, SetMeshViewBindGroup, SetMeshViewBindingArrayBindGroup,
    ViewKeyCache,
};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_phase::{
    AddRenderCommand, DrawFunctions, PhaseItemExtraIndex, RenderCommand, RenderCommandResult,
    SetItemPipeline, TrackedRenderPass, ViewSortedRenderPhases,
};
use bevy::render::render_resource::binding_types::{sampler, texture_2d};
use bevy::render::render_resource::{
    BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, IndexFormat,
    PipelineCache, PrimitiveTopology, RenderPipelineDescriptor, SamplerBindingType, ShaderStages,
    SpecializedRenderPipeline, SpecializedRenderPipelines, TextureSampleType, TextureViewId,
    VertexAttribute, VertexFormat, VertexStepMode,
};
use bevy::render::renderer::RenderDevice;
use bevy::render::texture::GpuImage;
use bevy::render::view::ExtractedView;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::{Shader, ShaderDefVal};

use super::pool::{GUI_VERTEX_STRIDE, GuiPool, GuiTextures, GuiView, item_entity};

const GUI_GROUP: usize = 2;

fn gui_vertex_layout() -> VertexBufferLayout {
    VertexBufferLayout {
        array_stride: GUI_VERTEX_STRIDE,
        step_mode: VertexStepMode::Vertex,
        attributes: vec![
            VertexAttribute {
                format: VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            VertexAttribute {
                format: VertexFormat::Float32x2,
                offset: 12,
                shader_location: 1,
            },
            VertexAttribute {
                format: VertexFormat::Float32x4,
                offset: 20,
                shader_location: 2,
            },
        ],
    }
}

#[derive(Resource, Default)]
pub struct GuiBindGroup {
    pub group: Option<BindGroup>,
    key: Option<(TextureViewId, TextureViewId)>,
}

#[derive(Resource)]
pub struct GuiPipeline {
    mesh_pipeline: MeshPipeline,
    shader: Handle<Shader>,
    vertex_layout_ref: MeshVertexBufferLayoutRef,
    layout: BindGroupLayoutDescriptor,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GuiPipelineKey {
    pub view: MeshPipelineKey,
}

fn gui_layout() -> BindGroupLayoutDescriptor {
    let entries = BindGroupLayoutEntries::sequential(
        ShaderStages::FRAGMENT,
        (
            texture_2d(TextureSampleType::Float { filterable: true }),
            sampler(SamplerBindingType::Filtering),
            texture_2d(TextureSampleType::Float { filterable: true }),
            sampler(SamplerBindingType::Filtering),
        ),
    );
    BindGroupLayoutDescriptor::new("gui", &entries)
}

impl FromWorld for GuiPipeline {
    fn from_world(world: &mut World) -> Self {
        let shader = world.resource::<GuiShader>().0.clone();
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
            shader,
            vertex_layout_ref,
            layout: gui_layout(),
        }
    }
}

impl SpecializedRenderPipeline for GuiPipeline {
    type Key = GuiPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let view_key = key.view
            | MeshPipelineKey::from_primitive_topology(PrimitiveTopology::TriangleList)
            | MeshPipelineKey::BLEND_ALPHA;
        let mut descriptor =
            <MeshPipeline as bevy::render::render_resource::SpecializedMeshPipeline>::specialize(
                &self.mesh_pipeline,
                view_key,
                &self.vertex_layout_ref,
            )
            .expect("gui vertex layout is missing ATTRIBUTE_POSITION");

        let group = ShaderDefVal::UInt("MATERIAL_BIND_GROUP".into(), GUI_GROUP as u32);
        descriptor.label = Some("gui".into());
        descriptor.vertex.shader = self.shader.clone();
        descriptor.vertex.buffers = vec![gui_vertex_layout()];
        descriptor.vertex.shader_defs.push(group.clone());
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader = self.shader.clone();
            fragment.shader_defs.push(group);
        }

        descriptor.layout.truncate(GUI_GROUP);
        descriptor.layout.push(self.layout.clone());
        descriptor.primitive.cull_mode = None;
        descriptor
    }
}

pub struct DrawGuiRange;

impl RenderCommand<Transparent3d> for DrawGuiRange {
    type Param = (SRes<GuiPool>, SRes<GuiBindGroup>);
    type ViewQuery = ();
    type ItemQuery = ();

    fn render<'w>(
        item: &Transparent3d,
        _view: (),
        _entity: Option<()>,
        (pool, bind_group): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let pool = pool.into_inner();
        let (Some(vertices), Some(indices)) = (&pool.vertices, &pool.indices) else {
            return RenderCommandResult::Skip;
        };
        let Some(group) = bind_group.into_inner().group.as_ref() else {
            return RenderCommandResult::Skip;
        };
        let PhaseItemExtraIndex::DynamicOffset(index) = item.extra_index else {
            return RenderCommandResult::Skip;
        };
        let Some(range) = pool.ranges.get(index as usize) else {
            return RenderCommandResult::Skip;
        };

        pass.set_bind_group(GUI_GROUP, group, &[]);
        pass.set_vertex_buffer(0, vertices.slice(..));
        pass.set_index_buffer(indices.slice(..), IndexFormat::Uint32);
        pass.draw_indexed(
            range.first_index..range.first_index + range.index_count,
            range.base_vertex,
            0..1,
        );
        RenderCommandResult::Success
    }
}

pub type DrawGui = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshViewBindingArrayBindGroup<1>,
    DrawGuiRange,
);

pub fn prepare_gui_bind_group(
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<GuiPipeline>,
    textures: Option<Res<GuiTextures>>,
    images: Res<RenderAssets<GpuImage>>,
    mut bind_group: ResMut<GuiBindGroup>,
) {
    let Some(textures) = textures else { return };
    let (Some(atlas), Some(unihex)) = (images.get(&textures.atlas), images.get(&textures.unihex))
    else {
        return;
    };

    let key = (atlas.texture_view.id(), unihex.texture_view.id());
    if bind_group.key == Some(key) && bind_group.group.is_some() {
        return;
    }

    let layout = pipeline_cache.get_bind_group_layout(&pipeline.layout);
    bind_group.group = Some(device.create_bind_group(
        "gui",
        &layout,
        &BindGroupEntries::sequential((
            &atlas.texture_view,
            &atlas.sampler,
            &unihex.texture_view,
            &unihex.sampler,
        )),
    ));
    bind_group.key = Some(key);
}

pub fn queue_gui(
    pool: Res<GuiPool>,
    gui_pipeline: Res<GuiPipeline>,
    pipeline_cache: Res<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<GuiPipeline>>,
    view_key_cache: Res<ViewKeyCache>,
    draw_functions: Res<DrawFunctions<Transparent3d>>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    views: Query<(&ExtractedView, &GuiView)>,
) {
    if pool.ranges.is_empty() {
        return;
    }
    let draw_function = draw_functions.read().id::<DrawGui>();

    for (view, gui_view) in views.iter() {
        let Some(&view_key) = view_key_cache.get(&view.retained_view_entity) else {
            continue;
        };
        let Some(phase) = phases.get_mut(&view.retained_view_entity) else {
            continue;
        };
        for (index, range) in pool.ranges.iter().enumerate() {
            if range.view != gui_view.0 || range.index_count == 0 {
                continue;
            }
            phase.add(Transparent3d {
                distance: f32::MIN,
                pipeline: pipelines.specialize(
                    &pipeline_cache,
                    &gui_pipeline,
                    GuiPipelineKey { view: view_key },
                ),
                entity: item_entity(index as u32),
                draw_function,
                batch_range: 0..1,
                extra_index: PhaseItemExtraIndex::DynamicOffset(index as u32),
                indexed: true,
            });
        }
    }
}

#[derive(Resource, Clone)]
pub struct GuiShader(pub Handle<Shader>);

fn init_gui_pipeline(world: &mut World) {
    let pipeline = GuiPipeline::from_world(world);
    world.insert_resource(pipeline);
}

pub fn build(app: &mut App) {
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app
        .init_resource::<GuiPool>()
        .init_resource::<GuiBindGroup>()
        .init_resource::<SpecializedRenderPipelines<GuiPipeline>>()
        .add_render_command::<Transparent3d, DrawGui>()
        .add_systems(RenderStartup, init_gui_pipeline)
        .add_systems(bevy::render::ExtractSchedule, crate::gui::pool::extract_gui)
        .add_systems(
            Render,
            (
                prepare_gui_bind_group.in_set(RenderSystems::PrepareBindGroups),
                queue_gui.in_set(RenderSystems::Queue),
            ),
        );
}

pub fn finish(app: &mut App, shader: Handle<Shader>) {
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app.insert_resource(GuiShader(shader));
}
