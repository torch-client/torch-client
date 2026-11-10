use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::keyframe::{self, AnimationDefinition};
use crate::entities::state::EntityState;

use super::animations::{camel as adult_anim, camel_baby as baby_anim};
use crate::util::mth::DEG_TO_RAD;

const MAX_WALK_ANIMATION_SPEED: f32 = 2.0;
const WALK_ANIMATION_SCALE_FACTOR: f32 = 2.5;

fn adult_mesh() -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let body = root.part(
        "body",
        (0, 25),
        [-7.5, -12.0, -23.5, 15.0, 12.0, 27.0],
        PartPose::offset(0.0, 4.0, 9.5),
    );
    body.part(
        "hump",
        (74, 0),
        [-4.5, -5.0, -5.5, 9.0, 5.0, 11.0],
        PartPose::offset(0.0, -12.0, -10.0),
    );
    body.part(
        "tail",
        (122, 0),
        [-1.5, 0.0, 0.0, 3.0, 14.0, 0.0],
        PartPose::offset(0.0, -9.0, 3.5),
    );
    let head = body.child(
        "head",
        CubeList::new()
            .tex_offs(60, 24)
            .add_box(-3.5, -7.0, -15.0, 7.0, 8.0, 19.0)
            .tex_offs(21, 0)
            .add_box(-3.5, -21.0, -15.0, 7.0, 14.0, 7.0)
            .tex_offs(50, 0)
            .add_box(-2.5, -21.0, -21.0, 5.0, 5.0, 6.0),
        PartPose::offset(0.0, -3.0, -19.5),
    );
    head.part(
        "left_ear",
        (45, 0),
        [-0.5, 0.5, -1.0, 3.0, 1.0, 2.0],
        PartPose::offset(2.5, -21.0, -9.5),
    );
    head.part(
        "right_ear",
        (67, 0),
        [-2.5, 0.5, -1.0, 3.0, 1.0, 2.0],
        PartPose::offset(-2.5, -21.0, -9.5),
    );
    root.part(
        "left_hind_leg",
        (58, 16),
        [-2.5, 2.0, -2.5, 5.0, 21.0, 5.0],
        PartPose::offset(4.9, 1.0, 9.5),
    );
    root.part(
        "right_hind_leg",
        (94, 16),
        [-2.5, 2.0, -2.5, 5.0, 21.0, 5.0],
        PartPose::offset(-4.9, 1.0, 9.5),
    );
    root.part(
        "left_front_leg",
        (0, 0),
        [-2.5, 2.0, -2.5, 5.0, 21.0, 5.0],
        PartPose::offset(4.9, 1.0, -10.5),
    );
    root.part(
        "right_front_leg",
        (0, 26),
        [-2.5, 2.0, -2.5, 5.0, 21.0, 5.0],
        PartPose::offset(-4.9, 1.0, -10.5),
    );
    mesh
}

pub fn layer() -> LayerDef {
    LayerDef::create(adult_mesh(), 128, 128)
}

pub fn saddle_layer() -> LayerDef {
    let mut mesh = adult_mesh();
    let inflate = Grow::all(0.05);
    let body = mesh.root().get("body");
    body.child(
        "saddle",
        CubeList::new()
            .tex_offs(74, 64)
            .add_box_grow(-4.5, -17.0, -15.5, 9.0, 5.0, 11.0, inflate)
            .tex_offs(92, 114)
            .add_box_grow(-3.5, -20.0, -15.5, 7.0, 3.0, 11.0, inflate)
            .tex_offs(0, 89)
            .add_box_grow(-7.5, -12.0, -23.5, 15.0, 12.0, 27.0, inflate),
        PartPose::offset(0.0, 0.0, 0.0),
    );
    let head = body.get("head");
    head.child(
        "reins",
        CubeList::new()
            .tex_offs(98, 42)
            .add_box(3.51, -18.0, -17.0, 0.0, 7.0, 15.0)
            .tex_offs(84, 57)
            .add_box(-3.5, -18.0, -2.0, 7.0, 7.0, 0.0)
            .tex_offs(98, 42)
            .add_box(-3.51, -18.0, -17.0, 0.0, 7.0, 15.0),
        PartPose::offset(0.0, 0.0, 0.0),
    );
    head.child(
        "bridle",
        CubeList::new()
            .tex_offs(60, 87)
            .add_box_grow(-3.5, -7.0, -15.0, 7.0, 8.0, 19.0, inflate)
            .tex_offs(21, 64)
            .add_box_grow(-3.5, -21.0, -15.0, 7.0, 14.0, 7.0, inflate)
            .tex_offs(50, 64)
            .add_box_grow(-2.5, -21.0, -21.0, 5.0, 5.0, 6.0, inflate)
            .tex_offs(74, 70)
            .add_box(2.5, -19.0, -18.0, 1.0, 2.0, 2.0)
            .tex_offs(74, 70)
            .mirror()
            .add_box(-3.5, -19.0, -18.0, 1.0, 2.0, 2.0)
            .mirror_if(false),
        PartPose::offset(0.0, 0.0, 0.0),
    );
    LayerDef::create(mesh, 128, 128)
}

