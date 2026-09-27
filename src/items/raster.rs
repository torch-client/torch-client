use std::sync::Arc;

use image::RgbaImage;

use super::model::{Tex, Transform};

pub const SUPERSAMPLE: u32 = 1;

pub struct Quad {
    pub pos: [[f32; 3]; 4],
    pub uv: [[f32; 2]; 4],
    pub tex: Arc<Tex>,
    pub color: [f32; 3],
}

pub fn rotate_xyz(p: [f32; 3], deg: [f32; 3]) -> [f32; 3] {
    let [mut x, mut y, mut z] = p;
    let (rx, ry, rz) = (
        deg[0].to_radians(),
        deg[1].to_radians(),
        deg[2].to_radians(),
    );
    if rz != 0.0 {
        let (s, c) = rz.sin_cos();
        let (nx, ny) = (x * c - y * s, x * s + y * c);
        x = nx;
        y = ny;
    }
    if ry != 0.0 {
        let (s, c) = ry.sin_cos();
        let (nx, nz) = (x * c + z * s, -x * s + z * c);
        x = nx;
        z = nz;
    }
    if rx != 0.0 {
        let (s, c) = rx.sin_cos();
        let (ny, nz) = (y * c - z * s, y * s + z * c);
        y = ny;
        z = nz;
    }
    [x, y, z]
}

pub fn rotate_axis(
    p: [f32; 3],
    origin: [f32; 3],
    axis: usize,
    deg: f32,
    rescale: bool,
) -> [f32; 3] {
    let (s, c) = deg.to_radians().sin_cos();
    let mut v = [p[0] - origin[0], p[1] - origin[1], p[2] - origin[2]];
    if rescale && c.abs() > 1e-4 {
        let f = 1.0 / c.abs();
        for (i, comp) in v.iter_mut().enumerate() {
            if i != axis {
                *comp *= f;
            }
        }
    }
    let r = match axis {
        0 => [v[0], v[1] * c - v[2] * s, v[1] * s + v[2] * c],
        1 => [v[0] * c + v[2] * s, v[1], -v[0] * s + v[2] * c],
        _ => [v[0] * c - v[1] * s, v[0] * s + v[1] * c, v[2]],
    };
    [r[0] + origin[0], r[1] + origin[1], r[2] + origin[2]]
}

fn project(p: [f32; 3], t: &Transform, size: f32) -> [f32; 3] {
    let scaled = [
        (p[0] - 0.5) * t.scale[0],
        (p[1] - 0.5) * t.scale[1],
        (p[2] - 0.5) * t.scale[2],
    ];
    let r = rotate_xyz(scaled, t.rotation);
    let q = [
        r[0] + t.translation[0],
        r[1] + t.translation[1],
        r[2] + t.translation[2],
    ];
    let half = size * 0.5;
    [half + size * q[0], half - size * q[1], q[2]]
}

pub fn render(quads: &[Quad], transform: &Transform, icon: u32) -> RgbaImage {
    let size = icon * SUPERSAMPLE;
    let n = (size * size) as usize;
    let mut color = vec![[0.0f32; 4]; n];
    let mut depth = vec![f32::NEG_INFINITY; n];

    for quad in quads {
        let mut screen = [[0.0f32; 3]; 4];
        for i in 0..4 {
            screen[i] = project(quad.pos[i], transform, size as f32);
        }
        let a = [
            quad.pos[1][0] - quad.pos[0][0],
            quad.pos[1][1] - quad.pos[0][1],
            quad.pos[1][2] - quad.pos[0][2],
        ];
        let b = [
            quad.pos[2][0] - quad.pos[0][0],
            quad.pos[2][1] - quad.pos[0][1],
            quad.pos[2][2] - quad.pos[0][2],
        ];
        let normal = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let view_normal = rotate_xyz(
            [
                normal[0] * transform.scale[1] * transform.scale[2],
                normal[1] * transform.scale[0] * transform.scale[2],
                normal[2] * transform.scale[0] * transform.scale[1],
            ],
            transform.rotation,
        );
        if view_normal[2] <= 0.0 {
            continue;
        }
        for tri in [[0usize, 1, 2], [0, 2, 3]] {
            triangle(
                &mut color,
                &mut depth,
                size,
                [screen[tri[0]], screen[tri[1]], screen[tri[2]]],
                [quad.uv[tri[0]], quad.uv[tri[1]], quad.uv[tri[2]]],
                &quad.tex,
                quad.color,
            );
        }
    }

    downsample(&color, size, icon)
}

