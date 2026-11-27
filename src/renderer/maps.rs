use bevy::asset::RenderAssetUsages;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use std::sync::Arc;

use crate::client::tracking::{MAP_BYTES, MAP_EDGE};
use crate::session::{MapDecoration, SideMutex};
use crate::util::global;
use crate::util::map_color::PACKED_RGBA;
use crate::util::mth::DEG_TO_RAD;

pub const MAP_TEXTURE_PREFIX: &str = "map/";

pub const DECORATION_ATLAS: &str = "map_decorations";

pub struct DecorationType {
    pub sprite: &'static str,
    pub on_frame: bool,
}

const fn decoration(sprite: &'static str, on_frame: bool) -> DecorationType {
    DecorationType { sprite, on_frame }
}

pub const DECORATION_TYPES: [DecorationType; 35] = [
    decoration("player", false),
    decoration("frame", true),
    decoration("red_marker", false),
    decoration("blue_marker", false),
    decoration("target_x", true),
    decoration("target_point", true),
    decoration("player_off_map", false),
    decoration("player_off_limits", false),
    decoration("woodland_mansion", true),
    decoration("ocean_monument", true),
    decoration("white_banner", true),
    decoration("orange_banner", true),
    decoration("magenta_banner", true),
    decoration("light_blue_banner", true),
    decoration("yellow_banner", true),
    decoration("lime_banner", true),
    decoration("pink_banner", true),
    decoration("gray_banner", true),
    decoration("light_gray_banner", true),
    decoration("cyan_banner", true),
    decoration("purple_banner", true),
    decoration("blue_banner", true),
    decoration("brown_banner", true),
    decoration("green_banner", true),
    decoration("red_banner", true),
    decoration("black_banner", true),
    decoration("red_x", true),
    decoration("desert_village", true),
    decoration("plains_village", true),
    decoration("savanna_village", true),
    decoration("snowy_village", true),
    decoration("taiga_village", true),
    decoration("jungle_temple", true),
    decoration("swamp_hut", true),
    decoration("trial_chambers", true),
];

pub fn shows_on_item_frame(kind: u8) -> bool {
    DECORATION_TYPES
        .get(kind as usize)
        .is_some_and(|t| t.on_frame)
}

const DECORATION_PX: u32 = 8;

const MAP_EDGE_PIXELS: f32 = MAP_EDGE as f32;

const DECORATION_Z: f32 = -0.02;
const DECORATION_Z_OFFSET: f32 = -0.001;

const DECORATION_SCALE: [f32; 3] = [4.0, 4.0, 3.0];

pub fn marker_pose(decoration: &MapDecoration) -> bevy::math::Affine2 {
    use bevy::math::{Affine2, Vec2};
    let turn = decoration.rot as f32 * 360.0 / 16.0;
    Affine2::from_translation(Vec2::new(
        decoration.x as f32 / 2.0 + MAP_EDGE_PIXELS / 2.0,
        decoration.y as f32 / 2.0 + MAP_EDGE_PIXELS / 2.0,
    )) * Affine2::from_angle(turn * DEG_TO_RAD)
        * Affine2::from_scale(Vec2::new(DECORATION_SCALE[0], DECORATION_SCALE[1]))
        * Affine2::from_translation(Vec2::new(-0.125, 0.125))
}

struct MapEntry {
    image: Handle<Image>,
    decorations: Arc<Vec<MapDecoration>>,
    markers: u32,
    revision: u32,
}

global! {
fn entries() -> SideMutex<HashMap<i32, MapEntry>>
}

#[derive(Resource, Default)]
pub struct MapTextures {
    sheet: Option<Handle<Image>>,
}

impl MapTextures {
    pub fn get(&self, texture: &str) -> Option<Handle<Image>> {
        let id: i32 = texture.strip_prefix(MAP_TEXTURE_PREFIX)?.parse().ok()?;
        self.image(id)
    }

    pub fn image(&self, id: i32) -> Option<Handle<Image>> {
        entries()
            .lock()
            .unwrap()
            .get(&id)
            .map(|entry| entry.image.clone())
    }

    pub fn decorations(&self, id: i32) -> Option<(Arc<Vec<MapDecoration>>, u32)> {
        let entries = entries().lock().unwrap();
        let entry = entries.get(&id)?;
        Some((entry.decorations.clone(), entry.markers))
    }

