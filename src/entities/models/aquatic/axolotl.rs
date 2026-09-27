#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::keyframe::{apply, apply_walk};
use crate::entities::models::aquatic::DEG_TO_RAD;
use crate::entities::models::aquatic::animation::baby_axolotl as anim;
use crate::entities::state::EntityState;

pub fn adult_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let body = mesh.root().child(
        "body",
        CubeList::new()
            .tex_offs(0, 11)
            .add_box(-4.0, -2.0, -9.0, 8.0, 4.0, 10.0)
            .tex_offs(2, 17)
            .add_box(0.0, -3.0, -8.0, 0.0, 5.0, 9.0),
        PartPose::offset(0.0, 19.5, 5.0),
    );
    let fudge = Grow::all(0.001);
    let head = body.child(
        "head",
        CubeList::new()
            .tex_offs(0, 1)
            .add_box_grow(-4.0, -3.0, -5.0, 8.0, 5.0, 5.0, fudge),
        PartPose::offset(0.0, 0.0, -9.0),
    );
    head.child(
        "top_gills",
        CubeList::new()
            .tex_offs(3, 37)
            .add_box_grow(-4.0, -3.0, 0.0, 8.0, 3.0, 0.0, fudge),
        PartPose::offset(0.0, -3.0, -1.0),
    );
    head.child(
        "left_gills",
        CubeList::new()
            .tex_offs(0, 40)
            .add_box_grow(-3.0, -5.0, 0.0, 3.0, 7.0, 0.0, fudge),
        PartPose::offset(-4.0, 0.0, -1.0),
    );
    head.child(
        "right_gills",
        CubeList::new()
            .tex_offs(11, 40)
            .add_box_grow(0.0, -5.0, 0.0, 3.0, 7.0, 0.0, fudge),
        PartPose::offset(4.0, 0.0, -1.0),
    );
    let left_leg = CubeList::new()
        .tex_offs(2, 13)
        .add_box_grow(-1.0, 0.0, 0.0, 3.0, 5.0, 0.0, fudge);
    let right_leg = CubeList::new()
        .tex_offs(2, 13)
        .add_box_grow(-2.0, 0.0, 0.0, 3.0, 5.0, 0.0, fudge);
    body.child(
        "right_hind_leg",
        right_leg.clone(),
        PartPose::offset(-3.5, 1.0, -1.0),
    );
    body.child(
        "left_hind_leg",
        left_leg.clone(),
        PartPose::offset(3.5, 1.0, -1.0),
    );
    body.child(
        "right_front_leg",
        right_leg,
        PartPose::offset(-3.5, 1.0, -8.0),
    );
    body.child("left_front_leg", left_leg, PartPose::offset(3.5, 1.0, -8.0));
    body.child(
        "tail",
        CubeList::new()
            .tex_offs(2, 19)
            .add_box(0.0, -3.0, 0.0, 0.0, 5.0, 12.0),
        PartPose::offset(0.0, 0.0, 1.0),
    );
    LayerDef::create(mesh, 64, 64)
}

struct Ids {
    body: usize,
    head: usize,
    tail: usize,
    top_gills: usize,
    left_gills: usize,
    right_gills: usize,
    left_hind_leg: usize,
    right_hind_leg: usize,
    left_front_leg: usize,
    right_front_leg: usize,
}

impl Ids {
    fn of(model: &BakedModel) -> Ids {
        Ids {
            body: model.id("body"),
            head: model.id("head"),
            tail: model.id("tail"),
            top_gills: model.id("top_gills"),
            left_gills: model.id("left_gills"),
            right_gills: model.id("right_gills"),
            left_hind_leg: model.id("left_hind_leg"),
            right_hind_leg: model.id("right_hind_leg"),
            left_front_leg: model.id("left_front_leg"),
            right_front_leg: model.id("right_front_leg"),
        }
    }
}

