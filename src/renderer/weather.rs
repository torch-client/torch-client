use bevy::asset::{Asset, RenderAssetUsages, embedded_asset};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::math::{DVec3, IVec3};
use bevy::mesh::{
    Indices, Mesh, MeshVertexAttribute, MeshVertexBufferLayoutRef, PrimitiveTopology, VertexFormat,
};
use bevy::pbr::{Material, MaterialPipeline, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use azalea_core::heightmap_kind::HeightmapKind;
use azalea_core::position::ChunkPos;

use super::AppState;
use super::environment::{Precipitation, precipitation_at};
use crate::util::javarandom::JavaRandom;
use crate::util::particles::{Emitter, ParticleStatus};

pub const RADIUS: i32 = 10;
const RAIN_PARTICLES_PER_BLOCK: f32 = 0.225;
const TABLE: usize = 32;
const HALF_TABLE: i32 = 16;

pub const ATTRIBUTE_UV: MeshVertexAttribute =
    MeshVertexAttribute::new("Weather_Uv", 910, VertexFormat::Float32x2);
pub const ATTRIBUTE_COLUMN: MeshVertexAttribute =
    MeshVertexAttribute::new("Weather_Column", 911, VertexFormat::Float32x4);
pub const ATTRIBUTE_PACKED: MeshVertexAttribute =
    MeshVertexAttribute::new("Weather_Packed", 912, VertexFormat::Uint32);

#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub struct WeatherParams {
    pub fade: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct WeatherMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub rain: Handle<Image>,
    #[texture(2)]
    #[sampler(3)]
    pub snow: Handle<Image>,
    #[texture(4)]
    #[sampler(5)]
    pub lightmap: Handle<Image>,
    #[uniform(6)]
    pub params: WeatherParams,
}

impl Material for WeatherMaterial {
    crate::renderer::material_shader_fns!();

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    crate::renderer::material_no_prepass_no_shadows!();

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            ATTRIBUTE_UV.at_shader_location(1),
            ATTRIBUTE_COLUMN.at_shader_location(2),
            ATTRIBUTE_PACKED.at_shader_location(3),
        ])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

fn shader() -> ShaderRef {
    super::embedded_shader("renderer/weather.wgsl")
}

pub struct WeatherPlugin;

impl Plugin for WeatherPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "weather.wgsl");
        app.add_plugins(MaterialPlugin::<WeatherMaterial>::default())
            .init_resource::<RainSoundTime>()
            .add_systems(
                Update,
                setup.run_if(
                    resource_exists::<super::systems::AssetsReady>
                        .and(resource_exists::<super::terrain::LightmapPixels>)
                        .and(not(resource_exists::<WeatherAssets>)),
                ),
            )
            .add_systems(
                Update,
                reload_textures
                    .run_if(on_message::<super::reload::AssetsReloaded>)
                    .run_if(resource_exists::<WeatherAssets>),
            )
            .add_systems(
                PostUpdate,
                update_weather
                    .in_set(super::CameraPinnedMeshes)
                    .run_if(in_state(AppState::InGame))
                    .run_if(resource_exists::<WeatherAssets>),
            );
    }
}

#[derive(Component)]
pub(crate) struct WeatherLayer;

#[derive(Resource, Default)]
pub(crate) struct RainSoundTime(pub i32);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Column {
    x: i32,
    z: i32,
    bottom: i32,
    top: i32,
    snow: bool,
    block: u8,
    sky: u8,
}

#[derive(Resource)]
struct WeatherAssets {
    mesh: Handle<Mesh>,
    material: Handle<WeatherMaterial>,
    scanned: Option<(IVec3, u64)>,
    columns: Vec<Column>,
    built_at: Option<IVec3>,
    size_x: [f32; TABLE * TABLE],
    size_z: [f32; TABLE * TABLE],
}

