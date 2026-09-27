#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::keyframe::{apply, apply_walk};
use crate::entities::models::aquatic::animation::frog as anim;
use crate::entities::state::{EntityState, Pose};

pub fn frog_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let model_root = mesh
        .root()
        .child("root", CubeList::new(), PartPose::offset(0.0, 24.0, 0.0));
    let body = model_root.child(
        "body",
        CubeList::new()
            .tex_offs(3, 1)
            .add_box(-3.5, -2.0, -8.0, 7.0, 3.0, 9.0)
            .tex_offs(23, 22)
            .add_box(-3.5, -1.0, -8.0, 7.0, 0.0, 9.0),
        PartPose::offset(0.0, -2.0, 4.0),
    );
    let eyes = body
        .child(
            "head",
            CubeList::new()
                .tex_offs(23, 13)
                .add_box(-3.5, -1.0, -7.0, 7.0, 0.0, 9.0)
                .tex_offs(0, 13)
                .add_box(-3.5, -2.0, -7.0, 7.0, 3.0, 9.0),
            PartPose::offset(0.0, -2.0, -1.0),
        )
        .child("eyes", CubeList::new(), PartPose::offset(-0.5, 0.0, 2.0));
    eyes.child(
        "right_eye",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-1.5, -1.0, -1.5, 3.0, 2.0, 3.0),
        PartPose::offset(-1.5, -3.0, -6.5),
    );
    eyes.child(
        "left_eye",
        CubeList::new()
            .tex_offs(0, 5)
            .add_box(-1.5, -1.0, -1.5, 3.0, 2.0, 3.0),
        PartPose::offset(2.5, -3.0, -6.5),
    );
    body.child(
        "croaking_body",
        CubeList::new().tex_offs(26, 5).add_box_grow(
            -3.5,
            -0.1,
            -2.9,
            7.0,
            2.0,
            3.0,
            Grow::all(-0.1),
        ),
        PartPose::offset(0.0, -1.0, -5.0),
    );
    body.child(
        "tongue",
        CubeList::new()
            .tex_offs(17, 13)
            .add_box(-2.0, 0.0, -7.1, 4.0, 0.0, 7.0),
        PartPose::offset(0.0, -1.01, 1.0),
    );
    body.child(
        "left_arm",
        CubeList::new()
            .tex_offs(0, 32)
            .add_box(-1.0, 0.0, -1.0, 2.0, 3.0, 3.0),
        PartPose::offset(4.0, -1.0, -6.5),
    )
    .child(
        "left_hand",
        CubeList::new()
            .tex_offs(18, 40)
            .add_box(-4.0, 0.01, -4.0, 8.0, 0.0, 8.0),
        PartPose::offset(0.0, 3.0, -1.0),
    );
    body.child(
        "right_arm",
        CubeList::new()
            .tex_offs(0, 38)
            .add_box(-1.0, 0.0, -1.0, 2.0, 3.0, 3.0),
        PartPose::offset(-4.0, -1.0, -6.5),
    )
    .child(
        "right_hand",
        CubeList::new()
            .tex_offs(2, 40)
            .add_box(-4.0, 0.01, -5.0, 8.0, 0.0, 8.0),
        PartPose::offset(0.0, 3.0, 0.0),
    );
    model_root
        .child(
            "left_leg",
            CubeList::new()
                .tex_offs(14, 25)
                .add_box(-1.0, 0.0, -2.0, 3.0, 3.0, 4.0),
            PartPose::offset(3.5, -3.0, 4.0),
        )
        .child(
            "left_foot",
            CubeList::new()
                .tex_offs(2, 32)
                .add_box(-4.0, 0.01, -4.0, 8.0, 0.0, 8.0),
            PartPose::offset(2.0, 3.0, 0.0),
        );
    model_root
        .child(
            "right_leg",
            CubeList::new()
                .tex_offs(0, 25)
                .add_box(-2.0, 0.0, -2.0, 3.0, 3.0, 4.0),
            PartPose::offset(-3.5, -3.0, 4.0),
        )
        .child(
            "right_foot",
            CubeList::new()
                .tex_offs(18, 32)
                .add_box(-4.0, 0.01, -4.0, 8.0, 0.0, 8.0),
            PartPose::offset(-2.0, 3.0, 0.0),
        );
    LayerDef::create(mesh, 48, 48)
}

const MAX_WALK_ANIMATION_SPEED: f32 = 1.5;
const MAX_SWIM_ANIMATION_SPEED: f32 = 1.0;
const WALK_ANIMATION_SCALE_FACTOR: f32 = 2.5;

pub fn frog_setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let seconds = st.age_ticks / 20.0;

    if st.extras.jump_progress > 0.0 {
        apply(&anim::FROG_JUMP, model, parts, seconds, 1.0);
    }
    let croaking = st.has_pose(Pose::Croaking);
    if croaking {
        apply(&anim::FROG_CROAK, model, parts, seconds, 1.0);
    }
    if st.has_pose(Pose::UsingTongue) {
        apply(&anim::FROG_TONGUE, model, parts, seconds, 1.0);
    }

    if st.is_in_water {
        apply_walk(
            &anim::FROG_SWIM,
            model,
            parts,
            st.walk_pos,
            st.walk_speed,
            MAX_SWIM_ANIMATION_SPEED,
            WALK_ANIMATION_SCALE_FACTOR,
        );
        apply(&anim::FROG_IDLE_WATER, model, parts, seconds, 1.0);
    } else {
        apply_walk(
            &anim::FROG_WALK,
            model,
            parts,
            st.walk_pos,
            st.walk_speed,
            MAX_WALK_ANIMATION_SPEED,
            WALK_ANIMATION_SCALE_FACTOR,
        );
    }

    parts[model.id("croaking_body")].visible = croaking;
}

pub fn tadpole_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-1.5, -1.0, 0.0, 3.0, 2.0, 3.0),
        PartPose::offset(0.0, 22.0, -3.0),
    );
    root.child(
        "tail",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(0.0, -1.0, 0.0, 0.0, 2.0, 7.0),
        PartPose::offset(0.0, 22.0, 0.0),
    );
    LayerDef::create(mesh, 16, 16)
}

pub fn tadpole_setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let amplitude = if st.is_in_water { 1.0 } else { 1.5 };
    parts[model.id("tail")].y_rot = -amplitude * 0.25 * (0.3 * st.age_ticks).sin();
}
