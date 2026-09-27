use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicI64, Ordering::Relaxed};

use azalea::Client;
use azalea::physics::clip::{BlockShapeType, ClipContext, FluidPickType, clip};
use azalea_core::position::Vec3 as AzVec3;
use azalea_registry::builtin::EntityKind;

use super::registry::{Id, aim_assist as setting};
use super::{Edge, MS_PER_TICK, Phase, roll, store};
use crate::session::{SharedMutex, SharedState};
use crate::util::mth::wrap_degrees;

const AIM_HEIGHT: f32 = 0.85;

static TARGET: AtomicI32 = AtomicI32::new(NO_TARGET);
const NO_TARGET: i32 = i32::MIN;

static ARM_AT: AtomicI64 = AtomicI64::new(0);

static EDGE: Edge = Edge::new(Id::AimAssist);

pub fn enabled() -> bool {
    store().enabled(Id::AimAssist)
}

pub fn tick(bot: &Client, shared: &Arc<SharedMutex>) {
    let s = store();
    match EDGE.poll(s) {
        Phase::Off => return,
        Phase::Stopped => {
            shared.lock().unwrap().session.aim_target = None;
            TARGET.store(NO_TARGET, Relaxed);
            return;
        }
        Phase::Started => TARGET.store(NO_TARGET, Relaxed),
        Phase::Running => {}
    }
    let range = s.num(Id::AimAssist, setting::RANGE);
    let fov = s.num(Id::AimAssist, setting::FOV);

    let picked = {
        let mut state = shared.lock().unwrap();
        let eye = eye(&state, 1.0);
        match pick(&state, eye, range, fov) {
            Some(found) => (eye, found),
            None => {
                state.session.aim_target = None;
                TARGET.store(NO_TARGET, Relaxed);
                return;
            }
        }
    };
    let (eye, (id, point)) = picked;

    if !s.flag(Id::AimAssist, setting::IGNORE_WALLS) && !visible(bot, eye, point) {
        shared.lock().unwrap().session.aim_target = None;
        return;
    }

    let now = crate::client::tracking::game_time();
    if TARGET.swap(id, Relaxed) != id {
        let (lo, hi) = s.range(Id::AimAssist, setting::REACTION);
        ARM_AT.store(now + (roll(lo, hi) / MS_PER_TICK).round() as i64, Relaxed);
    }
    let armed = now >= ARM_AT.load(Relaxed);
    shared.lock().unwrap().session.aim_target = armed.then_some(id);
}

fn pick(s: &SharedState, eye: [f32; 3], range: f32, fov: f32) -> Option<(i32, [f32; 3])> {
    let forward = forward(s.camera_yaw, s.camera_pitch);
    let cos_limit = (fov.clamp(0.0, 360.0) * 0.5).to_radians().cos();
    let range_sq = range * range;
    let mut best: Option<(f32, i32, [f32; 3])> = None;

    let mut consider = |id: i32, point: [f32; 3]| {
        let to = [point[0] - eye[0], point[1] - eye[1], point[2] - eye[2]];
        let dist_sq = to[0] * to[0] + to[1] * to[1] + to[2] * to[2];
        if dist_sq > range_sq || dist_sq < f32::EPSILON {
            return;
        }
        let cos = (forward[0] * to[0] + forward[1] * to[1] + forward[2] * to[2]) / dist_sq.sqrt();
        if cos < cos_limit {
            return;
        }
        if best.is_none_or(|(b, _, _)| cos > b) {
            best = Some((cos, id, point));
        }
    };

    if super::entities::kind_enabled(EntityKind::Player) {
        for p in &s.session.other_players {
            if p.health.is_some_and(|h| h <= 0.0) {
                continue;
            }
            consider(p.id, player_point(p.anim.position(1.0), p.discrete));
        }
    }
    for e in s.session.entities.iter() {
        if !e.is_living() || e.is_invisible() || !super::entities::kind_enabled(e.kind) {
            continue;
        }
        consider(e.id, entity_point(e.position(1.0), e.bounding_box_height()));
    }

    best.map(|(_, id, point)| (id, point))
}

fn visible(bot: &Client, eye: [f32; 3], point: [f32; 3]) -> bool {
    let Ok(world) = bot.world() else {
        return false;
    };
    let vec = |p: [f32; 3]| AzVec3 {
        x: p[0] as f64,
        y: p[1] as f64,
        z: p[2] as f64,
    };
    let hit = clip(
        &world.read().chunks,
        ClipContext {
            from: vec(eye),
            to: vec(point),
            block_shape_type: BlockShapeType::Visual,
            fluid_pick_type: FluidPickType::None,
        },
    );
    hit.miss
}

