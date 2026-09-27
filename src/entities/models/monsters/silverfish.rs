#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;

const SILVERFISH_SIZES: [[i32; 3]; 7] = [
    [3, 2, 2],
    [4, 3, 2],
    [6, 4, 3],
    [3, 3, 3],
    [2, 2, 3],
    [2, 1, 2],
    [1, 1, 2],
];
const SILVERFISH_TEXS: [[i32; 2]; 7] = [[0, 0], [0, 4], [0, 9], [0, 16], [0, 22], [11, 0], [13, 4]];

const ENDERMITE_SIZES: [[i32; 3]; 4] = [[4, 3, 2], [6, 4, 5], [3, 3, 1], [1, 2, 1]];
const ENDERMITE_TEXS: [[i32; 2]; 4] = [[0, 0], [0, 5], [0, 14], [0, 18]];

fn segments(mesh: &mut MeshDef, sizes: &[[i32; 3]], texs: &[[i32; 2]]) -> Vec<f32> {
    let root = mesh.root();
    let mut placement = -3.5;
    let mut z_placement = Vec::with_capacity(sizes.len());
    for i in 0..sizes.len() {
        root.child(
            &format!("segment{i}"),
            CubeList::new().tex_offs(texs[i][0], texs[i][1]).add_box(
                sizes[i][0] as f32 * -0.5,
                0.0,
                sizes[i][2] as f32 * -0.5,
                sizes[i][0] as f32,
                sizes[i][1] as f32,
                sizes[i][2] as f32,
            ),
            PartPose::offset(0.0, (24 - sizes[i][1]) as f32, placement),
        );
        z_placement.push(placement);
        if i < sizes.len() - 1 {
            placement += (sizes[i][2] + sizes[i + 1][2]) as f32 * 0.5;
        }
    }
    z_placement
}

pub fn silverfish_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let z = segments(&mut mesh, &SILVERFISH_SIZES, &SILVERFISH_TEXS);
    let root = mesh.root();
    root.child(
        "layer0",
        CubeList::new().tex_offs(20, 0).add_box(
            -5.0,
            0.0,
            SILVERFISH_SIZES[2][2] as f32 * -0.5,
            10.0,
            8.0,
            SILVERFISH_SIZES[2][2] as f32,
        ),
        PartPose::offset(0.0, 16.0, z[2]),
    );
    root.child(
        "layer1",
        CubeList::new().tex_offs(20, 11).add_box(
            -3.0,
            0.0,
            SILVERFISH_SIZES[4][2] as f32 * -0.5,
            6.0,
            4.0,
            SILVERFISH_SIZES[4][2] as f32,
        ),
        PartPose::offset(0.0, 20.0, z[4]),
    );
    root.child(
        "layer2",
        CubeList::new().tex_offs(20, 18).add_box(
            -3.0,
            0.0,
            SILVERFISH_SIZES[4][2] as f32 * -0.5,
            6.0,
            5.0,
            SILVERFISH_SIZES[1][2] as f32,
        ),
        PartPose::offset(0.0, 19.0, z[1]),
    );
    LayerDef::create(mesh, 64, 32)
}

pub fn endermite_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    segments(&mut mesh, &ENDERMITE_SIZES, &ENDERMITE_TEXS);
    LayerDef::create(mesh, 64, 32)
}

fn wriggle(
    model: &BakedModel,
    parts: &mut [PartState],
    st: &EntityState,
    count: usize,
    yaw_scale: f32,
    x_scale: f32,
) {
    for i in 0..count {
        let phase = st.age_ticks * 0.9 + i as f32 * 0.15 * 3.1415927;
        let id = model.id(&format!("segment{i}"));
        parts[id].y_rot = phase.cos() * 3.1415927 * yaw_scale * (1 + (i as i32 - 2).abs()) as f32;
        parts[id].x = phase.sin() * 3.1415927 * x_scale * (i as i32 - 2).abs() as f32;
    }
}

pub fn silverfish_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    wriggle(model, parts, st, 7, 0.05, 0.2);
    let (l0, l1, l2) = (model.id("layer0"), model.id("layer1"), model.id("layer2"));
    let (s1, s2, s4) = (
        model.id("segment1"),
        model.id("segment2"),
        model.id("segment4"),
    );
    parts[l0].y_rot = parts[s2].y_rot;
    parts[l1].y_rot = parts[s4].y_rot;
    parts[l1].x = parts[s4].x;
    parts[l2].y_rot = parts[s1].y_rot;
    parts[l2].x = parts[s1].x;
}

pub fn endermite_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    wriggle(model, parts, st, 4, 0.01, 0.1);
}
