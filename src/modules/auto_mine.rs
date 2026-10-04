use std::sync::{Arc, Mutex};

use azalea::Client;
use azalea::block::fluid_state::{FluidKind, FluidState};
use azalea::block::{BlockState, BlockTrait};
use azalea::mining::MineBlockPos;
use azalea_core::hit_result::BlockHitResult;
use azalea_core::position::BlockPos;

use super::registry::{Id, auto_mine as setting};
use super::{Edge, MS_PER_TICK, Phase, Store, roll, store};
use crate::session::{Gamemode, MineIntent, SharedMutex, SharedState};

const RELEASE: f64 = 0.15;

const STALL_TICKS: i64 = 60;

const JITTER_YAW: f32 = 1.5;
const JITTER_PITCH: f32 = 2.0;

const NOMINAL_RANGE: f32 = 0.5 + RELEASE as f32;

static EDGE: Edge = Edge::new(Id::AutoMine);

static RUN: Mutex<Option<Run>> = Mutex::new(None);

pub fn enabled() -> bool {
    store().enabled(Id::AutoMine)
}

struct Run {
    step: (i32, i32),
    origin: BlockPos,
    col: i32,
    sideways: f32,
    stage: Stage,
    strikes: u32,
}

#[derive(Clone, Copy)]
enum Stage {
    Mine {
        pos: BlockPos,
        up: f32,
        arm_at: i64,
        stall_at: i64,
        turn: f32,
    },
    Walk,
}

pub fn tick(bot: &Client, shared: &Arc<SharedMutex>, hit: Option<&BlockHitResult>) -> bool {
    let s = store();
    match EDGE.poll(s) {
        Phase::Off => return false,
        Phase::Stopped => {
            *RUN.lock().unwrap() = None;
            clear(shared);
            return false;
        }
        Phase::Started => *RUN.lock().unwrap() = None,
        Phase::Running => {}
    }

    let Ok(pos) = bot.position() else {
        clear(shared);
        *RUN.lock().unwrap() = None;
        return false;
    };

    let in_gui = s.flag(setting::IN_GUI);
    let (blocked, yaw) = {
        let st = shared.lock().unwrap();
        let blocked = !st.in_world
            || (st.screen_open && !in_gui)
            || st.session.dead
            || st.session.gamemode == Gamemode::Spectator;
        (blocked, st.camera_yaw)
    };
    if blocked {
        clear(shared);
        return false;
    }

    let now = crate::client::tracking::game_time();
    let mut guard = RUN.lock().unwrap();
    let run = guard.get_or_insert_with(|| Run::start(pos.x, pos.y, pos.z, yaw));

    let mut intent = MineIntent {
        aim: None,
        forward: false,
        turn: 0.0,
    };
    let mut attack = false;
    let mut halted = None;
    for _ in 0..3 {
        match run.stage {
            Stage::Mine {
                pos: target,
                up,
                arm_at,
                stall_at,
                turn,
            } => {
                if !present(bot, target) {
                    run.strikes = 0;
                    match choose(bot, run, now, s) {
                        Ok(stage) => run.stage = stage,
                        Err(halt) => {
                            halted = Some(halt);
                            break;
                        }
                    }
                    continue;
                }
                intent.aim = Some(face_point(run.step, target, run.sideways, up));
                intent.turn = turn;
                let on_target = hit.is_some_and(|h| h.block_pos == target);
                attack = on_target && now >= arm_at;
                let progressing =
                    bot.component::<MineBlockPos>().ok().and_then(|p| p.0) == Some(target);
                if progressing {
                    run.stage = Stage::Mine {
                        pos: target,
                        up,
                        arm_at,
                        stall_at: now + STALL_TICKS,
                        turn,
                    };
                } else if now >= stall_at {
                    run.strikes += 1;
                    if run.strikes >= 2 {
                        halted = Some(Halt::Stalled);
                        break;
                    }
                    match choose(bot, run, now, s) {
                        Ok(stage) => run.stage = stage,
                        Err(halt) => halted = Some(halt),
                    }
                }
                break;
            }
            Stage::Walk => {
                let goal = column_centre(run) * axis_sign(run.step) as f64 - RELEASE;
                if along(pos.x, pos.z, run.step) < goal {
                    intent.forward = true;
                    break;
                }
                run.col += 1;
                run.sideways = jitter(JITTER_YAW);
                let length = s.num(setting::LENGTH) as i32;
                if length > 0 && run.col > length {
                    halted = Some(Halt::Done);
                    break;
                }
                match choose(bot, run, now, s) {
                    Ok(stage) => run.stage = stage,
                    Err(halt) => {
                        halted = Some(halt);
                        break;
                    }
                }
            }
        }
    }
    drop(guard);
    if let Some(why) = halted {
        return stop(shared, why);
    }

    debug_assert!(!(attack && intent.forward));
    shared.lock().unwrap().session.auto_mine = Some(intent);
    attack
}

