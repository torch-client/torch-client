use std::{collections::HashMap, sync::Arc};

use azalea_core::game_type::GameMode;
use azalea_world::{PartialWorld, World};
use bevy_ecs::{component::Component, prelude::*};
use derive_more::{Deref, DerefMut};
use parking_lot::RwLock;
use uuid::Uuid;

use crate::{ClientInformation, player::PlayerInfo};

#[derive(Clone, Component)]
pub struct WorldHolder {
    pub partial: Arc<RwLock<PartialWorld>>,
    pub shared: Arc<RwLock<World>>,
}
#[deprecated = "renamed to `WorldHolder`."]
pub type InstanceHolder = WorldHolder;

#[derive(Clone, Component, Copy, Debug)]
pub struct PreviousGameMode(pub Option<GameMode>);

#[derive(Clone, Component, Default, Deref, DerefMut)]
pub struct PermissionLevel(pub u8);

#[derive(Clone, Debug, Default, Deref, DerefMut, Component)]
pub struct TabList(HashMap<Uuid, PlayerInfo>);

#[derive(Clone, Debug, Default, Deref, DerefMut, Resource)]
pub struct TabListResource(HashMap<Uuid, PlayerInfo>);

impl From<TabList> for TabListResource {
    fn from(t: TabList) -> Self {
        TabListResource(t.0)
    }
}
impl From<TabListResource> for TabList {
    fn from(t: TabListResource) -> Self {
        TabList(t.0)
    }
}

#[derive(Clone, Component, Debug)]
pub struct Hunger {
    pub food: u32,
    pub saturation: f32,
}

impl Default for Hunger {
    fn default() -> Self {
        Hunger {
            food: 20,
            saturation: 5.,
        }
    }
}
impl Hunger {
    pub fn is_enough_to_sprint(&self) -> bool {
        self.food > 6
    }
}

#[cfg(test)]
mod hunger_tests {
    use super::Hunger;

    #[test]
    fn sprint_requires_more_than_six_food_points() {
        for food in 0..=20 {
            let hunger = Hunger { food, saturation: 0.0 };
            assert_eq!(hunger.is_enough_to_sprint(), food > 6, "food={food}");
        }
    }
}

#[derive(Clone, Component, Debug)]
pub struct Experience {
    pub progress: f32,
    pub level: u32,
    pub total: u32,
}

impl Default for Experience {
    fn default() -> Self {
        Experience {
            progress: 0.0,
            level: 0,
            total: 0,
        }
    }
}

impl WorldHolder {
    pub fn new(entity: Entity, shared: Arc<RwLock<World>>) -> Self {
        let client_information = ClientInformation::default();

        WorldHolder {
            shared,
            partial: Arc::new(RwLock::new(PartialWorld::new(
                azalea_world::chunk::calculate_chunk_storage_range(
                    client_information.view_distance.into(),
                ),
                Some(entity),
            ))),
        }
    }

    pub fn reset(&mut self) {
        self.shared = Arc::new(RwLock::new(World::default()));

        self.partial.write().reset();
    }
}
