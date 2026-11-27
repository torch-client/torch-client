use azalea_protocol::packets::game::ClientboundGamePacket;
use azalea_registry::builtin::EntityKind;

use crate::client::tick_rate::WorldTick;
use crate::session::{SessionState, SharedMutex};
use crate::util::javarandom::JavaRandom;

const FLASH_TICKS: i32 = 2;

const MAX_BOLTS: usize = 1024;

const START_LIFE: i32 = 2;

const FLASH_BOUND: u32 = 3;

const RESTRIKE_BOUND: u32 = 10;

pub struct Bolt {
    id: i32,
    life: i32,
    flashes: i32,
    random: JavaRandom,
}

impl Bolt {
    fn new(id: i32) -> Bolt {
        let mut random = JavaRandom::time_seeded();
        let flashes = random.next_int(FLASH_BOUND) as i32 + 1;
        Bolt {
            id,
            life: START_LIFE,
            flashes,
            random,
        }
    }

    fn tick(&mut self) -> (bool, bool) {
        self.life -= 1;
        if self.life < 0 {
            if self.flashes == 0 {
                return (false, false);
            }
            if self.life < -(self.random.next_int(RESTRIKE_BOUND) as i32) {
                self.flashes -= 1;
                self.life = 1;
            }
        }
        (true, self.life >= 0)
    }
}

pub(crate) fn packet(shared: &SharedMutex, packet: &ClientboundGamePacket) {
    match packet {
        ClientboundGamePacket::AddEntity(p) if p.entity_type == EntityKind::LightningBolt => {
            let mut s = shared.lock().unwrap();
            let bolts = &mut s.session.lightning;
            bolts.retain(|bolt| bolt.id != p.id.0);
            if bolts.len() >= MAX_BOLTS {
                bolts.remove(0);
            }
            bolts.push(Bolt::new(p.id.0));
        }
        ClientboundGamePacket::RemoveEntities(p) => {
            let mut s = shared.lock().unwrap();
            if s.session.lightning.is_empty() {
                return;
            }
            s.session
                .lightning
                .retain(|bolt| !p.entity_ids.iter().any(|id| id.0 == bolt.id));
        }
        ClientboundGamePacket::Login(_) | ClientboundGamePacket::Respawn(_) => {
            let mut s = shared.lock().unwrap();
            s.session.lightning.clear();
            s.session.sky_flash_time = 0;
        }
        _ => {}
    }
}

pub(crate) fn tick(session: &mut SessionState, world_tick: &WorldTick) {
    let mut lit = false;
    session.lightning.retain_mut(|bolt| {
        if !world_tick.entity_ticks(bolt.id) {
            return true;
        }
        let (alive, flash) = bolt.tick();
        lit |= flash;
        alive
    });
    if lit {
        session.sky_flash_time = FLASH_TICKS;
    }
    if session.sky_flash_time > 0 {
        session.sky_flash_time -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_strike_flashes_for_two_ticks() {
        let mut session = SessionState::default();
        let mut bolt = Bolt::new(7);
        bolt.flashes = 0;
        session.lightning.push(bolt);
        let running = WorldTick::default();
        tick(&mut session, &running);
        assert_eq!(session.sky_flash_time, 1);
        tick(&mut session, &running);
        assert_eq!(session.sky_flash_time, 1);
        tick(&mut session, &running);
        assert_eq!(session.sky_flash_time, 0);
        assert!(session.lightning.is_empty());
    }
}
