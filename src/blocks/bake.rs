use std::sync::Arc;

use crate::items::model::{
    Assets, Elem, FaceDef, Resolved, corner_uv, default_face_uv, face_positions,
};
use crate::items::raster;
use crate::renderer::{ATLAS_COLS, TILE_PX};

use super::rotation::{self, Mat3};
use super::state::{ModelRef, Rot, Slot};

pub fn tile_uv_rect(tile: u32, atlas_rows: u32) -> [f32; 4] {
    let col = tile % ATLAS_COLS;
    let row = tile / ATLAS_COLS;
    let tw = 1.0 / ATLAS_COLS as f32;
    let th = 1.0 / atlas_rows as f32;
    let iu = 0.02 / (ATLAS_COLS * TILE_PX) as f32;
    let iv = 0.02 / (atlas_rows * TILE_PX) as f32;
    [
        col as f32 * tw + iu,
        row as f32 * th + iv,
        (col + 1) as f32 * tw - iu,
        (row + 1) as f32 * th - iv,
    ]
}

fn tile_of(path: &str) -> Option<u32> {
    let stem = path.rsplit('/').next()?;
    crate::TEXTURE_MAP.get()?.get(stem).copied()
}

fn texture_is_opaque(path: &str) -> bool {
    match path.rsplit('/').next() {
        Some(stem) => crate::renderer::texture_is_opaque(stem),
        None => false,
    }
}

#[derive(Clone, Debug)]
pub struct BakedQuad {
    pub pos: [[f32; 3]; 4],
    pub uv: [[f32; 2]; 4],
    pub normal: [f32; 3],
    pub face: u8,
    pub cull: Option<u8>,
    pub shaded: bool,
    pub tint: i32,
    pub opaque: bool,
}

impl BakedQuad {
    #[inline]
    pub fn shade(&self, cardinal: &[f32; 6]) -> f32 {
        if self.shaded {
            cardinal[self.face as usize]
        } else {
            cardinal[crate::renderer::dimension::UP]
        }
    }
}

#[derive(Debug)]
pub struct BakedModel {
    pub quads: Vec<BakedQuad>,
    #[allow(dead_code, reason = "read by the block coverage sweep in tests")]
    pub had_elements: bool,
    pub occludes: bool,
    pub full_cube: bool,
    pub ambient_occlusion: bool,
    pub missing_textures: u32,
    pub particle_uv: Option<[f32; 4]>,
}

impl Default for BakedModel {
    fn default() -> Self {
        Self {
            quads: Vec::new(),
            had_elements: false,
            occludes: false,
            full_cube: false,
            ambient_occlusion: true,
            missing_textures: 0,
            particle_uv: None,
        }
    }
}

#[derive(Debug)]
pub struct BakedSlot {
    pub choices: Vec<Arc<BakedModel>>,
    pub weights: Vec<u32>,
    pub total: u32,
}

impl BakedSlot {
    pub fn pick(&self, selection: u32) -> &BakedModel {
        let mut acc = 0u32;
        for (model, weight) in self.choices.iter().zip(self.weights.iter()) {
            acc += *weight;
            if selection < acc {
                return model;
            }
        }
        &self.choices[0]
    }
}

#[derive(Debug)]
pub struct BakedBlock {
    pub parts: Vec<BakedSlot>,
    pub is_solid: bool,
    pub full_cube: bool,
    pub ambient_occlusion: bool,
    pub randomized: bool,
}

impl Default for BakedBlock {
    fn default() -> Self {
        Self {
            parts: Vec::new(),
            is_solid: false,
            full_cube: false,
            ambient_occlusion: true,
            randomized: false,
        }
    }
}

pub fn bake_slot(slot: &Slot, mut resolve: impl FnMut(&ModelRef) -> Arc<BakedModel>) -> BakedSlot {
    let choices: Vec<Arc<BakedModel>> = slot.choices.iter().map(&mut resolve).collect();
    let weights: Vec<u32> = slot.choices.iter().map(|c| c.weight).collect();
    BakedSlot {
        choices,
        weights,
        total: slot.total,
    }
}

pub fn bake_model(assets: &Assets, model_path: &str, rot: Rot, atlas_rows: u32) -> BakedModel {
    let resolved = assets.model(model_path);
    bake_resolved(&resolved, rot, atlas_rows)
}

