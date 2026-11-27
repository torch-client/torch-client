mod collect;
mod crop;
mod draw;
mod find;
pub mod hud;
mod lanes;
mod work;

pub use crop::Crop;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Mutex, PoisonError};

use azalea::Client;
use azalea::mining::MineBlockPos;
use azalea::pathfinder::world::{is_block_state_passable, is_block_state_standable};
use azalea::pathfinder::{PathfinderClientExt, player_pos_to_block_pos};
use azalea_core::hit_result::{BlockHitResult, HitResult};
use azalea_core::position::{BlockPos, ChunkPos};
use azalea_world::World;

use super::auto_mine::shared::{EYE, MAX_STALLS, REACH, STALL_TICKS, aim_at, gate, is_fluid};
use super::esp::table::Merged;
use super::pathfinder::{self, QuietGoal, QuietOpts, Travel};
use super::registry::{Id, farmer as setting};
use super::{Edge, Phase, Store, roll, store};
use crate::session::{CameraAim, Gamemode, SharedMutex};
use crate::util::mth::wrap_degrees;
use hud::{Note, State};
use work::{AWAY_TICKS, Reachable, Work};

const SPOT_TARGETS: usize = 8;

const SPOT_RADIUS: i32 = 4;

const TRAVEL_IDLE: i64 = 300;

const STILL_TICKS: i64 = 40;

const STILL_DIST: f64 = 0.25;

const STILL_NOTE_TICKS: i64 = 10;

const WATCH_TICKS: i64 = 200;

const WATCH_TRIPS: u32 = 3;

const WATCH_AWAY: i64 = 40;

const PAUSE_SAY_TICKS: i64 = 40;

const FLASH_TICKS: i64 = 40;

const WAIT_TICKS: i64 = 20;

const RESWEEP_TICKS: i64 = 20;

const COLLECT_TICKS: i64 = 1200;

const OTHERS: [Id; 3] = [Id::AutoMine, Id::OreMiner, Id::AutoBuild];

static EDGE: Edge = Edge::new(Id::AutoFarm);

static RUN: Mutex<Option<Run>> = Mutex::new(None);

static LOOK_OUT: AtomicBool = AtomicBool::new(false);

pub fn enabled() -> bool {
    store().enabled(Id::AutoFarm)
}

pub fn drives_camera() -> bool {
    LOOK_OUT.load(Relaxed)
}

pub fn reset() {
    *RUN.lock().unwrap_or_else(PoisonError::into_inner) = None;
    LOOK_OUT.store(false, Relaxed);
    hud::reset();
    draw::clear();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Leg {
    Harvest,
    Stop,
    Collect,
    Home,
}

#[derive(Clone, Copy, Debug)]
enum Stage {
    Scan,
    Choose {
        arrived: bool,
    },
    Break {
        pos: BlockPos,
        aim: [f64; 3],
        arm_at: i64,
        stall_at: i64,
        turn: f32,
    },
    Travel {
        leg: Leg,
        since: i64,
        at: [f64; 3],
    },
    Wait {
        until: i64,
        note: Note,
    },
}

impl Stage {
    fn walking(&self) -> bool {
        matches!(self, Stage::Travel { .. })
    }

    fn name(&self) -> &'static str {
        match self {
            Stage::Scan => crate::lang::tr!("farmer.stage.scan"),
            Stage::Choose { .. } => crate::lang::tr!("farmer.stage.choose"),
            Stage::Break { .. } => crate::lang::tr!("farmer.stage.break"),
            Stage::Travel { leg: Leg::Stop, .. } => crate::lang::tr!("farmer.stage.stop"),
            Stage::Travel {
                leg: Leg::Harvest, ..
            } => crate::lang::tr!("farmer.stage.harvest"),
            Stage::Travel {
                leg: Leg::Collect, ..
            } => crate::lang::tr!("farmer.stage.collect"),
            Stage::Travel { leg: Leg::Home, .. } => crate::lang::tr!("farmer.stage.home"),
            Stage::Wait { .. } => crate::lang::tr!("farmer.stage.wait"),
        }
    }
}

fn travel(leg: Leg, here: &Here) -> Stage {
    Stage::Travel {
        leg,
        since: here.now,
        at: here.pos,
    }
}

