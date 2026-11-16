use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::{Mutex, PoisonError};

use azalea::Client;
use azalea::block::BlockState;
use azalea::mining::MineBlockPos;
use azalea::pathfinder::player_pos_to_block_pos;
use azalea_core::hit_result::{BlockHitResult, HitResult};
use azalea_core::position::BlockPos;

use super::auto_mine::shared::{
    self, EYE, MAX_STALLS, REACH, STALL_TICKS, TRAVEL_PROGRESS, TRAVEL_TICKS, aim_at, best_tool,
    exposes_fluid, harvests, inventory_full, present, select_tool, solid, state_at,
};
use super::auto_mine::{FluidRule, InvFull, LowHealth};
use super::list::BitList;
use super::pathfinder::{self, QuietGoal, QuietOpts, Travel, near_success};
use super::registry::{Id, options, ore_miner as setting};
use super::seed_xray::table::Kind;
use super::{ButtonAction, Edge, MS_PER_TICK, Phase, Store, roll, store};
use crate::renderer::world_draw::{self, Node};
use crate::session::{CameraAim, SharedMutex};

mod find;
mod want;

pub use want::{Want, named};

options! {
    pub enum OreSource {
        Auto = "Auto",
        Xray = "Xray",
        SeedXray = "Seed Xray",
    }
}

pub static ORES: BitList = BitList::new(
    "ore_miner",
    "Ores",
    214.0,
    &super::seed_xray::GROUPS,
    false,
    super::esp::blocks::PRESETS,
    super::seed_xray::build_ores,
);

const CAP: usize = 32;

const VEIN_CAP: usize = 64;

const REVEAL_TICKS: i64 = 10;

const NO_ROUTE_TRIES: u32 = 3;

const GONE_WITHIN: i32 = 2;

const OWNER: &str = "ore_miner";

const TARGET_COLOR: u32 = 0x30FF_AA00;

static EDGE: Edge = Edge::new(Id::OreMiner);

static RUN: Mutex<Option<Run>> = Mutex::new(None);

static TARGET: Mutex<OneShot> = Mutex::new(OneShot {
    want: None,
    count: None,
});

struct OneShot {
    want: Option<Want>,
    count: Option<u32>,
}

struct Home {
    pos: BlockPos,
    world: Option<azalea_world::WorldName>,
}

static HOME: Mutex<Option<Home>> = Mutex::new(None);

static HOME_ASK: AtomicBool = AtomicBool::new(false);

static HOME_PRESSED_AT: AtomicU32 = AtomicU32::new(0x7FC0_0000);

const HOME_LABEL_SECS: f32 = 2.0;

static LOOK_OUT: AtomicBool = AtomicBool::new(false);

static DRAWN: Mutex<Option<BlockPos>> = Mutex::new(None);

pub fn enabled() -> bool {
    store().enabled(Id::OreMiner)
}

pub fn drives_camera() -> bool {
    LOOK_OUT.load(Relaxed)
}

pub fn kind_name(kind: Kind) -> String {
    format!("{kind:?}").to_lowercase()
}

pub fn kind_named(name: &str) -> Option<Kind> {
    Kind::ALL
        .into_iter()
        .find(|k| kind_name(*k).eq_ignore_ascii_case(name))
}

pub fn start(target: Option<Want>, count: Option<u32>) {
    set_target(target, count);
    store().set_enabled(Id::OreMiner, true);
}

fn set_target(want: Option<Want>, count: Option<u32>) {
    *TARGET.lock().unwrap_or_else(PoisonError::into_inner) = OneShot { want, count };
}

fn limit(s: &Store) -> u32 {
    let count = TARGET.lock().unwrap_or_else(PoisonError::into_inner).count;
    count.unwrap_or_else(|| s.num(setting::AMOUNT) as u32)
}

