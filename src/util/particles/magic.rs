#[allow(unused_imports)]
use super::*;

use azalea_inventory::components::{FireworkExplosion, FireworkExplosionShape as Shape};

pub(super) fn recipe(
    particle: &P,
    atlas: &Atlas,
    arg: Vec3,
    rng: &mut JavaRandom,
    _ctx: &SpawnCtx,
) -> Option<Option<Recipe>> {
    Some(match particle {
        P::Trail(t) => trail(
            atlas,
            DVec3::new(t.target.x, t.target.y, t.target.z),
            [t.color.red(), t.color.green(), t.color.blue()],
            t.duration,
            rng,
        ),
        P::OminousSpawning => ominous_spawning(atlas, arg, rng),
        P::ReversePortal => reverse_portal(atlas, arg, rng),
        P::DragonBreath(o) => dragon_breath(atlas, arg, o.power, rng),
        P::Flash(o) => flash(
            atlas,
            [
                o.color.red(),
                o.color.green(),
                o.color.blue(),
                o.color.alpha(),
            ],
            rng,
        ),
        P::Firework => spark_recipe(atlas, arg, rng),
        P::EndRod => end_rod(atlas, arg, rng),
        P::TotemOfUndying => totem(atlas, arg, rng),
        _ => return None,
    })
}

static TRAIL: Behavior = Behavior {
    tick: Some(trail_tick),
    ..Behavior::NONE
};

fn trail(
    atlas: &Atlas,
    target: DVec3,
    rgb: [u8; 3],
    duration: i32,
    rng: &mut JavaRandom,
) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Fixed(random_frame(atlas, "trail", rng)?), rng);
    let mut channel = |c: u8| {
        let scaled = (f32::from(c) * (0.875 + rng.next_f32() * 0.25)) as i32;
        scaled.clamp(0, 255) as f32 / 255.0
    };
    let red = channel(rgb[0]);
    let green = channel(rgb[1]);
    let blue = channel(rgb[2]);
    r.tint = [red, green, blue, 1.0];
    r.size = 0.26;
    r.lifetime = duration.max(0) as u32;
    r.flags = FULLBRIGHT;
    r.target = target;
    r.behavior = Some(&TRAIL);
    Some(r)
}

fn trail_tick(p: &mut Particle, _ctx: &mut TickCtx<'_>) {
    let remaining = p.lifetime - p.age;
    if remaining == 0 {
        p.remove();
        return;
    }
    p.pos = p.pos.lerp(p.target, 1.0 / f64::from(remaining));
}

static FLY_STRAIGHT: Behavior = Behavior {
    tick: Some(fly_straight_tick),
    ..Behavior::NONE
};

const OMINOUS_START: [i32; 4] = [0xFF, 0x45, 0xAE, 0xFE];
const OMINOUS_END: [i32; 4] = [0xFF, 0xFF, 0xFF, 0xFF];

fn ominous_spawning(atlas: &Atlas, arg: Vec3, rng: &mut JavaRandom) -> Option<Recipe> {
    let mut r = base_recipe(
        Sprite::Fixed(random_frame(atlas, "ominous_spawning", rng)?),
        rng,
    );
    r.velocity = arg;
    r.start_offset = arg.as_dvec3();
    r.size = 0.1 * (rng.next_f32() * 0.5 + 0.2);
    r.lifetime = (rng.next_f32() * 5.0) as u32 + 25;
    r.size *= rng.next_f32() * 2.0 + 3.0;
    r.flags = 0;
    r.light = Light::Block;
    r.behavior = Some(&FLY_STRAIGHT);
    Some(r)
}

fn lerp_int(alpha: f32, p0: i32, p1: i32) -> i32 {
    p0 + (alpha * (p1 - p0) as f32).floor() as i32
}

fn fly_straight_tick(p: &mut Particle, _ctx: &mut TickCtx<'_>) {
    let t = p.age as f32 / p.lifetime as f32;
    p.pos = p.origin + (p.velocity * (1.0 - t)).as_dvec3();
    let [a, r, g, b]: [f32; 4] =
        std::array::from_fn(|i| lerp_int(t, OMINOUS_START[i], OMINOUS_END[i]) as f32 / 255.0);
    p.tint = [r, g, b, a];
}

static REVERSE_PORTAL: Behavior = Behavior {
    tick: Some(reverse_portal_tick),
    size: Some(reverse_portal_size),
    ..Behavior::NONE
};

