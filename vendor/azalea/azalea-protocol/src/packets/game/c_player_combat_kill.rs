use azalea_buf::AzBuf;
use azalea_chat::FormattedText;
use azalea_protocol_macros::ClientboundGamePacket;
use azalea_core::entity_id::MinecraftEntityId;

#[derive(AzBuf, ClientboundGamePacket, Clone, Debug, PartialEq)]
pub struct ClientboundPlayerCombatKill {
    #[var]
    pub player_id: MinecraftEntityId,
    pub message: FormattedText,
}
