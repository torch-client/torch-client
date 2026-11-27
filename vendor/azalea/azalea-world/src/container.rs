use std::{
    collections::HashMap,
    fmt::{self, Display},
    sync::{Arc, Weak},
};

use azalea_core::registry_holder::RegistryHolder;
use azalea_registry::identifier::Identifier;
use bevy_ecs::{component::Component, resource::Resource};
use derive_more::{Deref, DerefMut};
use nohash_hasher::IntMap;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use tracing::{debug, info};

use crate::{ChunkStorage, World};

#[derive(Default, Resource)]
pub struct Worlds {

    pub map: FxHashMap<WorldName, Weak<RwLock<World>>>,
}

impl Worlds {
    pub fn new() -> Self {
        Worlds::default()
    }

    pub fn get(&self, name: &WorldName) -> Option<Arc<RwLock<World>>> {
        self.map.get(name).and_then(|world| world.upgrade())
    }

    #[must_use = "the world will be immediately forgotten if unused"]
    pub fn get_or_insert(
        &mut self,
        name: WorldName,
        height: u32,
        min_y: i32,
        default_registries: &RegistryHolder,
    ) -> Arc<RwLock<World>> {
        let mut carried = None;
        if let Some(existing_lock) = self.map.get(&name).and_then(|world| world.upgrade()) {
            let (old_height, old_min_y) = {
                let existing = existing_lock.read();
                (existing.chunks.height(), existing.chunks.min_y())
            };
            if old_height == height && old_min_y == min_y {
                return existing_lock;
            }
            {
                let mut existing = existing_lock.write();
                carried = Some((
                    std::mem::take(&mut existing.entities_by_chunk),
                    std::mem::take(&mut existing.entity_by_id),
                ));
            }
            info!(
                "World {name:?} changed shape from {old_height}/{old_min_y} to \
                 {height}/{min_y}; rebuilding its chunk storage"
            );
        }

        let (entities_by_chunk, entity_by_id) = carried.unwrap_or_default();
        let world = Arc::new(RwLock::new(World {
            chunks: ChunkStorage::new(height, min_y),
            entities_by_chunk,
            entity_by_id,
            registries: default_registries.clone(),
        }));
        debug!("Added new world {name:?}");
        self.map.insert(name, Arc::downgrade(&world));
        world
    }
}

#[derive(Clone, Component, Debug, Deref, DerefMut, Eq, Hash, PartialEq)]
#[doc(alias("dimension"))]
pub struct WorldName(pub Identifier);
impl WorldName {
    pub fn new(name: &str) -> Self {
        Self(Identifier::new(name))
    }
}
impl Display for WorldName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl From<Identifier> for WorldName {
    fn from(ident: Identifier) -> Self {
        Self(ident)
    }
}
