use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::transform::TransformSystems;

use super::AppState;
use super::systems::WorldCamera;
use super::timeline::{self, CLOUD_HEIGHT};
use crate::direction::Direction;
use crate::gui::GuiState;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CloudStatus {
    Off,
    Fast,
    #[default]
    Fancy,
}

impl CloudStatus {
    pub fn caption(self) -> &'static str {
        match self {
            CloudStatus::Off => "OFF",
            CloudStatus::Fast => "Fast",
            CloudStatus::Fancy => "Fancy",
        }
    }

    pub fn serialized_name(self) -> &'static str {
        match self {
            CloudStatus::Off => "false",
            CloudStatus::Fast => "fast",
            CloudStatus::Fancy => "true",
        }
    }

    pub fn from_serialized_name(name: &str) -> Option<CloudStatus> {
        match name {
            "false" => Some(CloudStatus::Off),
            "fast" => Some(CloudStatus::Fast),
            "true" => Some(CloudStatus::Fancy),
            _ => None,
        }
    }

    pub fn next(self) -> CloudStatus {
        match self {
            CloudStatus::Off => CloudStatus::Fast,
            CloudStatus::Fast => CloudStatus::Fancy,
            CloudStatus::Fancy => CloudStatus::Off,
        }
    }
}

const CELL_SIZE: f32 = 12.0;
const CELL_HEIGHT: f32 = 4.0;
const TICKS_PER_CELL: i64 = 400;
const DRIFT_PER_TICK: f64 = 0.030_000_001;
const Z_OFFSET: f64 = 3.960_000_038_146_972_7;
const EMPTY_ALPHA: u8 = 10;

const CLOUD_RANGE_CHUNKS: i32 = 32;

const CLOUD_FOG_END: f32 = (CLOUD_RANGE_CHUNKS * 16) as f32;

const FACE_SHADE: [f32; 6] = [0.7, 1.0, 0.8, 0.8, 0.9, 0.9];

const FACE_VERTICES: [[[f32; 3]; 4]; 6] = [
    [
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0],
    ],
    [
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 1.0],
        [1.0, 1.0, 1.0],
        [1.0, 1.0, 0.0],
    ],
    [
        [0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
    ],
    [
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
        [0.0, 0.0, 1.0],
    ],
    [
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 1.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0],
    ],
    [
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [1.0, 1.0, 1.0],
        [1.0, 0.0, 1.0],
    ],
];

const DOWN: usize = Direction::Down.index();
const UP: usize = Direction::Up.index();
const NORTH: usize = Direction::North.index();
const SOUTH: usize = Direction::South.index();
const WEST: usize = Direction::West.index();
const EAST: usize = Direction::East.index();

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RelativeCameraPos {
    AboveClouds,
    InsideClouds,
    BelowClouds,
}

pub struct CloudPlugin;

impl Plugin for CloudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            setup.run_if(resource_added::<crate::renderer::systems::AssetsReady>),
        )
        .add_systems(
            PostUpdate,
            update_clouds
                .before(TransformSystems::Propagate)
                .run_if(in_state(AppState::InGame))
                .run_if(resource_exists::<CloudAssets>),
        );
    }
}

#[derive(Component)]
struct CloudLayer;

struct CloudTexture {
    width: usize,
    height: usize,
    cells: Vec<u8>,
}

const NORTH_OPEN: u8 = 1 << 3;
const EAST_OPEN: u8 = 1 << 2;
const SOUTH_OPEN: u8 = 1 << 1;
const WEST_OPEN: u8 = 1 << 0;
const SOLID: u8 = 1 << 4;

#[derive(Resource)]
struct CloudAssets {
    texture: Option<CloudTexture>,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    built: Option<(i32, i32, RelativeCameraPos, CloudStatus)>,
    painted: Option<(timeline::Argb, CloudStatus)>,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let path = crate::textures_dir().join("environment/clouds.png");
    let texture = match crate::platform::assets::open_image(&path) {
        Ok(img) => Some(CloudTexture::new(&img.to_rgba8())),
        Err(e) => {
            eprintln!("clouds: {} could not be read ({e})", path.display());
            None
        }
    };

