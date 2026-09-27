use std::f32::consts::PI;

use azalea_registry::builtin::BlockEntityKind;
use bevy::prelude::*;

use crate::blockentities::banner::{self, BannerLayer, MAX_PATTERNS};
use crate::blockentities::feed::BlockEntityData;
use crate::blockentities::models;
use crate::blockentities::render::chest::facing_y_rot;
use crate::blockentities::{BeRegistry, BeSpec, BeState, GeomKey};
use crate::entities::geom::{BakedModel, LayerDef, PartState};

pub fn register(registry: &mut BeRegistry) {
    registry.add(
        BlockEntityKind::Banner,
        BeSpec::model("banner_body", body_key, body_layer, base_texture, no_wave)
            .with_transform(transform),
    );
    registry.add(
        BlockEntityKind::Banner,
        BeSpec::model("banner_cloth", flag_key, flag_layer, base_texture, wave)
            .with_transform(transform),
    );
    registry.add(
        BlockEntityKind::Banner,
        BeSpec::model("banner_base", flag_key, flag_layer, mask_texture, wave)
            .with_transform(transform)
            .with_tint(base_tint)
            .with_depth_bias(1),
    );
    register_pattern_layers(registry);
}

fn layers(st: &BeState) -> Option<&[BannerLayer]> {
    match &*st.data {
        BlockEntityData::Banner(data) => Some(&data.layers),
        _ => None,
    }
}

fn slot(st: &BeState, index: usize) -> Option<&BannerLayer> {
    layers(st)?.get(index)
}

fn standing(st: &BeState) -> bool {
    !banner::is_wall(&st.state.block)
}

fn form(st: &BeState) -> GeomKey {
    GeomKey::new().str(if standing(st) { "standing" } else { "wall" })
}

fn body_key(st: &BeState) -> Option<u64> {
    Some(form(st).finish())
}

fn flag_key(st: &BeState) -> Option<u64> {
    Some(form(st).finish())
}

fn body_layer(st: &BeState) -> LayerDef {
    if standing(st) {
        models::banner_body_standing()
    } else {
        models::banner_body_wall()
    }
}

fn flag_layer(st: &BeState) -> LayerDef {
    if standing(st) {
        models::banner_flag_standing()
    } else {
        models::banner_flag_wall()
    }
}

fn base_texture(_st: &BeState) -> String {
    "entity/banner/banner_base".to_string()
}

fn mask_texture(_st: &BeState) -> String {
    "entity/banner/base".to_string()
}

fn base_tint(st: &BeState) -> [f32; 4] {
    banner::tint(banner::base_color_of(&st.state.block))
}

macro_rules! pattern_layers {
    ($registry:expr, $($name:literal => $slot:literal),* $(,)?) => {
        $(
            $registry.add(
                BlockEntityKind::Banner,
                BeSpec::model(
                    $name,
                    pattern_key::<$slot>,
                    flag_layer,
                    pattern_texture::<$slot>,
                    wave,
                )
                .with_transform(transform)
                .with_tint(pattern_tint::<$slot>)
                .with_depth_bias(2 + $slot),
            );
        )*
    };
}

fn register_pattern_layers(registry: &mut BeRegistry) {
    pattern_layers!(
        registry,
        "banner_layer_00" => 0,
        "banner_layer_01" => 1,
        "banner_layer_02" => 2,
        "banner_layer_03" => 3,
        "banner_layer_04" => 4,
        "banner_layer_05" => 5,
        "banner_layer_06" => 6,
        "banner_layer_07" => 7,
        "banner_layer_08" => 8,
        "banner_layer_09" => 9,
        "banner_layer_10" => 10,
        "banner_layer_11" => 11,
        "banner_layer_12" => 12,
        "banner_layer_13" => 13,
        "banner_layer_14" => 14,
        "banner_layer_15" => 15,
    );
    const _: () = assert!(MAX_PATTERNS == 16);
}

fn pattern_key<const SLOT: usize>(st: &BeState) -> Option<u64> {
    Some(form(st).str(&slot(st, SLOT)?.asset).finish())
}

fn pattern_texture<const SLOT: usize>(st: &BeState) -> String {
    match slot(st, SLOT) {
        Some(layer) => format!("entity/banner/{}", layer.asset),
        None => String::new(),
    }
}

fn pattern_tint<const SLOT: usize>(st: &BeState) -> [f32; 4] {
    match slot(st, SLOT) {
        Some(layer) => banner::tint(layer.color),
        None => [1.0; 4],
    }
}

fn wave(model: &BakedModel, parts: &mut [PartState], st: &BeState) {
    parts[model.id("flag")].x_rot = (-0.012_5 + 0.01 * (2.0 * PI * st.phase).cos()) * PI;
}

fn no_wave(_model: &BakedModel, _parts: &mut [PartState], _st: &BeState) {}

fn segment_degrees(segment: u32) -> f32 {
    segment as f32 * (360.0 / 16.0)
}

fn transform(st: &BeState) -> Transform {
    let angle = if banner::is_wall(&st.state.block) {
        facing_y_rot(st.state.prop("facing"))
    } else {
        segment_degrees(st.state.prop("rotation").parse().unwrap_or(0))
    };
    Transform {
        translation: Vec3::new(0.5, 0.0, 0.5),
        rotation: Quat::from_rotation_y(-angle.to_radians()),
        scale: Vec3::new(0.666_666_7, -0.666_666_7, -0.666_666_7),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockentities::render::test_state;

    #[test]
    fn the_two_forms_take_different_geometry() {
        let ground = test_state("white_banner", &[("rotation", "0")]);
        let wall = test_state("white_wall_banner", &[("facing", "north")]);
        assert!(standing(&ground));
        assert!(!standing(&wall));
        assert_ne!(body_key(&ground), body_key(&wall));
        assert_ne!(flag_key(&ground), flag_key(&wall));
    }

    #[test]
    fn an_unpatterned_banner_keys_no_pattern_layer() {
        let st = test_state("red_banner", &[("rotation", "8")]);
        assert!(body_key(&st).is_some());
        assert!(flag_key(&st).is_some());
        assert!(pattern_key::<0>(&st).is_none());
        assert!(pattern_key::<15>(&st).is_none());
        assert_eq!(base_tint(&st), banner::tint(14));
    }

    #[test]
    fn each_pattern_slot_keys_its_own_mask() {
        let st = crate::blockentities::render::banner_state(
            "cyan_banner",
            &[("creeper", 15), ("border", 4)],
        );
        assert!(pattern_key::<0>(&st).is_some());
        assert!(pattern_key::<1>(&st).is_some());
        assert!(pattern_key::<2>(&st).is_none());
        assert_ne!(pattern_key::<0>(&st), pattern_key::<1>(&st));
        assert_eq!(pattern_texture::<0>(&st), "entity/banner/creeper");
        assert_eq!(pattern_texture::<1>(&st), "entity/banner/border");
        assert_eq!(pattern_tint::<0>(&st), banner::tint(15));
        assert_eq!(pattern_tint::<1>(&st), banner::tint(4));
    }

    #[test]
    fn the_wave_stays_within_its_two_hundredths() {
        for step in 0..100 {
            let phase = step as f32 / 100.0;
            let x_rot = (-0.012_5 + 0.01 * (2.0 * PI * phase).cos()) * PI;
            assert!((-0.0225 * PI..=-0.0025 * PI).contains(&x_rot), "{x_rot}");
        }
    }
}
