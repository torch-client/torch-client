mod hops;
mod placing;

pub use placing::{PLACE_COST, THROWAWAY, place_tick, placing, throwaway};

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering::Relaxed};
use std::sync::{Arc, Mutex, PoisonError};

use azalea::Client;
use azalea::block::BlockTrait;
use azalea::ecs::entity::Entity;
use azalea::ecs::query::With;
use azalea::entity::metadata::{Player, Sprinting};
use azalea::entity::{Physics, Position};
use azalea::mining::MineBlockPos;
use azalea::pathfinder::goals::{BlockPosGoal, Goal, RadiusGoal};
use azalea::pathfinder::moves::{ExecutorIntent, MovesCtx, basic, place, uncommon};
use azalea::pathfinder::positions::RelBlockPos;
use azalea::pathfinder::{
    ExecutingPath, MoveSafety, PathfinderClientExt, PathfinderOpts, PlaceRules,
    player_pos_to_block_pos,
};
use azalea::player::GameProfileComponent;
use azalea::{BlockPos, Vec3};
use azalea_core::hit_result::BlockHitResult;
use uuid::Uuid;

use super::auto_mine::shared::{
    STALL_TICKS, aim_at, best_tool, gate, present, sees, select_tool, state_at,
};
use super::registry::pathfinder as setting;
use super::sent::{Look, WALK_KEYS, keys_toward, look_at, view_vector};
use super::{store, turn};
use crate::modules::chat_math::{GREEN, RED, say};
use crate::renderer::world_draw::{self, Node};
use crate::session::{
    CameraAim, MOVE_BACK, MOVE_DESCEND, MOVE_FORWARD, MOVE_JUMP, MOVE_LEFT, MOVE_RIGHT, MOVE_SNEAK,
    MOVE_SPRINT, SharedMutex,
};
use crate::util::mth::wrap_degrees;

const OWNER: &str = "pathfinder";

const TAKEOVER: u8 =
    MOVE_FORWARD | MOVE_BACK | MOVE_LEFT | MOVE_RIGHT | MOVE_JUMP | MOVE_SNEAK | MOVE_DESCEND;

fn turn_rate() -> f32 {
    store().num(setting::TURN_SPEED).max(1.0)
}

fn turn_ticks() -> u32 {
    (180.0 / turn_rate() * 20.0).ceil() as u32
}

const WALK_ERR: f32 = 45.0;

const NEAR_POINT: f64 = 0.3;

const COAST_WITHIN: f64 = 0.2;

const LOW_AIR: i32 = 100;

const LOOKAHEAD_NODES: usize = 3;

fn mine_extra_ticks() -> u32 {
    turn_ticks() + 10
}

const REVERT_LIMIT: u32 = 3;

const AIM_DISTANCE: f64 = 16.0;

const START_GRACE: u32 = 5;

const ARRIVED_WITHIN: f64 = 2.5;

fn follow_radius() -> f32 {
    store().num(setting::FOLLOW_DISTANCE)
}
const REPATH_TICKS: u32 = 10;
const REPATH_DIST: f64 = 1.5;
const IDLE_RETRY_TICKS: u32 = 40;
const PROGRESS_STEP: f64 = 0.5;
const MOVED_STEP: f64 = 2.0;
const AWAY_DIST: f64 = 24.0;
const AWAY_TICKS: u32 = 60;
const STUCK_TICKS: u32 = 100;
const GIVE_UP_TICKS: u32 = 300;
const GOTO_RESTART_LIMIT: u32 = 5;
const SKIP_AHEAD: usize = 60;

#[cfg(target_arch = "wasm32")]
const WEB_MIN_SEARCH: std::time::Duration = std::time::Duration::from_millis(100);
#[cfg(target_arch = "wasm32")]
const WEB_MAX_SEARCH: std::time::Duration = std::time::Duration::from_millis(400);

const ROUTE_WIDTH: f64 = 0.06;
const ROUTE_COLOR: u32 = 0xFF55_FFFF;
const TARGET_COLOR: u32 = 0x4055_FF55;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coord {
    Abs(i32),
    Rel(i32),
}

impl Coord {
    pub fn parse(text: &str) -> Option<Coord> {
        match text.strip_prefix('~') {
            Some("") => Some(Coord::Rel(0)),
            Some(n) => n.parse().ok().map(Coord::Rel),
            None => text.parse().ok().map(Coord::Abs),
        }
    }

