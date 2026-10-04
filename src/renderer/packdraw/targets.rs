use bevy::prelude::*;
use bevy::render::render_resource::{
    AddressMode, Buffer, BufferDescriptor, BufferUsages, Extent3d, FilterMode, Sampler,
    SamplerDescriptor, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    TextureView, TextureViewDescriptor,
};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::texture::GpuImage;

use super::GPU_LABEL;
use super::formats::{image_format, wgpu_format};
use super::images::CustomTexture;
use super::install::Installed;
use crate::shaderpack::customimages::{CustomImage, Dimension, ImageSize};
use crate::shaderpack::directives::{MAX_COLOR_TARGETS, MAX_SHADOW_COLORS};

pub(super) const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

pub(super) fn extent(width: u32, height: u32) -> Extent3d {
    Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}

fn texture_dimension(dimension: Dimension) -> TextureDimension {
    match dimension {
        Dimension::D1 => TextureDimension::D1,
        Dimension::D2 => TextureDimension::D2,
        Dimension::D3 => TextureDimension::D3,
    }
}

fn depth_pair(device: &RenderDevice, size: Extent3d, labels: [&str; 2]) -> (Target, Target) {
    (
        Target::new(
            device,
            labels[0],
            size,
            DEPTH_FORMAT,
            TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_SRC,
        ),
        Target::new(
            device,
            labels[1],
            size,
            DEPTH_FORMAT,
            TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        ),
    )
}

pub(super) struct Target {
    pub(super) texture: Texture,
    pub(super) view: TextureView,
    full: Option<TextureView>,
}

impl Target {
    fn new(
        device: &RenderDevice,
        label: &str,
        size: Extent3d,
        format: TextureFormat,
        usage: TextureUsages,
    ) -> Target {
        Target::with_dimension(device, label, size, TextureDimension::D2, format, usage)
    }

    fn with_dimension(
        device: &RenderDevice,
        label: &str,
        size: Extent3d,
        dimension: TextureDimension,
        format: TextureFormat,
        usage: TextureUsages,
    ) -> Target {
        Target::with_levels(device, label, size, dimension, format, usage, 1)
    }

    fn mipmapped(
        device: &RenderDevice,
        label: &str,
        size: Extent3d,
        format: TextureFormat,
        usage: TextureUsages,
    ) -> Target {
        let levels = super::mipmaps::level_count(size.width, size.height);
        Target::with_levels(
            device,
            label,
            size,
            TextureDimension::D2,
            format,
            usage,
            levels,
        )
    }

    fn with_levels(
        device: &RenderDevice,
        label: &str,
        size: Extent3d,
        dimension: TextureDimension,
        format: TextureFormat,
        usage: TextureUsages,
        levels: u32,
    ) -> Target {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: levels,
            sample_count: 1,
            dimension,
            format,
            usage,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor {
            mip_level_count: Some(1),
            ..default()
        });
        let full = (levels > 1).then(|| texture.create_view(&TextureViewDescriptor::default()));
        Target {
            texture,
            view,
            full,
        }
    }
}

pub(super) struct Statics {
    pub(super) white_2d: Target,
    pub(super) zero_3d: Target,
    pub(super) uint_2d: Target,
    pub(super) uint_3d: Target,
    pub(super) flat_normal: Target,
    pub(super) black_2d: Target,
    pub(super) linear: Sampler,
    pub(super) nearest: Sampler,
    pub(super) repeat: Sampler,
    pub(super) repeat_nearest: Sampler,
    pub(super) mip_linear: Sampler,
}

fn fill(queue: &RenderQueue, target: &Target, size: Extent3d, bytes: &[u8], row: u32) {
    queue.write_texture(
        target.texture.as_image_copy(),
        bytes,
        bevy::render::render_resource::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(row),
            rows_per_image: Some(size.height),
        },
        size,
    );
}

pub(super) fn custom_target(
    device: &RenderDevice,
    queue: &RenderQueue,
    image: &CustomTexture,
) -> Target {
    let size = extent(image.width, image.height);
    let target = Target::new(
        device,
        &format!("{GPU_LABEL} {}", image.path),
        size,
        TextureFormat::Rgba8Unorm,
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
    );
    fill(queue, &target, size, &image.rgba, image.width * 4);
    target
}

pub(super) fn noise_target(device: &RenderDevice, queue: &RenderQueue, size: u32) -> Target {
    let square = extent(size, size);
    let noise = Target::new(
        device,
        gpu_label!("noisetex"),
        square,
        TextureFormat::Rgba8Unorm,
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
    );
    fill(queue, &noise, square, &noise_bytes(size), size * 4);
    noise
}