#[allow(clippy::too_many_arguments)]
fn triangle(
    color: &mut [[f32; 4]],
    depth: &mut [f32],
    size: u32,
    p: [[f32; 3]; 3],
    uv: [[f32; 2]; 3],
    tex: &Tex,
    tint: [f32; 3],
) {
    let area =
        (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
    if area.abs() < 1e-9 {
        return;
    }
    let min_x = p
        .iter()
        .map(|v| v[0])
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(0.0) as u32;
    let max_x = (p
        .iter()
        .map(|v| v[0])
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil())
    .min(size as f32) as u32;
    let min_y = p
        .iter()
        .map(|v| v[1])
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(0.0) as u32;
    let max_y = (p
        .iter()
        .map(|v| v[1])
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil())
    .min(size as f32) as u32;

    for py in min_y..max_y {
        for px in min_x..max_x {
            let (x, y) = (px as f32 + 0.5, py as f32 + 0.5);
            let w0 = ((p[1][0] - x) * (p[2][1] - y) - (p[2][0] - x) * (p[1][1] - y)) / area;
            let w1 = ((p[2][0] - x) * (p[0][1] - y) - (p[0][0] - x) * (p[2][1] - y)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let z = w0 * p[0][2] + w1 * p[1][2] + w2 * p[2][2];
            let idx = (py * size + px) as usize;
            if z <= depth[idx] {
                continue;
            }
            let u = w0 * uv[0][0] + w1 * uv[1][0] + w2 * uv[2][0];
            let v = w0 * uv[0][1] + w1 * uv[1][1] + w2 * uv[2][1];
            let texel = tex.sample(u, v);
            let alpha = texel[3] as f32 / 255.0;
            if alpha < 0.01 {
                continue;
            }
            let src = [
                texel[0] as f32 / 255.0 * tint[0],
                texel[1] as f32 / 255.0 * tint[1],
                texel[2] as f32 / 255.0 * tint[2],
            ];
            let dst = &mut color[idx];
            for c in 0..3 {
                dst[c] = src[c] * alpha + dst[c] * (1.0 - alpha);
            }
            dst[3] = alpha + dst[3] * (1.0 - alpha);
            if alpha >= 0.5 {
                depth[idx] = z;
            }
        }
    }
}

fn downsample(color: &[[f32; 4]], size: u32, icon: u32) -> RgbaImage {
    let f = size / icon;
    let count = (f * f) as f32;
    let mut out = RgbaImage::new(icon, icon);
    for y in 0..icon {
        for x in 0..icon {
            let mut acc = [0.0f32; 4];
            for sy in 0..f {
                for sx in 0..f {
                    let s = color[((y * f + sy) * size + x * f + sx) as usize];
                    for c in 0..4 {
                        acc[c] += s[c];
                    }
                }
            }
            let a = acc[3] / count;
            let px = if a <= 0.0 {
                [0, 0, 0, 0]
            } else {
                let unpremultiply = |v: f32| ((v / count / a) * 255.0).clamp(0.0, 255.0) as u8;
                [
                    unpremultiply(acc[0]),
                    unpremultiply(acc[1]),
                    unpremultiply(acc[2]),
                    (a * 255.0).clamp(0.0, 255.0) as u8,
                ]
            };
            out.put_pixel(x, y, image::Rgba(px));
        }
    }
    out
}
