#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::models::aquatic::DEG_TO_RAD;
use crate::entities::state::EntityState;
use crate::util::mth::rot_lerp_rad;

pub fn adult_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let bone = mesh
        .root()
        .child("bone", CubeList::new(), PartPose::offset(0.0, 19.0, 0.0));
    let body = bone.child(
        "body",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-3.5, -4.0, -5.0, 7.0, 7.0, 10.0),
        PartPose::ZERO,
    );
    body.child(
        "stinger",
        CubeList::new()
            .tex_offs(26, 7)
            .add_box(0.0, -1.0, 5.0, 0.0, 1.0, 2.0),
        PartPose::ZERO,
    );
    body.child(
        "left_antenna",
        CubeList::new()
            .tex_offs(2, 0)
            .add_box(1.5, -2.0, -3.0, 1.0, 2.0, 3.0),
        PartPose::offset(0.0, -2.0, -5.0),
    );
    body.child(
        "right_antenna",
        CubeList::new()
            .tex_offs(2, 3)
            .add_box(-2.5, -2.0, -3.0, 1.0, 2.0, 3.0),
        PartPose::offset(0.0, -2.0, -5.0),
    );
    let wing = Grow::all(0.001);
    bone.child(
        "right_wing",
        CubeList::new()
            .tex_offs(0, 18)
            .add_box_grow(-9.0, 0.0, 0.0, 9.0, 0.0, 6.0, wing),
        PartPose::offset_rotation(-1.5, -4.0, -3.0, 0.0, -0.2618, 0.0),
    );
    bone.child(
        "left_wing",
        CubeList::new()
            .tex_offs(0, 18)
            .mirror()
            .add_box_grow(0.0, 0.0, 0.0, 9.0, 0.0, 6.0, wing),
        PartPose::offset_rotation(1.5, -4.0, -3.0, 0.0, 0.2618, 0.0),
    );
    bone.child(
        "front_legs",
        CubeList::new().add_box_at(-5.0, 0.0, 0.0, 7.0, 2.0, 0.0, Grow::NONE, 26, 1),
        PartPose::offset(1.5, 3.0, -2.0),
    );
    bone.child(
        "middle_legs",
        CubeList::new().add_box_at(-5.0, 0.0, 0.0, 7.0, 2.0, 0.0, Grow::NONE, 26, 3),
        PartPose::offset(1.5, 3.0, 0.0),
    );
    bone.child(
        "back_legs",
        CubeList::new().add_box_at(-5.0, 0.0, 0.0, 7.0, 2.0, 0.0, Grow::NONE, 26, 5),
        PartPose::offset(1.5, 3.0, 2.0),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn baby_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let bone = mesh.root().child(
        "bone",
        CubeList::new()
            .tex_offs(6, 12)
            .add_box(1.0, -1.6667, -2.1633, 1.0, 2.0, 2.0)
            .tex_offs(0, 12)
            .add_box(-2.0, -1.6667, -2.1933, 1.0, 2.0, 2.0),
        PartPose::offset(0.0, 19.6667, -1.8567),
    );
    bone.child(
        "body",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-2.0, -2.0, -2.5, 4.0, 4.0, 5.0),
        PartPose::offset(0.0, 1.3333, 2.3567),
    )
    .child(
        "stinger",
        CubeList::new()
            .tex_offs(13, 2)
            .add_box(0.0, -0.5, 0.0, 0.0, 1.0, 1.0),
        PartPose::offset(0.0, 0.5, 2.5),
    );
    bone.child(
        "right_wing",
        CubeList::new()
            .tex_offs(3, 9)
            .add_box(-3.0, 0.0, 0.0, 3.0, 0.0, 3.0),
        PartPose::offset_rotation(-1.0, -0.6667, 0.8567, 0.2182, 0.3491, 0.0),
    );
    bone.child(
        "left_wing",
        CubeList::new()
            .tex_offs(-3, 9)
            .mirror()
            .add_box(0.0, 0.0, 0.0, 3.0, 0.0, 3.0)
            .mirror_if(false),
        PartPose::offset_rotation(1.0, -0.6667, 0.8567, 0.2182, -0.3491, 0.0),
    );
    bone.child(
        "front_legs",
        CubeList::new()
            .tex_offs(13, 0)
            .add_box(-1.5, 0.0, 0.0, 3.0, 1.0, 0.0),
        PartPose::offset(0.0, 3.3333, 1.8567),
    );
    bone.child(
        "middle_legs",
        CubeList::new()
            .tex_offs(13, 1)
            .add_box(-1.5, 0.0, 0.0, 3.0, 1.0, 0.0),
        PartPose::offset(0.0, 3.3333, 2.8567),
    );
    bone.child(
        "back_legs",
        CubeList::new()
            .tex_offs(13, 2)
            .add_box(-1.5, 0.0, 0.0, 3.0, 1.0, 0.0),
        PartPose::offset(0.0, 3.3333, 3.8567),
    );
    LayerDef::create(mesh, 32, 32)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let bone = model.id("bone");
    let front_leg = model.id("front_legs");
    let mid_leg = model.id("middle_legs");
    let back_leg = model.id("back_legs");
    let right_wing = model.id("right_wing");
    let left_wing = model.id("left_wing");

    parts[model.id("stinger")].visible = st.extras.has_stinger;

    if !st.extras.on_ground {
        let roll = st.age_ticks * 120.321_13 * DEG_TO_RAD;
        parts[right_wing].y_rot = 0.0;
        parts[right_wing].z_rot = roll.cos() * std::f32::consts::PI * 0.15;
        let right = parts[right_wing];
        parts[left_wing].x_rot = right.x_rot;
        parts[left_wing].y_rot = right.y_rot;
        parts[left_wing].z_rot = -right.z_rot;
        parts[front_leg].x_rot = 0.785_398_2;
        parts[mid_leg].x_rot = 0.785_398_2;
        parts[back_leg].x_rot = 0.785_398_2;
    }

    if !st.extras.bee_angry() && !st.extras.on_ground {
        let speed = (st.age_ticks * 0.18).cos();
        parts[bone].x_rot = 0.1 + speed * std::f32::consts::PI * 0.025;
        parts[bone].y -= (st.age_ticks * 0.18).cos() * 0.9;
        parts[front_leg].x_rot = -speed * std::f32::consts::PI * 0.1 + 0.392_699_1;
        parts[back_leg].x_rot = -speed * std::f32::consts::PI * 0.05 + 0.785_398_2;
        if let (Some(left), Some(right)) = (model.find("left_antenna"), model.find("right_antenna"))
        {
            parts[left].x_rot = speed * std::f32::consts::PI * 0.03;
            parts[right].x_rot = speed * std::f32::consts::PI * 0.03;
        }
    }

    let roll_amount = st.extras.bee_roll;
    if roll_amount > 0.0 {
        let current = parts[bone].x_rot;
        parts[bone].x_rot = rot_lerp_rad(roll_amount, current, 3.091_592_8);
    }
}
