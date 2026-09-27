use std::{any, sync::Arc};

use azalea_core::position::Vec3;
use azalea_entity::{LocalEntity, Position, metadata};
use azalea_world::WorldName;
use bevy_ecs::{
    component::Component,
    entity::Entity,
    query::{QueryData, QueryEntityError, QueryFilter, QueryItem, ROQueryItem, With, Without},
    world::World,
};
use parking_lot::{MappedRwLockReadGuard, RwLock, RwLockReadGuard};

use crate::{
    Client,
    client_impl::error::{AzaleaResult, MissingComponentError},
    entity_ref::EntityRef,
};

impl Client {
    pub fn component<T: Component>(
        &self,
    ) -> Result<MappedRwLockReadGuard<'_, T>, MissingComponentError> {
        self.entity_component::<T>(self.entity).map_err(|mut err| {
            err.entity_description = "Player";
            err
        })
    }

    #[doc(hidden)]
    #[deprecated = "replaced with `Self::component`."]
    pub fn get_component<T: Component>(&self) -> Option<MappedRwLockReadGuard<'_, T>> {
        self.component().ok()
    }

    pub fn query_self<D: QueryData, R>(
        &self,
        f: impl FnOnce(QueryItem<D>) -> R,
    ) -> AzaleaResult<R> {
        self.query_entity(self.entity, f).map_err(|mut err| {
            err.entity_description = "Player";
            err
        })
    }

    #[doc(hidden)]
    #[deprecated = "replaced with `Self::query_self`."]
    pub fn try_query_self<D: QueryData, R>(
        &self,
        f: impl FnOnce(QueryItem<D>) -> R,
    ) -> Result<R, QueryEntityError> {
        let mut ecs = self.ecs.write();
        let mut qs = ecs.query::<D>();
        qs.get_mut(&mut ecs, self.entity).map(f)
    }

    pub fn query_entity<D: QueryData, R>(
        &self,
        entity: Entity,
        f: impl FnOnce(QueryItem<D>) -> R,
    ) -> AzaleaResult<R> {
        let mut ecs = self.ecs.write();
        let mut qs = ecs.query::<D>();
        qs.get_mut(&mut ecs, entity)
            .map(f)
            .map_err(|_| MissingComponentError {
                entity_description: "Entity",
                entity,
                component: any::type_name::<D>(),
            })
    }

    #[doc(hidden)]
    #[deprecated = "replaced with `Self::query_entity`."]
    pub fn try_query_entity<D: QueryData, R>(
        &self,
        entity: Entity,
        f: impl FnOnce(QueryItem<D>) -> R,
    ) -> Result<R, QueryEntityError> {
        let mut ecs = self.ecs.write();
        let mut qs = ecs.query::<D>();
        qs.get_mut(&mut ecs, entity).map(f)
    }

    pub fn any_entity_by<Q: QueryData, F: QueryFilter>(
        &self,
        predicate: impl EntityPredicate<Q, F>,
    ) -> AzaleaResult<Option<EntityRef>> {
        Ok(self
            .any_entity_id_by(predicate)?
            .map(|e| self.entity_ref_for(e)))
    }
    pub fn any_entity_id_by<Q: QueryData, F: QueryFilter>(
        &self,
        predicate: impl EntityPredicate<Q, F>,
    ) -> AzaleaResult<Option<Entity>> {
        let world_name = self.component::<WorldName>()?.clone();
        Ok(predicate.find_any(self.ecs.clone(), &world_name))
    }

    pub fn nearest_entity_by<Q: QueryData, F: QueryFilter>(
        &self,
        predicate: impl EntityPredicate<Q, F>,
    ) -> AzaleaResult<Option<EntityRef>> {
        Ok(self
            .nearest_entity_id_by(predicate)?
            .map(|e| self.entity_ref_for(e)))
    }
    pub fn nearest_entity_id_by<Q: QueryData, F: QueryFilter>(
        &self,
        predicate: impl EntityPredicate<Q, F>,
    ) -> AzaleaResult<Option<Entity>> {
        Ok(self.nearest_entity_ids_by(predicate)?.first().copied())
    }

    pub fn nearest_entities_by<Q: QueryData, F: QueryFilter>(
        &self,
        predicate: impl EntityPredicate<Q, F>,
    ) -> AzaleaResult<Box<[EntityRef]>> {
        Ok(self
            .nearest_entity_ids_by(predicate)?
            .into_iter()
            .map(|e| self.entity_ref_for(e))
            .collect())
    }
    pub fn nearest_entities<F: QueryFilter>(&self) -> AzaleaResult<Box<[EntityRef]>> {
        self.nearest_entities_by::<(), F>(|_| true)
    }

    pub fn nearby_players(&self) -> AzaleaResult<Box<[EntityRef]>> {
        self.nearest_entities::<(With<metadata::Player>, Without<LocalEntity>)>()
    }

    pub fn nearest_entity_ids_by<Q: QueryData, F: QueryFilter>(
        &self,
        predicate: impl EntityPredicate<Q, F>,
    ) -> AzaleaResult<Box<[Entity]>> {
        let (world_name, position) = {
            let world_name = self.component::<WorldName>()?;
            let position = self.component::<Position>()?;

            (world_name.clone(), **position)
        };

        Ok(predicate.find_all_sorted(self.ecs.clone(), &world_name, position))
    }

    pub fn entity_component<T: Component>(
        &self,
        entity: Entity,
    ) -> Result<MappedRwLockReadGuard<'_, T>, MissingComponentError> {
        self.get_entity_component::<T>(entity)
            .ok_or_else(|| MissingComponentError {
                entity_description: "Entity",
                entity,
                component: any::type_name::<T>(),
            })
    }

    pub fn get_entity_component<T: Component>(
        &self,
        entity: Entity,
    ) -> Option<MappedRwLockReadGuard<'_, T>> {
        let ecs = self.ecs.read();
        RwLockReadGuard::try_map(ecs, |ecs: &World| ecs.get(entity)).ok()
    }
}