pub fn adult_setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let ids = Ids::of(model);
    let playing_dead = st.extras.playing_dead_factor;
    let in_water = st.extras.in_water_factor;
    let on_ground = st.extras.on_ground_factor;
    let moving = st.extras.moving_factor;
    let not_moving = 1.0 - moving;
    let mirrored_legs = 1.0 - on_ground.min(moving);

    parts[ids.body].y_rot += st.y_rot * DEG_TO_RAD;
    swimming(&ids, parts, st.age_ticks, st.x_rot, moving.min(in_water));
    water_hovering(&ids, parts, st.age_ticks, not_moving.min(in_water));
    ground_crawling(&ids, parts, st.age_ticks, moving.min(on_ground));
    lay_still_on_ground(&ids, parts, st.age_ticks, not_moving.min(on_ground));
    play_dead(&ids, parts, playing_dead);
    mirror_leg_rotations(&ids, parts, mirrored_legs);
}

fn swimming(ids: &Ids, parts: &mut [PartState], age_ticks: f32, x_rot: f32, factor: f32) {
    if factor <= 1.0e-5 {
        return;
    }
    let speed = age_ticks * 0.33;
    let sine = speed.sin();
    let cosine = speed.cos();
    let body_sway = 0.13 * sine;
    parts[ids.body].x_rot += (x_rot * DEG_TO_RAD + body_sway) * factor;
    parts[ids.head].x_rot -= body_sway * 1.8 * factor;
    parts[ids.body].y -= 0.45 * cosine * factor;
    parts[ids.top_gills].x_rot += (-0.5 * sine - 0.8) * factor;
    let gill_y_rot = (0.3 * sine + 0.9) * factor;
    parts[ids.left_gills].y_rot += gill_y_rot;
    parts[ids.right_gills].y_rot -= gill_y_rot;
    parts[ids.tail].y_rot += 0.3 * (speed * 0.9).cos() * factor;
    parts[ids.left_hind_leg].x_rot += 1.884_955_8 * factor;
    parts[ids.left_hind_leg].y_rot += -0.4 * sine * factor;
    parts[ids.left_hind_leg].z_rot += std::f32::consts::FRAC_PI_2 * factor;
    parts[ids.left_front_leg].x_rot += 1.884_955_8 * factor;
    parts[ids.left_front_leg].y_rot += (-0.2 * cosine - 0.1) * factor;
    parts[ids.left_front_leg].z_rot += std::f32::consts::FRAC_PI_2 * factor;
}

fn water_hovering(ids: &Ids, parts: &mut [PartState], age_ticks: f32, factor: f32) {
    if factor <= 1.0e-5 {
        return;
    }
    let speed = age_ticks * 0.075;
    let cosine = speed.cos();
    let sine = speed.sin() * 0.15;
    let body_x_rot = (-0.15 + 0.075 * cosine) * factor;
    parts[ids.body].x_rot += body_x_rot;
    parts[ids.body].y -= sine * factor;
    parts[ids.head].x_rot -= body_x_rot;
    parts[ids.top_gills].x_rot += 0.2 * cosine * factor;
    let gill_y_rot = (-0.3 * cosine - 0.19) * factor;
    parts[ids.left_gills].y_rot += gill_y_rot;
    parts[ids.right_gills].y_rot -= gill_y_rot;
    parts[ids.left_hind_leg].x_rot += (2.356_194_5 - cosine * 0.11) * factor;
    parts[ids.left_hind_leg].y_rot += 0.471_238_94 * factor;
    parts[ids.left_hind_leg].z_rot += 1.727_876_1 * factor;
    parts[ids.left_front_leg].x_rot += (0.785_398_2 - cosine * 0.2) * factor;
    parts[ids.left_front_leg].y_rot += 2.042_035 * factor;
    parts[ids.tail].y_rot += 0.5 * cosine * factor;
}

