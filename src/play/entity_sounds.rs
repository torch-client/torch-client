use azalea::block::BlockState;
use azalea::block::fluid_state::FluidKind;
use azalea::ecs::entity::Entity;
use azalea::ecs::query::QueryState;
use azalea::ecs::world::{World, WorldId};
use azalea::entity::dimensions::EntityDimensions;
use azalea::entity::metadata::{
    AbstractAgeableBaby, AttackTarget, BeeAngerEndTime, ClientAngerLevel, CopperGolemState, Creepy,
    FireworksItem, Health, Invisible, NoAi, Phase, Silent, SnifferState, StaredAt, WeatherState,
};
use azalea::entity::{
    CopperGolemStateKind, EntityKindComponent, LookDirection, Physics, Position, SnifferStateKind,
    WeatheringCopperStateKind,
};
use azalea::physics::collision::BlockWithShape;
use azalea::physics::collision::sturdy::is_face_sturdy;
use azalea::prelude::*;
use azalea_core::aabb::Aabb;
use azalea_core::direction::Direction;
use azalea_core::entity_id::MinecraftEntityId;
use azalea_core::position::BlockPos;
use azalea_inventory::components::{FireworkExplosionShape, Fireworks};
use azalea_registry::builtin::{BlockKind, EntityKind};
use azalea_world::{World as Level, WorldName};
use bevy::platform::collections::HashMap;

use crate::audio::{self, Placed, SoundCategory, Tracked};
use crate::blocks::{sound_types, state_is_air};
use crate::play::riding::{Hull, hull_under_water};
use crate::renderer::dimension::{self, Dimension};
use crate::session::SideMutex;
use crate::util::global;
use crate::util::javarandom::JavaRandom;
use crate::util::mth;

const MIN_SPEED: f32 = 0.01;
const LOOP_RANGE: f64 = 16.0;
const MAX_LOOPS: usize = 8;
const MAX_BOUND: usize = 16;
const MAX_TWINKLES: usize = 32;
const CART_PITCH_DELTA: f32 = 0.0025;
const STARE_TICKS: u64 = 400;
const WARDEN_ANGRY: f32 = 80.0;

type EntityQuery = QueryState<(
    Entity,
    &'static EntityKindComponent,
    &'static WorldName,
    &'static MinecraftEntityId,
    &'static Position,
)>;

#[derive(Default)]
struct Mob {
    seen: u64,
    age: i32,
    creepy: bool,
    last_stare: Option<u64>,
    growl: i32,
    flap: f32,
    old_flap: f32,
    target: i32,
    attack_time: i32,
    attacking: bool,
    touched: bool,
    sound_tick: i32,
    ambient: i32,
    sniffer: SnifferStateKind,
    dig_start: i32,
    idle_start: i32,
    hurt: i32,
}

struct Voice {
    placed: Placed,
    event: &'static str,
    seen: u64,
    sent: ([f64; 3], f32, f32),
}

struct Hum {
    event: &'static str,
    category: SoundCategory,
    flat: bool,
    volume: f32,
    pitch: Option<f32>,
}

#[derive(Default)]
struct State {
    query: Option<(WorldId, EntityQuery)>,
    rng: Option<JavaRandom>,
    tick: u64,
    mobs: HashMap<i32, Mob>,
    loops: HashMap<i32, Voice>,
    flash: f32,
    flashes: Vec<Flash>,
}

global! {
fn state() -> SideMutex<State>
}

struct Flash {
    delay: u32,
    angles: (f32, f32),
    voice: Option<Tracked>,
}

struct Rocket {
    at: [f64; 3],
    large: bool,
    twinkle: Option<u32>,
}

struct Twinkle {
    at: [f64; 3],
    ticks: u32,
}

struct Bound {
    voice: Tracked,
    entity: Entity,
    volume: f32,
    pitch: f32,
    dig: bool,
}

pub(crate) struct Sound<'a> {
    pub(crate) event: &'a str,
    pub(crate) category: SoundCategory,
    pub(crate) volume: f32,
    pub(crate) pitch: f32,
    pub(crate) seed: i64,
}

#[derive(Default)]
struct Inbox {
    rockets: HashMap<i32, Rocket>,
    twinkles: Vec<Twinkle>,
    bound: Vec<Bound>,
    attacks: Vec<i32>,
    hurts: Vec<i32>,
}

global! {
fn inbox() -> SideMutex<Inbox>
}

