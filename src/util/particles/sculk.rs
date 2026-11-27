#[allow(unused_imports)]
use super::*;

use azalea::entity::particle::PositionSource;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

const ROT: usize = 0;
const ROT_O: usize = 1;
const PITCH: usize = 2;
const PITCH_O: usize = 3;
const BACK: usize = 4;
const READY: usize = 5;
const ENTITY: usize = 6;

pub(super) fn recipe(
    particle: &P,
    atlas: &Atlas,
    arg: Vec3,
    rng: &mut JavaRandom,
    ctx: &SpawnCtx,
) -> Option<Option<Recipe>> {
    let name = particle_name(particle);
    Some(match particle {
        P::SonicBoom => sonic_boom(atlas, name, rng),
        P::SculkCharge(options) => sculk_charge(atlas, name, arg, rng, options.roll),
        P::SculkChargePop => sculk_charge_pop(atlas, name, arg, rng),
        P::Soul => soul(atlas, name, arg, rng, false),
        P::SculkSoul => soul(atlas, name, arg, rng, true),
        P::Shriek(options) => shriek(atlas, name, rng, options.delay, ctx),
        P::TrialSpawnerDetection | P::TrialSpawnerDetectionOminous => {
            trial_spawner_detection(atlas, name, arg, rng)
        }
        P::Vibration(options) => vibration(atlas, name, &options.position, options.ticks, ctx),
        _ => return None,
    })
}

fn sonic_boom(atlas: &Atlas, name: &str, rng: &mut JavaRandom) -> Option<Recipe> {
    let mut r = blank_recipe(Sprite::Animated(atlas.sheet_index(name)?));
    r.lifetime = 16;
    let grey = rng.next_f32() * 0.6 + 0.4;
    r.tint = [grey, grey, grey, 1.0];
    r.size = 1.5;
    r.flags = FULLBRIGHT;
    Some(r)
}

fn sculk_charge(
    atlas: &Atlas,
    name: &str,
    arg: Vec3,
    rng: &mut JavaRandom,
    roll: f32,
) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
    r.velocity = arg;
    r.friction = Vec3::splat(0.96);
    r.size *= 1.5;
    r.lifetime = rng.next_int(12) + 8;
    r.roll = roll;
    r.flags = TRANSLUCENT;
    r.light = Light::Block;
    Some(r)
}

fn sculk_charge_pop(atlas: &Atlas, name: &str, arg: Vec3, rng: &mut JavaRandom) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
    r.velocity = arg;
    r.friction = Vec3::splat(0.96);
    r.lifetime = rng.next_int(4) + 6;
    r.flags = TRANSLUCENT;
    r.light = Light::Block;
    Some(r)
}

fn soul(
    atlas: &Atlas,
    name: &str,
    arg: Vec3,
    rng: &mut JavaRandom,
    glowing: bool,
) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
    r.velocity = base_velocity(arg, rng) * 0.01 + arg;
    let mut jitter = || f64::from((rng.next_f32() - rng.next_f32()) * 0.05);
    r.nudge = DVec3::new(jitter(), jitter(), jitter());
    r.friction = Vec3::splat(0.96);
    r.lifetime = short_lifetime(rng) + 4;
    r.size *= 1.5;
    r.flags = PHYSICS | TRANSLUCENT;
    if glowing {
        r.light = Light::Block;
    }
    Some(r)
}

const SHRIEK_X_ROT: f32 = 1.0472;

static SHRIEK: Behavior = Behavior {
    tick: Some(shriek_tick),
    size: Some(shriek_size),
    tint: Some(shriek_tint),
    orient: Some(shriek_orient),
    ..Behavior::NONE
};

fn shriek(
    atlas: &Atlas,
    name: &str,
    rng: &mut JavaRandom,
    delay: i32,
    ctx: &SpawnCtx,
) -> Option<Recipe> {
    let mut r = blank_recipe(Sprite::Fixed(random_frame(atlas, name, rng)?));
    r.size = 0.85;
    r.lifetime = 30;
    r.velocity = Vec3::new(0.0, 0.1, 0.0);
    r.flags = PHYSICS | TRANSLUCENT;
    r.light = Light::Block;
    r.delay_ticks = delay.max(0) as u32;
    r.behavior = Some(&SHRIEK);
    r.data[BACK] = back_side(shriek_quats().0, ctx.pos, ctx.eye);
    Some(r)
}

fn shriek_quats() -> (Quat, Quat) {
    (
        Quat::from_rotation_x(-SHRIEK_X_ROT),
        Quat::from_rotation_y(-PI) * Quat::from_rotation_x(SHRIEK_X_ROT),
    )
}

fn shriek_tick(p: &mut Particle, ctx: &mut TickCtx<'_>) {
    ctx.default_tick(p);
    p.data[BACK] = back_side(shriek_quats().0, p.prev.lerp(p.pos, 0.5), ctx.eye);
}

fn shriek_size(p: &Particle, partial: f32) -> f32 {
    p.size * ((p.age as f32 + partial) / p.lifetime.max(1) as f32 * 0.75).clamp(0.0, 1.0)
}

fn shriek_tint(p: &Particle, partial: f32) -> [f32; 4] {
    let alpha = 1.0 - ((p.age as f32 + partial) / p.lifetime.max(1) as f32).clamp(0.0, 1.0);
    [p.tint[0], p.tint[1], p.tint[2], alpha]
}

fn shriek_orient(p: &Particle, _partial: f32) -> Quat {
    let (front, back) = shriek_quats();
    if p.data[BACK] != 0.0 { back } else { front }
}

