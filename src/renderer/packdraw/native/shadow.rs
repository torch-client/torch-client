use std::num::NonZeroU64;

use bevy::prelude::*;
use bevy::render::render_resource::{
    AddressMode, BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingType,
    Buffer, BufferBindingType, BufferInitDescriptor, BufferUsages, ColorTargetState, ColorWrites,
    CompareFunction, DepthBiasState, DepthStencilState, Extent3d, FilterMode, FrontFace, LoadOp,
    PrimitiveState, RenderPassDescriptor, RenderPipeline, Sampler, SamplerDescriptor, ShaderStages,
    Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor, TextureViewDimension,
};
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::view::RetainedViewEntity;

use super::super::compile::{EntityKind, Segment};
use super::super::entities::{
    EntityFrame, EntityPasses, entity_instance_layout, entity_vertex_layout,
};
use super::super::inputs::Inputs;
use super::super::node::record::{
    SegmentTargets, color_attachment, depth_attachment, draw_pools, entity_segment,
};
use super::super::pipelines::entity_topology;
use super::super::targets::DEPTH_FORMAT;
use super::super::uniforms::FrameValues;
use super::super::{GPU_LABEL, pipeline_label};
use super::cascade;
use super::geometry::globals_layout;
use super::install::NativeInstalled;
use super::render::texture_format;
use crate::renderer::RenderOrigin;
use crate::renderer::terrain_pool::cull::{
    PackCullView, ShadowCullInput, ShadowCullMode, shadow_cull_shape,
};
use crate::renderer::terrain_pool::draw::{StreamSources, draw_region, packed_vertex_layout};
use crate::renderer::terrain_pool::{STREAM_CUTOUT, STREAM_SOLID, STREAM_WATER};
use crate::shaderpack::native::api::{
    self, Draws, FrameShadow, GLOBALS_GROUP, Resource as Global, SHADOW_DRAW_GROUP, TERRAIN_GROUP,
    TerrainStream,
};
use crate::shaderpack::native::build::{Built, BuiltShadow};

pub(super) const CASCADES: usize = 1;
const _: () = assert!(CASCADES >= 1 && CASCADES <= api::MAX_CASCADES);

const DEPTH_BIAS: DepthBiasState = DepthBiasState {
    constant: -2,
    slope_scale: -1.5,
    clamp: 0.0,
};

const CLEAR_COLOR: wgpu::Color = wgpu::Color::WHITE;

const FAR: f32 = 0.0;

fn array_layers(wanted: usize, gl: bool) -> u32 {
    let wanted = wanted.max(1) as u32;
    if gl { wanted.max(2) } else { wanted }
}

fn array_texture(
    device: &RenderDevice,
    label: &str,
    side: u32,
    layers: u32,
    format: TextureFormat,
    usage: TextureUsages,
) -> (Texture, TextureView) {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: layers,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor {
        label: Some(label),
        dimension: Some(TextureViewDimension::D2Array),
        ..default()
    });
    (texture, view)
}

fn layer_view(texture: &Texture, layer: usize) -> TextureView {
    texture.create_view(&TextureViewDescriptor {
        label: Some(gpu_label!("native shadow layer")),
        dimension: Some(TextureViewDimension::D2),
        base_array_layer: layer as u32,
        array_layer_count: Some(1),
        ..default()
    })
}

pub(super) struct InstallShadow {
    pub(super) depth: Option<TextureView>,
    pub(super) color: Option<TextureView>,
    pub(super) compare: Option<Sampler>,
    pub(super) pass: Option<ShadowPass>,
    _stubs: Vec<Texture>,
}

pub(super) struct ShadowPass {
    pub(super) resolution: u32,
    pub(super) distance: f32,
    pub(super) depth_layers: Vec<TextureView>,
    pub(super) color_layers: Vec<TextureView>,
    pub(super) color_format: Option<TextureFormat>,
    pub(super) globals: BindGroupLayout,
    pub(super) draw_layout: BindGroupLayout,
    pub(super) draws: Vec<BindGroup>,
    depth_array: TextureView,
    color_array: Option<TextureView>,
    _textures: Vec<Texture>,
    _buffers: Vec<Buffer>,
}

