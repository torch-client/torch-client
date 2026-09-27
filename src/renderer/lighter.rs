use crate::direction::Direction;
use crate::renderer::Occupancy;

pub mod coords {
    pub const FULL_BRIGHT: u32 = pack(15, 15);

    #[inline]
    pub const fn pack(block: u8, sky: u8) -> u32 {
        (block as u32) << 4 | (sky as u32) << 20
    }

    #[inline]
    pub const fn block(packed: u32) -> u32 {
        (packed >> 4) & 15
    }

    #[inline]
    pub const fn sky(packed: u32) -> u32 {
        (packed >> 20) & 15
    }

    #[inline]
    pub const fn with_block(packed: u32, block: u8) -> u32 {
        (packed & 0x00FF_0000) | (block as u32) << 4
    }

    #[inline]
    pub const fn smooth_block(packed: u32) -> u32 {
        packed & 255
    }

    #[inline]
    pub const fn smooth_sky(packed: u32) -> u32 {
        (packed >> 16) & 255
    }

    #[inline]
    pub const fn smooth_pack(block: u32, sky: u32) -> u32 {
        (block & 255) | (sky & 255) << 16
    }

    pub fn smooth_blend(mut n1: u32, mut n2: u32, mut n3: u32, center: u32) -> u32 {
        if sky(center) > 2 || block(center) > 2 {
            for n in [&mut n1, &mut n2, &mut n3] {
                if sky(*n) == 0 {
                    *n |= center & 0x00FF_0000;
                }
                if block(*n) == 0 {
                    *n |= center & 255;
                }
            }
        }
        (n1.wrapping_add(n2).wrapping_add(n3).wrapping_add(center) >> 2) & 0x00FF_00FF
    }

    pub fn smooth_weighted_blend(c: [u32; 4], w: [f32; 4]) -> u32 {
        let sky = (0..4).map(|i| smooth_sky(c[i]) as f32 * w[i]).sum::<f32>() as u32;
        let block = (0..4)
            .map(|i| smooth_block(c[i]) as f32 * w[i])
            .sum::<f32>() as u32;
        smooth_pack(block, sky)
    }
}

const OFFSETS: [(i32, i32, i32); 6] = [
    (0, -1, 0),
    (0, 1, 0),
    (0, 0, -1),
    (0, 0, 1),
    (-1, 0, 0),
    (1, 0, 0),
];

const DOWN: usize = Direction::Down.index();
const UP: usize = Direction::Up.index();
const NORTH: usize = Direction::North.index();
const SOUTH: usize = Direction::South.index();
const WEST: usize = Direction::West.index();
const EAST: usize = Direction::East.index();

const S_DOWN: usize = 0;
const S_UP: usize = 1;
const S_NORTH: usize = 2;
const S_SOUTH: usize = 3;
const S_WEST: usize = 4;
const S_EAST: usize = 5;
const S_FLIP_DOWN: usize = 6;
const S_FLIP_UP: usize = 7;
const S_FLIP_NORTH: usize = 8;
const S_FLIP_SOUTH: usize = 9;
const S_FLIP_WEST: usize = 10;
const S_FLIP_EAST: usize = 11;

struct Adjacency {
    corners: [usize; 4],
    weights: [[usize; 8]; 4],
}

