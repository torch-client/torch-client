use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::models::animals::{cow, quadruped};
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Mooshroom,
        RenderSpec::new(
            "mooshroom",
            cow::layer,
            adult_texture,
            quadruped::setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby),
    );
    registry.add(
        EntityKind::Mooshroom,
        RenderSpec::new(
            "mooshroom_baby",
            cow::baby_layer,
            baby_texture,
            quadruped::setup_anim,
        )
        .with_visible(|st| st.extras.is_baby),
    );
}

fn variant(st: &EntityState) -> &'static str {
    if st.extras.variant_id == 1 {
        "brown"
    } else {
        "red"
    }
}

fn adult_texture(st: &EntityState) -> String {
    format!("entity/cow/mooshroom_{}", variant(st))
}

fn baby_texture(st: &EntityState) -> String {
    format!("entity/cow/mooshroom_{}_baby", variant(st))
}
