use std::collections::HashSet;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};

use azalea::block::{BlockState, BlockTrait};
use azalea_core::position::BlockPos;
use azalea_registry::builtin::{BlockKind, ItemKind};

use super::BreakMode;
use crate::blocks::state_is_air;

pub trait Cells: Send + Sync {
    fn get(&self, index: usize) -> u32;
    fn each(&self, range: Range<usize>, f: &mut dyn FnMut(u32));
}

pub struct Region {
    pub origin: [i32; 3],
    pub size: [i32; 3],
    pub palette: Vec<u16>,
    pub cells: Arc<dyn Cells>,
}

pub(super) const MAX_CELLS: u64 = 1 << 27;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Leave,
    Keep,
    Want(BlockState),
}

struct Area {
    origin: [i32; 3],
    size: [i32; 3],
    slots: Vec<Slot>,
    cells: Arc<dyn Cells>,
    correct: Box<[AtomicU64]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Spot {
    area: usize,
    index: usize,
}

pub(super) struct Plan {
    areas: Vec<Area>,
    total: u64,
    lo: [i32; 3],
    hi: [i32; 3],
    fluids: AtomicBool,
}

impl std::fmt::Debug for Plan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plan")
            .field("regions", &self.areas.len())
            .field("total", &self.total)
            .field("lo", &self.lo)
            .field("hi", &self.hi)
            .finish()
    }
}

impl Plan {
    pub(super) fn new(regions: Vec<Region>) -> Result<Plan, String> {
        if regions.is_empty() {
            return Err("no regions".to_string());
        }
        let mut total = 0u64;
        let mut lo = [i32::MAX; 3];
        let mut hi = [i32::MIN; 3];
        for r in &regions {
            let [x, y, z] = r.size;
            if x <= 0 || y <= 0 || z <= 0 {
                return Err(format!("a region has size {x}x{y}x{z}"));
            }
            for i in 0..3 {
                let end = i64::from(r.origin[i]) + i64::from(r.size[i]);
                if end > i64::from(i32::MAX) {
                    return Err(
                        "a region reaches past the end of the world's coordinates".to_string()
                    );
                }
                lo[i] = lo[i].min(r.origin[i]);
                hi[i] = hi[i].max(end as i32);
            }
            total += x as u64 * y as u64 * z as u64;
            if total > MAX_CELLS {
                return Err(format!("the schematic has more than {MAX_CELLS} blocks"));
            }
        }
        let areas = regions.into_iter().map(Area::new).collect();
        Ok(Plan {
            areas,
            total,
            lo,
            hi,
            fluids: AtomicBool::new(true),
        })
    }

    pub(super) fn fluids(&self) -> bool {
        self.fluids.load(Relaxed)
    }

    pub(super) fn set_fluids(&self, on: bool) -> bool {
        self.fluids.swap(on, Relaxed) != on
    }

    pub(super) fn total(&self) -> u64 {
        self.total
    }

    pub(super) fn locate(&self, pos: BlockPos) -> Option<(Spot, BlockState)> {
        let p = [pos.x, pos.y, pos.z];
        if (0..3).any(|i| p[i] < self.lo[i] || p[i] >= self.hi[i]) {
            return None;
        }
        let fluids = self.fluids();
        self.areas.iter().enumerate().find_map(|(a, area)| {
            let index = area.index(p)?;
            match area.slot(index, fluids) {
                Slot::Want(state) => Some((Spot { area: a, index }, state)),
                Slot::Leave | Slot::Keep => None,
            }
        })
    }

    pub(super) fn kept(&self, pos: BlockPos) -> bool {
        let p = [pos.x, pos.y, pos.z];
        if (0..3).any(|i| p[i] < self.lo[i] || p[i] >= self.hi[i]) {
            return false;
        }
        let fluids = self.fluids();
        let mut keep = false;
        for area in &self.areas {
            let Some(index) = area.index(p) else {
                continue;
            };
            match area.slot(index, fluids) {
                Slot::Want(_) => return false,
                Slot::Keep => keep = true,
                Slot::Leave => {}
            }
        }
        keep
    }

    pub(super) fn want(&self, pos: BlockPos) -> Option<BlockState> {
        self.locate(pos).map(|(_, state)| state)
    }

    pub(super) fn mark(&self, spot: Spot, right: bool) {
        let word = &self.areas[spot.area].correct[spot.index / 64];
        let bit = 1u64 << (spot.index % 64);
        if right {
            word.fetch_or(bit, Relaxed);
        } else {
            word.fetch_and(!bit, Relaxed);
        }
    }

    pub(super) fn is_correct(&self, spot: Spot) -> bool {
        let word = self.areas[spot.area].correct[spot.index / 64].load(Relaxed);
        word >> (spot.index % 64) & 1 != 0
    }
}

impl Area {
    fn new(r: Region) -> Area {
        let volume = r.size.iter().map(|&n| n as usize).product::<usize>();
        Area {
            origin: r.origin,
            size: r.size,
            slots: r.palette.iter().map(|&id| slot_for(id)).collect(),
            cells: r.cells,
            correct: (0..volume.div_ceil(64))
                .map(|_| AtomicU64::new(0))
                .collect(),
        }
    }

    fn volume(&self) -> usize {
        self.size.iter().map(|&n| n as usize).product()
    }

    fn index(&self, p: [i32; 3]) -> Option<usize> {
        let l = [
            p[0] - self.origin[0],
            p[1] - self.origin[1],
            p[2] - self.origin[2],
        ];
        if (0..3).any(|i| l[i] < 0 || l[i] >= self.size[i]) {
            return None;
        }
        let [sx, _, sz] = self.size.map(|n| n as usize);
        Some(l[0] as usize + sx * (l[2] as usize + sz * l[1] as usize))
    }

