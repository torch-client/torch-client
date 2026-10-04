use bevy::render::render_resource::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource, BindingType,
    Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, Texture, TextureFormat,
    TextureSampleType, TextureView, TextureViewDescriptor, TextureViewDimension,
};
use bevy::render::renderer::RenderDevice;

use super::GPU_LABEL;
use super::fullscreen::{fullscreen_corner, fullscreen_pass, fullscreen_pipeline};

pub(super) fn level_count(width: u32, height: u32) -> u32 {
    32 - width.max(height).max(1).leading_zeros()
}

const DOWNSAMPLE_WGSL: &str = concat!(
    fullscreen_corner!(),
    "
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var bilinear: sampler;

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> Varyings {
    let at = corner(index);
    var out: Varyings;
    out.position = vec4<f32>(at * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(at.x, 1.0 - at.y);
    return out;
}

@fragment
fn fragment(in: Varyings) -> @location(0) vec4<f32> {
    return textureSampleLevel(source, bilinear, in.uv, 0.0);
}
"
);

pub(super) struct MipGen {
    module: wgpu::ShaderModule,
    layout: BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    sampler: Sampler,
    pipelines: Vec<(TextureFormat, wgpu::RenderPipeline)>,
}

impl MipGen {
    pub(super) fn new(device: &RenderDevice) -> MipGen {
        let module = device.create_and_validate_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(gpu_label!("mipmap downsample")),
            source: wgpu::ShaderSource::Wgsl(DOWNSAMPLE_WGSL.into()),
        });
        let layout = device.create_bind_group_layout(
            gpu_label!("mipmap"),
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        );
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(gpu_label!("mipmap")),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some(gpu_label!("mipmap")),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        MipGen {
            module,
            layout,
            pipeline_layout,
            sampler,
            pipelines: Vec::new(),
        }
    }

    pub(super) fn ensure(&mut self, device: &RenderDevice, format: TextureFormat) {
        if self.pipelines.iter().any(|(f, _)| *f == format) {
            return;
        }
        let label = format!("{GPU_LABEL} mipmap {format:?}");
        let pipeline =
            fullscreen_pipeline(device, &label, &self.module, &self.pipeline_layout, format);
        self.pipelines.push((format, pipeline));
    }

    pub(super) fn chain(
        &self,
        device: &RenderDevice,
        texture: &Texture,
        label: &str,
    ) -> Option<MipChain> {
        let format = texture.format();
        let levels = texture.mip_level_count();
        let pipeline = self.pipelines.iter().find(|(f, _)| *f == format)?.1.clone();
        if levels < 2 {
            return None;
        }
        let level = |n: u32| {
            texture.create_view(&TextureViewDescriptor {
                label: Some(label),
                base_mip_level: n,
                mip_level_count: Some(1),
                ..Default::default()
            })
        };
        let steps = (1..levels)
            .map(|n| {
                let source = level(n - 1);
                let group = device.create_bind_group(
                    label,
                    &self.layout,
                    &[
                        BindGroupEntry {
                            binding: 0,
                            resource: BindingResource::TextureView(&source),
                        },
                        BindGroupEntry {
                            binding: 1,
                            resource: BindingResource::Sampler(&self.sampler),
                        },
                    ],
                );
                (level(n), group)
            })
            .collect();
        Some(MipChain { pipeline, steps })
    }
}

pub(super) struct MipChain {
    pipeline: wgpu::RenderPipeline,
    steps: Vec<(TextureView, BindGroup)>,
}

impl MipChain {
    pub(super) fn record(&self, encoder: &mut wgpu::CommandEncoder) {
        for (target, source) in &self.steps {
            let load = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
            fullscreen_pass(
                encoder,
                gpu_label!("mipmap"),
                target,
                load,
                &self.pipeline,
                source,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_downsample_shader_validates() {
        let module = naga::front::wgsl::parse_str(DOWNSAMPLE_WGSL).expect("parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("validates");
    }

    #[test]
    fn chains_reach_one_texel() {
        assert_eq!(level_count(1, 1), 1);
        assert_eq!(level_count(2, 1), 2);
        assert_eq!(level_count(1920, 1080), 11);
        assert_eq!(level_count(0, 0), 1);
    }
}