struct Run {
    world: Option<azalea_world::WorldName>,
    home: BlockPos,
    crop: Crop,
    range: i32,
    table: Merged,
    stage: Stage,
    work: Work,
    scanned: bool,
    unloaded: HashSet<ChunkPos>,
    generation: u32,
    swept_at: i64,
    spot: Option<(BlockPos, BlockPos)>,
    spots_tried: HashMap<BlockPos, Vec<BlockPos>>,
    plan: lanes::Plan,
    plan_gen: u32,
    lane_lines: Vec<[BlockPos; 2]>,
    floors: Vec<i32>,
    tour: VecDeque<collect::Waypoint>,
    visiting: Vec<i32>,
    marks: HashMap<i32, i64>,
    collect_until: Option<i64>,
    last_break: i64,
    stalls: u32,
    capped_said: bool,
    note: (Note, u32),
    flash: Option<(Note, u32, i64)>,
    held_since: Option<i64>,
    pause_said: bool,
    watch_at: [f64; 3],
    watch_since: i64,
    watch_harvested: u32,
    watch_items: i32,
    trips: u32,
    abandon: Option<Stage>,
}

impl Run {
    fn new(world: Option<azalea_world::WorldName>, home: BlockPos, s: &Store) -> Run {
        let crop = s.choice(setting::CROP);
        Run {
            world,
            home,
            crop,
            range: s.num(setting::RANGE).max(1.0) as i32,
            table: find::table(crop),
            stage: Stage::Scan,
            work: Work::default(),
            scanned: false,
            unloaded: HashSet::new(),
            generation: 0,
            swept_at: i64::MIN,
            spot: None,
            spots_tried: HashMap::new(),
            plan: lanes::Plan::default(),
            plan_gen: 0,
            lane_lines: Vec::new(),
            floors: Vec::new(),
            tour: VecDeque::new(),
            visiting: Vec::new(),
            marks: HashMap::new(),
            collect_until: None,
            last_break: i64::MIN,
            stalls: 0,
            capped_said: false,
            note: (Note::None, 0),
            flash: None,
            held_since: None,
            pause_said: false,
            watch_at: [0.0; 3],
            watch_since: i64::MAX,
            watch_harvested: 0,
            watch_items: 0,
            trips: 0,
            abandon: None,
        }
    }

    fn state(&self) -> State {
        match self.stage {
            Stage::Scan => State::Scanning,
            Stage::Choose { .. } | Stage::Break { .. } => State::Harvesting,
            Stage::Travel {
                leg: Leg::Harvest | Leg::Stop,
                ..
            } => State::Walking,
            Stage::Travel {
                leg: Leg::Collect, ..
            } => State::Collecting,
            Stage::Travel { leg: Leg::Home, .. } => State::Home,
            Stage::Wait { .. } => State::Waiting,
        }
    }
}

enum Halt {
    Done(WayHome),
    Stuck(Stage, WayHome),
    Stalled,
    OtherWorld,
}

#[derive(Clone, Copy, Debug)]
enum WayHome {
    Back,
    Failed,
}

struct Here {
    feet: BlockPos,
    pos: [f64; 3],
    eye: [f64; 3],
    look: (f32, f32),
    now: i64,
}

#[derive(Default)]
struct Out {
    look: Option<CameraAim>,
    attack: bool,
    halt: Option<Halt>,
}

