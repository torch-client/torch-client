use std::time::Duration;

use azalea::block::BlockState;
use azalea::pathfinder::world::{is_block_state_passable, is_block_state_standable};
use azalea::physics::collision::BlockWithShape;
use azalea_core::position::BlockPos;
use azalea_registry::builtin::ItemKind;
use azalea_world::World;

use super::fly::HOVER_LIFT;
use super::plan::{Job, Plan, dist2, holds};
use super::rules::{self, Stock};
use crate::blocks::state_is_air;
use crate::modules::auto_mine::shared::{EYE, REACH, aim_at};
use crate::modules::place::Click;
use crate::platform::time::Instant;

pub(super) const SPOT_RADIUS: i32 = 4;

pub(super) const SPOT_UP: i32 = 3;

pub(super) const SPOT_DOWN: i32 = 5;

pub(super) const HALF_DIAG: f64 = 0.87;

const SPOT_BUDGET: Duration = Duration::from_millis(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Pass {
    Stand,
    Build,
    HoverOut,
    HoverIn,
}

impl Pass {
    fn flying(self) -> bool {
        matches!(self, Pass::HoverOut | Pass::HoverIn)
    }
}

pub(super) const CROWD_LIMIT: usize = 2;

pub(super) const WALK_NEAR: f64 = 12.0;

pub(super) fn walk_first(flying: bool, feet: BlockPos, cell: BlockPos) -> bool {
    let (dx, dz) = (f64::from(cell.x - feet.x), f64::from(cell.z - feet.z));
    !flying && (dx * dx + dz * dz).sqrt() <= WALK_NEAR && cell.y - feet.y <= SPOT_UP
}

pub(super) fn passes(fly: bool, walk_first: bool) -> Vec<Pass> {
    match (fly, walk_first) {
        (false, _) => vec![Pass::Stand, Pass::Build],
        (true, true) => vec![Pass::Stand, Pass::HoverOut, Pass::HoverIn, Pass::Build],
        (true, false) => vec![Pass::HoverOut, Pass::HoverIn, Pass::Stand, Pass::Build],
    }
}

pub(super) fn hover_passes() -> Vec<Pass> {
    vec![Pass::HoverOut, Pass::HoverIn]
}

pub(super) fn spot_cells(cell: BlockPos, feet: BlockPos, job: Job) -> Vec<BlockPos> {
    let limit = (REACH + HALF_DIAG) * (REACH + HALF_DIAG);
    let mut out = Vec::new();
    for dy in -SPOT_DOWN..=SPOT_UP {
        for dz in -SPOT_RADIUS..=SPOT_RADIUS {
            for dx in -SPOT_RADIUS..=SPOT_RADIUS {
                let p = BlockPos::new(cell.x + dx, cell.y + dy, cell.z + dz);
                if (dx == 0 && dz == 0) && (dy == 0 || dy == -1 || (job == Job::Break && dy > 0)) {
                    continue;
                }
                let eye = [
                    f64::from(p.x) + 0.5,
                    f64::from(p.y) + EYE,
                    f64::from(p.z) + 0.5,
                ];
                let c = [
                    f64::from(cell.x) + 0.5,
                    f64::from(cell.y) + 0.5,
                    f64::from(cell.z) + 0.5,
                ];
                if (0..3).map(|i| (eye[i] - c[i]).powi(2)).sum::<f64>() <= limit {
                    out.push(p);
                }
            }
        }
    }
    out.sort_by_key(|p| (dist2(*p, feet), p.y, p.x, p.z));
    out
}

pub(super) fn hover_point(pending: &[BlockPos]) -> Option<[f64; 3]> {
    if pending.is_empty() {
        return None;
    }
    let n = pending.len() as f64;
    let sum = pending.iter().fold([0.0; 3], |s, p| {
        [
            s[0] + f64::from(p.x),
            s[1] + f64::from(p.y),
            s[2] + f64::from(p.z),
        ]
    });
    Some([sum[0] / n + 0.5, sum[1] / n + 1.0, sum[2] / n + 0.5])
}

pub(super) fn hover_order(cells: &mut [BlockPos], over: [f64; 3], feet: BlockPos) {
    let d = |p: &BlockPos| {
        let f = [f64::from(p.x) + 0.5, f64::from(p.y), f64::from(p.z) + 0.5];
        (0..3).map(|i| (f[i] - over[i]).powi(2)).sum::<f64>()
    };
    cells.sort_by(|a, b| {
        dist2(*a, feet)
            .cmp(&dist2(*b, feet))
            .then(d(a).total_cmp(&d(b)))
    });
}

pub(super) fn rank(floor_cost: f32, inside: bool) -> Option<u8> {
    if !floor_cost.is_finite() {
        None
    } else if floor_cost == 0.0 {
        Some(0)
    } else if inside {
        Some(1)
    } else {
        Some(2)
    }
}

pub(super) enum Spot {
    Found(BlockPos),
    Pending,
    None,
}

pub(super) struct Target {
    pub(super) cell: BlockPos,
    pub(super) job: Job,
    pub(super) want: BlockState,
    pub(super) item: Option<ItemKind>,
    pub(super) air: bool,
}

pub(super) struct SpotSearch {
    pub(super) target: Target,
    passes: Vec<Pass>,
    pass: usize,
    over: [f64; 3],
    list: Vec<BlockPos>,
    next: usize,
    listed: bool,
    vertical: Option<bool>,
    pub(super) flush: Option<([f64; 2], bool)>,
    strict: bool,
    hover: bool,
    holes: std::collections::HashSet<BlockPos>,
    bad: std::collections::HashSet<BlockPos>,
    fallback: Option<Fallback>,
    partials: usize,
}

const SPOT_PARTIALS: usize = 8;

#[derive(Clone, Copy)]
struct Fallback {
    spot: BlockPos,
    pass: usize,
    flush: Option<([f64; 2], bool)>,
    held: usize,
}

impl SpotSearch {
    pub(super) fn new(target: Target, passes: Vec<Pass>, over: [f64; 3]) -> SpotSearch {
        let vertical = match (target.job, target.item) {
            (Job::Place, Some(item)) => crate::modules::place::vertical_need(
                &azalea_inventory::ItemStack::new(item, 1),
                target.want,
            ),
            _ => None,
        };
        let hover = passes.iter().any(|p| p.flying());
        SpotSearch {
            target,
            passes,
            pass: 0,
            over,
            list: Vec::new(),
            next: 0,
            listed: false,
            vertical,
            flush: None,
            strict: true,
            hover,
            holes: std::collections::HashSet::new(),
            bad: std::collections::HashSet::new(),
            fallback: None,
            partials: 0,
        }
    }

    pub(super) fn with_bad(mut self, bad: impl Iterator<Item = BlockPos>) -> SpotSearch {
        self.bad = bad.collect();
        self
    }

    pub(super) fn with_holes(mut self, holes: impl Iterator<Item = BlockPos>) -> SpotSearch {
        self.holes = holes
            .flat_map(|h| [h, BlockPos::new(h.x, h.y + 1, h.z)])
            .collect();
        self
    }

    pub(super) fn cell(&self) -> BlockPos {
        self.target.cell
    }

    fn current(&self) -> Option<Pass> {
        self.passes.get(self.pass).copied()
    }

    pub(super) fn walking(&self) -> bool {
        self.current().is_some_and(|p| !p.flying())
    }

    pub(super) fn standing(&self) -> bool {
        self.current() == Some(Pass::Stand)
    }

    pub(super) fn step(
        &mut self,
        w: &World,
        plan: &Plan,
        stock: &Stock,
        throwaway: bool,
        feet: BlockPos,
    ) -> Spot {
        let start = Instant::now();
        let mut searched = false;
        loop {
            let Some(pass) = self.current() else {
                if let Some(best) = self.fallback.take() {
                    self.pass = best.pass;
                    self.flush = best.flush;
                    return Spot::Found(best.spot);
                }
                if self.strict {
                    self.strict = false;
                    self.pass = 0;
                    self.listed = false;
                    continue;
                }
                return Spot::None;
            };
            if !self.listed {
                let mut list = match pass {
                    Pass::Build => self.buildable(w, plan, stock, throwaway, feet),
                    Pass::Stand => spot_cells(self.target.cell, feet, self.target.job),
                    Pass::HoverOut | Pass::HoverIn => {
                        let mut cells = spot_cells(self.target.cell, feet, self.target.job);
                        hover_order(&mut cells, self.over, feet);
                        if let Some(up) = self.vertical {
                            let y = self.target.cell.y;
                            cells.sort_by_key(|p| if up { p.y >= y } else { p.y <= y });
                        }
                        cells
                    }
                };
                probe_order(w, &self.target, &mut list, pass);
                front_order(w, self.target.cell, &mut list);
                self.list = list;
                self.next = 0;
                self.listed = true;
            }
            let Some(&p) = self.list.get(self.next) else {
                self.pass += 1;
                self.listed = false;
                continue;
            };
            self.next += 1;
            let Some(eye_y) = self.fits(w, plan, p, feet) else {
                continue;
            };
            if searched && start.elapsed() >= SPOT_BUDGET {
                self.next -= 1;
                return Spot::Pending;
            }
            searched = true;
            if let Some((held, of)) = self.reaches(w, p, eye_y, p == feet) {
                if held == of {
                    return Spot::Found(p);
                }
                if self.fallback.is_none_or(|b| held > b.held) {
                    self.fallback = Some(Fallback {
                        spot: p,
                        pass: self.pass,
                        flush: self.flush,
                        held,
                    });
                }
                self.flush = None;
                self.partials += 1;
                if self.partials >= SPOT_PARTIALS
                    && let Some(best) = self.fallback.take()
                {
                    self.pass = best.pass;
                    self.flush = best.flush;
                    return Spot::Found(best.spot);
                }
            }
        }
    }

    fn pending(&self, w: &World, plan: &Plan, c: BlockPos) -> bool {
        !self.holes.contains(&c)
            && plan.want(c).is_some_and(|want| {
                !state_is_air(want) && w.get_block_state(c).is_none_or(|s| s != want)
            })
    }

    fn fits(&self, w: &World, plan: &Plan, p: BlockPos, feet: BlockPos) -> Option<f64> {
        let own_flush = self.vertical.is_some() && self.current() == Some(Pass::Stand);
        if (p == feet && !own_flush) || self.bad.contains(&p) || !room(w, p) {
            return None;
        }
        let head = BlockPos::new(p.x, p.y + 1, p.z);
        if self.strict && (self.pending(w, plan, p) || self.pending(w, plan, head)) {
            return None;
        }
        match self.current()? {
            Pass::Stand => {
                if self.hover && p.y > feet.y + 1 {
                    return None;
                }
                let floor = BlockPos::new(p.x, p.y - 1, p.z);
                let state = w
                    .get_block_state(floor)
                    .filter(|s| is_block_state_standable(*s))?;
                Some(floor_top(state, floor) + EYE)
            }
            Pass::Build => Some(f64::from(p.y) + EYE),
            pass @ (Pass::HoverOut | Pass::HoverIn) => {
                let inside = pass == Pass::HoverIn;
                if !inside && crowd(w, p) > CROWD_LIMIT {
                    return None;
                }
                let below = BlockPos::new(p.x, p.y - 1, p.z);
                if !passable(w, below) {
                    return None;
                }
                let head = BlockPos::new(p.x, p.y + 1, p.z);
                let allowed = |c: BlockPos| match plan.want(c) {
                    None => true,
                    Some(want) => inside && state_is_air(want),
                };
                (allowed(p) && allowed(head)).then_some(f64::from(p.y) + HOVER_LIFT + EYE)
            }
        }
    }

    fn buildable(
        &self,
        w: &World,
        plan: &Plan,
        stock: &Stock,
        throwaway: bool,
        feet: BlockPos,
    ) -> Vec<BlockPos> {
        let mut ranked: Vec<(u8, i64, BlockPos)> =
            spot_cells(self.target.cell, feet, self.target.job)
                .into_iter()
                .filter_map(|p| {
                    let floor = BlockPos::new(p.x, p.y - 1, p.z);
                    if floor == self.target.cell || !room(w, p) {
                        return None;
                    }
                    if w.get_block_state(floor).is_none_or(holds) {
                        return None;
                    }
                    let r = rank(
                        rules::place_cost(plan.want(floor), stock, throwaway),
                        plan.want(p).is_some(),
                    )?;
                    Some((r, dist2(p, feet), p))
                })
                .collect();
        ranked.sort_by_key(|&(r, d, p)| (r, d, p.y, p.x, p.z));
        ranked.into_iter().map(|(_, _, p)| p).collect()
    }

    fn reaches(&mut self, w: &World, p: BlockPos, eye_y: f64, own: bool) -> Option<(usize, usize)> {
        self.flush = None;
        let centre = [f64::from(p.x) + 0.5, eye_y, f64::from(p.z) + 0.5];
        let near = (p.x - self.target.cell.x)
            .abs()
            .max((p.z - self.target.cell.z).abs())
            <= FLUSH_NEAR;
        if self.vertical.is_some()
            && self.current() == Some(Pass::Stand)
            && near
            && (own || first_click(w, &self.target, centre).is_none())
        {
            let (cx, cz) = (f64::from(p.x) + 0.5, f64::from(p.z) + 0.5);
            let shifts = |off: f64| -> Vec<f64> {
                if off > 0.01 {
                    vec![-FLUSH_JITTER]
                } else if off < -0.01 {
                    vec![FLUSH_JITTER]
                } else {
                    vec![FLUSH_JITTER, -FLUSH_JITTER]
                }
            };
            let holds_near = |eye: [f64; 3]| {
                let [x, y, z] = eye;
                let mut eyes = vec![eye];
                eyes.extend(shifts(x - cx).into_iter().map(|s| [x + s, y, z]));
                eyes.extend(shifts(z - cz).into_iter().map(|s| [x, y, z + s]));
                eyes.iter()
                    .all(|e| first_click(w, &self.target, *e).is_some())
            };
            let eyes = flush_eyes(p, eye_y);
            let robust = eyes.iter().find(|(eye, _, _)| holds_near(*eye)).copied();
            let found = robust.or_else(|| {
                eyes.iter()
                    .find(|(eye, _, _)| first_click(w, &self.target, *eye).is_some())
                    .copied()
            });
            if let Some((eye, feet, sneak)) = found {
                let _ = eye;
                self.flush = Some((feet, sneak));
                return Some((usize::from(robust.is_some()), 1));
            }
            return None;
        }
        if own {
            return None;
        }
        let t = &self.target;
        let click = first_click(w, t, centre)?;
        let (slack, vertical) = if self.walking() {
            (WALK_SLACK, false)
        } else {
            (FLY_SLACK, true)
        };
        let offsets = eye_offsets(centre, slack, vertical);
        let _ = click;
        let held = offsets
            .iter()
            .filter(|eye| reaches_from(w, t, **eye))
            .count();
        Some((held, offsets.len()))
    }
}

fn front_order(w: &World, cell: BlockPos, list: &mut [BlockPos]) {
    let dirs = [
        (1, 0, 0),
        (-1, 0, 0),
        (0, 0, 1),
        (0, 0, -1),
        (0, 1, 0),
        (0, -1, 0),
    ];
    let open: Vec<(i32, i32, i32)> = dirs
        .iter()
        .copied()
        .filter(|(x, y, z)| {
            !w.get_block_state(BlockPos::new(cell.x + x, cell.y + y, cell.z + z))
                .is_some_and(holds)
        })
        .collect();
    if open.is_empty() || open.len() > 2 {
        return;
    }
    let front = |p: &BlockPos| {
        let d = [p.x - cell.x, p.y - cell.y, p.z - cell.z];
        open.iter().any(|&(x, y, z)| {
            let n = [x, y, z];
            let along = d[0] * n[0] + d[1] * n[1] + d[2] * n[2];
            if !(1..=3).contains(&along) {
                return false;
            }
            if y == 0 {
                let side = if x != 0 { d[2] } else { d[0] };
                side == 0 && (-2..=1).contains(&d[1])
            } else {
                d[0] == 0 && d[2] == 0
            }
        })
    };
    list.sort_by_key(|p| !front(p));
}

const FLUSH_NEAR: i32 = 2;

const FLUSH_JITTER: f64 = 0.06;

pub(super) const SNEAK_EYE: f64 = 1.27;

const FLUSH_OFF: f64 = 0.19;

pub(super) fn flush_eyes(p: BlockPos, eye_y: f64) -> Vec<([f64; 3], [f64; 2], bool)> {
    let floor = eye_y - EYE;
    let (cx, cz) = (f64::from(p.x) + 0.5, f64::from(p.z) + 0.5);
    let offsets = [
        (1.0, 0.0),
        (-1.0, 0.0),
        (0.0, 1.0),
        (0.0, -1.0),
        (1.0, 1.0),
        (1.0, -1.0),
        (-1.0, 1.0),
        (-1.0, -1.0),
    ];
    let mut out = Vec::with_capacity(16);
    for sneak in [true, false] {
        let height = if sneak { SNEAK_EYE } else { EYE };
        for (dx, dz) in offsets {
            let feet = [cx + dx * FLUSH_OFF, cz + dz * FLUSH_OFF];
            out.push(([feet[0], floor + height, feet[1]], feet, sneak));
        }
    }
    out
}

fn probe_order(w: &World, t: &Target, list: &mut [BlockPos], pass: Pass) {
    use azalea_core::direction::Direction;
    use azalea_core::hit_result::BlockHitResult;
    use azalea_core::position::Vec3;
    use azalea_inventory::ItemStack;

    let (Job::Place, Some(item), false) = (t.job, t.item, pass == Pass::Build) else {
        return;
    };
    let held = ItemStack::new(item, 1);
    let (min_y, height) = (w.chunks.min_y(), w.chunks.height());
    let dirs = [
        Direction::East,
        Direction::West,
        Direction::Up,
        Direction::Down,
        Direction::South,
        Direction::North,
    ];
    let faces: Vec<BlockHitResult> = dirs
        .iter()
        .filter_map(|&d| {
            let n = t.cell + d.normal();
            if !w.get_block_state(n).is_some_and(holds) {
                return None;
            }
            let v = d.normal();
            let point = [
                f64::from(t.cell.x) + 0.5 + f64::from(v.x) * 0.5,
                f64::from(t.cell.y) + 0.5 + f64::from(v.y) * 0.5,
                f64::from(t.cell.z) + 0.5 + f64::from(v.z) * 0.5,
            ];
            let face = d.opposite();
            Some(BlockHitResult {
                location: Vec3::new(point[0], point[1], point[2]),
                miss: false,
                direction: face,
                block_pos: n,
                inside: false,
                world_border: false,
            })
        })
        .collect();
    if faces.is_empty() {
        return;
    }
    let eye_y = |p: &BlockPos| match pass {
        Pass::HoverOut | Pass::HoverIn => f64::from(p.y) + HOVER_LIFT + EYE,
        Pass::Stand | Pass::Build => f64::from(p.y) + EYE,
    };
    let gives = |p: &BlockPos| {
        let eye = [f64::from(p.x) + 0.5, eye_y(p), f64::from(p.z) + 0.5];
        faces.iter().any(|hit| {
            let look = crate::modules::sent::look_at(
                eye,
                [hit.location.x, hit.location.y, hit.location.z],
            );
            crate::play::placement::predict_placed(
                &held,
                hit,
                look.yaw,
                look.pitch,
                false,
                min_y,
                height,
                |at| w.get_block_state(at),
            )
            .is_some_and(|placed| {
                placed.pos == t.cell && crate::modules::place::placement_fits(placed.state, t.want)
            })
        })
    };
    list.sort_by_cached_key(|p| !gives(p));
}

pub(super) const WALK_SLACK: f64 = 0.3;

pub(super) const FLY_SLACK: f64 = 0.4;

pub(super) fn eye_offsets(centre: [f64; 3], slack: f64, vertical: bool) -> Vec<[f64; 3]> {
    let [x, y, z] = centre;
    let mut out = vec![
        [x + slack, y, z],
        [x - slack, y, z],
        [x, y, z + slack],
        [x, y, z - slack],
    ];
    if vertical {
        out.push([x, y + slack, z]);
        out.push([x, y - slack, z]);
    }
    out
}

fn first_click(w: &World, t: &Target, eye: [f64; 3]) -> Option<Option<Click>> {
    match t.job {
        Job::Place | Job::Pour | Job::Drain => match super::clicks_at(w, t, eye, true) {
            super::Reach::Clicks(clicks, _) => clicks.into_iter().next().map(Some),
            _ => None,
        },
        Job::Break | Job::Use => {
            let state = w.get_block_state(t.cell)?;
            aim_at(&w.chunks, eye, t.cell, state).map(|_| None)
        }
    }
}

pub(super) fn reaches_from(w: &World, t: &Target, eye: [f64; 3]) -> bool {
    first_click(w, t, eye).is_some()
}

fn floor_top(state: BlockState, floor: BlockPos) -> f64 {
    let shape = state.collision_shape(floor);
    let top = if shape.is_empty() {
        1.0
    } else {
        shape.bounds().max.y.min(1.0)
    };
    f64::from(floor.y) + top
}

fn crowd(w: &World, p: BlockPos) -> usize {
    let mut cells = Vec::with_capacity(9);
    for dy in [0, 1] {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            cells.push(BlockPos::new(p.x + dx, p.y + dy, p.z + dz));
        }
    }
    cells.push(BlockPos::new(p.x, p.y + 2, p.z));
    cells.into_iter().filter(|c| !passable(w, *c)).count()
}

