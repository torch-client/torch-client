use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

use azalea::entity::particle::Particle as P;

use crate::renderer::AppState;
use crate::renderer::entity_material::{
    EntityMaterial, EntityParams, LightMode, Lit, diffuse_lights,
};
use crate::renderer::systems::{ChunkMaterial, LightmapState, Shared, WorldCamera};
use crate::util::javarandom::JavaRandom;
use crate::util::particle_assets;

pub struct ParticlePlugin;

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ParticleMaterialCache>()
            .init_resource::<ParticleRng>()
            .init_resource::<LiveParticles>()
            .init_resource::<ParticleAtlas>()
            .init_resource::<ParticleBatches>()
            .init_resource::<AmbientClock>()
            .add_systems(OnEnter(AppState::InGame), load_particle_atlas)
            .add_systems(
                Update,
                (
                    spawn_particles,
                    spawn_crit_particles,
                    animate_tick_particles,
                    tick_particles,
                    draw_particles,
                )
                    .chain()
                    .after(crate::renderer::FrameViewSystems)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ParticleStatus {
    #[default]
    All,
    Decreased,
    Minimal,
}

impl ParticleStatus {
    pub const ALL: [ParticleStatus; 3] = [
        ParticleStatus::All,
        ParticleStatus::Decreased,
        ParticleStatus::Minimal,
    ];

    pub fn caption(self) -> &'static str {
        match self {
            ParticleStatus::All => "All",
            ParticleStatus::Decreased => "Decreased",
            ParticleStatus::Minimal => "Minimal",
        }
    }

    pub fn serialized_name(self) -> &'static str {
        match self {
            ParticleStatus::All => "all",
            ParticleStatus::Decreased => "decreased",
            ParticleStatus::Minimal => "minimal",
        }
    }

    pub fn from_serialized_name(name: &str) -> Option<ParticleStatus> {
        ParticleStatus::ALL
            .into_iter()
            .find(|s| s.serialized_name() == name)
    }

    pub fn next(self) -> ParticleStatus {
        let i = ParticleStatus::ALL
            .iter()
            .position(|s| *s == self)
            .unwrap_or(0);
        ParticleStatus::ALL[(i + 1) % ParticleStatus::ALL.len()]
    }
}

fn allow_particle(status: ParticleStatus, always_show: bool, rng: &mut JavaRandom) -> bool {
    let mut level = status;
    if always_show && level == ParticleStatus::Minimal && rng.next_int(10) == 0 {
        level = ParticleStatus::Decreased;
    }
    if level == ParticleStatus::Decreased && rng.next_int(3) == 0 {
        level = ParticleStatus::Minimal;
    }
    level != ParticleStatus::Minimal
}

#[derive(Clone, Copy)]
struct SpawnGate {
    override_limiter: bool,
    always_show: bool,
}

impl SpawnGate {
    const NORMAL: SpawnGate = SpawnGate {
        override_limiter: false,
        always_show: false,
    };
    const ALWAYS_SHOW: SpawnGate = SpawnGate {
        override_limiter: false,
        always_show: true,
    };
}

#[derive(Resource)]
struct ParticleRng(JavaRandom);

impl Default for ParticleRng {
    fn default() -> Self {
        let seed = crate::platform::time::SystemTime::now()
            .duration_since(crate::platform::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0);
        ParticleRng(JavaRandom::new(seed))
    }
}

#[derive(Resource, Default)]
struct LiveParticles(Vec<Particle>);

#[derive(Resource, Default)]
struct ParticleAtlas(Option<Atlas>);

pub(crate) struct Atlas {
    image: Handle<Image>,
    rows: u32,
    sheets: Vec<Vec<u32>>,
    by_name: HashMap<String, u16>,
    crit: Option<[f32; 4]>,
}

impl Atlas {
    fn sheet_index(&self, particle_type: &str) -> Option<u16> {
        self.by_name.get(particle_type).copied()
    }

    fn frame_uv(&self, sheet: u16, frame: usize) -> [f32; 4] {
        let tiles = &self.sheets[sheet as usize];
        particle_assets::tile_uv(tiles[frame % tiles.len()], self.rows)
    }
}

fn load_particle_atlas(mut atlas: ResMut<ParticleAtlas>, mut images: ResMut<Assets<Image>>) {
    if atlas.0.is_some() {
        return;
    }
    let built = particle_assets::build_atlas();
    if built.sheets.is_empty() {
        return;
    }
    let mut sheets = Vec::with_capacity(built.sheets.len());
    let mut by_name = HashMap::with_capacity(built.sheets.len());
    for (name, tiles) in built.sheets {
        by_name.insert(name, sheets.len() as u16);
        sheets.push(tiles);
    }
    let crit = by_name
        .get("crit")
        .map(|&i| particle_assets::tile_uv(sheets[i as usize][0], built.rows));
    atlas.0 = Some(Atlas {
        image: images.add(built.image),
        rows: built.rows,
        sheets,
        by_name,
        crit,
    });
}

#[derive(Resource, Default)]
struct ParticleMaterialCache(HashMap<BatchKey, Handle<EntityMaterial>>);

const MAX_CACHED_MATERIALS: usize = 256;

pub(crate) const MAX_LIVE_PARTICLES: usize = 5000;

const MAX_PARTICLE_BATCHES: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct BatchKey {
    block_light: u8,
    sky_light: u8,
    dim: u8,
    terrain: bool,
    translucent: bool,
}

#[derive(Resource, Default)]
struct ParticleBatches {
    pool: Vec<(Entity, Handle<Mesh>)>,
    index: HashMap<BatchKey, usize>,
    keys: Vec<BatchKey>,
    groups: Vec<Vec<u32>>,
}

pub(crate) struct Particle {
    prev: Vec3,
    pos: Vec3,
    velocity: Vec3,
    gravity: f32,
    friction: f32,
    tick_acc: f32,
    age: u32,
    lifetime: u32,
    tint: [f32; 4],
    size: f32,
    grow: Grow,
    sprite: Sprite,
    flags: u8,
}

