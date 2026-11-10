use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use super::frame_view::FrameView;
use crate::renderer::RenderOrigin;
use crate::session::{BreakingBlock, TargetedBlock};

#[derive(Resource)]
pub(super) struct BlockOutline {
    pub(super) entity: Entity,
    pub(super) mesh: Handle<Mesh>,
    pub(super) shown: Option<TargetedBlock>,
}

#[derive(Component)]
pub(crate) struct BlockOutlineMarker;

#[derive(Resource)]
pub struct BreakOverlay {
    pub(super) pool: Vec<(Entity, Handle<Mesh>)>,
    pub material: Handle<StandardMaterial>,
    pub(super) shown: Option<Vec<BreakingBlock>>,
}

impl BreakOverlay {
    pub(crate) fn forget_shown(&mut self) {
        self.shown = None;
    }
}

#[derive(Component)]
pub(crate) struct BreakOverlayMarker;

#[derive(Resource)]
pub(super) struct ChunkBorderOverlay {
    pub(super) entity: Entity,
    pub(super) mesh: Handle<Mesh>,
    pub(super) shown: Option<(i32, i32, i32, super::dimension::Dimension)>,
}

#[derive(Component)]
pub(super) struct ChunkBorderMarker;

#[derive(Resource, Clone)]
pub(crate) struct LineMaterial(pub(crate) Handle<StandardMaterial>);

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

pub(crate) fn line_mesh(positions: Vec<[f32; 3]>, colors: Option<Vec<[f32; 4]>>) -> Mesh {
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
    } else if cfg!(feature = "shader_support") {
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32, 1.0, 1.0, 1.0]; n]);
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
    let mut positions = Vec::with_capacity(boxes.len() * 24);
    outline_edges(boxes.iter().copied(), &mut positions);
    line_mesh(positions, None)
}

pub(crate) fn outline_edges(boxes: impl IntoIterator<Item = [f32; 6]>, out: &mut Vec<[f32; 3]>) {
    let boxes: Vec<[f32; 6]> = boxes.into_iter().collect();
    ShapeGrid::new(&boxes).edges(OUTLINE_INFLATE, out);
}

struct ShapeGrid {
    coords: [Vec<f32>; 3],
    full: Vec<bool>,
}

const FACE_DIRS: [(usize, isize); 6] = [(1, 1), (1, -1), (0, 1), (0, -1), (2, 1), (2, -1)];

impl ShapeGrid {
    fn new(boxes: &[[f32; 6]]) -> Self {
        let coords: [Vec<f32>; 3] = std::array::from_fn(|axis| {
            let mut v: Vec<f32> = boxes.iter().flat_map(|b| [b[axis], b[axis + 3]]).collect();
            v.sort_by(f32::total_cmp);
            v.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
            v
        });
        let mut grid = Self {
            coords,
            full: Vec::new(),
        };
        let [nx, ny, nz] = [0, 1, 2].map(|a| grid.size(a));
        grid.full = vec![false; nx * ny * nz];
        for x in 0..nx {
            for y in 0..ny {
                for z in 0..nz {
                    let c = [x, y, z];
                    let mid: [f32; 3] = std::array::from_fn(|a| {
                        (grid.coords[a][c[a]] + grid.coords[a][c[a] + 1]) * 0.5
                    });
                    grid.full[(x * ny + y) * nz + z] = boxes
                        .iter()
                        .any(|b| (0..3).all(|a| b[a] < mid[a] && mid[a] < b[a + 3]));
                }
            }
        }
        grid
    }

    fn size(&self, axis: usize) -> usize {
        self.coords[axis].len().saturating_sub(1)
    }

    fn is_full(&self, cell: [isize; 3]) -> bool {
        let [nx, ny, nz] = [0, 1, 2].map(|a| self.size(a) as isize);
        let [x, y, z] = cell;
        (0..nx).contains(&x)
            && (0..ny).contains(&y)
            && (0..nz).contains(&z)
            && self.full[((x * ny + y) * nz + z) as usize]
    }

