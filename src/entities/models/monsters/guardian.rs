#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

const SPIKE_NAMES: [&str; 12] = [
    "spike0", "spike1", "spike2", "spike3", "spike4", "spike5", "spike6", "spike7", "spike8",
    "spike9", "spike10", "spike11",
];

const SPIKE_X_ROT: [f32; 12] = [
    1.75, 0.25, 0.0, 0.0, 0.5, 0.5, 0.5, 0.5, 1.25, 0.75, 0.0, 0.0,
];
const SPIKE_Y_ROT: [f32; 12] = [
    0.0, 0.0, 0.0, 0.0, 0.25, 1.75, 1.25, 0.75, 0.0, 0.0, 0.0, 0.0,
];
const SPIKE_Z_ROT: [f32; 12] = [
    0.0, 0.0, 0.25, 1.75, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.75, 1.25,
];
const SPIKE_X: [f32; 12] = [
    0.0, 0.0, 8.0, -8.0, -8.0, 8.0, 8.0, -8.0, 0.0, 0.0, 8.0, -8.0,
];
const SPIKE_Y: [f32; 12] = [
    -8.0, -8.0, -8.0, -8.0, 0.0, 0.0, 0.0, 0.0, 8.0, 8.0, 8.0, 8.0,
];
const SPIKE_Z: [f32; 12] = [
    8.0, -8.0, 0.0, 0.0, -8.0, -8.0, 8.0, 8.0, 8.0, -8.0, 0.0, 0.0,
];

pub fn layer() -> LayerDef {
    LayerDef::create(body_mesh(), 64, 64)
}

pub fn elder_layer() -> LayerDef {
    const FACTOR: f32 = 2.35;
    const Y_OFFSET: f32 = 24.016 * (1.0 - FACTOR);
    let mesh = body_mesh().transformed(|pose| pose.scaled(FACTOR).translated(0.0, Y_OFFSET, 0.0));
    LayerDef::create(mesh, 64, 64)
}

fn body_mesh() -> MeshDef {
    let mut mesh = MeshDef::new();
    let head = mesh.root().child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-6.0, 10.0, -8.0, 12.0, 12.0, 16.0)
            .tex_offs(0, 28)
            .add_box(-8.0, 10.0, -6.0, 2.0, 12.0, 12.0)
            .tex_offs(0, 28)
            .add_box_mirror(6.0, 10.0, -6.0, 2.0, 12.0, 12.0, true)
            .tex_offs(16, 40)
            .add_box(-6.0, 8.0, -6.0, 12.0, 2.0, 12.0)
            .tex_offs(16, 40)
            .add_box(-6.0, 22.0, -6.0, 12.0, 2.0, 12.0),
        PartPose::ZERO,
    );
    let spike = CubeList::new()
        .tex_offs(0, 0)
        .add_box(-1.0, -4.5, -1.0, 2.0, 9.0, 2.0);
    for i in 0..12 {
        head.child(
            &format!("spike{i}"),
            spike.clone(),
            PartPose::offset_rotation(
                spike_x(i, 0.0, 0.0),
                spike_y(i, 0.0, 0.0),
                spike_z(i, 0.0, 0.0),
                3.1415927 * SPIKE_X_ROT[i],
                3.1415927 * SPIKE_Y_ROT[i],
                3.1415927 * SPIKE_Z_ROT[i],
            ),
        );
    }
    head.child(
        "eye",
        CubeList::new()
            .tex_offs(8, 0)
            .add_box(-1.0, 15.0, 0.0, 2.0, 2.0, 1.0),
        PartPose::offset(0.0, 0.0, -8.25),
    );
    let tail0 = head.child(
        "tail0",
        CubeList::new()
            .tex_offs(40, 0)
            .add_box(-2.0, 14.0, 7.0, 4.0, 4.0, 8.0),
        PartPose::ZERO,
    );
    let tail1 = tail0.child(
        "tail1",
        CubeList::new()
            .tex_offs(0, 54)
            .add_box(0.0, 14.0, 0.0, 3.0, 3.0, 7.0),
        PartPose::offset(-1.5, 0.5, 14.0),
    );
    tail1.child(
        "tail2",
        CubeList::new()
            .tex_offs(41, 32)
            .add_box(0.0, 14.0, 0.0, 2.0, 2.0, 6.0)
            .tex_offs(25, 19)
            .add_box(1.0, 10.5, 3.0, 1.0, 9.0, 9.0),
        PartPose::offset(0.5, 0.5, 6.0),
    );
    mesh
}

fn spike_offset(spike: usize, age_ticks: f32, withdrawal: f32) -> f32 {
    1.0 + (age_ticks * 1.5 + spike as f32).cos() * 0.01 - withdrawal
}

fn spike_x(spike: usize, age_ticks: f32, withdrawal: f32) -> f32 {
    SPIKE_X[spike] * spike_offset(spike, age_ticks, withdrawal)
}

fn spike_y(spike: usize, age_ticks: f32, withdrawal: f32) -> f32 {
    16.0 + SPIKE_Y[spike] * spike_offset(spike, age_ticks, withdrawal)
}

fn spike_z(spike: usize, age_ticks: f32, withdrawal: f32) -> f32 {
    SPIKE_Z[spike] * spike_offset(spike, age_ticks, withdrawal)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let head = model.id("head");
    parts[head].y_rot = st.y_rot * DEG_TO_RAD;
    parts[head].x_rot = st.x_rot * DEG_TO_RAD;

    let withdrawal = (1.0 - st.extras.spikes_animation) * 0.55;
    for i in 0..12 {
        let id = model.id(SPIKE_NAMES[i]);
        parts[id].x = spike_x(i, st.age_ticks, withdrawal);
        parts[id].y = spike_y(i, st.age_ticks, withdrawal);
        parts[id].z = spike_z(i, st.age_ticks, withdrawal);
    }

    let eye = model.id("eye");
    parts[eye].visible = true;

    let swim = st.extras.tail_animation;
    parts[model.id("tail0")].y_rot = swim.sin() * 3.1415927 * 0.05;
    parts[model.id("tail1")].y_rot = swim.sin() * 3.1415927 * 0.1;
    parts[model.id("tail2")].y_rot = swim.sin() * 3.1415927 * 0.15;
}
