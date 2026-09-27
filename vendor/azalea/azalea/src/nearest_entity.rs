use azalea_core::entity_id::MinecraftEntityId;
use azalea_entity::Position;
use azalea_world::WorldName;
use bevy_ecs::{
    prelude::Entity,
    query::{QueryFilter, With},
    system::{Query, SystemParam},
};

#[derive(SystemParam)]
pub struct EntityFinder<'w, 's, F = ()>
where
    F: QueryFilter + 'static,
{
    all_entities: Query<'w, 's, (&'static Position, &'static WorldName), With<MinecraftEntityId>>,

    filtered_entities: Query<
        'w,
        's,
        (Entity, &'static WorldName, &'static Position),
        (With<MinecraftEntityId>, F),
    >,
}

impl<'a, F> EntityFinder<'_, '_, F>
where
    F: QueryFilter + 'static,
{
    pub fn nearest_to_position(
        &'a self,
        position: Position,
        world_name: &WorldName,
        max_distance: f64,
    ) -> Option<Entity> {
        let mut nearest_entity = None;
        let mut min_distance = max_distance;

        for (target_entity, e_world, e_pos) in self.filtered_entities.iter() {
            if e_world != world_name {
                continue;
            }

            let target_distance = position.distance_to(**e_pos);
            if target_distance < min_distance {
                nearest_entity = Some(target_entity);
                min_distance = target_distance;
            }
        }

        nearest_entity
    }

    pub fn nearest_to_entity(&'a self, entity: Entity, max_distance: f64) -> Option<Entity> {
        let Ok((position, world_name)) = self.all_entities.get(entity) else {
            return None;
        };

        let mut nearest_entity = None;
        let mut min_distance = max_distance;

        for (target_entity, e_world, e_pos) in self.filtered_entities.iter() {
            if entity == target_entity {
                continue;
            };

            if e_world != world_name {
                continue;
            }

            let target_distance = position.distance_to(**e_pos);
            if target_distance < min_distance {
                nearest_entity = Some(target_entity);
                min_distance = target_distance;
            }
        }

        nearest_entity
    }

    pub fn nearby_entities_to_position(
        &'a self,
        position: &'a Position,
        world_name: &'a WorldName,
        max_distance: f64,
    ) -> impl Iterator<Item = (Entity, f64)> + 'a {
        self.filtered_entities
            .iter()
            .filter_map(move |(target_entity, e_world, e_pos)| {
                if e_world != world_name {
                    return None;
                }

                let distance = position.distance_to(**e_pos);
                if distance < max_distance {
                    Some((target_entity, distance))
                } else {
                    None
                }
            })
    }

    pub fn nearby_entities_to_entity(
        &'a self,
        entity: Entity,
        max_distance: f64,
    ) -> impl Iterator<Item = (Entity, f64)> + 'a {
        let position;
        let world_name;
        if let Ok((p, w)) = self.all_entities.get(entity) {
            position = *p;
            world_name = Some(w);
        } else {
            position = Position::default();
            world_name = None;
        };

        self.filtered_entities
            .iter()
            .filter_map(move |(target_entity, e_world, e_pos)| {
                if entity == target_entity {
                    return None;
                }

                if Some(e_world) != world_name {
                    return None;
                }

                let distance = position.distance_to(**e_pos);
                if distance < max_distance {
                    Some((target_entity, distance))
                } else {
                    None
                }
            })
    }
}
