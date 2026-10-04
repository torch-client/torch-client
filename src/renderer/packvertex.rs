use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use bevy::math::Vec3;

pub const UNMAPPED: u16 = u16::MAX;

pub const QUAD_ROW: u32 = 2048;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PackQuad(pub [u32; 4]);

const _: () = assert!(std::mem::size_of::<PackQuad>() == 16);

#[derive(PartialEq, Eq)]
pub struct BlockIds {
    ids: Box<[u16]>,
    lighting: PackLighting,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PackLighting {
    pub old_lighting: bool,
    pub separate_ao: bool,
}

impl BlockIds {
    #[cfg(test)]
    pub fn new(ids: Box<[u16]>) -> BlockIds {
        BlockIds {
            ids,
            lighting: PackLighting::default(),
        }
    }

    pub fn lighting(&self) -> PackLighting {
        self.lighting
    }

    pub(crate) fn build(
        entries: &[crate::shaderpack::blockids::Entry],
        lighting: PackLighting,
    ) -> BlockIds {
        use azalea::block::{BlockState, BlockTrait};
        use std::collections::HashMap;

        let mut by_block: HashMap<&str, Vec<&crate::shaderpack::blockids::Entry>> = HashMap::new();
        for entry in entries {
            by_block
                .entry(entry.block.as_str())
                .or_default()
                .push(entry);
        }
        let mut ids = vec![UNMAPPED; BlockState::MAX_STATE as usize + 1].into_boxed_slice();
        if by_block.is_empty() {
            return BlockIds { ids, lighting };
        }
        for (raw, slot) in ids.iter_mut().enumerate() {
            let Ok(state) = BlockState::try_from(raw as u32) else {
                continue;
            };
            let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
            let name = block.id();
            let Some(candidates) = by_block.get(name) else {
                continue;
            };
            let found = if candidates.iter().any(|e| !e.properties.is_empty()) {
                let properties = block.property_map();
                candidates
                    .iter()
                    .find(|e| !e.properties.is_empty() && e.matches(name, &properties))
            } else {
                None
            };
            let found = found.or_else(|| candidates.iter().find(|e| e.properties.is_empty()));
            if let Some(entry) = found {
                *slot = entry.id;
            }
        }
        BlockIds { ids, lighting }
    }

    pub fn of(&self, state: u16) -> u16 {
        self.ids.get(state as usize).copied().unwrap_or(UNMAPPED)
    }
}

static CURRENT: RwLock<Option<Arc<BlockIds>>> = RwLock::new(None);

static ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn active() -> bool {
    ACTIVE.load(Ordering::Acquire)
}

static PLAYER_SHADOW: AtomicBool = AtomicBool::new(false);

pub fn player_shadow() -> bool {
    PLAYER_SHADOW.load(Ordering::Relaxed)
}

pub fn set_player_shadow(on: bool) {
    PLAYER_SHADOW.store(on, Ordering::Relaxed);
}

pub fn current() -> Option<Arc<BlockIds>> {
    if !active() {
        return None;
    }
    CURRENT.read().ok().and_then(|ids| ids.clone())
}

pub fn set_current(ids: Option<Arc<BlockIds>>) {
    if let Ok(mut slot) = CURRENT.write() {
        ACTIVE.store(ids.is_some(), Ordering::Release);
        *slot = ids;
    }
}

pub fn quads_layout() -> bevy::render::render_resource::BindGroupLayoutDescriptor {
    use bevy::render::render_resource::binding_types::texture_2d;
    use bevy::render::render_resource::{
        BindGroupLayoutDescriptor, BindGroupLayoutEntries, ShaderStages, TextureSampleType,
    };
    BindGroupLayoutDescriptor::new(
        "shaderpack pack quads",
        &BindGroupLayoutEntries::single(ShaderStages::VERTEX, texture_2d(TextureSampleType::Uint)),
    )
}

pub fn quad_texel(index: u32) -> [u32; 2] {
    [index % QUAD_ROW, index / QUAD_ROW]
}

#[derive(Clone, Copy, Debug)]
pub struct PackBlock {
    pub id: u16,
    pub emission: u8,
    pub cell: [f32; 3],
    pub tint: [f32; 3],
    pub shades: [f32; 6],
    pub lighting: PackLighting,
}

impl PackBlock {
    pub fn color(&self, tint: [f32; 3], face_shade: f32, ao: f32) -> [f32; 4] {
        let shade = if self.lighting.old_lighting {
            face_shade
        } else {
            self.shades[crate::renderer::dimension::UP]
        };
        self.combine(tint, ao * shade)
    }

