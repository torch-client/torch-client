use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::turtle;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Turtle,
        RenderSpec::new("turtle", turtle::layer, texture, turtle::setup_anim)
            .with_visible(|st| !st.extras.is_baby),
    );
    registry.add(
        EntityKind::Turtle,
        RenderSpec::new(
            "turtle_baby",
            turtle::baby_layer,
            texture,
            turtle::setup_anim,
        )
        .with_visible(|st| st.extras.is_baby),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    if st.extras.is_baby {
        "entity/turtle/turtle_baby".into()
    } else {
        "entity/turtle/turtle".into()
    }
}
