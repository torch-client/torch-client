use std::sync::Mutex;

use azalea::Client;
use azalea::block::properties::Charges;
use azalea::entity::Physics;
use azalea::entity::inventory::Inventory;
use azalea::movement::LastSentInput;
use azalea::physics::clip::{BlockShapeType, ClipContext, FluidPickType, clip};
use azalea::physics::collision::BlockWithShape;
use azalea_core::aabb::Aabb;
use azalea_core::direction::Direction;
use azalea_core::hit_result::BlockHitResult;
use azalea_core::position::{BlockPos, Vec3};
use azalea_registry::builtin::{BlockKind, ItemKind};
use azalea_world::World;

use super::registry::{Id, safe_anchor as setting};
use super::sent::{Look, look_at, sent_view, view_vector};
use super::turn::{Shape, SilentTurn};
use super::{Edge, Phase, roll, store, turn};
use crate::play::interaction::{module_place, module_use_on, select_hotbar_slot};
use crate::play::placement::{has_use_interaction, replaceable};
use crate::session::{CameraAim, Gamemode, InteractionHand, SharedMutex};

const REACH: f64 = 4.5;
const TIMEOUT: i64 = 40;

static EDGE: Edge = Edge::new(Id::SafeAnchor);

static RUN: Mutex<Run> = Mutex::new(Run::IDLE);

struct Run {
    pending: Option<BlockPos>,
    job: Option<Job>,
    aim: Option<[f64; 3]>,
    turn: f32,
    silent: SilentTurn,
    published: bool,
    restore: Option<Restore>,
    fence: Vec<BlockPos>,
    charged: Vec<BlockPos>,
    clock: i64,
}

impl Run {
    const IDLE: Run = Run {
        pending: None,
        job: None,
        aim: None,
        turn: 0.0,
        silent: SilentTurn::IDLE,
        published: false,
        restore: None,
        fence: Vec::new(),
        charged: Vec::new(),
        clock: 0,
    };
}

#[derive(Clone, Copy)]
struct Job {
    anchor: BlockPos,
    stage: Stage,
    started: i64,
}

#[derive(Clone, Copy, PartialEq)]
enum Stage {
    Charge,
    Shield,
}

#[derive(Clone, Copy)]
struct Restore {
    home: u8,
    taken: u8,
    at: i64,
}

enum Act {
    Wait,
    Done,
    Charge(BlockHitResult),
    Shield(BlockHitResult),
}

pub fn enabled() -> bool {
    store().enabled(Id::SafeAnchor)
}

pub fn drives_camera() -> bool {
    enabled() && !store().flag(setting::SILENT)
}

pub(crate) fn note_hand_place(bot: &Client, pos: BlockPos) {
    if !enabled() || !is_anchor(bot, pos) {
        return;
    }
    RUN.lock().unwrap().pending = Some(pos);
}

pub fn use_gate(
    bot: &Client,
    use_held: bool,
    use_clicked: bool,
    hit: Option<&BlockHitResult>,
) -> (bool, bool) {
    if !enabled() || crate::play::interaction::is_using_item() {
        return (use_held, use_clicked);
    }
    let mut run = RUN.lock().unwrap();
    if run.pending.is_some() || run.job.is_some() || run.restore.is_some() {
        return (false, false);
    }
    if use_clicked || !use_held {
        run.fence.clear();
        return (use_held, use_clicked);
    }
    let Some(hit) = hit else {
        return (use_held, use_clicked);
    };
    if store().flag(setting::HOLD) && detonates(bot, &run.charged, hit.block_pos) {
        run.fence.clear();
        return (use_held, use_clicked);
    }
    if run.fence.contains(&hit.block_pos) {
        return (false, false);
    }
    run.fence.clear();
    drop(run);
    if is_anchor(bot, hit.block_pos) {
        return (false, false);
    }
    (use_held, use_clicked)
}

fn detonates(bot: &Client, charged: &[BlockPos], pos: BlockPos) -> bool {
    let Some(state) = bot.world().ok().and_then(|w| w.read().get_block_state(pos)) else {
        return false;
    };
    if BlockKind::from(state) != BlockKind::RespawnAnchor {
        return false;
    }
    let has_charge = charged.contains(&pos)
        || state
            .property::<Charges>()
            .is_some_and(|c| c != Charges::_0);
    let glowstone_held = bot
        .component::<Inventory>()
        .is_ok_and(|inv| inv.held_item().kind() == ItemKind::Glowstone);
    has_charge && !glowstone_held
}

pub fn steer(bot: &Client, yaw: f32, pitch: f32, flags: u8) -> Option<(f32, f32, u8)> {
    let s = store();
    let mut run = RUN.lock().unwrap();
    if !s.flag(setting::SILENT) {
        run.silent.drop_out();
        return None;
    }
    let want = match run.aim {
        Some(aim) => {
            let Ok(eye) = bot.eye_position() else {
                run.silent.drop_out();
                return None;
            };
            Some(look_at([eye.x, eye.y, eye.z], aim))
        }
        None => None,
    };
    let rate = run.turn;
    let next = run
        .silent
        .next(bot, Look { yaw, pitch }, want, rate, Shape::Eased)?;
    Some((
        next.yaw,
        next.pitch,
        turn::walk_keys(flags, yaw, next.yaw, s.flag(setting::MOVE_FIX)),
    ))
}