pub fn tick(bot: &Client, shared: &SharedMutex, hit: Option<&BlockHitResult>) -> bool {
    let s = store();
    match EDGE.poll(s) {
        Phase::Off => return false,
        Phase::Stopped => {
            finish(bot, shared);
            return false;
        }
        Phase::Started => {
            end_run(bot);
            hud::reset();
            for id in OTHERS {
                if s.enabled(id) {
                    s.set_enabled(id, false);
                    let name = super::registry::module(id).label();
                    say(
                        shared,
                        &crate::lang::trf!("farmer.switched_other_off", name),
                    );
                }
            }
        }
        Phase::Running => {
            if let Some(&id) = OTHERS.iter().find(|&&id| s.enabled(id)) {
                let name = super::registry::module(id).label();
                say(shared, &crate::lang::trf!("farmer.switched_self_off", name));
                s.set_enabled(Id::AutoFarm, false);
                finish(bot, shared);
                return false;
            }
        }
    }

    let Ok(pos) = bot.position() else {
        if let Some(run) = RUN.lock().unwrap_or_else(PoisonError::into_inner).as_mut() {
            pause(bot, run);
        }
        publish(shared, None);
        return false;
    };
    let world = bot
        .component::<azalea_world::WorldName>()
        .ok()
        .map(|n| n.clone());
    let in_gui = s.flag(setting::IN_GUI);
    let (hand, look) = {
        let st = shared.lock().unwrap();
        (st.hand_move_keys != 0, (st.camera_yaw, st.camera_pitch))
    };
    let user = pathfinder::user_route();
    let held = gate(shared, in_gui).is_none() || hand || user;
    let why_held = if held {
        held_reason(shared, in_gui, hand, user)
    } else {
        Note::None
    };
    let now = crate::client::tracking::client_ticks();
    let feet = player_pos_to_block_pos(pos);
    let here = Here {
        feet,
        pos: [pos.x, pos.y, pos.z],
        eye: [pos.x, pos.y + EYE, pos.z],
        look,
        now,
    };
    let crop_now: Crop = s.choice(setting::CROP);
    let items = find::held_count(bot, crop_now.drop_item());

    let mut guard = RUN.lock().unwrap_or_else(PoisonError::into_inner);
    let run = guard.get_or_insert_with(|| Run::new(world.clone(), feet, s));
    let out = if run.world != world {
        Out {
            halt: Some(Halt::OtherWorld),
            ..Out::default()
        }
    } else if held {
        if !matches!(run.stage, Stage::Choose { .. } | Stage::Scan) {}
        pause(bot, run);
        let since = *run.held_since.get_or_insert(now);
        if !run.pause_said && now - since >= PAUSE_SAY_TICKS {
            run.pause_said = true;
            if let Some(why) = why_held.text(0) {
                say(shared, &crate::lang::trf!("farmer.paused", why));
            }
        }
        run.watch_since = now;
        Out::default()
    } else {
        run.held_since = None;
        run.pause_said = false;
        run.note = (Note::None, 0);
        let mut out = step(bot, shared, hit, run, s, &here);
        if out.halt.is_none() {
            out.halt = watchdog(bot, shared, run, &here, items);
        }
        out
    };
    let note = if held {
        (why_held, 0)
    } else {
        match run.flash {
            Some((n, k, until)) if now < until && run.note.0 == Note::None => (n, k),
            _ => run.note,
        }
    };
    hud::publish(
        if held { State::Paused } else { run.state() },
        note,
        run.work.harvested,
        run.work.to_go(),
        run.unloaded.len() as u32,
        run.scanned,
    );
    if s.flag(setting::RENDER) {
        let target = match run.stage {
            Stage::Break { pos, .. } => Some(pos),
            _ => None,
        };
        let work = &run.work;
        draw::show(
            run.home,
            target,
            work.to_go(),
            run.plan_gen,
            &run.lane_lines,
            || {
                let mut left: Vec<BlockPos> = work.ready(i64::MIN).collect();
                left.sort_unstable_by_key(|p| find::dist2(*p, feet));
                left.truncate(draw::SHOWN + 1);
                left
            },
        );
    } else {
        draw::clear();
    }
    let tally = (run.work.harvested, run.work.unreachable, run.crop);
    drop(guard);
    if let Some(why) = out.halt {
        stop(bot, shared, why, tally);
        return false;
    }
    publish(shared, out.look);
    out.attack
}

fn step(
    bot: &Client,
    shared: &SharedMutex,
    hit: Option<&BlockHitResult>,
    run: &mut Run,
    s: &Store,
    here: &Here,
) -> Out {
    let mut out = Out::default();
    late_chunks(bot, run, here);
    for _ in 0..4 {
        let again = match run.stage {
            Stage::Scan => scan(bot, shared, run, here),
            Stage::Choose { arrived } => choose(bot, run, s, here, arrived),
            Stage::Break { .. } => break_step(bot, run, hit, here, &mut out),
            Stage::Travel { leg, since, at } => {
                travel_step(bot, run, here, leg, since, at, &mut out)
            }
            Stage::Wait { until, note } => {
                run.note = (note, 0);
                if here.now >= until {
                    run.stage = Stage::Choose { arrived: false };
                    true
                } else {
                    false
                }
            }
        };
        if !again || out.halt.is_some() {
            break;
        }
    }
    out
}

fn held_reason(shared: &SharedMutex, in_gui: bool, hand: bool, user: bool) -> Note {
    let st = shared.lock().unwrap();
    if !st.in_world {
        Note::NoWorld
    } else if st.screen_open && !st.pause_screen && !in_gui {
        Note::Screen
    } else if st.session.dead {
        Note::Dead
    } else if st.session.gamemode == Gamemode::Spectator {
        Note::Spectator
    } else if hand {
        Note::Keys
    } else if user {
        Note::UserRoute
    } else {
        Note::None
    }
}

