use std::sync::{Arc, Mutex};

use azalea::Client;
use azalea::attack::AttackQueued;
use azalea::ecs::entity::Entity as AzEntity;
use azalea::entity::dimensions::EntityDimensions;
use azalea::entity::metadata::FallFlying;
use azalea::entity::{LastSentPosition, Physics};
use azalea::movement::LastSentLookDirection;

use super::registry::{Id, auto_mace as setting, options};
use super::{Edge, MS_PER_TICK, Phase, roll, store, turn};
use crate::session::{
    Gamemode, MOVE_BACK, MOVE_FORWARD, MOVE_LEFT, MOVE_RIGHT, MaceAim, SharedMutex, SharedState,
    SlotStack,
};
use crate::util::mth::wrap_degrees;

options! {
    pub enum AimPoint {
        Body = "Body",
        Head = "Head",
        Center = "Center",
        Nearest = "Nearest",
    }
}

options! {
    pub enum MaceSlot {
        Auto = "Auto",
        One = "1",
        Two = "2",
        Three = "3",
        Four = "4",
        Five = "5",
        Six = "6",
        Seven = "7",
        Eight = "8",
        Nine = "9",
    }
}

const TICK: f32 = MS_PER_TICK / 1000.0;

const STUCK_TICKS: i64 = 5;

static EDGE: Edge = Edge::new(Id::AutoMace);

static RUN: Mutex<Run> = Mutex::new(Run::IDLE);

struct Run {
    target: Option<Target>,
    silent: Option<Look>,
    turn: f32,
    swung: Option<Swung>,
}

impl Run {
    const IDLE: Run = Run {
        target: None,
        silent: None,
        turn: 0.0,
        swung: None,
    };
}

struct Target {
    id: i32,
    aim: [f64; 3],
}

