use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::feline;
use crate::entities::registry::Registry;
use crate::entities::render::{add_adult_and_baby, aged_texture};
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    add_adult_and_baby(
        registry,
        &[EntityKind::Ocelot],
        RenderSpec::new("ocelot", feline::ocelot_layer, texture, feline::setup_anim),
        RenderSpec::new(
            "ocelot_baby",
            feline::baby_layer,
            texture,
            feline::baby_setup_anim,
        ),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    aged_texture(st, "entity/cat/ocelot", "entity/cat/ocelot_baby")
}
