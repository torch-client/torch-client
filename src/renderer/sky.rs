use crate::util::javarandom::JavaRandom;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::camera::{Camera, ClearColorConfig};
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::transform::TransformSystems;
#[cfg(feature = "builtin_shaders")]
use bevy::{
    asset::{Asset, embedded_asset},
    mesh::{MeshBuilder, MeshVertexBufferLayoutRef, SphereKind, SphereMeshBuilder},
    pbr::{Material, MaterialPipeline, MaterialPlugin},
    render::render_resource::{
        AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
};

use super::AppState;
use super::dimension::{self, Skybox};
use super::systems::{Shared, WorldCamera};
use super::timeline::{self, Argb, HORIZON_HEIGHT, SkyState};

const SKY_SCALE: f32 = 16.0;

const SKY_DISC_RADIUS: f32 = 512.0;
const SKY_DISC_HEIGHT: f32 = 16.0;
const DARK_DISC_OFFSET: f32 = 12.0;
const SUN_SIZE: f32 = 30.0;
const SUN_HEIGHT: f32 = 100.0;
const MOON_SIZE: f32 = 20.0;
const MOON_HEIGHT: f32 = 100.0;
const STAR_COUNT: usize = 1500;
const STAR_DISTANCE: f32 = 100.0;
const SUNRISE_STEPS: usize = 16;
const STAR_SEED: i64 = 10842;

const DISC_SIDES: usize = 8;

const DISC_RINGS: usize = 16;

#[cfg(feature = "builtin_shaders")]
#[derive(Clone, Copy, ShaderType)]
pub struct SkyParams {
    pub sun: Vec4,
    pub tuning: Vec4,
    pub sky_color: Vec4,
    pub fog_color: Vec4,
}

#[cfg(feature = "builtin_shaders")]
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct SkyMaterial {
    #[uniform(0)]
    pub params: SkyParams,
}

#[cfg(feature = "builtin_shaders")]
impl Material for SkyMaterial {
    fn fragment_shader() -> ShaderRef {
        let crate_name = module_path!().split(':').next().unwrap_or("torch_client");
        ShaderRef::Path(format!("embedded://{crate_name}/renderer/sky.wgsl").into())
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn depth_bias(&self) -> f32 {
        BIAS_SKY_DOME
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

#[cfg(feature = "builtin_shaders")]
#[derive(Component)]
struct SkyDome;

#[cfg(feature = "builtin_shaders")]
const DOME_SECTORS: u32 = 48;
#[cfg(feature = "builtin_shaders")]
const DOME_STACKS: u32 = 24;

#[cfg(feature = "builtin_shaders")]
const GLARE_GAIN: f32 = 6.0;

const BIAS_SKY_DISC: f32 = 0.0;
#[cfg(feature = "builtin_shaders")]
const BIAS_SKY_DOME: f32 = -1000.0 * SKY_SCALE;
const BIAS_END_SKY: f32 = 0.0;
const BIAS_SUNRISE: f32 = 1000.0 * SKY_SCALE;
const BIAS_SUN: f32 = 2000.0 * SKY_SCALE;
const BIAS_MOON: f32 = 3000.0 * SKY_SCALE;
const BIAS_STARS: f32 = 4000.0 * SKY_SCALE;
const BIAS_DARK_DISC: f32 = 5000.0 * SKY_SCALE;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "builtin_shaders")]
        {
            embedded_asset!(app, "sky.wgsl");
            app.add_plugins(MaterialPlugin::<SkyMaterial>::default());
        }
        app.add_systems(
            Update,
            setup.run_if(resource_added::<crate::renderer::systems::AssetsReady>),
        )
        .add_systems(
            PostUpdate,
            update_sky
                .before(TransformSystems::Propagate)
                .run_if(in_state(AppState::InGame))
                .run_if(resource_exists::<SkyAssets>),
        );
    }
}

#[derive(Component)]
pub struct SkyRoot;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum SkyPart {
    SkyDisc,
    DarkDisc,
    Sun,
    Moon,
    Stars,
    SunriseFan,
    EndSky,
}

#[derive(Resource)]
struct SkyAssets {
    sky_disc_mesh: Handle<Mesh>,
    dark_disc_mesh: Handle<Mesh>,
    sunrise_material: Handle<StandardMaterial>,
    star_material: Handle<StandardMaterial>,
    moon_materials: Vec<Handle<StandardMaterial>>,
    discs_built_for: Option<(Argb, Argb, f32)>,
    moon_phase: usize,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    #[cfg(feature = "builtin_shaders")] mut sky_materials: ResMut<Assets<SkyMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let celestial = crate::textures_dir().join("environment/celestial");

