use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
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

fn texture(st: &EntityState) -> TexturePath {
    match (st.extras.suffocating, st.extras.is_baby) {
        (true, true) => "entity/strider/strider_cold_baby".into(),
        (true, false) => "entity/strider/strider_cold".into(),
        (false, true) => "entity/strider/strider_baby".into(),
        (false, false) => "entity/strider/strider".into(),
    }
}

fn saddle_texture(_st: &EntityState) -> TexturePath {
    "entity/equipment/strider_saddle/saddle".into()
}