const ADJACENCY: [Adjacency; 6] = [
    Adjacency {
        corners: [WEST, EAST, NORTH, SOUTH],
        weights: [
            [
                S_FLIP_WEST,
                S_SOUTH,
                S_FLIP_WEST,
                S_FLIP_SOUTH,
                S_WEST,
                S_FLIP_SOUTH,
                S_WEST,
                S_SOUTH,
            ],
            [
                S_FLIP_WEST,
                S_NORTH,
                S_FLIP_WEST,
                S_FLIP_NORTH,
                S_WEST,
                S_FLIP_NORTH,
                S_WEST,
                S_NORTH,
            ],
            [
                S_FLIP_EAST,
                S_NORTH,
                S_FLIP_EAST,
                S_FLIP_NORTH,
                S_EAST,
                S_FLIP_NORTH,
                S_EAST,
                S_NORTH,
            ],
            [
                S_FLIP_EAST,
                S_SOUTH,
                S_FLIP_EAST,
                S_FLIP_SOUTH,
                S_EAST,
                S_FLIP_SOUTH,
                S_EAST,
                S_SOUTH,
            ],
        ],
    },
    Adjacency {
        corners: [EAST, WEST, NORTH, SOUTH],
        weights: [
            [
                S_EAST,
                S_SOUTH,
                S_EAST,
                S_FLIP_SOUTH,
                S_FLIP_EAST,
                S_FLIP_SOUTH,
                S_FLIP_EAST,
                S_SOUTH,
            ],
            [
                S_EAST,
                S_NORTH,
                S_EAST,
                S_FLIP_NORTH,
                S_FLIP_EAST,
                S_FLIP_NORTH,
                S_FLIP_EAST,
                S_NORTH,
            ],
            [
                S_WEST,
                S_NORTH,
                S_WEST,
                S_FLIP_NORTH,
                S_FLIP_WEST,
                S_FLIP_NORTH,
                S_FLIP_WEST,
                S_NORTH,
            ],
            [
                S_WEST,
                S_SOUTH,
                S_WEST,
                S_FLIP_SOUTH,
                S_FLIP_WEST,
                S_FLIP_SOUTH,
                S_FLIP_WEST,
                S_SOUTH,
            ],
        ],
    },
    Adjacency {
        corners: [UP, DOWN, EAST, WEST],
        weights: [
            [
                S_UP,
                S_FLIP_WEST,
                S_UP,
                S_WEST,
                S_FLIP_UP,
                S_WEST,
                S_FLIP_UP,
                S_FLIP_WEST,
            ],
            [
                S_UP,
                S_FLIP_EAST,
                S_UP,
                S_EAST,
                S_FLIP_UP,
                S_EAST,
                S_FLIP_UP,
                S_FLIP_EAST,
            ],
            [
                S_DOWN,
                S_FLIP_EAST,
                S_DOWN,
                S_EAST,
                S_FLIP_DOWN,
                S_EAST,
                S_FLIP_DOWN,
                S_FLIP_EAST,
            ],
            [
                S_DOWN,
                S_FLIP_WEST,
                S_DOWN,
                S_WEST,
                S_FLIP_DOWN,
                S_WEST,
                S_FLIP_DOWN,
                S_FLIP_WEST,
            ],
        ],
    },
    Adjacency {
        corners: [WEST, EAST, DOWN, UP],
        weights: [
            [
                S_UP,
                S_FLIP_WEST,
                S_FLIP_UP,
                S_FLIP_WEST,
                S_FLIP_UP,
                S_WEST,
                S_UP,
                S_WEST,
            ],
            [
                S_DOWN,
                S_FLIP_WEST,
                S_FLIP_DOWN,
                S_FLIP_WEST,
                S_FLIP_DOWN,
                S_WEST,
                S_DOWN,
                S_WEST,
            ],
            [
                S_DOWN,
                S_FLIP_EAST,
                S_FLIP_DOWN,
                S_FLIP_EAST,
                S_FLIP_DOWN,
                S_EAST,
                S_DOWN,
                S_EAST,
            ],
            [
                S_UP,
                S_FLIP_EAST,
                S_FLIP_UP,
                S_FLIP_EAST,
                S_FLIP_UP,
                S_EAST,
                S_UP,
                S_EAST,
            ],
        ],
    },
    Adjacency {
        corners: [UP, DOWN, NORTH, SOUTH],
        weights: [
            [
                S_UP,
                S_SOUTH,
                S_UP,
                S_FLIP_SOUTH,
                S_FLIP_UP,
                S_FLIP_SOUTH,
                S_FLIP_UP,
                S_SOUTH,
            ],
            [
                S_UP,
                S_NORTH,
                S_UP,
                S_FLIP_NORTH,
                S_FLIP_UP,
                S_FLIP_NORTH,
                S_FLIP_UP,
                S_NORTH,
            ],
            [
                S_DOWN,
                S_NORTH,
                S_DOWN,
                S_FLIP_NORTH,
                S_FLIP_DOWN,
                S_FLIP_NORTH,
                S_FLIP_DOWN,
                S_NORTH,
            ],
            [
                S_DOWN,
                S_SOUTH,
                S_DOWN,
                S_FLIP_SOUTH,
                S_FLIP_DOWN,
                S_FLIP_SOUTH,
                S_FLIP_DOWN,
                S_SOUTH,
            ],
        ],
    },
    Adjacency {
        corners: [DOWN, UP, NORTH, SOUTH],
        weights: [
            [
                S_FLIP_DOWN,
                S_SOUTH,
                S_FLIP_DOWN,
                S_FLIP_SOUTH,
                S_DOWN,
                S_FLIP_SOUTH,
                S_DOWN,
                S_SOUTH,
            ],
            [
                S_FLIP_DOWN,
                S_NORTH,
                S_FLIP_DOWN,
                S_FLIP_NORTH,
                S_DOWN,
                S_FLIP_NORTH,
                S_DOWN,
                S_NORTH,
            ],
            [
                S_FLIP_UP,
                S_NORTH,
                S_FLIP_UP,
                S_FLIP_NORTH,
                S_UP,
                S_FLIP_NORTH,
                S_UP,
                S_NORTH,
            ],
            [
                S_FLIP_UP,
                S_SOUTH,
                S_FLIP_UP,
                S_FLIP_SOUTH,
                S_UP,
                S_FLIP_SOUTH,
                S_UP,
                S_SOUTH,
            ],
        ],
    },
];

