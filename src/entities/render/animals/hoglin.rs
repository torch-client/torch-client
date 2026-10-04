use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::hoglin;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

const KINDS: [EntityKind; 2] = [EntityKind::Hoglin, EntityKind::Zoglin];

pub fn register(registry: &mut Registry) {
    registry.add_many(
        &KINDS,
        RenderSpec::new("hoglin", hoglin::layer, texture, hoglin::setup_anim)
            .with_visible(|st| !st.extras.is_baby),
    );
    registry.add_many(
        &KINDS,
        RenderSpec::new(
            "hoglin_baby",
            hoglin::baby_layer,
            texture,
            hoglin::baby_setup_anim,
        )
        .with_visible(|st| st.extras.is_baby),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    match (st.kind == EntityKind::Zoglin, st.extras.is_baby) {
        (true, true) => "entity/hoglin/zoglin_baby",
        (true, false) => "entity/hoglin/zoglin",
        (false, true) => "entity/hoglin/hoglin_baby",
        (false, false) => "entity/hoglin/hoglin",
    }
    .into()
}
