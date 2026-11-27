use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use bevy::prelude::*;
use bevy::render::Extract;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{
    AddressMode, BindGroupLayout, Buffer, BufferDescriptor, BufferUsages, FilterMode,
    RenderPipeline, Sampler, TextureFormat, TextureUsages,
};
use bevy::render::renderer::{RenderAdapter, RenderDevice, RenderQueue};
use bevy::render::texture::GpuImage;
use bevy::render::view::{ExtractedView, ViewTarget};

use super::super::frame::PackRender;
use super::super::fullscreen::{opaque_target, pipeline_layout, screen_pipeline};
use super::super::inputs::Inputs;
use super::super::pipelines::{Building, make_layout};
use super::super::targets::{
    DEPTH_FORMAT, Target, depth_pair, extent, fill, noise_target, sampler,
};
use super::super::uniforms::{FrameValues, History};
use super::super::{GPU_ERROR, GPU_FAILED, GPU_LABEL, pipeline_label, set_current_generation};
use super::bind::pass_layout;
use super::cascade;
use super::geometry::{Geometry, GroupLayouts, GroupPipelines, group_pipelines};
use super::install::{NativeInstalled, NativePack};
use super::overlay::{OverlayPipelines, overlay_pipelines};
use super::plans::{self, PlanViews, WorldPlans};
use super::shadow::{InstallShadow, ShadowLayouts, ShadowPipelines, shadow_pipelines};
use super::uniform::{self, FrameHistory};
use crate::renderer::terrain_pool::cull::PackCullView;
use crate::renderer::terrain_pool::draw::{PackTakeover, TerrainPipeline};
use crate::renderer::terrain_pool::pools::TerrainPools;
use crate::renderer::terrain_pool::{TerrainTextures, TerrainView};
use crate::shaderpack::directives::Constants;
use crate::shaderpack::native::api::{
    self, OutputSize, Resource as Global, ScreenOutput, TerrainTier as SeamTier,
};
use crate::shaderpack::native::build::{Built, BuiltPass};
use crate::shaderpack::native::caps::{self, Format};

#[derive(Resource, Default)]
pub(crate) struct NativeRender {
    failed: u32,
    pub(super) install: Option<Box<Install>>,
}

pub(super) struct Install {
    pub(super) installed: Arc<NativeInstalled>,
    formats: Vec<TextureFormat>,
    pub(super) layouts: Vec<BindGroupLayout>,
    pub(super) uniform: Buffer,
    pub(super) samplers: Samplers,
    pub(super) textures: InstallTextures,
    pub(super) pipelines: Building<Pipelines>,
    finals: Vec<(TextureFormat, Building<Finals>)>,
    pub(super) sized: Option<Sized>,
    pub(super) inputs: Inputs,
    far: f32,
    history: History,
    frame_history: FrameHistory,
    constants: Constants,
    pub(super) geometry: Option<Geometry>,
    pub(super) shadow: InstallShadow,
}

impl Install {
    pub(super) fn draws_world(&self) -> bool {
        self.geometry.is_some()
    }
}

pub(super) struct Samplers {
    pub(super) linear_clamp: Sampler,
    pub(super) linear_repeat: Sampler,
    pub(super) nearest_clamp: Sampler,
}

pub(super) struct InstallTextures {
    pub(super) noise: Option<Target>,
    pub(super) white: Option<Target>,
}

pub(super) struct Pipelines {
    modules: Vec<wgpu::ShaderModule>,
    layouts: Vec<wgpu::PipelineLayout>,
    pub(super) passes: Vec<Option<RenderPipeline>>,
    pub(super) groups: Vec<GroupPipelines>,
    pub(super) shadow: Option<ShadowPipelines>,
}

pub(super) struct Finals {
    last: RenderPipeline,
    pub(super) overlays: Option<OverlayPipelines>,
}

pub(super) struct Sized {
    pub(super) size: UVec2,
    pub(super) targets: Vec<Vec<Target>>,
    pub(super) front: AtomicU32,
    pub(super) cleared: AtomicBool,
    pub(super) world: Option<WorldSized>,
}

pub(super) struct WorldSized {
    pub(super) depth: Target,
    pub(super) depth_copy: Option<Target>,
    pub(super) plans: Option<WorldPlans>,
}

pub(super) struct Ready<'a> {
    pub(super) install: &'a Install,
    pub(super) pipelines: &'a Pipelines,
    pub(super) last: &'a RenderPipeline,
    pub(super) sized: &'a Sized,
}