    pub fn decoration_sheet(&mut self, images: &mut Assets<Image>) -> Handle<Image> {
        self.sheet
            .get_or_insert_with(|| build_decoration_sheet(images))
            .clone()
    }

    pub fn sheet(&self) -> Option<Handle<Image>> {
        self.sheet.clone()
    }
}

pub fn gui_map(id: i32) -> Option<(Arc<Vec<MapDecoration>>, u32)> {
    let entries = entries().lock().unwrap();
    let entry = entries.get(&id)?;
    Some((entry.decorations.clone(), entry.revision))
}

pub fn image_id(id: i32) -> Option<AssetId<Image>> {
    entries()
        .lock()
        .unwrap()
        .get(&id)
        .map(|entry| entry.image.id())
}

pub fn revision(id: i32) -> u32 {
    entries()
        .lock()
        .unwrap()
        .get(&id)
        .map_or(0, |entry| entry.revision)
}

fn build_decoration_sheet(images: &mut Assets<Image>) -> Handle<Image> {
    let sheet = decoration_pixels(crate::assets_root());
    let handle = images.add(crate::renderer::rgba_image(
        sheet.width(),
        sheet.height(),
        sheet.into_raw(),
        RenderAssetUsages::RENDER_WORLD,
    ));
    crate::renderer::reload::track(&handle, || Some(decoration_pixels(crate::assets_root())));
    handle
}

pub(crate) fn decoration_pixels(assets: &std::path::Path) -> image::RgbaImage {
    let count = DECORATION_TYPES.len() as u32;
    let width = count * DECORATION_PX;
    let mut pixels = vec![0u8; (width * DECORATION_PX * 4) as usize];
    let dir = assets.join("textures/map/decorations");
    for (index, name) in DECORATION_TYPES.iter().map(|t| t.sprite).enumerate() {
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
    image::RgbaImage::from_raw(width, DECORATION_PX, pixels).expect("sized to the sheet")
}

pub fn decoration_uv(kind: u8) -> Option<(f32, f32)> {
    let count = DECORATION_TYPES.len() as f32;
    let index = kind as usize;
    (index < DECORATION_TYPES.len()).then(|| (index as f32 / count, (index as f32 + 1.0) / count))
}

pub fn decoration_mesh(decorations: &[MapDecoration]) -> Mesh {
    let mut out = crate::entities::quads::QuadMesh::new();
    for (count, decoration) in decorations.iter().enumerate() {
        let Some((u0, u1)) = decoration_uv(decoration.kind) else {
            continue;
        };
        let pose = marker_pose(decoration);
        let z = DECORATION_Z + DECORATION_SCALE[2] * count as f32 * DECORATION_Z_OFFSET;
        let corner = |x: f32, y: f32| {
            let p = pose.transform_point2(Vec2::new(x, y));
            [p.x, p.y, z]
        };
        out.quad(
            [
                corner(-1.0, 1.0),
                corner(1.0, 1.0),
                corner(1.0, -1.0),
                corner(-1.0, -1.0),
            ],
            [[u0, 0.0], [u1, 0.0], [u1, 1.0], [u0, 1.0]],
            [0.0, 0.0, 1.0],
        );
    }
    out.finish()
}

const LABEL_GLYPH_Z: f32 = -0.025 - 0.1;
const LABEL_PLATE_Z: f32 = -0.025 - 0.1 + 0.01;
const LABEL_BACKGROUND: u32 = 0x8000_0000;
const LABEL_MAX_SCALE: f32 = 6.0 / 9.0;
const LABEL_DROP: f32 = 4.0;

pub fn label_place(decoration: &MapDecoration, width: f32) -> (f32, f32, f32) {
    let scale = (25.0 / width).clamp(0.0, LABEL_MAX_SCALE);
    let x = decoration.x as f32 / 2.0 + MAP_EDGE_PIXELS / 2.0 - width * scale / 2.0;
    let y = decoration.y as f32 / 2.0 + MAP_EDGE_PIXELS / 2.0 + LABEL_DROP;
    (x, y, scale)
}

pub fn paint_label_plate(painter: &mut crate::gui::painter::Painter, width: f32) {
    painter.fill(
        -1.0,
        -1.0,
        width + 1.0,
        crate::text::LINE_HEIGHT + 1.0,
        LABEL_BACKGROUND,
    );
}

pub fn paint_labels(
    painter: &mut crate::gui::painter::Painter,
    decorations: &[MapDecoration],
    pose: Mat4,
) {
    for decoration in decorations {
        let Some(name) = decoration.name.as_deref() else {
            continue;
        };
        let width = painter.atlas.font.width(name);
        let (x, y, scale) = label_place(decoration, width);
        let plate = painter.positions.len();
        paint_label_plate(painter, width);
        let glyphs = painter.positions.len();
        painter.text(name, 0.0, 0.0, false);
        for (i, position) in painter.positions[plate..].iter_mut().enumerate() {
            let z = if plate + i < glyphs {
                LABEL_PLATE_Z
            } else {
                LABEL_GLYPH_Z
            };
            let local = Vec3::new(x + position[0] * scale, y + position[1] * scale, z);
            *position = pose.transform_point3(local).to_array();
        }
    }
}

pub fn upload_maps(
    shared: Res<crate::renderer::Shared>,
    mut images: ResMut<Assets<Image>>,
    mut maps: ResMut<MapTextures>,
) {
    if maps.sheet.is_none() {
        maps.decoration_sheet(&mut images);
    }
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
        entries().lock().unwrap().clear();
    }
    for update in updates {
        let existing = entries()
            .lock()
            .unwrap()
            .get(&update.id)
            .map(|entry| entry.image.clone())
            .filter(|h| images.contains(h));
        if existing.is_none() || !update.dirty.is_empty() {
            let Some(pixels) = crate::client::tracking::with_map_colors(update.id, map_pixels)
            else {
                continue;
            };
            match existing {
                Some(handle) => {
                    if let Some(mut image) = images.get_mut(&handle) {
                        image.data = Some(pixels);
                    }
                }
                None => {
                    let handle = images.add(crate::renderer::rgba_image(
                        MAP_EDGE as u32,
                        MAP_EDGE as u32,
                        pixels,
                        RenderAssetUsages::RENDER_WORLD,
                    ));
                    entries()
                        .lock()
                        .unwrap()
                        .entry(update.id)
                        .and_modify(|entry| entry.image = handle.clone())
                        .or_insert_with(|| MapEntry {
                            image: handle,
                            decorations: Arc::default(),
                            markers: 0,
                            revision: 0,
                        });
                }
            }
        }
        let mut entries = entries().lock().unwrap();
        let Some(entry) = entries.get_mut(&update.id) else {
            continue;
        };
        entry.revision = entry.revision.wrapping_add(1);
        if let Some(decorations) = update.decorations {
            entry.decorations = decorations;
            entry.markers = entry.markers.wrapping_add(1);
        }
    }
}