fn watchdog(
    bot: &Client,
    shared: &SharedMutex,
    run: &mut Run,
    here: &Here,
    items: i32,
) -> Option<Halt> {
    let d2: f64 = (0..3)
        .map(|i| (here.pos[i] - run.watch_at[i]).powi(2))
        .sum();
    let progress = d2 > STILL_DIST * STILL_DIST
        || run.work.harvested != run.watch_harvested
        || items != run.watch_items;
    if progress || run.watch_since == i64::MAX {
        if progress && run.watch_since != i64::MAX {
            run.trips = 0;
        }
        run.watch_at = here.pos;
        run.watch_since = here.now;
        run.watch_harvested = run.work.harvested;
        run.watch_items = items;
        return None;
    }
    if here.now - run.watch_since < WATCH_TICKS {
        return None;
    }
    let stage = run.stage;
    run.trips += 1;
    run.watch_since = here.now;
    say(shared, &crate::lang::trf!("farmer.stuck", stage.name()));
    run.flash = Some((Note::Stuck, 0, here.now + FLASH_TICKS));
    match stage {
        Stage::Travel { leg, .. } => {
            pathfinder::cancel_quiet(bot);
            match leg {
                Leg::Home => {
                    return Some(Halt::Stuck(run.abandon.unwrap_or(stage), WayHome::Failed));
                }
                Leg::Harvest => spot_failed(run, here.now),
                Leg::Stop => {}
                Leg::Collect => mark_visited(run, here.now),
            }
        }
        Stage::Break { pos, .. } => run.work.strike(pos, here.now, WATCH_AWAY),
        Stage::Scan | Stage::Choose { .. } | Stage::Wait { .. } => {}
    }
    if run.trips >= WATCH_TRIPS && run.abandon.is_none() {
        run.abandon = Some(stage);
        say(
            shared,
            &crate::lang::trf!("farmer.stuck_trips", WATCH_TRIPS),
        );
    }
    if !matches!(stage, Stage::Scan) {
        run.stage = Stage::Choose { arrived: false };
    }
    None
}

fn scan(bot: &Client, shared: &SharedMutex, run: &mut Run, here: &Here) -> bool {
    let Some(sweep) = find::sweep(bot, &run.table, run.crop, run.home, run.range) else {
        return false;
    };
    for t in sweep.targets {
        run.work.add(t);
    }
    run.unloaded = sweep.unloaded;
    run.generation = crate::client::worldsync::chunk_generation();
    run.swept_at = here.now;
    run.scanned = true;
    note_capped(shared, run, sweep.capped);
    plan(bot, run, here.feet);
    run.stage = Stage::Choose { arrived: false };
    true
}

fn plan(bot: &Client, run: &mut Run, feet: BlockPos) {
    let Ok(world) = bot.world() else {
        return;
    };
    let crop = run.crop;
    let targets: Vec<BlockPos> = run.work.listed().collect();
    let (plan, _rows, _scattered) = {
        let w = world.read();
        lanes::Plan::build(&targets, crop.half_width(), feet, |p| {
            lane_cell(&w, crop, p)
        })
    };
    run.floors.extend(
        targets
            .iter()
            .map(|t| t.y - 1)
            .chain(plan.lanes.iter().map(|(l, _)| l.y)),
    );
    run.floors.sort_unstable();
    run.floors.dedup();
    run.lane_lines = plan
        .lanes
        .iter()
        .map(|(l, _)| {
            [
                l.axis.at(l.from, l.cross, l.y),
                l.axis.at(l.to, l.cross, l.y),
            ]
        })
        .collect();
    run.plan_gen = run.plan_gen.wrapping_add(1);
    run.plan = plan;
}

fn lane_cell(w: &World, crop: Crop, p: BlockPos) -> lanes::Cell {
    let at = |y: i32| w.get_block_state(BlockPos::new(p.x, y, p.z));
    let Some(feet) = at(p.y) else {
        return lanes::Cell::Blocked;
    };
    if is_fluid(feet) || at(p.y - 1).is_some_and(is_fluid) {
        return lanes::Cell::Water;
    }
    let head = at(p.y + 1).is_some_and(is_block_state_passable);
    let floor = at(p.y - 1).is_some_and(is_block_state_standable);
    if !(head && floor) {
        lanes::Cell::Blocked
    } else if crop.is_plant(feet) {
        lanes::Cell::Crop
    } else if is_block_state_passable(feet) {
        lanes::Cell::Free
    } else {
        lanes::Cell::Blocked
    }
}

