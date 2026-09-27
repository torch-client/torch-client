use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Camera, ClearColorConfig, OrthographicProjection, Projection, ScalingMode};
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::systems::Shared;
use super::timeline;
use super::{AppState, Gamemode};

const OVERLAY_LAYER: usize = 5;

const OVERLAY_CAMERA_ORDER: isize = 2;

const TILE_COUNT: f32 = 4.0;
const SCROLL_DIVISOR: f32 = 64.0;
const OVERLAY_ALPHA: f32 = 0.1;

#[derive(Component)]
pub struct OverlayCamera;

#[derive(Component)]
struct OverlayQuad;

#[derive(Resource)]
struct OverlayAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

pub struct WaterOverlayPlugin;

impl Plugin for WaterOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, update_overlay.run_if(in_state(AppState::InGame)));
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let path = crate::textures_dir().join("misc/underwater.png");
    let (w, h, data) = match crate::platform::assets::open_image(&path) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            (rgba.width(), rgba.height(), rgba.into_raw())
        }
        Err(e) => {
            eprintln!("water_overlay: {} could not be read ({e})", path.display());
            (1, 1, vec![0u8; 4])
        }
    };
    let mut texture = Image::new(
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
    texture.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::nearest()
    });

    let material = materials.add(StandardMaterial {
        base_color_texture: Some(images.add(texture)),
        base_color: Color::srgba(1.0, 1.0, 1.0, OVERLAY_ALPHA),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        double_sided: true,
        fog_enabled: false,
        ..default()
    });

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [-1.0, -1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, 1.0, -1.0],
            [-1.0, 1.0, -1.0],
        ],
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; 4]);
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![
            [TILE_COUNT, TILE_COUNT],
            [0.0, TILE_COUNT],
            [0.0, 0.0],
            [TILE_COUNT, 0.0],
        ],
    );
    mesh.insert_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
    let mesh = meshes.add(mesh);

    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(material.clone()),
        Transform::IDENTITY,
        Visibility::Hidden,
        RenderLayers::layer(OVERLAY_LAYER),
        OverlayQuad,
    ));

    commands.spawn((
        super::systems::scene_camera(
            OVERLAY_CAMERA_ORDER,
            ClearColorConfig::None,
            super::systems::CameraStart::Inactive,
            OVERLAY_LAYER,
        ),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: 2.0,
                height: 2.0,
            },
            near: 0.1,
            far: 10.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::IDENTITY,
        OverlayCamera,
    ));

    commands.insert_resource(OverlayAssets { mesh, material });
}

fn update_overlay(
    shared: Res<Shared>,
    assets: Res<OverlayAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut quad: Query<&mut Visibility, With<OverlayQuad>>,
    mut camera: Query<&mut Camera, With<OverlayCamera>>,
    mut last_br: Local<Option<f32>>,
) {
    crate::prof_span!("render:water_overlay");
    let Ok(mut visibility) = quad.single_mut() else {
        return;
    };

    let mut set_active = |on: bool| {
        if let Ok(mut c) = camera.single_mut()
            && c.is_active != on
        {
            c.is_active = on;
        }
    };

    let (eye_x, eye_y, eye_z, yaw, pitch, ticks) = {
        let Ok(state) = shared.0.lock() else { return };
        if state.session.gamemode == Gamemode::Spectator {
            *visibility = Visibility::Hidden;
            set_active(false);
            return;
        }
        let partial = super::systems::partial_ticks(&state);
        let eye_height = super::systems::session_eye_height(&state.session) as f64;
        (
            state.session.player_pos[0] as f64,
            state.session.player_pos[1] as f64 + eye_height,
            state.session.player_pos[2] as f64,
            state.camera_yaw,
            state.camera_pitch,
            state.session.day_clock.at(partial),
        )
    };

    let in_water = crate::util::block_model::eye_fluid_at(eye_x, eye_y, eye_z)
        == Some(crate::util::block_model::EyeFluid::Water);
    if !in_water {
        *visibility = Visibility::Hidden;
        set_active(false);
        return;
    }
    *visibility = Visibility::Visible;
    set_active(true);

    let uo = yaw / SCROLL_DIVISOR;
    let vo = pitch / SCROLL_DIVISOR;
    if let Some(mesh) = meshes.get_mut(&assets.mesh) {
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_0,
            vec![
                [TILE_COUNT + uo, TILE_COUNT + vo],
                [0.0 + uo, TILE_COUNT + vo],
                [0.0 + uo, 0.0 + vo],
                [TILE_COUNT + uo, 0.0 + vo],
            ],
        );
    }

    let br = timeline::sample(ticks).sky_light_factor.clamp(0.0, 1.0);
    if *last_br != Some(br)
        && let Some(material) = materials.get_mut(&assets.material)
    {
        material.base_color = Color::srgba(br, br, br, OVERLAY_ALPHA);
        *last_br = Some(br);
    }
}
