use azalea_registry::builtin::EntityKind;

use crate::entities::TexturePath;
use crate::entities::models::monsters::creaking;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{Blend, RenderSpec};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Creaking,
        RenderSpec::new("creaking", creaking::layer, texture, creaking::setup_anim),
    );
    registry.add(
        EntityKind::Creaking,
        RenderSpec::new(
            "creaking_eyes",
            creaking::eyes_layer,
            eyes_texture,
            creaking::setup_anim,
        )
        .with_visible(eyes_glowing)
        .with_blend(Blend::Additive),
    );
}

fn texture(_st: &EntityState) -> TexturePath {
    "entity/creaking/creaking".into()
}

fn eyes_texture(_st: &EntityState) -> TexturePath {
    "entity/creaking/creaking_eyes".into()
}

fn eyes_glowing(st: &EntityState) -> bool {
    st.extras.active
}
