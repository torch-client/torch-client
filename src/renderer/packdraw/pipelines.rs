use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use bevy::render::render_resource::{
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendComponent,
    BlendFactor, BlendOperation, BlendState, BufferBindingType, ColorTargetState, ColorWrites,
    CompareFunction, ComputePipeline, DepthStencilState, Face, FrontFace, MultisampleState,
    PrimitiveState, PrimitiveTopology, RenderPipeline, SamplerBindingType, ShaderStages,
    TextureFormat, TextureSampleType, TextureViewDimension,
};
use bevy::render::renderer::RenderDevice;
use bevy::render::view::ViewTarget;
use bevy::tasks::Task;

use super::compile::{EntityKind, ProgramKind};
use super::entities::{entity_instance_layout, entity_vertex_layout};
use super::formats::{image_format, wgpu_format};
use super::frame::{PackRender, Stage};
use super::fullscreen::{opaque_target, stores_display};
use super::install::{Installed, Module};
use super::sources::TextureSource;
use super::targets::DEPTH_FORMAT;
use super::usage::depth_index;
use super::{GPU_LABEL, pipeline_label};
use crate::renderer::terrain_pool::TerrainView;
use crate::renderer::terrain_pool::draw::{PackTakeover, TerrainPipeline, packed_vertex_layout};
use crate::shaderpack::directives::SampleKind;
use crate::shaderpack::pipeline::Binding;
use crate::shaderpack::programs::Geometry;
use crate::shaderpack::transform::ImageAccess;

fn checked_shaders() -> bool {
    static CHECKED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *CHECKED.get_or_init(|| std::env::var_os("MC_SHADERPACK_CHECKED").is_some_and(|v| v != "0"))
}

pub(super) struct Pipelines {
    pub(super) geometry: [Option<RenderPipeline>; 3],
    pub(super) screen: Vec<Option<RenderPipeline>>,
    pub(super) shadow: Option<(RenderPipeline, RenderPipeline)>,
    pub(super) entities: [Option<(RenderPipeline, RenderPipeline)>; EntityKind::COUNT],
    pub(super) compute: Vec<Option<[ComputePipeline; 2]>>,
}

pub(super) enum Building<T> {
    Task(Task<T>),
    Ready(T),
}

