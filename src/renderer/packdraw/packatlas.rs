use bevy::render::render_resource::{
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor, TextureViewId,
};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::texture::GpuImage;

use super::GPU_LABEL;
use super::frame::ProcessGpu;
use super::targets::{display_view, extent};

pub(super) struct PackAtlas {
    source: TextureViewId,
    view: TextureView,
}

impl ProcessGpu {
    pub(super) fn pack_atlas(
        &mut self,
        device: &RenderDevice,
        queue: &RenderQueue,
        atlas: &GpuImage,
    ) -> TextureView {
        if let Some(existing) = &self.pack_atlas
            && existing.source == atlas.texture_view.id()
        {
            return existing.view.clone();
        }
        #[cfg(all(target_arch = "wasm32", not(feature = "webgpu")))]
        if let Some(raw) = super::rawviews::atlas() {
            let texture = raw.upload(device, queue);
            if let Some(tiles) = crate::renderer::atlas::untinted_tiles() {
                write_tiles(queue, &texture, raw.width, raw.levels, &tiles);
            }
            let view = texture.create_view(&TextureViewDescriptor::default());
            self.pack_atlas = Some(PackAtlas {
                source: atlas.texture_view.id(),
                view: view.clone(),
            });
            return view;
        }
        let Some(tiles) = crate::renderer::atlas::untinted_tiles() else {
            return display_view(atlas);
        };
        let size = atlas.texture.size();
        let levels = atlas.texture.mip_level_count();
        let texture = device.create_texture(&TextureDescriptor {
            label: Some(&format!("{GPU_LABEL} atlas")),
            size,
            mip_level_count: levels,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: atlas.texture.format(),
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: if super::RAW_VIEWS {
                &[TextureFormat::Rgba8Unorm]
            } else {
                &[]
            },
        });
        let mut encoder =
            device
                .wgpu_device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some(gpu_label!("atlas")),
                });
        for level in 0..levels {
            let at = |texture| wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            };
            encoder.copy_texture_to_texture(
                at(&atlas.texture),
                at(&texture),
                extent((size.width >> level).max(1), (size.height >> level).max(1)),
            );
        }
        queue.submit([encoder.finish()]);
        write_tiles(queue, &texture, size.width, levels, &tiles);
        let view = texture.create_view(&TextureViewDescriptor {
            format: super::RAW_VIEWS.then_some(TextureFormat::Rgba8Unorm),
            ..Default::default()
        });
        self.pack_atlas = Some(PackAtlas {
            source: atlas.texture_view.id(),
            view: view.clone(),
        });
        view
    }
}

fn write_tiles(
    queue: &RenderQueue,
    texture: &wgpu::Texture,
    width: u32,
    levels: u32,
    tiles: &crate::renderer::atlas::UntintedTiles,
) {
    for (tile, tile_levels) in tiles.iter() {
        let (tx, ty) = (
            tile % crate::renderer::ATLAS_COLS,
            tile / crate::renderer::ATLAS_COLS,
        );
        for (level, bytes) in tile_levels.iter().enumerate().take(levels as usize) {
            let side = (width / crate::renderer::ATLAS_COLS) >> level;
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d {
                        x: tx * side,
                        y: ty * side,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(side * 4),
                    rows_per_image: Some(side),
                },
                extent(side, side),
            );
        }
    }
}