fn late_chunks(bot: &Client, run: &mut Run, here: &Here) {
    let now = here.now;
    if !run.scanned
        || run.unloaded.is_empty()
        || now < run.swept_at.saturating_add(RESWEEP_TICKS)
        || run.collect_until.is_some()
        || matches!(
            run.stage,
            Stage::Travel {
                leg: Leg::Collect | Leg::Home,
                ..
            }
        )
    {
        return;
    }
    let generation = crate::client::worldsync::chunk_generation();
    if generation == run.generation {
        return;
    }
    let Some(sweep) = find::sweep(bot, &run.table, run.crop, run.home, run.range) else {
        return;
    };
    run.generation = generation;
    run.swept_at = now;
    let arrived: HashSet<ChunkPos> = run.unloaded.difference(&sweep.unloaded).copied().collect();
    let mut added = 0;
    for t in sweep
        .targets
        .into_iter()
        .filter(|t| arrived.contains(&find::column(*t)))
    {
        if run.work.add(t) {
            added += 1;
        }
    }
    run.unloaded.retain(|c| !arrived.contains(c));
    if added > 0 {
        plan(bot, run, here.feet);
    }
}

fn note_capped(shared: &SharedMutex, run: &mut Run, capped: bool) {
    if capped && !run.capped_said {
        run.capped_said = true;
        say(
            shared,
            &crate::lang::trf!("farmer.capped", run.crop.label()),
        );
    }
}

fn choose(bot: &Client, run: &mut Run, s: &Store, here: &Here, arrived: bool) -> bool {
    if run.abandon.is_some() {
        return go_home(bot, run, here);
    }
    let crop = run.crop;
    let (pick, filtered, stop) = {
        let Ok(world) = bot.world() else {
            return false;
        };
        let w = world.read();
        let plant = |p: BlockPos, d: i32| {
            w.get_block_state(BlockPos::new(p.x, p.y - d, p.z))
                .is_some_and(|st| crop.is_plant(st))
        };
        run.work
            .settle(here.now, |t| crop.safe_to_break(|d| plant(t, d)));
        run.work
            .retain(|t| w.get_block_state(t).is_none() || crop.target(|d| plant(t, d)));
        let lane = run.plan.lane().cloned();
        let spot_target = run
            .spot
            .map(|(t, _)| t)
            .filter(|t| run.work.away(*t, here.now) && run.work.listed().any(|p| p == *t));
        let limit = (REACH + 1.0) * (REACH + 1.0);
        let mut reach: Vec<Reachable> = run
            .work
            .ready(here.now)
            .chain(spot_target)
            .filter(|t| lane.as_ref().is_none_or(|l| l.holds(*t)))
            .filter(|t| !(t.x == here.feet.x && t.z == here.feet.z && t.y < here.feet.y))
            .filter_map(|t| {
                let d2 = eye_d2(t, here.eye);
                if d2 > limit {
                    return None;
                }
                let state = w.get_block_state(t)?;
                let aim = aim_at(&w.chunks, here.eye, t, state)?;
                Some(Reachable {
                    pos: t,
                    aim,
                    turn: turn_to(here, aim),
                    d2,
                })
            })
            .collect();
        work::order(&mut reach);
        let mut ready: Vec<BlockPos> = run.work.ready(here.now).collect();
        ready.sort_unstable_by_key(|p| (find::dist2(*p, here.feet), p.x, p.y, p.z));
        let pick = reach.first().copied();
        let stop = if pick.is_none() {
            run.plan.next(&ready, here.feet, |p| plant(p, 0))
        } else {
            None
        };
        (pick, lane.is_some(), stop)
    };

    if let Some(r) = pick {
        run.spot = None;
        run.stage = break_stage(run, s, r.pos, r.aim, here.now);
        return true;
    }
    if arrived {
        spot_failed(run, here.now);
    }
    if let Some(stop) = stop {
        pathfinder::goto_quiet(bot, QuietGoal::Block(stop), opts());
        run.stage = travel(Leg::Stop, here);
        return false;
    }
    if filtered {
        return true;
    }
    if !run.work.finished() {
        let (spot, hopeless) = find_spot(bot, run, here);
        for t in &hopeless {
            run.work.give_up(*t);
        }
        if !hopeless.is_empty() {
            let before = match run.flash {
                Some((Note::NoSpot, n, until)) if here.now < until => n,
                _ => 0,
            };
            run.flash = Some((
                Note::NoSpot,
                before + hopeless.len() as u32,
                here.now + FLASH_TICKS,
            ));
        }
        if let Some((t, at)) = spot {
            run.spot = Some((t, at));
            pathfinder::goto_quiet(bot, QuietGoal::Block(at), opts());
            run.stage = travel(Leg::Harvest, here);
            return false;
        }
        if !hopeless.is_empty() {
            return true;
        }
        if !run.work.finished() {
            let note = if run.work.confirming() {
                Note::Confirming
            } else {
                Note::Chunks
            };
            run.stage = Stage::Wait {
                until: here.now + WAIT_TICKS,
                note,
            };
            return false;
        }
    }
    if s.flag(setting::COLLECT) {
        let until = *run.collect_until.get_or_insert(here.now + COLLECT_TICKS);
        if here.now < until && find::room_for(bot, crop.drop_item()) {
            if run.tour.is_empty() {
                plan_tour(bot, run, here);
            }
            while let Some(wp) = run.tour.pop_front() {
                if wp.at == here.feet {
                    run.visiting = wp.ids;
                    mark_visited(run, here.now);
                    continue;
                }
                run.visiting = wp.ids;
                pathfinder::goto_quiet(bot, QuietGoal::Block(wp.at), opts());
                run.stage = travel(Leg::Collect, here);
                return false;
            }
        } else {
            run.tour.clear();
        }
    }
    go_home(bot, run, here)
}