#[derive(Clone, Copy)]
enum Sprite {
    Fixed([f32; 4]),
    Animated(u16),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Grow {
    Fixed,
    In,
    Flame,
    Lava,
    Portal,
}

const PHYSICS: u8 = 1 << 0;
const TERRAIN: u8 = 1 << 1;
const TRANSLUCENT: u8 = 1 << 2;
const FULLBRIGHT: u8 = 1 << 3;
const CRIT_DECAY: u8 = 1 << 4;
const FADE_OUT: u8 = 1 << 5;

const DESTROY_GRID: i32 = 4;

fn spawn_particles(
    shared: Res<Shared>,
    gui: Res<crate::gui::GuiState>,
    atlas: Res<ParticleAtlas>,
    camera: Query<&GlobalTransform, With<WorldCamera>>,
    mut live: ResMut<LiveParticles>,
    mut particle_rng: ResMut<ParticleRng>,
) {
    let emits = std::mem::take(&mut shared.0.lock().unwrap().session.particle_emits);
    if emits.is_empty() {
        return;
    }
    let status = gui.options.particles;
    let eye = camera
        .iter()
        .next()
        .map(GlobalTransform::translation)
        .unwrap_or(Vec3::ZERO);
    let world_arc = particle_world();
    let world = world_arc.as_deref().map(|w| w.read());
    let world = world.as_deref();

    let (mut breaks, mut packets, mut events) = (0u32, 0u32, 0u32);
    let before = live.0.len();

    for emit in emits {
        if live.0.len() >= MAX_LIVE_PARTICLES {
            break;
        }
        match emit {
            crate::session::ParticleEmit::Block(spawn) => {
                breaks += 1;
                spawn_block_break(&spawn, status, world, &mut live.0, &mut particle_rng.0);
            }
            crate::session::ParticleEmit::Level(level) => {
                packets += 1;
                let Some(atlas) = atlas.0.as_ref() else {
                    continue;
                };
                spawn_level_particle(
                    &level,
                    atlas,
                    world,
                    eye,
                    status,
                    &mut live.0,
                    &mut particle_rng.0,
                );
            }
            crate::session::ParticleEmit::Event(event) => {
                let (Some(atlas), Some(world)) = (atlas.0.as_ref(), world) else {
                    continue;
                };
                let mut ctx = Emitter {
                    cell: event.pos,
                    world,
                    atlas,
                    status,
                    eye,
                    live: &mut live.0,
                    rng: &mut particle_rng.0,
                };
                events += 1;
                crate::util::level_events::spawn(&event, &mut ctx);
            }
        }
    }

    if breaks + packets + events > 0 {
        crate::log_debug!(
            "particles",
            "drained {breaks} break, {packets} packet, {events} event: {} live, was {before}",
            live.0.len()
        );
    }
}

pub(crate) fn spawn_block_break(
    spawn: &crate::session::ParticleSpawn,
    status: ParticleStatus,
    world: Option<&azalea_world::World>,
    live: &mut Vec<Particle>,
    particle_rng: &mut JavaRandom,
) {
    let Some(tile) = particle_uv(spawn.state) else {
        return;
    };
    let block_min = Vec3::new(
        spawn.pos[0] as f32,
        spawn.pos[1] as f32,
        spawn.pos[2] as f32,
    );
    let seed = ((particle_rng.next_int(i32::MAX as u32) as i64) << 31)
        | particle_rng.next_int(i32::MAX as u32) as i64;
    let mut rng = JavaRandom::new(seed);

    let points: Vec<(Vec3, Vec3)> = if spawn.mining {
        let mut pos = block_min
            + Vec3::new(
                0.1 + rng.next_f32() * 0.8,
                0.1 + rng.next_f32() * 0.8,
                0.1 + rng.next_f32() * 0.8,
            );
        let face = Vec3::from(spawn.face);
        if face.x.abs() > 0.5 {
            pos.x = block_min.x + if face.x > 0.0 { 1.1 } else { -0.1 };
        }
        if face.y.abs() > 0.5 {
            pos.y = block_min.y + if face.y > 0.0 { 1.1 } else { -0.1 };
        }
        if face.z.abs() > 0.5 {
            pos.z = block_min.z + if face.z > 0.0 { 1.1 } else { -0.1 };
        }
        vec![(pos, Vec3::ZERO)]
    } else {
        let mut out = Vec::with_capacity((DESTROY_GRID * DESTROY_GRID * DESTROY_GRID) as usize);
        for xx in 0..DESTROY_GRID {
            for yy in 0..DESTROY_GRID {
                for zz in 0..DESTROY_GRID {
                    let rel = Vec3::new(
                        (xx as f32 + 0.5) / DESTROY_GRID as f32,
                        (yy as f32 + 0.5) / DESTROY_GRID as f32,
                        (zz as f32 + 0.5) / DESTROY_GRID as f32,
                    );
                    out.push((block_min + rel, rel - Vec3::splat(0.5)));
                }
            }
        }
        out
    };

    let ratio = tint_ratio(spawn.state, spawn.pos, world);
    let tint = [0.6 * ratio[0], 0.6 * ratio[1], 0.6 * ratio[2], 1.0];

    for (pos, seed) in points {
        if live.len() >= MAX_LIVE_PARTICLES {
            break;
        }
        if !allow_particle(status, false, particle_rng) {
            continue;
        }
        let velocity = base_velocity(seed, &mut rng);
        let velocity = if spawn.mining {
            set_power(velocity, 0.2)
        } else {
            velocity
        };
        let size =
            0.1 * (rng.next_f32() * 0.5 + 0.5) * 2.0 / 2.0 * if spawn.mining { 0.6 } else { 1.0 };
        let lifetime = (4.0 / (rng.next_f32() * 0.9 + 0.1)) as u32;

        let mut particle = Particle {
            prev: pos,
            pos,
            velocity,
            gravity: 1.0,
            friction: 0.98,
            tick_acc: rng.next_f32() / 20.0,
            age: 0,
            lifetime,
            tint,
            size,
            grow: Grow::Fixed,
            sprite: Sprite::Fixed(random_sub_uv(tile, &mut rng)),
            flags: PHYSICS | TERRAIN,
        };
        if particle.lifetime > 0 {
            particle.age = 1;
            advance_one_tick(&mut particle, world);
        }
        live.push(particle);
    }
}

fn spawn_crit_particles(
    view: Res<crate::renderer::FrameView>,
    gui: Res<crate::gui::GuiState>,
    atlas: Res<ParticleAtlas>,
    mut live: ResMut<LiveParticles>,
    mut particle_rng: ResMut<ParticleRng>,
) {
    let hits = std::mem::take(&mut *crate::client::tracking::crit_hits().lock().unwrap());
    if hits.is_empty() {
        return;
    }
    let Some(uv) = atlas.0.as_ref().and_then(|a| a.crit) else {
        return;
    };
    let status = gui.options.particles;

    let partial = view.partial;
    let targets = hits.into_iter().filter_map(|(id, magic)| {
        let anim = view.entities.iter().find(|e| e.id == id)?;
        Some((
            Vec3::from(anim.position(partial)),
            anim.bounding_box_height(),
            magic,
        ))
    });

    for (base, height, magic) in targets {
        for _round in 0..3 {
            for _ in 0..16 {
                if live.0.len() >= MAX_LIVE_PARTICLES {
                    break;
                }
                let xa = particle_rng.0.next_f32() * 2.0 - 1.0;
                let ya = particle_rng.0.next_f32() * 2.0 - 1.0;
                let za = particle_rng.0.next_f32() * 2.0 - 1.0;
                if xa * xa + ya * ya + za * za > 1.0 {
                    continue;
                }
                if !allow_particle(status, false, &mut particle_rng.0) {
                    continue;
                }

                let pos = base + Vec3::new(xa / 4.0, height * (0.5 + ya / 4.0), za / 4.0);
                let velocity = Vec3::new(xa, ya + 0.2, za) * 0.4;

                let grey = particle_rng.0.next_f32() * 0.3 + 0.6;
                let mut tint = [grey, grey, grey, 1.0];
                if magic {
                    tint[0] *= 0.3;
                    tint[1] *= 0.8;
                }
                let size = 0.1 * (particle_rng.0.next_f32() * 0.5 + 0.5) * 2.0 * 0.75;
                let lifetime = (6.0 / (particle_rng.0.next_f32() * 0.8 + 0.6)).max(1.0) as u32;

                let mut particle = Particle {
                    prev: pos,
                    pos,
                    velocity,
                    gravity: 0.5,
                    friction: 0.7,
                    tick_acc: particle_rng.0.next_f32() / 20.0,
                    age: 0,
                    lifetime,
                    tint,
                    size,
                    grow: Grow::In,
                    sprite: Sprite::Fixed(uv),
                    flags: CRIT_DECAY,
                };
                if particle.lifetime > 0 {
                    particle.age = 1;
                    advance_one_tick(&mut particle, None);
                }
                live.0.push(particle);
            }
        }
    }
}

const MAX_EMIT_COUNT: u32 = 512;

const PARTICLE_CULL_DISTANCE_SQ: f32 = 1024.0;

#[allow(clippy::too_many_arguments)]
fn spawn_level_particle(
    emit: &crate::session::LevelParticle,
    atlas: &Atlas,
    world: Option<&azalea_world::World>,
    eye: Vec3,
    status: ParticleStatus,
    live: &mut Vec<Particle>,
    rng: &mut JavaRandom,
) {
    let base = Vec3::new(emit.pos[0] as f32, emit.pos[1] as f32, emit.pos[2] as f32);
    let gate = SpawnGate {
        override_limiter: emit.override_limiter,
        always_show: emit.always_show,
    };
    if emit.count == 0 {
        let arg = Vec3::new(
            emit.dist[0] * emit.max_speed,
            emit.dist[1] * emit.max_speed,
            emit.dist[2] * emit.max_speed,
        );
        add_level_particle(
            live,
            atlas,
            world,
            rng,
            status,
            &emit.particle,
            gate,
            base,
            arg,
            eye,
        );
        return;
    }
    for _ in 0..emit.count.min(MAX_EMIT_COUNT) {
        if live.len() >= MAX_LIVE_PARTICLES {
            break;
        }
        let pos = base
            + Vec3::new(
                rng.next_gaussian() as f32 * emit.dist[0],
                rng.next_gaussian() as f32 * emit.dist[1],
                rng.next_gaussian() as f32 * emit.dist[2],
            );
        let arg = Vec3::new(
            rng.next_gaussian() as f32 * emit.max_speed,
            rng.next_gaussian() as f32 * emit.max_speed,
            rng.next_gaussian() as f32 * emit.max_speed,
        );
        add_level_particle(
            live,
            atlas,
            world,
            rng,
            status,
            &emit.particle,
            gate,
            pos,
            arg,
            eye,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn add_level_particle(
    live: &mut Vec<Particle>,
    atlas: &Atlas,
    world: Option<&azalea_world::World>,
    rng: &mut JavaRandom,
    status: ParticleStatus,
    particle_kind: &azalea::entity::particle::Particle,
    gate: SpawnGate,
    pos: Vec3,
    arg: Vec3,
    eye: Vec3,
) {
    if !gate.override_limiter {
        if eye.distance_squared(pos) > PARTICLE_CULL_DISTANCE_SQ {
            return;
        }
        if !allow_particle(status, gate.always_show, rng) {
            return;
        }
    }
    let Some(recipe) = recipe(particle_kind, atlas, arg, rng) else {
        return;
    };
    let mut particle = Particle {
        prev: pos,
        pos,
        velocity: recipe.velocity,
        gravity: recipe.gravity,
        friction: recipe.friction,
        tick_acc: rng.next_f32() / 20.0,
        age: 0,
        lifetime: recipe.lifetime.max(1),
        tint: recipe.tint,
        size: recipe.size,
        grow: recipe.grow,
        sprite: recipe.sprite,
        flags: recipe.flags,
    };
    particle.age = 1;
    advance_one_tick(&mut particle, world);
    live.push(particle);
}

const ANIMATE_SAMPLES: u32 = 667;

const ANIMATE_RADII: [u32; 2] = [16, 32];

#[derive(Resource, Default)]
struct AmbientClock(f32);

fn animate_tick_particles(
    time: Res<Time>,
    gui: Res<crate::gui::GuiState>,
    atlas: Res<ParticleAtlas>,
    camera: Query<&GlobalTransform, With<WorldCamera>>,
    mut live: ResMut<LiveParticles>,
    mut particle_rng: ResMut<ParticleRng>,
    mut clock: ResMut<AmbientClock>,
) {
    clock.0 += time.delta_secs();
    if clock.0 < TICK {
        return;
    }
    clock.0 = (clock.0 - TICK).min(TICK);

    let Some(atlas) = atlas.0.as_ref() else {
        return;
    };
    let status = gui.options.particles;
    if status == ParticleStatus::Minimal {
        return;
    }
    let Some(eye) = camera.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    let world_arc = particle_world();
    let Some(world) = world_arc.as_deref().map(|w| w.read()) else {
        return;
    };

    let center = [
        eye.x.floor() as i32,
        eye.y.floor() as i32,
        eye.z.floor() as i32,
    ];
    let rng = &mut particle_rng.0;
    let before = live.0.len();
    'sampling: for _ in 0..ANIMATE_SAMPLES {
        for radius in ANIMATE_RADII {
            if live.0.len() >= MAX_LIVE_PARTICLES {
                break 'sampling;
            }
            let cell = [
                center[0] + rng.next_int(radius) as i32 - rng.next_int(radius) as i32,
                center[1] + rng.next_int(radius) as i32 - rng.next_int(radius) as i32,
                center[2] + rng.next_int(radius) as i32 - rng.next_int(radius) as i32,
            ];
            do_animate_tick(cell, &world, atlas, status, eye, &mut live.0, rng);
        }
    }

    if live.0.len() != before {
        crate::log_debug!(
            "particles",
            "animate tick spawned {}: {} live",
            live.0.len() - before,
            live.0.len()
        );
    }
}

fn do_animate_tick(
    cell: [i32; 3],
    world: &azalea_world::World,
    atlas: &Atlas,
    status: ParticleStatus,
    eye: Vec3,
    live: &mut Vec<Particle>,
    rng: &mut JavaRandom,
) {
    let Some(state) = block_at(world, cell) else {
        return;
    };
    let mut ctx = Emitter {
        cell,
        world,
        atlas,
        status,
        eye,
        live,
        rng,
    };
    if let Some(ambient) = crate::blocks::ambient::of(state) {
        emit_ambient(ambient, state, &mut ctx);
    }

    let fluid = azalea::block::fluid_state::FluidState::from(state);
    if fluid.is_empty() {
        return;
    }
    emit_fluid_ambient(&fluid, &mut ctx);

    if ctx.rng.next_int(10) == 0 {
        let below = [cell[0], cell[1] - 1, cell[2]];
        let under = [cell[0], cell[1] - 2, cell[2]];
        if !open_cell(ctx.world, below) && open_cell(ctx.world, under) {
            let drip = match fluid.kind {
                azalea::block::fluid_state::FluidKind::Lava => P::DrippingLava,
                _ => P::DrippingWater,
            };
            let x = ctx.rng.next_f32();
            let z = ctx.rng.next_f32();
            ctx.emit(&drip, rel(below, [x, -0.05, z]), Vec3::ZERO, false);
        }
    }
}

pub(crate) struct Emitter<'a> {
    pub(crate) cell: [i32; 3],
    pub(crate) world: &'a azalea_world::World,
    pub(crate) atlas: &'a Atlas,
    pub(crate) status: ParticleStatus,
    pub(crate) eye: Vec3,
    pub(crate) live: &'a mut Vec<Particle>,
    pub(crate) rng: &'a mut JavaRandom,
}

impl Emitter<'_> {
    pub(crate) fn emit(&mut self, kind: &P, pos: Vec3, velocity: Vec3, always: bool) {
        if self.live.len() >= MAX_LIVE_PARTICLES {
            return;
        }
        add_level_particle(
            self.live,
            self.atlas,
            Some(self.world),
            self.rng,
            self.status,
            kind,
            if always {
                SpawnGate::ALWAYS_SHOW
            } else {
                SpawnGate::NORMAL
            },
            pos,
            velocity,
            self.eye,
        );
    }

    pub(crate) fn f32(&mut self) -> f32 {
        self.rng.next_f32()
    }

    pub(crate) fn one_in(&mut self, bound: u32) -> bool {
        self.rng.next_int(bound) == 0
    }

    pub(crate) fn block(&self, cell: [i32; 3]) -> Option<azalea::block::BlockState> {
        block_at(self.world, cell)
    }

    pub(crate) fn solid(&self, cell: [i32; 3]) -> bool {
        self.block(cell)
            .is_some_and(crate::util::block_model::is_solid)
    }

    pub(crate) fn below(&mut self, kind: &P) {
        let x = self.f32();
        let z = self.f32();
        let cell = self.cell;
        self.emit(kind, rel(cell, [x, -0.05, z]), Vec3::ZERO, false);
    }
}

pub(crate) fn rel(cell: [i32; 3], offset: [f32; 3]) -> Vec3 {
    Vec3::new(
        cell[0] as f32 + offset[0],
        cell[1] as f32 + offset[1],
        cell[2] as f32 + offset[2],
    )
}

fn open_cell(world: &azalea_world::World, cell: [i32; 3]) -> bool {
    block_at(world, cell).is_none_or(|s| {
        !crate::util::block_model::is_solid(s)
            && azalea::block::fluid_state::FluidState::from(s).is_empty()
    })
}

pub(crate) fn block_at(
    world: &azalea_world::World,
    cell: [i32; 3],
) -> Option<azalea::block::BlockState> {
    world.get_block_state(azalea_core::position::BlockPos {
        x: cell[0],
        y: cell[1],
        z: cell[2],
    })
}

pub(crate) fn dust(color: [f32; 3], scale: f32) -> P {
    P::Dust(azalea::entity::particle::DustParticle {
        color: azalea_core::color::RgbColor::new(
            (color[0] * 255.0) as u8,
            (color[1] * 255.0) as u8,
            (color[2] * 255.0) as u8,
        ),
        scale,
    })
}

fn emit_ambient(
    ambient: crate::blocks::ambient::Ambient,
    state: azalea::block::BlockState,
    ctx: &mut Emitter,
) {
    use crate::blocks::ambient::{Ambient, Flame, Furnace, HORIZONTAL, Leaf, WireSide};

    let cell = ctx.cell;
    match ambient {
        Ambient::Torch { flame, offset } => {
            let pos = rel(cell, offset);
            ctx.emit(&P::Smoke, pos, Vec3::ZERO, false);
            let flame = match flame {
                Flame::Flame => P::Flame,
                Flame::Soul => P::SoulFireFlame,
                Flame::Copper => P::CopperFireFlame,
            };
            ctx.emit(&flame, pos, Vec3::ZERO, false);
        }
        Ambient::Candle { wicks, count } => {
            for wick in wicks.iter().take(count as usize) {
                let pos = rel(cell, *wick);
                if ctx.f32() < 0.3 {
                    ctx.emit(&P::Smoke, pos, Vec3::ZERO, false);
                }
                ctx.emit(&P::SmallFlame, pos, Vec3::ZERO, false);
            }
        }
        Ambient::Fire => {
            for _ in 0..3 {
                let x = ctx.f32();
                let y = ctx.f32() * 0.5 + 0.5;
                let z = ctx.f32();
                ctx.emit(&P::LargeSmoke, rel(cell, [x, y, z]), Vec3::ZERO, false);
            }
        }
        Ambient::Campfire => {
            if !ctx.one_in(5) {
                return;
            }
            let xa = ctx.f32() / 2.0;
            let za = ctx.f32() / 2.0;
            ctx.emit(
                &P::Lava,
                rel(cell, [0.5, 0.5, 0.5]),
                Vec3::new(xa, 5.0e-5, za),
                false,
            );
        }
        Ambient::Furnace { kind, front } => {
            if kind == Furnace::Smoker {
                ctx.emit(&P::Smoke, rel(cell, [0.5, 1.1, 0.5]), Vec3::ZERO, false);
                return;
            }
            let side = ctx.f32() * 0.6 - 0.3;
            let height = if kind == Furnace::Blast { 9.0 } else { 6.0 } / 16.0;
            let dy = ctx.f32() * height;
            let dx = if front[0] != 0.0 {
                front[0] * 0.52
            } else {
                side
            };
            let dz = if front[2] != 0.0 {
                front[2] * 0.52
            } else {
                side
            };
            let pos = rel(cell, [0.5 + dx, dy, 0.5 + dz]);
            ctx.emit(&P::Smoke, pos, Vec3::ZERO, false);
            if kind == Furnace::Furnace {
                ctx.emit(&P::Flame, pos, Vec3::ZERO, false);
            }
        }
        Ambient::BrewingStand => {
            let x = 0.4 + ctx.f32() * 0.2;
            let y = 0.7 + ctx.f32() * 0.3;
            let z = 0.4 + ctx.f32() * 0.2;
            ctx.emit(&P::Smoke, rel(cell, [x, y, z]), Vec3::ZERO, false);
        }
        Ambient::WitherRose => {
            for _ in 0..3 {
                if ctx.rng.next_int(2) != 0 {
                    continue;
                }
                let x = 0.5 + ctx.f32() / 5.0;
                let y = 0.5 - ctx.f32();
                let z = 0.5 + ctx.f32() / 5.0;
                ctx.emit(&P::Smoke, rel(cell, [x, y, z]), Vec3::ZERO, false);
            }
        }
        Ambient::EndPortal => {
            let x = ctx.f32();
            let z = ctx.f32();
            ctx.emit(&P::Smoke, rel(cell, [x, 0.8, z]), Vec3::ZERO, false);
        }
        Ambient::NetherPortal { axis_x } => {
            for _ in 0..4 {
                let mut x = ctx.f32();
                let y = ctx.f32();
                let mut z = ctx.f32();
                let mut xa = (ctx.f32() - 0.5) * 0.5;
                let ya = (ctx.f32() - 0.5) * 0.5;
                let mut za = (ctx.f32() - 0.5) * 0.5;
                let flip = ctx.rng.next_int(2) as f32 * 2.0 - 1.0;
                if axis_x {
                    z = 0.5 + 0.25 * flip;
                    za = ctx.f32() * 2.0 * flip;
                } else {
                    x = 0.5 + 0.25 * flip;
                    xa = ctx.f32() * 2.0 * flip;
                }
                ctx.emit(
                    &P::Portal,
                    rel(cell, [x, y, z]),
                    Vec3::new(xa, ya, za),
                    false,
                );
            }
        }
        Ambient::EnderChest => {
            for _ in 0..3 {
                let flip_x = ctx.rng.next_int(2) as f32 * 2.0 - 1.0;
                let flip_z = ctx.rng.next_int(2) as f32 * 2.0 - 1.0;
                let y = ctx.f32();
                let xa = ctx.f32() * flip_x;
                let ya = (ctx.f32() - 0.5) * 0.125;
                let za = ctx.f32() * flip_z;
                ctx.emit(
                    &P::Portal,
                    rel(cell, [0.5 + 0.25 * flip_x, y, 0.5 + 0.25 * flip_z]),
                    Vec3::new(xa, ya, za),
                    false,
                );
            }
        }
        Ambient::RespawnAnchor => {
            let x = 0.5 + (0.5 - ctx.f32());
            let z = 0.5 + (0.5 - ctx.f32());
            let ya = ctx.f32() * 0.04;
            ctx.emit(
                &P::ReversePortal,
                rel(cell, [x, 1.0, z]),
                Vec3::new(0.0, ya, 0.0),
                false,
            );
        }
        Ambient::EnchantingTable => {
            for dy in 0..2i32 {
                for dx in -2..=2i32 {
                    for dz in -2..=2i32 {
                        if dx.abs() != 2 && dz.abs() != 2 {
                            continue;
                        }
                        if !ctx.one_in(16) {
                            continue;
                        }
                        let shelf = [cell[0] + dx, cell[1] + dy, cell[2] + dz];
                        let between = [cell[0] + dx / 2, cell[1] + dy, cell[2] + dz / 2];
                        let is_shelf = ctx
                            .block(shelf)
                            .is_some_and(|s| s.as_block_kind().to_str() == "minecraft:bookshelf");
                        if !is_shelf || ctx.solid(between) {
                            continue;
                        }
                        let ox = dx as f32 + ctx.f32() - 0.5;
                        let oy = dy as f32 - ctx.f32() - 1.0;
                        let oz = dz as f32 + ctx.f32() - 0.5;
                        ctx.emit(
                            &P::Enchant,
                            rel(cell, [0.5, 2.0, 0.5]),
                            Vec3::new(ox, oy, oz),
                            false,
                        );
                    }
                }
            }
        }
        Ambient::EndRod { step } => {
            if !ctx.one_in(5) {
                return;
            }
            let x = 0.55 - ctx.f32() * 0.1;
            let y = 0.55 - ctx.f32() * 0.1;
            let z = 0.55 - ctx.f32() * 0.1;
            let reach = 0.4 - (ctx.f32() + ctx.f32()) * 0.4;
            let vel = Vec3::new(
                ctx.rng.next_gaussian() as f32 * 0.005,
                ctx.rng.next_gaussian() as f32 * 0.005,
                ctx.rng.next_gaussian() as f32 * 0.005,
            );
            ctx.emit(
                &P::EndRod,
                rel(
                    cell,
                    [
                        x + step[0] * reach,
                        y + step[1] * reach,
                        z + step[2] * reach,
                    ],
                ),
                vel,
                false,
            );
        }
        Ambient::Mycelium => {
            if !ctx.one_in(10) {
                return;
            }
            let x = ctx.f32();
            let z = ctx.f32();
            ctx.emit(&P::Mycelium, rel(cell, [x, 1.1, z]), Vec3::ZERO, false);
        }
        Ambient::Leaves { leaf, chance } => {
            let below = [cell[0], cell[1] - 1, cell[2]];
            let open = !ctx.solid(below);
            let exposed = crate::renderer::environment::rain_level() > 0.0
                && crate::renderer::lightmap::levels_at([
                    cell[0] as f32 + 0.5,
                    cell[1] as f32 + 1.5,
                    cell[2] as f32 + 0.5,
                ])
                .1 >= 15;
            if exposed && open && ctx.rng.next_int(15) == 1 {
                ctx.below(&P::DrippingWater);
            }
            if ctx.f32() < chance && open {
                let particle = match leaf {
                    Leaf::BiomeTinted | Leaf::AzaleaTint => {
                        P::TintedLeaves(azalea::entity::particle::ColorParticle::default())
                    }
                    Leaf::Cherry => P::CherryLeaves,
                    Leaf::PaleOak => P::PaleOakLeaves,
                };
                ctx.below(&particle);
            }
        }
        Ambient::WetSponge => {
            let Some(pos) = random_open_face(ctx, 0.05) else {
                return;
            };
            ctx.emit(&P::DrippingWater, pos, Vec3::ZERO, false);
        }
        Ambient::Beehive => ctx.below(&P::DrippingHoney),
        Ambient::CryingObsidian => {
            if !ctx.one_in(5) {
                return;
            }
            let Some(pos) = random_open_face(ctx, 0.1) else {
                return;
            };
            ctx.emit(&P::DrippingObsidianTear, pos, Vec3::ZERO, false);
        }
        Ambient::PointedDripstone => {
            if ctx.f32() > 0.12 {
                return;
            }
            let mut probe = cell;
            let mut fluid = None;
            for _ in 0..11 {
                probe[1] += 1;
                let Some(above) = ctx.block(probe) else { break };
                if above
                    .as_block_kind()
                    .to_str()
                    .ends_with("pointed_dripstone")
                {
                    continue;
                }
                let f = azalea::block::fluid_state::FluidState::from(above);
                if !f.is_empty() {
                    fluid = Some(f.kind);
                }
                break;
            }
            let drip = match fluid {
                Some(azalea::block::fluid_state::FluidKind::Water) => P::DrippingDripstoneWater,
                Some(azalea::block::fluid_state::FluidKind::Lava) => P::DrippingDripstoneLava,
                _ => return,
            };
            ctx.emit(&drip, rel(cell, [0.5, 0.25, 0.5]), Vec3::ZERO, false);
        }
        Ambient::SporeBlossom => {
            let x = ctx.f32();
            let z = ctx.f32();
            ctx.emit(
                &P::FallingSporeBlossom,
                rel(cell, [x, 0.7, z]),
                Vec3::ZERO,
                false,
            );
            for _ in 0..14 {
                let ax = cell[0] + ctx.rng.next_int(21) as i32 - 10;
                let ay = cell[1] - ctx.rng.next_int(10) as i32;
                let az = cell[2] + ctx.rng.next_int(21) as i32 - 10;
                if ctx.solid([ax, ay, az]) {
                    continue;
                }
                let ox = ctx.f32();
                let oy = ctx.f32();
                let oz = ctx.f32();
                ctx.emit(
                    &P::SporeBlossomAir,
                    rel([ax, ay, az], [ox, oy, oz]),
                    Vec3::ZERO,
                    false,
                );
            }
        }
        Ambient::BubbleColumn { drag } => {
            if drag {
                ctx.emit(
                    &P::CurrentDown,
                    rel(cell, [0.5, 0.8, 0.0]),
                    Vec3::ZERO,
                    true,
                );
            } else {
                let up = Vec3::new(0.0, 0.04, 0.0);
                ctx.emit(&P::BubbleColumnUp, rel(cell, [0.5, 0.0, 0.5]), up, true);
                let x = ctx.f32();
                let y = ctx.f32();
                let z = ctx.f32();
                ctx.emit(&P::BubbleColumnUp, rel(cell, [x, y, z]), up, true);
            }
        }
        Ambient::DriedGhast { waterlogged } => {
            if !ctx.one_in(6) {
                return;
            }
            if waterlogged {
                let x = 0.5 + (ctx.f32() * 2.0 - 1.0) / 3.0;
                let z = 0.5 + (ctx.f32() * 2.0 - 1.0) / 3.0;
                let ya = ctx.f32();
                ctx.emit(
                    &P::HappyVillager,
                    rel(cell, [x, 0.9, z]),
                    Vec3::new(0.0, ya, 0.0),
                    false,
                );
            } else {
                ctx.emit(
                    &P::WhiteSmoke,
                    rel(cell, [0.5, 0.5, 0.5]),
                    Vec3::new(0.0, 0.02, 0.0),
                    false,
                );
            }
        }
        Ambient::SculkSensor => {
            let step = HORIZONTAL[ctx.rng.next_int(4) as usize];
            let x = if step[0] == 0.0 {
                0.5 - ctx.f32()
            } else {
                step[0] * 0.6
            };
            let z = if step[2] == 0.0 {
                0.5 - ctx.f32()
            } else {
                step[2] * 0.6
            };
            let ya = ctx.f32() * 0.04;
            let particle =
                P::DustColorTransition(azalea::entity::particle::DustColorTransitionParticle {
                    from: azalea_core::color::RgbColor::new(0x39, 0xD0, 0xE0),
                    to: azalea_core::color::RgbColor::new(0xFF, 0x00, 0x00),
                    scale: 1.0,
                });
            ctx.emit(
                &particle,
                rel(cell, [0.5 + x, 0.25, 0.5 + z]),
                Vec3::new(0.0, ya, 0.0),
                false,
            );
        }
        Ambient::RedstoneWire { power, sides } => {
            let color = crate::util::biome_color::redstone_ratio(power);
            const DOWN: [f32; 3] = [0.0, -1.0, 0.0];
            const UP: [f32; 3] = [0.0, 1.0, 0.0];
            for (i, side) in sides.iter().enumerate() {
                let horizontal = HORIZONTAL[i];
                match side {
                    WireSide::Up => {
                        wire_line(ctx, color, horizontal, UP, -0.5, 0.5);
                        wire_line(ctx, color, DOWN, horizontal, 0.0, 0.5);
                    }
                    WireSide::Side => wire_line(ctx, color, DOWN, horizontal, 0.0, 0.5),
                    WireSide::None => wire_line(ctx, color, DOWN, horizontal, 0.0, 0.3),
                }
            }
        }
        Ambient::RedstoneTorch => {
            let x = 0.5 + (ctx.f32() - 0.5) * 0.2;
            let y = 0.7 + (ctx.f32() - 0.5) * 0.2;
            let z = 0.5 + (ctx.f32() - 0.5) * 0.2;
            let particle = dust([1.0, 0.0, 0.0], 1.0);
            ctx.emit(&particle, rel(cell, [x, y, z]), Vec3::ZERO, false);
        }
        Ambient::RedstoneOre => {
            const FACES: [[f32; 3]; 6] = [
                [0.0, -1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, -1.0],
                [0.0, 0.0, 1.0],
                [-1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
            ];
            for step in FACES {
                let neighbour = [
                    cell[0] + step[0] as i32,
                    cell[1] + step[1] as i32,
                    cell[2] + step[2] as i32,
                ];
                if ctx.solid(neighbour) {
                    continue;
                }
                let x = if step[0] == 0.0 {
                    ctx.f32()
                } else {
                    0.5 + 0.5625 * step[0]
                };
                let y = if step[1] == 0.0 {
                    ctx.f32()
                } else {
                    0.5 + 0.5625 * step[1]
                };
                let z = if step[2] == 0.0 {
                    ctx.f32()
                } else {
                    0.5 + 0.5625 * step[2]
                };
                let particle = dust([1.0, 0.0, 0.0], 1.0);
                ctx.emit(&particle, rel(cell, [x, y, z]), Vec3::ZERO, false);
            }
        }
        Ambient::Repeater { step, delay } => {
            let x = 0.5 + (ctx.f32() - 0.5) * 0.2;
            let y = 0.4 + (ctx.f32() - 0.5) * 0.2;
            let z = 0.5 + (ctx.f32() - 0.5) * 0.2;
            let offset = if ctx.rng.next_int(2) == 0 {
                delay as f32 * 2.0 - 1.0
            } else {
                -5.0
            } / 16.0;
            let particle = dust([1.0, 0.0, 0.0], 1.0);
            ctx.emit(
                &particle,
                rel(cell, [x + offset * step[0], y, z + offset * step[2]]),
                Vec3::ZERO,
                false,
            );
        }
        Ambient::Lever { offset } => {
            if ctx.f32() >= 0.25 {
                return;
            }
            let particle = dust([1.0, 0.0, 0.0], 0.5);
            ctx.emit(&particle, rel(cell, offset), Vec3::ZERO, false);
        }
        Ambient::FallingDust => {
            if !ctx.one_in(16) {
                return;
            }
            let below = [cell[0], cell[1] - 1, cell[2]];
            if ctx.solid(below) {
                return;
            }
            let particle =
                P::FallingDust(azalea::entity::particle::BlockParticle { block_state: state });
            ctx.below(&particle);
        }
        Ambient::FireflyBush => {
            let (block_light, sky_light) = crate::renderer::lightmap::levels_at([
                cell[0] as f32 + 0.5,
                cell[1] as f32 + 0.5,
                cell[2] as f32 + 0.5,
            ]);
            if block_light.max(sky_light) > 13 {
                return;
            }
            if ctx.rng.next_f64() > 0.7 {
                return;
            }
            let x = ctx.rng.next_f64() as f32 * 10.0 - 5.0;
            let y = ctx.rng.next_f64() as f32 * 5.0;
            let z = ctx.rng.next_f64() as f32 * 10.0 - 5.0;
            ctx.emit(&P::Firefly, rel(cell, [x, y, z]), Vec3::ZERO, false);
        }
    }
}

fn wire_line(
    ctx: &mut Emitter,
    color: [f32; 3],
    side: [f32; 3],
    along: [f32; 3],
    from: f32,
    to: f32,
) {
    let span = to - from;
    if ctx.f32() >= 0.2 * span {
        return;
    }
    let t = from + span * ctx.f32();
    let cell = ctx.cell;
    let particle = dust(color, 1.0);
    ctx.emit(
        &particle,
        rel(
            cell,
            [
                0.5 + 0.4375 * side[0] + t * along[0],
                0.5 + 0.4375 * side[1] + t * along[1],
                0.5 + 0.4375 * side[2] + t * along[2],
            ],
        ),
        Vec3::ZERO,
        false,
    );
}

fn random_open_face(ctx: &mut Emitter, inset: f32) -> Option<Vec3> {
    const FACES: [[f32; 3]; 5] = [
        [0.0, -1.0, 0.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, 1.0],
        [-1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
    ];
    let step = FACES[ctx.rng.next_int(5) as usize];
    let cell = ctx.cell;
    let neighbour = [
        cell[0] + step[0] as i32,
        cell[1] + step[1] as i32,
        cell[2] + step[2] as i32,
    ];
    if ctx.solid(neighbour) {
        return None;
    }
    let spread = 0.5 + inset;
    let x = if step[0] == 0.0 {
        ctx.f32()
    } else {
        0.5 + step[0] * spread
    };
    let y = if step[1] == 0.0 {
        ctx.f32() * 0.8
    } else {
        0.5 + step[1] * spread
    };
    let z = if step[2] == 0.0 {
        ctx.f32()
    } else {
        0.5 + step[2] * spread
    };
    Some(rel(cell, [x, y, z]))
}

fn emit_fluid_ambient(fluid: &azalea::block::fluid_state::FluidState, ctx: &mut Emitter) {
    use azalea::block::fluid_state::FluidKind;

    let cell = ctx.cell;
    match fluid.kind {
        FluidKind::Lava => {
            let above = [cell[0], cell[1] + 1, cell[2]];
            if ctx.block(above).is_some_and(|s| !s.is_air()) {
                return;
            }
            if !ctx.one_in(100) {
                return;
            }
            let x = ctx.f32();
            let z = ctx.f32();
            ctx.emit(&P::Lava, rel(cell, [x, 1.0, z]), Vec3::ZERO, false);
        }
        FluidKind::Water => {
            if fluid.amount == 0 || fluid.amount >= 8 {
                return;
            }
            if !ctx.one_in(10) {
                return;
            }
            let x = ctx.f32();
            let y = ctx.f32();
            let z = ctx.f32();
            ctx.emit(&P::Underwater, rel(cell, [x, y, z]), Vec3::ZERO, false);
        }
        FluidKind::Empty => {}
    }
}

struct Recipe {
    velocity: Vec3,
    gravity: f32,
    friction: f32,
    lifetime: u32,
    size: f32,
    tint: [f32; 4],
    grow: Grow,
    sprite: Sprite,
    flags: u8,
}

fn base_recipe(sprite: Sprite, rng: &mut JavaRandom) -> Recipe {
    Recipe {
        velocity: Vec3::ZERO,
        gravity: 1.0,
        friction: 0.98,
        lifetime: (4.0 / (rng.next_f32() * 0.9 + 0.1)) as u32,
        size: 0.1 * (rng.next_f32() * 0.5 + 0.5) * 2.0,
        tint: [1.0, 1.0, 1.0, 1.0],
        grow: Grow::Fixed,
        sprite,
        flags: PHYSICS,
    }
}

fn short_lifetime(rng: &mut JavaRandom) -> u32 {
    (8.0 / (rng.next_f32() * 0.8 + 0.2)) as u32
}

fn particle_name(particle: &azalea::entity::particle::Particle) -> &'static str {
    use azalea::entity::particle::Particle;
    match particle {
        Particle::AngryVillager => "angry_villager",
        Particle::Block(_) => "block",
        Particle::BlockMarker(_) => "block_marker",
        Particle::Bubble => "bubble",
        Particle::Cloud => "cloud",
        Particle::CopperFireFlame => "copper_fire_flame",
        Particle::Crit => "crit",
        Particle::DamageIndicator => "damage_indicator",
        Particle::DragonBreath(_) => "dragon_breath",
        Particle::DrippingLava => "dripping_lava",
        Particle::FallingLava => "falling_lava",
        Particle::LandingLava => "landing_lava",
        Particle::DrippingWater => "dripping_water",
        Particle::FallingWater => "falling_water",
        Particle::Dust(_) => "dust",
        Particle::DustColorTransition(_) => "dust_color_transition",
        Particle::Effect(_) => "effect",
        Particle::ElderGuardian => "elder_guardian",
        Particle::EnchantedHit => "enchanted_hit",
        Particle::Enchant => "enchant",
        Particle::EndRod => "end_rod",
        Particle::EntityEffect(_) => "entity_effect",
        Particle::ExplosionEmitter => "explosion_emitter",
        Particle::Explosion => "explosion",
        Particle::Gust => "gust",
        Particle::SmallGust => "small_gust",
        Particle::GustEmitterLarge => "gust_emitter_large",
        Particle::GustEmitterSmall => "gust_emitter_small",
        Particle::SonicBoom => "sonic_boom",
        Particle::FallingDust(_) => "falling_dust",
        Particle::Firework => "firework",
        Particle::Fishing => "fishing",
        Particle::Flame => "flame",
        Particle::Infested => "infested",
        Particle::CherryLeaves => "cherry_leaves",
        Particle::PaleOakLeaves => "pale_oak_leaves",
        Particle::TintedLeaves(_) => "tinted_leaves",
        Particle::SculkSoul => "sculk_soul",
        Particle::SculkCharge(_) => "sculk_charge",
        Particle::SculkChargePop => "sculk_charge_pop",
        Particle::SoulFireFlame => "soul_fire_flame",
        Particle::Soul => "soul",
        Particle::Flash(_) => "flash",
        Particle::HappyVillager => "happy_villager",
        Particle::Composter => "composter",
        Particle::Heart => "heart",
        Particle::InstantEffect(_) => "instant_effect",
        Particle::Item(_) => "item",
        Particle::Vibration(_) => "vibration",
        Particle::Trail(_) => "trail",
        Particle::PauseMobGrowth => "pause_mob_growth",
        Particle::ResetMobGrowth => "reset_mob_growth",
        Particle::ItemSlime => "item_slime",
        Particle::ItemCobweb => "item_cobweb",
        Particle::ItemSnowball => "item_snowball",
        Particle::LargeSmoke => "large_smoke",
        Particle::Lava => "lava",
        Particle::Mycelium => "mycelium",
        Particle::Note => "note",
        Particle::Poof => "poof",
        Particle::Portal => "portal",
        Particle::Rain => "rain",
        Particle::Smoke => "smoke",
        Particle::WhiteSmoke => "white_smoke",
        Particle::Sneeze => "sneeze",
        Particle::Spit => "spit",
        Particle::SquidInk => "squid_ink",
        Particle::SweepAttack => "sweep_attack",
        Particle::TotemOfUndying => "totem_of_undying",
        Particle::Underwater => "underwater",
        Particle::Splash => "splash",
        Particle::Witch => "witch",
        Particle::BubblePop => "bubble_pop",
        Particle::CurrentDown => "current_down",
        Particle::BubbleColumnUp => "bubble_column_up",
        Particle::Nautilus => "nautilus",
        Particle::Dolphin => "dolphin",
        Particle::CampfireCosySmoke => "campfire_cosy_smoke",
        Particle::CampfireSignalSmoke => "campfire_signal_smoke",
        Particle::DrippingHoney => "dripping_honey",
        Particle::FallingHoney => "falling_honey",
        Particle::LandingHoney => "landing_honey",
        Particle::FallingNectar => "falling_nectar",
        Particle::FallingSporeBlossom => "falling_spore_blossom",
        Particle::Ash => "ash",
        Particle::CrimsonSpore => "crimson_spore",
        Particle::WarpedSpore => "warped_spore",
        Particle::SporeBlossomAir => "spore_blossom_air",
        Particle::DrippingObsidianTear => "dripping_obsidian_tear",
        Particle::FallingObsidianTear => "falling_obsidian_tear",
        Particle::LandingObsidianTear => "landing_obsidian_tear",
        Particle::ReversePortal => "reverse_portal",
        Particle::WhiteAsh => "white_ash",
        Particle::SmallFlame => "small_flame",
        Particle::Snowflake => "snowflake",
        Particle::DrippingDripstoneLava => "dripping_dripstone_lava",
        Particle::FallingDripstoneLava => "falling_dripstone_lava",
        Particle::DrippingDripstoneWater => "dripping_dripstone_water",
        Particle::FallingDripstoneWater => "falling_dripstone_water",
        Particle::GlowSquidInk => "glow_squid_ink",
        Particle::Glow => "glow",
        Particle::WaxOn => "wax_on",
        Particle::WaxOff => "wax_off",
        Particle::ElectricSpark => "electric_spark",
        Particle::Scrape => "scrape",
        Particle::Shriek(_) => "shriek",
        Particle::EggCrack => "egg_crack",
        Particle::DustPlume => "dust_plume",
        Particle::TrialSpawnerDetection => "trial_spawner_detection",
        Particle::TrialSpawnerDetectionOminous => "trial_spawner_detection_ominous",
        Particle::VaultConnection => "vault_connection",
        Particle::DustPillar(_) => "dust_pillar",
        Particle::OminousSpawning => "ominous_spawning",
        Particle::RaidOmen => "raid_omen",
        Particle::TrialOmen => "trial_omen",
        Particle::BlockCrumble(_) => "block_crumble",
        Particle::Firefly => "firefly",
    }
}

fn recipe(
    particle: &azalea::entity::particle::Particle,
    atlas: &Atlas,
    arg: Vec3,
    rng: &mut JavaRandom,
) -> Option<Recipe> {
    use azalea::entity::particle::Particle as P;

    let name = particle_name(particle);
    match particle {
        P::Block(b) | P::BlockMarker(b) | P::FallingDust(b) => {
            terrain_recipe(b.block_state, arg, rng)
        }

        P::Flame | P::SoulFireFlame | P::CopperFireFlame => {
            flame_recipe(atlas, name, arg, rng, 1.0)
        }
        P::SmallFlame => flame_recipe(atlas, name, arg, rng, 0.5),

        P::Smoke | P::WhiteSmoke => ash_smoke_recipe(atlas, name, arg, rng, 1.0, 0.3),
        P::LargeSmoke => ash_smoke_recipe(atlas, name, arg, rng, 2.5, 0.3),
        P::Ash | P::WhiteAsh | P::CrimsonSpore | P::WarpedSpore | P::SporeBlossomAir => {
            ash_smoke_recipe(atlas, name, arg, rng, 1.0, 1.0)
        }
        P::CampfireCosySmoke => campfire_recipe(atlas, name, arg, rng, 80, 0.9),
        P::CampfireSignalSmoke => campfire_recipe(atlas, name, arg, rng, 280, 0.95),

        P::Poof => {
            let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
            r.velocity = arg
                + Vec3::new(
                    (rng.next_f32() - 0.5) * 0.1,
                    (rng.next_f32() - 0.5) * 0.1,
                    (rng.next_f32() - 0.5) * 0.1,
                );
            let grey = rng.next_f32() * 0.3 + 0.7;
            r.tint = [grey, grey, grey, 1.0];
            r.size = 0.1 * (rng.next_f32() * rng.next_f32() * 6.0 + 1.0);
            r.lifetime = (16.0 / (rng.next_f32() * 0.8 + 0.2)) as u32 + 2;
            r.gravity = -0.1;
            r.friction = 0.9;
            r.grow = Grow::In;
            Some(r)
        }
        P::Explosion => {
            let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
            r.velocity = Vec3::ZERO;
            r.gravity = 0.0;
            r.lifetime = 6 + rng.next_int(4);
            let grey = rng.next_f32() * 0.6 + 0.4;
            r.tint = [grey, grey, grey, 1.0];
            r.size = 2.0 * (1.0 - arg.x.clamp(0.0, 1.0) * 0.5);
            r.flags = FULLBRIGHT;
            Some(r)
        }
        P::ExplosionEmitter => {
            let mut r = base_recipe(Sprite::Animated(atlas.sheet_index("explosion")?), rng);
            r.velocity = Vec3::ZERO;
            r.gravity = 0.0;
            r.lifetime = 6 + rng.next_int(10);
            let grey = rng.next_f32() * 0.6 + 0.4;
            r.tint = [grey, grey, grey, 1.0];
            r.size = 2.0;
            r.flags = FULLBRIGHT;
            Some(r)
        }

        P::Splash | P::Rain => {
            let uv = random_frame(atlas, name, rng)?;
            let mut r = base_recipe(Sprite::Fixed(uv), rng);
            r.gravity = if matches!(particle, P::Splash) {
                0.04
            } else {
                0.06
            };
            r.velocity = Vec3::new(arg.x, rng.next_f32() * 0.2 + 0.1, arg.z);
            r.size = 0.01;
            r.lifetime = short_lifetime(rng);
            Some(r)
        }
        P::Bubble | P::BubblePop | P::CurrentDown | P::BubbleColumnUp | P::Underwater => {
            let uv = random_frame(atlas, name, rng)?;
            let mut r = base_recipe(Sprite::Fixed(uv), rng);
            r.size = 0.02 * (rng.next_f32() * 0.6 + 0.2);
            r.velocity = arg * 0.2
                + Vec3::new(
                    (rng.next_f32() - 0.5) * 0.04,
                    (rng.next_f32() - 0.5) * 0.04,
                    (rng.next_f32() - 0.5) * 0.04,
                );
            r.gravity = -0.05;
            r.friction = 0.85;
            r.lifetime = short_lifetime(rng);
            Some(r)
        }

        P::Dust(dust) => {
            let color = [
                dust.color.red() as f32 / 255.0,
                dust.color.green() as f32 / 255.0,
                dust.color.blue() as f32 / 255.0,
            ];
            dust_recipe(atlas, name, arg, rng, color, dust.scale)
        }
        P::DustColorTransition(dust) => {
            let color = [
                dust.from.red() as f32 / 255.0,
                dust.from.green() as f32 / 255.0,
                dust.from.blue() as f32 / 255.0,
            ];
            dust_recipe(atlas, name, arg, rng, color, dust.scale)
        }
        P::DustPlume => dust_recipe(atlas, name, arg, rng, [0.75, 0.65, 0.5], 1.0),

        P::HappyVillager | P::Composter | P::EggCrack => {
            suspended_recipe(atlas, name, arg, rng, [1.0, 1.0, 1.0, 1.0])
        }
        P::Mycelium => {
            let grey = rng.next_f32() * 0.1 + 0.2;
            suspended_recipe(atlas, name, arg, rng, [grey, grey, grey, 1.0])
        }
        P::Dolphin => {
            let alpha = 1.0 - rng.next_f32() * 0.7;
            suspended_recipe(atlas, name, arg, rng, [0.3, 0.5, 1.0, alpha])
        }

        P::Heart | P::AngryVillager => {
            let uv = random_frame(atlas, name, rng)?;
            let mut r = base_recipe(Sprite::Fixed(uv), rng);
            r.velocity = arg * 0.01 + Vec3::Y * 0.1;
            r.gravity = 0.0;
            r.friction = 0.86;
            r.lifetime = 16;
            r.size *= 1.5;
            r.grow = Grow::In;
            r.flags = 0;
            Some(r)
        }
        P::Note => {
            let uv = random_frame(atlas, name, rng)?;
            let mut r = base_recipe(Sprite::Fixed(uv), rng);
            let hue = arg.x;
            let channel = |offset: f32| {
                (((hue + offset) * std::f32::consts::TAU).sin() * 0.65 + 0.35).max(0.0)
            };
            r.tint = [channel(0.0), channel(1.0 / 3.0), channel(2.0 / 3.0), 1.0];
            r.velocity = Vec3::new(arg.x * 0.01, arg.y * 0.01 + 0.2, arg.z * 0.01);
            r.gravity = 0.0;
            r.friction = 0.66;
            r.lifetime = 6;
            r.size *= 1.5;
            r.grow = Grow::In;
            r.flags = 0;
            Some(r)
        }

        P::Portal | P::ReversePortal => {
            let uv = random_frame(atlas, name, rng)?;
            let mut r = base_recipe(Sprite::Fixed(uv), rng);
            let br = rng.next_f32() * 0.6 + 0.4;
            r.tint = [0.9 * br, 0.3 * br, br, 1.0];
            r.size = 0.1 * (rng.next_f32() * 0.2 + 0.5);
            r.lifetime = (rng.next_f32() * 10.0) as u32 + 40;
            r.velocity = arg * 0.05;
            r.gravity = 0.0;
            r.friction = 0.93;
            r.grow = Grow::Portal;
            r.flags = FULLBRIGHT;
            Some(r)
        }
        P::Enchant | P::Nautilus | P::VaultConnection => {
            let uv = random_frame(atlas, name, rng)?;
            let mut r = base_recipe(Sprite::Fixed(uv), rng);
            let br = rng.next_f32() * 0.6 + 0.4;
            r.tint = [0.9 * br, 0.9 * br, br, 1.0];
            r.size = 0.1 * (rng.next_f32() * 0.5 + 0.2);
            r.lifetime = (rng.next_f32() * 10.0) as u32 + 30;
            r.velocity = -arg * 0.03;
            r.gravity = 0.0;
            r.friction = 0.95;
            r.grow = Grow::Portal;
            r.flags = FULLBRIGHT;
            Some(r)
        }

        P::Lava => {
            let uv = random_frame(atlas, name, rng)?;
            let mut r = base_recipe(Sprite::Fixed(uv), rng);
            r.velocity = Vec3::new(
                (rng.next_f32() - 0.5) * 0.16,
                rng.next_f32() * 0.4 + 0.05,
                (rng.next_f32() - 0.5) * 0.16,
            );
            r.gravity = 0.75;
            r.friction = 0.999;
            r.size *= rng.next_f32() * 2.0 + 0.2;
            r.lifetime = (16.0 / (rng.next_f32() * 0.8 + 0.2)) as u32;
            r.grow = Grow::Lava;
            r.flags = PHYSICS | FULLBRIGHT;
            Some(r)
        }

        P::Cloud | P::Sneeze => {
            let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
            r.velocity = arg + Vec3::splat(0.1) * (rng.next_f32() - 0.5);
            let grey = 1.0 - rng.next_f32() * 0.3;
            r.tint = if matches!(particle, P::Sneeze) {
                [0.22, 1.0, 0.53, 0.4]
            } else {
                [grey, grey, grey, 1.0]
            };
            r.size *= 1.875;
            r.friction = 0.96;
            r.gravity = 0.0;
            r.lifetime = (((8.0 / (rng.next_f32() * 0.8 + 0.3)) * 2.5) as u32).max(1);
            r.grow = Grow::In;
            r.flags = TRANSLUCENT;
            Some(r)
        }

        P::Effect(_) | P::InstantEffect(_) | P::Infested | P::RaidOmen | P::TrialOmen => {
            spell_recipe(atlas, name, arg, rng, [1.0, 1.0, 1.0, 1.0])
        }
        P::EntityEffect(color) => spell_recipe(
            atlas,
            name,
            arg,
            rng,
            [
                color.color.red() as f32 / 255.0,
                color.color.green() as f32 / 255.0,
                color.color.blue() as f32 / 255.0,
                1.0,
            ],
        ),
        P::Witch => {
            let br = rng.next_f32() * 0.5 + 0.35;
            spell_recipe(atlas, name, arg, rng, [br, 0.0, br, 1.0])
        }

        P::EndRod => {
            let lifetime = 60 + rng.next_int(12);
            animated_recipe(atlas, name, rng, arg, 0.0125, lifetime, 0.75)
        }
        P::TotemOfUndying => {
            let lifetime = 60 + rng.next_int(12);
            let mut r = animated_recipe(atlas, name, rng, arg, 1.25, lifetime, 0.75)?;
            r.friction = 0.6;
            let v = rng.next_f32();
            r.tint = if rng.next_int(4) == 0 {
                [0.6 + v * 0.2, 0.6 + v * 0.3, v * 0.2, 1.0]
            } else {
                [0.1 + v * 0.2, 0.4 + v * 0.3, v * 0.2, 1.0]
            };
            Some(r)
        }
        P::Firework => {
            let lifetime = 20 + rng.next_int(20);
            animated_recipe(atlas, name, rng, arg, 0.1, lifetime, 1.0)
        }

        P::Glow | P::GlowSquidInk | P::WaxOn | P::WaxOff | P::ElectricSpark | P::Scrape => {
            let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
            r.velocity = arg * 0.25;
            r.gravity = 0.0;
            r.friction = 0.96;
            r.size *= 0.75;
            r.lifetime = match particle {
                P::ElectricSpark => rng.next_int(2) + 2,
                P::Glow | P::GlowSquidInk => short_lifetime(rng),
                _ => rng.next_int(30) + 10,
            };
            r.tint = match particle {
                P::WaxOn => [0.91, 0.55, 0.08, 1.0],
                P::WaxOff | P::ElectricSpark => [1.0, 0.9, 1.0, 1.0],
                P::Scrape => [0.29, 0.58, 0.51, 1.0],
                _ if rng.next_int(2) == 0 => [0.6, 1.0, 0.8, 1.0],
                _ => [0.08, 0.4, 0.4, 1.0],
            };
            r.flags = FULLBRIGHT;
            Some(r)
        }

        P::Crit | P::EnchantedHit | P::DamageIndicator => {
            let mut r = base_recipe(Sprite::Fixed(atlas.crit?), rng);
            let grey = rng.next_f32() * 0.3 + 0.6;
            r.tint = [grey, grey, grey, 1.0];
            if matches!(particle, P::EnchantedHit) {
                r.tint[0] *= 0.3;
                r.tint[1] *= 0.8;
            }
            r.velocity = arg * 0.4;
            r.gravity = 0.5;
            r.friction = 0.7;
            r.size *= 0.75;
            r.lifetime = ((6.0 / (rng.next_f32() * 0.8 + 0.6)) as u32).max(1);
            r.grow = Grow::In;
            r.flags = CRIT_DECAY;
            Some(r)
        }

        P::Item(_) | P::ItemSlime | P::ItemSnowball | P::ItemCobweb => None,
        P::GustEmitterLarge | P::GustEmitterSmall => None,

        _ => generic_recipe(atlas, name, arg, rng),
    }
}

fn terrain_recipe(
    state: azalea::block::BlockState,
    arg: Vec3,
    rng: &mut JavaRandom,
) -> Option<Recipe> {
    let tile = particle_uv(state)?;
    let mut r = base_recipe(Sprite::Fixed(random_sub_uv(tile, rng)), rng);
    r.velocity = arg * 0.1 + base_velocity(Vec3::ZERO, rng);
    r.tint = [0.6, 0.6, 0.6, 1.0];
    r.size /= 2.0;
    r.flags = PHYSICS | TERRAIN;
    Some(r)
}

fn flame_recipe(
    atlas: &Atlas,
    name: &str,
    arg: Vec3,
    rng: &mut JavaRandom,
    scale: f32,
) -> Option<Recipe> {
    let uv = random_frame(atlas, name, rng)?;
    let mut r = base_recipe(Sprite::Fixed(uv), rng);
    r.velocity = arg;
    r.gravity = 0.0;
    r.friction = 0.96;
    r.size *= scale;
    r.lifetime = (8.0 / (rng.next_f32() * 0.8 + 0.2)) as u32 + 4;
    r.grow = Grow::Flame;
    r.flags = FULLBRIGHT;
    Some(r)
}

fn ash_smoke_recipe(
    atlas: &Atlas,
    name: &str,
    arg: Vec3,
    rng: &mut JavaRandom,
    scale: f32,
    grey_range: f32,
) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
    r.velocity = arg
        + Vec3::new(
            (rng.next_f32() - 0.5) * 0.1,
            (rng.next_f32() - 0.5) * 0.1,
            (rng.next_f32() - 0.5) * 0.1,
        );
    let grey = rng.next_f32() * grey_range;
    r.tint = [grey, grey, grey, 1.0];
    r.size *= 0.75 * scale;
    r.gravity = -0.1;
    r.friction = 0.96;
    r.lifetime = (((8.0 / (rng.next_f32() * 0.8 + 0.2)) * scale) as u32).max(1);
    r.grow = Grow::In;
    Some(r)
}

fn campfire_recipe(
    atlas: &Atlas,
    name: &str,
    arg: Vec3,
    rng: &mut JavaRandom,
    base_lifetime: u32,
    alpha: f32,
) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
    r.velocity = arg;
    r.size *= 3.0;
    r.gravity = 3.0e-6 / 0.04;
    r.friction = 0.96;
    r.lifetime = rng.next_int(50) + base_lifetime;
    r.tint = [1.0, 1.0, 1.0, alpha];
    r.flags = PHYSICS | TRANSLUCENT;
    Some(r)
}