#[derive(Clone, Copy)]
struct Swung {
    mace: u8,
    restore: Option<u8>,
    queued_at: i64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Look {
    yaw: f32,
    pitch: f32,
}

pub fn enabled() -> bool {
    store().enabled(Id::AutoMace)
}

pub fn drives_camera() -> bool {
    enabled() && !store().flag(setting::SILENT)
}

pub fn steer(bot: &Client, yaw: f32, pitch: f32, flags: u8) -> (f32, f32, u8) {
    let s = store();
    let camera = Look { yaw, pitch };
    let mut run = RUN.lock().unwrap();
    if !s.flag(setting::SILENT) {
        run.silent = None;
        return (yaw, pitch, flags);
    }

    let target_aim = run.target.as_ref().map(|t| t.aim);
    let want = match target_aim {
        Some(aim) => {
            let Ok(eye) = bot.eye_position() else {
                run.silent = None;
                return (yaw, pitch, flags);
            };
            look_at([eye.x, eye.y, eye.z], aim)
        }
        None if run.silent.is_some() => camera,
        None => return (yaw, pitch, flags),
    };

    let from = run.silent.unwrap_or(camera);
    let next = Look {
        yaw: from.yaw + turn::step(wrap_degrees(want.yaw - from.yaw), run.turn, TICK),
        pitch: (from.pitch + turn::step(want.pitch - from.pitch, run.turn, TICK))
            .clamp(-90.0, 90.0),
    };
    if run.target.is_none()
        && wrap_degrees(next.yaw - yaw).abs() < turn::DEADZONE
        && (next.pitch - pitch).abs() < turn::DEADZONE
    {
        run.silent = None;
        return (yaw, pitch, flags);
    }
    run.silent = Some(next);

    let flags = if s.flag(setting::MOVE_FIX) {
        move_fix(flags, yaw, next.yaw)
    } else {
        flags
    };
    (next.yaw, next.pitch, flags)
}

pub fn tick(bot: &Client, shared: &Arc<SharedMutex>) {
    let s = store();
    match EDGE.poll(s) {
        Phase::Off => return,
        Phase::Stopped => {
            let run = std::mem::replace(&mut *RUN.lock().unwrap(), Run::IDLE);
            if let Some(sw) = run.swung {
                settle(bot, sw, s.flag(setting::SWAP_BACK));
            }
            shared.lock().unwrap().session.auto_mace = None;
            return;
        }
        Phase::Started => *RUN.lock().unwrap() = Run::IDLE,
        Phase::Running => {}
    }

    let now = crate::client::tracking::game_time();
    let mut run = RUN.lock().unwrap();

    if let Some(sw) = run.swung {
        let queued = bot.component::<AttackQueued>().is_ok();
        if queued && now - sw.queued_at < STUCK_TICKS {
            return;
        }
        if queued {
            bot.ecs
                .write()
                .entity_mut(bot.entity)
                .remove::<AttackQueued>();
        }
        run.swung = None;
        settle(bot, sw, s.flag(setting::SWAP_BACK));
    }

    let silent = s.flag(setting::SILENT);
    let Some(ready) = ready(bot, shared) else {
        run.target = None;
        shared.lock().unwrap().session.auto_mace = None;
        return;
    };

    let range = s.num(setting::RANGE) as f64;
    let aim = s.choice(setting::AIM);
    let Some(found) = pick(
        bot,
        ready.eye,
        ready.camera,
        range,
        s.num(setting::FOV),
        s.flag(setting::WALLS),
        aim,
    ) else {
        run.target = None;
        shared.lock().unwrap().session.auto_mace = None;
        return;
    };

    if run.target.as_ref().is_none_or(|t| t.id != found.id) {
        let (lo, hi) = s.range(setting::TURN);
        run.turn = roll(lo, hi);
    }
    let point = aim_point(found.min, found.max, ready.eye, aim);
    run.target = Some(Target {
        id: found.id,
        aim: point,
    });
    shared.lock().unwrap().session.auto_mace = (!silent).then_some(MaceAim {
        aim: point.map(|v| v as f32),
        turn: run.turn,
    });

    if ready.strength < s.num(setting::COOLDOWN) / 100.0 {
        return;
    }
    let Some((origin, dir)) = sent_ray(bot) else {
        return;
    };
    if !ray_hits(origin, dir, range, found.min, found.max) {
        return;
    }

    let Ok(current) = bot.selected_hotbar_slot() else {
        return;
    };
    let restore = (current != ready.mace).then(|| {
        bot.set_selected_hotbar_slot(ready.mace);
        current
    });
    bot.ecs.write().entity_mut(bot.entity).insert(AttackQueued {
        target: found.entity,
    });
    crate::client::tick::swing_local_player(false, shared);
    run.swung = Some(Swung {
        mace: ready.mace,
        restore,
        queued_at: now,
    });
}

struct Ready {
    eye: [f64; 3],
    camera: Look,
    strength: f32,
    mace: u8,
}

fn ready(bot: &Client, shared: &Arc<SharedMutex>) -> Option<Ready> {
    let s = store();
    let falling = {
        let physics = bot.component::<Physics>().ok()?;
        !physics.on_ground()
            && !physics.was_touching_water
            && physics.fall_distance >= s.num(setting::MIN_FALL) as f64
    };
    if !falling || bot.component::<FallFlying>().is_ok_and(|f| **f) {
        return None;
    }

    let (camera, strength, mace) = {
        let st = shared.lock().unwrap();
        if !st.in_world
            || st.screen_open
            || st.session.dead
            || st.session.gamemode == Gamemode::Spectator
        {
            return None;
        }
        let mace = mace_slot(
            &st.session.hotbar,
            st.session.hotbar_selected,
            s.choice(setting::SLOT),
        )?;
        let camera = Look {
            yaw: -st.camera_yaw - 180.0,
            pitch: -st.camera_pitch,
        };
        (camera, st.session.attack_strength, mace)
    };

    let eye = bot.eye_position().ok()?;
    Some(Ready {
        eye: [eye.x, eye.y, eye.z],
        camera,
        strength,
        mace,
    })
}

fn settle(bot: &Client, sw: Swung, swap_back: bool) {
    if !swap_back {
        return;
    }
    if let Some(slot) = sw.restore
        && bot.selected_hotbar_slot().is_ok_and(|s| s == sw.mace)
    {
        bot.set_selected_hotbar_slot(slot);
    }
}

fn mace_slot(hotbar: &[SlotStack], selected: u8, choice: MaceSlot) -> Option<u8> {
    let is_mace = |i: u8| {
        hotbar
            .get(i as usize)
            .is_some_and(|st| !st.is_empty() && st.item == "mace")
    };
    match choice {
        MaceSlot::Auto if is_mace(selected) => Some(selected),
        MaceSlot::Auto => (0..9).find(|&i| is_mace(i)),
        fixed => {
            let i = fixed as u8 - 1;
            is_mace(i).then_some(i)
        }
    }
}

fn sent_ray(bot: &Client) -> Option<([f64; 3], [f64; 3])> {
    let pos = azalea::Vec3::from(&*bot.component::<LastSentPosition>().ok()?);
    let eye_height = bot.component::<EntityDimensions>().ok()?.eye_height as f64;
    let look = {
        let l = bot.component::<LastSentLookDirection>().ok()?;
        Look {
            yaw: l.y_rot,
            pitch: l.x_rot,
        }
    };
    Some(([pos.x, pos.y + eye_height, pos.z], view_vector(look)))
}

struct Found {
    id: i32,
    entity: AzEntity,
    min: [f64; 3],
    max: [f64; 3],
}

fn pick(
    bot: &Client,
    eye: [f64; 3],
    camera: Look,
    range: f64,
    fov: f32,
    walls: bool,
    aim: AimPoint,
) -> Option<Found> {
    let cone = (fov < 360.0).then(|| ((fov as f64) * 0.5).to_radians().cos());
    let mut found = candidates(bot, eye, view_vector(camera), range, cone);
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().map(|(_, f)| f).find(|f| {
        walls || {
            let to = aim_point(f.min, f.max, eye, aim);
            super::aim_assist::visible(bot, eye.map(|v| v as f32), to.map(|v| v as f32))
        }
    })
}

fn candidates(
    bot: &Client,
    eye: [f64; 3],
    forward: [f64; 3],
    range: f64,
    cone: Option<f64>,
) -> Vec<(f64, Found)> {
    let mut out = Vec::new();
    super::nearby::each(bot, |e| {
        if !e.health.is_some_and(|h| h > 0.0) || !super::entities::kind_enabled(e.kind) {
            return;
        }
        let b = &e.physics.bounding_box;
        let (min, max) = ([b.min.x, b.min.y, b.min.z], [b.max.x, b.max.y, b.max.z]);
        let dist = box_distance(eye, min, max);
        if dist > range {
            return;
        }
        if let Some(limit) = cone {
            let c = [
                (min[0] + max[0]) * 0.5,
                (min[1] + max[1]) * 0.5,
                (min[2] + max[2]) * 0.5,
            ];
            let to = [c[0] - eye[0], c[1] - eye[1], c[2] - eye[2]];
            let len = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt();
            let cos = (forward[0] * to[0] + forward[1] * to[1] + forward[2] * to[2]) / len;
            if len > 1e-6 && cos < limit {
                return;
            }
        }
        out.push((
            dist,
            Found {
                id: e.id,
                entity: e.entity,
                min,
                max,
            },
        ));
    });
    out
}

pub fn look_step(s: &SharedState, yaw: f32, pitch: f32, dt: f32, d: (f32, f32)) -> (f32, f32) {
    let Some(intent) = s.session.auto_mace else {
        return d;
    };
    turn::look_toward(s, yaw, pitch, dt, d, intent.aim, intent.turn)
}

fn aim_point(min: [f64; 3], max: [f64; 3], eye: [f64; 3], at: AimPoint) -> [f64; 3] {
    let mid = |i: usize| (min[i] + max[i]) * 0.5;
    let up = |f: f64| min[1] + (max[1] - min[1]) * f;
    match at {
        AimPoint::Center => [mid(0), mid(1), mid(2)],
        AimPoint::Head => [mid(0), (max[1] - 0.1).max(mid(1)), mid(2)],
        AimPoint::Body => [mid(0), up(0.6), mid(2)],
        AimPoint::Nearest => {
            let near = [
                eye[0].clamp(min[0], max[0]),
                eye[1].clamp(min[1], max[1]),
                eye[2].clamp(min[2], max[2]),
            ];
            let core = [mid(0), up(0.4), mid(2)];
            [
                near[0] + (core[0] - near[0]) * 0.1,
                near[1] + (core[1] - near[1]) * 0.1,
                near[2] + (core[2] - near[2]) * 0.1,
            ]
        }
    }
}

fn box_distance(p: [f64; 3], min: [f64; 3], max: [f64; 3]) -> f64 {
    let d = |i: usize| (min[i] - p[i]).max(0.0).max(p[i] - max[i]);
    (d(0) * d(0) + d(1) * d(1) + d(2) * d(2)).sqrt()
}

fn ray_hits(origin: [f64; 3], dir: [f64; 3], len: f64, min: [f64; 3], max: [f64; 3]) -> bool {
    let (mut near, mut far) = (0.0_f64, len);
    for i in 0..3 {
        if dir[i].abs() < 1e-12 {
            if origin[i] < min[i] || origin[i] > max[i] {
                return false;
            }
            continue;
        }
        let (a, b) = ((min[i] - origin[i]) / dir[i], (max[i] - origin[i]) / dir[i]);
        near = near.max(a.min(b));
        far = far.min(a.max(b));
        if near > far {
            return false;
        }
    }
    true
}

fn view_vector(l: Look) -> [f64; 3] {
    let (y, p) = ((l.yaw as f64).to_radians(), (l.pitch as f64).to_radians());
    [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()]
}

fn look_at(eye: [f64; 3], to: [f64; 3]) -> Look {
    let (dx, dy, dz) = (to[0] - eye[0], to[1] - eye[1], to[2] - eye[2]);
    let horizontal = (dx * dx + dz * dz).sqrt();
    Look {
        yaw: (-dx).atan2(dz).to_degrees() as f32,
        pitch: (-dy).atan2(horizontal).to_degrees() as f32,
    }
}

const DIRS: [u8; 8] = [
    MOVE_FORWARD,
    MOVE_FORWARD | MOVE_RIGHT,
    MOVE_RIGHT,
    MOVE_BACK | MOVE_RIGHT,
    MOVE_BACK,
    MOVE_BACK | MOVE_LEFT,
    MOVE_LEFT,
    MOVE_FORWARD | MOVE_LEFT,
];

fn move_fix(flags: u8, camera_yaw: f32, sent_yaw: f32) -> u8 {
    const KEYS: u8 = MOVE_FORWARD | MOVE_BACK | MOVE_LEFT | MOVE_RIGHT;
    let Some(i) = DIRS.iter().position(|&d| d == flags & KEYS) else {
        return flags;
    };
    let heading = wrap_degrees(i as f32 * 45.0 + camera_yaw - sent_yaw);
    let j = ((heading / 45.0).round() as i32).rem_euclid(8) as usize;
    flags & !KEYS | DIRS[j]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looking_at_a_point_faces_it() {
        for (yaw, pitch) in [(0.0, 0.0), (90.0, 30.0), (-135.0, -45.0), (179.0, 80.0)] {
            let v = view_vector(Look { yaw, pitch });
            let eye = [1.0, 64.0, -2.0];
            let to = [
                eye[0] + v[0] * 5.0,
                eye[1] + v[1] * 5.0,
                eye[2] + v[2] * 5.0,
            ];
            let got = look_at(eye, to);
            assert!(wrap_degrees(got.yaw - yaw).abs() < 1e-3, "{yaw} -> {got:?}");
            assert!((got.pitch - pitch).abs() < 1e-3, "{pitch} -> {got:?}");
        }
    }

    #[test]
    fn the_view_vector_matches_vanilla() {
        let close = |a: [f64; 3], b: [f64; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9);
        assert!(close(
            view_vector(Look {
                yaw: 0.0,
                pitch: 0.0
            }),
            [0.0, 0.0, 1.0]
        ));
        assert!(close(
            view_vector(Look {
                yaw: 90.0,
                pitch: 0.0
            }),
            [-1.0, 0.0, 0.0]
        ));
        assert!(close(
            view_vector(Look {
                yaw: 0.0,
                pitch: 90.0
            }),
            [0.0, -1.0, 0.0]
        ));
    }

    const MIN: [f64; 3] = [-0.3, 64.0, -0.3];
    const MAX: [f64; 3] = [0.3, 65.8, 0.3];

    #[test]
    fn a_ray_hits_a_box_in_front_and_in_reach_only() {
        let eye = [0.0, 65.5, -2.0];
        let ahead = [0.0, 0.0, 1.0];
        assert!(ray_hits(eye, ahead, 3.0, MIN, MAX));
        assert!(!ray_hits(eye, ahead, 1.5, MIN, MAX), "short of the face");
        assert!(!ray_hits(eye, [0.0, 0.0, -1.0], 3.0, MIN, MAX), "behind");
        assert!(!ray_hits(eye, [1.0, 0.0, 0.0], 3.0, MIN, MAX), "beside");
        assert!(ray_hits([0.1, 67.0, 0.1], [0.0, -1.0, 0.0], 3.0, MIN, MAX));
        assert!(ray_hits([0.0, 65.0, 0.0], ahead, 0.1, MIN, MAX));
    }

    #[test]
    fn distance_is_to_the_box_not_its_centre() {
        assert_eq!(box_distance([0.0, 65.0, 0.0], MIN, MAX), 0.0);
        assert!((box_distance([0.0, 65.0, -2.3], MIN, MAX) - 2.0).abs() < 1e-9);
        assert!((box_distance([0.0, 68.8, 0.0], MIN, MAX) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn every_aim_point_is_inside_the_box() {
        let small = ([0.0, 64.0, 0.0], [0.4, 64.7, 0.4]);
        for (min, max) in [(MIN, MAX), small] {
            for eye in [[0.0, 70.0, 0.0], [3.0, 65.0, -1.0], [0.1, 64.2, 0.1]] {
                for at in [
                    AimPoint::Body,
                    AimPoint::Head,
                    AimPoint::Center,
                    AimPoint::Nearest,
                ] {
                    let p = aim_point(min, max, eye, at);
                    assert!(
                        (0..3).all(|i| p[i] >= min[i] && p[i] <= max[i]),
                        "{at:?} from {eye:?} is {p:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn move_fix_keeps_the_heading() {
        assert_eq!(move_fix(MOVE_FORWARD, 0.0, 90.0), MOVE_LEFT);
        assert_eq!(move_fix(MOVE_FORWARD, 0.0, 0.0), MOVE_FORWARD);
        assert_eq!(move_fix(MOVE_FORWARD, 0.0, 180.0), MOVE_BACK);
        assert_eq!(
            move_fix(MOVE_RIGHT, 30.0, 30.0 + 45.0),
            MOVE_FORWARD | MOVE_RIGHT
        );
        let jump = crate::session::MOVE_JUMP;
        assert_eq!(move_fix(MOVE_FORWARD | jump, 0.0, 90.0), MOVE_LEFT | jump);
        assert_eq!(move_fix(jump, 0.0, 90.0), jump);
        assert_eq!(
            move_fix(MOVE_FORWARD | MOVE_BACK, 0.0, 90.0),
            MOVE_FORWARD | MOVE_BACK
        );
    }

    #[test]
    fn the_mace_slot_is_where_the_setting_looks() {
        let mace = SlotStack {
            item: "mace",
            count: 1,
            ..Default::default()
        };
        let sword = SlotStack {
            item: "diamond_sword",
            count: 1,
            ..Default::default()
        };
        let mut bar = vec![SlotStack::default(); 9];
        bar[0] = sword.clone();
        bar[4] = mace.clone();
        bar[7] = mace;
        assert_eq!(
            mace_slot(&bar, 0, MaceSlot::Auto),
            Some(4),
            "first in the bar"
        );
        assert_eq!(mace_slot(&bar, 7, MaceSlot::Auto), Some(7), "the held one");
        assert_eq!(mace_slot(&bar, 0, MaceSlot::Eight), Some(7));
        assert_eq!(
            mace_slot(&bar, 0, MaceSlot::One),
            None,
            "a sword is no mace"
        );
        bar[4] = sword;
        bar[7] = SlotStack::default();
        assert_eq!(mace_slot(&bar, 0, MaceSlot::Auto), None);
    }
}
