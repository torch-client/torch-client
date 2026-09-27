use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Camera, ClearColorConfig, PerspectiveProjection, Projection};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::gui::GuiState;
use crate::renderer::AppState;
use crate::renderer::hand::HandCamera;
use crate::renderer::sky::SkyRoot;
use crate::renderer::systems::WorldCamera;

const PANORAMA_LAYER: usize = 3;

const PANORAMA_CAMERA_ORDER: isize = -1;

const FOV_DEGREES: f32 = 85.0;
const NEAR: f32 = 0.05;
const FAR: f32 = 10.0;

const PITCH_DEGREES: f32 = -10.0;

const SPIN_DEGREES_PER_SECOND: f32 = 20.0 * 0.1;

const HALF: f32 = 1.0;

#[derive(Component)]
pub struct PanoramaCamera;

static ABSENT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn absent() -> bool {
    ABSENT.load(std::sync::atomic::Ordering::Relaxed)
}

#[derive(Resource, Default)]
struct Spin(f32);

pub struct PanoramaPlugin;

impl Plugin for PanoramaPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Spin>()
            .add_systems(
                Update,
                setup.run_if(resource_added::<crate::renderer::systems::AssetsReady>),
            )
            .add_systems(Update, (spin, toggle_cameras));
    }
}

struct Face {
    file: &'static str,
    normal: Vec3,
    right: Vec3,
    up: Vec3,
}

const FACES: [Face; 6] = [
    Face {
        file: "panorama_0.png",
        normal: Vec3::NEG_Z,
        right: Vec3::X,
        up: Vec3::Y,
    },
    Face {
        file: "panorama_1.png",
        normal: Vec3::X,
        right: Vec3::Z,
        up: Vec3::Y,
    },
    Face {
        file: "panorama_2.png",
        normal: Vec3::Z,
        right: Vec3::NEG_X,
        up: Vec3::Y,
    },
    Face {
        file: "panorama_3.png",
        normal: Vec3::NEG_X,
        right: Vec3::NEG_Z,
        up: Vec3::Y,
    },
    Face {
        file: "panorama_4.png",
        normal: Vec3::Y,
        right: Vec3::X,
        up: Vec3::Z,
    },
    Face {
        file: "panorama_5.png",
        normal: Vec3::NEG_Y,
        right: Vec3::X,
        up: Vec3::NEG_Z,
    },
];

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let dir = crate::assets_root().join("textures/gui/title/background");

    for face in &FACES {
        let path = dir.join(face.file);
        let img = match crate::platform::assets::open_image(&path) {
            Ok(img) if img.width() <= 1 || img.height() <= 1 => {
                crate::log_info!(
                    "assets",
                    "the panorama is the jar's stub; the menu keeps its plain background"
                );
                ABSENT.store(true, std::sync::atomic::Ordering::Relaxed);
                return;
            }
            Ok(img) => img.to_rgba8(),
            Err(e) => {
                crate::log_warn!("assets", "panorama face {}: {e}", path.display());
                continue;
            }
        };
        let (w, h) = img.dimensions();
        let mut texture = Image::new(
            Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            img.into_raw(),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        texture.sampler = bevy::image::ImageSampler::linear();

        let material = materials.add(StandardMaterial {
            base_color_texture: Some(images.add(texture)),
            unlit: true,
            cull_mode: None,
            fog_enabled: false,
            ..default()
        });

        commands.spawn((
            Mesh3d(meshes.add(face_mesh(face))),
            MeshMaterial3d(material),
            Transform::default(),
            RenderLayers::layer(PANORAMA_LAYER),
            PanoramaLayerTag,
        ));
    }

    commands.spawn((
        super::systems::scene_camera(
            PANORAMA_CAMERA_ORDER,
            ClearColorConfig::Custom(Color::BLACK),
            super::systems::CameraStart::Inactive,
            PANORAMA_LAYER,
        ),
        Projection::Perspective(PerspectiveProjection {
            fov: FOV_DEGREES.to_radians(),
            near: NEAR,
            far: FAR,
            ..default()
        }),
        Transform::default(),
        PanoramaCamera,
    ));
}

#[derive(Component)]
struct PanoramaLayerTag;

fn face_mesh(face: &Face) -> Mesh {
    let center = face.normal * HALF;
    let (r, u) = (face.right * HALF, face.up * HALF);
    let corners = [
        center - r + u,
        center + r + u,
        center + r - u,
        center - r - u,
    ];
    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        corners.map(|c| [c.x, c.y, c.z]).to_vec(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs.to_vec());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![(-face.normal).to_array(); 4]);
    mesh.insert_indices(Indices::U32(vec![0, 2, 1, 0, 3, 2]));
    mesh
}

fn spin(
    time: Res<Time>,
    state: Res<GuiState>,
    app_state: Res<State<AppState>>,
    mut spin: ResMut<Spin>,
    mut camera: Query<&mut Transform, With<PanoramaCamera>>,
) {
    if !state.in_menu() && *app_state.get() == AppState::InGame {
        return;
    }
    spin.0 = (spin.0 + time.delta_secs() * SPIN_DEGREES_PER_SECOND).rem_euclid(360.0);
    if let Ok(mut t) = camera.single_mut() {
        t.rotation = Quat::from_rotation_y(-spin.0.to_radians())
            * Quat::from_rotation_x(PITCH_DEGREES.to_radians());
    }
}

fn toggle_cameras(
    state: Res<GuiState>,
    app_state: Res<State<AppState>>,
    mut panorama: Query<&mut Camera, (With<PanoramaCamera>, Without<WorldCamera>)>,
    mut hand: Query<
        &mut Camera,
        (
            With<HandCamera>,
            Without<PanoramaCamera>,
            Without<WorldCamera>,
        ),
    >,
    mut world: Query<&mut Camera, With<WorldCamera>>,
    mut sky: Query<&mut Visibility, With<SkyRoot>>,
) {
    let in_menu = state.in_menu() || *app_state.get() != AppState::InGame;

    let set = |camera: &mut Camera, active: bool| {
        if camera.is_active != active {
            camera.is_active = active;
        }
    };
    if let Ok(mut c) = panorama.single_mut() {
        set(&mut c, in_menu);
    }
    if let Ok(mut c) = hand.single_mut() {
        set(&mut c, !in_menu);
    }
    if let Ok(mut c) = world.single_mut() {
        set(&mut c, !in_menu);
    }
    if let Ok(mut visibility) = sky.single_mut() {
        let want = if in_menu {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *visibility != want {
            *visibility = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_form_a_cube() {
        for face in &FACES {
            assert!((face.normal.length() - 1.0).abs() < 1e-6);
            assert!(face.normal.dot(face.right).abs() < 1e-6);
            assert!(face.normal.dot(face.up).abs() < 1e-6);
            assert!(face.right.dot(face.up).abs() < 1e-6);
        }
        for (i, a) in FACES.iter().enumerate() {
            for b in FACES.iter().skip(i + 1) {
                assert!(a.normal.distance(b.normal) > 1e-6);
            }
        }
    }

    #[test]
    fn the_side_faces_stand_upright() {
        for face in FACES.iter().take(4) {
            assert_eq!(face.up, Vec3::Y);
        }
    }
}
