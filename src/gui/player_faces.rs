use std::collections::HashMap;

use bevy::prelude::*;

use super::atlas::{PLAYER_FACE_SLOTS, SKIN_HAT_U, SKIN_HEAD_SIZE, SKIN_HEAD_U, SKIN_HEAD_V};
use super::atlas_writes::AtlasWrites;

#[derive(Resource, Default)]
pub struct PlayerFaces {
    slots: HashMap<u128, u8>,
    held: Vec<Option<(u128, u64)>>,
    written: HashMap<u128, std::sync::Arc<str>>,
    tick: u64,
    seen_list: usize,
    seen_textures: u32,
}

impl PlayerFaces {
    pub fn slot_of(&mut self, uuid: u128) -> Option<usize> {
        let slot = *self.slots.get(&uuid)?;
        if let Some(entry) = self.held.get_mut(slot as usize)
            && let Some((_, seen)) = entry
        {
            *seen = self.tick;
        }
        Some(slot as usize)
    }

    pub fn begin_frame(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    fn claim(&mut self, uuid: u128) -> usize {
        if self.held.len() < PLAYER_FACE_SLOTS {
            self.held.resize(PLAYER_FACE_SLOTS, None);
        }
        if let Some(&slot) = self.slots.get(&uuid) {
            return slot as usize;
        }
        let free = self.held.iter().position(Option::is_none);
        let slot = match free {
            Some(slot) => slot,
            None => {
                let (slot, victim) = self
                    .held
                    .iter()
                    .enumerate()
                    .filter_map(|(i, e)| e.map(|(uuid, seen)| (i, (uuid, seen))))
                    .min_by_key(|(_, (_, seen))| *seen)
                    .map(|(i, (uuid, _))| (i, uuid))
                    .unwrap_or((0, 0));
                self.slots.remove(&victim);
                self.written.remove(&victim);
                slot
            }
        };
        self.slots.insert(uuid, slot as u8);
        self.held[slot] = Some((uuid, self.tick));
        slot
    }

    pub fn reset(&mut self) {
        self.slots.clear();
        self.written.clear();
        self.held.clear();
        self.seen_list = 0;
        self.seen_textures = 0;
    }
}

pub fn update_player_faces(
    shared: Res<crate::renderer::systems::Shared>,
    assets: Option<Res<super::render::GuiAssets>>,
    skins: Res<crate::renderer::skin::SkinTextures>,
    images: Res<Assets<Image>>,
    mut faces: ResMut<PlayerFaces>,
    mut writes: ResMut<AtlasWrites>,
) {
    let Some(assets) = assets else { return };

    let generation = skins.generation();
    let listed: Vec<u128> = {
        let s = shared.0.lock().unwrap();
        let list = &s.session.tab_list;
        let id = std::sync::Arc::as_ptr(list) as usize;
        if (id == faces.seen_list && generation == faces.seen_textures) || list.rows.is_empty() {
            return;
        }
        faces.seen_list = id;
        faces.seen_textures = generation;
        list.rows.iter().map(|row| row.uuid).collect()
    };

    for uuid in listed {
        let Some(url) = crate::client::skins::of(uuid).body else {
            continue;
        };
        if faces.written.get(&uuid).is_some_and(|w| *w == url) {
            continue;
        }
        let Some(handle) = skins.get(&url) else {
            continue;
        };
        let Some(image) = images.get(handle) else {
            continue;
        };
        let slot = faces.claim(uuid);
        let Some((face, hat)) = assets.atlas.player_face(slot) else {
            continue;
        };
        let Some(face_px) = cut(image, SKIN_HEAD_U, SKIN_HEAD_V) else {
            continue;
        };
        let Some(hat_px) = cut(image, SKIN_HAT_U, SKIN_HEAD_V) else {
            continue;
        };
        writes.push(face, face_px);
        writes.push(hat, hat_px);
        faces.written.insert(uuid, url);
    }
}

fn cut(image: &Image, x: u32, y: u32) -> Option<Vec<u8>> {
    let size = image.texture_descriptor.size;
    if x + SKIN_HEAD_SIZE > size.width || y + SKIN_HEAD_SIZE > size.height {
        return None;
    }
    let data = image.data.as_ref()?;
    let stride = size.width as usize * 4;
    let mut out = Vec::with_capacity((SKIN_HEAD_SIZE * SKIN_HEAD_SIZE * 4) as usize);
    for row in 0..SKIN_HEAD_SIZE {
        let start = (y + row) as usize * stride + x as usize * 4;
        let end = start + SKIN_HEAD_SIZE as usize * 4;
        out.extend_from_slice(data.get(start..end)?);
    }
    Some(out)
}
