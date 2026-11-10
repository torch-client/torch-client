use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering::Relaxed};
use std::sync::{Arc, Mutex, PoisonError};

use azalea::Client;
use azalea::block::{BlockState, BlockTrait};
use azalea::entity::inventory::Inventory;
use azalea::mining::MineBlockPos;
use azalea::pathfinder::player_pos_to_block_pos;
use azalea::pathfinder::world::{is_block_state_passable, is_block_state_standable};
use azalea::physics::clip::{BlockShapeType, ClipContext, FluidPickType, clip};
use azalea::physics::collision::BlockWithShape;
use azalea_core::hit_result::{BlockHitResult, HitResult};
use azalea_core::position::{BlockPos, Vec3};
use azalea_inventory::ItemStack;
use azalea_registry::builtin::BlockKind;

use super::{
    FluidRule, InvFull, ReachMode, STALL_TICKS, draw, exposes_fluid, hud, is_fluid, present,
};
use crate::modules::pathfinder::{self, Travel};
use crate::modules::registry::{Id, auto_mine as setting};
use crate::modules::{MS_PER_TICK, Store, roll, store};
use crate::session::{MineIntent, SharedMutex};

pub const MAX_VOLUME: i64 = 1 << 20;
pub const MAX_SIDE: i32 = 128;

const REACH: f64 = 4.2;

const EYE: f64 = 1.62;

const JITTER: f64 = 0.22;

const MAX_STALLS: u32 = 5;

const TRAVEL_TICKS: i64 = 600;
const TRAVEL_PROGRESS: i32 = 4;

const FIRST_TRIES: u32 = 3;

const SEARCH_BUDGET: usize = 4096;

pub struct Area {
    pub min: BlockPos,
    pub max: BlockPos,
    world: Option<azalea_world::WorldName>,
    stairs: HashSet<BlockPos>,
}

impl Area {
    pub fn volume(&self) -> i64 {
        let side = |a: i32, b: i32| i64::from(b - a) + 1;
        side(self.min.x, self.max.x) * side(self.min.y, self.max.y) * side(self.min.z, self.max.z)
    }
}

static AREA: Mutex<Option<Arc<Area>>> = Mutex::new(None);

static LIVE: AtomicBool = AtomicBool::new(false);
static BOUNDS: [AtomicI32; 6] = [const { AtomicI32::new(0) }; 6];

pub fn current() -> Option<Arc<Area>> {
    AREA.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

pub fn live() -> bool {
    LIVE.load(Relaxed)
}

pub fn define(bot: &Client, shared: &SharedMutex, a: BlockPos, b: BlockPos) {
    let (min, max) = (
        BlockPos::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z)),
        BlockPos::new(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z)),
    );
    let size = |a: i32, b: i32| b - a + 1;
    let (w, h, d) = (size(min.x, max.x), size(min.y, max.y), size(min.z, max.z));
    let volume = i64::from(w) * i64::from(h) * i64::from(d);
    if w > MAX_SIDE || d > MAX_SIDE || volume > MAX_VOLUME {
        say(
            shared,
            &format!(
                "AutoMine: that box is {w} x {h} x {d}; the most is {MAX_SIDE} on a side and {MAX_VOLUME} blocks. Define a smaller one"
            ),
        );
        return;
    }
    let near = bot
        .position()
        .map_or((min.x, min.z), |p| (p.x.floor() as i32, p.z.floor() as i32));
    let area = Area {
        min,
        max,
        world: bot
            .component::<azalea_world::WorldName>()
            .ok()
            .map(|n| n.clone()),
        stairs: stairs(min, max, near),
    };
    let narrow = area.stairs.is_empty();
    say(
        shared,
        &format!(
            "AutoMine: area {w} x {h} x {d} ({volume} blocks){}",
            if narrow {
                ", too narrow for an exit staircase"
            } else {
                ""
            },
        ),
    );
    for (slot, v) in BOUNDS
        .iter()
        .zip([min.x, min.y, min.z, max.x, max.y, max.z])
    {
        slot.store(v, Relaxed);
    }
    *AREA.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::new(area));
    LIVE.store(true, Relaxed);
    hud::invalidate();
    *RUN.lock().unwrap_or_else(PoisonError::into_inner) = None;
}

