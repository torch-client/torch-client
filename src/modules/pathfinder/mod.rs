use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Mutex, PoisonError};

use azalea::Client;
use azalea::ecs::entity::Entity;
use azalea::ecs::query::With;
use azalea::entity::metadata::Player;
use azalea::pathfinder::goals::{BlockPosGoal, RadiusGoal};
use azalea::pathfinder::{ExecutingPath, PathfinderClientExt, PathfinderOpts};
use azalea::player::GameProfileComponent;
use azalea::{BlockPos, Vec3};
use uuid::Uuid;

use crate::modules::chat_math::{GREEN, RED, show_local_colored};
use crate::renderer::world_draw::{self, Node};
use crate::session::{
    MOVE_BACK, MOVE_DESCEND, MOVE_FORWARD, MOVE_JUMP, MOVE_LEFT, MOVE_RIGHT, MOVE_SNEAK,
    SharedMutex,
};

const OWNER: &str = "pathfinder";

const TAKEOVER: u8 =
    MOVE_FORWARD | MOVE_BACK | MOVE_LEFT | MOVE_RIGHT | MOVE_JUMP | MOVE_SNEAK | MOVE_DESCEND;

const START_GRACE: u32 = 5;

const ARRIVED_WITHIN: f64 = 2.5;

const FOLLOW_RADIUS: f32 = 3.0;
const IN_REACH: f64 = FOLLOW_RADIUS as f64 + 1.0;
const REPATH_TICKS: u32 = 10;
const REPATH_DIST: f64 = 1.5;
const IDLE_RETRY_TICKS: u32 = 40;
const PROGRESS_STEP: f64 = 0.5;
const MOVED_STEP: f64 = 2.0;
const AWAY_DIST: f64 = 24.0;
const AWAY_TICKS: u32 = 60;
const STUCK_TICKS: u32 = 100;
const GIVE_UP_TICKS: u32 = 300;

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

enum Target {
    Block(BlockPos),
    Player(Follow),
}

struct Follow {
    uuid: Uuid,
    name: String,
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
    fn new(uuid: Uuid, name: String, feet: Vec3, after_stop: bool) -> Follow {
        Follow {
            uuid,
            name,
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
        let in_reach = dist <= IN_REACH;

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
});

fn run() -> std::sync::MutexGuard<'static, Run> {
    RUN.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn request(request: Request) {
    *PENDING.lock().unwrap_or_else(PoisonError::into_inner) = Some(request);
    BUSY.store(true, Relaxed);
}

pub fn drives(bot: &Client, flags: u8) -> bool {
    if !GOING.load(Relaxed) {
        return false;
    }
    if flags & TAKEOVER != 0 {
        bot.force_stop_pathfinding();
        GOING.store(false, Relaxed);
        JUST_STOPPED.store(true, Relaxed);
        return false;
    }
    !IDLE.load(Relaxed)
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
    *run() = Run::default();
    world_draw::clear(OWNER);
}

fn say(shared: &SharedMutex, text: &str, color: u32) {
    show_local_colored(&mut shared.lock().unwrap().session, text, color);
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
        .retry_on_no_path(false);
    #[cfg(target_arch = "wasm32")]
    let opts = {
        use azalea::pathfinder::astar::PathfinderTimeout;
        opts.min_timeout(PathfinderTimeout::Time(WEB_MIN_SEARCH))
            .max_timeout(PathfinderTimeout::Time(WEB_MAX_SEARCH))
            .yield_fn(web_yield)
    };
    opts
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
        reset();
        if let Some(name) = lost {
            say(shared, &format!("Lost {name}"), RED);
        }
        return;
    };