    fn local(&self, index: usize) -> [i32; 3] {
        let [sx, _, sz] = self.size.map(|n| n as usize);
        [
            (index % sx) as i32,
            (index / (sx * sz)) as i32,
            (index / sx % sz) as i32,
        ]
    }

    fn pos(&self, l: [i32; 3]) -> BlockPos {
        BlockPos::new(
            self.origin[0] + l[0],
            self.origin[1] + l[1],
            self.origin[2] + l[2],
        )
    }

    fn slot(&self, index: usize, fluids: bool) -> Slot {
        self.palette_slot(self.cells.get(index), fluids)
    }

    fn palette_slot(&self, p: u32, fluids: bool) -> Slot {
        let slot = self
            .slots
            .get(p as usize)
            .copied()
            .unwrap_or(Slot::Want(BlockState::AIR));
        match slot {
            Slot::Want(state) if !fluids && source(state) => Slot::Leave,
            Slot::Want(state) if !fluids => Slot::Want(dry(state)),
            other => other,
        }
    }
}

fn slot_for(id: u16) -> Slot {
    let Ok(state) = BlockState::try_from(id) else {
        return Slot::Leave;
    };
    if state_is_air(state) {
        return Slot::Want(BlockState::AIR);
    }
    match state.as_block_kind() {
        BlockKind::Water | BlockKind::Lava if source(state) => Slot::Want(state),
        BlockKind::StructureVoid => Slot::Keep,
        BlockKind::Water | BlockKind::Lava | BlockKind::BubbleColumn => Slot::Leave,
        _ => Slot::Want(state),
    }
}

fn dry(state: BlockState) -> BlockState {
    crate::modules::place::dry(state)
}

pub(super) fn pure_fluid(state: BlockState) -> bool {
    matches!(
        state.as_block_kind(),
        BlockKind::Water | BlockKind::Lava | BlockKind::BubbleColumn
    )
}

pub(super) fn source(state: BlockState) -> bool {
    matches!(state.as_block_kind(), BlockKind::Water | BlockKind::Lava)
        && Box::<dyn BlockTrait>::from(state).get_property("level") == Some("0")
}

pub(super) fn water_source(state: BlockState) -> bool {
    (state.as_block_kind() == BlockKind::Water && source(state))
        || Box::<dyn BlockTrait>::from(state).get_property("waterlogged") == Some("true")
}

fn needs_water(world: BlockState, want: BlockState) -> bool {
    let wet =
        |s: BlockState| Box::<dyn BlockTrait>::from(s).get_property("waterlogged") == Some("true");
    wet(want) && !wet(world) && crate::modules::place::same_placement(world, dry(want))
}

pub(super) fn refills(p: BlockPos, at: &impl Fn(BlockPos) -> Option<BlockState>) -> bool {
    use azalea::physics::collision::BlockWithShape;
    let Some(here) = at(p) else {
        return false;
    };
    if here.as_block_kind() != BlockKind::Water || !source(here) {
        return false;
    }
    let sides = [(1, 0), (-1, 0), (0, 1), (0, -1)]
        .iter()
        .filter(|(dx, dz)| at(BlockPos::new(p.x + dx, p.y, p.z + dz)).is_some_and(water_source))
        .count();
    if sides < 2 {
        return false;
    }
    at(BlockPos::new(p.x, p.y - 1, p.z)).is_some_and(|below| {
        water_source(below) || (holds(below) && !below.is_collision_shape_empty())
    })
}

