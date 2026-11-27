#[allow(unused_imports)]
use super::*;

use azalea::block::BlockState;

pub(super) fn recipe(
    particle: &P,
    atlas: &Atlas,
    arg: Vec3,
    rng: &mut JavaRandom,
    ctx: &SpawnCtx,
) -> Option<Option<Recipe>> {
    let name = particle_name(particle);
    let made = match particle {
        P::Block(b) => terrain_particle(atlas, b.block_state, arg, rng, ctx),
        P::BlockCrumble(b) => crumble(atlas, b.block_state, arg, rng, ctx),
        P::DustPillar(b) => dust_pillar(atlas, b.block_state, arg, rng, ctx),
        P::BlockMarker(b) => block_marker(atlas, b.block_state),
        P::Dust(dust) => dust_particle(atlas, name, arg, rng, unit_rgb(&dust.color), dust.scale),
        P::DustColorTransition(dust) => dust_color_transition(
            atlas,
            name,
            arg,
            rng,
            [unit_rgb(&dust.from), unit_rgb(&dust.to)],
            dust.scale,
        ),
        P::DustPlume => dust_plume(atlas, name, arg, rng),
        P::Crit | P::EnchantedHit | P::DamageIndicator => crit(particle, atlas, name, arg, rng),
        P::Spit => spit(atlas, name, arg, rng),
        P::PauseMobGrowth | P::ResetMobGrowth => {
            simple_vertical(atlas, name, arg, rng, matches!(particle, P::ResetMobGrowth))
        }
        P::Lava => lava(atlas, name, rng),
        P::Cloud | P::Sneeze => cloud(atlas, name, arg, rng, matches!(particle, P::Sneeze)),
        _ => return None,
    };
    Some(made)
}

fn spawns_terrain_particles(state: BlockState) -> bool {
    use azalea::block::BlockTrait;

    if state.is_air() {
        return false;
    }
    let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
    !matches!(block.id(), "moving_piston" | "barrier" | "structure_void")
}

fn block_sprite(atlas: &Atlas, state: BlockState) -> Option<([f32; 4], u32)> {
    use azalea::block::BlockTrait;

    if let Some(uv) = particle_uv(state) {
        return Some((uv, TERRAIN));
    }
    if state.is_air() {
        return None;
    }
    let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
    atlas.item_uv(block.id()).map(|uv| (uv, ITEM))
}

fn sprite_layer(state: BlockState) -> u32 {
    if crate::util::block_model::block_visual(state).translucent {
        TRANSLUCENT
    } else {
        0
    }
}

fn quarter_uv(tile: [f32; 4], rng: &mut JavaRandom) -> [f32; 4] {
    let [u0, v0, u1, v1] = tile;
    let (w, h) = ((u1 - u0) / 4.0, (v1 - v0) / 4.0);
    let (uo, vo) = (rng.next_f32() * 3.0, rng.next_f32() * 3.0);
    [
        u0 + (uo + 1.0) * w,
        v0 + vo * h,
        u0 + uo * w,
        v0 + (vo + 1.0) * h,
    ]
}

fn terrain_particle(
    atlas: &Atlas,
    state: BlockState,
    arg: Vec3,
    rng: &mut JavaRandom,
    ctx: &SpawnCtx,
) -> Option<Recipe> {
    if !spawns_terrain_particles(state) {
        return None;
    }
    let (tile, sheet) = block_sprite(atlas, state)?;
    let mut r = base_recipe(Sprite::Fixed(quarter_uv(tile, rng)), rng);
    r.velocity = base_velocity(arg, rng);
    r.gravity = 1.0;
    let ratio = tint_ratio(state, ctx.cell(), ctx.world);
    r.tint = [0.6 * ratio[0], 0.6 * ratio[1], 0.6 * ratio[2], 1.0];
    r.size /= 2.0;
    r.flags = PHYSICS | sheet | sprite_layer(state);
    Some(r)
}

fn crumble(
    atlas: &Atlas,
    state: BlockState,
    arg: Vec3,
    rng: &mut JavaRandom,
    ctx: &SpawnCtx,
) -> Option<Recipe> {
    let mut r = terrain_particle(atlas, state, arg, rng, ctx)?;
    r.velocity = Vec3::ZERO;
    r.lifetime = rng.next_int(10) + 1;
    Some(r)
}

fn dust_pillar(
    atlas: &Atlas,
    state: BlockState,
    arg: Vec3,
    rng: &mut JavaRandom,
    ctx: &SpawnCtx,
) -> Option<Recipe> {
    let mut r = terrain_particle(atlas, state, arg, rng, ctx)?;
    let x = rng.next_gaussian() / 30.0;
    let y = f64::from(arg.y) + rng.next_gaussian() / 2.0;
    let z = rng.next_gaussian() / 30.0;
    r.velocity = Vec3::new(x as f32, y as f32, z as f32);
    r.lifetime = rng.next_int(20) + 20;
    Some(r)
}