impl InstallShadow {
    pub(super) fn new(
        device: &RenderDevice,
        built: &Built,
        draws_world: bool,
        gl: bool,
        binds: &dyn Fn(Global) -> bool,
    ) -> Result<InstallShadow, String> {
        let pass = match &built.shadow {
            Some(shadow) if draws_world => Some(ShadowPass::new(device, built, shadow, gl)?),
            _ => None,
        };
        let mut stubs = Vec::new();
        let mut stub = |label: &str, format: TextureFormat| {
            let usage = TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT;
            let (texture, view) =
                array_texture(device, label, 1, array_layers(1, gl), format, usage);
            stubs.push(texture);
            view
        };
        let depth = match &pass {
            Some(pass) => Some(pass.depth_array.clone()),
            None => binds(Global::ShadowDepth)
                .then(|| stub(gpu_label!("native shadow depth stand-in"), DEPTH_FORMAT)),
        };
        let color = match pass.as_ref().and_then(|p| p.color_array.clone()) {
            Some(view) => Some(view),
            None => binds(Global::ShadowColor).then(|| {
                let format =
                    texture_format(api::SHADOW_COLOR_FORMAT).unwrap_or(TextureFormat::Rgba8Unorm);
                stub(gpu_label!("native shadow colour stand-in"), format)
            }),
        };
        let compare = (binds(Global::ShadowCompare) || pass.is_some()).then(|| {
            device.create_sampler(&SamplerDescriptor {
                label: Some(gpu_label!("native shadow compare")),
                address_mode_u: AddressMode::ClampToEdge,
                address_mode_v: AddressMode::ClampToEdge,
                address_mode_w: AddressMode::ClampToEdge,
                mag_filter: FilterMode::Linear,
                min_filter: FilterMode::Linear,
                compare: Some(CompareFunction::GreaterEqual),
                ..default()
            })
        });
        Ok(InstallShadow {
            depth,
            color,
            compare,
            pass,
            _stubs: stubs,
        })
    }
}

impl ShadowPass {
    fn new(
        device: &RenderDevice,
        built: &Built,
        shadow: &BuiltShadow,
        gl: bool,
    ) -> Result<ShadowPass, String> {
        let max = device.limits().max_texture_dimension_2d;
        let resolution = shadow.resolution.clamp(1, max);
        if resolution != shadow.resolution {
            crate::log_info!(
                "shaders",
                "{}: the shadow map is {resolution} a side, this device's largest",
                built.id
            );
        }
        let layers = array_layers(CASCADES, gl);
        let usage = TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING;
        let (depth, depth_array) = array_texture(
            device,
            gpu_label!("native shadow depth"),
            resolution,
            layers,
            DEPTH_FORMAT,
            usage,
        );
        let depth_layers = (0..CASCADES).map(|c| layer_view(&depth, c)).collect();
        let mut textures = vec![depth];
        let color_format = match shadow.color {
            Some(format) => {
                Some(texture_format(format).ok_or("the shadow colour format is depth")?)
            }
            None => None,
        };
        let (color_array, color_layers) = match color_format {
            Some(format) => {
                let (color, array) = array_texture(
                    device,
                    gpu_label!("native shadow colour"),
                    resolution,
                    layers,
                    format,
                    usage,
                );
                let views = (0..CASCADES).map(|c| layer_view(&color, c)).collect();
                textures.push(color);
                (Some(array), views)
            }
            None => (None, Vec::new()),
        };
        let what = format!("[shadow] ({})", shadow.file);
        let globals = globals_layout(device, built, &shadow.bindings, &what)?;
        let draw_layout = device.create_bind_group_layout(
            gpu_label!("native shadow cascade"),
            &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX_FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(16),
                },
                count: None,
            }],
        );
        let buffers: Vec<Buffer> = (0..CASCADES)
            .map(|c| {
                device.create_buffer_with_data(&BufferInitDescriptor {
                    label: Some(gpu_label!("native shadow cascade")),
                    contents: bytemuck::bytes_of(&[c as u32, 0, 0, 0]),
                    usage: BufferUsages::UNIFORM,
                })
            })
            .collect();
        let draws = buffers
            .iter()
            .map(|buffer| {
                device.create_bind_group(
                    gpu_label!("native shadow cascade"),
                    &draw_layout,
                    &[BindGroupEntry {
                        binding: 0,
                        resource: buffer.as_entire_binding(),
                    }],
                )
            })
            .collect();
        Ok(ShadowPass {
            resolution,
            distance: shadow.distance as f32,
            depth_layers,
            color_layers,
            color_format,
            globals,
            draw_layout,
            draws,
            depth_array,
            color_array,
            _textures: textures,
            _buffers: buffers,
        })
    }
}

