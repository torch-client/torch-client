use azalea::physics::collision::world_collisions::get_block_collisions;
use azalea_core::aabb::Aabb;
use azalea_core::position::Vec3;

use crate::modules::{MS_PER_TICK, roll};

pub(super) fn falls(world: &azalea_world::World, bb: &Aabb, vel: [f64; 3], shrink: f64) -> bool {
    let b = bb
        .deflate(Vec3::new(shrink, 0.0, shrink))
        .move_relative(Vec3::new(vel[0], -1.0, vel[2]));
    get_block_collisions(world, &b).iter().all(|s| s.is_empty())
}

pub(super) struct Eagle {
    holding: bool,
    start: u64,
    hold_ms: f32,
}

impl Eagle {
    pub const IDLE: Eagle = Eagle {
        holding: false,
        start: 0,
        hold_ms: 150.0,
    };

    pub fn reset(&mut self) {
        self.holding = false;
    }

    pub fn step(&mut self, now: u64, falling: bool, delay: (f32, f32)) -> bool {
        let held_ms = now.saturating_sub(self.start) as f32 * MS_PER_TICK;
        let extended = !falling && self.holding && held_ms < self.hold_ms && self.hold_ms > 30.0;
        if !(falling || extended) {
            self.holding = false;
            return false;
        }
        if !self.holding {
            self.hold_ms = roll(delay.0, delay.1);
            self.start = now;
            self.holding = true;
        }
        if !extended {
            self.start = now;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hold_outlasts_the_edge_by_the_delay() {
        let mut e = Eagle::IDLE;
        assert!(!e.step(0, false, (200.0, 200.0)), "nowhere near an edge");
        assert!(e.step(1, true, (200.0, 200.0)), "at the edge");
        assert!(e.step(2, false, (200.0, 200.0)), "50 ms after");
        assert!(e.step(4, false, (200.0, 200.0)), "150 ms after");
        assert!(!e.step(5, false, (200.0, 200.0)), "200 ms after: released");
    }

    #[test]
    fn a_zero_delay_releases_at_once() {
        let mut e = Eagle::IDLE;
        assert!(e.step(1, true, (0.0, 0.0)));
        assert!(!e.step(2, false, (0.0, 0.0)));
    }
}
