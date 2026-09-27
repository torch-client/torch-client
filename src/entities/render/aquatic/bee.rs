use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
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

fn texture(st: &EntityState) -> String {
    let mut name = String::from("bee");
    if st.extras.bee_angry {
        name.push_str("_angry");
    }
    if st.extras.has_nectar {
        name.push_str("_nectar");
    }
    if st.extras.is_baby {
        name.push_str("_baby");
    }
    format!("entity/bee/{name}")
}
