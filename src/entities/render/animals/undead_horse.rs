use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::equine;
use crate::entities::registry::Registry;
use crate::entities::render::{add_adult_and_baby, aged_texture};
use crate::entities::state::EntityState;

const KINDS: [EntityKind; 2] = [EntityKind::SkeletonHorse, EntityKind::ZombieHorse];

pub fn register(registry: &mut Registry) {
    add_adult_and_baby(
        registry,
        &KINDS,
        RenderSpec::new(
            "undead_horse",
            equine::undead_horse_layer,
            texture,
            equine::setup_anim,
        ),
        RenderSpec::new(
            "undead_horse_baby",
            equine::baby_horse_layer,
            texture,
            equine::baby_setup_anim,
        ),
    );
    registry.add_many(
        &KINDS,
        RenderSpec::new(
            "undead_horse_armor",
            equine::undead_horse_armor_layer,
            super::horse::armor_texture,
            equine::setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby && super::horse::wears_armor(st))
        .ignoring_invisibility(),
    );
    registry.add_many(
        &KINDS,
        RenderSpec::new(
            "undead_horse_saddle",
            equine::undead_horse_saddle_layer,
            saddle_texture,
            equine::saddle_setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby && st.extras.saddled)
        .ignoring_invisibility(),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    if st.kind == EntityKind::ZombieHorse {
        aged_texture(
            st,
            "entity/horse/horse_zombie",
            "entity/horse/horse_zombie_baby",
        )
    } else {
        aged_texture(
            st,
            "entity/horse/horse_skeleton",
            "entity/horse/horse_skeleton_baby",
        )
    }
}

fn saddle_texture(st: &EntityState) -> TexturePath {
    if st.kind == EntityKind::ZombieHorse {
        "entity/equipment/zombie_horse_saddle/saddle".into()
    } else {
        "entity/equipment/skeleton_horse_saddle/saddle".into()
    }
}