fn dust_recipe(
    atlas: &Atlas,
    name: &str,
    arg: Vec3,
    rng: &mut JavaRandom,
    color: [f32; 3],
    scale: f32,
) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
    let scale = scale.clamp(0.01, 4.0);
    let base = rng.next_f32() * 0.4 + 0.6;
    r.tint = [
        (rng.next_f32() * 0.2 + 0.8) * color[0] * base,
        (rng.next_f32() * 0.2 + 0.8) * color[1] * base,
        (rng.next_f32() * 0.2 + 0.8) * color[2] * base,
        1.0,
    ];
    r.velocity = arg * 0.1;
    r.size *= 0.75 * scale;
    r.gravity = 0.0;
    r.friction = 0.96;
    r.lifetime = ((short_lifetime(rng) as f32 * scale) as u32).max(1);
    r.grow = Grow::In;
    Some(r)
}

fn suspended_recipe(
    atlas: &Atlas,
    name: &str,
    arg: Vec3,
    rng: &mut JavaRandom,
    tint: [f32; 4],
) -> Option<Recipe> {
    let uv = random_frame(atlas, name, rng)?;
    let mut r = base_recipe(Sprite::Fixed(uv), rng);
    r.tint = tint;
    r.velocity = arg * 0.02;
    r.size = 0.02 * (rng.next_f32() * 0.6 + 0.5);
    r.gravity = 0.0;
    r.friction = 0.99;
    r.lifetime = (20.0 / (rng.next_f32() * 0.8 + 0.2)) as u32;
    r.flags = 0;
    Some(r)
}