pub fn button(key: &str, now: f32) -> ButtonAction {
    if key == "set_home" {
        HOME_ASK.store(true, Relaxed);
        HOME_PRESSED_AT.store(now.to_bits(), Relaxed);
    }
    ButtonAction::None
}

pub fn button_text(key: &str, text: &'static str, now: f32) -> &'static str {
    let at = f32::from_bits(HOME_PRESSED_AT.load(Relaxed));
    if key == "set_home" && (0.0..HOME_LABEL_SECS).contains(&(now - at)) {
        "Home set"
    } else {
        text
    }
}

pub fn reset() {
    *RUN.lock().unwrap_or_else(PoisonError::into_inner) = None;
    set_target(None, None);
    *HOME.lock().unwrap_or_else(PoisonError::into_inner) = None;
    HOME_ASK.store(false, Relaxed);
    LOOK_OUT.store(false, Relaxed);
    clear_draw();
}

fn take_home_ask(bot: &Client, shared: &SharedMutex) {
    let Ok(pos) = bot.position() else {
        return;
    };
    HOME_ASK.store(false, Relaxed);
    let at = player_pos_to_block_pos(pos);
    let world = bot
        .component::<azalea_world::WorldName>()
        .ok()
        .map(|n| n.clone());
    *HOME.lock().unwrap_or_else(PoisonError::into_inner) = Some(Home { pos: at, world });
    say(
        shared,
        &format!("OreMiner: home set at {} {} {}", at.x, at.y, at.z),
    );
}

fn home_for(s: &Store, world: &Option<azalea_world::WorldName>) -> Result<BlockPos, &'static str> {
    if !s.flag(setting::HOME) {
        return Err("");
    }
    match HOME.lock().unwrap_or_else(PoisonError::into_inner).as_ref() {
        None => Err(", no home set"),
        Some(h) if h.world != *world => Err(", home is in another world"),
        Some(h) => Ok(h.pos),
    }
}

struct Run {
    world: Option<azalea_world::WorldName>,
    stage: Stage,
    list: Vec<BlockPos>,
    want: Want,
    bad: HashSet<BlockPos>,
    mined: u32,
    not_ore: u32,
    unreachable: u32,
    failed_trips: u32,
    stalls: u32,
    fallback_said: bool,
    tool_said: bool,
    rest: Vec<BlockPos>,
}

#[derive(Clone, Copy)]
enum Stage {
    Choose {
        arrived: bool,
    },
    Travel {
        until: i64,
        from: BlockPos,
    },
    Reveal {
        pos: BlockPos,
        until: i64,
    },
    Mine {
        pos: BlockPos,
        group: u8,
        aim: [f64; 3],
        arm_at: i64,
        stall_at: i64,
        turn: f32,
        tool: Option<u8>,
    },
    Home {
        pos: BlockPos,
        leg: Option<(i64, BlockPos)>,
    },
}

impl Run {
    fn new(world: Option<azalea_world::WorldName>) -> Run {
        Run {
            world,
            stage: Stage::Choose { arrived: false },
            list: Vec::new(),
            want: Want::Ores(0),
            bad: HashSet::new(),
            mined: 0,
            not_ore: 0,
            unreachable: 0,
            failed_trips: 0,
            stalls: 0,
            fallback_said: false,
            tool_said: false,
            rest: Vec::new(),
        }
    }

    fn drop_block(&mut self, pos: BlockPos) {
        self.bad.insert(pos);
        self.list.retain(|p| *p != pos);
    }

    fn tally(&self) -> (u32, u32, u32) {
        (self.mined, self.not_ore, self.unreachable)
    }
}