impl<T> Building<T> {
    fn spawn(build: impl FnOnce() -> T + Send + 'static) -> Building<T>
    where
        T: Send + 'static,
    {
        Building::Task(bevy::tasks::AsyncComputeTaskPool::get().spawn(async move { build() }))
    }

    fn poll(&mut self) {
        if let Building::Task(task) = self
            && let Some(value) = bevy::tasks::futures::check_ready(task)
        {
            *self = Building::Ready(value);
        }
    }

    pub(super) fn ready(&self) -> Option<&T> {
        match self {
            Building::Ready(value) => Some(value),
            Building::Task(_) => None,
        }
    }
}

pub(super) fn make_layout(
    device: &RenderDevice,
    descriptor: &BindGroupLayoutDescriptor,
) -> BindGroupLayout {
    device.create_bind_group_layout(descriptor.label.as_ref(), &descriptor.entries)
}

fn binding_kind(installed: &Installed, source: TextureSource, filtered: [bool; 2]) -> SampleKind {
    match source {
        TextureSource::ShadowDepth(depth) if filtered[depth_index(depth)] => SampleKind::Float,
        TextureSource::Image { index, .. } => installed.custom_images[usize::from(index)]
            .format
            .sample_kind(),
        _ => source.sample_kind(&installed.targets),
    }
}

fn storage_visibility(kind: ProgramKind, vertex: bool) -> ShaderStages {
    match (kind, vertex) {
        (ProgramKind::Compute, _) => ShaderStages::COMPUTE,
        (_, true) => ShaderStages::VERTEX_FRAGMENT,
        (_, false) => ShaderStages::FRAGMENT,
    }
}

pub(super) fn program_layout(
    installed: &Installed,
    index: usize,
    filtered: [bool; 2],
) -> BindGroupLayoutDescriptor {
    let program = &installed.programs[index];
    let stages = if program.kind == ProgramKind::Compute {
        ShaderStages::COMPUTE
    } else {
        ShaderStages::VERTEX_FRAGMENT
    };
    let mut entries = Vec::with_capacity(program.layout.bindings.len());
    for binding in &program.layout.bindings {
        match binding {
            Binding::Uniform { binding, .. } => entries.push(BindGroupLayoutEntry {
                binding: *binding,
                visibility: stages,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }),
            Binding::Texture { binding, .. } => {
                let source = program.texture_source(*binding);
                let kind = binding_kind(installed, source, filtered);
                entries.push(BindGroupLayoutEntry {
                    binding: *binding,
                    visibility: stages,
                    ty: BindingType::Texture {
                        sample_type: match kind {
                            SampleKind::Float => TextureSampleType::Float { filterable: true },
                            SampleKind::UnfilterableFloat => {
                                TextureSampleType::Float { filterable: false }
                            }
                            SampleKind::Uint => TextureSampleType::Uint,
                            SampleKind::Sint => TextureSampleType::Sint,
                        },
                        view_dimension: source.view_dimension(),
                        multisampled: false,
                    },
                    count: None,
                });
            }
            Binding::Image {
                binding,
                ty,
                vertex,
                access,
                name,
            } => {
                let Some(image) = installed.custom_images.iter().find(|i| i.name == *name) else {
                    continue;
                };
                entries.push(BindGroupLayoutEntry {
                    binding: *binding,
                    visibility: storage_visibility(program.kind, *vertex),
                    ty: BindingType::StorageTexture {
                        access: match access {
                            ImageAccess::Write => {
                                bevy::render::render_resource::StorageTextureAccess::WriteOnly
                            }
                            ImageAccess::Read => {
                                bevy::render::render_resource::StorageTextureAccess::ReadOnly
                            }
                            ImageAccess::ReadWrite => {
                                bevy::render::render_resource::StorageTextureAccess::ReadWrite
                            }
                        },
                        format: image_format(image.format),
                        view_dimension: match crate::shaderpack::transform::untyped(ty) {
                            "image1D" => TextureViewDimension::D1,
                            "image3D" => TextureViewDimension::D3,
                            _ => TextureViewDimension::D2,
                        },
                    },
                    count: None,
                });
            }
            Binding::Buffer {
                binding,
                read_only,
                vertex,
                ..
            } => entries.push(BindGroupLayoutEntry {
                binding: *binding,
                visibility: storage_visibility(program.kind, *vertex),
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage {
                        read_only: *read_only,
                    },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }),
            Binding::Sampler { binding, .. } => {
                let kind = binding_kind(installed, program.texture_source(binding - 1), filtered);
                entries.push(BindGroupLayoutEntry {
                    binding: *binding,
                    visibility: stages,
                    ty: BindingType::Sampler(if kind == SampleKind::Float {
                        SamplerBindingType::Filtering
                    } else {
                        SamplerBindingType::NonFiltering
                    }),
                    count: None,
                });
            }
        }
    }
    BindGroupLayoutDescriptor::new(format!("{GPU_LABEL} {}", program.name), &entries)
}

struct PackPipeline<'a> {
    label: String,
    layouts: Vec<&'a BindGroupLayout>,
    vertex: &'a Module,
    buffers: Vec<bevy::mesh::VertexBufferLayout>,
    primitive: PrimitiveState,
    depth: Option<(bool, CompareFunction)>,
    fragment: &'a Module,
    targets: Vec<Option<ColorTargetState>>,
}

struct Builder<'a> {
    device: &'a RenderDevice,
    modules: HashMap<*const Module, wgpu::ShaderModule>,
}

