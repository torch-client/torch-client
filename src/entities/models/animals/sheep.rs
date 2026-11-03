use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;

use super::quadruped;

pub fn layer() -> LayerDef {
    let mut mesh = quadruped::body_mesh(12, false, true, Grow::NONE);
    let root = mesh.root();
    root.part(
        "head",
        (0, 0),
        [-3.0, -4.0, -6.0, 6.0, 6.0, 8.0],
        PartPose::offset(0.0, 6.0, -8.0),
    );
    root.part(
        "body",
        (28, 8),
        [-4.0, -10.0, -7.0, 8.0, 16.0, 6.0],
        PartPose::offset_rotation(0.0, 5.0, 2.0, std::f32::consts::FRAC_PI_2, 0.0, 0.0),
    );
    LayerDef::create(mesh, 64, 32)
}

pub fn baby_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.part(
        "body",
        (0, 10),
        [-3.0, -2.0, -4.5, 6.0, 4.0, 9.0],
        PartPose::offset(0.0, 17.0, 0.5),
    );
    root.part(
        "head",
        (0, 0),
        [-2.5, -4.5, -3.5, 5.0, 5.0, 5.0],
        PartPose::offset(0.0, 15.5, -2.5),
    );
    root.part(
        "right_hind_leg",
        (0, 23),
        [-1.0, 0.0, -1.0, 2.0, 5.0, 2.0],
        PartPose::offset(-2.0, 19.0, 3.0),
    );
    root.part(
        "left_hind_leg",
        (24, 12),
        [-1.0, 0.0, -1.0, 2.0, 5.0, 2.0],
        PartPose::offset(2.0, 19.0, 3.0),
    );
    root.part(
        "right_front_leg",
        (8, 23),
        [-1.0, 0.0, -1.0, 2.0, 5.0, 2.0],
        PartPose::offset(-2.0, 19.0, -2.0),
    );
    root.part(
        "left_front_leg",
        (24, 5),
        [-1.0, 0.0, -1.0, 2.0, 5.0, 2.0],
        PartPose::offset(2.0, 19.0, -2.0),
    );
    LayerDef::create(mesh, 64, 32)
}

pub fn fur_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.part_grow(
        "head",
        (0, 0),
        [-3.0, -4.0, -4.0, 6.0, 6.0, 6.0],
        Grow::all(0.6),
        PartPose::offset(0.0, 6.0, -8.0),
    );
    root.part_grow(
        "body",
        (28, 8),
        [-4.0, -10.0, -7.0, 8.0, 16.0, 6.0],
        Grow::all(1.75),
        PartPose::offset_rotation(0.0, 5.0, 2.0, std::f32::consts::FRAC_PI_2, 0.0, 0.0),
    );
    let leg = CubeList::new().tex_offs(0, 16).add_box_grow(
        -2.0,
        0.0,
        -2.0,
        4.0,
        6.0,
        4.0,
        Grow::all(0.5),
    );
    root.child(
        "right_hind_leg",
        leg.clone(),
        PartPose::offset(-3.0, 12.0, 7.0),
    );
    root.child(
        "left_hind_leg",
        leg.clone(),
        PartPose::offset(3.0, 12.0, 7.0),
    );
    root.child(
        "right_front_leg",
        leg.clone(),
        PartPose::offset(-3.0, 12.0, -5.0),
    );
    root.child("left_front_leg", leg, PartPose::offset(3.0, 12.0, -5.0));
    LayerDef::create(mesh, 64, 32)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    quadruped::setup_anim(model, parts, st);
    let head = model.id("head");
    parts[head].y += st.extras.head_eat_position_scale * 9.0 * st.age_scale;
    parts[head].x_rot = st.extras.head_eat_angle_scale;
}
