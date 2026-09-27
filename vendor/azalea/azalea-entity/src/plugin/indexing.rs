use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Debug},
};

use azalea_core::{entity_id::MinecraftEntityId, position::ChunkPos};
use azalea_world::{World, WorldName, Worlds};
use bevy_ecs::prelude::*;
use derive_more::{Deref, DerefMut};
use nohash_hasher::IntMap;
use tracing::{debug, trace, warn};
use uuid::Uuid;

use super::LoadedBy;
use crate::{EntityUuid, LocalEntity, Position};

#[derive(Default, Resource)]
pub struct EntityUuidIndex {
    entity_by_uuid: HashMap<Uuid, Entity>,
}
impl EntityUuidIndex {
    pub fn new() -> Self {
        Self {
            entity_by_uuid: HashMap::default(),
        }
    }

    pub fn get(&self, uuid: &Uuid) -> Option<Entity> {
        self.entity_by_uuid.get(uuid).copied()
    }

    pub fn contains_key(&self, uuid: &Uuid) -> bool {
        self.entity_by_uuid.contains_key(uuid)
    }

    pub fn insert(&mut self, uuid: Uuid, entity: Entity) {
        self.entity_by_uuid.insert(uuid, entity);
    }

    pub fn remove(&mut self, uuid: &Uuid) -> Option<Entity> {
        self.entity_by_uuid.remove(uuid)
    }
}

#[derive(Component, Default)]
pub struct EntityIdIndex {
    entity_by_id: IntMap<MinecraftEntityId, Entity>,
    id_by_entity: HashMap<Entity, MinecraftEntityId>,
}

impl EntityIdIndex {
    pub fn get_by_minecraft_entity(&self, id: MinecraftEntityId) -> Option<Entity> {
        self.entity_by_id.get(&id).copied()
    }
    pub fn get_by_ecs_entity(&self, entity: Entity) -> Option<MinecraftEntityId> {
        self.id_by_entity.get(&entity).copied()
    }

    pub fn contains_minecraft_entity(&self, id: MinecraftEntityId) -> bool {
        self.entity_by_id.contains_key(&id)
    }
    pub fn contains_ecs_entity(&self, id: Entity) -> bool {
        self.id_by_entity.contains_key(&id)
    }

    pub fn insert(&mut self, id: MinecraftEntityId, entity: Entity) {
        self.entity_by_id.insert(id, entity);
        self.id_by_entity.insert(entity, id);
        trace!("Inserted {id} -> {entity:?} into a client's EntityIdIndex");
    }

    pub fn remove_by_minecraft_entity(&mut self, id: MinecraftEntityId) -> Option<Entity> {
        if let Some(entity) = self.entity_by_id.remove(&id) {
            trace!(
                "Removed {id} -> {entity:?} from a client's EntityIdIndex (using EntityIdIndex::remove)"
            );
            self.id_by_entity.remove(&entity);
            Some(entity)
        } else {
            trace!(
                "Failed to remove {id} from a client's EntityIdIndex (using EntityIdIndex::remove)"
            );
            None
        }
    }

    pub fn remove_by_ecs_entity(&mut self, entity: Entity) -> Option<MinecraftEntityId> {
        if let Some(id) = self.id_by_entity.remove(&entity) {
            trace!(
                "Removed {id} -> {entity:?} from a client's EntityIdIndex (using EntityIdIndex::remove_by_ecs_entity)."
            );
            self.entity_by_id.remove(&id);
            Some(id)
        } else {
            trace!(
                "Failed to remove {entity:?} from a client's EntityIdIndex (using EntityIdIndex::remove_by_ecs_entity). This may be expected behavior."
            );
            None
        }
    }
}

impl Debug for EntityUuidIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EntityUuidIndex").finish()
    }
}

#[derive(Component, Debug, Deref, DerefMut)]
pub struct EntityChunkPos(pub ChunkPos);

pub fn update_entity_chunk_positions(
    mut query: Query<(Entity, &Position, &WorldName, &mut EntityChunkPos), Changed<Position>>,
    worlds: Res<Worlds>,
) {
    for (entity, pos, world_name, mut entity_chunk_pos) in query.iter_mut() {
        let old_chunk = **entity_chunk_pos;
        let new_chunk = ChunkPos::from(*pos);
        if old_chunk != new_chunk {
            **entity_chunk_pos = new_chunk;

            if old_chunk != new_chunk {
                let Some(world_lock) = worlds.get(world_name) else {
                    continue;
                };
                let mut world = world_lock.write();

                if let Some(entities) = world.entities_by_chunk.get_mut(&old_chunk) {
                    entities.remove(&entity);
                }
                world
                    .entities_by_chunk
                    .entry(new_chunk)
                    .or_default()
                    .insert(entity);
                trace!("Entity {entity:?} moved from {old_chunk:?} to {new_chunk:?}");
            }
        }
    }
}