fn map_pixels(colors: &[u8; MAP_BYTES]) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(MAP_BYTES * 4);
    for &packed in colors.iter() {
        pixels.extend_from_slice(&PACKED_RGBA[packed as usize]);
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marker(kind: u8) -> MapDecoration {
        MapDecoration {
            kind,
            x: 0,
            y: 0,
            rot: 0,
            name: None,
        }
    }

    #[test]
    fn decoration_mesh_has_one_quad_per_marker() {
        assert_eq!(decoration_mesh(&[]).count_vertices(), 0);
        assert_eq!(decoration_mesh(&[marker(0)]).count_vertices(), 4);
        assert_eq!(
            decoration_mesh(&[marker(0), marker(34)]).count_vertices(),
            8
        );
        assert_eq!(decoration_mesh(&[marker(35)]).count_vertices(), 0);
    }

    #[test]
    fn decoration_sits_at_the_map_centre() {
        let mesh = decoration_mesh(&[marker(0)]);
        let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(bevy::mesh::VertexAttributeValues::Float32x3(v)) => v.clone(),
            _ => panic!("no positions"),
        };
        let xs: Vec<f32> = positions.iter().map(|p| p[0]).collect();
        let (min, max) = (
            xs.iter().copied().fold(f32::MAX, f32::min),
            xs.iter().copied().fold(f32::MIN, f32::max),
        );
        assert!((max - min - 8.0).abs() < 1e-5, "{min}..{max}");
        assert!(((min + max) / 2.0 - (MAP_EDGE_PIXELS / 2.0 - 0.5)).abs() < 1e-5);
    }

    #[test]
    fn frame_filter_follows_the_registry() {
        for (kind, on_frame) in [
            (0, false),
            (1, true),
            (2, false),
            (3, false),
            (6, false),
            (10, true),
            (34, true),
            (35, false),
        ] {
            assert_eq!(shows_on_item_frame(kind), on_frame, "kind {kind}");
        }
    }
}