pub fn nudge(s: &SharedState, yaw: f32, pitch: f32, dx: f32, dy: f32) -> (f32, f32) {
    let store = store();
    let Some(id) = s.session.aim_target else {
        return (dx, dy);
    };
    let pull = Pull {
        gain: store.num(Id::AimAssist, setting::STRENGTH) / 100.0,
        friction: store.num(Id::AimAssist, setting::FRICTION) / 100.0,
        deadzone: store.num(Id::AimAssist, setting::DEADZONE),
        window: store.num(Id::AimAssist, setting::FOV) * 0.5,
    };
    if pull.gain <= 0.0 && pull.friction <= 0.0 {
        return (dx, dy);
    }
    let partial = crate::renderer::systems::partial_ticks(s);
    let Some(point) = target_point(s, id, partial) else {
        return (dx, dy);
    };
    let (want_yaw, want_pitch) = look_at(eye(s, partial), point);
    (
        assist(dx, wrap_degrees(want_yaw - yaw), pull),
        assist(dy, want_pitch - pitch, pull),
    )
}

#[derive(Clone, Copy)]
struct Pull {
    gain: f32,
    friction: f32,
    deadzone: f32,
    window: f32,
}

fn assist(delta: f32, err: f32, p: Pull) -> f32 {
    let own = -delta;
    if own == 0.0 {
        return delta;
    }
    if (own > 0.0) == (err > 0.0) {
        let room = err.abs() - p.deadzone;
        if room <= 0.0 {
            return delta;
        }
        return delta - (own.abs() * p.gain).min(room).copysign(err);
    }
    if p.friction <= 0.0 || p.window <= 0.0 {
        return delta;
    }
    let closeness = 1.0 - (err.abs() / p.window).min(1.0);
    delta * (1.0 - p.friction * closeness)
}

fn target_point(s: &SharedState, id: i32, partial: f32) -> Option<[f32; 3]> {
    if let Some(p) = s.session.other_players.iter().find(|p| p.id == id) {
        return Some(player_point(p.anim.position(partial), p.discrete));
    }
    let e = s.session.entities.iter().find(|e| e.id == id)?;
    Some(entity_point(e.position(partial), e.bounding_box_height()))
}

fn player_point(pos: [f32; 3], discrete: bool) -> [f32; 3] {
    [
        pos[0],
        pos[1] + crate::renderer::systems::eye_height(discrete),
        pos[2],
    ]
}

fn entity_point(pos: [f32; 3], height: f32) -> [f32; 3] {
    [pos[0], pos[1] + height * AIM_HEIGHT, pos[2]]
}

pub(super) fn eye(s: &SharedState, partial: f32) -> [f32; 3] {
    let (prev, cur) = (s.session.player_pos_prev, s.session.player_pos);
    let at = |i: usize| prev[i] + (cur[i] - prev[i]) * partial;
    [
        at(0),
        at(1) + crate::renderer::systems::session_eye_height(&s.session),
        at(2),
    ]
}

fn forward(yaw: f32, pitch: f32) -> [f32; 3] {
    let (y, p) = (yaw.to_radians(), pitch.to_radians());
    [-y.sin() * p.cos(), p.sin(), -y.cos() * p.cos()]
}

pub(super) fn look_at(eye: [f32; 3], to: [f32; 3]) -> (f32, f32) {
    let (dx, dy, dz) = (to[0] - eye[0], to[1] - eye[1], to[2] - eye[2]);
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
        deadzone: 0.0,
        window: 30.0,
    };

    #[test]
    fn the_assist_only_ever_scales_the_hand() {
        assert_eq!(assist(0.0, 40.0, GAIN), 0.0);
        assert_eq!(assist(-3.0, 40.0, Pull { gain: 0.5, ..GAIN }), -4.5);
        assert_eq!(
            assist(
                -30.0,
                10.0,
                Pull {
                    deadzone: 2.0,
                    ..GAIN
                }
            ),
            -38.0
        );
        assert_eq!(
            assist(
                -3.0,
                1.5,
                Pull {
                    deadzone: 2.0,
                    ..GAIN
                }
            ),
            -3.0
        );
        assert_eq!(assist(3.0, 40.0, GAIN), 3.0);
    }

    #[test]
    fn friction_damps_the_way_out_and_nothing_else() {
        let p = Pull {
            gain: 0.0,
            friction: 0.5,
            deadzone: 0.0,
            window: 30.0,
        };
        assert_eq!(assist(-4.0, 0.0, p), -2.0);
        assert_eq!(assist(-4.0, -15.0, p), -3.0);
        assert_eq!(assist(-4.0, -30.0, p), -4.0);
        assert_eq!(assist(-4.0, -90.0, p), -4.0);
        let hard = Pull { friction: 0.9, ..p };
        assert!(assist(-4.0, -1.0, hard) < 0.0);
    }

    #[test]
    fn looking_at_a_point_faces_it() {
        for (yaw, pitch) in [(0.0, 0.0), (90.0, 30.0), (-135.0, -45.0), (179.0, 80.0)] {
            let f = forward(yaw, pitch);
            let eye = [1.0, 64.0, -2.0];
            let to = [
                eye[0] + f[0] * 5.0,
                eye[1] + f[1] * 5.0,
                eye[2] + f[2] * 5.0,
            ];
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
