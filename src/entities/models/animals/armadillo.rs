use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::keyframe::{self, AnimationDefinition};
use crate::entities::state::EntityState;

use super::animations::{armadillo as adult_anim, armadillo_baby as baby_anim};
use crate::util::mth::DEG_TO_RAD;

const MAX_WALK_ANIMATION_SPEED: f32 = 16.5;
const WALK_ANIMATION_SCALE_FACTOR: f32 = 2.5;
const MAX_DOWN_HEAD_ROTATION_EXTENT: f32 = 25.0;
const MAX_UP_HEAD_ROTATION_EXTENT: f32 = 22.5;

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let body = root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 20)
            .add_box_grow(-4.0, -7.0, -10.0, 8.0, 8.0, 12.0, Grow::all(0.3))
            .tex_offs(0, 40)
            .add_box(-4.0, -7.0, -10.0, 8.0, 8.0, 12.0),
        PartPose::offset(0.0, 21.0, 4.0),
    );
    body.part(
        "tail",
        (44, 53),
        [-0.5, -0.0865, 0.0933, 1.0, 6.0, 1.0],
        PartPose::offset_rotation(0.0, -3.0, 1.0, 0.5061, 0.0, 0.0),
    );
    let head = body.child("head", CubeList::new(), PartPose::offset(0.0, -2.0, -11.0));
    head.part(
        "head_cube",
        (43, 15),
        [-1.5, -1.0, -1.0, 3.0, 5.0, 2.0],
        PartPose::offset_rotation(0.0, 0.0, 0.0, -0.3927, 0.0, 0.0),
    );
    let right_ear = head.child(
        "right_ear",
        CubeList::new(),
        PartPose::offset(-1.0, -1.0, 0.0),
    );
    right_ear.part(
        "right_ear_cube",
        (43, 10),
        [-2.0, -3.0, 0.0, 2.0, 5.0, 0.0],
        PartPose::offset_rotation(-0.5, 0.0, -0.6, 0.1886, -0.3864, -0.0718),
    );
    let left_ear = head.child(
        "left_ear",
        CubeList::new(),
        PartPose::offset(1.0, -2.0, 0.0),
    );
    left_ear.part(
        "left_ear_cube",
        (47, 10),
        [0.0, -3.0, 0.0, 2.0, 5.0, 0.0],
        PartPose::offset_rotation(0.5, 1.0, -0.6, 0.1886, 0.3864, 0.0718),
    );
    let root = mesh.root();
    root.part(
        "right_hind_leg",
        (51, 31),
        [-1.0, 0.0, -1.0, 2.0, 3.0, 2.0],
        PartPose::offset(-2.0, 21.0, 4.0),
    );
    root.part(
        "left_hind_leg",
        (42, 31),
        [-1.0, 0.0, -1.0, 2.0, 3.0, 2.0],
        PartPose::offset(2.0, 21.0, 4.0),
    );
    root.part(
        "right_front_leg",
        (51, 43),
        [-1.0, 0.0, -1.0, 2.0, 3.0, 2.0],
        PartPose::offset(-2.0, 21.0, -4.0),
    );
    root.part(
        "left_front_leg",
        (42, 43),
        [-1.0, 0.0, -1.0, 2.0, 3.0, 2.0],
        PartPose::offset(2.0, 21.0, -4.0),
    );
    root.part(
        "cube",
        (0, 0),
        [-5.0, -10.0, -6.0, 10.0, 10.0, 10.0],
        PartPose::offset(0.0, 24.0, 0.0),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn baby_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let body = root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box_grow(-2.5, -2.0, -3.5, 5.0, 4.0, 7.0, Grow::all(0.3))
            .tex_offs(0, 11)
            .add_box(-2.5, -2.0, -3.0, 5.0, 4.0, 6.0),
        PartPose::offset(0.0, 20.0, 0.5),
    );
    let tail = body.child("tail", CubeList::new(), PartPose::offset(0.0, 0.0, 3.4));
    tail.part(
        "right_ear_cube",
        (22, 11),
        [-0.5, -0.5, -2.0, 1.0, 1.0, 4.0],
        PartPose::offset_rotation(0.0, 1.5, 1.0, -1.0472, 0.0, 0.0),
    );
    let head = body.child("head", CubeList::new(), PartPose::offset(0.0, 0.0, -3.2));
    let head_group = head.part(
        "head_cube",
        (20, 17),
        [-1.0, -2.0, -4.0, 2.0, 2.0, 4.0],
        PartPose::offset_rotation(0.0, 0.0, 0.0, 0.7417649, 0.0, 0.0),
    );
    head_group.child(
        "right_ear",
        CubeList::new()
            .tex_offs(28, 8)
            .mirror()
            .add_box(-1.8, -2.0, 0.0, 2.0, 3.0, 0.0)
            .mirror_if(false),
        PartPose::offset_rotation(-1.0, -2.0, -0.3, -0.4363, -0.1134, 0.0524),
    );
    head_group.part(
        "left_ear",
        (28, 8),
        [-0.2, -2.0, 0.0, 2.0, 3.0, 0.0],
        PartPose::offset_rotation(1.0, -2.0, -0.3, -0.4363, 0.1134, -0.0524),
    );
    let root = mesh.root();
    root.child(
        "right_hind_leg",
        CubeList::new()
            .tex_offs(20, 27)
            .mirror()
            .add_box(-1.0, 0.0, -1.0, 2.0, 2.0, 2.0)
            .mirror_if(false),
        PartPose::offset(-1.5, 22.0, 2.5),
    );
    root.part(
        "left_hind_leg",
        (20, 27),
        [-1.0, 0.0, -1.0, 2.0, 2.0, 2.0],
        PartPose::offset(1.5, 22.0, 2.5),
    );
    root.part(
        "right_front_leg",
        (20, 23),
        [-1.0, 0.0, -1.0, 2.0, 2.0, 2.0],
        PartPose::offset(1.5, 22.0, -1.5),
    );
    root.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(24, 0)
            .mirror()
            .add_box(-1.0, 0.0, -1.0, 2.0, 2.0, 2.0)
            .mirror_if(false),
        PartPose::offset(-1.5, 22.0, -1.5),
    );
    root.part_grow(
        "cube",
        (0, 25),
        [-3.0, -3.0, -3.0, 6.0, 6.0, 6.0],
        Grow::all(0.3),
        PartPose::offset(0.0, 20.7, 0.5),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(
        model,
        parts,
        st,
        &Set {
            walk: &adult_anim::ARMADILLO_WALK,
            roll_out: &adult_anim::ARMADILLO_ROLL_OUT,
            roll_up: &adult_anim::ARMADILLO_ROLL_UP,
            peek: &adult_anim::ARMADILLO_PEEK,
        },
    );
}

