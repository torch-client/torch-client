use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;

use super::{quadruped, scaling};

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-3.5, -3.0, -3.0, 7.0, 7.0, 7.0)
            .tex_offs(0, 44)
            .add_box(-2.5, 1.0, -6.0, 5.0, 3.0, 3.0)
            .tex_offs(26, 0)
            .add_box(-4.5, -4.0, -1.0, 2.0, 2.0, 1.0)
            .tex_offs(26, 0)
            .mirror()
            .add_box(2.5, -4.0, -1.0, 2.0, 2.0, 1.0),
        PartPose::offset(0.0, 10.0, -16.0),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 19)
            .add_box(-5.0, -13.0, -7.0, 14.0, 14.0, 11.0)
            .tex_offs(39, 0)
            .add_box(-4.0, -25.0, -7.0, 12.0, 12.0, 10.0),
        PartPose::offset_rotation(-2.0, 9.0, 12.0, std::f32::consts::FRAC_PI_2, 0.0, 0.0),
    );
    let hind_leg = CubeList::new()
        .tex_offs(50, 22)
        .add_box(-2.0, 0.0, -2.0, 4.0, 10.0, 8.0);
    root.child(
        "right_hind_leg",
        hind_leg.clone(),
        PartPose::offset(-4.5, 14.0, 6.0),
    );
    root.child("left_hind_leg", hind_leg, PartPose::offset(4.5, 14.0, 6.0));
    let front_leg = CubeList::new()
        .tex_offs(50, 40)
        .add_box(-2.0, 0.0, -2.0, 4.0, 10.0, 6.0);
    root.child(
        "right_front_leg",
        front_leg.clone(),
        PartPose::offset(-3.5, 14.0, -8.0),
    );
    root.child(
        "left_front_leg",
        front_leg,
        PartPose::offset(3.5, 14.0, -8.0),
    );
    LayerDef::create(scaling(mesh, 1.2), 128, 64)
}

pub fn baby_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 9)
            .add_box(-4.0, -3.5, -6.0, 8.0, 7.0, 12.0),
        PartPose::offset(0.0, 17.5, 0.0),
    );
    root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-3.0, -2.625, -4.25, 6.0, 5.0, 4.0)
            .tex_offs(20, 3)
            .add_box(-2.0, 0.375, -6.25, 4.0, 2.0, 2.0)
            .tex_offs(20, 0)
            .add_box(-4.0, -3.625, -2.75, 2.0, 2.0, 1.0)
            .tex_offs(26, 0)
            .add_box(2.0, -3.625, -2.75, 2.0, 2.0, 1.0),
        PartPose::offset(0.0, 18.625, -5.75),
    );
    root.child(
        "right_hind_leg",
        CubeList::new()
            .tex_offs(0, 34)
            .add_box(-1.5, -0.5, -1.5, 3.0, 3.0, 3.0),
        PartPose::offset(-2.5, 21.5, 4.5),
    );
    root.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(12, 34)
            .add_box(-1.5, -0.5, -1.5, 3.0, 3.0, 3.0),
        PartPose::offset(2.5, 21.5, 4.5),
    );
    root.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(0, 28)
            .add_box(-1.5, -0.5, -1.5, 3.0, 3.0, 3.0),
        PartPose::offset(-2.5, 21.5, -4.5),
    );
    root.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(12, 28)
            .add_box(-1.5, -0.5, -1.5, 3.0, 3.0, 3.0),
        PartPose::offset(2.5, 21.5, -4.5),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    quadruped::setup_anim(model, parts, st);
    let stand = st.extras.stand_scale * st.extras.stand_scale;
    let body_age_scale = st.age_scale;
    let head = model.id("head");
    let body = model.id("body");
    let right_front = model.id("right_front_leg");
    let left_front = model.id("left_front_leg");

    parts[body].x_rot -= stand * std::f32::consts::PI * 0.35;
    parts[body].y += stand * body_age_scale * 2.0;
    parts[right_front].y -= stand * body_age_scale * 20.0;
    parts[right_front].z += stand * body_age_scale * 4.0;
    parts[right_front].x_rot -= stand * std::f32::consts::PI * 0.45;
    parts[left_front].y = parts[right_front].y;
    parts[left_front].z = parts[right_front].z;
    parts[left_front].x_rot -= stand * std::f32::consts::PI * 0.45;
    parts[head].y -= stand * 24.0;
    parts[head].z += stand * 13.0;
    parts[head].x_rot += stand * std::f32::consts::PI * 0.15;
}
