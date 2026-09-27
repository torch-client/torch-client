use azalea_buf::AzBuf;
use azalea_core::{delta::LpVec3, entity_id::MinecraftEntityId, position::Vec3};
use azalea_protocol_macros::ClientboundGamePacket;
use azalea_registry::builtin::EntityKind;
#[cfg(feature = "bevy_ecs")]
use azalea_world::WorldName;
use uuid::Uuid;

#[derive(AzBuf, ClientboundGamePacket, Clone, Debug, PartialEq)]
pub struct ClientboundAddEntity {
    #[var]
    pub id: MinecraftEntityId,
    pub uuid: Uuid,
    pub entity_type: EntityKind,
    pub position: Vec3,
    pub movement: LpVec3,
    pub x_rot: i8,
    pub y_rot: i8,
    pub y_head_rot: i8,
    #[var]
    pub data: i32,
}

impl ClientboundAddEntity {
    #[cfg(feature = "bevy_ecs")]
    pub fn as_entity_bundle(&self, world_name: WorldName) -> azalea_entity::EntityBundle {
        azalea_entity::EntityBundle::new(self.uuid, self.position, self.entity_type, world_name)
    }

    #[cfg(feature = "bevy_ecs")]
    pub fn apply_metadata(&self, entity: &mut bevy_ecs::system::EntityCommands) {
        azalea_entity::metadata::apply_default_metadata(entity, self.entity_type);
    }
}
