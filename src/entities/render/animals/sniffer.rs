use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::sniffer;
use crate::entities::registry::Registry;
use crate::entities::render::{add_adult_and_baby, aged_texture};
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    add_adult_and_baby(
        registry,
        &[EntityKind::Sniffer],
        RenderSpec::new("sniffer", sniffer::layer, texture, sniffer::setup_anim),
        RenderSpec::new(
            "snifflet",
            sniffer::baby_layer,
            texture,
            sniffer::baby_setup_anim,
        ),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    aged_texture(st, "entity/sniffer/sniffer", "entity/sniffer/snifflet")
}
