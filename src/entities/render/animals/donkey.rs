use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::equine;
use crate::entities::registry::Registry;
use crate::entities::render::{add_adult_and_baby, aged_texture};
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    add_adult_and_baby(
        registry,
        &[EntityKind::Donkey],
        RenderSpec::new(
            "donkey",
            equine::donkey_layer,
            texture,
            equine::donkey_setup_anim,
        ),
        RenderSpec::new(
            "donkey_baby",
            equine::baby_donkey_layer,
            texture,
            equine::baby_donkey_setup_anim,
        ),
    );
    registry.add(
        EntityKind::Donkey,
        RenderSpec::new(
            "donkey_saddle",
            equine::donkey_saddle_layer,
            saddle_texture,
            equine::saddle_setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby && st.extras.saddled)
        .ignoring_invisibility(),
    );
    add_adult_and_baby(
        registry,
        &[EntityKind::Mule],
        RenderSpec::new(
            "mule",
            equine::mule_layer,
            texture,
            equine::donkey_setup_anim,
        ),
        RenderSpec::new(
            "mule_baby",
            equine::baby_donkey_layer,
            texture,
            equine::baby_donkey_setup_anim,
        ),
    );
    registry.add(
        EntityKind::Mule,
        RenderSpec::new(
            "mule_saddle",
            equine::mule_saddle_layer,
            saddle_texture,
            equine::saddle_setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby && st.extras.saddled)
        .ignoring_invisibility(),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    if st.kind == EntityKind::Mule {
        aged_texture(st, "entity/horse/mule", "entity/horse/mule_baby")
    } else {
        aged_texture(st, "entity/horse/donkey", "entity/horse/donkey_baby")
    }
}

fn saddle_texture(st: &EntityState) -> TexturePath {
    if st.kind == EntityKind::Mule {
        "entity/equipment/mule_saddle/saddle".into()
    } else {
        "entity/equipment/donkey_saddle/saddle".into()
    }
}