fn block_marker(atlas: &Atlas, state: BlockState) -> Option<Recipe> {
    let (tile, sheet) = block_sprite(atlas, state)?;
    let mut r = blank_recipe(Sprite::Fixed(tile));
    r.lifetime = 80;
    r.size = 0.5;
    r.gravity = 0.0;
    r.flags = sheet | sprite_layer(state);
    Some(r)
}

fn dust_base(
    atlas: &Atlas,
    name: &str,
    arg: Vec3,
    rng: &mut JavaRandom,
    scale: f32,
) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
    let scale = scale.clamp(0.01, 4.0);
    r.velocity = base_velocity(arg, rng) * 0.1;
    r.friction = Vec3::splat(0.96);
    r.gravity = 0.0;
    r.size *= 0.75 * scale;
    let base_lifetime = (8.0 / (rng.next_f64() * 0.8 + 0.2)) as u32;
    r.lifetime = (base_lifetime as f32 * scale).max(1.0) as u32;
    r.grow = Grow::In;
    r.flags = PHYSICS | SPEED_UP_Y_BLOCKED;
    Some(r)
}

fn randomize_color(rng: &mut JavaRandom, color: [f32; 3], base: f32) -> [f32; 3] {
    color.map(|c| (rng.next_f32() * 0.2 + 0.8) * c * base)
}

fn dust_particle(
    atlas: &Atlas,
    name: &str,
    arg: Vec3,
    rng: &mut JavaRandom,
    color: [f32; 3],
    scale: f32,
) -> Option<Recipe> {
    let mut r = dust_base(atlas, name, arg, rng, scale)?;
    let base = rng.next_f32() * 0.4 + 0.6;
    let [cr, cg, cb] = randomize_color(rng, color, base);
    r.tint = [cr, cg, cb, 1.0];
    Some(r)
}

fn dust_color_transition(
    atlas: &Atlas,
    name: &str,
    arg: Vec3,
    rng: &mut JavaRandom,
    [from, to]: [[f32; 3]; 2],
    scale: f32,
) -> Option<Recipe> {
    let mut r = dust_base(atlas, name, arg, rng, scale)?;
    let base = rng.next_f32() * 0.4 + 0.6;
    let from = randomize_color(rng, from, base);
    let to = randomize_color(rng, to, base);
    r.data[..3].copy_from_slice(&from);
    r.data[3..6].copy_from_slice(&to);
    r.tint = [from[0], from[1], from[2], 1.0];
    r.behavior = Some(&DUST_COLOR_TRANSITION);
    Some(r)
}

static DUST_COLOR_TRANSITION: Behavior = Behavior {
    tint: Some(dust_color_transition_tint),
    ..Behavior::NONE
};

fn dust_color_transition_tint(p: &Particle, partial: f32) -> [f32; 4] {
    let a = (p.age as f32 + partial) / (p.lifetime as f32 + 1.0);
    let lerp = |i: usize| p.data[i] + (p.data[i + 3] - p.data[i]) * a;
    [lerp(0), lerp(1), lerp(2), p.tint[3]]
}

static DUST_PLUME: Behavior = Behavior {
    tick: Some(dust_plume_tick),
    ..Behavior::NONE
};

fn dust_plume_tick(p: &mut Particle, ctx: &mut TickCtx<'_>) {
    p.gravity *= 0.88;
    p.friction *= 0.92;
    ctx.default_tick(p);
}

fn dust_plume(atlas: &Atlas, name: &str, arg: Vec3, rng: &mut JavaRandom) -> Option<Recipe> {
    let ash = AshSmoke {
        dir: Vec3::new(0.7, 0.6, 0.7),
        scale: 1.0,
        color_random: 0.5,
        max_lifetime: 7.0,
        gravity: 0.5,
        physics: false,
    };
    let mut r = ash_smoke_recipe(atlas, name, arg + Vec3::new(0.0, 0.15, 0.0), rng, ash)?;
    let shift = rng.next_f32() * 0.2;
    r.tint = [
        unit(0xBA) - shift,
        unit(0xB1) - shift,
        unit(0xC2) - shift,
        1.0,
    ];
    r.behavior = Some(&DUST_PLUME);
    Some(r)
}