pub trait EntityPredicate<Q: QueryData, Filter: QueryFilter> {
    fn find_any(&self, ecs_lock: Arc<RwLock<World>>, world_name: &WorldName) -> Option<Entity>;
    fn find_all_sorted(
        &self,
        ecs_lock: Arc<RwLock<World>>,
        world_name: &WorldName,
        nearest_to: Vec3,
    ) -> Box<[Entity]>;
}
impl<F, Q: QueryData, Filter: QueryFilter> EntityPredicate<Q, Filter> for F
where
    F: Fn(ROQueryItem<Q>) -> bool,
    for<'w, 's> <<Q as QueryData>::ReadOnly as QueryData>::Item<'w, 's>: Copy,
{
    fn find_any(&self, ecs_lock: Arc<RwLock<World>>, world_name: &WorldName) -> Option<Entity> {
        let mut ecs = ecs_lock.write();
        let mut query = ecs.query_filtered::<(Entity, &WorldName, Q), Filter>();
        query
            .iter(&ecs)
            .find(|(_, e_world_name, q)| *e_world_name == world_name && (self)(*q))
            .map(|(e, _, _)| e)
    }

    fn find_all_sorted(
        &self,
        ecs_lock: Arc<RwLock<World>>,
        world_name: &WorldName,
        nearest_to: Vec3,
    ) -> Box<[Entity]> {
        let mut ecs = ecs_lock.write();
        let mut query = ecs.query_filtered::<(Entity, &WorldName, &Position, Q), Filter>();
        let mut entities = query
            .iter(&ecs)
            .filter(|(_, e_world_name, _, q)| *e_world_name == world_name && (self)(*q))
            .map(|(e, _, position, _)| (e, Vec3::from(position)))
            .collect::<Vec<(Entity, Vec3)>>();

        entities.sort_by_cached_key(|(_, position)| {
            position.distance_squared_to(nearest_to).to_bits()
        });

        entities
            .into_iter()
            .map(|(e, _)| e)
            .collect::<Box<[Entity]>>()
    }
}