fn load_texture(name: &str) -> Image {
    let path = crate::textures_dir().join("environment").join(name);
    let (width, height, data) = match crate::platform::assets::open_image(&path) {
        Ok(img) => {
            let img = img.to_rgba8();
            (img.width(), img.height(), img.into_raw())
        }
        Err(e) => {
            crate::log_warn!(
                "assets",
                "weather: {} could not be read ({e})",
                path.display()
            );
            (1, 1, vec![0; 4])
        }
    };
    let mut image = super::rgba_image(width, height, data, RenderAssetUsages::RENDER_WORLD);
    image.sampler = crate::entities::repeating_nearest();
    image
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<WeatherMaterial>>,
    lightmap: Res<super::terrain::LightmapPixels>,
) {
    let material = materials.add(WeatherMaterial {
        rain: images.add(load_texture("rain.png")),
        snow: images.add(load_texture("snow.png")),
        lightmap: lightmap.image.clone(),
        params: WeatherParams::default(),
    });
    let mesh = meshes.add(write_mesh(
        &[],
        IVec3::ZERO,
        &[0.0; TABLE * TABLE],
        &[0.0; TABLE * TABLE],
    ));
    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(material.clone()),
        Transform::default(),
        Visibility::Hidden,
        NoFrustumCulling,
        WeatherLayer,
    ));

    let mut size_x = [0.0; TABLE * TABLE];
    let mut size_z = [0.0; TABLE * TABLE];
    for z in 0..TABLE {
        for x in 0..TABLE {
            let dx = x as f32 - HALF_TABLE as f32;
            let dz = z as f32 - HALF_TABLE as f32;
            let distance = (dx * dx + dz * dz).sqrt();
            size_x[z * TABLE + x] = -dz / distance;
            size_z[z * TABLE + x] = dx / distance;
        }
    }
    commands.insert_resource(WeatherAssets {
        mesh,
        material,
        scanned: None,
        columns: Vec::new(),
        built_at: None,
        size_x,
        size_z,
    });
}

fn reload_textures(
    assets: Res<WeatherAssets>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<WeatherMaterial>>,
) {
    if let Some(material) = materials.get_mut(&assets.material) {
        material.rain = images.add(load_texture("rain.png"));
        material.snow = images.add(load_texture("snow.png"));
    }
}

#[allow(clippy::too_many_arguments)]
fn update_weather(
    view: Res<super::FrameView>,
    env: Res<super::environment::Environment>,
    origin: Res<super::RenderOrigin>,
    mut assets: ResMut<WeatherAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WeatherMaterial>>,
    mut layer: Query<(&mut Transform, &mut Visibility), With<WeatherLayer>>,
) {
    crate::prof_span!("render:update_weather");
    let Ok((mut transform, mut visibility)) = layer.single_mut() else {
        return;
    };
    let intensity = env.rain;
    if intensity <= 0.0 {
        visibility.set_if_neq(Visibility::Hidden);
        if assets.scanned.is_some() {
            assets.scanned = None;
        }
        return;
    }

    let eye = view.camera_world;
    let camera = IVec3::new(
        eye.x.floor() as i32,
        eye.y.floor() as i32,
        eye.z.floor() as i32,
    );
    let tick = view.game_time;
    if assets.scanned != Some((camera, tick)) {
        assets.scanned = Some((camera, tick));
        let columns = scan(camera);
        let moved = assets.built_at != Some(camera);
        if moved || columns != assets.columns {
            let rebuilt = write_mesh(&columns, camera, &assets.size_x, &assets.size_z);
            if let Some(mesh) = meshes.get_mut(&assets.mesh) {
                *mesh = rebuilt;
            }
            assets.built_at = Some(camera);
            assets.columns = columns;
        }
    }
    if assets.columns.is_empty() {
        visibility.set_if_neq(Visibility::Hidden);
        return;
    }
    visibility.set_if_neq(Visibility::Visible);
    let at = origin.to_render(DVec3::new(f64::from(camera.x), 0.0, f64::from(camera.z)));
    if transform.translation != at {
        transform.translation = at;
    }

    let params = WeatherParams {
        fade: Vec4::new(intensity, (RADIUS * RADIUS) as f32, 0.0, 0.0),
    };
    if materials
        .get(&assets.material)
        .is_some_and(|m| m.params != params)
        && let Some(material) = materials.get_mut(&assets.material)
    {
        material.params = params;
    }
}

struct ColumnProbe<'w> {
    world: &'w azalea_world::World,
    biomes: std::sync::Arc<Vec<u8>>,
    min_y: i32,
    quart_lo: i32,
    quart_hi: i32,
    sea_level: i32,
    held_pos: Option<ChunkPos>,
    held: Option<std::sync::Arc<parking_lot::RwLock<azalea_world::Chunk>>>,
}