fn trial_spawner_detection(
    atlas: &Atlas,
    name: &str,
    arg: Vec3,
    rng: &mut JavaRandom,
) -> Option<Recipe> {
    const SCALE: f32 = 1.5;
    let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
    let drift = base_velocity(Vec3::ZERO, rng);
    r.velocity = Vec3::new(0.0, drift.y * 0.9, 0.0) + arg;
    r.friction = Vec3::splat(0.96);
    r.gravity = -0.1;
    r.size *= 0.75 * SCALE;
    r.lifetime = ((8.0 / (rng.next_f32() * 0.5 + 0.5) * SCALE) as u32).max(1);
    r.grow = Grow::In;
    r.facing = Facing::CameraY;
    r.flags = PHYSICS | SPEED_UP_Y_BLOCKED;
    r.light = Light::Block;
    Some(r)
}

static VIBRATION: Behavior = Behavior {
    tick: Some(vibration_tick),
    tint: Some(vibration_tint),
    orient: Some(vibration_orient),
    ..Behavior::NONE
};

fn vibration(
    atlas: &Atlas,
    name: &str,
    destination: &PositionSource,
    arrival_ticks: u32,
    ctx: &SpawnCtx,
) -> Option<Recipe> {
    let mut r = blank_recipe(Sprite::Clock(atlas.sheet_index(name)?));
    r.size = 0.3;
    r.lifetime = arrival_ticks;
    r.flags = TRANSLUCENT;
    r.light = Light::Block;
    r.behavior = Some(&VIBRATION);
    match destination {
        PositionSource::Block(cell) => {
            let target = DVec3::new(
                f64::from(cell.x) + 0.5,
                f64::from(cell.y) + 0.5,
                f64::from(cell.z) + 0.5,
            );
            let (rot, pitch) = aim(ctx.pos, target);
            r.target = target;
            r.data[ROT] = rot;
            r.data[ROT_O] = rot;
            r.data[PITCH] = pitch;
            r.data[PITCH_O] = pitch;
            r.data[READY] = 1.0;
            let front = vibration_quats(rot, pitch, 0.5).0;
            r.data[BACK] = back_side(front, ctx.pos, ctx.eye);
        }
        PositionSource::Entity { id, y_offset } => {
            r.target = DVec3::new(f64::from(id.0), f64::from(*y_offset), 0.0);
            r.data[ENTITY] = 1.0;
        }
    }
    Some(r)
}

fn vibration_destination(p: &Particle, ctx: &TickCtx<'_>) -> Option<DVec3> {
    if p.data[ENTITY] == 0.0 {
        return Some(p.target);
    }
    ctx.entity_pos(p.target.x as i32)
        .map(|at| at + DVec3::new(0.0, p.target.y, 0.0))
}

fn vibration_tick(p: &mut Particle, ctx: &mut TickCtx<'_>) {
    let Some(destination) = vibration_destination(p, ctx) else {
        p.remove();
        return;
    };
    if p.data[READY] == 0.0 {
        let (rot, pitch) = aim(p.pos, destination);
        p.data[ROT] = rot;
        p.data[PITCH] = pitch;
        p.data[READY] = 1.0;
    }
    let remaining = p.lifetime.saturating_sub(p.age);
    if remaining == 0 {
        p.remove();
        return;
    }
    p.pos = p.pos.lerp(destination, 1.0 / f64::from(remaining));
    let (rot, pitch) = aim(p.pos, destination);
    p.data[ROT_O] = p.data[ROT];
    p.data[ROT] = rot;
    p.data[PITCH_O] = p.data[PITCH];
    p.data[PITCH] = pitch;
    let front = vibration_quats(
        lerp(0.5, p.data[ROT_O], p.data[ROT]),
        lerp(0.5, p.data[PITCH_O], p.data[PITCH]),
        p.age as f32 + 0.5,
    )
    .0;
    p.data[BACK] = back_side(front, p.prev.lerp(p.pos, 0.5), ctx.eye);
}

fn vibration_tint(p: &Particle, _partial: f32) -> [f32; 4] {
    if p.data[READY] == 0.0 {
        return [p.tint[0], p.tint[1], p.tint[2], 0.0];
    }
    p.tint
}

fn vibration_orient(p: &Particle, partial: f32) -> Quat {
    let (front, back) = vibration_quats(
        lerp(partial, p.data[ROT_O], p.data[ROT]),
        lerp(partial, p.data[PITCH_O], p.data[PITCH]),
        p.age as f32 + partial,
    );
    if p.data[BACK] != 0.0 { back } else { front }
}

fn vibration_quats(rot: f32, pitch: f32, age: f32) -> (Quat, Quat) {
    let sway = ((age - TAU) * 0.05).sin() * 2.0;
    let pitch = pitch + FRAC_PI_2;
    (
        Quat::from_rotation_y(rot) * Quat::from_rotation_x(-pitch) * Quat::from_rotation_y(sway),
        Quat::from_rotation_y(-PI + rot)
            * Quat::from_rotation_x(pitch)
            * Quat::from_rotation_y(sway),
    )
}

fn aim(from: DVec3, destination: DVec3) -> (f32, f32) {
    let d = from - destination;
    (
        mth_atan2(d.x, d.z) as f32,
        mth_atan2(d.y, (d.x * d.x + d.z * d.z).sqrt()) as f32,
    )
}

fn mth_atan2(y: f64, x: f64) -> f64 {
    let y = if y == 0.0 { 0.0 } else { y };
    let x = if x == 0.0 { 0.0 } else { x };
    y.atan2(x)
}

fn lerp(delta: f32, a: f32, b: f32) -> f32 {
    a + delta * (b - a)
}

fn back_side(front: Quat, centre: DVec3, eye: DVec3) -> f32 {
    if (front * Vec3::Z).dot((eye - centre).as_vec3()) > 0.0 {
        0.0
    } else {
        1.0
    }
}
