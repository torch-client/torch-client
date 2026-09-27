use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::keyframe;
use crate::entities::state::EntityState;

use super::animations::fox_baby;
use crate::util::mth::DEG_TO_RAD;

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let head = root.child(
        "head",
        CubeList::new()
            .tex_offs(1, 5)
            .add_box(-3.0, -2.0, -5.0, 8.0, 6.0, 6.0),
        PartPose::offset(-1.0, 16.5, -3.0),
    );
    head.child(
        "right_ear",
        CubeList::new()
            .tex_offs(8, 1)
            .add_box(-3.0, -4.0, -4.0, 2.0, 2.0, 1.0),
        PartPose::ZERO,
    );
    head.child(
        "left_ear",
        CubeList::new()
            .tex_offs(15, 1)
            .add_box(3.0, -4.0, -4.0, 2.0, 2.0, 1.0),
        PartPose::ZERO,
    );
    head.child(
        "nose",
        CubeList::new()
            .tex_offs(6, 18)
            .add_box(-1.0, 2.01, -8.0, 4.0, 2.0, 3.0),
        PartPose::ZERO,
    );
    let body = root.child(
        "body",
        CubeList::new()
            .tex_offs(24, 15)
            .add_box(-3.0, 3.999, -3.5, 6.0, 11.0, 6.0),
        PartPose::offset_rotation(0.0, 16.0, -6.0, std::f32::consts::FRAC_PI_2, 0.0, 0.0),
    );
    body.child(
        "tail",
        CubeList::new()
            .tex_offs(30, 0)
            .add_box(2.0, 0.0, -1.0, 4.0, 9.0, 5.0),
        PartPose::offset_rotation(-4.0, 15.0, -1.0, -0.05235988, 0.0, 0.0),
    );
    let fudge = Grow::all(0.001);
    let left_leg = CubeList::new()
        .tex_offs(4, 24)
        .add_box_grow(2.0, 0.5, -1.0, 2.0, 6.0, 2.0, fudge);
    let right_leg = CubeList::new()
        .tex_offs(13, 24)
        .add_box_grow(2.0, 0.5, -1.0, 2.0, 6.0, 2.0, fudge);
    root.child(
        "right_hind_leg",
        right_leg.clone(),
        PartPose::offset(-5.0, 17.5, 7.0),
    );
    root.child(
        "left_hind_leg",
        left_leg.clone(),
        PartPose::offset(-1.0, 17.5, 7.0),
    );
    root.child(
        "right_front_leg",
        right_leg,
        PartPose::offset(-5.0, 17.5, 0.0),
    );
    root.child(
        "left_front_leg",
        left_leg,
        PartPose::offset(-1.0, 17.5, 0.0),
    );
    LayerDef::create(mesh, 48, 32)
}

pub fn baby_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-3.0, -2.125, -5.125, 6.0, 5.0, 5.0)
            .tex_offs(18, 20)
            .add_box(-1.0, 0.875, -7.125, 2.0, 2.0, 2.0)
            .tex_offs(22, 8)
            .add_box(-3.0, -4.125, -4.125, 2.0, 2.0, 1.0)
            .tex_offs(22, 11)
            .add_box(1.0, -4.125, -4.125, 2.0, 2.0, 1.0),
        PartPose::offset(0.0, 18.125, 0.125),
    );
    root.child(
        "right_hind_leg",
        CubeList::new()
            .tex_offs(22, 4)
            .add_box(-1.0, 0.0, -1.0, 2.0, 2.0, 2.0),
        PartPose::offset(-1.5, 22.0, 4.0),
    );
    root.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(22, 0)
            .add_box(-1.0, 0.0, -1.0, 2.0, 2.0, 2.0),
        PartPose::offset(1.5, 22.0, 4.0),
    );
    root.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(22, 4)
            .add_box(-1.0, 0.0, -1.0, 2.0, 2.0, 2.0),
        PartPose::offset(-1.5, 22.0, 0.0),
    );
    root.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(22, 0)
            .add_box(-1.0, 0.0, -1.0, 2.0, 2.0, 2.0),
        PartPose::offset(1.5, 22.0, 0.0),
    );
    let body = root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 10)
            .add_box(-2.5, -2.0, -3.0, 5.0, 4.0, 6.0),
        PartPose::offset(0.0, 20.0, 2.0),
    );
    body.child(
        "tail",
        CubeList::new()
            .tex_offs(0, 20)
            .add_box(-1.5, -1.48, -1.0, 3.0, 3.0, 6.0),
        PartPose::offset(0.0, -0.5, 3.0),
    );
    LayerDef::create(mesh, 32, 32)
}

