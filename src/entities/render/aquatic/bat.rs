use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::models::aquatic::bat;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Bat,
        RenderSpec::new("bat", bat::layer, texture, bat::setup_anim),
    );
}

fn texture(_st: &EntityState) -> String {
    "entity/bat/bat".to_string()
}