impl<'w> ColumnProbe<'w> {
    fn new(world: &'w azalea_world::World) -> Self {
        let min_y = world.chunks.min_y();
        let height = world.chunks.height() as i32;
        ColumnProbe {
            world,
            biomes: crate::client::worker::biome_table(&world.registries),
            min_y,
            quart_lo: min_y >> 2,
            quart_hi: ((min_y + height) >> 2) - 1,
            sea_level: super::dimension::sea_level(),
            held_pos: None,
            held: None,
        }
    }

    fn column(&mut self, x: i32, y: i32, z: i32) -> Option<(i32, Precipitation)> {
        let pos = ChunkPos {
            x: x >> 4,
            z: z >> 4,
        };
        if self.held_pos != Some(pos) {
            self.held = self.world.chunks.get(&pos);
            self.held_pos = Some(pos);
        }
        let chunk = self.held.as_ref()?.read();
        let height = chunk
            .heightmaps
            .get(&HeightmapKind::MotionBlocking)
            .map_or(self.min_y, |h| {
                h.get_first_available((x & 15) as u8, (z & 15) as u8)
            });
        let quart = [x >> 2, (y >> 2).clamp(self.quart_lo, self.quart_hi), z >> 2];
        let row = crate::client::envprobe::quart_biome(&chunk, self.min_y, &self.biomes, quart);
        let env = crate::util::biome_color::env(row);
        Some((height, precipitation_at(env, y, self.sea_level)))
    }
}

fn rain_reaches(
    rain: f32,
    can_have_weather: bool,
    sky_light: u8,
    height: i32,
    y: i32,
    precipitation: Precipitation,
) -> bool {
    rain > 0.2
        && can_have_weather
        && sky_light >= 15
        && height <= y
        && precipitation == Precipitation::Rain
}

#[test]
fn rain_needs_every_input() {
    use Precipitation::{Rain, Snow};
    assert!(rain_reaches(0.21, true, 15, 64, 64, Rain));
    assert!(!rain_reaches(0.2, true, 15, 64, 64, Rain));
    assert!(!rain_reaches(1.0, false, 15, 64, 64, Rain));
    assert!(!rain_reaches(1.0, true, 14, 64, 64, Rain));
    assert!(!rain_reaches(1.0, true, 15, 65, 64, Rain));
    assert!(!rain_reaches(1.0, true, 15, 64, 64, Snow));
}

pub(crate) fn raining_at(world: &azalea_world::World, cells: &[[i32; 3]]) -> bool {
    let rain = super::environment::rain_level();
    let can_have_weather = super::dimension::current().can_have_weather();
    if rain <= 0.2 || !can_have_weather {
        return false;
    }
    let light = crate::client::tracking::read_lock(
        crate::client::worldsync::light_map(),
        "light:raining_at",
    );
    let mut probe = ColumnProbe::new(world);
    cells.iter().any(|&[x, y, z]| {
        probe
            .column(x, y, z)
            .is_some_and(|(height, precipitation)| {
                let sky = super::lightmap::levels_in(&light, [x, y, z]).1;
                rain_reaches(rain, can_have_weather, sky, height, y, precipitation)
            })
    })
}

fn scan(camera: IVec3) -> Vec<Column> {
    let Some(world) = crate::client::tracking::current_world()
        .lock()
        .unwrap()
        .clone()
    else {
        return Vec::new();
    };
    let world = crate::client::tracking::read_lock(&world, "weather_scan");
    let light = crate::client::tracking::read_lock(
        crate::client::worldsync::light_map(),
        "light:weather_scan",
    );
    let mut probe = ColumnProbe::new(&world);
    let mut columns = Vec::with_capacity(((2 * RADIUS + 1) * (2 * RADIUS + 1)) as usize);
    for z in camera.z - RADIUS..=camera.z + RADIUS {
        for x in camera.x - RADIUS..=camera.x + RADIUS {
            if x == camera.x && z == camera.z {
                continue;
            }
            let Some((terrain, precipitation)) = probe.column(x, camera.y, z) else {
                continue;
            };
            let bottom = (camera.y - RADIUS).max(terrain);
            let top = (camera.y + RADIUS).max(terrain);
            if top == bottom || precipitation == Precipitation::None {
                continue;
            }
            let at = [x, camera.y.max(terrain), z];
            let (mut block, mut sky) = super::lightmap::levels_in(&light, at);
            if let Some(state) = crate::util::particles::block_at(&world, at) {
                block = block.max(crate::lighting::props::of(state).emission);
            }
            let snow = precipitation == Precipitation::Snow;
            if snow {
                block = (block * 3 + 15) / 4;
                sky = (sky * 3 + 15) / 4;
            }
            columns.push(Column {
                x,
                z,
                bottom,
                top,
                snow,
                block,
                sky,
            });
        }
    }
    columns
}

