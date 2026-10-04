use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::feline;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Ocelot,
        RenderSpec::new("ocelot", feline::ocelot_layer, texture, feline::setup_anim)
            .with_visible(|st| !st.extras.is_baby),
    );
    registry.add(
        EntityKind::Ocelot,
        RenderSpec::new(
            "ocelot_baby",
            feline::baby_layer,
            texture,
            feline::baby_setup_anim,
        )
        .with_visible(|st| st.extras.is_baby),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    if st.extras.is_baby {
        "entity/cat/ocelot_baby".into()
    } else {
        "entity/cat/ocelot".into()
    }
}
