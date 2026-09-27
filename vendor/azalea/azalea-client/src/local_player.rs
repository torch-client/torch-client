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

#[derive(Clone, Component, Debug, Default, Deref, DerefMut, Resource)]
pub struct TabList(HashMap<Uuid, PlayerInfo>);

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
        self.food >= 6
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
        let registries = self.shared.read().registries.clone();

        let new_world = World {
            registries,
            ..Default::default()
        };
        self.shared = Arc::new(RwLock::new(new_world));

        self.partial.write().reset();
    }
}
