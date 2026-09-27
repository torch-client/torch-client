use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use super::systems::BlockTileMap;
use super::systems::Shared;
use crate::renderer::{ATLAS_COLS, TILE_PX};
use crate::session::{BreakingBlock, TargetedBlock};

#[derive(Resource)]
pub(super) struct BlockOutline {
    pub(super) entity: Entity,
    pub(super) mesh: Handle<Mesh>,
    pub(super) shown: Option<TargetedBlock>,
}

#[derive(Component)]
pub(super) struct BlockOutlineMarker;

#[derive(Resource)]
pub struct BreakOverlay {
    pub(super) entity: Entity,
    pub(super) mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
    pub(super) shown: Option<BreakingBlock>,
}

#[derive(Component)]
pub(super) struct BreakOverlayMarker;

#[derive(Resource)]
pub(super) struct ChunkBorderOverlay {
    pub(super) entity: Entity,
    pub(super) mesh: Handle<Mesh>,
    pub(super) shown: Option<(i32, i32, i32, super::dimension::Dimension)>,
}

#[derive(Component)]
pub(super) struct ChunkBorderMarker;

#[cfg(feature = "click_gui")]
#[derive(Resource)]
pub(super) struct EspOverlay {
    pub(super) entity: Entity,
    pub(super) mesh: Handle<Mesh>,
    pub(super) shown: Option<std::sync::Arc<crate::modules::esp::EspFrame>>,
}

#[cfg(feature = "click_gui")]
#[derive(Component)]
pub(super) struct EspMarker;

pub(super) const OUTLINE_INFLATE: f32 = 0.002;

fn box_corners(min: [f32; 3], max: [f32; 3]) -> [[f32; 3]; 8] {
    let ([x0, y0, z0], [x1, y1, z1]) = (min, max);
    [
        [x0, y0, z0],
        [x1, y0, z0],
        [x1, y0, z1],
        [x0, y0, z1],
        [x0, y1, z0],
        [x1, y1, z0],
        [x1, y1, z1],
        [x0, y1, z1],
    ]
}

const BOX_EDGES: [(usize, usize); 12] = [
    (0, 1),
    (1, 2),
    (2, 3),
    (3, 0),
    (4, 5),
    (5, 6),
    (6, 7),
    (7, 4),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

fn line_mesh(positions: Vec<[f32; 3]>, colors: Option<Vec<[f32; 4]>>) -> Mesh {
    let n = positions.len();
    let mut mesh = Mesh::new(
        PrimitiveTopology::LineList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; n]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; n]);

    if let Some(colors) = colors {
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    }

    mesh
}

pub(super) fn chunk_border_mesh(ymin: f32, ymax: f32, sec_y0: f32) -> Mesh {
    fn srgba(r: f32, g: f32, b: f32, a: f32) -> [f32; 4] {
        use crate::util::mth::srgb_to_linear;
        [srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b), a]
    }
    let cell_border = srgba(0.0, 155.0 / 255.0, 155.0 / 255.0, 1.0);
    let yellow = srgba(1.0, 1.0, 0.0, 1.0);
    let major_lines = srgba(0.25, 0.25, 1.0, 1.0);
    let thick_red = srgba(1.0, 0.0, 0.0, 0.5);

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut line = |a: [f32; 3], b: [f32; 3], c: [f32; 4]| {
        positions.push(a);
        positions.push(b);
        colors.push(c);
        colors.push(c);
    };

    let mut x = -16.0;
    while x <= 32.0 {
        let mut z = -16.0;
        while z <= 32.0 {
            line([x, ymin, z], [x, ymax, z], thick_red);
            z += 16.0;
        }
        x += 16.0;
    }

    let mut x = 2.0;
    while x < 16.0 {
        let c = if x % 4.0 == 0.0 { cell_border } else { yellow };
        line([x, ymin, 0.0], [x, ymax, 0.0], c);
        line([x, ymin, 16.0], [x, ymax, 16.0], c);
        x += 2.0;
    }

    let mut z = 2.0;
    while z < 16.0 {
        let c = if z % 4.0 == 0.0 { cell_border } else { yellow };
        line([0.0, ymin, z], [0.0, ymax, z], c);
        line([16.0, ymin, z], [16.0, ymax, z], c);
        z += 2.0;
    }

    let mut y = ymin;
    while y <= ymax {
        let c = if (y - ymin) as i32 % 8 == 0 {
            cell_border
        } else {
            yellow
        };
        line([0.0, y, 0.0], [0.0, y, 16.0], c);
        line([0.0, y, 16.0], [16.0, y, 16.0], c);
        line([16.0, y, 16.0], [16.0, y, 0.0], c);
        line([16.0, y, 0.0], [0.0, y, 0.0], c);
        y += 2.0;
    }

    for &x in &[0.0, 16.0] {
        for &z in &[0.0, 16.0] {
            line([x, ymin, z], [x, ymax, z], major_lines);
        }
    }

    let mut y = ymin;
    while y <= ymax {
        line([0.0, y, 0.0], [0.0, y, 16.0], major_lines);
        line([0.0, y, 16.0], [16.0, y, 16.0], major_lines);
        line([16.0, y, 16.0], [16.0, y, 0.0], major_lines);
        line([16.0, y, 0.0], [0.0, y, 0.0], major_lines);
        y += 16.0;
    }

    let corners = box_corners([0.0, sec_y0, 0.0], [16.0, sec_y0 + 16.0, 16.0]);
    for (a, b) in BOX_EDGES {
        line(corners[a], corners[b], major_lines);
    }

    line_mesh(positions, Some(colors))
}

