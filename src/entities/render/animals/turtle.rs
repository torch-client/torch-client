use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::turtle;
use crate::entities::registry::Registry;
use crate::entities::render::{add_adult_and_baby, aged_texture};
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    add_adult_and_baby(
        registry,
        &[EntityKind::Turtle],
        RenderSpec::new("turtle", turtle::layer, texture, turtle::setup_anim),
        RenderSpec::new(
            "turtle_baby",
            turtle::baby_layer,
            texture,
            turtle::setup_anim,
        ),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    aged_texture(st, "entity/turtle/turtle", "entity/turtle/turtle_baby")
}
