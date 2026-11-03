use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::camel;
use crate::entities::registry::Registry;
use crate::entities::render::{add_adult_and_baby, saddle_spec};
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    add_adult_and_baby(
        registry,
        &[EntityKind::Camel],
        RenderSpec::new("camel", camel::layer, texture, camel::setup_anim),
        RenderSpec::new(
            "camel_baby",
            camel::baby_layer,
            texture,
            camel::baby_setup_anim,
        ),
    );
    registry.add(
        EntityKind::Camel,
        saddle_spec(
            "camel_saddle",
            camel::saddle_layer,
            saddle_texture,
            camel::saddle_setup_anim,
        ),
    );
    registry.add(
        EntityKind::CamelHusk,
        RenderSpec::new("camel_husk", camel::layer, texture, camel::setup_anim),
    );
    registry.add(
        EntityKind::CamelHusk,
        RenderSpec::new(
            "camel_husk_saddle",
            camel::saddle_layer,
            saddle_texture,
            camel::saddle_setup_anim,
        )
        .with_visible(|st| st.extras.saddled)
        .ignoring_invisibility(),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    if st.kind == EntityKind::CamelHusk {
        "entity/camel/camel_husk".into()
    } else if st.extras.is_baby {
        "entity/camel/camel_baby".into()
    } else {
        "entity/camel/camel".into()
    }
}

fn saddle_texture(st: &EntityState) -> TexturePath {
    if st.kind == EntityKind::CamelHusk {
        "entity/equipment/camel_husk_saddle/saddle".into()
    } else {
        "entity/equipment/camel_saddle/saddle".into()
    }
}
