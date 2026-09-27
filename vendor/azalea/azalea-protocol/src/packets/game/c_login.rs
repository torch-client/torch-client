use azalea_buf::AzBuf;
use azalea_registry::identifier::Identifier;
use azalea_protocol_macros::ClientboundGamePacket;
use azalea_core::entity_id::MinecraftEntityId;

use crate::packets::common::CommonPlayerSpawnInfo;

#[derive(AzBuf, ClientboundGamePacket, Clone, Debug, PartialEq)]
pub struct ClientboundLogin {
    pub player_id: MinecraftEntityId,
    pub hardcore: bool,
    pub levels: Vec<Identifier>,
    #[var]
    pub max_players: i32,
    #[var]
    pub chunk_radius: u32,
    #[var]
    pub simulation_distance: u32,
    pub reduced_debug_info: bool,
    pub show_death_screen: bool,
    pub do_limited_crafting: bool,
    pub common: CommonPlayerSpawnInfo,
    pub enforces_secure_chat: bool,
}