pub fn tick(bot: &Client, shared: &SharedMutex) {
    let s = store();
    match EDGE.poll(s) {
        Phase::Off => return,
        Phase::Stopped => {
            let run = std::mem::replace(&mut *RUN.lock().unwrap(), Run::IDLE);
            if let Some(r) = run.restore {
                put_back(bot, r);
            }
            shared.lock().unwrap().session.safe_anchor = None;
            return;
        }
        Phase::Started => *RUN.lock().unwrap() = Run::IDLE,
        Phase::Running => {}
    }

    let mut run = RUN.lock().unwrap();
    run.clock += 1;
    let now = run.clock;
    if !run.charged.is_empty() {
        run.charged.retain(|&p| is_anchor(bot, p));
    }
    if let Some(pos) = run.pending.take()
        && is_anchor(bot, pos)
    {
        let (lo, hi) = s.range(setting::TURN);
        run.turn = roll(lo, hi);
        run.job = Some(Job {
            anchor: pos,
            stage: Stage::Charge,
            started: now,
        });
        run.fence = vec![pos];
    }

    let busy = match run.job {
        Some(job) if now - job.started <= TIMEOUT => step(bot, shared, &mut run, job, now),
        _ => false,
    };
    if !busy {
        run.job = None;
        run.aim = None;
    }

    if !busy
        && let Some(r) = run.restore
        && r.at < now
    {
        if s.flag(setting::SWAP_BACK) {
            put_back(bot, r);
        }
        run.restore = None;
    }

    let camera = if s.flag(setting::SILENT) {
        None
    } else {
        run.aim
    };
    if camera.is_some() || run.published {
        run.published = camera.is_some();
        shared.lock().unwrap().session.safe_anchor = camera.map(|aim| CameraAim {
            aim,
            turn: run.turn,
        });
    }
}

fn step(bot: &Client, shared: &SharedMutex, run: &mut Run, mut job: Job, now: i64) -> bool {
    let shield = store().flag(setting::SHIELD);
    let (selected, hotbar) = {
        let st = shared.lock().unwrap();
        if !st.in_world
            || st.screen_open
            || st.session.dead
            || st.session.gamemode == Gamemode::Spectator
        {
            return true;
        }
        let selected = st.session.hotbar_selected;
        let is_glowstone = |i: u8| {
            st.session
                .hotbar
                .get(i as usize)
                .is_some_and(|x| x.item == "glowstone")
        };
        let hotbar = if is_glowstone(selected) {
            Some(selected)
        } else {
            (0..9u8).find(|&i| is_glowstone(i))
        };
        (selected, hotbar)
    };
    let offhand = bot.component::<Inventory>().is_ok_and(|inv| {
        inv.inventory_menu
            .slot(45)
            .is_some_and(|st| st.kind() == ItemKind::Glowstone)
    });
    let Some((eye, look)) = sent_view(bot) else {
        return true;
    };
    let shift = bot.component::<LastSentInput>().is_ok_and(|i| i.0.shift);
    let body = match bot.component::<Physics>() {
        Ok(p) => p.bounding_box,
        Err(_) => return true,
    };

    let act = {
        let Ok(world) = bot.world() else {
            return true;
        };
        let w = world.read();
        if !w
            .get_block_state(job.anchor)
            .is_some_and(|st| BlockKind::from(st) == BlockKind::RespawnAnchor)
        {
            return false;
        }
        match job.stage {
            Stage::Charge => {
                if hotbar.is_none() && !offhand {
                    return false;
                }
                match trace(&w, eye, look).filter(|h| h.block_pos == job.anchor) {
                    Some(hit) if !shift => Act::Charge(hit),
                    Some(_) => Act::Wait,
                    None => {
                        run.aim = Some(face_point(eye, job.anchor));
                        Act::Wait
                    }
                }
            }
            Stage::Shield => match shield_support(&w, eye, &body, job.anchor) {
                None => Act::Done,
                Some(_) if hotbar.is_none() => Act::Done,
                Some((support, face, aim)) => {
                    match trace(&w, eye, look)
                        .filter(|h| h.block_pos == support && h.direction == face)
                    {
                        Some(hit) => Act::Shield(hit),
                        None => {
                            run.aim = Some(aim);
                            Act::Wait
                        }
                    }
                }
            },
        }
    };

    match act {
        Act::Wait => true,
        Act::Done => false,
        Act::Charge(hit) => {
            let hand = match hotbar {
                Some(slot) if slot == selected => InteractionHand::Main,
                _ if offhand => InteractionHand::Off,
                Some(slot) => {
                    take(bot, run, selected, slot, now);
                    InteractionHand::Main
                }
                None => return false,
            };
            module_use_on(bot, shared, &hit, hand);
            if !run.charged.contains(&job.anchor) {
                run.charged.push(job.anchor);
            }
            run.aim = None;
            if !shield {
                return false;
            }
            job.stage = Stage::Shield;
            run.job = Some(job);
            true
        }
        Act::Shield(hit) => {
            let Some(slot) = hotbar else {
                return false;
            };
            if slot != selected {
                take(bot, run, selected, slot, now);
            }
            module_place(bot, shared, &hit, (look.pitch, look.yaw), shift);
            run.fence.push(hit.block_pos);
            run.fence
                .push(hit.block_pos.offset_with_direction(hit.direction));
            false
        }
    }
}

