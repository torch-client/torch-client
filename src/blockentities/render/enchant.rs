use std::f32::consts::PI;

use azalea_registry::builtin::BlockEntityKind;
use bevy::prelude::*;

use crate::blockentities::models;
use crate::blockentities::sim::Anim;
use crate::blockentities::sim::book::Book;
use crate::blockentities::{BeRegistry, BeSpec, BeState, GeomKey};
use crate::entities::geom::{BakedModel, LayerDef, PartState};
use crate::util::mth::{frac, lerp, wrap_rad};

pub fn register(registry: &mut BeRegistry) {
    registry.add(
        BlockEntityKind::EnchantingTable,
        BeSpec::model("enchanting_table_book", key, layer, texture, setup)
            .with_transform(transform),
    );
}

fn key(_st: &BeState) -> Option<u64> {
    Some(GeomKey::new().finish())
}

pub fn layer(_st: &BeState) -> LayerDef {
    models::book()
}

pub fn texture(_st: &BeState) -> String {
    "entity/enchantment/enchanting_table_book".to_string()
}

pub fn pose_book(
    model: &BakedModel,
    parts: &mut [PartState],
    openness: f32,
    page_flip_1: f32,
    page_flip_2: f32,
) {
    let x = openness.sin();
    parts[model.id("left_lid")].y_rot = PI + openness;
    parts[model.id("right_lid")].y_rot = -openness;
    for (part, y_rot) in [
        ("left_pages", openness),
        ("right_pages", -openness),
        ("flip_page1", openness - openness * 2.0 * page_flip_1),
        ("flip_page2", openness - openness * 2.0 * page_flip_2),
    ] {
        let state = &mut parts[model.id(part)];
        state.y_rot = y_rot;
        state.x = x;
    }
}

fn book(st: &BeState) -> Book {
    match &st.anim {
        Anim::Book(book) => *book,
        _ => Book::default(),
    }
}

fn page_flip(flip: f32, offset: f32) -> f32 {
    (frac(flip + offset) * 1.6 - 0.3).clamp(0.0, 1.0)
}

fn openness(time: f32, open: f32) -> f32 {
    ((time * 0.02).sin() * 0.1 + 1.25) * open
}

fn setup(model: &BakedModel, parts: &mut [PartState], st: &BeState) {
    let book = book(st);
    let flip = lerp(st.partial, book.o_flip, book.flip);
    let open = lerp(st.partial, book.o_open, book.open);
    let time = book.time as f32 + st.partial;
    pose_book(
        model,
        parts,
        openness(time, open),
        page_flip(flip, 0.25),
        page_flip(flip, 0.75),
    );
}

fn transform(st: &BeState) -> Transform {
    let book = book(st);
    let time = book.time as f32 + st.partial;
    let delta = wrap_rad(book.rot - book.o_rot);
    let y_rot = book.o_rot + delta * st.partial;
    Transform {
        translation: Vec3::new(0.5, 0.85 + (time * 0.1).sin() * 0.01, 0.5),
        rotation: Quat::from_rotation_y(-y_rot) * Quat::from_rotation_z(80.0_f32.to_radians()),
        scale: Vec3::ONE,
    }
}

#[cfg(test)]
pub(super) fn test_states(out: &mut Vec<BeState>) {
    out.push(super::test_state("enchanting_table", &[]));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_flips_stay_in_the_unit_range() {
        for i in 0..200 {
            let flip = i as f32 * 0.137 - 5.0;
            for offset in [0.25, 0.75] {
                assert!((0.0..=1.0).contains(&page_flip(flip, offset)));
            }
        }
    }

    #[test]
    fn a_closed_book_stays_closed() {
        assert_eq!(openness(123.0, 0.0), 0.0);
    }

    #[test]
    fn the_book_texture_is_on_disk() {
        let st = super::super::test_state("enchanting_table", &[]);
        let file = crate::assets_root().join(format!("textures/{}.png", texture(&st)));
        assert!(file.exists());
    }
}