enum Halt {
    Done(&'static str),
    NoOre,
    NoBlocks(String),
    NoKinds,
    NoRoute,
    Stalled,
    Hazard,
    Full,
    NeedEsp,
    NeedSeed,
}

enum Next {
    Stage(Stage),
    Travel,
    Wait,
    Halt(Halt),
}

pub fn tick(bot: &Client, shared: &SharedMutex, hit: Option<&BlockHitResult>) -> bool {
    if HOME_ASK.load(Relaxed) {
        take_home_ask(bot, shared);
    }
    let s = store();
    match EDGE.poll(s) {
        Phase::Off => return false,
        Phase::Stopped => {
            end_run(bot);
            set_target(None, None);
            publish(shared, None);
            clear_draw();
            return false;
        }
        Phase::Started => {
            end_run(bot);
            if s.enabled(Id::AutoMine) {
                s.set_enabled(Id::AutoMine, false);
                say(
                    shared,
                    "OreMiner: switched AutoMine off, the two cannot run together",
                );
            }
        }
        Phase::Running => {}
    }

    if s.choice(setting::LOW_HEALTH) == LowHealth::Disconnect
        && shared::low_health(shared, true, s.num(setting::HEALTH), "OreMiner")
    {
        end_run(bot);
        s.set_enabled(Id::OreMiner, false);
        return false;
    }

    let Ok(pos) = bot.position() else {
        end_run(bot);
        publish(shared, None);
        clear_draw();
        return false;
    };
    let world = bot
        .component::<azalea_world::WorldName>()
        .ok()
        .map(|n| n.clone());
    {
        let mut guard = RUN.lock().unwrap_or_else(PoisonError::into_inner);
        if guard.as_ref().is_some_and(|r| r.world != world)
            && guard.take().is_some_and(|r| r.stage.walking())
        {
            pathfinder::cancel_quiet(bot);
        }
    }

    let hand = shared.lock().unwrap().hand_move_keys != 0;
    if shared::gate(shared, s.flag(setting::IN_GUI)).is_none() || hand || pathfinder::user_route() {
        if let Some(run) = RUN.lock().unwrap_or_else(PoisonError::into_inner).as_mut() {
            pause(bot, run);
        }
        publish(shared, None);
        return false;
    }

    let now = crate::client::tracking::game_time();
    let feet = player_pos_to_block_pos(pos);
    let eye = [pos.x, pos.y + EYE, pos.z];
    let mut guard = RUN.lock().unwrap_or_else(PoisonError::into_inner);
    let run = guard.get_or_insert_with(|| Run::new(world));

    let mut look = None;
    let mut attack = false;
    let mut halt = None;
    for _ in 0..4 {
        match run.stage {
            Stage::Choose { arrived } => match choose(bot, shared, run, s, feet, eye, now, arrived)
            {
                Next::Stage(stage) => {
                    run.stage = stage;
                    continue;
                }
                Next::Travel => {
                    pathfinder::goto_quiet(bot, QuietGoal::Near(run.list.clone()), opts(s));
                    run.stage = Stage::Travel {
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
            Stage::Travel { until, from } => {
                match pathfinder::travel_status() {
                    Travel::Arrived => {
                        run.failed_trips = 0;
                        run.stage = Stage::Choose { arrived: true };
                        continue;
                    }
                    Travel::Failed => {
                        run.unreachable += run.list.len() as u32;
                        let list = std::mem::take(&mut run.list);
                        run.bad.extend(list);
                        run.failed_trips += 1;
                        if run.failed_trips >= NO_ROUTE_TRIES {
                            halt = Some(Halt::NoRoute);
                        } else {
                            run.stage = Stage::Choose { arrived: false };
                        }
                    }
                    Travel::Going | Travel::Idle => match timed_out(bot, now, until, from, feet) {
                        None => {}
                        Some(true) => {
                            pathfinder::goto_quiet(bot, QuietGoal::Near(run.list.clone()), opts(s));
                            run.stage = Stage::Travel {
                                until: now + TRAVEL_TICKS,
                                from: feet,
                            };
                        }
                        Some(false) => {
                            if let Some(n) = nearest(&run.list, feet) {
                                run.drop_block(n);
                                run.unreachable += 1;
                            }
                            run.stage = Stage::Choose { arrived: false };
                        }
                    },
                }
                break;
            }
            Stage::Home { pos: home, leg } => {
                halt = home_step(bot, run, s, home, leg, feet, now);
                break;
            }
            Stage::Reveal { pos: t, until } => {
                let state = state_at(bot, t);
                if state.is_some_and(|st| !solid(st)) {
                    run.list.retain(|p| *p != t);
                    run.stage = Stage::Choose { arrived: false };
                    continue;
                }
                if state.and_then(|st| run.want.group(st)).is_some() {
                    run.stage = Stage::Choose { arrived: false };
                    continue;
                }
                if now >= until {
                    run.drop_block(t);
                    run.not_ore += 1;
                    run.stage = Stage::Choose { arrived: false };
                    continue;
                }
                break;
            }
            Stage::Mine {
                pos: t,
                group,
                aim,
                arm_at,
                stall_at,
                turn,
                tool,
            } => {
                if !present(bot, t) {
                    run.mined += 1;
                    run.stalls = 0;
                    run.list.retain(|p| *p != t);
                    let amount = limit(s);
                    if amount > 0 && run.mined >= amount {
                        halt = Some(Halt::Done(""));
                        break;
                    }
                    run.list = if s.flag(setting::VEIN) {
                        let vein =
                            find::vein(bot, t, &run.want, group, &run.bad, y_limits(s), VEIN_CAP);
                        let near = in_reach(&vein, feet, eye);
                        run.rest = vein.into_iter().filter(|p| !near.contains(p)).collect();
                        near
                    } else {
                        Vec::new()
                    };
                    run.stage = Stage::Choose { arrived: false };
                    continue;
                }
                let ready = select_tool(bot, tool);
                look = Some(CameraAim { aim, turn });
                attack = ready && hit.is_some_and(|h| h.block_pos == t) && now >= arm_at;
                let progressing = bot.is_mining()
                    && bot.component::<MineBlockPos>().ok().and_then(|p| p.0) == Some(t);
                let blocked = hit.is_none() && matches!(bot.hit_result(), Ok(HitResult::Entity(_)));
                if progressing || blocked {
                    run.stage = Stage::Mine {
                        pos: t,
                        group,
                        aim,
                        arm_at,
                        stall_at: now + STALL_TICKS,
                        turn,
                        tool,
                    };
                } else if now >= stall_at {
                    attack = false;
                    look = None;
                    run.drop_block(t);
                    run.unreachable += 1;
                    run.stalls += 1;
                    if run.stalls >= MAX_STALLS {
                        halt = Some(Halt::Stalled);
                    } else {
                        run.stage = Stage::Choose { arrived: false };
                    }
                }
                break;
            }
        }
    }
    if matches!(halt, Some(Halt::Done(""))) {
        halt = match home_for(s, &run.world) {
            Ok(home) => {
                look = None;
                attack = false;
                home_step(bot, run, s, home, None, feet, now)
            }
            Err(note) => Some(Halt::Done(note)),
        };
    }
    let target = match run.stage {
        Stage::Mine { pos, .. } | Stage::Reveal { pos, .. } => Some(pos),
        _ => None,
    };
    let tally = run.tally();
    drop(guard);
    if let Some(why) = halt {
        return stop(bot, shared, why, tally);
    }
    draw(target);
    publish(shared, look);
    attack
}

#[allow(clippy::too_many_arguments)]
fn choose(
    bot: &Client,
    shared: &SharedMutex,
    run: &mut Run,
    s: &Store,
    feet: BlockPos,
    eye: [f64; 3],
    now: i64,
    arrived: bool,
) -> Next {
    if s.choice(setting::INV_FULL) == InvFull::Stop && inventory_full(bot) {
        return Next::Halt(Halt::Full);
    }
    let want = wanted();
    if matches!(want, Want::Ores(0)) {
        return Next::Halt(Halt::NoKinds);
    }
    if !want.same(&run.want) {
        run.want = want;
        run.list.clear();
        run.rest.clear();
    }
    let amount = limit(s);
    let mut arrived = arrived;
    for _ in 0..8 {
        if run.list.is_empty() {
            match find::candidates(
                bot,
                shared,
                s,
                &run.want,
                feet,
                &run.bad,
                &mut run.fallback_said,
                CAP,
            ) {
                Ok(mut list) => {
                    let rest = std::mem::take(&mut run.rest);
                    list.extend(rest.into_iter().filter(|p| !run.bad.contains(p)));
                    find::sort_nearest(&mut list, feet);
                    list.dedup();
                    list.truncate(CAP);
                    run.list = list;
                }
                Err(h) => return Next::Halt(h),
            }
            if run.list.is_empty() {
                return Next::Halt(match &run.want {
                    Want::Blocks { name, .. } => Halt::NoBlocks(name.clone()),
                    Want::Ores(_) => Halt::NoOre,
                });
            }
        }
        let pick = {
            let Ok(world) = bot.world() else {
                return Next::Wait;
            };
            let w = world.read();
            let before = run.list.len();
            run.list.retain(|p| {
                let close = (p.x - feet.x)
                    .abs()
                    .max((p.y - feet.y).abs())
                    .max((p.z - feet.z).abs())
                    <= GONE_WITHIN;
                !(close && w.get_block_state(*p).is_some_and(|st| !solid(st)))
            });
            run.mined += (before - run.list.len()) as u32;
            in_reach(&run.list, feet, eye).into_iter().find_map(|t| {
                let state = w.get_block_state(t).filter(|st| solid(*st))?;
                aim_at(&w.chunks, eye, t, state).map(|aim| (t, state, aim))
            })
        };
        if amount > 0 && run.mined >= amount {
            return Next::Halt(Halt::Done(""));
        }
        if let Some((t, state, aim)) = pick {
            let Some(group) = run.want.group(state) else {
                if let Want::Blocks { .. } = run.want {
                    run.drop_block(t);
                    run.not_ore += 1;
                    continue;
                }
                return Next::Stage(Stage::Reveal {
                    pos: t,
                    until: now + REVEAL_TICKS,
                });
            };
            if exposes_fluid(bot, t) {
                match s.choice(setting::FLUIDS) {
                    FluidRule::Skip => {
                        run.drop_block(t);
                        continue;
                    }
                    FluidRule::Stop => return Next::Halt(Halt::Hazard),
                }
            }
            let stage = mine_stage(bot, s, t, state, group, aim, now);
            if let Stage::Mine { tool, .. } = stage
                && !harvests(bot, state, tool)
            {
                run.drop_block(t);
                run.unreachable += 1;
                if !run.tool_said {
                    run.tool_said = true;
                    say(
                        shared,
                        "OreMiner: no tool in the hotbar gets drops from some of these ores, so they are left",
                    );
                }
                continue;
            }
            return Next::Stage(stage);
        }
        if arrived {
            arrived = false;
            if let Some(n) = nearest(&run.list, feet) {
                run.drop_block(n);
                run.unreachable += 1;
            }
        }
        if run.list.is_empty() {
            continue;
        }
        return Next::Travel;
    }
    Next::Wait
}

fn mine_stage(
    bot: &Client,
    s: &Store,
    t: BlockPos,
    state: BlockState,
    group: u8,
    aim: [f64; 3],
    now: i64,
) -> Stage {
    let (lo, hi) = s.range(setting::TURN);
    let (dlo, dhi) = s.range(setting::DELAY);
    Stage::Mine {
        pos: t,
        group,
        aim,
        arm_at: now + (roll(dlo, dhi) / MS_PER_TICK).round() as i64,
        stall_at: now + STALL_TICKS,
        turn: roll(lo, hi),
        tool: best_tool(bot, state),
    }
}

fn timed_out(bot: &Client, now: i64, until: i64, from: BlockPos, feet: BlockPos) -> Option<bool> {
    if now < until {
        return None;
    }
    let moved = (feet.x - from.x).abs() + (feet.y - from.y).abs() + (feet.z - from.z).abs();
    pathfinder::cancel_quiet(bot);
    Some(moved >= TRAVEL_PROGRESS)
}

fn home_step(
    bot: &Client,
    run: &mut Run,
    s: &Store,
    home: BlockPos,
    leg: Option<(i64, BlockPos)>,
    feet: BlockPos,
    now: i64,
) -> Option<Halt> {
    let resend = match leg {
        None => true,
        Some((until, from)) => match pathfinder::travel_status() {
            Travel::Arrived => return Some(Halt::Done(", back home")),
            Travel::Failed => return Some(Halt::Done(", could not get home")),
            Travel::Going | Travel::Idle => match timed_out(bot, now, until, from, feet) {
                None => false,
                Some(true) => true,
                Some(false) => return Some(Halt::Done(", could not get home")),
            },
        },
    };
    if resend {
        pathfinder::goto_quiet(bot, QuietGoal::Block(home), opts(s));
        run.stage = Stage::Home {
            pos: home,
            leg: Some((now + TRAVEL_TICKS, feet)),
        };
    }
    None
}

fn opts(s: &Store) -> QuietOpts {
    QuietOpts {
        parkour: false,
        mining: s.flag(setting::DIG),
    }
}

fn y_limits(s: &Store) -> (i32, i32) {
    let (a, b) = (s.num(setting::MIN_Y) as i32, s.num(setting::MAX_Y) as i32);
    (a.min(b), a.max(b))
}

fn wanted() -> Want {
    if let Some(w) = TARGET
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .want
        .as_ref()
    {
        return w.clone();
    }
    Want::Ores(Kind::ALL.iter().fold(0, |m, k| match ORES.find(k.label()) {
        Some(row) if ORES.enabled(row) => m | 1 << *k as u16,
        _ => m,
    }))
}

fn in_reach(list: &[BlockPos], feet: BlockPos, eye: [f64; 3]) -> Vec<BlockPos> {
    let d2 = |t: &BlockPos| {
        let c = [
            f64::from(t.x) + 0.5,
            f64::from(t.y) + 0.5,
            f64::from(t.z) + 0.5,
        ];
        (0..3).map(|i| (c[i] - eye[i]).powi(2)).sum::<f64>()
    };
    let limit = (REACH + 1.0) * (REACH + 1.0);
    let mut out: Vec<(bool, f64, BlockPos)> = list
        .iter()
        .filter(|p| !(p.x == feet.x && p.z == feet.z && p.y < feet.y))
        .map(|p| (!near_success(std::slice::from_ref(p), feet), d2(p), *p))
        .filter(|(_, d, _)| *d <= limit)
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    out.into_iter().map(|(_, _, p)| p).collect()
}

fn wants(mask: u16, kind: Kind) -> bool {
    mask >> kind as u16 & 1 != 0
}

fn nearest(list: &[BlockPos], feet: BlockPos) -> Option<BlockPos> {
    list.iter().copied().min_by_key(|p| find::dist2(*p, feet))
}

fn pause(bot: &Client, run: &mut Run) {
    if run.stage.walking() {
        pathfinder::cancel_quiet(bot);
    }
    run.stage = match run.stage {
        Stage::Home { pos, .. } => Stage::Home { pos, leg: None },
        _ => Stage::Choose { arrived: false },
    };
}

impl Stage {
    fn walking(&self) -> bool {
        matches!(self, Stage::Travel { .. } | Stage::Home { .. })
    }
}

fn end_run(bot: &Client) {
    let run = RUN.lock().unwrap_or_else(PoisonError::into_inner).take();
    if run.is_some_and(|r| r.stage.walking()) {
        pathfinder::cancel_quiet(bot);
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
    st.session.ore_miner_look = look;
}

fn draw(target: Option<BlockPos>) {
    let mut drawn = DRAWN.lock().unwrap_or_else(PoisonError::into_inner);
    if *drawn == target {
        return;
    }
    *drawn = target;
    match target {
        Some(t) => {
            let min = [f64::from(t.x), f64::from(t.y), f64::from(t.z)];
            let max = [min[0] + 1.0, min[1] + 1.0, min[2] + 1.0];
            world_draw::set(OWNER, vec![Node::boxed(min, max, TARGET_COLOR, true)]);
        }
        None => world_draw::clear(OWNER),
    }
}

fn clear_draw() {
    draw(None);
}

fn say(shared: &SharedMutex, text: &str) {
    let mut st = shared.lock().unwrap();
    super::chat_math::show_local(&mut st.session, text);
}

fn stop(
    bot: &Client,
    shared: &SharedMutex,
    why: Halt,
    (mined, not_ore, unreachable): (u32, u32, u32),
) -> bool {
    end_run(bot);
    store().set_enabled(Id::OreMiner, false);
    clear_draw();
    let no_blocks;
    let mut note = "";
    let what = match why {
        Halt::Done(n) => {
            note = n;
            "done"
        }
        Halt::NoOre => "no ore in range",
        Halt::NoBlocks(name) => {
            no_blocks = format!("stopped, no {name} in render distance");
            no_blocks.as_str()
        }
        Halt::NoKinds => "stopped, no ore family is switched on in Ores",
        Halt::NoRoute => "stopped, no route reaches the ores",
        Halt::Stalled => "stopped, blocks are not breaking",
        Halt::Hazard => "stopped, there is a fluid behind the next ore",
        Halt::Full => "stopped, the inventory is full",
        Halt::NeedEsp => "stopped, switch Xray or OreESP on, or set Source to Seed Xray",
        Halt::NeedSeed => "stopped, Seed Xray has to be on with a seed for this Source",
    };
    let text = format!(
        "OreMiner: {what}; mined {mined}, not ore {not_ore}, unreachable {unreachable}{note}"
    );
    LOOK_OUT.store(false, Relaxed);
    let mut st = shared.lock().unwrap();
    st.session.ore_miner_look = None;
    super::chat_math::show_local(&mut st.session, &text);
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_a_command_name() {
        for k in Kind::ALL {
            let name = kind_name(k);
            assert!(!name.contains(' '), "{name}");
            assert_eq!(kind_named(&name), Some(k));
            assert_eq!(kind_named(&name.to_uppercase()), Some(k));
        }
        assert_eq!(kind_named("debris"), Some(Kind::Debris));
        assert_eq!(kind_named("stone"), None);
    }

    #[test]
    fn a_mask_bit_per_kind() {
        let mask = 1 << Kind::Diamond as u16 | 1 << Kind::Iron as u16;
        assert!(wants(mask, Kind::Diamond));
        assert!(wants(mask, Kind::Iron));
        assert!(!wants(mask, Kind::Coal));
    }

    #[test]
    fn in_reach_orders_adjacent_then_nearest() {
        let feet = BlockPos::new(0, 64, 0);
        let eye = [0.5, 64.0 + EYE, 0.5];
        let list = [
            BlockPos::new(0, 63, 0),
            BlockPos::new(0, 60, 0),
            BlockPos::new(0, 66, 0),
            BlockPos::new(1, 66, 0),
            BlockPos::new(1, 64, 0),
            BlockPos::new(0, 65, -1),
            BlockPos::new(9, 64, 0),
        ];
        let order = in_reach(&list, feet, eye);
        assert_eq!(
            order,
            vec![
                BlockPos::new(0, 65, -1),
                BlockPos::new(1, 64, 0),
                BlockPos::new(0, 66, 0),
                BlockPos::new(1, 66, 0),
            ]
        );
    }
}
