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
    body.child(
        "tail",
        CubeList::new()
            .tex_offs(44, 53)
            .add_box(-0.5, -0.0865, 0.0933, 1.0, 6.0, 1.0),
        PartPose::offset_rotation(0.0, -3.0, 1.0, 0.5061, 0.0, 0.0),
    );
    let head = body.child("head", CubeList::new(), PartPose::offset(0.0, -2.0, -11.0));
    head.child(
        "head_cube",
        CubeList::new()
            .tex_offs(43, 15)
            .add_box(-1.5, -1.0, -1.0, 3.0, 5.0, 2.0),
        PartPose::offset_rotation(0.0, 0.0, 0.0, -0.3927, 0.0, 0.0),
    );
    let right_ear = head.child(
        "right_ear",
        CubeList::new(),
        PartPose::offset(-1.0, -1.0, 0.0),
    );
    right_ear.child(
        "right_ear_cube",
        CubeList::new()
            .tex_offs(43, 10)
            .add_box(-2.0, -3.0, 0.0, 2.0, 5.0, 0.0),
        PartPose::offset_rotation(-0.5, 0.0, -0.6, 0.1886, -0.3864, -0.0718),
    );
    let left_ear = head.child(
        "left_ear",
        CubeList::new(),
        PartPose::offset(1.0, -2.0, 0.0),
    );
    left_ear.child(
        "left_ear_cube",
        CubeList::new()
            .tex_offs(47, 10)
            .add_box(0.0, -3.0, 0.0, 2.0, 5.0, 0.0),
        PartPose::offset_rotation(0.5, 1.0, -0.6, 0.1886, 0.3864, 0.0718),
    );
    let root = mesh.root();
    root.child(
        "right_hind_leg",
        CubeList::new()
            .tex_offs(51, 31)
            .add_box(-1.0, 0.0, -1.0, 2.0, 3.0, 2.0),
        PartPose::offset(-2.0, 21.0, 4.0),
    );
    root.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(42, 31)
            .add_box(-1.0, 0.0, -1.0, 2.0, 3.0, 2.0),
        PartPose::offset(2.0, 21.0, 4.0),
    );
    root.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(51, 43)
            .add_box(-1.0, 0.0, -1.0, 2.0, 3.0, 2.0),
        PartPose::offset(-2.0, 21.0, -4.0),
    );
    root.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(42, 43)
            .add_box(-1.0, 0.0, -1.0, 2.0, 3.0, 2.0),
        PartPose::offset(2.0, 21.0, -4.0),
    );
    root.child(
        "cube",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-5.0, -10.0, -6.0, 10.0, 10.0, 10.0),
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
    tail.child(
        "right_ear_cube",
        CubeList::new()
            .tex_offs(22, 11)
            .add_box(-0.5, -0.5, -2.0, 1.0, 1.0, 4.0),
        PartPose::offset_rotation(0.0, 1.5, 1.0, -1.0472, 0.0, 0.0),
    );
    let head = body.child("head", CubeList::new(), PartPose::offset(0.0, 0.0, -3.2));
    let head_group = head.child(
        "head_cube",
        CubeList::new()
            .tex_offs(20, 17)
            .add_box(-1.0, -2.0, -4.0, 2.0, 2.0, 4.0),
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
    head_group.child(
        "left_ear",
        CubeList::new()
            .tex_offs(28, 8)
            .add_box(-0.2, -2.0, 0.0, 2.0, 3.0, 0.0),
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
    root.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(20, 27)
            .add_box(-1.0, 0.0, -1.0, 2.0, 2.0, 2.0),
        PartPose::offset(1.5, 22.0, 2.5),
    );
    root.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(20, 23)
            .add_box(-1.0, 0.0, -1.0, 2.0, 2.0, 2.0),
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
    root.child(
        "cube",
        CubeList::new().tex_offs(0, 25).add_box_grow(
            -3.0,
            -3.0,
            -3.0,
            6.0,
            6.0,
            6.0,
            Grow::all(0.3),
        ),
        PartPose::offset(0.0, 20.7, 0.5),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(
        model,
        parts,
        st,
        &adult_anim::ARMADILLO_WALK,
        &adult_anim::ARMADILLO_ROLL_UP,
    );
}

pub fn baby_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(
        model,
        parts,
        st,
        &baby_anim::ARMADILLO_BABY_WALK,
        &baby_anim::ARMADILLO_BABY_ROLL_UP,
    );
}

fn setup(
    model: &BakedModel,
    parts: &mut [PartState],
    st: &EntityState,
    walk: &AnimationDefinition,
    roll_up: &AnimationDefinition,
) {
    let body = model.id("body");
    let head = model.id("head");
    let tail = model.id("tail");
    let cube = model.id("cube");
    let right_hind = model.id("right_hind_leg");
    let left_hind = model.id("left_hind_leg");

    let hiding = st.extras.armadillo_state != 0;

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
            walk,
            model,
            parts,
            st.walk_pos,
            st.walk_speed,
            MAX_WALK_ANIMATION_SPEED,
            WALK_ANIMATION_SCALE_FACTOR,
        );
    } else {
        keyframe::apply(roll_up, model, parts, roll_up.length_seconds, 1.0);
    }
}
