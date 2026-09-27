use std::f32::consts::PI;

use azalea_registry::builtin::BlockEntityKind;
use bevy::prelude::*;

use crate::blockentities::models;
use crate::blockentities::{BeRegistry, BeSpec, BeState, GeomKey};
use crate::entities::geom::{BakedModel, LayerDef, PartState};

pub fn register(registry: &mut BeRegistry) {
    registry.add(
        BlockEntityKind::Skull,
        BeSpec::model("skull", key, layer, texture, setup).with_transform(transform),
    );
}

fn skull_type(block: &str) -> &'static str {
    match block {
        "skeleton_skull" | "skeleton_wall_skull" => "skeleton",
        "wither_skeleton_skull" | "wither_skeleton_wall_skull" => "wither_skeleton",
        "zombie_head" | "zombie_wall_head" => "zombie",
        "creeper_head" | "creeper_wall_head" => "creeper",
        "dragon_head" | "dragon_wall_head" => "dragon",
        "piglin_head" | "piglin_wall_head" => "piglin",
        _ => "player",
    }
}

fn is_wall(block: &str) -> bool {
    block.ends_with("_wall_skull") || block.ends_with("_wall_head")
}

fn key(st: &BeState) -> Option<u64> {
    Some(GeomKey::new().str(skull_type(&st.state.block)).finish())
}

pub(crate) fn layer_for_type(kind: &str) -> LayerDef {
    match kind {
        "zombie" | "player" => models::skull_humanoid_head(),
        "piglin" => models::piglin_head(),
        "dragon" => models::dragon_head(),
        _ => models::skull_mob_head(),
    }
}

fn layer(st: &BeState) -> LayerDef {
    layer_for_type(skull_type(&st.state.block))
}

pub(crate) fn texture_for_type(kind: &str) -> &'static str {
    match kind {
        "skeleton" => "entity/skeleton/skeleton",
        "wither_skeleton" => "entity/skeleton/wither_skeleton",
        "zombie" => "entity/zombie/zombie",
        "creeper" => "entity/creeper/creeper",
        "dragon" => "entity/enderdragon/dragon",
        "piglin" => "entity/piglin/piglin",
        _ => "entity/player/wide/steve",
    }
}

fn texture(st: &BeState) -> String {
    texture_for_type(skull_type(&st.state.block)).to_string()
}

fn setup(model: &BakedModel, parts: &mut [PartState], st: &BeState) {
    match skull_type(&st.state.block) {
        "dragon" => {
            let t = st.anim * PI * 0.2;
            parts[model.id("jaw")].x_rot = (t.sin() + 1.0) * 0.2;
        }
        "piglin" => {
            let t = st.anim * PI * 0.2;
            parts[model.id("left_ear")].z_rot = -((t * 1.2).cos() + 2.5) * 0.2;
            parts[model.id("right_ear")].z_rot = (t.cos() + 2.5) * 0.2;
        }
        _ => {}
    }
}

fn segment_degrees(segment: u32) -> f32 {
    segment as f32 * (360.0 / 16.0)
}

fn facing_step(facing: &str) -> (f32, f32) {
    match facing {
        "north" => (0.0, -1.0),
        "west" => (-1.0, 0.0),
        "east" => (1.0, 0.0),
        _ => (0.0, 1.0),
    }
}

fn opposite(facing: &str) -> &'static str {
    match facing {
        "north" => "south",
        "west" => "east",
        "east" => "west",
        _ => "north",
    }
}

fn wall_transform(st: &BeState) -> Transform {
    let facing = st.state.prop("facing");
    let (step_x, step_z) = facing_step(facing);
    let rotation =
        Quat::from_rotation_y(-super::chest::facing_y_rot(opposite(facing)).to_radians());
    Transform {
        translation: Vec3::new(0.5 - step_x * 0.25, 0.25, 0.5 - step_z * 0.25),
        rotation,
        scale: Vec3::new(-1.0, -1.0, 1.0),
    }
}

fn ground_transform(st: &BeState) -> Transform {
    let segment: u32 = st.state.prop("rotation").parse().unwrap_or(0);
    Transform {
        translation: Vec3::new(0.5, 0.0, 0.5),
        rotation: Quat::from_rotation_y(-segment_degrees(segment).to_radians()),
        scale: Vec3::new(-1.0, -1.0, 1.0),
    }
}

fn transform(st: &BeState) -> Transform {
    if is_wall(&st.state.block) {
        wall_transform(st)
    } else {
        ground_transform(st)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_skull_block_names_its_type() {
        assert_eq!(skull_type("skeleton_skull"), "skeleton");
        assert_eq!(skull_type("wither_skeleton_wall_skull"), "wither_skeleton");
        assert_eq!(skull_type("zombie_head"), "zombie");
        assert_eq!(skull_type("creeper_wall_head"), "creeper");
        assert_eq!(skull_type("dragon_head"), "dragon");
        assert_eq!(skull_type("piglin_wall_head"), "piglin");
        assert_eq!(skull_type("player_head"), "player");
    }

    #[test]
    fn skull_textures_resolve_to_files() {
        for block in [
            "skeleton_skull",
            "wither_skeleton_skull",
            "zombie_head",
            "creeper_head",
            "dragon_head",
            "piglin_head",
            "player_head",
        ] {
            let st = super::super::test_state(block, &[("rotation", "0")]);
            let path = texture(&st);
            let file = crate::assets_root().join(format!("textures/{path}.png"));
            assert!(file.exists(), "missing {path} for {block}");
        }
    }

    #[test]
    fn ground_and_wall_skulls_sit_where_vanilla_puts_them() {
        let ground = super::super::test_state("skeleton_skull", &[("rotation", "0")]);
        let t = ground_transform(&ground);
        assert!(t.translation.abs_diff_eq(Vec3::new(0.5, 0.0, 0.5), 1e-6));
        assert!(t.rotation.is_near_identity());

        let wall = super::super::test_state("skeleton_wall_skull", &[("facing", "north")]);
        let t = wall_transform(&wall);
        assert!(t.translation.abs_diff_eq(Vec3::new(0.5, 0.25, 0.75), 1e-6));
    }

    #[test]
    fn the_dragon_jaw_flaps_with_the_animation_clock() {
        let model = crate::entities::geom::bake(&models::dragon_head());
        let mut parts = model.rest_pose();
        let mut st = super::super::test_state("dragon_head", &[("rotation", "0")]);
        setup(&model, &mut parts, &st);
        assert!((parts[model.id("jaw")].x_rot - 0.2).abs() < 1e-6);

        st.anim = 2.5;
        setup(&model, &mut parts, &st);
        assert!((parts[model.id("jaw")].x_rot - 0.2).abs() > 1e-6);
    }
}