fn reverse_portal(atlas: &Atlas, arg: Vec3, rng: &mut JavaRandom) -> Option<Recipe> {
    let mut r = base_recipe(
        Sprite::Fixed(random_frame(atlas, "reverse_portal", rng)?),
        rng,
    );
    r.velocity = arg;
    r.size = 0.1 * (rng.next_f32() * 0.2 + 0.5) * 1.5;
    let br = rng.next_f32() * 0.6 + 0.4;
    r.tint = [br * 0.9, br * 0.3, br, 1.0];
    r.lifetime = (rng.next_f32() * 2.0) as u32 + 60;
    r.flags = 0;
    r.light = Light::Quartic;
    r.behavior = Some(&REVERSE_PORTAL);
    Some(r)
}

fn reverse_portal_tick(p: &mut Particle, _ctx: &mut TickCtx<'_>) {
    let speed = p.age as f32 / p.lifetime as f32;
    p.pos += (p.velocity * speed).as_dvec3();
}

fn reverse_portal_size(p: &Particle, partial: f32) -> f32 {
    p.size * (1.0 - (p.age as f32 + partial) / (p.lifetime as f32 * 1.5))
}

static DRAGON_BREATH: Behavior = Behavior {
    tick: Some(dragon_breath_tick),
    ..Behavior::NONE
};

const HAS_HIT_GROUND: usize = 0;

fn dragon_breath(atlas: &Atlas, arg: Vec3, power: f32, rng: &mut JavaRandom) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Animated(atlas.sheet_index("dragon_breath")?), rng);
    r.friction = Vec3::splat(0.96);
    r.velocity = set_power(arg, power);
    let red = rng.next_f32() * (0.8745098 - 0.7176471) + 0.7176471;
    let blue = rng.next_f32() * (0.9764706 - 0.8235294) + 0.8235294;
    r.tint = [red, 0.0, blue, 1.0];
    r.size *= 0.75;
    r.lifetime = (20.0 / (f64::from(rng.next_f32()) * 0.8 + 0.2)) as u32;
    r.grow = Grow::In;
    r.flags = 0;
    r.behavior = Some(&DRAGON_BREATH);
    Some(r)
}

fn dragon_breath_tick(p: &mut Particle, ctx: &mut TickCtx<'_>) {
    if p.on_ground {
        p.velocity.y = 0.0;
        p.data[HAS_HIT_GROUND] = 1.0;
    }
    let hit_ground = p.data[HAS_HIT_GROUND] != 0.0;
    if hit_ground {
        p.velocity.y += 0.002;
    }
    let velocity = p.velocity;
    ctx.move_by(p, velocity);
    if p.pos.y == p.prev.y {
        p.velocity.x *= 1.1;
        p.velocity.z *= 1.1;
    }
    p.velocity.x *= p.friction.x;
    p.velocity.z *= p.friction.z;
    if hit_ground {
        p.velocity.y *= p.friction.y;
    }
}

static FLASH: Behavior = Behavior {
    size: Some(flash_size),
    tint: Some(flash_tint),
    ..Behavior::NONE
};

fn flash(atlas: &Atlas, rgba: [u8; 4], rng: &mut JavaRandom) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Fixed(random_frame(atlas, "flash", rng)?), rng);
    r.lifetime = 4;
    r.tint = [unit(rgba[0]), unit(rgba[1]), unit(rgba[2]), unit(rgba[3])];
    r.flags = TRANSLUCENT;
    r.behavior = Some(&FLASH);
    Some(r)
}

fn flash_size(p: &Particle, partial: f32) -> f32 {
    7.1 * ((p.age as f32 + partial - 1.0) * 0.25 * std::f32::consts::PI).sin()
}

fn flash_tint(p: &Particle, partial: f32) -> [f32; 4] {
    let alpha = 0.6 - (p.age as f32 + partial - 1.0) * 0.25 * 0.5;
    [p.tint[0], p.tint[1], p.tint[2], alpha]
}

const FADE_R: usize = 0;
const FADE_B: usize = 2;
const HAS_FADE: usize = 3;
const TRAIL_FLAG: usize = 4;
const TWINKLE_FLAG: usize = 5;

fn simple_animated(
    atlas: &Atlas,
    name: &str,
    velocity: Vec3,
    gravity: f32,
    rng: &mut JavaRandom,
) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
    r.velocity = velocity;
    r.gravity = gravity;
    r.friction = Vec3::splat(0.91);
    r.flags = PHYSICS | TRANSLUCENT | FULLBRIGHT | FADE_OUT;
    Some(r)
}

fn set_fade(r: &mut Recipe, rgb: u32) {
    r.data[FADE_R] = ((rgb >> 16) & 0xFF) as f32 / 255.0;
    r.data[FADE_R + 1] = ((rgb >> 8) & 0xFF) as f32 / 255.0;
    r.data[FADE_B] = (rgb & 0xFF) as f32 / 255.0;
    r.data[HAS_FADE] = 1.0;
}

