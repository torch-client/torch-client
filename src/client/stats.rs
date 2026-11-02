use std::collections::HashMap;
use std::sync::Arc;

use azalea_protocol::packets::game::ClientboundGamePacket;
use azalea_protocol::packets::game::c_award_stats::Stat;

use crate::session::SharedMutex;

#[derive(Clone, Debug, Default)]
pub struct StatsCounter {
    values: HashMap<Stat, i32>,
    pub updates: u32,
}

impl StatsCounter {
    pub fn get(&self, stat: Stat) -> i32 {
        self.values.get(&stat).copied().unwrap_or(0)
    }

    pub fn iter(&self) -> impl Iterator<Item = (Stat, i32)> + '_ {
        self.values.iter().map(|(k, v)| (*k, *v))
    }
}

pub(crate) fn packet(shared: &Arc<SharedMutex>, packet: &ClientboundGamePacket) {
    match packet {
        ClientboundGamePacket::AwardStats(p) => {
            let mut s = shared.lock().unwrap();
            let counter = Arc::make_mut(&mut s.session.stats);
            counter.values.extend(p.stats.iter().map(|(k, v)| (*k, *v)));
            counter.updates = counter.updates.wrapping_add(1);
        }
        ClientboundGamePacket::Login(_) => {
            let mut s = shared.lock().unwrap();
            let counter = Arc::make_mut(&mut s.session.stats);
            counter.values.clear();
        }
        _ => {}
    }
}