impl<'a> Builder<'a> {
    fn new(device: &'a RenderDevice) -> Builder<'a> {
        Builder {
            device,
            modules: HashMap::new(),
        }
    }

    fn module(&mut self, module: &Module) -> wgpu::ShaderModule {
        let device = self.device;
        self.modules
            .entry(std::ptr::from_ref(module))
            .or_insert_with(|| {
                let descriptor = wgpu::ShaderModuleDescriptor {
                    label: Some(&module.label),
                    source: wgpu::ShaderSource::Wgsl(module.wgsl.as_str().into()),
                };
                if checked_shaders() {
                    device.create_and_validate_shader_module(descriptor)
                } else {
                    unsafe { device.create_shader_module(descriptor) }
                }
            })
            .clone()
    }

    fn layout(&self, label: &str, layouts: &[&BindGroupLayout]) -> wgpu::PipelineLayout {
        let layouts: Vec<&wgpu::BindGroupLayout> = layouts.iter().map(|l| &***l).collect();
        self.device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &layouts,
                push_constant_ranges: &[],
            })
    }

    fn render(&mut self, desc: PackPipeline) -> RenderPipeline {
        let vertex = self.module(desc.vertex);
        let fragment = self.module(desc.fragment);
        let layout = self.layout(&desc.label, &desc.layouts);
        let buffers: Vec<wgpu::VertexBufferLayout> = desc
            .buffers
            .iter()
            .map(|b| wgpu::VertexBufferLayout {
                array_stride: b.array_stride,
                step_mode: b.step_mode,
                attributes: &b.attributes,
            })
            .collect();
        let options = wgpu::PipelineCompilationOptions {
            zero_initialize_workgroup_memory: false,
            ..default()
        };
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&desc.label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &vertex,
                    entry_point: Some("main"),
                    compilation_options: options.clone(),
                    buffers: &buffers,
                },
                primitive: desc.primitive,
                depth_stencil: desc.depth.map(|(write, compare)| DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: write,
                    depth_compare: compare,
                    stencil: default(),
                    bias: default(),
                }),
                multisample: MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &fragment,
                    entry_point: Some("main"),
                    compilation_options: options,
                    targets: &desc.targets,
                }),
                multiview: None,
                cache: None,
            })
    }

    fn compute(
        &mut self,
        label: String,
        layout: &BindGroupLayout,
        module: &Module,
    ) -> ComputePipeline {
        let module = self.module(module);
        let pipeline_layout = self.layout(&label, &[layout]);
        self.device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(&label),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions {
                    zero_initialize_workgroup_memory: false,
                    ..default()
                },
                cache: None,
            })
    }
}

fn pack_triangles(topology: PrimitiveTopology, cull: Option<Face>) -> PrimitiveState {
    PrimitiveState {
        topology,
        front_face: FrontFace::Cw,
        cull_mode: cull,
        ..default()
    }
}

struct EntityPassState {
    blend: Option<BlendState>,
    depth: Option<(bool, CompareFunction)>,
    cull: Option<Face>,
}

