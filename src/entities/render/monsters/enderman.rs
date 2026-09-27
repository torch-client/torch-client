use azalea_registry::builtin::EntityKind;

use crate::entities::models::monsters::enderman;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{Blend, RenderSpec};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Enderman,
        RenderSpec::new("enderman", enderman::layer, texture, enderman::setup_anim),
    );
    registry.add(
        EntityKind::Enderman,
        RenderSpec::new(
            "enderman_eyes",
            enderman::layer,
            eyes_texture,
            enderman::setup_anim,
        )
        .with_blend(Blend::Additive),
    );
}

fn texture(_st: &EntityState) -> String {
    "entity/enderman/enderman".to_string()
}

fn eyes_texture(_st: &EntityState) -> String {
    "entity/enderman/enderman_eyes".to_string()
}