    let root = commands
        .spawn((SkyRoot, Transform::IDENTITY, Visibility::Hidden))
        .id();

    #[cfg(feature = "builtin_shaders")]
    commands.spawn((
        Mesh3d(
            meshes.add(
                SphereMeshBuilder::new(
                    SKY_DISC_RADIUS,
                    SphereKind::Uv {
                        sectors: DOME_SECTORS,
                        stacks: DOME_STACKS,
                    },
                )
                .build(),
            ),
        ),
        MeshMaterial3d(sky_materials.add(SkyMaterial {
            params: SkyParams {
                sun: Vec4::new(0.0, 1.0, 0.0, 1.0),
                tuning: Vec4::new(1.0, GLARE_GAIN, 0.0, 0.0),
                sky_color: Vec4::ONE,
                fog_color: Vec4::ONE,
            },
        })),
        Transform::IDENTITY,
        ChildOf(root),
        NotShadowCaster,
        NoFrustumCulling,
        SkyDome,
    ));

    let sky_material = materials.add(StandardMaterial {
        depth_bias: BIAS_SKY_DISC,
        ..sky_material_props()
    });
    let dark_material = materials.add(StandardMaterial {
        depth_bias: BIAS_DARK_DISC,
        ..sky_material_props()
    });
    let sunrise_material = materials.add(StandardMaterial {
        depth_bias: BIAS_SUNRISE,
        ..sky_material_props()
    });
    let star_material = materials.add(StandardMaterial {
        depth_bias: BIAS_STARS,
        ..celestial_material_props()
    });

    let sun_material = materials.add(StandardMaterial {
        base_color_texture: Some(images.add(load_celestial(&celestial.join("sun.png")))),
        depth_bias: BIAS_SUN,
        ..celestial_material_props()
    });
    let moon_materials: Vec<Handle<StandardMaterial>> = timeline::MOON_PHASE_NAMES
        .iter()
        .map(|name| {
            let path = celestial.join(format!("moon/{name}.png"));
            materials.add(StandardMaterial {
                base_color_texture: Some(images.add(load_celestial(&path))),
                depth_bias: BIAS_MOON,
                ..celestial_material_props()
            })
        })
        .collect();

    let white = Argb(0xFFFF_FFFF);
    let black = Argb(0xFF00_0000);
    let sky_disc_mesh = meshes.add(disc_mesh(SKY_DISC_HEIGHT, white, white, SKY_DISC_RADIUS));
    let dark_disc_mesh = meshes.add(disc_mesh(-SKY_DISC_HEIGHT, black, white, SKY_DISC_RADIUS));

    commands.spawn((
        Mesh3d(sky_disc_mesh.clone()),
        MeshMaterial3d(sky_material.clone()),
        Transform::IDENTITY,
        ChildOf(root),
        NotShadowCaster,
        NoFrustumCulling,
        SkyPart::SkyDisc,
    ));
    commands.spawn((
        Mesh3d(dark_disc_mesh.clone()),
        MeshMaterial3d(dark_material.clone()),
        Transform::from_xyz(0.0, DARK_DISC_OFFSET, 0.0),
        Visibility::Hidden,
        ChildOf(root),
        NotShadowCaster,
        NoFrustumCulling,
        SkyPart::DarkDisc,
    ));
    commands.spawn((
        Mesh3d(meshes.add(celestial_quad(false))),
        MeshMaterial3d(sun_material),
        Transform::IDENTITY,
        ChildOf(root),
        NotShadowCaster,
        NoFrustumCulling,
        SkyPart::Sun,
    ));
    commands.spawn((
        Mesh3d(meshes.add(celestial_quad(true))),
        MeshMaterial3d(moon_materials[0].clone()),
        Transform::IDENTITY,
        ChildOf(root),
        NotShadowCaster,
        NoFrustumCulling,
        SkyPart::Moon,
    ));
    commands.spawn((
        Mesh3d(meshes.add(star_mesh())),
        MeshMaterial3d(star_material.clone()),
        Transform::IDENTITY,
        Visibility::Hidden,
        ChildOf(root),
        NotShadowCaster,
        NoFrustumCulling,
        SkyPart::Stars,
    ));
    commands.spawn((
        Mesh3d(meshes.add(sunrise_mesh())),
        MeshMaterial3d(sunrise_material.clone()),
        Transform::IDENTITY,
        Visibility::Hidden,
        ChildOf(root),
        NotShadowCaster,
        NoFrustumCulling,
        SkyPart::SunriseFan,
    ));
    let end_sky_material = materials.add(StandardMaterial {
        base_color_texture: Some(images.add(load_tiled(
            &crate::textures_dir().join("environment/end_sky.png"),
        ))),
        depth_bias: BIAS_END_SKY,
        ..sky_material_props()
    });
    commands.spawn((
        Mesh3d(meshes.add(end_sky_mesh())),
        MeshMaterial3d(end_sky_material),
        Transform::IDENTITY,
        Visibility::Hidden,
        ChildOf(root),
        NotShadowCaster,
        NoFrustumCulling,
        SkyPart::EndSky,
    ));

