use std::sync::{Arc, OnceLock};

use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    TextureView, TextureViewDescriptor,
};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use parking_lot::RwLock;

use super::GPU_LABEL;
use super::fullscreen::fullscreen_corner;

pub(crate) struct RawAtlas {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) levels: u32,
    pub(super) data: Vec<u8>,
}

static ATLAS: RwLock<Option<Arc<RawAtlas>>> = RwLock::new(None);

pub(crate) fn keep_atlas(width: u32, height: u32, levels: u32, data: &[u8]) {
    *ATLAS.write() = Some(Arc::new(RawAtlas {
        width,
        height,
        levels,
        data: data.to_vec(),
    }));
}

pub(super) fn atlas() -> Option<Arc<RawAtlas>> {
    ATLAS.read().clone()
}

impl RawAtlas {
    pub(super) fn upload(&self, device: &RenderDevice, queue: &RenderQueue) -> Texture {
        let size = super::targets::extent(self.width, self.height);
        let texture = device.create_texture(&TextureDescriptor {
            label: Some(&format!("{GPU_LABEL} atlas raw")),
            size,
            mip_level_count: self.levels,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let mut at = 0;
        for level in 0..self.levels {
            let (w, h) = ((self.width >> level).max(1), (self.height >> level).max(1));
            let len = (w * h * 4) as usize;
            let Some(bytes) = self.data.get(at..at + len) else {
                break;
            };
            at += len;
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * 4),
                    rows_per_image: Some(h),
                },
                super::targets::extent(w, h),
            );
        }
        texture
    }
}

const DEPTH_COPY_WGSL: &str = concat!(
    fullscreen_corner!(),
    "
@group(0) @binding(0) var source: texture_2d<f32>;

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(corner(index) * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fragment(@builtin(position) position: vec4<f32>) -> @builtin(frag_depth) f32 {
    return textureLoad(source, vec2<i32>(position.xy), 0).x;
}
"
);

struct DepthCopy {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}

static DEPTH_COPY: OnceLock<DepthCopy> = OnceLock::new();

fn depth_copy(device: &RenderDevice) -> &'static DepthCopy {
    DEPTH_COPY.get_or_init(|| {
        let device = device.wgpu_device();
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(gpu_label!("depth copy gl")),
            source: wgpu::ShaderSource::Wgsl(DEPTH_COPY_WGSL.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(gpu_label!("depth copy gl")),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(gpu_label!("depth copy gl")),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(gpu_label!("depth copy gl")),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: super::targets::DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[],
            }),
            multiview_mask: None,
            cache: None,
        });
        DepthCopy { layout, pipeline }
    })
}

pub(super) fn copy_depth(
    device: &RenderDevice,
    encoder: &mut wgpu::CommandEncoder,
    from: &TextureView,
    to: &TextureView,
) {
    let copy = depth_copy(device);
    let group = device
        .wgpu_device()
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(gpu_label!("depth copy gl")),
            layout: &copy.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(from),
            }],
        });
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(gpu_label!("depth copy gl")),
        color_attachments: &[],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: to,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(&copy.pipeline);
    pass.set_bind_group(0, &group, &[]);
    pass.draw(0..3, 0..1);
}

const LIGHTMAP_SIZE: u32 = 16;

static LIGHTMAP: OnceLock<(Texture, TextureView)> = OnceLock::new();

fn lightmap(device: &RenderDevice) -> &'static (Texture, TextureView) {
    LIGHTMAP.get_or_init(|| {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some(&format!("{GPU_LABEL} lightmap raw")),
            size: Extent3d {
                width: LIGHTMAP_SIZE,
                height: LIGHTMAP_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());
        (texture, view)
    })
}

pub(super) fn lightmap_view(device: &RenderDevice) -> TextureView {
    lightmap(device).1.clone()
}

pub(crate) fn mirror_lightmap(
    pixels: Option<Res<crate::renderer::terrain::LightmapPixels>>,
    render: Option<Res<super::frame::PackRender>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let Some(pixels) = pixels else { return };
    if render.is_none_or(|r| r.generation() == 0)
        || pixels.data.len() != (LIGHTMAP_SIZE * LIGHTMAP_SIZE * 4) as usize
    {
        return;
    }
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &lightmap(&device).0,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &pixels.data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(LIGHTMAP_SIZE * 4),
            rows_per_image: Some(LIGHTMAP_SIZE),
        },
        super::targets::extent(LIGHTMAP_SIZE, LIGHTMAP_SIZE),
    );
}
