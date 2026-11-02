use azalea_registry::builtin::EntityKind;
use bevy::math::{DVec3, Quat, Vec3};
use bevy::prelude::Mesh;

use crate::entities::quads::QuadMesh;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{RenderSpec, RootPose, TexturePath};
use crate::util::variants::PaintingVariant;

const EDGE: f32 = 0.03125;
const EDGE_STRIP: f32 = 0.0625;
const SHIFT_TO_BLOCK_WALL: f32 = 0.46875;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Painting,
        RenderSpec::built("painting", variant_key, front_mesh, front_texture)
            .with_root(painting_root)
            .ignoring_invisibility(),
    );
    registry.add(
        EntityKind::Painting,
        RenderSpec::built("painting_back", variant_key, back_mesh, back_texture)
            .with_root(painting_root)
            .ignoring_invisibility(),
    );
}

fn variant(st: &EntityState) -> Option<&'static PaintingVariant> {
    crate::util::variants::painting(st.extras.variant.as_deref()?)
}

fn variant_key(st: &EntityState) -> TexturePath {
    match variant(st) {
        Some(_) => st.extras.variant.clone().unwrap_or_default().into(),
        None => "".into(),
    }
}

fn front_texture(st: &EntityState) -> Option<TexturePath> {
    variant(st).map(|v| format!("painting/{}", v.asset_id).into())
}

fn back_texture(_st: &EntityState) -> Option<TexturePath> {
    Some("painting/back".into())
}

fn size_offset(size: u32) -> f32 {
    if size % 2 == 0 { 0.5 } else { 0.0 }
}

fn painting_root(st: &EntityState) -> RootPose {
    let dir = st.extras.hanging_direction;
    let (w, h) = variant(st).map_or((1, 1), |v| (v.width, v.height));
    let pos = DVec3::from_array(st.pos);
    let to_block = (pos.floor() - pos).as_vec3();
    let offset = to_block + Vec3::splat(0.5) - Vec3::from(dir.normal()) * SHIFT_TO_BLOCK_WALL
        + Vec3::from(dir.counter_clockwise().normal()) * size_offset(w)
        + Vec3::Y * size_offset(h);
    RootPose {
        world_offset: offset,
        rotation: Quat::from_rotation_y((180.0 - dir.to_y_rot()).to_radians()),
        ..RootPose::default()
    }
}

struct Segment {
    x0: f32,
    x1: f32,
    y0: f32,
    y1: f32,
}

fn segments(v: &PaintingVariant, mut each: impl FnMut(u32, u32, Segment)) {
    let (offset_x, offset_y) = (-(v.width as f32) / 2.0, -(v.height as f32) / 2.0);
    for sx in 0..v.width {
        for sy in 0..v.height {
            each(
                sx,
                sy,
                Segment {
                    x0: offset_x + (sx + 1) as f32,
                    x1: offset_x + sx as f32,
                    y0: offset_y + (sy + 1) as f32,
                    y1: offset_y + sy as f32,
                },
            );
        }
    }
}

fn front_mesh(st: &EntityState) -> Mesh {
    let mut out = QuadMesh::new();
    if let Some(v) = variant(st) {
        let (du, dv) = (1.0 / v.width as f32, 1.0 / v.height as f32);
        segments(v, |sx, sy, s| {
            let u0 = du * (v.width - sx) as f32;
            let u1 = du * (v.width - (sx + 1)) as f32;
            let v0 = dv * (v.height - sy) as f32;
            let v1 = dv * (v.height - (sy + 1)) as f32;
            out.quad(
                [
                    [s.x0, s.y1, -EDGE],
                    [s.x1, s.y1, -EDGE],
                    [s.x1, s.y0, -EDGE],
                    [s.x0, s.y0, -EDGE],
                ],
                [[u1, v0], [u0, v0], [u0, v1], [u1, v1]],
                [0.0, 0.0, -1.0],
            );
        });
    }
    out.finish()
}

fn back_mesh(st: &EntityState) -> Mesh {
    let mut out = QuadMesh::new();
    if let Some(v) = variant(st) {
        let full = [[1.0, 0.0], [0.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
        let top_bottom = [[0.0, 0.0], [1.0, 0.0], [1.0, EDGE_STRIP], [0.0, EDGE_STRIP]];
        let left_right = [[EDGE_STRIP, 0.0], [EDGE_STRIP, 1.0], [0.0, 1.0], [0.0, 0.0]];
        segments(v, |sx, sy, s| {
            out.quad(
                [
                    [s.x0, s.y0, EDGE],
                    [s.x1, s.y0, EDGE],
                    [s.x1, s.y1, EDGE],
                    [s.x0, s.y1, EDGE],
                ],
                full,
                [0.0, 0.0, 1.0],
            );
            if sy == v.height - 1 {
                out.quad(
                    [
                        [s.x0, s.y0, -EDGE],
                        [s.x1, s.y0, -EDGE],
                        [s.x1, s.y0, EDGE],
                        [s.x0, s.y0, EDGE],
                    ],
                    top_bottom,
                    [0.0, 1.0, 0.0],
                );
            }
            if sy == 0 {
                out.quad(
                    [
                        [s.x0, s.y1, EDGE],
                        [s.x1, s.y1, EDGE],
                        [s.x1, s.y1, -EDGE],
                        [s.x0, s.y1, -EDGE],
                    ],
                    top_bottom,
                    [0.0, -1.0, 0.0],
                );
            }
            if sx == v.width - 1 {
                out.quad(
                    [
                        [s.x0, s.y0, EDGE],
                        [s.x0, s.y1, EDGE],
                        [s.x0, s.y1, -EDGE],
                        [s.x0, s.y0, -EDGE],
                    ],
                    left_right,
                    [-1.0, 0.0, 0.0],
                );
            }
            if sx == 0 {
                out.quad(
                    [
                        [s.x1, s.y0, -EDGE],
                        [s.x1, s.y1, -EDGE],
                        [s.x1, s.y1, EDGE],
                        [s.x1, s.y0, EDGE],
                    ],
                    left_right,
                    [1.0, 0.0, 0.0],
                );
            }
        });
    }
    out.finish()
}
