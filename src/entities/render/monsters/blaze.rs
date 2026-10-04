use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::monsters::blaze;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Blaze,
        RenderSpec::new("blaze", blaze::layer, texture, blaze::setup_anim),
    );
}

fn texture(_st: &EntityState) -> TexturePath {
    "entity/blaze/blaze".into()
}