impl NativeRender {
    pub(super) fn ready(&self, format: TextureFormat) -> Option<Ready<'_>> {
        let install = self.install.as_deref()?;
        let pipelines = install.pipelines.ready()?;
        let last = &install
            .finals
            .iter()
            .find(|(f, _)| *f == format)?
            .1
            .ready()?
            .last;
        let sized = install.sized.as_ref()?;
        if install.draws_world()
            && sized
                .world
                .as_ref()
                .and_then(|w| w.plans.as_ref())
                .is_none_or(|plans| plans.format != format)
        {
            return None;
        }
        Some(Ready {
            install,
            pipelines,
            last,
            sized,
        })
    }

    pub(super) fn blocked(&self, format: TextureFormat) -> &'static str {
        let Some(install) = self.install.as_deref() else {
            return "";
        };
        if install.pipelines.ready().is_none() {
            return "the pipelines are still building";
        }
        match install.finals.iter().find(|(f, _)| *f == format) {
            None => "no pipeline has been started for this view's format",
            Some((_, building)) if building.ready().is_none() => {
                "the view-writing pipeline is still building"
            }
            Some(_) if install.sized.is_none() => "the targets have not been made",
            Some(_) if install.draws_world() => {
                "the world plans have not been made (the atlas or the lightmap is missing)"
            }
            Some(_) => "",
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn extract_native(
    main: Extract<Option<Res<NativePack>>>,
    environment: Extract<Option<Res<crate::renderer::environment::Environment>>>,
    inputs: Extract<Option<Res<Inputs>>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    adapter: Res<RenderAdapter>,
    terrain: Option<Res<TerrainPipeline>>,
    pools: Option<Res<TerrainPools>>,
    glsl: Option<Res<PackRender>>,
    mut render: ResMut<NativeRender>,
) {
    let render = &mut *render;
    let failed = render.failed;
    let wanted = main
        .as_deref()
        .map(|p| &p.installed)
        .filter(|i| i.generation != failed);
    let held = render.install.as_ref().map(|i| i.installed.generation);
    if held != wanted.map(|i| i.generation) {
        render.install = None;
        if let Some(installed) = wanted {
            let layouts = match (terrain.as_deref(), pools.as_deref()) {
                (Some(terrain), Some(pools)) => {
                    let direct = installed.built.terrain == SeamTier::Direct;
                    Some((
                        make_layout(&device, &terrain.terrain_layout(direct)),
                        pools.quads_layout.clone(),
                    ))
                }
                _ => None,
            };
            if layouts.is_some() || !installed.geometry() {
                let gl = adapter.get_info().backend == wgpu::Backend::Gl;
                match install(&device, &queue, gl, Arc::clone(installed), layouts) {
                    Ok(install) => render.install = Some(Box::new(install)),
                    Err(e) => {
                        crate::log_warn!("shaders", "{} cannot be drawn here: {e}", installed.name);
                        crate::shaderpack::FAILED.set(vec![installed.name.clone()]);
                        render.failed = installed.generation;
                        super::super::install::reject(installed.generation);
                    }
                }
            }
        }
    }
    set_current_generation(match render.install.as_deref() {
        Some(install) => install.installed.generation,
        None => glsl.map_or(0, |r| r.generation()),
    });
    let Some(install) = render.install.as_deref_mut() else {
        return;
    };
    if let Some(environment) = environment.as_deref() {
        install.far = (environment.render_distance * 16) as f32;
    }
    if let Some(inputs) = inputs.as_deref() {
        install.inputs = *inputs;
    }
}

pub(super) fn texture_format(format: Format) -> Option<TextureFormat> {
    Some(match format {
        Format::R8Unorm => TextureFormat::R8Unorm,
        Format::Rg8Unorm => TextureFormat::Rg8Unorm,
        Format::Rgba8Unorm => TextureFormat::Rgba8Unorm,
        Format::Rgb10a2Unorm => TextureFormat::Rgb10a2Unorm,
        Format::Rg11b10Ufloat => TextureFormat::Rg11b10Ufloat,
        Format::R16Float => TextureFormat::R16Float,
        Format::Rg16Float => TextureFormat::Rg16Float,
        Format::Rgba16Float => TextureFormat::Rgba16Float,
        Format::R32Float => TextureFormat::R32Float,
        Format::Rg32Float => TextureFormat::Rg32Float,
        Format::Rgba32Float => TextureFormat::Rgba32Float,
        Format::R32Uint => TextureFormat::R32Uint,
        Format::Rg32Uint => TextureFormat::Rg32Uint,
        Format::Rgba32Uint => TextureFormat::Rgba32Uint,
        Format::Depth => return None,
    })
}

const TARGET_USAGES: TextureUsages =
    TextureUsages::RENDER_ATTACHMENT.union(TextureUsages::TEXTURE_BINDING);

pub(super) fn renderable_formats(adapter: &RenderAdapter) -> Vec<Format> {
    caps::FORMATS
        .iter()
        .map(|row| row.format)
        .filter(|&format| {
            texture_format(format).is_some_and(|texture| {
                adapter
                    .get_texture_format_features(texture)
                    .allowed_usages
                    .contains(TARGET_USAGES)
            })
        })
        .collect()
}

fn install(
    device: &RenderDevice,
    queue: &RenderQueue,
    gl: bool,
    installed: Arc<NativeInstalled>,
    terrain: Option<(BindGroupLayout, BindGroupLayout)>,
) -> Result<Install, String> {
    let built = &installed.built;
    let mut formats = Vec::with_capacity(built.targets.len());
    for target in &built.targets {
        let format = texture_format(target.format)
            .ok_or_else(|| format!("target `{}` is {}", target.id, target.format.name()))?;
        formats.push(format);
    }
    let max = device.limits().max_color_attachments as usize;
    let mut layouts = Vec::with_capacity(built.passes.len());
    for pass in &built.passes {
        if let ScreenOutput::Targets(writes) = &pass.output
            && writes.len() > max
        {
            return Err(format!(
                "pass `{}` writes {} targets; this device draws into at most {max} at once",
                pass.id,
                writes.len()
            ));
        }
        layouts.push(pass_layout(device, built, pass)?);
    }
    let geometry = if installed.geometry() {
        Some(Geometry::new(device, built)?)
    } else {
        None
    };
    let uniform = device.create_buffer(&BufferDescriptor {
        label: Some(gpu_label!("native frame")),
        size: size_of::<api::Frame>() as u64,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let samplers = Samplers {
        linear_clamp: sampler(device, FilterMode::Linear, AddressMode::ClampToEdge),
        linear_repeat: sampler(device, FilterMode::Linear, AddressMode::Repeat),
        nearest_clamp: sampler(device, FilterMode::Nearest, AddressMode::ClampToEdge),
    };
    let constants = Constants::default();
    let binds = |resource: Global| {
        let passes = built.passes.iter().map(|p| &p.bindings);
        passes
            .chain(built.geometry.iter().map(|g| &g.bindings))
            .flatten()
            .any(|b| b.resource == resource)
    };
    let textures = InstallTextures {
        noise: binds(Global::Noise)
            .then(|| noise_target(device, queue, constants.noise_resolution)),
        white: (geometry.is_some() || binds(Global::EntityTexture))
            .then(|| white_target(device, queue)),
    };
    let shadow = InstallShadow::new(device, built, geometry.is_some(), gl, &binds)?;
    let pipelines = {
        let (device, installed, layouts, formats) = (
            device.clone(),
            Arc::clone(&installed),
            layouts.clone(),
            formats.clone(),
        );
        let groups: Vec<BindGroupLayout> = geometry
            .as_ref()
            .map_or_else(Vec::new, |g| g.layouts.clone());
        let shadow = shadow
            .pass
            .as_ref()
            .map(|p| (p.globals.clone(), p.draw_layout.clone(), p.color_format));
        Building::spawn(move || {
            build_pipelines(
                &device,
                &installed,
                &layouts,
                &formats,
                &groups,
                terrain.as_ref(),
                shadow.as_ref(),
            )
        })
    };
    Ok(Install {
        installed,
        formats,
        layouts,
        uniform,
        samplers,
        textures,
        pipelines,
        finals: Vec::new(),
        sized: None,
        inputs: Inputs::default(),
        far: 0.0,
        history: History::default(),
        frame_history: FrameHistory::default(),
        constants,
        geometry,
        shadow,
    })
}

fn white_target(device: &RenderDevice, queue: &RenderQueue) -> Target {
    let one = extent(1, 1);
    let usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    let white = Target::new(
        device,
        gpu_label!("native white"),
        one,
        TextureFormat::Rgba8Unorm,
        usage,
    );
    fill(queue, &white, one, &[255; 4], 4);
    white
}

fn build_pipelines(
    device: &RenderDevice,
    installed: &NativeInstalled,
    layouts: &[BindGroupLayout],
    formats: &[TextureFormat],
    group_layouts: &[BindGroupLayout],
    terrain: Option<&(BindGroupLayout, BindGroupLayout)>,
    shadow: Option<&(BindGroupLayout, BindGroupLayout, Option<TextureFormat>)>,
) -> Pipelines {
    let generation = installed.generation;
    let mut out = Pipelines {
        modules: Vec::new(),
        layouts: Vec::new(),
        passes: Vec::new(),
        groups: Vec::new(),
        shadow: None,
    };
    for (pass, layout) in installed.built.passes.iter().zip(layouts) {
        let module = device.create_and_validate_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&format!(
                "{GPU_LABEL}/{}/{generation}/{}",
                installed.name, pass.file
            )),
            source: wgpu::ShaderSource::Wgsl(pass.wgsl.as_str().into()),
        });
        let label = pipeline_label(generation, format_args!("native {}", pass.id));
        let pipeline_layout = pipeline_layout(device, &label, layout);
        let pipeline = match &pass.output {
            ScreenOutput::Screen => None,
            ScreenOutput::Targets(writes) => {
                let targets: Vec<_> = writes.iter().map(|&t| opaque_target(formats[t])).collect();
                Some(pass_pipeline(
                    device,
                    &label,
                    &module,
                    &pipeline_layout,
                    pass,
                    &targets,
                ))
            }
        };
        out.modules.push(module);
        out.layouts.push(pipeline_layout);
        out.passes.push(pipeline);
    }
    if let Some((terrain, quads)) = terrain {
        for (index, globals) in group_layouts.iter().enumerate() {
            let layouts = GroupLayouts {
                globals,
                terrain,
                quads,
            };
            out.groups
                .push(group_pipelines(device, installed, index, layouts, formats));
        }
        if let Some((globals, draw, color)) = shadow {
            let layouts = ShadowLayouts {
                globals,
                terrain,
                quads,
                draw,
            };
            out.shadow = shadow_pipelines(device, installed, layouts, *color);
        }
    }
    out
}

