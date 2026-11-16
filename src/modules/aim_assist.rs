use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, Ordering::Relaxed};

use azalea::Client;
use azalea::physics::clip::{BlockShapeType, ClipContext, FluidPickType, clip};
use azalea_core::position::Vec3 as AzVec3;
use azalea_registry::builtin::EntityKind;

use super::registry::{Id, aim_assist as setting};
use super::{Edge, MS_PER_TICK, Phase, roll, store, turn};
use crate::session::{SharedMutex, SharedState};
use crate::util::mth::wrap_degrees;
use crate::util::offset_f32 as offset;

const CORE_WIDTH: f32 = 0.5;

const CORE_LOW: f32 = 0.35;

const CORE_HIGH: f32 = 0.9;

const STICK: f32 = 0.5;

const STICK_MARGIN: f32 = 5.0;

const ON_CORE: f32 = 1e-3;

const PULL_WINDOW: f32 = 15.0;

const AWAY_HOLD: f32 = 0.25;

const MAX_DT: f32 = 0.05;

const FLAT_PITCH: f32 = 80.0;

static TARGET: AtomicI32 = AtomicI32::new(NO_TARGET);
const NO_TARGET: i32 = i32::MIN;

static ARMED: AtomicBool = AtomicBool::new(false);

static TICKS: AtomicI64 = AtomicI64::new(0);

static ARM_AT: AtomicI64 = AtomicI64::new(0);

static EDGE: Edge = Edge::new(Id::AimAssist);

thread_local! {
    static CARRY: Cell<(f32, f32)> = const { Cell::new((0.0, 0.0)) };
    static HOLD: Cell<f32> = const { Cell::new(0.0) };
}

pub fn enabled() -> bool {
    store().enabled(Id::AimAssist)
}

pub fn pulls() -> bool {
    enabled() && ARMED.load(Relaxed) && store().num(setting::PULL) > 0.0
}

fn publish(s: &mut SharedState, target: Option<i32>) {
    s.session.aim_target = target;
    ARMED.store(target.is_some(), Relaxed);
}

pub fn tick(bot: &Client, shared: &SharedMutex) {
    let s = store();
    match EDGE.poll(s) {
        Phase::Off => return,
        Phase::Stopped => {
            publish(&mut shared.lock().unwrap(), None);
            TARGET.store(NO_TARGET, Relaxed);
            return;
        }
        Phase::Started => TARGET.store(NO_TARGET, Relaxed),
        Phase::Running => {}
    }
    let now = TICKS.fetch_add(1, Relaxed) + 1;
    let range = s.num(setting::RANGE);
    let fov = s.num(setting::FOV);

    let picked = {
        let mut state = shared.lock().unwrap();
        let eye = eye(&state, 1.0);
        match pick(&state, eye, range, fov, TARGET.load(Relaxed)) {
            Some(found) => (eye, found),
            None => {
                publish(&mut state, None);
                TARGET.store(NO_TARGET, Relaxed);
                return;
            }
        }
    };
    let (eye, (id, core)) = picked;

    if !s.flag(setting::IGNORE_WALLS) && !visible(bot, eye, core.centre()) {
        publish(&mut shared.lock().unwrap(), None);
        return;
    }

    if TARGET.swap(id, Relaxed) != id {
        let (lo, hi) = s.range(setting::REACTION);
        ARM_AT.store(now + (roll(lo, hi) / MS_PER_TICK).round() as i64, Relaxed);
    }
    let armed = now >= ARM_AT.load(Relaxed);
    publish(&mut shared.lock().unwrap(), armed.then_some(id));
}

