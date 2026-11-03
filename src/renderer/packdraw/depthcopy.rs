use bevy::render::render_resource::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindingResource, TextureFormat, TextureSampleType,
    TextureView,
};
use bevy::render::renderer::RenderDevice;

use super::fullscreen::{
    fullscreen_corner, fullscreen_pass, fullscreen_pipeline, pipeline_layout, single_texture_layout,
};

pub(super) const FORMAT: TextureFormat = TextureFormat::R32Float;

const COPY_WGSL: &str = concat!(
    fullscreen_corner!(),
    "
@group(0) @binding(0) var depth: texture_depth_2d;

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(corner(index) * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(textureLoad(depth, vec2<i32>(position.xy), 0), 0.0, 0.0, 1.0);
}
"
);

pub(super) fn supported(device: &RenderDevice) -> bool {
    !cfg!(all(target_arch = "wasm32", not(feature = "webgpu")))
        && device
            .features()
            .contains(wgpu::Features::FLOAT32_FILTERABLE)
}

pub(super) struct DepthCopy {
    layout: BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}

impl DepthCopy {
    pub(super) fn new(device: &RenderDevice) -> DepthCopy {
        let module = device.create_and_validate_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(gpu_label!("depth copy")),
            source: wgpu::ShaderSource::Wgsl(COPY_WGSL.into()),
        });
        let layout =
            single_texture_layout(device, gpu_label!("depth copy"), TextureSampleType::Depth);
        let pipeline_layout = pipeline_layout(device, gpu_label!("depth copy"), &layout);
        let pipeline = fullscreen_pipeline(
            device,
            gpu_label!("depth copy"),
            &module,
            &pipeline_layout,
            FORMAT,
        );
        DepthCopy { layout, pipeline }
    }

    pub(super) fn source(&self, device: &RenderDevice, depth: &TextureView) -> BindGroup {
        device.create_bind_group(
            gpu_label!("depth copy"),
            &self.layout,
            &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(depth),
            }],
        )
    }

    pub(super) fn record(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        source: &BindGroup,
        target: &TextureView,
    ) {
        let load = wgpu::LoadOp::Clear(wgpu::Color::WHITE);
        fullscreen_pass(
            encoder,
            gpu_label!("depth copy"),
            target,
            load,
            &self.pipeline,
            source,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_copy_shader_validates() {
        let module = naga::front::wgsl::parse_str(COPY_WGSL).expect("parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("validates");
    }
}