pub fn baby_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(
        model,
        parts,
        st,
        &Set {
            walk: &baby_anim::ARMADILLO_BABY_WALK,
            roll_out: &baby_anim::ARMADILLO_BABY_ROLL_OUT,
            roll_up: &baby_anim::ARMADILLO_BABY_ROLL_UP,
            peek: &baby_anim::ARMADILLO_BABY_PEEK,
        },
    );
}

struct Set {
    walk: &'static AnimationDefinition,
    roll_out: &'static AnimationDefinition,
    roll_up: &'static AnimationDefinition,
    peek: &'static AnimationDefinition,
}

fn setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState, set: &Set) {
    let body = model.id("body");
    let head = model.id("head");
    let tail = model.id("tail");
    let cube = model.id("cube");
    let right_hind = model.id("right_hind_leg");
    let left_hind = model.id("left_hind_leg");

    let hiding = st.extras.hiding_in_shell;

    if hiding {
        parts[body].skip_draw = true;
        parts[left_hind].visible = false;
        parts[right_hind].visible = false;
        parts[tail].visible = false;
        parts[cube].visible = true;
    } else {
        parts[body].skip_draw = false;
        parts[left_hind].visible = true;
        parts[right_hind].visible = true;
        parts[tail].visible = true;
        parts[cube].visible = false;
        parts[head].x_rot = st
            .x_rot
            .clamp(-MAX_UP_HEAD_ROTATION_EXTENT, MAX_DOWN_HEAD_ROTATION_EXTENT)
            * DEG_TO_RAD;
        parts[head].y_rot = st.y_rot.clamp(-32.5, 32.5) * DEG_TO_RAD;
    }

    if !hiding {
        keyframe::apply_walk(
            set.walk,
            model,
            parts,
            st.walk_pos,
            st.walk_speed,
            MAX_WALK_ANIMATION_SPEED,
            WALK_ANIMATION_SCALE_FACTOR,
        );
    }

    use crate::entities::state::anim_slot::armadillo as slot;
    let states = &st.extras.anim;
    keyframe::apply_state(
        set.roll_out,
        model,
        parts,
        states[slot::ROLL_OUT],
        st.age_ticks,
    );
    keyframe::apply_state(
        set.roll_up,
        model,
        parts,
        states[slot::ROLL_UP],
        st.age_ticks,
    );
    keyframe::apply_state(set.peek, model, parts, states[slot::PEEK], st.age_ticks);
}
