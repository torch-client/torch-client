use std::sync::atomic::{AtomicBool, Ordering};

use super::lighter::{self, QuadLight};
use super::{BlockGeom, Occupancy, RenderedBlock};
use crate::TEXTURE_MAP;
use crate::blocks::bake::tile_uv_rect;
use crate::blocks::rand::position_seed;
use crate::util::javarandom::JavaRandom;

static SMOOTH_LIGHTING: AtomicBool = AtomicBool::new(true);

pub fn smooth_lighting_enabled() -> bool {
    SMOOTH_LIGHTING.load(Ordering::Relaxed)
}

pub fn set_smooth_lighting(enabled: bool) {
    SMOOTH_LIGHTING.store(enabled, Ordering::Relaxed);
}

static LIGHTING_ENABLED: AtomicBool = AtomicBool::new(true);

pub fn lighting_enabled() -> bool {
    LIGHTING_ENABLED.load(Ordering::Relaxed)
}

pub fn set_lighting_enabled(enabled: bool) {
    LIGHTING_ENABLED.store(enabled, Ordering::Relaxed);
}

fn uv_horizontal(x0: f32, z0: f32, x1: f32, z1: f32, tile: u32, rows: u32) -> [[f32; 2]; 4] {
    let [au0, av0, au1, av1] = tile_uv_rect(tile, rows);
    let u0 = lerp(au0, au1, x0);
    let u1 = lerp(au0, au1, x1);
    let v0 = lerp(av0, av1, z0);
    let v1 = lerp(av0, av1, z1);
    [[u0, v0], [u1, v0], [u1, v1], [u0, v1]]
}

fn uv_ns(x0: f32, x1: f32, y0: f32, y1: f32, tile: u32, rows: u32) -> [[f32; 2]; 4] {
    let [au0, av0, au1, av1] = tile_uv_rect(tile, rows);
    let u0 = lerp(au0, au1, x0);
    let u1 = lerp(au0, au1, x1);
    let fv_top = lerp(av0, av1, 1.0 - y1);
    let fv_bot = lerp(av0, av1, 1.0 - y0);
    [[u0, fv_bot], [u1, fv_bot], [u1, fv_top], [u0, fv_top]]
}