    let mesh = meshes.add(empty_mesh());
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: Some(bevy::render::render_resource::Face::Back),
        double_sided: false,
        fog_enabled: false,
        ..default()
    });

    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(material.clone()),
        Transform::from_xyz(0.0, CLOUD_HEIGHT, 0.0),
        Visibility::Hidden,
        CloudLayer,
    ));

    commands.insert_resource(CloudAssets {
        texture,
        mesh,
        material,
        built: None,
        painted: None,
    });
}

impl CloudTexture {
    fn new(image: &image::RgbaImage) -> CloudTexture {
        let (width, height) = (image.width() as usize, image.height() as usize);
        let empty = |x: usize, y: usize| image.get_pixel(x as u32, y as u32).0[3] < EMPTY_ALPHA;
        let mut cells = vec![0u8; width * height];
        for y in 0..height {
            for x in 0..width {
                if empty(x, y) {
                    continue;
                }
                let mut flags = SOLID;
                if empty(x, (y + height - 1) % height) {
                    flags |= NORTH_OPEN;
                }
                if empty((x + 1) % width, y) {
                    flags |= EAST_OPEN;
                }
                if empty(x, (y + 1) % height) {
                    flags |= SOUTH_OPEN;
                }
                if empty((x + width - 1) % width, y) {
                    flags |= WEST_OPEN;
                }
                cells[x + y * width] = flags;
            }
        }
        CloudTexture {
            width,
            height,
            cells,
        }
    }

    fn cell(&self, x: i32, y: i32) -> u8 {
        let ix = x.rem_euclid(self.width as i32) as usize;
        let iy = y.rem_euclid(self.height as i32) as usize;
        self.cells[ix + iy * self.width]
    }
}

fn empty_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
    mesh.insert_indices(Indices::U32(Vec::new()));
    mesh
}

