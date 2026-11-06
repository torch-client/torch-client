use bevy::asset::embedded_asset;
use bevy::camera::{ClearColor, NormalizedRenderTarget};
use bevy::color::LinearRgba;
use bevy::mesh::VertexBufferLayout;
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::render::camera::{ExtractedCamera, SortedCameras};
use bevy::render::diagnostic::RecordDiagnostics;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_graph::{
    Node, NodeRunError, RenderGraph, RenderGraphContext, RenderLabel,
};
use std::num::NonZeroU64;

use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer_sized};
use bevy::render::render_resource::{
    BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
    BlendFactor as F, BlendState, CachedRenderPipelineId, ColorTargetState, ColorWrites,
    FragmentState, IndexFormat, LoadOp, Operations, PipelineCache, RenderPassColorAttachment,
    RenderPassDescriptor, RenderPipelineDescriptor, Sampler, SamplerBindingType, ShaderStages,
    SpecializedRenderPipeline, SpecializedRenderPipelines, StoreOp, TextureFormat,
    TextureSampleType, TextureView, TextureViewId, VertexAttribute, VertexFormat, VertexState,
    VertexStepMode,
};
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::texture::{GpuImage, OutputColorAttachment};
use bevy::render::view::{ExtractedWindows, ViewTargetAttachments};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::{Shader, ShaderDefVal};
use bevy::window::WindowRef;

use super::pool::{GLINT_UNIFORM_SIZE, GUI_VERTEX_STRIDE, GuiPool, GuiTextures};
use crate::renderer::ssaa::SceneImage;

pub const UNIHEX_UV_BIAS: f32 = 2.0;

pub const GLINT_UV_BIAS: f32 = 2.0;

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
pub struct GuiImageViews(pub std::collections::HashMap<u64, (TextureView, Sampler)>);

#[derive(Resource, Default)]
pub(crate) struct GuiImageGroups(std::collections::HashMap<u64, (TextureViewId, BindGroup)>);

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
    Vignette,
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
            uniform_buffer_sized(false, NonZeroU64::new(GLINT_UNIFORM_SIZE)),
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
        let glint_def = || {
            vec![ShaderDefVal::UInt(
                "GLINT_ROTATION_BITS".into(),
                crate::items::glint::ROTATION.to_bits(),
            )]
        };
        let (label, shader, layout, vertex_entry, fragment_entry, buffers, blend, defs) =
            match key.kind {
                GuiPipelineKind::Gui => (
                    "gui",
                    &self.shaders.gui,
                    self.gui_layout.clone(),
                    "vertex",
                    "fragment",
                    vec![gui_vertex_layout()],
                    Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    glint_def(),
                ),
                GuiPipelineKind::Vignette => (
                    "gui_vignette",
                    &self.shaders.gui,
                    self.gui_layout.clone(),
                    "vertex",
                    "vignette_fragment",
                    vec![gui_vertex_layout()],
                    Some(crate::renderer::blend(
                        F::Zero,
                        F::OneMinusSrc,
                        F::Zero,
                        F::One,
                    )),
                    glint_def(),
                ),
                GuiPipelineKind::Blit => (
                    "gui_scene_blit",
                    &self.shaders.blit,
                    self.blit_layout.clone(),
                    "blit_vertex",
                    "blit_fragment",
                    Vec::new(),
                    None,
                    Vec::new(),
                ),
            };
        RenderPipelineDescriptor {
            label: Some(label.into()),
            layout: vec![layout],
            vertex: VertexState {
                shader: shader.clone(),
                shader_defs: defs.clone(),
                entry_point: Some(vertex_entry.into()),
                buffers,
            },
            fragment: Some(FragmentState {
                shader: shader.clone(),
                shader_defs: defs,
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
    vignette: Option<CachedRenderPipelineId>,
}

#[derive(Resource, Default)]
struct GuiPlan(Vec<WindowPass>);

fn prepare_gui_bind_group(
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<GuiPipeline>,
    textures: Option<Res<GuiTextures>>,
    images: Res<RenderAssets<GpuImage>>,
    pool: Res<GuiPool>,
    mut bind_group: ResMut<GuiBindGroup>,
) {
    let Some(textures) = textures else { return };
    let (Some(atlas), Some(unihex)) = (images.get(&textures.atlas), images.get(&textures.unihex))
    else {
        return;
    };
    let Some(glint) = &pool.glint else { return };

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
            glint.as_entire_binding(),
        )),
    ));
    bind_group.key = Some(key);
}

#[allow(clippy::too_many_arguments, reason = "one system, flat arguments")]
pub(crate) fn prepare_gui_image_groups(
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<GuiPipeline>,
    textures: Option<Res<GuiTextures>>,
    images: Res<RenderAssets<GpuImage>>,
    pool: Res<GuiPool>,
    views: Res<GuiImageViews>,
    mut groups: ResMut<GuiImageGroups>,
) {
    groups.0.retain(|key, _| views.0.contains_key(key));
    if views.0.is_empty() {
        return;
    }
    let Some(unihex) = textures.as_ref().and_then(|t| images.get(&t.unihex)) else {
        return;
    };
    let Some(glint) = &pool.glint else { return };
    let layout = pipeline_cache.get_bind_group_layout(&pipeline.gui_layout);
    for (&key, (view, sampler)) in &views.0 {
        if groups.0.get(&key).is_some_and(|(id, _)| *id == view.id()) {
            continue;
        }
        let group = device.create_bind_group(
            "gui_image",
            &layout,
            &BindGroupEntries::sequential((
                view,
                sampler,
                &unihex.texture_view,
                &unihex.sampler,
                glint.as_entire_binding(),
            )),
        );
        groups.0.insert(key, (view.id(), group));
    }
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
        let vignette = pool
            .ranges
            .iter()
            .any(|r| r.window == window && r.texture == super::painter::VIGNETTE_TEXTURE)
            .then(|| specialize(GuiPipelineKind::Vignette))
            .flatten();

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
            vignette,
        });
    }
}

