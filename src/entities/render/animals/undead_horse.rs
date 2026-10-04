use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::equine;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

const KINDS: [EntityKind; 2] = [EntityKind::SkeletonHorse, EntityKind::ZombieHorse];

pub fn register(registry: &mut Registry) {
    registry.add_many(
        &KINDS,
        RenderSpec::new(
            "undead_horse",
            equine::undead_horse_layer,
            texture,
            equine::setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby),
    );
    registry.add_many(
        &KINDS,
        RenderSpec::new(
            "undead_horse_baby",
            equine::baby_horse_layer,
            texture,
            equine::baby_setup_anim,
        )
        .with_visible(|st| st.extras.is_baby),
    );
    registry.add_many(
        &KINDS,
        RenderSpec::new(
            "undead_horse_armor",
            equine::undead_horse_armor_layer,
            armor_texture,
            equine::setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby && super::horse::armor_material(st).is_some()),
    );
    registry.add_many(
        &KINDS,
        RenderSpec::new(
            "undead_horse_saddle",
            equine::undead_horse_saddle_layer,
            saddle_texture,
            equine::saddle_setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby && st.extras.saddled),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    match (st.kind == EntityKind::ZombieHorse, st.extras.is_baby) {
        (true, true) => "entity/horse/horse_zombie_baby",
        (true, false) => "entity/horse/horse_zombie",
        (false, true) => "entity/horse/horse_skeleton_baby",
        (false, false) => "entity/horse/horse_skeleton",
    }
    .into()
}

fn armor_texture(st: &EntityState) -> TexturePath {
    let material = super::horse::armor_material(st).unwrap_or("iron");
    format!("entity/equipment/horse_body/{material}").into()
}

fn saddle_texture(st: &EntityState) -> TexturePath {
    if st.kind == EntityKind::ZombieHorse {
        "entity/equipment/zombie_horse_saddle/saddle".into()
    } else {
        "entity/equipment/skeleton_horse_saddle/saddle".into()
    }
}
