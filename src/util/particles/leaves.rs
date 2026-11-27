#[allow(unused_imports)]
use super::*;

pub(super) fn recipe(
    particle: &P,
    atlas: &Atlas,
    arg: Vec3,
    rng: &mut JavaRandom,
    ctx: &SpawnCtx,
) -> Option<Option<Recipe>> {
    let name = particle_name(particle);
    Some(match particle {
        P::CherryLeaves => falling_leaf(atlas, name, rng, CHERRY),
        P::PaleOakLeaves => falling_leaf(atlas, name, rng, PALE_OAK),
        P::TintedLeaves(c) => falling_leaf(atlas, name, rng, PALE_OAK).map(|mut r| {
            let [red, green, blue] = [c.color.red(), c.color.green(), c.color.blue()].map(unit);
            r.tint = [red, green, blue, 1.0];
            r
        }),
        P::FallingDust(b) => falling_dust(atlas, name, b.block_state, rng, ctx),
        P::Snowflake => snowflake(atlas, name, arg, rng),
        P::Firefly => firefly(atlas, name, arg, rng),
        _ => return None,
    })
}

#[derive(Clone, Copy)]
struct Leaf {
    fall_acceleration: f32,
    side_acceleration: f32,
    swirl: bool,
    flow_away: bool,
    scale: f32,
    start_velocity: f32,
}

const CHERRY: Leaf = Leaf {
    fall_acceleration: 0.25,
    side_acceleration: 2.0,
    swirl: false,
    flow_away: true,
    scale: 1.0,
    start_velocity: 0.0,
};

const PALE_OAK: Leaf = Leaf {
    fall_acceleration: 0.07,
    side_acceleration: 10.0,
    swirl: true,
    flow_away: false,
    scale: 2.0,
    start_velocity: 0.021,
};

const LEAF_LIFETIME: u32 = 300;
const LEAF_ACCELERATION_SCALE: f64 = 0.0025f32 as f64;

const LEAF_ROT_SPEED: usize = 0;
const LEAF_SPIN_ACCELERATION: usize = 1;
const LEAF_WIND_BIG: usize = 2;
const LEAF_XA_FLOW_SCALE: usize = 3;
const LEAF_ZA_FLOW_SCALE: usize = 4;
const LEAF_SWIRL_PERIOD: usize = 5;
const LEAF_SWIRL: usize = 6;
const LEAF_FLOW_AWAY: usize = 7;

static FALLING_LEAF: Behavior = Behavior {
    tick: Some(leaf_tick),
    ..Behavior::NONE
};

fn falling_leaf(atlas: &Atlas, name: &str, rng: &mut JavaRandom, leaf: Leaf) -> Option<Recipe> {
    let frame = random_frame(atlas, name, rng)?;
    let mut r = base_recipe(Sprite::Fixed(frame), rng);
    let rot_degrees: f32 = if rng.next_bool() { -30.0 } else { 30.0 };
    let spin_degrees: f32 = if rng.next_bool() { -5.0 } else { 5.0 };
    let rot_speed = rot_degrees.to_radians();
    let spin_acceleration = spin_degrees.to_radians();
    r.lifetime = LEAF_LIFETIME;
    r.gravity = leaf.fall_acceleration * 1.2 * 0.0025;
    r.size = leaf.scale * if rng.next_bool() { 0.05 } else { 0.075 };
    r.friction = Vec3::ONE;
    r.velocity = Vec3::new(0.0, -leaf.start_velocity, 0.0);
    let particle_random = f64::from(rng.next_f32());
    let flow_angle = (particle_random * 60.0).to_radians();
    let wind_big = f64::from(leaf.side_acceleration);
    r.data[LEAF_ROT_SPEED] = rot_speed;
    r.data[LEAF_SPIN_ACCELERATION] = spin_acceleration;
    r.data[LEAF_WIND_BIG] = leaf.side_acceleration;
    r.data[LEAF_XA_FLOW_SCALE] = (flow_angle.cos() * wind_big) as f32;
    r.data[LEAF_ZA_FLOW_SCALE] = (flow_angle.sin() * wind_big) as f32;
    r.data[LEAF_SWIRL_PERIOD] = (1000.0 + particle_random * 3000.0).to_radians() as f32;
    r.data[LEAF_SWIRL] = if leaf.swirl { 1.0 } else { 0.0 };
    r.data[LEAF_FLOW_AWAY] = if leaf.flow_away { 1.0 } else { 0.0 };
    r.flags = PHYSICS;
    r.behavior = Some(&FALLING_LEAF);
    Some(r)
}

