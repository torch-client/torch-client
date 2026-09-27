use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::models::animals::strider;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Strider,
        RenderSpec::new("strider", strider::layer, texture, strider::setup_anim)
            .with_visible(|st| !st.extras.is_baby),
    );
    registry.add(
        EntityKind::Strider,
        RenderSpec::new(
            "strider_baby",
            strider::baby_layer,
            texture,
            strider::baby_setup_anim,
        )
        .with_visible(|st| st.extras.is_baby),
    );
    registry.add(
        EntityKind::Strider,
        RenderSpec::new(
            "strider_saddle",
            strider::layer,
            saddle_texture,
            strider::setup_anim,
        )
        .with_visible(|st| !st.extras.is_baby && st.extras.saddled),
    );
}

fn texture(st: &EntityState) -> String {
    match (st.extras.suffocating, st.extras.is_baby) {
        (true, true) => "entity/strider/strider_cold_baby".to_string(),
        (true, false) => "entity/strider/strider_cold".to_string(),
        (false, true) => "entity/strider/strider_baby".to_string(),
        (false, false) => "entity/strider/strider".to_string(),
    }
}

fn saddle_texture(_st: &EntityState) -> String {
    "entity/equipment/strider_saddle/saddle".to_string()
}