fn go_home(bot: &Client, run: &mut Run, here: &Here) -> bool {
    pathfinder::goto_quiet(bot, QuietGoal::Block(run.home), opts());
    run.stage = travel(Leg::Home, here);
    false
}

fn plan_tour(bot: &Client, run: &mut Run, here: &Here) {
    run.marks.retain(|_, until| *until > here.now);
    let skip: HashSet<i32> = run.marks.keys().copied().collect();
    let items: Vec<collect::Item> =
        find::drops(bot, run.crop, run.home, run.range, here.feet, &skip)
            .iter()
            .map(|d| collect::Item {
                id: d.id,
                pos: d.pos,
                block: d.block,
            })
            .collect();
    let crop = run.crop;
    let cleanup = {
        let Ok(world) = bot.world() else {
            return;
        };
        let w = world.read();
        collect::plan(
            &items,
            run.plan.axis,
            &run.floors,
            here.feet,
            run.home,
            |p| lane_cell(&w, crop, p),
        )
    };
    run.tour = collect::waypoints(&cleanup.passes, here.feet).into();
}

fn mark_visited(run: &mut Run, now: i64) {
    for id in std::mem::take(&mut run.visiting) {
        run.marks.insert(id, now + AWAY_TICKS);
    }
}

fn break_stage(run: &Run, s: &Store, pos: BlockPos, aim: [f64; 3], now: i64) -> Stage {
    let (lo, hi) = s.range(setting::TURN);
    let delay = s.num(setting::DELAY).max(0.0) as i64;
    Stage::Break {
        pos,
        aim,
        arm_at: now.max(run.last_break.saturating_add(delay)),
        stall_at: now + STALL_TICKS,
        turn: roll(lo, hi),
    }
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
    } = run.stage
    else {
        return false;
    };
    let crop = run.crop;
    let read = bot.world().ok().and_then(|world| {
        let w = world.read();
        let at = |d: i32| w.get_block_state(BlockPos::new(t.x, t.y - d, t.z));
        let (here_now, below) = (at(0)?, at(1)?);
        let plant = |d: i32| match d {
            0 => crop.is_plant(here_now),
            1 => crop.is_plant(below),
            _ => at(d).is_some_and(|st| crop.is_plant(st)),
        };
        Some((plant(0), plant(1), crop.safe_to_break(&plant)))
    });
    let Some((standing, base, safe)) = read else {
        return false;
    };
    if !standing {
        run.last_break = here.now;
        run.stalls = 0;
        run.stage = Stage::Choose { arrived: false };
        if base {
            run.work.broken(t, here.now);
        } else {
            run.work.lost(t);
        }
        return true;
    }
    if !safe {
        run.work.lost(t);
        run.stage = Stage::Choose { arrived: false };
        return true;
    }
    out.look = Some(CameraAim { aim, turn });
    out.attack = hit.is_some_and(|h| h.block_pos == t) && here.now >= arm_at;
    let progressing =
        bot.is_mining() && bot.component::<MineBlockPos>().ok().and_then(|p| p.0) == Some(t);
    let blocked = hit.is_none() && matches!(bot.hit_result(), Ok(HitResult::Entity(_)));
    run.note = (
        if here.now < arm_at {
            Note::Delay
        } else if blocked {
            Note::Mob
        } else if out.attack {
            Note::Breaking
        } else {
            Note::Turning
        },
        0,
    );
    if progressing || blocked || here.now < arm_at {
        run.stage = Stage::Break {
            pos: t,
            aim,
            arm_at,
            stall_at: stall_at.max(here.now + STALL_TICKS),
            turn,
        };
    } else if here.now >= stall_at {
        out.attack = false;
        out.look = None;
        run.work.strike(t, here.now, AWAY_TICKS);
        run.stalls += 1;
        if run.stalls >= MAX_STALLS {
            out.halt = Some(Halt::Stalled);
        }
        run.stage = Stage::Choose { arrived: false };
    }
    false
}

