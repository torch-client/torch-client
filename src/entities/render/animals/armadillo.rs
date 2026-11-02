use azalea_registry::builtin::EntityKind;

use crate::entities::RenderSpec;
use crate::entities::TexturePath;
use crate::entities::models::animals::armadillo;
use crate::entities::registry::Registry;
use crate::entities::render::{add_adult_and_baby, aged_texture};
use crate::entities::state::EntityState;

pub fn register(registry: &mut Registry) {
    add_adult_and_baby(
        registry,
        &[EntityKind::Armadillo],
        RenderSpec::new(
            "armadillo",
            armadillo::layer,
            texture,
            armadillo::setup_anim,
        ),
        RenderSpec::new(
            "armadillo_baby",
            armadillo::baby_layer,
            texture,
            armadillo::baby_setup_anim,
        ),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    aged_texture(
        st,
        "entity/armadillo/armadillo",
        "entity/armadillo/armadillo_baby",
    )
}