    let stopped_now = JUST_STOPPED.swap(false, Relaxed);
    let live = run().target.is_some();
    if live && !GOING.load(Relaxed) {
        let note = match following() {
            Some(name) => (format!("Stopped following {name}"), GREEN),
            None => ("Route cancelled".to_string(), RED),
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

    let target = {
        let mut r = run();
        r.age = (r.age + 1).min(START_GRACE);
        let block = match &r.target {
            Some(Target::Block(goal)) => Some(Some(*goal)),
            Some(Target::Player(_)) => Some(None),
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
    let goal = match block {
        Some(goal) => tick_goto(bot, shared, feet, goal, age),
        None => tick_follow(bot, shared, feet),
    };
    if let Some(goal) = goal {
        draw_route(bot, goal);
    }
}

fn start(bot: &Client, shared: &SharedMutex, req: Request, feet: Vec3) -> bool {
    let was = following();
    let stopped = |shared: &SharedMutex| {
        if let Some(name) = &was {
            say(shared, &format!("Stopped following {name}"), GREEN);
        }
    };
    match req {
        Request::Stop => {
            bot.force_stop_pathfinding();
            finish(shared, None);
            match &was {
                Some(_) => stopped(shared),
                None => say(shared, "Stopped", GREEN),
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
            bot.start_goto_with_opts(BlockPosGoal(goal), opts());
            begin(Target::Block(goal));
            draw(goal, goal, &[]);
            stopped(shared);
            say(
                shared,
                &format!("Going to {} {} {}", goal.x, goal.y, goal.z),
                GREEN,
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
            say(shared, "Follow who? Add a player name", RED);
            keep_or_finish(shared)
        }
        Request::Follow(Some(name)) => {
            let own = bot
                .component::<GameProfileComponent>()
                .is_ok_and(|g| g.0.name.eq_ignore_ascii_case(&name));
            let found = if own {
                Err("That's you".to_string())
            } else {
                find_player(bot, &name).ok_or_else(|| format!("Can't see {name}"))
            };
            match found {
                Ok((uuid, name)) => {
                    let same = matches!(&run().target, Some(Target::Player(f)) if f.uuid == uuid);
                    if same {
                        say(shared, &format!("Already following {name}"), GREEN);
                        return true;
                    }
                    let replacing = GOING.load(Relaxed);
                    if replacing {
                        bot.force_stop_pathfinding();
                    }
                    begin(Target::Player(Follow::new(
                        uuid,
                        name.clone(),
                        feet,
                        replacing,
                    )));
                    let text = if was.is_some() {
                        format!("Following {name} now")
                    } else {
                        format!("Following {name}")
                    };
                    say(shared, &text, GREEN);
                    true
                }
                Err(why) => {
                    let text = match &was {
                        Some(old) => format!("{why}, stopped following {old}"),
                        None => why,
                    };
                    say(shared, &text, RED);
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

fn begin(target: Target) {
    *run() = Run {
        target: Some(target),
        age: 0,
        drawn: None,
    };
    IDLE.store(false, Relaxed);
    GOING.store(true, Relaxed);
    BUSY.store(true, Relaxed);
}

fn tick_goto(
    bot: &Client,
    shared: &SharedMutex,
    feet: Vec3,
    goal: BlockPos,
    age: u32,
) -> Option<BlockPos> {
    if age >= START_GRACE && bot.is_goto_target_reached() {
        let near = [
            goal.x as f64 + 0.5 - feet.x,
            goal.y as f64 - feet.y,
            goal.z as f64 + 0.5 - feet.z,
        ]
        .iter()
        .map(|d| d * d)
        .sum::<f64>()
            <= ARRIVED_WITHIN * ARRIVED_WITHIN;
        let note = if near {
            ("Arrived", GREEN)
        } else {
            ("No route to the target", RED)
        };
        finish(shared, Some(note));
        return None;
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
        finish(shared, Some((&format!("Lost {name}"), RED)));
        return None;
    };
    let executing = bot.is_executing_path();
    let calculating = bot.is_calculating_path();
    IDLE.store(!executing, Relaxed);

    let (step, name) = {
        let mut r = run();
        let Some(Target::Player(f)) = &mut r.target else {
            return None;
        };
        let step = f.step(feet, pos, executing, calculating);
        let name = step.note.map(|_| f.name.clone());
        (step, name)
    };
    if step.repath {
        bot.start_goto_with_opts(RadiusGoal::new(pos, FOLLOW_RADIUS), opts());
    }
    if let (Some(note), Some(name)) = (step.note, name) {
        match note {
            Note::Away => say(shared, &format!("{name}'s getting away"), RED),
            Note::Stuck => say(shared, &format!("Can't find a way to {name}"), RED),
            Note::GiveUp => {
                bot.force_stop_pathfinding();
                finish(shared, Some((&format!("Gave up on {name}"), RED)));
                return None;
            }
        }
    }
    Some(BlockPos::from(pos))
}

fn finish(shared: &SharedMutex, note: Option<(&str, u32)>) {
    GOING.store(false, Relaxed);
    IDLE.store(false, Relaxed);
    BUSY.store(false, Relaxed);
    if PENDING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_some()
    {
        BUSY.store(true, Relaxed);
    }
    *run() = Run::default();
    world_draw::clear(OWNER);
    if let Some((text, color)) = note {
        say(shared, text, color);
    }
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

    fn at(x: f64) -> Vec3 {
        Vec3::new(x, 64.0, 0.0)
    }

    fn follow() -> Follow {
        Follow::new(Uuid::nil(), "Steve".to_string(), at(0.0), false)
    }

    #[test]
    fn after_a_stop_the_first_goal_waits_one_tick() {
        let mut f = Follow::new(Uuid::nil(), "Steve".to_string(), at(0.0), true);
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
}