fn pass_pipeline(
    device: &RenderDevice,
    label: &str,
    module: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    pass: &BuiltPass,
    targets: &[Option<wgpu::ColorTargetState>],
) -> RenderPipeline {
    let constants: Vec<(&str, f64)> = pass
        .overrides
        .iter()
        .map(|(id, value)| (id.as_str(), *value))
        .collect();
    let entries = [api::SCREEN_VERTEX, api::SCREEN_FRAGMENT];
    RenderPipeline::from(screen_pipeline(
        device, label, module, layout, entries, targets, &constants,
    ))
}

fn target_size(size: OutputSize, view: UVec2) -> UVec2 {
    match size {
        OutputSize::Scale(scale) => (view.as_vec2() * scale).floor().max(Vec2::ONE).as_uvec2(),
        OutputSize::Fixed([width, height]) => UVec2::new(width, height),
    }
}

fn make_sized(
    device: &RenderDevice,
    built: &Built,
    formats: &[TextureFormat],
    view: UVec2,
    geometry: Option<&Geometry>,
) -> Sized {
    let targets = built
        .targets
        .iter()
        .zip(formats)
        .map(|(target, format)| {
            let size = target_size(target.size, view);
            let label = format!("{GPU_LABEL} native {}", target.id);
            let count = if target.double() { 2 } else { 1 };
            (0..count)
                .map(|_| {
                    Target::new(
                        device,
                        &label,
                        extent(size.x, size.y),
                        *format,
                        TARGET_USAGES,
                    )
                })
                .collect()
        })
        .collect();
    let world = geometry.map(|geometry| {
        let size = extent(view.x, view.y);
        let labels = [gpu_label!("native depth"), gpu_label!("native depth copy")];
        let (depth, depth_copy) = if geometry.reads_depth.iter().any(|reads| *reads) {
            let (depth, copy) = depth_pair(device, size, labels);
            (depth, Some(copy))
        } else {
            let usage = TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING;
            (
                Target::new(device, labels[0], size, DEPTH_FORMAT, usage),
                None,
            )
        };
        WorldSized {
            depth,
            depth_copy,
            plans: None,
        }
    });
    Sized {
        size: view,
        targets,
        front: AtomicU32::new(0),
        cleared: AtomicBool::new(false),
        world,
    }
}