fn travel_step(
    bot: &Client,
    run: &mut Run,
    here: &Here,
    leg: Leg,
    since: i64,
    at: [f64; 3],
    out: &mut Out,
) -> bool {
    match pathfinder::travel_status() {
        Travel::Arrived => {
            if leg == Leg::Home {
                out.halt = Some(match run.abandon {
                    Some(stage) => Halt::Stuck(stage, WayHome::Back),
                    None => Halt::Done(WayHome::Back),
                });
                return false;
            }
            if leg == Leg::Collect {
                mark_visited(run, here.now);
            }
            run.stage = Stage::Choose {
                arrived: leg == Leg::Harvest,
            };
            true
        }
        Travel::Failed => {
            route_failed(run, here, leg, out);
            false
        }
        Travel::Going | Travel::Idle
            if leg == Leg::Collect && run.collect_until.is_some_and(|u| here.now >= u) =>
        {
            pathfinder::cancel_quiet(bot);
            mark_visited(run, here.now);
            run.stage = Stage::Choose { arrived: false };
            false
        }
        Travel::Going | Travel::Idle => {
            let d2: f64 = (0..3).map(|i| (here.pos[i] - at[i]).powi(2)).sum();
            if d2 > STILL_DIST * STILL_DIST {
                run.stage = Stage::Travel {
                    leg,
                    since: here.now,
                    at: here.pos,
                };
                run.note = (walk_note(leg), 0);
                return false;
            }
            let searching = bot.is_calculating_path();
            let still = here.now - since;
            run.note = if searching {
                (Note::FindingPath, 0)
            } else if still >= STILL_NOTE_TICKS {
                (Note::RouteStill, 0)
            } else {
                (walk_note(leg), 0)
            };
            if still >= if searching { TRAVEL_IDLE } else { STILL_TICKS } {
                pathfinder::cancel_quiet(bot);
                run.flash = Some((Note::RouteStuck, 0, here.now + FLASH_TICKS));
                route_failed(run, here, leg, out);
            }
            false
        }
    }
}

fn walk_note(leg: Leg) -> Note {
    match leg {
        Leg::Stop => Note::Lane,
        Leg::Harvest => Note::Spot,
        Leg::Collect => Note::Pass,
        Leg::Home => Note::None,
    }
}

fn route_failed(run: &mut Run, here: &Here, leg: Leg, out: &mut Out) {
    match leg {
        Leg::Home => {
            out.halt = Some(match run.abandon {
                Some(stage) => Halt::Stuck(stage, WayHome::Failed),
                None => Halt::Done(WayHome::Failed),
            });
            return;
        }
        Leg::Harvest => spot_failed(run, here.now),
        Leg::Stop => {}
        Leg::Collect => mark_visited(run, here.now),
    }
    run.stage = Stage::Choose { arrived: false };
}

fn spot_failed(run: &mut Run, now: i64) {
    let Some((t, at)) = run.spot.take() else {
        return;
    };
    run.spots_tried.entry(t).or_default().push(at);
    run.work.strike(t, now, 0);
}

