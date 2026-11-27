use std::sync::Mutex;

use azalea::Client;
use azalea::attack::AttackQueued;
use azalea::ecs::entity::Entity as AzEntity;
use azalea::entity::Physics;
use azalea::entity::metadata::{FallFlying, Sprinting};

use super::auto_totem::{Landing, landing};
use super::registry::{Id, auto_mace as setting, options};
use super::sent::{Look, box_distance, look_at, ray_hits, sent_ray, view_vector};
use super::turn::{Shape, SilentTurn};
use super::{Edge, Phase, roll, store, turn};
use crate::session::{CameraAim, Gamemode, MOVE_FORWARD, MOVE_SPRINT, SharedMutex, SlotStack};
use crate::util::fall::airborne_path;

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

const STUCK_TICKS: i64 = 5;

const HORIZON: usize = 60;

const MARGIN: usize = 1;

const TARGET_SLACK: f64 = 0.5;

const TARGET_SPEED_CAP: f64 = 1.0;

const SPRINT_TICKS: usize = 2;

const STOP_SPRINT_LEAD: usize = SPRINT_TICKS + 4;

static EDGE: Edge = Edge::new(Id::AutoMace);

static RUN: Mutex<Run> = Mutex::new(Run::IDLE);

struct Run {
    target: Option<Target>,
    silent: SilentTurn,
    turn: f32,
    swung: Option<Swung>,
    stop_sprint: bool,
    reason: Option<Reason>,
}

impl Run {
    const IDLE: Run = Run {
        target: None,
        silent: SilentTurn::IDLE,
        turn: 0.0,
        swung: None,
        stop_sprint: false,
        reason: None,
    };
}

