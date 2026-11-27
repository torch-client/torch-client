#[allow(unused_imports)]
use super::*;

pub(super) fn recipe(
    particle: &P,
    atlas: &Atlas,
    arg: Vec3,
    rng: &mut JavaRandom,
    _ctx: &SpawnCtx,
) -> Option<Option<Recipe>> {
    let name = particle_name(particle);
    Some(match particle {
        P::Splash => water_drop(atlas, name, arg, rng, true),
        P::Rain => water_drop(atlas, name, arg, rng, false),
        P::Bubble => bubble(atlas, name, arg, rng),
        P::BubbleColumnUp => bubble_column_up(atlas, name, arg, rng),
        P::BubblePop => bubble_pop(atlas, name, arg, rng),
        P::CurrentDown => current_down(atlas, name, rng),
        P::Fishing => wake(atlas, name, arg, rng),
        P::SquidInk => squid_ink(atlas, name, arg, rng, [0.0, 0.0, 0.0]),
        P::GlowSquidInk => squid_ink(atlas, name, arg, rng, [0.2, 0.8, 0.6]),
        P::Glow => glow_squid(atlas, name, arg, rng),
        P::SulfurBubbles => sulfur_bubble(atlas, name, arg, rng),
        P::GeyserPlume(g) => geyser_plume(atlas, name, arg, rng, g.water_blocks),
        _ => return None,
    })
}

fn out_of_water(ctx: &TickCtx<'_>, pos: DVec3) -> bool {
    ctx.world().is_some() && !ctx.in_water(pos)
}

static WATER_DROP_HOOKS: Behavior = Behavior {
    tick: Some(water_drop_tick),
    ..Behavior::NONE
};

fn water_drop_tick(p: &mut Particle, ctx: &mut TickCtx<'_>) {
    p.velocity.y -= p.data[0];
    let v = p.velocity;
    ctx.move_by(p, v);
    p.velocity *= 0.98;
    if p.on_ground {
        if ctx.rng().next_f32() < 0.5 {
            p.remove();
        }
        p.velocity.x *= 0.7;
        p.velocity.z *= 0.7;
    }
    if let Some(world) = ctx.world() {
        let cell = p.pos.floor().as_ivec3();
        if let Some(state) = block_at(world, cell.to_array()) {
            let top = crate::renderer::weather::surface_top(
                world,
                state,
                cell,
                p.pos.x - f64::from(cell.x),
                p.pos.z - f64::from(cell.z),
            );
            if top > 0.0 && p.pos.y < f64::from(cell.y) + top {
                p.remove();
            }
        }
    }
}

fn water_drop(
    atlas: &Atlas,
    name: &str,
    arg: Vec3,
    rng: &mut JavaRandom,
    splash: bool,
) -> Option<Recipe> {
    let uv = random_frame(atlas, name, rng)?;
    let mut r = base_recipe(Sprite::Fixed(uv), rng);
    let base = base_velocity(Vec3::ZERO, rng);
    r.velocity = Vec3::new(base.x * 0.3, rng.next_f32() * 0.2 + 0.1, base.z * 0.3);
    r.lifetime = (8.0 / (f64::from(rng.next_f32()) * 0.8 + 0.2)) as u32;
    r.data[0] = if splash { 0.04 } else { 0.06 };
    if splash && arg.y == 0.0 && (arg.x != 0.0 || arg.z != 0.0) {
        r.velocity = Vec3::new(arg.x, 0.1, arg.z);
    }
    r.flags = PHYSICS;
    r.behavior = Some(&WATER_DROP_HOOKS);
    Some(r)
}

static BUBBLE_HOOKS: Behavior = Behavior {
    tick: Some(bubble_tick),
    ..Behavior::NONE
};

fn bubble_tick(p: &mut Particle, ctx: &mut TickCtx<'_>) {
    p.velocity.y += 0.002;
    let v = p.velocity;
    ctx.move_by(p, v);
    p.velocity *= 0.85;
    if out_of_water(ctx, p.pos) {
        p.remove();
    }
}