fn ground_crawling(ids: &Ids, parts: &mut [PartState], age_ticks: f32, factor: f32) {
    if factor <= 1.0e-5 {
        return;
    }
    let speed = age_ticks * 0.11;
    let cosine = speed.cos();
    let hind_leg_y_rot_sway = (cosine * cosine - 2.0 * cosine) / 5.0;
    let front_leg_y_rot_sway = 0.7 * cosine;
    let head_and_tail_y_rot = 0.09 * cosine * factor;
    parts[ids.head].y_rot += head_and_tail_y_rot;
    parts[ids.tail].y_rot += head_and_tail_y_rot;
    let gill_angle = (0.6 - 0.08 * (cosine * cosine + 2.0 * speed.sin())) * factor;
    parts[ids.top_gills].x_rot += gill_angle;
    parts[ids.left_gills].y_rot -= gill_angle;
    parts[ids.right_gills].y_rot += gill_angle;
    let hind_leg_x_rot = 0.942_477_9 * factor;
    let front_leg_x_rot = 1.099_557_4 * factor;
    parts[ids.left_hind_leg].x_rot += hind_leg_x_rot;
    parts[ids.left_hind_leg].y_rot += (1.5 - hind_leg_y_rot_sway) * factor;
    parts[ids.left_hind_leg].z_rot += -0.1 * factor;
    parts[ids.left_front_leg].x_rot += front_leg_x_rot;
    parts[ids.left_front_leg].y_rot +=
        (std::f32::consts::FRAC_PI_2 - front_leg_y_rot_sway) * factor;
    parts[ids.right_hind_leg].x_rot += hind_leg_x_rot;
    parts[ids.right_hind_leg].y_rot += (-1.0 - hind_leg_y_rot_sway) * factor;
    parts[ids.right_front_leg].x_rot += front_leg_x_rot;
    parts[ids.right_front_leg].y_rot +=
        (-std::f32::consts::FRAC_PI_2 - front_leg_y_rot_sway) * factor;
}

fn lay_still_on_ground(ids: &Ids, parts: &mut [PartState], age_ticks: f32, factor: f32) {
    if factor <= 1.0e-5 {
        return;
    }
    let speed = age_ticks * 0.09;
    let sine = speed.sin();
    let cosine = speed.cos();
    let movement = sine * sine - 2.0 * sine;
    let movement2 = cosine * cosine - 3.0 * sine;
    parts[ids.head].x_rot += -0.09 * movement * factor;
    parts[ids.head].z_rot += -0.2 * factor;
    parts[ids.tail].y_rot += (-0.1 + 0.1 * movement) * factor;
    let gill_angle = (0.6 + 0.05 * movement2) * factor;
    parts[ids.top_gills].x_rot += gill_angle;
    parts[ids.left_gills].y_rot -= gill_angle;
    parts[ids.right_gills].y_rot += gill_angle;
    parts[ids.left_hind_leg].x_rot += 1.1 * factor;
    parts[ids.left_hind_leg].y_rot += 1.0 * factor;
    parts[ids.left_front_leg].x_rot += 0.8 * factor;
    parts[ids.left_front_leg].y_rot += 2.3 * factor;
    parts[ids.left_front_leg].z_rot -= 0.5 * factor;
}

fn play_dead(ids: &Ids, parts: &mut [PartState], factor: f32) {
    if factor <= 1.0e-5 {
        return;
    }
    parts[ids.left_hind_leg].x_rot += 1.413_716_7 * factor;
    parts[ids.left_hind_leg].y_rot += 1.099_557_4 * factor;
    parts[ids.left_hind_leg].z_rot += 0.785_398_2 * factor;
    parts[ids.left_front_leg].x_rot += 0.785_398_2 * factor;
    parts[ids.left_front_leg].y_rot += 2.042_035 * factor;
    parts[ids.body].x_rot += -0.15 * factor;
    parts[ids.body].z_rot += 0.35 * factor;
}

fn mirror_leg_rotations(ids: &Ids, parts: &mut [PartState], factor: f32) {
    if factor <= 1.0e-5 {
        return;
    }
    let (hind, front) = (parts[ids.left_hind_leg], parts[ids.left_front_leg]);
    parts[ids.right_hind_leg].x_rot += hind.x_rot * factor;
    parts[ids.right_hind_leg].y_rot += -hind.y_rot * factor;
    parts[ids.right_hind_leg].z_rot += -hind.z_rot * factor;
    parts[ids.right_front_leg].x_rot += front.x_rot * factor;
    parts[ids.right_front_leg].y_rot += -front.y_rot * factor;
    parts[ids.right_front_leg].z_rot += -front.z_rot * factor;
}