struct Ids {
    head: usize,
    body: usize,
    tail: usize,
    right_hind: usize,
    left_hind: usize,
    right_front: usize,
    left_front: usize,
}

impl Ids {
    fn new(model: &BakedModel) -> Ids {
        Ids {
            head: model.id("head"),
            body: model.id("body"),
            tail: model.id("tail"),
            right_hind: model.id("right_hind_leg"),
            left_hind: model.id("left_hind_leg"),
            right_front: model.id("right_front_leg"),
            left_front: model.id("left_front_leg"),
        }
    }
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let id = Ids::new(model);
    walking_pose(parts, &id, st);
    let pos = st.walk_pos;
    let speed = st.walk_speed;
    parts[id.right_hind].x_rot = (pos * 0.6662).cos() * 1.4 * speed;
    parts[id.left_hind].x_rot = (pos * 0.6662 + std::f32::consts::PI).cos() * 1.4 * speed;
    parts[id.right_front].x_rot = (pos * 0.6662 + std::f32::consts::PI).cos() * 1.4 * speed;
    parts[id.left_front].x_rot = (pos * 0.6662).cos() * 1.4 * speed;

    if st.extras.crouching {
        crouching_pose(parts, &id, st);
        parts[id.body].y += st.extras.crouch_amount;
    } else if st.extras.sleeping {
        sleeping_pose(parts, &id);
        parts[id.body].z_rot = -std::f32::consts::FRAC_PI_2;
        parts[id.body].y += 5.0;
        parts[id.tail].x_rot = -2.6179938;
        parts[id.head].x += 2.0;
        parts[id.head].y += 2.99;
        parts[id.head].y_rot = -2.0943952;
        parts[id.head].z_rot = 0.0;
    } else if st.extras.sitting {
        sitting_pose(parts, &id);
        parts[id.body].x_rot = 0.5235988;
        parts[id.body].y -= 7.0;
        parts[id.body].z += 3.0;
        parts[id.tail].x_rot = 0.7853982;
        parts[id.head].y -= 6.5;
        parts[id.head].z += 2.75;
        parts[id.right_front].x_rot = -0.2617994;
        parts[id.left_front].x_rot = -0.2617994;
        parts[id.right_hind].x_rot = -1.3089969;
        parts[id.right_hind].y += 4.0;
        parts[id.right_hind].z -= 0.25;
        parts[id.left_hind].x_rot = -1.3089969;
        parts[id.left_hind].y += 4.0;
        parts[id.left_hind].z -= 0.25;
        parts[id.tail].z -= 1.0;
    }

    if st.extras.pouncing {
        let crouch = st.extras.crouch_amount / 2.0;
        parts[id.body].y -= crouch;
        parts[id.head].y -= crouch;
    }

    finish(parts, &id, st);
}