fn is_anchor(bot: &Client, pos: BlockPos) -> bool {
    bot.world().ok().is_some_and(|w| {
        w.read()
            .get_block_state(pos)
            .is_some_and(|st| BlockKind::from(st) == BlockKind::RespawnAnchor)
    })
}

fn shield_support(
    w: &World,
    eye: [f64; 3],
    body: &Aabb,
    anchor: BlockPos,
) -> Option<(BlockPos, Direction, [f64; 3])> {
    let (dx, dz) = (
        eye[0] - (anchor.x as f64 + 0.5),
        eye[2] - (anchor.z as f64 + 0.5),
    );
    let toward = if dx.abs() >= dz.abs() {
        if dx > 0.0 {
            Direction::East
        } else {
            Direction::West
        }
    } else if dz > 0.0 {
        Direction::South
    } else {
        Direction::North
    };
    let cell = anchor.offset_with_direction(toward);
    let state = w.get_block_state(cell)?;
    if replaceable(state, BlockKind::Glowstone) != Some(true) {
        return None;
    }
    let min = Vec3::new(cell.x as f64, cell.y as f64, cell.z as f64);
    if body.intersects_aabb(&Aabb {
        min,
        max: min + Vec3::new(1.0, 1.0, 1.0),
    }) {
        return None;
    }
    [
        Direction::Down,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ]
    .into_iter()
    .filter(|&d| d != toward.opposite())
    .find_map(|d| {
        let support = cell.offset_with_direction(d);
        let st = w.get_block_state(support)?;
        if !st.is_collision_shape_full() || has_use_interaction(st) {
            return None;
        }
        let face = d.opposite();
        let aim = face_centre(support, face);
        let to = [
            eye[0] + (aim[0] - eye[0]) * 1.2,
            eye[1] + (aim[1] - eye[1]) * 1.2,
            eye[2] + (aim[2] - eye[2]) * 1.2,
        ];
        let hit = trace_to(w, eye, to);
        (!hit.miss
            && hit.block_pos == support
            && hit.direction == face
            && distance(eye, hit.location) <= REACH)
            .then_some((support, face, aim))
    })
}

fn face_centre(pos: BlockPos, face: Direction) -> [f64; 3] {
    let n = face.normal();
    [
        pos.x as f64 + 0.5 + n.x as f64 * 0.45,
        pos.y as f64 + 0.5 + n.y as f64 * 0.45,
        pos.z as f64 + 0.5 + n.z as f64 * 0.45,
    ]
}

fn face_point(eye: [f64; 3], anchor: BlockPos) -> [f64; 3] {
    let lo = [anchor.x as f64, anchor.y as f64, anchor.z as f64];
    std::array::from_fn(|i| eye[i].clamp(lo[i] + 0.2, lo[i] + 0.8))
}

fn trace(w: &World, eye: [f64; 3], look: Look) -> Option<BlockHitResult> {
    let d = view_vector(look);
    let to = [
        eye[0] + d[0] * REACH,
        eye[1] + d[1] * REACH,
        eye[2] + d[2] * REACH,
    ];
    let hit = trace_to(w, eye, to);
    (!hit.miss && distance(eye, hit.location) <= REACH).then_some(hit)
}

fn trace_to(w: &World, from: [f64; 3], to: [f64; 3]) -> BlockHitResult {
    clip(
        &w.chunks,
        ClipContext {
            from: Vec3::new(from[0], from[1], from[2]),
            to: Vec3::new(to[0], to[1], to[2]),
            block_shape_type: BlockShapeType::Outline,
            fluid_pick_type: FluidPickType::None,
            held_item: ItemKind::Air,
            entity: None,
        },
    )
}

fn distance(a: [f64; 3], b: Vec3) -> f64 {
    ((a[0] - b.x).powi(2) + (a[1] - b.y).powi(2) + (a[2] - b.z).powi(2)).sqrt()
}

fn take(bot: &Client, run: &mut Run, selected: u8, slot: u8, now: i64) {
    let home = run.restore.map_or(selected, |r| r.home);
    select_hotbar_slot(bot, slot);
    run.restore = (home != slot).then_some(Restore {
        home,
        taken: slot,
        at: now,
    });
}

fn put_back(bot: &Client, r: Restore) {
    if bot.selected_hotbar_slot().is_ok_and(|s| s == r.taken) {
        select_hotbar_slot(bot, r.home);
    }
}
