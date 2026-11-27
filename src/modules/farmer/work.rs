use std::collections::{HashMap, HashSet};

use azalea_core::position::BlockPos;

pub(super) const AWAY_TICKS: i64 = 600;

pub(super) const STRIKES: u8 = 3;

pub(super) const SETTLE_TICKS: i64 = 10;

#[derive(Clone, Copy, Debug, Default)]
struct Marks {
    strikes: u8,
    away_until: i64,
}

#[derive(Debug, Default)]
pub(super) struct Work {
    todo: Vec<BlockPos>,
    settling: Vec<(BlockPos, i64)>,
    done: HashSet<BlockPos>,
    marks: HashMap<BlockPos, Marks>,
    pub(super) harvested: u32,
    pub(super) unreachable: u32,
    pub(super) gone: u32,
}

impl Work {
    pub(super) fn add(&mut self, t: BlockPos) -> bool {
        if self.done.contains(&t)
            || self.marks.get(&t).is_some_and(|m| m.strikes >= STRIKES)
            || self.todo.contains(&t)
            || self.settling.iter().any(|(p, _)| *p == t)
        {
            return false;
        }
        self.todo.push(t);
        true
    }

    pub(super) fn to_go(&self) -> u32 {
        (self.todo.len() + self.settling.len()) as u32
    }

    pub(super) fn finished(&self) -> bool {
        self.todo.is_empty() && self.settling.is_empty()
    }

    pub(super) fn away(&self, t: BlockPos, now: i64) -> bool {
        self.marks.get(&t).is_some_and(|m| m.away_until > now)
    }

    pub(super) fn confirming(&self) -> bool {
        !self.settling.is_empty()
    }

    pub(super) fn listed(&self) -> impl Iterator<Item = BlockPos> + '_ {
        self.todo.iter().copied()
    }

    pub(super) fn ready(&self, now: i64) -> impl Iterator<Item = BlockPos> + '_ {
        self.todo
            .iter()
            .copied()
            .filter(move |t| !self.away(*t, now))
    }

    pub(super) fn retain(&mut self, mut keep: impl FnMut(BlockPos) -> bool) {
        let before = self.todo.len();
        self.todo.retain(|t| keep(*t));
        self.gone += (before - self.todo.len()) as u32;
    }

    pub(super) fn lost(&mut self, t: BlockPos) {
        let before = self.todo.len();
        self.todo.retain(|p| *p != t);
        self.gone += (before - self.todo.len()) as u32;
    }

    pub(super) fn broken(&mut self, t: BlockPos, now: i64) {
        self.todo.retain(|p| *p != t);
        self.settling.push((t, now + SETTLE_TICKS));
    }

    pub(super) fn settle(&mut self, now: i64, mut back: impl FnMut(BlockPos) -> bool) {
        let mut i = 0;
        while i < self.settling.len() {
            let (t, at) = self.settling[i];
            if at > now {
                i += 1;
                continue;
            }
            self.settling.swap_remove(i);
            if back(t) {
                self.todo.push(t);
                self.strike(t, now, 0);
            } else {
                self.harvested += 1;
                self.done.insert(t);
            }
        }
    }

    pub(super) fn give_up(&mut self, t: BlockPos) {
        let before = self.todo.len();
        self.todo.retain(|p| *p != t);
        if self.todo.len() < before {
            self.unreachable += 1;
            self.marks.entry(t).or_default().strikes = STRIKES;
        }
    }

    pub(super) fn strike(&mut self, t: BlockPos, now: i64, away: i64) {
        let m = self.marks.entry(t).or_default();
        m.strikes += 1;
        m.away_until = now + away;
        if m.strikes >= STRIKES {
            let before = self.todo.len();
            self.todo.retain(|p| *p != t);
            self.unreachable += (before - self.todo.len()) as u32;
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Reachable {
    pub(super) pos: BlockPos,
    pub(super) aim: [f64; 3],
    pub(super) turn: f32,
    pub(super) d2: f64,
}

pub(super) fn order(list: &mut [Reachable]) {
    list.sort_by(|a, b| {
        a.turn
            .total_cmp(&b.turn)
            .then(a.d2.total_cmp(&b.d2))
            .then((a.pos.x, a.pos.y, a.pos.z).cmp(&(b.pos.x, b.pos.y, b.pos.z)))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: i32) -> BlockPos {
        BlockPos::new(x, 65, 0)
    }

    #[test]
    fn a_harvested_target_is_not_added_after_regrowth() {
        let mut w = Work::default();
        assert!(w.add(p(0)));
        assert!(!w.add(p(0)), "already listed");
        w.broken(p(0), 100);
        assert!(!w.add(p(0)), "settling");
        w.settle(100 + SETTLE_TICKS, |_| false);
        assert_eq!((w.harvested, w.to_go()), (1, 0));
        assert!(w.finished());
        assert!(!w.add(p(0)), "harvested this run");
        assert!(w.finished());
    }

    #[test]
    fn a_break_put_back_is_tried_again() {
        let mut w = Work::default();
        w.add(p(0));
        w.broken(p(0), 0);
        w.settle(SETTLE_TICKS - 1, |_| panic!("not due yet"));
        w.settle(SETTLE_TICKS, |_| true);
        assert_eq!((w.harvested, w.to_go()), (0, 1));
        assert_eq!(w.ready(SETTLE_TICKS).collect::<Vec<_>>(), vec![p(0)]);
    }

    #[test]
    fn strikes_give_a_target_up_and_away_marks_expire() {
        let mut w = Work::default();
        w.add(p(0));
        w.add(p(1));
        w.strike(p(0), 0, AWAY_TICKS);
        assert_eq!(w.ready(0).collect::<Vec<_>>(), vec![p(1)]);
        assert_eq!(w.ready(AWAY_TICKS).count(), 2);
        for _ in 1..STRIKES {
            w.strike(p(0), 0, 0);
        }
        assert_eq!((w.unreachable, w.to_go()), (1, 1));
        assert!(!w.add(p(0)), "given up for the run");
    }

    #[test]
    fn give_up_counts_unreachable_and_refuses_the_target() {
        let mut w = Work::default();
        w.add(p(0));
        w.add(p(1));
        w.strike(p(0), 0, 0);
        w.give_up(p(0));
        assert_eq!((w.unreachable, w.to_go()), (1, 1));
        assert!(!w.add(p(0)));
        w.give_up(p(7));
        assert_eq!(w.unreachable, 1, "not on the list");
    }

    #[test]
    fn to_go_counts_listed_and_settling() {
        let mut w = Work::default();
        for x in 0..4 {
            w.add(p(x));
        }
        w.broken(p(0), 0);
        assert_eq!((w.harvested, w.to_go()), (0, 4));
        w.settle(SETTLE_TICKS, |_| false);
        assert_eq!((w.harvested, w.to_go()), (1, 3));
        w.lost(p(1));
        assert_eq!((w.gone, w.to_go()), (1, 2));
    }

    #[test]
    fn least_turn_first_then_nearest() {
        let r = |x: i32, turn: f32, d2: f64| Reachable {
            pos: p(x),
            aim: [0.0; 3],
            turn,
            d2,
        };
        let mut list = [
            r(0, 40.0, 1.0),
            r(1, 5.0, 9.0),
            r(2, 5.0, 4.0),
            r(3, 0.0, 16.0),
        ];
        order(&mut list);
        assert_eq!(list.map(|c| c.pos.x), [3, 2, 1, 0]);
    }
}
