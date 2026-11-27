use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use azalea::BlockPos;
use azalea::pathfinder::moves::{MovesCtx, basic, parkour, place, uncommon};
use azalea::pathfinder::positions::RelBlockPos;

use crate::platform::time::Instant;

pub(super) fn is_hop(a: BlockPos, b: BlockPos) -> bool {
    let (dx, dz) = ((b.x - a.x).abs(), (b.z - a.z).abs());
    let along = dx.max(dz);
    dx.min(dz) == 0 && (along >= 3 || (along == 2 && b.y >= a.y))
}

pub(super) const LAUNCH_YAW_ERR: f32 = 6.0;

pub(super) fn holds_launch(err: f32, sprinting: bool, wants_sprint: bool) -> bool {
    err.abs() > LAUNCH_YAW_ERR || (sprinting && !wants_sprint)
}

pub(super) const PREAIM_WITHIN: f64 = 1.5;

const FALL_BELOW: f64 = 0.5;

const HOP_TIMEOUT_TICKS: usize = 40;

pub(super) fn hop_failed(
    feet_y: f64,
    start: BlockPos,
    target: BlockPos,
    ticks: usize,
    off_start: bool,
) -> Option<&'static str> {
    let floor = start.y.min(target.y) as f64;
    if feet_y < floor - FALL_BELOW {
        Some("fell")
    } else if off_start && ticks >= HOP_TIMEOUT_TICKS {
        Some("timeout")
    } else {
        None
    }
}

const MAX_FAILED_HOPS: usize = 32;

const FAILED_HOP_TTL: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, PartialEq)]
struct FailedHop {
    start: BlockPos,
    target: BlockPos,
    route: u32,
    at: Instant,
}

#[derive(Debug)]
struct FailedHops {
    list: Vec<FailedHop>,
}

impl FailedHops {
    const fn new() -> FailedHops {
        FailedHops { list: Vec::new() }
    }

    fn record(&mut self, start: BlockPos, target: BlockPos, route: u32, now: Instant) -> bool {
        if let Some(h) = self
            .list
            .iter_mut()
            .find(|h| h.start == start && h.target == target)
        {
            h.route = route;
            h.at = now;
            return false;
        }
        if self.list.len() >= MAX_FAILED_HOPS {
            self.list.remove(0);
        }
        self.list.push(FailedHop {
            start,
            target,
            route,
            at: now,
        });
        true
    }

    fn contains(&self, start: BlockPos, target: BlockPos) -> bool {
        self.list
            .iter()
            .any(|h| h.start == start && h.target == target)
    }

    fn prune(&mut self, now: Instant, live: Option<u32>) {
        for h in &mut self.list {
            if Some(h.route) == live {
                h.at = now;
            }
        }
        self.list
            .retain(|h| now.duration_since(h.at) <= FAILED_HOP_TTL);
    }
}

static FAILED: Mutex<FailedHops> = Mutex::new(FailedHops::new());

static ANY_FAILED: AtomicBool = AtomicBool::new(false);

static ROUTE_SEQ: AtomicU32 = AtomicU32::new(0);

fn failed() -> MutexGuard<'static, FailedHops> {
    FAILED.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(super) fn begin_route() {
    let live = ROUTE_SEQ.fetch_add(1, Relaxed).wrapping_add(1);
    if ANY_FAILED.load(Relaxed) {
        let mut f = failed();
        f.prune(Instant::now(), Some(live));
        ANY_FAILED.store(!f.list.is_empty(), Relaxed);
    }
}

pub(super) fn keep_live() {
    if ANY_FAILED.load(Relaxed) {
        let mut f = failed();
        f.prune(Instant::now(), Some(ROUTE_SEQ.load(Relaxed)));
        ANY_FAILED.store(!f.list.is_empty(), Relaxed);
    }
}

pub(super) fn note_failed(start: BlockPos, target: BlockPos, why: &str) {
    let new = failed().record(start, target, ROUTE_SEQ.load(Relaxed), Instant::now());
    ANY_FAILED.store(true, Relaxed);
    {
        let _ = (new, why);
    }
}

pub(super) fn clear() {
    failed().list.clear();
    ANY_FAILED.store(false, Relaxed);
}

