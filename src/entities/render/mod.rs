use azalea_registry::builtin::EntityKind;

use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{RenderSpec, TexturePath};

pub mod animals;
pub mod aquatic;
pub mod humanoid;
pub mod monsters;
pub mod objects;

fn add_adult_and_baby(
    registry: &mut Registry,
    kinds: &[EntityKind],
    adult: RenderSpec,
    baby: RenderSpec,
) {
    registry.add_many(kinds, adult.with_visible(|st| !st.extras.is_baby));
    registry.add_many(kinds, baby.with_visible(|st| st.extras.is_baby));
}

fn aged_texture(st: &EntityState, adult: &'static str, baby: &'static str) -> TexturePath {
    (if st.extras.is_baby { baby } else { adult }).into()
}

pub fn register_all(registry: &mut Registry) {
    animals::register(registry);
    aquatic::register(registry);
    humanoid::register(registry);
    monsters::register(registry);
    objects::register(registry);
}
