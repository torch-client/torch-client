use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::monsters::ravager;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Ravager,
        RenderSpec::new("ravager", ravager::layer, texture, ravager::setup_anim),
    );
}

fn texture(_st: &EntityState) -> TexturePath {
    "entity/illager/ravager".into()
}