pub(super) fn with_parkour(ctx: &mut MovesCtx, node: RelBlockPos) {
    basic::basic_move(ctx, node);
    let first = ctx.edges.len();
    parkour::parkour_move(ctx, node);
    if ANY_FAILED.load(Relaxed) && ctx.edges.len() > first {
        let origin = ctx.world.origin();
        let start = node.apply(origin);
        let f = failed();
        let mut i = first;
        while i < ctx.edges.len() {
            if f.contains(start, ctx.edges[i].movement.target.apply(origin)) {
                ctx.edges.remove(i);
            } else {
                i += 1;
            }
        }
    }
    uncommon::uncommon_move(ctx, node);
    place::place_move(ctx, node);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hops_are_told_from_other_edges_by_their_length_and_height() {
        let a = BlockPos::new(0, 64, 0);
        assert!(is_hop(a, BlockPos::new(2, 64, 0)));
        assert!(is_hop(a, BlockPos::new(0, 65, -2)));
        assert!(!is_hop(a, BlockPos::new(2, 63, 0)));
        assert!(!is_hop(a, BlockPos::new(0, 61, 2)));
        assert!(is_hop(a, BlockPos::new(-3, 63, 0)));
        assert!(is_hop(a, BlockPos::new(3, 65, 0)));
        assert!(is_hop(a, BlockPos::new(0, 64, 4)));
        assert!(!is_hop(a, BlockPos::new(1, 64, 0)));
        assert!(!is_hop(a, BlockPos::new(1, 64, 1)));
        assert!(!is_hop(a, BlockPos::new(2, 64, 2)));
        assert!(!is_hop(a, BlockPos::new(0, 65, 0)));
    }

    #[test]
    fn launch_tolerance_follows_from_the_flight() {
        let accel = 0.1 * 1.3 * 0.98;
        let ground = 0.6 * 0.91;
        let carried = ground * accel / (1.0 - ground);
        let launch = carried + accel + 0.2;
        assert!((launch - 0.4806).abs() < 1e-3, "{launch}");

        let (mut y, mut vy, mut ticks) = (0.0f64, 0.42f64, 0);
        loop {
            y += vy;
            ticks += 1;
            vy = (vy - 0.08) * 0.98;
            if y <= 0.0 {
                break;
            }
        }
        assert_eq!(ticks, 12);

        let (mut carry, mut v) = (0.0f64, 1.0f64);
        for k in 0..ticks {
            carry += v;
            v *= if k == 0 { ground } else { 0.91 };
        }
        assert!((carry - 4.917).abs() < 1e-3, "{carry}");

        let bound = (0.25 / (launch * carry)).asin().to_degrees() as f32;
        assert!(
            LAUNCH_YAW_ERR <= bound && bound - LAUNCH_YAW_ERR < 0.5,
            "{bound}"
        );
    }

    #[test]
    fn the_launch_waits_for_the_yaw_and_for_an_unwanted_sprint_to_end() {
        assert!(!holds_launch(0.0, false, false));
        assert!(!holds_launch(-LAUNCH_YAW_ERR, true, true));
        assert!(holds_launch(LAUNCH_YAW_ERR + 0.1, false, true));
        assert!(holds_launch(-30.0, false, false));
        assert!(holds_launch(0.0, true, false));
    }

    #[test]
    fn a_hop_fails_by_falling_or_by_timing_out_off_the_take_off_block() {
        let start = BlockPos::new(0, 64, 0);
        let up = BlockPos::new(2, 65, 0);
        let down = BlockPos::new(3, 63, 0);
        assert_eq!(hop_failed(65.2, start, up, 8, true), None);
        assert_eq!(hop_failed(63.0, start, down, 8, true), None);
        assert_eq!(hop_failed(63.0, start, up, 8, true), Some("fell"));
        assert_eq!(hop_failed(62.4, start, down, 8, true), Some("fell"));
        assert_eq!(
            hop_failed(64.0, start, up, HOP_TIMEOUT_TICKS, true),
            Some("timeout")
        );
        assert_eq!(hop_failed(64.0, start, up, HOP_TIMEOUT_TICKS, false), None);
        assert_eq!(
            hop_failed(64.0, start, up, HOP_TIMEOUT_TICKS - 1, true),
            None
        );
    }

    fn b(x: i32) -> BlockPos {
        BlockPos::new(x, 64, 0)
    }

    #[test]
    fn a_failed_hop_is_remembered_once_and_the_oldest_goes_past_the_bound() {
        let t0 = Instant::now();
        let mut f = FailedHops::new();
        assert!(f.record(b(0), b(2), 1, t0));
        assert!(!f.record(b(0), b(2), 1, t0), "a repeat refreshes");
        assert!(f.contains(b(0), b(2)));
        assert!(!f.contains(b(2), b(0)));
        for i in 1..MAX_FAILED_HOPS as i32 {
            assert!(f.record(b(10 * i), b(10 * i + 3), 1, t0));
        }
        assert_eq!(f.list.len(), MAX_FAILED_HOPS);
        assert!(f.record(b(-10), b(-12), 1, t0));
        assert_eq!(f.list.len(), MAX_FAILED_HOPS);
        assert!(!f.contains(b(0), b(2)), "the oldest went");
        assert!(f.contains(b(-10), b(-12)));
    }

    #[test]
    fn a_failed_hop_lasts_its_route_and_then_the_ttl() {
        let t0 = Instant::now();
        let mut f = FailedHops::new();
        f.record(b(0), b(2), 1, t0);
        f.record(b(5), b(8), 2, t0);
        let later = t0 + FAILED_HOP_TTL + Duration::from_secs(1);
        f.prune(later, Some(1));
        assert!(f.contains(b(0), b(2)));
        assert!(!f.contains(b(5), b(8)));
        f.prune(later + FAILED_HOP_TTL, Some(3));
        assert!(f.contains(b(0), b(2)));
        f.prune(later + FAILED_HOP_TTL + Duration::from_secs(1), Some(3));
        assert!(!f.contains(b(0), b(2)));
    }
}