    commands.insert_resource(SkyAssets {
        sky_disc_mesh,
        dark_disc_mesh,
        sunrise_material,
        star_material,
        moon_materials,
        discs_built_for: None,
        moon_phase: 0,
    });
}

fn sky_material_props() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        double_sided: true,
        fog_enabled: false,
        ..default()
    }
}

fn celestial_material_props() -> StandardMaterial {
    StandardMaterial {
        alpha_mode: AlphaMode::Add,
        ..sky_material_props()
    }
}

fn load_celestial(path: &std::path::Path) -> Image {
    use bevy::image::ImageSampler;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    let (w, h, data) = match crate::platform::assets::open_image(path) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            (rgba.width(), rgba.height(), rgba.into_raw())
        }
        Err(e) => {
            eprintln!("sky: {} could not be read ({e})", path.display());
            (1, 1, vec![0u8; 4])
        }
    };
    let mut image = Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    image
}

fn load_tiled(path: &std::path::Path) -> Image {
    use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};

    let mut image = load_celestial(path);
    let mut descriptor = ImageSamplerDescriptor::nearest();
    descriptor.address_mode_u = ImageAddressMode::Repeat;
    descriptor.address_mode_v = ImageAddressMode::Repeat;
    image.sampler = ImageSampler::Descriptor(descriptor);
    image
}

fn disc_mesh(yy: f32, sky: Argb, fog: Argb, sky_end: f32) -> Mesh {
    use crate::util::mth::srgb_to_linear;

    let sign = yy.signum();
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let sky = sky.to_rgba_f32();
    let fog = fog.to_rgba_f32();
    let shade = |dist: f32| -> [f32; 4] {
        let f = if sky_end <= 0.0 {
            1.0
        } else {
            (dist / sky_end).clamp(0.0, 1.0)
        };
        let mix = |a: f32, b: f32| srgb_to_linear(a + (b - a) * f);
        [
            mix(sky[0], fog[0]),
            mix(sky[1], fog[1]),
            mix(sky[2], fog[2]),
            sky[3],
        ]
    };

    let height = yy.abs();
    positions.push([0.0, yy, 0.0]);
    colors.push(shade(height));

    let mut radii: Vec<f32> = Vec::with_capacity(DISC_RINGS + 1);
    for k in 1..=DISC_RINGS {
        let dist = height + (sky_end.max(height) - height) * (k as f32 / DISC_RINGS as f32);
        let r = (dist * dist - height * height).max(0.0).sqrt();
        radii.push(r.min(SKY_DISC_RADIUS));
    }
    radii.push(SKY_DISC_RADIUS);

    for &r in &radii {
        for i in 0..DISC_SIDES {
            let angle = (-180.0 + 45.0 * i as f32).to_radians();
            positions.push([sign * r * angle.cos(), yy, r * angle.sin()]);
            colors.push(shade((r * r + height * height).sqrt()));
        }
    }

    for i in 0..DISC_SIDES {
        let a = 1 + i as u32;
        let b = 1 + ((i + 1) % DISC_SIDES) as u32;
        indices.extend_from_slice(&[0, a, b]);
    }
    for ring in 0..radii.len() - 1 {
        let inner = 1 + (ring * DISC_SIDES) as u32;
        let outer = inner + DISC_SIDES as u32;
        for i in 0..DISC_SIDES as u32 {
            let j = (i + 1) % DISC_SIDES as u32;
            indices.extend_from_slice(&[inner + i, outer + i, outer + j]);
            indices.extend_from_slice(&[inner + i, outer + j, inner + j]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn celestial_quad(mirrored: bool) -> Mesh {
    let positions = [
        [-1.0, 0.0, -1.0],
        [1.0, 0.0, -1.0],
        [1.0, 0.0, 1.0],
        [-1.0, 0.0, 1.0],
    ];
    let uvs: [[f32; 2]; 4] = if mirrored {
        [[1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0]]
    } else {
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]
    };
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions.to_vec());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 4]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs.to_vec());
    mesh.insert_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
    mesh
}

