use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
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

fn texture(st: &EntityState) -> String {
    let base = if st.kind == EntityKind::Mule {
        "mule"
    } else {
        "donkey"
    };
    if st.extras.is_baby {
        format!("entity/horse/{base}_baby")
    } else {
        format!("entity/horse/{base}")
    }
}

fn saddle_texture(st: &EntityState) -> String {
    let base = if st.kind == EntityKind::Mule {
        "mule_saddle"
    } else {
        "donkey_saddle"
    };
    format!("entity/equipment/{base}/saddle")
}
