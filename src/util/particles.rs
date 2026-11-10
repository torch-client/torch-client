use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::math::{DVec3, IVec3};
use bevy::mesh::{Indices, Mesh, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

use azalea::entity::particle::Particle as P;
use azalea_core::heightmap_kind::HeightmapKind;

use crate::entities::quads::QUAD_INDICES;
use crate::renderer::entity_material::{
    EntityMaterial, EntityParams, LightMode, Lit, diffuse_lights,
};
use crate::renderer::systems::{ChunkMaterial, LightmapState, Shared};
use crate::renderer::{AppState, RenderOrigin};
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
}

crate::util::named_cycle::named_cycle!(ParticleStatus);

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
    fn normal(kind: &P) -> SpawnGate {
        SpawnGate {
            override_limiter: overrides_limiter(kind),
            always_show: false,
        }
    }
    const ALWAYS_SHOW: SpawnGate = SpawnGate {
        override_limiter: false,
        always_show: true,
    };

    fn allows(&self, pos: DVec3, eye: DVec3, status: ParticleStatus, rng: &mut JavaRandom) -> bool {
        self.override_limiter
            || (eye.distance_squared(pos) <= f64::from(PARTICLE_CULL_DISTANCE_SQ)
                && allow_particle(status, self.always_show, rng))
    }
}

#[derive(Resource)]
struct ParticleRng(JavaRandom);

impl Default for ParticleRng {
    fn default() -> Self {
        ParticleRng(JavaRandom::time_seeded())
    }
}

#[derive(Resource, Default)]
struct LiveParticles(Vec<Particle>);

#[derive(Resource, Default)]
pub(crate) struct ParticleAtlas(Option<Atlas>);

impl ParticleAtlas {
    pub(crate) fn is_built(&self) -> bool {
        self.0.is_some()
    }

    pub(crate) fn adopt(
        &mut self,
        built: particle_assets::ParticleAtlasData,
        images: &mut Assets<Image>,
    ) {
        let Some(atlas) = &mut self.0 else { return };
        let (image, rebuilt) = Atlas::from_data(built);
        let _ = images.insert(&atlas.image, image);
        *atlas = Atlas {
            image: atlas.image.clone(),
            ..rebuilt
        };
    }
}

pub(crate) struct Atlas {
    image: Handle<Image>,
    rows: u32,
    sheets: Vec<Vec<u32>>,
    by_name: HashMap<String, u16>,
    crit: Option<[f32; 4]>,
    items: Option<std::sync::Arc<crate::gui::atlas::GuiAtlas>>,
}

impl Atlas {
    fn from_data(built: particle_assets::ParticleAtlasData) -> (Image, Atlas) {
        let mut sheets = Vec::with_capacity(built.sheets.len());
        let mut by_name = HashMap::with_capacity(built.sheets.len());
        for (name, tiles) in built.sheets {
            by_name.insert(name, sheets.len() as u16);
            sheets.push(tiles);
        }
        let crit = by_name
            .get("crit")
            .map(|&i| particle_assets::tile_uv(sheets[i as usize][0], built.rows));
        let atlas = Atlas {
            image: Handle::default(),
            rows: built.rows,
            sheets,
            by_name,
            crit,
            items: None,
        };
        (built.image, atlas)
    }

    fn item_uv(&self, id: &str) -> Option<[f32; 4]> {
        let items = self.items.as_ref()?;
        let region = items.item_for_size(id, 16.0)?;
        let [u0, v0] = items.uv(region.x as f32, region.y as f32);
        let [u1, v1] = items.uv((region.x + region.w) as f32, (region.y + region.h) as f32);
        Some([u0, v0, u1, v1])
    }

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
    let (image, built) = Atlas::from_data(built);
    atlas.0 = Some(Atlas {
        image: images.add(image),
        ..built
    });
}

#[derive(Resource, Default)]
struct ParticleMaterialCache(HashMap<BatchKey, Handle<EntityMaterial>>);

const MAX_CACHED_MATERIALS: usize = 256;

pub(crate) const MAX_LIVE_PARTICLES: usize = 5000;