fn leaf_tick(p: &mut Particle, ctx: &mut TickCtx<'_>) {
    let alive_ticks = p.age as f32;
    let relative_age = f64::from((alive_ticks / LEAF_LIFETIME as f32).min(1.0));
    let wind_big = f64::from(p.data[LEAF_WIND_BIG]);
    let (mut xa, mut za) = (0.0f64, 0.0f64);
    if p.data[LEAF_FLOW_AWAY] != 0.0 {
        let curve = relative_age.powf(1.25);
        xa += f64::from(p.data[LEAF_XA_FLOW_SCALE]) * curve;
        za += f64::from(p.data[LEAF_ZA_FLOW_SCALE]) * curve;
    }
    if p.data[LEAF_SWIRL] != 0.0 {
        let turn = relative_age * f64::from(p.data[LEAF_SWIRL_PERIOD]);
        xa += relative_age * turn.cos() * wind_big;
        za += relative_age * turn.sin() * wind_big;
    }
    p.velocity.x += (xa * LEAF_ACCELERATION_SCALE) as f32;
    p.velocity.z += (za * LEAF_ACCELERATION_SCALE) as f32;
    p.velocity.y -= p.gravity;
    p.data[LEAF_ROT_SPEED] += p.data[LEAF_SPIN_ACCELERATION] / 20.0;
    p.roll += p.data[LEAF_ROT_SPEED] / 20.0;
    let delta = p.velocity;
    let moved = ctx.move_by(p, delta);
    if moved.on_ground || (p.age >= 2 && (p.velocity.x == 0.0 || p.velocity.z == 0.0)) {
        p.remove();
    }
}

static FALLING_DUST: Behavior = Behavior {
    tick: Some(dust_tick),
    ..Behavior::NONE
};

const DUST_ROT_SPEED: usize = 0;

fn falling_dust(
    atlas: &Atlas,
    name: &str,
    state: azalea::block::BlockState,
    rng: &mut JavaRandom,
    ctx: &SpawnCtx,
) -> Option<Recipe> {
    if is_invisible(state) {
        return None;
    }
    let [red, green, blue] = dust_color(state, ctx);
    let sheet = atlas.sheet_index(name)?;
    let mut r = base_recipe(Sprite::Animated(sheet), rng);
    r.tint = [red, green, blue, 1.0];
    r.size *= 0.674_999_95;
    r.grow = Grow::In;
    let base_lifetime = (32.0 / (f64::from(rng.next_f32()) * 0.8 + 0.2)) as u32;
    r.lifetime = (base_lifetime as f32 * 0.9).max(1.0) as u32;
    r.data[DUST_ROT_SPEED] = (rng.next_f32() - 0.5) * 0.1;
    r.roll = rng.next_f32() * std::f32::consts::TAU;
    r.flags = PHYSICS;
    r.behavior = Some(&FALLING_DUST);
    Some(r)
}

fn dust_tick(p: &mut Particle, ctx: &mut TickCtx<'_>) {
    p.roll += std::f32::consts::PI * p.data[DUST_ROT_SPEED] * 2.0;
    if p.on_ground {
        p.prev_roll = 0.0;
        p.roll = 0.0;
    }
    let delta = p.velocity;
    ctx.move_by(p, delta);
    p.velocity.y = (p.velocity.y - 0.003).max(-0.14);
}

