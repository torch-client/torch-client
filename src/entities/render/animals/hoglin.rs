use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::hoglin;
use crate::entities::registry::Registry;
use crate::entities::render::{add_adult_and_baby, aged_texture};
use crate::entities::state::EntityState;

const KINDS: [EntityKind; 2] = [EntityKind::Hoglin, EntityKind::Zoglin];

pub fn register(registry: &mut Registry) {
    add_adult_and_baby(
        registry,
        &KINDS,
        RenderSpec::new("hoglin", hoglin::layer, texture, hoglin::setup_anim),
        RenderSpec::new(
            "hoglin_baby",
            hoglin::baby_layer,
            texture,
            hoglin::baby_setup_anim,
        ),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    if st.kind == EntityKind::Zoglin {
        aged_texture(st, "entity/hoglin/zoglin", "entity/hoglin/zoglin_baby")
    } else {
        aged_texture(st, "entity/hoglin/hoglin", "entity/hoglin/hoglin_baby")
    }
}