fn find_spot(
    bot: &Client,
    run: &Run,
    here: &Here,
) -> (Option<(BlockPos, BlockPos)>, Vec<BlockPos>) {
    let mut hopeless = Vec::new();
    let Ok(world) = bot.world() else {
        return (None, hopeless);
    };
    let w = world.read();
    let crop = run.crop;
    let plant = |p: BlockPos| w.get_block_state(p).is_some_and(|st| crop.is_plant(st));
    let mut todo: Vec<BlockPos> = run.work.listed().collect();
    todo.sort_unstable_by_key(|p| (find::dist2(*p, here.feet), p.x, p.y, p.z));
    let limit = REACH - lanes::ARRIVE;
    for t in todo.into_iter().take(SPOT_TARGETS) {
        let Some(state) = w.get_block_state(t) else {
            continue;
        };
        let tried = run.spots_tried.get(&t);
        let mut best: Option<(i64, BlockPos)> = None;
        for dy in -2..=0 {
            for dz in -SPOT_RADIUS..=SPOT_RADIUS {
                for dx in -SPOT_RADIUS..=SPOT_RADIUS {
                    let p = BlockPos::new(t.x + dx, t.y + dy, t.z + dz);
                    if p == here.feet || tried.is_some_and(|v| v.contains(&p)) {
                        continue;
                    }
                    let key = find::dist2(p, here.feet);
                    if best.is_some_and(|(k, _)| k <= key) {
                        continue;
                    }
                    let dry = match lane_cell(&w, crop, p) {
                        lanes::Cell::Free => true,
                        lanes::Cell::Crop => !plant(BlockPos::new(p.x, p.y + 1, p.z)),
                        lanes::Cell::Water | lanes::Cell::Blocked => false,
                    };
                    if !dry {
                        continue;
                    }
                    let eye = [
                        f64::from(p.x) + 0.5,
                        f64::from(p.y) + EYE,
                        f64::from(p.z) + 0.5,
                    ];
                    let close = aim_at(&w.chunks, eye, t, state).is_some_and(|aim| {
                        (0..3).map(|i| (aim[i] - eye[i]).powi(2)).sum::<f64>() <= limit * limit
                    });
                    if close {
                        best = Some((key, p));
                    }
                }
            }
        }
        match best {
            Some((_, p)) => return (Some((t, p)), hopeless),
            None => hopeless.push(t),
        }
    }
    (None, hopeless)
}

fn opts() -> QuietOpts {
    QuietOpts {
        parkour: false,
        mining: false,
        placing: None,
    }
}

fn eye_d2(p: BlockPos, eye: [f64; 3]) -> f64 {
    let c = [
        f64::from(p.x) + 0.5,
        f64::from(p.y) + 0.5,
        f64::from(p.z) + 0.5,
    ];
    (0..3).map(|i| (c[i] - eye[i]).powi(2)).sum()
}

fn turn_to(here: &Here, aim: [f64; 3]) -> f32 {
    let (yaw, pitch) = super::aim_assist::look_at(here.eye, aim);
    wrap_degrees(yaw - here.look.0).abs() + (pitch - here.look.1).abs()
}

fn pause(bot: &Client, run: &mut Run) {
    if run.stage.walking() {
        pathfinder::cancel_quiet(bot);
    }
    if !matches!(run.stage, Stage::Scan) {
        run.stage = Stage::Choose { arrived: false };
    }
}

fn end_run(bot: &Client) {
    let run = RUN.lock().unwrap_or_else(PoisonError::into_inner).take();
    if run.is_some_and(|r| r.stage.walking()) {
        pathfinder::cancel_quiet(bot);
    }
}

fn finish(bot: &Client, shared: &SharedMutex) {
    end_run(bot);
    draw::clear();
    publish(shared, None);
}

fn stop(
    bot: &Client,
    shared: &SharedMutex,
    why: Halt,
    (harvested, unreachable, crop): (u32, u32, Crop),
) {
    finish(bot, shared);
    store().set_enabled(Id::AutoFarm, false);
    let crop = crop.label();
    let text = match why {
        Halt::Done(WayHome::Back) => {
            crate::lang::trf!("farmer.end.done.back", harvested, crop, unreachable)
        }
        Halt::Done(WayHome::Failed) => {
            crate::lang::trf!("farmer.end.done.failed", harvested, crop, unreachable)
        }
        Halt::Stuck(stage, WayHome::Back) => {
            crate::lang::trf!(
                "farmer.end.stuck.back",
                stage.name(),
                harvested,
                crop,
                unreachable
            )
        }
        Halt::Stuck(stage, WayHome::Failed) => {
            crate::lang::trf!(
                "farmer.end.stuck.failed",
                stage.name(),
                harvested,
                crop,
                unreachable
            )
        }
        Halt::Stalled => crate::lang::trf!("farmer.end.stalled", harvested, crop, unreachable),
        Halt::OtherWorld => {
            crate::lang::trf!("farmer.end.other_world", harvested, crop, unreachable)
        }
    };
    say(shared, &text);
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
    st.session.farmer_look = look;
}

fn say(shared: &SharedMutex, text: &str) {
    let mut st = shared.lock().unwrap();
    super::chat_math::show_local(&mut st.session, text);
}
