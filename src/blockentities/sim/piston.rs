mod push_reactions;

use std::sync::Arc;

use azalea::block::{BlockState, BlockTrait};
use azalea::physics::collision::{BlockWithShape, MovingBlocks};
use azalea_core::aabb::Aabb;
use azalea_core::direction::Direction;
use azalea_core::position::{BlockPos, Vec3};
use azalea_registry::builtin::BlockKind;
use azalea_registry::tags::blocks::FIRE;
use bevy::platform::collections::HashMap as FastMap;

use crate::blocks::{facing, is_air};
use push_reactions::{has_block_entity, push_reaction};

const MAX_PUSH_DEPTH: usize = 12;

const DEATH_TICKS: u8 = 5;

const LEAVING_TICKS: u8 = 100;

const MAX_LIVE: usize = 65_536;

const MAX_EVENT: usize = MAX_PUSH_DEPTH + 2;

const MAX_IDLE_UPDATES: usize = 1024;

const NEAR_MARGIN: f64 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PushReaction {
    Normal,
    Destroy,
    Block,
    PushOnly,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Look {
    #[default]
    None,
    Block(u32),
    Head {
        long: u32,
        short: u32,
    },
    Retracting {
        long: u32,
        short: u32,
        base: u32,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Moving {
    pub moved: BlockState,
    pub direction: Direction,
    pub extending: bool,
    pub source: bool,
    pub look: Look,
}

impl Moving {
    fn new(moved: BlockState, direction: Direction, extending: bool, source: bool) -> Moving {
        let kind = BlockKind::from(moved);
        let look = if is_air(kind) {
            Look::None
        } else if kind == BlockKind::PistonHead {
            Look::Head {
                long: id(with(moved, &[("short", "false")])),
                short: id(with(moved, &[("short", "true")])),
            }
        } else if source && !extending {
            let base = Box::<dyn BlockTrait>::from(moved);
            let head = with(
                BlockState::from(BlockKind::PistonHead),
                &[
                    ("type", piston_type(kind == BlockKind::StickyPiston)),
                    ("facing", base.get_property("facing").unwrap_or("north")),
                ],
            );
            Look::Retracting {
                long: id(with(head, &[("short", "false")])),
                short: id(with(head, &[("short", "true")])),
                base: id(with(moved, &[("extended", "true")])),
            }
        } else {
            Look::Block(id(moved))
        };
        Moving {
            moved,
            direction,
            extending,
            source,
            look,
        }
    }

    pub fn movement(&self) -> Direction {
        if self.extending {
            self.direction
        } else {
            self.direction.opposite()
        }
    }

    pub fn corner_at(&self, pos: [i32; 3], progress: f32) -> Vec3 {
        let offset = self.offset(progress);
        Vec3::new(
            f64::from(pos[0]) + f64::from(offset[0]),
            f64::from(pos[1]) + f64::from(offset[1]),
            f64::from(pos[2]) + f64::from(offset[2]),
        )
    }

    fn is_retracting_base(&self) -> bool {
        self.source && !self.extending && is_piston(BlockKind::from(self.moved))
    }

    fn at_rest(self) -> Moving {
        let look = if is_air(BlockKind::from(self.moved)) {
            Look::None
        } else {
            Look::Block(id(self.moved))
        };
        Moving {
            source: false,
            look,
            ..self
        }
    }

    pub fn offset(&self, progress: f32) -> [f32; 3] {
        let extended = if self.extending {
            progress - 1.0
        } else {
            1.0 - progress
        };
        [
            self.direction.x() as f32 * extended,
            self.direction.y() as f32 * extended,
            self.direction.z() as f32 * extended,
        ]
    }

    pub fn draws(&self, progress: f32) -> [Option<(u32, [f32; 3])>; 2] {
        let offset = self.offset(progress);
        match self.look {
            Look::None => [None, None],
            Look::Block(state) => [Some((state, offset)), None],
            Look::Head { long, short } => [
                Some((if progress <= 0.5 { short } else { long }, offset)),
                None,
            ],
            Look::Retracting { long, short, base } => [
                Some((if progress >= 0.5 { short } else { long }, offset)),
                Some((base, [0.0; 3])),
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Progress {
    pub progress: f32,
    pub progress_o: f32,
}

impl Progress {
    const DONE: Progress = Progress {
        progress: 1.0,
        progress_o: 1.0,
    };

    pub fn tick(&mut self) {
        self.progress_o = self.progress;
        if self.progress_o < 1.0 {
            self.progress = (self.progress + 0.5).min(1.0);
        }
    }

    pub fn at(&self, partial: f32) -> f32 {
        let a = partial.min(1.0);
        self.progress_o + (self.progress - self.progress_o) * a
    }
}

#[derive(Debug, Default)]
struct Shapes {
    moved: Vec<Aabb>,
    head: [Vec<Aabb>; 2],
    base: Vec<Aabb>,
}

impl Shapes {
    fn of(pos: [i32; 3], moving: &Moving) -> Shapes {
        let at = BlockPos::new(pos[0], pos[1], pos[2]);
        let boxes = |state: BlockState| state.collision_shape(at).to_aabbs();
        let mut shapes = Shapes {
            moved: boxes(moving.moved),
            ..Shapes::default()
        };
        if moving.source {
            shapes.head = ["false", "true"].map(|short| {
                boxes(with(
                    BlockState::from(BlockKind::PistonHead),
                    &[("facing", facing::name(moving.direction)), ("short", short)],
                ))
            });
        }
        if moving.is_retracting_base() {
            shapes.base = boxes(with(moving.moved, &[("extended", "true")]));
        }
        shapes
    }

    fn pushing(&self, moving: &Moving, progress: f32) -> &[Aabb] {
        if moving.is_retracting_base() {
            &self.head[usize::from(progress > 0.25)]
        } else {
            &self.moved
        }
    }

    fn solid(
        &self,
        pos: [i32; 3],
        moving: &Moving,
        progress: f32,
        noclip: Option<Direction>,
        out: &mut Vec<(BlockPos, Aabb)>,
    ) {
        let at = BlockPos::new(pos[0], pos[1], pos[2]);
        let corner = at.to_vec3_floored();
        out.extend(
            self.base
                .iter()
                .map(|aabb| (at, aabb.move_relative(corner))),
        );
        if progress < 1.0 && noclip == Some(moving.movement()) {
            return;
        }
        let boxes = if moving.source {
            &self.head[usize::from(moving.extending != (1.0 - progress < 0.25))]
        } else {
            &self.moved
        };
        let corner = moving.corner_at(pos, progress);
        out.extend(boxes.iter().map(|aabb| (at, aabb.move_relative(corner))));
    }
}

struct Live {
    seq: u64,
    since: u64,
    moving: Moving,
    progress: Progress,
    last: Progress,
    death_ticks: u8,
    settled: bool,
    vacated: Option<[i32; 3]>,
    covers: bool,
    published: bool,
    shapes: Option<Arc<Shapes>>,
}

impl Live {
    fn shapes(&mut self, pos: [i32; 3]) -> Arc<Shapes> {
        self.shapes
            .get_or_insert_with(|| Arc::new(Shapes::of(pos, &self.moving)))
            .clone()
    }

    fn skip_base(&self) -> bool {
        matches!(self.moving.look, Look::Retracting { .. }) && !self.covers
    }
}

struct Leaving {
    seq: u64,
    moving: Moving,
    skip_base: bool,
    ticks: u8,
}

struct Vacated {
    seq: u64,
    since: u64,
    drawn: bool,
}

#[derive(Clone, Copy, Default)]
struct Held {
    number: u64,
    state: Option<BlockState>,
}

#[derive(Default)]
struct Table {
    live: FastMap<[i32; 3], Live>,
    resting: usize,
    leaving: FastMap<[i32; 3], Leaving>,
    vacated: FastMap<[i32; 3], Vacated>,
    order: Vec<(u64, [i32; 3])>,
    updates: Vec<(u64, [i32; 3], BlockState)>,
    next_update: u64,
    known: FastMap<[i32; 3], (u64, BlockState)>,
    indexed: usize,
    held: FastMap<[i32; 3], Held>,
    ack: Option<u64>,
    fresh: Vec<[i32; 3]>,
    frozen: bool,
    next_seq: u64,
    touched_cells: Vec<[i32; 3]>,
    touched_moves: Vec<[i32; 3]>,
    redraw: bool,
}

#[derive(Default)]
struct Shared {
    table: Table,
    generation: u64,
}

crate::util::global!(fn shared() -> parking_lot::Mutex<Shared>);

fn checkout() -> (Table, u64) {
    let mut shared = shared().lock();
    (std::mem::take(&mut shared.table), shared.generation)
}

fn checkin(table: Table, generation: u64) {
    let mut shared = shared().lock();
    if shared.generation == generation {
        shared.table = table;
    }
}

impl Table {
    fn insert(&mut self, created: Created) {
        let Created {
            pos,
            moving,
            vacated,
            covers,
        } = created;
        self.take(pos);
        self.leaving.remove(&pos);
        self.next_seq += 1;
        let seq = self.next_seq;
        if let Some((cell, drawn)) = vacated {
            self.vacate(cell, seq, drawn);
        }
        self.live.insert(
            pos,
            Live {
                seq,
                since: self.next_update,
                moving,
                progress: Progress::default(),
                last: Progress::default(),
                death_ticks: 0,
                settled: false,
                vacated: vacated.map(|(cell, _)| cell),
                covers,
                published: false,
                shapes: None,
            },
        );
        self.fresh.push(pos);
        self.order.push((seq, pos));
        if self.order.len() > 2 * self.live.len() + 64 {
            self.prune_order();
        }
        self.touch(pos);
    }

    fn prune_order(&mut self) {
        let Table { order, live, .. } = self;
        order.retain(|(seq, pos)| {
            live.get(pos)
                .is_some_and(|live| live.seq == *seq && !live.settled)
        });
    }

    fn vacate(&mut self, cell: [i32; 3], seq: u64, drawn: bool) {
        let since = self.next_update;
        self.vacated.insert(cell, Vacated { seq, since, drawn });
        self.fresh.push(cell);
        self.touched_cells.push(cell);
    }

    fn make_room(&mut self) -> bool {
        if self.vacated.len() + MAX_EVENT > 2 * MAX_LIVE {
            return false;
        }
        if self.live.len() + MAX_EVENT <= MAX_LIVE {
            return true;
        }
        if self.frozen || self.resting < MAX_LIVE / 4 {
            return false;
        }
        self.evict();
        true
    }

    fn touch(&mut self, pos: [i32; 3]) {
        self.touched_cells.push(pos);
        self.touched_moves.push(pos);
        self.redraw = true;
    }

    fn evict(&mut self) {
        let mut settled: Vec<(u64, [i32; 3])> = self
            .live
            .iter()
            .filter(|(_, live)| live.settled)
            .map(|(pos, live)| (live.seq, *pos))
            .collect();
        settled.sort_unstable();
        for (_, pos) in settled.iter().take(settled.len().div_ceil(4)) {
            self.remove(*pos);
        }
    }

    fn remove(&mut self, pos: [i32; 3]) -> Option<Live> {
        let live = self.take(pos)?;
        if let Some(cell) = live.vacated
            && self.vacated.get(&cell).is_some_and(|v| v.seq == live.seq)
        {
            self.vacated.remove(&cell);
            self.touched_cells.push(cell);
        }
        Some(live)
    }

    fn take(&mut self, pos: [i32; 3]) -> Option<Live> {
        let live = self.live.remove(&pos)?;
        self.resting -= usize::from(live.settled);
        self.touch(pos);
        Some(live)
    }

    fn leave(&mut self, pos: [i32; 3]) {
        if let Some(live) = self.take(pos)
            && live.published
        {
            self.leaving.insert(
                pos,
                Leaving {
                    seq: live.seq,
                    moving: live.moving,
                    skip_base: live.skip_base(),
                    ticks: 0,
                },
            );
        }
    }

    fn settle(&mut self, pos: [i32; 3]) {
        let Some(live) = self.live.get_mut(&pos) else {
            return;
        };
        live.covers |= live.skip_base();
        live.moving = live.moving.at_rest();
        self.resting += usize::from(!live.settled);
        live.settled = true;
        live.progress = Progress::DONE;
        live.shapes = None;
        self.touch(pos);
    }

    fn server_update(&mut self, pos: [i32; 3], state: BlockState) {
        self.updates.push((self.next_update, pos, state));
        self.next_update += 1;
    }

    fn note_ack(&mut self) {
        self.ack.get_or_insert(self.next_update);
        self.next_update += 1;
    }

    fn written(&mut self, is_predicting: impl Fn([i32; 3]) -> bool) {
        let mut updates = std::mem::take(&mut self.updates);
        for (number, pos, state) in updates.drain(..) {
            if is_predicting(pos) {
                let state = Some(state);
                self.held.insert(pos, Held { number, state });
            } else {
                self.answer(number, pos, Some(state));
            }
        }
        for pos in self.fresh.drain(..) {
            if is_predicting(pos) {
                self.held.entry(pos).or_default();
            }
        }
        updates.shrink_to(MAX_IDLE_UPDATES);
        self.updates = updates;
        self.known.clear();
        self.indexed = 0;
    }

    fn acknowledged(&mut self, is_predicting: impl Fn([i32; 3]) -> bool) {
        let ack = self.ack.take().unwrap_or(u64::MAX);
        if self.held.is_empty() {
            return;
        }
        let done: Vec<([i32; 3], Held)> = self
            .held
            .iter()
            .map(|(pos, held)| (*pos, *held))
            .filter(|(pos, _)| !is_predicting(*pos))
            .collect();
        for (pos, Held { number, state }) in done {
            self.held.remove(&pos);
            self.answer(number.max(ack), pos, state);
        }
    }

    fn answer(&mut self, number: u64, pos: [i32; 3], state: Option<BlockState>) {
        if let Some(live) = self.live.get_mut(&pos).filter(|live| number >= live.since) {
            if state.map(BlockKind::from) != Some(BlockKind::MovingPiston) {
                self.leave(pos);
            } else if live.skip_base() {
                live.covers = true;
                self.touch(pos);
            }
        }
        if self.vacated.get(&pos).is_some_and(|v| number >= v.since) {
            self.vacated.remove(&pos);
            self.touched_cells.push(pos);
        }
    }

    fn index_updates(&mut self) {
        for (number, pos, state) in &self.updates[self.indexed..] {
            self.known.insert(*pos, (*number, *state));
        }
        self.indexed = self.updates.len();
    }

    fn told(&self, pos: &[i32; 3], since: u64) -> Option<BlockState> {
        self.known
            .get(pos)
            .filter(|(number, _)| *number >= since)
            .map(|(_, state)| *state)
    }

    fn is_answered(&self, pos: &[i32; 3], live: &Live) -> bool {
        self.told(pos, live.since)
            .is_some_and(|state| BlockKind::from(state) != BlockKind::MovingPiston)
    }

    fn hides(&self, cell: &[i32; 3]) -> Option<u64> {
        let own = self
            .live
            .get(cell)
            .filter(|live| live.covers)
            .map(|live| live.seq);
        let emptied = self.vacated.get(cell).filter(|v| v.drawn).map(|v| v.seq);
        own.max(emptied)
    }

    fn delta(&mut self) -> Delta {
        let mut cells = std::mem::take(&mut self.touched_cells);
        cells.sort_unstable();
        cells.dedup();
        let mut moves = std::mem::take(&mut self.touched_moves);
        moves.sort_unstable();
        moves.dedup();
        Delta {
            cells: cells
                .into_iter()
                .map(|cell| (cell, self.hides(&cell)))
                .collect(),
            moves: moves
                .into_iter()
                .map(|pos| (pos, self.move_state(&pos)))
                .collect(),
        }
    }

    fn move_state(&self, pos: &[i32; 3]) -> MoveState {
        if let Some(live) = self.live.get(pos) {
            let emptied = live
                .vacated
                .filter(|cell| self.vacated.get(cell).is_some_and(|v| v.drawn));
            return MoveState::Live {
                seq: live.seq,
                cells: [live.covers.then_some(*pos), emptied],
            };
        }
        match self.leaving.get(pos) {
            Some(left) => MoveState::Left { seq: left.seq },
            None => MoveState::Gone,
        }
    }

    fn copies(&mut self) -> FastMap<[i32; 3], MovingCopy> {
        let mut out = FastMap::with_capacity_and_hasher(
            self.live.len() + self.leaving.len(),
            Default::default(),
        );
        self.redraw = false;
        for (pos, left) in &self.leaving {
            out.insert(
                *pos,
                MovingCopy {
                    moving: left.moving,
                    progress: Progress::DONE,
                    last: Progress::DONE,
                    skip_base: left.skip_base,
                },
            );
        }
        for (pos, live) in &mut self.live {
            out.insert(
                *pos,
                MovingCopy {
                    moving: live.moving,
                    progress: live.progress,
                    last: live.last,
                    skip_base: live.skip_base(),
                },
            );
            self.redraw |= live.last != live.progress;
            live.last = live.progress;
            live.published = true;
        }
        out
    }

    fn state(&self, pos: [i32; 3]) -> Option<BlockState> {
        if let Some(live) = self.live.get(&pos)
            && !self.is_answered(&pos, live)
        {
            return Some(if live.settled {
                live.moving.moved
            } else {
                BlockState::from(BlockKind::MovingPiston)
            });
        }
        match self.vacated.get(&pos) {
            Some(vacated) => Some(self.told(&pos, vacated.since).unwrap_or(BlockState::AIR)),
            None => self.told(&pos, 0),
        }
    }

    fn moving_at(&self, pos: [i32; 3]) -> Option<Moving> {
        self.live
            .get(&pos)
            .filter(|live| !live.settled && !self.is_answered(&pos, live))
            .map(|live| live.moving)
    }

    fn near(&mut self, reach: &Aabb) -> Near {
        let reach = reach.inflate_all(2.0);
        let inside = |pos: &[i32; 3]| {
            let at = Vec3::new(f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2]));
            (reach.min.x..=reach.max.x).contains(&at.x)
                && (reach.min.y..=reach.max.y).contains(&at.y)
                && (reach.min.z..=reach.max.z).contains(&at.z)
        };
        let mut near = Near::default();
        for (pos, live) in &mut self.live {
            if inside(pos) {
                live.shapes(*pos);
                near.live.push(*pos);
            }
        }
        near.vacated.extend(
            self.vacated
                .keys()
                .filter(|pos| inside(pos) && !self.live.contains_key(*pos)),
        );
        near
    }

    fn moving_blocks(&self, near: &Near, noclip: Option<Direction>) -> MovingBlocks {
        let at = |pos: &[i32; 3]| BlockPos::new(pos[0], pos[1], pos[2]);
        let mut out = MovingBlocks::default();
        for pos in &near.live {
            let Some(live) = self.live.get(pos) else {
                continue;
            };
            out.cells.insert(at(pos));
            if let Some(shapes) = &live.shapes {
                let progress = live.progress.progress;
                shapes.solid(*pos, &live.moving, progress, noclip, &mut out.boxes);
            }
        }
        out.cells.extend(near.vacated.iter().map(at));
        out
    }
}

#[derive(Default)]
pub struct Near {
    live: Vec<[i32; 3]>,
    vacated: Vec<[i32; 3]>,
}

pub fn reach(bounds: &Aabb, velocity: Vec3) -> Aabb {
    let fastest = velocity.x.abs().max(velocity.y.abs()).max(velocity.z.abs());
    bounds.inflate_all(NEAR_MARGIN + 2.0 * fastest)
}

pub fn moving_blocks(movers: &[Aabb]) -> Option<Vec<MovingBlocks>> {
    {
        let table = &shared().lock().table;
        if table.live.is_empty() && table.vacated.is_empty() {
            return None;
        }
    }
    let (mut table, generation) = checkout();
    let blocks = movers
        .iter()
        .map(|reach| {
            let near = table.near(reach);
            table.moving_blocks(&near, None)
        })
        .collect();
    checkin(table, generation);
    Some(blocks)
}

pub fn on_server_updates(updates: impl Iterator<Item = (BlockPos, BlockState)>) {
    let mut shared = shared().lock();
    for (pos, state) in updates {
        shared.table.server_update([pos.x, pos.y, pos.z], state);
    }
}

pub fn on_ack() {
    shared().lock().table.note_ack();
}

pub fn on_updates_written(is_predicting: impl Fn([i32; 3]) -> bool) {
    let mut shared = shared().lock();
    let table = &mut shared.table;
    if !table.updates.is_empty() || !table.fresh.is_empty() {
        table.written(is_predicting);
    }
}

pub fn on_acks_handled(is_predicting: impl Fn([i32; 3]) -> bool) {
    shared().lock().table.acknowledged(is_predicting);
}

pub fn on_block_event(
    world: &parking_lot::RwLock<azalea_world::World>,
    pos: [i32; 3],
    action: u8,
    param: u8,
) -> Vec<([i32; 3], u32)> {
    let (mut table, generation) = checkout();
    table.index_updates();
    let event = block_event(&world.read(), &table, pos, action, param);
    let mut destroyed = Vec::new();
    if let Some(event) = event
        && table.make_room()
    {
        for pos in event.settled {
            table.settle(pos);
        }
        for pos in event.removed {
            table.remove(pos);
        }
        for created in event.created {
            table.insert(created);
        }
        for (cell, drawn) in event.emptied {
            table.next_seq += 1;
            let seq = table.next_seq;
            table.vacate(cell, seq, drawn);
        }
        if let Some(base) = event.base {
            let pos = base.pos;
            table.insert(base);
            table.settle(pos);
        }
        destroyed = event.destroyed;
    }
    checkin(table, generation);
    destroyed
}

pub fn drop_column(chunk_x: i32, chunk_z: i32) {
    let inside = |pos: &[i32; 3]| pos[0] >> 4 == chunk_x && pos[2] >> 4 == chunk_z;
    let mut shared = shared().lock();
    let table = &mut shared.table;
    let gone: Vec<[i32; 3]> = table.live.keys().copied().filter(inside).collect();
    for pos in gone {
        table.remove(pos);
    }
    let Table {
        vacated,
        leaving,
        touched_cells,
        touched_moves,
        redraw,
        ..
    } = table;
    vacated.retain(|pos, _| {
        if inside(pos) {
            touched_cells.push(*pos);
        }
        !inside(pos)
    });
    leaving.retain(|pos, _| {
        if inside(pos) {
            touched_moves.push(*pos);
            *redraw = true;
        }
        !inside(pos)
    });
    table.updates.retain(|(_, pos, _)| !inside(pos));
    table.known.retain(|pos, _| !inside(pos));
    table.indexed = 0;
}

pub fn reset() {
    {
        let mut shared = shared().lock();
        shared.table = Table::default();
        shared.generation += 1;
    }
    *drawn_cell().lock() = Drawn::default();
    crate::client::worldsync::hidden::reset();
}

#[derive(Clone, Copy, Debug)]
pub struct MovingCopy {
    pub moving: Moving,
    progress: Progress,
    last: Progress,
    pub skip_base: bool,
}

#[derive(Clone, Default)]
pub struct Drawn {
    pub all: Arc<FastMap<[i32; 3], MovingCopy>>,
    before: u64,
}

impl Drawn {
    pub fn progress(&self, copy: &MovingCopy, client_ticks: u64) -> Progress {
        if client_ticks == self.before {
            copy.last
        } else {
            copy.progress
        }
    }
}

crate::util::global!(fn drawn_cell() -> parking_lot::Mutex<Drawn>);

pub fn drawn() -> Drawn {
    drawn_cell().lock().clone()
}

#[derive(Default)]
pub struct Delta {
    pub cells: Vec<([i32; 3], Option<u64>)>,
    pub moves: Vec<([i32; 3], MoveState)>,
}

impl Delta {
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty() && self.moves.is_empty()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MoveState {
    Live {
        seq: u64,
        cells: [Option<[i32; 3]>; 2],
    },
    Left {
        seq: u64,
    },
    Gone,
}

pub fn publish(client_ticks: u64) {
    let (delta, copies) = {
        let mut shared = shared().lock();
        let table = &mut shared.table;
        let Table {
            leaving,
            touched_moves,
            redraw,
            ..
        } = &mut *table;
        leaving.retain(|pos, left| {
            left.ticks += 1;
            let keep = left.ticks <= LEAVING_TICKS;
            if !keep {
                touched_moves.push(*pos);
                *redraw = true;
            }
            keep
        });
        (
            table.delta(),
            table.redraw.then(|| Arc::new(table.copies())),
        )
    };
    if let Some(all) = copies {
        let drawn = Drawn {
            all,
            before: client_ticks,
        };
        let _old = std::mem::replace(&mut *drawn_cell().lock(), drawn);
    }
    let gone = crate::client::worldsync::hidden::sync(&delta);
    if gone.is_empty() {
        return;
    }
    let mut shared = shared().lock();
    let table = &mut shared.table;
    for pos in gone {
        if table.leaving.remove(&pos).is_some() {
            table.touched_moves.push(pos);
            table.redraw = true;
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Step {
    pub pos: [i32; 3],
    pub moving: Moving,
    pub progress: f32,
}

impl Step {
    pub const DELTA: f64 = 0.5;

    pub fn place(&self, aabb: &Aabb) -> Aabb {
        aabb.move_relative(self.moving.corner_at(self.pos, self.progress))
    }

    pub fn can_reach(&self, bounds: &Aabb) -> bool {
        let at = BlockPos::new(self.pos[0], self.pos[1], self.pos[2]).to_vec3_floored();
        let cell = Aabb {
            min: at,
            max: at + Vec3::new(1.0, 1.5, 1.0),
        };
        cell.inflate_all(1.6).intersects_aabb(bounds)
    }
}

pub struct Ticking {
    table: Table,
    generation: u64,
}

pub fn begin_tick(runs: bool) -> Option<Ticking> {
    let mut shared = shared().lock();
    shared.table.frozen = !runs;
    if !runs || shared.table.order.is_empty() {
        return None;
    }
    Some(Ticking {
        table: std::mem::take(&mut shared.table),
        generation: shared.generation,
    })
}

impl Ticking {
    pub fn order(&self) -> Vec<(u64, [i32; 3])> {
        self.table.order.clone()
    }

    pub fn step(&mut self, seq: u64, pos: [i32; 3]) -> Option<Step> {
        let live = self.table.live.get_mut(&pos)?;
        if live.seq != seq || live.settled {
            return None;
        }
        if live.progress.progress < 1.0 {
            return Some(Step {
                pos,
                moving: live.moving,
                progress: live.progress.progress,
            });
        }
        self.store(pos);
        let live = self.table.live.get_mut(&pos)?;
        if live.death_ticks < DEATH_TICKS {
            live.death_ticks += 1;
        } else {
            self.table.settle(pos);
        }
        None
    }

    pub fn store(&mut self, pos: [i32; 3]) {
        if let Some(live) = self.table.live.get_mut(&pos) {
            let before = live.progress;
            live.progress.tick();
            self.table.redraw |= live.progress != before;
        }
    }

    pub fn near(&mut self, reach: &Aabb) -> Near {
        self.table.near(reach)
    }

    pub fn pushing(&mut self, step: &Step) -> Vec<Aabb> {
        self.table
            .live
            .get_mut(&step.pos)
            .map_or_else(Vec::new, |live| {
                live.shapes(step.pos)
                    .pushing(&step.moving, step.progress)
                    .to_vec()
            })
    }

    pub fn moved(&mut self, step: &Step) -> Vec<Aabb> {
        self.table
            .live
            .get_mut(&step.pos)
            .map_or_else(Vec::new, |live| live.shapes(step.pos).moved.clone())
    }

    pub fn moving_blocks(&self, near: &Near, noclip: Option<Direction>) -> MovingBlocks {
        self.table.moving_blocks(near, noclip)
    }

    pub fn finish(mut self) {
        self.table.prune_order();
        checkin(self.table, self.generation);
    }
}

#[derive(Default)]
struct Event {
    settled: Vec<[i32; 3]>,
    removed: Vec<[i32; 3]>,
    created: Vec<Created>,
    emptied: Vec<([i32; 3], bool)>,
    base: Option<Created>,
    destroyed: Vec<([i32; 3], u32)>,
}

struct Created {
    pos: [i32; 3],
    moving: Moving,
    vacated: Option<([i32; 3], bool)>,
    covers: bool,
}

#[derive(Default)]
struct Moved {
    created: Vec<([i32; 3], Moving)>,
    destroyed: Vec<[i32; 3]>,
}

fn block_event(
    world: &azalea_world::World,
    table: &Table,
    pos: [i32; 3],
    action: u8,
    param: u8,
) -> Option<Event> {
    let world_at = |p: [i32; 3]| {
        world
            .get_block_state(BlockPos::new(p[0], p[1], p[2]))
            .unwrap_or_default()
    };
    let state_at = |p: [i32; 3]| table.state(p).unwrap_or_else(|| world_at(p));
    let terrain_at = |p: [i32; 3]| table.told(&p, 0).unwrap_or_else(|| world_at(p));
    let drawn = |p: [i32; 3]| {
        let kind = BlockKind::from(terrain_at(p));
        !is_air(kind) && kind != BlockKind::MovingPiston
    };
    let state = state_at(pos);
    let kind = BlockKind::from(state);
    if !is_piston(kind) || action > 2 {
        return None;
    }
    let direction = Box::<dyn BlockTrait>::from(state)
        .get_property("facing")
        .and_then(facing::from_name)?;
    let min_y = world.chunks.min_y();
    let retracting = action != 0;
    let arm = relative(pos, direction, 1);
    let two = relative(pos, direction, 2);
    let at_arm = table.moving_at(arm).filter(|_| retracting);
    let head_at_arm = match at_arm {
        Some(moving) => moving.source,
        None => BlockKind::from(state_at(arm)) == BlockKind::PistonHead,
    };
    let piston_piece = retracting
        && kind == BlockKind::StickyPiston
        && table
            .moving_at(two)
            .is_some_and(|moving| moving.extending && moving.direction == direction);
    let moving_piston = BlockState::from(BlockKind::MovingPiston);
    let level = Level {
        get: |p: [i32; 3]| {
            if retracting && p == pos {
                moving_piston
            } else if retracting && p == arm {
                match at_arm {
                    _ if head_at_arm => BlockState::AIR,
                    Some(moving) => moving.moved,
                    None => state_at(p),
                }
            } else {
                state_at(p)
            }
        },
        min_y,
        max_y: min_y + world.chunks.height() as i32 - 1,
    };
    let Moved { created, destroyed } =
        trigger_event(&level, state, pos, direction, action, param, piston_piece);

    let mut event = Event {
        destroyed: destroyed
            .into_iter()
            .map(|p| (p, level.state(p)))
            .filter(|(_, state)| !FIRE.contains(&BlockKind::from(*state)))
            .map(|(p, state)| (p, id(state)))
            .collect(),
        ..Event::default()
    };
    if action == 0 && !created.is_empty() {
        let extended = with(state, &[("extended", "true")]);
        event.base = Some(Created {
            pos,
            moving: Moving::new(extended, direction, true, false),
            vacated: None,
            covers: terrain_at(pos) != extended,
        });
    }
    let pulled = created.len() > 1;
    let arm_emptied = match at_arm {
        Some(moving) if moving.source => true,
        _ => !piston_piece && (!pulled || head_at_arm),
    };
    if at_arm.is_some() {
        if arm_emptied {
            event.removed.push(arm);
        } else {
            event.settled.push(arm);
        }
    }
    if piston_piece {
        event.settled.push(two);
    }
    for (p, moving) in created {
        let vacated = match (moving.source, moving.extending) {
            (true, true) => None,
            (true, false) => arm_emptied.then_some(arm),
            (false, _) => Some(relative(p, moving.movement(), -1)),
        };
        let vacated = vacated.map(|cell| (cell, drawn(cell)));
        if world
            .get_block_state(BlockPos::new(p[0], p[1], p[2]))
            .is_none()
        {
            event.emptied.extend(vacated);
            continue;
        }
        let covers = match moving.look {
            Look::Retracting { base, .. } => id(terrain_at(p)) != base,
            _ => drawn(p),
        };
        event.created.push(Created {
            pos: p,
            moving,
            vacated,
            covers,
        });
    }
    Some(event)
}

fn trigger_event<F: Fn([i32; 3]) -> BlockState>(
    level: &Level<F>,
    state: BlockState,
    pos: [i32; 3],
    direction: Direction,
    action: u8,
    param: u8,
    piston_piece: bool,
) -> Moved {
    let mut out = Moved::default();
    let kind = BlockKind::from(state);
    let sticky = match kind {
        BlockKind::Piston => false,
        BlockKind::StickyPiston => true,
        _ => return out,
    };
    match action {
        0 => move_blocks(level, pos, direction, true, sticky, &mut out),
        1 | 2 => {
            let base = facing::from_3d_data_value(i32::from(param & 7));
            let moved = with(BlockState::from(kind), &[("facing", facing::name(base))]);
            out.created
                .push((pos, Moving::new(moved, direction, false, true)));
            if sticky && !piston_piece {
                let two = relative(pos, direction, 2);
                let moving = level.state(two);
                let moving_kind = BlockKind::from(moving);
                let pulls = action == 1
                    && !is_air(moving_kind)
                    && level.is_pushable(moving, two, direction.opposite(), false, direction)
                    && (reaction(moving) == PushReaction::Normal || is_piston(moving_kind));
                if pulls {
                    move_blocks(level, pos, direction, false, sticky, &mut out);
                }
            }
        }
        _ => {}
    }
    out
}

fn move_blocks<F: Fn([i32; 3]) -> BlockState>(
    level: &Level<F>,
    piston: [i32; 3],
    direction: Direction,
    extending: bool,
    sticky: bool,
    out: &mut Moved,
) {
    let Some((to_push, to_destroy)) = resolve(level, piston, direction, extending) else {
        return;
    };
    let push = if extending {
        direction
    } else {
        direction.opposite()
    };
    out.destroyed.extend(to_destroy.into_iter().rev());
    out.created.extend(to_push.iter().rev().map(|&p| {
        (
            relative(p, push, 1),
            Moving::new(level.state(p), direction, extending, false),
        )
    }));
    if extending {
        let head = with(
            BlockState::from(BlockKind::PistonHead),
            &[
                ("facing", facing::name(direction)),
                ("type", piston_type(sticky)),
            ],
        );
        out.created.push((
            relative(piston, direction, 1),
            Moving::new(head, direction, true, true),
        ));
    }
}

#[allow(clippy::type_complexity)]
fn resolve<F: Fn([i32; 3]) -> BlockState>(
    level: &Level<F>,
    piston: [i32; 3],
    direction: Direction,
    extending: bool,
) -> Option<(Vec<[i32; 3]>, Vec<[i32; 3]>)> {
    let (push, start) = if extending {
        (direction, relative(piston, direction, 1))
    } else {
        (direction.opposite(), relative(piston, direction, 2))
    };
    let mut resolver = Resolver {
        level,
        piston,
        push,
        to_push: Vec::new(),
        to_destroy: Vec::new(),
    };
    let next = level.state(start);
    if !level.is_pushable(next, start, push, false, direction) {
        return (extending && reaction(next) == PushReaction::Destroy)
            .then(|| (Vec::new(), vec![start]));
    }
    if !resolver.add_block_line(start, push) {
        return None;
    }
    let mut i = 0;
    while i < resolver.to_push.len() {
        let pos = resolver.to_push[i];
        if is_sticky(level.state(pos)) && !resolver.add_branching_blocks(pos) {
            return None;
        }
        i += 1;
    }
    Some((resolver.to_push, resolver.to_destroy))
}

struct Level<F> {
    get: F,
    min_y: i32,
    max_y: i32,
}

impl<F: Fn([i32; 3]) -> BlockState> Level<F> {
    fn state(&self, pos: [i32; 3]) -> BlockState {
        (self.get)(pos)
    }

    fn is_pushable(
        &self,
        state: BlockState,
        pos: [i32; 3],
        direction: Direction,
        allow_destroyable: bool,
        connection: Direction,
    ) -> bool {
        if pos[1] < self.min_y || pos[1] > self.max_y {
            return false;
        }
        let kind = BlockKind::from(state);
        if is_air(kind) {
            return true;
        }
        if matches!(
            kind,
            BlockKind::Obsidian
                | BlockKind::CryingObsidian
                | BlockKind::RespawnAnchor
                | BlockKind::ReinforcedDeepslate
        ) {
            return false;
        }
        if (direction == Direction::Down && pos[1] == self.min_y)
            || (direction == Direction::Up && pos[1] == self.max_y)
        {
            return false;
        }
        let block = Box::<dyn BlockTrait>::from(state);
        if is_piston(kind) {
            if block.get_property("extended") == Some("true") {
                return false;
            }
        } else {
            if block.behavior().destroy_time == -1.0 {
                return false;
            }
            match push_reaction(block.id()) {
                PushReaction::Block => return false,
                PushReaction::Destroy => return allow_destroyable,
                PushReaction::PushOnly => return direction == connection,
                PushReaction::Normal => {}
            }
        }
        !has_block_entity(block.id())
    }
}

struct Resolver<'a, F> {
    level: &'a Level<F>,
    piston: [i32; 3],
    push: Direction,
    to_push: Vec<[i32; 3]>,
    to_destroy: Vec<[i32; 3]>,
}

impl<F: Fn([i32; 3]) -> BlockState> Resolver<'_, F> {
    fn add_block_line(&mut self, start: [i32; 3], direction: Direction) -> bool {
        let level = self.level;
        let mut next = level.state(start);
        if is_air(BlockKind::from(next))
            || !level.is_pushable(next, start, self.push, false, direction)
            || start == self.piston
            || self.to_push.contains(&start)
        {
            return true;
        }
        let back = self.push.opposite();
        let mut count = 1;
        if count + self.to_push.len() > MAX_PUSH_DEPTH {
            return false;
        }
        while is_sticky(next) {
            let pos = relative(start, back, count as i32);
            let previous = next;
            next = level.state(pos);
            if is_air(BlockKind::from(next))
                || !can_stick_to_each_other(previous, next)
                || !level.is_pushable(next, pos, self.push, false, back)
                || pos == self.piston
            {
                break;
            }
            count += 1;
            if count + self.to_push.len() > MAX_PUSH_DEPTH {
                return false;
            }
        }
        self.to_push
            .extend((0..count).rev().map(|i| relative(start, back, i as i32)));
        let mut added = count;
        let mut i = 1;
        loop {
            let pos = relative(start, self.push, i);
            if let Some(collision) = self.to_push.iter().position(|p| *p == pos) {
                self.to_push[collision..].rotate_right(added);
                for j in 0..=collision + added {
                    let at = self.to_push[j];
                    if is_sticky(level.state(at)) && !self.add_branching_blocks(at) {
                        return false;
                    }
                }
                return true;
            }
            next = level.state(pos);
            if is_air(BlockKind::from(next)) {
                return true;
            }
            if !level.is_pushable(next, pos, self.push, true, self.push) || pos == self.piston {
                return false;
            }
            if reaction(next) == PushReaction::Destroy {
                self.to_destroy.push(pos);
                return true;
            }
            if self.to_push.len() >= MAX_PUSH_DEPTH {
                return false;
            }
            self.to_push.push(pos);
            added += 1;
            i += 1;
        }
    }

    fn add_branching_blocks(&mut self, from: [i32; 3]) -> bool {
        let from_state = self.level.state(from);
        for direction in crate::blocks::direction::Direction::ALL.map(Direction::from) {
            if direction == self.push || direction == self.push.opposite() {
                continue;
            }
            let neighbour = relative(from, direction, 1);
            if can_stick_to_each_other(self.level.state(neighbour), from_state)
                && !self.add_block_line(neighbour, direction)
            {
                return false;
            }
        }
        true
    }
}

fn reaction(state: BlockState) -> PushReaction {
    push_reaction(crate::blocks::block_name(state))
}

fn is_piston(kind: BlockKind) -> bool {
    matches!(kind, BlockKind::Piston | BlockKind::StickyPiston)
}

fn is_sticky(state: BlockState) -> bool {
    matches!(
        BlockKind::from(state),
        BlockKind::SlimeBlock | BlockKind::HoneyBlock
    )
}

fn can_stick_to_each_other(a: BlockState, b: BlockState) -> bool {
    match (BlockKind::from(a), BlockKind::from(b)) {
        (BlockKind::HoneyBlock, BlockKind::SlimeBlock)
        | (BlockKind::SlimeBlock, BlockKind::HoneyBlock) => false,
        _ => is_sticky(a) || is_sticky(b),
    }
}

fn piston_type(sticky: bool) -> &'static str {
    if sticky { "sticky" } else { "normal" }
}

fn relative(pos: [i32; 3], direction: Direction, n: i32) -> [i32; 3] {
    [
        pos[0] + direction.x() * n,
        pos[1] + direction.y() * n,
        pos[2] + direction.z() * n,
    ]
}

fn with(state: BlockState, props: &[(&str, &str)]) -> BlockState {
    let mut block = Box::<dyn BlockTrait>::from(state);
    for (name, value) in props {
        let _ = block.set_property(name, value);
    }
    block.as_block_state()
}

fn id(state: BlockState) -> u32 {
    u32::from(state)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    const PISTON: [i32; 3] = [0, 64, 0];

    fn block(kind: BlockKind) -> BlockState {
        BlockState::from(kind)
    }

    fn resolve_in(
        blocks: &[([i32; 3], BlockState)],
        direction: Direction,
        extending: bool,
    ) -> Option<Vec<[i32; 3]>> {
        let map: HashMap<[i32; 3], BlockState> = blocks.iter().copied().collect();
        let level = Level {
            get: |p: [i32; 3]| map.get(&p).copied().unwrap_or_default(),
            min_y: -64,
            max_y: 319,
        };
        resolve(&level, PISTON, direction, extending).map(|(to_push, _)| to_push)
    }

    fn line(n: i32, kind: BlockKind) -> Vec<([i32; 3], BlockState)> {
        (1..=n).map(|i| ([i, 64, 0], block(kind))).collect()
    }

    #[test]
    fn a_line_is_pushed_nearest_first() {
        let pushed = resolve_in(&line(3, BlockKind::Stone), Direction::East, true);
        assert_eq!(pushed, Some(vec![[1, 64, 0], [2, 64, 0], [3, 64, 0]]));
    }

    #[test]
    fn twelve_blocks_push_and_thirteen_do_not() {
        let twelve = resolve_in(&line(12, BlockKind::Stone), Direction::East, true);
        assert_eq!(twelve.map(|v| v.len()), Some(12));
        assert_eq!(
            resolve_in(&line(13, BlockKind::Stone), Direction::East, true),
            None
        );
    }

    #[test]
    fn obsidian_and_block_entities_stop_the_push() {
        let mut blocks = line(2, BlockKind::Stone);
        blocks.push(([3, 64, 0], block(BlockKind::Obsidian)));
        assert_eq!(resolve_in(&blocks, Direction::East, true), None);
        blocks[2].1 = block(BlockKind::Chest);
        assert_eq!(resolve_in(&blocks, Direction::East, true), None);
    }

    #[test]
    fn a_destroyable_block_ends_the_line() {
        let mut blocks = line(2, BlockKind::Stone);
        blocks.push(([3, 64, 0], block(BlockKind::Torch)));
        let pushed = resolve_in(&blocks, Direction::East, true);
        assert_eq!(pushed, Some(vec![[1, 64, 0], [2, 64, 0]]));
        let lone = resolve_in(
            &[([1, 64, 0], block(BlockKind::Torch))],
            Direction::East,
            true,
        );
        assert_eq!(lone, Some(Vec::new()));
    }

    #[test]
    fn slime_drags_its_neighbours() {
        let blocks = [
            ([1, 64, 0], block(BlockKind::SlimeBlock)),
            ([1, 65, 0], block(BlockKind::Stone)),
            ([1, 64, 1], block(BlockKind::Stone)),
            ([1, 63, 0], block(BlockKind::Obsidian)),
        ];
        let pushed = resolve_in(&blocks, Direction::East, true).unwrap();
        assert_eq!(pushed.len(), 3);
        assert_eq!(pushed[0], [1, 64, 0]);
        assert!(pushed.contains(&[1, 65, 0]));
        assert!(pushed.contains(&[1, 64, 1]));
    }

    #[test]
    fn honey_does_not_stick_to_slime() {
        let blocks = [
            ([1, 64, 0], block(BlockKind::SlimeBlock)),
            ([1, 65, 0], block(BlockKind::HoneyBlock)),
        ];
        let pushed = resolve_in(&blocks, Direction::East, true);
        assert_eq!(pushed, Some(vec![[1, 64, 0]]));
    }

    #[test]
    fn a_sticky_retract_pulls_toward_the_piston() {
        let blocks = [([2, 64, 0], block(BlockKind::Stone))];
        let pulled = resolve_in(&blocks, Direction::East, false);
        assert_eq!(pulled, Some(vec![[2, 64, 0]]));
    }

    #[test]
    fn extending_moves_the_line_and_adds_the_head() {
        let mut blocks = line(2, BlockKind::Stone);
        blocks.push(([3, 64, 0], block(BlockKind::Torch)));
        let map: HashMap<[i32; 3], BlockState> = blocks.into_iter().collect();
        let level = Level {
            get: |p: [i32; 3]| map.get(&p).copied().unwrap_or_default(),
            min_y: -64,
            max_y: 319,
        };
        let piston = with(block(BlockKind::Piston), &[("facing", "east")]);
        let out = trigger_event(&level, piston, PISTON, Direction::East, 0, 5, false);
        assert_eq!(out.destroyed, vec![[3, 64, 0]]);
        let out = out.created;
        let positions: Vec<[i32; 3]> = out.iter().map(|(p, _)| *p).collect();
        assert_eq!(positions, vec![[3, 64, 0], [2, 64, 0], [1, 64, 0]]);
        assert!(matches!(out[2].1.look, Look::Head { .. }));
        assert!(
            out.iter()
                .all(|(_, m)| m.extending && m.direction == Direction::East)
        );
    }

    fn pushed_stone() -> Created {
        Created {
            pos: [2, 64, 0],
            moving: Moving::new(block(BlockKind::Stone), Direction::East, true, false),
            vacated: Some(([1, 64, 0], true)),
            covers: false,
        }
    }

    fn everywhere() -> Aabb {
        Aabb {
            min: Vec3::new(-64.0, 0.0, -64.0),
            max: Vec3::new(64.0, 128.0, 64.0),
        }
    }

    fn write(table: &mut Table, pos: [i32; 3], state: BlockState) {
        table.server_update(pos, state);
        table.written(|_| false);
        table.acknowledged(|_| false);
    }

    #[test]
    fn an_update_answers_the_move_before_it_only() {
        let pos = [2, 64, 0];
        let mut table = Table::default();
        table.insert(pushed_stone());
        table.server_update(pos, block(BlockKind::Stone));
        table.index_updates();
        assert_eq!(table.state(pos), Some(block(BlockKind::Stone)));
        assert!(table.moving_at(pos).is_none());
        assert!(table.live.contains_key(&pos));
        table.insert(pushed_stone());
        assert!(table.moving_at(pos).is_some());
        assert_eq!(table.state([1, 64, 0]), Some(BlockState::AIR));
        table.written(|_| false);
        assert!(table.moving_at(pos).is_some());
        assert!(table.vacated.contains_key(&[1, 64, 0]));
    }

    #[test]
    fn a_held_update_ends_the_copy_at_the_acknowledgement() {
        let (origin, pos) = ([1, 64, 0], [2, 64, 0]);
        let mut table = Table::default();
        table.insert(pushed_stone());
        table.copies();
        table.written(|_| false);
        table.server_update(origin, BlockState::AIR);
        table.server_update(pos, block(BlockKind::Stone));
        table.written(|_| true);
        table.acknowledged(|_| true);
        assert!(table.moving_at(pos).is_some());
        assert_eq!(table.state(origin), Some(BlockState::AIR));
        assert!(table.leaving.is_empty());
        table.note_ack();
        table.acknowledged(|p| p == origin);
        assert!(table.live.is_empty() && table.leaving.contains_key(&pos));
        assert_eq!(table.state(origin), Some(BlockState::AIR));
        table.note_ack();
        table.acknowledged(|_| false);
        assert!(table.vacated.is_empty() && table.held.is_empty());
    }

    #[test]
    fn a_rolled_back_change_ends_the_move_made_over_it() {
        let (origin, pos) = ([1, 64, 0], [2, 64, 0]);
        let mut table = Table::default();
        table.insert(pushed_stone());
        table.written(|p| p == pos);
        table.acknowledged(|p| p == pos);
        assert!(table.moving_at(pos).is_some());
        table.note_ack();
        table.acknowledged(|_| false);
        assert!(table.live.is_empty() && table.held.is_empty());
        assert_eq!(table.state(origin), Some(BlockState::AIR));
    }

    #[test]
    fn an_acknowledgement_before_the_move_does_not_end_it() {
        let pos = [2, 64, 0];
        let mut table = Table::default();
        table.note_ack();
        table.insert(pushed_stone());
        table.written(|p| p == pos);
        table.acknowledged(|_| false);
        assert!(table.moving_at(pos).is_some() && table.held.is_empty());
    }

    #[test]
    fn a_frozen_table_is_bounded() {
        let mut table = Table {
            frozen: true,
            ..Table::default()
        };
        for _ in 0..1000 {
            table.insert(pushed_stone());
        }
        assert!(table.order.len() <= 2 * table.live.len() + 64);
        assert!(table.make_room());
        for x in 0..(MAX_LIVE - MAX_EVENT) as i32 {
            table.insert(Created {
                pos: [x, 70, 0],
                vacated: None,
                ..pushed_stone()
            });
        }
        let full = table.live.len();
        assert!(!table.make_room());
        for x in 0..full as i32 / 2 {
            table.settle([x, 70, 0]);
        }
        assert!(!table.make_room());
        assert_eq!(table.live.len(), full);
        table.frozen = false;
        assert!(table.make_room());
        assert!(table.live.len() < full && !table.live.contains_key(&[0, 70, 0]));
        assert_eq!(
            table.resting,
            table.live.values().filter(|live| live.settled).count()
        );
    }

    #[test]
    fn a_moving_block_covers_its_cell_and_empties_its_origin() {
        let mut table = Table::default();
        table.insert(pushed_stone());
        assert_eq!(
            table.state([2, 64, 0]),
            Some(block(BlockKind::MovingPiston))
        );
        assert_eq!(table.state([1, 64, 0]), Some(BlockState::AIR));
        write(&mut table, [2, 64, 0], block(BlockKind::MovingPiston));
        assert!(table.moving_at([2, 64, 0]).is_some());
        write(&mut table, [2, 64, 0], block(BlockKind::Stone));
        assert!(table.moving_at([2, 64, 0]).is_none());
        assert_eq!(table.state([1, 64, 0]), Some(BlockState::AIR));
        write(&mut table, [1, 64, 0], BlockState::AIR);
        assert!(table.vacated.is_empty());
    }

    #[test]
    fn a_settled_block_is_the_block_it_carried() {
        let mut table = Table::default();
        table.insert(pushed_stone());
        table.settle([2, 64, 0]);
        assert!(table.moving_at([2, 64, 0]).is_none());
        assert_eq!(table.state([2, 64, 0]), Some(block(BlockKind::Stone)));
    }

    #[test]
    fn a_moving_block_rests_where_it_ends_until_the_server_writes_it() {
        let pos = [2, 64, 0];
        let mut table = Table::default();
        table.insert(pushed_stone());
        let mut ticking = Ticking {
            table,
            generation: 0,
        };
        let seq = ticking.order()[0].0;
        for progress in [0.0, 0.5] {
            assert_eq!(ticking.step(seq, pos).map(|s| s.progress), Some(progress));
            ticking.store(pos);
        }
        for _ in 0..DEATH_TICKS {
            assert!(ticking.step(seq, pos).is_none());
            assert!(ticking.table.moving_at(pos).is_some());
        }
        assert!(ticking.step(seq, pos).is_none());
        assert!(ticking.table.moving_at(pos).is_none());
        let mut table = ticking.table;
        assert_eq!(table.state(pos), Some(block(BlockKind::Stone)));
        assert_eq!(table.state([1, 64, 0]), Some(BlockState::AIR));
        let near = table.near(&everywhere());
        let blocks = table.moving_blocks(&near, Some(Direction::East));
        assert_eq!((blocks.cells.len(), blocks.boxes.len()), (2, 1));
        write(&mut table, pos, block(BlockKind::Stone));
        assert_eq!(table.state(pos), None);
    }

    #[test]
    fn a_delta_names_the_newest_copy_of_each_cell() {
        let mut table = Table::default();
        table.insert(pushed_stone());
        assert_eq!(
            table.delta().cells,
            vec![([1, 64, 0], Some(1)), ([2, 64, 0], None)]
        );
        table.copies();
        table.insert(Created {
            pos: [1, 64, 0],
            moving: Moving::new(block(BlockKind::Stone), Direction::East, true, false),
            vacated: None,
            covers: true,
        });
        assert_eq!(table.delta().cells, vec![([1, 64, 0], Some(2))]);
        assert!(table.delta().is_empty());
        write(&mut table, [2, 64, 0], block(BlockKind::Stone));
        let delta = table.delta();
        assert_eq!(delta.cells, vec![([2, 64, 0], None)]);
        assert!(matches!(delta.moves[..], [(_, MoveState::Left { seq: 1 })]));
    }

    #[test]
    fn an_origin_alone_is_empty_until_the_server_writes_it() {
        let mut table = Table::default();
        table.vacate([15, 64, 0], 1, true);
        assert_eq!(table.state([15, 64, 0]), Some(BlockState::AIR));
        assert_eq!(table.hides(&[15, 64, 0]), Some(1));
        write(&mut table, [15, 64, 0], BlockState::AIR);
        assert_eq!(table.hides(&[15, 64, 0]), None);
    }

    #[test]
    fn the_shape_follows_the_block_and_steps_aside_for_its_push() {
        let mut table = Table::default();
        table.insert(pushed_stone());
        table.live.get_mut(&[2, 64, 0]).unwrap().progress.progress = 0.5;
        let near = table.near(&everywhere());
        let blocks = table.moving_blocks(&near, None);
        assert!(blocks.covers(BlockPos::new(1, 64, 0)));
        assert!(blocks.covers(BlockPos::new(2, 64, 0)));
        let [(cell, shape)] = blocks.boxes[..] else {
            panic!("{:?}", blocks.boxes);
        };
        assert_eq!(cell, BlockPos::new(2, 64, 0));
        assert_eq!((shape.min.x, shape.max.x), (1.5, 2.5));
        let pushing = table.moving_blocks(&near, Some(Direction::East));
        assert!(pushing.boxes.is_empty());
        assert_eq!(pushing.cells.len(), 2);
        assert_eq!(
            table.moving_blocks(&near, Some(Direction::Up)).boxes.len(),
            1
        );
        let far = everywhere().move_relative(Vec3::new(0.0, 500.0, 0.0));
        let near = table.near(&far);
        assert!(table.moving_blocks(&near, None).is_empty());
    }

    #[test]
    fn a_retracting_base_collides_as_its_head() {
        let piston = with(block(BlockKind::StickyPiston), &[("facing", "west")]);
        let moving = Moving::new(piston, Direction::West, false, true);
        assert_eq!(moving.movement(), Direction::East);
        let shapes = Shapes::of(PISTON, &moving);
        let reach = |progress| {
            let boxes = shapes.pushing(&moving, progress);
            let min = boxes.iter().map(|b| b.min.x).fold(f64::MAX, f64::min);
            let max = boxes.iter().map(|b| b.max.x).fold(f64::MIN, f64::max);
            (min, max)
        };
        assert_eq!(reach(0.0), (0.0, 1.25));
        assert_eq!(reach(0.25), (0.0, 1.25));
        assert_eq!(reach(0.5), (0.0, 1.0));
        assert!(!shapes.base.is_empty());
    }

    #[test]
    fn progress_takes_two_ticks_and_holds_at_one() {
        let mut progress = Progress::default();
        progress.tick();
        assert_eq!((progress.progress_o, progress.progress), (0.0, 0.5));
        assert_eq!(progress.at(0.5), 0.25);
        progress.tick();
        progress.tick();
        assert_eq!((progress.progress_o, progress.progress), (1.0, 1.0));
        assert_eq!(progress.at(2.0), 1.0);
    }

    #[test]
    fn offsets_start_behind_and_end_at_the_block() {
        let extending = Moving {
            direction: Direction::Up,
            extending: true,
            ..Moving::default()
        };
        assert_eq!(extending.offset(0.0), [0.0, -1.0, 0.0]);
        assert_eq!(extending.offset(1.0), [0.0, 0.0, 0.0]);
        let retracting = Moving {
            extending: false,
            ..extending
        };
        assert_eq!(retracting.offset(0.0), [0.0, 1.0, 0.0]);
        assert_eq!(retracting.offset(0.75), [0.0, 0.25, 0.0]);
    }

    #[test]
    fn a_retracting_source_draws_its_head_and_extended_base() {
        let piston = with(block(BlockKind::StickyPiston), &[("facing", "west")]);
        let moving = Moving::new(piston, Direction::West, false, true);
        let Look::Retracting { long, short, base } = moving.look else {
            panic!("{:?}", moving.look);
        };
        let state = |id: u32| Box::<dyn BlockTrait>::from(BlockState::try_from(id).unwrap());
        assert_eq!(state(long).id(), "piston_head");
        assert_eq!(state(long).get_property("type"), Some("sticky"));
        assert_eq!(state(long).get_property("facing"), Some("west"));
        assert_eq!(state(short).get_property("short"), Some("true"));
        assert_eq!(state(base).get_property("extended"), Some("true"));
        let [head, base_draw] = moving.draws(0.75);
        assert_eq!(head.map(|d| d.0), Some(short));
        assert_eq!(base_draw, Some((base, [0.0; 3])));
    }
}
