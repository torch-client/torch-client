use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::rabbit;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Rabbit,
        RenderSpec::new("rabbit", rabbit::layer, texture, rabbit::setup_anim)
            .with_visible(|st| !st.extras.is_baby),
    );
    registry.add(
        EntityKind::Rabbit,
        RenderSpec::new(
            "rabbit_baby",
            rabbit::baby_layer,
            texture,
            rabbit::baby_setup_anim,
        )
        .with_visible(|st| st.extras.is_baby),
    );
}

fn variant(st: &EntityState) -> &'static str {
    match st.extras.variant_id {
        1 => "white",
        2 => "black",
        3 => "white_splotched",
        4 => "gold",
        5 => "salt",
        99 => "caerbannog",
        _ => "brown",
    }
}

fn texture(st: &EntityState) -> TexturePath {
    let name = if st.extras.magic_name_toast() {
        "toast"
    } else {
        variant(st)
    };
    if st.extras.is_baby {
        format!("entity/rabbit/rabbit_{name}_baby").into()
    } else {
        format!("entity/rabbit/rabbit_{name}").into()
    }
}
