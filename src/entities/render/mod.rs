use crate::entities::registry::Registry;

pub mod animals;
pub mod aquatic;
pub mod humanoid;
pub mod monsters;
pub mod objects;

pub fn register_all(registry: &mut Registry) {
    animals::register(registry);
    aquatic::register(registry);
    humanoid::register(registry);
    monsters::register(registry);
    objects::register(registry);
}
