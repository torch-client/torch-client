use std::collections::HashSet;

use super::random::WorldgenRandom;
use super::table::{Biomes, Count, Height, Kind, Row, Target};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    Air,
    Decor,
    Fluid,
    Tree,
    Stone,
    Deepslate,
    Netherrack,
    NetherBase,
    Buried,
    Ore(Kind),
    Other,
}

impl Class {
    fn was_target(self, target: Target) -> bool {
        match (self, target) {
            (Class::Ore(_), _) => true,
            (Class::Stone | Class::Deepslate | Class::Buried, Target::Overworld) => true,
            (Class::Netherrack, Target::Netherrack | Target::NetherBase) => true,
            (Class::NetherBase, Target::NetherBase) => true,
            _ => false,
        }
    }

    fn was_air(self) -> bool {
        matches!(self, Class::Air | Class::Decor)
    }

    pub fn holds_ore(self) -> bool {
        matches!(
            self,
            Class::Stone
                | Class::Deepslate
                | Class::Netherrack
                | Class::NetherBase
                | Class::Buried
                | Class::Ore(_)
        )
    }
}

pub trait Terrain {
    fn class(&self, x: i32, y: i32, z: i32) -> Class;
    fn floor(&self, x: i32, z: i32) -> i32;
    fn has_feature(&self, row: usize, x: i32, y: i32, z: i32) -> bool;
    fn outside(&self, y: i32) -> bool;
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Placed {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub kind: Kind,
}

fn mth_sin(v: f64) -> f32 {
    const SCALE: f64 = 10430.378350470453;
    let i = (v * SCALE) as i64 & 65535;
    (i as f64 / SCALE).sin() as f32
}

fn java_round(v: f32) -> i32 {
    (v as f64 + 0.5).floor() as i32
}

fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

fn sample_height(height: Height, r: &mut WorldgenRandom) -> i32 {
    match height {
        Height::Uniform(min, max) => r.between(min, max),
        Height::Trapezoid(min, max) => {
            let range = max - min;
            let start = range / 2;
            let end = range - start;
            let a = r.between(0, end);
            let b = r.between(0, start);
            min + a + b
        }
    }
}

fn can_place(
    t: &impl Terrain,
    r: &mut WorldgenRandom,
    row: &Row,
    overlay: &HashSet<[i32; 3]>,
    p: [i32; 3],
) -> bool {
    if overlay.contains(&p) || !t.class(p[0], p[1], p[2]).was_target(row.target) {
        return false;
    }
    let skip_air_check = if row.discard <= 0.0 {
        true
    } else if row.discard >= 1.0 {
        false
    } else {
        r.next_float() >= row.discard
    };
    if skip_air_check {
        return true;
    }
    const SIDES: [[i32; 3]; 6] = [
        [0, -1, 0],
        [0, 1, 0],
        [0, 0, -1],
        [0, 0, 1],
        [-1, 0, 0],
        [1, 0, 0],
    ];
    !SIDES
        .iter()
        .any(|d| t.class(p[0] + d[0], p[1] + d[1], p[2] + d[2]).was_air())
}

#[derive(Default)]
struct Tested(Vec<u64>);

impl Tested {
    fn test_and_set(&mut self, i: usize) -> bool {
        let (word, bit) = (i / 64, 1u64 << (i % 64));
        if word >= self.0.len() {
            self.0.resize(word + 1, 0);
        }
        let seen = self.0[word] & bit != 0;
        self.0[word] |= bit;
        seen
    }
}

fn place_vein(
    t: &impl Terrain,
    r: &mut WorldgenRandom,
    row: &Row,
    origin: [i32; 3],
    overlay: &mut HashSet<[i32; 3]>,
    out: &mut Vec<Placed>,
) {
    let size = row.size;
    let dir = r.next_float() * 3.141_592_7_f32;
    let spread = size as f32 / 8.0;
    let max_radius = ((size as f32 / 16.0 * 2.0 + 1.0) / 2.0).ceil() as i32;
    let (sin, cos) = ((dir as f64).sin(), (dir as f64).cos());
    let x0 = origin[0] as f64 + sin * spread as f64;
    let x1 = origin[0] as f64 - sin * spread as f64;
    let z0 = origin[2] as f64 + cos * spread as f64;
    let z1 = origin[2] as f64 - cos * spread as f64;
    let y0 = (origin[1] + r.next_int(3) - 2) as f64;
    let y1 = (origin[1] + r.next_int(3) - 2) as f64;
    let reach = spread.ceil() as i32 + max_radius;
    let (x_start, y_start, z_start) = (
        origin[0] - reach,
        origin[1] - 2 - max_radius,
        origin[2] - reach,
    );
    let size_xz = 2 * reach;
    let size_y = 2 * (2 + max_radius);

    let reaches_ground = (x_start..=x_start + size_xz)
        .any(|x| (z_start..=z_start + size_xz).any(|z| y_start <= t.floor(x, z)));
    if !reaches_ground {
        return;
    }

    let n = size as usize;
    let mut data = vec![[0.0f64; 4]; n];
    for (i, d) in data.iter_mut().enumerate() {
        let step = i as f32 / size as f32;
        let thickness = r.next_double() * size as f64 / 16.0;
        let radius =
            ((mth_sin((3.141_592_7_f32 * step) as f64) + 1.0) as f64 * thickness + 1.0) / 2.0;
        let step = step as f64;
        *d = [
            lerp(step, x0, x1),
            lerp(step, y0, y1),
            lerp(step, z0, z1),
            radius,
        ];
    }

    for i in 0..n.saturating_sub(1) {
        if data[i][3] <= 0.0 {
            continue;
        }
        for j in i + 1..n {
            if data[j][3] <= 0.0 {
                continue;
            }
            let dx = data[i][0] - data[j][0];
            let dy = data[i][1] - data[j][1];
            let dz = data[i][2] - data[j][2];
            let dr = data[i][3] - data[j][3];
            if dr * dr > dx * dx + dy * dy + dz * dz {
                if dr > 0.0 {
                    data[j][3] = -1.0;
                } else {
                    data[i][3] = -1.0;
                }
            }
        }
    }

    let mut tested = Tested::default();
    for &[xx, yy, zz, radius] in &data {
        if radius < 0.0 {
            continue;
        }
        let x_min = ((xx - radius).floor() as i32).max(x_start);
        let y_min = ((yy - radius).floor() as i32).max(y_start);
        let z_min = ((zz - radius).floor() as i32).max(z_start);
        let x_max = ((xx + radius).floor() as i32).max(x_min);
        let y_max = ((yy + radius).floor() as i32).max(y_min);
        let z_max = ((zz + radius).floor() as i32).max(z_min);

        for x in x_min..=x_max {
            let xd = (x as f64 + 0.5 - xx) / radius;
            if xd * xd >= 1.0 {
                continue;
            }
            for y in y_min..=y_max {
                let yd = (y as f64 + 0.5 - yy) / radius;
                if xd * xd + yd * yd >= 1.0 {
                    continue;
                }
                for z in z_min..=z_max {
                    let zd = (z as f64 + 0.5 - zz) / radius;
                    if xd * xd + yd * yd + zd * zd >= 1.0 || t.outside(y) {
                        continue;
                    }
                    let bit =
                        x - x_start + (y - y_start) * size_xz + (z - z_start) * size_xz * size_y;
                    if tested.test_and_set(bit as usize) {
                        continue;
                    }
                    if can_place(t, r, row, overlay, [x, y, z]) {
                        overlay.insert([x, y, z]);
                        out.push(Placed {
                            x,
                            y,
                            z,
                            kind: row.kind,
                        });
                    }
                }
            }
        }
    }
}

fn place_scattered(
    t: &impl Terrain,
    r: &mut WorldgenRandom,
    row: &Row,
    origin: [i32; 3],
    overlay: &mut HashSet<[i32; 3]>,
    out: &mut Vec<Placed>,
) {
    let tries = r.next_int(row.size + 1);
    for i in 0..tries {
        let spread = i.min(7) as f32;
        let mut axis = || java_round((r.next_float() - r.next_float()) * spread);
        let (dx, dy, dz) = (axis(), axis(), axis());
        let p = [origin[0] + dx, origin[1] + dy, origin[2] + dz];
        if can_place(t, r, row, overlay, p) {
            overlay.insert(p);
            out.push(Placed {
                x: p[0],
                y: p[1],
                z: p[2],
                kind: row.kind,
            });
        }
    }
}

pub fn simulate(
    seed: i64,
    rows: &[Row],
    chunk_x: i32,
    chunk_z: i32,
    t: &impl Terrain,
) -> Vec<Placed> {
    let (bx, bz) = (chunk_x * 16, chunk_z * 16);
    let mut r = WorldgenRandom::new();
    let decoration = r.set_decoration_seed(seed, bx, bz);
    let mut overlay = HashSet::new();
    let mut out = Vec::new();

    for (i, row) in rows.iter().enumerate() {
        r.set_feature_seed(decoration, row.index, row.step);
        let attempts = match row.count {
            Count::Once | Count::Rarity(_) => 1,
            Count::Fixed(n) => n,
            Count::Uniform(min, max) => r.between(min, max),
        };
        for _ in 0..attempts {
            if let Count::Rarity(chance) = row.count
                && r.next_float() >= 1.0 / chance as f32
            {
                continue;
            }
            let x = r.next_int(16) + bx;
            let z = r.next_int(16) + bz;
            let y = sample_height(row.height, &mut r);
            if !matches!(row.biomes, Biomes::All) && !t.has_feature(i, x, y, z) {
                continue;
            }
            if row.scattered {
                place_scattered(t, &mut r, row, [x, y, z], &mut overlay, &mut out);
            } else {
                place_vein(t, &mut r, row, [x, y, z], &mut overlay, &mut out);
            }
        }
    }
    out
}