fn pick(s: &SharedState, eye: [f64; 3], range: f32, fov: f32, held: i32) -> Option<(i32, Core)> {
    let forward = forward(s.camera_yaw, s.camera_pitch);
    let cos_limit = (fov.clamp(0.0, 360.0) * 0.5).to_radians().cos();
    let range_sq = range * range;
    let crosshair = s.session.crosshair_entity;
    let mut best: Option<(f32, i32, Core)> = None;
    let mut kept: Option<(f32, i32, Core)> = None;
    let mut under: Option<(i32, Core)> = None;

    let mut consider = |id: i32, core: Core| {
        let to = offset(eye, core.centre());
        let dist_sq = to[0] * to[0] + to[1] * to[1] + to[2] * to[2];
        if dist_sq > range_sq || dist_sq < f32::EPSILON {
            return;
        }
        let cos = (forward[0] * to[0] + forward[1] * to[1] + forward[2] * to[2]) / dist_sq.sqrt();
        if cos < cos_limit {
            return;
        }
        let angle = cos.clamp(-1.0, 1.0).acos().to_degrees();
        if crosshair == Some(id) {
            under = Some((id, core));
        }
        if id == held {
            kept = Some((angle, id, core));
        }
        if best.is_none_or(|(b, _, _)| angle < b) {
            best = Some((angle, id, core));
        }
    };

    if super::entities::kind_enabled(EntityKind::Player) {
        for p in &s.session.other_players {
            if p.health.is_some_and(|h| h <= 0.0) || super::friends::protected(&p.username) {
                continue;
            }
            consider(p.id, player_core(p.anim.position(1.0), p.discrete));
        }
    }
    for e in s.session.entities.iter() {
        if !e.is_living() || e.is_invisible() || !super::entities::kind_enabled(e.kind) {
            continue;
        }
        consider(e.id, entity_core(e));
    }

    if under.is_some() {
        return under;
    }
    match (kept, best) {
        (Some((k, id, core)), Some((b, _, _))) if !(b < k * STICK && k - b > STICK_MARGIN) => {
            Some((id, core))
        }
        (_, best) => best.map(|(_, id, core)| (id, core)),
    }
}

pub(super) fn visible(bot: &Client, eye: [f64; 3], point: [f64; 3]) -> bool {
    let Ok(world) = bot.world() else {
        return false;
    };
    let vec = |p: [f64; 3]| AzVec3 {
        x: p[0],
        y: p[1],
        z: p[2],
    };
    let hit = clip(
        &world.read().chunks,
        ClipContext {
            from: vec(eye),
            to: vec(point),
            block_shape_type: BlockShapeType::Collider,
            fluid_pick_type: FluidPickType::None,
            held_item: azalea_registry::builtin::ItemKind::Air,
            entity: None,
        },
    );
    hit.miss
}

pub fn nudge(s: &SharedState, yaw: f32, pitch: f32, dt: f32, hand: (f32, f32)) -> (f32, f32) {
    let store = store();
    let Some(id) = s.session.aim_target else {
        ARMED.store(false, Relaxed);
        CARRY.set((0.0, 0.0));
        return hand;
    };
    if store.flag(setting::CLICK_ONLY) && !s.session.attack_held {
        CARRY.set((0.0, 0.0));
        return hand;
    }
    let partial = crate::renderer::systems::partial_ticks(s);
    let Some(core) = target_core(s, id, partial) else {
        CARRY.set((0.0, 0.0));
        return hand;
    };
    let eye = eye(s, partial);
    let flat = Flat::at(pitch);
    let dt = dt.min(MAX_DT);
    let pull = Pull {
        gain: store.num(setting::STRENGTH) / 100.0,
        friction: store.num(setting::FRICTION) / 100.0,
        window: store.num(setting::FOV) * 0.5,
    };

    let (own, away) = if hand == (0.0, 0.0) {
        ((0.0, 0.0), false)
    } else {
        let h = flat.weigh(hand);
        let edge = flat.weigh(core_error(eye, yaw, pitch, &core));
        let centre = flat.weigh(centre_error(eye, yaw, pitch, &core));
        let (assisted, away) = assist(h, edge, centre, pull);
        (flat.unweigh((assisted.0 - h.0, assisted.1 - h.1)), away)
    };
    let held = hold(away, dt);

    let rate = store.num(setting::PULL);
    let pulled = if rate > 0.0 && !held && !s.screen_open {
        let (ty, tp) = (yaw - hand.0 - own.0, pitch - hand.1 - own.1);
        flat.unweigh(pull_step(
            flat.weigh(core_error(eye, ty, tp, &core)),
            rate,
            dt,
        ))
    } else {
        (0.0, 0.0)
    };

    let module = on_grid((own.0 + pulled.0, own.1 + pulled.1));
    (hand.0 + module.0, hand.1 + module.1)
}

#[derive(Clone, Copy)]
struct Pull {
    gain: f32,
    friction: f32,
    window: f32,
}