fn entity_pass_state(kind: EntityKind, second: bool) -> EntityPassState {
    let cutout = EntityPassState {
        blend: None,
        depth: Some((true, CompareFunction::Less)),
        cull: None,
    };
    let blended = EntityPassState {
        blend: Some(BlendState::ALPHA_BLENDING),
        depth: Some((false, CompareFunction::Less)),
        cull: None,
    };
    match kind {
        EntityKind::Entity | EntityKind::Block | EntityKind::Particle | EntityKind::Hand => {
            if second {
                blended
            } else {
                cutout
            }
        }
        EntityKind::HandWater
        | EntityKind::EntityTranslucent
        | EntityKind::BlockTranslucent
        | EntityKind::ParticleTranslucent => blended,
        EntityKind::SkyBasic | EntityKind::SkyTextured => EntityPassState {
            blend: Some(if second {
                BlendState {
                    color: BlendComponent {
                        src_factor: BlendFactor::SrcAlpha,
                        dst_factor: BlendFactor::One,
                        operation: BlendOperation::Add,
                    },
                    alpha: BlendComponent::OVER,
                }
            } else {
                BlendState::ALPHA_BLENDING
            }),
            depth: None,
            cull: None,
        },
        EntityKind::Clouds => EntityPassState {
            blend: Some(BlendState::ALPHA_BLENDING),
            depth: Some((true, CompareFunction::Less)),
            cull: (!second).then_some(Face::Back),
        },
        EntityKind::ShadowCaster => EntityPassState {
            blend: None,
            depth: Some((true, CompareFunction::Less)),
            cull: None,
        },
        EntityKind::DamagedBlock => EntityPassState {
            blend: Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::Dst,
                    dst_factor: BlendFactor::Src,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent::REPLACE,
            }),
            depth: Some((false, CompareFunction::LessEqual)),
            cull: None,
        },
        EntityKind::Line => EntityPassState {
            blend: Some(BlendState::ALPHA_BLENDING),
            depth: Some((false, CompareFunction::LessEqual)),
            cull: None,
        },
        EntityKind::SpiderEyes => EntityPassState {
            blend: Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::SrcAlpha,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::Zero,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
            }),
            depth: Some((false, CompareFunction::LessEqual)),
            cull: None,
        },
    }
}

fn entity_topology(kind: EntityKind) -> PrimitiveTopology {
    match kind {
        EntityKind::Line => PrimitiveTopology::LineList,
        _ => PrimitiveTopology::TriangleList,
    }
}

fn shadow_color_targets(installed: &Installed, buffers: &[u8]) -> Vec<Option<ColorTargetState>> {
    buffers
        .iter()
        .map(|buffer| {
            opaque_target(wgpu_format(
                installed.targets.shadow[*buffer as usize].format,
            ))
        })
        .collect()
}

fn color_targets(
    installed: &Installed,
    buffers: &[u8],
    blend: bool,
) -> Vec<Option<ColorTargetState>> {
    blended_targets(
        installed,
        buffers,
        blend.then_some(BlendState::ALPHA_BLENDING),
    )
}

fn blended_targets(
    installed: &Installed,
    buffers: &[u8],
    blend: Option<BlendState>,
) -> Vec<Option<ColorTargetState>> {
    buffers
        .iter()
        .map(|buffer| {
            let format = installed.targets.settings[*buffer as usize].format;
            let blend = blend.filter(|_| format.sample_kind() == SampleKind::Float);
            Some(ColorTargetState {
                format: wgpu_format(format),
                blend,
                write_mask: ColorWrites::ALL,
            })
        })
        .collect()
}

pub(crate) fn prepare_pipelines(
    device: Res<RenderDevice>,
    pools: Option<Res<crate::renderer::terrain_pool::pools::TerrainPools>>,
    terrain: Option<Res<TerrainPipeline>>,
    takeover: Option<ResMut<PackTakeover>>,
    render: Option<ResMut<PackRender>>,
) {
    let (Some(terrain), Some(mut takeover), Some(mut render)) = (terrain, takeover, render) else {
        return;
    };
    let Some(install) = render.install.as_deref_mut() else {
        if takeover.0 {
            takeover.0 = false;
        }
        return;
    };

    if matches!(install.stage, Stage::Queued) {
        let Some(pools) = pools else { return };
        let terrain_layout =
            make_layout(&device, &terrain.terrain_layout(install.installed.direct));
        let quads_layout = pools.quads_layout.clone();
        let device = device.clone();
        let installed = Arc::clone(&install.installed);
        let layouts = install.gpu.layouts.clone();
        let shadow = install.usage.shadow;
        install.stage = Stage::Started {
            pipelines: Building::spawn(move || {
                build_pipelines(
                    &device,
                    &installed,
                    &layouts,
                    &terrain_layout,
                    &quads_layout,
                    shadow,
                )
            }),
            sized: None,
        };
    }
    if let Stage::Started { pipelines, .. } = &mut install.stage {
        pipelines.poll();
    }
}