fn passable(w: &World, p: BlockPos) -> bool {
    w.get_block_state(p).is_some_and(is_block_state_passable)
}

fn room(w: &World, p: BlockPos) -> bool {
    passable(w, p) && passable(w, BlockPos::new(p.x, p.y + 1, p.z))
}

#[cfg(test)]
mod tests {
    use azalea_registry::builtin::BlockKind;

    use super::*;

    #[test]
    fn stand_cells_are_within_reach_and_nearest_the_player_first() {
        let cell = BlockPos::new(10, 64, 10);
        let feet = BlockPos::new(10, 64, 6);
        let list = spot_cells(cell, feet, Job::Place);
        assert!(!list.contains(&cell) && !list.contains(&BlockPos::new(10, 63, 10)));
        assert!(list.contains(&BlockPos::new(10, 65, 10)));
        assert!(!spot_cells(cell, feet, Job::Break).contains(&BlockPos::new(10, 65, 10)));
        assert!(!list.contains(&BlockPos::new(14, 64, 14)));
        assert!(!list.contains(&BlockPos::new(10, 68, 11)));
        assert!(!list.contains(&BlockPos::new(10, 58, 11)));
        assert_eq!(list[0], BlockPos::new(10, 64, 6));
        let d: Vec<i64> = list.iter().map(|p| dist2(*p, feet)).collect();
        assert!(d.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn walking_or_flying() {
        let feet = BlockPos::new(0, 100, 0);
        assert!(walk_first(false, feet, BlockPos::new(5, 100, 5)));
        assert!(walk_first(false, feet, BlockPos::new(0, 103, 8)));
        assert!(!walk_first(false, feet, BlockPos::new(0, 104, 2)));
        assert!(!walk_first(false, feet, BlockPos::new(13, 100, 0)));
        assert!(!walk_first(true, feet, BlockPos::new(1, 100, 0)));
        assert_eq!(passes(false, true), vec![Pass::Stand, Pass::Build]);
        assert_eq!(passes(false, false), vec![Pass::Stand, Pass::Build]);
        assert_eq!(
            passes(true, true),
            vec![Pass::Stand, Pass::HoverOut, Pass::HoverIn, Pass::Build]
        );
        assert_eq!(
            passes(true, false),
            vec![Pass::HoverOut, Pass::HoverIn, Pass::Stand, Pass::Build]
        );
    }

    #[test]
    fn built_floors_prefer_the_build_s_own_blocks() {
        assert_eq!(rank(0.0, false), Some(0));
        assert_eq!(rank(40.0, true), Some(1));
        assert_eq!(rank(20.0, false), Some(2));
        assert_eq!(rank(f32::INFINITY, true), None);
    }

    #[test]
    fn a_slab_floor_lowers_the_eye() {
        let floor = BlockPos::new(0, 103, 0);
        assert_eq!(
            floor_top(BlockState::from(BlockKind::OakSlab), floor),
            103.5
        );
        assert_eq!(floor_top(BlockState::from(BlockKind::Stone), floor), 104.0);
        assert_eq!(
            floor_top(BlockState::from(BlockKind::OakStairs), floor),
            104.0
        );
    }

    #[test]
    fn a_spot_is_tested_from_where_the_player_may_really_stop() {
        let c = [10.5, 65.62, 4.5];
        let near = |a: [f64; 3], b: [f64; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9);
        let walk = eye_offsets(c, WALK_SLACK, false);
        assert_eq!(walk.len(), 4);
        assert!(
            walk.iter().any(|e| near(*e, [10.8, 65.62, 4.5]))
                && walk.iter().any(|e| near(*e, [10.5, 65.62, 4.2]))
        );
        assert!(
            walk.iter()
                .all(|e| e[1] == c[1] && (e[0] - 10.0).abs() < 1.0 && (e[2] - 4.0).abs() < 1.0)
        );
        let fly = eye_offsets(c, FLY_SLACK, true);
        assert_eq!(fly.len(), 6);
        assert!(
            fly.iter().any(|e| near(*e, [10.5, 66.02, 4.5]))
                && fly.iter().any(|e| near(*e, [10.5, 65.22, 4.5]))
        );
        assert!(
            fly.iter()
                .all(|e| e[0] > 10.0 && e[0] < 11.0 && e[2] > 4.0 && e[2] < 5.0)
        );
    }

    #[test]
    fn hover_spots_are_ranked_from_over_the_pending_cells() {
        let pending = [
            BlockPos::new(0, 100, 0),
            BlockPos::new(2, 100, 0),
            BlockPos::new(0, 100, 2),
            BlockPos::new(2, 100, 2),
        ];
        let over = hover_point(&pending).unwrap();
        assert_eq!(over, [1.5, 101.0, 1.5]);
        assert_eq!(hover_point(&[]), None);
        let feet = BlockPos::new(10, 100, 10);
        let mut cells = vec![
            BlockPos::new(1, 104, 1),
            BlockPos::new(1, 101, 1),
            BlockPos::new(5, 101, 5),
        ];
        hover_order(&mut cells, over, feet);
        assert_eq!(
            cells,
            vec![
                BlockPos::new(5, 101, 5),
                BlockPos::new(1, 101, 1),
                BlockPos::new(1, 104, 1)
            ]
        );
        let mut tie = vec![BlockPos::new(1, 100, 3), BlockPos::new(3, 100, 1)];
        hover_order(&mut tie, [3.5, 100.0, 1.5], BlockPos::new(2, 100, 2));
        assert_eq!(tie[0], BlockPos::new(3, 100, 1));
    }
}
