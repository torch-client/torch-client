pub mod shared_impls;

use std::fmt::Debug;

use azalea_entity::{EntityKindComponent, EntityUuid};
use azalea_registry::builtin::EntityKind;
use bevy_ecs::{
    component::Component,
    entity::Entity,
    query::{QueryData, QueryEntityError, QueryItem},
};
use parking_lot::MappedRwLockReadGuard;
use uuid::Uuid;

use crate::{
    Client,
    client_impl::error::{AzaleaResult, MissingComponentError},
};

#[derive(Clone)]
pub struct EntityRef {
    client: Client,
    entity: Entity,
}

impl EntityRef {
    pub fn new(client: Client, entity: Entity) -> Self {
        Self { client, entity }
    }

    pub fn id(&self) -> Entity {
        self.entity
    }

    pub fn component<T: Component>(
        &self,
    ) -> Result<MappedRwLockReadGuard<'_, T>, MissingComponentError> {
        self.client.entity_component(self.entity)
    }

    pub fn get_component<T: Component>(&self) -> Option<MappedRwLockReadGuard<'_, T>> {
        self.client.get_entity_component(self.entity)
    }

    pub fn query_self<D: QueryData, R>(
        &self,
        f: impl FnOnce(QueryItem<D>) -> R,
    ) -> AzaleaResult<R> {
        self.client.query_entity(self.entity, f)
    }

    #[doc(hidden)]
    #[deprecated = "replaced with `Self::query_self`."]
    pub fn try_query_self<D: QueryData, R>(
        &self,
        f: impl FnOnce(QueryItem<D>) -> R,
    ) -> Result<R, QueryEntityError> {
        #[allow(deprecated)]
        self.client.try_query_entity(self.entity, f)
    }
}

impl Debug for EntityRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EntityRef")
            .field("client", &self.client.entity)
            .field("entity", &self.entity)
            .finish()
    }
}

impl EntityRef {
    pub fn kind(&self) -> AzaleaResult<EntityKind> {
        Ok(**self.component::<EntityKindComponent>()?)
    }

    pub fn uuid(&self) -> AzaleaResult<Uuid> {
        Ok(**self.component::<EntityUuid>()?)
    }
}

impl EntityRef {
    pub fn attack(&self) {
        self.client.attack(self.entity);
    }

    pub fn interact(&self) {
        self.client.entity_interact(self.entity);
    }

    pub fn look_at(&self) -> AzaleaResult<()> {
        self.client.look_at(self.eye_position()?);
        Ok(())
    }

    pub fn distance_to_client(&self) -> AzaleaResult<f64> {
        Ok(self.position()?.distance_to(self.client.position()?))
    }
}
