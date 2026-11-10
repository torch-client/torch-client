use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::aquatic::parrot;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Parrot,
        RenderSpec::new("parrot", parrot::layer, texture, parrot::setup_anim),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    variant_texture(i64::from(st.extras.variant_id)).into()
}

pub(crate) fn variant_texture(id: i64) -> &'static str {
    match id.clamp(0, 4) {
        1 => "entity/parrot/parrot_blue",
        2 => "entity/parrot/parrot_green",
        3 => "entity/parrot/parrot_yellow_blue",
        4 => "entity/parrot/parrot_grey",
        _ => "entity/parrot/parrot_red_blue",
    }
}
