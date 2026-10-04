use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::monsters::guardian;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Guardian,
        RenderSpec::new("guardian", guardian::layer, texture, guardian::setup_anim),
    );
    registry.add(
        EntityKind::ElderGuardian,
        RenderSpec::new(
            "elder_guardian",
            guardian::elder_layer,
            elder_texture,
            guardian::setup_anim,
        ),
    );
}

fn texture(_st: &EntityState) -> TexturePath {
    "entity/guardian/guardian".into()
}

fn elder_texture(_st: &EntityState) -> TexturePath {
    "entity/guardian/guardian_elder".into()
}