pub fn forget() {
    LIVE.store(false, Relaxed);
    *AREA.lock().unwrap_or_else(PoisonError::into_inner) = None;
    *RUN.lock().unwrap_or_else(PoisonError::into_inner) = None;
    hud::invalidate();
    draw::clear();
}

static NO_AREA_SAID: AtomicBool = AtomicBool::new(false);

pub fn stop_run(bot: &Client) {
    NO_AREA_SAID.store(false, Relaxed);
    let mut guard = RUN.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(run) = guard.as_mut() {
        pause(bot, run);
        run.arrived_once = false;
        run.failed_trips = 0;
        run.stalls = 0;
    }
}

fn pause(bot: &Client, run: &mut Run) {
    if let Stage::Travel { .. } = run.stage {
        pathfinder::cancel_quiet(bot);
    }
    run.stage = Stage::Choose;
    run.search = None;
}

pub fn block_changed(pos: BlockPos, old: BlockState, new: BlockState) {
    if !LIVE.load(Relaxed) {
        return;
    }
    let b: [i32; 6] = std::array::from_fn(|i| BOUNDS[i].load(Relaxed));
    if pos.x < b[0] || pos.y < b[1] || pos.z < b[2] || pos.x > b[3] || pos.y > b[4] || pos.z > b[5]
    {
        return;
    }
    let delta = i64::from(solid(new)) - i64::from(solid(old));
    if delta != 0 {
        hud::changed(delta);
    }
}

pub fn solid(state: BlockState) -> bool {
    !state.is_air() && !is_fluid(state)
}

static RUN: Mutex<Option<Run>> = Mutex::new(None);

struct Run {
    stage: Stage,
    top: i32,
    targets: Option<HashSet<BlockPos>>,
    unknown: bool,
    relisted: bool,
    skipped: HashSet<BlockPos>,
    bad: HashSet<BlockPos>,
    breakable: HashMap<u32, bool>,
    arrived_at: Option<BlockPos>,
    search: Option<i32>,
    arrived_once: bool,
    failed_trips: u32,
    stalls: u32,
    unreachable: u32,
    unloaded: u32,
}

#[derive(Clone, Copy)]
enum Stage {
    Choose,
    Travel {
        spot: BlockPos,
        until: i64,
        from: BlockPos,
    },
    Mine {
        pos: BlockPos,
        aim: [f64; 3],
        arm_at: i64,
        stall_at: i64,
        turn: f32,
    },
}

enum Next {
    Mine(Stage),
    Travel(BlockPos),
    Wait,
    Halt(Halt),
}

enum Halt {
    Done,
    Hazard,
    Stalled,
    Full,
    NotLoaded,
    NoRoute,
}

impl Run {
    fn new(area: &Area) -> Run {
        Run {
            stage: Stage::Choose,
            top: area.max.y,
            targets: None,
            unknown: false,
            relisted: false,
            skipped: HashSet::new(),
            bad: HashSet::new(),
            breakable: HashMap::new(),
            arrived_at: None,
            search: None,
            arrived_once: false,
            failed_trips: 0,
            stalls: 0,
            unreachable: 0,
            unloaded: 0,
        }
    }

    fn next_pair(&mut self) {
        self.targets = None;
        self.unknown = false;
        self.relisted = false;
        self.search = None;
        self.top -= 2;
        self.bad.clear();
    }
}

