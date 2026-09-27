use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

pub const SKIN_W: f32 = 64.0;
pub const SKIN_H: f32 = 64.0;

pub const SKIN_SHEET: [f32; 2] = [SKIN_W, SKIN_H];

#[cfg(feature = "skins")]
pub const CAPE_SHEET: [f32; 2] = [64.0, 32.0];

pub const MODEL_TO_BLOCKS: f32 = 1.0 / 16.0;

const DEFAULT_SKIN: &str = "textures/entity/player/wide/steve.png";

pub fn box_mesh(
    sheet: [f32; 2],
    tex: [f32; 2],
    origin: [f32; 3],
    size: [f32; 3],
    grow: f32,
    scale: f32,
    color: Option<[f32; 4]>,
) -> Mesh {
    let (w, h, d) = (size[0], size[1], size[2]);
    let min_x = (origin[0] - grow) * scale;
    let min_y = (origin[1] - grow) * scale;
    let min_z = (origin[2] - grow) * scale;
    let max_x = (origin[0] + w + grow) * scale;
    let max_y = (origin[1] + h + grow) * scale;
    let max_z = (origin[2] + d + grow) * scale;

    let t0 = [min_x, min_y, min_z];
    let t1 = [max_x, min_y, min_z];
    let t2 = [max_x, max_y, min_z];
    let t3 = [min_x, max_y, min_z];
    let l0 = [min_x, min_y, max_z];
    let l1 = [max_x, min_y, max_z];
    let l2 = [max_x, max_y, max_z];
    let l3 = [min_x, max_y, max_z];

    let u0 = tex[0];
    let u1 = u0 + d;
    let u2 = u0 + d + w;
    let u22 = u0 + d + w + w;
    let u3 = u0 + d + w + d;
    let u4 = u0 + d + w + d + w;
    let v0 = tex[1];
    let v1 = v0 + d;
    let v2 = v0 + d + h;

    let mut pos: Vec<[f32; 3]> = Vec::with_capacity(24);
    let mut nrm: Vec<[f32; 3]> = Vec::with_capacity(24);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(24);
    let mut idx: Vec<u32> = Vec::with_capacity(36);

    let mut face =
        |corners: [[f32; 3]; 4], normal: [f32; 3], ua: f32, va: f32, ub: f32, vb: f32| {
            let base = pos.len() as u32;
            let quad_uv = [
                [ub / sheet[0], va / sheet[1]],
                [ua / sheet[0], va / sheet[1]],
                [ua / sheet[0], vb / sheet[1]],
                [ub / sheet[0], vb / sheet[1]],
            ];
            for i in 0..4 {
                pos.push(corners[i]);
                nrm.push(normal);
                uvs.push(quad_uv[i]);
            }
            idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        };

    face([l1, l0, t0, t1], [0.0, -1.0, 0.0], u1, v0, u2, v1);
    face([t2, t3, l3, l2], [0.0, 1.0, 0.0], u2, v1, u22, v0);
    face([t0, l0, l3, t3], [-1.0, 0.0, 0.0], u0, v1, u1, v2);
    face([t1, t0, t3, t2], [0.0, 0.0, -1.0], u1, v1, u2, v2);
    face([l1, t1, t2, l2], [1.0, 0.0, 0.0], u2, v1, u3, v2);
    face([l0, l1, l2, l3], [0.0, 0.0, 1.0], u3, v1, u4, v2);

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nrm);

    if let Some(color) = color {
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![color; uvs.len()]);
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(idx));
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

    Some(images.add(super::rgba_image(
        decoded.width(),
        decoded.height(),
        decoded.into_raw(),
        RenderAssetUsages::default(),
    )))
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