fn spell_recipe(
    atlas: &Atlas,
    name: &str,
    arg: Vec3,
    rng: &mut JavaRandom,
    tint: [f32; 4],
) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
    r.tint = tint;
    let drift_x = if arg.x == 0.0 {
        0.5 - rng.next_f32()
    } else {
        arg.x
    };
    let drift_z = if arg.z == 0.0 {
        0.5 - rng.next_f32()
    } else {
        arg.z
    };
    r.velocity = Vec3::new(drift_x * 0.1, arg.y * 0.2, drift_z * 0.1);
    r.size *= 0.75;
    r.gravity = -0.1;
    r.friction = 0.96;
    r.lifetime = short_lifetime(rng);
    r.flags = TRANSLUCENT;
    Some(r)
}

fn animated_recipe(
    atlas: &Atlas,
    name: &str,
    rng: &mut JavaRandom,
    arg: Vec3,
    gravity: f32,
    lifetime: u32,
    scale: f32,
) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
    r.velocity = arg;
    r.gravity = gravity;
    r.friction = 0.91;
    r.size *= scale;
    r.lifetime = lifetime.max(1);
    r.flags = TRANSLUCENT | FULLBRIGHT | FADE_OUT;
    Some(r)
}

fn generic_recipe(atlas: &Atlas, name: &str, arg: Vec3, rng: &mut JavaRandom) -> Option<Recipe> {
    let sheet = atlas.sheet_index(name)?;
    let animated = atlas.sheets[sheet as usize].len() > 1;
    let sprite = if animated {
        Sprite::Animated(sheet)
    } else {
        Sprite::Fixed(atlas.frame_uv(sheet, 0))
    };
    let mut r = base_recipe(sprite, rng);
    r.velocity = arg * 0.1;
    r.gravity = 0.0;
    r.friction = 0.96;
    r.size *= 0.75;
    r.lifetime = short_lifetime(rng);
    r.flags = 0;
    Some(r)
}

