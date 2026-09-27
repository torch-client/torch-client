use bevy::prelude::*;

use super::atlas::Region;

#[derive(Clone)]
pub struct AtlasWrite {
    pub region: Region,
    pub pixels: std::sync::Arc<Vec<u8>>,
}

#[derive(Resource, Clone, Default)]
pub struct AtlasWrites {
    pub image: Option<Handle<Image>>,
    pub pending: Vec<AtlasWrite>,
}

impl AtlasWrites {
    pub fn push(&mut self, region: Region, pixels: impl Into<std::sync::Arc<Vec<u8>>>) {
        let pixels = pixels.into();
        if pixels.len() as u32 == region.w * region.h * 4 {
            self.pending.push(AtlasWrite { region, pixels });
        }
    }
}

pub fn extract_atlas_writes(
    mut main: ResMut<bevy::render::MainWorld>,
    mut target: ResMut<AtlasWrites>,
) {
    let Some(mut source) = main.get_resource_mut::<AtlasWrites>() else {
        return;
    };
    if target.image.is_none() {
        target.image = source.image.clone();
    }
    if !source.pending.is_empty() {
        let taken = std::mem::take(&mut source.pending);
        target.pending.extend(taken);
    }
}

pub fn upload_atlas_writes(
    writes: Option<ResMut<AtlasWrites>>,
    images: Res<bevy::render::render_asset::RenderAssets<bevy::render::texture::GpuImage>>,
    queue: Res<bevy::render::renderer::RenderQueue>,
) {
    use bevy::render::render_resource::{
        Extent3d, Origin3d, TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect,
    };

    let Some(mut writes) = writes else { return };
    if writes.pending.is_empty() {
        return;
    }
    let Some(image) = writes.image.clone() else {
        return;
    };
    let Some(gpu) = images.get(&image) else {
        return;
    };

    for write in &writes.pending {
        let r = write.region;
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &gpu.texture,
                mip_level: 0,
                origin: Origin3d {
                    x: r.x,
                    y: r.y,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            &write.pixels,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(r.w * 4),
                rows_per_image: Some(r.h),
            },
            Extent3d {
                width: r.w,
                height: r.h,
                depth_or_array_layers: 1,
            },
        );
    }
    writes.pending.clear();
}
