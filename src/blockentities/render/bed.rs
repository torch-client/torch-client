use bevy::prelude::*;

use crate::blockentities::models;
use crate::blockentities::render::chest::facing_y_rot;
use crate::blockentities::{BeRegistry, BeSpec, BeState, GeomKey};
use crate::entities::geom::{BakedModel, LayerDef, PartState};

pub fn register(registry: &mut BeRegistry) {
    registry.add(
        azalea_registry::builtin::BlockEntityKind::Bed,
        BeSpec::model("bed", key, layer, texture, setup).with_transform(transform),
    );
}

fn color(st: &BeState) -> &str {
    st.state.block.strip_suffix("_bed").unwrap_or("red")
}

fn part(st: &BeState) -> &str {
    match st.state.prop("part") {
        "head" => "head",
        _ => "foot",
    }
}

fn key(st: &BeState) -> Option<u64> {
    Some(GeomKey::new().str(color(st)).str(part(st)).finish())
}

fn layer(st: &BeState) -> LayerDef {
    if part(st) == "head" {
        models::bed_head()
    } else {
        models::bed_foot()
    }
}

fn texture(st: &BeState) -> String {
    format!("entity/bed/{}", color(st))
}

fn transform(st: &BeState) -> Transform {
    let facing = Quat::from_rotation_z((180.0 + facing_y_rot(st.state.facing)).to_radians());
    let lie_flat = Quat::from_rotation_x(90.0_f32.to_radians());
    let pivot = Vec3::new(0.5, 0.5, 0.5);
    let base = Vec3::new(0.0, 0.5625, 0.0);
    Transform {
        translation: lie_flat * (pivot - facing * pivot) + base,
        rotation: lie_flat * facing,
        scale: Vec3::ONE,
    }
}

fn setup(_model: &BakedModel, _parts: &mut [PartState], _st: &BeState) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_and_parts_resolve_to_files() {
        let cases = [
            ("red_bed", "head", "entity/bed/red"),
            ("white_bed", "foot", "entity/bed/white"),
        ];
        for (block, part, expected) in cases {
            let st = super::super::test_state(block, &[("facing", "south"), ("part", part)]);
            assert_eq!(texture(&st), expected);
            let file = crate::assets_root().join(format!("textures/{expected}.png"));
            assert!(file.exists(), "missing {expected}");
        }
    }

    #[test]
    fn a_south_facing_bed_sits_in_its_own_block() {
        let st = super::super::test_state("red_bed", &[("facing", "south"), ("part", "foot")]);
        let transform = transform(&st);
        assert!(
            (transform.translation - Vec3::new(1.0, 0.5625, 1.0)).length() < 1e-5,
            "{:?}",
            transform.translation
        );
    }
}
