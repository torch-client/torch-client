mod fly;
mod plan;
mod rules;
mod sight;
mod spot;

pub use plan::{Cells, Region};

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering::Relaxed};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use azalea::Client;
use azalea::block::BlockState;
use azalea::entity::inventory::Inventory;
use azalea::entity::{Physics, PlayerAbilities};
use azalea::mining::MineBlockPos;
use azalea::pathfinder::world::{is_block_state_passable, is_block_state_standable};
use azalea::pathfinder::{PlaceRules, player_pos_to_block_pos};
use azalea_core::hit_result::{BlockHitResult, HitResult};
use azalea_core::position::BlockPos;
use azalea_inventory::ItemStack;
use azalea_registry::builtin::ItemKind;
use azalea_world::World;

use super::auto_mine::shared::{
    EYE, MAX_STALLS, REACH, STALL_TICKS, aim_at, best_tool, breakable, gate, present, select_tool,
    solid, state_at,
};
use super::pathfinder::{self, QuietGoal, QuietOpts, Travel};
use super::place::stock::{self, Held};
use super::place::{self, Click, Fired, Want};
use super::registry::{Id, builder as setting, options};
use super::sent::{self, Look};
use super::{Edge, MS_PER_TICK, Phase, Store, roll, store};
use crate::blocks::state_is_air;
use crate::platform::time::Instant;
use crate::renderer::world_draw::{self, Node};
use crate::session::{CameraAim, SharedMutex};
use fly::{Fly, Grid};
use plan::{
    EXPOSED_PHASE, Job, Plan, Scan, Verdict, Wrong, contained, dist2, holds, job, job_item, judge,
    refills, source,
};
use rules::{Rules, Stock, falls};
use sight::{Choice, Sight, angle, covers, least_turn, pick};
use spot::{HALF_DIAG, Spot, SpotSearch, Target, hover_point};

options! {
    pub enum BuildMode {
        Legit = "Legit",
        Fast = "Fast",
    }
}

options! {
    pub enum BreakMode {
        All = "All mismatches",
        OnlyWhereNeeded = "Only where a block goes",
    }
}

const WRONG_CAP: usize = 1024;

const SCAN_BUDGET: Duration = Duration::from_millis(2);

const SCAN_BATCH: usize = 4096;

const NEAR_RADIUS: i32 = 5;

const CHANGED_CAP: usize = 4096;

const NOTES_CAP: usize = 4096;

const SOLVES_PER_TICK: usize = 6;

const SETTLE_TICKS: i64 = 10;

const PLACE_TRIES: u8 = 3;

const STRIKES: u8 = 3;

const STRIKE_AWAY: i64 = 200;

const MISS_TRIES: u8 = 3;

const FIRE_ANGLE: f32 = 2.0;

const YAW_MARGIN: f32 = 5.0;

const HIT_MARGIN: f64 = 0.15;

const CLICK_REACH: f64 = REACH - fly::HOLD_SLACK;

const USE_SPARE: usize = 2;

const LOOP_LIMIT: u8 = 3;

const FAILS_IN_ROW: u32 = 8;

const FAIL_WINDOW: usize = 20;
const FAIL_WINDOW_LIMIT: usize = 15;

const SPARE_SLOTS: [u8; 3] = [6, 7, 8];

const FLY_START_TICKS: i64 = 10;

const FLY_IDLE: i64 = 60;

const LAND_DEPTH: i32 = 32;

const SUPPORT_WAIT: i64 = 2 * SETTLE_TICKS;

const UNSEEN_HOP: f64 = 16.0;

const BREAK_TRIES: u8 = 3;

const GOAL_CELLS: usize = 64;

const GOAL_RANGE: f64 = 3.0;

const LAYER_NEAR: i32 = 2 * spot::SPOT_RADIUS;

const UNSEEN_RANGE: f64 = 32.0;

const UNREACHABLE_TICKS: i64 = 600;

const TRAVEL_IDLE: i64 = 300;

const WAIT_TICKS: i64 = 20;

const TURN_MARGIN: i64 = 20;

const MISSING_SHOWN: usize = 3;

const FLY_COLLIDE_TICKS: u32 = 20;

const NUDGE_TICKS: i64 = 30;

const NUDGE_CLOSE: f64 = 0.02;

const WATCHDOG_TICKS: i64 = 400;

const WATCHDOG_TRIPS: u8 = 3;

const WATCHDOG_MOVE: f64 = 0.5;

const GATE_SAY_TICKS: i64 = 40;

const HOPELESS_NEAR: i32 = 6;

const SOURCE_RADIUS: i32 = 16;
const SOURCE_DEPTH: i32 = 8;

const REFILL_WAIT: i64 = 5 + SETTLE_TICKS;
const SOURCE_GIVE_UP: i64 = 60;

const SOURCE_SEARCH_TICKS: i64 = 100;

const SOURCE_TRIES: u8 = 3;

const OWNER: &str = "builder";

const BREAK_COLOR: u32 = 0x30FF_AA00;
const PLACE_COLOR: u32 = 0x3055_FF55;

static EDGE: Edge = Edge::new(Id::AutoBuild);

static RUN: Mutex<Option<Run>> = Mutex::new(None);

static NEXT: Mutex<Option<Arc<Plan>>> = Mutex::new(None);

static ACTIVE: AtomicBool = AtomicBool::new(false);

static PAUSED: AtomicBool = AtomicBool::new(false);

static STATUS: Mutex<Option<Status>> = Mutex::new(None);

static ROUTE: Mutex<Option<(Arc<Plan>, Arc<Stock>)>> = Mutex::new(None);

static CHANGED: Mutex<Changed> = Mutex::new(Changed {
    cells: Vec::new(),
    lost: false,
});

struct Changed {
    cells: Vec<BlockPos>,
    lost: bool,
}

static NOTES: Mutex<Vec<BlockPos>> = Mutex::new(Vec::new());

static LOOK_OUT: AtomicBool = AtomicBool::new(false);

static FLY_KEYS: AtomicU8 = AtomicU8::new(0);

static DRAWN: Mutex<Option<(BlockPos, u32)>> = Mutex::new(None);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Scanning,
    Placing,
    Breaking,
    Walking,
    Paused,
    Waiting,
    Done,
}

#[derive(Clone, Debug)]
pub struct Status {
    pub state: State,
    pub note: String,
    pub placed: u32,
    pub broken: u32,
    pub left: u32,
    pub skipped: u32,
    pub total: u64,
    pub missing: Vec<(ItemKind, u32)>,
    pub target: Option<BlockPos>,
}

pub fn enabled() -> bool {
    store().enabled(Id::AutoBuild)
}

pub fn drives_camera() -> bool {
    LOOK_OUT.load(Relaxed)
}

pub fn fly_keys() -> u8 {
    FLY_KEYS.load(Relaxed)
}

pub fn start(regions: Vec<Region>) -> Result<(), String> {
    let plan = Arc::new(Plan::new(regions)?);
    *STATUS.lock().unwrap_or_else(PoisonError::into_inner) = Some(Status {
        state: State::Scanning,
        note: String::new(),
        placed: 0,
        broken: 0,
        left: 0,
        skipped: 0,
        total: plan.total(),
        missing: Vec::new(),
        target: None,
    });
    *NEXT.lock().unwrap_or_else(PoisonError::into_inner) = Some(plan);
    PAUSED.store(false, Relaxed);
    store().set_enabled(Id::AutoBuild, true);
    Ok(())
}

pub fn pause(on: bool) {
    PAUSED.store(on, Relaxed);
    if on
        && let Some(st) = STATUS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        && st.state != State::Done
    {
        st.state = State::Paused;
        st.note = "Paused".to_string();
    }
}

pub fn stop() {
    *NEXT.lock().unwrap_or_else(PoisonError::into_inner) = None;
    PAUSED.store(false, Relaxed);
    mark_stopped();
    store().set_enabled(Id::AutoBuild, false);
}

