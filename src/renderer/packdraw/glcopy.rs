use std::cell::OnceCell;

use bevy::render::render_resource::{
    BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource, BindingType,
    RenderPipeline, ShaderStages, TextureFormat, TextureSampleType, TextureView,
    TextureViewDimension,
};
use bevy::render::renderer::RenderDevice;

use super::fullscreen::{fullscreen_corner, fullscreen_pass, fullscreen_pipeline};

const REENCODE_WGSL: &str = concat!(
    fullscreen_corner!(),
    r#"
@group(0) @binding(0) var source: texture_2d<f32>;

struct Out {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> Out {
    var out: Out;
    out.position = vec4<f32>(corner(index) * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

fn encode(c: vec3<f32>) -> vec3<f32> {
    let low = c * 12.92;
    let high = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, c <= vec3<f32>(0.0031308));
}

@fragment
fn fragment(in: Out) -> @location(0) vec4<f32> {
    let color = textureLoad(source, vec2<i32>(in.position.xy), 0);
    return vec4<f32>(encode(color.rgb), color.a);
}
"#
);

struct Reencode {
    layout: BindGroupLayout,
    pipeline: RenderPipeline,
}

thread_local! {
    static REENCODE: OnceCell<Reencode> = const { OnceCell::new() };
}

fn reencode(device: &RenderDevice) -> Reencode {
    let layout = device.create_bind_group_layout(
        gpu_label!("entity reencode"),
        &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Texture {
                sample_type: TextureSampleType::Float { filterable: false },
                view_dimension: TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        }],
    );
    let module = device.create_and_validate_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(gpu_label!("entity reencode")),
        source: wgpu::ShaderSource::Wgsl(REENCODE_WGSL.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(gpu_label!("entity reencode")),
        bind_group_layouts: &[&layout],
        push_constant_ranges: &[],
    });
    let pipeline = fullscreen_pipeline(
        device,
        gpu_label!("entity reencode"),
        &module,
        &pipeline_layout,
        TextureFormat::Rgba8Unorm,
    );
    Reencode {
        layout,
        pipeline: RenderPipeline::from(pipeline),
    }
}

pub(super) fn copy_srgb_bytes(
    device: &RenderDevice,
    encoder: &mut wgpu::CommandEncoder,
    source: &TextureView,
    target: &TextureView,
) {
    REENCODE.with(|cell| {
        let made = cell.get_or_init(|| reencode(device));
        let group = device.create_bind_group(
            gpu_label!("entity reencode"),
            &made.layout,
            &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(source),
            }],
        );
        fullscreen_pass(
            encoder,
            gpu_label!("entity reencode"),
            target,
            wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            &made.pipeline,
            &group,
        );
    });
}
