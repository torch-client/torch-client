use std::sync::Mutex;

use azalea::Client;
use azalea::app::{App, Plugin};
use azalea::ecs::prelude::*;
use azalea::packet::game::SendGamePacketEvent;
use azalea::physics::PhysicsSystems;
use azalea_core::tick::GameTick;
use azalea_protocol::packets::Packet;
use azalea_protocol::packets::game::ServerboundGamePacket;

static QUEUE: Mutex<Vec<(Entity, ServerboundGamePacket)>> = Mutex::new(Vec::new());

pub(crate) fn send(bot: &Client, packet: impl Packet<ServerboundGamePacket>) {
    QUEUE
        .lock()
        .unwrap()
        .push((bot.entity, packet.into_variant()));
}

fn flush(world: &mut World) {
    let queued = std::mem::take(&mut *QUEUE.lock().unwrap());
    for (sent_by, packet) in queued {
        world.trigger(SendGamePacketEvent { sent_by, packet });
    }
}

pub(crate) struct OutboxPlugin;

impl Plugin for OutboxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            GameTick,
            flush
                .before(azalea::inventory::ensure_has_sent_carried_item)
                .before(azalea::mining::handle_mining_queued)
                .before(azalea::interact::handle_start_use_item_queued)
                .before(azalea::attack::handle_attack_queued)
                .before(PhysicsSystems),
        );
    }
}