fn assist(d: (f32, f32), edge: (f32, f32), centre: (f32, f32), p: Pull) -> ((f32, f32), bool) {
    let own = (-d.0, -d.1);
    if own == (0.0, 0.0) {
        return (d, false);
    }
    let mag = edge.0.hypot(edge.1);
    if mag > 0.0 {
        let dir = (edge.0 / mag, edge.1 / mag);
        let along = own.0 * dir.0 + own.1 * dir.1;
        if along > 0.0 {
            let add = (along * p.gain).min((mag - along).max(0.0));
            return ((d.0 - dir.0 * add, d.1 - dir.1 * add), false);
        }
    } else if own.0 * centre.0 + own.1 * centre.1 >= 0.0 {
        return (d, false);
    }
    if p.friction <= 0.0 || p.window <= 0.0 {
        return (d, true);
    }
    let keep = 1.0 - p.friction.min(1.0) * (1.0 - (mag / p.window).min(1.0));
    ((d.0 * keep, d.1 * keep), true)
}

fn pull_step(err: (f32, f32), rate: f32, dt: f32) -> (f32, f32) {
    let mag = err.0.hypot(err.1);
    if mag < turn::DEADZONE || mag >= PULL_WINDOW {
        return (0.0, 0.0);
    }
    let cap = rate * (1.0 - mag / PULL_WINDOW) * dt;
    let len = (mag * turn::ease(dt)).min(cap);
    (-err.0 / mag * len, -err.1 / mag * len)
}

fn hold(away: bool, dt: f32) -> bool {
    let left = if away {
        AWAY_HOLD
    } else {
        (HOLD.get() - dt).max(0.0)
    };
    HOLD.set(left);
    left > 0.0
}

fn on_grid(own: (f32, f32)) -> (f32, f32) {
    let (cx, cy) = CARRY.get();
    let want = (own.0 + cx, own.1 + cy);
    let got = (turn::quantise(want.0), turn::quantise(want.1));
    CARRY.set((want.0 - got.0, want.1 - got.1));
    got
}

#[derive(Clone, Copy)]
struct Flat {
    yaw_scale: f32,
}

impl Flat {
    fn at(pitch: f32) -> Flat {
        Flat {
            yaw_scale: if pitch.abs() > FLAT_PITCH {
                0.0
            } else {
                pitch.to_radians().cos()
            },
        }
    }

    fn weigh(self, v: (f32, f32)) -> (f32, f32) {
        (v.0 * self.yaw_scale, v.1)
    }

    fn unweigh(self, v: (f32, f32)) -> (f32, f32) {
        let yaw = if self.yaw_scale > 0.0 {
            v.0 / self.yaw_scale
        } else {
            0.0
        };
        (yaw, v.1)
    }
}

fn target_core(s: &SharedState, id: i32, partial: f32) -> Option<Core> {
    if let Some(p) = s.session.other_players.iter().find(|p| p.id == id) {
        return Some(player_core(p.anim.position(partial), p.discrete));
    }
    let e = s.session.entities.iter().find(|e| e.id == id)?;
    Some(Core::around(
        e.position(partial),
        e.bounding_box_width(),
        e.bounding_box_height(),
        e.eye_height(),
    ))
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Core {
    min: [f64; 3],
    max: [f64; 3],
}

impl Core {
    fn around(pos: [f64; 3], width: f32, height: f32, eye: f32) -> Core {
        let half = f64::from(width * 0.5 * CORE_WIDTH);
        let top = eye.min(height * CORE_HIGH);
        let low = pos[1] + f64::from((height * CORE_LOW).min(top * 0.5));
        let high = (pos[1] + f64::from(top)).max(low);
        Core {
            min: [pos[0] - half, low, pos[2] - half],
            max: [pos[0] + half, high, pos[2] + half],
        }
    }

    fn centre(&self) -> [f64; 3] {
        std::array::from_fn(|i| (self.min[i] + self.max[i]) * 0.5)
    }

    fn clamp(&self, p: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|i| p[i].clamp(self.min[i], self.max[i]))
    }
}

fn player_core(pos: [f64; 3], discrete: bool) -> Core {
    let pose = if discrete {
        azalea::entity::Pose::Crouching
    } else {
        azalea::entity::Pose::Standing
    };
    let d = crate::session::player_dimensions(pose, 1.0);
    Core::around(pos, d.width, d.height, d.eye_height)
}

