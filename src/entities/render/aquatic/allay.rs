use azalea_registry::builtin::EntityKind;

use crate::entities::TexturePath;
use crate::entities::models::aquatic::allay;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{Blend, RenderSpec};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Allay,
        RenderSpec::new("allay", allay::layer, texture, allay::setup_anim)
            .with_blend(Blend::Translucent),
    );
}

fn texture(_st: &EntityState) -> TexturePath {
    "entity/allay/allay".into()
}
