use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::models::animals::polar_bear;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::PolarBear,
        RenderSpec::new(
            "polar_bear",
            polar_bear::layer,
            texture,
            polar_bear::setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby),
    );
    registry.add(
        EntityKind::PolarBear,
        RenderSpec::new(
            "polar_bear_baby",
            polar_bear::baby_layer,
            texture,
            polar_bear::setup_anim,
        )
        .with_visible(|st| st.extras.is_baby),
    );
}

fn texture(st: &EntityState) -> String {
    if st.extras.is_baby {
        "entity/bear/polarbear_baby".to_string()
    } else {
        "entity/bear/polarbear".to_string()
    }
}