impl Run {
    fn start(x: f64, y: f64, z: f64, yaw: f32) -> Run {
        let step = match ((-yaw / 90.0).round() as i32).rem_euclid(4) {
            0 => (0, -1),
            1 => (1, 0),
            2 => (0, 1),
            _ => (-1, 0),
        };
        Run {
            step,
            origin: BlockPos::new(x.floor() as i32, y.floor() as i32, z.floor() as i32),
            col: 0,
            sideways: jitter(JITTER_YAW),
            stage: Stage::Walk,
            strikes: 0,
        }
    }
}

enum Halt {
    Done,
    Unbreakable,
    Hazard,
    Stalled,
}

enum Cell {
    Clear,
    Diggable,
    Unbreakable,
}

fn present(bot: &Client, pos: BlockPos) -> bool {
    state_at(bot, pos).is_some_and(|st| !st.is_air() && !is_fluid(st))
}

fn cell(bot: &Client, pos: BlockPos) -> Cell {
    let Some(state) = state_at(bot, pos) else {
        return Cell::Clear;
    };
    if state.is_air() || is_fluid(state) {
        return Cell::Clear;
    }
    let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
    if block.behavior().destroy_time < 0.0 {
        Cell::Unbreakable
    } else {
        Cell::Diggable
    }
}

fn state_at(bot: &Client, pos: BlockPos) -> Option<BlockState> {
    bot.world().ok().and_then(|w| w.read().get_block_state(pos))
}

fn is_fluid(state: BlockState) -> bool {
    FluidState::from(state).kind != FluidKind::Empty
}

fn choose(bot: &Client, run: &Run, now: i64, s: &Store) -> Result<Stage, Halt> {
    let lower = BlockPos::new(
        run.origin.x + run.step.0 * run.col,
        run.origin.y,
        run.origin.z + run.step.1 * run.col,
    );
    let upper = BlockPos::new(lower.x, lower.y + 1, lower.z);

    for pos in [upper, lower] {
        match cell(bot, pos) {
            Cell::Clear => continue,
            Cell::Unbreakable => return Err(Halt::Unbreakable),
            Cell::Diggable => {}
        }
        if s.flag(setting::HAZARDS) && exposes_fluid(bot, pos) {
            return Err(Halt::Hazard);
        }
        let (lo, hi) = s.range(setting::TURN);
        let (dlo, dhi) = s.range(setting::DELAY);
        return Ok(Stage::Mine {
            pos,
            up: jitter(JITTER_PITCH),
            arm_at: now + (roll(dlo, dhi) / MS_PER_TICK).round() as i64,
            stall_at: now + STALL_TICKS,
            turn: roll(lo, hi),
        });
    }
    Ok(Stage::Walk)
}

fn exposes_fluid(bot: &Client, pos: BlockPos) -> bool {
    let Ok(instance) = bot.world() else {
        return false;
    };
    let world = instance.read();
    [
        BlockPos::new(pos.x + 1, pos.y, pos.z),
        BlockPos::new(pos.x - 1, pos.y, pos.z),
        BlockPos::new(pos.x, pos.y + 1, pos.z),
        BlockPos::new(pos.x, pos.y - 1, pos.z),
        BlockPos::new(pos.x, pos.y, pos.z + 1),
        BlockPos::new(pos.x, pos.y, pos.z - 1),
    ]
    .iter()
    .any(|p| world.get_block_state(*p).is_some_and(is_fluid))
}

fn jitter(half: f32) -> f32 {
    (roll(0.0, half * 2.0) - half).to_radians() * NOMINAL_RANGE
}

fn face_point(step: (i32, i32), pos: BlockPos, sideways: f32, up: f32) -> [f32; 3] {
    let (sx, sz) = (step.0 as f32, step.1 as f32);
    let (px, pz) = (-sz, sx);
    [
        pos.x as f32 + 0.5 - sx * 0.5 + px * sideways,
        pos.y as f32 + 0.5 + up,
        pos.z as f32 + 0.5 - sz * 0.5 + pz * sideways,
    ]
}

