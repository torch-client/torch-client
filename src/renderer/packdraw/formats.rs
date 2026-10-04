use bevy::render::render_resource::{TextureFormat, TextureUsages};

use crate::shaderpack::customimages::ImageFormat;
use crate::shaderpack::directives::TargetFormat;

const OPTIONAL_FORMATS: [(TextureFormat, TextureFormat); 10] = [
    (TextureFormat::R8Snorm, TextureFormat::R16Float),
    (TextureFormat::Rg8Snorm, TextureFormat::Rg16Float),
    (TextureFormat::Rgba8Snorm, TextureFormat::Rgba16Float),
    (TextureFormat::R16Unorm, TextureFormat::R16Float),
    (TextureFormat::Rg16Unorm, TextureFormat::Rg16Float),
    (TextureFormat::Rgba16Unorm, TextureFormat::Rgba16Float),
    (TextureFormat::R16Snorm, TextureFormat::R16Float),
    (TextureFormat::Rg16Snorm, TextureFormat::Rg16Float),
    (TextureFormat::Rgba16Snorm, TextureFormat::Rgba16Float),
    (TextureFormat::Rg11b10Ufloat, TextureFormat::Rgba16Float),
];

static RENDERABLE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub(crate) fn init_formats(adapter: &bevy::render::renderer::RenderAdapter) {
    let bits = OPTIONAL_FORMATS
        .iter()
        .enumerate()
        .fold(0u32, |bits, (at, (format, _))| {
            let features = adapter.get_texture_format_features(*format);
            let usable = features
                .allowed_usages
                .contains(TextureUsages::RENDER_ATTACHMENT)
                && features
                    .flags
                    .contains(wgpu::TextureFormatFeatureFlags::BLENDABLE)
                && features
                    .flags
                    .contains(wgpu::TextureFormatFeatureFlags::FILTERABLE);
            if usable { bits | 1 << at } else { bits }
        });
    RENDERABLE.store(bits, std::sync::atomic::Ordering::Relaxed);
}

fn renderable(format: TextureFormat) -> TextureFormat {
    let bits = RENDERABLE.load(std::sync::atomic::Ordering::Relaxed);
    match OPTIONAL_FORMATS.iter().position(|(f, _)| *f == format) {
        Some(at) if bits & (1 << at) != 0 => format,
        Some(at) => OPTIONAL_FORMATS[at].1,
        None => format,
    }
}

pub(super) fn wgpu_format(format: TargetFormat) -> TextureFormat {
    use TargetFormat as F;
    use TextureFormat as T;
    renderable(match format {
        F::R16Unorm => T::R16Unorm,
        F::Rg16Unorm => T::Rg16Unorm,
        F::Rgba16Unorm => T::Rgba16Unorm,
        F::R16Snorm => T::R16Snorm,
        F::Rg16Snorm => T::Rg16Snorm,
        F::Rgba16Snorm => T::Rgba16Snorm,
        F::Rg11b10Ufloat => T::Rg11b10Ufloat,
        F::R8Unorm => T::R8Unorm,
        F::Rg8Unorm => T::Rg8Unorm,
        F::Rgba8Unorm => T::Rgba8Unorm,
        F::R8Snorm => T::R8Snorm,
        F::Rg8Snorm => T::Rg8Snorm,
        F::Rgba8Snorm => T::Rgba8Snorm,
        F::R16Float => T::R16Float,
        F::Rg16Float => T::Rg16Float,
        F::Rgba16Float => T::Rgba16Float,
        F::R32Float => T::R32Float,
        F::Rg32Float => T::Rg32Float,
        F::Rgba32Float => T::Rgba32Float,
        F::Rgb10a2Unorm => T::Rgb10a2Unorm,
        F::R8Uint => T::R8Uint,
        F::Rg8Uint => T::Rg8Uint,
        F::Rgba8Uint => T::Rgba8Uint,
        F::R8Sint => T::R8Sint,
        F::Rg8Sint => T::Rg8Sint,
        F::Rgba8Sint => T::Rgba8Sint,
        F::R16Uint => T::R16Uint,
        F::Rg16Uint => T::Rg16Uint,
        F::Rgba16Uint => T::Rgba16Uint,
        F::R16Sint => T::R16Sint,
        F::Rg16Sint => T::Rg16Sint,
        F::Rgba16Sint => T::Rgba16Sint,
        F::R32Uint => T::R32Uint,
        F::Rg32Uint => T::Rg32Uint,
        F::Rgba32Uint => T::Rgba32Uint,
        F::R32Sint => T::R32Sint,
        F::Rg32Sint => T::Rg32Sint,
        F::Rgba32Sint => T::Rgba32Sint,
        F::Rgb10a2Uint => T::Rgb10a2Uint,
    })
}

pub(super) fn image_format(format: ImageFormat) -> TextureFormat {
    match format {
        ImageFormat::R8Ui => TextureFormat::R8Uint,
        ImageFormat::R32Ui => TextureFormat::R32Uint,
        ImageFormat::R32I => TextureFormat::R32Sint,
        ImageFormat::R32F => TextureFormat::R32Float,
        ImageFormat::Rg32F => TextureFormat::Rg32Float,
        ImageFormat::Rgba8 => TextureFormat::Rgba8Unorm,
        ImageFormat::Rgba8Ui => TextureFormat::Rgba8Uint,
        ImageFormat::Rgba16F => TextureFormat::Rgba16Float,
        ImageFormat::Rgba16Ui => TextureFormat::Rgba16Uint,
        ImageFormat::Rgba32F => TextureFormat::Rgba32Float,
        ImageFormat::Rgba32Ui => TextureFormat::Rgba32Uint,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_target_format_maps() {
        for name in [
            "RGBA8",
            "RGBA16F",
            "R11F_G11F_B10F",
            "RGB10_A2",
            "R32UI",
            "RG16",
            "RGBA32F",
            "RGBA16",
            "RGBA16_SNORM",
        ] {
            let format = TargetFormat::parse(name).unwrap();
            let wgpu = wgpu_format(format);
            assert!(!wgpu.is_combined_depth_stencil_format(), "{name}");
        }
    }
}
