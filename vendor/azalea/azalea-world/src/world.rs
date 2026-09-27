use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Debug},
};

use azalea_block::{BlockState, fluid_state::FluidState};
use azalea_core::{
    position::{BlockPos, ChunkPos},
    registry_holder::RegistryHolder,
};
use azalea_registry::data::Biome;
use bevy_ecs::entity::Entity;
use nohash_hasher::IntMap;

use crate::{ChunkStorage, PartialChunkStorage};

pub struct PartialWorld {
    pub chunks: PartialChunkStorage,
    pub entity_infos: PartialEntityInfos,
}

impl PartialWorld {
    pub fn new(chunk_radius: u32, owner_entity: Option<Entity>) -> Self {
        PartialWorld {
            chunks: PartialChunkStorage::new(chunk_radius),
            entity_infos: PartialEntityInfos::new(owner_entity),
        }
    }

    pub fn reset(&mut self) {
        self.chunks = PartialChunkStorage::new(self.chunks.chunk_radius);
    }
}

#[deprecated = "moved to `azalea_core::entity_id::MinecraftEntityId`."]
pub type MinecraftEntityId = azalea_core::entity_id::MinecraftEntityId;

#[derive(Debug, Default)]
pub struct PartialEntityInfos {
    pub owner_entity: Option<Entity>,
    pub updates_received: IntMap<azalea_core::entity_id::MinecraftEntityId, u32>,
}

impl PartialEntityInfos {
    pub fn new(owner_entity: Option<Entity>) -> Self {
        Self {
            owner_entity,
            updates_received: IntMap::default(),
        }
    }
}

#[derive(Debug, Default)]
#[doc(alias("instance", "dimension", "level"))]
pub struct World {
    pub chunks: ChunkStorage,

    pub entities_by_chunk: HashMap<ChunkPos, HashSet<Entity>>,

    pub entity_by_id: IntMap<azalea_core::entity_id::MinecraftEntityId, Entity>,

    pub registries: RegistryHolder,
}

#[deprecated = "renamed to `World`."]
pub type Instance = World;

impl World {
    pub fn get_block_state(&self, pos: BlockPos) -> Option<BlockState> {
        self.chunks.get_block_state(pos)
    }

    pub fn get_fluid_state(&self, pos: BlockPos) -> Option<FluidState> {
        self.chunks.get_block_state(pos).map(FluidState::from)
    }

    pub fn get_biome(&self, pos: BlockPos) -> Option<Biome> {
        self.chunks.get_biome(pos)
    }

    pub fn set_block_state(&self, pos: BlockPos, state: BlockState) -> Option<BlockState> {
        self.chunks.set_block_state(pos, state)
    }
}

impl Debug for PartialWorld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PartialWorld")
            .field("chunks", &self.chunks)
            .field("entity_infos", &self.entity_infos)
            .finish()
    }
}

impl Default for PartialWorld {
    fn default() -> Self {
        let chunk_storage = PartialChunkStorage::default();
        let entity_storage = PartialEntityInfos::default();
        Self {
            chunks: chunk_storage,
            entity_infos: entity_storage,
        }
    }
}

impl From<ChunkStorage> for World {
    fn from(chunks: ChunkStorage) -> Self {
        Self {
            chunks,
            entities_by_chunk: HashMap::new(),
            entity_by_id: IntMap::default(),
            registries: RegistryHolder::default(),
        }
    }
}
