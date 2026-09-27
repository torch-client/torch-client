use azalea_buf::AzBuf;
use azalea_core::position::BlockPos;
use azalea_protocol_macros::ClientboundGamePacket;
use azalea_core::entity_id::MinecraftEntityId;

#[derive(AzBuf, ClientboundGamePacket, Clone, Debug, PartialEq)]
pub struct ClientboundBlockDestruction {
    #[var]
    pub id: MinecraftEntityId,
    pub pos: BlockPos,
    pub progress: u8,
}
