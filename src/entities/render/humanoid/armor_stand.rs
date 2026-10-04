use azalea_registry::builtin::EntityKind;
use bevy::prelude::{Quat, Vec3};

use crate::entities::TexturePath;
use crate::entities::models::humanoid::{armor_stand, mannequin};
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{RenderSpec, RootPose, hook_scale_then_translate};

use super::armor;

const AVATAR_SCALE: f32 = 0.937_5;

const CROUCH_RENDER_OFFSET_Y: f32 = -2.0 / 16.0;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::ArmorStand,
        RenderSpec::new(
            "armor_stand",
            armor_stand::armor_stand_layer,
            armor_stand_texture,
            armor_stand::setup_anim,
        )
        .with_root(armor_stand_root)
        .with_visible(is_big),
    );
    registry.add(
        EntityKind::ArmorStand,
        RenderSpec::new(
            "armor_stand_small",
            armor_stand::armor_stand_small_layer,
            armor_stand_texture,
            armor_stand::setup_anim,
        )
        .with_root(armor_stand_root)
        .with_visible(is_small),
    );
    armor::register_set(
        registry,
        &[EntityKind::ArmorStand],
        [
            "armor_stand_helmet",
            "armor_stand_chestplate",
            "armor_stand_leggings",
            "armor_stand_boots",
        ],
        armor::ARMOR_STAND,
        armor_stand::armor_setup_anim,
        armor::BIG_STAND,
    );
    armor::register_set(
        registry,
        &[EntityKind::ArmorStand],
        [
            "armor_stand_small_helmet",
            "armor_stand_small_chestplate",
            "armor_stand_small_leggings",
            "armor_stand_small_boots",
        ],
        armor::ARMOR_STAND_SMALL,
        armor_stand::armor_setup_anim,
        armor::SMALL_STAND,
    );

    registry.add(
        EntityKind::Mannequin,
        RenderSpec::new(
            "mannequin",
            mannequin::wide_layer,
            mannequin_texture,
            mannequin::setup_anim,
        )
        .with_root(avatar_root),
    );
    armor::register_set(
        registry,
        &[EntityKind::Mannequin],
        [
            "mannequin_helmet",
            "mannequin_chestplate",
            "mannequin_leggings",
            "mannequin_boots",
        ],
        armor::HUMANOID,
        mannequin::setup_anim,
        armor::FILLED,
    );
}

fn is_big(st: &EntityState) -> bool {
    !st.extras.small
}

fn is_small(st: &EntityState) -> bool {
    st.extras.small
}

fn armor_stand_texture(_st: &EntityState) -> TexturePath {
    "entity/armorstand/armorstand".into()
}

fn mannequin_texture(_st: &EntityState) -> TexturePath {
    "entity/player/wide/steve".into()
}

fn armor_stand_root(st: &EntityState) -> RootPose {
    let mut rotation = Quat::from_rotation_y((180.0 - st.body_rot).to_radians());
    if st.extras.wiggle < 5.0 {
        rotation *= Quat::from_rotation_y(
            ((st.extras.wiggle / 1.5 * std::f32::consts::PI).sin() * 3.0).to_radians(),
        );
    }
    RootPose {
        rotation,
        scale: st.scale,
        ..RootPose::default()
    }
}

fn avatar_root(st: &EntityState) -> RootPose {
    let mut pose = crate::entities::setup_rotations(st, 90.0);
    pose.hook = hook_scale_then_translate(Vec3::splat(AVATAR_SCALE), Vec3::ZERO);
    if st.extras.crouching {
        pose.world_offset.y += st.scale * CROUCH_RENDER_OFFSET_Y;
    }
    pose
}