fn build_pipelines(
    device: &RenderDevice,
    installed: &Installed,
    layouts: &[BindGroupLayout],
    terrain_layout: &BindGroupLayout,
    quads_layout: &BindGroupLayout,
    shadow: bool,
) -> Pipelines {
    let generation = installed.generation;
    let mut builder = Builder::new(device);
    let geometry = std::array::from_fn(|slot| {
        let index = installed.geometry[slot]?;
        let program = &installed.programs[index];
        let geometry = Geometry::ALL[slot];
        Some(builder.render(PackPipeline {
            label: pipeline_label(generation, format_args!("{} {geometry:?}", program.name)),
            layouts: vec![&layouts[index], terrain_layout, quads_layout],
            vertex: &program.vertex,
            buffers: vec![packed_vertex_layout()],
            primitive: pack_triangles(PrimitiveTopology::TriangleList, Some(Face::Back)),
            depth: Some((true, CompareFunction::Less)),
            fragment: program.fragment_for(super::compile::geometry_alpha(geometry, false)),
            targets: color_targets(installed, &program.draw_buffers, geometry.translucent()),
        }))
    });
    let screen = (0..installed.programs.len())
        .map(|index| {
            let program = &installed.programs[index];
            if program.kind != ProgramKind::Screen || Some(index) == installed.final_pass {
                return None;
            }
            Some(builder.render(PackPipeline {
                label: pipeline_label(generation, format_args!("{}", program.name)),
                layouts: vec![&layouts[index]],
                vertex: &program.vertex,
                buffers: vec![],
                primitive: PrimitiveState::default(),
                depth: None,
                fragment: &program.fragment,
                targets: color_targets(installed, &program.draw_buffers, false),
            }))
        })
        .collect();
    let shadow_pipelines = installed.shadow.filter(|_| shadow).map(|index| {
        let program = &installed.programs[index];
        let mut variant = |geometry: Geometry, cutout: bool| {
            builder.render(PackPipeline {
                label: pipeline_label(
                    generation,
                    format_args!("{}{}", program.name, if cutout { " cutout" } else { "" }),
                ),
                layouts: vec![&layouts[index], terrain_layout, quads_layout],
                vertex: &program.vertex,
                buffers: vec![packed_vertex_layout()],
                primitive: PrimitiveState::default(),
                depth: Some((true, CompareFunction::Less)),
                fragment: program.fragment_for(super::compile::geometry_alpha(geometry, true)),
                targets: shadow_color_targets(installed, &program.draw_buffers),
            })
        };
        (
            variant(Geometry::TerrainSolid, false),
            variant(Geometry::TerrainCutout, true),
        )
    });
    let entities = std::array::from_fn(|at| {
        let kind = EntityKind::ALL[at];
        if kind == EntityKind::ShadowCaster && !shadow {
            return None;
        }
        let index = installed.entities[at]?;
        let program = &installed.programs[index];
        let mut variant = |second: bool| {
            let state = entity_pass_state(kind, second);
            builder.render(PackPipeline {
                label: pipeline_label(
                    generation,
                    format_args!("{} {kind:?} {}", program.name, u8::from(second)),
                ),
                layouts: vec![&layouts[index]],
                vertex: &program.vertex,
                buffers: vec![entity_vertex_layout(), entity_instance_layout()],
                primitive: pack_triangles(entity_topology(kind), state.cull),
                depth: state.depth,
                fragment: program.fragment_for(kind.alpha_tests()[usize::from(second)]),
                targets: if kind == EntityKind::ShadowCaster {
                    shadow_color_targets(installed, &program.draw_buffers)
                } else {
                    blended_targets(installed, &program.draw_buffers, state.blend)
                },
            })
        };
        Some((variant(false), variant(true)))
    });
    let compute = installed
        .programs
        .iter()
        .enumerate()
        .map(|(index, program)| {
            let mut made = |module: &Module| {
                builder.compute(
                    pipeline_label(generation, format_args!("{}", program.name)),
                    &layouts[index],
                    module,
                )
            };
            match &program.compute_parity {
                Some([(even, _), (odd, _)]) => Some([made(even), made(odd)]),
                None => {
                    let only = made(program.compute.as_ref()?);
                    Some([only.clone(), only])
                }
            }
        })
        .collect();
    Pipelines {
        geometry,
        screen,
        shadow: shadow_pipelines,
        entities,
        compute,
    }
}