fn random_frame(atlas: &Atlas, name: &str, rng: &mut JavaRandom) -> Option<[f32; 4]> {
    let sheet = atlas.sheet_index(name)?;
    let len = atlas.sheets[sheet as usize].len();
    Some(atlas.frame_uv(sheet, rng.next_int(len as u32) as usize))
}

fn base_velocity(seed: Vec3, rng: &mut JavaRandom) -> Vec3 {
    let jitter = Vec3::new(
        (rng.next_f32() * 2.0 - 1.0) * 0.4,
        (rng.next_f32() * 2.0 - 1.0) * 0.4,
        (rng.next_f32() * 2.0 - 1.0) * 0.4,
    );
    let mut d = seed + jitter;
    let speed = (rng.next_f32() + rng.next_f32() + 1.0) * 0.15;
    let len = d.length();
    if len > 1e-5 {
        d /= len;
    } else {
        d = Vec3::ZERO;
    }
    Vec3::new(
        d.x * speed * 0.4,
        d.y * speed * 0.4 + 0.1,
        d.z * speed * 0.4,
    )
}

fn set_power(v: Vec3, power: f32) -> Vec3 {
    Vec3::new(v.x * power, (v.y - 0.1) * power + 0.1, v.z * power)
}

fn is_solid_at(world: Option<&azalea_world::World>, pos: Vec3) -> bool {
    let Some(world) = world else { return false };
    let block_pos = azalea_core::position::BlockPos {
        x: pos.x.floor() as i32,
        y: pos.y.floor() as i32,
        z: pos.z.floor() as i32,
    };
    world
        .get_block_state(block_pos)
        .is_some_and(crate::util::block_model::is_solid)
}