pub fn baby_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh
        .root()
        .child("root", CubeList::new(), PartPose::offset(0.0, 24.0, 0.0));
    let body = root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-2.0, -0.75, -2.75, 4.0, 2.0, 6.0)
            .tex_offs(0, 12)
            .add_box(0.0, -1.75, -2.75, 0.0, 3.0, 5.0),
        PartPose::offset(0.0, -1.25, 1.75),
    );
    body.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(20, 16)
            .add_box(-3.0, 0.0, -0.5, 3.0, 0.0, 1.0),
        PartPose::offset(-2.0, 0.25, -1.25),
    );
    body.child(
        "right_hind_leg",
        CubeList::new(),
        PartPose::offset_rotation(-2.0, 0.25, 1.75, 0.0, 1.5708, 1.5708),
    )
    .child(
        "right_leg_r1",
        CubeList::new()
            .tex_offs(20, 14)
            .add_box(0.0, 0.0, -0.5, 3.0, 0.0, 1.0),
        PartPose::offset_rotation(0.0, 0.0, 0.0, -1.5708, 0.0, 1.5708),
    );
    body.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(20, 13)
            .add_box(0.0, 0.0, -0.5, 3.0, 0.0, 1.0),
        PartPose::offset(2.0, 0.25, -1.25),
    );
    body.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(20, 14)
            .add_box(0.0, 0.0, -0.5, 3.0, 0.0, 1.0),
        PartPose::offset(2.0, 0.25, 1.75),
    );
    body.child(
        "tail",
        CubeList::new()
            .tex_offs(10, 9)
            .add_box(0.0, -1.5, -1.0, 0.0, 3.0, 8.0),
        PartPose::offset(0.0, -0.25, 3.25),
    );
    let head = body.child(
        "head",
        CubeList::new()
            .tex_offs(0, 8)
            .add_box(-3.0, -2.0, -4.0, 6.0, 3.0, 4.0),
        PartPose::offset(0.0, 0.25, -2.75),
    );
    head.child(
        "left_gills",
        CubeList::new()
            .tex_offs(20, 8)
            .add_box(0.0, -3.5, 0.0, 3.0, 5.0, 0.0),
        PartPose::offset(3.0, -0.5, -2.0),
    );
    head.child(
        "right_gills",
        CubeList::new()
            .tex_offs(20, 3)
            .add_box(-3.0, -3.5, 0.0, 3.0, 5.0, 0.0),
        PartPose::offset(-3.0, -0.5, -2.0),
    );
    head.child(
        "top_gills",
        CubeList::new()
            .tex_offs(20, 0)
            .add_box(-3.0, -3.0, 0.0, 6.0, 3.0, 0.0),
        PartPose::offset(0.0, -2.0, -2.0),
    );
    LayerDef::create(mesh, 32, 32)
}

const MAX_WALK_ANIMATION_SPEED: f32 = 15.0;
const WALK_ANIMATION_SCALE_FACTOR: f32 = 30.0;

pub fn baby_setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let seconds = st.age_ticks / 20.0;
    let in_water = st.extras.in_water_factor;
    let on_ground = st.extras.on_ground_factor;
    let moving = st.extras.moving_factor;
    let not_moving = 1.0 - moving;

    if st.walk_speed > 0.0 {
        apply_walk(
            &anim::AXOLOTL_WALK_FLOOR,
            model,
            parts,
            st.walk_pos,
            st.walk_speed,
            MAX_WALK_ANIMATION_SPEED,
            WALK_ANIMATION_SCALE_FACTOR,
        );
    }

    let choices = [
        (moving.min(in_water), &anim::BABY_AXOLOTL_SWIM),
        (moving.min(on_ground), &anim::WALK_FLOOR_UNDERWATER),
        (not_moving.min(in_water), &anim::IDLE_UNDERWATER),
        (not_moving.min(on_ground), &anim::BABY_AXOLOTL_IDLE_FLOOR),
    ];
    if let Some((_, animation)) = choices
        .iter()
        .copied()
        .filter(|(weight, _)| *weight > 1.0e-5)
        .max_by(|a, b| a.0.total_cmp(&b.0))
    {
        apply(animation, model, parts, seconds, 1.0);
    }
    if not_moving.min(on_ground.min(in_water)) > 1.0e-5 {
        apply(&anim::IDLE_FLOOR_UNDERWATER, model, parts, seconds, 1.0);
    }
    if st.extras.playing_dead_factor > 1.0e-5 {
        apply(&anim::BABY_AXOLOTL_PLAY_DEAD, model, parts, seconds, 1.0);
    }
}
