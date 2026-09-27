#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::models::aquatic::DEG_TO_RAD;
use crate::entities::state::EntityState;

pub fn dolphin_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let body = mesh.root().child(
        "body",
        CubeList::new()
            .tex_offs(22, 0)
            .add_box(-4.0, -7.0, 0.0, 8.0, 7.0, 13.0),
        PartPose::offset(0.0, 22.0, -5.0),
    );
    body.child(
        "back_fin",
        CubeList::new()
            .tex_offs(51, 0)
            .add_box(-0.5, 0.0, 8.0, 1.0, 4.0, 5.0),
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
    body.child(
        "right_fin",
        CubeList::new()
            .tex_offs(48, 20)
            .add_box(-0.5, -4.0, 0.0, 1.0, 4.0, 7.0),
        PartPose::offset_rotation(-2.0, -2.0, 4.0, 1.047_197_6, 0.0, -2.094_395_2),
    );
    body.child(
        "tail",
        CubeList::new()
            .tex_offs(0, 19)
            .add_box(-2.0, -2.5, 0.0, 4.0, 5.0, 11.0),
        PartPose::offset_rotation(0.0, -2.5, 11.0, -0.104_719_76, 0.0, 0.0),
    )
    .child(
        "tail_fin",
        CubeList::new()
            .tex_offs(19, 20)
            .add_box(-5.0, -0.5, 0.0, 10.0, 1.0, 6.0),
        PartPose::offset(0.0, 0.0, 9.0),
    );
    body.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -3.0, -3.0, 8.0, 7.0, 6.0),
        PartPose::offset(0.0, -4.0, -3.0),
    )
    .child(
        "nose",
        CubeList::new()
            .tex_offs(0, 13)
            .add_box(-1.0, 2.0, -7.0, 2.0, 2.0, 4.0),
        PartPose::ZERO,
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn baby_dolphin_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let body = mesh.root().child(
        "body",
        CubeList::new()
            .tex_offs(20, 0)
            .add_box(-3.0, -2.5, -4.0, 6.0, 5.0, 8.0),
        PartPose::offset(0.0, 21.5, 0.0),
    );
    body.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-3.0, -3.5, -4.0, 6.0, 5.0, 4.0),
        PartPose::offset(0.0, 1.0, -4.0),
    )
    .child(
        "nose",
        CubeList::new()
            .tex_offs(0, 9)
            .add_box(-1.0, -1.0, -2.0, 2.0, 2.0, 2.0),
        PartPose::offset(0.0, 0.5, -4.0),
    );
    body.child(
        "left_fin",
        CubeList::new()
            .tex_offs(34, 18)
            .add_box(-0.5, -1.5, -0.5, 1.0, 3.0, 6.0),
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
    body.child(
        "tail",
        CubeList::new()
            .tex_offs(0, 13)
            .add_box(-2.0, -1.5, 0.0, 4.0, 3.0, 7.0),
        PartPose::offset(0.0, 1.0, 4.0),
    )
    .child(
        "tail_fin",
        CubeList::new()
            .tex_offs(22, 13)
            .add_box(-4.0, -0.5, -1.0, 8.0, 1.0, 4.0),
        PartPose::offset(0.0, 0.0, 6.0),
    );
    body.child(
        "back_fin",
        CubeList::new()
            .tex_offs(42, 0)
            .add_box(-0.5, -1.0, 1.0, 1.0, 3.0, 4.0),
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
