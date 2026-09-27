use azalea_buf::AzBuf;
use azalea_protocol_macros::ClientboundGamePacket;
use azalea_core::entity_id::MinecraftEntityId;

#[derive(AzBuf, ClientboundGamePacket, Clone, Debug, PartialEq)]
pub struct ClientboundAnimate {
    #[var]
    pub id: MinecraftEntityId,
    pub action: AnimationAction,
}

#[derive(AzBuf, Clone, Copy, Debug, PartialEq)]
pub enum AnimationAction {
    SwingMainHand = 0,
    Hurt = 1,
    WakeUp = 2,
    SwingOffHand = 3,
    CriticalHit = 4,
    MagicCriticalHit = 5,
}