fn bubble_base(atlas: &Atlas, name: &str, arg: Vec3, rng: &mut JavaRandom) -> Option<Recipe> {
    let uv = random_frame(atlas, name, rng)?;
    let mut r = base_recipe(Sprite::Fixed(uv), rng);
    r.size *= rng.next_f32() * 0.6 + 0.2;
    r.velocity = arg * 0.2 + centred_jitter(rng, 0.04);
    r.flags = PHYSICS;
    Some(r)
}

fn bubble(atlas: &Atlas, name: &str, arg: Vec3, rng: &mut JavaRandom) -> Option<Recipe> {
    let mut r = bubble_base(atlas, name, arg, rng)?;
    r.lifetime = (8.0 / (f64::from(rng.next_f32()) * 0.8 + 0.2)) as u32;
    r.behavior = Some(&BUBBLE_HOOKS);
    Some(r)
}

static BUBBLE_COLUMN_UP_HOOKS: Behavior = Behavior {
    tick: Some(bubble_column_up_tick),
    ..Behavior::NONE
};

fn bubble_column_up_tick(p: &mut Particle, ctx: &mut TickCtx<'_>) {
    ctx.default_tick(p);
    if !p.dead && out_of_water(ctx, p.pos) {
        p.remove();
    }
}

fn bubble_column_up(atlas: &Atlas, name: &str, arg: Vec3, rng: &mut JavaRandom) -> Option<Recipe> {
    let mut r = bubble_base(atlas, name, arg, rng)?;
    r.gravity = -0.125;
    r.friction = Vec3::splat(0.85);
    r.lifetime = (40.0 / (f64::from(rng.next_f32()) * 0.8 + 0.2)) as u32;
    r.behavior = Some(&BUBBLE_COLUMN_UP_HOOKS);
    Some(r)
}

static BUBBLE_POP_HOOKS: Behavior = Behavior {
    tick: Some(bubble_pop_tick),
    ..Behavior::NONE
};

fn bubble_pop_tick(p: &mut Particle, ctx: &mut TickCtx<'_>) {
    p.velocity.y -= 0.008;
    let v = p.velocity;
    ctx.move_by(p, v);
}

fn bubble_pop(atlas: &Atlas, name: &str, arg: Vec3, rng: &mut JavaRandom) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
    r.lifetime = 4;
    r.velocity = arg;
    r.flags = PHYSICS;
    r.behavior = Some(&BUBBLE_POP_HOOKS);
    Some(r)
}

static CURRENT_DOWN_HOOKS: Behavior = Behavior {
    tick: Some(current_down_tick),
    ..Behavior::NONE
};

fn current_down_tick(p: &mut Particle, ctx: &mut TickCtx<'_>) {
    let angle = p.data[0];
    p.velocity.x += 0.6 * angle.cos();
    p.velocity.z += 0.6 * angle.sin();
    p.velocity.x *= 0.07;
    p.velocity.z *= 0.07;
    let v = p.velocity;
    ctx.move_by(p, v);
    if out_of_water(ctx, p.pos) || p.on_ground {
        p.remove();
    }
    p.data[0] = angle + 0.08;
}

fn current_down(atlas: &Atlas, name: &str, rng: &mut JavaRandom) -> Option<Recipe> {
    let uv = random_frame(atlas, name, rng)?;
    let mut r = base_recipe(Sprite::Fixed(uv), rng);
    r.lifetime = (rng.next_f32() * 60.0) as u32 + 30;
    r.flags = 0;
    r.velocity = Vec3::new(0.0, -0.05, 0.0);
    r.size *= rng.next_f32() * 0.6 + 0.2;
    r.behavior = Some(&CURRENT_DOWN_HOOKS);
    Some(r)
}

static WAKE_HOOKS: Behavior = Behavior {
    tick: Some(wake_tick),
    ..Behavior::NONE
};

fn wake_tick(p: &mut Particle, ctx: &mut TickCtx<'_>) {
    let life = 60 + i64::from(p.age) - 1 - i64::from(p.lifetime);
    let v = p.velocity;
    ctx.move_by(p, v);
    p.velocity *= 0.98;
    if let Some(atlas) = ctx.atlas() {
        let sheet = p.data[0] as u16;
        if let Some(frames) = atlas.sheets.get(sheet as usize).map(Vec::len) {
            let frame = life.rem_euclid(4) as usize * frames.saturating_sub(1) / 4;
            p.sprite = Sprite::Fixed(atlas.frame_uv(sheet, frame));
        }
    }
}

