use bevy_ecs::entity::Entity;
use thiserror::Error;

#[derive(Error, Debug)]
#[error("{entity_description} {entity} is missing a required component: '{component}'")]
pub struct MissingComponentError {
    pub entity_description: &'static str,
    pub entity: Entity,
    pub component: &'static str,
}

pub type AzaleaResult<T> = Result<T, MissingComponentError>;
