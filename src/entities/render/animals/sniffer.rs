use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::sniffer;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Sniffer,
        RenderSpec::new("sniffer", sniffer::layer, texture, sniffer::setup_anim)
            .with_visible(|st| !st.extras.is_baby),
    );
    registry.add(
        EntityKind::Sniffer,
        RenderSpec::new(
            "snifflet",
            sniffer::baby_layer,
            texture,
            sniffer::baby_setup_anim,
        )
        .with_visible(|st| st.extras.is_baby),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    if st.extras.is_baby {
        "entity/sniffer/snifflet".into()
    } else {
        "entity/sniffer/sniffer".into()
    }
}
