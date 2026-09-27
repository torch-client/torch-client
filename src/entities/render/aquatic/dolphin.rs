use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::models::aquatic::dolphin;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Dolphin,
        RenderSpec::new(
            "dolphin",
            dolphin::dolphin_layer,
            adult_texture,
            dolphin::setup_anim,
        )
        .with_visible(is_adult),
    );
    registry.add(
        EntityKind::Dolphin,
        RenderSpec::new(
            "dolphin_baby",
            dolphin::baby_dolphin_layer,
            baby_texture,
            dolphin::setup_anim,
        )
        .with_visible(is_baby),
    );
}

fn is_adult(st: &EntityState) -> bool {
    !st.extras.is_baby
}

fn is_baby(st: &EntityState) -> bool {
    st.extras.is_baby
}

fn adult_texture(_st: &EntityState) -> String {
    "entity/dolphin/dolphin".to_string()
}

fn baby_texture(_st: &EntityState) -> String {
    "entity/dolphin/dolphin_baby".to_string()
}
