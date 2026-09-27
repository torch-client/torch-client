use crate::items::model::{DOWN, NORTH, SOUTH, UP, WEST};

use super::state::Rot;

pub type Mat3 = [[i8; 3]; 3];

pub const IDENTITY: Mat3 = [[1, 0, 0], [0, 1, 0], [0, 0, 1]];

pub const DIR_VEC: [[i8; 3]; 6] = [
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
    [-1, 0, 0],
    [1, 0, 0],
];

pub fn mul(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut out = [[0i8; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    out
}

pub fn transpose(m: &Mat3) -> Mat3 {
    let mut out = [[0i8; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = m[j][i];
        }
    }
    out
}

pub fn apply(m: &Mat3, v: [f32; 3]) -> [f32; 3] {
    let mut out = [0.0f32; 3];
    for (i, o) in out.iter_mut().enumerate() {
        *o = m[i][0] as f32 * v[0] + m[i][1] as f32 * v[1] + m[i][2] as f32 * v[2];
    }
    out
}

fn apply_i(m: &Mat3, v: [i8; 3]) -> [i8; 3] {
    let mut out = [0i8; 3];
    for (i, o) in out.iter_mut().enumerate() {
        *o = m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2];
    }
    out
}

fn dir_of(v: [i8; 3]) -> Option<usize> {
    DIR_VEC.iter().position(|d| *d == v)
}

fn x_mat(quadrant: u8) -> Mat3 {
    match quadrant & 3 {
        1 => [[1, 0, 0], [0, 0, 1], [0, -1, 0]],
        2 => [[1, 0, 0], [0, -1, 0], [0, 0, -1]],
        3 => [[1, 0, 0], [0, 0, -1], [0, 1, 0]],
        _ => IDENTITY,
    }
}

fn y_mat(quadrant: u8) -> Mat3 {
    match quadrant & 3 {
        1 => [[0, 0, -1], [0, 1, 0], [1, 0, 0]],
        2 => [[-1, 0, 0], [0, 1, 0], [0, 0, -1]],
        3 => [[0, 0, 1], [0, 1, 0], [-1, 0, 0]],
        _ => IDENTITY,
    }
}

fn z_mat(quadrant: u8) -> Mat3 {
    match quadrant & 3 {
        1 => [[0, 1, 0], [-1, 0, 0], [0, 0, 1]],
        2 => [[-1, 0, 0], [0, -1, 0], [0, 0, 1]],
        3 => [[0, -1, 0], [1, 0, 0], [0, 0, 1]],
        _ => IDENTITY,
    }
}

pub fn model_matrix(rot: Rot) -> Mat3 {
    mul(&y_mat(rot.y), &mul(&x_mat(rot.x), &z_mat(rot.z)))
}

pub fn rotate_about_centre(m: &Mat3, p: [f32; 3]) -> [f32; 3] {
    let centred = [p[0] - 0.5, p[1] - 0.5, p[2] - 0.5];
    let r = apply(m, centred);
    [r[0] + 0.5, r[1] + 0.5, r[2] + 0.5]
}

pub fn rotate_dir(m: &Mat3, dir: usize) -> Option<usize> {
    dir_of(apply_i(m, DIR_VEC[dir]))
}

fn local_to_global(dir: usize) -> Mat3 {
    match dir {
        DOWN => [[1, 0, 0], [0, 0, -1], [0, 1, 0]],
        UP => [[1, 0, 0], [0, 0, 1], [0, -1, 0]],
        NORTH => [[-1, 0, 0], [0, 1, 0], [0, 0, -1]],
        SOUTH => IDENTITY,
        WEST => [[0, 0, -1], [0, 1, 0], [1, 0, 0]],
        _ => [[0, 0, 1], [0, 1, 0], [-1, 0, 0]],
    }
}

pub fn uv_transform(model: &Mat3, dir: usize) -> [[f32; 2]; 2] {
    let l2g = local_to_global(dir);
    let Some(new_dir) = rotate_dir(model, dir) else {
        return [[1.0, 0.0], [0.0, 1.0]];
    };
    let face = mul(&transpose(&local_to_global(new_dir)), &mul(model, &l2g));
    let inverse = transpose(&face);
    [
        [inverse[0][0] as f32, inverse[0][1] as f32],
        [inverse[1][0] as f32, inverse[1][1] as f32],
    ]
}

pub fn apply_uv(t: &[[f32; 2]; 2], u: f32, v: f32) -> [f32; 2] {
    let (cu, cv) = (u - 0.5, v - 0.5);
    [
        t[0][0] * cu + t[0][1] * cv + 0.5,
        t[1][0] * cu + t[1][1] * cv + 0.5,
    ]
}