fn is_invisible(state: azalea::block::BlockState) -> bool {
    use azalea::block::BlockTrait;
    let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
    matches!(
        block.id(),
        "barrier"
            | "bubble_column"
            | "end_gateway"
            | "end_portal"
            | "light"
            | "water"
            | "lava"
            | "structure_void"
            | "moving_piston"
    )
}

fn dust_color(state: azalea::block::BlockState, ctx: &SpawnCtx) -> [f32; 3] {
    use azalea::block::BlockTrait;
    let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
    let id = block.id();
    match id {
        "sand" => return rgb(14_406_560),
        "red_sand" => return rgb(11_098_145),
        "gravel" => return rgb(-8_356_741i32 as u32),
        "dragon_egg" => return [0.0; 3],
        "anvil" | "chipped_anvil" | "damaged_anvil" => return map_color(6),
        _ => {}
    }
    if let Some(color) = tint_source_color(state, id, ctx) {
        return color;
    }
    block_map_color(id).unwrap_or([1.0; 3])
}

fn tint_source_color(
    state: azalea::block::BlockState,
    id: &str,
    ctx: &SpawnCtx,
) -> Option<[f32; 3]> {
    use crate::renderer::TintKind;
    use crate::util::biome_color::{Tints, with_tints};
    use crate::util::mth::{linear_to_srgb, srgb_byte_to_linear};

    match id {
        "grass_block" | "pink_petals" | "wildflowers" => return Some([1.0; 3]),
        "spruce_leaves" => return Some(rgb(-10_380_959i32 as u32)),
        "birch_leaves" => return Some(rgb(-8_345_771i32 as u32)),
        "lily_pad" => return Some(rgb(-14_647_248i32 as u32)),
        "attached_melon_stem" | "attached_pumpkin_stem" => return Some(rgb(-2_046_180i32 as u32)),
        _ => {}
    }
    let cell = ctx.cell();
    let plains = with_tints(|t| Tints::plains(t));
    let base: [u8; 3] = match crate::util::block_model::block_visual(state).tint_kind {
        TintKind::None => return None,
        TintKind::Redstone(_) | TintKind::Stem(_) => {
            return Some(tint_ratio(state, cell, ctx.world));
        }
        TintKind::Grass => [plains.grass[0], plains.grass[1], plains.grass[2]],
        TintKind::Foliage => [plains.foliage[0], plains.foliage[1], plains.foliage[2]],
        TintKind::DryFoliage => [
            plains.dry_foliage[0],
            plains.dry_foliage[1],
            plains.dry_foliage[2],
        ],
        TintKind::Water => [0x3F, 0x76, 0xE4],
    };
    let ratio = tint_ratio(state, cell, ctx.world);
    Some(std::array::from_fn(|i| {
        linear_to_srgb(srgb_byte_to_linear(base[i]) * ratio[i]).clamp(0.0, 1.0)
    }))
}

fn block_map_color(id: &str) -> Option<[f32; 3]> {
    match id {
        "suspicious_sand" => return Some(map_color(2)),
        "suspicious_gravel" => return Some(map_color(11)),
        "air" | "cave_air" | "void_air" => return Some([0.0; 3]),
        _ => {}
    }
    for suffix in [
        "_glazed_terracotta",
        "_concrete_powder",
        "_concrete",
        "_wool",
        "_carpet",
        "_stained_glass",
    ] {
        if let Some(dye) = id
            .strip_suffix(suffix)
            .and_then(crate::util::dye::id_by_name)
        {
            return Some(map_color(dye_map_color(dye)));
        }
    }
    if let Some(dye) = id
        .strip_suffix("_terracotta")
        .and_then(crate::util::dye::id_by_name)
    {
        return Some(map_color(36 + dye as u8));
    }
    None
}

