#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::models::aquatic::DEG_TO_RAD;
use crate::entities::state::EntityState;

pub fn dolphin_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let body = mesh.root().part(
        "body",
        (22, 0),
        [-4.0, -7.0, 0.0, 8.0, 7.0, 13.0],
        PartPose::offset(0.0, 22.0, -5.0),
    );
    body.part(
        "back_fin",
        (51, 0),
        [-0.5, 0.0, 8.0, 1.0, 4.0, 5.0],
        PartPose::rotation(1.047_197_6, 0.0, 0.0),
    );
    body.child(
        "left_fin",
        CubeList::new()
            .tex_offs(48, 20)
            .mirror()
            .add_box(-0.5, -4.0, 0.0, 1.0, 4.0, 7.0),
        PartPose::offset_rotation(2.0, -2.0, 4.0, 1.047_197_6, 0.0, 2.094_395_2),
    );
    body.part(
        "right_fin",
        (48, 20),
        [-0.5, -4.0, 0.0, 1.0, 4.0, 7.0],
        PartPose::offset_rotation(-2.0, -2.0, 4.0, 1.047_197_6, 0.0, -2.094_395_2),
    );
    body.part(
        "tail",
        (0, 19),
        [-2.0, -2.5, 0.0, 4.0, 5.0, 11.0],
        PartPose::offset_rotation(0.0, -2.5, 11.0, -0.104_719_76, 0.0, 0.0),
    )
    .part(
        "tail_fin",
        (19, 20),
        [-5.0, -0.5, 0.0, 10.0, 1.0, 6.0],
        PartPose::offset(0.0, 0.0, 9.0),
    );
    body.part(
        "head",
        (0, 0),
        [-4.0, -3.0, -3.0, 8.0, 7.0, 6.0],
        PartPose::offset(0.0, -4.0, -3.0),
    )
    .part(
        "nose",
        (0, 13),
        [-1.0, 2.0, -7.0, 2.0, 2.0, 4.0],
        PartPose::ZERO,
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn baby_dolphin_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let body = mesh.root().part(
        "body",
        (20, 0),
        [-3.0, -2.5, -4.0, 6.0, 5.0, 8.0],
        PartPose::offset(0.0, 21.5, 0.0),
    );
    body.part(
        "head",
        (0, 0),
        [-3.0, -3.5, -4.0, 6.0, 5.0, 4.0],
        PartPose::offset(0.0, 1.0, -4.0),
    )
    .part(
        "nose",
        (0, 9),
        [-1.0, -1.0, -2.0, 2.0, 2.0, 2.0],
        PartPose::offset(0.0, 0.5, -4.0),
    );
    body.part(
        "left_fin",
        (34, 18),
        [-0.5, -1.5, -0.5, 1.0, 3.0, 6.0],
        PartPose::offset_rotation(1.8, 0.85, -2.6, 0.8727, 0.0, 1.7017),
    );
    body.child(
        "right_fin",
        CubeList::new()
            .tex_offs(48, 18)
            .mirror()
            .add_box(-0.5, -1.5, -0.5, 1.0, 3.0, 6.0)
            .mirror_if(false),
        PartPose::offset_rotation(-1.8, 0.85, -2.6, 0.8727, 0.0, -1.7017),
    );
    body.part(
        "tail",
        (0, 13),
        [-2.0, -1.5, 0.0, 4.0, 3.0, 7.0],
        PartPose::offset(0.0, 1.0, 4.0),
    )
    .part(
        "tail_fin",
        (22, 13),
        [-4.0, -0.5, -1.0, 8.0, 1.0, 4.0],
        PartPose::offset(0.0, 0.0, 6.0),
    );
    body.part(
        "back_fin",
        (42, 0),
        [-0.5, -1.0, 1.0, 1.0, 3.0, 4.0],
        PartPose::offset_rotation(0.0, -1.0, -2.7, 0.8727, 0.0, 0.0),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let body = model.id("body");
    parts[body].x_rot = st.x_rot * DEG_TO_RAD;
    parts[body].y_rot = st.y_rot * DEG_TO_RAD;
    if st.extras.moving {
        let wobble = (st.age_ticks * 0.3).cos();
        parts[body].x_rot += -0.05 - 0.05 * wobble;
        parts[model.id("tail")].x_rot = -0.1 * wobble;
        parts[model.id("tail_fin")].x_rot = -0.2 * wobble;
    }
}