fn axis_sign(step: (i32, i32)) -> i32 {
    if step.0 != 0 { step.0 } else { step.1 }
}

fn along(x: f64, z: f64, step: (i32, i32)) -> f64 {
    let coord = if step.0 != 0 { x } else { z };
    coord * axis_sign(step) as f64
}

fn column_centre(run: &Run) -> f64 {
    let origin = if run.step.0 != 0 {
        run.origin.x
    } else {
        run.origin.z
    };
    (origin + axis_sign(run.step) * run.col) as f64 + 0.5
}

fn clear(shared: &Arc<SharedMutex>) {
    shared.lock().unwrap().session.auto_mine = None;
}

fn stop(shared: &Arc<SharedMutex>, why: Halt) -> bool {
    *RUN.lock().unwrap() = None;
    store().set_enabled(Id::AutoMine, false);
    let mut st = shared.lock().unwrap();
    st.session.auto_mine = None;
    super::chat_math::show_local(
        &mut st.session,
        match why {
            Halt::Done => "AutoMine: reached the end of the tunnel",
            Halt::Unbreakable => "AutoMine: stopped at a block that cannot be broken",
            Halt::Hazard => "AutoMine: stopped, there is a fluid behind that wall",
            Halt::Stalled => "AutoMine: stopped, the block is not breaking",
        },
    );
    false
}

pub fn look_step(s: &SharedState, yaw: f32, pitch: f32, dt: f32, d: (f32, f32)) -> (f32, f32) {
    let Some(intent) = s.session.auto_mine else {
        return d;
    };
    let Some(aim) = intent.aim else {
        return d;
    };
    super::turn::look_toward(s, yaw, pitch, dt, d, aim, intent.turn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_facing_snaps_to_the_axis_it_is_nearest() {
        let step = |yaw| Run::start(0.5, 64.0, 0.5, yaw).step;
        assert_eq!(step(0.0), (0, -1), "north");
        assert_eq!(step(-20.0), (0, -1), "still north");
        assert_eq!(step(-90.0), (1, 0), "east");
        assert_eq!(step(180.0), (0, 1), "south");
        assert_eq!(step(-180.0), (0, 1), "south the other way round");
        assert_eq!(step(90.0), (-1, 0), "west");
        assert_eq!(step(400.0), (-1, 0), "past a full turn");
    }

    #[test]
    fn progress_grows_toward_the_next_column() {
        for step in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            let run = Run {
                step,
                origin: BlockPos::new(10, 64, 10),
                col: 1,
                sideways: 0.0,
                stage: Stage::Walk,
                strikes: 0,
            };
            let goal = column_centre(&run) * axis_sign(step) as f64;
            let here = along(10.5, 10.5, step);
            assert!(here < goal, "{step:?}: {here} should be short of {goal}");
            assert!(
                (goal - here - 1.0).abs() < 1e-9,
                "{step:?}: a column is one block"
            );
        }
    }

    #[test]
    fn the_aim_point_sits_on_the_near_face() {
        for step in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            let pos = BlockPos::new(4, 64, 7);
            let bound = |half: f32| half.to_radians() * NOMINAL_RANGE;
            let (sw, up) = (bound(JITTER_YAW), bound(JITTER_PITCH));
            for at in [(0.0, 0.0), (sw, up), (-sw, -up), (sw, -up)] {
                let p = face_point(step, pos, at.0, at.1);
                assert!(p[0] >= 4.0 && p[0] <= 5.0, "{step:?} {at:?} x {p:?}");
                assert!(p[1] >= 64.0 && p[1] <= 65.0, "{step:?} {at:?} y {p:?}");
                assert!(p[2] >= 7.0 && p[2] <= 8.0, "{step:?} {at:?} z {p:?}");
                let face = if step.0 != 0 { p[0] } else { p[2] };
                let centre = if step.0 != 0 { 4.5 } else { 7.5 };
                let near = centre - axis_sign(step) as f32 * 0.5;
                assert!((face - near).abs() < 1e-6, "{step:?} {at:?} {p:?}");
            }
        }
    }

    #[test]
    fn the_aim_offset_stays_a_couple_of_degrees() {
        for half in [JITTER_YAW, JITTER_PITCH] {
            for _ in 0..64 {
                let blocks = jitter(half);
                let degrees = (blocks / NOMINAL_RANGE).atan().to_degrees();
                assert!(degrees.abs() <= half + 1e-3, "{degrees} degrees off centre");
            }
        }
        assert!(JITTER_YAW <= JITTER_PITCH, "yaw is the one that shows");
    }
}
