use bevy::asset::embedded_asset;
use bevy::camera::{ClearColor, NormalizedRenderTarget};
use bevy::color::LinearRgba;
use bevy::mesh::VertexBufferLayout;
use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_graph::{
    Node, NodeRunError, RenderGraph, RenderGraphContext, RenderLabel,
};
use bevy::render::render_resource::binding_types::{sampler, texture_2d};
use bevy::render::render_resource::{
    BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BlendState,
    CachedRenderPipelineId, ColorTargetState, ColorWrites, FragmentState, IndexFormat,
    PipelineCache, RenderPassDescriptor, RenderPipelineDescriptor, SamplerBindingType,
    ShaderStages, SpecializedRenderPipeline, SpecializedRenderPipelines, TextureFormat,
    TextureSampleType, TextureViewId, VertexAttribute, VertexFormat, VertexState, VertexStepMode,
};
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::texture::{GpuImage, OutputColorAttachment};
use bevy::render::view::{ExtractedWindows, ViewTargetAttachments};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;
use bevy::window::WindowRef;

use super::pool::{GUI_VERTEX_STRIDE, GuiPool, GuiTextures};
use crate::renderer::ssaa::SceneImage;

pub const UNIHEX_UV_BIAS: f32 = 2.0;

fn gui_vertex_layout() -> VertexBufferLayout {
    VertexBufferLayout {
        array_stride: GUI_VERTEX_STRIDE,
        step_mode: VertexStepMode::Vertex,
        attributes: vec![
            VertexAttribute {
                format: VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            VertexAttribute {
                format: VertexFormat::Float32x2,
                offset: 8,
                shader_location: 1,
            },
            VertexAttribute {
                format: VertexFormat::Float32x4,
                offset: 16,
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

#[derive(Resource, Default)]
struct SceneBlitBindGroup(Option<(TextureViewId, BindGroup)>);

#[derive(Resource)]
pub struct GuiPipeline {
    shaders: GuiShaders,
    gui_layout: BindGroupLayoutDescriptor,
    blit_layout: BindGroupLayoutDescriptor,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum GuiPipelineKind {
    Gui,
    Blit,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GuiPipelineKey {
    pub kind: GuiPipelineKind,
    pub format: TextureFormat,
}

fn two_texture_layout() -> BindGroupLayoutDescriptor {
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

fn blit_layout() -> BindGroupLayoutDescriptor {
    let entries = BindGroupLayoutEntries::sequential(
        ShaderStages::FRAGMENT,
        (
            texture_2d(TextureSampleType::Float { filterable: true }),
            sampler(SamplerBindingType::Filtering),
        ),
    );
    BindGroupLayoutDescriptor::new("gui_scene_blit", &entries)
}

impl SpecializedRenderPipeline for GuiPipeline {
    type Key = GuiPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let (label, shader, layout, vertex_entry, fragment_entry, buffers, blend) = match key.kind {
            GuiPipelineKind::Gui => (
                "gui",
                &self.shaders.gui,
                self.gui_layout.clone(),
                "vertex",
                "fragment",
                vec![gui_vertex_layout()],
                Some(BlendState::ALPHA_BLENDING),
            ),
            GuiPipelineKind::Blit => (
                "gui_scene_blit",
                &self.shaders.blit,
                self.blit_layout.clone(),
                "blit_vertex",
                "blit_fragment",
                Vec::new(),
                None,
            ),
        };
        RenderPipelineDescriptor {
            label: Some(label.into()),
            layout: vec![layout],
            vertex: VertexState {
                shader: shader.clone(),
                shader_defs: Vec::new(),
                entry_point: Some(vertex_entry.into()),
                buffers,
            },
            fragment: Some(FragmentState {
                shader: shader.clone(),
                shader_defs: Vec::new(),
                entry_point: Some(fragment_entry.into()),
                targets: vec![Some(ColorTargetState {
                    format: key.format,
                    blend,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            ..default()
        }
    }
}

struct WindowPass {
    window: Entity,
    target: NormalizedRenderTarget,
    clear: LinearRgba,
    blit: Option<CachedRenderPipelineId>,
    gui: Option<CachedRenderPipelineId>,
}

#[derive(Resource, Default)]
struct GuiPlan(Vec<WindowPass>);

fn prepare_gui_bind_group(
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

    let layout = pipeline_cache.get_bind_group_layout(&pipeline.gui_layout);
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

#[allow(clippy::too_many_arguments, reason = "one system, flat arguments")]
fn plan_gui_passes(
    mut plan: ResMut<GuiPlan>,
    pool: Res<GuiPool>,
    windows: Res<ExtractedWindows>,
    cameras: Query<&ExtractedCamera>,
    scene: Option<Res<SceneImage>>,
    images: Res<RenderAssets<GpuImage>>,
    clear_color: Res<ClearColor>,
    mut attachments: ResMut<ViewTargetAttachments>,
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    gui_pipeline: Res<GuiPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<GuiPipeline>>,
    mut blit_group: ResMut<SceneBlitBindGroup>,
) {
    plan.0.clear();

    let scene_image = scene
        .as_ref()
        .and_then(|s| s.0.as_ref())
        .filter(|handle| {
            cameras.iter().any(|camera| {
                matches!(&camera.target, Some(NormalizedRenderTarget::Image(t)) if t.handle == **handle)
            })
        })
        .and_then(|handle| images.get(handle));
    if let Some(image) = scene_image {
        let id = image.texture_view.id();
        if blit_group
            .0
            .as_ref()
            .is_none_or(|(cached, _)| *cached != id)
        {
            let layout = pipeline_cache.get_bind_group_layout(&gui_pipeline.blit_layout);
            let group = device.create_bind_group(
                "gui_scene_blit",
                &layout,
                &BindGroupEntries::sequential((&image.texture_view, &image.sampler)),
            );
            blit_group.0 = Some((id, group));
        }
    }

    for (&window, extracted) in windows.iter() {
        let primary = windows.primary == Some(window);
        let wants_blit = primary && scene_image.is_some();
        let has_ranges = pool
            .ranges
            .iter()
            .any(|r| r.window == window && r.index_count > 0);
        if !has_ranges && !wants_blit {
            continue;
        }
        let (Some(view), Some(view_format)) = (
            extracted.swap_chain_texture_view.as_ref(),
            extracted.swap_chain_texture_view_format,
        ) else {
            continue;
        };

        let Some(target) = WindowRef::Entity(window)
            .normalize(None)
            .map(NormalizedRenderTarget::Window)
        else {
            continue;
        };
        let format = attachments
            .entry(target.clone())
            .or_insert_with(|| OutputColorAttachment::new(view.clone(), view_format))
            .view_format;

        let mut specialize = |kind| {
            let id = pipelines.specialize(
                &pipeline_cache,
                &gui_pipeline,
                GuiPipelineKey { kind, format },
            );
            pipeline_cache.get_render_pipeline(id).map(|_| id)
        };
        let blit = if wants_blit {
            match specialize(GuiPipelineKind::Blit) {
                Some(id) => Some(id),
                None => continue,
            }
        } else {
            None
        };
        let gui = if !has_ranges {
            None
        } else {
            match specialize(GuiPipelineKind::Gui) {
                Some(id) => Some(id),
                None => continue,
            }
        };

        plan.0.push(WindowPass {
            window,
            target,
            clear: if primary {
                clear_color.0.to_linear()
            } else {
                LinearRgba::BLACK
            },
            blit,
            gui,
        });
    }
}

#[derive(RenderLabel, Debug, Hash, PartialEq, Eq, Clone)]
struct GuiLabel;

struct GuiNode;

impl Node for GuiNode {
    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let plan = world.resource::<GuiPlan>();
        if plan.0.is_empty() {
            return Ok(());
        }
        let attachments = world.resource::<ViewTargetAttachments>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let pool = world.resource::<GuiPool>();
        let gui_group = world.resource::<GuiBindGroup>().group.as_ref();
        let blit_group = world
            .resource::<SceneBlitBindGroup>()
            .0
            .as_ref()
            .map(|(_, group)| group);

        for pass in &plan.0 {
            let Some(attachment) = attachments.get(&pass.target) else {
                continue;
            };
            let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("gui"),
                color_attachments: &[Some(attachment.get_attachment(Some(pass.clear)))],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let (Some(id), Some(group)) = (pass.blit, blit_group)
                && let Some(pipeline) = pipeline_cache.get_render_pipeline(id)
            {
                render_pass.set_render_pipeline(pipeline);
                render_pass.set_bind_group(0, group, &[]);
                render_pass.draw(0..3, 0..1);
            }

            if let (Some(id), Some(group), Some(vertices), Some(indices)) =
                (pass.gui, gui_group, &pool.vertices, &pool.indices)
                && let Some(pipeline) = pipeline_cache.get_render_pipeline(id)
            {
                render_pass.set_render_pipeline(pipeline);
                render_pass.set_bind_group(0, group, &[]);
                render_pass.set_vertex_buffer(0, vertices.slice(..));
                render_pass.set_index_buffer(indices.slice(..), IndexFormat::Uint32);
                for range in &pool.ranges {
                    if range.window != pass.window || range.index_count == 0 {
                        continue;
                    }
                    render_pass.draw_indexed(
                        range.first_index..range.first_index + range.index_count,
                        range.base_vertex,
                        0..1,
                    );
                }
            }
        }
        Ok(())
    }
}

fn present_gui_windows(plan: Res<GuiPlan>, mut windows: ResMut<ExtractedWindows>) {
    for pass in &plan.0 {
        if let Some(window) = windows.get_mut(&pass.window) {
            window.present();
        }
    }
}

#[derive(Resource, Clone)]
pub struct GuiShaders {
    pub gui: Handle<Shader>,
    pub blit: Handle<Shader>,
}

fn init_gui_pipeline(mut commands: Commands, shaders: Res<GuiShaders>) {
    commands.insert_resource(GuiPipeline {
        shaders: shaders.clone(),
        gui_layout: two_texture_layout(),
        blit_layout: blit_layout(),
    });
}

pub fn build(app: &mut App) {
    embedded_asset!(app, "gui.wgsl");
    embedded_asset!(app, "scene_blit.wgsl");
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app
        .init_resource::<GuiPool>()
        .init_resource::<GuiBindGroup>()
        .init_resource::<SceneBlitBindGroup>()
        .init_resource::<GuiPlan>()
        .init_resource::<SpecializedRenderPipelines<GuiPipeline>>()
        .add_systems(RenderStartup, init_gui_pipeline)
        .add_systems(bevy::render::ExtractSchedule, crate::gui::pool::extract_gui)
        .add_systems(
            Render,
            (
                plan_gui_passes
                    .in_set(RenderSystems::ManageViews)
                    .after(bevy::render::view::prepare_view_targets),
                prepare_gui_bind_group.in_set(RenderSystems::PrepareBindGroups),
                present_gui_windows
                    .in_set(RenderSystems::Render)
                    .after(bevy::render::renderer::render_system),
            ),
        );

    let mut graph = render_app.world_mut().resource_mut::<RenderGraph>();
    graph.add_node(GuiLabel, GuiNode);
    graph.add_node_edge(bevy::render::graph::CameraDriverLabel, GuiLabel);
}

pub fn finish(app: &mut App) {
    let crate_name = module_path!().split(':').next().unwrap_or("torch_client");
    let server = app.world().resource::<AssetServer>();
    let shaders = GuiShaders {
        gui: server.load(format!("embedded://{crate_name}/gui/gui.wgsl")),
        blit: server.load(format!("embedded://{crate_name}/gui/scene_blit.wgsl")),
    };
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app.insert_resource(shaders);
}