fn simple_animated_tick(p: &mut Particle, ctx: &mut TickCtx<'_>) {
    ctx.default_tick(p);
    let half = p.lifetime / 2;
    if p.age > half {
        p.tint[3] = 1.0 - (p.age - half) as f32 / p.lifetime as f32;
        if p.data[HAS_FADE] != 0.0 {
            for i in 0..3 {
                p.tint[i] += (p.data[FADE_R + i] - p.tint[i]) * 0.2;
            }
        }
    }
}

static END_ROD: Behavior = Behavior {
    tick: Some(simple_animated_tick),
    ..Behavior::NONE
};

fn end_rod(atlas: &Atlas, arg: Vec3, rng: &mut JavaRandom) -> Option<Recipe> {
    let mut r = simple_animated(atlas, "end_rod", arg, 0.0125, rng)?;
    r.size *= 0.75;
    r.lifetime = 60 + rng.next_int(12);
    set_fade(&mut r, 15916745);
    r.flags = TRANSLUCENT | FULLBRIGHT;
    r.behavior = Some(&END_ROD);
    Some(r)
}

fn totem(atlas: &Atlas, arg: Vec3, rng: &mut JavaRandom) -> Option<Recipe> {
    let mut r = simple_animated(atlas, "totem_of_undying", arg, 1.25, rng)?;
    r.friction = Vec3::splat(0.6);
    r.size *= 0.75;
    r.lifetime = 60 + rng.next_int(12);
    let (red, green, blue) = if rng.next_int(4) == 0 {
        let red = 0.6 + rng.next_f32() * 0.2;
        let green = 0.6 + rng.next_f32() * 0.3;
        (red, green, rng.next_f32() * 0.2)
    } else {
        let red = 0.1 + rng.next_f32() * 0.2;
        let green = 0.4 + rng.next_f32() * 0.3;
        (red, green, rng.next_f32() * 0.2)
    };
    r.tint = [red, green, blue, 1.0];
    Some(r)
}

static SPARK: Behavior = Behavior {
    tick: Some(spark_tick),
    tint: Some(spark_tint),
    ..Behavior::NONE
};

pub(super) fn spark_recipe(atlas: &Atlas, velocity: Vec3, rng: &mut JavaRandom) -> Option<Recipe> {
    let mut r = simple_animated(atlas, "firework", velocity, 0.1, rng)?;
    r.size *= 0.75;
    r.lifetime = 48 + rng.next_int(12);
    r.tint[3] = 0.99;
    r.flags = PHYSICS | TRANSLUCENT | FULLBRIGHT | FIREWORK_SPARK;
    r.behavior = Some(&SPARK);
    Some(r)
}

pub(super) fn style_spark(r: &mut Recipe, rgb: u32, fade: Option<u32>, trail: bool, twinkle: bool) {
    r.tint[0] = ((rgb >> 16) & 0xFF) as f32 / 255.0;
    r.tint[1] = ((rgb >> 8) & 0xFF) as f32 / 255.0;
    r.tint[2] = (rgb & 0xFF) as f32 / 255.0;
    if let Some(fade) = fade {
        set_fade(r, fade);
    }
    r.data[TRAIL_FLAG] = if trail { 1.0 } else { 0.0 };
    r.data[TWINKLE_FLAG] = if twinkle { 1.0 } else { 0.0 };
}

fn spark_tick(p: &mut Particle, ctx: &mut TickCtx<'_>) {
    simple_animated_tick(p, ctx);
    if p.data[TRAIL_FLAG] == 0.0 || p.age >= p.lifetime / 2 || (p.age + p.lifetime) % 2 != 0 {
        return;
    }
    let Some(atlas) = ctx.atlas() else { return };
    let Some(mut child) = spark_recipe(atlas, Vec3::ZERO, ctx.rng()) else {
        return;
    };
    child.tint = [p.tint[0], p.tint[1], p.tint[2], 0.99];
    child.start_age = child.lifetime / 2;
    child.data[TWINKLE_FLAG] = p.data[TWINKLE_FLAG];
    ctx.spawn_recipe(child, p.pos);
}

fn spark_tint(p: &Particle, _partial: f32) -> [f32; 4] {
    let l = p.lifetime;
    let shown = p.data[TWINKLE_FLAG] == 0.0 || p.age < l / 3 || (p.age + l) / 3 % 2 == 0;
    if shown {
        p.tint
    } else {
        [p.tint[0], p.tint[1], p.tint[2], 0.0]
    }
}

