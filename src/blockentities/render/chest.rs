use std::f32::consts::FRAC_PI_2;

use azalea_core::direction::Direction;
use azalea_registry::builtin::BlockEntityKind;
use bevy::prelude::*;

use crate::blockentities::feed::ChestHalf;
use crate::blockentities::models;
use crate::blockentities::{BeRegistry, BeSpec, BeState, GeomKey};
use crate::entities::geom::{BakedModel, LayerDef, PartState};

pub fn register(registry: &mut BeRegistry) {
    registry.add_many(
        &[
            BlockEntityKind::Chest,
            BlockEntityKind::TrappedChest,
            BlockEntityKind::EnderChest,
        ],
        BeSpec::model("chest", key, layer, texture, setup).with_transform(transform),
    );
}

fn chest_type(st: &BeState) -> &str {
    match st.state.chest_half {
        ChestHalf::Left => "left",
        ChestHalf::Right => "right",
        ChestHalf::Single => "single",
    }
}

fn key(st: &BeState) -> Option<u64> {
    Some(
        GeomKey::new()
            .str(material(st))
            .str(chest_type(st))
            .finish(),
    )
}

fn layer(st: &BeState) -> LayerDef {
    match chest_type(st) {
        "left" => models::chest_left(),
        "right" => models::chest_right(),
        _ => models::chest_single(),
    }
}

fn material(st: &BeState) -> &'static str {
    let block = st.state.block.as_str();
    if block.ends_with("copper_chest") {
        return if block.contains("exposed") {
            "copper_exposed"
        } else if block.contains("weathered") {
            "copper_weathered"
        } else if block.contains("oxidized") {
            "copper_oxidized"
        } else {
            "copper"
        };
    }
    if block == "ender_chest" {
        return "ender";
    }
    if is_extended_christmas() {
        return "christmas";
    }
    if block == "trapped_chest" {
        "trapped"
    } else {
        "normal"
    }
}

fn texture(st: &BeState) -> String {
    let material = material(st);
    if material == "ender" {
        return "entity/chest/ender".to_string();
    }
    match chest_type(st) {
        "left" => format!("entity/chest/{material}_left"),
        "right" => format!("entity/chest/{material}_right"),
        _ => format!("entity/chest/{material}"),
    }
}

fn transform(st: &BeState) -> Transform {
    let rotation = Quat::from_rotation_y(-facing_y_rot(st.state.facing).to_radians());
    let pivot = Vec3::new(0.5, 0.0, 0.5);
    Transform {
        translation: pivot - rotation * pivot,
        rotation,
        scale: Vec3::ONE,
    }
}

pub fn facing_y_rot(facing: Option<Direction>) -> f32 {
    match facing {
        Some(Direction::West) => 90.0,
        Some(Direction::North) => 180.0,
        Some(Direction::East) => 270.0,
        _ => 0.0,
    }
}

fn setup(model: &BakedModel, parts: &mut [PartState], st: &BeState) {
    let open = 1.0 - st.open;
    let open = 1.0 - open * open * open;
    let angle = -(open * FRAC_PI_2);
    parts[model.id("lid")].x_rot = angle;
    parts[model.id("lock")].x_rot = angle;
}

fn is_extended_christmas() -> bool {
    let Ok(now) =
        crate::platform::time::SystemTime::now().duration_since(crate::platform::time::UNIX_EPOCH)
    else {
        return false;
    };
    let (month, day) = civil_from_days((now.as_secs() / 86_400) as i64);
    month == 12 && (24..=26).contains(&day)
}

fn civil_from_days(days: i64) -> (u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_calendar_finds_christmas() {
        assert_eq!(civil_from_days(0), (1, 1));
        assert_eq!(civil_from_days(20_811), (12, 24));
        assert_eq!(civil_from_days(20_813), (12, 26));
        assert_eq!(civil_from_days(20_814), (12, 27));
    }

    #[test]
    fn facing_maps_to_vanilla_yaw() {
        assert_eq!(facing_y_rot(Some(Direction::South)), 0.0);
        assert_eq!(facing_y_rot(Some(Direction::West)), 90.0);
        assert_eq!(facing_y_rot(Some(Direction::North)), 180.0);
        assert_eq!(facing_y_rot(Some(Direction::East)), 270.0);
        assert_eq!(facing_y_rot(None), 0.0);
    }

    #[test]
    fn a_south_facing_chest_is_not_turned() {
        let st = super::super::test_state("chest", &[("facing", "south"), ("type", "single")]);
        let transform = transform(&st);
        assert!(transform.rotation.is_near_identity());
        assert!(transform.translation.abs_diff_eq(Vec3::ZERO, 1e-6));
    }

    #[test]
    fn a_north_facing_chest_turns_about_the_block_centre() {
        let st = super::super::test_state("chest", &[("facing", "north"), ("type", "single")]);
        let transform = transform(&st);
        let front = transform.transform_point(Vec3::new(0.5, 0.5, 1.0));
        assert!(
            front.abs_diff_eq(Vec3::new(0.5, 0.5, 0.0), 1e-5),
            "{front:?}"
        );
    }

    #[test]
    fn the_lid_swings_a_quarter_turn() {
        let model = crate::entities::geom::bake(&models::chest_single());
        let mut parts = model.rest_pose();
        let mut st = super::super::test_state("chest", &[("facing", "south"), ("type", "single")]);

        setup(&model, &mut parts, &st);
        assert_eq!(parts[model.id("lid")].x_rot, 0.0);

        st.open = 1.0;
        setup(&model, &mut parts, &st);
        assert!((parts[model.id("lid")].x_rot + FRAC_PI_2).abs() < 1e-6);
        assert_eq!(parts[model.id("lock")].x_rot, parts[model.id("lid")].x_rot);
    }

    #[test]
    fn the_lid_eases_out() {
        let model = crate::entities::geom::bake(&models::chest_single());
        let mut parts = model.rest_pose();
        let mut st = super::super::test_state("chest", &[("facing", "south"), ("type", "single")]);
        st.open = 0.5;
        setup(&model, &mut parts, &st);
        let expected = -(0.875 * FRAC_PI_2);
        assert!((parts[model.id("lid")].x_rot - expected).abs() < 1e-6);
    }

    #[test]
    fn chest_materials_resolve_to_files() {
        let cases = [
            ("chest", "single", "entity/chest/normal"),
            ("chest", "left", "entity/chest/normal_left"),
            ("trapped_chest", "right", "entity/chest/trapped_right"),
            ("ender_chest", "single", "entity/chest/ender"),
            ("copper_chest", "single", "entity/chest/copper"),
            (
                "waxed_exposed_copper_chest",
                "left",
                "entity/chest/copper_exposed_left",
            ),
            (
                "oxidized_copper_chest",
                "single",
                "entity/chest/copper_oxidized",
            ),
        ];
        for (block, chest_type, expected) in cases {
            let st = super::super::test_state(block, &[("facing", "south"), ("type", chest_type)]);
            if is_extended_christmas() && matches!(block, "chest" | "trapped_chest") {
                continue;
            }
            assert_eq!(texture(&st), expected);
            let file = crate::assets_root().join(format!("textures/{expected}.png"));
            assert!(file.exists(), "missing {expected}");
        }
    }
}