pub(crate) fn reset() {
    if let Ok(mut state) = state().try_lock() {
        state.mobs.clear();
        state.flash = 0.0;
        for (_, voice) in state.loops.drain() {
            audio::end_loop(voice.placed.id());
        }
        for voice in state.flashes.drain(..).filter_map(|flash| flash.voice) {
            audio::end_loop(voice.id());
        }
    }
    let mut inbox = inbox().lock().unwrap();
    for sound in inbox.bound.drain(..) {
        audio::end_loop(sound.voice.id());
    }
    *inbox = Inbox::default();
}

fn far_from_ears(at: [f64; 3]) -> bool {
    let (ears, ..) = audio::listener();
    let [x, y, z] = [0, 1, 2].map(|i| at[i] - ears[i]);
    x * x + y * y + z * z >= 256.0
}

pub(crate) fn firework_blast(id: i32) {
    let mut inbox = inbox().lock().unwrap();
    let Some(rocket) = inbox.rockets.remove(&id) else {
        return;
    };
    if let Some(ticks) = rocket.twinkle
        && inbox.twinkles.len() < MAX_TWINKLES
    {
        inbox.twinkles.push(Twinkle {
            at: rocket.at,
            ticks,
        });
    }
    let event = match (rocket.large, far_from_ears(rocket.at)) {
        (true, true) => "entity.firework_rocket.large_blast_far",
        (true, false) => "entity.firework_rocket.large_blast",
        (false, true) => "entity.firework_rocket.blast_far",
        (false, false) => "entity.firework_rocket.blast",
    };
    let pitch = 0.95 + audio::random_f32() * 0.1;
    audio::play_at_delayed(event, SoundCategory::Ambient, rocket.at, 20.0, pitch);
}

fn read_rocket(ecs: &World, entity: Entity, at: [f64; 3]) -> Option<Rocket> {
    let fireworks = ecs
        .get::<FireworksItem>(entity)?
        .0
        .get_component::<Fireworks>()?;
    let explosions = &fireworks.explosions;
    let large = explosions.len() >= 3
        || explosions
            .iter()
            .any(|e| matches!(e.shape, FireworkExplosionShape::LargeBall));
    let twinkle = explosions
        .iter()
        .any(|e| e.has_twinkle)
        .then_some(explosions.len() as u32 * 2 + 14);
    (!explosions.is_empty()).then_some(Rocket { at, large, twinkle })
}

fn bind(bound: &mut Vec<Bound>, entity: Entity, at: [f64; 3], sound: Sound<'_>, dig: bool) {
    let Sound {
        event,
        category,
        volume,
        pitch,
        seed,
    } = sound;
    let voice = (bound.len() < MAX_BOUND)
        .then(|| audio::start_bound(event, category, at, volume, pitch, seed))
        .flatten();
    match voice {
        Some(voice) => bound.push(Bound {
            voice,
            entity,
            volume,
            pitch,
            dig,
        }),
        None => audio::play_seeded(event, category, at, volume, pitch, seed),
    }
}

pub(crate) fn follow(entity: Entity, at: [f64; 3], sound: Sound<'_>, dig: bool) {
    bind(&mut inbox().lock().unwrap().bound, entity, at, sound, dig);
}

pub(crate) fn guardian_attack(id: i32) {
    inbox().lock().unwrap().attacks.push(id);
}

pub(crate) fn hurt(id: i32) {
    inbox().lock().unwrap().hurts.push(id);
}

fn voice_pitch(rng: &mut JavaRandom) -> f32 {
    (rng.next_f32() - rng.next_f32()) * 0.2 + 1.0
}

fn horizontal_speed(ecs: &World, entity: Entity) -> f32 {
    ecs.get::<Physics>(entity)
        .map_or(0.0, |p| p.velocity.horizontal_distance() as f32)
}

fn wing(tick: i64) -> f64 {
    (tick as f64 * 7.448451 * 0.017453292 + std::f64::consts::PI).cos()
}

fn mob_of(mobs: &mut HashMap<i32, Mob>, id: i32, now: u64) -> &mut Mob {
    let mob = mobs.entry(id).or_insert_with(|| Mob {
        growl: 100,
        ..Mob::default()
    });
    mob.seen = now;
    mob.age += 1;
    mob
}

fn alive(ecs: &World, entity: Entity) -> bool {
    ecs.get::<Health>(entity)
        .is_none_or(|health| health.0 > 0.0)
}

