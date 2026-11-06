use std::sync::atomic::{AtomicBool, Ordering};

use azalea::ecs::prelude::*;
use azalea_protocol::packets::config::ClientboundConfigPacket;
use azalea_protocol::packets::game::ClientboundGamePacket;
use azalea_protocol::packets::game::c_game_event::EventType;

use crate::SHARED;
use crate::session::SharedMutex;

pub(crate) fn packet(shared: &SharedMutex, packet: &ClientboundGamePacket) {
    match packet {
        ClientboundGamePacket::GameEvent(p) if p.event == EventType::WinGame => {
            shared.lock().unwrap().session.win_game = true;
        }
        ClientboundGamePacket::StartConfiguration(_) => {
            shared.lock().unwrap().session.reconfiguring = true;
        }
        ClientboundGamePacket::Login(_) => {
            shared.lock().unwrap().session.reconfiguring = false;
        }
        _ => {}
    }
}

static ACCEPT_PENDING: AtomicBool = AtomicBool::new(false);

pub(crate) fn accept_code_of_conduct() {
    ACCEPT_PENDING.store(true, Ordering::Release);
}

pub(crate) struct CodeOfConductPlugin;

impl azalea::app::Plugin for CodeOfConductPlugin {
    fn build(&self, app: &mut azalea::app::App) {
        app.add_systems(
            azalea::app::Update,
            (relay_code_of_conduct, send_acceptance).chain(),
        );
    }
}

fn relay_code_of_conduct(
    mut events: MessageReader<azalea::packet::config::ReceiveConfigPacketEvent>,
) {
    let Some(shared) = SHARED.get() else {
        return;
    };
    for event in events.read() {
        if let ClientboundConfigPacket::CodeOfConduct(p) = event.packet.as_ref() {
            ACCEPT_PENDING.store(false, Ordering::Release);
            shared.lock().unwrap().session.code_of_conduct = Some(p.code_of_conduct.clone());
        }
    }
}

fn send_acceptance(
    mut commands: Commands,
    query: Query<
        Entity,
        (
            With<azalea::InConfigState>,
            With<azalea::local_player::WorldHolder>,
        ),
    >,
) {
    if !ACCEPT_PENDING.load(Ordering::Acquire) {
        return;
    }
    let Ok(entity) = query.single() else {
        return;
    };
    ACCEPT_PENDING.store(false, Ordering::Release);
    use azalea_protocol::packets::config::s_accept_code_of_conduct::ServerboundAcceptCodeOfConduct;
    commands.trigger(azalea::packet::config::SendConfigPacketEvent::new(
        entity,
        ServerboundAcceptCodeOfConduct,
    ));
}