fn entity_core(e: &crate::entities::feed::EntityAnim) -> Core {
    Core::around(
        e.position(1.0),
        e.bounding_box_width(),
        e.bounding_box_height(),
        e.eye_height(),
    )
}

fn core_error(eye: [f64; 3], yaw: f32, pitch: f32, core: &Core) -> (f32, f32) {
    let f = forward(yaw, pitch);
    let to = offset(eye, core.centre());
    let depth = f64::from((f[0] * to[0] + f[1] * to[1] + f[2] * to[2]).max(0.1));
    let ray: [f64; 3] = std::array::from_fn(|i| eye[i] + f64::from(f[i]) * depth);
    let err = angles_to(eye, core.clamp(ray), yaw, pitch);
    if err.0.abs() < ON_CORE && err.1.abs() < ON_CORE {
        (0.0, 0.0)
    } else {
        err
    }
}

fn centre_error(eye: [f64; 3], yaw: f32, pitch: f32, core: &Core) -> (f32, f32) {
    angles_to(eye, core.centre(), yaw, pitch)
}

fn angles_to(eye: [f64; 3], to: [f64; 3], yaw: f32, pitch: f32) -> (f32, f32) {
    let (want_yaw, want_pitch) = look_at(eye, to);
    (wrap_degrees(want_yaw - yaw), want_pitch - pitch)
}

pub(super) fn eye(s: &SharedState, partial: f32) -> [f64; 3] {
    let (prev, cur) = (s.session.player_pos_prev, s.session.player_pos);
    let at = |i: usize| prev[i] + (cur[i] - prev[i]) * f64::from(partial);
    [
        at(0),
        at(1) + f64::from(crate::renderer::systems::session_eye_height(&s.session)),
        at(2),
    ]
}

fn forward(yaw: f32, pitch: f32) -> [f32; 3] {
    let (y, p) = (yaw.to_radians(), pitch.to_radians());
    [-y.sin() * p.cos(), p.sin(), -y.cos() * p.cos()]
}

