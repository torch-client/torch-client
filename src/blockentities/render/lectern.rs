use std::f32::consts::PI;

use azalea_registry::builtin::BlockEntityKind;
use bevy::prelude::*;

use crate::blockentities::models;
use crate::blockentities::render::chest::facing_y_rot;
use crate::blockentities::{BeRegistry, BeSpec, BeState, GeomKey};
use crate::entities::geom::{BakedModel, LayerDef, PartState};

pub fn register(registry: &mut BeRegistry) {
    registry.add(
        BlockEntityKind::Lectern,
        BeSpec::model("lectern_book", key, layer, texture, setup).with_transform(transform),
    );
}

const OPENNESS: f32 = 1.5;
const PAGE_FLIP_1: f32 = 0.1;
const PAGE_FLIP_2: f32 = 0.9;

fn key(st: &BeState) -> Option<u64> {
    (st.state.prop("has_book") == "true").then(|| GeomKey::new().finish())
}

fn layer(_st: &BeState) -> LayerDef {
    models::book()
}

fn texture(_st: &BeState) -> String {
    "entity/enchantment/enchanting_table_book".to_string()
}

fn setup(model: &BakedModel, parts: &mut [PartState], _st: &BeState) {
    let x = OPENNESS.sin();
    parts[model.id("left_lid")].y_rot = PI + OPENNESS;
    parts[model.id("right_lid")].y_rot = -OPENNESS;
    for (part, y_rot) in [
        ("left_pages", OPENNESS),
        ("right_pages", -OPENNESS),
        ("flip_page1", OPENNESS - OPENNESS * 2.0 * PAGE_FLIP_1),
        ("flip_page2", OPENNESS - OPENNESS * 2.0 * PAGE_FLIP_2),
    ] {
        let state = &mut parts[model.id(part)];
        state.y_rot = y_rot;
        state.x = x;
    }
}

fn clockwise(facing: &str) -> &'static str {
    match facing {
        "north" => "east",
        "east" => "south",
        "west" => "north",
        _ => "west",
    }
}

fn transform(st: &BeState) -> Transform {
    let y_rot = facing_y_rot(clockwise(st.state.prop("facing")));
    let rotation =
        Quat::from_rotation_y(-y_rot.to_radians()) * Quat::from_rotation_z(67.5_f32.to_radians());
    Transform {
        translation: Vec3::new(0.5, 1.0625, 0.5) + rotation * Vec3::new(0.0, -0.125, 0.0),
        rotation,
        scale: Vec3::ONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_stocked_lectern_draws() {
        let stocked =
            super::super::test_state("lectern", &[("facing", "north"), ("has_book", "true")]);
        let empty =
            super::super::test_state("lectern", &[("facing", "north"), ("has_book", "false")]);
        assert!(key(&stocked).is_some());
        assert!(key(&empty).is_none());
    }

    #[test]
    fn the_book_texture_is_on_disk() {
        let st = super::super::test_state("lectern", &[("facing", "north"), ("has_book", "true")]);
        let file = crate::assets_root().join(format!("textures/{}.png", texture(&st)));
        assert!(file.exists(), "missing {}", texture(&st));
    }

    #[test]
    fn the_book_rests_on_the_stand() {
        for facing in ["north", "south", "east", "west"] {
            let st =
                super::super::test_state("lectern", &[("facing", facing), ("has_book", "true")]);
            let at = transform(&st).translation;
            assert!(at.y > 0.9375, "{facing}: {at:?}");
            assert!(
                (0.0..=1.0).contains(&at.x) && (0.0..=1.0).contains(&at.z),
                "{facing}: {at:?}"
            );
        }
    }
}
