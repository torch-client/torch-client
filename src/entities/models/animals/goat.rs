use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;

use super::quadruped;

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let head = root.child(
        "head",
        CubeList::new()
            .tex_offs(2, 61)
            .add_box(-6.0, -11.0, -10.0, 3.0, 2.0, 1.0)
            .tex_offs(2, 61)
            .mirror()
            .add_box(2.0, -11.0, -10.0, 3.0, 2.0, 1.0)
            .mirror_if(false)
            .tex_offs(23, 52)
            .add_box(-0.5, -3.0, -14.0, 0.0, 7.0, 5.0),
        PartPose::offset(1.0, 14.0, 0.0),
    );
    head.child(
        "left_horn",
        CubeList::new()
            .tex_offs(12, 55)
            .add_box(-0.01, -16.0, -10.0, 2.0, 7.0, 2.0),
        PartPose::offset(0.0, 0.0, 0.0),
    );
    head.child(
        "right_horn",
        CubeList::new()
            .tex_offs(12, 55)
            .add_box(-2.99, -16.0, -10.0, 2.0, 7.0, 2.0),
        PartPose::offset(0.0, 0.0, 0.0),
    );
    head.child(
        "nose",
        CubeList::new()
            .tex_offs(34, 46)
            .add_box(-3.0, -4.0, -8.0, 5.0, 7.0, 10.0),
        PartPose::offset_rotation(0.0, -8.0, -8.0, 0.9599, 0.0, 0.0),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(1, 1)
            .add_box(-4.0, -17.0, -7.0, 9.0, 11.0, 16.0)
            .tex_offs(0, 28)
            .add_box(-5.0, -18.0, -8.0, 11.0, 14.0, 11.0),
        PartPose::offset(0.0, 24.0, 0.0),
    );
    root.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(36, 29)
            .add_box(0.0, 4.0, 0.0, 3.0, 6.0, 3.0),
        PartPose::offset(1.0, 14.0, 4.0),
    );
    root.child(
        "right_hind_leg",
        CubeList::new()
            .tex_offs(49, 29)
            .add_box(0.0, 4.0, 0.0, 3.0, 6.0, 3.0),
        PartPose::offset(-3.0, 14.0, 4.0),
    );
    root.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(49, 2)
            .add_box(0.0, 0.0, 0.0, 3.0, 10.0, 3.0),
        PartPose::offset(1.0, 14.0, -6.0),
    );
    root.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(35, 2)
            .add_box(0.0, 0.0, 0.0, 3.0, 10.0, 3.0),
        PartPose::offset(-3.0, 14.0, -6.0),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn baby_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(29, 12)
            .add_box(-1.0, -0.5, -1.0, 2.0, 5.0, 2.0),
        PartPose::offset(1.5, 19.5, 3.0),
    );
    root.child(
        "right_hind_leg",
        CubeList::new()
            .tex_offs(21, 12)
            .add_box(-1.0, -0.5, -1.0, 2.0, 5.0, 2.0),
        PartPose::offset(-1.5, 19.5, 3.0),
    );
    root.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(21, 5)
            .add_box(-1.0, -0.5, -1.0, 2.0, 5.0, 2.0),
        PartPose::offset(-1.5, 19.5, -2.0),
    );
    root.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(29, 5)
            .add_box(-1.0, -0.5, -1.0, 2.0, 5.0, 2.0),
        PartPose::offset(1.5, 19.5, -2.0),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 10)
            .add_box(-3.0, -2.3, -4.5, 6.0, 5.0, 9.0)
            .tex_offs(0, 24)
            .add_box(-2.5, -2.2, -4.0, 5.0, 4.0, 8.0),
        PartPose::offset(0.0, 17.8, 0.0),
    );
    let head = root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-2.0, -3.8126, -5.1548, 4.0, 4.0, 6.0),
        PartPose::offset_rotation(0.0, 15.5, -3.0, 0.4363, 0.0, 0.0),
    );
    head.child(
        "right_horn",
        CubeList::new()
            .tex_offs(24, 0)
            .mirror()
            .add_box(0.0, -4.5, 0.0, 1.0, 2.0, 1.0)
            .mirror_if(false),
        PartPose::offset_rotation(-1.5, -1.5, -1.0, -0.3926991, 0.0, 0.0),
    );
    head.child(
        "left_horn",
        CubeList::new()
            .tex_offs(24, 0)
            .mirror()
            .add_box(2.0, -4.5, 0.0, 1.0, 2.0, 1.0)
            .mirror_if(false),
        PartPose::offset_rotation(-1.5, -1.5, -1.0, -0.3926991, 0.0, 0.0),
    );
    head.child(
        "right_ear",
        CubeList::new()
            .tex_offs(0, 12)
            .mirror()
            .add_box(-2.0, -0.5, -0.5, 2.0, 1.0, 1.0)
            .mirror_if(false),
        PartPose::offset_rotation(-1.7, -2.3126, 0.1452, 0.0, -0.5236, 0.0),
    );
    head.child(
        "left_ear",
        CubeList::new()
            .tex_offs(0, 12)
            .add_box(0.0, -0.5, -0.5, 2.0, 1.0, 1.0),
        PartPose::offset_rotation(1.7, -2.3126, 0.1452, 0.0, 0.5236, 0.0),
    );
    head.child(
        "HeadMain",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-2.0, -2.5, -4.0, 4.0, 4.0, 6.0),
        PartPose::offset(0.0, -1.3126, -1.1548),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    quadruped::setup_anim(model, parts, st);
    parts[model.id("left_horn")].visible = st.extras.left_horn;
    parts[model.id("right_horn")].visible = st.extras.right_horn;
    if st.extras.ramming_x_head_rot != 0.0 {
        parts[model.id("head")].x_rot = st.extras.ramming_x_head_rot;
    }
}

pub fn baby_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup_anim(model, parts, st);
    if st.extras.ramming_x_head_rot == 0.0 {
        parts[model.id("head")].x_rot = 0.3926991;
    }
}