fn end_sky_mesh() -> Mesh {
    use crate::util::mth::srgb_to_linear;

    const CORNERS: [[f32; 3]; 4] = [
        [-100.0, -100.0, -100.0],
        [-100.0, -100.0, 100.0],
        [100.0, -100.0, 100.0],
        [100.0, -100.0, -100.0],
    ];
    const UVS: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 16.0], [16.0, 16.0], [16.0, 0.0]];

    let quarter = std::f32::consts::FRAC_PI_2;
    let rotations = [
        Quat::IDENTITY,
        Quat::from_rotation_x(quarter),
        Quat::from_rotation_x(-quarter),
        Quat::from_rotation_x(std::f32::consts::PI),
        Quat::from_rotation_z(quarter),
        Quat::from_rotation_z(-quarter),
    ];

    let grey = srgb_to_linear(40.0 / 255.0);
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(24);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(24);
    let mut indices: Vec<u32> = Vec::with_capacity(36);
    for (face, rotation) in rotations.iter().enumerate() {
        let base = face as u32 * 4;
        for corner in 0..4 {
            positions.push((*rotation * Vec3::from(CORNERS[corner])).to_array());
            uvs.push(UVS[corner]);
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    let n = positions.len();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[grey, grey, grey, 1.0]; n]);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn sunrise_mesh() -> Mesh {
    let mut positions = vec![[0.0f32, 100.0, 0.0]];
    let mut colors = vec![[1.0f32, 1.0, 1.0, 1.0]];
    for i in 0..=SUNRISE_STEPS {
        let angle = i as f32 * std::f32::consts::TAU / SUNRISE_STEPS as f32;
        let (sin, cos) = (angle.sin(), angle.cos());
        positions.push([sin * 120.0, cos * 120.0, -cos * 40.0]);
        colors.push([1.0, 1.0, 1.0, 0.0]);
    }
    let mut indices = Vec::new();
    for i in 1..SUNRISE_STEPS as u32 + 1 {
        indices.extend_from_slice(&[0, i, i + 1]);
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    let n = positions.len();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; n]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn star_mesh() -> Mesh {
    let mut random = JavaRandom::new(STAR_SEED);
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for _ in 0..STAR_COUNT {
        let x = random.next_f32() * 2.0 - 1.0;
        let y = random.next_f32() * 2.0 - 1.0;
        let z = random.next_f32() * 2.0 - 1.0;
        let size = 0.15 + random.next_f32() * 0.1;
        let length_sq = x * x + y * y + z * z;
        if length_sq <= 0.010000001 || length_sq >= 1.0 {
            continue;
        }
        let center = Vec3::new(x, y, z).normalize() * STAR_DISTANCE;
        let z_rot = (random.next_f64() * std::f64::consts::PI * 2.0) as f32;

        let dir = (-center).normalize();
        let left = Vec3::Y.cross(dir);
        if left.length_squared() < 1e-12 {
            continue;
        }
        let left = left.normalize();
        let up = dir.cross(left);
        let rotation = Mat3::from_cols(left, up, dir) * Mat3::from_rotation_z(-z_rot);

        let base = positions.len() as u32;
        for corner in [
            Vec3::new(size, -size, 0.0),
            Vec3::new(size, size, 0.0),
            Vec3::new(-size, size, 0.0),
            Vec3::new(-size, -size, 0.0),
        ] {
            positions.push((rotation * corner + center).to_array());
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    let n = positions.len();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n]);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

use crate::util::block_model::{EyeFluid, eye_fluid_at};

const LAVA_FOG_COLOR: Argb = Argb(0xFF99_1900);

const LAVA_FOG_RANGE: (f32, f32) = (0.25, 1.0);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_sky(
    shared: Res<Shared>,
    env: Res<super::environment::Environment>,
    mut assets: ResMut<SkyAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    #[cfg(feature = "builtin_shaders")] mut sky_materials: ResMut<Assets<SkyMaterial>>,
    #[cfg(feature = "builtin_shaders")] mut dome: Query<
        (&MeshMaterial3d<SkyMaterial>, &mut Visibility),
        (
            With<SkyDome>,
            Without<SkyPart>,
            Without<WorldCamera>,
            Without<SkyRoot>,
        ),
    >,
    world_camera: Query<&Transform, (With<WorldCamera>, Without<SkyRoot>, Without<SkyPart>)>,
    mut world_view: Query<(&mut DistanceFog, &mut Camera), With<WorldCamera>>,
    mut root: Query<&mut Transform, (With<SkyRoot>, Without<WorldCamera>, Without<SkyPart>)>,
    mut parts: Query<
        (
            &SkyPart,
            &mut Transform,
            &mut Visibility,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        (Without<WorldCamera>, Without<SkyRoot>),
    >,
) {
    crate::prof_span!("render:update_sky");
    let (eye_x, eye_y, eye_z) = {
        let state = shared.0.lock().unwrap();
        let eye_height = super::systems::session_eye_height(&state.session) as f64;
        (
            state.session.player_pos[0] as f64,
            state.session.player_pos[1] as f64 + eye_height,
            state.session.player_pos[2] as f64,
        )
    };
    let dim = dimension::current();
    let skybox = dim.skybox();
    let mut sky = env.sky;

    let Ok(world_transform) = world_camera.single() else {
        return;
    };

    sky.fog_color = super::environment::fog_base_color(
        &sky,
        env.sky_fog_end,
        world_transform.forward().x,
        env.render_distance,
        env.rain,
        env.thunder,
    );

    let eye_fluid = eye_fluid_at(eye_x, eye_y, eye_z);
    let environmental = match eye_fluid {
        Some(EyeFluid::Water) => {
            sky.fog_color = env.water_fog_color;
            env.water_fog_distance
        }
        Some(EyeFluid::Lava) => {
            sky.fog_color = LAVA_FOG_COLOR;
            LAVA_FOG_RANGE
        }
        None => env.fog_distance,
    };
    let fog_color = sky.fog_color.to_color();

    if let Ok((mut fog, mut camera)) = world_view.single_mut() {
        fog.color = fog_color;
        fog.falloff = FogFalloff::Linear {
            start: env.render_fog_distance.0,
            end: env.render_fog_distance.1,
        };
        fog.directional_light_color =
            Color::linear_rgba(environmental.0, environmental.1, 0.0, 0.0);
        camera.clear_color = ClearColorConfig::Custom(fog_color);
    }

    if let Ok(mut rig) = root.single_mut() {
        rig.translation = world_transform.translation;
        rig.scale = Vec3::splat(SKY_SCALE);
    }

    rebuild_discs(&mut assets, &mut meshes, &sky, env.sky_fog_end);

    let yaw = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2);
    let place = |angle: f32, height: f32, size: f32| {
        let rotation = yaw * Quat::from_rotation_x(angle);
        Transform {
            translation: rotation * Vec3::new(0.0, height, 0.0),
            rotation,
            scale: Vec3::new(size, 1.0, size),
        }
    };

    let sunrise_alpha = sky.sunrise_color.alpha() as f32 / 255.0;

    #[cfg(feature = "builtin_shaders")]
    if let Ok((handle, mut dome_visibility)) = dome.single_mut() {
        let fancy = crate::renderer::terrain::builtin_shaders_enabled();
        *dome_visibility = if skybox == Skybox::Overworld && fancy {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if fancy && let Some(material) = sky_materials.get_mut(&handle.0) {
            let to_sun = (yaw * Quat::from_rotation_x(sky.sun_angle)) * Vec3::Y;
            let sky_rgba = sky.sky_color.to_rgba_f32();
            let fog_rgba = sky.fog_color.to_rgba_f32();
            let linear = |c: [f32; 4]| {
                Vec4::new(
                    crate::util::mth::srgb_to_linear(c[0]),
                    crate::util::mth::srgb_to_linear(c[1]),
                    crate::util::mth::srgb_to_linear(c[2]),
                    1.0,
                )
            };
            material.params = SkyParams {
                sun: to_sun.extend(to_sun.y.max(0.0)),
                tuning: Vec4::new(
                    ((to_sun.y + 0.15) * 3.0).clamp(0.0, 1.0),
                    GLARE_GAIN,
                    0.0,
                    0.0,
                ),
                sky_color: linear(sky_rgba),
                fog_color: linear(fog_rgba),
            };
        }
    }

    for (part, mut transform, mut visibility, mut material) in parts.iter_mut() {
        if *part == SkyPart::EndSky {
            *visibility = if skybox == Skybox::End {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            continue;
        }
        if skybox != Skybox::Overworld {
            *visibility = Visibility::Hidden;
            continue;
        }
        if crate::renderer::terrain::builtin_shaders_enabled()
            && matches!(part, SkyPart::SkyDisc | SkyPart::DarkDisc)
        {
            *visibility = Visibility::Hidden;
            continue;
        }
        match part {
            SkyPart::SkyDisc => *visibility = Visibility::Inherited,
            SkyPart::DarkDisc => {
                *visibility = if eye_y < HORIZON_HEIGHT {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
            SkyPart::Sun => *transform = place(sky.sun_angle, SUN_HEIGHT, SUN_SIZE),
            SkyPart::Moon => {
                *transform = place(sky.moon_angle, MOON_HEIGHT, MOON_SIZE);
                if sky.moon_phase != assets.moon_phase {
                    assets.moon_phase = sky.moon_phase;
                    material.0 = assets.moon_materials[sky.moon_phase].clone();
                }
            }
            SkyPart::Stars => {
                let brightness = sky.star_brightness;
                *visibility = if brightness > 0.0 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                transform.rotation = yaw * Quat::from_rotation_x(sky.star_angle);
                if let Some(material) = materials.get_mut(&assets.star_material) {
                    material.base_color =
                        Color::srgba(brightness, brightness, brightness, brightness);
                }
            }
            SkyPart::SunriseFan => {
                *visibility = if sunrise_alpha > 0.001 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if sunrise_alpha > 0.001 {
                    let flip = if sky.sun_angle.sin() < 0.0 {
                        180.0f32
                    } else {
                        0.0
                    };
                    transform.rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)
                        * Quat::from_rotation_z((flip + 90.0).to_radians());
                    transform.scale = Vec3::new(1.0, 1.0, sunrise_alpha);
                    if let Some(material) = materials.get_mut(&assets.sunrise_material) {
                        material.base_color = sky.sunrise_color.to_color();
                    }
                }
            }
            SkyPart::EndSky => {}
        }
    }
}

fn rebuild_discs(assets: &mut SkyAssets, meshes: &mut Assets<Mesh>, sky: &SkyState, sky_end: f32) {
    let key = (sky.sky_color, sky.fog_color, sky_end);
    if assets.discs_built_for == Some(key) {
        return;
    }
    assets.discs_built_for = Some(key);

    if let Some(mesh) = meshes.get_mut(&assets.sky_disc_mesh) {
        *mesh = disc_mesh(SKY_DISC_HEIGHT, sky.sky_color, sky.fog_color, sky_end);
    }
    if let Some(mesh) = meshes.get_mut(&assets.dark_disc_mesh) {
        *mesh = disc_mesh(-SKY_DISC_HEIGHT, Argb(0xFF00_0000), sky.fog_color, sky_end);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_legacy_generator_matches_java() {
        let mut r = JavaRandom::new(0);
        assert!((r.next_f32() - 0.730_967_8).abs() < 1e-6);
        assert!((r.next_f32() - 0.831_441_0).abs() < 1e-6);
    }

    #[test]
    fn the_star_field_is_built() {
        let mesh = star_mesh();
        let count = mesh.count_vertices();
        assert!(count % 4 == 0);
        assert!(count / 4 > 600 && count / 4 < STAR_COUNT, "{count}");
    }

    #[test]
    fn the_disc_rim_is_fog_coloured() {
        let mesh = disc_mesh(16.0, Argb(0xFF00_00FF), Argb(0xFFFF_0000), 128.0);
        let colors = match mesh.attribute(Mesh::ATTRIBUTE_COLOR).unwrap() {
            bevy::mesh::VertexAttributeValues::Float32x4(v) => v.clone(),
            _ => panic!("colours are not 4-float"),
        };
        assert_eq!(*colors.last().unwrap(), [1.0, 0.0, 0.0, 1.0]);
        let expected = crate::util::mth::srgb_to_linear(0.875);
        assert!((colors[0][2] - expected).abs() < 1e-5, "{:?}", colors[0]);
    }
}
