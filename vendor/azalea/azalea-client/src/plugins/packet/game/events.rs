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

pub fn handle_outgoing_packets_observer(
    trigger: On<SendGamePacketEvent>,
    mut query: Query<(&mut RawConnection, Option<&InGameState>)>,
) {
    let event = trigger.event();

    if let Ok((mut raw_connection, in_game_state)) = query.get_mut(event.sent_by) {
        if in_game_state.is_none() {
            error!(
                "Tried to send a game packet {:?} while not in game state",
                event.packet
            );
            return;
        }

        trace!("Sending game packet: {:?}", event.packet);
        if let Err(e) = raw_connection.write(event.packet.clone()) {
            error!("Failed to send packet: {e}");
        }
    } else {
        trace!("Not sending game packet: {:?}", event.packet);
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