fn dye_map_color(dye: usize) -> u8 {
    if dye == 0 { 8 } else { 14 + dye as u8 }
}

fn map_color(id: u8) -> [f32; 3] {
    let [r, g, b, _] = crate::util::map_color::rgba_from_packed_id((id << 2) | 2);
    [unit(r), unit(g), unit(b)]
}

fn rgb(color: u32) -> [f32; 3] {
    [(color >> 16) & 255, (color >> 8) & 255, color & 255].map(|c| c as f32 / 255.0)
}

fn snowflake(atlas: &Atlas, name: &str, arg: Vec3, rng: &mut JavaRandom) -> Option<Recipe> {
    let sheet = atlas.sheet_index(name)?;
    let mut r = base_recipe(Sprite::Animated(sheet), rng);
    r.gravity = 0.225;
    r.friction = Vec3::new(0.95, 0.9, 0.95);
    let mut jitter = || (rng.next_f32() * 2.0 - 1.0) * 0.05;
    r.velocity = arg + Vec3::new(jitter(), jitter(), jitter());
    r.size = 0.1 * (rng.next_f32() * rng.next_f32() + 1.0);
    r.lifetime = (16.0 / (f64::from(rng.next_f32()) * 0.8 + 0.2)) as u32 + 2;
    r.tint = [0.923, 0.964, 0.999, 1.0];
    r.flags = PHYSICS;
    Some(r)
}

static FIREFLY: Behavior = Behavior {
    tick: Some(firefly_tick),
    light: Some(firefly_light),
    ..Behavior::NONE
};

fn firefly(atlas: &Atlas, name: &str, arg: Vec3, rng: &mut JavaRandom) -> Option<Recipe> {
    let xa = (0.5 - rng.next_f64()) as f32;
    let ya = if rng.next_bool() { arg.y } else { -arg.y };
    let za = (0.5 - rng.next_f64()) as f32;
    let frame = random_frame(atlas, name, rng)?;
    let mut r = base_recipe(Sprite::Fixed(frame), rng);
    r.velocity = base_velocity(Vec3::new(xa, ya, za), rng) * 0.8;
    r.friction = Vec3::splat(0.96);
    r.size *= 0.75 * 1.5;
    r.lifetime = 200 + rng.next_int(101);
    r.tint[3] = 0.0;
    r.flags = PHYSICS | TRANSLUCENT | SPEED_UP_Y_BLOCKED;
    r.behavior = Some(&FIREFLY);
    Some(r)
}

fn firefly_tick(p: &mut Particle, ctx: &mut TickCtx<'_>) {
    ctx.default_tick(p);
    if !ctx.block(p.pos).is_none_or(crate::blocks::state_is_air) {
        p.remove();
        return;
    }
    p.tint[3] = firefly_fade(firefly_progress(p, p.age as f32), 0.3, 0.5);
    let rng = ctx.rng();
    if rng.next_f32() > 0.95 || p.age == 1 {
        p.velocity = Vec3::new(
            -0.05 + 0.1 * rng.next_f32(),
            -0.05 + 0.1 * rng.next_f32(),
            -0.05 + 0.1 * rng.next_f32(),
        );
    }
}

fn firefly_light(p: &Particle, partial: f32, _block: u8, _sky: u8) -> (u8, u8) {
    let fade = firefly_fade(firefly_progress(p, p.age as f32 + partial), 0.1, 0.3);
    let packed = (255.0 * fade) as u32;
    (((packed >> 4) & 15) as u8, 0)
}

fn firefly_progress(p: &Particle, age: f32) -> f32 {
    (age / p.lifetime as f32).clamp(0.0, 1.0)
}

fn firefly_fade(progress: f32, fade_in: f32, fade_out: f32) -> f32 {
    if progress >= 1.0 - fade_in {
        (1.0 - progress) / fade_in
    } else if progress <= fade_out {
        progress / fade_out
    } else {
        1.0
    }
}