pub fn baby_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let id = Ids::new(model);
    walking_pose(parts, &id, st);
    keyframe::apply_walk(
        &fox_baby::FOX_BABY_WALK,
        model,
        parts,
        st.walk_pos,
        st.walk_speed,
        1.0,
        2.5,
    );

    let age_scale = st.age_scale;
    if st.extras.crouching {
        crouching_pose(parts, &id, st);
        parts[id.body].y += st.extras.crouch_amount / 6.0;
    } else if st.extras.sleeping {
        sleeping_pose(parts, &id);
        parts[id.body].z_rot = -std::f32::consts::FRAC_PI_2;
        parts[id.body].x_rot = -0.17453292;
        parts[id.body].y += 1.0;
        parts[id.body].z -= 1.0;
        parts[id.body].x -= 1.0;
        parts[id.tail].x_rot = -2.1816616;
        parts[id.tail].x -= 0.7;
        parts[id.tail].z += 0.6;
        parts[id.tail].y += 0.9;
        parts[id.head].x -= 2.0;
        parts[id.head].y += 2.8;
        parts[id.head].z -= 4.0;
        parts[id.head].y_rot = -2.0943952;
        parts[id.head].z_rot = 0.0;
    } else if st.extras.sitting {
        sitting_pose(parts, &id);
        parts[id.body].x_rot = -0.959931;
        parts[id.body].z -= 4.5 * age_scale;
        parts[id.body].y += 3.0 * age_scale;
        parts[id.tail].y -= 0.6;
        parts[id.tail].z -= 2.0 * age_scale;
        parts[id.tail].x_rot = 0.95993114;
        parts[id.head].y -= 0.75;
        parts[id.right_front].x_rot = -0.2617994;
        parts[id.left_front].x_rot = -0.2617994;
        parts[id.right_front].z -= 1.0;
        parts[id.left_front].z -= 1.0;
        parts[id.right_front].x += 0.01;
        parts[id.left_front].x -= 0.01;
        parts[id.right_hind].z -= 3.75;
        parts[id.left_hind].z -= 3.75;
        parts[id.right_hind].x += 0.01;
        parts[id.left_hind].x -= 0.01;
    }

    finish(parts, &id, st);
}

fn walking_pose(parts: &mut [PartState], id: &Ids, st: &EntityState) {
    parts[id.head].z_rot = st.extras.head_roll_angle;
    parts[id.right_hind].visible = true;
    parts[id.left_hind].visible = true;
    parts[id.right_front].visible = true;
    parts[id.left_front].visible = true;
}

fn sitting_pose(parts: &mut [PartState], id: &Ids) {
    parts[id.head].x_rot = 0.0;
    parts[id.head].y_rot = 0.0;
}

fn sleeping_pose(parts: &mut [PartState], id: &Ids) {
    parts[id.right_hind].visible = false;
    parts[id.left_hind].visible = false;
    parts[id.right_front].visible = false;
    parts[id.left_front].visible = false;
}

fn crouching_pose(parts: &mut [PartState], id: &Ids, st: &EntityState) {
    parts[id.body].x_rot += 0.10471976;
    parts[id.head].y += st.extras.crouch_amount * st.age_scale;
    let wiggle = st.age_ticks.cos() * 0.05;
    parts[id.body].y_rot = wiggle;
    parts[id.right_hind].z_rot = wiggle;
    parts[id.left_hind].z_rot = wiggle;
    parts[id.right_front].z_rot = wiggle / 2.0;
    parts[id.left_front].z_rot = wiggle / 2.0;
}

fn finish(parts: &mut [PartState], id: &Ids, st: &EntityState) {
    let e = &st.extras;
    if !e.sleeping && !e.faceplanted && !e.crouching {
        parts[id.head].x_rot = st.x_rot * DEG_TO_RAD;
        parts[id.head].y_rot = st.y_rot * DEG_TO_RAD;
    }
    if e.sleeping {
        parts[id.head].x_rot = 0.0;
        parts[id.head].y_rot = -2.0943952;
        parts[id.head].z_rot = (st.age_ticks * 0.027).cos() / 22.0;
    }
    if e.faceplanted {
        let leg_motion_pos = st.age_ticks * 0.67;
        parts[id.right_hind].x_rot = (leg_motion_pos * 0.4662).cos() * 0.1;
        parts[id.left_hind].x_rot = (leg_motion_pos * 0.4662 + std::f32::consts::PI).cos() * 0.1;
        parts[id.right_front].x_rot = (leg_motion_pos * 0.4662 + std::f32::consts::PI).cos() * 0.1;
        parts[id.left_front].x_rot = (leg_motion_pos * 0.4662).cos() * 0.1;
    }
}