pub fn tick(bot: &Client, shared: &SharedMutex, hit: Option<&BlockHitResult>, s: &Store) -> bool {
    let Some(area) = current() else {
        if !NO_AREA_SAID.swap(true, Relaxed) {
            say(
                shared,
                "AutoMine: no area yet; press Define area and click two corners",
            );
        }
        draw::clear();
        super::clear(shared);
        return false;
    };
    let Ok(pos) = bot.position() else {
        super::clear(shared);
        return false;
    };
    let same_world = match (&area.world, bot.component::<azalea_world::WorldName>()) {
        (Some(w), Ok(here)) => *w == *here,
        (None, Err(_)) => true,
        _ => false,
    };
    if !same_world {
        stop_run(bot);
        forget();
        super::clear(shared);
        say(
            shared,
            "AutoMine: left the world the area is in, so it was cleared",
        );
        return false;
    }
    if s.flag(setting::PROGRESS) {
        hud::count(bot, &area);
    }
    let hand = shared.lock().unwrap().hand_move_keys != 0;
    if super::gate(shared, s).is_none() || hand || pathfinder::user_route() {
        if let Some(run) = RUN.lock().unwrap_or_else(PoisonError::into_inner).as_mut() {
            pause(bot, run);
        }
        super::clear(shared);
        return false;
    }

    let now = crate::client::tracking::game_time();
    let feet = player_pos_to_block_pos(pos);
    let mut guard = RUN.lock().unwrap_or_else(PoisonError::into_inner);
    let run = guard.get_or_insert_with(|| Run::new(&area));

    let mut intent = MineIntent {
        aim: None,
        forward: false,
        turn: 0.0,
    };
    let mut attack = false;
    let mut halt = None;
    for _ in 0..3 {
        match run.stage {
            Stage::Choose => match choose(bot, run, &area, pos, now, s) {
                Next::Mine(stage) => {
                    run.stage = stage;
                    continue;
                }
                Next::Travel(spot) => {
                    pathfinder::goto_quiet(bot, spot, false);
                    run.stage = Stage::Travel {
                        spot,
                        until: now + TRAVEL_TICKS,
                        from: feet,
                    };
                    break;
                }
                Next::Wait => break,
                Next::Halt(h) => {
                    halt = Some(h);
                    break;
                }
            },
            Stage::Travel { spot, until, from } => {
                match pathfinder::travel_status() {
                    Travel::Arrived => {
                        run.arrived_once = true;
                        run.arrived_at = Some(spot);
                        run.stage = Stage::Choose;
                        continue;
                    }
                    Travel::Failed => {
                        run.bad.insert(spot);
                        if !run.arrived_once {
                            run.failed_trips += 1;
                            if run.failed_trips >= FIRST_TRIES {
                                halt = Some(Halt::NoRoute);
                                break;
                            }
                        }
                        run.stage = Stage::Choose;
                    }
                    Travel::Going | Travel::Idle => {
                        if now >= until {
                            let moved = (feet.x - from.x).abs()
                                + (feet.y - from.y).abs()
                                + (feet.z - from.z).abs();
                            pathfinder::cancel_quiet(bot);
                            if moved >= TRAVEL_PROGRESS {
                                pathfinder::goto_quiet(bot, spot, false);
                                run.stage = Stage::Travel {
                                    spot,
                                    until: now + TRAVEL_TICKS,
                                    from: feet,
                                };
                            } else {
                                run.bad.insert(spot);
                                run.stage = Stage::Choose;
                            }
                        }
                    }
                }
                break;
            }
            Stage::Mine {
                pos: target,
                aim,
                arm_at,
                stall_at,
                turn,
            } => {
                if !present(bot, target) {
                    run.stalls = 0;
                    run.stage = Stage::Choose;
                    continue;
                }
                intent.aim = Some(aim);
                intent.turn = turn;
                attack = hit.is_some_and(|h| h.block_pos == target) && now >= arm_at;
                let progressing =
                    bot.component::<MineBlockPos>().ok().and_then(|p| p.0) == Some(target);
                let blocked = hit.is_none() && matches!(bot.hit_result(), Ok(HitResult::Entity(_)));
                if progressing || blocked {
                    run.stage = Stage::Mine {
                        pos: target,
                        aim,
                        arm_at,
                        stall_at: now + STALL_TICKS,
                        turn,
                    };
                } else if now >= stall_at {
                    attack = false;
                    run.skipped.insert(target);
                    run.stalls += 1;
                    if run.stalls >= MAX_STALLS {
                        halt = Some(Halt::Stalled);
                    } else {
                        run.stage = Stage::Choose;
                    }
                }
                break;
            }
        }
    }
    let target = match run.stage {
        Stage::Mine { pos, .. } => Some(pos),
        _ => None,
    };
    let tally = (run.skipped.len(), run.unreachable, run.unloaded);
    if matches!(halt, Some(Halt::Done)) {
        *guard = None;
    }
    drop(guard);
    if let Some(why) = halt {
        return stop(bot, shared, why, tally);
    }
    draw::area(&area, target);
    super::publish(shared, intent);
    attack
}