fn advance_one_tick(p: &mut Particle, world: Option<&azalea_world::World>) {
    p.prev = p.pos;
    p.velocity.y -= 0.04 * p.gravity;

    let mut next = p.pos + p.velocity;
    let mut on_ground = false;
    if p.flags & PHYSICS != 0 {
        if p.velocity.y != 0.0 && is_solid_at(world, Vec3::new(p.pos.x, next.y, p.pos.z)) {
            if p.velocity.y < 0.0 {
                next.y = next.y.floor() + 1.0;
                on_ground = true;
            } else {
                next.y = next.y.floor();
            }
            p.velocity.y = 0.0;
        }
        if p.velocity.x != 0.0 && is_solid_at(world, Vec3::new(next.x, p.pos.y, p.pos.z)) {
            next.x = p.pos.x;
            p.velocity.x = 0.0;
        }
        if p.velocity.z != 0.0 && is_solid_at(world, Vec3::new(p.pos.x, p.pos.y, next.z)) {
            next.z = p.pos.z;
            p.velocity.z = 0.0;
        }
    }
    p.pos = next;

    p.velocity *= p.friction;
    if on_ground {
        p.velocity.x *= 0.7;
        p.velocity.z *= 0.7;
    }

    if p.flags & CRIT_DECAY != 0 {
        p.tint[1] *= 0.96;
        p.tint[2] *= 0.9;
    }
    if p.flags & FADE_OUT != 0 && p.age * 2 > p.lifetime {
        let half = p.lifetime / 2;
        p.tint[3] = 1.0 - (p.age.saturating_sub(half) as f32 / p.lifetime as f32);
    }
}

