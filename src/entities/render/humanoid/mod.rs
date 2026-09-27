use crate::entities::registry::Registry;

pub mod armor;
pub mod armor_stand;
pub mod elytra;
pub mod illager;
pub mod piglin;
pub mod skeleton;
pub mod villager;
pub mod zombie;

pub fn register(registry: &mut Registry) {
    zombie::register(registry);
    skeleton::register(registry);
    villager::register(registry);
    illager::register(registry);
    piglin::register(registry);
    armor_stand::register(registry);
}