fn choose(bot: &Client, run: &mut Run, area: &Area, pos: Vec3, now: i64, s: &Store) -> Next {
    if s.choice(setting::INV_FULL) == InvFull::Stop && inventory_full(bot) {
        return Next::Halt(Halt::Full);
    }
    let mode = s.choice(setting::REACH);
    let feet = player_pos_to_block_pos(pos);
    let eye = [pos.x, pos.y + EYE, pos.z];
    for _ in 0..8 {
        if run.top < area.min.y {
            return Next::Halt(Halt::Done);
        }
        if run.targets.is_none() {
            let Some((set, unknown)) = list(bot, run, area) else {
                return Next::Wait;
            };
            if set.is_empty() && unknown {
                return Next::Halt(Halt::NotLoaded);
            }
            run.unknown = unknown;
            run.targets = Some(set);
        }
        if run.search.is_none()
            && let Some((t, aim)) = from_here(bot, run, feet, pos, eye, mode)
        {
            run.arrived_at = None;
            if exposes_fluid(bot, t) {
                match s.choice(setting::FLUIDS) {
                    FluidRule::Skip => {
                        run.skipped.insert(t);
                        continue;
                    }
                    FluidRule::Stop => return Next::Halt(Halt::Hazard),
                }
            }
            let (lo, hi) = s.range(setting::TURN);
            let (dlo, dhi) = s.range(setting::DELAY);
            return Next::Mine(Stage::Mine {
                pos: t,
                aim,
                arm_at: now + (roll(dlo, dhi) / MS_PER_TICK).round() as i64,
                stall_at: now + STALL_TICKS,
                turn: roll(lo, hi),
            });
        }
        if run.search.is_none() {
            run.bad.insert(feet);
            if let Some(spot) = run.arrived_at.take() {
                run.bad.insert(spot);
            }
            prune(bot, run);
            if run.targets.as_ref().is_some_and(HashSet::is_empty) {
                finish_pair(run);
                continue;
            }
            run.search = Some(0);
        }
        match spot(bot, run, area, feet, mode) {
            Search::Found(spot) => {
                run.search = None;
                return Next::Travel(spot);
            }
            Search::Pending => return Next::Wait,
            Search::Exhausted => {
                run.search = None;
                let left = run.targets.take().unwrap_or_default();
                run.unreachable += left.len() as u32;
                run.skipped.extend(left);
                run.targets = Some(HashSet::new());
                finish_pair(run);
            }
        }
    }
    Next::Wait
}

fn finish_pair(run: &mut Run) {
    if run.unknown && !run.relisted {
        run.relisted = true;
        run.targets = None;
        run.search = None;
        return;
    }
    if run.unknown {
        run.unloaded += 1;
    }
    run.next_pair();
}

fn prune(bot: &Client, run: &mut Run) {
    let Ok(world) = bot.world() else { return };
    let world = world.read();
    if let Some(set) = run.targets.as_mut() {
        set.retain(|p| world.get_block_state(*p).is_some_and(solid));
    }
}