pub fn bake_resolved(resolved: &Resolved, rot: Rot, atlas_rows: u32) -> BakedModel {
    let matrix = rotation::model_matrix(rot);
    let identity = rot.is_identity();
    let particle_uv = resolved
        .texture_path("particle")
        .and_then(|path| tile_of(&path))
        .map(|tile| tile_uv_rect(tile, atlas_rows));
    let mut out = BakedModel {
        had_elements: !resolved.elements.is_empty(),
        ambient_occlusion: resolved.ambient_occlusion,
        particle_uv,
        ..Default::default()
    };

    for elem in &resolved.elements {
        let from = [
            elem.from[0] / 16.0,
            elem.from[1] / 16.0,
            elem.from[2] / 16.0,
        ];
        let to = [elem.to[0] / 16.0, elem.to[1] / 16.0, elem.to[2] / 16.0];

        let mut faces: [Option<(&FaceDef, u32, bool)>; 6] = [None; 6];
        for face in &elem.faces {
            let Some(path) = resolved.texture_path(&face.texture) else {
                out.missing_textures += 1;
                continue;
            };
            match tile_of(&path) {
                Some(tile) => faces[face.dir] = Some((face, tile, texture_is_opaque(&path))),
                None => out.missing_textures += 1,
            }
        }

        let full_element = is_full_cube(elem)
            && faces.iter().enumerate().all(|(dir, f)| match f {
                Some((face, _, _)) => face.cull == Some(dir),
                None => false,
            });
        let occluding_element = full_element
            && faces
                .iter()
                .all(|f| matches!(f, Some((_, _, opaque)) if *opaque));
        out.full_cube |= full_element;
        out.occludes |= occluding_element;

        for (dir, face) in faces.iter().enumerate() {
            let Some((face, tile, _)) = face else {
                continue;
            };
            let face_opaque = faces[dir].map(|(_, _, opaque)| opaque).unwrap_or(false);
            let mut pos = face_positions(from, to, dir);
            if let Some(er) = elem.rotation {
                let origin = [
                    er.origin[0] / 16.0,
                    er.origin[1] / 16.0,
                    er.origin[2] / 16.0,
                ];
                for p in pos.iter_mut() {
                    *p = raster::rotate_axis(*p, origin, er.axis, er.angle, er.rescale);
                }
            }
            if !identity {
                for p in pos.iter_mut() {
                    *p = rotation::rotate_about_centre(&matrix, *p);
                }
            }

            let rect = face
                .uv
                .unwrap_or_else(|| default_face_uv(elem.from, elem.to, dir));
            let uv_t = (rot.uvlock && !identity).then(|| rotation::uv_transform(&matrix, dir));
            let [au0, av0, au1, av1] = tile_uv_rect(*tile, atlas_rows);
            let mut uv = [[0.0f32; 2]; 4];
            for (i, out_uv) in uv.iter_mut().enumerate() {
                let [u, v] = corner_uv(rect, face.quadrant, i);
                let (mut u, mut v) = (u / 16.0, v / 16.0);
                if let Some(t) = &uv_t {
                    [u, v] = rotation::apply_uv(t, u, v);
                }
                *out_uv = [au0 + (au1 - au0) * u, av0 + (av1 - av0) * v];
            }

            let cull = match face.cull {
                Some(d) if identity => Some(d as u8),
                Some(d) => rotation::rotate_dir(&matrix, d).map(|d| d as u8),
                None => None,
            };

            let rotated_dir = if identity {
                dir
            } else {
                rotation::rotate_dir(&matrix, dir).unwrap_or(dir)
            };

            if !identity && elem.rotation.is_none() {
                recalculate_winding(&mut pos, &mut uv, rotated_dir);
            }
            out.quads.push(BakedQuad {
                pos,
                uv,
                normal: quad_normal(&pos, &matrix, dir),
                face: rotated_dir as u8,
                cull,
                shaded: elem.shade,
                tint: face.tint,
                opaque: face_opaque,
            });
        }
    }
    out
}

fn is_full_cube(elem: &Elem) -> bool {
    elem.rotation.is_none()
        && elem.from.iter().all(|c| *c == 0.0)
        && elem.to.iter().all(|c| *c == 16.0)
}

fn quad_normal(pos: &[[f32; 3]; 4], matrix: &Mat3, dir: usize) -> [f32; 3] {
    let e1 = sub(pos[1], pos[0]);
    let e2 = sub(pos[2], pos[0]);
    let n = [
        e1[1] * e2[2] - e1[2] * e2[1],
        e1[2] * e2[0] - e1[0] * e2[2],
        e1[0] * e2[1] - e1[1] * e2[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len > 1e-9 {
        return [n[0] / len, n[1] / len, n[2] / len];
    }
    let d = rotation::DIR_VEC[dir];
    rotation::apply(matrix, [d[0] as f32, d[1] as f32, d[2] as f32])
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn recalculate_winding(pos: &mut [[f32; 3]; 4], uv: &mut [[f32; 2]; 4], dir: usize) {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for p in pos.iter() {
        for axis in 0..3 {
            min[axis] = min[axis].min(p[axis]);
            max[axis] = max[axis].max(p[axis]);
        }
    }
    for vertex in 0..4 {
        let sel = crate::items::model::FACE_CORNERS[dir][vertex];
        let want = [
            if sel[0] { max[0] } else { min[0] },
            if sel[1] { max[1] } else { min[1] },
            if sel[2] { max[2] } else { min[2] },
        ];
        let found = (vertex..4).min_by(|a, b| {
            let d = |i: usize| (0..3).map(|k| (pos[i][k] - want[k]).abs()).sum::<f32>();
            d(*a).total_cmp(&d(*b))
        });
        if let Some(found) = found
            && found != vertex
        {
            pos.swap(found, vertex);
            uv.swap(found, vertex);
        }
    }
}