impl ShadowPass {
    pub(super) fn frame(
        &self,
        inputs: &Inputs,
        values: &FrameValues,
        origin: RenderOrigin,
        far: f32,
        pack_view: Option<&mut PackCullView>,
    ) -> FrameShadow {
        let toward_light = cascade::toward_light(inputs);
        if let Some(pack_view) = pack_view {
            pack_view.0 = Some(shadow_cull_shape(&ShadowCullInput {
                mode: ShadowCullMode::Advanced,
                camera_clip: values.camera_clip(),
                toward_light,
                camera: values.camera().as_dvec3(),
                shadow_distance: self.distance,
                voxel_distance: 0.0,
                render_mul: 1.0,
                render_distance: far,
            }));
        }
        let camera = origin.to_world(values.camera());
        cascade::fit(
            toward_light,
            camera,
            self.resolution,
            self.distance,
            self.depth_layers.len(),
        )
    }
}

pub(super) struct ShadowPipelines {
    pub(super) terrain: Vec<(u32, RenderPipeline)>,
    pub(super) entities: (RenderPipeline, RenderPipeline),
}

pub(super) struct ShadowLayouts<'a> {
    pub(super) globals: &'a BindGroupLayout,
    pub(super) terrain: &'a BindGroupLayout,
    pub(super) quads: &'a BindGroupLayout,
    pub(super) draw: &'a BindGroupLayout,
}

pub(super) fn shadow_pipelines(
    device: &RenderDevice,
    installed: &NativeInstalled,
    layouts: ShadowLayouts<'_>,
    color: Option<TextureFormat>,
) -> Option<ShadowPipelines> {
    let generation = installed.generation;
    let shadow = installed.built.shadow.as_ref()?;
    let module = device.create_and_validate_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(&format!(
            "{GPU_LABEL}/{}/{generation}/shadow {}",
            installed.name, shadow.file
        )),
        source: wgpu::ShaderSource::Wgsl(shadow.wgsl.as_str().into()),
    });
    let constants: Vec<(&str, f64)> = shadow
        .overrides
        .iter()
        .map(|(id, value)| (id.as_str(), *value))
        .collect();
    let pipeline_layout = |groups: &[Option<&BindGroupLayout>]| {
        let groups: Vec<Option<&wgpu::BindGroupLayout>> =
            groups.iter().map(|l| l.map(|l| &**l)).collect();
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&pipeline_label(generation, format_args!("native shadow"))),
            bind_group_layouts: &groups,
            immediate_size: 0,
        })
    };
    let draw = Some(layouts.draw);
    let globals = Some(layouts.globals);
    let terrain_layout =
        pipeline_layout(&[globals, Some(layouts.terrain), Some(layouts.quads), draw]);
    let mesh_layout = pipeline_layout(&[globals, None, None, draw]);
    let make = |label: String,
                layout: &wgpu::PipelineLayout,
                vertex: &str,
                fragment: Option<&str>,
                buffers: Vec<bevy::mesh::VertexBufferLayout>,
                topology| {
        let buffers: Vec<wgpu::VertexBufferLayout> = buffers
            .iter()
            .map(|b| wgpu::VertexBufferLayout {
                array_stride: b.array_stride,
                step_mode: b.step_mode,
                attributes: &b.attributes,
            })
            .collect();
        let options = || wgpu::PipelineCompilationOptions {
            constants: constants.as_slice(),
            ..Default::default()
        };
        let targets: Vec<Option<ColorTargetState>> = color
            .map(|format| ColorTargetState {
                format,
                blend: None,
                write_mask: ColorWrites::ALL,
            })
            .into_iter()
            .map(Some)
            .collect();
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&label),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some(vertex),
                compilation_options: options(),
                buffers: &buffers,
            },
            primitive: PrimitiveState {
                topology,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                ..default()
            },
            depth_stencil: Some(DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: default(),
                bias: DEPTH_BIAS,
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: fragment.map(|entry| wgpu::FragmentState {
                module: &module,
                entry_point: Some(entry),
                compilation_options: options(),
                targets: &targets,
            }),
            multiview_mask: None,
            cache: None,
        })
    };

    let caster = &shadow.terrain;
    let streams: &[TerrainStream] = match caster.draws {
        Draws::Terrain(streams) => streams,
        _ => &[],
    };
    let terrain = streams
        .iter()
        .map(|stream| {
            let (number, fragment) = match stream {
                TerrainStream::Solid => (STREAM_SOLID, caster.fragment.as_deref()),
                TerrainStream::Cutout => (STREAM_CUTOUT, Some(caster.cutout.as_str())),
                TerrainStream::Water => (STREAM_WATER, caster.fragment.as_deref()),
            };
            let label = pipeline_label(generation, format_args!("native shadow stream {number}"));
            let topology = bevy::render::render_resource::PrimitiveTopology::TriangleList;
            (
                number,
                make(
                    label,
                    &terrain_layout,
                    caster.vertex.as_str(),
                    fragment,
                    vec![packed_vertex_layout()],
                    topology,
                ),
            )
        })
        .collect();
    let entities = &shadow.entities;
    let topology = entity_topology(EntityKind::ShadowCaster);
    let mesh = |fragment: Option<&str>, which: u8| {
        let label = pipeline_label(generation, format_args!("native shadow casters {which}"));
        make(
            label,
            &mesh_layout,
            entities.vertex.as_str(),
            fragment,
            vec![entity_vertex_layout(), entity_instance_layout()],
            topology,
        )
    };
    let cutout = mesh(Some(entities.cutout.as_str()), 0);
    let plain = mesh(entities.fragment.as_deref(), 1);
    Some(ShadowPipelines {
        terrain,
        entities: (cutout, plain),
    })
}