fn list(bot: &Client, run: &mut Run, area: &Area) -> Option<(HashSet<BlockPos>, bool)> {
    let world = bot.world().ok()?;
    let world = world.read();
    let mut set = HashSet::new();
    let mut unknown = false;
    for y in [run.top, run.top - 1] {
        if y < area.min.y {
            continue;
        }
        for x in area.min.x..=area.max.x {
            for z in area.min.z..=area.max.z {
                let p = BlockPos::new(x, y, z);
                if area.stairs.contains(&p) || run.skipped.contains(&p) {
                    continue;
                }
                let Some(state) = world.get_block_state(p) else {
                    unknown = true;
                    continue;
                };
                if solid(state)
                    && breakable(&mut run.breakable, state)
                    && !holds_up_a_step(&area.stairs, p, |q| world.get_block_state(q))
                {
                    set.insert(p);
                }
            }
        }
    }
    Some((set, unknown))
}

fn holds_up_a_step(
    stairs: &HashSet<BlockPos>,
    p: BlockPos,
    at: impl Fn(BlockPos) -> Option<BlockState>,
) -> bool {
    let mut q = p.up(1);
    for _ in 0..8 {
        if !at(q).is_some_and(falls) {
            return false;
        }
        if stairs.contains(&q) {
            return true;
        }
        q = q.up(1);
    }
    false
}

fn falls(state: BlockState) -> bool {
    use BlockKind as B;
    matches!(
        BlockKind::from(state),
        B::Sand
            | B::RedSand
            | B::Gravel
            | B::SuspiciousSand
            | B::SuspiciousGravel
            | B::Anvil
            | B::ChippedAnvil
            | B::DamagedAnvil
            | B::DragonEgg
            | B::Scaffolding
            | B::WhiteConcretePowder
            | B::OrangeConcretePowder
            | B::MagentaConcretePowder
            | B::LightBlueConcretePowder
            | B::YellowConcretePowder
            | B::LimeConcretePowder
            | B::PinkConcretePowder
            | B::GrayConcretePowder
            | B::LightGrayConcretePowder
            | B::CyanConcretePowder
            | B::PurpleConcretePowder
            | B::BlueConcretePowder
            | B::BrownConcretePowder
            | B::GreenConcretePowder
            | B::RedConcretePowder
            | B::BlackConcretePowder
    )
}

fn breakable(cache: &mut HashMap<u32, bool>, state: BlockState) -> bool {
    *cache.entry(state.id() as u32).or_insert_with(|| {
        let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
        block.behavior().destroy_time >= 0.0
    })
}

fn allowed(mode: ReachMode, feet: BlockPos, t: BlockPos) -> bool {
    let (dx, dy, dz) = (t.x - feet.x, t.y - feet.y, t.z - feet.z);
    if dx == 0 && dz == 0 {
        return false;
    }
    match mode {
        ReachMode::Adjacent => dx.abs() + dz.abs() == 1 && (-1..=1).contains(&dy),
        ReachMode::Wide => dx.abs().max(dz.abs()) == 1 && (-1..=1).contains(&dy),
        ReachMode::All => dx.abs() <= 5 && dz.abs() <= 5 && (-4..=6).contains(&dy),
    }
}

fn offsets(mode: ReachMode) -> std::ops::RangeInclusive<i32> {
    match mode {
        ReachMode::Adjacent | ReachMode::Wide => -1..=1,
        ReachMode::All => -5..=5,
    }
}

fn dy_range(mode: ReachMode) -> std::ops::RangeInclusive<i32> {
    match mode {
        ReachMode::Adjacent | ReachMode::Wide => -1..=1,
        ReachMode::All => -4..=6,
    }
}