struct GuiCameraDriverNode {
    cameras: QueryState<&'static ExtractedCamera>,
}

impl Node for GuiCameraDriverNode {
    fn update(&mut self, world: &mut World) {
        self.cameras.update_archetypes(world);
    }

    fn run(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let sorted = world.resource::<SortedCameras>();
        let windows = world.resource::<ExtractedWindows>();
        let mut camera_windows = HashSet::<Entity>::default();
        for sorted_camera in &sorted.0 {
            let Ok(camera) = self.cameras.get_manual(world, sorted_camera.entity) else {
                continue;
            };
            if let Some(NormalizedRenderTarget::Window(window_ref)) = camera.target {
                let window = window_ref.entity();
                if !windows
                    .get(&window)
                    .is_some_and(|w| w.physical_width > 0 && w.physical_height > 0)
                {
                    continue;
                }
                camera_windows.insert(window);
            }
            graph.run_sub_graph(
                camera.render_graph,
                vec![],
                Some(sorted_camera.entity),
                Some(format!(
                    "Camera {} ({})",
                    sorted_camera.order, sorted_camera.entity
                )),
            )?;
        }

        let plan = world.resource::<GuiPlan>();
        let clear = world.resource::<ClearColor>();
        for (id, window) in windows.iter() {
            if camera_windows.contains(id) || plan.0.iter().any(|pass| pass.window == *id) {
                continue;
            }
            let Some(view) = &window.swap_chain_texture_view else {
                continue;
            };
            render_context
                .command_encoder()
                .begin_render_pass(&RenderPassDescriptor {
                    label: Some("no_camera_clear_pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Clear(clear.to_linear().into()),
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
        }
        Ok(())
    }
}

#[derive(RenderLabel, Debug, Hash, PartialEq, Eq, Clone)]
pub(crate) struct GuiLabel;

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
        let image_groups = world.resource::<GuiImageGroups>();
        let blit_group = world
            .resource::<SceneBlitBindGroup>()
            .0
            .as_ref()
            .map(|(_, group)| group);

        let recorder = render_context.diagnostic_recorder();
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
            let span = recorder.pass_span(&mut render_pass, "gui");

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
                let atlas = crate::gui::painter::ATLAS_TEXTURE;
                let mut bound = atlas;
                let vignette = pass
                    .vignette
                    .and_then(|id| pipeline_cache.get_render_pipeline(id));
                for range in &pool.ranges {
                    if range.window != pass.window || range.index_count == 0 {
                        continue;
                    }
                    if range.texture == crate::gui::painter::VIGNETTE_TEXTURE {
                        if let Some(vignette) = vignette {
                            if bound != atlas {
                                render_pass.set_bind_group(0, group, &[]);
                                bound = atlas;
                            }
                            render_pass.set_render_pipeline(vignette);
                            render_pass.draw_indexed(
                                range.first_index..range.first_index + range.index_count,
                                range.base_vertex,
                                0..1,
                            );
                            render_pass.set_render_pipeline(pipeline);
                        }
                        continue;
                    }
                    if range.texture != bound {
                        let next = if range.texture == atlas {
                            Some(group)
                        } else {
                            image_groups.0.get(&range.texture).map(|(_, g)| g)
                        };
                        let Some(next) = next else { continue };
                        render_pass.set_bind_group(0, next, &[]);
                        bound = range.texture;
                    }
                    render_pass.draw_indexed(
                        range.first_index..range.first_index + range.index_count,
                        range.base_vertex,
                        0..1,
                    );
                }
            }
            span.end(&mut render_pass);
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
        .init_resource::<GuiImageViews>()
        .init_resource::<GuiImageGroups>()
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
                prepare_gui_image_groups.in_set(RenderSystems::PrepareBindGroups),
                present_gui_windows
                    .in_set(RenderSystems::Render)
                    .after(bevy::render::renderer::render_system),
            ),
        );

    let driver = GuiCameraDriverNode {
        cameras: render_app.world_mut().query(),
    };
    let mut graph = render_app.world_mut().resource_mut::<RenderGraph>();
    graph
        .remove_node(bevy::render::graph::CameraDriverLabel)
        .expect("CameraPlugin adds the camera driver before the gui plugin builds");
    graph.add_node(bevy::render::graph::CameraDriverLabel, driver);
    graph.add_node(GuiLabel, GuiNode);
    graph.add_node_edge(bevy::render::graph::CameraDriverLabel, GuiLabel);
}

pub fn finish(app: &mut App) {
    let server = app.world().resource::<AssetServer>();
    let shaders = GuiShaders {
        gui: server.load(crate::renderer::embedded_path("gui/gui.wgsl")),
        blit: server.load(crate::renderer::embedded_path("gui/scene_blit.wgsl")),
    };
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app.insert_resource(shaders);
}