const CREEPER_COORDS: [[f64; 2]; 12] = [
    [0.0, 0.2],
    [0.2, 0.2],
    [0.2, 0.6],
    [0.6, 0.6],
    [0.6, 0.2],
    [0.2, 0.2],
    [0.2, 0.0],
    [0.4, 0.0],
    [0.4, -0.6],
    [0.2, -0.6],
    [0.2, -0.4],
    [0.0, -0.4],
];

const STAR_COORDS: [[f64; 2]; 6] = [
    [0.0, 1.0],
    [0.3455, 0.309],
    [0.9511, 0.309],
    [0.3795918367346939, -0.12653061224489795],
    [0.6122448979591837, -0.8040816326530612],
    [0.0, -0.35918367346938773],
];

const BLACK_FIREWORK: i32 = 1973019;

const MAX_SHOWS: usize = 64;

pub(super) struct Show {
    pos: DVec3,
    velocity: [f64; 3],
    explosions: std::sync::Arc<[FireworkExplosion]>,
    next: usize,
    wait: u32,
}

pub(super) fn spawn_firework(
    emit: &crate::session::FireworkEmit,
    atlas: &Atlas,
    world: Option<&azalea_world::World>,
    eye: DVec3,
    status: ParticleStatus,
    live: &mut Vec<Particle>,
    rng: &mut JavaRandom,
    sparks: &mut u32,
    shows: &mut Vec<Show>,
) {
    let pos = DVec3::from_array(emit.pos);
    if emit.explosions.is_empty() {
        let mut i = 0;
        while i < rng.next_int(3) + 2 {
            let x = rng.next_gaussian() * 0.05;
            let z = rng.next_gaussian() * 0.05;
            let gate = SpawnGate::normal(&P::Poof);
            add_level_particle(
                live,
                atlas,
                world,
                rng,
                status,
                &P::Poof,
                gate,
                pos,
                Vec3::new(x as f32, 0.005, z as f32),
                eye,
            );
            i += 1;
        }
        return;
    }
    build(
        atlas,
        live,
        rng,
        sparks,
        pos,
        emit.velocity,
        &emit.explosions[0],
        1,
    );
    if emit.explosions.len() > 1 && shows.len() < MAX_SHOWS {
        shows.push(Show {
            pos,
            velocity: emit.velocity,
            explosions: emit.explosions.clone(),
            next: 1,
            wait: 3,
        });
    }
}

pub(super) fn step_shows(
    shows: &mut Vec<Show>,
    atlas: &Atlas,
    live: &mut Vec<Particle>,
    rng: &mut JavaRandom,
    sparks: &mut u32,
) {
    shows.retain_mut(|show| {
        show.wait -= 1;
        if show.wait > 0 {
            return true;
        }
        build(
            atlas,
            live,
            rng,
            sparks,
            show.pos,
            show.velocity,
            &show.explosions[show.next],
            0,
        );
        show.next += 1;
        show.wait = 2;
        show.next < show.explosions.len()
    });
}

#[allow(clippy::too_many_arguments)]
fn build(
    atlas: &Atlas,
    live: &mut Vec<Particle>,
    rng: &mut JavaRandom,
    sparks: &mut u32,
    pos: DVec3,
    velocity: [f64; 3],
    explosion: &FireworkExplosion,
    delay: u32,
) {
    if live.len() >= MAX_LIVE_PARTICLES {
        return;
    }
    let count = sparks_in(explosion.shape);
    let fits = *sparks + count <= FIREWORK_SPARK_LIMIT;
    if fits {
        *sparks += count;
    }
    Starter {
        atlas,
        live,
        rng,
        pos,
        velocity,
        explosion,
        delay,
    }
    .explode(fits);
}

const fn sparks_in(shape: Shape) -> u32 {
    match shape {
        Shape::SmallBall => 9 * 2 + 16 * 5,
        Shape::LargeBall => 49 * 2 + 32 * 9,
        Shape::Star => 1 + 3 * 5 * 4 * 2,
        Shape::Creeper => 1 + 3 * 11 * 4 * 2,
        Shape::Burst => 70,
    }
}

struct Starter<'a> {
    atlas: &'a Atlas,
    live: &'a mut Vec<Particle>,
    rng: &'a mut JavaRandom,
    pos: DVec3,
    velocity: [f64; 3],
    explosion: &'a FireworkExplosion,
    delay: u32,
}