fn from_here(
    bot: &Client,
    run: &Run,
    feet: BlockPos,
    pos: Vec3,
    eye: [f64; 3],
    mode: ReachMode,
) -> Option<(BlockPos, [f64; 3])> {
    let targets = run.targets.as_ref()?;
    let under = |t: BlockPos| {
        t.y == feet.y - 1
            && (pos.x - 0.3).floor() as i32 <= t.x
            && t.x <= (pos.x + 0.3).floor() as i32
            && (pos.z - 0.3).floor() as i32 <= t.z
            && t.z <= (pos.z + 0.3).floor() as i32
    };
    let mut found: Vec<BlockPos> = Vec::new();
    for dx in offsets(mode) {
        for dz in offsets(mode) {
            for dy in dy_range(mode) {
                let t = BlockPos::new(feet.x + dx, feet.y + dy, feet.z + dz);
                if targets.contains(&t)
                    && allowed(mode, feet, t)
                    && !under(t)
                    && !run.skipped.contains(&t)
                {
                    found.push(t);
                }
            }
        }
    }
    let dist = |t: &BlockPos| {
        let c = [
            f64::from(t.x) + 0.5,
            f64::from(t.y) + 0.5,
            f64::from(t.z) + 0.5,
        ];
        (0..3).map(|i| (c[i] - eye[i]).powi(2)).sum::<f64>()
    };
    found.sort_by(|a, b| b.y.cmp(&a.y).then(dist(a).total_cmp(&dist(b))));
    let world = bot.world().ok()?;
    let world = world.read();
    found.into_iter().find_map(|t| {
        let state = world.get_block_state(t).filter(|st| solid(*st))?;
        aim_at(&world.chunks, eye, t, state).map(|aim| (t, aim))
    })
}

fn aim_at(
    chunks: &azalea_world::ChunkStorage,
    eye: [f64; 3],
    t: BlockPos,
    state: BlockState,
) -> Option<[f64; 3]> {
    let shape = state.outline_shape(t);
    if shape.is_empty() {
        return None;
    }
    let b = shape.bounds();
    let base = [f64::from(t.x), f64::from(t.y), f64::from(t.z)];
    let lo = [base[0] + b.min.x, base[1] + b.min.y, base[2] + b.min.z];
    let hi = [base[0] + b.max.x, base[1] + b.max.y, base[2] + b.max.z];
    let mut faces: Vec<(usize, [f64; 3])> = Vec::with_capacity(3);
    for axis in 0..3 {
        let plane = if eye[axis] < lo[axis] {
            lo[axis]
        } else if eye[axis] > hi[axis] {
            hi[axis]
        } else {
            continue;
        };
        let mut c = [
            (lo[0] + hi[0]) * 0.5,
            (lo[1] + hi[1]) * 0.5,
            (lo[2] + hi[2]) * 0.5,
        ];
        c[axis] = plane;
        faces.push((axis, c));
    }
    let d2 = |p: &[f64; 3]| (0..3).map(|i| (p[i] - eye[i]).powi(2)).sum::<f64>();
    faces.sort_by(|a, b| d2(&a.1).total_cmp(&d2(&b.1)));
    for (axis, centre) in faces {
        let mut jittered = centre;
        for i in (0..3).filter(|&i| i != axis) {
            let half = JITTER.min((hi[i] - lo[i]) * 0.4);
            jittered[i] += f64::from(roll(0.0, 2.0 * half as f32)) - half;
        }
        for p in [jittered, centre] {
            let len = d2(&p).sqrt();
            if len > REACH {
                continue;
            }
            let len = len.max(1e-6);
            let dir = [
                (p[0] - eye[0]) / len,
                (p[1] - eye[1]) / len,
                (p[2] - eye[2]) / len,
            ];
            let hit = clip(
                chunks,
                ClipContext {
                    from: Vec3::new(eye[0], eye[1], eye[2]),
                    to: Vec3::new(
                        p[0] + dir[0] * 0.05,
                        p[1] + dir[1] * 0.05,
                        p[2] + dir[2] * 0.05,
                    ),
                    block_shape_type: BlockShapeType::Outline,
                    fluid_pick_type: FluidPickType::None,
                    held_item: azalea_registry::builtin::ItemKind::Air,
                    entity: None,
                },
            );
            if !hit.miss && hit.block_pos == t {
                return Some(p);
            }
        }
    }
    None
}

enum Search {
    Found(BlockPos),
    Pending,
    Exhausted,
}