struct MeshBuilder {
    positions: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl MeshBuilder {
    fn face(&mut self, x: i32, z: i32, direction: usize, inside: bool, top_color: bool) {
        let base = self.positions.len() as u32;
        let shade =
            crate::util::mth::srgb_to_linear(FACE_SHADE[if top_color { UP } else { direction }]);
        for corner in FACE_VERTICES[direction] {
            self.positions.push([
                (corner[0] + x as f32) * CELL_SIZE,
                corner[1] * CELL_HEIGHT,
                (corner[2] + z as f32) * CELL_SIZE,
            ]);
            self.colors.push([shade, shade, shade, 1.0]);
        }
        if inside {
            self.indices.extend_from_slice(&[
                base + 3,
                base + 2,
                base + 1,
                base + 3,
                base + 1,
                base,
            ]);
        } else {
            self.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
}

fn build_mesh(
    texture: &CloudTexture,
    center_x: i32,
    center_z: i32,
    relative: RelativeCameraPos,
    status: CloudStatus,
) -> Mesh {
    let radius_cells = (CLOUD_RANGE_CHUNKS * 16) as f32 / CELL_SIZE;
    let radius_cells = radius_cells.ceil() as i32;
    let extrude = status == CloudStatus::Fancy;

    let mut b = MeshBuilder {
        positions: Vec::new(),
        colors: Vec::new(),
        indices: Vec::new(),
    };
    for ring in 0..=2 * radius_cells {
        for rel_x in -ring..=ring {
            let rel_z = ring - rel_x.abs();
            if rel_z < 0 || rel_z > radius_cells {
                continue;
            }
            if rel_x * rel_x + rel_z * rel_z > radius_cells * radius_cells {
                continue;
            }
            if rel_z != 0 {
                try_cell(
                    &mut b, texture, center_x, center_z, relative, extrude, rel_x, -rel_z,
                );
            }
            try_cell(
                &mut b, texture, center_x, center_z, relative, extrude, rel_x, rel_z,
            );
        }
    }

    for (position, color) in b.positions.iter().zip(b.colors.iter_mut()) {
        let distance = (position[0] * position[0] + position[2] * position[2]).sqrt();
        color[3] = 1.0 - (distance / CLOUD_FOG_END).clamp(0.0, 1.0);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    let normals = vec![[0.0, 1.0, 0.0]; b.positions.len()];
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, b.positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, b.colors);
    mesh.insert_indices(Indices::U32(b.indices));
    mesh
}

#[allow(clippy::too_many_arguments)]
fn try_cell(
    b: &mut MeshBuilder,
    texture: &CloudTexture,
    center_x: i32,
    center_z: i32,
    relative: RelativeCameraPos,
    extrude: bool,
    x: i32,
    z: i32,
) {
    let cell = texture.cell(center_x + x, center_z + z);
    if cell == 0 {
        return;
    }
    if !extrude {
        b.face(x, z, DOWN, false, true);
        return;
    }

    if relative != RelativeCameraPos::BelowClouds {
        b.face(x, z, UP, false, false);
    }
    if relative != RelativeCameraPos::AboveClouds {
        b.face(x, z, DOWN, false, false);
    }
    if cell & NORTH_OPEN != 0 && z > 0 {
        b.face(x, z, NORTH, false, false);
    }
    if cell & SOUTH_OPEN != 0 && z < 0 {
        b.face(x, z, SOUTH, false, false);
    }
    if cell & WEST_OPEN != 0 && x > 0 {
        b.face(x, z, WEST, false, false);
    }
    if cell & EAST_OPEN != 0 && x < 0 {
        b.face(x, z, EAST, false, false);
    }
    if relative == RelativeCameraPos::InsideClouds && x.abs() <= 1 && z.abs() <= 1 {
        for direction in 0..6 {
            b.face(x, z, direction, true, false);
        }
    }
}

fn update_clouds(
    gui: Res<GuiState>,
    shared: Res<super::systems::Shared>,
    env: Res<super::environment::Environment>,
    mut assets: ResMut<CloudAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera: Query<&Transform, With<WorldCamera>>,
    mut layer: Query<(&mut Transform, &mut Visibility), (With<CloudLayer>, Without<WorldCamera>)>,
) {
    crate::prof_span!("render:update_clouds");
    let Ok((mut transform, mut visibility)) = layer.single_mut() else {
        return;
    };
    let status = gui.options.clouds;

    let partial = {
        let state = shared.0.lock().unwrap();
        super::systems::partial_ticks(&state)
    };
    let color = env.sky.cloud_color;

    let Some((texture_cells_w, texture_cells_h)) =
        assets.texture.as_ref().map(|t| (t.width, t.height))
    else {
        *visibility = Visibility::Hidden;
        return;
    };
    if status == CloudStatus::Off || color.alpha() == 0 {
        *visibility = Visibility::Hidden;
        return;
    }
    *visibility = Visibility::Visible;

    let Ok(camera) = camera.single() else { return };
    let (cam_x, cam_y, cam_z) = (
        camera.translation.x as f64,
        camera.translation.y as f64,
        camera.translation.z as f64,
    );

    let relative_bottom = CLOUD_HEIGHT as f64 - cam_y;
    let relative_top = relative_bottom + CELL_HEIGHT as f64;
    let relative = if relative_top < 0.0 {
        RelativeCameraPos::AboveClouds
    } else if relative_bottom > 0.0 {
        RelativeCameraPos::BelowClouds
    } else {
        RelativeCameraPos::InsideClouds
    };

    let period = texture_cells_w as i64 * TICKS_PER_CELL;
    let offset = crate::client::tracking::game_time().rem_euclid(period) as f64 + partial as f64;
    let texture_w = texture_cells_w as f64 * CELL_SIZE as f64;
    let texture_h = texture_cells_h as f64 * CELL_SIZE as f64;
    let mut cloud_x = cam_x + offset * DRIFT_PER_TICK;
    let mut cloud_z = cam_z + Z_OFFSET;
    cloud_x -= (cloud_x / texture_w).floor() * texture_w;
    cloud_z -= (cloud_z / texture_h).floor() * texture_h;
    let cell_x = (cloud_x / CELL_SIZE as f64).floor() as i32;
    let cell_z = (cloud_z / CELL_SIZE as f64).floor() as i32;
    let x_in_cell = cloud_x - cell_x as f64 * CELL_SIZE as f64;
    let z_in_cell = cloud_z - cell_z as f64 * CELL_SIZE as f64;

    transform.translation = Vec3::new(
        (cam_x - x_in_cell) as f32,
        CLOUD_HEIGHT,
        (cam_z - z_in_cell) as f32,
    );

    let key = (cell_x, cell_z, relative, status);
    if assets.built != Some(key) {
        assets.built = Some(key);
        let rebuilt = match assets.texture.as_ref() {
            Some(texture) => build_mesh(texture, cell_x, cell_z, relative, status),
            None => return,
        };
        if let Some(mesh) = meshes.get_mut(&assets.mesh) {
            *mesh = rebuilt;
        }
    }

    if assets.painted != Some((color, status)) {
        assets.painted = Some((color, status));
        if let Some(material) = materials.get_mut(&assets.material) {
            material.base_color = color.to_color();
            material.cull_mode = match status {
                CloudStatus::Fancy => Some(bevy::render::render_resource::Face::Back),
                _ => None,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkerboard() -> CloudTexture {
        let mut image = image::RgbaImage::new(4, 4);
        image.put_pixel(1, 1, image::Rgba([255, 255, 255, 255]));
        CloudTexture::new(&image)
    }

    #[test]
    fn neighbours_are_packed_per_cell() {
        let t = checkerboard();
        assert_eq!(
            t.cell(1, 1),
            SOLID | NORTH_OPEN | EAST_OPEN | SOUTH_OPEN | WEST_OPEN
        );
        assert_eq!(t.cell(0, 0), 0);
        assert_eq!(t.cell(5, 5), t.cell(1, 1));
    }

    #[test]
    fn fast_clouds_are_one_quad_per_cell() {
        let t = checkerboard();
        let fast = build_mesh(&t, 0, 0, RelativeCameraPos::BelowClouds, CloudStatus::Fast);
        let fancy = build_mesh(&t, 0, 0, RelativeCameraPos::BelowClouds, CloudStatus::Fancy);
        assert!(fast.count_vertices() > 0);
        assert!(fancy.count_vertices() > fast.count_vertices());
        assert_eq!(fast.count_vertices() % 4, 0);
    }

    #[test]
    fn the_far_side_of_the_layer_is_not_built() {
        let t = checkerboard();
        let below = build_mesh(&t, 0, 0, RelativeCameraPos::BelowClouds, CloudStatus::Fancy);
        let inside = build_mesh(
            &t,
            0,
            0,
            RelativeCameraPos::InsideClouds,
            CloudStatus::Fancy,
        );
        assert!(inside.count_vertices() > below.count_vertices());
    }

    #[test]
    fn the_fade_reaches_zero_at_the_rim() {
        let t = checkerboard();
        let mesh = build_mesh(&t, 0, 0, RelativeCameraPos::BelowClouds, CloudStatus::Fast);
        let colors = match mesh.attribute(Mesh::ATTRIBUTE_COLOR).unwrap() {
            bevy::mesh::VertexAttributeValues::Float32x4(v) => v.clone(),
            _ => panic!("colours are not 4-float"),
        };
        let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap() {
            bevy::mesh::VertexAttributeValues::Float32x3(v) => v.clone(),
            _ => panic!("positions are not 3-float"),
        };
        for (p, c) in positions.iter().zip(colors.iter()) {
            let d = (p[0] * p[0] + p[2] * p[2]).sqrt();
            assert!(c[3] >= 0.0 && c[3] <= 1.0);
            if d >= CLOUD_FOG_END {
                assert_eq!(c[3], 0.0);
            }
        }
    }

    #[test]
    fn the_option_cycles_and_round_trips() {
        let mut status = CloudStatus::Off;
        for expected in [CloudStatus::Fast, CloudStatus::Fancy, CloudStatus::Off] {
            status = status.next();
            assert_eq!(status, expected);
        }
        for status in [CloudStatus::Off, CloudStatus::Fast, CloudStatus::Fancy] {
            assert_eq!(
                CloudStatus::from_serialized_name(status.serialized_name()),
                Some(status)
            );
        }
    }
}