const REMAP: [[usize; 4]; 6] = [
    [0, 1, 2, 3],
    [2, 3, 0, 1],
    [3, 0, 1, 2],
    [0, 1, 2, 3],
    [3, 0, 1, 2],
    [1, 2, 3, 0],
];

pub struct QuadLight {
    pub shade: [f32; 4],
    pub face_shade: f32,
    pub light: [[f32; 2]; 4],
}

impl QuadLight {
    pub fn flat(face_shade: f32, light: u32) -> Self {
        let l = [
            (coords::block(light) * 16) as f32,
            (coords::sky(light) * 16) as f32,
        ];
        Self {
            shade: [1.0; 4],
            face_shade,
            light: [l; 4],
        }
    }
}

#[inline]
pub fn light_coords(occ: &Occupancy, x: i32, y: i32, z: i32) -> u32 {
    if occ.is_emissive(x, y, z) {
        return coords::FULL_BRIGHT;
    }
    let packed = coords::pack(occ.block_light(x, y, z), occ.sky_light(x, y, z));
    let emission = occ.emission(x, y, z);
    if coords::block(packed) < emission as u32 {
        coords::with_block(packed, emission)
    } else {
        packed
    }
}

#[inline]
fn shade_brightness(occ: &Occupancy, x: i32, y: i32, z: i32) -> f32 {
    if occ.is_full_cube(x, y, z) { 0.2 } else { 1.0 }
}

struct QuadShape {
    face_shape: [f32; 12],
    cubic: bool,
    partial: bool,
}

fn quad_shape(pos: &[[f32; 3]; 4], dir: usize, collision_full: bool) -> QuadShape {
    let (mut min, mut max) = ([32.0f32; 3], [-32.0f32; 3]);
    for p in pos {
        for a in 0..3 {
            min[a] = min[a].min(p[a]);
            max[a] = max[a].max(p[a]);
        }
    }
    let mut face_shape = [0.0f32; 12];
    face_shape[S_WEST] = min[0];
    face_shape[S_EAST] = max[0];
    face_shape[S_DOWN] = min[1];
    face_shape[S_UP] = max[1];
    face_shape[S_NORTH] = min[2];
    face_shape[S_SOUTH] = max[2];
    face_shape[S_FLIP_WEST] = 1.0 - min[0];
    face_shape[S_FLIP_EAST] = 1.0 - max[0];
    face_shape[S_FLIP_DOWN] = 1.0 - min[1];
    face_shape[S_FLIP_UP] = 1.0 - max[1];
    face_shape[S_FLIP_NORTH] = 1.0 - min[2];
    face_shape[S_FLIP_SOUTH] = 1.0 - max[2];

    const LO: f32 = 1.0e-4;
    const HI: f32 = 0.9999;
    let partial = match dir {
        DOWN | UP => min[0] >= LO || min[2] >= LO || max[0] <= HI || max[2] <= HI,
        NORTH | SOUTH => min[0] >= LO || min[1] >= LO || max[0] <= HI || max[1] <= HI,
        _ => min[1] >= LO || min[2] >= LO || max[1] <= HI || max[2] <= HI,
    };
    let cubic = match dir {
        DOWN => min[1] == max[1] && (min[1] < LO || collision_full),
        UP => min[1] == max[1] && (max[1] > HI || collision_full),
        NORTH => min[2] == max[2] && (min[2] < LO || collision_full),
        SOUTH => min[2] == max[2] && (max[2] > HI || collision_full),
        WEST => min[0] == max[0] && (min[0] < LO || collision_full),
        _ => min[0] == max[0] && (max[0] > HI || collision_full),
    };
    QuadShape {
        face_shape,
        cubic,
        partial,
    }
}