pub(super) fn holds(state: BlockState) -> bool {
    !state_is_air(state)
        && !pure_fluid(state)
        && crate::play::placement::replaceable(state, BlockKind::Air) != Some(true)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Verdict {
    Right,
    Fluid,
    Wrong,
}

pub(super) fn judge(world: BlockState, want: BlockState, mode: BreakMode, fluids: bool) -> Verdict {
    if source(want) {
        return if world == want {
            Verdict::Right
        } else {
            Verdict::Wrong
        };
    }
    if !state_is_air(want) {
        return if crate::modules::place::same_placement(world, want) {
            Verdict::Right
        } else {
            Verdict::Wrong
        };
    }
    if state_is_air(world) {
        Verdict::Right
    } else if pure_fluid(world) {
        if fluids && mode == BreakMode::All && source(world) {
            Verdict::Wrong
        } else {
            Verdict::Fluid
        }
    } else if mode == BreakMode::All {
        Verdict::Wrong
    } else {
        Verdict::Right
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Job {
    Break,
    Place,
    Use,
    Pour,
    Drain,
}

pub(super) fn job(world: BlockState, want: BlockState, fluids: bool) -> Job {
    if state_is_air(want) {
        return if fluids && source(world) {
            Job::Drain
        } else {
            Job::Break
        };
    }
    let takes = state_is_air(world)
        || pure_fluid(world)
        || crate::play::placement::replaceable(world, want.as_block_kind()) == Some(true);
    if source(want) {
        return if takes { Job::Pour } else { Job::Break };
    }
    if crate::modules::place::use_clicks(world, want).is_some() {
        return Job::Use;
    }
    if fluids && needs_water(world, want) {
        return Job::Pour;
    }
    if takes { Job::Place } else { Job::Break }
}

pub(super) fn job_item(job: Job, want: BlockState) -> Option<ItemKind> {
    match job {
        Job::Place => crate::modules::place::stock::item_for(want),
        Job::Pour if want.as_block_kind() == BlockKind::Lava => Some(ItemKind::LavaBucket),
        Job::Pour => Some(ItemKind::WaterBucket),
        Job::Drain => Some(ItemKind::Bucket),
        Job::Break | Job::Use => None,
    }
}

pub(super) fn contained(
    plan: &Plan,
    cell: BlockPos,
    fluids: bool,
    at: &impl Fn(BlockPos) -> Option<BlockState>,
    given_up: &impl Fn(BlockPos) -> bool,
) -> (bool, bool) {
    let mut walls_done = true;
    let mut exposed = false;
    for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (0, -1, 0)] {
        let n = BlockPos::new(cell.x + dx, cell.y + dy, cell.z + dz);
        let world = at(n);
        match plan.want(n).filter(|_| !given_up(n)) {
            Some(want) if !state_is_air(want) && !source(want) => {
                if !world.is_some_and(|w| {
                    judge(w, want, BreakMode::OnlyWhereNeeded, fluids) != Verdict::Wrong
                }) {
                    walls_done = false;
                }
            }
            Some(_) => {}
            None => exposed |= !world.is_some_and(holds),
        }
    }
    (walls_done, exposed)
}

pub(super) fn partner_half(state: BlockState) -> bool {
    let block = Box::<dyn BlockTrait>::from(state);
    (block.get_property("half") == Some("upper") && block.get_property("hinge").is_some())
        || (block.get_property("part") == Some("head") && block.get_property("occupied").is_some())
}

pub(super) fn partner_base(cell: BlockPos, state: BlockState) -> BlockPos {
    let block = Box::<dyn BlockTrait>::from(state);
    if block.get_property("hinge").is_some() {
        return BlockPos::new(cell.x, cell.y - 1, cell.z);
    }
    let (dx, dz) = match block.get_property("facing") {
        Some("north") => (0, 1),
        Some("south") => (0, -1),
        Some("west") => (1, 0),
        _ => (-1, 0),
    };
    BlockPos::new(cell.x + dx, cell.y, cell.z + dz)
}

pub(super) fn phase(want: BlockState, job: Job) -> u8 {
    match job {
        Job::Use => return 2,
        Job::Pour => return 3,
        Job::Drain => return 0,
        Job::Place | Job::Break => {}
    }
    let kind = want.as_block_kind();
    let id = kind.to_str().trim_start_matches("minecraft:");
    match id {
        "redstone_torch"
        | "redstone_wall_torch"
        | "redstone_block"
        | "observer"
        | "piston"
        | "sticky_piston"
        | "dispenser"
        | "dropper"
        | "lever" => 2,
        "redstone_wire" | "repeater" | "comparator" | "redstone_lamp" => 1,
        _ if id == "rail" || id.ends_with("_rail") => 1,
        _ => 0,
    }
}

pub(super) const EXPOSED_PHASE: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum HoleKind {
    Open,
    Reserve,
    Dig,
}

pub(super) fn hole_forbidden(state: BlockState) -> bool {
    use azalea::physics::collision::BlockWithShape;
    let block = Box::<dyn BlockTrait>::from(state);
    phase(state, Job::Place) > 0
        || ["facing", "rotation", "axis", "half", "type", "face"]
            .iter()
            .any(|p| block.get_property(p).is_some())
        || state.is_collision_shape_empty()
        || pure_fluid(state)
        || block.get_property("waterlogged") == Some("true")
}

pub(super) fn choose_hole(
    target: BlockPos,
    want: &impl Fn(BlockPos) -> Option<BlockState>,
    world: &impl Fn(BlockPos) -> Option<BlockState>,
    may_dig: &impl Fn(BlockPos, BlockState) -> bool,
) -> Option<(BlockPos, HoleKind)> {
    use azalea::pathfinder::world::{is_block_state_passable, is_block_state_standable};
    use azalea::physics::collision::BlockWithShape;

    let mut best: Option<((HoleKind, bool, usize), BlockPos)> = None;
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let head = BlockPos::new(target.x + dx, target.y, target.z + dz);
        let hole = BlockPos::new(head.x, head.y - 1, head.z);
        let below = BlockPos::new(head.x, head.y - 2, head.z);
        if !world(head).is_some_and(is_block_state_passable)
            || want(head).is_some_and(|w| !state_is_air(w))
        {
            continue;
        }
        let ground_now = world(below).is_some_and(is_block_state_standable);
        if !ground_now && !want(below).is_some_and(|w| w.is_collision_shape_full()) {
            continue;
        }
        let Some(here) = world(hole) else {
            continue;
        };
        let kind = match want(hole) {
            Some(w) if !state_is_air(w) => {
                if hole_forbidden(w) {
                    continue;
                }
                if holds(here) {
                    if !may_dig(hole, here) {
                        continue;
                    }
                    HoleKind::Dig
                } else {
                    HoleKind::Reserve
                }
            }
            _ if !holds(here) => HoleKind::Open,
            _ => {
                if !may_dig(hole, here) {
                    continue;
                }
                HoleKind::Dig
            }
        };
        let dependents = [
            (1, 0, 0),
            (-1, 0, 0),
            (0, 1, 0),
            (0, -1, 0),
            (0, 0, 1),
            (0, 0, -1),
        ]
        .iter()
        .filter(|(x, y, z)| {
            want(BlockPos::new(hole.x + x, hole.y + y, hole.z + z))
                .is_some_and(|w| !state_is_air(w))
        })
        .count();
        let key = (kind, !ground_now, dependents);
        if best.is_none_or(|(k, _)| key < k) {
            best = Some((key, hole));
        }
    }
    best.map(|((kind, _, _), hole)| (hole, kind))
}

pub(super) fn dist2(a: BlockPos, b: BlockPos) -> i64 {
    let d = [
        i64::from(a.x - b.x),
        i64::from(a.y - b.y),
        i64::from(a.z - b.z),
    ];
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
}

pub(super) struct Wrong {
    cells: HashSet<BlockPos>,
    skipped: HashSet<BlockPos>,
    cap: usize,
    far: Option<(BlockPos, BlockPos, i64)>,
}

impl Wrong {
    pub(super) fn new(cap: usize) -> Wrong {
        Wrong {
            cells: HashSet::new(),
            skipped: HashSet::new(),
            cap,
            far: None,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.cells.len()
    }

    pub(super) fn skipped(&self) -> usize {
        self.skipped.len()
    }

    pub(super) fn contains(&self, pos: BlockPos) -> bool {
        self.cells.contains(&pos)
    }

    pub(super) fn is_skipped(&self, pos: BlockPos) -> bool {
        self.skipped.contains(&pos)
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = BlockPos> + '_ {
        self.cells.iter().copied()
    }

    pub(super) fn skipped_iter(&self) -> impl Iterator<Item = BlockPos> + '_ {
        self.skipped.iter().copied()
    }

    pub(super) fn clear(&mut self) {
        self.cells.clear();
        self.far = None;
    }

    pub(super) fn remove(&mut self, pos: BlockPos) {
        if self.cells.remove(&pos) && self.far.is_some_and(|f| f.1 == pos) {
            self.far = None;
        }
    }

    pub(super) fn take_skipped(&mut self, pick: impl Fn(BlockPos) -> bool) -> Vec<BlockPos> {
        let back: Vec<BlockPos> = self.skipped.iter().copied().filter(|p| pick(*p)).collect();
        for p in &back {
            self.skipped.remove(p);
        }
        back
    }

    pub(super) fn skip(&mut self, pos: BlockPos) {
        self.remove(pos);
        self.skipped.insert(pos);
    }

    pub(super) fn insert(&mut self, pos: BlockPos, near: BlockPos) -> bool {
        if self.skipped.contains(&pos) || self.cells.contains(&pos) {
            return true;
        }
        if self.cells.len() < self.cap {
            self.cells.insert(pos);
            self.far = None;
            return true;
        }
        let (far, d) = match self.far {
            Some((n, far, d)) if n == near => (far, d),
            _ => {
                let Some(far) = self.cells.iter().copied().max_by_key(|p| dist2(*p, near)) else {
                    return false;
                };
                (far, dist2(far, near))
            }
        };
        if dist2(pos, near) >= d {
            self.far = Some((near, far, d));
            return false;
        }
        self.cells.remove(&far);
        self.cells.insert(pos);
        self.far = None;
        false
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn note(
    plan: &Plan,
    wrong: &mut Wrong,
    spot: Spot,
    pos: BlockPos,
    want: BlockState,
    world: BlockState,
    mode: BreakMode,
    near: BlockPos,
) -> (Verdict, bool) {
    let verdict = judge(world, want, mode, plan.fluids());
    plan.mark(spot, verdict != Verdict::Wrong);
    if verdict == Verdict::Wrong {
        let kept = wrong.insert(pos, near);
        (verdict, kept)
    } else {
        wrong.remove(pos);
        (verdict, true)
    }
}

pub(super) fn recheck(
    plan: &Plan,
    wrong: &mut Wrong,
    pos: BlockPos,
    mode: BreakMode,
    near: BlockPos,
    state_at: &impl Fn(BlockPos) -> Option<BlockState>,
) -> bool {
    let Some((spot, want)) = plan.locate(pos) else {
        return true;
    };
    match state_at(pos) {
        Some(world) => note(plan, wrong, spot, pos, want, world, mode, near).1,
        None => true,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Pass {
    pub unseen: u32,
    pub nearest_unseen: Option<BlockPos>,
    pub fluids: u32,
    pub overflowed: bool,
}

pub(super) struct Scan {
    area: usize,
    index: usize,
    generation: u32,
    complete: bool,
    now: Pass,
    pub last: Option<Pass>,
}

impl Scan {
    pub(super) fn new(generation: u32) -> Scan {
        Scan {
            area: 0,
            index: 0,
            generation,
            complete: false,
            now: Pass::default(),
            last: None,
        }
    }

    pub(super) fn complete(&self) -> bool {
        self.complete
    }

    pub(super) fn clean(&self) -> bool {
        self.complete && self.last.is_some_and(|p| p.unseen == 0 && !p.overflowed)
    }

    pub(super) fn restart_if(
        &mut self,
        generation: u32,
        wrong_len: usize,
        cap: usize,
        force: bool,
    ) {
        let more = self.complete
            && (generation != self.generation
                || (self.last.is_some_and(|p| p.overflowed) && wrong_len < cap / 2));
        if force || more {
            *self = Scan {
                last: self.last,
                ..Scan::new(generation)
            };
        }
    }

    pub(super) fn overflowed(&mut self) {
        self.now.overflowed = true;
        if let Some(last) = self.last.as_mut() {
            last.overflowed = true;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn batch(
        &mut self,
        plan: &Plan,
        wrong: &mut Wrong,
        near: BlockPos,
        mode: BreakMode,
        n: usize,
        state_at: &impl Fn(BlockPos) -> Option<BlockState>,
    ) {
        let mut left = n;
        while left > 0 && !self.complete {
            let Some(area) = plan.areas.get(self.area) else {
                self.complete = true;
                self.last = Some(self.now);
                break;
            };
            let volume = area.volume();
            let end = (self.index + left).min(volume);
            let shared = plan.areas.len() > 1;
            let mut l = area.local(self.index);
            let mut index = self.index;
            let (a, now) = (self.area, &mut self.now);
            let fluids = plan.fluids();
            area.cells.each(self.index..end, &mut |p| {
                let pos = area.pos(l);
                let spot = Spot { area: a, index };
                step(&mut l, area.size);
                index += 1;
                let Slot::Want(want) = area.palette_slot(p, fluids) else {
                    return;
                };
                if shared && plan.locate(pos).is_none_or(|(s, _)| s.area != a) {
                    return;
                }
                match state_at(pos) {
                    None => {
                        now.unseen += 1;
                        if now
                            .nearest_unseen
                            .is_none_or(|u| dist2(pos, near) < dist2(u, near))
                        {
                            now.nearest_unseen = Some(pos);
                        }
                    }
                    Some(world) => {
                        let (verdict, kept) = note(plan, wrong, spot, pos, want, world, mode, near);
                        now.fluids += u32::from(verdict == Verdict::Fluid);
                        now.overflowed |= !kept;
                    }
                }
            });
            left -= end - self.index;
            self.index = end;
            if end == volume {
                self.area += 1;
                self.index = 0;
            }
        }
    }
}

fn step(l: &mut [i32; 3], size: [i32; 3]) {
    l[0] += 1;
    if l[0] == size[0] {
        l[0] = 0;
        l[2] += 1;
        if l[2] == size[2] {
            l[2] = 0;
            l[1] += 1;
        }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub(in crate::modules::builder) struct Vecs(pub Vec<u32>);

    impl Cells for Vecs {
        fn get(&self, index: usize) -> u32 {
            self.0[index]
        }
        fn each(&self, range: Range<usize>, f: &mut dyn FnMut(u32)) {
            for &p in &self.0[range] {
                f(p);
            }
        }
    }

    pub(in crate::modules::builder) fn id(kind: BlockKind) -> u16 {
        BlockState::from(kind).id() as u16
    }

    pub(in crate::modules::builder) fn region(
        origin: [i32; 3],
        size: [i32; 3],
        palette: Vec<u16>,
        cells: Vec<u32>,
    ) -> Region {
        Region {
            origin,
            size,
            palette,
            cells: Arc::new(Vecs(cells)),
        }
    }

    fn stone() -> BlockState {
        BlockState::from(BlockKind::Stone)
    }

    #[test]
    fn index_and_local_are_inverses() {
        let plan = Plan::new(vec![region([10, 64, -5], [2, 3, 4], vec![0], vec![0; 24])]).unwrap();
        let area = &plan.areas[0];
        for i in 0..24 {
            let l = area.local(i);
            assert_eq!(area.index([l[0] + 10, l[1] + 64, l[2] - 5]), Some(i));
        }
        assert_eq!(area.index([11, 64, -5]), Some(1));
        assert_eq!(area.index([10, 64, -4]), Some(2));
        assert_eq!(area.index([10, 65, -5]), Some(8));
        assert_eq!(area.index([12, 64, -5]), None);
        let mut l = [0, 0, 0];
        for i in 0..24 {
            assert_eq!(l, area.local(i));
            step(&mut l, area.size);
        }
    }

    #[test]
    fn want_reads_the_palette_and_leaves_structure_void() {
        let palette = vec![0, id(BlockKind::Stone), id(BlockKind::StructureVoid)];
        let plan = Plan::new(vec![region(
            [0, 0, 0],
            [2, 1, 2],
            palette,
            vec![0, 1, 2, 7],
        )])
        .unwrap();
        assert_eq!(plan.want(BlockPos::new(0, 0, 0)), Some(BlockState::AIR));
        assert_eq!(plan.want(BlockPos::new(1, 0, 0)), Some(stone()));
        assert_eq!(plan.want(BlockPos::new(0, 0, 1)), None);
        assert_eq!(plan.want(BlockPos::new(1, 0, 1)), Some(BlockState::AIR));
        assert_eq!(plan.want(BlockPos::new(2, 0, 0)), None);
        assert!(plan.kept(BlockPos::new(0, 0, 1)));
        assert!(!plan.kept(BlockPos::new(1, 0, 0)));
        assert!(!plan.kept(BlockPos::new(2, 0, 0)));
        assert_eq!(plan.total(), 4);
    }

    #[test]
    fn the_first_region_wins_and_void_falls_through() {
        let first = region(
            [0, 0, 0],
            [2, 1, 1],
            vec![id(BlockKind::Stone), id(BlockKind::StructureVoid)],
            vec![0, 1],
        );
        let second = region([0, 0, 0], [2, 1, 1], vec![id(BlockKind::Dirt)], vec![0, 0]);
        let plan = Plan::new(vec![first, second]).unwrap();
        assert_eq!(plan.want(BlockPos::new(0, 0, 0)), Some(stone()));
        assert_eq!(
            plan.want(BlockPos::new(1, 0, 0)),
            Some(BlockState::from(BlockKind::Dirt))
        );
    }

    #[test]
    fn bad_regions_are_refused() {
        assert!(Plan::new(Vec::new()).is_err());
        assert!(Plan::new(vec![region([0, 0, 0], [0, 1, 1], vec![], vec![])]).is_err());
        assert!(
            Plan::new(vec![region(
                [i32::MAX - 1, 0, 0],
                [4, 1, 1],
                vec![],
                vec![0; 4]
            )])
            .is_err()
        );
        let huge = Region {
            origin: [0, 0, 0],
            size: [1 << 10, 1 << 10, 1 << 8],
            palette: vec![],
            cells: Arc::new(Vecs(vec![])),
        };
        assert!(Plan::new(vec![huge]).is_err());
    }

    #[test]
    fn the_correct_bits_are_per_cell() {
        let plan = Plan::new(vec![region(
            [0, 0, 0],
            [70, 1, 1],
            vec![id(BlockKind::Stone)],
            vec![0; 70],
        )])
        .unwrap();
        let (a, _) = plan.locate(BlockPos::new(3, 0, 0)).unwrap();
        let (b, _) = plan.locate(BlockPos::new(67, 0, 0)).unwrap();
        assert!(!plan.is_correct(a));
        plan.mark(a, true);
        plan.mark(b, true);
        assert!(plan.is_correct(a) && plan.is_correct(b));
        plan.mark(a, false);
        assert!(!plan.is_correct(a) && plan.is_correct(b));
    }

    #[test]
    fn matching_rules() {
        let water = BlockState::from(BlockKind::Water);
        let dirt = BlockState::from(BlockKind::Dirt);
        let grass = BlockState::from(BlockKind::ShortGrass);
        let air = BlockState::AIR;
        let cave = BlockState::from(BlockKind::CaveAir);
        assert_eq!(judge(water, air, BreakMode::All, false), Verdict::Fluid);
        assert_eq!(judge(cave, air, BreakMode::All, false), Verdict::Right);
        assert_eq!(judge(stone(), air, BreakMode::All, false), Verdict::Wrong);
        assert_eq!(
            judge(stone(), air, BreakMode::OnlyWhereNeeded, false),
            Verdict::Right
        );
        assert_eq!(
            judge(stone(), stone(), BreakMode::OnlyWhereNeeded, false),
            Verdict::Right
        );
        assert_eq!(judge(water, stone(), BreakMode::All, false), Verdict::Wrong);
        assert_eq!(
            judge(dirt, stone(), BreakMode::OnlyWhereNeeded, false),
            Verdict::Wrong
        );
        assert_eq!(job(water, stone(), false), Job::Place);
        assert_eq!(job(air, stone(), false), Job::Place);
        assert_eq!(job(grass, stone(), false), Job::Place);
        assert_eq!(job(dirt, stone(), false), Job::Break);
        assert_eq!(job(stone(), air, false), Job::Break);
        assert_eq!(job(water, air, false), Job::Break);
    }

    fn with(kind: BlockKind, props: &[(&str, &str)]) -> BlockState {
        let mut b = Box::<dyn BlockTrait>::from(BlockState::from(kind));
        for (k, v) in props {
            b.set_property(k, v).unwrap();
        }
        b.as_block_state()
    }

    #[test]
    fn fluid_judgement_table() {
        let water = BlockState::from(BlockKind::Water);
        let lava = BlockState::from(BlockKind::Lava);
        let flowing = with(BlockKind::Water, &[("level", "3")]);
        let air = BlockState::AIR;
        let all = BreakMode::All;
        assert!(source(water) && source(lava) && !source(flowing));
        assert_eq!(judge(water, water, all, true), Verdict::Right);
        assert_eq!(judge(air, water, all, true), Verdict::Wrong);
        assert_eq!(job(air, water, true), Job::Pour);
        assert_eq!(judge(flowing, water, all, true), Verdict::Wrong);
        assert_eq!(job(flowing, water, true), Job::Pour);
        assert_eq!(job(stone(), water, true), Job::Break);
        assert_eq!(judge(water, lava, all, true), Verdict::Wrong);
        assert_eq!(job_item(Job::Pour, lava), Some(ItemKind::LavaBucket));
        let wet = with(BlockKind::OakSlab, &[("waterlogged", "true")]);
        let dry_slab = BlockState::from(BlockKind::OakSlab);
        assert_eq!(judge(wet, wet, all, true), Verdict::Right);
        assert_eq!(judge(dry_slab, wet, all, true), Verdict::Wrong);
        assert_eq!(job(dry_slab, wet, true), Job::Pour);
        assert_eq!(job_item(Job::Pour, wet), Some(ItemKind::WaterBucket));
        assert_eq!(job(air, wet, true), Job::Place);
        assert_eq!(judge(wet, dry_slab, all, true), Verdict::Right);
        assert_eq!(judge(water, air, all, true), Verdict::Wrong);
        assert_eq!(job(water, air, true), Job::Drain);
        assert_eq!(judge(lava, air, all, true), Verdict::Wrong);
        assert_eq!(judge(flowing, air, all, true), Verdict::Fluid);
        assert_eq!(
            judge(water, air, BreakMode::OnlyWhereNeeded, true),
            Verdict::Fluid
        );
        assert_eq!(job(water, stone(), true), Job::Place);
    }

    #[test]
    fn fluids_off_leaves_sources_and_dries_wants() {
        let wet = with(BlockKind::OakSlab, &[("waterlogged", "true")]);
        let palette = vec![
            id(BlockKind::Water),
            wet.id() as u16,
            with(BlockKind::Water, &[("level", "2")]).id() as u16,
        ];
        let plan = Plan::new(vec![region([0, 0, 0], [3, 1, 1], palette, vec![0, 1, 2])]).unwrap();
        assert_eq!(
            plan.want(BlockPos::new(0, 0, 0)),
            Some(BlockState::from(BlockKind::Water))
        );
        assert_eq!(plan.want(BlockPos::new(1, 0, 0)), Some(wet));
        assert_eq!(plan.want(BlockPos::new(2, 0, 0)), None);
        assert!(plan.set_fluids(false));
        assert_eq!(plan.want(BlockPos::new(0, 0, 0)), None);
        assert_eq!(
            plan.want(BlockPos::new(1, 0, 0)),
            Some(BlockState::from(BlockKind::OakSlab))
        );
        assert!(!plan.set_fluids(false));
        assert_eq!(
            slot_for(id(BlockKind::CaveAir)),
            Slot::Want(BlockState::AIR)
        );
        assert_eq!(slot_for(id(BlockKind::BubbleColumn)), Slot::Leave);
    }

    #[test]
    fn fluids_come_last() {
        let water = BlockState::from(BlockKind::Water);
        assert_eq!(phase(water, Job::Pour), 3);
        assert!(phase(BlockState::from(BlockKind::Repeater), Job::Use) < phase(water, Job::Pour));
        assert!(phase(BlockState::from(BlockKind::Observer), Job::Place) < phase(water, Job::Pour));
        assert!(phase(water, Job::Pour) < EXPOSED_PHASE);
        assert_eq!(phase(BlockState::AIR, Job::Drain), 0);
    }

    #[test]
    fn a_pour_waits_for_its_walls() {
        let palette = vec![id(BlockKind::Stone), id(BlockKind::Water)];
        let plan = Plan::new(vec![
            region([0, 1, 0], [3, 1, 1], palette.clone(), vec![0, 1, 0]),
            region([0, 0, 0], [3, 1, 1], palette, vec![0, 0, 0]),
        ])
        .unwrap();
        let cell = BlockPos::new(1, 1, 0);
        let walls_up = |p: BlockPos| {
            Some(if p.y == 0 || p.x != 1 {
                stone()
            } else {
                BlockState::AIR
            })
        };
        let none = |_: BlockPos| false;
        assert_eq!(contained(&plan, cell, true, &walls_up, &none), (true, true));
        let one_wall_missing = |p: BlockPos| {
            Some(if p.y == 0 || p.x == 0 {
                stone()
            } else {
                BlockState::AIR
            })
        };
        assert!(!contained(&plan, cell, true, &one_wall_missing, &none).0);
        let east = |p: BlockPos| p == BlockPos::new(2, 1, 0);
        assert_eq!(
            contained(&plan, cell, true, &one_wall_missing, &east),
            (true, true)
        );
        let closed = |p: BlockPos| Some(if p == cell { BlockState::AIR } else { stone() });
        assert_eq!(contained(&plan, cell, true, &closed, &none), (true, false));
    }

    #[test]
    fn a_stand_hole_is_a_floor_cell_with_ground_under_it() {
        let target = BlockPos::new(0, 1, 0);
        let floor = |p: BlockPos| p.y == 0 && (-1..=2).contains(&p.x) && (-1..=1).contains(&p.z);
        let want = |p: BlockPos| floor(p).then(stone);
        let world = |p: BlockPos| Some(if p.y == -1 { stone() } else { BlockState::AIR });
        let dig = |_: BlockPos, _: BlockState| true;
        assert_eq!(
            choose_hole(target, &want, &world, &dig),
            Some((BlockPos::new(-1, 0, 0), HoleKind::Reserve))
        );
        let platform = stone();
        let world = |p: BlockPos| {
            Some(
                if p.y == -1 && p != BlockPos::new(-1, -1, 0) && p != BlockPos::new(0, -1, 1) {
                    platform
                } else {
                    BlockState::AIR
                },
            )
        };
        let want = |p: BlockPos| (floor(p) || p == BlockPos::new(0, -1, 1)).then(stone);
        assert_eq!(
            choose_hole(target, &want, &world, &dig),
            Some((BlockPos::new(0, 0, -1), HoleKind::Reserve))
        );
    }

    #[test]
    fn a_stand_hole_refuses_what_it_must_not_take() {
        let target = BlockPos::new(0, 1, 0);
        let world = |p: BlockPos| Some(if p.y <= 0 { stone() } else { BlockState::AIR });
        let no_dig = |_: BlockPos, _: BlockState| false;
        assert_eq!(choose_hole(target, &|_| None, &world, &no_dig), None);
        let repeater = BlockState::from(BlockKind::Repeater);
        let want = |p: BlockPos| match (p.x, p.y, p.z) {
            (1, 0, 0) | (-1, 0, 0) => Some(repeater),
            (0, 1, 1) | (0, 1, -1) => Some(stone()),
            _ => None,
        };
        let open = |p: BlockPos| Some(if p.y < 0 { stone() } else { BlockState::AIR });
        assert_eq!(choose_hole(target, &want, &open, &no_dig), None);
        assert!(hole_forbidden(repeater));
        assert!(hole_forbidden(BlockState::from(BlockKind::OakStairs)));
        assert!(!hole_forbidden(stone()));
        let yes = |_: BlockPos, _: BlockState| true;
        let one_open = |p: BlockPos| {
            Some(if p.y < 0 || (p.y == 0 && p != BlockPos::new(0, 0, 1)) {
                stone()
            } else {
                BlockState::AIR
            })
        };
        assert_eq!(
            choose_hole(target, &|_| None, &one_open, &yes),
            Some((BlockPos::new(0, 0, 1), HoleKind::Open))
        );
    }

    #[test]
    fn infinite_sources() {
        let water = BlockState::from(BlockKind::Water);
        let pool = |cells: &'static [(i32, i32)], floor: BlockState| {
            move |p: BlockPos| {
                Some(if p.y == 0 && cells.contains(&(p.x, p.z)) {
                    water
                } else if p.y == -1 {
                    floor
                } else {
                    BlockState::AIR
                })
            }
        };
        let square = pool(&[(0, 0), (1, 0), (0, 1), (1, 1)], stone());
        for (x, z) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            assert!(refills(BlockPos::new(x, 0, z), &square));
        }
        let trench = pool(&[(0, 0), (1, 0), (2, 0)], stone());
        assert!(!refills(BlockPos::new(0, 0, 0), &trench));
        assert!(refills(BlockPos::new(1, 0, 0), &trench));
        assert!(!refills(BlockPos::new(2, 0, 0), &trench));
        let single = pool(&[(0, 0)], stone());
        assert!(!refills(BlockPos::new(0, 0, 0), &single));
        let hanging = pool(&[(0, 0), (1, 0), (2, 0)], BlockState::AIR);
        assert!(!refills(BlockPos::new(1, 0, 0), &hanging));
    }

    #[test]
    fn the_cap_keeps_the_nearest() {
        let near = BlockPos::new(0, 0, 0);
        let mut wrong = Wrong::new(3);
        assert!(wrong.insert(BlockPos::new(1, 0, 0), near));
        assert!(wrong.insert(BlockPos::new(5, 0, 0), near));
        assert!(wrong.insert(BlockPos::new(3, 0, 0), near));
        assert!(!wrong.insert(BlockPos::new(9, 0, 0), near));
        assert_eq!(wrong.len(), 3);
        assert!(!wrong.insert(BlockPos::new(2, 0, 0), near));
        let mut kept: Vec<i32> = wrong.iter().map(|p| p.x).collect();
        kept.sort_unstable();
        assert_eq!(kept, vec![1, 2, 3]);
        assert!(wrong.insert(BlockPos::new(2, 0, 0), near));
        wrong.skip(BlockPos::new(1, 0, 0));
        assert!(wrong.insert(BlockPos::new(1, 0, 0), near));
        assert_eq!((wrong.len(), wrong.skipped()), (2, 1));
    }

    #[test]
    fn redstone_comes_in_two_late_passes() {
        let s = |k: BlockKind| BlockState::from(k);
        assert_eq!(phase(s(BlockKind::Stone), Job::Place), 0);
        assert_eq!(phase(s(BlockKind::OakStairs), Job::Place), 0);
        assert_eq!(phase(s(BlockKind::RedstoneWire), Job::Place), 1);
        assert_eq!(phase(s(BlockKind::Repeater), Job::Place), 1);
        assert_eq!(phase(s(BlockKind::PoweredRail), Job::Place), 1);
        assert_eq!(phase(s(BlockKind::RedstoneLamp), Job::Place), 1);
        assert_eq!(phase(s(BlockKind::Observer), Job::Place), 2);
        assert_eq!(phase(s(BlockKind::StickyPiston), Job::Place), 2);
        assert_eq!(phase(s(BlockKind::RedstoneTorch), Job::Place), 2);
        assert_eq!(phase(s(BlockKind::Lever), Job::Place), 2);
        assert_eq!(phase(s(BlockKind::Repeater), Job::Use), 2);
        let mut d3 = Box::<dyn BlockTrait>::from(s(BlockKind::Repeater));
        d3.set_property("delay", "3").unwrap();
        assert_eq!(
            job(s(BlockKind::Repeater), d3.as_block_state(), true),
            Job::Use
        );
    }

    #[test]
    fn a_door_top_and_a_bed_head_come_with_their_partner() {
        let mut top = Box::<dyn BlockTrait>::from(BlockState::from(BlockKind::OakDoor));
        top.set_property("half", "upper").unwrap();
        let top = top.as_block_state();
        assert!(partner_half(top));
        assert!(!partner_half(BlockState::from(BlockKind::OakDoor)));
        assert_eq!(
            partner_base(BlockPos::new(0, 65, 0), top),
            BlockPos::new(0, 64, 0)
        );
        let mut head = Box::<dyn BlockTrait>::from(BlockState::from(BlockKind::RedBed));
        head.set_property("part", "head").unwrap();
        head.set_property("facing", "north").unwrap();
        let head = head.as_block_state();
        assert!(partner_half(head));
        assert_eq!(
            partner_base(BlockPos::new(0, 64, 0), head),
            BlockPos::new(0, 64, 1)
        );
        let mut slab = Box::<dyn BlockTrait>::from(BlockState::from(BlockKind::OakSlab));
        slab.set_property("type", "top").unwrap();
        assert!(!partner_half(slab.as_block_state()));
    }

    #[test]
    fn skipped_cells_come_back_once_picked() {
        let near = BlockPos::new(0, 0, 0);
        let mut wrong = Wrong::new(8);
        for x in 1..=3 {
            wrong.insert(BlockPos::new(x, 0, 0), near);
            wrong.skip(BlockPos::new(x, 0, 0));
        }
        assert_eq!((wrong.len(), wrong.skipped()), (0, 3));
        let back = wrong.take_skipped(|p| p.x != 2);
        assert_eq!(back.len(), 2);
        assert_eq!(wrong.skipped(), 1);
        for p in back {
            assert!(wrong.insert(p, near));
        }
        assert_eq!(wrong.len(), 2);
        assert_eq!(wrong.take_skipped(|_| true), vec![BlockPos::new(2, 0, 0)]);
    }

    #[test]
    fn a_pass_finds_the_wrong_cells_and_the_unloaded_ones() {
        let plan = Plan::new(vec![region(
            [0, 0, 0],
            [4, 1, 1],
            vec![id(BlockKind::Stone)],
            vec![0; 4],
        )])
        .unwrap();
        let world = |p: BlockPos| match p.x {
            0 => Some(stone()),
            1 => Some(BlockState::from(BlockKind::Dirt)),
            2 => Some(BlockState::AIR),
            _ => None,
        };
        let mut wrong = Wrong::new(16);
        let mut scan = Scan::new(0);
        let near = BlockPos::new(0, 0, 0);
        scan.batch(&plan, &mut wrong, near, BreakMode::All, 3, &world);
        assert!(!scan.complete());
        scan.batch(&plan, &mut wrong, near, BreakMode::All, 3, &world);
        assert!(scan.complete() && !scan.clean());
        let last = scan.last.unwrap();
        assert_eq!(
            (last.unseen, last.nearest_unseen),
            (1, Some(BlockPos::new(3, 0, 0)))
        );
        let mut cells: Vec<i32> = wrong.iter().map(|p| p.x).collect();
        cells.sort_unstable();
        assert_eq!(cells, vec![1, 2]);
        assert!(plan.is_correct(plan.locate(near).unwrap().0));
        scan.restart_if(0, wrong.len(), 16, false);
        assert!(scan.complete());
        scan.restart_if(1, wrong.len(), 16, false);
        assert!(!scan.complete());
    }
}
