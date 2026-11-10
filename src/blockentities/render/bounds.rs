use bevy::camera::visibility::NoFrustumCulling;
use bevy::math::DVec3;
use bevy::prelude::*;

use crate::blockentities::feed::BlockEntityData;
use crate::blockentities::sim::bounds::{Data, Mode};
use crate::renderer::RenderOrigin;
use crate::renderer::overlays::{LineMaterial, box_faces_mesh, line_mesh, outline_edges};
use crate::session::Gamemode;

const VIEW_DISTANCE: f64 = 96.0;
const TEST_VIEW_DISTANCE: f64 = 256.0;
const ERROR_PADDING: f32 = 0.02;
const ERROR_FILL: [f32; 4] = [1.0, 0.0, 0.0, 0.375];
const GAME_MASTER_LEVEL: u8 = 2;

pub fn plugin(app: &mut App) {
    app.init_resource::<BoundsOverlay>()
        .add_systems(
            Startup,
            spawn.after(crate::renderer::systems::RendererSetup),
        )
        .add_systems(
            Update,
            update
                .after(crate::renderer::FrameViewSystems)
                .run_if(in_state(crate::renderer::AppState::InGame)),
        )
        .add_systems(OnExit(crate::renderer::AppState::InGame), clear);
}

#[derive(Component)]
struct BoundsLines;

#[derive(Component)]
struct BoundsFill;

#[derive(Resource, Default)]
struct BoundsOverlay {
    version: u64,
    entries: Vec<([i32; 3], Data)>,
    shown: Option<(bool, Vec<usize>, RenderOrigin)>,
    scratch: Vec<usize>,
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    lines: Res<LineMaterial>,
) {
    let fill = materials.add(StandardMaterial {
        base_color: Color::srgba(ERROR_FILL[0], ERROR_FILL[1], ERROR_FILL[2], ERROR_FILL[3]),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(line_mesh(Vec::new(), Some(Vec::new())))),
        MeshMaterial3d(lines.0.clone()),
        Transform::default(),
        Visibility::Hidden,
        NoFrustumCulling,
        BoundsLines,
    ));
    commands.spawn((
        Mesh3d(meshes.add(fill_mesh(&[]))),
        MeshMaterial3d(fill),
        Transform::default(),
        Visibility::Hidden,
        NoFrustumCulling,
        BoundsFill,
    ));
}

fn clear(
    mut overlay: ResMut<BoundsOverlay>,
    mut targets: Query<&mut Visibility, Or<(With<BoundsLines>, With<BoundsFill>)>>,
) {
    overlay.shown = None;
    for mut visibility in &mut targets {
        *visibility = Visibility::Hidden;
    }
}

fn visible(gamemode: Gamemode, op_level: u8) -> bool {
    gamemode == Gamemode::Spectator
        || (gamemode == Gamemode::Creative && op_level >= GAME_MASTER_LEVEL)
}

fn center_distance(pos: [i32; 3], camera: DVec3) -> (f64, f64) {
    let d = crate::util::block_centre(pos) - camera;
    (d.x.hypot(d.z), d.length())
}

fn in_range(pos: [i32; 3], data: &Data, camera: DVec3) -> bool {
    let (flat, full) = center_distance(pos, camera);
    full < VIEW_DISTANCE || (data.test_instance && flat < TEST_VIEW_DISTANCE)
}

fn update(
    view: Res<crate::renderer::FrameView>,
    mut overlay: ResMut<BoundsOverlay>,
    mut meshes: ResMut<Assets<Mesh>>,
    origin: Res<RenderOrigin>,
    mut lines: Query<(&Mesh3d, &mut Visibility), (With<BoundsLines>, Without<BoundsFill>)>,
    mut fills: Query<(&Mesh3d, &mut Visibility), (With<BoundsFill>, Without<BoundsLines>)>,
) {
    let camera = view.camera_world;
    let origin = *origin;
    let show = visible(view.gamemode, view.op_level);
    if view.block_entities_version != overlay.version {
        overlay.version = view.block_entities_version;
        let mut entries: Vec<([i32; 3], Data)> = view
            .block_entities
            .iter()
            .filter_map(|(pos, info)| match &*info.data {
                BlockEntityData::Bounds(data) => Some((*pos, data.clone())),
                _ => None,
            })
            .collect();
        entries.sort_unstable_by_key(|(pos, _)| *pos);
        if entries != overlay.entries {
            overlay.entries = entries;
            overlay.shown = None;
        }
    }

    let overlay = &mut *overlay;
    overlay.scratch.clear();
    for (i, (pos, data)) in overlay.entries.iter().enumerate() {
        if in_range(*pos, data, camera) {
            overlay.scratch.push(i);
        }
    }
    if overlay
        .shown
        .as_ref()
        .is_some_and(|(s, idx, o)| *s == show && *idx == overlay.scratch && *o == origin)
    {
        return;
    }

    let mut edges = Vec::new();
    let mut fills_boxes = Vec::new();
    for &i in &overlay.scratch {
        let (pos, data) = &overlay.entries[i];
        if show && data.mode != Mode::None && data.size.iter().all(|s| *s >= 1) {
            let min = std::array::from_fn::<_, 3, _>(|a| pos[a] + data.local[a]);
            let max = std::array::from_fn::<_, 3, _>(|a| min[a] + data.size[a]);
            outline_edges([block_box(min, max, origin)], &mut edges);
        }
        fills_boxes.extend(
            data.errors
                .iter()
                .map(|b| block_box(*b, b.map(|v| v + 1), origin)),
        );
    }

    if let Ok((mesh, mut visibility)) = lines.single_mut() {
        *visibility = if edges.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
        if !edges.is_empty()
            && let Some(mut slot) = meshes.get_mut(&mesh.0)
        {
            let colors = vec![[0.9, 0.9, 0.9, 1.0]; edges.len()];
            *slot = line_mesh(edges, Some(colors));
        }
    }
    if let Ok((mesh, mut visibility)) = fills.single_mut() {
        *visibility = if fills_boxes.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
        if !fills_boxes.is_empty()
            && let Some(mut slot) = meshes.get_mut(&mesh.0)
        {
            *slot = fill_mesh(&fills_boxes);
        }
    }
    overlay.shown = Some((show, std::mem::take(&mut overlay.scratch), origin));
}

fn block_box(min: [i32; 3], max: [i32; 3], origin: RenderOrigin) -> [f32; 6] {
    let [x0, y0, z0] = origin.block_to_render(min).to_array();
    let [x1, y1, z1] = origin.block_to_render(max).to_array();
    [x0, y0, z0, x1, y1, z1]
}

fn fill_mesh(boxes: &[[f32; 6]]) -> Mesh {
    box_faces_mesh(boxes, ERROR_PADDING, [[0.0; 2]; 4])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creative_needs_game_master_permission() {
        assert!(!visible(Gamemode::Creative, 0));
        assert!(visible(Gamemode::Creative, 2));
        assert!(visible(Gamemode::Spectator, 0));
        assert!(!visible(Gamemode::Survival, 4));
    }

    #[test]
    fn fill_mesh_has_a_cuboid_per_block() {
        let mesh = fill_mesh(&[
            block_box([0, 0, 0], [1, 1, 1], RenderOrigin::default()),
            block_box([3, 3, 3], [4, 4, 4], RenderOrigin::default()),
        ]);
        assert_eq!(mesh.count_vertices(), 48);
    }
}
