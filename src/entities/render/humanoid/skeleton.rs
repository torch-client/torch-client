use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::humanoid::skeleton;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

use super::armor;

const SKELETON_ARMOR_KINDS: [EntityKind; 5] = [
    EntityKind::Skeleton,
    EntityKind::WitherSkeleton,
    EntityKind::Stray,
    EntityKind::Bogged,
    EntityKind::Parched,
];

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Skeleton,
        RenderSpec::new(
            "skeleton",
            skeleton::skeleton_layer,
            skeleton_texture,
            skeleton::setup_anim,
        ),
    );
    registry.add(
        EntityKind::WitherSkeleton,
        RenderSpec::new(
            "wither_skeleton",
            skeleton::skeleton_layer,
            wither_skeleton_texture,
            skeleton::setup_anim,
        ),
    );

    registry.add(
        EntityKind::Stray,
        RenderSpec::new(
            "stray",
            skeleton::skeleton_layer,
            stray_texture,
            skeleton::setup_anim,
        ),
    );
    registry.add(
        EntityKind::Stray,
        RenderSpec::new(
            "stray_overlay",
            skeleton::stray_outer_layer,
            stray_overlay_texture,
            skeleton::setup_anim,
        ),
    );

    registry.add(
        EntityKind::Bogged,
        RenderSpec::new(
            "bogged",
            skeleton::bogged_layer,
            bogged_texture,
            skeleton::bogged_setup_anim,
        ),
    );
    registry.add(
        EntityKind::Bogged,
        RenderSpec::new(
            "bogged_overlay",
            skeleton::bogged_outer_layer,
            bogged_overlay_texture,
            skeleton::setup_anim,
        ),
    );

    registry.add(
        EntityKind::Parched,
        RenderSpec::new(
            "parched",
            skeleton::parched_layer,
            parched_texture,
            skeleton::setup_anim,
        ),
    );

    armor::register_set(
        registry,
        &SKELETON_ARMOR_KINDS,
        [
            "skeleton_helmet",
            "skeleton_chestplate",
            "skeleton_leggings",
            "skeleton_boots",
        ],
        armor::HUMANOID,
        skeleton::setup_anim,
        armor::FILLED,
    );
}

fn skeleton_texture(_st: &EntityState) -> TexturePath {
    "entity/skeleton/skeleton".into()
}

fn wither_skeleton_texture(_st: &EntityState) -> TexturePath {
    "entity/skeleton/wither_skeleton".into()
}

fn stray_texture(_st: &EntityState) -> TexturePath {
    "entity/skeleton/stray".into()
}

fn stray_overlay_texture(_st: &EntityState) -> TexturePath {
    "entity/skeleton/stray_overlay".into()
}

fn bogged_texture(_st: &EntityState) -> TexturePath {
    "entity/skeleton/bogged".into()
}

fn bogged_overlay_texture(_st: &EntityState) -> TexturePath {
    "entity/skeleton/bogged_overlay".into()
}

fn parched_texture(_st: &EntityState) -> TexturePath {
    "entity/skeleton/parched".into()
}
