use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;

use crate::entities::geom;
use crate::entities::quads::QuadMesh;

pub const SKIN_W: f32 = 64.0;
pub const SKIN_H: f32 = 64.0;

pub const SKIN_SHEET: [f32; 2] = [SKIN_W, SKIN_H];

#[cfg(feature = "skins")]
pub const CAPE_SHEET: [f32; 2] = [64.0, 32.0];

pub const MODEL_TO_BLOCKS: f32 = 1.0 / 16.0;

pub(crate) const DEFAULT_SKIN: &str = "textures/entity/player/wide/steve.png";

pub fn box_mesh(
    sheet: [f32; 2],
    tex: [f32; 2],
    origin: [f32; 3],
    size: [f32; 3],
    grow: f32,
    scale: f32,
    color: Option<[f32; 4]>,
) -> Mesh {
    let cube = geom::CubeDef {
        tex,
        origin,
        size,
        grow: geom::Grow::all(grow),
        mirror: false,
        tex_scale: [1.0, 1.0],
        faces: geom::Faces::ALL,
    };
    let mut out = QuadMesh::new();
    geom::push_cube(&mut out, &cube, sheet, scale);
    let mut mesh = out.into_mesh(RenderAssetUsages::default(), false);
    if let Some(color) = color {
        let n = mesh.count_vertices();
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![color; n]);
    }
    mesh
}

#[cfg(feature = "skins")]
pub fn default_skin_path(index: u8) -> String {
    match crate::gui::tablist::DEFAULT_SKINS.get(index as usize) {
        Some(skin) => format!("textures/entity/player/{}.png", skin.path),
        None => DEFAULT_SKIN.to_owned(),
    }
}

pub fn load_default_skin(images: &mut Assets<Image>, who: &str) -> Option<Handle<Image>> {
    load_texture(images, DEFAULT_SKIN, who)
}

pub fn load_texture(images: &mut Assets<Image>, path: &str, who: &str) -> Option<Handle<Image>> {
    let path = crate::assets_root().join(path);
    let decoded = match crate::platform::assets::open_image(&path) {
        Ok(img) => img.to_rgba8(),
        Err(e) => {
            error!("{who}: cannot read {}: {e}", path.display());
            return None;
        }
    };

    let handle = images.add(super::rgba_image(
        decoded.width(),
        decoded.height(),
        decoded.into_raw(),
        RenderAssetUsages::default(),
    ));
    super::reload::track(&handle, move || {
        Some(crate::platform::assets::open_image(&path).ok()?.to_rgba8())
    });
    Some(handle)
}

#[cfg(feature = "skins")]
#[derive(Resource, Default)]
pub struct SkinTextures {
    downloaded: std::collections::HashMap<std::sync::Arc<str>, Handle<Image>>,
    defaults: std::collections::HashMap<u8, Option<Handle<Image>>>,
    generation: u32,
}

#[cfg(feature = "skins")]
impl SkinTextures {
    pub fn get(&self, url: &str) -> Option<&Handle<Image>> {
        self.downloaded.get(url)
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }

    pub fn default_skin(&mut self, index: u8, images: &mut Assets<Image>) -> Option<Handle<Image>> {
        if let Some(cached) = self.defaults.get(&index) {
            return cached.clone();
        }
        let handle = load_texture(images, &default_skin_path(index), "player model");
        self.defaults.insert(index, handle.clone());
        handle
    }
}

#[cfg(feature = "skins")]
pub fn upload_skins(mut textures: ResMut<SkinTextures>, mut images: ResMut<Assets<Image>>) {
    for (url, _kind, image) in crate::client::skins::take_ready() {
        let (w, h) = image.dimensions();
        let handle = images.add(super::rgba_image(
            w,
            h,
            image.into_raw(),
            RenderAssetUsages::default(),
        ));
        textures.downloaded.insert(url, handle);
        textures.generation = textures.generation.wrapping_add(1);
    }
}