fn direction(x_rot: f32, y_rot: f32) -> [f64; 3] {
    let (y_sin, y_cos) = (-y_rot * mth::DEG_TO_RAD - std::f32::consts::PI).sin_cos();
    let (x_sin, x_cos) = (-x_rot * mth::DEG_TO_RAD).sin_cos();
    [
        f64::from(y_sin * -x_cos),
        f64::from(x_sin),
        f64::from(y_cos * -x_cos),
    ]
}

fn touching_water(level: &Level, bounds: &Aabb) -> bool {
    const MARGIN: f64 = 0.001;
    let (min, max) = (bounds.min, bounds.max);
    for x in (min.x + MARGIN).floor() as i32..(max.x - MARGIN).ceil() as i32 {
        for y in (min.y + MARGIN).floor() as i32..(max.y - MARGIN).ceil() as i32 {
            for z in (min.z + MARGIN).floor() as i32..(max.z - MARGIN).ceil() as i32 {
                let cell = BlockPos::new(x, y, z);
                let Some(fluid) = level
                    .get_fluid_state(cell)
                    .filter(|f| f.kind == FluidKind::Water)
                else {
                    continue;
                };
                let height = level
                    .get_fluid_state(cell.up(1))
                    .map_or(fluid.height(), |up| fluid.height_under(&up));
                if f64::from(y) + f64::from(height) >= min.y + MARGIN {
                    return true;
                }
            }
        }
    }
    false
}

fn open_columns(level: &Level, bounds: &Aabb) -> u32 {
    const DEFLATE: f64 = 9.999999747378752e-6;
    let (min, max) = (bounds.min, bounds.max);
    let mut columns = 0;
    for x in (min.x + DEFLATE).floor() as i32..=(max.x - DEFLATE).floor() as i32 {
        for y in (min.y + DEFLATE).floor() as i32..=(max.y - DEFLATE).floor() as i32 {
            for z in (min.z + DEFLATE).floor() as i32..=(max.z - DEFLATE).floor() as i32 {
                let cell = BlockPos::new(x, y, z);
                let column = level
                    .get_block_state(cell)
                    .is_some_and(|s| s.as_block_kind() == BlockKind::BubbleColumn);
                let above = level.get_block_state(cell.up(1)).unwrap_or_default();
                let fluid = level.get_fluid_state(cell.up(1));
                if column && above.is_collision_shape_empty() && fluid.is_none_or(|f| f.is_empty())
                {
                    columns += 1;
                }
            }
        }
    }
    columns
}

fn invisible(state: BlockState) -> bool {
    use BlockKind as B;
    state_is_air(state)
        || matches!(
            state.as_block_kind(),
            B::Barrier
                | B::BubbleColumn
                | B::EndGateway
                | B::EndPortal
                | B::Lava
                | B::Light
                | B::MovingPiston
                | B::StructureVoid
                | B::Water
        )
}

fn drive(
    loops: &mut HashMap<i32, Voice>,
    id: i32,
    hum: Hum,
    at: [f64; 3],
    audible: bool,
    now: u64,
) {
    if let Some(voice) = loops.get_mut(&id) {
        if audible && voice.event == hum.event {
            voice.seen = now;
            let pitch = hum
                .pitch
                .unwrap_or((voice.sent.2 + CART_PITCH_DELTA).min(1.0));
            let next = (at, hum.volume, pitch);
            if next != voice.sent {
                audio::update_placed(&voice.placed, at, hum.volume, pitch);
                voice.sent = next;
            }
            return;
        }
        audio::end_loop(voice.placed.id());
        loops.remove(&id);
    }
    if audible && loops.len() < MAX_LOOPS {
        let pitch = hum.pitch.unwrap_or(CART_PITCH_DELTA);
        let start = if hum.flat {
            audio::start_flat_loop
        } else {
            audio::start_placed_loop
        };
        if let Some(placed) = start(hum.event, hum.category, hum.volume, pitch, at) {
            let sent = (at, hum.volume, pitch);
            loops.insert(
                id,
                Voice {
                    placed,
                    event: hum.event,
                    seen: now,
                    sent,
                },
            );
        }
    }
}