fn column_seed(x: i32, z: i32) -> i32 {
    let a = x
        .wrapping_mul(x)
        .wrapping_mul(3121)
        .wrapping_add(x.wrapping_mul(45_238_971));
    let b = z
        .wrapping_mul(z)
        .wrapping_mul(418_711)
        .wrapping_add(z.wrapping_mul(13_761));
    a ^ b
}

fn tick_offset(x: i32, z: i32) -> i32 {
    let a = x
        .wrapping_mul(x)
        .wrapping_mul(3121)
        .wrapping_add(x.wrapping_mul(45_238_971));
    let b = z
        .wrapping_mul(z)
        .wrapping_mul(418_711)
        .wrapping_add(z.wrapping_mul(13_761));
    a.wrapping_add(b) & 255
}

fn write_mesh(
    columns: &[Column],
    camera: IVec3,
    size_x: &[f32; TABLE * TABLE],
    size_z: &[f32; TABLE * TABLE],
) -> Mesh {
    let n = columns.len() * 4;
    let mut positions = Vec::with_capacity(n);
    let mut uvs = Vec::with_capacity(n);
    let mut params = Vec::with_capacity(n);
    let mut packed = Vec::with_capacity(n);
    let mut indices = Vec::with_capacity(columns.len() * 6);

    for column in columns {
        let dx = column.x - camera.x;
        let dz = column.z - camera.z;
        let index = ((dz + HALF_TABLE) * TABLE as i32 + dx + HALF_TABLE) as usize;
        let half_x = size_x[index] / 2.0;
        let half_z = size_z[index] / 2.0;
        let centre_x = dx as f32 + 0.5;
        let centre_z = dz as f32 + 0.5;
        let (x0, x1) = (centre_x - half_x, centre_x + half_x);
        let (z0, z1) = (centre_z - half_z, centre_z + half_z);
        let (y0, y1) = (column.bottom as f32, column.top as f32);

        let mut random = JavaRandom::new(i64::from(column_seed(column.x, column.z)));
        let (u0, v_base, a, b) = if column.snow {
            let u = random.next_f64() as f32;
            let u_rate = 0.01 * random.next_gaussian() as f32;
            let v = random.next_f64() as f32;
            let v_rate = random.next_gaussian() as f32 * 0.001;
            (u, v, u_rate, v_rate)
        } else {
            let speed = 3.0 + random.next_f32();
            (0.0, 0.0, tick_offset(column.x, column.z) as f32, speed)
        };
        let u1 = u0 + 1.0;
        let v0 = y0 * 0.25 + v_base;
        let v1 = y1 * 0.25 + v_base;
        let word =
            u32::from(column.block) | u32::from(column.sky) << 4 | u32::from(column.snow) << 8;
        let base = positions.len() as u32;
        positions.extend([[x0, y1, z0], [x1, y1, z1], [x1, y0, z1], [x0, y0, z0]]);
        uvs.extend([[u0, v0], [u1, v0], [u1, v1], [u0, v1]]);
        params.extend([[centre_x, centre_z, a, b]; 4]);
        packed.extend([word; 4]);
        indices.extend(crate::entities::quads::QUAD_INDICES.map(|i| base + i));
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(ATTRIBUTE_UV, uvs);
    mesh.insert_attribute(ATTRIBUTE_COLUMN, params);
    mesh.insert_attribute(ATTRIBUTE_PACKED, packed);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

pub(crate) fn tick_rain_particles(ctx: &mut Emitter, rain: f32, tick: u64, sound_time: &mut i32) {
    use azalea::entity::particle::Particle as P;

    if rain <= 0.0 {
        return;
    }
    let mut random = JavaRandom::new((tick as i64).wrapping_mul(312_987_231));
    let camera = IVec3::new(
        ctx.eye.x.floor() as i32,
        ctx.eye.y.floor() as i32,
        ctx.eye.z.floor() as i32,
    );
    let diameter = 2 * RADIUS + 1;
    let area = (diameter * diameter) as f32;
    let mut count = (RAIN_PARTICLES_PER_BLOCK * area * rain * rain) as i32;
    if ctx.status == ParticleStatus::Decreased {
        count /= 2;
    }

    let mut probe = ColumnProbe::new(ctx.world);
    let mut landed: Option<IVec3> = None;
    for _ in 0..count {
        let x = camera.x + random.next_int(diameter as u32) as i32 - RADIUS;
        let z = camera.z + random.next_int(diameter as u32) as i32 - RADIUS;
        let Some((height, _)) = probe.column(x, camera.y, z) else {
            continue;
        };
        if height <= probe.min_y || height > camera.y + 10 || height < camera.y - 10 {
            continue;
        }
        if probe.column(x, height, z).map(|(_, p)| p) != Some(Precipitation::Rain) {
            continue;
        }
        let pos = IVec3::new(x, height - 1, z);
        landed = Some(pos);
        if ctx.status == ParticleStatus::Minimal {
            break;
        }
        let block_x = random.next_f64();
        let block_z = random.next_f64();
        let cell = pos.to_array();
        let Some(state) = ctx.block(cell) else {
            continue;
        };
        let top = surface_top(ctx.world, state, pos, block_x, block_z);
        let fluid = azalea::block::fluid_state::FluidState::from(state);
        let smoke = fluid.kind == azalea::block::fluid_state::FluidKind::Lava
            || state.as_block_kind() == azalea_registry::builtin::BlockKind::MagmaBlock
            || matches!(
                crate::blocks::ambient::of(state),
                Some(crate::blocks::ambient::Ambient::Campfire)
            );
        let at = DVec3::new(
            f64::from(pos.x) + block_x,
            f64::from(pos.y) + top,
            f64::from(pos.z) + block_z,
        );
        ctx.emit(
            if smoke { &P::Smoke } else { &P::Rain },
            at,
            Vec3::ZERO,
            false,
        );
    }

    let Some(pos) = landed else {
        return;
    };
    let previous = *sound_time;
    *sound_time += 1;
    if random.next_int(3) as i32 >= previous {
        return;
    }
    *sound_time = 0;
    #[cfg(feature = "audio")]
    {
        let covered = probe
            .column(camera.x, camera.y, camera.z)
            .is_some_and(|(height, _)| height > camera.y);
        let (event, volume, pitch) = if pos.y > camera.y + 1 && covered {
            ("weather.rain.above", 0.1, 0.5)
        } else {
            ("weather.rain", 0.2, 1.0)
        };
        let at = crate::util::block_centre(pos.to_array()).to_array();
        crate::audio::play_at(
            event,
            crate::audio::SoundCategory::Weather,
            at,
            volume,
            pitch,
        );
    }
    #[cfg(not(feature = "audio"))]
    let _ = pos;
}

fn shape_top(state: azalea::block::BlockState, pos: IVec3, x: f64, z: f64) -> f64 {
    use azalea::physics::collision::BlockWithShape;
    if state.is_collision_shape_empty() {
        return 0.0;
    }
    let block_pos = azalea_core::position::BlockPos::new(pos.x, pos.y, pos.z);
    state
        .collision_shape(block_pos)
        .to_aabbs()
        .iter()
        .filter(|b| b.min.x <= x && x <= b.max.x && b.min.z <= z && z <= b.max.z)
        .map(|b| b.max.y)
        .fold(0.0, f64::max)
}

pub(crate) fn surface_top(
    world: &azalea_world::World,
    state: azalea::block::BlockState,
    pos: IVec3,
    x: f64,
    z: f64,
) -> f64 {
    shape_top(state, pos, x, z).max(f64::from(fluid_top(world, state, pos.to_array())))
}

fn fluid_top(world: &azalea_world::World, state: azalea::block::BlockState, cell: [i32; 3]) -> f32 {
    use azalea::block::fluid_state::FluidState;
    let fluid = FluidState::from(state);
    if fluid.is_empty() {
        return 0.0;
    }
    let above = crate::util::particles::block_at(world, [cell[0], cell[1] + 1, cell[2]])
        .map(FluidState::from)
        .unwrap_or_default();
    fluid.height_under(&above)
}
