use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::equine;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Donkey,
        RenderSpec::new(
            "donkey",
            equine::donkey_layer,
            texture,
            equine::donkey_setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby),
    );
    registry.add(
        EntityKind::Donkey,
        RenderSpec::new(
            "donkey_baby",
            equine::baby_donkey_layer,
            texture,
            equine::baby_donkey_setup_anim,
        )
        .with_visible(|st| st.extras.is_baby),
    );
    registry.add(
        EntityKind::Donkey,
        RenderSpec::new(
            "donkey_saddle",
            equine::donkey_saddle_layer,
            saddle_texture,
            equine::saddle_setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby && st.extras.saddled),
    );
    registry.add(
        EntityKind::Mule,
        RenderSpec::new(
            "mule",
            equine::mule_layer,
            texture,
            equine::donkey_setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby),
    );
    registry.add(
        EntityKind::Mule,
        RenderSpec::new(
            "mule_baby",
            equine::baby_donkey_layer,
            texture,
            equine::baby_donkey_setup_anim,
        )
        .with_visible(|st| st.extras.is_baby),
    );
    registry.add(
        EntityKind::Mule,
        RenderSpec::new(
            "mule_saddle",
            equine::mule_saddle_layer,
            saddle_texture,
            equine::saddle_setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby && st.extras.saddled),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    match (st.kind == EntityKind::Mule, st.extras.is_baby) {
        (true, true) => "entity/horse/mule_baby",
        (true, false) => "entity/horse/mule",
        (false, true) => "entity/horse/donkey_baby",
        (false, false) => "entity/horse/donkey",
    }
    .into()
}

fn saddle_texture(st: &EntityState) -> TexturePath {
    if st.kind == EntityKind::Mule {
        "entity/equipment/mule_saddle/saddle".into()
    } else {
        "entity/equipment/donkey_saddle/saddle".into()
    }
}
