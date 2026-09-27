use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::models::animals::armadillo;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Armadillo,
        RenderSpec::new(
            "armadillo",
            armadillo::layer,
            texture,
            armadillo::setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby),
    );
    registry.add(
        EntityKind::Armadillo,
        RenderSpec::new(
            "armadillo_baby",
            armadillo::baby_layer,
            texture,
            armadillo::baby_setup_anim,
        )
        .with_visible(|st| st.extras.is_baby),
    );
}

fn texture(st: &EntityState) -> String {
    if st.extras.is_baby {
        "entity/armadillo/armadillo_baby".to_string()
    } else {
        "entity/armadillo/armadillo".to_string()
    }
}
