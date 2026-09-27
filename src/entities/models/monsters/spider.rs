#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

pub fn layer() -> LayerDef {
    LayerDef::create(body_mesh(), 64, 32)
}

pub fn cave_spider_layer() -> LayerDef {
    const FACTOR: f32 = 0.7;
    const Y_OFFSET: f32 = 24.016 * (1.0 - FACTOR);
    let mesh = body_mesh().transformed(|pose| pose.scaled(FACTOR).translated(0.0, Y_OFFSET, 0.0));
    LayerDef::create(mesh, 64, 32)
}

fn body_mesh() -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "head",
        CubeList::new()
            .tex_offs(32, 4)
            .add_box(-4.0, -4.0, -8.0, 8.0, 8.0, 8.0),
        PartPose::offset(0.0, 15.0, -3.0),
    );
    root.child(
        "body0",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-3.0, -3.0, -3.0, 6.0, 6.0, 6.0),
        PartPose::offset(0.0, 15.0, 0.0),
    );
    root.child(
        "body1",
        CubeList::new()
            .tex_offs(0, 12)
            .add_box(-5.0, -4.0, -6.0, 10.0, 8.0, 12.0),
        PartPose::offset(0.0, 15.0, 9.0),
    );
    let right_leg = CubeList::new()
        .tex_offs(18, 0)
        .add_box(-15.0, -1.0, -1.0, 16.0, 2.0, 2.0);
    let left_leg = CubeList::new()
        .tex_offs(18, 0)
        .mirror()
        .add_box(-1.0, -1.0, -1.0, 16.0, 2.0, 2.0);
    root.child(
        "right_hind_leg",
        right_leg.clone(),
        PartPose::offset_rotation(-4.0, 15.0, 2.0, 0.0, 0.7853982, -0.7853982),
    );
    root.child(
        "left_hind_leg",
        left_leg.clone(),
        PartPose::offset_rotation(4.0, 15.0, 2.0, 0.0, -0.7853982, 0.7853982),
    );
    root.child(
        "right_middle_hind_leg",
        right_leg.clone(),
        PartPose::offset_rotation(-4.0, 15.0, 1.0, 0.0, 0.3926991, -0.58119464),
    );
    root.child(
        "left_middle_hind_leg",
        left_leg.clone(),
        PartPose::offset_rotation(4.0, 15.0, 1.0, 0.0, -0.3926991, 0.58119464),
    );
    root.child(
        "right_middle_front_leg",
        right_leg.clone(),
        PartPose::offset_rotation(-4.0, 15.0, 0.0, 0.0, -0.3926991, -0.58119464),
    );
    root.child(
        "left_middle_front_leg",
        left_leg.clone(),
        PartPose::offset_rotation(4.0, 15.0, 0.0, 0.0, 0.3926991, 0.58119464),
    );
    root.child(
        "right_front_leg",
        right_leg,
        PartPose::offset_rotation(-4.0, 15.0, -1.0, 0.0, -0.7853982, -0.7853982),
    );
    root.child(
        "left_front_leg",
        left_leg,
        PartPose::offset_rotation(4.0, 15.0, -1.0, 0.0, 0.7853982, 0.7853982),
    );
    mesh
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let head = model.id("head");
    parts[head].y_rot = st.y_rot * DEG_TO_RAD;
    parts[head].x_rot = st.x_rot * DEG_TO_RAD;

    let pos = st.walk_pos * 0.6662;
    let speed = st.walk_speed;
    let swing_hind = -((pos * 2.0 + 0.0).cos() * 0.4) * speed;
    let swing_middle_hind = -((pos * 2.0 + 3.1415927).cos() * 0.4) * speed;
    let swing_middle_front = -((pos * 2.0 + 1.5707964).cos() * 0.4) * speed;
    let swing_front = -((pos * 2.0 + 4.712389).cos() * 0.4) * speed;
    let step_hind = ((pos + 0.0).sin() * 0.4).abs() * speed;
    let step_middle_hind = ((pos + 3.1415927).sin() * 0.4).abs() * speed;
    let step_middle_front = ((pos + 1.5707964).sin() * 0.4).abs() * speed;
    let step_front = ((pos + 4.712389).sin() * 0.4).abs() * speed;

    for (name, swing, step) in [
        ("right_hind_leg", swing_hind, step_hind),
        ("left_hind_leg", -swing_hind, -step_hind),
        ("right_middle_hind_leg", swing_middle_hind, step_middle_hind),
        (
            "left_middle_hind_leg",
            -swing_middle_hind,
            -step_middle_hind,
        ),
        (
            "right_middle_front_leg",
            swing_middle_front,
            step_middle_front,
        ),
        (
            "left_middle_front_leg",
            -swing_middle_front,
            -step_middle_front,
        ),
        ("right_front_leg", swing_front, step_front),
        ("left_front_leg", -swing_front, -step_front),
    ] {
        let id = model.id(name);
        parts[id].y_rot += swing;
        parts[id].z_rot += step;
    }
}
