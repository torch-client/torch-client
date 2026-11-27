use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use azalea_core::position::BlockPos;

use crate::modules::sent::keys_toward;
use crate::session::{MOVE_DESCEND, MOVE_JUMP};

pub(super) const H_DRAG: f64 = 0.91;

pub(super) const V_DRAG: f64 = 0.6;

pub(super) const V_IMPULSE: f64 = 3.0;

pub(super) const DEFAULT_SPEED: f64 = 0.05;

pub(super) const HOVER_LIFT: f64 = 0.1;

pub(super) const ARRIVE: f64 = 0.5;

pub(super) const PASS: f64 = 0.35;

pub(super) const TIGHT: f64 = 0.3;

pub(super) const K_H: f64 = 1.0 - H_DRAG;
pub(super) const K_V: f64 = 1.0 - V_DRAG;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Caps {
    pub(super) h: f64,
    pub(super) v: f64,
}

pub(super) const NEAR_CAPS: Caps = Caps { h: 0.1, v: 0.1 };

pub(super) const FAR_CAPS: Caps = Caps { h: 0.3, v: 0.2 };

pub(super) const FAR_LEG: f64 = 6.0;

pub(super) const CLEAR: f64 = 1.0;
pub(super) const LINE_MARGIN: f64 = 0.2;

pub(super) const CROWD_COST: i32 = 2;

pub(super) const RISE_FIRST: f64 = 0.3;

pub(super) const LAND_SINK: f64 = 0.4;

pub(super) const HOLD_SLACK: f64 = 0.3;

pub(super) const WALL_SLACK: f64 = 0.1;

pub(super) const WALL_GAP: f64 = 0.12;

pub(super) fn clear_hold(at: [f64; 3], solid: impl Fn(BlockPos) -> bool) -> ([f64; 3], [f64; 2]) {
    let cell = BlockPos::new(
        at[0].floor() as i32,
        at[1].floor() as i32,
        at[2].floor() as i32,
    );
    let mut out = at;
    let mut slack = [HOLD_SLACK; 2];
    for (i, axis) in [(0usize, 0usize), (1, 2)] {
        let side = |d: i32| {
            (0..2).any(|dy| {
                let mut c = [cell.x, cell.y + dy, cell.z];
                c[axis] += d;
                solid(BlockPos::new(c[0], c[1], c[2]))
            })
        };
        let base = f64::from([cell.x, cell.y, cell.z][axis]);
        let (lo, hi) = (side(-1), side(1));
        let min = base + 0.3 + WALL_GAP;
        let max = base + 1.0 - 0.3 - WALL_GAP;
        match (lo, hi) {
            (true, true) => out[axis] = base + 0.5,
            (true, false) => out[axis] = out[axis].max(min),
            (false, true) => out[axis] = out[axis].min(max),
            (false, false) => continue,
        }
        slack[i] = WALL_SLACK;
    }
    (out, slack)
}

const BRAKE_H: f64 = 0.05;

const BRAKE_V: f64 = 0.15;

const MAX_NODES: usize = 4096;
pub(super) const NODES_PER_TICK: usize = 512;

pub(super) const TURN: f32 = 360.0;

const SAMPLE: f64 = 0.25;

#[cfg(test)]
pub(super) fn coast(v: f64, drag: f64) -> f64 {
    v / (1.0 - drag)
}

