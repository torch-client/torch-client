use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::polar_bear;
use crate::entities::registry::Registry;
use crate::entities::render::{add_adult_and_baby, aged_texture};
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    add_adult_and_baby(
        registry,
        &[EntityKind::PolarBear],
        RenderSpec::new(
            "polar_bear",
            polar_bear::layer,
            texture,
            polar_bear::setup_anim,
        ),
        RenderSpec::new(
            "polar_bear_baby",
            polar_bear::baby_layer,
            texture,
            polar_bear::setup_anim,
        ),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    aged_texture(st, "entity/bear/polarbear", "entity/bear/polarbear_baby")
}