    fn edges(&self, inflate: f32, out: &mut Vec<[f32; 3]>) {
        for c_axis in 0..3 {
            let (a_axis, b_axis) = ((c_axis + 1) % 3, (c_axis + 2) % 3);
            for a in 0..=self.size(a_axis) {
                for b in 0..=self.size(b_axis) {
                    let mut run: Option<(usize, [i32; 2])> = None;
                    for c in 0..=self.size(c_axis) {
                        let mut solid = 0;
                        let mut odd = 0;
                        let mut toward = [0i32; 2];
                        for da in 0..2isize {
                            for db in 0..2isize {
                                let mut cell = [0isize; 3];
                                cell[a_axis] = a as isize + da - 1;
                                cell[b_axis] = b as isize + db - 1;
                                cell[c_axis] = c as isize;
                                if self.is_full(cell) {
                                    solid += 1;
                                    odd ^= da ^ db;
                                } else {
                                    toward[0] += 2 * da as i32 - 1;
                                    toward[1] += 2 * db as i32 - 1;
                                }
                            }
                        }
                        let edge = solid == 1 || solid == 3 || (solid == 2 && odd == 0);
                        let push = toward.map(i32::signum);
                        if let Some((start, was)) = run
                            && !(edge && was == push)
                        {
                            let mut p0 = [0.0f32; 3];
                            p0[a_axis] = self.coords[a_axis][a] + was[0] as f32 * inflate;
                            p0[b_axis] = self.coords[b_axis][b] + was[1] as f32 * inflate;
                            let mut p1 = p0;
                            p0[c_axis] = self.coords[c_axis][start] - inflate;
                            p1[c_axis] = self.coords[c_axis][c] + inflate;
                            out.push(p0);
                            out.push(p1);
                            run = None;
                        }
                        if edge && run.is_none() {
                            run = Some((c, push));
                        }
                    }
                }
            }
        }
    }

    fn faces(&self, mut emit: impl FnMut(usize, [f32; 3], [f32; 3])) {
        for x in 0..self.size(0) {
            for y in 0..self.size(1) {
                for z in 0..self.size(2) {
                    let cell = [x as isize, y as isize, z as isize];
                    if !self.is_full(cell) {
                        continue;
                    }
                    let c = [x, y, z];
                    let lo: [f32; 3] = std::array::from_fn(|a| self.coords[a][c[a]]);
                    let hi: [f32; 3] = std::array::from_fn(|a| self.coords[a][c[a] + 1]);
                    for (f, &(axis, dir)) in FACE_DIRS.iter().enumerate() {
                        let mut next = cell;
                        next[axis] += dir;
                        if !self.is_full(next) {
                            emit(f, lo, hi);
                        }
                    }
                }
            }
        }
    }
}

pub(super) fn crumbling_mesh(boxes: &[[f32; 6]], uv: [f32; 4]) -> Mesh {
    const INFLATE: f32 = 0.002;
    let [u0, v0, u1, v1] = uv;
    let project = |f: usize, p: [f32; 3]| -> [f32; 2] {
        let [x, y, z] = p.map(|c| c.clamp(0.0, 1.0));
        let [u, v] = match f {
            0 => [x, z],
            1 => [x, 1.0 - z],
            2 => [z, 1.0 - y],
            3 => [1.0 - z, 1.0 - y],
            4 => [x, 1.0 - y],
            _ => [1.0 - x, 1.0 - y],
        };
        [u0 + u * (u1 - u0), v0 + v * (v1 - v0)]
    };
    let mut quads = Vec::new();
    ShapeGrid::new(boxes).faces(|f, lo, hi| {
        let (corners, normal) = box_face(f, lo, hi);
        let uv = corners.map(|p| project(f, p));
        let pos: [[f32; 3]; 4] =
            corners.map(|p| std::array::from_fn(|a| p[a] + normal[a] * INFLATE));
        quads.push((pos, normal, uv));
    });
    quads_mesh(quads)
}

fn box_face(f: usize, lo: [f32; 3], hi: [f32; 3]) -> ([[f32; 3]; 4], [f32; 3]) {
    let ([x0, y0, z0], [x1, y1, z1]) = (lo, hi);
    match f {
        0 => (
            [[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]],
            [0., 1., 0.],
        ),
        1 => (
            [[x0, y0, z1], [x1, y0, z1], [x1, y0, z0], [x0, y0, z0]],
            [0., -1., 0.],
        ),
        2 => (
            [[x1, y0, z0], [x1, y0, z1], [x1, y1, z1], [x1, y1, z0]],
            [1., 0., 0.],
        ),
        3 => (
            [[x0, y0, z1], [x0, y0, z0], [x0, y1, z0], [x0, y1, z1]],
            [-1., 0., 0.],
        ),
        4 => (
            [[x1, y0, z1], [x0, y0, z1], [x0, y1, z1], [x1, y1, z1]],
            [0., 0., 1.],
        ),
        _ => (
            [[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0]],
            [0., 0., -1.],
        ),
    }
}

