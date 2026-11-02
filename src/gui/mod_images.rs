use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, FilterMode, Origin3d, SamplerDescriptor, TexelCopyBufferLayout, TexelCopyTextureInfo,
    Texture, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    TextureViewDescriptor,
};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::{Render, RenderApp, RenderSystems};

use super::draw::GuiImageViews;
use super::painter::ATLAS_TEXTURE;
use super::pool::GuiPool;
use crate::modules::script::image_pixels;

struct Uploaded {
    texture: Texture,
    size: (u32, u32),
    generation: u64,
}

#[derive(Resource, Default)]
struct ModTextures {
    textures: HashMap<u64, Uploaded>,
    sampler: Option<bevy::render::render_resource::Sampler>,
    too_big: HashSet<u64>,
}

fn upload_mod_images(
    pool: Res<GuiPool>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    mut store: ResMut<ModTextures>,
    mut views: ResMut<GuiImageViews>,
) {
    store.textures.retain(|&key, _| image_pixels(key).is_some());
    views.0.retain(|key, _| store.textures.contains_key(key));
    let mut drawn: Vec<u64> = pool
        .ranges
        .iter()
        .map(|r| r.texture)
        .filter(|&t| t != ATLAS_TEXTURE)
        .collect();
    if drawn.is_empty() {
        return;
    }
    drawn.sort_unstable();
    drawn.dedup();
    let sampler = store
        .sampler
        .get_or_insert_with(|| {
            device.create_sampler(&SamplerDescriptor {
                label: Some("mod_image"),
                mag_filter: FilterMode::Nearest,
                min_filter: FilterMode::Nearest,
                ..Default::default()
            })
        })
        .clone();
    let max_side = device.limits().max_texture_dimension_2d;
    for key in drawn {
        let Some(px) = image_pixels(key) else {
            continue;
        };
        let generation = px.generation();
        let size = (px.w, px.h);
        if size.0 > max_side || size.1 > max_side {
            if store.too_big.insert(key) {
                crate::log_warn!(
                    "mods",
                    "a torch.image of {}x{} is over this GPU's {max_side} a side; not drawn",
                    size.0,
                    size.1
                );
            }
            continue;
        }
        let fresh = store.textures.get(&key).is_none_or(|u| u.size != size);
        if !fresh
            && store
                .textures
                .get(&key)
                .is_some_and(|u| u.generation == generation)
        {
            continue;
        }
        if fresh {
            let texture = device.create_texture(&TextureDescriptor {
                label: Some("mod_image"),
                size: Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            });
            views.0.insert(
                key,
                (
                    texture.create_view(&TextureViewDescriptor::default()),
                    sampler.clone(),
                ),
            );
            store.textures.insert(
                key,
                Uploaded {
                    texture,
                    size,
                    generation,
                },
            );
        }
        let Some(up) = store.textures.get_mut(&key) else {
            continue;
        };
        let rgba = px.read();
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &up.texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &rgba[..],
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size.0 * 4),
                rows_per_image: Some(size.1),
            },
            Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
        );
        up.generation = generation;
    }
}

pub fn build(app: &mut App) {
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app.init_resource::<ModTextures>().add_systems(
        Render,
        upload_mod_images.in_set(RenderSystems::PrepareResources),
    );
}
