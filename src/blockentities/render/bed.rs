use bevy::prelude::*;

use crate::blockentities::models;
use crate::blockentities::render::chest::facing_y_rot;
use crate::blockentities::{BeRegistry, BeSpec, BeState, GeomKey, no_anim};
use crate::entities::geom::LayerDef;
use crate::entities::rotate_around;

pub fn register(registry: &mut BeRegistry) {
    registry.add(
        azalea_registry::builtin::BlockEntityKind::Bed,
        BeSpec::model("bed", key, layer, texture, no_anim).with_transform(transform),
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
    models::bed_half(part(st) == "head")
}

fn texture(st: &BeState) -> String {
    format!("entity/bed/{}", color(st))
}

fn transform(st: &BeState) -> Transform {
    let facing = Quat::from_rotation_z((180.0 + facing_y_rot(st.state.facing)).to_radians());
    let lie_flat = Quat::from_rotation_x(90.0_f32.to_radians());
    Transform::from_xyz(0.0, 0.5625, 0.0).with_rotation(lie_flat)
        * rotate_around(facing, Vec3::splat(0.5))
}

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
