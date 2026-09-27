use std::sync::Arc;

use azalea_protocol::packets::{
    Packet,
    login::{
        ClientboundCustomQuery, ClientboundHello, ClientboundLoginPacket, ServerboundLoginPacket,
    },
};
use bevy_ecs::prelude::*;
use tracing::{debug, error};

use super::InLoginState;
use crate::{account::Account, connection::RawConnection};

#[derive(Clone, Debug, Message)]
pub struct ReceiveLoginPacketEvent {
    pub entity: Entity,
    pub packet: Arc<ClientboundLoginPacket>,
}

#[derive(Clone, Debug, EntityEvent)]
pub struct ReceiveHelloEvent {
    pub entity: Entity,
    pub account: Account,
    pub packet: ClientboundHello,
}

#[derive(Clone, Debug, Message)]
pub struct ReceiveCustomQueryEvent {
    pub entity: Entity,
    pub packet: ClientboundCustomQuery,
    pub disabled: bool,
}

#[derive(Clone, Debug, EntityEvent)]
pub struct SendLoginPacketEvent {
    #[event_target]
    pub sent_by: Entity,
    pub packet: ServerboundLoginPacket,
}
impl SendLoginPacketEvent {
    pub fn new(entity: Entity, packet: impl Packet<ServerboundLoginPacket>) -> Self {
        let packet = packet.into_variant();
        Self {
            sent_by: entity,
            packet,
        }
    }
}

pub fn handle_outgoing_packets_observer(
    trigger: On<SendLoginPacketEvent>,
    mut query: Query<(&mut RawConnection, Option<&InLoginState>)>,
) {
    let event = trigger.event();
    if let Ok((mut raw_conn, in_login_state)) = query.get_mut(event.sent_by) {
        if in_login_state.is_none() {
            error!(
                "Tried to send a login packet {:?} while not in login state",
                event.packet
            );
            return;
        }
        debug!("Sending login packet: {:?}", event.packet);
        if let Err(e) = raw_conn.write(event.packet.clone()) {
            error!("Failed to send packet: {e}");
        }
    }
}
