use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::models::aquatic::parrot;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Parrot,
        RenderSpec::new("parrot", parrot::layer, texture, parrot::setup_anim),
    );
}

fn texture(st: &EntityState) -> String {
    match st.extras.variant_id {
        1 => "entity/parrot/parrot_blue".to_string(),
        2 => "entity/parrot/parrot_green".to_string(),
        3 => "entity/parrot/parrot_yellow_blue".to_string(),
        4 => "entity/parrot/parrot_grey".to_string(),
        _ => "entity/parrot/parrot_red_blue".to_string(),
    }
}