fn wake(atlas: &Atlas, name: &str, arg: Vec3, rng: &mut JavaRandom) -> Option<Recipe> {
    let sheet = atlas.sheet_index(name)?;
    let mut r = base_recipe(Sprite::Fixed(atlas.frame_uv(sheet, 0)), rng);
    r.lifetime = (8.0 / (f64::from(rng.next_f32()) * 0.8 + 0.2)) as u32;
    r.velocity = arg;
    r.flags = PHYSICS;
    r.data[0] = f32::from(sheet);
    r.behavior = Some(&WAKE_HOOKS);
    Some(r)
}

static SQUID_INK_HOOKS: Behavior = Behavior {
    tick: Some(squid_ink_tick),
    ..Behavior::NONE
};

fn squid_ink_tick(p: &mut Particle, ctx: &mut TickCtx<'_>) {
    ctx.default_tick(p);
    let half = p.lifetime / 2;
    if p.age > half {
        p.tint[3] = 1.0 - (p.age as f32 - half as f32) / p.lifetime as f32;
    }
    if ctx.is_air(p.pos) {
        p.velocity.y -= 0.0074;
    }
}

fn squid_ink(
    atlas: &Atlas,
    name: &str,
    arg: Vec3,
    rng: &mut JavaRandom,
    rgb: [f32; 3],
) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
    r.friction = Vec3::splat(0.92);
    r.size = 0.5;
    r.tint = [rgb[0], rgb[1], rgb[2], 1.0];
    r.lifetime = (0.5 * 12.0 / (rng.next_f32() * 0.8 + 0.2)) as u32;
    r.flags = TRANSLUCENT | FULLBRIGHT;
    r.velocity = arg;
    r.behavior = Some(&SQUID_INK_HOOKS);
    Some(r)
}

fn glow_squid(atlas: &Atlas, name: &str, arg: Vec3, rng: &mut JavaRandom) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
    let seed = Vec3::new(
        (0.5 - rng.next_f64()) as f32,
        arg.y,
        (0.5 - rng.next_f64()) as f32,
    );
    let mut v = base_velocity(seed, rng);
    v.y *= 0.2;
    if arg.x == 0.0 && arg.z == 0.0 {
        v.x *= 0.1;
        v.z *= 0.1;
    }
    r.velocity = v;
    r.tint = if rng.next_bool() {
        [0.6, 1.0, 0.8, 1.0]
    } else {
        [0.08, 0.4, 0.4, 1.0]
    };
    r.lifetime = (8.0 / (rng.next_f64() * 0.8 + 0.2)) as u32;
    r.friction = Vec3::splat(0.96);
    r.size *= 0.75;
    r.flags = SPEED_UP_Y_BLOCKED;
    r.light = Light::Linear;
    Some(r)
}

const SULFUR_RISE: f64 = 3.0;
const SULFUR_SIZE_END: f32 = 0.15;

static SULFUR_BUBBLE_HOOKS: Behavior = Behavior {
    tick: Some(sulfur_bubble_tick),
    ..Behavior::NONE
};

fn sulfur_wiggle(rng: &mut JavaRandom) -> f32 {
    let magnitude = rng.next_f32() * 0.003;
    let sign = if rng.next_bool() { 1.0 } else { -1.0 };
    magnitude * sign * 0.5
}

fn sulfur_bubble_tick(p: &mut Particle, ctx: &mut TickCtx<'_>) {
    ctx.default_tick(p);
    if !p.dead && ctx.world().is_some() && !ctx.in_water_source(p.pos) {
        p.remove();
    }
    let y_start = p.origin.y;
    if !p.dead && p.pos.y >= y_start + SULFUR_RISE {
        p.remove();
    }
    if !p.dead && p.pos.y <= p.prev.y {
        p.remove();
    }
    let wx = sulfur_wiggle(ctx.rng());
    let wz = sulfur_wiggle(ctx.rng());
    p.velocity.x += wx;
    p.velocity.z += wz;
    let step = Vec3::new(p.velocity.x, 0.0, p.velocity.z);
    ctx.move_by(p, step);
    let progress = ((p.pos.y - y_start) / SULFUR_RISE) as f32;
    let size_start = p.data[0];
    p.size = size_start + progress * (SULFUR_SIZE_END - size_start);
}

