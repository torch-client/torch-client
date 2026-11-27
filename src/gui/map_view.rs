use bevy::math::Vec2;
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::texture::GpuImage;

use super::draw::GuiImageViews;
use super::painter::Painter;
use super::pool::GuiPool;
use crate::renderer::maps;

const MAP_KEY_BASE: u64 = 1 << 62;
const MAP_KEY_ID_BITS: u64 = 0xFFFF_FFFF;

pub fn map_key(id: i32) -> u64 {
    MAP_KEY_BASE | u64::from(id as u32)
}

fn map_of_key(key: u64) -> Option<i32> {
    (key & !MAP_KEY_ID_BITS == MAP_KEY_BASE).then_some((key & MAP_KEY_ID_BITS) as u32 as i32)
}

#[cfg_attr(
    not(lua_mods),
    allow(
        dead_code,
        reason = "only `gui::mod_images` asks, and it is `lua_mods` only"
    )
)]
pub fn is_map_key(key: u64) -> bool {
    map_of_key(key).is_some()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapMeta {
    pub scale: u8,
    pub locked: bool,
}

pub fn meta(id: i32) -> Option<MapMeta> {
    crate::client::tracking::map_meta(id).map(|(scale, locked)| MapMeta { scale, locked })
}

#[allow(
    dead_code,
    reason = "offered to the tooltip's line cache, which may not key on it"
)]
pub fn revision(id: i32) -> u32 {
    maps::revision(id)
}

pub fn draw(p: &mut Painter, id: i32, x: f32, y: f32, size: f32) -> bool {
    let Some((decorations, _)) = maps::gui_map(id) else {
        return false;
    };
    p.image(
        map_key(id),
        x,
        y,
        size,
        size,
        [0.0, 0.0, 1.0, 1.0],
        0xFFFF_FFFF,
    );
    let k = size / crate::client::tracking::MAP_EDGE as f32;
    let on_frame = || {
        decorations
            .iter()
            .filter(|d| maps::shows_on_item_frame(d.kind))
    };

    if let Some(sheet) = p.atlas.sheet(maps::DECORATION_ATLAS) {
        for decoration in on_frame() {
            let Some((u0, u1)) = maps::decoration_uv(decoration.kind) else {
                continue;
            };
            let pose = maps::marker_pose(decoration);
            let corner = |cx: f32, cy: f32| {
                let m = pose.transform_point2(Vec2::new(cx, cy));
                [x + m.x * k, y + m.y * k]
            };
            p.region_quad(
                sheet,
                [
                    corner(-1.0, -1.0),
                    corner(1.0, -1.0),
                    corner(1.0, 1.0),
                    corner(-1.0, 1.0),
                ],
                [[u0, 1.0], [u1, 1.0], [u1, 0.0], [u0, 0.0]],
                [1.0; 4],
            );
        }
    }

    for decoration in on_frame() {
        let Some(name) = decoration.name.as_deref() else {
            continue;
        };
        let width = p.atlas.font.width(name);
        let (lx, ly, scale) = maps::label_place(decoration, width);
        p.scaled_in(scale * k, x + lx * k, y + ly * k, |p| {
            maps::paint_label_plate(p, width);
            p.text(name, 0.0, 0.0, false);
        });
    }
    true
}

pub(crate) fn bind_map_images(
    pool: Res<GuiPool>,
    images: Res<RenderAssets<GpuImage>>,
    mut views: ResMut<GuiImageViews>,
) {
    views
        .0
        .retain(|&key, _| map_of_key(key).is_none_or(|id| maps::image_id(id).is_some()));
    for range in &pool.ranges {
        let Some(id) = map_of_key(range.texture) else {
            continue;
        };
        let Some(asset) = maps::image_id(id) else {
            continue;
        };
        let Some(gpu) = images.get(asset) else {
            continue;
        };
        if views
            .0
            .get(&range.texture)
            .is_some_and(|(view, _)| view.id() == gpu.texture_view.id())
        {
            continue;
        }
        views.0.insert(
            range.texture,
            (gpu.texture_view.clone(), gpu.sampler.clone()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_keys_are_their_own_range() {
        for id in [0, 1, -1, i32::MAX, i32::MIN, 4096] {
            assert_eq!(map_of_key(map_key(id)), Some(id));
        }
        for key in [
            crate::gui::painter::ATLAS_TEXTURE,
            crate::gui::painter::VIGNETTE_TEXTURE,
            1,
            2,
            1 << 40,
        ] {
            assert!(!is_map_key(key), "{key}");
        }
    }
}
