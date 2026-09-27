use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

use super::Store;
use super::registry::Id;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Off,
    Started,
    Running,
    Stopped,
}

pub struct Edge {
    id: Id,
    seen: AtomicU32,
}

impl Edge {
    const UNSEEN: u32 = u32::MAX;

    pub const fn new(id: Id) -> Edge {
        Edge {
            id,
            seen: AtomicU32::new(Self::UNSEEN),
        }
    }

    pub fn poll(&self, s: &Store) -> Phase {
        let i = self.id as usize;
        let now = s.toggle_count(i);
        if now == 0 {
            return Phase::Off;
        }
        let seen = self.seen.swap(now, Relaxed);
        match (s.enabled_at(i), seen == now) {
            (true, true) => Phase::Running,
            (true, false) => Phase::Started,
            (false, false) if seen != Self::UNSEEN => Phase::Stopped,
            (false, _) => Phase::Off,
        }
    }
}

pub const MS_PER_TICK: f32 = 50.0;

pub fn roll(lo: f32, hi: f32) -> f32 {
    static RNG: AtomicU32 = AtomicU32::new(0x9E37_79B9);
    let mut x = RNG.load(Relaxed);
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    RNG.store(x, Relaxed);
    let (lo, hi) = (lo.min(hi).max(0.0), hi.max(lo).max(0.0));
    let t = (x >> 8) as f32 / (1 << 24) as f32;
    lo + (hi - lo) * t
}

#[cfg(all(test, feature = "click_gui"))]
mod tests {
    use super::*;
    use crate::modules::registry::Mode;

    #[test]
    fn an_edge_reports_each_transition_once() {
        let s = Store::from_registry();
        let e = Edge::new(Id::TriggerBot);
        let i = Id::TriggerBot as usize;

        assert_eq!(e.poll(&s), Phase::Off, "nobody has touched it");
        s.set_enabled_at(i, true);
        assert_eq!(e.poll(&s), Phase::Started);
        assert_eq!(e.poll(&s), Phase::Running, "a start happens once");
        s.set_enabled_at(i, false);
        assert_eq!(e.poll(&s), Phase::Stopped);
        assert_eq!(e.poll(&s), Phase::Off, "a stop happens once");

        s.set_enabled_at(i, true);
        s.set_enabled_at(i, false);
        assert_eq!(e.poll(&s), Phase::Stopped, "on and off inside one tick");
        s.set_enabled_at(i, true);
        s.set_enabled_at(i, false);
        s.set_enabled_at(i, true);
        assert_eq!(e.poll(&s), Phase::Started, "and off and on again");
    }

    #[test]
    fn a_start_and_stop_before_the_first_poll_is_not_a_stop() {
        let s = Store::from_registry();
        let e = Edge::new(Id::TriggerBot);
        let i = Id::TriggerBot as usize;
        s.set_enabled_at(i, true);
        s.set_enabled_at(i, false);
        assert_eq!(e.poll(&s), Phase::Off, "nothing was set up to put back");
    }

    #[test]
    fn a_profile_change_is_an_edge_too() {
        let s = Store::from_registry();
        let e = Edge::new(Id::Flight);
        s.set_profile(Mode::Rage);
        s.set_enabled_at(Id::Flight as usize, true);
        assert_eq!(e.poll(&s), Phase::Started);
        s.set_profile(Mode::Normal);
        assert_eq!(e.poll(&s), Phase::Stopped, "Normal does not permit Flight");
        assert!(s.armed(Id::Flight), "and the switch is still the player's");
    }
}