pub(super) fn look_at(eye: [f64; 3], to: [f64; 3]) -> (f32, f32) {
    let [dx, dy, dz] = offset(eye, to);
    let horizontal = (dx * dx + dz * dz).sqrt();
    (
        (-dx).atan2(-dz).to_degrees(),
        dy.atan2(horizontal).to_degrees(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAIN: Pull = Pull {
        gain: 1.0,
        friction: 0.0,
        window: 30.0,
    };

    #[test]
    fn the_assist_only_ever_scales_the_hand() {
        let e = (40.0, 0.0);
        assert_eq!(assist((0.0, 0.0), e, e, GAIN), ((0.0, 0.0), false));
        assert_eq!(
            assist((-3.0, 0.0), e, e, Pull { gain: 0.5, ..GAIN }),
            ((-4.5, 0.0), false)
        );
        assert_eq!(assist((3.0, 0.0), e, e, GAIN), ((3.0, 0.0), true));
    }

    #[test]
    fn the_gain_never_carries_past_the_edge() {
        let e = (4.0, 0.0);
        let strong = Pull { gain: 1.75, ..GAIN };
        assert_eq!(assist((-3.0, 0.0), e, e, strong), ((-4.0, 0.0), false));
        assert_eq!(assist((-30.0, 0.0), e, e, strong), ((-30.0, 0.0), false));
    }

    #[test]
    fn the_gain_follows_the_error_not_the_hand() {
        let e = (10.0, 10.0);
        let ((dx, dy), _) = assist((-2.0, 0.0), e, e, GAIN);
        let along = 2.0 * std::f32::consts::FRAC_1_SQRT_2;
        let step = along * std::f32::consts::FRAC_1_SQRT_2;
        assert!((dx - (-2.0 - step)).abs() < 1e-5, "{dx}");
        assert!((dy - -step).abs() < 1e-5, "{dy}");
    }

    #[test]
    fn friction_damps_the_way_out_and_nothing_else() {
        let p = Pull {
            gain: 0.0,
            friction: 0.5,
            window: 30.0,
        };
        let on = (0.0, 0.0);
        assert_eq!(assist((-4.0, 0.0), on, (-1.0, 0.0), p), ((-2.0, 0.0), true));
        assert_eq!(assist((4.0, 0.0), on, (-1.0, 0.0), p), ((4.0, 0.0), false));
        let off = |e: f32| (e, 0.0);
        assert_eq!(
            assist((-4.0, 0.0), off(-15.0), off(-16.0), p),
            ((-3.0, 0.0), true)
        );
        assert_eq!(
            assist((-4.0, 0.0), off(-30.0), off(-31.0), p),
            ((-4.0, 0.0), true)
        );
        assert_eq!(
            assist((-4.0, 0.0), off(-90.0), off(-91.0), p),
            ((-4.0, 0.0), true)
        );
        let hard = Pull { friction: 0.9, ..p };
        assert!(assist((-4.0, 0.0), off(-1.0), off(-2.0), hard).0.0 < 0.0);
    }

    #[test]
    fn the_pull_is_bounded() {
        let (dx, dy) = pull_step((10.0, 0.0), 120.0, 1.0 / 60.0);
        let cap = 120.0 * (1.0 - 10.0 / PULL_WINDOW) / 60.0;
        assert!((dx + cap).abs() < 1e-5 && dy == 0.0, "{dx}");
        assert_eq!(pull_step((PULL_WINDOW, 0.0), 120.0, 0.05), (0.0, 0.0));
        assert_eq!(pull_step((0.1, 0.0), 120.0, 0.05), (0.0, 0.0));
        let (dx, dy) = pull_step((3.0, 3.0), 1000.0, 0.05);
        let len = dx.hypot(dy);
        assert!(len <= 3.0f32.hypot(3.0), "{len}");
    }

    #[test]
    fn the_grid_carries_what_it_drops() {
        CARRY.set((0.0, 0.0));
        let grid = crate::renderer::input::LOOK_SCALE;
        let mut sum = 0.0;
        for _ in 0..10 {
            let (x, _) = on_grid((grid * 0.3, 0.0));
            assert!((x / grid - (x / grid).round()).abs() < 1e-4);
            sum += x;
        }
        assert!((sum - grid * 3.0).abs() <= grid * 0.5 + 1e-5, "{sum}");
    }

    #[test]
    fn the_error_is_to_the_core_not_a_point() {
        let eye = [0.0, 1.62, 0.0];
        let core = player_core([0.0, 0.0, -3.0], false);
        let (yaw, pitch) = look_at(eye, core.centre());
        assert_eq!(core_error(eye, yaw, pitch, &core), (0.0, 0.0));
        assert_eq!(core_error(eye, yaw + 1.0, pitch, &core), (0.0, 0.0));
        let (ey, ep) = core_error(eye, yaw + 20.0, pitch, &core);
        assert!(ey < 0.0 && ey > -20.0, "{ey}");
        assert!(ep.abs() < 1.0, "{ep}");
    }

    #[test]
    fn a_low_eyed_entity_has_a_core() {
        let core = Core::around([0.0, 0.0, 0.0], 0.4, 0.3, 0.13);
        assert!(core.max[1] > core.min[1], "{core:?}");
    }

    #[test]
    fn steep_pitch_drops_yaw() {
        let flat = Flat::at(85.0);
        assert_eq!(flat.weigh((30.0, 2.0)), (0.0, 2.0));
        assert_eq!(flat.unweigh((5.0, 2.0)), (0.0, 2.0));
        let level = Flat::at(0.0);
        assert_eq!(level.unweigh(level.weigh((3.0, 1.0))), (3.0, 1.0));
    }

    #[test]
    fn looking_at_a_point_faces_it() {
        for (yaw, pitch) in [(0.0, 0.0), (90.0, 30.0), (-135.0, -45.0), (179.0, 80.0)] {
            let f = forward(yaw, pitch);
            let eye = [1.0, 64.0, -2.0];
            let to: [f64; 3] = std::array::from_fn(|i| eye[i] + f64::from(f[i]) * 5.0);
            let (got_yaw, got_pitch) = look_at(eye, to);
            assert!(
                wrap_degrees(got_yaw - yaw).abs() < 1e-3,
                "yaw {yaw} came back as {got_yaw}"
            );
            assert!(
                (got_pitch - pitch).abs() < 1e-3,
                "pitch {pitch} came back as {got_pitch}"
            );
        }
    }
}