impl Statics {
    pub(super) fn new(device: &RenderDevice, queue: &RenderQueue) -> Statics {
        let one = extent(1, 1);
        let sampled = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
        let d3 = TextureDimension::D3;
        let rgba8 = TextureFormat::Rgba8Unorm;
        let white_2d = Target::new(device, gpu_label!("white"), one, rgba8, sampled);
        let zero_3d =
            Target::with_dimension(device, gpu_label!("zero 3d"), one, d3, rgba8, sampled);
        let uint_2d = Target::new(
            device,
            gpu_label!("zero uint"),
            one,
            TextureFormat::R32Uint,
            sampled,
        );
        let uint_3d = Target::with_dimension(
            device,
            gpu_label!("zero uint 3d"),
            one,
            d3,
            TextureFormat::R32Uint,
            sampled,
        );
        let flat_normal = Target::new(device, gpu_label!("flat normal"), one, rgba8, sampled);
        let black_2d = Target::new(device, gpu_label!("black"), one, rgba8, sampled);
        fill(queue, &white_2d, one, &[255; 4], 4);
        fill(queue, &flat_normal, one, &[127, 127, 255, 255], 4);

        let sampler = |filter: FilterMode, address: AddressMode| {
            device.create_sampler(&SamplerDescriptor {
                label: Some(gpu_label!("sampler")),
                address_mode_u: address,
                address_mode_v: address,
                address_mode_w: address,
                mag_filter: filter,
                min_filter: filter,
                mipmap_filter: FilterMode::Nearest,
                ..default()
            })
        };
        Statics {
            white_2d,
            zero_3d,
            uint_2d,
            uint_3d,
            flat_normal,
            black_2d,
            linear: sampler(FilterMode::Linear, AddressMode::ClampToEdge),
            nearest: sampler(FilterMode::Nearest, AddressMode::ClampToEdge),
            repeat: sampler(FilterMode::Linear, AddressMode::Repeat),
            repeat_nearest: sampler(FilterMode::Nearest, AddressMode::Repeat),
            mip_linear: device.create_sampler(&SamplerDescriptor {
                label: Some(gpu_label!("mipmapped sampler")),
                mag_filter: FilterMode::Linear,
                min_filter: FilterMode::Linear,
                mipmap_filter: FilterMode::Linear,
                ..default()
            }),
        }
    }
}

fn noise_bytes(size: u32) -> Vec<u8> {
    let mut state: u32 = 0x9e37_79b9;
    (0..size * size * 4)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state >> 24) as u8
        })
        .collect()
}

pub(super) fn display_view(image: &GpuImage) -> TextureView {
    if image.texture_format != TextureFormat::Rgba8UnormSrgb {
        return image.texture_view.clone();
    }
    image.texture.create_view(&TextureViewDescriptor {
        label: Some(gpu_label!("display view")),
        format: Some(TextureFormat::Rgba8Unorm),
        ..default()
    })
}

pub(super) struct Targets {
    pub(super) size: UVec2,
    color: [Option<[Target; 2]>; MAX_COLOR_TARGETS],
    pub(super) depth: Target,
    pub(super) depth_opaque: Target,
    pub(super) depth_no_hand: Option<Target>,
    pub(super) mipmapped: u16,
}

pub(super) struct ShadowTargets {
    pub(super) depth: Target,
    pub(super) depth_opaque: Target,
    pub(super) color: [Option<Target>; MAX_SHADOW_COLORS],
    pub(super) filtered: [Option<Target>; 2],
}

pub(super) fn image_stub(device: &RenderDevice, image: &CustomImage) -> Target {
    let dimension = match image.size {
        ImageSize::Fixed { dimension, .. } => texture_dimension(dimension),
        ImageSize::Relative { .. } => TextureDimension::D2,
    };
    Target::with_dimension(
        device,
        &format!("{GPU_LABEL} image {} stub", image.name),
        extent(1, 1),
        dimension,
        image_format(image.format),
        TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
    )
}