impl Starter<'_> {
    fn explode(&mut self, sparks: bool) {
        if sparks {
            match self.explosion.shape {
                Shape::SmallBall => self.ball(0.25, 2),
                Shape::LargeBall => self.ball(0.5, 4),
                Shape::Star => self.shape(0.5, &STAR_COORDS, false),
                Shape::Creeper => self.shape(0.5, &CREEPER_COORDS, true),
                Shape::Burst => self.burst(),
            }
        }
        let color = self.colors()[0] as u32;
        let rgba = [(color >> 16) as u8, (color >> 8) as u8, color as u8, 0xFF];
        if let Some(mut flash) = flash(self.atlas, rgba, self.rng) {
            flash.delay_ticks = self.delay;
            push_recipe(self.live, self.rng, flash, self.pos);
        }
    }

    fn colors(&self) -> &[i32] {
        if self.explosion.colors.is_empty() {
            &[BLACK_FIREWORK]
        } else {
            &self.explosion.colors
        }
    }

    fn spark(&mut self, velocity: [f64; 3]) {
        if self.live.len() >= MAX_LIVE_PARTICLES {
            return;
        }
        let velocity = Vec3::new(velocity[0] as f32, velocity[1] as f32, velocity[2] as f32);
        let Some(mut recipe) = spark_recipe(self.atlas, velocity, self.rng) else {
            return;
        };
        let pick = self.rng.next_int(self.colors().len() as u32) as usize;
        let rgb = self.colors()[pick] as u32;
        let explosion = self.explosion;
        let fades = &explosion.fade_colors;
        let fade = (!fades.is_empty())
            .then(|| fades[self.rng.next_int(fades.len() as u32) as usize] as u32);
        style_spark(
            &mut recipe,
            rgb,
            fade,
            explosion.has_trail,
            explosion.has_twinkle,
        );
        recipe.delay_ticks = self.delay;
        push_recipe(self.live, self.rng, recipe, self.pos);
    }

    fn ball(&mut self, base_speed: f64, steps: i32) {
        let mut y_step = -steps;
        while y_step <= steps {
            let mut x_step = -steps;
            while x_step <= steps {
                let mut z_step = -steps;
                while z_step <= steps {
                    let xa = f64::from(x_step) + (self.rng.next_f64() - self.rng.next_f64()) * 0.5;
                    let ya = f64::from(y_step) + (self.rng.next_f64() - self.rng.next_f64()) * 0.5;
                    let za = f64::from(z_step) + (self.rng.next_f64() - self.rng.next_f64()) * 0.5;
                    let len = (xa * xa + ya * ya + za * za).sqrt() / base_speed
                        + self.rng.next_gaussian() * 0.05;
                    self.spark([xa / len, ya / len, za / len]);
                    if y_step != -steps && y_step != steps && x_step != -steps && x_step != steps {
                        z_step += steps * 2 - 1;
                    }
                    z_step += 1;
                }
                x_step += 1;
            }
            y_step += 1;
        }
    }

    fn shape(&mut self, base_speed: f64, coords: &[[f64; 2]], flat: bool) {
        let [sx, sy] = coords[0];
        self.spark([sx * base_speed, sy * base_speed, 0.0]);
        let base_angle = self.rng.next_f32() * std::f32::consts::PI;
        let angle_mod = if flat { 0.034 } else { 0.34 };
        for angle_step in 0..3 {
            let angle = f64::from(base_angle)
                + f64::from(angle_step as f32 * std::f32::consts::PI) * angle_mod;
            let (sin, cos) = angle.sin_cos();
            let (mut ox, mut oy) = (sx, sy);
            for &[tx, ty] in &coords[1..] {
                let mut sub_step = 0.25;
                while sub_step <= 1.0 {
                    let mut xa = (ox + sub_step * (tx - ox)) * base_speed;
                    let ya = (oy + sub_step * (ty - oy)) * base_speed;
                    let za = xa * sin;
                    xa *= cos;
                    for flip in [-1.0, 1.0] {
                        self.spark([xa * flip, ya, za * flip]);
                    }
                    sub_step += 0.25;
                }
                ox = tx;
                oy = ty;
            }
        }
    }

    fn burst(&mut self) {
        let base_x = self.rng.next_gaussian() * 0.05;
        let base_z = self.rng.next_gaussian() * 0.05;
        for _ in 0..70 {
            let xa = self.velocity[0] * 0.5 + self.rng.next_gaussian() * 0.15 + base_x;
            let za = self.velocity[2] * 0.5 + self.rng.next_gaussian() * 0.15 + base_z;
            let ya = self.velocity[1] * 0.5 + self.rng.next_f64() * 0.5;
            self.spark([xa, ya, za]);
        }
    }
}
