use bevy::asset::RenderAssetUsages;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use crate::client::tracking::{MAP_BYTES, MAP_EDGE};
use crate::session::MapDecoration;
use crate::util::map_color::rgba_from_packed_id;

pub const MAP_TEXTURE_PREFIX: &str = "map/";

pub const DECORATION_SPRITES: [&str; 35] = [
    "player",
    "frame",
    "red_marker",
    "blue_marker",
    "target_x",
    "target_point",
    "player_off_map",
    "player_off_limits",
    "woodland_mansion",
    "ocean_monument",
    "white_banner",
    "orange_banner",
    "magenta_banner",
    "light_blue_banner",
    "yellow_banner",
    "lime_banner",
    "pink_banner",
    "gray_banner",
    "light_gray_banner",
    "cyan_banner",
    "purple_banner",
    "blue_banner",
    "brown_banner",
    "green_banner",
    "red_banner",
    "black_banner",
    "red_x",
    "desert_village",
    "plains_village",
    "savanna_village",
    "snowy_village",
    "taiga_village",
    "jungle_temple",
    "swamp_hut",
    "trial_chambers",
];

const DECORATION_PX: u32 = 8;

struct MapEntry {
    image: Handle<Image>,
    decorations: std::sync::Arc<Vec<MapDecoration>>,
    revision: u32,
}

#[derive(Resource, Default)]
pub struct MapTextures {
    by_id: HashMap<i32, MapEntry>,
}

impl MapTextures {
    pub fn get(&self, texture: &str) -> Option<Handle<Image>> {
        let id: i32 = texture.strip_prefix(MAP_TEXTURE_PREFIX)?.parse().ok()?;
        self.by_id.get(&id).map(|entry| entry.image.clone())
    }

    pub fn image(&self, id: i32) -> Option<Handle<Image>> {
        self.by_id.get(&id).map(|entry| entry.image.clone())
    }

    pub fn decorations(&self, id: i32) -> Option<(&[MapDecoration], u32)> {
        let entry = self.by_id.get(&id)?;
        Some((&entry.decorations, entry.revision))
    }
}

pub fn build_decoration_sheet(images: &mut Assets<Image>) -> Handle<Image> {
    let count = DECORATION_SPRITES.len() as u32;
    let width = count * DECORATION_PX;
    let mut pixels = vec![0u8; (width * DECORATION_PX * 4) as usize];
    let dir = crate::assets_root().join("textures/map/decorations");
    for (index, name) in DECORATION_SPRITES.iter().enumerate() {
        let path = dir.join(format!("{name}.png"));
        let decoded = match crate::platform::assets::open_gui_image(&path) {
            Ok(image) => image,
            Err(e) => {
                crate::log_warn!("maps", "map decoration {name}: {e}");
                continue;
            }
        };
        if decoded.width() != DECORATION_PX || decoded.height() != DECORATION_PX {
            crate::log_warn!(
                "maps",
                "map decoration {name} is {}x{}, expected {DECORATION_PX} square",
                decoded.width(),
                decoded.height()
            );
            continue;
        }
        let left = index as u32 * DECORATION_PX;
        for y in 0..DECORATION_PX {
            let src = (y * DECORATION_PX * 4) as usize;
            let dst = ((y * width + left) * 4) as usize;
            let row = (DECORATION_PX * 4) as usize;
            pixels[dst..dst + row].copy_from_slice(&decoded.as_raw()[src..src + row]);
        }
    }
    images.add(crate::renderer::rgba_image(
        width,
        DECORATION_PX,
        pixels,
        RenderAssetUsages::RENDER_WORLD,
    ))
}

pub fn decoration_uv(kind: u8) -> Option<(f32, f32)> {
    let count = DECORATION_SPRITES.len() as f32;
    let index = kind as usize;
    (index < DECORATION_SPRITES.len()).then(|| (index as f32 / count, (index as f32 + 1.0) / count))
}

pub fn upload_maps(
    shared: Res<crate::renderer::Shared>,
    mut images: ResMut<Assets<Image>>,
    mut maps: ResMut<MapTextures>,
) {
    let (updates, cleared) = {
        let mut s = shared.0.lock().unwrap();
        if s.session.map_updates.is_empty() && !s.session.clear_maps {
            return;
        }
        (
            std::mem::take(&mut s.session.map_updates),
            std::mem::take(&mut s.session.clear_maps),
        )
    };
    if cleared {
        maps.by_id.clear();
    }
    for update in updates {
        if update.colors.len() != MAP_BYTES {
            continue;
        }
        let mut pixels = Vec::with_capacity(MAP_BYTES * 4);
        for packed in update.colors.iter() {
            pixels.extend_from_slice(&rgba_from_packed_id(*packed));
        }
        let existing = maps.by_id.get(&update.id).map(|entry| entry.image.clone());
        match existing.as_ref().and_then(|h| images.get_mut(h)) {
            Some(image) => image.data = Some(pixels),
            None => {
                let usage = RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD;
                let handle = images.add(crate::renderer::rgba_image(
                    MAP_EDGE as u32,
                    MAP_EDGE as u32,
                    pixels,
                    usage,
                ));
                maps.by_id.insert(
                    update.id,
                    MapEntry {
                        image: handle,
                        decorations: std::sync::Arc::default(),
                        revision: 0,
                    },
                );
            }
        }
        if let Some(decorations) = update.decorations
            && let Some(entry) = maps.by_id.get_mut(&update.id)
        {
            entry.decorations = decorations;
            entry.revision = entry.revision.wrapping_add(1);
        }
    }
}