pub(super) fn image_target(device: &RenderDevice, image: &CustomImage, screen: UVec2) -> Target {
    let (size, dimension) = match image.size {
        ImageSize::Fixed {
            width,
            height,
            depth,
            dimension,
        } => (
            Extent3d {
                width,
                height,
                depth_or_array_layers: depth,
            },
            texture_dimension(dimension),
        ),
        ImageSize::Relative { width, height } => {
            let scaled = |n: u32, scale: f32| ((n as f32 * scale).ceil() as u32).max(1);
            (
                extent(scaled(screen.x, width), scaled(screen.y, height)),
                TextureDimension::D2,
            )
        }
    };
    Target::with_dimension(
        device,
        &format!("{GPU_LABEL} image {}", image.name),
        size,
        dimension,
        image_format(image.format),
        TextureUsages::STORAGE_BINDING
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_DST
            | TextureUsages::COPY_SRC,
    )
}

pub(super) fn zero_layout(texture: &bevy::render::render_resource::Texture) -> (u32, u64) {
    let size = texture.size();
    let texel = texture.format().block_copy_size(None).unwrap_or(4);
    let row = (size.width * texel).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    (
        row,
        u64::from(row) * u64::from(size.height) * u64::from(size.depth_or_array_layers),
    )
}

pub(super) fn storage_buffer(
    device: &RenderDevice,
    buffer: &crate::shaderpack::customimages::BufferObject,
    screen: [u32; 2],
) -> Buffer {
    let size = buffer.bytes(screen).div_ceil(4) * 4;
    device.create_buffer(&BufferDescriptor {
        label: Some(&format!("{GPU_LABEL} bufferObject.{}", buffer.index)),
        size,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

const MAX_SHADOW_MAP: u32 = 8192;

impl ShadowTargets {
    pub(super) fn new(
        device: &RenderDevice,
        installed: &Installed,
        filtering: [bool; 2],
    ) -> ShadowTargets {
        let size = installed.constants.shadow_map_resolution.clamp(
            1,
            MAX_SHADOW_MAP.min(device.limits().max_texture_dimension_2d),
        );
        let square = extent(size, size);
        let color = std::array::from_fn(|index| {
            (installed.used_shadow & (1 << index) != 0).then(|| {
                Target::new(
                    device,
                    &format!("{GPU_LABEL} shadowcolor{index}"),
                    square,
                    wgpu_format(installed.targets.shadow[index].format),
                    TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                )
            })
        });
        let (depth, depth_opaque) = depth_pair(
            device,
            square,
            [gpu_label!("shadowtex0"), gpu_label!("shadowtex1")],
        );
        ShadowTargets {
            depth,
            depth_opaque,
            color,
            filtered: {
                [0, 1].map(|n| {
                    filtering[n].then(|| {
                        Target::new(
                            device,
                            &format!("{GPU_LABEL} shadowtex{n} filtered"),
                            square,
                            super::depthcopy::FORMAT,
                            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                        )
                    })
                })
            },
        }
    }
}

impl Targets {
    pub(super) fn new(
        device: &RenderDevice,
        installed: &Installed,
        mipmapped: u16,
        no_hand: bool,
        size: UVec2,
    ) -> Targets {
        let color = std::array::from_fn(|index| {
            (installed.used_targets & (1 << index) != 0).then(|| {
                let settings = &installed.targets.settings[index];
                let format = wgpu_format(settings.format);
                let [width, height] = settings.resolve_size([size.x, size.y]);
                let extent = extent(width, height);
                let usage = TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING;
                [0, 1].map(|half| {
                    let label = format!("{GPU_LABEL} colortex{index}.{half}");
                    if mipmapped & (1 << index) != 0 {
                        Target::mipmapped(device, &label, extent, format, usage)
                    } else {
                        Target::new(device, &label, extent, format, usage)
                    }
                })
            })
        });
        let extent = extent(size.x, size.y);
        let (depth, depth_opaque) = depth_pair(
            device,
            extent,
            [gpu_label!("depthtex0"), gpu_label!("depthtex1")],
        );
        Targets {
            size,
            color,
            depth,
            depth_opaque,
            depth_no_hand: no_hand.then(|| {
                Target::new(
                    device,
                    gpu_label!("depthtex2"),
                    extent,
                    DEPTH_FORMAT,
                    TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                )
            }),
            mipmapped,
        }
    }

    pub(super) fn color(&self, index: u8, half: usize) -> &TextureView {
        &self.pair(index)[half].view
    }

    pub(super) fn color_mipmapped(&self, index: u8, half: usize) -> &TextureView {
        let target = &self.pair(index)[half];
        target.full.as_ref().unwrap_or(&target.view)
    }

    pub(super) fn pair(&self, index: u8) -> &[Target; 2] {
        self.color[index as usize]
            .as_ref()
            .expect("a used target is allocated")
    }
}