pub fn status() -> Option<Status> {
    STATUS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

pub fn route_want(cell: BlockPos) -> Option<BlockState> {
    if !ACTIVE.load(Relaxed) {
        return None;
    }
    let (plan, stock) = ROUTE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()?;
    let want = plan.want(cell)?;
    rules::route_block(want, &stock).then_some(want)
}

pub fn note_route_place(cell: BlockPos) {
    if !ACTIVE.load(Relaxed) {
        return;
    }
    let mut notes = NOTES.lock().unwrap_or_else(PoisonError::into_inner);
    if notes.len() < NOTES_CAP {
        notes.push(cell);
    }
}

pub fn block_changed(x: i32, y: i32, z: i32) {
    if !ACTIVE.load(Relaxed) {
        return;
    }
    let mut changed = CHANGED.lock().unwrap_or_else(PoisonError::into_inner);
    if changed.cells.len() < CHANGED_CAP {
        changed.cells.push(BlockPos::new(x, y, z));
    } else {
        changed.lost = true;
    }
}

pub fn reset() {
    *RUN.lock().unwrap_or_else(PoisonError::into_inner) = None;
    *NEXT.lock().unwrap_or_else(PoisonError::into_inner) = None;
    *STATUS.lock().unwrap_or_else(PoisonError::into_inner) = None;
    *ROUTE.lock().unwrap_or_else(PoisonError::into_inner) = None;
    ACTIVE.store(false, Relaxed);
    PAUSED.store(false, Relaxed);
    LOOK_OUT.store(false, Relaxed);
    FLY_KEYS.store(0, Relaxed);
    take_changed();
    NOTES.lock().unwrap_or_else(PoisonError::into_inner).clear();
    draw(None);
}

#[derive(Clone, Copy, Debug, Default)]
struct Marks {
    strikes: u8,
    places: u8,
    breaks: u8,
    away_until: i64,
    routed: bool,
    no_fly_until: i64,
    no_walk_until: i64,
}

#[derive(Clone, Copy, Debug)]
struct Support {
    state: BlockState,
    cell: BlockPos,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Idle {
    Trying,
    Away(u32),
    Settling,
    Checking,
    NoWay(u32, Why),
    NoWater(u32),
    Refilling(BlockPos),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Why {
    NoItem,
    NoSupport,
    NoSpot,
    LookUp,
    LookDown,
    NoClick,
    Refused,
    NoRoute,
    Lava,
    NeedsPower,
    NotBreaking,
    Stuck,
}

impl Why {
    fn label(self) -> &'static str {
        self.key().english()
    }

    fn key(self) -> crate::lang::Key {
        use crate::lang::Key;
        match self {
            Why::NoItem => const { Key::of("builder.why.no_item") },
            Why::NoSupport => const { Key::of("builder.why.no_support") },
            Why::NoSpot => const { Key::of("builder.why.no_spot") },
            Why::LookUp => const { Key::of("builder.why.look_up") },
            Why::LookDown => const { Key::of("builder.why.look_down") },
            Why::NoClick => const { Key::of("builder.why.no_click") },
            Why::Refused => const { Key::of("builder.why.refused") },
            Why::NoRoute => const { Key::of("builder.why.no_route") },
            Why::Lava => const { Key::of("builder.why.lava") },
            Why::NeedsPower => const { Key::of("builder.why.needs_power") },
            Why::NotBreaking => const { Key::of("builder.why.not_breaking") },
            Why::Stuck => const { Key::of("builder.why.stuck") },
        }
    }
}

fn render(key: crate::lang::Key, args: &[String], english: bool) -> String {
    crate::util::placeholders::substitute(if english { key.english() } else { key.get() }, args)
}

fn click_why(want: BlockState, item: Option<ItemKind>) -> Why {
    use azalea::block::BlockTrait;
    let id = want.as_block_kind().to_str();
    if id.contains("iron") && Box::<dyn BlockTrait>::from(want).get_property("open") == Some("true")
    {
        return Why::NeedsPower;
    }
    match item.and_then(|i| place::vertical_need(&ItemStack::new(i, 1), want)) {
        Some(true) => Why::LookUp,
        Some(false) => Why::LookDown,
        None => Why::NoClick,
    }
}

fn spot_why(want: BlockState, item: Option<ItemKind>) -> Why {
    match click_why(want, item) {
        Why::NoClick => Why::NoSpot,
        other => other,
    }
}

fn top_why(whys: impl Iterator<Item = Why>) -> Why {
    let mut counts: HashMap<Why, usize> = HashMap::new();
    for w in whys {
        *counts.entry(w).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        .map_or(Why::NoSpot, |(w, _)| w)
}

fn not_built(run: &Run, english: bool) -> String {
    const TO_BREAK: crate::lang::Key = crate::lang::Key::of("builder.not_built.to_break");
    let cells: Vec<(String, Why, BlockPos)> = run
        .wrong
        .skipped_iter()
        .chain(run.wrong.iter())
        .filter_map(|p| {
            let want = run.plan.want(p)?;
            let name = if crate::blocks::state_is_air(want) {
                render(TO_BREAK, &[], english)
            } else {
                block_name(want)
            };
            Some((name, run.why.get(&p).copied().unwrap_or(Why::NoClick), p))
        })
        .collect();
    not_built_line(&cells, english)
}

const NOT_BUILT_GROUPS: usize = 6;
const NOT_BUILT_AT: usize = 3;

fn not_built_line(cells: &[(String, Why, BlockPos)], english: bool) -> String {
    use crate::lang::Key;
    let mut groups: Vec<((String, Why), Vec<BlockPos>)> = Vec::new();
    for (name, why, at) in cells {
        match groups.iter_mut().find(|((n, w), _)| n == name && w == why) {
            Some((_, list)) => list.push(*at),
            None => groups.push(((name.clone(), *why), vec![*at])),
        }
    }
    if groups.is_empty() {
        return String::new();
    }
    groups.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
    let more = groups.len().saturating_sub(NOT_BUILT_GROUPS);
    let parts: Vec<String> = groups
        .iter()
        .take(NOT_BUILT_GROUPS)
        .map(|((name, why), at)| {
            let mut list = at.clone();
            list.sort_by_key(|p| (p.y, p.x, p.z));
            let shown: Vec<String> = list
                .iter()
                .take(NOT_BUILT_AT)
                .map(|p| format!("{} {} {}", p.x, p.y, p.z))
                .collect();
            let etc = if list.len() > NOT_BUILT_AT {
                ", ..."
            } else {
                ""
            };
            let args = [
                list.len().to_string(),
                name.clone(),
                render(why.key(), &[], english),
                shown.join(", ") + etc,
            ];
            render(const { Key::of("builder.not_built.group") }, &args, english)
        })
        .collect();
    if more > 0 {
        render(
            const { Key::of("builder.not_built.more") },
            &[parts.join("; "), more.to_string()],
            english,
        )
    } else {
        render(
            const { Key::of("builder.not_built") },
            &[parts.join("; ")],
            english,
        )
    }
}

#[derive(Default)]
struct Refill {
    source: Option<BlockPos>,
    announced: HashSet<BlockPos>,
    bad: HashSet<BlockPos>,
    tries: u8,
    gone_since: Option<i64>,
    filled_at: i64,
    searched_at: Option<i64>,
    filling: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RefillNext {
    Pour,
    Fill,
    Missing,
}

fn refill_next(water: u32, empty: u32, filling: bool) -> RefillNext {
    if filling && empty > 0 {
        RefillNext::Fill
    } else if water > 0 {
        RefillNext::Pour
    } else if empty > 0 {
        RefillNext::Fill
    } else {
        RefillNext::Missing
    }
}

#[derive(Clone, Copy)]
struct Watchdog {
    since: i64,
    pos: [f64; 3],
    trips: u8,
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
struct Shot {
    face: azalea_core::direction::Direction,
    at: [f64; 3],
    look: Look,
    sent: Look,
    pos: [f64; 3],
}

impl Shot {
    fn of(click: &Click, sent: Look, pos: [f64; 3]) -> Shot {
        Shot {
            face: click.hit.direction,
            at: sight::point(click),
            look: click.look,
            sent,
            pos,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Expect {
    Block(BlockState),
    Exact(BlockState),
    Drained,
    Gone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Trip {
    InRow(u32),
    Ratio { ok: usize },
}

#[derive(Default)]
struct Fails {
    in_row: u32,
    recent: VecDeque<bool>,
    last: Option<BlockPos>,
}

impl Fails {
    fn note(&mut self, ok: bool, cell: BlockPos) -> Option<Trip> {
        if self.recent.len() == FAIL_WINDOW {
            self.recent.pop_front();
        }
        self.recent.push_back(ok);
        if ok {
            self.in_row = 0;
            return None;
        }
        self.in_row += 1;
        self.last = Some(cell);
        if self.in_row >= FAILS_IN_ROW {
            return Some(Trip::InRow(self.in_row));
        }
        let failed = self.recent.iter().filter(|o| !**o).count();
        (self.recent.len() == FAIL_WINDOW && failed >= FAIL_WINDOW_LIMIT).then_some(Trip::Ratio {
            ok: FAIL_WINDOW - failed,
        })
    }

    fn clear(&mut self) {
        *self = Fails::default();
    }
}

#[derive(Default)]
struct Spares {
    items: [Option<ItemKind>; SPARE_SLOTS.len()],
    next: usize,
}

impl Spares {
    fn pick(&mut self, item: ItemKind, selected: u8) -> u8 {
        if let Some(i) = self.items.iter().position(|k| *k == Some(item)) {
            return SPARE_SLOTS[i];
        }
        for _ in 0..SPARE_SLOTS.len() {
            let i = self.next;
            self.next = (self.next + 1) % SPARE_SLOTS.len();
            if SPARE_SLOTS[i] != selected {
                self.items[i] = Some(item);
                return SPARE_SLOTS[i];
            }
        }
        SPARE_SLOTS[0]
    }
}

struct Run {
    plan: Arc<Plan>,
    world: Option<azalea_world::WorldName>,
    stage: Stage,
    wrong: Wrong,
    scan: Scan,
    mode: BreakMode,
    marks: HashMap<BlockPos, Marks>,
    batch: Vec<BlockPos>,
    settling: Vec<(BlockPos, Expect, i64)>,
    shots: HashMap<BlockPos, Shot>,
    fails: Fails,
    spares: Spares,
    fly: Fly,
    click: Option<Click>,
    #[allow(dead_code)]
    click_eye: Option<[f64; 3]>,
    scaffold: HashSet<BlockPos>,
    finishing: bool,
    scrap: HashSet<BlockPos>,
    breakable: HashMap<u32, bool>,
    missing: Vec<(ItemKind, u32)>,
    fast_at: i64,
    sight: Sight,
    own: HashMap<BlockPos, i64>,
    spot: Option<SpotSearch>,
    expect: Option<BlockPos>,
    idle: Idle,
    unsupported: HashSet<BlockPos>,
    supports: HashMap<BlockPos, Support>,
    support_failed: HashSet<BlockPos>,
    loops: HashMap<(BlockPos, BlockPos), u8>,
    retried: bool,
    not_aimed: u32,
    declined: u32,
    placed: u32,
    broken: u32,
    stalls: u32,
    hold: Option<String>,
    hopeless: HashSet<BlockPos>,
    spot_settings: (bool, bool),
    refill: Refill,
    watchdog: Watchdog,
    gated_since: Option<(i64, bool)>,
    why: HashMap<BlockPos, Why>,
    nudge: Option<Nudge>,
    stepped_out: Option<(BlockPos, i64)>,
    fly_collided: u32,
    fly_watch: Option<([f64; 3], fly::Watch)>,
    watchdog_target: Option<BlockPos>,
    last_spot: Option<(BlockPos, BlockPos)>,
    bad_spots: HashSet<(BlockPos, BlockPos)>,
    holes: HashMap<BlockPos, Hole>,
    no_hole: HashSet<BlockPos>,
    vertical: HashMap<BlockState, Option<bool>>,
    left_holes: Vec<BlockPos>,
}

#[derive(Clone, Copy, Debug)]
struct Hole {
    target: BlockPos,
    kind: plan::HoleKind,
    original: Option<BlockState>,
    dug: bool,
}

#[derive(Clone, Copy, Debug)]
struct Nudge {
    cell: BlockPos,
    stand: BlockPos,
    feet: [f64; 2],
    sneak: bool,
    started: Option<i64>,
    there: bool,
}

#[derive(Clone, Copy)]
enum Stage {
    Choose,
    Spot,
    Break {
        pos: BlockPos,
        aim: [f64; 3],
        arm_at: i64,
        stall_at: i64,
        turn: f32,
        tool: Option<u8>,
    },
    Place {
        cell: BlockPos,
        want: BlockState,
        item: ItemKind,
        aim: [f64; 3],
        look: Look,
        turn: f32,
        arm_at: i64,
        give_up_at: i64,
        misses: u8,
        air: bool,
        fragile: bool,
    },
    Travel {
        idle_at: i64,
        at: BlockPos,
    },
    Fly {
        watch: fly::Watch,
    },
    Use {
        cell: BlockPos,
        want: BlockState,
        aim: [f64; 3],
        turn: f32,
        wait_until: i64,
        give_up_at: i64,
        clicks: u8,
        budget: u8,
    },
    Waiting {
        recheck_at: i64,
    },
}

impl Run {
    fn new(plan: Arc<Plan>, world: Option<azalea_world::WorldName>, mode: BreakMode) -> Run {
        Run {
            plan,
            world,
            stage: Stage::Choose,
            wrong: Wrong::new(WRONG_CAP),
            scan: Scan::new(crate::client::worldsync::chunk_generation()),
            mode,
            marks: HashMap::new(),
            batch: Vec::new(),
            settling: Vec::new(),
            shots: HashMap::new(),
            fails: Fails::default(),
            spares: Spares::default(),
            fly: Fly::default(),
            click: None,
            click_eye: None,
            scaffold: HashSet::new(),
            finishing: false,
            scrap: HashSet::new(),
            breakable: HashMap::new(),
            missing: Vec::new(),
            fast_at: i64::MIN,
            sight: Sight::new(BlockPos::new(0, i32::MIN, 0)),
            own: HashMap::new(),
            spot: None,
            expect: None,
            idle: Idle::Checking,
            unsupported: HashSet::new(),
            supports: HashMap::new(),
            support_failed: HashSet::new(),
            loops: HashMap::new(),
            retried: false,
            not_aimed: 0,
            declined: 0,
            placed: 0,
            broken: 0,
            stalls: 0,
            hold: None,
            hopeless: HashSet::new(),
            spot_settings: (false, false),
            refill: Refill::default(),
            watchdog: Watchdog {
                since: i64::MIN,
                pos: [f64::NAN; 3],
                trips: 0,
            },
            gated_since: None,
            why: HashMap::new(),
            nudge: None,
            stepped_out: None,
            fly_collided: 0,
            fly_watch: None,
            watchdog_target: None,
            last_spot: None,
            bad_spots: HashSet::new(),
            holes: HashMap::new(),
            no_hole: HashSet::new(),
            vertical: HashMap::new(),
            left_holes: Vec::new(),
        }
    }

    fn strike_why(&mut self, pos: BlockPos, now: i64, why: Why) {
        self.why.insert(pos, why);
        self.strike(pos, now);
    }

    fn skip_why(&mut self, pos: BlockPos, why: Why) {
        self.why.insert(pos, why);
        self.skip(pos);
    }

    fn worked(&mut self, now: i64) {
        self.watchdog.since = now;
        self.watchdog.trips = 0;
    }

    fn marks(&mut self, pos: BlockPos) -> &mut Marks {
        self.marks.entry(pos).or_default()
    }

    fn away(&self, pos: BlockPos, now: i64) -> bool {
        self.marks.get(&pos).is_some_and(|m| m.away_until > now)
    }

    fn skip(&mut self, pos: BlockPos) {
        self.wrong.skip(pos);
        self.scrap.remove(&pos);
    }

    fn strike(&mut self, pos: BlockPos, now: i64) {
        let m = self.marks(pos);
        m.strikes += 1;
        m.away_until = now + STRIKE_AWAY;
        m.routed = false;
        let strikes = m.strikes;
        if strikes >= STRIKES {
            self.skip(pos);
        }
    }

    fn placed_own(
        &mut self,
        cell: BlockPos,
        want: BlockState,
        eye: [f64; 3],
        now: i64,
        shot: Option<Shot>,
    ) {
        self.placed_as(cell, Expect::Block(want), eye, now, shot);
    }

    fn placed_as(
        &mut self,
        cell: BlockPos,
        expect: Expect,
        eye: [f64; 3],
        now: i64,
        shot: Option<Shot>,
    ) {
        self.worked(now);
        self.hopeless.remove(&cell);
        self.placed += 1;
        self.marks(cell).places += 1;
        self.settling.push((cell, expect, now + SETTLE_TICKS));
        self.loops.clear();
        if let Some(shot) = shot {
            self.shots.insert(cell, shot);
        }
        self.own.insert(cell, now + SETTLE_TICKS);
        self.sight.placed(cell, eye);
    }

    fn hold(&mut self, why: &str) {
        self.hold = Some(why.to_string());
        PAUSED.store(true, Relaxed);
    }
}

impl Stage {
    fn walking(&self) -> bool {
        matches!(self, Stage::Travel { .. })
    }
}

#[derive(Clone, Copy)]
struct Creative {
    on: bool,
    fly: bool,
    flying: bool,
    speed: f64,
}

struct Here {
    pos: [f64; 3],
    feet: BlockPos,
    eye: [f64; 3],
    now: i64,
    look: Look,
    held: Option<ItemKind>,
    slot: u8,
    vel: [f64; 3],
    #[allow(dead_code)]
    collided: bool,
    creative: Creative,
    air: bool,
}

struct Ended {
    chat: String,
    note: String,
}

#[derive(Default)]
struct Out {
    look: Option<CameraAim>,
    attack: bool,
    target: Option<(BlockPos, u32)>,
    done: Option<Ended>,
    lines: Vec<String>,
    keys: u8,
    toggle_fly: bool,
}

pub fn tick(bot: &Client, shared: &SharedMutex, hit: Option<&BlockHitResult>) -> bool {
    let s = store();
    match EDGE.poll(s) {
        Phase::Off => return false,
        Phase::Stopped => {
            finish(bot, shared, None);
            return false;
        }
        Phase::Started => {
            for id in [Id::AutoMine, Id::OreMiner] {
                if s.enabled(id) {
                    s.set_enabled(id, false);
                    let name = super::registry::module(id).label();
                    say(
                        shared,
                        &crate::lang::trf!("builder.switched_other_off", name),
                    );
                }
            }
        }
        Phase::Running => {}
    }
    let next = NEXT.lock().unwrap_or_else(PoisonError::into_inner).take();
    if let Some(plan) = next {
        end_run(bot);
        let world = bot
            .component::<azalea_world::WorldName>()
            .ok()
            .map(|n| n.clone());
        *RUN.lock().unwrap_or_else(PoisonError::into_inner) =
            Some(Run::new(plan, world, s.choice(setting::BREAK)));
        take_changed();
        NOTES.lock().unwrap_or_else(PoisonError::into_inner).clear();
        ACTIVE.store(true, Relaxed);
    }

    if ACTIVE.load(Relaxed) {
        let mut st = shared.lock().unwrap();
        let asked = st.session.sign_editor_open.take();
        let open = st.session.sign_edit_open_pos.take();
        drop(st);
        let _ = (asked, open);
    }
    let hand = shared.lock().unwrap().hand_move_keys != 0;
    let gated = gate(shared, s.flag(setting::IN_GUI)).is_none();
    let pos = bot.position().ok();
    let world = bot
        .component::<azalea_world::WorldName>()
        .ok()
        .map(|n| n.clone());

    let out = {
        let mut guard = RUN.lock().unwrap_or_else(PoisonError::into_inner);
        if guard.is_none() {
            drop(guard);
            s.set_enabled(Id::AutoBuild, false);
            say(shared, crate::lang::tr!("builder.no_schematic"));
            return false;
        }
        let Some(run) = guard.as_mut() else {
            return false;
        };
        match held(run, pos, &world, gated, hand) {
            Err((note, gate)) => {
                pause_run(bot, run);
                write_status(run, Some(&note), 0);
                let mut out = Out::default();
                gate_line(run, gate, &mut out);
                out
            }
            Ok(pos) => {
                run.gated_since = None;
                step(bot, shared, hit, run, s, pos)
            }
        }
    };
    FLY_KEYS.store(out.keys, Relaxed);
    for line in &out.lines {
        say(shared, line);
    }
    if out.toggle_fly {
        shared.lock().unwrap().session.fly_toggle = true;
    }
    if let Some(ended) = out.done {
        finish(bot, shared, Some(&ended));
        return false;
    }
    draw(out.target);
    publish(shared, out.look);
    out.attack
}

fn held(
    run: &mut Run,
    pos: Option<azalea_core::position::Vec3>,
    world: &Option<azalea_world::WorldName>,
    gated: bool,
    hand: bool,
) -> Result<[f64; 3], (String, Option<crate::lang::Key>)> {
    use crate::lang::Key;
    let Some(pos) = pos else {
        return Err(("Between worlds".to_string(), None));
    };
    if run.world != *world {
        return Err(("The schematic is in another world".to_string(), None));
    }
    if PAUSED.load(Relaxed) {
        return Err((
            run.hold.clone().unwrap_or_else(|| "Paused".to_string()),
            None,
        ));
    }
    if run.hold.take().is_some() {
        run.stalls = 0;
        run.fails.clear();
        run.watchdog.trips = 0;
    }
    let gate = if gated {
        const { Key::of("builder.gate.screen") }
    } else if hand {
        const { Key::of("builder.gate.walk_keys") }
    } else if pathfinder::user_route() {
        const { Key::of("builder.gate.route") }
    } else {
        return Ok([pos.x, pos.y, pos.z]);
    };
    Err((gate.english().to_string(), Some(gate)))
}

fn gate_line(run: &mut Run, gate: Option<crate::lang::Key>, out: &mut Out) {
    let now = crate::client::tracking::client_ticks();
    run.watchdog.since = now;
    let Some(gate) = gate else {
        run.gated_since = None;
        return;
    };
    let (since, said) = *run.gated_since.get_or_insert((now, false));
    if !said && now - since >= GATE_SAY_TICKS {
        out.lines
            .push(crate::lang::trf!("builder.gate_line", gate.get()));
        run.gated_since = Some((since, true));
    }
}

fn step(
    bot: &Client,
    shared: &SharedMutex,
    hit: Option<&BlockHitResult>,
    run: &mut Run,
    s: &Store,
    pos: [f64; 3],
) -> Out {
    let now = crate::client::tracking::client_ticks();
    let feet = player_pos_to_block_pos(azalea_core::position::Vec3::new(pos[0], pos[1], pos[2]));
    let eye_height = bot
        .component::<azalea::entity::dimensions::EntityDimensions>()
        .map_or(EYE, |d| f64::from(d.eye_height));
    let eye = [pos[0], pos[1] + eye_height, pos[2]];
    let look = sent::sent_view(bot).map_or(
        Look {
            yaw: 0.0,
            pitch: 0.0,
        },
        |(_, l)| l,
    );
    let held = bot
        .component::<Inventory>()
        .ok()
        .map(|inv| inv.held_item().kind());
    let slot = bot.selected_hotbar_slot().unwrap_or(0);
    let (vel, collided) = bot.component::<Physics>().map_or(([0.0; 3], false), |p| {
        (
            [p.velocity.x, p.velocity.y, p.velocity.z],
            p.horizontal_collision,
        )
    });
    let stock = Stock::read(bot);
    let creative = creative(bot, run, s, stock.creative);
    let here = Here {
        pos,
        feet,
        eye,
        now,
        look,
        held,
        slot,
        vel,
        collided,
        creative,
        air: s.flag(setting::AIR),
    };
    *ROUTE.lock().unwrap_or_else(PoisonError::into_inner) =
        Some((run.plan.clone(), Arc::new(stock.clone())));
    take_notes(run);
    let mode = s.choice(setting::BREAK);
    let fluids_changed = run.plan.set_fluids(s.flag(setting::FLUIDS));
    let changed = scan_tick(bot, run, feet, mode, fluids_changed);
    let ours: Vec<BlockPos> = changed
        .iter()
        .copied()
        .filter(|p| run.own.contains_key(p))
        .collect();
    forget_hopeless(run, &ours, (here.air, here.creative.fly), fluids_changed);
    run.own.retain(|_, until| *until > now);
    let foreign: Vec<BlockPos> = changed
        .into_iter()
        .filter(|p| !run.own.contains_key(p))
        .collect();
    run.sight.refresh(feet, eye, &foreign, NEAR_RADIUS);

    let mut out = Out::default();
    settle(bot, run, &here, &mut out);
    let stepping = nudge_tick(run, &here, &mut out);
    if out.lines.is_empty() && !stepping {
        for _ in 0..5 {
            if !stage_step(bot, shared, hit, run, s, &here, &stock, mode, &mut out)
                || out.done.is_some()
            {
                break;
            }
        }
    }
    if here.creative.on && here.creative.flying && !matches!(run.stage, Stage::Fly { .. }) {
        out.keys = match run.fly.hold {
            Some(at) => {
                let (at, slack) = match bot.world() {
                    Ok(world) => {
                        let w = world.read();
                        fly::clear_hold(at, |p| {
                            w.get_block_state(p)
                                .is_some_and(|s| !is_block_state_passable(s))
                        })
                    }
                    Err(_) => (at, [fly::HOLD_SLACK; 2]),
                };
                fly::hold_at(
                    here.pos,
                    here.vel,
                    at,
                    here.look.yaw,
                    here.creative.speed,
                    slack,
                    FLY_KEYS.load(Relaxed),
                )
            }
            None => fly::brake(here.vel, here.look.yaw),
        };
    }
    out.target = match run.stage {
        Stage::Break { pos, .. } => Some((pos, BREAK_COLOR)),
        Stage::Place { cell, .. } | Stage::Use { cell, .. } => Some((cell, PLACE_COLOR)),
        _ => None,
    };
    if out.done.is_none() {
        watchdog(bot, run, &here, &mut out);
    }
    write_status(run, None, now);
    out
}

fn nudge_tick(run: &mut Run, here: &Here, out: &mut Out) -> bool {
    use crate::session::MOVE_SNEAK;

    let Some(mut n) = run.nudge else {
        return false;
    };
    let on_the_way = matches!(
        run.stage,
        Stage::Travel { .. } | Stage::Fly { .. } | Stage::Spot
    );
    let other_cell = match run.stage {
        Stage::Place { cell, .. } | Stage::Use { cell, .. } => cell != n.cell,
        Stage::Break { pos, .. } => pos != n.cell,
        _ => false,
    };
    if !run.wrong.contains(n.cell)
        || other_cell
        || here.creative.flying
        || (here.feet != n.stand && !on_the_way)
    {
        run.nudge = None;
        return false;
    }
    if here.feet != n.stand || on_the_way {
        return false;
    }
    let sneak = if n.sneak { MOVE_SNEAK } else { 0 };
    out.keys |= sneak;
    if n.there {
        return false;
    }
    let started = *n.started.get_or_insert(here.now);
    let (dx, dz) = (n.feet[0] - here.pos[0], n.feet[1] - here.pos[2]);
    let close = (dx * dx + dz * dz).sqrt() <= NUDGE_CLOSE;
    if close || here.now - started > NUDGE_TICKS {
        n.there = true;
        run.nudge = Some(n);
        run.sight.forget(n.cell);
        return false;
    }
    out.keys |= sent::keys_toward(dx, dz, here.look.yaw).unwrap_or(0);
    run.nudge = Some(n);
    true
}

fn forget_hopeless(
    run: &mut Run,
    changed: &[BlockPos],
    settings: (bool, bool),
    fluids_changed: bool,
) {
    if settings != run.spot_settings || fluids_changed {
        run.spot_settings = settings;
        run.hopeless.clear();
        return;
    }
    if changed.is_empty() {
        return;
    }
    let within = |c: &BlockPos, q: &BlockPos, r: i32| {
        (c.x - q.x).abs() <= r && (c.y - q.y).abs() <= r && (c.z - q.z).abs() <= r
    };
    run.hopeless
        .retain(|c| !changed.iter().any(|q| within(c, q, HOPELESS_NEAR)));
    run.bad_spots
        .retain(|(c, s)| !changed.iter().any(|q| within(c, q, 1) || q == s));
}

fn watchdog(bot: &Client, run: &mut Run, here: &Here, out: &mut Out) {
    let moved = (0..3)
        .map(|i| (here.pos[i] - run.watchdog.pos[i]).powi(2))
        .sum::<f64>()
        .sqrt();
    let waiting = matches!(run.stage, Stage::Waiting { .. })
        || run.scan.last.is_none()
        || (matches!(run.stage, Stage::Break { .. }) && out.attack);
    if !(moved <= WATCHDOG_MOVE) || waiting || run.watchdog.since == i64::MIN {
        run.watchdog.pos = here.pos;
        run.watchdog.since = here.now;
        return;
    }
    if here.now - run.watchdog.since < WATCHDOG_TICKS {
        return;
    }
    let target = match run.stage {
        Stage::Break { pos, .. } => Some(pos),
        Stage::Place { cell, .. } | Stage::Use { cell, .. } => Some(cell),
        _ => run
            .spot
            .as_ref()
            .map(SpotSearch::cell)
            .or(run.expect)
            .or(run.batch.first().copied()),
    };
    if let Some(t) = target {
        if matches!(run.stage, Stage::Fly { .. }) {
            run.batch = vec![t];
            fly_failed(run, here);
        }
        run.strike_why(t, here.now, Why::Stuck);
    }
    pause_run(bot, run);
    run.watchdog.since = here.now;
    run.watchdog.pos = here.pos;
    if run.watchdog_target != target {
        run.watchdog.trips = 0;
        run.watchdog_target = target;
    }
    run.watchdog.trips += 1;
    if run.watchdog.trips >= WATCHDOG_TRIPS && target.is_none() {
        use crate::lang::Key;
        let mut args = vec![
            (WATCHDOG_TICKS / 20).to_string(),
            WATCHDOG_TRIPS.to_string(),
        ];
        let key = match target {
            Some(t) => {
                args.extend([t.x.to_string(), t.y.to_string(), t.z.to_string()]);
                const { Key::of("builder.watchdog.at") }
            }
            None => const { Key::of("builder.watchdog") },
        };
        let shown = crate::util::placeholders::substitute(key.get(), &args);
        out.lines.push(crate::lang::trf!("builder.stopped", shown));
        run.hold(&crate::util::placeholders::substitute(key.english(), &args));
    }
}

#[allow(clippy::too_many_arguments)]
fn stage_step(
    bot: &Client,
    shared: &SharedMutex,
    hit: Option<&BlockHitResult>,
    run: &mut Run,
    s: &Store,
    here: &Here,
    stock: &Stock,
    mode: BreakMode,
    out: &mut Out,
) -> bool {
    match run.stage {
        Stage::Choose => choose(bot, shared, run, s, here, stock, mode, out),
        Stage::Spot => spot_step(bot, run, s, stock, here, out),
        Stage::Break { .. } => break_step(bot, run, hit, here, out),
        Stage::Place { .. } => place_step(bot, shared, run, here, mode, out),
        Stage::Travel { idle_at, at } => travel_step(bot, run, here, idle_at, at),
        Stage::Fly { watch } => fly_step(bot, run, here, watch, out),
        Stage::Use { .. } => use_step(bot, shared, run, here, out),
        Stage::Waiting { recheck_at } => {
            if here.now >= recheck_at {
                run.stage = Stage::Choose;
                true
            } else {
                false
            }
        }
    }
}

fn creative(bot: &Client, run: &mut Run, s: &Store, on: bool) -> Creative {
    let (flying, speed) =
        bot.component::<PlayerAbilities>()
            .map_or((false, fly::DEFAULT_SPEED), |a| {
                let speed = if a.flying_speed > 0.0 {
                    f64::from(a.flying_speed)
                } else {
                    fly::DEFAULT_SPEED
                };
                (a.flying, speed)
            });
    let fly = on && s.flag(setting::FLY) && !run.fly.refused;
    Creative {
        on,
        fly,
        flying,
        speed,
    }
}

fn scan_tick(
    bot: &Client,
    run: &mut Run,
    feet: BlockPos,
    mode: BreakMode,
    fluids_changed: bool,
) -> Vec<BlockPos> {
    let (changed, lost) = take_changed();
    if lost {
        run.sight.reset();
        run.hopeless.clear();
    }
    let Ok(world) = bot.world() else {
        return changed;
    };
    let fresh = run.mode != mode || fluids_changed;
    if fresh {
        run.mode = mode;
        run.wrong.clear();
    }
    let generation = crate::client::worldsync::chunk_generation();
    run.scan
        .restart_if(generation, run.wrong.len(), WRONG_CAP, lost || fresh);
    {
        let w = world.read();
        let at = |p: BlockPos| w.get_block_state(p);
        let mut kept = true;
        for pos in &changed {
            kept &= plan::recheck(&run.plan, &mut run.wrong, *pos, mode, feet, &at);
        }
        let r = NEAR_RADIUS;
        for dy in -r..=r {
            for dz in -r..=r {
                for dx in -r..=r {
                    let pos = BlockPos::new(feet.x + dx, feet.y + dy, feet.z + dz);
                    kept &= plan::recheck(&run.plan, &mut run.wrong, pos, mode, feet, &at);
                }
            }
        }
        if !kept {
            run.scan.overflowed();
        }
    }
    let start = Instant::now();
    while !run.scan.complete() && start.elapsed() < SCAN_BUDGET {
        let w = world.read();
        run.scan
            .batch(&run.plan, &mut run.wrong, feet, mode, SCAN_BATCH, &|p| {
                w.get_block_state(p)
            });
    }
    changed
}

fn take_changed() -> (Vec<BlockPos>, bool) {
    let mut c = CHANGED.lock().unwrap_or_else(PoisonError::into_inner);
    let lost = std::mem::take(&mut c.lost);
    (std::mem::take(&mut c.cells), lost)
}

fn take_notes(run: &mut Run) {
    let notes = std::mem::take(&mut *NOTES.lock().unwrap_or_else(PoisonError::into_inner));
    for cell in notes {
        if run.plan.want(cell).is_none_or(state_is_air) {
            run.scaffold.insert(cell);
        }
    }
}

fn settle(bot: &Client, run: &mut Run, here: &Here, out: &mut Out) {
    let now = here.now;
    let due: Vec<(BlockPos, Expect)> = run
        .settling
        .iter()
        .filter(|(_, _, at)| *at <= now)
        .map(|&(c, e, _)| (c, e))
        .collect();
    run.settling.retain(|(_, _, at)| *at > now);
    for (cell, expect) in due {
        let right = match expect {
            Expect::Block(want) => {
                state_at(bot, cell).is_some_and(|w| place::placement_fits(w, want))
            }
            Expect::Exact(want) => {
                state_at(bot, cell).is_some_and(|w| w == want || place::same_placement(w, want))
            }
            Expect::Drained => state_at(bot, cell).is_some_and(|w| !plan::source(w)),
            Expect::Gone => !present(bot, cell),
        };
        let trip = run.fails.note(right, cell);
        let shot = run.shots.remove(&cell);
        let _ = shot;
        if right {
            continue;
        }
        run.sight.reset();
        if run.marks(cell).places >= PLACE_TRIES {
            run.skip_why(
                cell,
                if expect == Expect::Gone {
                    Why::NotBreaking
                } else {
                    Why::Refused
                },
            );
        }
        if let Some(trip) = trip {
            let (key, args) = trip_parts(trip, cell, here.air);
            let shown = crate::util::placeholders::substitute(key.get(), &args);
            out.lines.push(crate::lang::trf!("builder.stopped", shown));
            pause_run(bot, run);
            run.hold(&trip_note(trip, cell, here.air));
            return;
        }
    }
}

fn trip_note(trip: Trip, at: BlockPos, air: bool) -> String {
    let (key, args) = trip_parts(trip, at, air);
    crate::util::placeholders::substitute(key.english(), &args)
}

fn trip_parts(trip: Trip, at: BlockPos, air: bool) -> (crate::lang::Key, Vec<String>) {
    use crate::lang::Key;
    let (key, mut args) = match (trip, air) {
        (Trip::InRow(n), false) => (
            const { Key::of("builder.trip.in_row") },
            vec![n.to_string()],
        ),
        (Trip::InRow(n), true) => (
            const { Key::of("builder.trip.in_row.air") },
            vec![n.to_string()],
        ),
        (Trip::Ratio { ok }, false) => (
            const { Key::of("builder.trip.ratio") },
            vec![ok.to_string(), FAIL_WINDOW.to_string()],
        ),
        (Trip::Ratio { ok }, true) => (
            const { Key::of("builder.trip.ratio.air") },
            vec![ok.to_string(), FAIL_WINDOW.to_string()],
        ),
    };
    args.extend([at.x.to_string(), at.y.to_string(), at.z.to_string()]);
    (key, args)
}

#[derive(Clone, Copy)]
struct Cand {
    pos: BlockPos,
    want: BlockState,
    world: BlockState,
    job: Job,
}

enum Clicks {
    Ready(Vec<(BlockPos, BlockState, ItemKind, Click, bool)>),
    Solving,
    None,
}

struct Found {
    cands: Vec<Cand>,
    brk: Option<(BlockPos, BlockState, [f64; 3])>,
    usable: Option<(BlockPos, BlockState, [f64; 3])>,
    clicks: Clicks,
    far: Vec<(BlockPos, u8)>,
    unsolved: Vec<(Cand, u8)>,
    floating: Vec<BlockPos>,
    supported: bool,
}

#[allow(clippy::too_many_arguments)]
fn choose(
    bot: &Client,
    shared: &SharedMutex,
    run: &mut Run,
    s: &Store,
    here: &Here,
    stock: &Stock,
    mode: BreakMode,
    out: &mut Out,
) -> bool {
    let Ok(world) = bot.world() else {
        return false;
    };
    if run
        .nudge
        .is_some_and(|n| !n.there && here.feet == n.stand && !here.creative.flying)
    {
        return false;
    }
    let fast = s.choice(setting::MODE) == BuildMode::Fast;
    let clicks_wanted = if fast {
        (s.num(setting::PER_TICK) as usize).clamp(1, 8)
    } else {
        1
    };
    let plug = if stock.has(ItemKind::Bucket) {
        None
    } else {
        support_block(bot, here.creative.on)
    };
    let found = {
        let w = world.read();
        look_around(&w, run, here, stock, mode, fast, clicks_wanted, plug)
    };
    run.missing = missing(&found.cands, stock);

    if let Some((pos, state, aim)) = found.brk {
        if run.expect == Some(pos) {
            run.expect = None;
        }
        run.stage = break_stage(bot, s, pos, state, aim, here);
        return true;
    }
    if let Some((cell, want, aim)) = found.usable {
        if run.expect == Some(cell) {
            run.expect = None;
        }
        let (lo, hi) = s.range(setting::TURN);
        let turn = roll(lo, hi);
        let need = state_at(bot, cell)
            .and_then(|w| place::use_clicks(w, want))
            .unwrap_or(1);
        run.stage = Stage::Use {
            cell,
            want,
            aim,
            turn,
            wait_until: here.now + delay_ticks(s),
            give_up_at: here.now + turn_ticks(180.0, turn),
            clicks: 0,
            budget: (need + USE_SPARE).min(u8::MAX as usize) as u8,
        };
        return true;
    }
    if run.refill.filling && refill(bot, run, s, stock, here, &found.cands, out) {
        return false;
    }
    match found.clicks {
        Clicks::Ready(clicks) => {
            if clicks.iter().any(|c| Some(c.0) == run.expect) {
                run.expect = None;
            }
            let bucket = clicks.first().is_some_and(|c| is_bucket(c.2));
            if fast && !bucket {
                let blocks: Vec<_> = clicks.into_iter().filter(|c| !is_bucket(c.2)).collect();
                if fast_place(bot, shared, run, blocks, here) > 0 {
                    run.fast_at = here.now;
                }
                return false;
            }
            if let Some((cell, want, item, click, air)) = clicks.into_iter().next() {
                let fragile = sensitive(&world.read(), item, &click, 90.0, 0.35);
                run.stage = place_stage(s, cell, want, item, &click, air, fragile, here);
                run.click = Some(click);
                run.click_eye = Some(here.eye);
                return true;
            }
            return false;
        }
        Clicks::Solving => {
            run.idle = Idle::Trying;
            return false;
        }
        Clicks::None => {}
    }

    if let Some(cell) = run.expect.take()
        && let Some(c) = found.cands.iter().find(|c| c.pos == cell)
    {
        if let Some((for_cell, spot)) = run.last_spot.take()
            && for_cell == cell
        {
            run.bad_spots.insert((cell, spot));
        }
        let why = click_why(c.want, job_item(c.job, c.want));
        if matches!(why, Why::LookUp | Why::LookDown) {
            let t = Target {
                cell,
                job: c.job,
                want: c.want,
                item: job_item(c.job, c.want),
                air: here.air,
            };
            let before = run.supports.len();
            add_support(bot, run, here, &t);
            let new = run.supports.len() > before;
            if new {
                return true;
            }
        }
        run.strike_why(cell, here.now, why);
        return true;
    }
    let boxed = found
        .cands
        .iter()
        .any(|c| c.job == Job::Place && in_player_box(c.pos, here.pos));
    if boxed
        && !here.creative.flying
        && run
            .stepped_out
            .is_none_or(|(at, t)| at != here.feet || here.now - t > UNREACHABLE_TICKS)
    {
        let to = {
            let w = world.read();
            step_out_cell(&w, &run.plan, here.feet)
        };
        if let Some(to) = to {
            run.stepped_out = Some((here.feet, here.now));
            send_route(bot, run, s, stock, QuietGoal::Block(to), Vec::new(), here);
            return false;
        }
    }
    let batch = rules::batch(&found.far, here.feet, GOAL_CELLS);
    if !batch.is_empty() {
        if !throwaway_ready(bot, run, here) {
            return false;
        }
        let goal = QuietGoal::Within {
            blocks: batch.clone(),
            range: GOAL_RANGE,
        };
        send_route(bot, run, s, stock, goal, batch, here);
        return false;
    }
    if let Some(&(c, _)) = found
        .unsolved
        .iter()
        .min_by_key(|(c, strikes)| (c.pos.y, dist2(c.pos, here.feet), *strikes))
    {
        let seen = run.loops.entry((c.pos, here.feet)).or_default();
        *seen += 1;
        if *seen > LOOP_LIMIT {
            run.loops.remove(&(c.pos, here.feet));
            run.strike_why(c.pos, here.now, spot_why(c.want, job_item(c.job, c.want)));
            return true;
        }
        let m = run.marks.get(&c.pos).copied().unwrap_or_default();
        let flies = here.creative.fly && m.no_fly_until <= here.now;
        let walks = m.no_walk_until <= here.now;
        let walk = walks && spot::walk_first(here.creative.flying, here.feet, c.pos);
        let passes = if flies && !walks {
            spot::hover_passes()
        } else {
            spot::passes(flies, walk)
        };
        let pending = layer_near(&found.cands, c.pos);
        let over = hover_point(&pending).unwrap_or([
            f64::from(c.pos.x) + 0.5,
            f64::from(c.pos.y) + 1.0,
            f64::from(c.pos.z) + 0.5,
        ]);
        let target = Target {
            cell: c.pos,
            job: c.job,
            want: c.want,
            item: job_item(c.job, c.want),
            air: here.air,
        };
        let bad: Vec<BlockPos> = run
            .bad_spots
            .iter()
            .filter(|(cell, _)| *cell == c.pos)
            .map(|(_, p)| *p)
            .collect();
        run.spot = Some(
            SpotSearch::new(target, passes, over)
                .with_holes(run.holes.keys().copied())
                .with_bad(bad.into_iter()),
        );
        run.stage = Stage::Spot;
        return true;
    }
    let hopeless: Vec<BlockPos> = found
        .cands
        .iter()
        .map(|c| c.pos)
        .filter(|p| run.hopeless.contains(p) && !run.away(*p, here.now))
        .collect();
    if !hopeless.is_empty() {
        let top = top_why(
            hopeless
                .iter()
                .map(|p| run.why.get(p).copied().unwrap_or(Why::NoSpot)),
        );
        run.idle = Idle::NoWay(hopeless.len() as u32, top);
        for p in hopeless {
            run.strike(p, here.now);
        }
        return false;
    }
    if !run.missing.is_empty() {
        run.idle = Idle::Trying;
        if refill(bot, run, s, stock, here, &found.cands, out) {
            return false;
        }
        run.stage = Stage::Waiting {
            recheck_at: here.now + WAIT_TICKS,
        };
        return false;
    }
    if !found.floating.is_empty() && !found.supported {
        if !run.settling.is_empty() {
            run.idle = Idle::Settling;
            return false;
        }
        for p in &found.floating {
            run.unsupported.insert(*p);
            run.skip_why(*p, Why::NoSupport);
        }
        return true;
    }
    if !found.cands.is_empty() {
        run.idle = Idle::Away(found.cands.len() as u32);
        if run.settling.is_empty() {
            let mut back = 0;
            for c in &found.cands {
                if let Some(m) = run.marks.get_mut(&c.pos)
                    && m.away_until > here.now
                {
                    m.away_until = here.now;
                    back += 1;
                }
            }
            return back > 0;
        }
        return false;
    }
    if !run.scan.complete() {
        run.idle = Idle::Checking;
        return false;
    }
    if !run.scan.clean() {
        if let Some(u) = run.scan.last.and_then(|p| p.nearest_unseen) {
            if here.creative.fly {
                fly_toward_unseen(bot, run, here, u);
            } else {
                send_route(
                    bot,
                    run,
                    s,
                    stock,
                    QuietGoal::Within {
                        blocks: vec![u],
                        range: UNSEEN_RANGE,
                    },
                    vec![u],
                    here,
                );
            }
        } else {
            run.idle = Idle::Checking;
        }
        return false;
    }
    if !run.retried {
        run.retried = true;
        let back = {
            let w = world.read();
            retry_skipped(&w, run, here)
        };
        if back > 0 {
            return true;
        }
    }
    if !run.finishing && s.flag(setting::SCAFFOLD) {
        run.finishing = true;
        run.scrap = std::mem::take(&mut run.scaffold);
        return !run.scrap.is_empty();
    }
    out.done = Some(Ended {
        chat: done_line(run, false),
        note: done_line(run, true),
    });
    false
}

#[allow(clippy::too_many_arguments)]
fn look_around(
    w: &World,
    run: &mut Run,
    here: &Here,
    stock: &Stock,
    mode: BreakMode,
    fast: bool,
    clicks_wanted: usize,
    plug: Option<BlockState>,
) -> Found {
    let mut all = survey(w, run, mode, plug);
    stand_holes(w, run, &mut all, stock, here.air, mode);
    let floats = |c: &Cand| match c.job {
        Job::Place => !gravity_ok(w, c) || (!here.air && !has_support(w, c.pos)),
        Job::Pour => source(c.want) && !has_support(w, c.pos),
        _ => false,
    };
    let fluids = run.plan.fluids();
    let wrong = &run.wrong;
    let phase_of = |c: &Cand| -> Option<u8> {
        if c.job != Job::Pour {
            return Some(plan::phase(c.want, c.job));
        }
        let at = |p: BlockPos| w.get_block_state(p);
        let (walls, exposed) = contained(&run.plan, c.pos, fluids, &at, &|p| wrong.is_skipped(p));
        match (walls, exposed) {
            (false, _) => None,
            (true, true) => Some(EXPOSED_PHASE),
            (true, false) => Some(plan::phase(c.want, c.job)),
        }
    };
    let filling = run.refill.filling;
    let has_item = |c: &Cand| job_item(c.job, c.want).is_none_or(|i| usable(stock, filling, i));
    let progress = |c: &Cand| {
        !floats(c) && !run.away(c.pos, here.now) && !run.hopeless.contains(&c.pos) && has_item(c)
    };
    let phases: Vec<Option<u8>> = all.iter().map(phase_of).collect();
    let pass = all
        .iter()
        .zip(&phases)
        .filter(|(c, _)| progress(*c))
        .filter_map(|(_, p)| *p)
        .min()
        .or_else(|| {
            all.iter()
                .zip(&phases)
                .filter(|(c, _)| !floats(*c))
                .filter_map(|(_, p)| *p)
                .min()
        })
        .unwrap_or(0);
    let cands: Vec<Cand> = all
        .iter()
        .zip(&phases)
        .filter(|(c, p)| **p == Some(pass) || floats(*c))
        .map(|(c, _)| *c)
        .collect();
    let brk = break_from_here(w, run, &cands, here);
    let to_use = if brk.is_none() {
        use_from_here(w, run, &cands, here)
    } else {
        None
    };
    let clicks = if brk.is_none() && to_use.is_none() {
        clicks_from_here(w, run, &cands, here, stock, fast, clicks_wanted)
    } else {
        Clicks::None
    };
    let mut far = Vec::new();
    let mut unsolved = Vec::new();
    let mut floating = Vec::new();
    let mut supported = false;
    for (c, phase) in all.iter().zip(&phases) {
        match c.job {
            Job::Break => {
                if !breakable(&mut run.breakable, c.world) {
                    run.skip_why(c.pos, Why::NotBreaking);
                    continue;
                }
            }
            Job::Use => {}
            Job::Place | Job::Pour | Job::Drain => {
                if !has_item(c) {
                    continue;
                }
                if floats(c) {
                    floating.push(c.pos);
                    continue;
                }
            }
        }
        supported = true;
        if *phase != Some(pass) {
            continue;
        }
        let m = run.marks.get(&c.pos).copied().unwrap_or_default();
        if m.away_until > here.now || run.hopeless.contains(&c.pos) {
            continue;
        }
        if in_reach(c.pos, here.eye) {
            if run.sight.tried(c.pos) {
                unsolved.push((*c, m.strikes));
            }
        } else if m.routed || here.creative.fly {
            unsolved.push((*c, m.strikes));
        } else {
            far.push((c.pos, m.strikes));
        }
    }
    Found {
        cands: all,
        brk,
        usable: to_use,
        clicks,
        far,
        unsolved,
        floating,
        supported,
    }
}

fn use_from_here(
    w: &World,
    run: &mut Run,
    cands: &[Cand],
    here: &Here,
) -> Option<(BlockPos, BlockState, [f64; 3])> {
    let mut near: Vec<(f64, Cand)> = cands
        .iter()
        .filter(|c| c.job == Job::Use && !run.away(c.pos, here.now) && !run.sight.tried(c.pos))
        .filter(|c| in_reach(c.pos, here.eye))
        .map(|c| (eye_d2(c.pos, here.eye), *c))
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, c) in near {
        match aim_at(&w.chunks, here.eye, c.pos, c.world) {
            Some(aim) => return Some((c.pos, c.want, aim)),
            None => run.sight.fail(c.pos),
        }
    }
    None
}

fn stand_holes(
    w: &World,
    run: &mut Run,
    out: &mut Vec<Cand>,
    stock: &Stock,
    air: bool,
    mode: BreakMode,
) {
    use plan::HoleKind;

    if !air {
        let targets: Vec<(BlockPos, BlockState)> = out
            .iter()
            .filter(|c| c.job == Job::Place)
            .map(|c| (c.pos, c.want))
            .collect();
        for (t, want) in targets {
            if run.no_hole.contains(&t) || run.holes.values().any(|h| h.target == t) {
                continue;
            }
            let need = *run.vertical.entry(want).or_insert_with(|| {
                stock::item_for(want)
                    .and_then(|i| place::vertical_need(&ItemStack::new(i, 1), want))
            });
            if need != Some(true) {
                continue;
            }
            let plan = run.plan.clone();
            let cache = std::cell::RefCell::new(std::mem::take(&mut run.breakable));
            let held = |p: BlockPos| run.holes.contains_key(&p);
            let may_dig = |p: BlockPos, state: BlockState| {
                mode == BreakMode::All
                    && !held(p)
                    && breakable(&mut cache.borrow_mut(), state)
                    && !crate::play::placement::has_use_interaction(state)
                    && !plan::hole_forbidden(state)
                    && [
                        (0, 0, 0),
                        (1, 0, 0),
                        (-1, 0, 0),
                        (0, 1, 0),
                        (0, -1, 0),
                        (0, 0, 1),
                        (0, 0, -1),
                    ]
                    .iter()
                    .all(|(x, y, z)| {
                        w.get_block_state(BlockPos::new(p.x + x, p.y + y, p.z + z))
                            .is_none_or(|s| !plan::pure_fluid(s) && !plan::water_source(s))
                    })
            };
            let choice =
                plan::choose_hole(t, &|p| plan.want(p), &|p| w.get_block_state(p), &may_dig);
            run.breakable = cache.into_inner();
            match choice {
                Some((h, kind)) => {
                    let original = if kind == HoleKind::Dig && plan.want(h).is_none() {
                        w.get_block_state(h)
                    } else {
                        None
                    };
                    run.holes.insert(
                        h,
                        Hole {
                            target: t,
                            kind,
                            original,
                            dug: false,
                        },
                    );
                }
                None => {
                    run.no_hole.insert(t);
                }
            }
        }
    }
    let holes: Vec<(BlockPos, Hole)> = run.holes.iter().map(|(p, h)| (*p, *h)).collect();
    for (h, mut hole) in holes {
        let Some(world) = w.get_block_state(h) else {
            continue;
        };
        let t = hole.target;
        let settled = !run.settling.iter().any(|s| s.0 == t);
        let right = run
            .plan
            .locate(t)
            .is_some_and(|(spot, _)| run.plan.is_correct(spot));
        let done = run.wrong.is_skipped(t) || (!run.wrong.contains(t) && settled && right);
        if !done {
            out.retain(|c| c.pos != h);
            if holds(world) {
                if hole.kind == HoleKind::Dig {
                    out.push(Cand {
                        pos: h,
                        want: BlockState::AIR,
                        world,
                        job: Job::Break,
                    });
                }
            } else if !hole.dug {
                hole.dug = true;
                run.holes.insert(h, hole);
            }
            continue;
        }
        match hole.original {
            Some(orig) if !holds(world) => {
                if stock::item_for(orig).is_some_and(|i| stock.has(i)) {
                    out.push(Cand {
                        pos: h,
                        want: orig,
                        world,
                        job: Job::Place,
                    });
                } else {
                    run.left_holes.push(h);
                    run.holes.remove(&h);
                }
            }
            _ => {
                run.holes.remove(&h);
            }
        }
    }
}

fn survey(w: &World, run: &mut Run, mode: BreakMode, plug: Option<BlockState>) -> Vec<Cand> {
    let mut out = Vec::new();
    let fluids = run.plan.fluids();
    let cells: Vec<BlockPos> = run.wrong.iter().collect();
    for pos in cells {
        let Some((spot, want)) = run.plan.locate(pos) else {
            run.wrong.remove(pos);
            continue;
        };
        let Some(world) = w.get_block_state(pos) else {
            continue;
        };
        if judge(world, want, mode, fluids) != Verdict::Wrong {
            run.plan.mark(spot, true);
            run.wrong.remove(pos);
            continue;
        }
        let job = job(world, want, fluids);
        if job == Job::Place && stock::item_for(want).is_none() {
            run.skip_why(pos, Why::NoItem);
            continue;
        }
        if job == Job::Drain
            && let Some(block) = plug
        {
            out.push(Cand {
                pos,
                want: block,
                world,
                job: Job::Place,
            });
            continue;
        }
        if job == Job::Break && run.supports.contains_key(&pos) {
            continue;
        }
        if job == Job::Place && plan::partner_half(want) {
            if run.wrong.is_skipped(plan::partner_base(pos, want)) {
                run.skip(pos);
            }
            continue;
        }
        out.push(Cand {
            pos,
            want,
            world,
            job,
        });
    }
    survey_supports(w, run, &mut out);
    let scrap: Vec<BlockPos> = run.scrap.iter().copied().collect();
    for pos in scrap {
        match w.get_block_state(pos) {
            Some(world) if solid(world) => {
                if !run.wrong.contains(pos) {
                    out.push(Cand {
                        pos,
                        want: BlockState::AIR,
                        world,
                        job: Job::Break,
                    });
                }
            }
            Some(_) => {
                run.scrap.remove(&pos);
            }
            None => {}
        }
    }
    out
}

fn survey_supports(w: &World, run: &mut Run, out: &mut Vec<Cand>) {
    let supports: Vec<(BlockPos, Support)> = run.supports.iter().map(|(p, s)| (*p, *s)).collect();
    for (pos, sup) in supports {
        let Some(world) = w.get_block_state(pos) else {
            continue;
        };
        if run.wrong.is_skipped(pos) {
            run.supports.remove(&pos);
            run.support_failed.insert(pos);
            continue;
        }
        if !run.wrong.contains(sup.cell) {
            run.supports.remove(&pos);
            if holds(world) {
                run.scaffold.insert(pos);
            }
            continue;
        }
        if job(world, sup.state, false) == Job::Place && !holds(world) {
            out.push(Cand {
                pos,
                want: sup.state,
                world,
                job: Job::Place,
            });
        }
    }
}

const SUPPORT_ACROSS: [f64; 3] = [0.5, 0.25, 0.75];

const SUPPORT_YAWS: [f32; 8] = [0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0];
const SUPPORT_PITCHES: [f32; 4] = [30.0, 60.0, -30.0, -60.0];

#[allow(clippy::too_many_arguments)]
fn support_for(
    w: &World,
    plan: &Plan,
    cell: BlockPos,
    want: BlockState,
    item: ItemKind,
    support: BlockState,
    failed: &HashSet<BlockPos>,
) -> Option<BlockPos> {
    use azalea_core::direction::Direction;
    use azalea_core::position::Vec3;

    let held = ItemStack::new(item, 1);
    let (min_y, height) = (w.chunks.min_y(), w.chunks.height());
    let dirs = [
        Direction::East,
        Direction::West,
        Direction::South,
        Direction::North,
        Direction::Down,
        Direction::Up,
    ];
    dirs.into_iter().find_map(|d| {
        let n = cell + d.normal();
        let free = w.get_block_state(n).is_some_and(|s| !holds(s));
        if !free || failed.contains(&n) || plan.want(n).is_some_and(|s| !state_is_air(s)) {
            return None;
        }
        if !dirs
            .iter()
            .map(|e| n + e.normal())
            .any(|m| m != cell && w.get_block_state(m).is_some_and(holds))
        {
            return None;
        }
        let nv = d.normal();
        let axis = match d {
            Direction::East | Direction::West => 0,
            Direction::Up | Direction::Down => 1,
            Direction::North | Direction::South => 2,
        };
        let others: [usize; 2] = match axis {
            0 => [1, 2],
            1 => [0, 2],
            _ => [0, 1],
        };
        let base = [f64::from(cell.x), f64::from(cell.y), f64::from(cell.z)];
        let block_at = |p: BlockPos| {
            if p == n {
                Some(support)
            } else {
                w.get_block_state(p)
            }
        };
        let works = SUPPORT_ACROSS.iter().any(|&a| {
            SUPPORT_ACROSS.iter().any(|&b| {
                let mut point = [0.0; 3];
                point[axis] = base[axis] + 0.5 + f64::from([nv.x, nv.y, nv.z][axis]) * 0.5;
                point[others[0]] = base[others[0]] + a;
                point[others[1]] = base[others[1]] + b;
                let hit = BlockHitResult {
                    location: Vec3::new(point[0], point[1], point[2]),
                    miss: false,
                    direction: d.opposite(),
                    block_pos: n,
                    inside: false,
                    world_border: false,
                };
                SUPPORT_YAWS.iter().any(|&yaw| {
                    SUPPORT_PITCHES.iter().any(|&pitch| {
                        crate::play::placement::predict_placed(
                            &held, &hit, yaw, pitch, false, min_y, height, block_at,
                        )
                        .is_some_and(|p| p.pos == cell && place::placement_fits(p.state, want))
                    })
                })
            })
        });
        works.then_some(n)
    })
}

fn hangs(want: BlockState) -> bool {
    use azalea::block::BlockTrait;
    use azalea::physics::collision::BlockWithShape;
    let id = want
        .as_block_kind()
        .to_str()
        .trim_start_matches("minecraft:");
    let block = Box::<dyn BlockTrait>::from(want);
    want.is_collision_shape_empty()
        || id.contains("wall_")
        || ["face", "attachment", "hanging"]
            .iter()
            .any(|p| block.get_property(p).is_some())
}

fn support_block(bot: &Client, creative: bool) -> Option<BlockState> {
    use std::str::FromStr;
    let item = pathfinder::throwaway(bot).or_else(|| {
        if !creative {
            return None;
        }
        let list = &pathfinder::THROWAWAY;
        (0..list.count())
            .filter(|&i| list.enabled(i))
            .find_map(|i| ItemKind::from_str(list.label(i)).ok())
    })?;
    azalea_registry::builtin::BlockKind::from_str(item.to_str())
        .ok()
        .map(BlockState::from)
}

fn add_support(bot: &Client, run: &mut Run, here: &Here, t: &Target) -> bool {
    let (Job::Place, Some(item)) = (t.job, t.item) else {
        return false;
    };
    if here.air || run.supports.contains_key(&t.cell) || hangs(t.want) {
        return false;
    }
    if let Some((&at, sup)) = run.supports.iter().find(|(_, s)| s.cell == t.cell) {
        let down = bot
            .world()
            .ok()
            .is_some_and(|w| w.read().get_block_state(at).is_some_and(|s| s == sup.state));
        if down {
            return false;
        }
        run.marks(t.cell).away_until = here.now + SUPPORT_WAIT;
        return true;
    }
    let Some(block) = support_block(bot, here.creative.on) else {
        return false;
    };
    let Ok(world) = bot.world() else {
        return false;
    };
    let at = {
        let w = world.read();
        let mut avoid = run.support_failed.clone();
        for h in run.holes.keys() {
            avoid.insert(*h);
            avoid.insert(BlockPos::new(h.x, h.y + 1, h.z));
        }
        support_for(&w, &run.plan, t.cell, t.want, item, block, &avoid)
    };
    let Some(at) = at else {
        return false;
    };
    run.supports.insert(
        at,
        Support {
            state: block,
            cell: t.cell,
        },
    );
    run.marks(t.cell).away_until = here.now + SUPPORT_WAIT;
    true
}

fn retry_skipped(w: &World, run: &mut Run, here: &Here) -> usize {
    let plan = run.plan.clone();
    let fluids = plan.fluids();
    let cells = run.wrong.take_skipped(|p| {
        let (Some(want), Some(world)) = (plan.want(p), w.get_block_state(p)) else {
            return false;
        };
        let job = job(world, want, fluids);
        matches!(job, Job::Break | Job::Use | Job::Drain) || here.air || has_support(w, p)
    });
    for p in &cells {
        run.marks.remove(p);
        run.unsupported.remove(p);
        run.hopeless.remove(p);
        run.wrong.insert(*p, here.feet);
    }
    cells.len()
}

fn break_from_here(
    w: &World,
    run: &mut Run,
    cands: &[Cand],
    here: &Here,
) -> Option<(BlockPos, BlockState, [f64; 3])> {
    let mut near: Vec<(f64, Cand)> = cands
        .iter()
        .filter(|c| c.job == Job::Break && !run.away(c.pos, here.now) && !run.sight.tried(c.pos))
        .filter(|c| in_reach(c.pos, here.eye))
        .map(|c| (eye_d2(c.pos, here.eye), *c))
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, c) in near {
        if !breakable(&mut run.breakable, c.world)
            || run
                .marks
                .get(&c.pos)
                .is_some_and(|m| m.breaks >= BREAK_TRIES)
        {
            run.skip_why(c.pos, Why::NotBreaking);
            continue;
        }
        if c.pos.x == here.feet.x && c.pos.z == here.feet.z && c.pos.y < here.feet.y {
            run.sight.fail(c.pos);
            continue;
        }
        match aim_at(&w.chunks, here.eye, c.pos, c.world) {
            Some(aim) => return Some((c.pos, c.world, aim)),
            None => run.sight.fail(c.pos),
        }
    }
    None
}

fn clicks_from_here(
    w: &World,
    run: &mut Run,
    cands: &[Cand],
    here: &Here,
    stock: &Stock,
    fast: bool,
    wanted: usize,
) -> Clicks {
    let mut live: Vec<(Cand, ItemKind)> = Vec::new();
    for c in cands
        .iter()
        .filter(|c| matches!(c.job, Job::Place | Job::Pour | Job::Drain))
    {
        let Some(item) = job_item(c.job, c.want).filter(|i| usable(stock, run.refill.filling, *i))
        else {
            continue;
        };
        let unheld = match c.job {
            Job::Place => !gravity_ok(w, c) || (!here.air && !has_support(w, c.pos)),
            Job::Pour => source(c.want) && !has_support(w, c.pos),
            _ => false,
        };
        if run.away(c.pos, here.now)
            || !in_reach(c.pos, here.eye)
            || run.sight.tried(c.pos)
            || unheld
        {
            continue;
        }
        if run
            .marks
            .get(&c.pos)
            .is_some_and(|m| m.places >= PLACE_TRIES)
        {
            run.skip_why(c.pos, Why::Refused);
            continue;
        }
        live.push((*c, item));
    }
    let mut unknown: Vec<(Cand, ItemKind)> = live
        .iter()
        .filter(|(c, _)| !run.sight.known(c.pos))
        .copied()
        .collect();
    unknown.sort_by_key(|(c, _)| (c.pos.y, dist2(c.pos, here.feet)));
    for (c, item) in unknown.into_iter().take(SOLVES_PER_TICK) {
        let t = Target {
            cell: c.pos,
            job: c.job,
            want: c.want,
            item: Some(item),
            air: here.air,
        };
        match clicks_at(w, &t, here.eye, !fast || c.job != Job::Place) {
            Reach::Clicks(clicks, true) => run.sight.solved_air(c.pos, clicks),
            Reach::Clicks(clicks, false) => run.sight.solved(c.pos, clicks),
            _other => {
                if matches!(_other, Reach::Lava) {
                    run.why.insert(c.pos, Why::Lava);
                }
                run.sight.fail(c.pos);
            }
        }
    }
    live.retain(|(c, _)| !run.sight.tried(c.pos));
    let Some(layer) = live.iter().map(|(c, _)| c.pos.y).min() else {
        return Clicks::None;
    };
    live.retain(|(c, _)| c.pos.y == layer);
    if live.iter().any(|(c, _)| !run.sight.known(c.pos)) {
        return Clicks::Solving;
    }
    Clicks::Ready(order(run, &live, here, fast, wanted))
}

fn usable(stock: &Stock, filling: bool, item: ItemKind) -> bool {
    stock.has(item) && !(filling && item == ItemKind::WaterBucket)
}

#[derive(Debug)]
pub(super) enum Reach {
    Clicks(Vec<Click>, bool),
    NoItem,
    PlayerBox,
    TooFar(f64),
    Gravity,
    NoClick,
    Lava,
}

const LAVA_RUN: i32 = 3;

const LAVA_DROP: i32 = 4;

fn lava_safe(cell: BlockPos, feet: [f64; 3]) -> bool {
    let lo = [feet[0] - 0.3, feet[1], feet[2] - 0.3];
    let hi = [feet[0] + 0.3, feet[1] + 1.8, feet[2] + 0.3];
    let zone_lo = [
        f64::from(cell.x - LAVA_RUN),
        f64::from(cell.y - LAVA_DROP),
        f64::from(cell.z - LAVA_RUN),
    ];
    let zone_hi = [
        f64::from(cell.x + 1 + LAVA_RUN),
        f64::from(cell.y + 1),
        f64::from(cell.z + 1 + LAVA_RUN),
    ];
    !(0..3).all(|i| lo[i] < zone_hi[i] && hi[i] > zone_lo[i])
}

pub(super) fn clicks_at(w: &World, t: &Target, eye: [f64; 3], visible: bool) -> Reach {
    let Some(item) = t.item else {
        return Reach::NoItem;
    };
    if in_player_box(t.cell, [eye[0], eye[1] - EYE, eye[2]]) {
        return Reach::PlayerBox;
    }
    if !in_reach(t.cell, eye) {
        return Reach::TooFar(eye_d2(t.cell, eye).sqrt());
    }
    if falls(t.want)
        && !w
            .get_block_state(BlockPos::new(t.cell.x, t.cell.y - 1, t.cell.z))
            .is_some_and(holds)
    {
        return Reach::Gravity;
    }
    let want = Want {
        cell: t.cell,
        state: Some(t.want),
    };
    match t.job {
        Job::Pour => {
            if item == ItemKind::LavaBucket && !lava_safe(t.cell, [eye[0], eye[1] - EYE, eye[2]]) {
                return Reach::Lava;
            }
            let clicks = place::bucket::pour_candidates(w, eye, CLICK_REACH, item, &want, true);
            return if clicks.is_empty() {
                Reach::NoClick
            } else {
                Reach::Clicks(clicks, false)
            };
        }
        Job::Drain => {
            let clicks = place::bucket::drain_candidates(w, eye, CLICK_REACH, t.cell);
            return if clicks.is_empty() {
                Reach::NoClick
            } else {
                Reach::Clicks(clicks, false)
            };
        }
        Job::Place | Job::Break | Job::Use => {}
    }
    let held = ItemStack::new(item, 1);
    let real: Vec<Click> = place::candidates(w, eye, CLICK_REACH, &held, &want, visible)
        .into_iter()
        .filter(|k| !k.sneak)
        .collect();
    let real = prefer_steady(w, item, real);
    if !real.is_empty() {
        return Reach::Clicks(real, false);
    }
    if t.air {
        let air = prefer_steady(
            w,
            item,
            place::air_candidates(w, eye, CLICK_REACH, &held, &want),
        );
        if !air.is_empty() {
            return Reach::Clicks(air, true);
        }
    }
    Reach::NoClick
}

fn order(
    run: &Run,
    live: &[(Cand, ItemKind)],
    here: &Here,
    fast: bool,
    wanted: usize,
) -> Vec<(BlockPos, BlockState, ItemKind, Click, bool)> {
    let mut left: Vec<(Cand, ItemKind, &[Click], bool)> = live
        .iter()
        .filter_map(|(c, item)| {
            Some((*c, *item, run.sight.clicks(c.pos)?, run.sight.is_air(c.pos)))
        })
        .collect();
    let mut look = here.look;
    let mut held = here.held;
    let mut out = Vec::new();
    while out.len() < wanted && !left.is_empty() {
        let choices: Vec<Choice> = left
            .iter()
            .enumerate()
            .map(|(i, (c, item, clicks, air))| {
                let hides = !fast
                    && left
                        .iter()
                        .enumerate()
                        .any(|(j, (_, _, other, other_air))| {
                            j != i && !other_air && covers(c.pos, here.eye, other)
                        });
                Choice {
                    y: c.pos.y,
                    air: *air,
                    covers: hides,
                    held: held == Some(*item),
                    angle: least_turn(clicks, look).map_or(f32::MAX, |(_, a)| a),
                }
            })
            .collect();
        let Some(i) = pick(&choices) else {
            break;
        };
        let (c, item, clicks, air) = left.swap_remove(i);
        let Some((click, _)) = least_turn(clicks, look) else {
            continue;
        };
        look = click.look;
        held = Some(item);
        out.push((c.pos, c.want, item, click.clone(), air));
    }
    out
}

fn layer_near(cands: &[Cand], cell: BlockPos) -> Vec<BlockPos> {
    cands
        .iter()
        .map(|c| c.pos)
        .filter(|p| {
            p.y == cell.y
                && (p.x - cell.x).abs() <= LAYER_NEAR
                && (p.z - cell.z).abs() <= LAYER_NEAR
        })
        .collect()
}

fn in_reach(p: BlockPos, eye: [f64; 3]) -> bool {
    eye_d2(p, eye) <= (REACH + HALF_DIAG) * (REACH + HALF_DIAG)
}

fn gravity_ok(w: &World, c: &Cand) -> bool {
    !falls(c.want)
        || w.get_block_state(BlockPos::new(c.pos.x, c.pos.y - 1, c.pos.z))
            .is_some_and(holds)
}

fn has_support(w: &World, cell: BlockPos) -> bool {
    [
        (1, 0, 0),
        (-1, 0, 0),
        (0, 1, 0),
        (0, -1, 0),
        (0, 0, 1),
        (0, 0, -1),
    ]
    .iter()
    .any(|&(x, y, z)| {
        w.get_block_state(BlockPos::new(cell.x + x, cell.y + y, cell.z + z))
            .is_some_and(holds)
    })
}

fn in_player_box(cell: BlockPos, pos: [f64; 3]) -> bool {
    let lo = [pos[0] - 0.3, pos[1], pos[2] - 0.3];
    let hi = [pos[0] + 0.3, pos[1] + 1.8, pos[2] + 0.3];
    let c = [f64::from(cell.x), f64::from(cell.y), f64::from(cell.z)];
    (0..3).all(|i| lo[i] < c[i] + 1.0 && hi[i] > c[i])
}

fn eye_d2(p: BlockPos, eye: [f64; 3]) -> f64 {
    let c = [
        f64::from(p.x) + 0.5,
        f64::from(p.y) + 0.5,
        f64::from(p.z) + 0.5,
    ];
    (0..3).map(|i| (c[i] - eye[i]).powi(2)).sum()
}

fn missing(cands: &[Cand], stock: &Stock) -> Vec<(ItemKind, u32)> {
    let mut need: HashMap<ItemKind, u32> = HashMap::new();
    for c in cands
        .iter()
        .filter(|c| matches!(c.job, Job::Place | Job::Pour | Job::Drain))
    {
        if let Some(item) = job_item(c.job, c.want).filter(|i| !stock.has(*i)) {
            *need.entry(item).or_default() += 1;
        }
    }
    let mut out: Vec<(ItemKind, u32)> = need.into_iter().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.to_str().cmp(b.0.to_str())));
    out
}

fn missing_note(missing: &[(ItemKind, u32)]) -> String {
    let names: Vec<String> = missing
        .iter()
        .take(MISSING_SHOWN)
        .map(|(item, n)| format!("{n} {}", item.to_str().trim_start_matches("minecraft:")))
        .collect();
    format!("Missing: {}", names.join(", "))
}

fn send_route(
    bot: &Client,
    run: &mut Run,
    s: &Store,
    stock: &Stock,
    goal: QuietGoal,
    cells: Vec<BlockPos>,
    here: &Here,
) {
    let rules: Arc<dyn PlaceRules> = Arc::new(Rules::new(
        run.plan.clone(),
        stock.clone(),
        pathfinder::throwaway(bot).is_some(),
    ));
    let opts = QuietOpts {
        parkour: false,
        mining: s.flag(setting::DIG),
        placing: Some(rules),
    };
    run.batch = cells;
    pathfinder::goto_quiet(bot, goal, opts);
    run.stage = Stage::Travel {
        idle_at: here.now + TRAVEL_IDLE,
        at: here.feet,
    };
}

fn throwaway_ready(bot: &Client, run: &mut Run, here: &Here) -> bool {
    if !here.creative.on || pathfinder::throwaway(bot).is_some() {
        return true;
    }
    let list = &pathfinder::THROWAWAY;
    let Some(item) = (0..list.count())
        .filter(|&i| list.enabled(i))
        .find_map(|i| <ItemKind as std::str::FromStr>::from_str(list.label(i)).ok())
    else {
        return true;
    };
    let slot = run.spares.pick(item, here.slot);
    let held = stock::hold(bot, item, slot);
    matches!(held, Held::Ready(_))
}

fn spot_step(
    bot: &Client,
    run: &mut Run,
    s: &Store,
    stock: &Stock,
    here: &Here,
    out: &mut Out,
) -> bool {
    let Some(mut search) = run.spot.take() else {
        run.stage = Stage::Choose;
        return true;
    };
    let cell = search.cell();
    let refilling = run.refill.source == Some(cell);
    if !refilling
        && !run.wrong.contains(cell)
        && !run.scrap.contains(&cell)
        && !run.supports.contains_key(&cell)
    {
        run.stage = Stage::Choose;
        return true;
    }
    if !here.creative.fly && !throwaway_ready(bot, run, here) {
        run.spot = Some(search);
        return false;
    }
    let throwaway = pathfinder::throwaway(bot).is_some();
    let Ok(world) = bot.world() else {
        run.spot = Some(search);
        return false;
    };
    let found = {
        let w = world.read();
        search.step(&w, &run.plan, stock, throwaway, here.feet)
    };
    if let Spot::Found(p) = &found {
        run.last_spot = Some((cell, *p));
    }
    if let (Spot::Found(p), Some((feet, sneak))) = (&found, search.flush) {
        run.nudge = Some(Nudge {
            cell,
            stand: *p,
            feet,
            sneak,
            started: None,
            there: false,
        });
        if here.feet == *p {
            run.expect = Some(cell);
            run.stage = Stage::Choose;
            return false;
        }
    }
    match found {
        Spot::Pending => {
            run.spot = Some(search);
            false
        }
        Spot::Found(p) if here.creative.fly && !search.walking() => {
            run.expect = Some(cell);
            take_off(bot, run, here, p, cell, Some(search.target), out);
            false
        }
        Spot::Found(p) if here.creative.flying && search.standing() => {
            run.expect = Some(cell);
            take_off(bot, run, here, p, cell, Some(search.target), out);
            run.fly.landing = true;
            false
        }
        Spot::Found(_) if here.creative.flying => {
            if run
                .fly
                .landed_at
                .is_none_or(|t| here.now - t >= FLY_START_TICKS)
            {
                out.toggle_fly = true;
                run.fly.landed_at = Some(here.now);
                run.fly.hold = None;
            }
            run.stage = Stage::Choose;
            false
        }
        Spot::Found(p) => {
            run.expect = Some(cell);
            send_route(bot, run, s, stock, QuietGoal::Block(p), vec![cell], here);
            false
        }
        Spot::None if refilling => {
            run.refill.bad.insert(cell);
            run.refill.source = None;
            run.stage = Stage::Choose;
            true
        }
        Spot::None => {
            if !add_support(bot, run, here, &search.target) {
                run.hopeless.insert(cell);
                if run.why.get(&cell) != Some(&Why::Lava) {
                    run.why
                        .insert(cell, spot_why(search.target.want, search.target.item));
                }
                let others = run
                    .wrong
                    .iter()
                    .any(|p| p != cell && !run.away(p, here.now) && !run.hopeless.contains(&p));
                if !others {
                    run.strike(cell, here.now);
                }
            }
            run.stage = Stage::Choose;
            true
        }
    }
}

fn refill(
    bot: &Client,
    run: &mut Run,
    s: &Store,
    stock: &Stock,
    here: &Here,
    cands: &[Cand],
    out: &mut Out,
) -> bool {
    use azalea_registry::builtin::BlockKind;

    let water =
        |c: &&Cand| c.job == Job::Pour && job_item(c.job, c.want) == Some(ItemKind::WaterBucket);
    let pours = cands.iter().filter(water).count() as u32;
    let next = refill_next(
        stock.count(ItemKind::WaterBucket),
        stock.count(ItemKind::Bucket),
        run.refill.filling,
    );
    if here.creative.on || !s.flag(setting::REFILL) || pours == 0 || next != RefillNext::Fill {
        if run.refill.filling {}
        run.refill.filling = false;
        return false;
    }
    let Ok(world) = bot.world() else {
        return false;
    };
    if run.refill.source.is_none() {
        if run
            .refill
            .searched_at
            .is_some_and(|t| here.now - t < SOURCE_SEARCH_TICKS)
        {
            run.idle = Idle::NoWater(pours);
            return false;
        }
        let near = cands.iter().find(water).map_or(here.feet, |c| c.pos);
        let found = {
            let w = world.read();
            find_source(&w, &run.plan, &run.refill.bad, here.feet, near)
        };
        match found {
            Some(p) => {
                run.refill.source = Some(p);
                run.refill.tries = 0;
                run.refill.searched_at = None;
            }
            None => {
                run.refill.searched_at = Some(here.now);
                run.idle = Idle::NoWater(pours);
                return false;
            }
        }
    }
    let Some(src) = run.refill.source else {
        return false;
    };
    if run.refill.announced.insert(src) {
        out.lines.push(crate::lang::trf!(
            "builder.water_source_found",
            src.x,
            src.y,
            src.z
        ));
    }
    if !run.refill.filling {
        run.refill.filling = true;
    }
    run.idle = Idle::Refilling(src);
    let w = world.read();
    let there = w
        .get_block_state(src)
        .is_some_and(|st| st.as_block_kind() == BlockKind::Water && source(st));
    if !there {
        let since = *run.refill.gone_since.get_or_insert(here.now);
        if here.now - since > SOURCE_GIVE_UP {
            run.refill.bad.insert(src);
            run.refill.source = None;
            run.refill.gone_since = None;
        }
        return true;
    }
    run.refill.gone_since = None;
    if here.now - run.refill.filled_at < REFILL_WAIT {
        return true;
    }
    let t = Target {
        cell: src,
        job: Job::Drain,
        want: BlockState::AIR,
        item: Some(ItemKind::Bucket),
        air: false,
    };
    if let Reach::Clicks(clicks, _) = clicks_at(&w, &t, here.eye, true)
        && let Some((click, _)) = least_turn(&clicks, here.look)
    {
        let click = click.clone();
        drop(w);
        run.stage = place_stage(
            s,
            src,
            BlockState::AIR,
            ItemKind::Bucket,
            &click,
            false,
            false,
            here,
        );
        run.click = Some(click);
        run.click_eye = Some(here.eye);
        return true;
    }
    drop(w);
    run.refill.tries += 1;
    if run.refill.tries > SOURCE_TRIES {
        run.refill.bad.insert(src);
        run.refill.source = None;
        return true;
    }
    let over = [
        f64::from(src.x) + 0.5,
        f64::from(src.y) + 1.0,
        f64::from(src.z) + 0.5,
    ];
    run.spot = Some(SpotSearch::new(t, spot::passes(false, true), over));
    run.stage = Stage::Spot;
    true
}

fn find_source(
    w: &World,
    plan: &Plan,
    bad: &HashSet<BlockPos>,
    feet: BlockPos,
    near: BlockPos,
) -> Option<BlockPos> {
    use azalea_registry::builtin::BlockKind;

    let at = |p: BlockPos| w.get_block_state(p);
    let mut best: Option<((bool, bool, i64), BlockPos)> = None;
    for centre in [feet, near] {
        for dy in -SOURCE_DEPTH..=SOURCE_DEPTH {
            for dz in -SOURCE_RADIUS..=SOURCE_RADIUS {
                for dx in -SOURCE_RADIUS..=SOURCE_RADIUS {
                    let p = BlockPos::new(centre.x + dx, centre.y + dy, centre.z + dz);
                    if at(p).is_none_or(|s| s.as_block_kind() != BlockKind::Water)
                        || bad.contains(&p)
                        || !refills(p, &at)
                    {
                        continue;
                    }
                    let own = plan.want(p).is_some();
                    let dry = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(x, z)| {
                        let n = BlockPos::new(p.x + x, p.y + 1, p.z + z);
                        let floor = BlockPos::new(n.x, n.y - 1, n.z);
                        let head = BlockPos::new(n.x, n.y + 1, n.z);
                        at(n).is_some_and(|s| is_block_state_passable(s) && !plan::pure_fluid(s))
                            && at(head).is_some_and(is_block_state_passable)
                            && at(floor).is_some_and(is_block_state_standable)
                    });
                    let key = (own, !dry, dist2(p, feet));
                    if best.is_none_or(|(k, _)| key < k) {
                        best = Some((key, p));
                    }
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

const STEP_OUT: i32 = 3;

fn step_out_cell(w: &World, plan: &Plan, feet: BlockPos) -> Option<BlockPos> {
    let pending = |c: BlockPos| {
        plan.want(c)
            .is_some_and(|want| !state_is_air(want) && w.get_block_state(c) != Some(want))
    };
    let mut best: Option<(i64, BlockPos)> = None;
    for dy in -1..=1 {
        for dz in -STEP_OUT..=STEP_OUT {
            for dx in -STEP_OUT..=STEP_OUT {
                let p = BlockPos::new(feet.x + dx, feet.y + dy, feet.z + dz);
                let head = BlockPos::new(p.x, p.y + 1, p.z);
                let floor = BlockPos::new(p.x, p.y - 1, p.z);
                let open = |c: BlockPos| w.get_block_state(c).is_some_and(is_block_state_passable);
                if p == feet || !open(p) || !open(head) || pending(p) || pending(head) {
                    continue;
                }
                if !w
                    .get_block_state(floor)
                    .is_some_and(is_block_state_standable)
                {
                    continue;
                }
                let d = dist2(p, feet);
                if best.is_none_or(|(b, _)| d < b) {
                    best = Some((d, p));
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

fn route_failed(run: &mut Run, here: &Here) {
    for pos in std::mem::take(&mut run.batch) {
        let m = run.marks(pos);
        let flies = here.creative.fly && m.no_fly_until <= here.now;
        if flies && m.no_walk_until <= here.now {
            m.no_walk_until = here.now + UNREACHABLE_TICKS;
            m.routed = true;
        } else {
            run.strike_why(pos, here.now, Why::NoRoute);
        }
    }
    run.expect = None;
}

fn travel_step(bot: &Client, run: &mut Run, here: &Here, idle_at: i64, at: BlockPos) -> bool {
    match pathfinder::travel_status() {
        Travel::Arrived => {
            for pos in std::mem::take(&mut run.batch) {
                run.marks(pos).routed = true;
            }
            run.stage = Stage::Choose;
            run.nudge.is_none_or(|n| n.there)
        }
        Travel::Failed => {
            route_failed(run, here);
            run.stage = Stage::Choose;
            false
        }
        Travel::Going | Travel::Idle => {
            match route_watch(bot, here, idle_at, at) {
                Some((idle_at, at)) => run.stage = Stage::Travel { idle_at, at },
                None => {
                    let nearest = run
                        .batch
                        .iter()
                        .copied()
                        .min_by_key(|p| dist2(*p, here.feet));
                    run.batch = nearest.into_iter().collect();
                    route_failed(run, here);
                    run.stage = Stage::Choose;
                }
            }
            false
        }
    }
}

fn take_off(
    bot: &Client,
    run: &mut Run,
    here: &Here,
    to: BlockPos,
    cell: BlockPos,
    goal: Option<Target>,
    out: &mut Out,
) {
    if run.fly.user_was_flying.is_none() {
        run.fly.user_was_flying = Some(here.creative.flying);
    }
    let Ok(world) = bot.world() else {
        return;
    };
    {
        let w = world.read();
        let passable = |p: BlockPos| w.get_block_state(p).is_some_and(is_block_state_passable);
        run.fly.go(here.pos, here.feet, to, &passable);
    }
    run.batch = vec![cell];
    run.fly.goal = goal;
    run.fly.tight = false;
    run.fly.hold = None;
    run.fly.landing = false;
    run.stage = Stage::Fly {
        watch: fly::Watch::new(f64::INFINITY, here.now, FLY_IDLE),
    };
    fly_start(run, here, out);
}

fn fly_start(run: &mut Run, here: &Here, out: &mut Out) -> bool {
    if here.creative.flying {
        if run.fly.toggled_at.take().is_some() {}
        return true;
    }
    match run.fly.toggled_at {
        Some(at) if here.now - at < FLY_START_TICKS => {}
        Some(_) => {
            run.fly.refused = true;
            run.fly.clear();
        }
        None => {
            out.toggle_fly = true;
            run.fly.toggled_at = Some(here.now);
        }
    }
    false
}

fn fly_step(bot: &Client, run: &mut Run, here: &Here, watch: fly::Watch, out: &mut Out) -> bool {
    if !here.creative.on || run.fly.refused {
        run.fly.clear();
        run.stage = Stage::Choose;
        return true;
    }
    if run.fly.landing && !here.creative.flying {
        run.fly.clear();
        run.batch.clear();
        run.stage = Stage::Choose;
        return true;
    }
    if !fly_start(run, here, out) {
        if run.fly.refused {
            run.stage = Stage::Choose;
            return true;
        }
        return false;
    }
    if goal_in_reach(bot, run, here) {
        if let Some(cell) = run.fly.goal.as_ref().map(|g| g.cell) {
            run.sight.forget(cell);
        }
        run.fly.clear();
        run.fly.hold = Some(here.pos);
        run.batch.clear();
        run.stage = Stage::Choose;
        return true;
    }
    if let Some(search) = run.fly.search.as_mut() {
        let Ok(world) = bot.world() else {
            return false;
        };
        let grid = {
            let w = world.read();
            let passable = |p: BlockPos| w.get_block_state(p).is_some_and(is_block_state_passable);
            let room = |p: BlockPos| passable(p) && passable(BlockPos::new(p.x, p.y + 1, p.z));
            let crowded = |p: BlockPos| {
                [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dz)| {
                    !passable(BlockPos::new(p.x + dx, p.y, p.z + dz))
                        || !passable(BlockPos::new(p.x + dx, p.y + 1, p.z + dz))
                })
            };
            search.step(fly::NODES_PER_TICK, &room, &crowded)
        };
        match grid {
            Grid::Pending => return false,
            Grid::Found(path) => {
                run.fly.path = path;
                run.fly.search = None;
            }
            Grid::None => {
                return fly_failed(run, here);
            }
        }
    }
    let Some(&next) = run.fly.path.first() else {
        run.stage = Stage::Choose;
        return true;
    };
    let last = run.fly.path.len() == 1;
    let near = match (last, run.fly.tight) {
        (false, _) => fly::PASS,
        (true, false) => fly::ARRIVE,
        (true, true) => fly::TIGHT,
    };
    let mut p = fly::point(next);
    if run.fly.landing && last {
        p[1] -= fly::LAND_SINK;
    }
    let left = fly::distance(here.pos, p);
    let caps = match bot.world() {
        Ok(world) => {
            let w = world.read();
            let passable = |c: BlockPos| w.get_block_state(c).is_some_and(is_block_state_passable);
            let caps = fly::caps_for(here.pos, left, &passable);
            if p[1] - here.pos[1] > fly::RISE_FIRST
                && !fly::box_clear(here.pos, fly::LINE_MARGIN, &passable)
            {
                fly::Caps { h: 0.0, ..caps }
            } else {
                caps
            }
        }
        Err(_) => fly::NEAR_CAPS,
    };
    let prev = FLY_KEYS.load(Relaxed);
    let (mut keys, reached) = fly::steer(
        here.pos,
        here.vel,
        p,
        here.look.yaw,
        here.creative.speed,
        near,
        caps,
        prev,
    );
    let lip = bot.world().ok().and_then(|world| {
        use azalea::physics::collision::BlockWithShape;
        let w = world.read();
        fly::lip(here.pos, p, |c| {
            let s = w.get_block_state(c)?;
            let shape = s.collision_shape(c);
            (!shape.is_empty()).then(|| f64::from(c.y) + shape.bounds().max.y.min(1.5))
        })
    });
    if let Some(top) = lip
        && !reached
    {
        let over = [p[0], p[1].max(top + fly::BAND_LOW + 0.02), p[2]];
        keys = fly::track(
            here.pos,
            here.vel,
            over,
            here.look.yaw,
            here.creative.speed,
            fly::Caps { h: 0.0, ..caps },
            prev,
        );
    }
    if here.collided {
        run.fly_collided += 1;
        keys &= !sent::WALK_KEYS;
        if run.fly_collided >= FLY_COLLIDE_TICKS {
            run.fly_collided = 0;
            return fly_failed(run, here);
        }
    } else {
        run.fly_collided = 0;
    }
    if reached && last && !run.fly.tight && run.fly.goal.is_some() {
        run.fly.tight = true;
        run.stage = Stage::Fly {
            watch: fly::Watch::new(f64::INFINITY, here.now, FLY_IDLE),
        };
        out.keys = fly::brake(here.vel, here.look.yaw);
        return false;
    }
    if reached {
        run.fly.path.remove(0);
        if run.fly.path.is_empty() {
            run.fly.clear();
            run.fly.hold = Some(p);
            run.batch.clear();
            run.stage = Stage::Choose;
            return true;
        }
        run.stage = Stage::Fly {
            watch: fly::Watch::new(f64::INFINITY, here.now, FLY_IDLE),
        };
        return false;
    }
    out.keys = keys;
    out.look = Some(CameraAim {
        aim: [p[0], p[1] + EYE, p[2]],
        turn: fly::TURN,
    });
    let watch = match run.fly_watch {
        Some((q, kept)) if watch.best.is_infinite() && fly::distance(q, p) < 0.01 => kept,
        _ => watch,
    };
    match watch.tick(fly::distance(here.pos, p), here.now, FLY_IDLE) {
        Some(watch) => {
            run.fly_watch = Some((p, watch));
            run.stage = Stage::Fly { watch };
            false
        }
        None => fly_failed(run, here),
    }
}

fn goal_in_reach(bot: &Client, run: &Run, here: &Here) -> bool {
    let Some(goal) = run.fly.goal.as_ref() else {
        return false;
    };
    if in_player_box(goal.cell, here.pos) || !in_reach(goal.cell, here.eye) {
        return false;
    }
    let Ok(world) = bot.world() else {
        return false;
    };
    let w = world.read();
    spot::reaches_from(&w, goal, here.eye)
}

fn fly_failed(run: &mut Run, here: &Here) -> bool {
    for pos in std::mem::take(&mut run.batch) {
        run.marks(pos).no_fly_until = here.now + UNREACHABLE_TICKS;
        if let Some((cell, spot)) = run.last_spot
            && cell == pos
        {
            run.bad_spots.insert((cell, spot));
        }
    }
    run.expect = None;
    run.fly.clear();
    run.stage = Stage::Choose;
    true
}

fn fly_toward_unseen(bot: &Client, run: &mut Run, here: &Here, u: BlockPos) {
    let (dx, dz) = (
        f64::from(u.x) + 0.5 - here.pos[0],
        f64::from(u.z) + 0.5 - here.pos[2],
    );
    let d = (dx * dx + dz * dz).sqrt().max(1e-6);
    let hop = UNSEEN_HOP.min(d);
    let to = BlockPos::new(
        (here.pos[0] + dx / d * hop).floor() as i32,
        here.feet.y,
        (here.pos[2] + dz / d * hop).floor() as i32,
    );
    let Ok(world) = bot.world() else {
        return;
    };
    let free = {
        let w = world.read();
        let passable = |p: BlockPos| w.get_block_state(p).is_some_and(is_block_state_passable);
        fly::line_free(here.pos, fly::point(to), fly::LINE_MARGIN, &passable)
    };
    if free {
        let mut out = Out::default();
        take_off(bot, run, here, to, u, None, &mut out);
        run.fly.toggled_at = None;
    } else {
        run.idle = Idle::Checking;
    }
}

fn route_watch(bot: &Client, here: &Here, idle_at: i64, at: BlockPos) -> Option<(i64, BlockPos)> {
    if here.feet != at || pathfinder::mining() || pathfinder::placing() {
        return Some((here.now + TRAVEL_IDLE, here.feet));
    }
    if here.now < idle_at {
        return Some((idle_at, at));
    }
    pathfinder::cancel_quiet(bot);
    None
}

fn break_stage(
    bot: &Client,
    s: &Store,
    pos: BlockPos,
    state: BlockState,
    aim: [f64; 3],
    here: &Here,
) -> Stage {
    let (lo, hi) = s.range(setting::TURN);
    let tool = if here.creative.on {
        creative_tool(bot, here.slot)
    } else {
        best_tool(bot, state)
    };
    Stage::Break {
        pos,
        aim,
        arm_at: here.now + delay_ticks(s),
        stall_at: here.now + STALL_TICKS,
        turn: roll(lo, hi),
        tool,
    }
}

fn creative_tool(bot: &Client, selected: u8) -> Option<u8> {
    let inv = bot.component::<Inventory>().ok()?;
    let menu = &inv.inventory_menu;
    let first = *menu.hotbar_slots_range().start();
    let breaks = |h: u8| {
        menu.slot(first + usize::from(h)).is_none_or(|s| {
            s.as_present()
                .and_then(|d| d.get_unless_removed::<azalea_inventory::components::Tool>())
                .is_none_or(|t| t.can_destroy_blocks_in_creative)
        })
    };
    if breaks(selected) {
        return None;
    }
    (0..9u8).find(|&h| breaks(h))
}

#[allow(clippy::too_many_arguments)]
fn click_aim(click: &Click, eye: [f64; 3]) -> [f64; 3] {
    let point = sight::point(click);
    if angle(sent::look_at(eye, point), click.look) <= 1.0 {
        return point;
    }
    let v = sent::view_vector(click.look);
    [
        eye[0] + v[0] * 2.0,
        eye[1] + v[1] * 2.0,
        eye[2] + v[2] * 2.0,
    ]
}

fn place_stage(
    s: &Store,
    cell: BlockPos,
    want: BlockState,
    item: ItemKind,
    click: &Click,
    air: bool,
    fragile: bool,
    here: &Here,
) -> Stage {
    let (lo, hi) = s.range(setting::TURN);
    let turn = roll(lo, hi);
    Stage::Place {
        cell,
        want,
        item,
        aim: click_aim(click, here.eye),
        look: click.look,
        turn,
        arm_at: here.now + delay_ticks(s),
        give_up_at: here.now + turn_ticks(angle(here.look, click.look), turn),
        misses: 0,
        air,
        fragile,
    }
}

fn sensitive(w: &World, item: ItemKind, click: &Click, turn: f32, shift: f64) -> bool {
    sensitive_in(
        |p| w.get_block_state(p),
        (w.chunks.min_y(), w.chunks.height()),
        item,
        click,
        turn,
        shift,
    )
}

fn sensitive_in(
    block_at: impl Fn(BlockPos) -> Option<BlockState> + Copy,
    (min_y, height): (i32, u32),
    item: ItemKind,
    click: &Click,
    turn: f32,
    shift: f64,
) -> bool {
    use azalea_core::position::Vec3;

    let held = ItemStack::new(item, 1);
    let at = |hit: &BlockHitResult, yaw: f32, pitch: f32| {
        crate::play::placement::predict_placed(
            &held, hit, yaw, pitch, false, min_y, height, block_at,
        )
    };
    let (yaw, pitch) = (click.look.yaw, click.look.pitch);
    let base = at(&click.hit, yaw, pitch);
    let looks = [
        (yaw + turn, pitch),
        (yaw - turn, pitch),
        (yaw, (pitch + turn).min(90.0)),
        (yaw, (pitch - turn).max(-90.0)),
    ];
    if looks.iter().any(|&(y, p)| at(&click.hit, y, p) != base) {
        return true;
    }
    use azalea_core::direction::Direction;
    let axes: [usize; 2] = match click.hit.direction {
        Direction::East | Direction::West => [1, 2],
        Direction::Up | Direction::Down => [0, 2],
        Direction::North | Direction::South => [0, 1],
    };
    let loc = [
        click.hit.location.x,
        click.hit.location.y,
        click.hit.location.z,
    ];
    axes.iter().any(|&i| {
        [shift, -shift].iter().any(|d| {
            let mut moved = loc;
            moved[i] += d;
            let hit = BlockHitResult {
                location: Vec3::new(moved[0], moved[1], moved[2]),
                ..click.hit.clone()
            };
            at(&hit, yaw, pitch) != base
        })
    })
}

fn steady(w: &World, item: ItemKind, click: &Click) -> bool {
    !sensitive(w, item, click, YAW_MARGIN, HIT_MARGIN)
}

fn prefer_steady(w: &World, item: ItemKind, clicks: Vec<Click>) -> Vec<Click> {
    let steady: Vec<Click> = clicks
        .iter()
        .filter(|k| steady(w, item, k))
        .cloned()
        .collect();
    if steady.is_empty() { clicks } else { steady }
}

fn delay_ticks(s: &Store) -> i64 {
    let (lo, hi) = s.range(setting::DELAY);
    (roll(lo, hi) / MS_PER_TICK).round() as i64
}

fn turn_ticks(degrees: f32, turn: f32) -> i64 {
    (degrees / turn.max(1.0) * 20.0).ceil() as i64 + TURN_MARGIN
}

fn break_step(
    bot: &Client,
    run: &mut Run,
    hit: Option<&BlockHitResult>,
    here: &Here,
    out: &mut Out,
) -> bool {
    let Stage::Break {
        pos: t,
        aim,
        arm_at,
        stall_at,
        turn,
        tool,
    } = run.stage
    else {
        return false;
    };
    if !present(bot, t) {
        run.worked(here.now);
        let settings = run.spot_settings;
        forget_hopeless(run, &[t], settings, false);
        run.broken += 1;
        run.stalls = 0;
        run.marks(t).breaks += 1;
        run.scrap.remove(&t);
        run.settling
            .push((t, Expect::Gone, here.now + SETTLE_TICKS));
        run.loops.clear();
        run.sight.reset();
        run.stage = Stage::Choose;
        return true;
    }
    let ready = select_tool(bot, tool);
    out.look = Some(CameraAim { aim, turn });
    out.attack = ready && hit.is_some_and(|h| h.block_pos == t) && here.now >= arm_at;
    let progressing =
        bot.is_mining() && bot.component::<MineBlockPos>().ok().and_then(|p| p.0) == Some(t);
    let blocked = hit.is_none() && matches!(bot.hit_result(), Ok(HitResult::Entity(_)));
    if progressing || blocked {
        run.stage = Stage::Break {
            pos: t,
            aim,
            arm_at,
            stall_at: here.now + STALL_TICKS,
            turn,
            tool,
        };
    } else if here.now >= stall_at {
        out.attack = false;
        out.look = None;
        run.skip_why(t, Why::NotBreaking);
        run.stalls += 1;
        if run.stalls >= MAX_STALLS {
            out.lines.push(
                "AutoBuild paused: blocks are not breaking; resume from the menu".to_string(),
            );
            run.hold("Blocks are not breaking");
        }
        run.stage = Stage::Choose;
    }
    false
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClickKind {
    Block,
    Pour,
    Fill,
}

fn click_kind(item: ItemKind) -> ClickKind {
    match item {
        ItemKind::WaterBucket | ItemKind::LavaBucket => ClickKind::Pour,
        ItemKind::Bucket => ClickKind::Fill,
        _ => ClickKind::Block,
    }
}

fn is_bucket(item: ItemKind) -> bool {
    click_kind(item) != ClickKind::Block
}

#[allow(unused_variables)]
fn place_step(
    bot: &Client,
    shared: &SharedMutex,
    run: &mut Run,
    here: &Here,
    mode: BreakMode,
    out: &mut Out,
) -> bool {
    let Stage::Place {
        cell,
        want,
        item,
        aim,
        look,
        turn,
        arm_at,
        give_up_at,
        misses,
        air,
        fragile,
    } = run.stage
    else {
        return false;
    };
    let fluids = run.plan.fluids();
    let kind = click_kind(item);
    let still = state_at(bot, cell).is_some_and(|w| match kind {
        ClickKind::Fill => source(w),
        ClickKind::Pour => {
            judge(w, want, mode, fluids) == Verdict::Wrong && job(w, want, fluids) == Job::Pour
        }
        ClickKind::Block => {
            judge(w, want, mode, fluids) == Verdict::Wrong && job(w, want, fluids) == Job::Place
        }
    });
    if !still {
        run.stage = Stage::Choose;
        return true;
    }
    match hold(bot, run, here, item) {
        Held::Missing => {
            run.stage = Stage::Choose;
            return true;
        }
        Held::Moving => {
            out.look = Some(CameraAim { aim, turn });
            return false;
        }
        Held::Ready(_) => {}
    }
    out.look = Some(CameraAim { aim, turn });
    if here.now < arm_at {
        return false;
    }
    let want_at = Want {
        cell,
        state: Some(want),
    };
    let (sent_eye, sent_look) = sent::sent_view(bot).unwrap_or((here.eye, here.look));
    let live_look = sent::look_at(sent_eye, aim);
    let _ = look;
    let from_eye = (0..3)
        .map(|i| (aim[i] - sent_eye[i]).powi(2))
        .sum::<f64>()
        .sqrt();
    if from_eye > REACH {
        return miss(bot, run, here, (cell, want, item), turn, misses, air);
    }
    let settled = !fragile || angle(sent_look, live_look) <= FIRE_ANGLE;
    let fired = match (settled, air, run.click.as_ref(), kind) {
        (false, _, _, _) => Fired::NotAimed,
        (true, _, _, ClickKind::Pour) => place::bucket::fire_pour(bot, &want_at, REACH),
        (true, _, _, ClickKind::Fill) => place::bucket::fire_drain(bot, cell, REACH),
        (true, true, Some(click), ClickKind::Block) => fire_air(bot, shared, &want_at, click),
        _ => place::fire(bot, shared, &want_at, REACH),
    };
    match fired {
        Fired::Placed => {
            let shot = run.click.as_ref().map(|k| Shot::of(k, here.look, here.pos));
            match kind {
                ClickKind::Block => run.placed_own(cell, want, here.eye, here.now, shot),
                ClickKind::Pour => {
                    run.placed_as(cell, Expect::Exact(want), here.eye, here.now, shot);
                }
                ClickKind::Fill if run.refill.source == Some(cell) => {
                    run.worked(here.now);
                    run.refill.tries = 0;
                    run.refill.gone_since = None;
                    run.refill.filled_at = here.now;
                    run.loops.clear();
                }
                ClickKind::Fill => {
                    run.placed_as(cell, Expect::Drained, here.eye, here.now, shot);
                }
            }
            run.stage = Stage::Choose;
            true
        }
        Fired::NotAimed => {
            run.not_aimed += 1;
            if here.now < give_up_at {
                return false;
            }
            miss(bot, run, here, (cell, want, item), turn, misses, air)
        }
        Fired::Declined => {
            run.declined += 1;
            miss(bot, run, here, (cell, want, item), turn, misses, air)
        }
    }
}

fn use_step(bot: &Client, shared: &SharedMutex, run: &mut Run, here: &Here, out: &mut Out) -> bool {
    let Stage::Use {
        cell,
        want,
        aim,
        turn,
        wait_until,
        give_up_at,
        clicks,
        budget,
    } = run.stage
    else {
        return false;
    };
    let Some(now_state) = state_at(bot, cell) else {
        return false;
    };
    if place::same_placement(now_state, want) {
        run.loops.clear();
        run.stage = Stage::Choose;
        return true;
    }
    if place::use_clicks(now_state, want).is_none() {
        run.stage = Stage::Choose;
        return true;
    }
    out.look = Some(CameraAim { aim, turn });
    if here.now < wait_until {
        return false;
    }
    if clicks >= budget {
        run.strike(cell, here.now);
        run.stage = Stage::Choose;
        return true;
    }
    let on_block = sent::sent_view(bot).is_some_and(|(eye, look)| {
        let lo = [f64::from(cell.x), f64::from(cell.y), f64::from(cell.z)];
        sent::ray_hits(
            eye,
            sent::view_vector(look),
            REACH,
            lo,
            [lo[0] + 1.0, lo[1] + 1.0, lo[2] + 1.0],
        )
    });
    if !on_block {
        if here.now >= give_up_at {
            run.sight.fail(cell);
            run.strike(cell, here.now);
            run.stage = Stage::Choose;
            return true;
        }
        return false;
    }
    empty_hand(bot);
    let hit = BlockHitResult {
        location: azalea_core::position::Vec3::new(aim[0], aim[1], aim[2]),
        miss: false,
        direction: face_of(cell, aim),
        block_pos: cell,
        inside: false,
        world_border: false,
    };
    place::use_block(bot, shared, &hit);
    run.worked(here.now);
    run.stage = Stage::Use {
        cell,
        want,
        aim,
        turn,
        wait_until: here.now + SETTLE_TICKS,
        give_up_at: here.now + SETTLE_TICKS + turn_ticks(180.0, turn),
        clicks: clicks + 1,
        budget,
    };
    false
}

fn face_of(cell: BlockPos, p: [f64; 3]) -> azalea_core::direction::Direction {
    use azalea_core::direction::Direction;
    let d = [
        p[0] - f64::from(cell.x) - 0.5,
        p[1] - f64::from(cell.y) - 0.5,
        p[2] - f64::from(cell.z) - 0.5,
    ];
    let axis = (0..3)
        .max_by(|&a, &b| d[a].abs().total_cmp(&d[b].abs()))
        .unwrap_or(1);
    match (axis, d[axis] >= 0.0) {
        (0, true) => Direction::East,
        (0, false) => Direction::West,
        (1, true) => Direction::Up,
        (1, false) => Direction::Down,
        (_, true) => Direction::South,
        (_, false) => Direction::North,
    }
}

fn empty_hand(bot: &Client) {
    let slot = bot.component::<Inventory>().ok().and_then(|inv| {
        let menu = &inv.inventory_menu;
        let first = *menu.hotbar_slots_range().start();
        (0..9u8).find(|&h| {
            menu.slot(first + usize::from(h))
                .is_none_or(|s| s.is_empty())
        })
    });
    if let Some(slot) = slot {
        crate::play::interaction::select_hotbar_slot(bot, slot);
    }
}

fn fire_air(bot: &Client, shared: &SharedMutex, want: &Want, click: &Click) -> Fired {
    let Some((eye, look)) = sent::sent_view(bot) else {
        return Fired::NotAimed;
    };
    let c = want.cell;
    let lo = [f64::from(c.x), f64::from(c.y), f64::from(c.z)];
    let hi = [lo[0] + 1.0, lo[1] + 1.0, lo[2] + 1.0];
    if !sent::ray_hits(eye, sent::view_vector(look), REACH, lo, hi)
        && angle(look, click.look) > FIRE_ANGLE
    {
        return Fired::NotAimed;
    }
    let fired = place::fire_at(bot, shared, want, &click.hit);
    fired
}

#[allow(clippy::too_many_arguments)]
fn miss(
    bot: &Client,
    run: &mut Run,
    here: &Here,
    (cell, want, item): (BlockPos, BlockState, ItemKind),
    turn: f32,
    misses: u8,
    air: bool,
) -> bool {
    let misses = misses + 1;
    let fragile = matches!(run.stage, Stage::Place { fragile: true, .. });
    let again = if misses < MISS_TRIES {
        solve_again(bot, here, cell, want, item, air)
    } else {
        None
    };
    let Some(click) = again else {
        run.sight.fail(cell);
        run.click = None;
        run.click_eye = None;
        run.strike_why(cell, here.now, click_why(want, Some(item)));
        run.stage = Stage::Choose;
        return true;
    };
    run.stage = Stage::Place {
        cell,
        want,
        item,
        aim: click_aim(&click, here.eye),
        look: click.look,
        turn,
        arm_at: here.now + 1,
        give_up_at: here.now + turn_ticks(angle(here.look, click.look), turn),
        misses,
        air,
        fragile,
    };
    run.click = Some(click);
    run.click_eye = Some(here.eye);
    false
}

fn solve_again(
    bot: &Client,
    here: &Here,
    cell: BlockPos,
    want: BlockState,
    item: ItemKind,
    air: bool,
) -> Option<Click> {
    let world = bot.world().ok()?;
    let w = world.read();
    let job = match click_kind(item) {
        ClickKind::Block => Job::Place,
        ClickKind::Pour => Job::Pour,
        ClickKind::Fill => Job::Drain,
    };
    let t = Target {
        cell,
        job,
        want,
        item: Some(item),
        air,
    };
    match clicks_at(&w, &t, here.eye, true) {
        Reach::Clicks(clicks, _) => least_turn(&clicks, here.look).map(|(k, _)| k.clone()),
        _ => None,
    }
}

fn fast_place(
    bot: &Client,
    shared: &SharedMutex,
    run: &mut Run,
    clicks: Vec<(BlockPos, BlockState, ItemKind, Click, bool)>,
    here: &Here,
) -> usize {
    let mut fired = 0;
    for (cell, want, item, click, air) in clicks {
        match hold(bot, run, here, item) {
            Held::Ready(_) => {}
            Held::Moving => break,
            Held::Missing => continue,
        }
        if place::fire_fast(bot, shared, &click) {
            let _ = air;
            fired += 1;
            let shot = Shot::of(&click, click.look, here.pos);
            run.placed_own(cell, want, here.eye, here.now, Some(shot));
        } else {
            run.declined += 1;
            run.sight.fail(cell);
        }
    }
    if fired > 0 {
        place::restore_look(bot);
    }
    fired
}

fn done_line(run: &Run, english: bool) -> String {
    use crate::lang::Key;
    let fluids = run.scan.last.map_or(0, |p| p.fluids);
    let hanging = run.unsupported.len();
    let mut args = vec![
        run.placed.to_string(),
        run.broken.to_string(),
        run.wrong.skipped().to_string(),
    ];
    let key = match (hanging > 0, fluids > 0) {
        (false, false) => const { Key::of("builder.done") },
        (true, false) => {
            args.push(hanging.to_string());
            const { Key::of("builder.done.unsupported") }
        }
        (false, true) => {
            args.push(fluids.to_string());
            const { Key::of("builder.done.fluids") }
        }
        (true, true) => {
            args.extend([hanging.to_string(), fluids.to_string()]);
            const { Key::of("builder.done.unsupported_fluids") }
        }
    };
    let done = render(key, &args, english);
    let not_built = not_built(run, english);
    let done = if not_built.is_empty() {
        done
    } else {
        format!("{done}. {not_built}")
    };
    if run.left_holes.is_empty() {
        return done;
    }
    let at: Vec<String> = run
        .left_holes
        .iter()
        .take(NOT_BUILT_AT)
        .map(|p| format!("{} {} {}", p.x, p.y, p.z))
        .collect();
    let etc = if run.left_holes.len() > NOT_BUILT_AT {
        ", ..."
    } else {
        ""
    };
    format!(
        "{done}. Left {} holes at {}{etc}",
        run.left_holes.len(),
        at.join(", ")
    )
}

fn pause_run(bot: &Client, run: &mut Run) {
    if run.stage.walking() {
        pathfinder::cancel_quiet(bot);
    }
    run.spot = None;
    run.expect = None;
    run.batch.clear();
    run.fly.clear();
    run.click = None;
    run.stage = Stage::Choose;
}

fn hold(bot: &Client, run: &mut Run, here: &Here, item: ItemKind) -> Held {
    let slot = run.spares.pick(item, here.slot);
    let held = stock::hold(bot, item, slot);
    held
}

fn write_status(run: &Run, paused: Option<&str>, now: i64) {
    let refill_note = || {
        run.refill
            .source
            .filter(|_| run.refill.filling)
            .map(|p| format!("Refilling water at {} {} {}", p.x, p.y, p.z))
    };
    let (state, note) = match (paused, run.stage) {
        (Some(why), _) => (State::Paused, why.to_string()),
        (None, Stage::Break { .. }) => (State::Breaking, String::new()),
        (None, Stage::Place { .. }) => (State::Placing, refill_note().unwrap_or_default()),
        (None, Stage::Travel { .. }) => (State::Walking, refill_note().unwrap_or_default()),
        (None, Stage::Spot) => {
            let what = run.spot.as_ref().map_or(String::new(), |s| {
                let t = &s.target;
                let name = match t.job {
                    Job::Drain if run.refill.source == Some(t.cell) => {
                        "the water source".to_string()
                    }
                    _ => block_name(t.want),
                };
                format!(" for {name}")
            });
            (
                State::Walking,
                format!("Looking for a place to stand{what}"),
            )
        }
        (None, Stage::Fly { .. }) => (State::Walking, "Flying".to_string()),
        (None, Stage::Use { .. }) => (State::Placing, "Right clicking".to_string()),
        (None, Stage::Waiting { .. }) => match run.idle {
            Idle::NoWater(n) => (
                State::Waiting,
                format!("Need water buckets or an infinite water source ({n} water)"),
            ),
            _ => (State::Waiting, missing_note(&run.missing)),
        },
        (None, Stage::Choose) if run.scan.last.is_none() => (State::Scanning, String::new()),
        (None, Stage::Choose) if run.fast_at == now => (State::Placing, String::new()),
        (None, Stage::Choose) => idle_status(run.idle),
    };
    let target = match run.stage {
        Stage::Break { pos, .. } => Some(pos),
        Stage::Place { cell, .. } | Stage::Use { cell, .. } => Some(cell),
        _ => None,
    };
    *STATUS.lock().unwrap_or_else(PoisonError::into_inner) = Some(Status {
        state,
        note,
        placed: run.placed,
        broken: run.broken,
        left: run.wrong.len() as u32,
        skipped: run.wrong.skipped() as u32,
        total: run.plan.total(),
        missing: run.missing.clone(),
        target,
    });
}

fn idle_status(idle: Idle) -> (State, String) {
    match idle {
        Idle::Trying => (State::Placing, String::new()),
        Idle::Away(n) => (State::Waiting, format!("No way to reach {n} blocks")),
        Idle::Settling => (
            State::Waiting,
            "Waiting for placements to settle".to_string(),
        ),
        Idle::Checking => (State::Waiting, "Checking the build".to_string()),
        Idle::NoWay(n, why) => (
            State::Waiting,
            format!("No way to place {n} blocks (mostly: {})", why.label()),
        ),
        Idle::NoWater(n) => (
            State::Waiting,
            format!("Need water buckets or an infinite water source ({n} water)"),
        ),
        Idle::Refilling(p) => (
            State::Waiting,
            format!(
                "Refilling water at {} {} {}: waiting for the source",
                p.x, p.y, p.z
            ),
        ),
    }
}

fn block_name(state: BlockState) -> String {
    state
        .as_block_kind()
        .to_str()
        .trim_start_matches("minecraft:")
        .replace('_', " ")
}

fn mark_stopped() {
    if let Some(st) = STATUS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
        && st.state != State::Done
    {
        st.state = State::Paused;
        st.note = "Stopped".to_string();
        st.target = None;
    }
}

fn end_run(bot: &Client) -> Option<Run> {
    let run = RUN.lock().unwrap_or_else(PoisonError::into_inner).take();
    ACTIVE.store(false, Relaxed);
    FLY_KEYS.store(0, Relaxed);
    *ROUTE.lock().unwrap_or_else(PoisonError::into_inner) = None;
    if run.as_ref().is_some_and(|r| r.stage.walking()) {
        pathfinder::cancel_quiet(bot);
    }
    run
}

fn land(bot: &Client, shared: &SharedMutex, run: &Run) {
    if run.fly.user_was_flying != Some(false) {
        return;
    }
    let flying = bot.component::<PlayerAbilities>().is_ok_and(|a| a.flying);
    let (Ok(pos), Ok(world)) = (bot.position(), bot.world()) else {
        return;
    };
    let feet = player_pos_to_block_pos(pos);
    let ground = {
        let w = world.read();
        ground_below(feet, LAND_DEPTH, |p| w.get_block_state(p))
    };
    if flying && ground.is_some() {
        shared.lock().unwrap().session.fly_toggle = true;
    }
}

fn ground_below(
    feet: BlockPos,
    depth: i32,
    state_at: impl Fn(BlockPos) -> Option<BlockState>,
) -> Option<BlockPos> {
    for d in 1..=depth {
        let p = BlockPos::new(feet.x, feet.y - d, feet.z);
        let state = state_at(p)?;
        if is_block_state_standable(state) {
            return Some(p);
        }
        if !is_block_state_passable(state) {
            return None;
        }
    }
    None
}

fn finish(bot: &Client, shared: &SharedMutex, done: Option<&Ended>) {
    if let Some(run) = end_run(bot) {
        land(bot, shared, &run);
    }
    PAUSED.store(false, Relaxed);
    draw(None);
    publish(shared, None);
    match done {
        Some(ended) => {
            if let Some(st) = STATUS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_mut()
            {
                st.state = State::Done;
                st.note = ended.note.clone();
                st.target = None;
            }
            store().set_enabled(Id::AutoBuild, false);
            say(shared, &ended.chat);
        }
        None => mark_stopped(),
    }
}

fn publish(shared: &SharedMutex, look: Option<CameraAim>) {
    if look.is_none() && !LOOK_OUT.load(Relaxed) {
        return;
    }
    LOOK_OUT.store(look.is_some(), Relaxed);
    let mut st = shared.lock().unwrap();
    if let Some(a) = look
        && super::freecam::active()
    {
        super::turn::body_step(&mut st, a.aim, a.turn);
    }
    st.session.builder_look = look;
}

fn draw(target: Option<(BlockPos, u32)>) {
    let mut drawn = DRAWN.lock().unwrap_or_else(PoisonError::into_inner);
    if *drawn == target {
        return;
    }
    *drawn = target;
    match target {
        Some((t, color)) => {
            let min = [f64::from(t.x), f64::from(t.y), f64::from(t.z)];
            let max = [min[0] + 1.0, min[1] + 1.0, min[2] + 1.0];
            world_draw::set(OWNER, vec![Node::boxed(min, max, color, true)]);
        }
        None => world_draw::clear(OWNER),
    }
}

fn say(shared: &SharedMutex, text: &str) {
    let mut st = shared.lock().unwrap();
    super::chat_math::show_local(&mut st.session, text);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_player_box_covers_feet_and_head_cells() {
        let pos = [0.5, 64.0, 0.5];
        assert!(in_player_box(BlockPos::new(0, 64, 0), pos));
        assert!(in_player_box(BlockPos::new(0, 65, 0), pos));
        assert!(!in_player_box(BlockPos::new(0, 66, 0), pos));
        assert!(!in_player_box(BlockPos::new(0, 63, 0), pos));
        assert!(!in_player_box(BlockPos::new(1, 64, 0), pos));
        let edge = [1.0, 64.0, 0.5];
        assert!(
            in_player_box(BlockPos::new(0, 64, 0), edge)
                && in_player_box(BlockPos::new(1, 64, 0), edge)
        );
    }

    #[test]
    fn a_half_turn_at_180_degrees_a_second_is_a_second() {
        assert_eq!(turn_ticks(180.0, 180.0), 20 + TURN_MARGIN);
        assert_eq!(turn_ticks(180.0, 45.0), 80 + TURN_MARGIN);
    }

    #[test]
    fn the_missing_note_names_the_most_needed_first() {
        let note = missing_note(&[
            (ItemKind::OakPlanks, 12),
            (ItemKind::Glass, 3),
            (ItemKind::Stone, 2),
            (ItemKind::Dirt, 1),
        ]);
        assert_eq!(note, "Missing: 12 oak_planks, 3 glass, 2 stone");
    }

    fn click_on(
        support: BlockPos,
        face: azalea_core::direction::Direction,
        at: [f64; 3],
        yaw: f32,
        pitch: f32,
    ) -> Click {
        Click {
            hit: BlockHitResult {
                location: azalea_core::position::Vec3::new(at[0], at[1], at[2]),
                miss: false,
                direction: face,
                block_pos: support,
                inside: false,
                world_border: false,
            },
            look: Look { yaw, pitch },
            sneak: false,
        }
    }

    #[test]
    fn a_stair_click_near_a_facing_boundary_is_not_steady() {
        use azalea_core::direction::Direction;
        use azalea_registry::builtin::BlockKind;
        let stone = BlockPos::new(0, 63, 0);
        let world = move |p: BlockPos| {
            Some(if p == stone {
                BlockState::from(BlockKind::Stone)
            } else {
                BlockState::AIR
            })
        };
        let heights = (-64, 384);
        let top = [0.5, 64.0, 0.5];
        let straight = click_on(stone, Direction::Up, top, 0.0, 60.0);
        let edge = click_on(stone, Direction::Up, top, 44.0, 60.0);
        assert!(!sensitive_in(
            world,
            heights,
            ItemKind::OakStairs,
            &straight,
            YAW_MARGIN,
            HIT_MARGIN
        ));
        assert!(sensitive_in(
            world,
            heights,
            ItemKind::OakStairs,
            &edge,
            YAW_MARGIN,
            HIT_MARGIN
        ));
        assert!(sensitive_in(
            world,
            heights,
            ItemKind::OakStairs,
            &straight,
            90.0,
            0.35
        ));
        assert!(!sensitive_in(
            world,
            heights,
            ItemKind::Stone,
            &straight,
            90.0,
            0.35
        ));
    }

    #[test]
    fn a_slab_click_on_the_half_line_is_not_steady() {
        use azalea_core::direction::Direction;
        use azalea_registry::builtin::BlockKind;
        let stone = BlockPos::new(1, 64, 0);
        let world = move |p: BlockPos| {
            Some(if p == stone {
                BlockState::from(BlockKind::Stone)
            } else {
                BlockState::AIR
            })
        };
        let heights = (-64, 384);
        let middle = click_on(stone, Direction::West, [1.0, 64.5, 0.5], 90.0, 0.0);
        let low = click_on(stone, Direction::West, [1.0, 64.15, 0.5], 90.0, 0.0);
        assert!(sensitive_in(
            world,
            heights,
            ItemKind::OakSlab,
            &middle,
            YAW_MARGIN,
            HIT_MARGIN
        ));
        assert!(!sensitive_in(
            world,
            heights,
            ItemKind::OakSlab,
            &low,
            YAW_MARGIN,
            HIT_MARGIN
        ));
    }

    #[test]
    fn failures_in_a_row_trip_and_a_success_resets_them() {
        let cell = BlockPos::new(1, 2, 3);
        let mut fails = Fails::default();
        for _ in 0..FAILS_IN_ROW - 1 {
            assert_eq!(fails.note(false, cell), None);
        }
        assert_eq!(fails.note(true, cell), None);
        assert_eq!(fails.in_row, 0);
        for _ in 0..FAILS_IN_ROW - 1 {
            assert_eq!(fails.note(false, cell), None);
        }
        assert_eq!(fails.note(false, cell), Some(Trip::InRow(FAILS_IN_ROW)));
        assert_eq!(fails.last, Some(cell));
    }

    #[test]
    fn a_mostly_refusing_server_trips_the_window() {
        let cell = BlockPos::new(0, 0, 0);
        let mut fails = Fails::default();
        let mut tripped = None;
        for i in 0..FAIL_WINDOW * 2 {
            if let Some(t) = fails.note(i % 4 == 0, cell) {
                tripped = Some((i, t));
                break;
            }
        }
        let (i, trip) = tripped.expect("the window trips");
        assert!(i < FAIL_WINDOW + 4, "{i}");
        assert!(matches!(trip, Trip::Ratio { ok } if ok == FAIL_WINDOW - FAIL_WINDOW_LIMIT));
        assert!(fails.in_row < FAILS_IN_ROW);
    }

    #[test]
    fn the_failure_line_names_the_cause_and_air_place() {
        let at = BlockPos::new(10, 64, -3);
        assert_eq!(
            trip_note(Trip::InRow(8), at, false),
            "the server did not accept 8 placements in a row; last at 10 64 -3"
        );
        assert!(
            trip_note(Trip::Ratio { ok: 5 }, at, true)
                .ends_with("(Air place is on: air placement is probably blocked here)")
        );
    }

    #[test]
    fn spare_slots_rotate_per_material_and_skip_the_selected_one() {
        let mut spares = Spares::default();
        let a = spares.pick(ItemKind::Stone, 0);
        let b = spares.pick(ItemKind::Dirt, 0);
        assert_ne!(a, b);
        assert_eq!(spares.pick(ItemKind::Stone, 0), a);
        let c = spares.pick(ItemKind::Glass, SPARE_SLOTS[2]);
        assert_ne!(c, SPARE_SLOTS[2]);
    }

    #[test]
    fn ground_is_found_through_air_only() {
        use azalea_registry::builtin::BlockKind;
        let feet = BlockPos::new(0, 70, 0);
        let column = |floor: i32, roof: Option<i32>| {
            move |p: BlockPos| {
                Some(if p.y == floor || Some(p.y) == roof {
                    BlockState::from(BlockKind::Stone)
                } else {
                    BlockState::AIR
                })
            }
        };
        assert_eq!(
            ground_below(feet, LAND_DEPTH, column(60, None)),
            Some(BlockPos::new(0, 60, 0))
        );
        assert_eq!(ground_below(feet, LAND_DEPTH, column(20, None)), None);
        assert_eq!(ground_below(feet, 5, |_| None), None);
    }

    #[test]
    fn water_is_poured_out_then_every_bucket_refilled() {
        use RefillNext::{Fill, Missing, Pour};
        assert_eq!(refill_next(3, 0, false), Pour);
        assert_eq!(refill_next(1, 2, false), Pour);
        assert_eq!(refill_next(0, 3, false), Fill);
        assert_eq!(refill_next(1, 2, true), Fill);
        assert_eq!(refill_next(2, 1, true), Fill);
        assert_eq!(refill_next(3, 0, true), Pour);
        assert_eq!(refill_next(0, 0, false), Missing);
        assert_eq!(refill_next(0, 0, true), Missing);
    }

    #[test]
    fn lava_is_poured_from_above_or_far_off() {
        let cell = BlockPos::new(0, 64, 0);
        assert!(lava_safe(cell, [1.5, 65.0, 0.5]));
        assert!(!lava_safe(cell, [1.5, 64.0, 0.5]));
        assert!(!lava_safe(cell, [0.5, 61.0, 0.5]));
        assert!(lava_safe(cell, [4.5, 64.0, 0.5]));
        assert!(!lava_safe(cell, [3.5, 64.0, 0.5]));
    }

    #[test]
    fn the_not_built_line_groups_by_block_and_reason() {
        assert_eq!(not_built_line(&[], true), "");
        let at = |x: i32| BlockPos::new(x, 101, 1036);
        let cells = vec![
            ("observer".to_string(), Why::LookUp, at(1040)),
            ("dispenser".to_string(), Why::LookUp, at(1034)),
            ("stone".to_string(), Why::NoSpot, at(1)),
            ("stone".to_string(), Why::NoSpot, at(3)),
            ("stone".to_string(), Why::NoSpot, at(2)),
            ("stone".to_string(), Why::NoSpot, at(4)),
        ];
        assert_eq!(
            not_built_line(&cells, true),
            "Not built: 4 stone (no place to stand) at 1 101 1036, 2 101 1036, 3 101 1036, ...; \
             1 dispenser (needs a look from below) at 1034 101 1036; \
             1 observer (needs a look from below) at 1040 101 1036"
        );
        assert_eq!(
            top_why([Why::LookUp, Why::NoSpot, Why::LookUp].into_iter()),
            Why::LookUp
        );
    }

    #[test]
    fn flush_eyes_keep_the_box_in_the_cell() {
        let p = BlockPos::new(10, 64, -3);
        let eyes = spot::flush_eyes(p, 64.0 + EYE);
        assert_eq!(eyes.len(), 16);
        for (eye, feet, sneak) in &eyes {
            assert!(feet[0] - 0.3 > 10.0 && feet[0] + 0.3 < 11.0);
            assert!(feet[1] - 0.3 > -3.0 && feet[1] + 0.3 < -2.0);
            let height = if *sneak { spot::SNEAK_EYE } else { EYE };
            assert!((eye[1] - (64.0 + height)).abs() < 1e-9);
        }
    }

    #[test]
    fn a_stand_hole_gives_a_look_from_below() {
        let up = |eye: [f64; 3], p: [f64; 3]| -sent::look_at(eye, p).pitch;
        let standing = [1.31, EYE - 1.0, 0.5];
        let sneaking = [1.31, spot::SNEAK_EYE - 1.0, 0.5];
        let under = [0.97, 1.0, 0.5];
        assert!(up(sneaking, under) > 50.0);
        assert!(up(standing, under) < 50.0);
        let corner = [1.31, spot::SNEAK_EYE - 1.0, 0.31];
        let side = [0.97, 0.97, 0.0];
        assert!(up(corner, side) > 50.0);
        assert!(up([1.5, spot::SNEAK_EYE - 1.0, 0.5], [0.5, 1.0, 0.5]) < 50.0);
    }

    #[test]
    fn only_free_standing_blocks_get_supports() {
        use azalea_registry::builtin::BlockKind;
        let s = |k: BlockKind| BlockState::from(k);
        assert!(hangs(s(BlockKind::StoneButton)));
        assert!(hangs(s(BlockKind::Lever)));
        assert!(hangs(s(BlockKind::RedstoneWallTorch)));
        assert!(hangs(s(BlockKind::OakSign)));
        assert!(hangs(s(BlockKind::StonePressurePlate)));
        assert!(!hangs(s(BlockKind::OakTrapdoor)));
        assert!(!hangs(s(BlockKind::Hopper)));
        assert!(!hangs(s(BlockKind::Chest)));
        assert!(!hangs(s(BlockKind::OakLog)));
    }
}