    fn resolve(self, feet: i32) -> i32 {
        match self {
            Coord::Abs(n) => n,
            Coord::Rel(n) => feet.saturating_add(n),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Request {
    Goto([Coord; 3]),
    Follow(Option<String>),
    Stop,
}

static PENDING: Mutex<Option<Request>> = Mutex::new(None);

static BUSY: AtomicBool = AtomicBool::new(false);

static GOING: AtomicBool = AtomicBool::new(false);

static IDLE: AtomicBool = AtomicBool::new(false);

static JUST_STOPPED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Travel {
    Idle,
    Going,
    Arrived,
    Failed,
}

impl Travel {
    const ALL: [Travel; 4] = [Travel::Idle, Travel::Going, Travel::Arrived, Travel::Failed];
}

static TRAVEL: AtomicU8 = AtomicU8::new(0);

fn set_travel(t: Travel) {
    TRAVEL.store(t as u8, Relaxed);
}

#[derive(Clone, Debug, PartialEq)]
pub enum QuietGoal {
    Block(BlockPos),
    Near(Vec<BlockPos>),
    Within { blocks: Vec<BlockPos>, range: f64 },
}

#[derive(Clone, Debug)]
pub struct QuietOpts {
    pub parkour: bool,
    pub mining: bool,
    pub placing: Option<Arc<dyn PlaceRules>>,
}

impl QuietGoal {
    fn reached(&self, feet: BlockPos) -> bool {
        match self {
            QuietGoal::Block(b) => feet == *b,
            QuietGoal::Near(ores) => near_success(ores, feet),
            QuietGoal::Within { blocks, range } => within_success(blocks, *range, feet),
        }
    }

    fn box_at(&self, feet: Vec3) -> BlockPos {
        match self {
            QuietGoal::Block(b) => *b,
            QuietGoal::Near(list) | QuietGoal::Within { blocks: list, .. } => {
                let d2 = |b: &BlockPos| {
                    let c = [
                        b.x as f64 + 0.5 - feet.x,
                        b.y as f64 + 0.5 - feet.y,
                        b.z as f64 + 0.5 - feet.z,
                    ];
                    c[0] * c[0] + c[1] * c[1] + c[2] * c[2]
                };
                list.iter()
                    .copied()
                    .min_by(|a, b| d2(a).total_cmp(&d2(b)))
                    .unwrap_or(BlockPos::from(feet))
            }
        }
    }

    fn empty(&self) -> bool {
        match self {
            QuietGoal::Block(_) => false,
            QuietGoal::Near(list) | QuietGoal::Within { blocks: list, .. } => list.is_empty(),
        }
    }
}

#[derive(Debug)]
struct NearGoal(Vec<BlockPos>);

pub(in crate::modules) fn near_success(ores: &[BlockPos], n: BlockPos) -> bool {
    ores.iter().any(|o| {
        let dy = o.y - n.y;
        (o.x - n.x).abs() + (o.z - n.z).abs() == 1 && (dy == 0 || dy == 1)
    })
}

impl Goal for NearGoal {
    fn heuristic(&self, n: BlockPos) -> f32 {
        let mut best = f32::INFINITY;
        for o in &self.0 {
            let side = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .into_iter()
                .map(|(dx, dz)| BlockPosGoal(BlockPos::new(o.x + dx, n.y, o.z + dz)).heuristic(n))
                .fold(f32::INFINITY, f32::min);
            let height = [0, 1]
                .into_iter()
                .map(|dy| BlockPosGoal(BlockPos::new(n.x, o.y - dy, n.z)).heuristic(n))
                .fold(f32::INFINITY, f32::min);
            best = best.min(side + height);
        }
        best
    }

    fn success(&self, n: BlockPos) -> bool {
        near_success(&self.0, n)
    }
}

const STANDING_EYE: f64 = 1.62;

fn within_success(blocks: &[BlockPos], range: f64, n: BlockPos) -> bool {
    let eye = [
        n.x as f64 + 0.5,
        n.y as f64 + STANDING_EYE,
        n.z as f64 + 0.5,
    ];
    let head = BlockPos::new(n.x, n.y + 1, n.z);
    let near = |b: &BlockPos| {
        let d = [
            b.x as f64 + 0.5 - eye[0],
            b.y as f64 + 0.5 - eye[1],
            b.z as f64 + 0.5 - eye[2],
        ];
        d[0] * d[0] + d[1] * d[1] + d[2] * d[2] <= range * range
    };
    !blocks.iter().any(|b| *b == n || *b == head) && blocks.iter().any(near)
}

const WITHIN_SCAN: f64 = 16.0;

#[derive(Debug)]
struct WithinGoal {
    blocks: Vec<BlockPos>,
    range: f64,
    slack: f32,
}

impl WithinGoal {
    fn new(blocks: Vec<BlockPos>, range: f64) -> WithinGoal {
        if range.is_nan() || range > WITHIN_SCAN {
            return WithinGoal {
                blocks,
                range,
                slack: f32::INFINITY,
            };
        }
        let origin = BlockPos::new(0, 0, 0);
        let r = range.max(0.0).ceil() as i32 + 2;
        let mut slack = 0.0f32;
        for dx in -r..=r {
            for dy in -r..=r {
                for dz in -r..=r {
                    let c = BlockPos::new(dx, dy, dz);
                    if within_success(&[origin], range, c) {
                        slack = slack.max(BlockPosGoal(origin).heuristic(c));
                    }
                }
            }
        }
        WithinGoal {
            blocks,
            range,
            slack,
        }
    }
}

impl Goal for WithinGoal {
    fn heuristic(&self, n: BlockPos) -> f32 {
        let best = self
            .blocks
            .iter()
            .map(|b| BlockPosGoal(*b).heuristic(n))
            .fold(f32::INFINITY, f32::min);
        (best - self.slack).max(0.0)
    }

    fn success(&self, n: BlockPos) -> bool {
        within_success(&self.blocks, self.range, n)
    }
}

#[derive(Clone)]
struct Route {
    goal: QuietGoal,
    opts: QuietOpts,
}

impl Route {
    fn send(&self, bot: &Client) {
        let opts = path_opts(&self.opts);
        match &self.goal {
            QuietGoal::Block(b) => bot.start_goto_with_opts(BlockPosGoal(*b), opts),
            QuietGoal::Near(ores) => bot.start_goto_with_opts(NearGoal(ores.clone()), opts),
            QuietGoal::Within { blocks, range } => {
                bot.start_goto_with_opts(WithinGoal::new(blocks.clone(), *range), opts)
            }
        }
    }
}

enum Target {
    Block(Route),
    Player(Follow),
}

struct Follow {
    uuid: Uuid,
    name: String,
    radius: f32,
    goal_at: Option<Vec3>,
    since_repath: u32,
    best: f64,
    anchor: Vec3,
    stalled: u32,
    far_best: f64,
    far_ticks: u32,
    warned: Option<u32>,
}

#[derive(Debug, Default, PartialEq)]
struct Step {
    repath: bool,
    note: Option<Note>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Note {
    Away,
    Stuck,
    GiveUp,
}

impl Follow {
    fn new(uuid: Uuid, name: String, radius: f32, feet: Vec3, after_stop: bool) -> Follow {
        Follow {
            uuid,
            name,
            radius,
            goal_at: None,
            since_repath: if after_stop {
                REPATH_TICKS - 2
            } else {
                REPATH_TICKS - 1
            },
            best: f64::INFINITY,
            anchor: feet,
            stalled: 0,
            far_best: f64::INFINITY,
            far_ticks: 0,
            warned: None,
        }
    }

    fn step(&mut self, feet: Vec3, target: Vec3, executing: bool, calculating: bool) -> Step {
        let dist = feet.distance_to(target);
        let in_reach = dist <= self.radius as f64 + 1.0;

        self.since_repath = self.since_repath.saturating_add(1);
        let moved = self
            .goal_at
            .is_none_or(|g| g.distance_to(target) > REPATH_DIST);
        let retry = !executing && self.since_repath >= IDLE_RETRY_TICKS;
        let repath =
            !calculating && !in_reach && self.since_repath >= REPATH_TICKS && (moved || retry);
        if repath {
            self.goal_at = Some(target);
            self.since_repath = 0;
        }

        if in_reach
            || dist < self.best - PROGRESS_STEP
            || feet.distance_to(self.anchor) > MOVED_STEP
        {
            self.best = dist;
            self.anchor = feet;
            self.stalled = 0;
        } else {
            self.stalled = self.stalled.saturating_add(1);
        }
        if dist <= AWAY_DIST {
            self.far_best = f64::INFINITY;
            self.far_ticks = 0;
        } else if dist < self.far_best - PROGRESS_STEP {
            self.far_best = dist;
            self.far_ticks = 0;
        } else {
            self.far_ticks = self.far_ticks.saturating_add(1);
        }

        let trouble = if self.far_ticks >= AWAY_TICKS {
            Some(Note::Away)
        } else if self.stalled >= STUCK_TICKS {
            Some(Note::Stuck)
        } else {
            None
        };
        let note = match (trouble, self.warned) {
            (None, _) => {
                self.warned = None;
                None
            }
            (Some(n), None) => {
                self.warned = Some(0);
                Some(n)
            }
            (Some(_), Some(t)) => {
                let t = t + 1;
                self.warned = Some(t);
                (t >= GIVE_UP_TICKS).then_some(Note::GiveUp)
            }
        };
        Step { repath, note }
    }
}

#[derive(Default)]
struct Run {
    target: Option<Target>,
    age: u32,
    drawn: Option<DrawKey>,
    quiet: bool,
    deferred: bool,
    restarts: Restarts,
}

#[derive(Clone, Debug, PartialEq)]
struct Restarts {
    node: Option<BlockPos>,
    ticks: usize,
    ahead: Vec<BlockPos>,
    walked: bool,
    best: f64,
    count: u32,
}

impl Restarts {
    const NEW: Restarts = Restarts {
        node: None,
        ticks: 0,
        ahead: Vec::new(),
        walked: false,
        best: f64::INFINITY,
        count: 0,
    };

    fn step(
        &mut self,
        route: Option<(BlockPos, usize)>,
        targets: impl Iterator<Item = BlockPos>,
        dist: f64,
        busy: bool,
    ) -> bool {
        let restarted = match (route, self.node) {
            (Some(_), None) => self.walked,
            (Some((node, ticks)), Some(prev)) if node == prev => ticks < self.ticks && !busy,
            (Some((node, _)), Some(_)) => !self.ahead.contains(&node),
            (None, _) => false,
        };
        self.walked |= route.is_some();
        self.node = route.map(|(n, _)| n);
        self.ticks = route.map_or(0, |(_, t)| t);
        self.ahead.clear();
        if route.is_some() {
            self.ahead.extend(targets.take(SKIP_AHEAD));
        }
        if dist < self.best - PROGRESS_STEP {
            self.best = dist;
            self.count = 0;
        } else if restarted {
            self.count += 1;
        }
        self.count > GOTO_RESTART_LIMIT
    }
}

impl Default for Restarts {
    fn default() -> Restarts {
        Restarts::NEW
    }
}

#[derive(Clone, PartialEq, Eq)]
struct DrawKey {
    goal: BlockPos,
    from: BlockPos,
    nodes: Vec<BlockPos>,
}

static RUN: Mutex<Run> = Mutex::new(Run {
    target: None,
    age: 0,
    drawn: None,
    quiet: false,
    deferred: false,
    restarts: Restarts::NEW,
});

static PARKOUR: AtomicBool = AtomicBool::new(false);

static MINING: AtomicBool = AtomicBool::new(false);

static MINE_FAILED: AtomicBool = AtomicBool::new(false);

struct MineAt {
    at: Option<BlockPos>,
    aim: Option<[f64; 3]>,
    tool: Option<u8>,
    tool_ready: bool,
    track: Option<Track>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Track {
    block: BlockPos,
    idle: u32,
    repeats: u32,
    met: bool,
}

impl Track {
    fn new(block: BlockPos) -> Track {
        Track {
            block,
            idle: 0,
            repeats: 0,
            met: false,
        }
    }

    fn step(&mut self, progressing: bool, limit: u32) -> bool {
        self.idle = if progressing {
            0
        } else {
            self.idle.saturating_add(1)
        };
        self.idle > limit || self.repeats >= REVERT_LIMIT
    }

    fn ask(track: &mut Option<Track>, block: BlockPos) {
        match track {
            Some(t) if t.block == block => {
                if t.met {
                    t.repeats += 1;
                    t.met = false;
                }
            }
            _ => *track = Some(Track::new(block)),
        }
    }
}

static MINE_AT: Mutex<MineAt> = Mutex::new(MineAt {
    at: None,
    aim: None,
    tool: None,
    tool_ready: false,
    track: None,
});

fn mine_at() -> std::sync::MutexGuard<'static, MineAt> {
    MINE_AT.lock().unwrap_or_else(PoisonError::into_inner)
}

fn clear_mine(route_over: bool) {
    if !route_over && !MINING.load(Relaxed) {
        return;
    }
    MINING.store(false, Relaxed);
    let mut m = mine_at();
    m.at = None;
    m.aim = None;
    m.tool = None;
    m.tool_ready = false;
    if route_over {
        m.track = None;
        MINE_FAILED.store(false, Relaxed);
    }
}

pub fn mining() -> bool {
    MINING.load(Relaxed)
}

pub fn attack(shared: &SharedMutex, hit: Option<&BlockHitResult>) -> bool {
    if !MINING.load(Relaxed)
        || !GOING.load(Relaxed)
        || IDLE.load(Relaxed)
        || MINE_FAILED.load(Relaxed)
    {
        return false;
    }
    let ready = {
        let m = mine_at();
        m.tool_ready && m.at.is_some() && hit.is_some_and(|h| Some(h.block_pos) == m.at)
    };
    ready && gate(shared, false).is_some()
}

fn run() -> std::sync::MutexGuard<'static, Run> {
    RUN.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn request(request: Request) {
    *PENDING.lock().unwrap_or_else(PoisonError::into_inner) = Some(request);
    BUSY.store(true, Relaxed);
}

pub fn drives(bot: &Client, hand_keys: u8) -> bool {
    if !GOING.load(Relaxed) {
        return false;
    }
    if hand_keys & TAKEOVER != 0 {
        bot.force_stop_pathfinding();
        clear_mine(true);
        placing::clear(true);
        GOING.store(false, Relaxed);
        JUST_STOPPED.store(true, Relaxed);
        return false;
    }
    !IDLE.load(Relaxed)
}

pub fn steer(bot: &Client, shared: &SharedMutex, camera: Look, sent: Look) -> u8 {
    let intent = bot
        .component::<ExecutorIntent>()
        .ok()
        .map(|i| *i)
        .unwrap_or_default();
    let eye = bot.eye_position().ok().map(|e| [e.x, e.y, e.z]);
    let feet = bot.component::<Position>().ok().map(|p| **p);
    let (velocity, in_water, on_ground) = bot
        .component::<Physics>()
        .ok()
        .map_or((Vec3::ZERO, false, false), |p| {
            (p.velocity, p.is_in_water(), p.on_ground())
        });
    let front = PARKOUR.load(Relaxed).then(|| route_front(bot)).flatten();
    if front.is_some() {
        hops::keep_live();
    }
    if let (Some(r), Some(f)) = (&front, feet)
        && r.hop
        && let Some(why) = hops::hop_failed(
            f.y,
            r.start,
            r.target,
            r.ticks,
            player_pos_to_block_pos(f) != r.start,
        )
    {
        hops::note_failed(r.start, r.target, why);
    }

    let (point, exact, free_yaw) = match (intent.mine, intent.place) {
        (Some(block), _) => {
            placing::clear(false);
            (Some(dig_point(bot, block, eye)), true, true)
        }
        (None, Some(cell)) => {
            clear_mine(false);
            note_gone(bot);
            let (p, free) = placing::point(bot, cell, eye);
            (Some(p), true, free)
        }
        (None, None) => {
            clear_mine(false);
            placing::clear(false);
            note_gone(bot);
            let pre = match (&front, feet, eye) {
                (Some(r), Some(feet), Some(eye)) if on_ground => preaim(r, &intent, feet, eye[1]),
                _ => None,
            };
            let ahead = pre.or_else(|| {
                (!intent.jump && !intent.sneak && !(intent.sprint && PARKOUR.load(Relaxed)))
                    .then(|| eye.and_then(|eye| look_ahead(bot, eye[1])))
                    .flatten()
            });
            (ahead.or(intent.look_target), false, true)
        }
    };
    let want = match (point, eye) {
        (Some(t), Some(eye)) => {
            let w = look_at(eye, [t.x, t.y, t.z]);
            let (dx, dz) = (t.x - eye[0], t.z - eye[2]);
            let near = free_yaw && dx * dx + dz * dz < NEAR_POINT * NEAR_POINT;
            let w = if near {
                Look {
                    yaw: camera.yaw,
                    ..w
                }
            } else {
                w
            };
            let aim = if exact && !near {
                [t.x, t.y, t.z]
            } else {
                let v = view_vector(w);
                std::array::from_fn(|i| eye[i] + v[i] * AIM_DISTANCE)
            };
            Some((w, aim))
        }
        _ => None,
    };
    publish_look(shared, want.map(|(_, aim)| aim));

    let facing = match (intent.look_target, eye) {
        (Some(t), Some(eye)) => {
            let (dx, dz) = (t.x - eye[0], t.z - eye[2]);
            dx * dx + dz * dz < NEAR_POINT * NEAR_POINT
                || wrap_degrees(look_at(eye, [t.x, t.y, t.z]).yaw - camera.yaw).abs() <= WALK_ERR
        }
        _ => true,
    };
    let coast = match (intent.look_target, feet) {
        (Some(t), Some(feet)) if !intent.jump => {
            coasts_in(feet, t, velocity, friction_below(bot, feet))
        }
        _ => false,
    };

    let mut keys = 0;
    if intent.mine.is_none()
        && !coast
        && let Some((dx, dz)) = intent.world_dir
    {
        keys |= keys_toward(dx, dz, sent.yaw).unwrap_or(0);
    }
    if intent.sprint && facing && keys & WALK_KEYS != 0 {
        keys |= MOVE_SPRINT;
    }
    if intent.jump && (!intent.sprint || facing || in_water) {
        keys |= MOVE_JUMP;
    }
    if intent.sneak {
        keys |= MOVE_SNEAK;
    }
    let air = bot
        .component::<azalea::entity::metadata::AirSupply>()
        .map(|a| a.0)
        .unwrap_or(crate::session::MAX_AIR_SUPPLY);
    if in_water && air < LOW_AIR {
        keys = (keys | MOVE_JUMP) & !MOVE_SNEAK;
    }

    let hop_err = match (&front, eye) {
        (Some(r), Some(eye)) if r.hop => Some(wrap_degrees(
            look_at(eye, centre_at(r.target, eye[1])).yaw - sent.yaw,
        )),
        _ => None,
    };
    let sprinting = bot.component::<Sprinting>().is_ok_and(|s| **s);
    let take_off = match (&front, feet) {
        (Some(r), Some(f)) => on_ground && !in_water && player_pos_to_block_pos(f) == r.start,
        _ => false,
    };
    if let Some(err) = hop_err
        && take_off
        && hops::holds_launch(err, sprinting, intent.sprint)
    {
        keys &= MOVE_SNEAK;
    }
    keys
}

fn coasts_in(feet: Vec3, t: Vec3, velocity: Vec3, friction: f64) -> bool {
    let (ox, oz) = (feet.x - t.x, feet.z - t.z);
    if ox * ox + oz * oz >= NEAR_POINT * NEAR_POINT {
        return false;
    }
    let scale = 0.4 / (1.0 - friction).max(1e-3);
    let (px, pz) = (ox + velocity.x * scale, oz + velocity.z * scale);
    px * px + pz * pz < COAST_WITHIN * COAST_WITHIN
}

fn friction_below(bot: &Client, feet: Vec3) -> f64 {
    let below = azalea::physics::get_block_pos_below_that_affects_movement(Position::new(feet));
    let state = state_at(bot, below).unwrap_or_default();
    Box::<dyn BlockTrait>::from(state).behavior().friction as f64
}

fn look_ahead(bot: &Client, eye_y: f64) -> Option<Vec3> {
    let path = bot.component::<ExecutingPath>().ok()?;
    let first = path.path.front()?.movement.target;
    let mut prev = path.last_reached_node;
    let mut furthest = first;
    let mut level = true;
    for edge in path.path.iter().take(LOOKAHEAD_NODES) {
        let t = edge.movement.target;
        if hops::is_hop(prev, t) {
            return None;
        }
        prev = t;
        level &= t.y == first.y;
        if level {
            furthest = t;
        }
    }
    let c = centre_at(furthest, eye_y);
    Some(Vec3::new(c[0], c[1], c[2]))
}

fn centre_at(b: BlockPos, y: f64) -> [f64; 3] {
    [b.x as f64 + 0.5, y, b.z as f64 + 0.5]
}

struct RouteFront {
    start: BlockPos,
    target: BlockPos,
    hop: bool,
    next_hop: Option<BlockPos>,
    ticks: usize,
}

fn route_front(bot: &Client) -> Option<RouteFront> {
    let path = bot.component::<ExecutingPath>().ok()?;
    let start = path.last_reached_node;
    let target = path.path.front()?.movement.target;
    let next_hop = path
        .path
        .get(1)
        .map(|e| e.movement.target)
        .filter(|&t| hops::is_hop(target, t));
    Some(RouteFront {
        start,
        target,
        hop: hops::is_hop(start, target),
        next_hop,
        ticks: path.ticks_since_last_node_reached,
    })
}

fn preaim(r: &RouteFront, intent: &ExecutorIntent, feet: Vec3, eye_y: f64) -> Option<Vec3> {
    let landing = r.next_hop.filter(|_| !r.hop && r.target.y <= r.start.y)?;
    if intent.jump || intent.sneak || intent.look_target.is_none() || intent.world_dir.is_none() {
        return None;
    }
    let c = centre_at(r.target, feet.y);
    let (dx, dz) = (c[0] - feet.x, c[2] - feet.z);
    if dx * dx + dz * dz > hops::PREAIM_WITHIN * hops::PREAIM_WITHIN {
        return None;
    }
    let p = centre_at(landing, eye_y);
    Some(Vec3::new(p[0], p[1], p[2]))
}

fn dig_point(bot: &Client, block: BlockPos, eye: Option<[f64; 3]>) -> Vec3 {
    let state = state_at(bot, block);
    let progressing =
        bot.is_mining() && bot.component::<MineBlockPos>().ok().and_then(|p| p.0) == Some(block);
    let best = state.and_then(|st| best_tool(bot, st));
    let limit = STALL_TICKS as u32 + mine_extra_ticks();

    let (aim, tool, failed) = {
        let mut m = mine_at();
        if m.at != Some(block) {
            m.at = Some(block);
            m.aim = None;
            m.tool = best;
            m.tool_ready = false;
            Track::ask(&mut m.track, block);
        }
        MINING.store(true, Relaxed);
        if let (Some(eye), Some(state), Ok(world)) = (eye, state, bot.world()) {
            let world = world.read();
            let keep = m.aim.is_some_and(|p| sees(&world.chunks, eye, block, p));
            if !keep {
                m.aim = aim_at(&world.chunks, eye, block, state);
            }
        }
        let failed = m.track.as_mut().is_some_and(|t| t.step(progressing, limit));
        (m.aim, m.tool, failed)
    };
    if failed {
        MINE_FAILED.store(true, Relaxed);
        BUSY.store(true, Relaxed);
    }

    let ready = select_tool(bot, tool);
    mine_at().tool_ready = ready;

    match aim {
        Some(p) => Vec3::new(p[0], p[1], p[2]),
        None => Vec3::new(
            block.x as f64 + 0.5,
            block.y as f64 + 0.5,
            block.z as f64 + 0.5,
        ),
    }
}

fn note_gone(bot: &Client) {
    let block = match &mine_at().track {
        Some(t) if !t.met => t.block,
        _ => return,
    };
    if !present(bot, block) {
        let mut m = mine_at();
        if let Some(t) = m.track.as_mut().filter(|t| t.block == block) {
            t.met = true;
        }
    }
}

static LOOK_OUT: AtomicBool = AtomicBool::new(false);

pub fn publish_look(shared: &SharedMutex, aim: Option<[f64; 3]>) {
    if aim.is_none() && !LOOK_OUT.load(Relaxed) {
        return;
    }
    LOOK_OUT.store(aim.is_some(), Relaxed);
    let rate = turn_rate();
    let mut st = shared.lock().unwrap();
    if let Some(aim) = aim
        && super::freecam::active()
    {
        turn::body_step(&mut st, aim, rate);
    }
    st.session.route_look = aim.map(|aim| CameraAim { aim, turn: rate });
}

pub fn drives_camera() -> bool {
    LOOK_OUT.load(Relaxed)
}

pub fn goto_quiet(bot: &Client, goal: QuietGoal, opts: QuietOpts) {
    if goal.empty() {
        cancel_quiet(bot);
        set_travel(Travel::Failed);
        return;
    }
    let feet = bot.position().unwrap_or_default();
    let shown = goal.box_at(feet);
    let route = Route { goal, opts };
    let wait = JUST_STOPPED.load(Relaxed);
    if !wait {
        route.send(bot);
    }
    let parkour = route.opts.parkour;
    begin(Target::Block(route), true, parkour);
    if wait {
        run().deferred = true;
    }
    draw(shown, shown, &[]);
}

pub fn user_route() -> bool {
    let r = run();
    r.target.is_some() && !r.quiet
}

pub fn travel_status() -> Travel {
    Travel::ALL[TRAVEL.load(Relaxed) as usize]
}

pub fn cancel_quiet(bot: &Client) {
    let quiet = {
        let r = run();
        r.quiet && r.target.is_some()
    };
    if quiet {
        bot.force_stop_pathfinding();
        end();
        JUST_STOPPED.store(true, Relaxed);
        BUSY.store(true, Relaxed);
    }
    set_travel(Travel::Idle);
}

pub fn reset() {
    PENDING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    GOING.store(false, Relaxed);
    IDLE.store(false, Relaxed);
    JUST_STOPPED.store(false, Relaxed);
    BUSY.store(false, Relaxed);
    PARKOUR.store(false, Relaxed);
    clear_mine(true);
    placing::clear(true);
    hops::clear();
    set_travel(Travel::Idle);
    *run() = Run::default();
    world_draw::clear(OWNER);
}

fn following() -> Option<String> {
    match &run().target {
        Some(Target::Player(f)) => Some(f.name.clone()),
        _ => None,
    }
}

fn opts() -> PathfinderOpts {
    let opts = PathfinderOpts::new()
        .allow_mining(false)
        .retry_on_no_path(false)
        .safety(safety());
    #[cfg(target_arch = "wasm32")]
    let opts = {
        use azalea::pathfinder::astar::PathfinderTimeout;
        opts.min_timeout(PathfinderTimeout::Time(WEB_MIN_SEARCH))
            .max_timeout(PathfinderTimeout::Time(WEB_MAX_SEARCH))
            .yield_fn(web_yield)
    };
    opts
}

fn safety() -> MoveSafety {
    let s = store();
    MoveSafety {
        avoid_hazards: s.flag(setting::AVOID_HAZARDS),
        max_fall: s.num(setting::MAX_FALL).round().max(1.0) as u32,
    }
}

fn opts_for(parkour: bool) -> PathfinderOpts {
    opts().successors_fn(if parkour {
        hops::with_parkour
    } else {
        no_parkour
    })
}

fn path_opts(o: &QuietOpts) -> PathfinderOpts {
    opts_for(o.parkour)
        .allow_mining(o.mining)
        .placing(o.placing.clone())
}

fn user_opts(bot: &Client) -> QuietOpts {
    let s = store();
    QuietOpts {
        parkour: s.flag(setting::PARKOUR),
        mining: s.flag(setting::DIG),
        placing: s.flag(setting::PLACE).then(|| placing::user_rules(bot)),
    }
}

fn no_parkour(ctx: &mut MovesCtx, node: RelBlockPos) {
    basic::basic_move(ctx, node);
    uncommon::uncommon_move(ctx, node);
    place::place_move(ctx, node);
}

#[cfg(target_arch = "wasm32")]
fn web_yield() -> std::pin::Pin<Box<dyn Future<Output = ()>>> {
    Box::pin(crate::platform::time::yield_to_browser())
}

fn find_player(bot: &Client, name: &str) -> Option<(Uuid, String)> {
    let me = bot.entity;
    let found = bot
        .any_entity_by::<(Entity, &GameProfileComponent), With<Player>>(
            |(e, p): (Entity, &GameProfileComponent)| {
                e != me && p.0.name.eq_ignore_ascii_case(name)
            },
        )
        .ok()??;
    let profile = found.get_component::<GameProfileComponent>()?;
    Some((profile.0.uuid, profile.0.name.clone()))
}

pub fn tick(bot: &Client, shared: &SharedMutex) {
    if !BUSY.load(Relaxed) {
        return;
    }
    let Ok(feet) = bot.position() else {
        let lost = following();
        let quiet = travel_status() == Travel::Going;
        reset();
        if quiet {
            set_travel(Travel::Failed);
        }
        if let Some(name) = lost {
            say(shared, RED, crate::lang::trf!("pathfinder.lost", name));
        }
        return;
    };

    let stopped_now = JUST_STOPPED.swap(false, Relaxed);
    let live = run().target.is_some();
    let mine_failed = MINE_FAILED.swap(false, Relaxed);
    let place_failed = placing::take_failed();
    if (mine_failed || place_failed) && live && GOING.load(Relaxed) {
        bot.force_stop_pathfinding();
        let note = if mine_failed {
            crate::lang::tr!("pathfinder.cancelled.mine")
        } else {
            crate::lang::tr!("pathfinder.cancelled.place")
        };
        finish(shared, Some((note, RED)));
        JUST_STOPPED.store(true, Relaxed);
        BUSY.store(true, Relaxed);
        return;
    }
    if live && !GOING.load(Relaxed) {
        let note = match following() {
            Some(name) => (
                crate::lang::trf!("pathfinder.stopped_following", name),
                GREEN,
            ),
            None => (crate::lang::tr!("pathfinder.cancelled").to_string(), RED),
        };
        finish(shared, Some((&note.0, note.1)));
    }
    if stopped_now {
        return;
    }

    let req = PENDING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    if let Some(req) = req
        && !start(bot, shared, req, feet)
    {
        return;
    }

    let deferred = {
        let mut r = run();
        let deferred = std::mem::take(&mut r.deferred);
        match &r.target {
            Some(Target::Block(route)) if deferred => Some(route.clone()),
            _ => None,
        }
    };
    if let Some(route) = deferred {
        route.send(bot);
    }

    let target = {
        let mut r = run();
        r.age = (r.age + 1).min(START_GRACE);
        let block = match &r.target {
            Some(Target::Block(_)) => Some(true),
            Some(Target::Player(_)) => Some(false),
            None => None,
        };
        (block, r.age)
    };
    let (Some(block), age) = target else {
        finish(shared, None);
        return;
    };
    if !GOING.load(Relaxed) {
        finish(shared, None);
        return;
    }
    let goal = if block {
        tick_goto(bot, shared, feet, age)
    } else {
        tick_follow(bot, shared, feet)
    };
    if let Some(goal) = goal {
        draw_route(bot, goal);
    }
}

fn start(bot: &Client, shared: &SharedMutex, req: Request, feet: Vec3) -> bool {
    let was = following();
    let stopped = |shared: &SharedMutex| {
        if let Some(name) = &was {
            say(
                shared,
                GREEN,
                crate::lang::trf!("pathfinder.stopped_following", name),
            );
        }
    };
    match req {
        Request::Stop => {
            bot.force_stop_pathfinding();
            finish(shared, None);
            match &was {
                Some(_) => stopped(shared),
                None => say(shared, GREEN, crate::lang::tr!("pathfinder.stopped")),
            }
            false
        }
        Request::Goto(at) => {
            let here = [
                feet.x.floor() as i32,
                feet.y.floor() as i32,
                feet.z.floor() as i32,
            ];
            let goal = BlockPos::new(
                at[0].resolve(here[0]),
                at[1].resolve(here[1]),
                at[2].resolve(here[2]),
            );
            let opts = user_opts(bot);
            let parkour = opts.parkour;
            let route = Route {
                goal: QuietGoal::Block(goal),
                opts,
            };
            route.send(bot);
            begin(Target::Block(route), false, parkour);
            draw(goal, goal, &[]);
            stopped(shared);
            say(
                shared,
                GREEN,
                crate::lang::trf!("pathfinder.going_to", goal.x, goal.y, goal.z),
            );
            true
        }
        Request::Follow(None) => {
            if was.is_some() {
                bot.force_stop_pathfinding();
                finish(shared, None);
                stopped(shared);
                return false;
            }
            say(shared, RED, crate::lang::tr!("pathfinder.follow_who"));
            keep_or_finish(shared)
        }
        Request::Follow(Some(name)) => {
            let own = bot
                .component::<GameProfileComponent>()
                .is_ok_and(|g| g.0.name.eq_ignore_ascii_case(&name));
            let found = if own {
                Err(true)
            } else {
                find_player(bot, &name).ok_or(false)
            };
            match found {
                Ok((uuid, name)) => {
                    let same = matches!(&run().target, Some(Target::Player(f)) if f.uuid == uuid);
                    if same {
                        say(
                            shared,
                            GREEN,
                            crate::lang::trf!("pathfinder.already_following", name),
                        );
                        return true;
                    }
                    let replacing = GOING.load(Relaxed);
                    if replacing {
                        bot.force_stop_pathfinding();
                    }
                    let follow = Follow::new(uuid, name.clone(), follow_radius(), feet, replacing);
                    begin(
                        Target::Player(follow),
                        false,
                        store().flag(setting::PARKOUR),
                    );
                    let text = if was.is_some() {
                        crate::lang::trf!("pathfinder.following_now", name)
                    } else {
                        crate::lang::trf!("pathfinder.following", name)
                    };
                    say(shared, GREEN, text);
                    true
                }
                Err(own) => {
                    let text = match (own, &was) {
                        (true, Some(old)) => crate::lang::trf!("pathfinder.self.stopped", old),
                        (true, None) => crate::lang::tr!("pathfinder.self").to_string(),
                        (false, Some(old)) => {
                            crate::lang::trf!("pathfinder.unseen.stopped", name, old)
                        }
                        (false, None) => crate::lang::trf!("pathfinder.unseen", name),
                    };
                    say(shared, RED, text);
                    if was.is_some() {
                        bot.force_stop_pathfinding();
                        finish(shared, None);
                        return false;
                    }
                    keep_or_finish(shared)
                }
            }
        }
    }
}

fn keep_or_finish(shared: &SharedMutex) -> bool {
    if GOING.load(Relaxed) {
        return true;
    }
    finish(shared, None);
    false
}

fn begin(target: Target, quiet: bool, parkour: bool) {
    clear_mine(true);
    placing::clear(true);
    hops::begin_route();
    PARKOUR.store(parkour, Relaxed);
    {
        let mut r = run();
        if r.quiet && r.target.is_some() && travel_status() == Travel::Going {
            set_travel(Travel::Failed);
        }
        *r = Run {
            target: Some(target),
            quiet,
            ..Run::default()
        };
    }
    if quiet {
        set_travel(Travel::Going);
    }
    IDLE.store(false, Relaxed);
    GOING.store(true, Relaxed);
    BUSY.store(true, Relaxed);
}

fn tick_goto(bot: &Client, shared: &SharedMutex, feet: Vec3, age: u32) -> Option<BlockPos> {
    let feet_block = player_pos_to_block_pos(feet);
    let (there, goal, quiet) = {
        let r = run();
        let Some(Target::Block(route)) = &r.target else {
            return None;
        };
        (
            route.goal.reached(feet_block),
            route.goal.box_at(feet),
            r.quiet,
        )
    };
    let dist = [
        goal.x as f64 + 0.5 - feet.x,
        goal.y as f64 - feet.y,
        goal.z as f64 + 0.5 - feet.z,
    ]
    .iter()
    .map(|d| d * d)
    .sum::<f64>()
    .sqrt();
    if age >= START_GRACE && bot.is_goto_target_reached() {
        if quiet {
            set_travel(if there {
                Travel::Arrived
            } else {
                Travel::Failed
            });
            finish(shared, None);
            return None;
        }
        let note = if dist <= ARRIVED_WITHIN {
            (crate::lang::tr!("pathfinder.arrived"), GREEN)
        } else {
            (crate::lang::tr!("pathfinder.no_route"), RED)
        };
        finish(shared, Some(note));
        return None;
    }
    if !quiet {
        let give_up = {
            let path = bot
                .component::<ExecutingPath>()
                .ok()
                .filter(|p| !p.path.is_empty());
            let route = path
                .as_ref()
                .map(|p| (p.last_reached_node, p.ticks_since_last_node_reached));
            let targets = path
                .iter()
                .flat_map(|p| p.path.iter().map(|e| e.movement.target));
            run()
                .restarts
                .step(route, targets, dist, mining() || placing())
        };
        if give_up {
            bot.force_stop_pathfinding();
            let note = crate::lang::trf!("pathfinder.gave_up_restarts", GOTO_RESTART_LIMIT + 1);
            finish(shared, Some((&note, RED)));
            return None;
        }
    }
    Some(goal)
}

fn tick_follow(bot: &Client, shared: &SharedMutex, feet: Vec3) -> Option<BlockPos> {
    let uuid = match &run().target {
        Some(Target::Player(f)) => f.uuid,
        _ => return None,
    };
    let Some(pos) = bot.entity_by_uuid(uuid).and_then(|e| e.position().ok()) else {
        let name = following().unwrap_or_default();
        bot.force_stop_pathfinding();
        finish(
            shared,
            Some((&crate::lang::trf!("pathfinder.lost", name), RED)),
        );
        return None;
    };
    let executing = bot.is_executing_path();
    let calculating = bot.is_calculating_path();
    IDLE.store(!executing, Relaxed);
    if !executing {
        clear_mine(false);
        placing::clear(false);
    }

    let radius = follow_radius();
    let (step, name) = {
        let mut r = run();
        let Some(Target::Player(f)) = &mut r.target else {
            return None;
        };
        f.radius = radius;
        let step = f.step(feet, pos, executing, calculating);
        let name = step.note.map(|_| f.name.clone());
        (step, name)
    };
    if step.repath {
        let o = user_opts(bot);
        PARKOUR.store(o.parkour, Relaxed);
        bot.start_goto_with_opts(RadiusGoal::new(pos, radius), path_opts(&o));
    }
    if let (Some(note), Some(name)) = (step.note, name) {
        match note {
            Note::Away => say(
                shared,
                RED,
                crate::lang::trf!("pathfinder.getting_away", name),
            ),
            Note::Stuck => say(shared, RED, crate::lang::trf!("pathfinder.no_way", name)),
            Note::GiveUp => {
                bot.force_stop_pathfinding();
                finish(
                    shared,
                    Some((&crate::lang::trf!("pathfinder.gave_up", name), RED)),
                );
                return None;
            }
        }
    }
    Some(BlockPos::from(pos))
}

fn finish(shared: &SharedMutex, note: Option<(&str, u32)>) {
    let quiet = run().quiet;
    end();
    if let Some((text, color)) = note
        && !quiet
    {
        say(shared, color, text);
    }
}

fn end() {
    GOING.store(false, Relaxed);
    IDLE.store(false, Relaxed);
    BUSY.store(false, Relaxed);
    PARKOUR.store(false, Relaxed);
    clear_mine(true);
    placing::clear(true);
    if PENDING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_some()
    {
        BUSY.store(true, Relaxed);
    }
    let quiet = {
        let mut r = run();
        let quiet = r.quiet && r.target.is_some();
        *r = Run::default();
        quiet
    };
    if quiet && travel_status() == Travel::Going {
        set_travel(Travel::Failed);
    }
    world_draw::clear(OWNER);
}

fn draw_route(bot: &Client, goal: BlockPos) {
    let changed = match bot.component::<ExecutingPath>() {
        Ok(p) => {
            let nodes = p
                .path
                .iter()
                .chain(p.queued_path.iter().flatten())
                .map(|e| e.movement.target);
            let from = p.last_reached_node;
            let same = run().drawn.as_ref().is_some_and(|k| {
                k.goal == goal && k.from == from && k.nodes.iter().copied().eq(nodes.clone())
            });
            (!same).then(|| (from, nodes.collect::<Vec<_>>()))
        }
        Err(_) => {
            let same = run()
                .drawn
                .as_ref()
                .is_some_and(|k| k.goal == goal && k.nodes.is_empty());
            (!same).then(|| (goal, Vec::new()))
        }
    };
    if let Some((from, nodes)) = changed {
        publish(DrawKey { goal, from, nodes });
    }
}

fn draw(goal: BlockPos, from: BlockPos, nodes: &[BlockPos]) {
    let same = run()
        .drawn
        .as_ref()
        .is_some_and(|k| k.goal == goal && k.from == from && k.nodes == nodes);
    if !same {
        publish(DrawKey {
            goal,
            from,
            nodes: nodes.to_vec(),
        });
    }
}

fn floor_center(p: BlockPos) -> [f64; 3] {
    [p.x as f64 + 0.5, p.y as f64 + 0.1, p.z as f64 + 0.5]
}

fn publish(key: DrawKey) {
    let mut out = Vec::with_capacity(key.nodes.len() + 1);
    let goal = key.goal;
    let min = [goal.x as f64, goal.y as f64, goal.z as f64];
    out.push(Node::boxed(
        min,
        [min[0] + 1.0, min[1] + 1.0, min[2] + 1.0],
        TARGET_COLOR,
        true,
    ));
    let mut prev = key.from;
    for &next in &key.nodes {
        out.push(Node::line(
            floor_center(prev),
            floor_center(next),
            ROUTE_WIDTH,
            ROUTE_COLOR,
            true,
        ));
        prev = next;
    }
    run().drawn = Some(key);
    world_draw::set(OWNER, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_read_numbers_and_tildes() {
        assert_eq!(Coord::parse("12"), Some(Coord::Abs(12)));
        assert_eq!(Coord::parse("-3"), Some(Coord::Abs(-3)));
        assert_eq!(Coord::parse("~"), Some(Coord::Rel(0)));
        assert_eq!(Coord::parse("~5"), Some(Coord::Rel(5)));
        assert_eq!(Coord::parse("~-2"), Some(Coord::Rel(-2)));
        assert_eq!(Coord::parse("x"), None);
        assert_eq!(Coord::parse("~x"), None);
        assert_eq!(Coord::parse("1.5"), None);
    }

    #[test]
    fn relative_coordinates_offset_the_feet() {
        assert_eq!(Coord::Rel(4).resolve(10), 14);
        assert_eq!(Coord::Abs(4).resolve(10), 4);
        assert_eq!(Coord::Rel(1).resolve(i32::MAX), i32::MAX);
    }

    #[test]
    fn travel_status_round_trips() {
        for t in Travel::ALL {
            assert_eq!(Travel::ALL[t as usize], t);
        }
    }

    fn at(x: f64) -> Vec3 {
        Vec3::new(x, 64.0, 0.0)
    }

    const RADIUS: f32 = 3.0;

    fn follow() -> Follow {
        Follow::new(Uuid::nil(), "Steve".to_string(), RADIUS, at(0.0), false)
    }

    #[test]
    fn after_a_stop_the_first_goal_waits_one_tick() {
        let mut f = Follow::new(Uuid::nil(), "Steve".to_string(), RADIUS, at(0.0), true);
        assert!(!f.step(at(0.0), at(10.0), false, false).repath);
        assert!(f.step(at(0.0), at(10.0), false, false).repath);
    }

    #[test]
    fn the_first_tick_paths_and_a_still_target_does_not_repath() {
        let mut f = follow();
        assert!(f.step(at(0.0), at(10.0), false, false).repath);
        for _ in 0..30 {
            assert!(!f.step(at(0.0), at(10.0), true, false).repath);
        }
    }

    #[test]
    fn a_moving_target_repaths_at_most_every_interval_and_never_mid_search() {
        let mut f = follow();
        f.step(at(0.0), at(10.0), false, false);
        assert!(!f.step(at(0.0), at(13.0), true, false).repath);
        for _ in 0..REPATH_TICKS {
            f.step(at(0.0), at(13.0), true, true);
        }
        assert!(
            !f.step(at(0.0), at(13.0), true, true).repath,
            "a search is still running"
        );
        assert!(f.step(at(0.0), at(13.0), true, false).repath);
    }

    #[test]
    fn in_reach_never_repaths() {
        let mut f = follow();
        assert!(!f.step(at(0.0), at(2.0), false, false).repath);
    }

    #[test]
    fn an_idle_follow_out_of_reach_retries() {
        let mut f = follow();
        f.step(at(0.0), at(10.0), false, false);
        let retried = (0..IDLE_RETRY_TICKS).any(|_| f.step(at(0.0), at(10.0), false, false).repath);
        assert!(retried);
    }

    #[test]
    fn no_progress_warns_once_then_gives_up() {
        let mut f = follow();
        let notes: Vec<Note> = (0..STUCK_TICKS + GIVE_UP_TICKS + 1)
            .filter_map(|_| f.step(at(0.0), at(10.0), true, false).note)
            .collect();
        assert_eq!(notes, [Note::Stuck, Note::GiveUp]);
    }

    #[test]
    fn progress_clears_a_warning_silently() {
        let mut f = follow();
        for _ in 0..=STUCK_TICKS {
            f.step(at(0.0), at(10.0), true, false);
        }
        assert!(f.warned.is_some());
        assert_eq!(f.step(at(3.0), at(10.0), true, false).note, None);
        assert!(f.warned.is_none());
    }

    #[test]
    fn a_target_far_and_not_closing_is_getting_away() {
        let mut f = follow();
        let notes: Vec<Note> = (0..=AWAY_TICKS)
            .filter_map(|i| {
                let me = i as f64 * 0.3;
                f.step(at(me), at(me + 30.0), true, false).note
            })
            .collect();
        assert_eq!(notes, [Note::Away]);
    }

    #[test]
    fn near_succeeds_beside_a_block_at_feet_or_head_level() {
        let ore = BlockPos::new(10, 20, 30);
        let goal = NearGoal(vec![ore]);
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            assert!(goal.success(BlockPos::new(10 + dx, 20, 30 + dz)));
            assert!(goal.success(BlockPos::new(10 + dx, 19, 30 + dz)));
            assert!(!goal.success(BlockPos::new(10 + dx, 21, 30 + dz)));
            assert!(!goal.success(BlockPos::new(10 + dx, 18, 30 + dz)));
        }
        assert!(!goal.success(BlockPos::new(11, 20, 31)));
        assert!(!goal.success(BlockPos::new(10, 21, 30)));
        assert!(!goal.success(BlockPos::new(10, 18, 30)));
        assert!(!goal.success(ore));
        assert!(!goal.success(BlockPos::new(12, 20, 30)));
        assert!(!NearGoal(Vec::new()).success(ore));
    }

    #[test]
    fn near_heuristic_is_zero_exactly_where_it_succeeds() {
        let ores = vec![BlockPos::new(0, 64, 0), BlockPos::new(20, 40, -7)];
        let goal = NearGoal(ores);
        for x in -3..=23 {
            for y in [38, 39, 40, 41, 62, 63, 64, 65] {
                for z in -9..=3 {
                    let n = BlockPos::new(x, y, z);
                    let h = goal.heuristic(n);
                    assert!(h >= 0.0, "{n:?}: {h}");
                    if goal.success(n) {
                        assert_eq!(h, 0.0, "{n:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn near_heuristic_takes_the_nearest_block_and_cell() {
        let a = BlockPos::new(0, 64, 0);
        let b = BlockPos::new(30, 64, 0);
        let both = NearGoal(vec![a, b]);
        let n = BlockPos::new(25, 64, 0);
        assert_eq!(
            both.heuristic(n),
            BlockPosGoal(BlockPos::new(29, 64, 0)).heuristic(n)
        );
        assert_eq!(both.heuristic(n), NearGoal(vec![b]).heuristic(n));
        let up = NearGoal(vec![BlockPos::new(5, 66, 0)]);
        let n = BlockPos::new(0, 64, 0);
        assert_eq!(
            up.heuristic(n),
            BlockPosGoal(BlockPos::new(4, 65, 0)).heuristic(n)
        );
    }

    #[test]
    fn a_near_goal_is_reached_by_its_own_test_and_boxes_the_nearest_block() {
        let goal = QuietGoal::Near(vec![BlockPos::new(0, 64, 0), BlockPos::new(9, 64, 0)]);
        assert!(goal.reached(BlockPos::new(8, 64, 0)));
        assert!(!goal.reached(BlockPos::new(7, 64, 0)));
        assert_eq!(
            goal.box_at(Vec3::new(7.5, 64.0, 0.5)),
            BlockPos::new(9, 64, 0)
        );
        let block = QuietGoal::Block(BlockPos::new(1, 2, 3));
        assert!(block.reached(BlockPos::new(1, 2, 3)));
        assert_eq!(
            block.box_at(Vec3::new(50.0, 0.0, 0.0)),
            BlockPos::new(1, 2, 3)
        );
    }

    #[test]
    fn coasting_starts_only_inside_the_near_point_with_the_stop_inside_the_tolerance() {
        let t = Vec3::new(0.5, 64.0, 0.5);
        let still = Vec3::ZERO;
        assert!(coasts_in(Vec3::new(0.6, 64.0, 0.5), t, still, 0.6));
        assert!(!coasts_in(Vec3::new(0.75, 64.0, 0.5), t, still, 0.6));
        assert!(coasts_in(
            Vec3::new(0.75, 64.0, 0.5),
            t,
            Vec3::new(-0.1, 0.0, 0.0),
            0.6
        ));
        assert!(!coasts_in(
            Vec3::new(0.6, 64.0, 0.5),
            t,
            Vec3::new(0.2, 0.0, 0.0),
            0.6
        ));
        assert!(!coasts_in(
            Vec3::new(1.0, 64.0, 0.5),
            t,
            Vec3::new(-0.5, 0.0, 0.0),
            0.6
        ));
        assert!(!coasts_in(
            Vec3::new(0.75, 64.0, 0.5),
            t,
            Vec3::new(-0.1, 0.0, 0.0),
            0.98
        ));
    }

    #[test]
    fn a_dig_stalls_after_the_budget_and_progress_resets_it() {
        let mut t = Track::new(BlockPos::new(0, 0, 0));
        let budget = STALL_TICKS as u32 + 20;
        for _ in 0..budget {
            assert!(!t.step(false, budget));
        }
        assert!(!t.step(true, budget));
        for _ in 0..budget {
            assert!(!t.step(false, budget));
        }
        assert!(t.step(false, budget));
    }

    #[test]
    fn a_block_asked_for_again_after_three_breaks_ends_the_route() {
        let b = BlockPos::new(1, 2, 3);
        let mut track = None;
        Track::ask(&mut track, b);
        for i in 1..=REVERT_LIMIT {
            track.as_mut().unwrap().met = true;
            Track::ask(&mut track, b);
            assert_eq!(track.unwrap().repeats, i);
        }
        assert!(track.as_mut().unwrap().step(true, u32::MAX));
        let mut track = None;
        Track::ask(&mut track, b);
        Track::ask(&mut track, b);
        assert_eq!(track.unwrap().repeats, 0);
        track.as_mut().unwrap().met = true;
        Track::ask(&mut track, BlockPos::new(9, 9, 9));
        assert_eq!(track, Some(Track::new(BlockPos::new(9, 9, 9))));
    }

    fn node(x: i32) -> BlockPos {
        BlockPos::new(x, 64, 0)
    }

    fn tick(r: &mut Restarts, last: i32, ticks: usize, dist: f64) -> bool {
        r.step(
            Some((node(last), ticks)),
            (last + 1..=last + 3).map(node),
            dist,
            false,
        )
    }

    #[test]
    fn walking_the_route_is_no_restart() {
        let mut r = Restarts::NEW;
        for i in 0..20 {
            assert!(!tick(&mut r, i, 1, 50.0));
            assert!(!tick(&mut r, i, 2, 50.0));
        }
        assert!(!tick(&mut r, 22, 0, 50.0));
        assert_eq!(r.count, 0);
    }

    #[test]
    fn restarts_without_progress_give_up_after_the_limit() {
        let mut r = Restarts::NEW;
        assert!(!tick(&mut r, 0, 30, 20.0));
        for n in 1..=GOTO_RESTART_LIMIT {
            assert!(!tick(&mut r, 0, 0, 20.0));
            assert_eq!(r.count, n);
            assert!(!tick(&mut r, 0, 30, 20.0));
        }
        assert!(tick(&mut r, 40, 0, 20.0));
    }

    #[test]
    fn progress_clears_the_restarts_and_a_dig_is_no_restart() {
        let mut r = Restarts::NEW;
        tick(&mut r, 0, 30, 20.0);
        tick(&mut r, 0, 0, 20.0);
        tick(&mut r, 9, 0, 20.0);
        assert_eq!(r.count, 2);
        tick(&mut r, 9, 1, 19.0);
        assert_eq!(r.count, 0);
        assert!(!r.step(Some((node(9), 0)), std::iter::empty(), 19.0, true));
        assert_eq!(r.count, 0);
    }

    #[test]
    fn a_route_that_ran_out_and_came_back_is_a_restart() {
        let mut r = Restarts::NEW;
        tick(&mut r, 0, 1, 20.0);
        assert_eq!(r.count, 0);
        assert!(!r.step(None, std::iter::empty(), 20.0, false));
        tick(&mut r, 0, 0, 20.0);
        assert_eq!(r.count, 1);
    }

    #[test]
    fn within_measures_from_the_eye_and_never_stands_in_a_block() {
        let b = BlockPos::new(0, 64, 0);
        let goal = WithinGoal::new(vec![b], 4.5);
        assert!(goal.success(BlockPos::new(1, 64, 0)));
        assert!(!goal.success(b));
        assert!(!goal.success(BlockPos::new(0, 63, 0)));
        assert!(goal.success(BlockPos::new(0, 65, 0)));
        assert!(!goal.success(BlockPos::new(5, 64, 0)));
        assert!(!WithinGoal::new(Vec::new(), 4.5).success(b));
    }

    #[test]
    fn within_heuristic_never_overestimates_and_is_zero_where_it_succeeds() {
        let blocks = vec![BlockPos::new(0, 64, 0), BlockPos::new(12, 60, -5)];
        let goal = WithinGoal::new(blocks, 3.0);
        let mut cells = Vec::new();
        for x in -6..=18 {
            for y in 54..=70 {
                for z in -11..=6 {
                    let c = BlockPos::new(x, y, z);
                    if goal.success(c) {
                        cells.push(c);
                    }
                }
            }
        }
        assert!(!cells.is_empty());
        for x in (-10..=22).step_by(4) {
            for y in (50..=74).step_by(4) {
                for z in (-15..=10).step_by(5) {
                    let n = BlockPos::new(x, y, z);
                    let h = goal.heuristic(n);
                    let truth = cells
                        .iter()
                        .map(|c| BlockPosGoal(*c).heuristic(n))
                        .fold(f32::INFINITY, f32::min);
                    assert!(h >= 0.0 && h <= truth + 1e-3, "{n:?}: {h} > {truth}");
                }
            }
        }
        for c in &cells {
            assert_eq!(goal.heuristic(*c), 0.0, "{c:?}");
        }
    }

    #[test]
    fn a_within_goal_is_reached_by_its_own_test_and_boxes_the_nearest_block() {
        let goal = QuietGoal::Within {
            blocks: vec![BlockPos::new(0, 64, 0), BlockPos::new(9, 64, 0)],
            range: 3.0,
        };
        assert!(goal.reached(BlockPos::new(7, 64, 0)));
        assert!(!goal.reached(BlockPos::new(9, 64, 0)));
        assert_eq!(
            goal.box_at(Vec3::new(7.5, 64.0, 0.5)),
            BlockPos::new(9, 64, 0)
        );
        assert!(
            QuietGoal::Within {
                blocks: Vec::new(),
                range: 3.0
            }
            .empty()
        );
    }
}