fn spot(bot: &Client, run: &mut Run, area: &Area, feet: BlockPos, mode: ReachMode) -> Search {
    let (Some(targets), Some(start)) = (run.targets.as_ref(), run.search) else {
        return Search::Exhausted;
    };
    let Ok(world) = bot.world() else {
        return Search::Pending;
    };
    let world = world.read();
    let dy = dy_range(mode);
    let (lo_y, hi_y) = (run.top - 1 - *dy.end(), run.top - *dy.start());
    let passable = |p: BlockPos| {
        world
            .get_block_state(p)
            .is_some_and(is_block_state_passable)
    };
    let standable = |p: BlockPos| {
        passable(p)
            && passable(p.up(1))
            && world
                .get_block_state(p.down(1))
                .is_some_and(is_block_state_standable)
    };
    let has_work = |sp: BlockPos| {
        offsets(mode).any(|dx| {
            offsets(mode).any(|dz| {
                dy_range(mode).any(|d| {
                    let t = BlockPos::new(sp.x + dx, sp.y + d, sp.z + dz);
                    targets.contains(&t) && allowed(mode, sp, t) && !run.skipped.contains(&t)
                })
            })
        })
    };
    let (x0, x1, z0, z1) = (
        area.min.x - 1,
        area.max.x + 1,
        area.min.z - 1,
        area.max.z + 1,
    );
    let last = [x0 - feet.x, feet.x - x1, z0 - feet.z, feet.z - z1]
        .into_iter()
        .map(i32::abs)
        .max()
        .unwrap_or(0)
        .max((x1 - x0).max(z1 - z0))
        + 1;
    let mut budget = SEARCH_BUDGET;
    let mut r = start;
    while r <= last {
        let mut columns: Vec<(i32, i32)> = Vec::new();
        let (rx0, rx1) = ((feet.x - r).max(x0), (feet.x + r).min(x1));
        for z in [feet.z - r, feet.z + r] {
            if (z0..=z1).contains(&z) && rx0 <= rx1 {
                columns.extend((rx0..=rx1).map(|x| (x, z)));
            }
            if r == 0 {
                break;
            }
        }
        let (rz0, rz1) = ((feet.z - r + 1).max(z0), (feet.z + r - 1).min(z1));
        if r > 0 && rz0 <= rz1 {
            for x in [feet.x - r, feet.x + r] {
                if (x0..=x1).contains(&x) {
                    columns.extend((rz0..=rz1).map(|z| (x, z)));
                }
            }
        }
        let cells = columns.len() * (hi_y - lo_y + 1).max(0) as usize;
        if cells > budget && budget < SEARCH_BUDGET {
            run.search = Some(r);
            return Search::Pending;
        }
        budget = budget.saturating_sub(cells);
        let mut best: Option<(i32, BlockPos)> = None;
        for (x, z) in columns {
            for y in lo_y..=hi_y {
                let p = BlockPos::new(x, y, z);
                if p == feet || run.bad.contains(&p) || !standable(p) || !has_work(p) {
                    continue;
                }
                let cost = (y - feet.y).abs();
                if best.is_none_or(|(c, _)| cost < c) {
                    best = Some((cost, p));
                }
            }
        }
        if let Some((_, p)) = best {
            return Search::Found(p);
        }
        r += 1;
    }
    Search::Exhausted
}

fn stairs(min: BlockPos, max: BlockPos, near: (i32, i32)) -> HashSet<BlockPos> {
    let (x0, x1, z0, z1) = (min.x, max.x, min.z, max.z);
    if x1 - x0 < 2 || z1 - z0 < 2 {
        return HashSet::new();
    }
    let mut ring: Vec<(i32, i32)> = Vec::new();
    ring.extend((x0..=x1).map(|x| (x, z0)));
    ring.extend((z0 + 1..=z1).map(|z| (x1, z)));
    ring.extend((x0..x1).rev().map(|x| (x, z1)));
    ring.extend((z0 + 1..z1).rev().map(|z| (x0, z)));
    let start = (0..ring.len())
        .min_by_key(|&i| (ring[i].0 - near.0).pow(2) + (ring[i].1 - near.1).pow(2))
        .unwrap_or(0);
    let height = max.y - min.y + 1;
    (0..height)
        .map(|k| {
            let (x, z) = ring[(start + k as usize) % ring.len()];
            BlockPos::new(x, max.y - k, z)
        })
        .collect()
}

