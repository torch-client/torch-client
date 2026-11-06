use std::{
    io::{self, Cursor, Write},
    sync::Arc,
};

use azalea_buf::{AzBuf, BufReadError};
use azalea_core::heightmap_kind::HeightmapKind;
use azalea_protocol_macros::ClientboundGamePacket;
use azalea_registry::{
    builtin::BlockEntityKind,
    unknown::{self, Space},
};
use simdnbt::owned::Nbt;

use super::c_light_update::ClientboundLightUpdatePacketData;

#[derive(AzBuf, ClientboundGamePacket, Clone, Debug, PartialEq)]
pub struct ClientboundLevelChunkWithLight {
    pub x: i32,
    pub z: i32,
    pub chunk_data: ClientboundLevelChunkPacketData,
    pub light_data: ClientboundLightUpdatePacketData,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClientboundLevelChunkPacketData {
    pub heightmaps: Vec<(HeightmapKind, Box<[u64]>)>,
    pub data: Arc<Box<[u8]>>,
    pub block_entities: Vec<BlockEntity>,
}

impl AzBuf for ClientboundLevelChunkPacketData {
    fn azalea_read(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        Ok(Self {
            heightmaps: AzBuf::azalea_read(buf)?,
            data: AzBuf::azalea_read(buf)?,
            block_entities: unknown::read_list_skipping(buf, |buf| {
                let packed_xz = u8::azalea_read(buf)?;
                let y = u16::azalea_read(buf)?;
                let kind = unknown::read_id(buf, Space::BlockEntities)?;
                let data = Nbt::azalea_read(buf)?;
                Ok(kind.ok().map(|kind| BlockEntity {
                    packed_xz,
                    y,
                    kind,
                    data,
                }))
            })?,
        })
    }
    fn azalea_write(&self, buf: &mut impl Write) -> io::Result<()> {
        self.heightmaps.azalea_write(buf)?;
        self.data.azalea_write(buf)?;
        self.block_entities.azalea_write(buf)
    }
}

#[derive(AzBuf, Clone, Debug, PartialEq)]
pub struct BlockEntity {
    pub packed_xz: u8,
    pub y: u16,
    pub kind: BlockEntityKind,
    pub data: Nbt,
}

#[cfg(test)]
mod tests {
    use azalea_buf::AzBufVar;
    use simdnbt::owned::{BaseNbt, NbtCompound, NbtTag};

    use super::*;

    fn tag() -> Nbt {
        Nbt::Some(BaseNbt::new(
            "",
            NbtCompound::from_values(vec![("id".into(), NbtTag::Int(3))]),
        ))
    }

    fn block_entity(packed_xz: u8, kind: BlockEntityKind) -> BlockEntity {
        BlockEntity {
            packed_xz,
            y: 64,
            kind,
            data: tag(),
        }
    }

    #[test]
    fn unknown_block_entity_keeps_the_known_ones() {
        let first = block_entity(1, BlockEntityKind::Chest);
        let last = block_entity(2, BlockEntityKind::Sign);
        let sections: Arc<Box<[u8]>> = Arc::new(vec![1, 2, 3].into());

        let mut bytes = Vec::new();
        Vec::<(HeightmapKind, Box<[u64]>)>::new()
            .azalea_write(&mut bytes)
            .unwrap();
        sections.azalea_write(&mut bytes).unwrap();
        3_u32.azalea_write_var(&mut bytes).unwrap();
        first.azalea_write(&mut bytes).unwrap();
        9_u8.azalea_write(&mut bytes).unwrap();
        64_u16.azalea_write(&mut bytes).unwrap();
        9999_u32.azalea_write_var(&mut bytes).unwrap();
        tag().azalea_write(&mut bytes).unwrap();
        last.azalea_write(&mut bytes).unwrap();

        let mut cursor = Cursor::new(&bytes[..]);
        let read = ClientboundLevelChunkPacketData::azalea_read(&mut cursor).unwrap();

        assert_eq!(cursor.position(), bytes.len() as u64);
        assert_eq!(read.data, sections);
        assert_eq!(read.block_entities, vec![first, last]);
        assert!(unknown::snapshot().block_entities >= 1);
    }
}
