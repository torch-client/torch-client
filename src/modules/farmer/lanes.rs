use std::collections::VecDeque;

use azalea_core::position::BlockPos;

use crate::modules::auto_mine::shared::{EYE, REACH};

pub(super) const MIN_RUN: usize = 3;

pub(super) const MAX_GAP: i32 = 2;

pub(super) const LANE_COVER: i32 = 1;

pub(super) const ARRIVE: f64 = 0.3;

const FACE_Y: f64 = 1.5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Axis {
    #[default]
    X,
    Z,
}

impl Axis {
    pub(super) fn along(self, p: BlockPos) -> i32 {
        match self {
            Axis::X => p.x,
            Axis::Z => p.z,
        }
    }

    pub(super) fn cross(self, p: BlockPos) -> i32 {
        match self {
            Axis::X => p.z,
            Axis::Z => p.x,
        }
    }

    pub(super) fn at(self, along: i32, cross: i32, y: i32) -> BlockPos {
        match self {
            Axis::X => BlockPos::new(along, y, cross),
            Axis::Z => BlockPos::new(cross, y, along),
        }
    }

    fn shift(self, p: BlockPos, by: i32) -> BlockPos {
        self.at(self.along(p) + by, self.cross(p), p.y)
    }

    #[allow(dead_code)]
    pub(super) fn name(self) -> &'static str {
        match self {
            Axis::X => "x",
            Axis::Z => "z",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Row {
    pub(super) y: i32,
    pub(super) cross: i32,
    pub(super) from: i32,
    pub(super) to: i32,
    pub(super) count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Cell {
    Free,
    Crop,
    Water,
    Blocked,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Lane {
    pub(super) axis: Axis,
    pub(super) y: i32,
    pub(super) cross: i32,
    pub(super) from: i32,
    pub(super) to: i32,
    pub(super) rows: Vec<i32>,
    pub(super) reach: i32,
    pub(super) spacing: i32,
    #[allow(dead_code)]
    pub(super) crop: bool,
}

impl Lane {
    pub(super) fn holds(&self, t: BlockPos) -> bool {
        t.y == self.y + 1
            && self.rows.contains(&self.axis.cross(t))
            && (self.from.min(self.to)..=self.from.max(self.to)).contains(&self.axis.along(t))
    }

    fn end(&self, forward: bool) -> BlockPos {
        let a = if forward { self.from } else { self.to };
        self.axis.at(a, self.cross, self.y)
    }

    pub(super) fn stops(&self, forward: bool) -> Vec<BlockPos> {
        let (start, end, dir) = if forward {
            (self.from, self.to, 1)
        } else {
            (self.to, self.from, -1)
        };
        let reach = self.reach.max(0);
        let step = self.spacing.max(1);
        let mut p = start;
        let mut out = vec![self.axis.at(p, self.cross, self.y)];
        while (end - p) * dir > reach {
            p += dir * step;
            if (end - p) * dir < 0 {
                p = end;
            }
            out.push(self.axis.at(p, self.cross, self.y));
        }
        out
    }
}

fn flat_reach() -> f64 {
    let r = REACH - ARRIVE;
    let dy = EYE - FACE_Y;
    (r * r - dy * dy).sqrt()
}

pub(super) fn window(d: i32, half: f64) -> i32 {
    let h = flat_reach();
    if d == 0 {
        return (h + half).floor() as i32;
    }
    let lateral = f64::from(d.abs()) - half;
    if lateral > h {
        return -1;
    }
    (h * h - lateral * lateral).sqrt().floor() as i32
}

pub(super) fn covers(axis: Axis, stop: BlockPos, t: BlockPos, half: f64) -> bool {
    if t.y != stop.y + 1 {
        return false;
    }
    let d = (axis.cross(t) - axis.cross(stop)).abs();
    let da = (axis.along(t) - axis.along(stop)).abs();
    da <= window(d, half)
}

pub(super) fn runs(targets: &[BlockPos], axis: Axis) -> (Vec<Row>, usize) {
    let mut keys: Vec<(i32, i32, i32)> = targets
        .iter()
        .map(|t| (t.y, axis.cross(*t), axis.along(*t)))
        .collect();
    keys.sort_unstable();
    keys.dedup();
    let mut rows = Vec::new();
    let mut pairs = 0;
    let mut i = 0;
    while i < keys.len() {
        let (y, cross, from) = keys[i];
        let mut to = from;
        let mut adjacent = 0;
        let mut j = i + 1;
        while j < keys.len()
            && keys[j].0 == y
            && keys[j].1 == cross
            && keys[j].2 - to <= MAX_GAP + 1
        {
            if keys[j].2 - to == 1 {
                adjacent += 1;
            }
            to = keys[j].2;
            j += 1;
        }
        let count = j - i;
        if count >= MIN_RUN {
            rows.push(Row {
                y,
                cross,
                from,
                to,
                count,
            });
            pairs += adjacent;
        }
        i = j;
    }
    (rows, pairs)
}

pub(super) fn rows(targets: &[BlockPos]) -> (Axis, Vec<Row>, Vec<BlockPos>) {
    let (xs, x_pairs) = runs(targets, Axis::X);
    let (zs, z_pairs) = runs(targets, Axis::Z);
    let held = |rows: &[Row]| rows.iter().map(|r| r.count).sum::<usize>();
    let (axis, rows) = if (z_pairs, held(&zs)) > (x_pairs, held(&xs)) {
        (Axis::Z, zs)
    } else {
        (Axis::X, xs)
    };
    let scattered = targets
        .iter()
        .copied()
        .filter(|t| {
            !rows.iter().any(|r| {
                r.y == t.y && r.cross == axis.cross(*t) && (r.from..=r.to).contains(&axis.along(*t))
            })
        })
        .collect();
    (axis, rows, scattered)
}

pub(super) fn line(
    axis: Axis,
    cross: i32,
    y: i32,
    from: i32,
    to: i32,
    cell: &impl Fn(BlockPos) -> Cell,
) -> Option<(i32, i32)> {
    let (mut crop, mut blocked) = (0, 0);
    for a in from.min(to)..=from.max(to) {
        match cell(axis.at(a, cross, y)) {
            Cell::Free => {}
            Cell::Crop => crop += 1,
            Cell::Water => return None,
            Cell::Blocked => blocked += 1,
        }
    }
    (blocked * 2 <= (to - from).abs() + 1).then_some((crop, blocked))
}

pub(super) fn lanes(
    rows: &[Row],
    axis: Axis,
    half: f64,
    cell: impl Fn(BlockPos) -> Cell,
) -> Vec<Lane> {
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by_key(|&i| (rows[i].y, rows[i].cross, rows[i].from));
    let mut covered = vec![false; rows.len()];
    let mut out = Vec::new();
    for &i in &order {
        if covered[i] {
            continue;
        }
        let r = rows[i];
        let y = r.y - 1;
        let mut best: Option<((usize, i32, i32, bool, i32), Lane, Vec<usize>)> = None;
        for c in r.cross - LANE_COVER..=r.cross + LANE_COVER {
            let members: Vec<usize> = order
                .iter()
                .copied()
                .filter(|&j| {
                    !covered[j] && rows[j].y == r.y && (rows[j].cross - c).abs() <= LANE_COVER
                })
                .collect();
            let from = members
                .iter()
                .map(|&j| rows[j].from)
                .min()
                .unwrap_or(r.from);
            let to = members.iter().map(|&j| rows[j].to).max().unwrap_or(r.to);
            let Some((crop, blocked)) = line(axis, c, y, from, to, &cell) else {
                continue;
            };
            let near: i32 = members
                .iter()
                .map(|&j| LANE_COVER + 1 - (rows[j].cross - c).abs())
                .sum();
            let score = (
                members.len(),
                -blocked,
                near,
                crop == 0,
                -(c - r.cross).abs(),
            );
            if best.as_ref().is_some_and(|(s, _, _)| *s >= score) {
                continue;
            }
            let far = members
                .iter()
                .map(|&j| (rows[j].cross - c).abs())
                .max()
                .unwrap_or(0);
            let reach = window(far, half);
            let in_row = window(0, half);
            let mut spacing = reach + 2;
            let (mut from, mut to) = (from, to);
            if crop > 0 {
                spacing = spacing.min(in_row);
                if cell(axis.at(from - 1, c, y)) == Cell::Free {
                    from -= 1;
                }
                if cell(axis.at(to + 1, c, y)) == Cell::Free {
                    to += 1;
                }
            }
            let mut covers: Vec<i32> = members.iter().map(|&j| rows[j].cross).collect();
            covers.sort_unstable();
            covers.dedup();
            let lane = Lane {
                axis,
                y,
                cross: c,
                from,
                to,
                rows: covers,
                reach,
                spacing: spacing.max(1),
                crop: crop > 0,
            };
            best = Some((score, lane, members));
        }
        match best {
            Some((_, lane, members)) => {
                for j in members {
                    covered[j] = true;
                }
                out.push(lane);
            }
            None => covered[i] = true,
        }
    }
    out
}

pub(super) fn order(lanes: &[Lane], start: BlockPos) -> Vec<(usize, bool)> {
    let ends: Vec<[BlockPos; 2]> = lanes.iter().map(|l| [l.end(true), l.end(false)]).collect();
    order_ends(&ends, start)
}

pub(super) fn order_ends(ends: &[[BlockPos; 2]], start: BlockPos) -> Vec<(usize, bool)> {
    let d2 = super::find::dist2;
    let mut left: Vec<usize> = (0..ends.len()).collect();
    let mut at = start;
    let mut out = Vec::with_capacity(ends.len());
    while !left.is_empty() {
        let mut best = (i64::MAX, 0, true);
        for (k, &i) in left.iter().enumerate() {
            for forward in [true, false] {
                let d = d2(if forward { ends[i][0] } else { ends[i][1] }, at);
                if d < best.0 {
                    best = (d, k, forward);
                }
            }
        }
        let i = left.remove(best.1);
        out.push((i, best.2));
        at = if best.2 { ends[i][1] } else { ends[i][0] };
    }
    out
}

#[derive(Debug, Default)]
pub(super) struct Plan {
    pub(super) axis: Axis,
    pub(super) half: f64,
    pub(super) stops: VecDeque<(BlockPos, usize)>,
    pub(super) lanes: Vec<(Lane, bool)>,
    current: Option<usize>,
}

impl Plan {
    pub(super) fn build(
        targets: &[BlockPos],
        half: f64,
        start: BlockPos,
        cell: impl Fn(BlockPos) -> Cell,
    ) -> (Plan, usize, usize) {
        let (axis, rows, scattered) = rows(targets);
        let lanes = lanes(&rows, axis, half, cell);
        let mut plan = Plan {
            axis,
            half,
            ..Plan::default()
        };
        for (k, (i, forward)) in order(&lanes, start).into_iter().enumerate() {
            plan.stops
                .extend(lanes[i].stops(forward).into_iter().map(|s| (s, k)));
            plan.lanes.push((lanes[i].clone(), forward));
        }
        (plan, rows.len(), scattered.len())
    }

    pub(super) fn lane(&self) -> Option<&Lane> {
        let k = self.current.or_else(|| self.stops.front().map(|s| s.1))?;
        self.lanes.get(k).map(|(l, _)| l)
    }

    pub(super) fn next(
        &mut self,
        ready: &[BlockPos],
        feet: BlockPos,
        plant: impl Fn(BlockPos) -> bool,
    ) -> Option<BlockPos> {
        while let Some((stop, k)) = self.stops.pop_front() {
            let lane = &self.lanes[k].0;
            if !ready
                .iter()
                .any(|t| lane.holds(*t) && covers(self.axis, stop, *t, self.half))
            {
                continue;
            }
            self.current = Some(k);
            let head = |p: BlockPos| BlockPos::new(p.x, p.y + 1, p.z);
            let mut at = stop;
            let dir = (self.axis.along(feet) - self.axis.along(stop)).signum();
            if plant(head(stop)) && dir != 0 {
                for k in 1..=window(0, self.half) {
                    let c = self.axis.shift(stop, dir * k);
                    if !plant(head(c)) {
                        at = c;
                        break;
                    }
                }
            }
            if at == feet {
                continue;
            }
            return Some(at);
        }
        self.current = None;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HALF: f64 = 0.375;

    fn farm() -> Vec<BlockPos> {
        let mut out = Vec::new();
        for z in [1043, 1045, 1046, 1048, 1049, 1051] {
            for x in 1008..=1024 {
                out.push(BlockPos::new(x, 101, z));
            }
        }
        out
    }

    fn farm_cell(p: BlockPos) -> Cell {
        if p.y != 100 {
            return Cell::Blocked;
        }
        let in_farm = (1008..=1024).contains(&p.x);
        match p.z {
            1044 | 1047 | 1050 if in_farm => Cell::Water,
            1043 | 1045 | 1046 | 1048 | 1049 | 1051 if in_farm => Cell::Crop,
            _ => Cell::Free,
        }
    }

    #[test]
    fn windows_follow_the_reach() {
        assert_eq!(
            [0, 1, 2, 3, 4, 5].map(|d| window(d, HALF)),
            [4, 3, 3, 2, 1, -1]
        );
    }

    #[test]
    fn the_reported_farm_forms_six_rows_along_x() {
        let (axis, rows, scattered) = rows(&farm());
        assert_eq!(axis, Axis::X);
        assert_eq!(
            rows.iter().map(|r| r.cross).collect::<Vec<_>>(),
            vec![1043, 1045, 1046, 1048, 1049, 1051]
        );
        assert!(
            rows.iter()
                .all(|r| (r.from, r.to, r.count) == (1008, 1024, 17))
        );
        assert!(scattered.is_empty());
    }

    #[test]
    fn a_farm_along_z_groups_along_z() {
        let targets: Vec<BlockPos> = [0, 3]
            .iter()
            .flat_map(|&x| (0..10).map(move |z| BlockPos::new(x, 65, z)))
            .collect();
        let (axis, rows, _) = rows(&targets);
        assert_eq!(axis, Axis::Z);
        assert_eq!(
            rows.iter()
                .map(|r| (r.cross, r.from, r.to))
                .collect::<Vec<_>>(),
            vec![(0, 0, 9), (3, 0, 9)]
        );
    }

    #[test]
    fn small_gaps_join_a_row_and_large_ones_split_it() {
        let xs = [0, 1, 2, 5, 6, 7, 20, 21];
        let targets: Vec<BlockPos> = xs.iter().map(|&x| BlockPos::new(x, 65, 0)).collect();
        let (rows, _) = runs(&targets, Axis::X);
        assert_eq!(
            rows,
            vec![Row {
                y: 65,
                cross: 0,
                from: 0,
                to: 7,
                count: 6
            }]
        );
    }

    #[test]
    fn scattered_stalks_belong_to_no_row() {
        let mut targets: Vec<BlockPos> = (0..8).map(|x| BlockPos::new(x, 65, 0)).collect();
        targets.push(BlockPos::new(30, 65, 30));
        targets.push(BlockPos::new(3, 66, 0));
        let (_, rows, scattered) = rows(&targets);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            scattered,
            vec![BlockPos::new(30, 65, 30), BlockPos::new(3, 66, 0)]
        );
    }

    #[test]
    fn the_reported_farm_gets_four_lanes_through_rows() {
        let (axis, rows, _) = rows(&farm());
        let lanes = lanes(&rows, axis, HALF, farm_cell);
        let got: Vec<(i32, Vec<i32>, i32, i32, bool, i32)> = lanes
            .iter()
            .map(|l| (l.cross, l.rows.clone(), l.from, l.to, l.crop, l.spacing))
            .collect();
        assert_eq!(
            got,
            vec![
                (1043, vec![1043], 1007, 1025, true, 4),
                (1045, vec![1045, 1046], 1007, 1025, true, 4),
                (1048, vec![1048, 1049], 1007, 1025, true, 4),
                (1051, vec![1051], 1007, 1025, true, 4),
            ]
        );
    }

    #[test]
    fn a_row_beats_free_ground_on_drops_and_free_ground_serves_a_blocked_row() {
        let targets: Vec<BlockPos> = (0..10).map(|x| BlockPos::new(x, 65, 5)).collect();
        let cell = |p: BlockPos| match p.z {
            5 => Cell::Crop,
            4 | 6 => Cell::Free,
            _ => Cell::Blocked,
        };
        let (axis, rows, _) = rows(&targets);
        let lanes = lanes(&rows, axis, HALF, cell);
        assert_eq!(lanes.len(), 1);
        assert_eq!((lanes[0].cross, lanes[0].crop), (5, true));
        let blocked_row = |p: BlockPos| match p.z {
            4 | 6 => Cell::Free,
            _ => Cell::Blocked,
        };
        let lanes = super::lanes(&rows, axis, HALF, blocked_row);
        assert_eq!(
            (lanes[0].cross, lanes[0].crop, lanes[0].spacing),
            (4, false, 5)
        );
    }

    #[test]
    fn water_is_never_a_lane() {
        let targets: Vec<BlockPos> = (0..10).map(|x| BlockPos::new(x, 65, 5)).collect();
        let cell = |p: BlockPos| match p.z {
            4 => Cell::Free,
            _ => Cell::Water,
        };
        let (axis, rows, _) = rows(&targets);
        let lanes = lanes(&rows, axis, HALF, cell);
        assert_eq!(lanes.iter().map(|l| l.cross).collect::<Vec<_>>(), vec![4]);
        let all_water = |_: BlockPos| Cell::Water;
        assert!(super::lanes(&rows, axis, HALF, all_water).is_empty());
    }

    #[test]
    fn stops_are_spaced_and_end_in_reach_of_the_far_end() {
        let lane = Lane {
            axis: Axis::X,
            y: 100,
            cross: 1045,
            from: 1007,
            to: 1025,
            rows: vec![],
            reach: 3,
            spacing: 4,
            crop: true,
        };
        let xs = |f: bool| lane.stops(f).iter().map(|p| p.x).collect::<Vec<_>>();
        assert_eq!(xs(true), vec![1007, 1011, 1015, 1019, 1023]);
        assert_eq!(xs(false), vec![1025, 1021, 1017, 1013, 1009]);
        let short = Lane {
            from: 0,
            to: 2,
            ..lane.clone()
        };
        assert_eq!(short.stops(true).len(), 1);
        let free = Lane {
            from: 0,
            to: 16,
            spacing: 5,
            crop: false,
            ..lane
        };
        assert_eq!(
            free.stops(true).iter().map(|p| p.x).collect::<Vec<_>>(),
            vec![0, 5, 10, 15]
        );
    }

    #[test]
    fn lanes_are_walked_as_a_serpentine_from_the_nearest_end() {
        let lane = |cross: i32| Lane {
            axis: Axis::X,
            y: 100,
            cross,
            from: 0,
            to: 20,
            rows: vec![],
            reach: 3,
            spacing: 5,
            crop: false,
        };
        let lanes = [lane(0), lane(4), lane(8)];
        assert_eq!(
            order(&lanes, BlockPos::new(22, 100, 9)),
            vec![(2, false), (1, true), (0, false)]
        );
        assert_eq!(
            order(&lanes, BlockPos::new(-3, 100, -1)),
            vec![(0, true), (1, false), (2, true)]
        );
    }

    #[test]
    fn next_skips_empty_stops_and_steps_off_a_standing_stalk() {
        let (mut plan, rows, scattered) =
            Plan::build(&farm(), HALF, BlockPos::new(1016, 100, 1040), farm_cell);
        assert_eq!((rows, scattered), (6, 0));
        assert_eq!(plan.stops.len(), 20);
        let first = plan.stops.front().unwrap().0;
        assert_eq!((first.x, first.z), (1007, 1043));
        let far = BlockPos::new(1022, 101, first.z);
        let stop = plan
            .next(&[far], BlockPos::new(1016, 100, 1040), |_| false)
            .unwrap();
        assert!(covers(Axis::X, stop, far, HALF));
        let next = plan.stops.front().unwrap().0;
        let blocked = BlockPos::new(next.x, 101, next.z);
        let moved = plan.next(&[blocked], stop, |p| p == blocked).unwrap();
        assert_eq!((moved.z, (moved.x - next.x).abs()), (next.z, 1));
        assert_eq!(plan.next(&[], moved, |_| false), None);
        assert!(plan.lane().is_none());
    }

    #[test]
    fn stops_and_harvest_follow_the_current_lane() {
        let (mut plan, _, _) =
            Plan::build(&farm(), HALF, BlockPos::new(1016, 100, 1040), farm_cell);
        assert_eq!(plan.lane().map(|l| l.cross), Some(1043));
        let row_1045 = BlockPos::new(1009, 101, 1045);
        let row_1043 = BlockPos::new(1009, 101, 1043);
        let first = plan.lanes[0].0.clone();
        assert!(first.holds(row_1043) && !first.holds(row_1045));
        let stop = plan
            .next(&[row_1045], BlockPos::new(1016, 100, 1040), |_| false)
            .unwrap();
        assert_eq!(stop, BlockPos::new(1013, 100, 1045));
        assert_eq!(
            plan.lane().map(|l| (l.cross, l.rows.clone())),
            Some((1045, vec![1045, 1046]))
        );
    }
}