fn inventory_full(bot: &Client) -> bool {
    let Ok(inv) = bot.component::<Inventory>() else {
        return false;
    };
    let menu = &inv.inventory_menu;
    menu.player_slots_without_hotbar_range()
        .chain(menu.hotbar_slots_range())
        .all(|i| match menu.slot(i) {
            Some(ItemStack::Present(d)) => d.count >= azalea_inventory::bundle::max_stack_size(d),
            _ => false,
        })
}

fn say(shared: &SharedMutex, text: &str) {
    let mut st = shared.lock().unwrap();
    crate::modules::chat_math::show_local(&mut st.session, text);
}

fn stop(
    bot: &Client,
    shared: &SharedMutex,
    why: Halt,
    (skipped, unreachable, unloaded): (usize, u32, u32),
) -> bool {
    stop_run(bot);
    store().set_enabled(Id::AutoMine, false);
    let text = match why {
        Halt::Done => {
            let mut text = if skipped == 0 {
                "AutoMine: area cleared".to_string()
            } else {
                format!("AutoMine: area done; {skipped} blocks left ({unreachable} out of reach)")
            };
            if unloaded > 0 {
                text.push_str(&format!(", {unloaded} layer pairs were partly unloaded"));
            }
            text
        }
        Halt::Hazard => "AutoMine: stopped, there is a fluid behind the next block".to_string(),
        Halt::Stalled => "AutoMine: stopped, blocks are not breaking".to_string(),
        Halt::Full => "AutoMine: stopped, the inventory is full".to_string(),
        Halt::NotLoaded => {
            "AutoMine: stopped, the area is not loaded; go closer and switch AutoMine back on"
                .to_string()
        }
        Halt::NoRoute => {
            "AutoMine starts from the top, go to the top of the area you want to mine".to_string()
        }
    };
    let mut st = shared.lock().unwrap();
    st.session.auto_mine = None;
    crate::modules::chat_math::show_local(&mut st.session, &text);
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_staircase_steps_one_level_per_ring_cell() {
        let steps = stairs(BlockPos::new(0, 0, 0), BlockPos::new(4, 9, 3), (0, 0));
        assert_eq!(steps.len(), 10);
        let mut by_level: Vec<BlockPos> = steps.into_iter().collect();
        by_level.sort_by_key(|p| -p.y);
        assert_eq!(by_level[0].y, 9);
        for w in by_level.windows(2) {
            let (a, b) = (w[0], w[1]);
            assert_eq!(a.y - 1, b.y);
            assert_eq!((a.x - b.x).abs() + (a.z - b.z).abs(), 1, "{a:?} -> {b:?}");
        }
    }

    #[test]
    fn a_narrow_box_has_no_staircase() {
        assert!(stairs(BlockPos::new(0, 0, 0), BlockPos::new(1, 5, 9), (0, 0)).is_empty());
    }

    #[test]
    fn the_own_column_is_never_allowed() {
        let feet = BlockPos::new(0, 64, 0);
        for mode in [ReachMode::Adjacent, ReachMode::Wide, ReachMode::All] {
            for dy in -2..=3 {
                assert!(!allowed(mode, feet, BlockPos::new(0, 64 + dy, 0)));
            }
        }
        assert!(allowed(ReachMode::Adjacent, feet, BlockPos::new(1, 63, 0)));
        assert!(!allowed(ReachMode::Adjacent, feet, BlockPos::new(1, 64, 1)));
        assert!(allowed(ReachMode::Wide, feet, BlockPos::new(1, 65, 1)));
    }
}