pub(super) struct ShadowFrame<'a> {
    pub(super) world: &'a World,
    pub(super) globals: &'a BindGroup,
    pub(super) terrain_group: &'a BindGroup,
    pub(super) sources: &'a StreamSources<'a>,
    pub(super) key: RetainedViewEntity,
    pub(super) entity_frame: &'a EntityFrame,
    pub(super) entity_passes: &'a EntityPasses,
}

pub(super) fn record_shadow(
    render_context: &mut RenderContext,
    pass: &ShadowPass,
    pipelines: &ShadowPipelines,
    frame: &ShadowFrame<'_>,
) {
    for (c, (depth, draw)) in pass.depth_layers.iter().zip(&pass.draws).enumerate() {
        let color = pass.color_layers.get(c);
        let passes = pipelines.terrain.len().max(1);
        for at in 0..passes {
            let first = at == 0;
            let color_load = if first {
                LoadOp::Clear(CLEAR_COLOR)
            } else {
                LoadOp::Load
            };
            let depth_load = if first {
                LoadOp::Clear(FAR)
            } else {
                LoadOp::Load
            };
            let attachments: Vec<_> = color
                .and_then(|view| color_attachment(view, color_load))
                .into_iter()
                .map(Some)
                .collect();
            let mut encoder = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some(gpu_label!("native shadow")),
                color_attachments: &attachments,
                depth_stencil_attachment: Some(depth_attachment(depth, depth_load)),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let Some((stream, pipeline)) = pipelines.terrain.get(at) else {
                continue;
            };
            encoder.set_render_pipeline(pipeline);
            encoder.set_bind_group(GLOBALS_GROUP as usize, frame.globals, &[]);
            encoder.set_bind_group(TERRAIN_GROUP as usize, frame.terrain_group, &[]);
            encoder.set_bind_group(SHADOW_DRAW_GROUP as usize, draw, &[]);
            draw_pools(&mut encoder, frame.sources, |encoder, pool| {
                let _ = draw_region(encoder, frame.sources, frame.key, pool, *stream);
            });
        }
        let segment = SegmentTargets {
            depth,
            color: color.map(std::slice::from_ref),
            draw_group: Some((SHADOW_DRAW_GROUP, draw)),
        };
        entity_segment(
            render_context,
            frame.world,
            frame.entity_frame,
            frame.entity_passes,
            Segment::Shadow,
            depth,
            Some(&segment),
        );
    }
}