    pub fn fluid_color(&self, tint: [f32; 3], face: usize) -> [f32; 4] {
        let shade = if self.lighting.old_lighting {
            self.shades[face]
        } else {
            1.0
        };
        self.combine(tint, shade)
    }

    fn combine(&self, tint: [f32; 3], brightness: f32) -> [f32; 4] {
        if self.lighting.separate_ao {
            [tint[0], tint[1], tint[2], brightness]
        } else {
            [
                tint[0] * brightness,
                tint[1] * brightness,
                tint[2] * brightness,
                1.0,
            ]
        }
    }
}

fn snorm(v: f32) -> i8 {
    (v.clamp(-1.0, 1.0) * 127.0).round() as i8
}

fn unit(v: Vec3) -> Option<Vec3> {
    let length = v.length();
    (length > 1e-12).then(|| v / length)
}

pub fn quad(
    block: &PackBlock,
    corners: &[[f32; 3]; 4],
    uvs: &[[f32; 2]; 4],
    normal: [f32; 3],
) -> PackQuad {
    let normal = unit(Vec3::from(normal)).unwrap_or(Vec3::Y);
    let e1 = Vec3::from(corners[1]) - Vec3::from(corners[0]);
    let e2 = Vec3::from(corners[3]) - Vec3::from(corners[0]);
    let (du1, dv1) = (uvs[1][0] - uvs[0][0], uvs[1][1] - uvs[0][1]);
    let (du2, dv2) = (uvs[3][0] - uvs[0][0], uvs[3][1] - uvs[0][1]);
    let det = du1 * dv2 - du2 * dv1;
    let fallback = || {
        let axis = if normal.x.abs() < 0.9 {
            Vec3::X
        } else {
            Vec3::Z
        };
        (unit(axis.cross(normal)).unwrap_or(Vec3::X), 1.0)
    };
    let (tangent, handedness) = if det.abs() > 1e-12 {
        let t = (e1 * dv2 - e2 * dv1) / det;
        let b = (e2 * du1 - e1 * du2) / det;
        match unit(t - normal * normal.dot(t)) {
            Some(t) => (
                t,
                if t.cross(normal).dot(b) < 0.0 {
                    -1.0
                } else {
                    1.0
                },
            ),
            None => fallback(),
        }
    } else {
        fallback()
    };

    let mid_u = (uvs[0][0] + uvs[1][0] + uvs[2][0] + uvs[3][0]) * 0.25;
    let mid_v = (uvs[0][1] + uvs[1][1] + uvs[2][1] + uvs[3][1]) * 0.25;
    let unorm16 = |v: f32| u32::from((v.clamp(0.0, 1.0) * 65535.0).round() as u16);
    let bytes = |v: [f32; 4]| u32::from_le_bytes(v.map(|c| snorm(c) as u8));
    let cell = block.cell.map(|c| (c as i32 as u32) & 0xF);
    PackQuad([
        bytes(normal.extend(0.0).to_array()),
        bytes(tangent.extend(handedness).to_array()),
        unorm16(mid_u) | unorm16(mid_v) << 16,
        u32::from(block.id)
            | cell[0] << 16
            | cell[1] << 20
            | cell[2] << 24
            | u32::from(block.emission.min(15)) << 28,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCK: PackBlock = PackBlock {
        id: 10304,
        emission: 0,
        cell: [2.0, 3.0, 4.0],
        tint: [1.0; 3],
        shades: [0.5, 1.0, 0.8, 0.8, 0.6, 0.6],
        lighting: PackLighting {
            old_lighting: false,
            separate_ao: false,
        },
    };

    struct Decoded {
        normal: [i8; 3],
        tangent: [i8; 4],
        mid_uv: [u16; 2],
        id: u16,
        cell: [u32; 3],
        emission: u32,
    }

    fn decode(q: PackQuad) -> Decoded {
        let [x, y, z, w] = q.0;
        let b = |v: u32| v.to_le_bytes().map(|c| c as i8);
        let n = b(x);
        Decoded {
            normal: [n[0], n[1], n[2]],
            tangent: b(y),
            mid_uv: [z as u16, (z >> 16) as u16],
            id: w as u16,
            cell: [(w >> 16) & 0xF, (w >> 20) & 0xF, (w >> 24) & 0xF],
            emission: w >> 28,
        }
    }

    #[test]
    fn a_top_face_points_up_with_u_along_x() {
        let corners = [
            [2.0, 4.0, 4.0],
            [2.0, 4.0, 5.0],
            [3.0, 4.0, 5.0],
            [3.0, 4.0, 4.0],
        ];
        let uvs = [[0.0, 0.0], [0.0, 0.5], [0.5, 0.5], [0.5, 0.0]];
        let q = decode(quad(&BLOCK, &corners, &uvs, [0.0, 1.0, 0.0]));
        assert_eq!(q.normal, [0, 127, 0]);
        assert_eq!(&q.tangent[..3], &[127, 0, 0]);
        assert_eq!(q.mid_uv, [16384, 16384]);
        assert_eq!(q.id, 10304);
        assert_eq!(q.cell, [2, 3, 4]);
        assert_eq!(q.emission, 0);
        assert_eq!(q.tangent[3], 127);
    }

    #[test]
    fn degenerate_uvs_still_give_a_unit_tangent() {
        let corners = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let t = decode(quad(&BLOCK, &corners, &[[0.25, 0.25]; 4], [0.0, 0.0, -1.0])).tangent;
        let length = ((t[0] as f32).powi(2) + (t[1] as f32).powi(2) + (t[2] as f32).powi(2)).sqrt();
        assert!((length - 127.0).abs() < 2.0, "{t:?}");
        assert_eq!(t[2], 0);
    }

    #[test]
    fn unmapped_ids_and_bright_blocks_keep_their_lanes() {
        let block = PackBlock {
            id: UNMAPPED,
            emission: 15,
            cell: [15.0, 0.0, 15.0],
            ..BLOCK
        };
        let corners = [
            [15.0, 0.0, 15.0],
            [16.0, 0.0, 15.0],
            [16.0, 1.0, 15.0],
            [15.0, 1.0, 15.0],
        ];
        let q = decode(quad(
            &block,
            &corners,
            &[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            [0.0, 0.0, -1.0],
        ));
        assert_eq!(q.id, UNMAPPED);
        assert_eq!(q.cell, [15, 0, 15]);
        assert_eq!(q.emission, 15);
    }

    #[test]
    fn quads_wrap_onto_the_next_row() {
        assert_eq!(quad_texel(0), [0, 0]);
        assert_eq!(quad_texel(QUAD_ROW - 1), [QUAD_ROW - 1, 0]);
        assert_eq!(quad_texel(QUAD_ROW + 3), [3, 1]);
    }

    #[test]
    fn vertex_colour_follows_the_pack_keys() {
        let tint = [0.5, 0.8, 0.25];
        let separate = PackBlock {
            lighting: PackLighting {
                old_lighting: false,
                separate_ao: true,
            },
            ..BLOCK
        };
        assert_eq!(separate.color(tint, 0.6, 0.7), [0.5, 0.8, 0.25, 0.7]);
        let mixed = PackBlock {
            lighting: PackLighting::default(),
            ..BLOCK
        };
        assert_eq!(mixed.color(tint, 0.6, 0.5), [0.25, 0.4, 0.125, 1.0]);
        let old = PackBlock {
            lighting: PackLighting {
                old_lighting: true,
                separate_ao: true,
            },
            ..BLOCK
        };
        assert_eq!(
            old.color([1.0; 3], 0.6, 1.0),
            [1.0, 1.0, 1.0, 0.6],
            "the shade goes to alpha"
        );
        let nether = PackBlock {
            shades: [0.9; 6],
            ..separate
        };
        assert_eq!(nether.color(tint, 0.5, 1.0), [0.5, 0.8, 0.25, 0.9]);
        assert_eq!(separate.fluid_color(tint, 4), [0.5, 0.8, 0.25, 1.0]);
        assert_eq!(old.fluid_color([1.0; 3], 4), [1.0, 1.0, 1.0, 0.6]);
    }

    #[test]
    fn unknown_states_are_unmapped() {
        let ids = BlockIds::new(vec![7, UNMAPPED].into_boxed_slice());
        assert_eq!(ids.of(0), 7);
        assert_eq!(ids.of(1), UNMAPPED);
        assert_eq!(ids.of(99), UNMAPPED);
    }
}
