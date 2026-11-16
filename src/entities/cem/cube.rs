use crate::blocks::direction::Direction;
use crate::entities::geom::{self, CubeDef};
use crate::entities::quads::QuadMesh;

#[derive(Clone, Copy, Debug)]
pub struct CemCube {
    pub layout: Layout,
    pub slot: [f32; 2],
}

#[derive(Clone, Copy, Debug)]
pub enum Layout {
    Vanilla,
    MirroredBox { u: bool, v: bool },
    PerFace([Option<[f32; 4]>; 6]),
}

impl CemCube {
    pub fn slotted(slot: usize, count: usize) -> CemCube {
        CemCube {
            layout: Layout::Vanilla,
            slot: [slot as f32, count as f32],
        }
    }

    pub fn is_custom(&self) -> bool {
        !matches!(self.layout, Layout::Vanilla)
    }

    pub fn slot_uvs(&self, mut uvs: [[f32; 2]; 4]) -> [[f32; 2]; 4] {
        let [slot, count] = self.slot;
        if count > 1.0 {
            for uv in &mut uvs {
                uv[1] = (slot + uv[1]) / count;
            }
        }
        uvs
    }
}

type Quad = [usize; 4];

const Y_MIN: Quad = [5, 4, 0, 1];
const Y_MIN_PER_FACE: Quad = [0, 1, 5, 4];
const Y_MAX: Quad = [2, 3, 7, 6];
const Y_MAX_PER_FACE: Quad = [7, 6, 2, 3];
const X_MIN: Quad = [0, 4, 7, 3];
const X_MAX: Quad = [5, 1, 2, 6];
const Z_MIN: Quad = [1, 0, 3, 2];
const Z_MAX: Quad = [4, 5, 6, 7];

pub fn push(out: &mut QuadMesh, cube: &CubeDef, cem: &CemCube, sheet: [f32; 2], scale: f32) {
    if !cem.is_custom() {
        geom::push_cube(out, cube, sheet, scale);
        return;
    }
    let [w, h, d] = cube.size;
    let [ox, oy, oz] = cube.origin;
    let x0 = (ox - cube.grow.0) * scale;
    let y0 = (oy - cube.grow.1) * scale;
    let z0 = (oz - cube.grow.2) * scale;
    let x1 = (ox + w + cube.grow.0) * scale;
    let y1 = (oy + h + cube.grow.1) * scale;
    let z1 = (oz + d + cube.grow.2) * scale;
    let corners = [
        [x0, y0, z0],
        [x1, y0, z0],
        [x1, y1, z0],
        [x0, y1, z0],
        [x0, y0, z1],
        [x1, y0, z1],
        [x1, y1, z1],
        [x0, y1, z1],
    ];
    let (su, sv) = (sheet[0] * cube.tex_scale[0], sheet[1] * cube.tex_scale[1]);
    let mut quad = |q: Quad, rect: [f32; 4], face: Direction| {
        let [u0, v0, u1, v1] = rect;
        let uvs = cem.slot_uvs([
            [u1 / su, v0 / sv],
            [u0 / su, v0 / sv],
            [u0 / su, v1 / sv],
            [u1 / su, v1 / sv],
        ]);
        out.quad(q.map(|i| corners[i]), uvs, face.normal());
    };
    match cem.layout {
        Layout::Vanilla => {}
        Layout::MirroredBox { u: mu, v: mv } => {
            let [tu, tv] = cube.tex;
            let (j, k, l) = (tu, tu + d, tu + d + w);
            let (m, n, o) = (tu + d + w + w, tu + d + w + d, tu + d + w + d + w);
            let (p, q, r) = (tv, tv + d, tv + d + h);
            let pick = |swap: bool, a: f32, b: f32| if swap { b } else { a };
            quad(
                if mv { Y_MAX } else { Y_MIN },
                [
                    pick(mu, k, l),
                    pick(mv, p, q),
                    pick(mu, l, k),
                    pick(mv, q, p),
                ],
                if mv { Direction::Up } else { Direction::Down },
            );
            quad(
                if mv { Y_MIN } else { Y_MAX },
                [
                    pick(mu, l, m),
                    pick(mv, q, p),
                    pick(mu, m, l),
                    pick(mv, p, q),
                ],
                if mv { Direction::Down } else { Direction::Up },
            );
            quad(
                if mu { X_MAX } else { X_MIN },
                [
                    pick(mu, j, k),
                    pick(mv, q, r),
                    pick(mu, k, j),
                    pick(mv, r, q),
                ],
                if mu { Direction::East } else { Direction::West },
            );
            quad(
                Z_MIN,
                [
                    pick(mu, k, l),
                    pick(mv, q, r),
                    pick(mu, l, k),
                    pick(mv, r, q),
                ],
                Direction::North,
            );
            quad(
                if mu { X_MIN } else { X_MAX },
                [
                    pick(mu, l, n),
                    pick(mv, q, r),
                    pick(mu, n, l),
                    pick(mv, r, q),
                ],
                if mu { Direction::West } else { Direction::East },
            );
            quad(
                Z_MAX,
                [
                    pick(mu, n, o),
                    pick(mv, q, r),
                    pick(mu, o, n),
                    pick(mv, r, q),
                ],
                Direction::South,
            );
        }
        Layout::PerFace(rects) => {
            let faces = [
                (Direction::Up, Y_MIN_PER_FACE, Direction::Down),
                (Direction::Down, Y_MAX_PER_FACE, Direction::Up),
                (Direction::West, X_MAX, Direction::East),
                (Direction::North, Z_MIN, Direction::North),
                (Direction::East, X_MIN, Direction::West),
                (Direction::South, Z_MAX, Direction::South),
            ];
            for (key, corners, face) in faces {
                if let Some(rect) = rects[key as usize] {
                    quad(corners, rect, face);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::geom::{Faces, Grow};

    fn cube(cem: CemCube) -> CubeDef {
        CubeDef {
            tex: [0.0, 0.0],
            origin: [0.0, 0.0, 0.0],
            size: [2.0, 3.0, 4.0],
            grow: Grow::NONE,
            mirror: false,
            tex_scale: [1.0, 1.0],
            faces: Faces::ALL,
            cem: Some(cem),
        }
    }

    #[test]
    fn per_face_skips_missing_rectangles() {
        let mut rects = [None; 6];
        rects[Direction::North as usize] = Some([0.0, 0.0, 2.0, 3.0]);
        rects[Direction::Up as usize] = Some([0.0, 0.0, 2.0, 4.0]);
        let cem = CemCube {
            layout: Layout::PerFace(rects),
            slot: [0.0, 1.0],
        };
        let mut out = QuadMesh::new();
        push(&mut out, &cube(cem), [16.0, 16.0], 1.0);
        assert_eq!(out.finish().count_vertices(), 8);
    }

    #[test]
    fn slot_maps_v_into_its_band() {
        let cem = CemCube::slotted(1, 4);
        let uvs = cem.slot_uvs([[0.5, 0.0], [0.5, 1.0], [0.0, 0.5], [1.0, 0.25]]);
        assert_eq!(uvs[0], [0.5, 0.25]);
        assert_eq!(uvs[1], [0.5, 0.5]);
        assert_eq!(uvs[2], [0.0, 0.375]);
    }
}