pub fn baby_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let body = root.part(
        "body",
        (0, 14),
        [-4.5, -4.0, -8.0, 9.0, 8.0, 16.0],
        PartPose::offset(0.0, 7.0, 0.0),
    );
    body.part(
        "tail",
        (50, 38),
        [-1.5, -0.5, 0.0, 3.0, 9.0, 0.0],
        PartPose::offset(0.0, -1.5, 8.05),
    );
    let head = body.child(
        "head",
        CubeList::new()
            .tex_offs(20, 0)
            .add_box(-2.5, -3.0, -7.5, 5.0, 5.0, 7.0)
            .tex_offs(0, 0)
            .add_box(-2.5, -12.0, -7.5, 5.0, 9.0, 5.0)
            .tex_offs(0, 14)
            .add_box(-2.5, -12.0, -10.5, 5.0, 4.0, 3.0),
        PartPose::offset(0.0, 1.0, -7.5),
    );
    head.part(
        "right_ear",
        (37, 0),
        [-3.0, -0.5, -1.0, 3.0, 1.0, 2.0],
        PartPose::offset(-2.5, -11.0, -4.0),
    );
    head.part(
        "left_ear",
        (47, 0),
        [0.0, -0.5, -1.0, 3.0, 1.0, 2.0],
        PartPose::offset(2.5, -11.0, -4.0),
    );
    root.part(
        "right_front_leg",
        (36, 14),
        [-1.5, -0.5, -1.5, 3.0, 13.0, 3.0],
        PartPose::offset(-3.0, 11.5, -5.5),
    );
    root.part(
        "left_front_leg",
        (48, 14),
        [-1.5, -0.5, -1.5, 3.0, 13.0, 3.0],
        PartPose::offset(3.0, 11.5, -5.5),
    );
    root.part(
        "left_hind_leg",
        (12, 38),
        [-1.5, -0.5, -1.5, 3.0, 13.0, 3.0],
        PartPose::offset(3.0, 11.5, 5.5),
    );
    root.part(
        "right_hind_leg",
        (0, 38),
        [-1.5, -0.5, -1.5, 3.0, 13.0, 3.0],
        PartPose::offset(-3.0, 11.5, 5.5),
    );
    LayerDef::create(mesh, 64, 64)
}

struct Set {
    walk: &'static AnimationDefinition,
    sit: &'static AnimationDefinition,
    sit_pose: &'static AnimationDefinition,
    standup: &'static AnimationDefinition,
    idle: &'static AnimationDefinition,
    dash: &'static AnimationDefinition,
}

const ADULT: Set = Set {
    walk: &adult_anim::CAMEL_WALK,
    sit: &adult_anim::CAMEL_SIT,
    sit_pose: &adult_anim::CAMEL_SIT_POSE,
    standup: &adult_anim::CAMEL_STANDUP,
    idle: &adult_anim::CAMEL_IDLE,
    dash: &adult_anim::CAMEL_DASH,
};

const BABY: Set = Set {
    walk: &baby_anim::CAMEL_BABY_WALK,
    sit: &baby_anim::CAMEL_BABY_SIT,
    sit_pose: &baby_anim::CAMEL_BABY_SIT_POSE,
    standup: &baby_anim::CAMEL_BABY_STANDUP,
    idle: &baby_anim::CAMEL_BABY_IDLE,
    dash: &baby_anim::CAMEL_BABY_DASH,
};

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(model, parts, st, &ADULT);
}

pub fn baby_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(model, parts, st, &BABY);
}

pub fn saddle_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(model, parts, st, &ADULT);
    parts[model.id("reins")].visible = st.extras.ridden;
}

fn setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState, set: &Set) {
    apply_head_rotation(model, parts, st);
    keyframe::apply_walk(
        set.walk,
        model,
        parts,
        st.walk_pos,
        st.walk_speed,
        MAX_WALK_ANIMATION_SPEED,
        WALK_ANIMATION_SCALE_FACTOR,
    );
    use crate::entities::state::anim_slot::camel as slot;
    let states = &st.extras.anim;
    let age = st.age_ticks;
    keyframe::apply_state(set.sit, model, parts, states[slot::SIT], age);
    keyframe::apply_state(set.sit_pose, model, parts, states[slot::SIT_POSE], age);
    keyframe::apply_state(set.standup, model, parts, states[slot::SIT_UP], age);
    keyframe::apply_state(set.idle, model, parts, states[slot::IDLE], age);
    keyframe::apply_state(set.dash, model, parts, states[slot::DASH], age);
}

fn apply_head_rotation(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let y_rot = st.y_rot.clamp(-30.0, 30.0);
    let mut x_rot = st.x_rot.clamp(-25.0, 45.0);
    if st.extras.jump_cooldown > 0.0 {
        let head_rotation = 45.0 * st.extras.jump_cooldown / 55.0;
        x_rot = (x_rot + head_rotation).clamp(-25.0, 70.0);
    }
    let head = model.id("head");
    parts[head].y_rot = y_rot * DEG_TO_RAD;
    parts[head].x_rot = x_rot * DEG_TO_RAD;
}