const MAX_PARTICLE_BATCHES: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Sheet {
    Particles,
    Terrain,
    Items,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct BatchKey {
    block_light: u8,
    sky_light: u8,
    dim: u8,
    sheet: Sheet,
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
    prev: DVec3,
    pos: DVec3,
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
    drip: Option<(Drip, DripStage)>,
    origin: DVec3,
    motion: Motion,
    light: Light,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Drip {
    Water,
    Lava,
    Honey,
    DripstoneWater,
    DripstoneLava,
    ObsidianTear,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DripStage {
    Hang,
    Fall,
    Landed,
    Land,
}

impl Drip {
    fn falling(self) -> P {
        match self {
            Drip::Water => P::FallingWater,
            Drip::Lava => P::FallingLava,
            Drip::Honey => P::FallingHoney,
            Drip::DripstoneWater => P::FallingDripstoneWater,
            Drip::DripstoneLava => P::FallingDripstoneLava,
            Drip::ObsidianTear => P::FallingObsidianTear,
        }
    }

    fn landing(self) -> P {
        match self {
            Drip::Water | Drip::DripstoneWater => P::Splash,
            Drip::Lava | Drip::DripstoneLava => P::LandingLava,
            Drip::Honey => P::LandingHoney,
            Drip::ObsidianTear => P::LandingObsidianTear,
        }
    }

    fn landing_sound(self) -> Option<&'static str> {
        match self {
            Drip::DripstoneWater => Some("block.pointed_dripstone.drip_water"),
            Drip::DripstoneLava => Some("block.pointed_dripstone.drip_lava"),
            Drip::Honey => Some("block.beehive.drip"),
            _ => None,
        }
    }

    fn fluid(self) -> Option<azalea::block::fluid_state::FluidKind> {
        use azalea::block::fluid_state::FluidKind;
        match self {
            Drip::Water | Drip::DripstoneWater => Some(FluidKind::Water),
            Drip::Lava | Drip::DripstoneLava => Some(FluidKind::Lava),
            Drip::Honey | Drip::ObsidianTear => None,
        }
    }

    fn cools(self) -> bool {
        matches!(self, Drip::Lava | Drip::DripstoneLava)
    }
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Motion {
    Integrate,
    Campfire,
    Portal,
    FlyTowards { vault: bool },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Light {
    Cell,
    Linear,
    Quartic,
    Block,
}

const PHYSICS: u8 = 1 << 0;
const TERRAIN: u8 = 1 << 1;
const TRANSLUCENT: u8 = 1 << 2;
const FULLBRIGHT: u8 = 1 << 3;
const CRIT_DECAY: u8 = 1 << 4;
const FADE_OUT: u8 = 1 << 5;
const WATER_DROP: u8 = 1 << 6;
const ITEM: u8 = 1 << 7;

const DESTROY_GRID: i32 = 4;

fn spawn_particles(
    shared: Res<Shared>,
    gui: Res<crate::gui::GuiState>,
    gui_assets: Option<Res<crate::gui::render::GuiAssets>>,
    mut atlas: ResMut<ParticleAtlas>,
    view: Res<crate::renderer::FrameView>,
    mut live: ResMut<LiveParticles>,
    mut particle_rng: ResMut<ParticleRng>,
    mut elder_guardian: ResMut<crate::entities::elder_guardian::ElderGuardianCurse>,
) {
    use crate::session::ParticleEmit;

    let emits = std::mem::take(&mut shared.0.lock().unwrap().session.particle_emits);
    if emits.is_empty() {
        return;
    }
    if let (Some(atlas), Some(gui_assets)) = (&mut atlas.0, &gui_assets) {
        atlas.items = Some(gui_assets.atlas.clone());
    }
    let status = gui.options.particles;
    let eye = view.camera_world;
    let world_arc = particle_world();
    let world = world_arc
        .as_deref()
        .map(|w| crate::client::tracking::read_lock(w, "spawn_particles"));
    let world = world.as_deref();
    let atlas = atlas.0.as_ref();
    let rng = &mut particle_rng.0;

    let [
        mut breaks,
        mut levels,
        mut events,
        mut blasts,
        mut rounds,
        mut uses,
    ] = [0u32; 6];
    let before = live.0.len();
    let mut explosions = Vec::new();

    for emit in emits {
        if live.0.len() >= MAX_LIVE_PARTICLES {
            break;
        }
        match emit {
            ParticleEmit::Block(spawn) => {
                breaks += 1;
                spawn_block_break(&spawn, world, &mut live.0, rng);
            }
            ParticleEmit::Level(level) => {
                levels += 1;
                if matches!(level.particle, P::ElderGuardian) {
                    elder_guardian.add(if level.count == 0 {
                        1
                    } else {
                        level.count.min(MAX_EMIT_COUNT)
                    });
                    continue;
                }
                if let Some(atlas) = atlas {
                    spawn_level_particle(&level, atlas, world, eye, status, &mut live.0, rng);
                }
            }
            ParticleEmit::Event(event) => {
                events += 1;
                let (Some(atlas), Some(world)) = (atlas, world) else {
                    continue;
                };
                let mut ctx = Emitter {
                    cell: event.pos,
                    world,
                    atlas,
                    status,
                    sky_darken: frame_sky_darken(&view),
                    day_ticks: view.day_ticks,
                    eye,
                    live: &mut live.0,
                    rng: &mut *rng,
                };
                crate::util::level_events::spawn(&event, &mut ctx);
            }
            ParticleEmit::Explosion(explosion) => {
                blasts += 1;
                let Some(atlas) = atlas else { continue };
                let c = explosion.center;
                add_level_particle(
                    &mut live.0,
                    atlas,
                    world,
                    rng,
                    status,
                    &explosion.particle,
                    SpawnGate::normal(&explosion.particle),
                    DVec3::from_array(c),
                    Vec3::X,
                    eye,
                );
                if !explosion.block_particles.is_empty() {
                    explosions.push(explosion);
                }
            }
            ParticleEmit::Tracking(round) => {
                rounds += 1;
                let Some(atlas) = atlas else { continue };
                spawn_tracking_round(&round, atlas, world, eye, status, &mut live.0, rng);
            }
            ParticleEmit::Cloud(cloud) => {
                levels += 1;
                let Some(atlas) = atlas else { continue };
                spawn_cloud_round(&cloud, atlas, world, eye, status, &mut live.0, rng);
            }
            ParticleEmit::ItemUse(item_use) => {
                uses += 1;
                let Some(atlas) = atlas else { continue };
                spawn_item_use(&item_use, atlas, world, eye, status, &mut live.0, rng);
            }
        }
    }

    if let (Some(atlas), Some(world)) = (atlas, world) {
        let mut ctx = Emitter {
            cell: [0; 3],
            world,
            atlas,
            status,
            sky_darken: frame_sky_darken(&view),
            day_ticks: view.day_ticks,
            eye,
            live: &mut live.0,
            rng,
        };
        scatter_explosions(&explosions, &mut ctx);
    }

    if breaks + levels + events + blasts + rounds + uses > 0 {
        crate::log_debug!(
            "particles",
            "drained {breaks} break, {levels} level, {events} event, {blasts} explosion, \
             {rounds} tracking, {uses} item use: {} live, was {before}",
            live.0.len()
        );
    }
}

fn spawn_item_use(
    emit: &crate::session::ItemUseEmit,
    atlas: &Atlas,
    world: Option<&azalea_world::World>,
    eye: DVec3,
    status: ParticleStatus,
    live: &mut Vec<Particle>,
    rng: &mut JavaRandom,
) {
    use crate::util::mth::DEG_TO_RAD;

    let (x_sin, x_cos) = (-emit.x_rot * DEG_TO_RAD).sin_cos();
    let (y_sin, y_cos) = (-emit.y_rot * DEG_TO_RAD).sin_cos();
    let (x_sin, x_cos, y_sin, y_cos) = (
        f64::from(x_sin),
        f64::from(x_cos),
        f64::from(y_sin),
        f64::from(y_cos),
    );
    let turn = |v: DVec3| {
        let (y, z) = (v.y * x_cos + v.z * x_sin, v.z * x_cos - v.y * x_sin);
        DVec3::new(v.x * y_cos + z * y_sin, y, z * y_cos - v.x * y_sin)
    };
    let gate = SpawnGate {
        override_limiter: false,
        always_show: false,
    };
    for _ in 0..emit.count {
        if live.len() >= MAX_LIVE_PARTICLES {
            break;
        }
        let d = DVec3::new(
            (f64::from(rng.next_f32()) - 0.5) * 0.1,
            f64::from(rng.next_f32()) * 0.1 + 0.1,
            0.0,
        );
        let d = turn(d);
        let y1 = f64::from(-rng.next_f32()) * 0.6 - 0.3;
        let p = turn(DVec3::new((f64::from(rng.next_f32()) - 0.5) * 0.3, y1, 0.6));
        let pos = DVec3::from_array(emit.eye) + p;
        if !gate.allows(pos, eye, status, rng) {
            continue;
        }
        let aux = Vec3::new(d.x as f32, (d.y + 0.05) as f32, d.z as f32);
        if let Some(recipe) = item_recipe(atlas, emit.item, Some(aux), rng) {
            push_recipe(live, world, rng, recipe, pos);
        }
    }
}

const TRACKING_CANDIDATES: u32 = 16;

fn spawn_tracking_round(
    round: &crate::session::TrackingEmit,
    atlas: &Atlas,
    world: Option<&azalea_world::World>,
    eye: DVec3,
    status: ParticleStatus,
    live: &mut Vec<Particle>,
    rng: &mut JavaRandom,
) {
    let feet = DVec3::from_array(round.pos);
    let [width, height] = round.size.map(f64::from);
    let gate = SpawnGate::normal(&round.particle);
    for _ in 0..TRACKING_CANDIDATES {
        if live.len() >= MAX_LIVE_PARTICLES {
            break;
        }
        let xa = rng.next_f32() * 2.0 - 1.0;
        let ya = rng.next_f32() * 2.0 - 1.0;
        let za = rng.next_f32() * 2.0 - 1.0;
        if xa * xa + ya * ya + za * za > 1.0 {
            continue;
        }
        let pos = feet
            + DVec3::new(
                width * f64::from(xa / 4.0),
                height * f64::from(0.5 + ya / 4.0),
                width * f64::from(za / 4.0),
            );
        add_level_particle(
            live,
            atlas,
            world,
            rng,
            status,
            &round.particle,
            gate,
            pos,
            Vec3::new(xa, ya + 0.2, za),
            eye,
        );
    }
}

fn spawn_cloud_round(
    cloud: &crate::session::CloudEmit,
    atlas: &Atlas,
    world: Option<&azalea_world::World>,
    eye: DVec3,
    status: ParticleStatus,
    live: &mut Vec<Particle>,
    rng: &mut JavaRandom,
) {
    if cloud.waiting && rng.next_bool() {
        return;
    }
    let (count, radius) = if cloud.waiting {
        (2, 0.2)
    } else {
        (
            (std::f32::consts::PI * cloud.radius * cloud.radius).ceil() as u32,
            cloud.radius,
        )
    };
    let default = crate::play::area_cloud::default_particle();
    let entity_effect = matches!(cloud.particle, P::EntityEffect(_));
    let [x, y, z] = cloud.pos;
    for _ in 0..count {
        if live.len() >= MAX_LIVE_PARTICLES {
            break;
        }
        let angle = rng.next_f32() * std::f32::consts::TAU;
        let distance = rng.next_f32().sqrt() * radius;
        let pos = DVec3::new(
            x + f64::from(angle.cos() * distance),
            y,
            z + f64::from(angle.sin() * distance),
        );
        let (particle, arg) = if entity_effect {
            let kind = if cloud.waiting && rng.next_bool() {
                &default
            } else {
                &cloud.particle
            };
            (kind, Vec3::ZERO)
        } else if cloud.waiting {
            (&cloud.particle, Vec3::ZERO)
        } else {
            let xa = ((0.5 - rng.next_f64()) * 0.15) as f32;
            let za = ((0.5 - rng.next_f64()) * 0.15) as f32;
            (&cloud.particle, Vec3::new(xa, 0.01, za))
        };
        add_level_particle(
            live,
            atlas,
            world,
            rng,
            status,
            particle,
            SpawnGate::ALWAYS_SHOW,
            pos,
            arg,
            eye,
        );
    }
}

const MAX_EXPLOSION_PARTICLES: i32 = 512;

fn scatter_explosions(explosions: &[crate::session::ExplosionEmit], ctx: &mut Emitter) {
    if ctx.status != ParticleStatus::All {
        return;
    }
    let total = explosions
        .iter()
        .fold(0i32, |sum, e| sum.saturating_add(e.block_count.max(0)));
    for _ in 0..total.min(MAX_EXPLOSION_PARTICLES) {
        if let Some(explosion) = weighted_pick(explosions, |e| e.block_count, ctx.rng) {
            scatter_one(explosion, ctx);
        }
    }
}

fn scatter_one(explosion: &crate::session::ExplosionEmit, ctx: &mut Emitter) {
    let dir = Vec3::new(
        ctx.f32() * 2.0 - 1.0,
        ctx.f32() * 2.0 - 1.0,
        ctx.f32() * 2.0 - 1.0,
    )
    .normalize_or_zero();
    let radius = ctx.f32().cbrt() * explosion.radius;
    let local = dir * radius;
    let c = explosion.center;
    let center = DVec3::from_array(c);
    let cell = (center + local.as_dvec3()).floor();
    if ctx
        .block([cell.x as i32, cell.y as i32, cell.z as i32])
        .is_some_and(|s| !s.is_air())
    {
        return;
    }
    let speed = 0.5 / (radius / explosion.radius + 0.1) * ctx.f32() * ctx.f32() + 0.3;
    let Some(info) = weighted_pick(&explosion.block_particles, |w| w.weight, ctx.rng) else {
        return;
    };
    let info = &info.value;
    ctx.emit(
        &info.particle,
        center + (local * info.scaling).as_dvec3(),
        dir * (speed * info.speed),
        false,
    );
}

fn weighted_pick<'a, T>(
    items: &'a [T],
    weight: impl Fn(&T) -> i32,
    rng: &mut JavaRandom,
) -> Option<&'a T> {
    let total = items
        .iter()
        .fold(0i32, |sum, i| sum.saturating_add(weight(i).max(0)));
    if total <= 0 {
        return None;
    }
    let mut pick = rng.next_int(total as u32) as i32;
    items.iter().find(|i| {
        pick -= weight(i).max(0);
        pick < 0
    })
}

pub(crate) fn spawn_block_break(
    spawn: &crate::session::ParticleSpawn,
    world: Option<&azalea_world::World>,
    live: &mut Vec<Particle>,
    particle_rng: &mut JavaRandom,
) {
    let Some(tile) = particle_uv(spawn.state) else {
        return;
    };
    let block_min = IVec3::from_array(spawn.pos).as_dvec3();
    let seed = ((particle_rng.next_int(i32::MAX as u32) as i64) << 31)
        | particle_rng.next_int(i32::MAX as u32) as i64;
    let mut rng = JavaRandom::new(seed);

    let points: Vec<(DVec3, Vec3)> = if spawn.mining {
        let mut pos = block_min
            + Vec3::new(
                0.1 + rng.next_f32() * 0.8,
                0.1 + rng.next_f32() * 0.8,
                0.1 + rng.next_f32() * 0.8,
            )
            .as_dvec3();
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
                    out.push((block_min + rel.as_dvec3(), rel - Vec3::splat(0.5)));
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
            drip: None,
            origin: pos,
            motion: Motion::Integrate,
            light: Light::Cell,
        };
        if particle.lifetime > 0 {
            particle.age = 1;
            advance_one_tick(&mut particle, world, &mut rng);
        }
        live.push(particle);
    }
}

const MAX_EMIT_COUNT: u32 = 512;

const PARTICLE_CULL_DISTANCE_SQ: f32 = 1024.0;

#[allow(clippy::too_many_arguments)]
fn spawn_level_particle(
    emit: &crate::session::LevelParticle,
    atlas: &Atlas,
    world: Option<&azalea_world::World>,
    eye: DVec3,
    status: ParticleStatus,
    live: &mut Vec<Particle>,
    rng: &mut JavaRandom,
) {
    let base = DVec3::from_array(emit.pos);
    let gate = SpawnGate {
        override_limiter: emit.override_limiter || overrides_limiter(&emit.particle),
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
            )
            .as_dvec3();
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
    pos: DVec3,
    arg: Vec3,
    eye: DVec3,
) {
    if !gate.allows(pos, eye, status, rng) {
        return;
    }
    if matches!(particle_kind, P::ExplosionEmitter) {
        spawn_explosion_seed(live, atlas, world, rng, status, pos, eye);
        return;
    }
    if matches!(particle_kind, P::GustEmitterLarge) {
        spawn_gust_seed(live, atlas, world, rng, status, pos, eye, 3.0, 7, 0);
        return;
    }
    if matches!(particle_kind, P::GustEmitterSmall) {
        spawn_gust_seed(live, atlas, world, rng, status, pos, eye, 1.0, 3, 2);
        return;
    }
    if matches!(particle_kind, P::NoxiousGasCloud) {
        spawn_noxious_gas_cloud_seed(live, atlas, world, rng, status, pos, eye);
        return;
    }
    if let P::Geyser(options) = particle_kind {
        spawn_geyser_eruption_seed(
            live,
            atlas,
            world,
            rng,
            status,
            pos,
            arg,
            eye,
            options.water_blocks,
        );
        return;
    }
    let pos = match particle_kind {
        P::GeyserBase(_) | P::GeyserPoof(_) => DVec3::new(
            pos.x + f64::from((rng.next_f32() - 0.5) * 0.5),
            pos.y + f64::from((rng.next_f32() - 0.5) * 0.5) + 0.200_000_002_98,
            pos.z + f64::from((rng.next_f32() - 0.5) * 0.5),
        ),
        P::GeyserPlume(_) => DVec3::new(
            pos.x + f64::from((rng.next_f32() - 0.5) * 0.2),
            pos.y + f64::from(rng.next_f32()),
            pos.z + f64::from((rng.next_f32() - 0.5) * 0.2),
        ),
        _ => pos,
    };
    if let Some(recipe) = recipe(particle_kind, atlas, arg, rng) {
        push_recipe(live, world, rng, recipe, pos);
    }
}

fn push_recipe(
    live: &mut Vec<Particle>,
    world: Option<&azalea_world::World>,
    rng: &mut JavaRandom,
    recipe: Recipe,
    pos: DVec3,
) {
    let origin = pos + recipe.nudge;
    let pos = match recipe.motion {
        Motion::FlyTowards { .. } => origin + recipe.velocity.as_dvec3(),
        _ => origin,
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
        drip: recipe.drip,
        origin,
        motion: recipe.motion,
        light: recipe.light,
    };
    particle.age = 1;
    advance_one_tick(&mut particle, world, rng);
    live.push(particle);
}

pub(crate) fn overrides_limiter(kind: &P) -> bool {
    matches!(
        kind,
        P::BlockMarker(_)
            | P::DamageIndicator
            | P::ElderGuardian
            | P::ExplosionEmitter
            | P::Explosion
            | P::Gust
            | P::GustEmitterLarge
            | P::GustEmitterSmall
            | P::SonicBoom
            | P::SculkCharge(_)
            | P::SculkChargePop
            | P::Vibration(_)
            | P::Poof
            | P::Spit
            | P::SquidInk
            | P::SweepAttack
            | P::CampfireCosySmoke
            | P::CampfireSignalSmoke
            | P::GlowSquidInk
            | P::Glow
            | P::WaxOn
            | P::WaxOff
            | P::ElectricSpark
            | P::Scrape
            | P::TrialSpawnerDetection
            | P::TrialSpawnerDetectionOminous
            | P::VaultConnection
            | P::OminousSpawning
            | P::Geyser(_)
            | P::GeyserBase(_)
            | P::GeyserPoof(_)
            | P::GeyserPlume(_)
    )
}

const SEED_TICKS: u32 = 8;

fn spawn_explosion_seed(
    live: &mut Vec<Particle>,
    atlas: &Atlas,
    world: Option<&azalea_world::World>,
    rng: &mut JavaRandom,
    status: ParticleStatus,
    pos: DVec3,
    eye: DVec3,
) {
    for tick in 0..SEED_TICKS {
        if !seed_puffs(
            live,
            atlas,
            world,
            rng,
            status,
            pos,
            eye,
            &P::Explosion,
            6,
            4.0,
            tick,
            SEED_TICKS,
        ) {
            return;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn seed_puffs(
    live: &mut Vec<Particle>,
    atlas: &Atlas,
    world: Option<&azalea_world::World>,
    rng: &mut JavaRandom,
    status: ParticleStatus,
    pos: DVec3,
    eye: DVec3,
    kind: &P,
    count: u32,
    scale: f64,
    tick: u32,
    lifetime: u32,
) -> bool {
    fn spread(rng: &mut JavaRandom, scale: f64) -> f64 {
        (rng.next_f64() - rng.next_f64()) * scale
    }
    for _ in 0..count {
        if live.len() >= MAX_LIVE_PARTICLES {
            return false;
        }
        let offset = DVec3::new(spread(rng, scale), spread(rng, scale), spread(rng, scale));
        let len = live.len();
        add_level_particle(
            live,
            atlas,
            world,
            rng,
            status,
            kind,
            SpawnGate::normal(kind),
            pos + offset,
            Vec3::new(tick as f32 / lifetime as f32, 0.0, 0.0),
            eye,
        );
        if let Some(puff) = live.get_mut(len) {
            puff.tick_acc -= tick as f32 * TICK;
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn spawn_gust_seed(
    live: &mut Vec<Particle>,
    atlas: &Atlas,
    world: Option<&azalea_world::World>,
    rng: &mut JavaRandom,
    status: ParticleStatus,
    pos: DVec3,
    eye: DVec3,
    scale: f64,
    lifetime: u32,
    delay: u32,
) {
    for tick in (0..=lifetime).step_by(delay as usize + 1) {
        if !seed_puffs(
            live,
            atlas,
            world,
            rng,
            status,
            pos,
            eye,
            &P::Gust,
            3,
            scale,
            tick,
            lifetime,
        ) {
            return;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_held(
    live: &mut Vec<Particle>,
    atlas: &Atlas,
    world: Option<&azalea_world::World>,
    rng: &mut JavaRandom,
    status: ParticleStatus,
    eye: DVec3,
    kind: &P,
    gate: SpawnGate,
    pos: DVec3,
    arg: Vec3,
    tick: u32,
) -> bool {
    if live.len() >= MAX_LIVE_PARTICLES {
        return false;
    }
    let len = live.len();
    add_level_particle(live, atlas, world, rng, status, kind, gate, pos, arg, eye);
    if let Some(particle) = live.get_mut(len) {
        particle.tick_acc -= tick as f32 * TICK;
    }
    true
}

fn spawn_noxious_gas_cloud_seed(
    live: &mut Vec<Particle>,
    atlas: &Atlas,
    world: Option<&azalea_world::World>,
    rng: &mut JavaRandom,
    status: ParticleStatus,
    pos: DVec3,
    eye: DVec3,
) {
    let centre = pos.floor() + DVec3::splat(0.5);
    for age in (2..=20u32).step_by(2) {
        let dir = Vec3::new(rng.next_f32() - 0.5, 0.0, rng.next_f32() - 0.5).normalize_or_zero();
        let distance = rng.next_f32() * 3.0;
        let at = centre + (dir * distance).as_dvec3() - DVec3::new(0.0, 0.25, 0.0);
        if let Some(world) = world {
            let below = block_at(
                world,
                [
                    at.x.floor() as i32,
                    (at.y - 1.0).floor() as i32,
                    at.z.floor() as i32,
                ],
            );
            let in_water_above = below.is_some_and(|state| {
                let fluid = azalea::block::fluid_state::FluidState::from(state);
                fluid.kind == azalea::block::fluid_state::FluidKind::Water && fluid.is_source()
            });
            if is_solid_at(Some(world), at) || !in_water_above {
                continue;
            }
        }
        let gate = SpawnGate::ALWAYS_SHOW;
        if !spawn_held(
            live,
            atlas,
            world,
            rng,
            status,
            eye,
            &P::NoxiousGas,
            gate,
            at,
            Vec3::ZERO,
            age,
        ) {
            return;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_geyser_eruption_seed(
    live: &mut Vec<Particle>,
    atlas: &Atlas,
    world: Option<&azalea_world::World>,
    rng: &mut JavaRandom,
    status: ParticleStatus,
    pos: DVec3,
    arg: Vec3,
    eye: DVec3,
    water_blocks: i32,
) {
    use azalea::entity::particle::{GeyserBaseParticle, GeyserParticle};
    let plume = P::GeyserPlume(GeyserParticle { water_blocks });
    let base = P::GeyserBase(GeyserBaseParticle {
        water_blocks,
        burst_impulse_base: 1.5,
    });
    let poof = P::GeyserPoof(GeyserBaseParticle {
        water_blocks,
        burst_impulse_base: 2.0,
    });
    for age in 1..=21u32 {
        let mut batches: [(&P, u32); 3] = [(&base, 0), (&plume, 0), (&poof, 0)];
        if age % 2 == 0 {
            batches[0].1 = 2;
        }
        batches[1].1 = (water_blocks + 2).max(0) as u32;
        if age % 10 == 0 {
            batches[2].1 = 20;
        }
        for (kind, count) in batches {
            for _ in 0..count {
                let gate = SpawnGate::normal(kind);
                if !spawn_held(
                    live, atlas, world, rng, status, eye, kind, gate, pos, arg, age,
                ) {
                    return;
                }
            }
        }
    }
}

const ANIMATE_SAMPLES: u32 = 667;

const ANIMATE_RADII: [u32; 2] = [16, 32];

const SAMPLE_FOR_SOUND: bool = cfg!(feature = "audio");

#[derive(Resource, Default)]
struct AmbientClock(f32);

struct AnimateLevel {
    thundering: bool,
    game_time: u64,
    biomes: Option<std::sync::Arc<Vec<u8>>>,
}

fn frame_sky_darken(view: &crate::renderer::FrameView) -> i32 {
    crate::entities::shadow::sky_darken(crate::renderer::dimension::current(), view.day_ticks)
}

fn animate_tick_particles(
    time: Res<Time>,
    gui: Res<crate::gui::GuiState>,
    atlas: Res<ParticleAtlas>,
    view: Res<crate::renderer::FrameView>,
    env: Res<crate::renderer::environment::Environment>,
    mut live: ResMut<LiveParticles>,
    mut particle_rng: ResMut<ParticleRng>,
    mut clock: ResMut<AmbientClock>,
    mut rain_sound: ResMut<crate::renderer::weather::RainSoundTime>,
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
    let rain = env.rain;
    if status == ParticleStatus::Minimal && rain <= 0.0 && !SAMPLE_FOR_SOUND {
        return;
    }
    let eye = view.camera_world;
    let world_arc = particle_world();
    let Some(world) = world_arc
        .as_deref()
        .map(|w| crate::client::tracking::read_lock(w, "animate_tick"))
    else {
        return;
    };
    let light = crate::client::tracking::read_lock(
        crate::client::worldsync::light_map(),
        "light:animate_tick",
    );

    let center = crate::renderer::lightmap::cell_of(eye.to_array());
    let before = live.0.len();
    let level = AnimateLevel {
        thundering: env.thunder > 0.9,
        game_time: view.game_time,
        biomes: crate::util::biome_color::any_ambient_particles()
            .then(|| crate::client::worker::biome_table(&world.registries))
            .filter(|table| !table.is_empty()),
    };
    let mut ctx = Emitter {
        cell: center,
        world: &world,
        atlas,
        status,
        sky_darken: frame_sky_darken(&view),
        day_ticks: view.day_ticks,
        eye,
        live: &mut live.0,
        rng: &mut particle_rng.0,
    };
    'sampling: for _ in 0..ANIMATE_SAMPLES {
        if status == ParticleStatus::Minimal && !SAMPLE_FOR_SOUND {
            break;
        }
        for radius in ANIMATE_RADII {
            if ctx.live.len() >= MAX_LIVE_PARTICLES && !SAMPLE_FOR_SOUND {
                break 'sampling;
            }
            let rng = &mut *ctx.rng;
            ctx.cell = [
                center[0] + rng.next_int(radius) as i32 - rng.next_int(radius) as i32,
                center[1] + rng.next_int(radius) as i32 - rng.next_int(radius) as i32,
                center[2] + rng.next_int(radius) as i32 - rng.next_int(radius) as i32,
            ];
            do_animate_tick(&mut ctx, &light, &level);
        }
    }
    let splashes = !view.no_render.has(crate::modules::no_render::Part::Weather);
    crate::renderer::weather::tick_rain_particles(
        &mut ctx,
        rain,
        view.game_time,
        &mut rain_sound.0,
        splashes,
    );

    if live.0.len() != before {
        crate::log_debug!(
            "particles",
            "animate tick spawned {}: {} live",
            live.0.len() - before,
            live.0.len()
        );
    }
}

fn is_quiet(state: azalea::block::BlockState) -> bool {
    use std::sync::atomic::{AtomicU8, Ordering::Relaxed};
    const UNKNOWN: u8 = 0;
    const QUIET: u8 = 1;
    const ACTIVE: u8 = 2;
    const STATES: usize = azalea::block::BlockState::MAX_STATE as usize + 1;
    static CACHE: [AtomicU8; STATES] = [const { AtomicU8::new(UNKNOWN) }; STATES];

    let work_out = || {
        crate::blocks::ambient::of(state).is_none()
            && azalea::block::fluid_state::FluidState::from(state).is_empty()
    };
    let Some(slot) = CACHE.get(state.id() as usize) else {
        return work_out();
    };
    match slot.load(Relaxed) {
        QUIET => true,
        ACTIVE => false,
        _ => {
            let quiet = work_out();
            slot.store(if quiet { QUIET } else { ACTIVE }, Relaxed);
            quiet
        }
    }
}

#[cfg(test)]
#[test]
fn only_a_cell_with_nothing_to_emit_is_quiet() {
    use azalea::block::BlockState;
    use azalea::block::blocks::{Air, Stone, Torch, Water};

    for _ in 0..2 {
        assert!(is_quiet(BlockState::from(Air {})));
        assert!(is_quiet(BlockState::from(Stone {})));
        assert!(!is_quiet(BlockState::from(Torch {})));
        assert!(!is_quiet(BlockState::from(Water {
            level: azalea::block::properties::WaterLevel::from(0)
        })));
    }
}

fn do_animate_tick(
    ctx: &mut Emitter,
    light: &crate::lighting::level::LightMap,
    level: &AnimateLevel,
) {
    let Some(state) = ctx.block(ctx.cell) else {
        return;
    };
    if !is_quiet(state) {
        block_and_fluid_ambient(ctx, state, light, level);
    }
    biome_ambient(ctx, state, level);
}

fn biome_ambient(ctx: &mut Emitter, state: azalea::block::BlockState, level: &AnimateLevel) {
    use azalea::physics::collision::BlockWithShape;

    let Some(biomes) = level.biomes.as_deref() else {
        return;
    };
    if ctx.status == ParticleStatus::Minimal || state.is_collision_shape_full() {
        return;
    }
    let cell = ctx.cell;
    let idx = {
        let Some(chunk) = ctx.world.chunks.get(&azalea_core::position::ChunkPos {
            x: cell[0] >> 4,
            z: cell[2] >> 4,
        }) else {
            return;
        };
        let chunk = chunk.read();
        let min_y = ctx.world.chunks.min_y();
        let height = ctx.world.chunks.height() as i32;
        let qy = (cell[1] >> 2).clamp(min_y >> 2, ((min_y + height) >> 2) - 1);
        crate::client::envprobe::quart_biome(
            &chunk,
            min_y,
            biomes,
            [cell[0] >> 2, qy, cell[2] >> 2],
        )
    };
    for (particle, probability) in crate::util::biome_color::ambient_particles(idx) {
        if ctx.rng.next_f32() <= *probability {
            let x = ctx.rng.next_f64();
            let y = ctx.rng.next_f64();
            let z = ctx.rng.next_f64();
            let pos = DVec3::new(
                f64::from(cell[0]) + x,
                f64::from(cell[1]) + y,
                f64::from(cell[2]) + z,
            );
            ctx.emit(particle, pos, Vec3::ZERO, false);
        }
    }
}

fn block_and_fluid_ambient(
    ctx: &mut Emitter,
    state: azalea::block::BlockState,
    light: &crate::lighting::level::LightMap,
    level: &AnimateLevel,
) {
    let cell = ctx.cell;
    if let Some(ambient) = crate::blocks::ambient::of(state) {
        emit_ambient(ambient, state, ctx, light, level);
    }

    let fluid = azalea::block::fluid_state::FluidState::from(state);
    if fluid.is_empty() {
        return;
    }
    emit_fluid_ambient(&fluid, ctx);

    if ctx.rng.next_int(10) == 0 {
        let drip = match fluid.kind {
            azalea::block::fluid_state::FluidKind::Lava => P::DrippingLava,
            _ => P::DrippingWater,
        };
        let watertight = azalea::physics::collision::sturdy::is_face_sturdy(
            state,
            azalea_core::direction::Direction::Down,
        );
        try_spawn_drip(ctx, [cell[0], cell[1] - 1, cell[2]], &drip, watertight);
    }
}

fn try_spawn_drip(ctx: &mut Emitter, below: [i32; 3], drip: &P, watertight: bool) {
    use azalea::block::fluid_state::FluidState;
    use azalea::physics::collision::BlockWithShape;

    let shape_of = |state: azalea::block::BlockState, cell: [i32; 3]| {
        let shape = state.collision_shape(azalea_core::position::BlockPos::new(
            cell[0], cell[1], cell[2],
        ));
        (!shape.is_empty()).then(|| shape.bounds())
    };
    let Some(state) = ctx.block(below) else {
        return;
    };
    if !FluidState::from(state).is_empty() {
        return;
    }
    let bounds = shape_of(state, below);
    let top = bounds.as_ref().map_or(f64::NEG_INFINITY, |b| b.max.y);
    fn lerp(rng: &mut JavaRandom, lo: f64, hi: f64) -> f32 {
        (lo + rng.next_f64() * (hi - lo)) as f32
    }
    if top < 1.0 {
        if watertight {
            let x = lerp(ctx.rng, 0.0, 1.0);
            let z = lerp(ctx.rng, 0.0, 1.0);
            ctx.emit(drip, rel(below, [x, 1.0 - 0.05, z]), Vec3::ZERO, false);
        }
        return;
    }
    let Some(bounds) = bounds else {
        return;
    };
    if azalea_registry::tags::blocks::IMPERMEABLE
        .contains(&azalea_registry::builtin::BlockKind::from(state))
    {
        return;
    }
    let y = if bounds.min.y > 0.0 {
        bounds.min.y - 0.05
    } else {
        let under = [below[0], below[1] - 1, below[2]];
        let Some(under_state) = ctx.block(under) else {
            return;
        };
        let under_top = shape_of(under_state, under).map_or(f64::NEG_INFINITY, |b| b.max.y);
        if under_top >= 1.0 || !FluidState::from(under_state).is_empty() {
            return;
        }
        -0.05
    };
    let x = lerp(ctx.rng, bounds.min.x, bounds.max.x);
    let z = lerp(ctx.rng, bounds.min.z, bounds.max.z);
    ctx.emit(drip, rel(below, [x, y as f32, z]), Vec3::ZERO, false);
}

pub(crate) struct Emitter<'a> {
    pub(crate) cell: [i32; 3],
    pub(crate) world: &'a azalea_world::World,
    pub(crate) atlas: &'a Atlas,
    pub(crate) status: ParticleStatus,
    pub(crate) sky_darken: i32,
    pub(crate) day_ticks: f64,
    pub(crate) eye: DVec3,
    pub(crate) live: &'a mut Vec<Particle>,
    pub(crate) rng: &'a mut JavaRandom,
}

impl Emitter<'_> {
    pub(crate) fn emit(&mut self, kind: &P, pos: DVec3, velocity: Vec3, always: bool) {
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
                SpawnGate::normal(kind)
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

pub(crate) fn rel(cell: [i32; 3], offset: [f32; 3]) -> DVec3 {
    DVec3::new(
        f64::from(cell[0]) + f64::from(offset[0]),
        f64::from(cell[1]) + f64::from(offset[1]),
        f64::from(cell[2]) + f64::from(offset[2]),
    )
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
    light: &crate::lighting::level::LightMap,
    level: &AnimateLevel,
) {
    use crate::blocks::ambient::{Ambient, Flame, Furnace, HORIZONTAL, Leaf, WireSide};
    use crate::renderer::lightmap::levels_in;
    use crate::util::level_events::{Bus, play, play_local};
    use azalea_registry::builtin::BlockKind;
    use azalea_registry::tags::blocks as tags;

    let cell = ctx.cell;
    let centre = rel(cell, [0.5, 0.5, 0.5]).to_array();
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
                let chance = ctx.f32();
                if chance < 0.3 {
                    ctx.emit(&P::Smoke, pos, Vec3::ZERO, false);
                    if chance < 0.17 {
                        let volume = 1.0 + ctx.f32();
                        let pitch = ctx.f32() * 0.7 + 0.3;
                        let at = [pos.x + 0.5, pos.y + 0.5, pos.z + 0.5];
                        play("block.candle.ambient", Bus::Blocks, at, volume, pitch);
                    }
                }
                ctx.emit(&P::SmallFlame, pos, Vec3::ZERO, false);
            }
        }
        Ambient::Fire => {
            if ctx.one_in(24) {
                let volume = 1.0 + ctx.f32();
                let pitch = ctx.f32() * 0.7 + 0.3;
                play("block.fire.ambient", Bus::Blocks, centre, volume, pitch);
            }
            for _ in 0..3 {
                let x = ctx.f32();
                let y = ctx.f32() * 0.5 + 0.5;
                let z = ctx.f32();
                ctx.emit(&P::LargeSmoke, rel(cell, [x, y, z]), Vec3::ZERO, false);
            }
        }
        Ambient::Campfire => {
            if ctx.one_in(10) {
                let volume = 0.5 + ctx.f32();
                let pitch = ctx.f32() * 0.7 + 0.6;
                play("block.campfire.crackle", Bus::Blocks, centre, volume, pitch);
            }
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
            if ctx.rng.next_f64() < 0.1 {
                let event = match kind {
                    Furnace::Furnace => "block.furnace.fire_crackle",
                    Furnace::Blast => "block.blastfurnace.fire_crackle",
                    Furnace::Smoker => "block.smoker.smoke",
                };
                let at = [
                    f64::from(cell[0]) + 0.5,
                    f64::from(cell[1]),
                    f64::from(cell[2]) + 0.5,
                ];
                play(event, Bus::Blocks, at, 1.0, 1.0);
            }
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
            if ctx.one_in(100) {
                let pitch = ctx.f32() * 0.4 + 0.8;
                play("block.portal.ambient", Bus::Blocks, centre, 0.5, pitch);
            }
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
            if ctx.one_in(100) {
                play(
                    "block.respawn_anchor.ambient",
                    Bus::Blocks,
                    centre,
                    1.0,
                    1.0,
                );
            }
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
                && levels_in(light, [cell[0], cell[1] + 1, cell[2]]).1 >= 15;
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
            if ctx.one_in(200) {
                let event = if drag {
                    "block.bubble_column.whirlpool_ambient"
                } else {
                    "block.bubble_column.upwards_ambient"
                };
                let volume = 0.2 + ctx.f32() * 0.2;
                let pitch = 0.9 + ctx.f32() * 0.15;
                play(
                    event,
                    Bus::Blocks,
                    rel(cell, [0.0; 3]).to_array(),
                    volume,
                    pitch,
                );
            }
        }
        Ambient::EndGateway => {
            let faces = [
                [0, -1, 0],
                [0, 1, 0],
                [0, 0, -1],
                [0, 0, 1],
                [-1, 0, 0],
                [1, 0, 0],
            ];
            let count = faces
                .iter()
                .filter(|d| !ctx.solid([cell[0] + d[0], cell[1] + d[1], cell[2] + d[2]]))
                .count();
            for _ in 0..count {
                let rng = &mut *ctx.rng;
                let mut x = rng.next_f64();
                let y = rng.next_f64();
                let mut z = rng.next_f64();
                let mut xa = (rng.next_f64() - 0.5) * 0.5;
                let ya = (rng.next_f64() - 0.5) * 0.5;
                let mut za = (rng.next_f64() - 0.5) * 0.5;
                let flip = f64::from(rng.next_int(2) as i32 * 2 - 1);
                if rng.next_bool() {
                    z = 0.5 + 0.25 * flip;
                    za = f64::from(rng.next_f32() * 2.0) * flip;
                } else {
                    x = 0.5 + 0.25 * flip;
                    xa = f64::from(rng.next_f32() * 2.0) * flip;
                }
                let pos = DVec3::new(
                    f64::from(cell[0]) + x,
                    f64::from(cell[1]) + y,
                    f64::from(cell[2]) + z,
                );
                ctx.emit(
                    &P::Portal,
                    pos,
                    Vec3::new(xa as f32, ya as f32, za as f32),
                    false,
                );
            }
        }
        Ambient::LightningRod { axis } => {
            if !level.thundering
                || i64::from(ctx.rng.next_int(200)) > (level.game_time % 200) as i64
                || column_height(ctx.world, cell, HeightmapKind::WorldSurface) != Some(cell[1] + 1)
            {
                return;
            }
            let sparks = 1 + ctx.rng.next_int(2);
            let along = |a: u8| if a == axis { 0.5 } else { 0.125 };
            for _ in 0..sparks {
                let rng = &mut *ctx.rng;
                let mut next = || rng.next_f64() * 2.0 - 1.0;
                let x = 0.5 + next() * along(0);
                let y = 0.5 + next() * along(1);
                let z = 0.5 + next() * along(2);
                let xa = if axis == 0 { next() } else { 0.0 };
                let ya = if axis == 1 { next() } else { 0.0 };
                let za = if axis == 2 { next() } else { 0.0 };
                let pos = DVec3::new(
                    f64::from(cell[0]) + x,
                    f64::from(cell[1]) + y,
                    f64::from(cell[2]) + z,
                );
                ctx.emit(
                    &P::ElectricSpark,
                    pos,
                    Vec3::new(xa as f32, ya as f32, za as f32),
                    false,
                );
            }
        }
        Ambient::PotentSulfur => {
            let above = ctx.block([cell[0], cell[1] + 1, cell[2]]);
            let water_source = above.is_some_and(|state| {
                let fluid = azalea::block::fluid_state::FluidState::from(state);
                fluid.kind == azalea::block::fluid_state::FluidKind::Water && fluid.is_source()
            });
            if !water_source {
                return;
            }
            for _ in 0..2 {
                let x = ctx.f32();
                let y = ctx.f32();
                let z = ctx.f32();
                ctx.emit(
                    &P::SulfurBubbles,
                    rel(cell, [x, 1.0 + y, z]),
                    Vec3::ZERO,
                    true,
                );
            }
            if ctx.one_in(10) {
                play(
                    "block.potent_sulfur.noxious_gas",
                    Bus::Ambient,
                    rel(cell, [0.0; 3]).to_array(),
                    1.0,
                    1.0,
                );
            }
        }
        Ambient::DriedGhast { waterlogged } => {
            if ctx.one_in(40) {
                let below = [cell[0], cell[1] - 1, cell[2]];
                if waterlogged {
                    play(
                        "block.dried_ghast.ambient_water",
                        Bus::Blocks,
                        centre,
                        1.0,
                        1.0,
                    );
                } else if in_tag(ctx, below, &tags::TRIGGERS_AMBIENT_DRIED_GHAST_BLOCK_SOUNDS) {
                    play("block.dried_ghast.ambient", Bus::Blocks, centre, 1.0, 1.0);
                }
            }
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
        Ambient::FallingDust { sand } => {
            if cfg!(feature = "audio") && sand {
                sand_idle(ctx);
            }
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
            if ctx.one_in(30)
                && overworld_track(ctx, FIREFLY_SOUNDS)
                && column_height(ctx.world, cell, HeightmapKind::MotionBlockingNoLeaves)
                    .is_some_and(|h| h <= cell[1])
            {
                play("block.firefly_bush.idle", Bus::Ambient, centre, 1.0, 1.0);
            }
            let (block_light, sky_light) = levels_in(light, cell);
            if i32::from(block_light).max(i32::from(sky_light) - ctx.sky_darken) > 13 {
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
        Ambient::OpenEyeblossom => {
            if ctx.one_in(700)
                && ctx
                    .block([cell[0], cell[1] - 1, cell[2]])
                    .map(BlockKind::from)
                    == Some(BlockKind::PaleMossBlock)
            {
                let at = rel(cell, [0.0; 3]).to_array();
                play("block.eyeblossom.idle", Bus::Ambient, at, 1.0, 1.0);
            }
        }
        Ambient::PaleHangingMoss => {
            if ctx.one_in(500) {
                let above = [cell[0], cell[1] + 1, cell[2]];
                if in_tag(ctx, above, &tags::PALE_OAK_LOGS)
                    || ctx.block(above).map(BlockKind::from) == Some(BlockKind::PaleOakLeaves)
                {
                    let at = rel(cell, [0.0; 3]).to_array();
                    play("block.pale_hanging_moss.idle", Bus::Ambient, at, 1.0, 1.0);
                }
            }
        }
        Ambient::CreakingHeart => {
            if overworld_track(ctx, CREAKING_ACTIVE) && ctx.one_in(16) {
                const SIDES: [[i32; 3]; 6] = [
                    [0, -1, 0],
                    [0, 1, 0],
                    [0, 0, -1],
                    [0, 0, 1],
                    [-1, 0, 0],
                    [1, 0, 0],
                ];
                let boxed = SIDES.iter().all(|s| {
                    in_tag(
                        ctx,
                        [cell[0] + s[0], cell[1] + s[1], cell[2] + s[2]],
                        &tags::PALE_OAK_LOGS,
                    )
                });
                if boxed {
                    let at = rel(cell, [0.0; 3]).to_array();
                    play("block.creaking_heart.idle", Bus::Blocks, at, 1.0, 1.0);
                }
            }
        }
        Ambient::DeadBush => {
            if ctx.one_in(130) {
                let below = [cell[0], cell[1] - 1, cell[2]];
                let on_red = ctx.block(below).map(BlockKind::from) == Some(BlockKind::RedSand)
                    || in_tag(ctx, below, &tags::TERRACOTTA);
                if on_red && ctx.rng.next_int(3) != 0 {
                    return;
                }
                if desert_dry_vegetation_ok(ctx, below) {
                    let at = rel(cell, [0.0; 3]).to_array();
                    play("block.deadbush.idle", Bus::Ambient, at, 1.0, 1.0);
                }
            }
        }
        Ambient::DryGrass => {
            if ctx.one_in(200) && desert_dry_vegetation_ok(ctx, [cell[0], cell[1] - 1, cell[2]]) {
                play_local("block.dry_grass.ambient", Bus::Ambient, 1.0, 1.0);
            }
        }
    }
}

const FIREFLY_SOUNDS: [f64; 2] = [12600.0, 23401.0];
const CREAKING_ACTIVE: [f64; 2] = [12600.0, 23401.0];

fn overworld_track(ctx: &Emitter, [on, off]: [f64; 2]) -> bool {
    if crate::renderer::dimension::current() != crate::renderer::dimension::Dimension::Overworld {
        return false;
    }
    let t = ctx.day_ticks.rem_euclid(24000.0);
    t >= on && t < off
}

fn in_tag(
    ctx: &Emitter,
    cell: [i32; 3],
    tag: &azalea_registry::tags::RegistryTag<azalea_registry::builtin::BlockKind>,
) -> bool {
    ctx.block(cell)
        .is_some_and(|s| tag.contains(&azalea_registry::builtin::BlockKind::from(s)))
}

fn desert_dry_vegetation_ok(ctx: &Emitter, below: [i32; 3]) -> bool {
    use azalea_registry::tags::blocks::TRIGGERS_AMBIENT_DESERT_DRY_VEGETATION_BLOCK_SOUNDS as TAG;
    in_tag(ctx, below, &TAG) && in_tag(ctx, [below[0], below[1] - 1, below[2]], &TAG)
}

pub(crate) fn column_height(
    world: &azalea_world::World,
    cell: [i32; 3],
    kind: HeightmapKind,
) -> Option<i32> {
    let chunk = world.chunks.get(&azalea_core::position::ChunkPos {
        x: cell[0] >> 4,
        z: cell[2] >> 4,
    })?;
    let chunk = chunk.read();
    let map = chunk.heightmaps.get(&kind)?;
    Some(map.get_first_available((cell[0] & 15) as u8, (cell[2] & 15) as u8))
}

fn sand_idle(ctx: &mut Emitter) {
    use crate::util::level_events::{Bus, play};
    use azalea_registry::tags::blocks::TRIGGERS_AMBIENT_DESERT_SAND_BLOCK_SOUNDS as SAND;
    let cell = ctx.cell;
    if !ctx.one_in(2100)
        || !ctx
            .block([cell[0], cell[1] + 1, cell[2]])
            .is_some_and(|s| s.is_air())
    {
        return;
    }
    let column = |ctx: &Emitter, x: i32, y: i32, z: i32| -> bool {
        let Some(surface) = column_height(ctx.world, [x, y, z], HeightmapKind::WorldSurface) else {
            return false;
        };
        let surface = surface - 1;
        if (surface - y).abs() > 5 {
            let mut at = y + 6;
            let mut above = ctx.block([x, at, z]);
            at -= 1;
            for _ in 0..10 {
                let current = ctx.block([x, at, z]);
                if above.is_some_and(state_is_air)
                    && current.is_some_and(|s| {
                        SAND.contains(&azalea_registry::builtin::BlockKind::from(s))
                    })
                {
                    return true;
                }
                above = current;
                at -= 1;
            }
            false
        } else {
            ctx.block([x, surface + 1, z]).is_some_and(state_is_air)
                && in_tag(ctx, [x, surface, z], &SAND)
        }
    };
    let mut found = 0;
    let mut checked = 0;
    for [dx, dz] in [[0, -8], [8, 0], [0, 8], [-8, 0]] {
        if column(ctx, cell[0] + dx, cell[1], cell[2] + dz) {
            found += 1;
            if found > 3 {
                play(
                    "block.sand.idle",
                    Bus::Ambient,
                    rel(cell, [0.0; 3]).to_array(),
                    1.0,
                    1.0,
                );
                return;
            }
        }
        checked += 1;
        if 4 - checked + found < 3 {
            return;
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

fn random_open_face(ctx: &mut Emitter, inset: f32) -> Option<DVec3> {
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

use crate::blocks::state_is_air;

fn emit_fluid_ambient(fluid: &azalea::block::fluid_state::FluidState, ctx: &mut Emitter) {
    use crate::util::level_events::{Bus, play};
    use azalea::block::fluid_state::FluidKind;

    let cell = ctx.cell;
    match fluid.kind {
        FluidKind::Lava => {
            let above = [cell[0], cell[1] + 1, cell[2]];
            if ctx.block(above).is_some_and(|s| !state_is_air(s)) {
                return;
            }
            if ctx.one_in(100) {
                let x = ctx.f32();
                let z = ctx.f32();
                let at = rel(cell, [x, 1.0, z]);
                ctx.emit(&P::Lava, at, Vec3::ZERO, false);
                let volume = 0.2 + ctx.f32() * 0.2;
                let pitch = 0.9 + ctx.f32() * 0.15;
                play("block.lava.pop", Bus::Ambient, at.to_array(), volume, pitch);
            }
            if ctx.one_in(200) {
                let volume = 0.2 + ctx.f32() * 0.2;
                let pitch = 0.9 + ctx.f32() * 0.15;
                play(
                    "block.lava.ambient",
                    Bus::Ambient,
                    rel(cell, [0.0; 3]).to_array(),
                    volume,
                    pitch,
                );
            }
        }
        FluidKind::Water => {
            if fluid.is_empty() {
                return;
            }
            if !fluid.is_source() && !fluid.falling {
                if !ctx.one_in(64) {
                    return;
                }
                let volume = ctx.f32() * 0.25 + 0.75;
                let pitch = ctx.f32() + 0.5;
                play(
                    "block.water.ambient",
                    Bus::Ambient,
                    rel(cell, [0.5, 0.5, 0.5]).to_array(),
                    volume,
                    pitch,
                );
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
    drip: Option<(Drip, DripStage)>,
    motion: Motion,
    light: Light,
    nudge: DVec3,
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
        drip: None,
        motion: Motion::Integrate,
        light: Light::Cell,
        nudge: DVec3::ZERO,
    }
}

fn short_lifetime(rng: &mut JavaRandom) -> u32 {
    (8.0 / (rng.next_f32() * 0.8 + 0.2)) as u32
}

fn centred_jitter(rng: &mut JavaRandom, spread: f32) -> Vec3 {
    Vec3::new(
        (rng.next_f32() - 0.5) * spread,
        (rng.next_f32() - 0.5) * spread,
        (rng.next_f32() - 0.5) * spread,
    )
}

fn unit_rgb(color: &azalea_core::color::RgbColor) -> [f32; 3] {
    [color.red(), color.green(), color.blue()].map(unit)
}

fn unit(c: u8) -> f32 {
    f32::from(c) / 255.0
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
        Particle::SulfurBubbles => "sulfur_bubbles",
        Particle::NoxiousGas => "noxious_gas",
        Particle::NoxiousGasCloud => "noxious_gas_cloud",
        Particle::Geyser(_) => "geyser",
        Particle::GeyserBase(_) => "geyser_base",
        Particle::GeyserPoof(_) => "geyser_poof",
        Particle::GeyserPlume(_) => "geyser_plume",
        Particle::SulfurCubeGoo => "sulfur_cube_goo",
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

        P::Smoke | P::WhiteSmoke | P::LargeSmoke => {
            let scale = if matches!(particle, P::LargeSmoke) {
                2.5
            } else {
                1.0
            };
            let ash = AshSmoke {
                dir: Vec3::splat(0.1),
                scale,
                color_random: 0.3,
                max_lifetime: 8.0,
                gravity: -0.1,
                physics: true,
            };
            ash_smoke_recipe(atlas, name, arg, rng, ash)
        }
        P::Ash => {
            let ash = AshSmoke {
                dir: Vec3::new(0.1, -0.1, 0.1),
                scale: 1.0,
                color_random: 0.5,
                max_lifetime: 20.0,
                gravity: 0.1,
                physics: false,
            };
            ash_smoke_recipe(atlas, name, Vec3::ZERO, rng, ash)
        }
        P::WhiteAsh => {
            let xa = rng.next_f32() * -1.9 * rng.next_f32() * 0.1;
            let ya = rng.next_f32() * -0.5 * rng.next_f32() * 0.1 * 5.0;
            let za = rng.next_f32() * -1.9 * rng.next_f32() * 0.1;
            let ash = AshSmoke {
                dir: Vec3::new(0.1, -0.1, 0.1),
                scale: 1.0,
                color_random: 0.0,
                max_lifetime: 20.0,
                gravity: 0.0125,
                physics: false,
            };
            let mut r = ash_smoke_recipe(atlas, name, Vec3::new(xa, ya, za), rng, ash)?;
            r.tint = [unit(0xBA), unit(0xB1), unit(0xC2), 1.0];
            Some(r)
        }
        P::WarpedSpore => {
            let ya = rng.next_f32() * -1.9 * rng.next_f32() * 0.1;
            let mut r = suspended_particle_recipe(atlas, name, Some(Vec3::new(0.0, ya, 0.0)), rng)?;
            r.tint = [0.1, 0.1, 0.3, 1.0];
            Some(r)
        }
        P::CrimsonSpore => {
            let seed = Vec3::new(
                rng.next_gaussian() as f32 * 1.0e-6,
                rng.next_gaussian() as f32 * 1.0e-4,
                rng.next_gaussian() as f32 * 1.0e-6,
            );
            let mut r = suspended_particle_recipe(atlas, name, Some(seed), rng)?;
            r.tint = [0.9, 0.4, 0.5, 1.0];
            Some(r)
        }
        P::SporeBlossomAir => {
            let mut r =
                suspended_particle_recipe(atlas, name, Some(Vec3::new(0.0, -0.8, 0.0)), rng)?;
            r.lifetime = 500 + rng.next_int(501);
            r.gravity = 0.01 / 0.04;
            r.tint = [0.32, 0.5, 0.22, 1.0];
            Some(r)
        }
        P::CampfireCosySmoke => campfire_recipe(atlas, name, arg, rng, 80, 0.9),
        P::CampfireSignalSmoke => campfire_recipe(atlas, name, arg, rng, 280, 0.95),

        P::Poof => {
            let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
            r.velocity = arg + centred_jitter(rng, 0.1);
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

        P::Splash | P::Rain => {
            let uv = random_frame(atlas, name, rng)?;
            let mut r = base_recipe(Sprite::Fixed(uv), rng);
            r.gravity = if matches!(particle, P::Splash) {
                0.04 / 0.04
            } else {
                0.06 / 0.04
            };
            r.velocity = Vec3::new(arg.x, rng.next_f32() * 0.2 + 0.1, arg.z);
            r.lifetime = short_lifetime(rng);
            r.flags |= WATER_DROP;
            Some(r)
        }
        P::Underwater => {
            let mut r = suspended_particle_recipe(atlas, name, None, rng)?;
            r.tint = [0.4, 0.4, 0.7, 1.0];
            Some(r)
        }
        P::Bubble | P::BubblePop | P::CurrentDown | P::BubbleColumnUp => {
            let uv = random_frame(atlas, name, rng)?;
            let mut r = base_recipe(Sprite::Fixed(uv), rng);
            r.size *= rng.next_f32() * 0.6 + 0.2;
            r.velocity = arg * 0.2 + centred_jitter(rng, 0.04);
            r.gravity = -0.05;
            r.friction = 0.85;
            r.lifetime = short_lifetime(rng);
            Some(r)
        }

        P::Dust(dust) => dust_recipe(atlas, name, arg, rng, unit_rgb(&dust.color), dust.scale),
        P::DustColorTransition(dust) => {
            dust_recipe(atlas, name, arg, rng, unit_rgb(&dust.from), dust.scale)
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

        P::Portal => {
            let uv = random_frame(atlas, name, rng)?;
            let mut r = base_recipe(Sprite::Fixed(uv), rng);
            r.size = 0.1 * (rng.next_f32() * 0.2 + 0.5);
            let br = rng.next_f32() * 0.6 + 0.4;
            r.tint = [0.9 * br, 0.3 * br, br, 1.0];
            r.lifetime = (rng.next_f32() * 10.0) as u32 + 40;
            r.velocity = arg;
            r.gravity = 0.0;
            r.friction = 1.0;
            r.grow = Grow::Portal;
            r.flags = 0;
            r.motion = Motion::Portal;
            r.light = Light::Quartic;
            Some(r)
        }
        P::ReversePortal => {
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
            r.flags = 0;
            r.light = Light::Quartic;
            Some(r)
        }
        P::Enchant | P::Nautilus | P::VaultConnection => {
            let vault = matches!(particle, P::VaultConnection);
            let uv = random_frame(atlas, name, rng)?;
            let mut r = base_recipe(Sprite::Fixed(uv), rng);
            r.size = 0.1 * (rng.next_f32() * 0.5 + 0.2);
            let br = rng.next_f32() * 0.6 + 0.4;
            r.tint = [0.9 * br, 0.9 * br, br, if vault { 0.0 } else { 1.0 }];
            r.lifetime = (rng.next_f32() * 10.0) as u32 + 30;
            r.velocity = arg;
            r.gravity = 0.0;
            r.friction = 1.0;
            r.flags = if vault { TRANSLUCENT } else { 0 };
            if vault {
                r.size *= 1.5;
            }
            r.motion = Motion::FlyTowards { vault };
            r.light = if vault { Light::Block } else { Light::Quartic };
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

        P::Effect(spell) | P::InstantEffect(spell) => {
            let [r, g, b] = unit_rgb(&spell.color);
            spell_recipe(atlas, name, arg, rng, [r, g, b, 1.0]).map(|mut recipe| {
                recipe.velocity = set_power(recipe.velocity, spell.power);
                recipe
            })
        }
        P::Infested | P::RaidOmen | P::TrialOmen => {
            spell_recipe(atlas, name, arg, rng, [1.0, 1.0, 1.0, 1.0])
        }
        P::EntityEffect(option) => {
            let c = option.color;
            spell_recipe(
                atlas,
                name,
                arg,
                rng,
                [c.red(), c.green(), c.blue(), c.alpha()].map(unit),
            )
        }
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
            r.velocity = match particle {
                P::Scrape => arg * 0.01,
                P::WaxOn | P::WaxOff => Vec3::new(arg.x * 0.005, arg.y * 0.01, arg.z * 0.005),
                _ => arg * 0.25,
            };
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
            r.flags = 0;
            r.light = Light::Linear;
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

        P::Item(p) => item_recipe(
            atlas,
            crate::client::tick::item_id(&p.item)?,
            Some(arg),
            rng,
        ),
        P::ItemSlime => item_recipe(atlas, "slime_ball", None, rng),
        P::ItemSnowball => item_recipe(atlas, "snowball", None, rng),
        P::ItemCobweb => item_recipe(atlas, "cobweb", None, rng),

        P::DrippingWater => drip_recipe(atlas, name, Drip::Water, DripStage::Hang, rng),
        P::DrippingLava => drip_recipe(atlas, name, Drip::Lava, DripStage::Hang, rng),
        P::DrippingHoney => drip_recipe(atlas, name, Drip::Honey, DripStage::Hang, rng),
        P::DrippingDripstoneWater => {
            drip_recipe(atlas, name, Drip::DripstoneWater, DripStage::Hang, rng)
        }
        P::DrippingDripstoneLava => {
            drip_recipe(atlas, name, Drip::DripstoneLava, DripStage::Hang, rng)
        }
        P::DrippingObsidianTear => {
            drip_recipe(atlas, name, Drip::ObsidianTear, DripStage::Hang, rng)
        }
        P::FallingWater => drip_recipe(atlas, name, Drip::Water, DripStage::Fall, rng),
        P::FallingLava => drip_recipe(atlas, name, Drip::Lava, DripStage::Fall, rng),
        P::FallingHoney => drip_recipe(atlas, name, Drip::Honey, DripStage::Fall, rng),
        P::FallingDripstoneWater => {
            drip_recipe(atlas, name, Drip::DripstoneWater, DripStage::Fall, rng)
        }
        P::FallingDripstoneLava => {
            drip_recipe(atlas, name, Drip::DripstoneLava, DripStage::Fall, rng)
        }
        P::FallingObsidianTear => {
            drip_recipe(atlas, name, Drip::ObsidianTear, DripStage::Fall, rng)
        }
        P::LandingLava => drip_recipe(atlas, name, Drip::Lava, DripStage::Land, rng),
        P::LandingHoney => drip_recipe(atlas, name, Drip::Honey, DripStage::Land, rng),
        P::LandingObsidianTear => {
            drip_recipe(atlas, name, Drip::ObsidianTear, DripStage::Land, rng)
        }

        P::Gust | P::SmallGust => {
            let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
            r.gravity = 0.0;
            r.friction = 1.0;
            r.lifetime = 12 + rng.next_int(4);
            r.size = if matches!(particle, P::SmallGust) {
                0.15
            } else {
                1.0
            };
            r.flags = FULLBRIGHT;
            Some(r)
        }

        P::GustEmitterLarge | P::GustEmitterSmall => None,

        P::SulfurBubbles => {
            let uv = random_frame(atlas, name, rng)?;
            let mut r = base_recipe(Sprite::Fixed(uv), rng);
            let xd = arg.x * 0.2 + (rng.next_f32() * 2.0 - 1.0) * 0.02;
            let zd = arg.y * 0.2 + (rng.next_f32() * 2.0 - 1.0) * 0.02;
            r.velocity = Vec3::new(xd, 0.0, zd);
            let size_start = 0.02 + 0.02 * rng.next_f32();
            r.size = size_start + 0.5 * (0.15 - size_start);
            r.gravity = -0.04;
            r.friction = 0.85;
            r.lifetime = 285;
            Some(r)
        }

        P::NoxiousGas => {
            let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
            r.velocity = base_velocity(Vec3::ZERO, rng) * 0.1 + arg;
            r.size *= 0.75 * 3.0;
            r.gravity = -0.02;
            r.friction = 0.96;
            r.lifetime = ((6.0 / (rng.next_f32() * 0.5 + 0.5) * 3.0) as u32).max(1);
            r.grow = Grow::In;
            r.flags = PHYSICS | TRANSLUCENT | FADE_OUT;
            Some(r)
        }

        P::GeyserBase(g) | P::GeyserPoof(g) => {
            let water = g.water_blocks as f32;
            let burst = g.burst_impulse_base + 0.25 * water;
            let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
            let mut v = base_velocity(Vec3::ZERO, rng) * burst + arg;
            v.y = v.y.abs();
            r.velocity = v;
            r.size *= 0.75 * (3.0 + 0.125 * water);
            r.gravity = 0.0;
            r.friction = 0.725;
            r.lifetime = (25.0 * (0.8 + 0.2 * rng.next_f32())) as u32;
            r.grow = Grow::In;
            Some(r)
        }

        P::GeyserPlume(g) => {
            let height = 5 * g.water_blocks.max(1);
            let prop = (if g.water_blocks == 1 { 1.5 } else { 1.0 }) * height as f32 * 1.45;
            let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
            let v0 = 0.04 * prop;
            let decel = v0 * v0 / (2.0 * (height - 1) as f32);
            r.velocity = Vec3::new(
                (rng.next_f32() - 0.5) * 0.2 * 0.5,
                v0 + decel,
                (rng.next_f32() - 0.5) * 0.2 * 0.5,
            );
            r.size *= 0.75 * (2.5 + height as f32 / 8.0);
            r.gravity = decel / 0.04;
            r.friction = 1.0;
            r.lifetime = ((v0 / decel).ceil() as u32 + 5)
                .min(height as u32 * 5)
                .max(1);
            r.flags = PHYSICS;
            Some(r)
        }

        P::SulfurCubeGoo => {
            let sheet = atlas.sheet_index(name)?;
            let [u0, v0, u1, v1] = atlas.frame_uv(sheet, 0);
            let (w, h) = ((u1 - u0) / 4.0, (v1 - v0) / 4.0);
            let (uo, vo) = (rng.next_f32() * 3.0, rng.next_f32() * 3.0);
            let uv = [
                u0 + (uo + 1.0) * w,
                v0 + vo * h,
                u0 + uo * w,
                v0 + (vo + 1.0) * h,
            ];
            let mut r = base_recipe(Sprite::Fixed(uv), rng);
            r.velocity = base_velocity(Vec3::ZERO, rng);
            r.size /= 2.0;
            Some(r)
        }

        P::NoxiousGasCloud | P::Geyser(_) => None,

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

fn item_recipe(atlas: &Atlas, id: &str, aux: Option<Vec3>, rng: &mut JavaRandom) -> Option<Recipe> {
    let [u0, v0, u1, v1] = atlas.item_uv(id)?;
    let (w, h) = ((u1 - u0) / 4.0, (v1 - v0) / 4.0);
    let (uo, vo) = (rng.next_f32() * 3.0, rng.next_f32() * 3.0);
    let uv = [
        u0 + (uo + 1.0) * w,
        v0 + vo * h,
        u0 + uo * w,
        v0 + (vo + 1.0) * h,
    ];
    let mut r = base_recipe(Sprite::Fixed(uv), rng);
    let base = base_velocity(Vec3::ZERO, rng);
    r.velocity = aux.map_or(base, |aux| base * 0.1 + aux);
    r.size /= 2.0;
    r.flags = PHYSICS | ITEM;
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
    r.velocity = base_velocity(arg, rng) * 0.01 + arg;
    let mut jitter = || f64::from((rng.next_f32() - rng.next_f32()) * 0.05);
    r.nudge = DVec3::new(jitter(), jitter(), jitter());
    r.gravity = 0.0;
    r.friction = 0.96;
    r.size *= scale;
    r.lifetime = short_lifetime(rng) + 4;
    r.grow = Grow::Flame;
    r.flags = 0;
    r.light = Light::Linear;
    Some(r)
}

struct AshSmoke {
    dir: Vec3,
    scale: f32,
    color_random: f32,
    max_lifetime: f32,
    gravity: f32,
    physics: bool,
}

fn ash_smoke_recipe(
    atlas: &Atlas,
    name: &str,
    arg: Vec3,
    rng: &mut JavaRandom,
    ash: AshSmoke,
) -> Option<Recipe> {
    let mut r = base_recipe(Sprite::Animated(atlas.sheet_index(name)?), rng);
    r.velocity = base_velocity(Vec3::ZERO, rng) * ash.dir + arg;
    let grey = rng.next_f32() * ash.color_random;
    r.tint = [grey, grey, grey, 1.0];
    r.size *= 0.75 * ash.scale;
    r.gravity = ash.gravity;
    r.friction = 0.96;
    r.lifetime = ((f64::from(ash.max_lifetime) / (f64::from(rng.next_f32()) * 0.8 + 0.2)
        * f64::from(ash.scale)) as u32)
        .max(1);
    r.grow = Grow::In;
    r.flags = if ash.physics { PHYSICS } else { 0 };
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
    let uv = random_frame(atlas, name, rng)?;
    let mut r = base_recipe(Sprite::Fixed(uv), rng);
    r.size *= 3.0;
    r.lifetime = rng.next_int(50) + base_lifetime;
    r.gravity = 3.0e-6 / 0.04;
    r.velocity = Vec3::new(arg.x, arg.y + rng.next_f32() / 500.0, arg.z);
    r.friction = 1.0;
    r.tint = [1.0, 1.0, 1.0, alpha];
    r.flags = PHYSICS | TRANSLUCENT;
    r.motion = Motion::Campfire;
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
    r.size *= rng.next_f32() * 0.6 + 0.5;
    r.gravity = 0.0;
    r.friction = 0.99;
    r.lifetime = (20.0 / (rng.next_f32() * 0.8 + 0.2)) as u32;
    r.flags = 0;
    Some(r)
}

fn suspended_particle_recipe(
    atlas: &Atlas,
    name: &str,
    seed: Option<Vec3>,
    rng: &mut JavaRandom,
) -> Option<Recipe> {
    let uv = random_frame(atlas, name, rng)?;
    let mut r = base_recipe(Sprite::Fixed(uv), rng);
    r.velocity = match seed {
        Some(seed) => base_velocity(seed, rng),
        None => Vec3::ZERO,
    };
    r.size *= rng.next_f32() * 0.6 + if seed.is_some() { 0.6 } else { 0.2 };
    r.lifetime = (16.0 / (rng.next_f32() * 0.8 + 0.2)) as u32;
    r.gravity = 0.0;
    r.friction = 1.0;
    r.flags = 0;
    r.nudge = DVec3::new(0.0, -0.125, 0.0);
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
    let seed = Vec3::new(
        0.5 - rng.next_f64() as f32,
        arg.y,
        0.5 - rng.next_f64() as f32,
    );
    let mut v = base_velocity(seed, rng);
    v.y *= 0.2;
    if arg.x == 0.0 && arg.z == 0.0 {
        v.x *= 0.1;
        v.z *= 0.1;
    }
    r.velocity = v;
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

fn drip_recipe(
    atlas: &Atlas,
    name: &str,
    drip: Drip,
    stage: DripStage,
    rng: &mut JavaRandom,
) -> Option<Recipe> {
    const GRAVITY: f32 = 0.06;
    let mut r = base_recipe(Sprite::Fixed(random_frame(atlas, name, rng)?), rng);
    let viscous = matches!(drip, Drip::Honey | Drip::ObsidianTear);
    let rgb = match (drip, stage) {
        (Drip::Water | Drip::DripstoneWater, _) => [0.2, 0.3, 1.0],
        (Drip::Lava | Drip::DripstoneLava, _) => [1.0, 0.2857143, 0.083333336],
        (Drip::Honey, DripStage::Hang) => [0.622, 0.508, 0.082],
        (Drip::Honey, DripStage::Land) => [0.522, 0.408, 0.082],
        (Drip::Honey, _) => [0.582, 0.448, 0.082],
        (Drip::ObsidianTear, _) => [0.51171875, 0.03125, 0.890625],
    };
    r.tint = [rgb[0], rgb[1], rgb[2], 1.0];
    let spread = f64::from(rng.next_f32()) * 0.8 + 0.2;
    let gravity = match stage {
        DripStage::Hang => {
            r.lifetime = if viscous { 100 } else { 40 };
            r.friction = 0.98 * 0.02;
            GRAVITY * 0.02 * if viscous { 0.01 } else { 1.0 }
        }
        DripStage::Fall | DripStage::Landed => {
            r.lifetime = (64.0 / spread) as u32;
            if viscous { 0.01 } else { GRAVITY }
        }
        DripStage::Land => {
            let life = match drip {
                Drip::Honey => 128.0,
                Drip::ObsidianTear => 28.0,
                _ => 16.0,
            };
            r.lifetime = (life / spread) as u32;
            GRAVITY
        }
    };
    r.gravity = gravity / 0.04;
    if drip == Drip::ObsidianTear {
        r.flags |= FULLBRIGHT;
    }
    if stage != DripStage::Land {
        r.flags &= !PHYSICS;
    }
    r.drip = Some((drip, stage));
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

fn is_solid_at(world: Option<&azalea_world::World>, pos: DVec3) -> bool {
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

fn drip_floor(world: &azalea_world::World, from: DVec3, to: DVec3) -> Option<f64> {
    use azalea::physics::collision::BlockWithShape;
    use azalea_core::position::{BlockPos, Vec3 as WorldVec3};

    let (start, end) = (
        WorldVec3::new(from.x, from.y, from.z),
        WorldVec3::new(to.x, to.y, to.z),
    );
    let (x, z) = (from.x.floor() as i32, from.z.floor() as i32);
    (to.y.floor() as i32..=from.y.floor() as i32)
        .rev()
        .find_map(|y| {
            let pos = BlockPos::new(x, y, z);
            let hit = world
                .get_block_state(pos)?
                .collision_shape(pos)
                .clip(start, end, pos)?;
            Some(hit.location.y)
        })
}

fn drip_fall(p: &mut Particle, world: Option<&azalea_world::World>, drip: Drip, next: &mut DVec3) {
    let Some(world) = world else { return };
    if let Some(floor) = drip_floor(world, p.pos, *next) {
        next.y = floor;
        p.drip = Some((drip, DripStage::Landed));
        p.lifetime = p.age;
        return;
    }
    let cell = next.floor().as_ivec3();
    let (x, z) = (next.x - f64::from(cell.x), next.z - f64::from(cell.z));
    if let Some(kind) = drip.fluid()
        && let Some(state) = block_at(world, cell.to_array())
        && azalea::block::fluid_state::FluidState::from(state).kind == kind
        && next.y
            < f64::from(cell.y) + crate::renderer::weather::surface_top(world, state, cell, x, z)
    {
        p.lifetime = p.age;
    }
}

fn advance_one_tick(p: &mut Particle, world: Option<&azalea_world::World>, rng: &mut JavaRandom) {
    p.prev = p.pos;
    match p.motion {
        Motion::Portal => {
            let a = p.age as f32 / p.lifetime as f32;
            let pos = 1.0 - (-a + a * a * 2.0);
            p.pos = p.origin + (p.velocity * pos).as_dvec3();
            p.pos.y += f64::from(1.0 - a);
            return;
        }
        Motion::FlyTowards { vault } => {
            let pos = 1.0 - p.age as f32 / p.lifetime as f32;
            let pp = (1.0 - pos).powi(4);
            p.pos = p.origin + (p.velocity * pos).as_dvec3();
            p.pos.y -= f64::from(pp * 1.2);
            if vault {
                let t = ((p.age as f32 / p.lifetime as f32 - 0.25) / 0.75).clamp(0.0, 1.0);
                p.tint[3] = t * 0.6;
            }
            return;
        }
        Motion::Campfire => {
            let mut drift = || rng.next_f32() / 5000.0 * if rng.next_bool() { 1.0 } else { -1.0 };
            p.velocity.x += drift();
            p.velocity.z += drift();
        }
        Motion::Integrate => {}
    }
    p.velocity.y -= 0.04 * p.gravity;

    let mut next = p.pos + p.velocity.as_dvec3();
    let mut on_ground = false;
    if p.flags & PHYSICS != 0 {
        if p.velocity.y != 0.0 && is_solid_at(world, DVec3::new(p.pos.x, next.y, p.pos.z)) {
            if p.velocity.y < 0.0 {
                next.y = next.y.floor() + 1.0;
                on_ground = true;
            } else {
                next.y = next.y.floor();
            }
            p.velocity.y = 0.0;
        }
        if p.velocity.x != 0.0 && is_solid_at(world, DVec3::new(next.x, p.pos.y, p.pos.z)) {
            next.x = p.pos.x;
            p.velocity.x = 0.0;
        }
        if p.velocity.z != 0.0 && is_solid_at(world, DVec3::new(p.pos.x, p.pos.y, next.z)) {
            next.z = p.pos.z;
            p.velocity.z = 0.0;
        }
    }
    match p.drip {
        Some((drip, DripStage::Fall)) => drip_fall(p, world, drip, &mut next),
        Some((drip, DripStage::Hang)) if drip.cools() => {
            let age = p.age as f32;
            p.tint = [1.0, 16.0 / (age + 16.0), 4.0 / (age + 8.0), 1.0];
        }
        _ => {}
    }
    p.pos = next;

    p.velocity *= p.friction;
    if on_ground {
        p.velocity.x *= 0.7;
        p.velocity.z *= 0.7;
    }
    if p.flags & WATER_DROP != 0
        && let Some(world) = world
    {
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
                p.lifetime = p.age;
            }
        }
    }

    if p.flags & CRIT_DECAY != 0 {
        p.tint[1] *= 0.96;
        p.tint[2] *= 0.9;
    }
    if p.flags & FADE_OUT != 0 && p.age * 2 > p.lifetime {
        let half = p.lifetime / 2;
        p.tint[3] = 1.0 - (p.age.saturating_sub(half) as f32 / p.lifetime as f32);
    }
    if p.motion == Motion::Campfire {
        if p.age >= p.lifetime.saturating_sub(60) && p.tint[3] > 0.01 {
            p.tint[3] -= 0.015;
        }
        if p.tint[3] <= 0.0 {
            p.lifetime = p.age;
        }
    }
}

fn particle_world() -> Option<std::sync::Arc<parking_lot::RwLock<azalea_world::World>>> {
    crate::client::tracking::current_world()
        .lock()
        .unwrap()
        .clone()
}

const TICK: f32 = 1.0 / 20.0;

struct DripNext<'a> {
    atlas: Option<&'a Atlas>,
    rng: &'a mut JavaRandom,
    status: ParticleStatus,
    eye: DVec3,
}

fn drip_stage_ended(particle: &mut Particle, next: &mut DripNext) -> bool {
    let Some((drip, stage)) = particle.drip else {
        return false;
    };
    let kind = match stage {
        DripStage::Hang => drip.falling(),
        DripStage::Landed => {
            if let Some(event) = drip.landing_sound() {
                let volume = next.rng.next_f32() * (1.0 - 0.3) + 0.3;
                crate::util::level_events::play(
                    event,
                    crate::util::level_events::Bus::Blocks,
                    particle.pos.to_array(),
                    volume,
                    1.0,
                );
            }
            drip.landing()
        }
        DripStage::Fall | DripStage::Land => return false,
    };
    let Some(atlas) = next.atlas else {
        return false;
    };
    if !SpawnGate::normal(&kind).allows(particle.pos, next.eye, next.status, next.rng) {
        return false;
    }
    let Some(recipe) = recipe(&kind, atlas, Vec3::ZERO, next.rng) else {
        return false;
    };
    particle.prev = particle.pos;
    particle.velocity = recipe.velocity;
    particle.gravity = recipe.gravity;
    particle.friction = recipe.friction;
    particle.age = 0;
    particle.lifetime = recipe.lifetime.max(1);
    particle.tint = recipe.tint;
    particle.size = recipe.size;
    particle.grow = recipe.grow;
    particle.sprite = recipe.sprite;
    particle.flags = recipe.flags;
    particle.drip = recipe.drip;
    particle.origin = particle.pos;
    particle.motion = recipe.motion;
    particle.light = recipe.light;
    true
}

fn tick_particles(
    time: Res<Time>,
    gui: Res<crate::gui::GuiState>,
    view: Res<crate::renderer::FrameView>,
    atlas: Res<ParticleAtlas>,
    mut particle_rng: ResMut<ParticleRng>,
    mut live: ResMut<LiveParticles>,
) {
    if live.0.is_empty() {
        return;
    }
    let mut next = DripNext {
        atlas: atlas.0.as_ref(),
        rng: &mut particle_rng.0,
        status: gui.options.particles,
        eye: view.camera_world,
    };
    let dt = time.delta_secs();
    let world_arc = particle_world();
    let world = world_arc
        .as_deref()
        .map(|w| crate::client::tracking::read_lock(w, "tick_particles"));
    let world = world.as_deref();

    live.0.retain_mut(|particle| {
        particle.tick_acc += dt;
        while particle.tick_acc >= TICK {
            particle.tick_acc -= TICK;
            if particle.age >= particle.lifetime {
                break;
            }
            particle.age += 1;
            advance_one_tick(particle, world, next.rng);
        }
        particle.age < particle.lifetime || drip_stage_ended(particle, &mut next)
    });
}

#[derive(Component)]
pub(crate) struct ParticleBatch;

fn draw_particles(
    lightmap: Res<LightmapState>,
    chunk_material: Res<ChunkMaterial>,
    standard: Res<Assets<StandardMaterial>>,
    atlas: Res<ParticleAtlas>,
    gui_assets: Option<Res<crate::gui::render::GuiAssets>>,
    live: Res<LiveParticles>,
    mut ent_materials: ResMut<Assets<EntityMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut material_cache: ResMut<ParticleMaterialCache>,
    mut batches: ResMut<ParticleBatches>,
    view: Res<crate::renderer::FrameView>,
    render_origin: Res<RenderOrigin>,
    mut drawn: Query<(&mut MeshMaterial3d<EntityMaterial>, &mut Visibility), With<ParticleBatch>>,
    mut commands: Commands,
) {
    if live.0.is_empty() {
        material_cache.0.clear();
        hide_from(&batches.pool, 0, &mut drawn);
        return;
    }

    let dim = crate::renderer::dimension::current();
    let dirs = diffuse_lights(dim);
    let terrain_atlas = particle_atlas(&chunk_material, &standard);
    let sheet = atlas.0.as_ref();
    let item_image = gui_assets.as_ref().map(|gui| &gui.image);
    let origin = render_origin.world();
    let rotation = view.camera_rotation;
    let right = rotation * Vec3::X;
    let up = rotation * Vec3::Y;
    let normal = (rotation * Vec3::Z).to_array();

    let used = {
        let light_map = crate::client::tracking::read_lock(
            crate::client::worldsync::light_map(),
            "light:tick_particles",
        );
        let ParticleBatches {
            index,
            keys,
            groups,
            ..
        } = &mut *batches;
        index.clear();
        let mut used = 0usize;
        for (i, particle) in live.0.iter().enumerate() {
            if particle.tick_acc < 0.0 {
                continue;
            }
            let (drawn_from, ready) = if particle.flags & TERRAIN != 0 {
                (Sheet::Terrain, terrain_atlas.is_some())
            } else if particle.flags & ITEM != 0 {
                (Sheet::Items, item_image.is_some())
            } else {
                (Sheet::Particles, sheet.is_some())
            };
            if !ready {
                continue;
            }
            let (block_light, sky_light) = if particle.flags & FULLBRIGHT != 0 {
                (15, 15)
            } else {
                let (block, sky) = crate::renderer::lightmap::levels_in(
                    &light_map,
                    crate::renderer::lightmap::cell_of(particle.pos.to_array()),
                );
                (emitted_block_light(particle, block), sky)
            };
            let key = BatchKey {
                block_light,
                sky_light,
                dim: dim as u8,
                sheet: drawn_from,
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
        let image = match key.sheet {
            Sheet::Terrain => terrain_atlas.clone(),
            Sheet::Items => item_image.cloned(),
            Sheet::Particles => sheet.map(|sheet| sheet.image.clone()),
        };
        let Some(image) = image else { continue };
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
                    Transform::IDENTITY,
                    NoFrustumCulling,
                ))
                .id();
            #[cfg(feature = "builtin_shaders")]
            commands.entity(entity).insert(bevy::light::NotShadowCaster);
            batches.pool.push((entity, mesh));
        } else if let Ok((mut mesh_material, mut visibility)) = drawn.get_mut(batches.pool[slot].0)
        {
            if mesh_material.0 != material {
                mesh_material.0 = material;
            }
            if *visibility != Visibility::Inherited {
                *visibility = Visibility::Inherited;
            }
        }
        let handle = batches.pool[slot].1.clone();
        let Some(mut mesh) = meshes.get_mut(&handle) else {
            continue;
        };
        write_batch_mesh(
            &mut mesh,
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
        let lit = Lit {
            tint: [1.0; 4],
            light: crate::renderer::lightmap::cell_light(
                &lightmap.current,
                key.block_light,
                key.sky_light,
            ),
            dirs,
            overlay_alpha: 1.0,
            overlay_white: false,
        };
        EntityMaterial::relight(&mut ent_materials, handle, lit);
    }
}

fn hide_from(
    pool: &[(Entity, Handle<Mesh>)],
    first: usize,
    drawn: &mut Query<(&mut MeshMaterial3d<EntityMaterial>, &mut Visibility), With<ParticleBatch>>,
) {
    for (entity, _) in pool.iter().skip(first) {
        if let Ok((_, mut visibility)) = drawn.get_mut(*entity)
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
        glint_texture: None,
        alpha_mode: if key.translucent {
            AlphaMode::Blend
        } else {
            AlphaMode::Mask(0.1)
        },
        depth_bias: 0,
        sort_bias: 0.0,
        see_through: false,
    });
    cache.0.insert(key, handle.clone());
    handle
}

fn emitted_block_light(particle: &Particle, block: u8) -> u8 {
    let t = (particle.age as f32 / particle.lifetime.max(1) as f32).clamp(0.0, 1.0);
    let emission = match particle.light {
        Light::Cell => return block,
        Light::Block => return 15,
        Light::Linear => t,
        Light::Quartic => t * t * t * t,
    };
    let packed = (u32::from(block) * 16 + (emission * 240.0) as u32).min(240);
    (packed / 16) as u8
}

#[allow(clippy::too_many_arguments)]
fn write_batch_mesh(
    mesh: &mut Mesh,
    particles: &[Particle],
    group: &[u32],
    origin: DVec3,
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
        let centre = (p.prev.lerp(p.pos, f64::from(frac)) - origin).as_vec3();
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
        indices.extend(QUAD_INDICES.map(|i| base + i));
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
    if let TintKind::Stem(age) = kind {
        return crate::util::biome_color::stem_ratio(age);
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
        TintKind::Redstone(_) | TintKind::Stem(_) => unreachable!(),
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
