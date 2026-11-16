use std::sync::{Arc, Weak};

use azalea_chat::FormattedText;
use azalea_protocol::packets::{
    Packet,
    game::{ClientboundGamePacket, ClientboundPlayerCombatKill, ServerboundGamePacket},
};
use azalea_world::{World, WorldName};
use bevy_ecs::prelude::*;
use parking_lot::RwLock;
use tracing::{error, trace};
use uuid::Uuid;

use crate::{client::InGameState, connection::RawConnection, player::PlayerInfo};

#[derive(Clone, Debug, Message)]
pub struct ReceiveGamePacketEvent {
    pub entity: Entity,
    pub packet: Arc<ClientboundGamePacket>,
}

#[derive(Clone, Debug, EntityEvent)]
pub struct SendGamePacketEvent {
    #[event_target]
    pub sent_by: Entity,
    pub packet: ServerboundGamePacket,
}
impl SendGamePacketEvent {
    pub fn new(sent_by: Entity, packet: impl Packet<ServerboundGamePacket>) -> Self {
        let packet = packet.into_variant();
        Self { sent_by, packet }
    }
}

static OUTGOING_GAME_FILTER: std::sync::OnceLock<fn(&ServerboundGamePacket) -> bool> =
    std::sync::OnceLock::new();

pub fn set_outgoing_game_filter(
    filter: fn(&ServerboundGamePacket) -> bool,
) -> Result<(), fn(&ServerboundGamePacket) -> bool> {
    OUTGOING_GAME_FILTER.set(filter)
}

#[derive(Clone, Debug, EntityEvent)]
pub struct SendGamePacketBypassEvent {
    #[event_target]
    pub sent_by: Entity,
    pub packet: ServerboundGamePacket,
}

pub fn handle_outgoing_packets_bypass_observer(
    trigger: On<SendGamePacketBypassEvent>,
    query: Query<(&mut RawConnection, Option<&InGameState>)>,
) {
    let event = trigger.event();
    write_game_packet(query, event.sent_by, &event.packet);
}

pub fn handle_outgoing_packets_observer(
    trigger: On<SendGamePacketEvent>,
    query: Query<(&mut RawConnection, Option<&InGameState>)>,
) {
    let event = trigger.event();
    if OUTGOING_GAME_FILTER.get().is_some_and(|f| !f(&event.packet)) {
        trace!("Filtered game packet: {:?}", event.packet);
        return;
    }
    write_game_packet(query, event.sent_by, &event.packet);
}

fn write_game_packet(
    mut query: Query<(&mut RawConnection, Option<&InGameState>)>,
    sent_by: Entity,
    packet: &ServerboundGamePacket,
) {
    if let Ok((mut raw_connection, in_game_state)) = query.get_mut(sent_by) {
        if in_game_state.is_none() {
            error!(
                "Tried to send a game packet {:?} while not in game state",
                packet
            );
            return;
        }

        trace!("Sending game packet: {:?}", packet);
        if let Err(e) = raw_connection.write(packet.clone()) {
            error!("Failed to send packet: {e}");
        }
    } else {
        trace!("Not sending game packet: {:?}", packet);
    }
}

#[derive(Clone, Debug, Message)]
pub struct AddPlayerEvent {
    pub entity: Entity,
    pub info: PlayerInfo,
}
#[derive(Clone, Debug, Message)]
pub struct RemovePlayerEvent {
    pub entity: Entity,
    pub info: PlayerInfo,
}
#[derive(Clone, Debug, Message)]
pub struct UpdatePlayerEvent {
    pub entity: Entity,
    pub info: PlayerInfo,
}

#[derive(Clone, Debug, Message)]
pub struct DeathEvent {
    pub entity: Entity,
    pub packet: Option<ClientboundPlayerCombatKill>,
}

#[derive(Clone, Debug, EntityEvent)]
pub struct KeepAliveEvent {
    pub entity: Entity,
    pub id: u64,
}

#[derive(Clone, Debug, Message)]
pub struct ResourcePackEvent {
    pub entity: Entity,
    pub id: Uuid,
    pub url: String,
    pub hash: String,
    pub required: bool,
    pub prompt: Option<FormattedText>,
}

#[derive(Clone, Debug, Message)]
pub struct WorldLoadedEvent {
    pub entity: Entity,
    pub name: WorldName,
    pub world: Weak<RwLock<World>>,
}
#[deprecated = "renamed to `WorldLoadedEvent`."]
pub type InstanceLoadedEvent = WorldLoadedEvent;

#[derive(Clone, Debug, EntityEvent)]
pub struct GamePingEvent {
    pub entity: Entity,
    pub packet: azalea_protocol::packets::game::ClientboundPing,
}
