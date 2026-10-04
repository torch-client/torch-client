use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::aquatic::bee;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Bee,
        RenderSpec::new("bee", bee::adult_layer, texture, bee::setup_anim).with_visible(is_adult),
    );
    registry.add(
        EntityKind::Bee,
        RenderSpec::new("bee_baby", bee::baby_layer, texture, bee::setup_anim)
            .with_visible(is_baby),
    );
}

fn is_adult(st: &EntityState) -> bool {
    !st.extras.is_baby
}

fn is_baby(st: &EntityState) -> bool {
    st.extras.is_baby
}

fn texture(st: &EntityState) -> TexturePath {
    match (
        st.extras.bee_angry(),
        st.extras.has_nectar,
        st.extras.is_baby,
    ) {
        (false, false, false) => "entity/bee/bee",
        (false, false, true) => "entity/bee/bee_baby",
        (false, true, false) => "entity/bee/bee_nectar",
        (false, true, true) => "entity/bee/bee_nectar_baby",
        (true, false, false) => "entity/bee/bee_angry",
        (true, false, true) => "entity/bee/bee_angry_baby",
        (true, true, false) => "entity/bee/bee_angry_nectar",
        (true, true, true) => "entity/bee/bee_angry_nectar_baby",
    }
    .into()
}
