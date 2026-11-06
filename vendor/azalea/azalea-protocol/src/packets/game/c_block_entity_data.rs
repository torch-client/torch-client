use std::io::{self, Cursor, Write};

use azalea_buf::{AzBuf, AzBufVar, BufReadError};
use azalea_core::position::BlockPos;
use azalea_protocol_macros::ClientboundGamePacket;
use azalea_registry::{
    builtin::BlockEntityKind,
    unknown::{self, Space},
};
use simdnbt::owned::Nbt;

#[derive(ClientboundGamePacket, Clone, Debug, PartialEq)]
pub struct ClientboundBlockEntityData {
    pub pos: BlockPos,
    pub block_entity_type: Result<BlockEntityKind, u32>,
    pub tag: Nbt,
}

impl AzBuf for ClientboundBlockEntityData {
    fn azalea_read(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        Ok(Self {
            pos: AzBuf::azalea_read(buf)?,
            block_entity_type: unknown::read_id(buf, Space::BlockEntities)?,
            tag: AzBuf::azalea_read(buf)?,
        })
    }
    fn azalea_write(&self, buf: &mut impl Write) -> io::Result<()> {
        self.pos.azalea_write(buf)?;
        match &self.block_entity_type {
            Ok(kind) => kind.azalea_write(buf)?,
            Err(raw) => raw.azalea_write_var(buf)?,
        }
        self.tag.azalea_write(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_type_round_trips() {
        let packet = ClientboundBlockEntityData {
            pos: BlockPos::new(1, 2, 3),
            block_entity_type: Err(9999),
            tag: Nbt::None,
        };
        let mut bytes = Vec::new();
        packet.azalea_write(&mut bytes).unwrap();

        let mut cursor = Cursor::new(&bytes[..]);
        let read = ClientboundBlockEntityData::azalea_read(&mut cursor).unwrap();

        assert_eq!(cursor.position(), bytes.len() as u64);
        assert_eq!(read, packet);
    }
}