fn particle_world() -> Option<std::sync::Arc<parking_lot::RwLock<azalea_world::World>>> {
    crate::client::tracking::current_world()
        .lock()
        .unwrap()
        .clone()
}

const TICK: f32 = 1.0 / 20.0;

fn tick_particles(time: Res<Time>, mut live: ResMut<LiveParticles>) {
    if live.0.is_empty() {
        return;
    }
    let dt = time.delta_secs();
    let world_arc = particle_world();
    let world = world_arc.as_deref().map(|w| w.read());
    let world = world.as_deref();

    live.0.retain_mut(|particle| {
        particle.tick_acc += dt;
        while particle.tick_acc >= TICK {
            particle.tick_acc -= TICK;
            if particle.age >= particle.lifetime {
                break;
            }
            particle.age += 1;
            advance_one_tick(particle, world);
        }
        particle.age < particle.lifetime
    });
}

#[derive(Component)]
pub(crate) struct ParticleBatch;

fn draw_particles(
    lightmap: Res<LightmapState>,
    chunk_material: Res<ChunkMaterial>,
    standard: Res<Assets<StandardMaterial>>,
    atlas: Res<ParticleAtlas>,
    live: Res<LiveParticles>,
    mut ent_materials: ResMut<Assets<EntityMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut material_cache: ResMut<ParticleMaterialCache>,
    mut batches: ResMut<ParticleBatches>,
    camera: Query<&GlobalTransform, With<WorldCamera>>,
    mut drawn: Query<
        (
            &mut Transform,
            &mut MeshMaterial3d<EntityMaterial>,
            &mut Visibility,
        ),
        With<ParticleBatch>,
    >,
    mut commands: Commands,
) {
    let Some(camera) = camera.iter().next() else {
        return;
    };
    if live.0.is_empty() {
        material_cache.0.clear();
        hide_from(&batches.pool, 0, &mut drawn);
        return;
    }

    let dim = crate::renderer::dimension::current();
    let dirs = diffuse_lights(dim);
    let terrain_atlas = particle_atlas(&chunk_material, &standard);
    let sheet = atlas.0.as_ref();
    let origin = (camera.translation() / 16.0).floor() * 16.0;
    let rotation = camera.rotation();
    let right = rotation * Vec3::X;
    let up = rotation * Vec3::Y;
    let normal = (rotation * Vec3::Z).to_array();

    let used = {
        let ParticleBatches {
            index,
            keys,
            groups,
            ..
        } = &mut *batches;
        index.clear();
        let mut used = 0usize;
        for (i, particle) in live.0.iter().enumerate() {
            let terrain = particle.flags & TERRAIN != 0;
            if terrain && terrain_atlas.is_none() {
                continue;
            }
            if !terrain && sheet.is_none() {
                continue;
            }
            let (block_light, sky_light) = if particle.flags & FULLBRIGHT != 0 {
                (15, 15)
            } else {
                crate::renderer::lightmap::levels_at([
                    particle.pos.x,
                    particle.pos.y,
                    particle.pos.z,
                ])
            };
            let key = BatchKey {
                block_light,
                sky_light,
                dim: dim as u8,
                terrain,
                translucent: particle.flags & TRANSLUCENT != 0,
            };
            let slot = match index.get(&key) {
                Some(&slot) => slot,
                None => {
                    if used >= MAX_PARTICLE_BATCHES {
                        continue;
                    }
                    let slot = used;
                    used += 1;
                    if groups.len() <= slot {
                        groups.push(Vec::new());
                        keys.push(key);
                    } else {
                        keys[slot] = key;
                    }
                    groups[slot].clear();
                    index.insert(key, slot);
                    slot
                }
            };
            groups[slot].push(i as u32);
        }
        used
    };

    for slot in 0..used {
        let key = batches.keys[slot];
        let image = match (key.terrain, terrain_atlas.as_ref(), sheet) {
            (true, Some(atlas), _) => atlas.clone(),
            (false, _, Some(sheet)) => sheet.image.clone(),
            _ => continue,
        };
        let material = batch_material(
            &mut material_cache,
            &mut ent_materials,
            &lightmap.current,
            &image,
            dirs,
            key,
        );
        if batches.pool.len() <= slot {
            let mesh = meshes.add(empty_particle_mesh());
            let entity = commands
                .spawn((
                    ParticleBatch,
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::from_translation(origin),
                    NoFrustumCulling,
                ))
                .id();
            #[cfg(feature = "builtin_shaders")]
            commands.entity(entity).insert(bevy::light::NotShadowCaster);
            batches.pool.push((entity, mesh));
        } else if let Ok((mut transform, mut mesh_material, mut visibility)) =
            drawn.get_mut(batches.pool[slot].0)
        {
            if transform.translation != origin {
                transform.translation = origin;
            }
            if mesh_material.0 != material {
                mesh_material.0 = material;
            }
            if *visibility != Visibility::Inherited {
                *visibility = Visibility::Inherited;
            }
        }
        let handle = batches.pool[slot].1.clone();
        let Some(mesh) = meshes.get_mut(&handle) else {
            continue;
        };
        write_batch_mesh(
            mesh,
            &live.0,
            &batches.groups[slot],
            origin,
            right,
            up,
            normal,
            sheet,
        );
    }
    hide_from(&batches.pool, used, &mut drawn);

    for (key, handle) in material_cache.0.iter() {
        let Some(material) = ent_materials.get_mut(handle) else {
            continue;
        };
        material.params.set(Lit {
            tint: [1.0; 4],
            light: crate::renderer::lightmap::cell_light(
                &lightmap.current,
                key.block_light,
                key.sky_light,
            ),
            dirs,
            overlay_alpha: 1.0,
            overlay_white: false,
        });
    }
}

