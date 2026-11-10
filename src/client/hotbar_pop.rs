use azalea_protocol::packets::game::ClientboundGamePacket;

use crate::session::{SessionState, SharedMutex};

pub(crate) const POP_TIME: u8 = 5;

fn hotbar_index(slot: u16) -> Option<usize> {
    match slot {
        36..=44 => Some(usize::from(slot - 36)),
        45 => Some(9),
        _ => None,
    }
}

pub(crate) fn packet(shared: &SharedMutex, packet: &ClientboundGamePacket) {
    match packet {
        ClientboundGamePacket::ContainerSetSlot(p) if p.container_id == 0 => {
            let Some(i) = hotbar_index(p.slot) else {
                return;
            };
            let mut s = shared.lock().unwrap();
            let session = &mut s.session;
            let grew = !p.item_stack.is_empty()
                && session.hotbar.get(i).is_none_or(|last| {
                    last.is_empty() || i32::from(last.count) < p.item_stack.count()
                });
            session.hotbar_pop[i] = if grew { POP_TIME } else { 0 };
        }
        ClientboundGamePacket::ContainerSetContent(p) if p.container_id == 0 => {
            shared.lock().unwrap().session.hotbar_pop = [0; 10];
        }
        _ => {}
    }
}

pub(crate) fn tick(session: &mut SessionState) {
    for pop in &mut session.hotbar_pop {
        *pop = pop.saturating_sub(1);
    }
}
