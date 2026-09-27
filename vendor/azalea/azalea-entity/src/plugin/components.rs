use azalea_block::fluid_state::FluidKind;
use azalea_core::position::{BlockPos, ChunkPos, Vec3};
use azalea_registry::builtin::EntityKind;
use azalea_world::WorldName;
use bevy_ecs::{bundle::Bundle, component::Component};
use derive_more::{Deref, DerefMut};
use uuid::Uuid;

use crate::{
    ActiveEffects, Attributes, EntityUuid, FluidOnEyes, LookDirection, Physics, Position,
    dimensions::EntityDimensions, indexing::EntityChunkPos,
};

#[derive(Bundle)]
pub struct EntityBundle {
    pub kind: EntityKindComponent,
    pub uuid: EntityUuid,
    pub world_name: WorldName,
    pub position: Position,
    pub last_sent_position: LastSentPosition,

    pub chunk_pos: EntityChunkPos,

    pub physics: Physics,
    pub direction: LookDirection,
    pub dimensions: EntityDimensions,
    pub attributes: Attributes,
    pub jumping: Jumping,
    pub crouching: Crouching,
    pub fluid_on_eyes: FluidOnEyes,
    pub on_climbable: OnClimbable,
    pub active_effects: ActiveEffects,
}

impl EntityBundle {
    pub fn new(uuid: Uuid, pos: Vec3, kind: EntityKind, world_name: WorldName) -> Self {
        let dimensions = EntityDimensions::from(kind);

        Self {
            kind: EntityKindComponent(kind),
            uuid: EntityUuid(uuid),
            world_name,
            position: Position(pos),
            chunk_pos: EntityChunkPos(ChunkPos::from(&pos)),
            last_sent_position: LastSentPosition(pos),
            physics: Physics::new(&dimensions, pos),
            dimensions,
            direction: LookDirection::default(),

            attributes: Attributes::new(EntityKind::Player),

            jumping: Jumping(false),
            crouching: Crouching(false),
            fluid_on_eyes: FluidOnEyes(FluidKind::Empty),
            on_climbable: OnClimbable(false),
            active_effects: ActiveEffects::default(),
        }
    }
}

#[derive(Clone, Component, Copy, Default)]
pub struct Dead;

#[derive(Clone, Component, Copy, Debug, Deref, PartialEq)]
pub struct EntityKindComponent(pub EntityKind);

#[derive(Clone, Component, Copy, Debug, Default)]
pub struct LocalEntity;

impl FluidOnEyes {
    pub fn new(fluid: FluidKind) -> Self {
        Self(fluid)
    }
}

#[derive(Clone, Component, Copy, Debug, Deref, DerefMut, PartialEq)]
pub struct OnClimbable(bool);

#[derive(Clone, Component, Copy, Default, Deref, DerefMut)]
pub struct Crouching(bool);

#[derive(Component)]
pub struct HasClientLoaded;

#[derive(Clone, Copy, Component, Debug, Default, Deref, DerefMut, PartialEq)]
pub struct LastSentPosition(Vec3);
impl From<&LastSentPosition> for Vec3 {
    fn from(value: &LastSentPosition) -> Self {
        value.0
    }
}
impl From<LastSentPosition> for ChunkPos {
    fn from(value: LastSentPosition) -> Self {
        ChunkPos::from(&value.0)
    }
}
impl From<LastSentPosition> for BlockPos {
    fn from(value: LastSentPosition) -> Self {
        BlockPos::from(&value.0)
    }
}
impl From<&LastSentPosition> for ChunkPos {
    fn from(value: &LastSentPosition) -> Self {
        ChunkPos::from(value.0)
    }
}
impl From<&LastSentPosition> for BlockPos {
    fn from(value: &LastSentPosition) -> Self {
        BlockPos::from(value.0)
    }
}

#[derive(Clone, Copy, Component, Debug, Default, Deref, DerefMut, Eq, PartialEq)]
pub struct Jumping(pub bool);
