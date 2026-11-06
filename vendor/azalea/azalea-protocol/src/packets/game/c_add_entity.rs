use std::io::{self, Cursor, Write};

use azalea_buf::{AzBuf, AzBufVar, BufReadError};
use azalea_core::{delta::LpVec3, entity_id::MinecraftEntityId, position::Vec3};
use azalea_protocol_macros::ClientboundGamePacket;
use azalea_registry::{
    builtin::EntityKind,
    unknown::{self, Space},
};
#[cfg(feature = "bevy_ecs")]
use azalea_world::WorldName;
use uuid::Uuid;

#[derive(ClientboundGamePacket, Clone, Debug, PartialEq)]
pub struct ClientboundAddEntity {
    pub id: MinecraftEntityId,
    pub uuid: Uuid,
    pub entity_type: EntityKind,
    pub modded_type: Option<u32>,
    pub position: Vec3,
    pub movement: LpVec3,
    pub x_rot: i8,
    pub y_rot: i8,
    pub y_head_rot: i8,
    pub data: i32,
}

impl AzBuf for ClientboundAddEntity {
    fn azalea_read(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        let id = MinecraftEntityId::azalea_read_var(buf)?;
        let uuid = Uuid::azalea_read(buf)?;
        let (entity_type, modded_type) = match unknown::read_id(buf, Space::Entities)? {
            Ok(kind) => (kind, None),
            Err(raw) => (EntityKind::Marker, Some(raw)),
        };
        Ok(Self {
            id,
            uuid,
            entity_type,
            modded_type,
            position: AzBuf::azalea_read(buf)?,
            movement: AzBuf::azalea_read(buf)?,
            x_rot: AzBuf::azalea_read(buf)?,
            y_rot: AzBuf::azalea_read(buf)?,
            y_head_rot: AzBuf::azalea_read(buf)?,
            data: AzBufVar::azalea_read_var(buf)?,
        })
    }
    fn azalea_write(&self, buf: &mut impl Write) -> io::Result<()> {
        self.id.azalea_write_var(buf)?;
        self.uuid.azalea_write(buf)?;
        match self.modded_type {
            Some(raw) => raw.azalea_write_var(buf)?,
            None => self.entity_type.azalea_write(buf)?,
        }
        self.position.azalea_write(buf)?;
        self.movement.azalea_write(buf)?;
        self.x_rot.azalea_write(buf)?;
        self.y_rot.azalea_write(buf)?;
        self.y_head_rot.azalea_write(buf)?;
        self.data.azalea_write_var(buf)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(entity_type: EntityKind, modded_type: Option<u32>) -> ClientboundAddEntity {
        ClientboundAddEntity {
            id: MinecraftEntityId(7),
            uuid: Uuid::from_u128(1234),
            entity_type,
            modded_type,
            position: Vec3::new(1.0, 2.0, 3.0),
            movement: LpVec3::Zero,
            x_rot: 1,
            y_rot: 2,
            y_head_rot: 3,
            data: 4,
        }
    }

    fn round_trip(packet: &ClientboundAddEntity) -> ClientboundAddEntity {
        let mut bytes = Vec::new();
        packet.azalea_write(&mut bytes).unwrap();
        let mut cursor = Cursor::new(&bytes[..]);
        let read = ClientboundAddEntity::azalea_read(&mut cursor).unwrap();
        assert_eq!(cursor.position(), bytes.len() as u64);
        read
    }

    #[test]
    fn unknown_type_is_a_marker_keeping_the_raw_id() {
        let sent = packet(EntityKind::Zombie, Some(9999));

        let read = round_trip(&sent);

        assert_eq!(read.entity_type, EntityKind::Marker);
        assert_eq!(read.modded_type, Some(9999));
        assert_eq!(read.data, 4);
        assert_eq!(round_trip(&read), read);
        assert!(unknown::snapshot().entities >= 1);
    }

    #[test]
    fn known_type_round_trips() {
        let sent = packet(EntityKind::Zombie, None);
        assert_eq!(round_trip(&sent), sent);
    }
}