struct Target {
    id: i32,
    aim: [f64; 3],
    centre: [f64; 3],
    vel: [f64; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reason {
    NoFall,
    Hands,
    NoTarget,
    Approach,
    Strength,
    RayMiss,
    Swung,
}

fn note(run: &mut Run, why: Reason) {
    if run.reason != Some(why) {
        run.reason = Some(why);
        crate::log_debug!("mace", "{why:?}");
    }
}

#[derive(Clone, Copy)]
struct Swung {
    mace: u8,
    restore: Option<u8>,
    queued_at: i64,
}

pub fn enabled() -> bool {
    store().enabled(Id::AutoMace)
}

pub fn drives_camera() -> bool {
    enabled() && !store().flag(setting::SILENT)
}

pub fn steer(bot: &Client, yaw: f32, pitch: f32, flags: u8) -> (f32, f32, u8) {
    let mut run = RUN.lock().unwrap();
    let (yaw, pitch, flags) = steer_look(bot, &mut run, yaw, pitch, flags);
    let flags = if run.stop_sprint {
        flags & !(MOVE_FORWARD | MOVE_SPRINT)
    } else {
        flags
    };
    (yaw, pitch, flags)
}

fn steer_look(bot: &Client, run: &mut Run, yaw: f32, pitch: f32, flags: u8) -> (f32, f32, u8) {
    let s = store();
    if !s.flag(setting::SILENT) {
        run.silent.drop_out();
        return (yaw, pitch, flags);
    }
    let want = match &run.target {
        Some(t) => {
            let aim = [t.aim[0] + t.vel[0], t.aim[1], t.aim[2] + t.vel[2]];
            let Ok(eye) = bot.eye_position() else {
                run.silent.drop_out();
                return (yaw, pitch, flags);
            };
            let v = bot
                .component::<Physics>()
                .map_or(Default::default(), |p| p.velocity);
            Some(look_at([eye.x + v.x, eye.y + v.y, eye.z + v.z], aim))
        }
        None => None,
    };
    let shape = if want.is_some() {
        Shape::Capped
    } else {
        Shape::Eased
    };
    let rate = run.turn;
    match run.silent.next(bot, Look { yaw, pitch }, want, rate, shape) {
        Some(next) => (
            next.yaw,
            next.pitch,
            turn::walk_keys(flags, yaw, next.yaw, s.flag(setting::MOVE_FIX)),
        ),
        None => (yaw, pitch, flags),
    }
}

pub fn tick(bot: &Client, shared: &SharedMutex) {
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

    let now = crate::client::tracking::client_ticks();
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
    let ready = match ready(bot, shared) {
        Ok(r) => r,
        Err(why) => {
            run.target = None;
            run.stop_sprint = false;
            shared.lock().unwrap().session.auto_mace = None;
            note(&mut run, why);
            return;
        }
    };

    let range = s.num(setting::RANGE) as f64;
    let aim = s.choice(setting::AIM);
    let Some(found) = pick(
        bot,
        &ready,
        range,
        s.num(setting::FOV),
        s.flag(setting::WALLS),
        aim,
    ) else {
        run.target = None;
        run.stop_sprint = false;
        shared.lock().unwrap().session.auto_mace = None;
        note(&mut run, Reason::NoTarget);
        return;
    };

    let centre = centre(found.min, found.max);
    let before = run
        .target
        .as_ref()
        .filter(|t| t.id == found.id)
        .map(|t| t.centre);
    let vel = match before {
        Some(c) => {
            let (dx, dz) = (centre[0] - c[0], centre[2] - c[2]);
            if dx.hypot(dz) <= TARGET_SPEED_CAP {
                [dx, 0.0, dz]
            } else {
                [0.0; 3]
            }
        }
        None => {
            let (lo, hi) = s.range(setting::TURN);
            run.turn = roll(lo, hi);
            [0.0; 3]
        }
    };
    let point = aim_point(found.min, found.max, ready.eye, aim);
    run.target = Some(Target {
        id: found.id,
        aim: point,
        centre,
        vel,
    });
    shared.lock().unwrap().session.auto_mace = (!silent).then_some(CameraAim {
        aim: point,
        turn: run.turn,
    });

    let min_fall = s.num(setting::MIN_FALL) as f64;
    let last = last_in_reach(
        &ready.path,
        ready.eye_height,
        ready.fall,
        min_fall,
        found.min,
        found.max,
        vel,
        range,
    );
    let charge = Charge {
        strength: ready.strength,
        delay: ready.delay,
        threshold: s.num(setting::COOLDOWN) / 100.0,
        sprinting: ready.sprinting,
        stop_sprint: s.flag(setting::STOP_SPRINT),
    };
    run.stop_sprint = charge.stop_sprint
        && (charge.sprinting || run.stop_sprint)
        && last.is_some_and(|l| l <= STOP_SPRINT_LEAD && charge.crit_within(l));

    if ready.fall < min_fall || box_distance(ready.eye, found.min, found.max) > range {
        note(&mut run, Reason::Approach);
        return;
    }
    let last = last.unwrap_or(0);
    if !swing_now(&charge, last, s.flag(setting::WAIT_CHARGE)) {
        note(&mut run, Reason::Strength);
        return;
    }
    let Some((origin, dir)) = sent_ray(bot) else {
        return;
    };
    if !ray_hits(origin, dir, range, found.min, found.max) {
        note(&mut run, Reason::RayMiss);
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
    run.stop_sprint = false;
    run.reason = Some(Reason::Swung);
    crate::log_debug!(
        "mace",
        "Swung: strength {:.2}, fall {:.1}, last tick in reach {last}, crit {}",
        ready.strength,
        ready.fall,
        charge.crit_ready(0)
    );
}

struct Ready {
    eye: [f64; 3],
    eye_height: f64,
    path: Vec<[f64; 3]>,
    fall: f64,
    camera: Look,
    strength: f32,
    delay: f32,
    sprinting: bool,
    mace: u8,
}

fn ready(bot: &Client, shared: &SharedMutex) -> Result<Ready, Reason> {
    let s = store();
    let min_fall = s.num(setting::MIN_FALL) as f64;
    let (fall, vel) = {
        let physics = bot.component::<Physics>().map_err(|_| Reason::NoFall)?;
        if physics.on_ground() || physics.was_touching_water {
            return Err(Reason::NoFall);
        }
        let v = physics.velocity;
        (physics.fall_distance, [v.x, v.y, v.z])
    };
    if bot.component::<FallFlying>().is_ok_and(|f| **f) {
        return Err(Reason::NoFall);
    }

    let (camera, strength, delay, mace) = {
        let st = shared.lock().unwrap();
        if !st.in_world
            || st.screen_open
            || st.session.dead
            || st.session.gamemode == Gamemode::Spectator
        {
            return Err(Reason::Hands);
        }
        let mace = mace_slot(
            &st.session.hotbar,
            st.session.hotbar_selected,
            s.choice(setting::SLOT),
        )
        .ok_or(Reason::Hands)?;
        let camera = Look::from_camera(st.camera_yaw, st.camera_pitch);
        (
            camera,
            st.session.attack_strength,
            st.session.attack_delay,
            mace,
        )
    };
    let sprinting = bot.component::<Sprinting>().is_ok_and(|f| **f);

    let feet = bot.position().map_err(|_| Reason::NoFall)?;
    let eye = bot.eye_position().map_err(|_| Reason::NoFall)?;
    let floor = match landing(bot, feet) {
        Landing::Ground(drop) | Landing::Fluid(drop) => Some(feet.y - drop),
        Landing::None => None,
    };
    if floor.is_some_and(|f| fall + (feet.y - f) < min_fall) {
        return Err(Reason::NoFall);
    }
    let path = airborne_path([feet.x, feet.y, feet.z], vel, floor, HORIZON);
    if path.is_empty() {
        return Err(Reason::NoFall);
    }

    Ok(Ready {
        eye: [eye.x, eye.y, eye.z],
        eye_height: eye.y - feet.y,
        path,
        fall,
        camera,
        strength,
        delay,
        sprinting,
        mace,
    })
}

struct Charge {
    strength: f32,
    delay: f32,
    threshold: f32,
    sprinting: bool,
    stop_sprint: bool,
}

impl Charge {
    fn charged(&self, k: usize) -> bool {
        self.strength + k as f32 / self.delay.max(1.0) >= self.threshold
    }

    fn crit_ready(&self, k: usize) -> bool {
        self.charged(k) && (!self.sprinting || (self.stop_sprint && k >= SPRINT_TICKS))
    }

    fn crit_within(&self, last: usize) -> bool {
        (0..=last).any(|k| self.crit_ready(k))
    }
}

fn swing_now(c: &Charge, last: usize, wait: bool) -> bool {
    if wait {
        return c.charged(0);
    }
    c.crit_ready(0) || last <= MARGIN
}

#[allow(clippy::too_many_arguments)]
fn last_in_reach(
    path: &[[f64; 3]],
    eye_height: f64,
    fall: f64,
    min_fall: f64,
    min: [f64; 3],
    max: [f64; 3],
    vel: [f64; 3],
    range: f64,
) -> Option<usize> {
    let mut fallen = fall;
    let mut prev = path.first()?[1];
    let mut last = None;
    for (k, p) in path.iter().enumerate() {
        fallen += (prev - p[1]).max(0.0);
        prev = p[1];
        let t = k as f64;
        let shift = |b: [f64; 3]| [b[0] + vel[0] * t, b[1], b[2] + vel[2] * t];
        let eye = [p[0], p[1] + eye_height, p[2]];
        if fallen >= min_fall && box_distance(eye, shift(min), shift(max)) <= range {
            last = Some(k);
        }
    }
    last
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

struct Found {
    id: i32,
    entity: AzEntity,
    min: [f64; 3],
    max: [f64; 3],
}

fn pick(
    bot: &Client,
    ready: &Ready,
    range: f64,
    fov: f32,
    walls: bool,
    aim: AimPoint,
) -> Option<Found> {
    let cone = (fov < 360.0).then(|| ((fov as f64) * 0.5).to_radians().cos());
    let eye = ready.eye;
    let mut found = candidates(bot, ready, view_vector(ready.camera), range, cone);
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().map(|(_, f)| f).find(|f| {
        walls || {
            let to = aim_point(f.min, f.max, eye, aim);
            super::aim_assist::visible(bot, eye, to)
        }
    })
}

fn candidates(
    bot: &Client,
    ready: &Ready,
    forward: [f64; 3],
    range: f64,
    cone: Option<f64>,
) -> Vec<(f64, Found)> {
    let eye = ready.eye;
    let reach = range + TARGET_SLACK;
    let travel = match (ready.path.first(), ready.path.last()) {
        (Some(a), Some(b)) => {
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
        }
        _ => 0.0,
    };
    let mut out = Vec::new();
    super::nearby::each(bot, |e| {
        if !e.health.is_some_and(|h| h > 0.0)
            || !super::entities::kind_enabled(e.kind)
            || e.protected()
        {
            return;
        }
        let b = &e.physics.bounding_box;
        let (min, max) = ([b.min.x, b.min.y, b.min.z], [b.max.x, b.max.y, b.max.z]);
        let dist = box_distance(eye, min, max);
        if dist > reach + travel
            || (dist > reach
                && last_in_reach(
                    &ready.path,
                    ready.eye_height,
                    0.0,
                    f64::NEG_INFINITY,
                    min,
                    max,
                    [0.0; 3],
                    reach,
                )
                .is_none())
        {
            return;
        }
        if let Some(limit) = cone {
            let c = centre(min, max);
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

fn centre(min: [f64; 3], max: [f64; 3]) -> [f64; 3] {
    [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ]
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

#[cfg(test)]
mod tests {
    use super::*;

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

    const EYE: f64 = 1.62;

    fn fall_from(drop: f64) -> Vec<[f64; 3]> {
        airborne_path([0.0, MIN[1] + drop, 0.0], [0.0; 3], Some(MIN[1]), HORIZON)
    }

    #[test]
    fn a_ten_block_fall_is_in_reach_for_three_ticks() {
        let path = fall_from(10.0);
        assert_eq!(path.len(), 18);
        assert_eq!(
            last_in_reach(&path, EYE, 0.0, 3.0, MIN, MAX, [0.0; 3], 3.0),
            Some(17)
        );
        let eye = |k: usize| [path[k][0], path[k][1] + EYE, path[k][2]];
        assert!(box_distance(eye(15), MIN, MAX) <= 3.0);
        assert!(
            box_distance(eye(14), MIN, MAX) > 3.0,
            "the tick before is out of reach"
        );
    }

    #[test]
    fn a_twenty_block_fall_is_in_reach_for_two_ticks() {
        let path = fall_from(20.0);
        assert_eq!(path.len(), 25);
        assert_eq!(
            last_in_reach(&path, EYE, 0.0, 3.0, MIN, MAX, [0.0; 3], 3.0),
            Some(24)
        );
        let eye = [path[22][0], path[22][1] + EYE, path[22][2]];
        assert!(box_distance(eye, MIN, MAX) > 3.0);
    }

    #[test]
    fn min_fall_is_counted_per_tick() {
        let path = fall_from(10.0);
        assert_eq!(
            last_in_reach(&path, EYE, 0.0, 9.0, MIN, MAX, [0.0; 3], 3.0),
            Some(17)
        );
        assert_eq!(
            last_in_reach(&path, EYE, 0.0, 10.0, MIN, MAX, [0.0; 3], 3.0),
            None
        );
        assert_eq!(
            last_in_reach(&path, EYE, 1.0, 10.0, MIN, MAX, [0.0; 3], 3.0),
            Some(17)
        );
    }

    #[test]
    fn a_target_that_moves_away_is_out_of_reach() {
        let path = fall_from(10.0);
        assert_eq!(
            last_in_reach(&path, EYE, 0.0, 3.0, MIN, MAX, [1.0, 0.0, 0.0], 3.0),
            None
        );
    }

    fn charge(strength: f32, sprinting: bool, stop_sprint: bool) -> Charge {
        Charge {
            strength,
            delay: 10.0,
            threshold: 0.95,
            sprinting,
            stop_sprint,
        }
    }

    #[test]
    fn short_of_the_charge_it_swings_at_the_last_tick() {
        assert!(swing_now(&charge(1.0, false, true), 10, false));
        let half = charge(0.5, false, true);
        assert!(!half.charged(4) && half.charged(5), "a tenth a tick");
        assert!(!swing_now(&half, 10, false), "a later tick is in reach");
        assert!(swing_now(&half, MARGIN, false));
        assert!(swing_now(&half, 0, false));
        assert!(!swing_now(&half, 0, true), "the hard gate");
        assert!(
            swing_now(&charge(1.0, true, false), 0, true),
            "the hard gate ignores sprint"
        );
    }

    #[test]
    fn a_sprint_delays_the_crit() {
        let sprinting = charge(1.0, true, true);
        assert!(!sprinting.crit_ready(0));
        assert!(sprinting.crit_ready(SPRINT_TICKS));
        assert!(
            !swing_now(&sprinting, 5, false),
            "waits for the sprint to end"
        );
        assert!(sprinting.crit_within(SPRINT_TICKS) && !sprinting.crit_within(SPRINT_TICKS - 1));
        let kept = charge(1.0, true, false);
        assert!(!kept.crit_within(10), "no crit while the sprint is kept");
        assert!(
            swing_now(&kept, MARGIN, false),
            "but the smash still goes in"
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