pub(crate) fn box_faces_mesh(boxes: &[[f32; 6]], inflate: f32, face_uv: [[f32; 2]; 4]) -> Mesh {
    let mut quads = Vec::with_capacity(boxes.len() * 6);
    for b in boxes {
        let lo = [b[0] - inflate, b[1] - inflate, b[2] - inflate];
        let hi = [b[3] + inflate, b[4] + inflate, b[5] + inflate];
        for f in 0..6 {
            let (corners, normal) = box_face(f, lo, hi);
            quads.push((corners, normal, face_uv));
        }
    }
    quads_mesh(quads)
}

fn quads_mesh(quads: Vec<([[f32; 3]; 4], [f32; 3], [[f32; 2]; 4])>) -> Mesh {
    let mut pos: Vec<[f32; 3]> = Vec::with_capacity(quads.len() * 4);
    let mut nrm: Vec<[f32; 3]> = Vec::with_capacity(quads.len() * 4);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(quads.len() * 4);
    let mut idx: Vec<u32> = Vec::with_capacity(quads.len() * 6);
    for (corners, normal, face_uv) in quads {
        let base = pos.len() as u32;
        for i in 0..4 {
            pos.push(corners[i]);
            nrm.push(normal);
            uvs.push(face_uv[i]);
        }
        idx.extend_from_slice(&[base + 2, base + 1, base, base + 3, base + 2, base]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    #[cfg(feature = "shader_support")]
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_COLOR,
        vec![[1.0f32, 1.0, 1.0, 1.0]; pos.len()],
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nrm);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

pub(super) fn update_break_overlay(
    mut commands: Commands,
    view: Res<FrameView>,
    mut overlay: ResMut<BreakOverlay>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut targets: Query<(&mut Transform, &mut Visibility), With<BreakOverlayMarker>>,
    origin: Res<RenderOrigin>,
    mut shown_origin: Local<RenderOrigin>,
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

    if overlay.shown.as_ref() == Some(&view.breaking) && *shown_origin == *origin {
        return;
    }
    *shown_origin = *origin;

    let blocks = crate::blocks::registry();
    let mut used = 0;
    for b in &view.breaking {
        let Some(tile) = blocks.sprites().tile(STAGE_TEX[b.stage.min(9) as usize]) else {
            continue;
        };
        let mesh = crumbling_mesh(&b.boxes, blocks.sprites().uv_rect(tile));
        let translation = origin.block_to_render(b.pos);
        match overlay.pool.get(used).cloned() {
            Some((entity, handle)) => {
                if let Some(mut slot) = meshes.get_mut(&handle) {
                    *slot = mesh;
                }
                if let Ok((mut transform, mut visibility)) = targets.get_mut(entity) {
                    transform.translation = translation;
                    *visibility = Visibility::Visible;
                }
            }
            None => {
                let spawned = super::systems::spawn_overlay(
                    &mut commands,
                    &mut meshes,
                    mesh,
                    overlay.material.clone(),
                    BreakOverlayMarker,
                );
                commands.entity(spawned.0).insert((
                    Transform::from_translation(translation),
                    Visibility::Visible,
                ));
                overlay.pool.push(spawned);
            }
        }
        used += 1;
    }
    for (entity, _) in &overlay.pool[used..] {
        if let Ok((_, mut visibility)) = targets.get_mut(*entity) {
            *visibility = Visibility::Hidden;
        }
    }
    overlay.shown = Some(view.breaking.clone());
}

pub(super) fn update_block_outline(
    view: Res<FrameView>,
    state: Res<crate::gui::GuiState>,
    mut outline: ResMut<BlockOutline>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut targets: Query<(&mut Transform, &mut Visibility), With<BlockOutlineMarker>>,
    origin: Res<RenderOrigin>,
    mut shown_origin: Local<RenderOrigin>,
) {
    let moved = *shown_origin != *origin;
    let target = if state.hide_gui {
        None
    } else {
        if view.targeted_block == outline.shown && !moved {
            return;
        }
        view.targeted_block.clone()
    };
    if target == outline.shown && !moved {
        return;
    }
    *shown_origin = *origin;
    let Ok((mut transform, mut visibility)) = targets.get_mut(outline.entity) else {
        return;
    };

    match &target {
        Some(t) => {
            let shape_changed = outline.shown.as_ref().map(|s| &s.boxes) != Some(&t.boxes);
            if shape_changed && let Some(mut mesh) = meshes.get_mut(&outline.mesh) {
                *mesh = outline_mesh(&t.boxes);
            }
            transform.translation = origin.block_to_render(t.pos);
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
fn esp_mesh(frame: &crate::modules::esp::EspFrame, origin: RenderOrigin) -> Mesh {
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
        let corner = origin.block_to_render([b.x, b.y, b.z]);
        let (x, y, z) = (corner.x - i, corner.y - i, corner.z - i);
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
    view: Res<FrameView>,
    mut overlay: ResMut<EspOverlay>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut targets: Query<&mut Visibility, With<EspMarker>>,
    origin: Res<RenderOrigin>,
    mut shown_origin: Local<RenderOrigin>,
) {
    if *shown_origin == *origin
        && overlay
            .shown
            .as_ref()
            .is_some_and(|shown| std::sync::Arc::ptr_eq(shown, &view.esp))
    {
        return;
    }
    *shown_origin = *origin;
    let frame = view.esp.clone();
    let Ok(mut visibility) = targets.get_mut(overlay.entity) else {
        return;
    };
    if frame.boxes.is_empty() {
        *visibility = Visibility::Hidden;
    } else {
        if let Some(mut mesh) = meshes.get_mut(&overlay.mesh) {
            *mesh = esp_mesh(&frame, *origin);
        }
        *visibility = Visibility::Visible;
    }
    overlay.shown = Some(frame);
}

#[cfg(feature = "click_gui")]
#[derive(Resource)]
pub(super) struct WorldDrawOverlay {
    pub(super) tested: (Entity, Handle<Mesh>),
    pub(super) through: (Entity, Handle<Mesh>),
    pub(super) shown: Option<std::sync::Arc<Vec<crate::renderer::world_draw::Node>>>,
}

#[cfg(feature = "click_gui")]
#[derive(Component)]
pub(super) struct WorldDrawMarker;

#[cfg(feature = "click_gui")]
fn world_draw_mesh(
    nodes: &[crate::renderer::world_draw::Node],
    through: bool,
    origin: RenderOrigin,
) -> Mesh {
    use crate::renderer::esp_material::{ATTRIBUTE_COLOR, ATTRIBUTE_PACKED};
    use crate::renderer::world_draw::Shape;
    use bevy::mesh::VertexAttributeValues;

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[u8; 4]> = Vec::new();
    let mut packed: Vec<u32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut quad = |corners: [Vec3; 4], color: [u8; 4], edges: u32, face: u32| {
        let base = positions.len() as u32;
        for (c, p) in corners.iter().enumerate() {
            positions.push(p.to_array());
            colors.push(color);
            packed.push(c as u32 | edges << 2 | face << 6);
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
    };
    for n in nodes.iter().filter(|n| n.through == through) {
        let color = [
            (n.argb >> 16) as u8,
            (n.argb >> 8) as u8,
            n.argb as u8,
            (n.argb >> 24) as u8,
        ];
        match n.shape {
            Shape::Box { min, max } => {
                let i = OUTLINE_INFLATE;
                let lo = origin.to_render_array(min) - Vec3::splat(i);
                let size = origin.to_render_array(max) + Vec3::splat(i) - lo;
                let edges = if n.outline { 15 } else { 0 };
                for (f, face) in FACES.iter().enumerate() {
                    quad(
                        face.corners.map(|c| lo + Vec3::from_array(c) * size),
                        color,
                        edges,
                        f as u32,
                    );
                }
            }
            Shape::Line { from, to, width } => {
                let (a, b) = (origin.to_render_array(from), origin.to_render_array(to));
                let dir = (b - a).normalize_or_zero();
                if dir == Vec3::ZERO {
                    continue;
                }
                let side = if dir.y.abs() < 0.99 { Vec3::Y } else { Vec3::X };
                let u = dir.cross(side).normalize() * (width as f32 * 0.5);
                let v = dir.cross(u).normalize() * (width as f32 * 0.5);
                for (f, (s1, s2)) in [
                    (u + v, u - v),
                    (u - v, -u - v),
                    (-u - v, -u + v),
                    (-u + v, u + v),
                ]
                .into_iter()
                .enumerate()
                {
                    quad([a + s1, b + s1, a + s2, b + s2], color, 0, 2 + f as u32);
                }
            }
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
pub(super) fn update_world_draw(
    mut overlay: ResMut<WorldDrawOverlay>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut targets: Query<&mut Visibility, With<WorldDrawMarker>>,
    origin: Res<RenderOrigin>,
    mut shown_origin: Local<RenderOrigin>,
) {
    let scene = crate::renderer::world_draw::scene();
    let same = match (&overlay.shown, &scene) {
        (Some(a), Some(b)) => std::sync::Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    };
    if same && *shown_origin == *origin {
        return;
    }
    *shown_origin = *origin;
    let nodes = scene.as_deref().map_or(&[][..], Vec::as_slice);
    for (through, (entity, mesh)) in [
        (false, overlay.tested.clone()),
        (true, overlay.through.clone()),
    ] {
        let any = nodes.iter().any(|n| n.through == through);
        if let Ok(mut visibility) = targets.get_mut(entity) {
            *visibility = if any {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
        if any && let Some(mut m) = meshes.get_mut(&mesh) {
            *m = world_draw_mesh(nodes, through, *origin);
        }
    }
    overlay.shown = scene;
}

pub(super) fn update_chunk_borders(
    view: Res<FrameView>,
    state: Res<crate::gui::GuiState>,
    mut overlay: ResMut<ChunkBorderOverlay>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut targets: Query<(&mut Transform, &mut Visibility), With<ChunkBorderMarker>>,
    origin: Res<RenderOrigin>,
    mut shown_origin: Local<RenderOrigin>,
) {
    let pos = state.chunk_borders.then_some(view.player_pos);
    let dim = super::dimension::current();
    let key = pos.map(|p| {
        let (cx, cz) = (
            (p[0].floor() as i32).div_euclid(16),
            (p[2].floor() as i32).div_euclid(16),
        );
        let sec_y0 = (p[1].floor() as i32).div_euclid(16) * 16;
        (cx, cz, sec_y0, dim)
    });
    if key == overlay.shown && *shown_origin == *origin {
        return;
    }
    let Ok((mut transform, mut visibility)) = targets.get_mut(overlay.entity) else {
        return;
    };
    *shown_origin = *origin;

    match key {
        Some((cx, cz, sec_y0, dim)) => {
            let shape_changed = overlay.shown.map(|(_, _, sy, d)| (sy, d)) != Some((sec_y0, dim));
            if shape_changed && let Some(mut mesh) = meshes.get_mut(&overlay.mesh) {
                let (ymin, ymax) = dim.y_range();
                *mesh = chunk_border_mesh(ymin as f32, ymax as f32, sec_y0 as f32);
            }
            transform.translation = origin.block_to_render([cx * 16, 0, cz * 16]);
            *visibility = Visibility::Visible;
        }
        None => *visibility = Visibility::Hidden,
    }
    overlay.shown = key;
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANVIL: [[f32; 6]; 7] = [
        [0.125, 0., 0.125, 0.875, 0.25, 0.875],
        [0.25, 0.25, 0.1875, 0.75, 0.3125, 0.8125],
        [0.375, 0.3125, 0.25, 0.625, 1., 0.75],
        [0.1875, 0.625, 0., 0.375, 1., 1.],
        [0.375, 0.625, 0., 0.8125, 1., 0.25],
        [0.375, 0.625, 0.75, 0.8125, 1., 1.],
        [0.625, 0.625, 0.25, 0.8125, 1., 0.75],
    ];

    #[test]
    fn a_lone_box_keeps_its_twelve_grown_edges() {
        let mut out = Vec::new();
        outline_edges([[0.0, 0.0, 0.0, 1.0, 0.5, 1.0]], &mut out);
        assert_eq!(out.len(), 24);
        let e = OUTLINE_INFLATE;
        for p in &out {
            for (a, max) in [1.0, 0.5, 1.0].into_iter().enumerate() {
                assert!(
                    (p[a] + e).abs() < 1e-6 || (p[a] - max - e).abs() < 1e-6,
                    "{p:?}"
                );
            }
        }
    }

    #[test]
    fn the_anvil_outlines_as_four_stacked_cuboids() {
        let mut out = Vec::new();
        outline_edges(ANVIL, &mut out);
        assert_eq!(out.len(), 4 * 12 * 2);

        let mut area = 0.0;
        ShapeGrid::new(&ANVIL).faces(|f, lo, hi| {
            let axis = FACE_DIRS[f].0;
            let d: Vec<f32> = (0..3)
                .filter(|&a| a != axis)
                .map(|a| hi[a] - lo[a])
                .collect();
            area += d[0] * d[1] * 256.0;
        });
        assert!((area - 1204.0).abs() < 1e-3, "{area}");
    }
}
