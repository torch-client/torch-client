use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::goat;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Goat,
        RenderSpec::new("goat", goat::layer, texture, goat::setup_anim)
            .with_visible(|st| !st.extras.is_baby),
    );
    registry.add(
        EntityKind::Goat,
        RenderSpec::new(
            "goat_baby",
            goat::baby_layer,
            texture,
            goat::baby_setup_anim,
        )
        .with_visible(|st| st.extras.is_baby),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    if st.extras.is_baby {
        "entity/goat/goat_baby".into()
    } else {
        "entity/goat/goat".into()
    }
}