fn final_pipeline(
    device: &RenderDevice,
    installed: &Installed,
    index: usize,
    layout: &BindGroupLayout,
    format: TextureFormat,
) -> RenderPipeline {
    let program = &installed.programs[index];
    let fragment = match (&program.fragment_linear, stores_display(format)) {
        (Some(linear), false) => linear,
        _ => &program.fragment,
    };
    Builder::new(device).render(PackPipeline {
        label: pipeline_label(
            installed.generation,
            format_args!("{} {format:?}", program.name),
        ),
        layouts: vec![layout],
        vertex: &program.vertex,
        buffers: vec![],
        primitive: PrimitiveState::default(),
        depth: None,
        fragment,
        targets: vec![opaque_target(format)],
    })
}

pub(crate) fn prepare_blit(
    device: Res<RenderDevice>,
    views: Query<&ViewTarget, With<TerrainView>>,
    render: Option<ResMut<PackRender>>,
) {
    let Some(mut render) = render else { return };
    let render = &mut *render;
    let (Some(install), Some(process)) = (render.install.as_deref_mut(), render.process.as_mut())
    else {
        return;
    };
    for target in &views {
        let format = target.main_texture_format();
        if let Some(index) = install.installed.final_pass
            && !install.finals.iter().any(|(f, _)| *f == format)
        {
            let device = device.clone();
            let installed = Arc::clone(&install.installed);
            let layout = install.gpu.layouts[index].clone();
            let building = Building::spawn(move || {
                final_pipeline(&device, &installed, index, &layout, format)
            });
            install.finals.push((format, building));
        }
        if process.blit.pipeline(format).is_none() {
            process.blit.add(&device, format);
        }
    }
    for (_, building) in &mut install.finals {
        building.poll();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_kinds_draw_as_iris_draws_them() {
        let state = |kind, second| entity_pass_state(kind, second);
        let sky = state(EntityKind::SkyBasic, false);
        assert!(sky.depth.is_none() && sky.blend.is_some());
        assert_eq!(EntityKind::SkyTextured.alpha_tests(), [None; 2]);
        let stars = state(EntityKind::SkyBasic, true).blend.expect("blends");
        assert_eq!(stars.color.dst_factor, BlendFactor::One, "additive");
        let clouds = state(EntityKind::Clouds, false);
        assert_eq!(clouds.depth, Some((true, CompareFunction::Less)));
        assert_eq!(clouds.cull, Some(Face::Back));
        assert_eq!(
            state(EntityKind::Clouds, true).cull,
            None,
            "fast clouds draw both faces"
        );
        let hand = state(EntityKind::Hand, false);
        assert!(hand.depth == Some((true, CompareFunction::Less)));
        assert_eq!(
            EntityKind::Hand.alpha_tests()[0],
            Some(super::super::compile::ONE_TENTH_ALPHA)
        );
        let caster = state(EntityKind::ShadowCaster, false);
        assert!(
            caster.blend.is_none()
                && caster.cull.is_none()
                && caster.depth == Some((true, CompareFunction::Less))
        );
        assert_eq!(
            EntityKind::ShadowCaster.alpha_tests()[0],
            Some(super::super::compile::ONE_TENTH_ALPHA)
        );
        let hand_water = state(EntityKind::HandWater, false);
        assert!(
            hand_water.blend.is_some() && hand_water.depth == Some((false, CompareFunction::Less))
        );
    }
}