pub(crate) fn drop_failed_native(mut render: ResMut<NativeRender>) {
    if !GPU_FAILED.load(Ordering::Acquire) {
        return;
    }
    let Some(install) = render.install.take() else {
        return;
    };
    let installed = &install.installed;
    render.failed = installed.generation;
    super::super::install::reject(installed.generation);
    let message = GPU_ERROR
        .lock()
        .ok()
        .and_then(|mut e| e.take())
        .unwrap_or_default();
    crate::log_warn!(
        "shaders",
        "the GPU rejected {}, drawing without it: {message}",
        installed.name
    );
    crate::shaderpack::FAILED.set(vec![installed.name.clone()]);
    GPU_FAILED.store(false, Ordering::Release);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_native(
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    views: Query<(&ExtractedView, &ViewTarget), With<TerrainView>>,
    time: Res<Time>,
    frames: Res<bevy::diagnostic::FrameCount>,
    origin: Res<crate::renderer::RenderOrigin>,
    textures: Option<Res<TerrainTextures>>,
    images: Res<RenderAssets<GpuImage>>,
    lightmap_pixels: Option<Res<crate::renderer::terrain::LightmapPixels>>,
    takeover: Option<Res<PackTakeover>>,
    mut pack_view: Option<ResMut<PackCullView>>,
    mut render: ResMut<NativeRender>,
) {
    let Some(install) = render.install.as_deref_mut() else {
        return;
    };
    install.pipelines.poll();
    for (_, building) in &mut install.finals {
        building.poll();
    }
    let Some((view, view_target)) = views.iter().next() else {
        return;
    };

    let format = view_target.main_texture_format();
    if let Some(pipelines) = install.pipelines.ready()
        && !install.finals.iter().any(|(f, _)| *f == format)
        && let Some(index) = install
            .installed
            .built
            .passes
            .iter()
            .position(|p| p.output == ScreenOutput::Screen)
    {
        let module = pipelines.modules[index].clone();
        let layout = pipelines.layouts[index].clone();
        let (device, installed) = (device.clone(), Arc::clone(&install.installed));
        let overlay = install.geometry.as_ref().map(|g| g.overlay.clone());
        let building = Building::spawn(move || {
            let pass = &installed.built.passes[index];
            let label = pipeline_label(
                installed.generation,
                format_args!("native {} {format:?}", pass.id),
            );
            Finals {
                last: pass_pipeline(
                    &device,
                    &label,
                    &module,
                    &layout,
                    pass,
                    &[opaque_target(format)],
                ),
                overlays: overlay
                    .map(|overlay| overlay_pipelines(&device, &installed, &overlay, format)),
            }
        });
        install.finals.push((format, building));
    }

    let texture = view_target.main_texture().size();
    let size = UVec2::new(texture.width, texture.height);
    if install.sized.as_ref().is_none_or(|s| s.size != size) {
        install.sized = Some(make_sized(
            &device,
            &install.installed.built,
            &install.formats,
            size,
            install.geometry.as_ref(),
        ));
    }

    let values = FrameValues::of(
        view,
        *origin,
        time.elapsed_secs_wrapped(),
        time.delta_secs(),
        frames.0,
        install.far,
        install.inputs,
        &install.constants,
        &mut install.history,
    );
    let light_floor = uniform::light_floor(lightmap_pixels.as_deref());
    let taken_over = takeover.is_some_and(|t| t.0) && install.draws_world();
    let shadow = match install.shadow.pass.as_ref().filter(|_| taken_over) {
        Some(pass) => pass.frame(
            &install.inputs,
            &values,
            *origin,
            install.far,
            pack_view.as_deref_mut(),
        ),
        None => cascade::none(),
    };
    let frame = uniform::frame(
        &values,
        view,
        *origin,
        &install.inputs,
        size.as_vec2(),
        light_floor,
        shadow,
        &mut install.frame_history,
    );
    queue.write_buffer(&install.uniform, 0, bytemuck::bytes_of(&frame));
    if let Some(geometry) = install.geometry.as_mut() {
        geometry.camera = values.camera();
    }

    let Some(textures) = textures else { return };
    let (Some(atlas), Some(lightmap)) =
        (images.get(&textures.atlas), images.get(&textures.lightmap))
    else {
        return;
    };
    let key = [atlas.texture_view.id(), lightmap.texture_view.id()];
    let overlays = install
        .finals
        .iter()
        .find(|(f, _)| *f == format)
        .and_then(|(_, b)| b.ready())
        .map(|f| f.overlays.clone());
    let made = match (install.pipelines.ready(), install.sized.as_ref(), overlays) {
        (Some(pipelines), Some(sized), Some(Some(overlays)))
            if sized.world.as_ref().is_some_and(|w| {
                w.plans
                    .as_ref()
                    .is_none_or(|p| p.key != key || p.format != format)
            }) =>
        {
            let views = PlanViews {
                atlas: &atlas.texture_view,
                lightmap: &lightmap.texture_view,
                key,
                format,
                overlays: &overlays,
            };
            plans::make(&device, install, pipelines, sized, views)
        }
        _ => return,
    };
    if let Some(world) = install.sized.as_mut().and_then(|s| s.world.as_mut()) {
        world.plans = made;
    }
}
