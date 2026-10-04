use azalea_registry::builtin::EntityKind;
use bevy::prelude::{Quat, Vec3};

use crate::entities::TexturePath;
use crate::entities::models::humanoid::zombie;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{RenderSpec, RootPose, setup_rotations};

use super::armor;

pub fn register(registry: &mut Registry) {
    register_zombie(registry);
    register_husk(registry);
    register_drowned(registry);
    register_giant(registry);
}

pub fn is_adult(st: &EntityState) -> bool {
    !st.extras.is_baby
}

pub fn is_baby(st: &EntityState) -> bool {
    st.extras.is_baby
}

fn register_zombie(registry: &mut Registry) {
    registry.add(
        EntityKind::Zombie,
        RenderSpec::new(
            "zombie",
            zombie::zombie_layer,
            zombie_texture,
            zombie::setup_anim,
        )
        .with_visible(is_adult),
    );
    registry.add(
        EntityKind::Zombie,
        RenderSpec::new(
            "zombie_baby",
            zombie::baby_zombie_layer,
            zombie_texture,
            zombie::setup_anim,
        )
        .with_visible(is_baby),
    );
    armor::register_set(
        registry,
        &[EntityKind::Zombie],
        [
            "zombie_helmet",
            "zombie_chestplate",
            "zombie_leggings",
            "zombie_boots",
        ],
        armor::HUMANOID,
        zombie::setup_anim,
        armor::ADULT,
    );
}

fn zombie_texture(st: &EntityState) -> TexturePath {
    if st.extras.is_baby {
        "entity/zombie/zombie_baby".into()
    } else {
        "entity/zombie/zombie".into()
    }
}

fn register_husk(registry: &mut Registry) {
    registry.add(
        EntityKind::Husk,
        RenderSpec::new("husk", zombie::husk_layer, husk_texture, zombie::setup_anim)
            .with_visible(is_adult),
    );
    registry.add(
        EntityKind::Husk,
        RenderSpec::new(
            "husk_baby",
            zombie::baby_zombie_layer,
            husk_texture,
            zombie::setup_anim,
        )
        .with_visible(is_baby),
    );
    armor::register_set(
        registry,
        &[EntityKind::Husk],
        [
            "husk_helmet",
            "husk_chestplate",
            "husk_leggings",
            "husk_boots",
        ],
        armor::HUSK,
        zombie::setup_anim,
        armor::ADULT,
    );
}

fn husk_texture(st: &EntityState) -> TexturePath {
    if st.extras.is_baby {
        "entity/zombie/husk_baby".into()
    } else {
        "entity/zombie/husk".into()
    }
}

fn register_drowned(registry: &mut Registry) {
    registry.add(
        EntityKind::Drowned,
        RenderSpec::new(
            "drowned",
            zombie::drowned_layer,
            drowned_texture,
            zombie::drowned_setup_anim,
        )
        .with_visible(is_adult)
        .with_root(drowned_root),
    );
    registry.add(
        EntityKind::Drowned,
        RenderSpec::new(
            "drowned_baby",
            zombie::baby_drowned_layer,
            drowned_texture,
            zombie::drowned_setup_anim,
        )
        .with_visible(is_baby)
        .with_root(drowned_root),
    );
    registry.add(
        EntityKind::Drowned,
        RenderSpec::new(
            "drowned_outer",
            zombie::drowned_outer_layer,
            drowned_outer_texture,
            zombie::drowned_setup_anim,
        )
        .with_visible(is_adult)
        .with_root(drowned_root),
    );
    registry.add(
        EntityKind::Drowned,
        RenderSpec::new(
            "drowned_baby_outer",
            zombie::baby_drowned_outer_layer,
            drowned_outer_texture,
            zombie::drowned_setup_anim,
        )
        .with_visible(is_baby)
        .with_root(drowned_root),
    );
    armor::register_set(
        registry,
        &[EntityKind::Drowned],
        [
            "drowned_helmet",
            "drowned_chestplate",
            "drowned_leggings",
            "drowned_boots",
        ],
        armor::HUMANOID,
        zombie::drowned_setup_anim,
        armor::ADULT,
    );
}

fn drowned_texture(st: &EntityState) -> TexturePath {
    if st.extras.is_baby {
        "entity/zombie/drowned_baby".into()
    } else {
        "entity/zombie/drowned".into()
    }
}

fn drowned_outer_texture(st: &EntityState) -> TexturePath {
    if st.extras.is_baby {
        "entity/zombie/drowned_outer_layer_baby".into()
    } else {
        "entity/zombie/drowned_outer_layer".into()
    }
}

fn drowned_root(st: &EntityState) -> RootPose {
    let mut pose = setup_rotations(st, 90.0);
    let swim = st.extras.swim_amount;
    if swim > 0.0 {
        let degrees = swim * (-10.0 - st.x_rot);
        let rotation = Quat::from_rotation_x(degrees.to_radians());
        let pivot = Vec3::new(0.0, st.bounding_box_height / 2.0 / st.scale, 0.0);
        pose.extra_offset += pivot - rotation * pivot;
        pose.extra_rotation *= rotation;
    }
    pose
}

fn register_giant(registry: &mut Registry) {
    registry.add(
        EntityKind::Giant,
        RenderSpec::new(
            "giant",
            zombie::giant_layer,
            giant_texture,
            zombie::setup_anim,
        ),
    );
    armor::register_set(
        registry,
        &[EntityKind::Giant],
        [
            "giant_helmet",
            "giant_chestplate",
            "giant_leggings",
            "giant_boots",
        ],
        armor::GIANT,
        zombie::setup_anim,
        armor::FILLED,
    );
}

fn giant_texture(_st: &EntityState) -> TexturePath {
    "entity/zombie/zombie".into()
}