pub fn insert_entity_chunk_position(
    query: Query<(Entity, &Position, &WorldName), Added<EntityChunkPos>>,
    worlds: Res<Worlds>,
) {
    for (entity, pos, world_name) in query.iter() {
        let Some(world_lock) = worlds.get(world_name) else {
            continue;
        };
        let mut world = world_lock.write();

        let chunk = ChunkPos::from(*pos);
        world
            .entities_by_chunk
            .entry(chunk)
            .or_default()
            .insert(entity);
    }
}

#[allow(clippy::type_complexity)]
pub fn remove_despawned_entities_from_indexes(
    mut commands: Commands,
    mut entity_uuid_index: ResMut<EntityUuidIndex>,
    worlds: Res<Worlds>,
    query: Query<
        (
            Entity,
            &EntityUuid,
            &MinecraftEntityId,
            &Position,
            &WorldName,
            &LoadedBy,
        ),
        (Changed<LoadedBy>, Without<LocalEntity>),
    >,
    mut entity_id_index_query: Query<&mut EntityIdIndex>,
) {
    for (entity, uuid, minecraft_id, position, world_name, loaded_by) in &query {
        let Some(world_lock) = worlds.get(world_name) else {
            debug!("Despawned entity {entity:?} because it's in a world that isn't loaded anymore");
            if entity_uuid_index.entity_by_uuid.remove(uuid).is_none() {
                warn!(
                    "Tried to remove entity {entity:?} from the uuid index but it was not there."
                );
            }
            commands.entity(entity).despawn();

            continue;
        };

        let mut world = world_lock.write();

        if !loaded_by.is_empty() {
            continue;
        }

        let chunk = ChunkPos::from(position);
        match world.entities_by_chunk.get_mut(&chunk) {
            Some(entities_in_chunk) => {
                if entities_in_chunk.remove(&entity) {
                    if entities_in_chunk.is_empty() {
                        world.entities_by_chunk.remove(&chunk);
                    }
                } else {
                    let mut found_in_other_chunks = HashSet::new();
                    for (other_chunk, entities_in_other_chunk) in &mut world.entities_by_chunk {
                        if entities_in_other_chunk.remove(&entity) {
                            found_in_other_chunks.insert(other_chunk);
                        }
                    }
                    if found_in_other_chunks.is_empty() {
                        warn!(
                            "Tried to remove entity {entity:?} from chunk {chunk:?} but the entity was not there or in any other chunks."
                        );
                    } else {
                        warn!(
                            "Tried to remove entity {entity:?} from chunk {chunk:?} but the entity was not there. Found in and removed from other chunk(s): {found_in_other_chunks:?}"
                        );
                    }
                }
            }
            _ => {
                let mut found_in_other_chunks = HashSet::new();
                for (other_chunk, entities_in_other_chunk) in &mut world.entities_by_chunk {
                    if entities_in_other_chunk.remove(&entity) {
                        found_in_other_chunks.insert(other_chunk);
                    }
                }
                if found_in_other_chunks.is_empty() {
                    warn!(
                        "Tried to remove entity {entity:?} from chunk {chunk:?} but the chunk was not found and the entity wasn't in any other chunks."
                    );
                } else {
                    warn!(
                        "Tried to remove entity {entity:?} from chunk {chunk:?} but the chunk was not found. Entity found in and removed from other chunk(s): {found_in_other_chunks:?}"
                    );
                }
            }
        }
        if entity_uuid_index.entity_by_uuid.remove(uuid).is_none() {
            warn!("Tried to remove entity {entity:?} from the uuid index but it was not there.");
        }
        if world.entity_by_id.remove(minecraft_id).is_none() {
            debug!(
                "Tried to remove entity {entity:?} from the per-world entity id index but it was not there. This may be expected if you're in a shared world."
            );
        }

        for mut entity_id_index in entity_id_index_query.iter_mut() {
            entity_id_index.remove_by_ecs_entity(entity);
        }

        commands.entity(entity).despawn();
        debug!("Despawned entity {entity:?} because it was not loaded by anything.");
    }
}

pub fn add_entity_to_indexes(
    entity_id: MinecraftEntityId,
    ecs_entity: Entity,
    entity_uuid: Option<Uuid>,
    entity_id_index: &mut EntityIdIndex,
    entity_uuid_index: &mut EntityUuidIndex,
    world: &mut World,
) {
    entity_id_index.insert(entity_id, ecs_entity);

    world.entity_by_id.insert(entity_id, ecs_entity);

    if let Some(uuid) = entity_uuid {
        entity_uuid_index.insert(uuid, ecs_entity);
    }
}