pub(super) fn outline_mesh(boxes: &[[f32; 6]]) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(boxes.len() * 24);
    for b in boxes {
        let e = OUTLINE_INFLATE;
        let c = box_corners(
            [b[0] - e, b[1] - e, b[2] - e],
            [b[3] + e, b[4] + e, b[5] + e],
        );
        for (a, b) in BOX_EDGES {
            positions.push(c[a]);
            positions.push(c[b]);
        }
    }

    line_mesh(positions, None)
}

pub(super) fn crumbling_mesh(boxes: &[[f32; 6]], tile: u32, atlas_rows: u32) -> Mesh {
    const EPS: f32 = 0.002;
    let col = tile % ATLAS_COLS;
    let row = tile / ATLAS_COLS;
    let tw = 1.0 / ATLAS_COLS as f32;
    let th = 1.0 / atlas_rows as f32;
    let iu = 0.02 / (ATLAS_COLS * TILE_PX) as f32;
    let iv = 0.02 / (atlas_rows * TILE_PX) as f32;
    let (u0, v0) = (col as f32 * tw + iu, row as f32 * th + iv);
    let (u1, v1) = ((col + 1) as f32 * tw - iu, (row + 1) as f32 * th - iv);
    let face_uv = [[u0, v1], [u1, v1], [u1, v0], [u0, v0]];

    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut nrm: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();

    for b in boxes {
        let (x0, y0, z0) = (b[0] - EPS, b[1] - EPS, b[2] - EPS);
        let (x1, y1, z1) = (b[3] + EPS, b[4] + EPS, b[5] + EPS);
        let faces: [([[f32; 3]; 4], [f32; 3]); 6] = [
            (
                [[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]],
                [0., 1., 0.],
            ),
            (
                [[x0, y0, z1], [x1, y0, z1], [x1, y0, z0], [x0, y0, z0]],
                [0., -1., 0.],
            ),
            (
                [[x1, y0, z0], [x1, y0, z1], [x1, y1, z1], [x1, y1, z0]],
                [1., 0., 0.],
            ),
            (
                [[x0, y0, z1], [x0, y0, z0], [x0, y1, z0], [x0, y1, z1]],
                [-1., 0., 0.],
            ),
            (
                [[x1, y0, z1], [x0, y0, z1], [x0, y1, z1], [x1, y1, z1]],
                [0., 0., 1.],
            ),
            (
                [[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0]],
                [0., 0., -1.],
            ),
        ];
        for (corners, normal) in faces {
            let base = pos.len() as u32;
            for i in 0..4 {
                pos.push(corners[i]);
                nrm.push(normal);
                uvs.push(face_uv[i]);
            }
            idx.extend_from_slice(&[base + 2, base + 1, base, base + 3, base + 2, base]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nrm);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

pub(super) fn update_break_overlay(
    shared: Res<Shared>,
    mut overlay: ResMut<BreakOverlay>,
    mut meshes: ResMut<Assets<Mesh>>,
    tile_map: Res<BlockTileMap>,
    mut targets: Query<(&mut Transform, &mut Visibility), With<BreakOverlayMarker>>,
) {
    const STAGE_TEX: [&str; 10] = [
        "destroy_stage_0",
        "destroy_stage_1",
        "destroy_stage_2",
        "destroy_stage_3",
        "destroy_stage_4",
        "destroy_stage_5",
        "destroy_stage_6",
        "destroy_stage_7",
        "destroy_stage_8",
        "destroy_stage_9",
    ];

    let breaking = shared.0.lock().unwrap().session.breaking.clone();
    if breaking == overlay.shown {
        return;
    }
    let Ok((mut transform, mut visibility)) = targets.get_mut(overlay.entity) else {
        return;
    };

    let tile = breaking
        .as_ref()
        .and_then(|b| tile_map.0.get(STAGE_TEX[b.stage.min(9) as usize]).copied());

    match (&breaking, tile) {
        (Some(b), Some(tile)) => {
            let rows = crate::ATLAS_ROWS.get().copied().unwrap_or(1);
            if let Some(mesh) = meshes.get_mut(&overlay.mesh) {
                *mesh = crumbling_mesh(&b.boxes, tile, rows);
            }
            transform.translation = Vec3::new(b.pos[0] as f32, b.pos[1] as f32, b.pos[2] as f32);
            *visibility = Visibility::Visible;
        }
        _ => *visibility = Visibility::Hidden,
    }
    overlay.shown = breaking;
}

pub(super) fn update_block_outline(
    shared: Res<Shared>,
    state: Res<crate::gui::GuiState>,
    mut outline: ResMut<BlockOutline>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut targets: Query<(&mut Transform, &mut Visibility), With<BlockOutlineMarker>>,
) {
    let target = if state.hide_gui {
        None
    } else {
        shared.0.lock().unwrap().session.targeted_block.clone()
    };
    if target == outline.shown {
        return;
    }
    let Ok((mut transform, mut visibility)) = targets.get_mut(outline.entity) else {
        return;
    };

    match &target {
        Some(t) => {
            let shape_changed = outline.shown.as_ref().map(|s| &s.boxes) != Some(&t.boxes);
            if shape_changed && let Some(mesh) = meshes.get_mut(&outline.mesh) {
                *mesh = outline_mesh(&t.boxes);
            }
            transform.translation = Vec3::new(t.pos[0] as f32, t.pos[1] as f32, t.pos[2] as f32);
            *visibility = Visibility::Visible;
        }
        None => *visibility = Visibility::Hidden,
    }
    outline.shown = target;
}

#[cfg(feature = "click_gui")]
struct Face {
    bit: u32,
    corners: [[f32; 3]; 4],
    borders: [(u32, u32); 4],
}

#[cfg(feature = "click_gui")]
const FACES: [Face; 6] = {
    use crate::modules::esp::scan::*;
    [
        Face {
            bit: TO,
            corners: [[0., 1., 0.], [1., 1., 0.], [0., 1., 1.], [1., 1., 1.]],
            borders: [(LE, TO_LE), (RI, TO_RI), (BA, TO_BA), (FO, TO_FO)],
        },
        Face {
            bit: BO,
            corners: [[0., 0., 0.], [1., 0., 0.], [0., 0., 1.], [1., 0., 1.]],
            borders: [(LE, BO_LE), (RI, BO_RI), (BA, BO_BA), (FO, BO_FO)],
        },
        Face {
            bit: FO,
            corners: [[0., 0., 1.], [1., 0., 1.], [0., 1., 1.], [1., 1., 1.]],
            borders: [(LE, FO_LE), (RI, FO_RI), (BO, BO_FO), (TO, TO_FO)],
        },
        Face {
            bit: BA,
            corners: [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [1., 1., 0.]],
            borders: [(LE, BA_LE), (RI, BA_RI), (BO, BO_BA), (TO, TO_BA)],
        },
        Face {
            bit: RI,
            corners: [[1., 0., 0.], [1., 0., 1.], [1., 1., 0.], [1., 1., 1.]],
            borders: [(BA, BA_RI), (FO, FO_RI), (BO, BO_RI), (TO, TO_RI)],
        },
        Face {
            bit: LE,
            corners: [[0., 0., 0.], [0., 0., 1.], [0., 1., 0.], [0., 1., 1.]],
            borders: [(BA, BA_LE), (FO, FO_LE), (BO, BO_LE), (TO, TO_LE)],
        },
    ]
};

#[cfg(feature = "click_gui")]
fn esp_mesh(frame: &crate::modules::esp::EspFrame) -> Mesh {
    use crate::renderer::esp_material::{ATTRIBUTE_COLOR, ATTRIBUTE_PACKED};
    use bevy::mesh::VertexAttributeValues;

    let guess = frame.boxes.len() * 3;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(guess * 4);
    let mut colors: Vec<[u8; 4]> = Vec::with_capacity(guess * 4);
    let mut packed: Vec<u32> = Vec::with_capacity(guess * 4);
    let mut indices: Vec<u32> = Vec::with_capacity(guess * 6);

    for b in &frame.boxes {
        let Some(slot) = frame.slots.get(b.slot as usize) else {
            continue;
        };
        let color = [
            (slot.rgb >> 16) as u8,
            (slot.rgb >> 8) as u8,
            slot.rgb as u8,
            slot.alpha,
        ];
        let i = OUTLINE_INFLATE;
        let (x, y, z) = (b.x as f32 - i, b.y as f32 - i, b.z as f32 - i);
        let size = 1.0 + 2.0 * i;

        for (f, face) in FACES.iter().enumerate() {
            if b.neigh & face.bit != 0 {
                continue;
            }
            let mut edges = 0u32;
            if slot.lines {
                for (e, (dir, diag)) in face.borders.iter().enumerate() {
                    if b.neigh & dir == 0 || b.neigh & diag != 0 {
                        edges |= 1 << e;
                    }
                }
            }
            let base = positions.len() as u32;
            for (c, corner) in face.corners.iter().enumerate() {
                positions.push([
                    x + corner[0] * size,
                    y + corner[1] * size,
                    z + corner[2] * size,
                ]);
                colors.push(color);
                packed.push(c as u32 | edges << 2 | (f as u32) << 6);
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
        }
    }

    let mut mesh = empty_esp_mesh();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(ATTRIBUTE_COLOR, VertexAttributeValues::Unorm8x4(colors));
    mesh.insert_attribute(ATTRIBUTE_PACKED, VertexAttributeValues::Uint32(packed));
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[cfg(feature = "click_gui")]
pub(super) fn empty_esp_mesh() -> Mesh {
    use crate::renderer::esp_material::{ATTRIBUTE_COLOR, ATTRIBUTE_PACKED};
    use bevy::mesh::VertexAttributeValues;

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(ATTRIBUTE_COLOR, VertexAttributeValues::Unorm8x4(Vec::new()));
    mesh.insert_attribute(ATTRIBUTE_PACKED, VertexAttributeValues::Uint32(Vec::new()));
    mesh.insert_indices(Indices::U32(Vec::new()));
    mesh
}

#[cfg(feature = "click_gui")]
pub(super) fn update_esp(
    shared: Res<Shared>,
    mut overlay: ResMut<EspOverlay>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut targets: Query<&mut Visibility, With<EspMarker>>,
) {
    let frame = shared.0.lock().unwrap().session.esp.clone();
    if overlay
        .shown
        .as_ref()
        .is_some_and(|shown| std::sync::Arc::ptr_eq(shown, &frame))
    {
        return;
    }
    let Ok(mut visibility) = targets.get_mut(overlay.entity) else {
        return;
    };
    if frame.boxes.is_empty() {
        *visibility = Visibility::Hidden;
    } else {
        if let Some(mesh) = meshes.get_mut(&overlay.mesh) {
            *mesh = esp_mesh(&frame);
        }
        *visibility = Visibility::Visible;
    }
    overlay.shown = Some(frame);
}

pub(super) fn update_chunk_borders(
    shared: Res<Shared>,
    state: Res<crate::gui::GuiState>,
    mut overlay: ResMut<ChunkBorderOverlay>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut targets: Query<(&mut Transform, &mut Visibility), With<ChunkBorderMarker>>,
) {
    let pos = state
        .chunk_borders
        .then(|| shared.0.lock().unwrap().session.player_pos);
    let dim = super::dimension::current();
    let key = pos.map(|p| {
        let (cx, cz) = (
            (p[0].floor() as i32).div_euclid(16),
            (p[2].floor() as i32).div_euclid(16),
        );
        let sec_y0 = (p[1].floor() as i32).div_euclid(16) * 16;
        (cx, cz, sec_y0, dim)
    });
    if key == overlay.shown {
        return;
    }
    let Ok((mut transform, mut visibility)) = targets.get_mut(overlay.entity) else {
        return;
    };

    match key {
        Some((cx, cz, sec_y0, dim)) => {
            let shape_changed = overlay.shown.map(|(_, _, sy, d)| (sy, d)) != Some((sec_y0, dim));
            if shape_changed && let Some(mesh) = meshes.get_mut(&overlay.mesh) {
                let (ymin, ymax) = dim.y_range();
                *mesh = chunk_border_mesh(ymin as f32, ymax as f32, sec_y0 as f32);
            }
            transform.translation = Vec3::new((cx * 16) as f32, 0.0, (cz * 16) as f32);
            *visibility = Visibility::Visible;
        }
        None => *visibility = Visibility::Hidden,
    }
    overlay.shown = key;
}
