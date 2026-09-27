use std::{
    any::{Any, TypeId},
    sync::Arc,
};

use bevy_ecs::component::Component;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;

#[derive(Clone, Component, Default)]
pub struct CustomPathfinderState(pub Arc<RwLock<CustomPathfinderStateRef>>);

#[derive(Debug, Default)]
pub struct CustomPathfinderStateRef {
    map: FxHashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl CustomPathfinderStateRef {
    pub fn insert<T: 'static + Send + Sync>(&mut self, t: T) {
        self.map.insert(TypeId::of::<T>(), Box::new(t));
    }

    pub fn get<T: 'static + Send + Sync>(&self) -> Option<&T> {
        self.map
            .get(&TypeId::of::<T>())
            .map(|value| value.downcast_ref().unwrap())
    }
}