pub fn quad_is_cubic(pos: &[[f32; 3]; 4], dir: usize, collision_full: bool) -> bool {
    quad_shape(pos, dir, collision_full).cubic
}

#[inline]
pub fn offset(p: (i32, i32, i32), dir: usize) -> (i32, i32, i32) {
    let (dx, dy, dz) = OFFSETS[dir];
    (p.0 + dx, p.1 + dy, p.2 + dz)
}

pub fn ambient_occlusion(
    occ: &Occupancy,
    center: (i32, i32, i32),
    dir: usize,
    pos: &[[f32; 3]; 4],
    face_shade: f32,
) -> QuadLight {
    let shape = quad_shape(pos, dir, occ.is_full_cube(center.0, center.1, center.2));
    let base = if shape.cubic {
        offset(center, dir)
    } else {
        center
    };
    let info = &ADJACENCY[dir];

    let mut n = [(0i32, 0i32, 0i32); 4];
    let mut light = [0u32; 4];
    let mut shade = [0.0f32; 4];
    for i in 0..4 {
        n[i] = offset(base, info.corners[i]);
        light[i] = light_coords(occ, n[i].0, n[i].1, n[i].2);
        shade[i] = shade_brightness(occ, n[i].0, n[i].1, n[i].2);
    }

    let mut translucent = [false; 4];
    for i in 0..4 {
        let p = offset(n[i], dir);
        translucent[i] = !occ.is_solid(p.0, p.1, p.2);
    }

    let diagonal = |a: usize, b: usize| -> (f32, u32) {
        if !translucent[b] && !translucent[a] {
            (shade[0], light[0])
        } else {
            let p = offset(n[a], info.corners[b]);
            (
                shade_brightness(occ, p.0, p.1, p.2),
                light_coords(occ, p.0, p.1, p.2),
            )
        }
    };
    let (shade_02, light_02) = diagonal(0, 2);
    let (shade_03, light_03) = diagonal(0, 3);
    let (shade_12, light_12) = diagonal(1, 2);
    let (shade_13, light_13) = diagonal(1, 3);

    let mut light_center = light_coords(occ, center.0, center.1, center.2);
    let next = offset(center, dir);
    if shape.cubic || !occ.is_solid(next.0, next.1, next.2) {
        light_center = light_coords(occ, next.0, next.1, next.2);
    }
    let shade_center = shade_brightness(occ, base.0, base.1, base.2);

    let level = [
        (shade[3] + shade[0] + shade_03 + shade_center) * 0.25,
        (shade[2] + shade[0] + shade_02 + shade_center) * 0.25,
        (shade[2] + shade[1] + shade_12 + shade_center) * 0.25,
        (shade[3] + shade[1] + shade_13 + shade_center) * 0.25,
    ];
    let blended = [
        coords::smooth_blend(light[3], light[0], light_03, light_center),
        coords::smooth_blend(light[2], light[0], light_02, light_center),
        coords::smooth_blend(light[2], light[1], light_12, light_center),
        coords::smooth_blend(light[3], light[1], light_13, light_center),
    ];

    let remap = REMAP[dir];
    let mut out = QuadLight {
        shade: [0.0; 4],
        face_shade,
        light: [[0.0; 2]; 4],
    };
    if shape.partial {
        for (v, slots) in info.weights.iter().enumerate() {
            let w = [
                shape.face_shape[slots[0]] * shape.face_shape[slots[1]],
                shape.face_shape[slots[2]] * shape.face_shape[slots[3]],
                shape.face_shape[slots[4]] * shape.face_shape[slots[5]],
                shape.face_shape[slots[6]] * shape.face_shape[slots[7]],
            ];
            let ao = (0..4).map(|i| level[i] * w[i]).sum::<f32>().clamp(0.0, 1.0);
            let packed = coords::smooth_weighted_blend(blended, w);
            out.shade[remap[v]] = ao;
            out.light[remap[v]] = [
                coords::smooth_block(packed) as f32,
                coords::smooth_sky(packed) as f32,
            ];
        }
    } else {
        for v in 0..4 {
            out.shade[remap[v]] = level[v];
            out.light[remap[v]] = [
                coords::smooth_block(blended[v]) as f32,
                coords::smooth_sky(blended[v]) as f32,
            ];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::coords::*;

    #[test]
    fn the_packed_forms_round_trip() {
        let p = pack(11, 4);
        assert_eq!(block(p), 11);
        assert_eq!(sky(p), 4);
    }

    #[test]
    fn blending_four_full_samples_gives_the_smooth_maximum() {
        let full = pack(15, 15);
        let out = smooth_blend(full, full, full, full);
        assert_eq!(smooth_block(out), 240);
        assert_eq!(smooth_sky(out), 240);
    }

    #[test]
    fn a_zero_neighbour_inherits_a_lit_centre() {
        let center = pack(10, 10);
        let dark = pack(0, 0);
        let out = smooth_blend(dark, dark, dark, center);
        assert_eq!(smooth_block(out), 160);
        assert_eq!(smooth_sky(out), 160);
    }
}

#[cfg(test)]
mod orientation {
    use super::*;
    use crate::renderer::Occupancy;

    fn scene(occluders: &[(i32, i32, i32)]) -> Occupancy {
        let mut occ = Occupancy::new(0, 0, -2, 6);
        for (x, y, z) in occluders {
            occ.set_solid(*x, *y, *z);
            occ.set_full_cube(*x, *y, *z);
        }
        occ
    }

    const UP_FACE: [[f32; 3]; 4] = [
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 1.0],
        [1.0, 1.0, 1.0],
        [1.0, 1.0, 0.0],
    ];

    #[test]
    fn an_occluder_darkens_the_corners_nearest_it() {
        let occ = scene(&[(0, 0, 0), (1, 1, 0)]);
        let lit = ambient_occlusion(&occ, (0, 0, 0), UP, &UP_FACE, 1.0);
        for (i, corner) in UP_FACE.iter().enumerate() {
            let east = corner[0] > 0.5;
            if east {
                assert!(
                    lit.shade[i] < 0.9,
                    "vertex {i} at {corner:?} should be shaded"
                );
            } else {
                assert!(
                    lit.shade[i] > 0.99,
                    "vertex {i} at {corner:?} should be clear"
                );
            }
        }
    }

    #[test]
    fn moving_the_occluder_moves_the_shadow_with_it() {
        let occ = scene(&[(0, 0, 0), (0, 1, -1)]);
        let lit = ambient_occlusion(&occ, (0, 0, 0), UP, &UP_FACE, 1.0);
        for (i, corner) in UP_FACE.iter().enumerate() {
            let north = corner[2] < 0.5;
            if north {
                assert!(
                    lit.shade[i] < 0.9,
                    "vertex {i} at {corner:?} should be shaded"
                );
            } else {
                assert!(
                    lit.shade[i] > 0.99,
                    "vertex {i} at {corner:?} should be clear"
                );
            }
        }
    }

    #[test]
    fn a_side_face_is_shaded_from_above() {
        const NORTH_FACE: [[f32; 3]; 4] = [
            [1.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let occ = scene(&[(0, 0, 0), (0, 1, -1)]);
        let lit = ambient_occlusion(&occ, (0, 0, 0), NORTH, &NORTH_FACE, 1.0);
        for (i, corner) in NORTH_FACE.iter().enumerate() {
            let top = corner[1] > 0.5;
            if top {
                assert!(
                    lit.shade[i] < 0.9,
                    "vertex {i} at {corner:?} should be shaded"
                );
            } else {
                assert!(
                    lit.shade[i] > 0.99,
                    "vertex {i} at {corner:?} should be clear"
                );
            }
        }
    }
}
