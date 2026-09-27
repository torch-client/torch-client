#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

pub fn layer() -> LayerDef {
    let deformation = Grow::all(-0.5);
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box_grow(-4.0, -8.0, -4.0, 8.0, 8.0, 8.0, deformation),
        PartPose::offset(0.0, 4.0, 0.0),
    );
    let arm =
        CubeList::new()
            .tex_offs(32, 0)
            .add_box_grow(-1.0, 0.0, -1.0, 12.0, 2.0, 2.0, deformation);
    root.child(
        "left_arm",
        arm.clone(),
        PartPose::offset_rotation(5.0, 6.0, 1.0, 0.0, 0.0, 1.0),
    );
    root.child(
        "right_arm",
        arm,
        PartPose::offset_rotation(-5.0, 6.0, -1.0, 0.0, 3.1415927, -1.0),
    );
    root.child(
        "upper_body",
        CubeList::new().tex_offs(0, 16).add_box_grow(
            -5.0,
            -10.0,
            -5.0,
            10.0,
            10.0,
            10.0,
            deformation,
        ),
        PartPose::offset(0.0, 13.0, 0.0),
    );
    root.child(
        "lower_body",
        CubeList::new().tex_offs(0, 36).add_box_grow(
            -6.0,
            -12.0,
            -6.0,
            12.0,
            12.0,
            12.0,
            deformation,
        ),
        PartPose::offset(0.0, 24.0, 0.0),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let head = model.id("head");
    let upper_body = model.id("upper_body");
    let left_arm = model.id("left_arm");
    let right_arm = model.id("right_arm");

    parts[head].y_rot = st.y_rot * DEG_TO_RAD;
    parts[head].x_rot = st.x_rot * DEG_TO_RAD;
    parts[upper_body].y_rot = st.y_rot * DEG_TO_RAD * 0.25;
    let body_y_rot = parts[upper_body].y_rot;
    let sin = body_y_rot.sin();
    let cos = body_y_rot.cos();
    parts[left_arm].y_rot = body_y_rot;
    parts[right_arm].y_rot = body_y_rot + 3.1415927;
    parts[left_arm].x = cos * 5.0;
    parts[left_arm].z = -sin * 5.0;
    parts[right_arm].x = -cos * 5.0;
    parts[right_arm].z = sin * 5.0;
}