#[cfg(test)]
pub(super) fn top_speed(push: f64, drag: f64) -> f64 {
    push * drag / (1.0 - drag)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn steer(
    pos: [f64; 3],
    vel: [f64; 3],
    target: [f64; 3],
    yaw: f32,
    speed: f64,
    near: f64,
    caps: Caps,
    prev: u8,
) -> (u8, bool) {
    let (dx, dy, dz) = (target[0] - pos[0], target[1] - pos[1], target[2] - pos[2]);
    if (dx * dx + dz * dz).sqrt() < near && dy.abs() < ARRIVE.max(near) {
        return (0, true);
    }
    (track(pos, vel, target, yaw, speed, caps, prev), false)
}

pub(super) fn track(
    pos: [f64; 3],
    vel: [f64; 3],
    target: [f64; 3],
    yaw: f32,
    speed: f64,
    caps: Caps,
    prev: u8,
) -> u8 {
    let (dx, dy, dz) = (target[0] - pos[0], target[1] - pos[1], target[2] - pos[2]);
    let dist = (dx * dx + dz * dz).sqrt();
    let scale = if dist > 1e-9 {
        caps.h.min(K_H * dist) / dist
    } else {
        0.0
    };
    let err = [dx * scale - vel[0], dz * scale - vel[2]];
    let mut keys = 0;
    if (err[0] * err[0] + err[1] * err[1]).sqrt() >= speed / 2.0 {
        keys |= keys_toward(err[0], err[1], yaw).unwrap_or(0);
    }
    keys | vertical_key(dy, vel[1], speed * V_IMPULSE, caps.v, prev)
}

pub(super) const TAP_ZONE: f64 = 0.5;

pub(super) const BAND_LOW: f64 = 0.08;

const TAP_STILL: f64 = 0.03;

pub(super) fn vertical_key(dy: f64, vy: f64, push: f64, cap: f64, prev: u8) -> u8 {
    if dy.abs() > TAP_ZONE {
        let want_v = dy.signum() * cap.min(K_V * dy.abs());
        let err_v = want_v - vy;
        return if err_v >= push / 2.0 {
            MOVE_JUMP
        } else if err_v <= -push / 2.0 {
            MOVE_DESCEND
        } else {
            0
        };
    }
    if vy.abs() > TAP_STILL || prev & (MOVE_JUMP | MOVE_DESCEND) != 0 {
        return 0;
    }
    let step = push / (1.0 - V_DRAG);
    let high = step - BAND_LOW + 0.01;
    if dy > BAND_LOW {
        MOVE_JUMP
    } else if dy < -high {
        MOVE_DESCEND
    } else {
        0
    }
}

pub(super) const LIP: f64 = 0.5;

pub(super) fn lip(
    pos: [f64; 3],
    to: [f64; 3],
    top_at: impl Fn(BlockPos) -> Option<f64>,
) -> Option<f64> {
    let (dx, dz) = (to[0] - pos[0], to[2] - pos[2]);
    let len = (dx * dx + dz * dz).sqrt();
    if len < 1e-6 {
        return None;
    }
    let ahead = len.min(1.0);
    let row = pos[1].floor() as i32;
    let mut best: Option<f64> = None;
    let mut s = 0.0;
    while s <= ahead + 1e-9 {
        let (x, z) = (pos[0] + dx / len * s, pos[2] + dz / len * s);
        for cx in ((x - 0.35).floor() as i32)..=((x + 0.35).floor() as i32) {
            for cz in ((z - 0.35).floor() as i32)..=((z + 0.35).floor() as i32) {
                if let Some(top) = top_at(BlockPos::new(cx, row, cz))
                    && top > pos[1]
                    && top - pos[1] <= LIP
                {
                    best = Some(best.map_or(top, |b: f64| b.max(top)));
                }
            }
        }
        s += SAMPLE;
    }
    best
}

#[allow(clippy::too_many_arguments)]
pub(super) fn hold_at(
    pos: [f64; 3],
    vel: [f64; 3],
    at: [f64; 3],
    yaw: f32,
    speed: f64,
    slack: [f64; 2],
    prev: u8,
) -> u8 {
    let (dx, dz) = (at[0] - pos[0], at[2] - pos[2]);
    let inside =
        (dx * dx + dz * dz).sqrt() <= HOLD_SLACK && dx.abs() <= slack[0] && dz.abs() <= slack[1];
    if inside {
        let walk = brake(vel, yaw) & !(MOVE_JUMP | MOVE_DESCEND);
        walk | vertical_key(at[1] - pos[1], vel[1], speed * V_IMPULSE, NEAR_CAPS.v, prev)
    } else {
        track(pos, vel, at, yaw, speed, NEAR_CAPS, prev)
    }
}

pub(super) fn brake(vel: [f64; 3], yaw: f32) -> u8 {
    let mut keys = 0;
    if (vel[0] * vel[0] + vel[2] * vel[2]).sqrt() > BRAKE_H {
        keys |= keys_toward(-vel[0], -vel[2], yaw).unwrap_or(0);
    }
    if vel[1] > BRAKE_V {
        keys |= MOVE_DESCEND;
    } else if vel[1] < -BRAKE_V {
        keys |= MOVE_JUMP;
    }
    keys
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Watch {
    pub(super) best: f64,
    pub(super) until: i64,
}

const PROGRESS: f64 = 0.1;

impl Watch {
    pub(super) fn new(dist: f64, now: i64, idle: i64) -> Watch {
        Watch {
            best: dist,
            until: now + idle,
        }
    }

    pub(super) fn tick(self, dist: f64, now: i64, idle: i64) -> Option<Watch> {
        if dist < self.best - PROGRESS {
            Some(Watch {
                best: dist,
                until: now + idle,
            })
        } else if now >= self.until {
            None
        } else {
            Some(self)
        }
    }
}

pub(super) fn distance(pos: [f64; 3], p: [f64; 3]) -> f64 {
    (0..3).map(|i| (p[i] - pos[i]).powi(2)).sum::<f64>().sqrt()
}

pub(super) fn point(p: BlockPos) -> [f64; 3] {
    [
        f64::from(p.x) + 0.5,
        f64::from(p.y) + HOVER_LIFT,
        f64::from(p.z) + 0.5,
    ]
}

fn box_cells(f: [f64; 3], margin: f64) -> impl Iterator<Item = BlockPos> {
    let r = 0.3 + margin;
    let lo = [
        (f[0] - r).floor() as i32,
        (f[1] - margin).floor() as i32,
        (f[2] - r).floor() as i32,
    ];
    let hi = [
        (f[0] + r).floor() as i32,
        (f[1] + 1.8 + margin).floor() as i32,
        (f[2] + r).floor() as i32,
    ];
    (lo[0]..=hi[0]).flat_map(move |x| {
        (lo[1]..=hi[1]).flat_map(move |y| (lo[2]..=hi[2]).map(move |z| BlockPos::new(x, y, z)))
    })
}

pub(super) fn line_free(
    a: [f64; 3],
    b: [f64; 3],
    margin: f64,
    passable: &impl Fn(BlockPos) -> bool,
) -> bool {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let steps = (len / SAMPLE).ceil().max(1.0) as usize;
    (0..=steps).all(|i| {
        let t = i as f64 / steps as f64;
        box_cells([a[0] + d[0] * t, a[1] + d[1] * t, a[2] + d[2] * t], margin).all(passable)
    })
}

#[allow(dead_code)]
pub(super) fn touching(
    f: [f64; 3],
    margin: f64,
    passable: &impl Fn(BlockPos) -> bool,
) -> Vec<BlockPos> {
    box_cells(f, margin).filter(|c| !passable(*c)).collect()
}

pub(super) fn box_clear(f: [f64; 3], margin: f64, passable: &impl Fn(BlockPos) -> bool) -> bool {
    box_cells(f, margin).all(passable)
}

pub(super) fn caps_for(f: [f64; 3], left: f64, passable: &impl Fn(BlockPos) -> bool) -> Caps {
    if left > FAR_LEG && box_clear(f, CLEAR, passable) {
        FAR_CAPS
    } else {
        NEAR_CAPS
    }
}

pub(super) enum Grid {
    Found(Vec<BlockPos>),
    Pending,
    None,
}

pub(super) struct GridSearch {
    goal: BlockPos,
    open: BinaryHeap<Reverse<(i32, i32, i32, i32, i32)>>,
    came: HashMap<BlockPos, BlockPos>,
    cost: HashMap<BlockPos, i32>,
    taken: usize,
}

fn manhattan(a: BlockPos, b: BlockPos) -> i32 {
    (a.x - b.x).abs() + (a.y - b.y).abs() + (a.z - b.z).abs()
}

impl GridSearch {
    pub(super) fn new(from: BlockPos, goal: BlockPos) -> GridSearch {
        let mut open = BinaryHeap::new();
        open.push(Reverse((manhattan(from, goal), 0, from.x, from.y, from.z)));
        let mut cost = HashMap::new();
        cost.insert(from, 0);
        GridSearch {
            goal,
            open,
            came: HashMap::new(),
            cost,
            taken: 0,
        }
    }

    pub(super) fn step(
        &mut self,
        budget: usize,
        room: &impl Fn(BlockPos) -> bool,
        crowded: &impl Fn(BlockPos) -> bool,
    ) -> Grid {
        let mut left = budget;
        while let Some(Reverse((_, g, x, y, z))) = self.open.pop() {
            let p = BlockPos::new(x, y, z);
            if self.cost.get(&p).is_some_and(|&c| c < g) {
                continue;
            }
            if p == self.goal {
                return Grid::Found(self.path());
            }
            self.taken += 1;
            if self.taken > MAX_NODES {
                return Grid::None;
            }
            for n in [
                BlockPos::new(x + 1, y, z),
                BlockPos::new(x - 1, y, z),
                BlockPos::new(x, y + 1, z),
                BlockPos::new(x, y - 1, z),
                BlockPos::new(x, y, z + 1),
                BlockPos::new(x, y, z - 1),
            ] {
                if !room(n) {
                    continue;
                }
                let step = if n != self.goal && crowded(n) {
                    1 + CROWD_COST
                } else {
                    1
                };
                let gn = g + step;
                if self.cost.get(&n).is_some_and(|&c| c <= gn) {
                    continue;
                }
                self.cost.insert(n, gn);
                self.came.insert(n, p);
                self.open
                    .push(Reverse((gn + manhattan(n, self.goal), gn, n.x, n.y, n.z)));
            }
            left -= 1;
            if left == 0 {
                return Grid::Pending;
            }
        }
        Grid::None
    }

    fn path(&self) -> Vec<BlockPos> {
        let mut cells = vec![self.goal];
        while let Some(&prev) = self.came.get(cells.last().unwrap_or(&self.goal)) {
            cells.push(prev);
        }
        cells.reverse();
        corners(&cells)
    }
}

pub(super) fn corners(cells: &[BlockPos]) -> Vec<BlockPos> {
    let step = |a: BlockPos, b: BlockPos| (b.x - a.x, b.y - a.y, b.z - a.z);
    let mut out = Vec::new();
    for i in 1..cells.len() {
        let last = i + 1 == cells.len();
        if last || step(cells[i - 1], cells[i]) != step(cells[i], cells[i + 1]) {
            out.push(cells[i]);
        }
    }
    out
}

#[derive(Default)]
pub(super) struct Fly {
    pub(super) user_was_flying: Option<bool>,
    pub(super) toggled_at: Option<i64>,
    pub(super) landed_at: Option<i64>,
    pub(super) refused: bool,
    pub(super) path: Vec<BlockPos>,
    pub(super) search: Option<GridSearch>,
    pub(super) goal: Option<super::spot::Target>,
    pub(super) tight: bool,
    pub(super) hold: Option<[f64; 3]>,
    pub(super) landing: bool,
}

impl Fly {
    pub(super) fn go(
        &mut self,
        pos: [f64; 3],
        from: BlockPos,
        to: BlockPos,
        passable: &impl Fn(BlockPos) -> bool,
    ) {
        if line_free(pos, point(to), LINE_MARGIN, passable) {
            self.path = vec![to];
            self.search = None;
        } else {
            self.path.clear();
            self.search = Some(GridSearch::new(from, to));
        }
    }

    pub(super) fn clear(&mut self) {
        self.path.clear();
        self.search = None;
        self.toggled_at = None;
        self.goal = None;
        self.tight = false;
        self.landing = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{MOVE_BACK, MOVE_FORWARD};

    #[test]
    fn stopping_distances_follow_the_drag() {
        let top = top_speed(DEFAULT_SPEED, H_DRAG);
        assert!((top - 0.50556).abs() < 1e-4);
        assert!((coast(top, H_DRAG) - 5.6173).abs() < 1e-3);
        let up = top_speed(DEFAULT_SPEED * V_IMPULSE, V_DRAG);
        assert!((up - 0.225).abs() < 1e-9);
        assert!((coast(up, V_DRAG) - 0.5625).abs() < 1e-9);
        assert!((coast(NEAR_CAPS.h, H_DRAG) - 1.1111).abs() < 1e-3);
        assert!((coast(K_H * 2.0, H_DRAG) - 2.0).abs() < 1e-9);
        assert!((coast(K_V * 2.0, V_DRAG) - 2.0).abs() < 1e-9);
    }

    fn push_z(keys: u8) -> f64 {
        f64::from(u8::from(keys & MOVE_FORWARD != 0)) - f64::from(u8::from(keys & MOVE_BACK != 0))
    }

    fn push_y(keys: u8) -> f64 {
        f64::from(u8::from(keys & MOVE_JUMP != 0)) - f64::from(u8::from(keys & MOVE_DESCEND != 0))
    }

    #[test]
    fn the_tracked_stop_needs_no_reverse_push() {
        for caps in [NEAR_CAPS, FAR_CAPS] {
            for target in [0.6, 1.0, 3.0, 5.0, 10.0] {
                let (mut z, mut v, mut pending) = (0.0f64, 0.0f64, 0u8);
                let mut most = 0.0f64;
                for _ in 0..400 {
                    let keys = pending;
                    pending = track(
                        [0.0, 64.0, z],
                        [0.0, 0.0, v],
                        [0.0, 64.0, target],
                        0.0,
                        DEFAULT_SPEED,
                        caps,
                        keys,
                    );
                    v += DEFAULT_SPEED * push_z(keys);
                    z += v;
                    v *= H_DRAG;
                    most = most.max(z);
                }
                assert!(
                    most - target < 0.25,
                    "{caps:?} {target}: overshoot {}",
                    most - target
                );
                assert!(
                    (z - target).abs() < TIGHT,
                    "{caps:?} {target}: ended at {z}"
                );
            }
            for target in [0.3, 1.0, 2.0, 4.0, -3.0] {
                let (mut y, mut v, mut pending) = (0.0f64, 0.0f64, 0u8);
                for _ in 0..300 {
                    let keys = pending;
                    pending = track(
                        [0.0, y, 0.0],
                        [0.0, v, 0.0],
                        [0.0, target, 0.0],
                        0.0,
                        DEFAULT_SPEED,
                        caps,
                        keys,
                    );
                    v += DEFAULT_SPEED * V_IMPULSE * push_y(keys);
                    y += v;
                    v *= V_DRAG;
                }
                assert!(
                    (y - target).abs() < ARRIVE,
                    "{caps:?} {target}: ended at {y}"
                );
            }
        }
    }

    #[test]
    fn small_vertical_errors_close_with_single_taps() {
        let step = DEFAULT_SPEED * V_IMPULSE / (1.0 - V_DRAG);
        let high = step - BAND_LOW + 0.01;
        for dy in [0.05, 0.1, 0.17, 0.3, -0.05, -0.1, -0.17, -0.3, -0.4, 0.45] {
            let (mut y, mut v, mut pending, mut taps) = (0.0f64, 0.0f64, 0u8, 0);
            let mut lowest = 0.0f64;
            for _ in 0..80 {
                let keys = pending;
                pending = vertical_key(dy - y, v, DEFAULT_SPEED * V_IMPULSE, NEAR_CAPS.v, keys);
                taps += usize::from(pending != 0);
                v += DEFAULT_SPEED * V_IMPULSE * push_y(keys);
                y += v;
                v *= V_DRAG;
                lowest = lowest.min(y - dy);
            }
            let e = y - dy;
            assert!(
                e >= -BAND_LOW - 1e-9 && e <= high + 1e-9,
                "error {dy}: ended {e} off"
            );
            assert!(taps <= 1, "error {dy}: {taps} taps");
            assert!(v.abs() < 1e-3, "error {dy}: still moving at {v}");
            if dy <= BAND_LOW {
                assert!(
                    lowest >= -BAND_LOW - 1e-9 || dy > 0.0,
                    "error {dy}: sank to {lowest}"
                );
            }
        }
        assert_eq!(
            vertical_key(0.17, 0.0, DEFAULT_SPEED * V_IMPULSE, NEAR_CAPS.v, 0),
            MOVE_JUMP
        );
        assert_eq!(
            vertical_key(0.17, 0.0, DEFAULT_SPEED * V_IMPULSE, NEAR_CAPS.v, MOVE_JUMP),
            0
        );
    }

    #[test]
    fn a_lip_ahead_is_found() {
        let stair = BlockPos::new(1037, 104, 1034);
        let top = |p: BlockPos| (p == stair).then_some(105.0);
        let pos = [1037.4949, 104.9292, 1034.2];
        assert_eq!(lip(pos, [1037.5, 105.1, 1035.5], top), Some(105.0));
        assert_eq!(
            lip([1037.5, 105.1, 1034.2], [1037.5, 105.1, 1035.5], top),
            None
        );
    }

    #[test]
    fn the_keys_head_for_the_point_and_stop_on_it() {
        let (keys, done) = steer(
            [0.5, 64.1, 0.5],
            [0.0; 3],
            [0.5, 64.1, 6.5],
            0.0,
            DEFAULT_SPEED,
            ARRIVE,
            NEAR_CAPS,
            0,
        );
        assert_eq!((keys, done), (MOVE_FORWARD, false));
        let (keys, _) = steer(
            [0.5, 64.1, 6.5],
            [0.0; 3],
            [0.5, 64.1, 0.5],
            0.0,
            DEFAULT_SPEED,
            ARRIVE,
            NEAR_CAPS,
            0,
        );
        assert_eq!(keys, MOVE_BACK);
        let (keys, _) = steer(
            [0.5, 64.1, 5.0],
            [0.0, 0.0, 0.4],
            [0.5, 64.1, 6.5],
            0.0,
            DEFAULT_SPEED,
            ARRIVE,
            NEAR_CAPS,
            0,
        );
        assert_eq!(keys, MOVE_BACK);
        let (keys, _) = steer(
            [0.5, 64.1, 0.5],
            [0.0, 0.0, 0.1],
            [0.5, 64.1, 6.5],
            0.0,
            DEFAULT_SPEED,
            ARRIVE,
            NEAR_CAPS,
            0,
        );
        assert_eq!(keys, 0);
        let (keys, _) = steer(
            [0.5, 64.1, 0.5],
            [0.0; 3],
            [0.5, 67.1, 0.5],
            0.0,
            DEFAULT_SPEED,
            ARRIVE,
            NEAR_CAPS,
            0,
        );
        assert_eq!(keys, MOVE_JUMP);
        let (keys, _) = steer(
            [0.5, 67.1, 0.5],
            [0.0; 3],
            [0.5, 64.1, 0.5],
            0.0,
            DEFAULT_SPEED,
            ARRIVE,
            NEAR_CAPS,
            0,
        );
        assert_eq!(keys, MOVE_DESCEND);
        assert_eq!(
            steer(
                [0.55, 64.12, 0.5],
                [0.0; 3],
                [0.5, 64.1, 0.5],
                0.0,
                DEFAULT_SPEED,
                ARRIVE,
                NEAR_CAPS,
                0
            ),
            (0, true)
        );
        assert_eq!(
            steer(
                [0.8, 64.3, 0.3],
                [0.2, 0.0, 0.0],
                [0.5, 64.1, 0.5],
                0.0,
                DEFAULT_SPEED,
                ARRIVE,
                NEAR_CAPS,
                0
            ),
            (0, true)
        );
        assert!(
            !steer(
                [0.95, 64.1, 0.5],
                [0.0; 3],
                [0.5, 64.1, 0.5],
                0.0,
                DEFAULT_SPEED,
                PASS,
                NEAR_CAPS,
                0
            )
            .1
        );
        assert!(
            !steer(
                [0.85, 64.3, 0.3],
                [0.0; 3],
                [0.5, 64.1, 0.5],
                0.0,
                DEFAULT_SPEED,
                TIGHT,
                NEAR_CAPS,
                0
            )
            .1
        );
        assert!(
            steer(
                [0.55, 64.15, 0.45],
                [0.0; 3],
                [0.5, 64.1, 0.5],
                0.0,
                DEFAULT_SPEED,
                TIGHT,
                NEAR_CAPS,
                0
            )
            .1
        );
    }

    #[test]
    fn holding_position_brakes_inside_the_slack_and_pulls_back_outside() {
        let at = [0.5, 64.1, 0.5];
        let open = [HOLD_SLACK; 2];
        assert_eq!(
            hold_at(
                [0.6, 64.1, 0.5],
                [0.0, 0.0, 0.2],
                at,
                0.0,
                DEFAULT_SPEED,
                open,
                0
            ),
            MOVE_BACK
        );
        assert_eq!(
            hold_at([0.6, 64.1, 0.5], [0.0; 3], at, 0.0, DEFAULT_SPEED, open, 0),
            0
        );
        assert_eq!(
            hold_at([0.5, 64.1, 1.5], [0.0; 3], at, 0.0, DEFAULT_SPEED, open, 0),
            MOVE_BACK
        );
        assert_ne!(
            hold_at(
                [0.65, 64.1, 0.5],
                [0.0; 3],
                at,
                0.0,
                DEFAULT_SPEED,
                [WALL_SLACK, HOLD_SLACK],
                0
            ),
            0
        );
    }

    #[test]
    fn a_hold_beside_a_wall_keeps_off_it() {
        let wall = |p: BlockPos| p.x == 1040;
        let (at, slack) = clear_hold([1041.30, 103.1, 1035.0], wall);
        assert!((at[0] - (1041.0 + 0.3 + WALL_GAP)).abs() < 1e-9);
        assert_eq!(at[2], 1035.0);
        assert_eq!(slack, [WALL_SLACK, HOLD_SLACK]);
        assert!(at[0] - WALL_SLACK - 0.3 > 1041.0);
        let both = |p: BlockPos| p.z == 4 || p.z == 6;
        let (at, slack) = clear_hold([0.5, 64.0, 5.2], both);
        assert_eq!(at[2], 5.5);
        assert_eq!(slack, [HOLD_SLACK, WALL_SLACK]);
    }

    #[test]
    fn the_brake_pushes_against_the_velocity_and_stops_short_of_reversing() {
        assert_eq!(brake([0.0, 0.0, 0.3], 0.0), MOVE_BACK);
        assert_eq!(brake([0.0, 0.0, 0.04], 0.0), 0);
        assert_eq!(brake([0.0, 0.2, 0.0], 0.0), MOVE_DESCEND);
        assert_eq!(brake([0.0, -0.2, 0.0], 0.0), MOVE_JUMP);
        assert_eq!(brake([0.0, 0.1, 0.0], 0.0), 0);
    }

    #[test]
    fn stuck_means_no_progress_not_one_block() {
        let w = Watch::new(5.0, 0, 60);
        let w = w.tick(4.8, 10, 60).unwrap();
        assert_eq!(
            w,
            Watch {
                best: 4.8,
                until: 70
            }
        );
        assert_eq!(w.tick(4.78, 69, 60), Some(w));
        assert_eq!(w.tick(4.78, 70, 60), None);
    }

    #[test]
    fn a_wall_is_flown_around() {
        let wall = |p: BlockPos| p.x == 2 && (-3..=3).contains(&p.z) && (64..=65).contains(&p.y);
        let passable = |p: BlockPos| !wall(p) && p.y >= 64;
        let room = |p: BlockPos| passable(p) && passable(BlockPos::new(p.x, p.y + 1, p.z));
        let crowded = |_: BlockPos| false;
        let from = BlockPos::new(0, 64, 0);
        let to = BlockPos::new(4, 64, 0);
        assert!(!line_free(point(from), point(to), 0.0, &passable));
        let mut search = GridSearch::new(from, to);
        let path = loop {
            match search.step(16, &room, &crowded) {
                Grid::Found(p) => break p,
                Grid::Pending => {}
                Grid::None => panic!("no path"),
            }
        };
        assert_eq!(path.last(), Some(&to));
        let mut at = from;
        for c in &path {
            let n = (c.x - at.x).abs() + (c.y - at.y).abs() + (c.z - at.z).abs();
            assert!(
                [c.x == at.x, c.y == at.y, c.z == at.z]
                    .iter()
                    .filter(|s| **s)
                    .count()
                    >= 2,
                "{at:?} -> {c:?}"
            );
            assert!(n > 0);
            at = *c;
        }
        assert!(line_free(
            point(from),
            point(BlockPos::new(0, 64, 6)),
            0.0,
            &passable
        ));
        assert!(!line_free(
            point(from),
            point(BlockPos::new(0, 64, 6)),
            LINE_MARGIN,
            &passable
        ));
    }

    #[test]
    fn the_path_keeps_off_walls() {
        let wall = |p: BlockPos| p.z == 1 && (0..=6).contains(&p.x) && (64..=70).contains(&p.y);
        let passable = |p: BlockPos| !wall(p) && p.y >= 60;
        let room = |p: BlockPos| passable(p) && passable(BlockPos::new(p.x, p.y + 1, p.z));
        let crowded = |p: BlockPos| {
            [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dz)| {
                !passable(BlockPos::new(p.x + dx, p.y, p.z + dz))
                    || !passable(BlockPos::new(p.x + dx, p.y + 1, p.z + dz))
            })
        };
        let mut search = GridSearch::new(BlockPos::new(0, 65, 0), BlockPos::new(6, 65, 0));
        let path = loop {
            match search.step(64, &room, &crowded) {
                Grid::Found(p) => break p,
                Grid::Pending => {}
                Grid::None => panic!("no path"),
            }
        };
        assert!(path.iter().any(|c| c.z == -1), "{path:?}");
    }

    #[test]
    fn straight_runs_are_cut_to_their_corners() {
        let cells: Vec<BlockPos> = [
            (0, 0, 0),
            (1, 0, 0),
            (2, 0, 0),
            (2, 1, 0),
            (2, 2, 0),
            (3, 2, 0),
        ]
        .iter()
        .map(|&(x, y, z)| BlockPos::new(x, y, z))
        .collect();
        assert_eq!(
            corners(&cells),
            vec![
                BlockPos::new(2, 0, 0),
                BlockPos::new(2, 2, 0),
                BlockPos::new(3, 2, 0)
            ]
        );
    }
}