fn sulfur_bubble(atlas: &Atlas, name: &str, arg: Vec3, rng: &mut JavaRandom) -> Option<Recipe> {
    let uv = random_frame(atlas, name, rng)?;
    let mut r = base_recipe(Sprite::Fixed(uv), rng);
    r.gravity = -0.04;
    r.friction = Vec3::splat(0.85);
    let xd = arg.x * 0.2 + (rng.next_f32() * 2.0 - 1.0) * 0.02;
    let zd = arg.y * 0.2 + (rng.next_f32() * 2.0 - 1.0) * 0.02;
    r.velocity = Vec3::new(xd, 0.0, zd);
    let size_start = 0.02 + 0.02 * rng.next_f32();
    r.size = size_start;
    r.data[0] = size_start;
    r.lifetime = i32::MAX as u32;
    r.flags = PHYSICS;
    r.behavior = Some(&SULFUR_BUBBLE_HOOKS);
    Some(r)
}

const PLUME_DONE: usize = 0;
const PLUME_PROPULSION: usize = 1;
const PLUME_SPRAY_X: usize = 2;
const PLUME_SPRAY_Z: usize = 3;
const PLUME_MIN_SIZE: usize = 4;
const PLUME_MAX_SIZE: usize = 5;
const PLUME_RISE: usize = 6;

static GEYSER_PLUME_HOOKS: Behavior = Behavior {
    tick: Some(geyser_plume_tick),
    ..Behavior::NONE
};

fn geyser_plume_tick(p: &mut Particle, ctx: &mut TickCtx<'_>) {
    ctx.default_tick(p);
    let rise = f64::from(p.data[PLUME_RISE]);
    let start_y = p.origin.y;
    if p.data[PLUME_DONE] == 0.0
        && (p.velocity.y < 0.0 || p.pos.y > start_y + rise || p.pos.y == p.prev.y)
    {
        p.lifetime = p.lifetime.min(p.age + 5);
        p.friction = Vec3::ZERO;
        p.data[PLUME_DONE] = 1.0;
    }
    let linear = ((p.pos.y - start_y) / rise).clamp(0.0, 1.0);
    let exponential = linear.powi(3);
    p.gravity = p.data[PLUME_PROPULSION] * exponential as f32 * 0.12;
    p.velocity.x = (linear * f64::from(p.data[PLUME_SPRAY_X])) as f32;
    p.velocity.z = (linear * f64::from(p.data[PLUME_SPRAY_Z])) as f32;
    let (min, max) = (p.data[PLUME_MIN_SIZE], p.data[PLUME_MAX_SIZE]);
    p.size = min + (linear * f64::from(max - min)) as f32;
}

fn geyser_plume(
    atlas: &Atlas,
    name: &str,
    arg: Vec3,
    rng: &mut JavaRandom,
    water_blocks: i32,
) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
    let mut v = base_velocity(arg, rng);
    v.y = 0.0;
    r.velocity = v;
    let plume_height = water_blocks.max(1).saturating_mul(5);
    r.lifetime = plume_height.saturating_mul(5) as u32;
    let height = plume_height as f32;
    r.data[PLUME_RISE] = height - 1.0;
    r.data[PLUME_SPRAY_X] = (rng.next_f32() - 0.5) * 0.2;
    r.data[PLUME_SPRAY_Z] = (rng.next_f32() - 0.5) * 0.2;
    r.friction = Vec3::ONE;
    let propulsion = (if water_blocks == 1 { 1.5 } else { 1.0 }) * height * 1.45;
    r.data[PLUME_PROPULSION] = propulsion;
    r.gravity = -propulsion;
    let randomized = r.size * 0.75;
    let min = randomized * (2.0 + height / 8.0);
    r.data[PLUME_MIN_SIZE] = min;
    r.data[PLUME_MAX_SIZE] = randomized * (3.0 + height / 8.0);
    r.size = min;
    r.flags = PHYSICS | SPEED_UP_Y_BLOCKED;
    r.behavior = Some(&GEYSER_PLUME_HOOKS);
    Some(r)
}