pub(crate) fn tick(bot: &Client, end_clock: i64) {
    use EntityKind as K;
    let Ok(origin) = bot.position() else { return };
    let world_name = {
        let Ok(name) = bot.component::<WorldName>() else {
            return;
        };
        name.clone()
    };
    let Ok(level) = bot.world() else { return };

    let mut guard = state().lock().unwrap();
    let State {
        query,
        rng,
        tick,
        mobs,
        loops,
        flash,
        flashes,
    } = &mut *guard;
    let query = crate::play::entity_feed::cached_query(bot, query);
    let rng = rng.get_or_insert_with(JavaRandom::time_seeded);
    *tick += 1;
    let now = *tick;
    let game_time = crate::client::tracking::game_time();

    let ecs = bot.ecs.read();
    let level = level.read();
    let mut inbox = inbox().lock().unwrap();
    let inbox = &mut *inbox;
    inbox.rockets.clear();
    for (entity, kind, name, id, pos) in query.iter(&ecs) {
        let minecart = matches!(
            kind.0,
            K::Minecart
                | K::ChestMinecart
                | K::CommandBlockMinecart
                | K::FurnaceMinecart
                | K::HopperMinecart
                | K::SpawnerMinecart
                | K::TntMinecart
        );
        let sounding = matches!(
            kind.0,
            K::FireworkRocket
                | K::Blaze
                | K::Enderman
                | K::Warden
                | K::EnderDragon
                | K::Phantom
                | K::Bee
                | K::Guardian
                | K::ElderGuardian
                | K::Breeze
                | K::CopperGolem
                | K::Sniffer
                | K::Illusioner
        );
        let boat = Hull::of(kind.0).is_some();
        if !(minecart || sounding || boat) || *name != world_name {
            continue;
        }
        let id = id.0;
        let at = [pos.x, pos.y, pos.z];
        let silent = ecs.get::<Silent>(entity).is_some_and(|s| s.0);
        let in_range = pos.distance_squared_to(origin) < LOOP_RANGE * LOOP_RANGE;
        match kind.0 {
            K::FireworkRocket => {
                if let Some(rocket) = read_rocket(&ecs, entity, at) {
                    inbox.rockets.insert(id, rocket);
                }
            }
            K::Blaze => {
                if rng.next_int(24) == 0 && !silent {
                    let place = at.map(|v| v + 0.5);
                    let (volume, pitch) = (1.0 + rng.next_f32(), rng.next_f32() * 0.7 + 0.3);
                    audio::play_at(
                        "entity.blaze.burn",
                        SoundCategory::Hostile,
                        place,
                        volume,
                        pitch,
                    );
                }
            }
            K::Enderman => {
                let mob = mob_of(mobs, id, now);
                let creepy = ecs.get::<Creepy>(entity).is_some_and(|c| c.0);
                let rising = creepy && !mob.creepy;
                mob.creepy = creepy;
                let stared = ecs.get::<StaredAt>(entity).is_some_and(|s| s.0);
                if rising && stared && mob.last_stare.is_none_or(|last| now >= last + STARE_TICKS) {
                    mob.last_stare = Some(now);
                    if !silent {
                        let eye = ecs
                            .get::<EntityDimensions>(entity)
                            .map_or(0.0, |d| f64::from(d.eye_height));
                        let place = [pos.x, pos.y + eye, pos.z];
                        audio::play_at(
                            "entity.enderman.stare",
                            SoundCategory::Hostile,
                            place,
                            2.5,
                            1.0,
                        );
                    }
                }
            }
            K::Warden => {
                let anger = ecs.get::<ClientAngerLevel>(entity).map_or(0, |a| a.0);
                let anger = mth::clamp(anger as f32 / WARDEN_ANGRY, 0.0, 1.0);
                let delay = 40 - (anger * 30.0).floor() as u64;
                if now % delay == 0 && !silent {
                    let pitch = voice_pitch(rng);
                    audio::play_at(
                        "entity.warden.heartbeat",
                        SoundCategory::Hostile,
                        at,
                        5.0,
                        pitch,
                    );
                }
            }
            K::EnderDragon => {
                let mob = mob_of(mobs, id, now);
                let (flap, old_flap) = (
                    (mob.flap * std::f32::consts::TAU).cos(),
                    (mob.old_flap * std::f32::consts::TAU).cos(),
                );
                if old_flap <= -0.3 && flap >= -0.3 && !silent {
                    let pitch = 0.8 + rng.next_f32() * 0.3;
                    audio::play_at(
                        "entity.ender_dragon.flap",
                        SoundCategory::Hostile,
                        at,
                        5.0,
                        pitch,
                    );
                }
                let sitting = ecs
                    .get::<Phase>(entity)
                    .is_none_or(|p| matches!(p.0, 5 | 6 | 7 | 10));
                if !silent && !sitting {
                    mob.growl -= 1;
                    if mob.growl < 0 {
                        let pitch = 0.8 + rng.next_f32() * 0.3;
                        audio::play_at(
                            "entity.ender_dragon.growl",
                            SoundCategory::Hostile,
                            at,
                            2.5,
                            pitch,
                        );
                        mob.growl = 200 + rng.next_int(200) as i32;
                    }
                }
                mob.old_flap = mob.flap;
                if alive(&ecs, entity) {
                    let rise = ecs.get::<Physics>(entity).map_or(0.0, |p| p.velocity.y);
                    let speed = 0.2 / (horizontal_speed(&ecs, entity) * 10.0 + 1.0)
                        * 2f64.powf(rise) as f32;
                    mob.flap += if sitting { 0.1 } else { speed };
                    if ecs.get::<NoAi>(entity).is_some_and(|n| n.0) {
                        mob.flap = 0.5;
                    }
                }
            }
            K::Guardian | K::ElderGuardian => {
                let mob = mob_of(mobs, id, now);
                let target = ecs.get::<AttackTarget>(entity).map_or(0, |t| t.0);
                let over = mob.target != 0 && target == 0;
                if target != mob.target {
                    mob.target = target;
                    mob.attack_time = 0;
                }
                let duration = if kind.0 == K::Guardian { 80 } else { 60 };
                if alive(&ecs, entity) {
                    if let Some(physics) = ecs.get::<Physics>(entity)
                        && !touching_water(&level, &physics.bounding_box)
                    {
                        let rise = physics.velocity.y;
                        if rise > 0.0 && mob.touched && !silent {
                            let flop = if kind.0 == K::Guardian {
                                "entity.guardian.flop"
                            } else {
                                "entity.elder_guardian.flop"
                            };
                            audio::play_at(flop, SoundCategory::Hostile, at, 1.0, 1.0);
                        }
                        let below = BlockPos::from(**pos).down(1);
                        let ground = level
                            .get_block_state(below)
                            .is_some_and(|s| is_face_sturdy(s, Direction::Up));
                        mob.touched = rise < 0.0 && ground;
                    }
                    if target != 0 && mob.attack_time < duration {
                        mob.attack_time += 1;
                    }
                }
                let scale = mob.attack_time as f32 / duration as f32;
                let hum = Hum {
                    event: "entity.guardian.attack",
                    category: SoundCategory::Hostile,
                    flat: true,
                    volume: scale * scale,
                    pitch: Some(0.7 + 0.5 * scale),
                };
                mob.attacking = !over && (mob.attacking || inbox.attacks.contains(&id));
                drive(loops, id, hum, at, mob.attacking && !silent, now);
            }
            K::Breeze => {
                let mob = mob_of(mobs, id, now);
                let bound = &mut inbox.bound;
                mob.sound_tick = if mob.sound_tick == 0 {
                    1 + rng.next_int(80) as i32
                } else {
                    mob.sound_tick - 1
                };
                if mob.sound_tick == 0 && !silent {
                    let (pitch, volume) = (0.7 + 0.4 * rng.next_f32(), 0.8 + 0.2 * rng.next_f32());
                    let sound = Sound {
                        event: "entity.breeze.whirl",
                        category: SoundCategory::Hostile,
                        volume,
                        pitch,
                        seed: audio::random_seed(),
                    };
                    bind(bound, entity, at, sound, false);
                }
                if alive(&ecs, entity) {
                    let due = (rng.next_int(1000) as i32) < mob.ambient;
                    mob.ambient += 1;
                    if due {
                        mob.ambient = -80;
                        let grounded = ecs.get::<Physics>(entity).is_some_and(|p| p.on_ground());
                        if !silent {
                            let sound = Sound {
                                event: if grounded {
                                    "entity.breeze.idle_ground"
                                } else {
                                    "entity.breeze.idle_air"
                                },
                                category: SoundCategory::Hostile,
                                volume: 1.0,
                                pitch: 1.0,
                                seed: audio::random_seed(),
                            };
                            bind(bound, entity, at, sound, false);
                        }
                    }
                }
            }
            K::CopperGolem => {
                let mob = mob_of(mobs, id, now);
                if ecs.get::<NoAi>(entity).is_some_and(|n| n.0) {
                    continue;
                }
                if ecs
                    .get::<CopperGolemState>(entity)
                    .is_some_and(|s| s.0 != CopperGolemStateKind::Idle)
                {
                    mob.idle_start = 0;
                } else if mob.idle_start == 0 {
                    mob.idle_start = mob.age + 200 + rng.next_int(40) as i32;
                } else if mob.age == mob.idle_start + 10 {
                    mob.idle_start = 0;
                    if !silent {
                        let weather = ecs.get::<WeatherState>(entity).map(|w| w.0);
                        let event = match weather {
                            Some(WeatheringCopperStateKind::Weathered) => {
                                "entity.copper_golem_weathered.spin"
                            }
                            Some(WeatheringCopperStateKind::Oxidized) => {
                                "entity.copper_golem_oxidized.spin"
                            }
                            _ => "entity.copper_golem.spin",
                        };
                        audio::play_at(event, SoundCategory::Neutral, at, 1.0, 1.0);
                    }
                }
            }
            K::Sniffer => {
                let mob = mob_of(mobs, id, now);
                let sniffer = ecs
                    .get::<SnifferState>(entity)
                    .map_or(SnifferStateKind::Idling, |s| s.0);
                if sniffer != mob.sniffer {
                    mob.sniffer = sniffer;
                    mob.dig_start = mob.age - 1;
                }
                match sniffer {
                    SnifferStateKind::Searching if mob.age % 20 == 0 => {
                        audio::play_at(
                            "entity.sniffer.searching",
                            SoundCategory::Neutral,
                            at,
                            1.0,
                            1.0,
                        );
                    }
                    SnifferStateKind::Digging if mob.age % 10 == 0 => {
                        let millis = (mob.age - mob.dig_start) * 50;
                        let Some(look) = ecs
                            .get::<LookDirection>(entity)
                            .filter(|_| millis > 1700 && millis < 6000)
                        else {
                            continue;
                        };
                        let [x, _, z] = direction(look.x_rot(), look.y_rot()).map(|v| v * 2.25);
                        let head_y = (pos.y + 0.20000000298023224).floor() as i32;
                        let below = BlockPos::new(
                            (pos.x + x).floor() as i32,
                            head_y - 1,
                            (pos.z + z).floor() as i32,
                        );
                        if let Some(state) = level.get_block_state(below).filter(|s| !invisible(*s))
                        {
                            let hit = sound_types::of(state).hit_event;
                            audio::play_at(hit, SoundCategory::Neutral, at, 0.5, 0.5);
                        }
                    }
                    _ => {}
                }
            }
            K::Illusioner => {
                let mob = mob_of(mobs, id, now);
                if inbox.hurts.contains(&id) {
                    mob.hurt = 10;
                }
                mob.hurt = (mob.hurt - 1).max(0);
                let hidden = ecs.get::<Invisible>(entity).is_some_and(|i| i.0);
                if hidden && (mob.hurt == 1 || mob.age % 1200 == 0) {
                    audio::play_at(
                        "entity.illusioner.mirror_move",
                        SoundCategory::Hostile,
                        at,
                        1.0,
                        1.0,
                    );
                }
            }
            K::Phantom => {
                let flap = i64::from(id) * 3 + now as i64;
                if wing(flap) > 0.0 && wing(flap + 1) <= 0.0 {
                    let (volume, pitch) =
                        (0.95 + rng.next_f32() * 0.05, 0.95 + rng.next_f32() * 0.05);
                    audio::play_at(
                        "entity.phantom.flap",
                        SoundCategory::Hostile,
                        at,
                        volume,
                        pitch,
                    );
                }
            }
            K::Bee => {
                let speed = horizontal_speed(&ecs, entity);
                let baby = ecs.get::<AbstractAgeableBaby>(entity).is_some_and(|b| b.0);
                let (min, max) = if baby { (1.1, 1.5) } else { (0.7, 1.1) };
                let angry = ecs
                    .get::<BeeAngerEndTime>(entity)
                    .is_some_and(|t| t.0 > 0 && t.0 > game_time);
                let hum = Hum {
                    event: if angry {
                        "entity.bee.loop_aggressive"
                    } else {
                        "entity.bee.loop"
                    },
                    category: SoundCategory::Neutral,
                    flat: false,
                    volume: mth::lerp(mth::clamp(speed, 0.0, 0.5), 0.0, 1.2),
                    pitch: Some(mth::lerp(mth::clamp(speed, min, max), min, max)),
                };
                drive(
                    loops,
                    id,
                    hum,
                    at,
                    !silent && in_range && speed >= MIN_SPEED,
                    now,
                );
            }
            _ if boat => {
                let Some(physics) = ecs.get::<Physics>(entity) else {
                    continue;
                };
                let columns = open_columns(&level, &physics.bounding_box);
                if columns > 0 && !hull_under_water(&level, &physics.bounding_box) {
                    for _ in 0..columns * 2 {
                        if rng.next_int(100) == 0 {
                            let pitch = 0.8 + 0.4 * rng.next_f32();
                            audio::play_at(
                                "entity.generic.splash",
                                SoundCategory::Neutral,
                                at,
                                1.0,
                                pitch,
                            );
                        }
                    }
                }
            }
            _ => {
                let speed = horizontal_speed(&ecs, entity);
                let volume = mth::lerp(mth::clamp(speed, 0.0, 0.5), 0.0, 0.7);
                let hum = Hum {
                    event: "entity.minecart.riding",
                    category: SoundCategory::Neutral,
                    flat: false,
                    volume,
                    pitch: None,
                };
                drive(
                    loops,
                    id,
                    hum,
                    at,
                    !silent && in_range && speed >= MIN_SPEED,
                    now,
                );
            }
        }
    }
    inbox.attacks.clear();
    inbox.hurts.clear();
    loops.retain(|_, voice| {
        let alive = voice.seen == now;
        if !alive {
            audio::end_loop(voice.placed.id());
        }
        alive
    });
    mobs.retain(|_, mob| mob.seen == now);

    inbox.bound.retain_mut(|sound| {
        let silent = ecs.get::<Silent>(sound.entity).is_some_and(|s| s.0);
        let digging = !sound.dig
            || ecs.get::<SnifferState>(sound.entity).is_some_and(|s| {
                matches!(s.0, SnifferStateKind::Searching | SnifferStateKind::Digging)
            });
        let playing = sound.voice.active() && digging && !silent;
        match ecs.get::<Position>(sound.entity).filter(|_| playing) {
            Some(pos) => {
                audio::update_placed(
                    sound.voice.placed(),
                    [pos.x, pos.y, pos.z],
                    sound.volume,
                    sound.pitch,
                );
                true
            }
            None => {
                audio::end_loop(sound.voice.id());
                false
            }
        }
    });

    inbox.twinkles.retain_mut(|twinkle| {
        if twinkle.ticks > 0 {
            twinkle.ticks -= 1;
            return true;
        }
        let event = if far_from_ears(twinkle.at) {
            "entity.firework_rocket.twinkle_far"
        } else {
            "entity.firework_rocket.twinkle"
        };
        let pitch = 0.9 + rng.next_f32() * 0.15;
        audio::play_at_delayed(event, SoundCategory::Ambient, twinkle.at, 20.0, pitch);
        false
    });

    let clock = end_clock;
    if dimension::current() == Dimension::End {
        let level = crate::renderer::endflash::intensity(clock);
        let started = level > 0.0 && *flash <= 0.0;
        *flash = level;
        if started && !audio::credits_up() {
            let params = crate::renderer::endflash::at(clock as f64);
            flashes.push(Flash {
                delay: 30,
                angles: (params.x_angle, params.y_angle),
                voice: None,
            });
        }
    } else {
        *flash = 0.0;
        flashes.retain(|queued| queued.voice.is_some());
    }
    let (ears, ..) = audio::listener();
    flashes.retain_mut(|queued| {
        let way = direction(queued.angles.0, queued.angles.1);
        let at = [0, 1, 2].map(|i| ears[i] + way[i] * 10.0);
        match &queued.voice {
            None if queued.delay > 0 => {
                queued.delay -= 1;
                true
            }
            None => {
                queued.voice = audio::start_flat("weather.end_flash", SoundCategory::Weather, at);
                queued.voice.is_some()
            }
            Some(voice) if voice.active() => {
                audio::update_placed(voice.placed(), at, 1.0, 1.0);
                true
            }
            Some(voice) => {
                audio::end_loop(voice.id());
                false
            }
        }
    });
}
