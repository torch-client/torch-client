use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::goat;
use crate::entities::registry::Registry;
use crate::entities::render::{add_adult_and_baby, aged_texture};
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    add_adult_and_baby(
        registry,
        &[EntityKind::Goat],
        RenderSpec::new("goat", goat::layer, texture, goat::setup_anim),
        RenderSpec::new(
            "goat_baby",
            goat::baby_layer,
            texture,
            goat::baby_setup_anim,
        ),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    aged_texture(st, "entity/goat/goat", "entity/goat/goat_baby")
}