fn crit(
    particle: &P,
    atlas: &Atlas,
    name: &str,
    arg: Vec3,
    rng: &mut JavaRandom,
) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Fixed(random_frame(atlas, name, rng)?), rng);
    let arg = if matches!(particle, P::DamageIndicator) {
        arg + Vec3::Y
    } else {
        arg
    };
    let mut velocity = base_velocity(Vec3::ZERO, rng) * 0.1 + arg * 0.4;
    let grey = rng.next_f32() * 0.3 + 0.6;
    r.tint = [grey, grey, grey, 1.0];
    r.size *= 0.75;
    r.lifetime = ((6.0 / (f64::from(rng.next_f32()) * 0.8 + 0.6)) as u32).max(1);
    r.gravity = 0.5;
    r.friction = Vec3::splat(0.7);
    r.grow = Grow::In;
    r.flags = CRIT_DECAY;

    velocity.y -= 0.04 * r.gravity;
    r.start_offset = velocity.as_dvec3();
    r.pre_ticked = true;
    r.velocity = velocity * 0.7;
    r.tint[1] *= 0.96;
    r.tint[2] *= 0.9;
    r.start_age = 1;

    match particle {
        P::DamageIndicator => r.lifetime = 20,
        P::EnchantedHit => {
            r.tint[0] *= 0.3;
            r.tint[1] *= 0.8;
        }
        _ => {}
    }
    Some(r)
}

fn spit(atlas: &Atlas, name: &str, arg: Vec3, rng: &mut JavaRandom) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
    let mut spread = || (rng.next_f32() * 2.0 - 1.0) * 0.05;
    r.velocity = arg + Vec3::new(spread(), spread(), spread());
    let grey = rng.next_f32() * 0.3 + 0.7;
    r.tint = [grey, grey, grey, 1.0];
    r.size = 0.1 * (rng.next_f32() * rng.next_f32() * 6.0 + 1.0);
    r.lifetime = (16.0 / (f64::from(rng.next_f32()) * 0.8 + 0.2)) as u32 + 2;
    r.gravity = 0.5;
    r.friction = Vec3::splat(0.9);
    r.flags = PHYSICS;
    Some(r)
}

fn simple_vertical(
    atlas: &Atlas,
    name: &str,
    arg: Vec3,
    rng: &mut JavaRandom,
    upwards: bool,
) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Fixed(random_frame(atlas, name, rng)?), rng);
    r.velocity = arg + Vec3::new(0.0, if upwards { 0.03 } else { -0.03 }, 0.0);
    r.gravity = 0.0;
    r.size *= rng.next_f32() * 0.6 + 0.5;
    r.lifetime = 8;
    Some(r)
}

static LAVA: Behavior = Behavior {
    tick: Some(lava_tick),
    ..Behavior::NONE
};

fn lava_tick(p: &mut Particle, ctx: &mut TickCtx<'_>) {
    ctx.default_tick(p);
    let odds = p.age as f32 / p.lifetime as f32;
    if ctx.rng().next_f32() > odds {
        ctx.spawn(P::Smoke, p.pos, p.velocity);
    }
}

fn lava(atlas: &Atlas, name: &str, rng: &mut JavaRandom) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Fixed(random_frame(atlas, name, rng)?), rng);
    let drift = base_velocity(Vec3::ZERO, rng) * 0.8;
    r.velocity = Vec3::new(drift.x, rng.next_f32() * 0.4 + 0.05, drift.z);
    r.gravity = 0.75;
    r.friction = Vec3::splat(0.999);
    r.size *= rng.next_f32() * 2.0 + 0.2;
    r.lifetime = (16.0 / (f64::from(rng.next_f32()) * 0.8 + 0.2)) as u32;
    r.grow = Grow::Lava;
    r.flags = PHYSICS;
    r.light = Light::Block;
    r.behavior = Some(&LAVA);
    Some(r)
}

static CLOUD: Behavior = Behavior {
    tick: Some(cloud_tick),
    ..Behavior::NONE
};

fn cloud_tick(p: &mut Particle, ctx: &mut TickCtx<'_>) {
    ctx.default_tick(p);
    if let Some((feet, velocity)) = ctx.nearest_player(p.pos, 2.0)
        && p.pos.y > feet.y
    {
        p.pos.y += (feet.y - p.pos.y) * 0.2;
        p.velocity.y += (velocity.y - p.velocity.y) * 0.2;
    }
}

fn cloud(
    atlas: &Atlas,
    name: &str,
    arg: Vec3,
    rng: &mut JavaRandom,
    sneeze: bool,
) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
    r.velocity = base_velocity(Vec3::ZERO, rng) * 0.1 + arg;
    let grey = 1.0 - rng.next_f32() * 0.3;
    r.tint = if sneeze {
        [0.22, 1.0, 0.53, 0.4]
    } else {
        [grey, grey, grey, 1.0]
    };
    r.size *= 1.875;
    let base_lifetime = (8.0 / (f64::from(rng.next_f32()) * 0.8 + 0.3)) as u32;
    r.lifetime = (base_lifetime as f32 * 2.5).max(1.0) as u32;
    r.friction = Vec3::splat(0.96);
    r.gravity = 0.0;
    r.grow = Grow::In;
    r.flags = TRANSLUCENT;
    r.behavior = Some(&CLOUD);
    Some(r)
}