fn hide_from(
    pool: &[(Entity, Handle<Mesh>)],
    first: usize,
    drawn: &mut Query<
        (
            &mut Transform,
            &mut MeshMaterial3d<EntityMaterial>,
            &mut Visibility,
        ),
        With<ParticleBatch>,
    >,
) {
    for (entity, _) in pool.iter().skip(first) {
        if let Ok((_, _, mut visibility)) = drawn.get_mut(*entity)
            && *visibility != Visibility::Hidden
        {
            *visibility = Visibility::Hidden;
        }
    }
}

fn batch_material(
    cache: &mut ParticleMaterialCache,
    materials: &mut Assets<EntityMaterial>,
    lightmap: &crate::renderer::lightmap::State,
    image: &Handle<Image>,
    dirs: [Vec3; 2],
    key: BatchKey,
) -> Handle<EntityMaterial> {
    if let Some(handle) = cache.0.get(&key) {
        return handle.clone();
    }
    if cache.0.len() >= MAX_CACHED_MATERIALS {
        cache.0.clear();
    }
    let cutoff = if key.translucent { 0.0 } else { 0.1 };
    let handle = materials.add(EntityMaterial {
        texture: Some(image.clone()),
        params: EntityParams::new(
            LightMode::Text,
            cutoff,
            Lit {
                tint: [1.0; 4],
                light: crate::renderer::lightmap::cell_light(
                    lightmap,
                    key.block_light,
                    key.sky_light,
                ),
                dirs,
                overlay_alpha: 1.0,
                overlay_white: false,
            },
        ),
        alpha_mode: if key.translucent {
            AlphaMode::Blend
        } else {
            AlphaMode::Mask(0.1)
        },
        depth_bias: 0,
        see_through: false,
    });
    cache.0.insert(key, handle.clone());
    handle
}

#[allow(clippy::too_many_arguments)]
fn write_batch_mesh(
    mesh: &mut Mesh,
    particles: &[Particle],
    group: &[u32],
    origin: Vec3,
    right: Vec3,
    up: Vec3,
    normal: [f32; 3],
    sheet: Option<&Atlas>,
) {
    let mut positions = match mesh.remove_attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(v)) => v,
        _ => Vec::new(),
    };
    let mut normals = match mesh.remove_attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(VertexAttributeValues::Float32x3(v)) => v,
        _ => Vec::new(),
    };
    let mut uvs = match mesh.remove_attribute(Mesh::ATTRIBUTE_UV_0) {
        Some(VertexAttributeValues::Float32x2(v)) => v,
        _ => Vec::new(),
    };
    let mut colors = match mesh.remove_attribute(Mesh::ATTRIBUTE_COLOR) {
        Some(VertexAttributeValues::Float32x4(v)) => v,
        _ => Vec::new(),
    };
    let mut indices = match mesh.remove_indices() {
        Some(Indices::U32(v)) => v,
        _ => Vec::new(),
    };
    positions.clear();
    normals.clear();
    uvs.clear();
    colors.clear();
    indices.clear();

    for &i in group {
        let p = &particles[i as usize];
        let lifetime = p.lifetime.max(1);
        let frac = (p.tick_acc / TICK).clamp(0.0, 1.0);
        let centre = p.prev.lerp(p.pos, frac) - origin;
        let t = (p.age as f32 + frac) / lifetime as f32;
        let scale = p.size
            * match p.grow {
                Grow::Fixed => 1.0,
                Grow::In => (t * 32.0).clamp(0.0, 1.0),
                Grow::Flame => 1.0 - t * t * 0.5,
                Grow::Lava => 1.0 - t * t,
                Grow::Portal => 1.0 - (1.0 - t) * (1.0 - t),
            };
        if scale <= 0.0 {
            continue;
        }
        let [u0, v0, u1, v1] = match p.sprite {
            Sprite::Fixed(uv) => uv,
            Sprite::Animated(index) => {
                let Some(sheet) = sheet else { continue };
                let frames = sheet.sheets[index as usize].len();
                let frame = p.age as usize * frames.saturating_sub(1) / lifetime as usize;
                sheet.frame_uv(index, frame)
            }
        };
        let base = positions.len() as u32;
        let dx = right * scale;
        let dy = up * scale;
        for (corner, uv) in [
            (centre - dx - dy, [u0, v1]),
            (centre + dx - dy, [u1, v1]),
            (centre + dx + dy, [u1, v0]),
            (centre - dx + dy, [u0, v0]),
        ] {
            positions.push(corner.to_array());
            normals.push(normal);
            uvs.push(uv);
            colors.push(p.tint);
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
}

fn empty_particle_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<[f32; 2]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
    mesh.insert_indices(Indices::U32(Vec::new()));
    mesh
}

fn particle_atlas(
    chunk_material: &ChunkMaterial,
    standard: &Assets<StandardMaterial>,
) -> Option<Handle<Image>> {
    standard
        .get(&chunk_material.0)
        .and_then(|m| m.base_color_texture.clone())
}

fn particle_uv(state: azalea::block::BlockState) -> Option<[f32; 4]> {
    use azalea::block::BlockTrait;

    if state.is_air() {
        return None;
    }
    let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
    let name = block.id();
    let props = block.property_map();
    let baked = crate::blocks::registry().baked(state.id(), name, &props);
    let model = baked.parts.first()?.choices.first()?;
    if let Some(uv) = model.particle_uv {
        return Some(uv);
    }
    let quad = model.quads.first()?;
    let us = [quad.uv[0][0], quad.uv[1][0], quad.uv[2][0], quad.uv[3][0]];
    let vs = [quad.uv[0][1], quad.uv[1][1], quad.uv[2][1], quad.uv[3][1]];
    Some([
        us.into_iter().fold(f32::INFINITY, f32::min),
        vs.into_iter().fold(f32::INFINITY, f32::min),
        us.into_iter().fold(f32::NEG_INFINITY, f32::max),
        vs.into_iter().fold(f32::NEG_INFINITY, f32::max),
    ])
}

fn tint_ratio(
    state: azalea::block::BlockState,
    pos: [i32; 3],
    world: Option<&azalea_world::World>,
) -> [f32; 3] {
    use crate::renderer::TintKind;
    use azalea::block::BlockTrait;
    use azalea_registry::DataRegistry;

    let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
    if block.id() == "grass_block" {
        return crate::util::biome_color::NEUTRAL;
    }

    let kind = crate::util::block_model::block_visual(state).tint_kind;
    if let TintKind::Redstone(power) = kind {
        return crate::util::biome_color::redstone_ratio(power);
    }
    if kind == TintKind::None {
        return crate::util::biome_color::NEUTRAL;
    }

    let Some(world) = world else {
        return crate::util::biome_color::NEUTRAL;
    };
    let block_pos = azalea_core::position::BlockPos {
        x: pos[0],
        y: pos[1],
        z: pos[2],
    };
    let Some(biome) = world.get_biome(block_pos) else {
        return crate::util::biome_color::NEUTRAL;
    };
    let table = crate::client::worker::biome_table(&world.registries);
    let idx = table
        .get(biome.protocol_id() as usize)
        .copied()
        .unwrap_or(u8::MAX);

    match kind {
        TintKind::None => unreachable!(),
        TintKind::Redstone(_) => unreachable!(),
        TintKind::Grass => crate::util::biome_color::grass_ratio(idx),
        TintKind::Foliage => crate::util::biome_color::foliage_ratio(idx),
        TintKind::DryFoliage => crate::util::biome_color::dry_foliage_ratio(idx),
        TintKind::Water => crate::util::biome_color::water_ratio(idx),
    }
}

fn random_sub_uv(tile: [f32; 4], rng: &mut JavaRandom) -> [f32; 4] {
    let [u0, v0, u1, v1] = tile;
    let tw = (u1 - u0) / 4.0;
    let th = (v1 - v0) / 4.0;
    let cu = rng.next_int(4) as f32;
    let cv = rng.next_int(4) as f32;
    [
        u0 + cu * tw,
        v0 + cv * th,
        u0 + (cu + 1.0) * tw,
        v0 + (cv + 1.0) * th,
    ]
}