fn uv_ew(z0: f32, z1: f32, y0: f32, y1: f32, tile: u32, rows: u32) -> [[f32; 2]; 4] {
    let [au0, av0, au1, av1] = tile_uv_rect(tile, rows);
    let u0 = lerp(au0, au1, z0);
    let u1 = lerp(au0, au1, z1);
    let fv_top = lerp(av0, av1, 1.0 - y1);
    let fv_bot = lerp(av0, av1, 1.0 - y0);
    [[u0, fv_bot], [u1, fv_bot], [u1, fv_top], [u0, fv_top]]
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PackedVertex {
    pub pos: [i16; 4],
    pub uv: [u16; 2],
    pub light: [u8; 4],
    pub color: [u8; 4],
}

const _: () = assert!(std::mem::size_of::<PackedVertex>() == 20);

#[derive(Default)]
pub struct MeshBuf {
    pub verts: Vec<PackedVertex>,
    pub idx: Vec<u32>,
    pub cutout_idx: Vec<u32>,
    pub min: [i16; 3],
    pub max: [i16; 3],
    #[cfg(feature = "shader_support")]
    pub pack: Vec<super::packvertex::PackQuad>,
    #[cfg(feature = "shader_support")]
    pub pack_block: Option<super::packvertex::PackBlock>,
}

#[cfg(feature = "shader_support")]
fn fluid_face(corners: &[[f32; 3]; 4]) -> usize {
    const DOWN: usize = 0;
    const UP: usize = 1;
    const NORTH: usize = 2;
    const SOUTH: usize = 3;
    const WEST: usize = 4;
    const EAST: usize = 5;
    let e1 = [0, 1, 2].map(|a| corners[1][a] - corners[0][a]);
    let e3 = [0, 1, 2].map(|a| corners[3][a] - corners[0][a]);
    let n = [
        e3[1] * e1[2] - e3[2] * e1[1],
        e3[2] * e1[0] - e3[0] * e1[2],
        e3[0] * e1[1] - e3[1] * e1[0],
    ];
    let (ax, ay, az) = (n[0].abs(), n[1].abs(), n[2].abs());
    if ay >= ax && ay >= az {
        if n[1] >= 0.0 { UP } else { DOWN }
    } else if ax >= az {
        if n[0] >= 0.0 { EAST } else { WEST }
    } else if n[2] >= 0.0 {
        SOUTH
    } else {
        NORTH
    }
}

#[inline]
fn pack_uv(uv: [f32; 2]) -> [u16; 2] {
    [
        (uv[0].clamp(0.0, 1.0) * 65535.0).round() as u16,
        (uv[1].clamp(0.0, 1.0) * 65535.0).round() as u16,
    ]
}

pub const MATERIAL_SOLID: u8 = 0;
pub const MATERIAL_LEAF: u8 = 1;
pub const MATERIAL_PLANT: u8 = 2;
pub const MATERIAL_WATER: u8 = 3;
pub const MATERIAL_LAVA: u8 = 4;
pub const MATERIAL_EMISSIVE_BASE: u8 = 5;

#[inline]
fn pack_light(light: [f32; 2], material: u8, anchor: f32) -> [u8; 4] {
    [
        light[0].clamp(0.0, 255.0).round() as u8,
        light[1].clamp(0.0, 255.0).round() as u8,
        material,
        (anchor.clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

#[inline]
fn pack_color(c: [f32; 4]) -> [u8; 4] {
    let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [ch(c[0]), ch(c[1]), ch(c[2]), ch(c[3])]
}

fn shade_to_linear(shade: f32) -> f32 {
    if shade <= 0.04045 {
        shade / 12.92
    } else {
        ((shade + 0.055) / 1.055).powf(2.4)
    }
}

static SRGB_TO_LINEAR: std::sync::LazyLock<[f32; 256]> = std::sync::LazyLock::new(|| {
    let mut t = [0.0f32; 256];
    for (i, v) in t.iter_mut().enumerate() {
        *v = shade_to_linear(i as f32 / 255.0);
    }
    t
});

#[inline]
fn shade_to_linear_q(shade: f32) -> f32 {
    SRGB_TO_LINEAR[(shade.clamp(0.0, 1.0) * 255.0).round() as usize]
}

impl MeshBuf {
    fn new() -> Self {
        Self {
            min: [i16::MAX; 3],
            max: [i16::MIN; 3],
            ..Default::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.verts.is_empty()
    }

    pub fn index_count(&self) -> usize {
        self.idx.len() + self.cutout_idx.len()
    }

    pub fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        (
            [
                self.min[0] as f32 / 256.0,
                self.min[1] as f32 / 256.0,
                self.min[2] as f32 / 256.0,
            ],
            [
                self.max[0] as f32 / 256.0,
                self.max[1] as f32 / 256.0,
                self.max[2] as f32 / 256.0,
            ],
        )
    }

    fn shrink_to_fit(&mut self) {
        self.verts.shrink_to_fit();
        #[cfg(feature = "shader_support")]
        self.pack.shrink_to_fit();
        self.idx.shrink_to_fit();
        self.cutout_idx.shrink_to_fit();
    }

    fn push_vertex(&mut self, pos: [f32; 3], uv: [u16; 2], light: [u8; 4], color: [u8; 4]) {
        let q = |v: f32| (v * 256.0).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        let p = [q(pos[0]), q(pos[1]), q(pos[2])];
        for axis in 0..3 {
            self.min[axis] = self.min[axis].min(p[axis]);
            self.max[axis] = self.max[axis].max(p[axis]);
        }
        self.verts.push(PackedVertex {
            pos: [p[0], p[1], p[2], 0],
            uv,
            light,
            color,
        });
    }

    fn push_quad(
        &mut self,
        corners: &[[f32; 3]; 4],
        face_uvs: &[[f32; 2]; 4],
        light: [f32; 2],
        tint: [f32; 3],
        material: u8,
    ) {
        let base = self.verts.len() as u32;
        let light = pack_light(light, material, 1.0);
        let col = pack_color([tint[0], tint[1], tint[2], 1.0]);
        #[cfg(feature = "shader_support")]
        let col = match &self.pack_block {
            Some(block) => pack_color(block.fluid_color(block.tint, fluid_face(corners))),
            None => col,
        };
        for i in 0..4 {
            self.push_vertex(corners[i], pack_uv(face_uvs[i]), light, col);
        }
        #[cfg(feature = "shader_support")]
        if let Some(block) = &self.pack_block {
            let e1 = [
                corners[1][0] - corners[0][0],
                corners[1][1] - corners[0][1],
                corners[1][2] - corners[0][2],
            ];
            let e3 = [
                corners[3][0] - corners[0][0],
                corners[3][1] - corners[0][1],
                corners[3][2] - corners[0][2],
            ];
            let normal = [
                e3[1] * e1[2] - e3[2] * e1[1],
                e3[2] * e1[0] - e3[0] * e1[2],
                e3[0] * e1[1] - e3[1] * e1[0],
            ];
            self.pack
                .push(super::packvertex::quad(block, corners, face_uvs, normal));
        }
        self.idx
            .extend_from_slice(&[base + 2, base + 1, base, base + 3, base + 2, base]);
    }

    #[inline]
    fn push_baked(
        &mut self,
        quad: &crate::blocks::BakedQuad,
        wx: f32,
        wy: f32,
        wz: f32,
        lit: &QuadLight,
        tint: [f32; 3],
        material: u8,
    ) {
        let base = self.verts.len() as u32;
        let shade = shade_to_linear_q(lit.face_shade);
        for i in 0..4 {
            let p = quad.pos[i];
            let ao = shade_to_linear_q(lit.shade[i]);
            let color = pack_color([ao * tint[0], ao * tint[1], ao * tint[2], shade]);
            #[cfg(feature = "shader_support")]
            let color = match &self.pack_block {
                Some(block) => {
                    let quad_tint = if quad.tint >= 0 { block.tint } else { [1.0; 3] };
                    pack_color(block.color(quad_tint, lit.face_shade, lit.shade[i]))
                }
                None => color,
            };
            let light = pack_light(lit.light[i], material, p[1]);
            self.push_vertex(
                [p[0] + wx, p[1] + wy, p[2] + wz],
                pack_uv(quad.uv[i]),
                light,
                color,
            );
        }
        #[cfg(feature = "shader_support")]
        if let Some(block) = &self.pack_block {
            let corners = quad.pos.map(|p| [p[0] + wx, p[1] + wy, p[2] + wz]);
            self.pack.push(super::packvertex::quad(
                block,
                &corners,
                &quad.uv,
                quad.normal,
            ));
        }
        let idx = if quad.opaque {
            &mut self.idx
        } else {
            &mut self.cutout_idx
        };
        idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

#[allow(clippy::too_many_arguments)]
fn face_geom(
    x0: f32,
    y0: f32,
    z0: f32,
    x1: f32,
    y1: f32,
    z1: f32,
    face: usize,
    tile: u32,
    atlas_rows: u32,
) -> ([[f32; 3]; 4], [f32; 3], [[f32; 2]; 4]) {
    match face {
        0 => (
            [[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]],
            [0., 1., 0.],
            uv_horizontal(x0, z0, x1, z1, tile, atlas_rows),
        ),
        1 => (
            [[x0, y0, z1], [x1, y0, z1], [x1, y0, z0], [x0, y0, z0]],
            [0., -1., 0.],
            uv_horizontal(x0, z0, x1, z1, tile, atlas_rows),
        ),
        2 => (
            [[x1, y0, z0], [x1, y0, z1], [x1, y1, z1], [x1, y1, z0]],
            [1., 0., 0.],
            uv_ew(z0, z1, y0, y1, tile, atlas_rows),
        ),
        3 => (
            [[x0, y0, z1], [x0, y0, z0], [x0, y1, z0], [x0, y1, z1]],
            [-1., 0., 0.],
            uv_ew(z0, z1, y0, y1, tile, atlas_rows),
        ),
        4 => (
            [[x1, y0, z1], [x0, y0, z1], [x0, y1, z1], [x1, y1, z1]],
            [0., 0., 1.],
            uv_ns(x0, x1, y0, y1, tile, atlas_rows),
        ),
        _ => (
            [[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0]],
            [0., 0., -1.],
            uv_ns(x0, x1, y0, y1, tile, atlas_rows),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_face(
    wx: f32,
    wy: f32,
    wz: f32,
    x0: f32,
    y0: f32,
    z0: f32,
    x1: f32,
    y1: f32,
    z1: f32,
    face: usize,
    tile: u32,
    atlas_rows: u32,
    light: [f32; 2],
    tint: [f32; 3],
    material: u8,
    buf: &mut MeshBuf,
) {
    let (mut corners, _normal, face_uvs) =
        face_geom(x0, y0, z0, x1, y1, z1, face, tile, atlas_rows);
    for c in &mut corners {
        c[0] += wx;
        c[1] += wy;
        c[2] += wz;
    }
    buf.push_quad(&corners, &face_uvs, light, tint, material);
}

const CULL_NBR: [(i32, i32, i32); 6] = [
    (0, -1, 0),
    (0, 1, 0),
    (0, 0, -1),
    (0, 0, 1),
    (-1, 0, 0),
    (1, 0, 0),
];

pub fn build_section_mesh(
    blocks: &[(i32, i32, i32, RenderedBlock)],
    x_base: i32,
    y_base: i32,
    z_base: i32,
    atlas_rows: u32,
    occ: &Occupancy,
) -> (MeshBuf, MeshBuf) {
    let mut buf = MeshBuf::new();
    let mut water = MeshBuf::new();
    let cardinal = super::dimension::current().cardinal();
    let (water_still, water_flow) = {
        let m = TEXTURE_MAP.get();
        let still = m.and_then(|m| m.get("water_still").copied()).unwrap_or(0);
        let flow = m.and_then(|m| m.get("water_flow").copied()).unwrap_or(0);
        (still, flow)
    };

    #[cfg(feature = "shader_support")]
    let pack_ids = super::packvertex::current();
    #[cfg(feature = "shader_support")]
    let water_pack_id = pack_ids.as_ref().map(|ids| {
        ids.of(
            azalea::block::BlockState::from(azalea_registry::builtin::BlockKind::Water).id() as u16,
        )
    });

    for (bx, by, bz, rb) in blocks {
        let (wx, wy, wz) = (
            (*bx - x_base) as f32,
            (*by - y_base) as f32,
            (*bz - z_base) as f32,
        );
        #[cfg(feature = "shader_support")]
        if let Some(ids) = &pack_ids {
            let block = super::packvertex::PackBlock {
                id: ids.of(rb.state),
                emission: rb.emission,
                cell: [wx, wy, wz],
                tint: [1.0; 3],
                shades: *cardinal,
                lighting: ids.lighting(),
            };
            buf.pack_block = Some(block);
            water.pack_block = Some(block);
        }

        match &rb.geom {
            BlockGeom::Model(baked) => {
                #[cfg(feature = "shader_support")]
                let target = if pack_ids.is_some() && rb.translucent {
                    &mut water
                } else {
                    &mut buf
                };
                #[cfg(not(feature = "shader_support"))]
                let target = &mut buf;
                emit_baked(wx, wy, wz, rb, baked, occ, *bx, *by, *bz, cardinal, target);
            }
            BlockGeom::Fluid {
                amount,
                still,
                flow,
                lava,
            } => {
                let is_water = !*lava;
                let tint = if is_water {
                    crate::util::biome_color::water_ratio(occ.biome(*bx, *by, *bz))
                } else {
                    crate::util::biome_color::NEUTRAL
                };
                let dest = if is_water { &mut water } else { &mut buf };
                #[cfg(feature = "shader_support")]
                if let Some(block) = dest.pack_block.as_mut() {
                    block.tint = if is_water {
                        crate::util::biome_color::pack_water(occ.biome(*bx, *by, *bz))
                    } else {
                        [1.0; 3]
                    };
                }
                let material = if is_water {
                    MATERIAL_WATER
                } else {
                    MATERIAL_LAVA
                };
                emit_fluid(
                    wx, wy, wz, material, *amount, *still, *flow, atlas_rows, occ, *bx, *by, *bz,
                    tint, dest,
                );
            }
        }

        if rb.waterlogged {
            #[cfg(feature = "shader_support")]
            if let (Some(block), Some(id)) = (water.pack_block.as_mut(), water_pack_id) {
                block.id = id;
                block.emission = 0;
                block.tint = crate::util::biome_color::pack_water(occ.biome(*bx, *by, *bz));
            }
            let tint = crate::util::biome_color::water_ratio(occ.biome(*bx, *by, *bz));
            emit_fluid(
                wx,
                wy,
                wz,
                MATERIAL_WATER,
                8,
                water_still,
                water_flow,
                atlas_rows,
                occ,
                *bx,
                *by,
                *bz,
                tint,
                &mut water,
            );
        }
    }
    buf.shrink_to_fit();
    water.shrink_to_fit();
    (buf, water)
}

#[allow(clippy::too_many_arguments)]
fn emit_baked(
    wx: f32,
    wy: f32,
    wz: f32,
    rb: &RenderedBlock,
    baked: &crate::blocks::BakedBlock,
    occ: &Occupancy,
    bx: i32,
    by: i32,
    bz: i32,
    cardinal: &[f32; 6],
    buf: &mut MeshBuf,
) {
    let mut random = baked
        .randomized
        .then(|| JavaRandom::new(position_seed(bx, by, bz)));

    let [ox, oy, oz] = rb.offset.at(bx, bz);
    let (wx, wy, wz) = (wx + ox, wy + oy, wz + oz);

    let smooth = smooth_lighting_enabled() && baked.ambient_occlusion && rb.emission == 0;
    let collision_full = occ.is_full_cube(bx, by, bz);

    let tint = match rb.tint_kind {
        super::TintKind::None => crate::util::biome_color::NEUTRAL,
        super::TintKind::Grass => crate::util::biome_color::grass_ratio(occ.biome(bx, by, bz)),
        super::TintKind::Foliage => crate::util::biome_color::foliage_ratio(occ.biome(bx, by, bz)),
        super::TintKind::DryFoliage => {
            crate::util::biome_color::dry_foliage_ratio(occ.biome(bx, by, bz))
        }
        super::TintKind::Redstone(power) => crate::util::biome_color::redstone_ratio(power),
        super::TintKind::Water => crate::util::biome_color::water_ratio(occ.biome(bx, by, bz)),
    };
    #[cfg(feature = "shader_support")]
    if let Some(block) = buf.pack_block.as_mut() {
        use crate::util::biome_color as bc;
        block.tint = match rb.tint_kind {
            super::TintKind::None => [1.0; 3],
            super::TintKind::Grass => bc::pack_grass(occ.biome(bx, by, bz)),
            super::TintKind::Foliage => bc::pack_foliage(occ.biome(bx, by, bz)),
            super::TintKind::DryFoliage => bc::pack_dry_foliage(occ.biome(bx, by, bz)),
            super::TintKind::Redstone(power) => bc::redstone_ratio(power),
            super::TintKind::Water => bc::pack_water(occ.biome(bx, by, bz)),
        };
    }

    #[cfg(feature = "builtin_shaders")]
    let sways = rb.sways;
    #[cfg(not(feature = "builtin_shaders"))]
    let sways = false;
    let material = if rb.emission > 0 {
        MATERIAL_EMISSIVE_BASE + rb.emission.min(15)
    } else if rb.tint_kind == super::TintKind::Foliage && !rb.is_solid && rb.full_cube {
        MATERIAL_LEAF
    } else if sways {
        MATERIAL_PLANT
    } else {
        MATERIAL_SOLID
    };

    for part in &baked.parts {
        let model = match (&mut random, part.choices.len()) {
            (Some(random), 2..) => part.pick(random.next_int(part.total)),
            _ => &part.choices[0],
        };
        for quad in &model.quads {
            if let Some(cull) = quad.cull {
                let (dx, dy, dz) = CULL_NBR[cull as usize];
                let (nx, ny, nz) = (bx + dx, by + dy, bz + dz);
                if occ.is_solid(nx, ny, nz) {
                    continue;
                }
                let neighbour_in_group = occ.same_cull_group(nx, ny, nz, rb.cull_group);
                if rb.cull_group == crate::util::block_model::LEAVES_CULL_GROUP {
                    if neighbour_in_group && cull % 2 == 0 {
                        continue;
                    }
                } else if neighbour_in_group {
                    continue;
                }
            }
            let face_shade = quad.shade(cardinal);
            let lit = if smooth {
                lighter::ambient_occlusion(
                    occ,
                    (bx, by, bz),
                    quad.face as usize,
                    &quad.pos,
                    face_shade,
                )
            } else {
                let cubic = lighter::quad_is_cubic(&quad.pos, quad.face as usize, collision_full);
                let light_at = if cubic {
                    lighter::offset((bx, by, bz), quad.face as usize)
                } else {
                    (bx, by, bz)
                };
                QuadLight::flat(face_shade, flat_light_coords(occ, (bx, by, bz), light_at))
            };
            let quad_tint = if quad.tint >= 0 {
                tint
            } else {
                crate::util::biome_color::NEUTRAL
            };
            buf.push_baked(quad, wx, wy, wz, &lit, quad_tint, material);
        }
    }
}

fn flat_light_coords(occ: &Occupancy, state: (i32, i32, i32), pos: (i32, i32, i32)) -> u32 {
    if occ.is_emissive(state.0, state.1, state.2) {
        return lighter::coords::FULL_BRIGHT;
    }
    let packed = lighter::coords::pack(
        occ.block_light(pos.0, pos.1, pos.2),
        occ.sky_light(pos.0, pos.1, pos.2),
    );
    let emission = occ.emission(state.0, state.1, state.2);
    if lighter::coords::block(packed) < emission as u32 {
        lighter::coords::with_block(packed, emission)
    } else {
        packed
    }
}

fn fluid_flow(occ: &Occupancy, bx: i32, by: i32, bz: i32, self_height: f32) -> (f32, f32) {
    const DIRS: [(i32, i32); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];
    let mut flow_x = 0.0f32;
    let mut flow_z = 0.0f32;
    for (dx, dz) in DIRS {
        let (nx, nz) = (bx + dx, bz + dz);
        let neighbor_height = occ.fluid_own_height(nx, by, nz);
        let distance = if neighbor_height == 0.0 {
            if occ.is_solid(nx, by, nz) {
                0.0
            } else {
                let below = occ.fluid_own_height(nx, by - 1, nz);
                if below > 0.0 {
                    self_height - (below - super::fluid_surface(8))
                } else {
                    0.0
                }
            }
        } else {
            self_height - neighbor_height
        };
        if distance != 0.0 {
            flow_x += dx as f32 * distance;
            flow_z += dz as f32 * distance;
        }
    }
    (flow_x, flow_z)
}

fn flowing_top_uvs(flow_x: f32, flow_z: f32, tile: u32, ar: u32) -> [[f32; 2]; 4] {
    let [u0, v0, u1, v1] = tile_uv_rect(tile, ar);
    let get_u = |t: f32| lerp(u0, u1, t);
    let get_v = |t: f32| lerp(v0, v1, t);
    let angle = flow_z.atan2(flow_x) - std::f32::consts::FRAC_PI_2;
    let c = angle.cos() * 0.25;
    let s = angle.sin() * 0.25;
    [
        [get_u(0.5 - c - s), get_v(0.5 - c + s)],
        [get_u(0.5 + c - s), get_v(0.5 - c - s)],
        [get_u(0.5 + c + s), get_v(0.5 + c - s)],
        [get_u(0.5 - c + s), get_v(0.5 + c + s)],
    ]
}

fn corner_height_input(occ: &Occupancy, x: i32, y: i32, z: i32, lava: bool) -> f32 {
    if occ.same_fluid(x, y, z, lava) {
        if occ.same_fluid(x, y + 1, z, lava) {
            1.0
        } else {
            occ.fluid_own_height(x, y, z)
        }
    } else if occ.is_solid(x, y, z) {
        -1.0
    } else {
        0.0
    }
}

fn add_weighted(w: &mut [f32; 2], height: f32) {
    if height >= 0.8 {
        w[0] += height * 10.0;
        w[1] += 10.0;
    } else if height >= 0.0 {
        w[0] += height;
        w[1] += 1.0;
    }
}

fn corner_height(self_height: f32, h1: f32, h2: f32, diagonal: f32) -> f32 {
    if h1 >= 1.0 || h2 >= 1.0 {
        return 1.0;
    }
    let mut w = [0.0f32; 2];
    if h1 > 0.0 || h2 > 0.0 {
        if diagonal >= 1.0 {
            return 1.0;
        }
        add_weighted(&mut w, diagonal);
    }
    add_weighted(&mut w, self_height);
    add_weighted(&mut w, h1);
    add_weighted(&mut w, h2);
    w[0] / w[1]
}

#[allow(clippy::too_many_arguments)]
fn emit_fluid_side(
    wx: f32,
    wy: f32,
    wz: f32,
    material: u8,
    fi: usize,
    bottom: f32,
    hh0: f32,
    hh1: f32,
    tile: u32,
    ar: u32,
    light: [f32; 2],
    tint: [f32; 3],
    buf: &mut MeshBuf,
) {
    const EPS: f32 = 0.001;
    let corners: [[f32; 3]; 4] = match fi {
        2 => [
            [1. - EPS, bottom, 0.],
            [1. - EPS, bottom, 1.],
            [1. - EPS, hh1, 1.],
            [1. - EPS, hh0, 0.],
        ],
        3 => [
            [EPS, bottom, 1.],
            [EPS, bottom, 0.],
            [EPS, hh1, 0.],
            [EPS, hh0, 1.],
        ],
        4 => [
            [1., bottom, 1. - EPS],
            [0., bottom, 1. - EPS],
            [0., hh1, 1. - EPS],
            [1., hh0, 1. - EPS],
        ],
        _ => [
            [0., bottom, EPS],
            [1., bottom, EPS],
            [1., hh1, EPS],
            [0., hh0, EPS],
        ],
    };
    let corners = corners.map(|[x, y, z]| [x + wx, y + wy, z + wz]);

    let [u0t, v0t, u1t, v1t] = tile_uv_rect(tile, ar);
    let get_u = |t: f32| lerp(u0t, u1t, t);
    let get_v = |t: f32| lerp(v0t, v1t, t);
    let (u_a, u_b) = (get_u(0.0), get_u(0.5));
    let v_bottom = get_v(0.5);
    let face_uvs = [
        [u_a, v_bottom],
        [u_b, v_bottom],
        [u_b, get_v((1.0 - hh1) * 0.5)],
        [u_a, get_v((1.0 - hh0) * 0.5)],
    ];
    buf.push_quad(&corners, &face_uvs, light, tint, material);
    push_back_face(buf, &corners, &face_uvs, light, tint, material);
}

#[allow(clippy::too_many_arguments)]
fn emit_fluid(
    wx: f32,
    wy: f32,
    wz: f32,
    material: u8,
    amount: u8,
    still: u32,
    flow: u32,
    ar: u32,
    occ: &Occupancy,
    bx: i32,
    by: i32,
    bz: i32,
    tint: [f32; 3],
    buf: &mut MeshBuf,
) {
    const EPS: f32 = 0.001;
    let lava = material == MATERIAL_LAVA;

    let above_fluid = occ.same_fluid(bx, by + 1, bz, lava);
    let (top_nw, top_ne, top_se, top_sw) = if above_fluid {
        (1.0, 1.0, 1.0, 1.0)
    } else {
        let self_height = super::fluid_surface(amount);
        let north = corner_height_input(occ, bx, by, bz - 1, lava);
        let south = corner_height_input(occ, bx, by, bz + 1, lava);
        let east = corner_height_input(occ, bx + 1, by, bz, lava);
        let west = corner_height_input(occ, bx - 1, by, bz, lava);
        let ne = corner_height(
            self_height,
            north,
            east,
            corner_height_input(occ, bx + 1, by, bz - 1, lava),
        );
        let nw = corner_height(
            self_height,
            north,
            west,
            corner_height_input(occ, bx - 1, by, bz - 1, lava),
        );
        let se = corner_height(
            self_height,
            south,
            east,
            corner_height_input(occ, bx + 1, by, bz + 1, lava),
        );
        let sw = corner_height(
            self_height,
            south,
            west,
            corner_height_input(occ, bx - 1, by, bz + 1, lava),
        );
        (nw - EPS, ne - EPS, se - EPS, sw - EPS)
    };

    let tiles = [still, still, flow, flow, flow, flow];
    let light = {
        let packed = flat_light_coords(occ, (bx, by, bz), (bx, by, bz));
        [
            (lighter::coords::block(packed) * 16) as f32,
            (lighter::coords::sky(packed) * 16) as f32,
        ]
    };
    const NBR: [(i32, i32, i32); 6] = [
        (0, 1, 0),
        (0, -1, 0),
        (1, 0, 0),
        (-1, 0, 0),
        (0, 0, 1),
        (0, 0, -1),
    ];
    let render_down = !occ.same_fluid(bx, by - 1, bz, lava) && !occ.is_solid(bx, by - 1, bz);
    let bottom = if render_down { EPS } else { 0.0 };
    for fi in 0..6usize {
        let (dx, dy, dz) = NBR[fi];
        let (nx, ny, nz) = (bx + dx, by + dy, bz + dz);
        match fi {
            0 => {
                let lowest = top_nw.min(top_ne).min(top_se).min(top_sw) + EPS;
                if above_fluid || (occ.is_solid(nx, ny, nz) && lowest >= 1.0) {
                    continue;
                }
                let (flow_x, flow_z) = fluid_flow(occ, bx, by, bz, super::fluid_surface(amount));
                let corners = [
                    [0., top_nw, 0.],
                    [1., top_ne, 0.],
                    [1., top_se, 1.],
                    [0., top_sw, 1.],
                ];
                let corners = corners.map(|[x, y, z]| [x + wx, y + wy, z + wz]);
                let face_uvs = if flow_x == 0.0 && flow_z == 0.0 {
                    uv_horizontal(0., 0., 1., 1., tiles[0], ar)
                } else {
                    flowing_top_uvs(flow_x, flow_z, flow, ar)
                };
                buf.push_quad(&corners, &face_uvs, light, tint, material);
                if backward_up_face(occ, bx, by + 1, bz, lava) {
                    push_back_face(buf, &corners, &face_uvs, light, tint, material);
                }
            }
            1 => {
                if !render_down {
                    continue;
                }
                emit_face(
                    wx, wy, wz, 0., EPS, 0., 1., 1., 1., 1, tiles[1], ar, light, tint, material,
                    buf,
                );
            }
            _ => {
                if occ.same_fluid(nx, ny, nz, lava) || occ.is_solid(nx, ny, nz) {
                    continue;
                }
                let (hh0, hh1) = match fi {
                    2 => (top_ne, top_se),
                    3 => (top_sw, top_nw),
                    4 => (top_se, top_sw),
                    _ => (top_nw, top_ne),
                };
                emit_fluid_side(
                    wx, wy, wz, material, fi, bottom, hh0, hh1, tiles[fi], ar, light, tint, buf,
                );
            }
        }
    }
}

fn backward_up_face(occ: &Occupancy, x: i32, y: i32, z: i32, lava: bool) -> bool {
    (-1..=1).any(|ox| {
        (-1..=1)
            .any(|oz| !occ.same_fluid(x + ox, y, z + oz, lava) && !occ.is_solid(x + ox, y, z + oz))
    })
}

fn push_back_face(
    buf: &mut MeshBuf,
    corners: &[[f32; 3]; 4],
    face_uvs: &[[f32; 2]; 4],
    light: [f32; 2],
    tint: [f32; 3],
    material: u8,
) {
    let order = [0, 3, 2, 1];
    buf.push_quad(
        &order.map(|i| corners[i]),
        &order.map(|i| face_uvs[i]),
        light,
        tint,
        material,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use azalea::block::BlockState;
    use azalea_registry::builtin::BlockKind;

    #[test]
    fn the_shade_table_matches_the_transfer_function() {
        assert_eq!(SRGB_TO_LINEAR[0], 0.0);
        assert_eq!(SRGB_TO_LINEAR[255], 1.0);
        for (i, got) in SRGB_TO_LINEAR.iter().enumerate() {
            let want = shade_to_linear(i as f32 / 255.0);
            assert!(
                (got - want).abs() <= 1.0 / 255.0,
                "entry {i}: {got} vs {want}"
            );
        }
    }

    #[test]
    fn a_block_in_the_dark_gets_no_light_on_any_vertex() {
        crate::blocks::tests::atlas_once();
        let mut occ = Occupancy::new(0, 0, -2, 20);
        for y in -2..18 {
            for x in -1..17 {
                for z in -1..17 {
                    occ.set_light(x, y, z, 0, 0);
                }
            }
        }
        let rb = crate::util::block_model::block_visual(BlockState::from(BlockKind::Stone));
        occ.set_solid(0, 0, 0);
        occ.set_full_cube(0, 0, 0);
        let blocks = vec![(0, 0, 0, rb)];
        let (buf, _) = build_section_mesh(&blocks, 0, 0, 0, 1, &occ);
        assert!(!buf.verts.is_empty(), "the block emitted no faces at all");
        for v in &buf.verts {
            assert_eq!(
                v.light[..2],
                [0, 0],
                "a vertex is lit in a cell with no light"
            );
        }
    }
}
