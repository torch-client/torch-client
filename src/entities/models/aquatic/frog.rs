#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::keyframe::{apply_state, apply_walk};
use crate::entities::models::aquatic::animation::frog as anim;
use crate::entities::state::EntityState;

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
    eyes.part(
        "right_eye",
        (0, 0),
        [-1.5, -1.0, -1.5, 3.0, 2.0, 3.0],
        PartPose::offset(-1.5, -3.0, -6.5),
    );
    eyes.part(
        "left_eye",
        (0, 5),
        [-1.5, -1.0, -1.5, 3.0, 2.0, 3.0],
        PartPose::offset(2.5, -3.0, -6.5),
    );
    body.part_grow(
        "croaking_body",
        (26, 5),
        [-3.5, -0.1, -2.9, 7.0, 2.0, 3.0],
        Grow::all(-0.1),
        PartPose::offset(0.0, -1.0, -5.0),
    );
    body.part(
        "tongue",
        (17, 13),
        [-2.0, 0.0, -7.1, 4.0, 0.0, 7.0],
        PartPose::offset(0.0, -1.01, 1.0),
    );
    body.part(
        "left_arm",
        (0, 32),
        [-1.0, 0.0, -1.0, 2.0, 3.0, 3.0],
        PartPose::offset(4.0, -1.0, -6.5),
    )
    .part(
        "left_hand",
        (18, 40),
        [-4.0, 0.01, -4.0, 8.0, 0.0, 8.0],
        PartPose::offset(0.0, 3.0, -1.0),
    );
    body.part(
        "right_arm",
        (0, 38),
        [-1.0, 0.0, -1.0, 2.0, 3.0, 3.0],
        PartPose::offset(-4.0, -1.0, -6.5),
    )
    .part(
        "right_hand",
        (2, 40),
        [-4.0, 0.01, -5.0, 8.0, 0.0, 8.0],
        PartPose::offset(0.0, 3.0, 0.0),
    );
    model_root
        .part(
            "left_leg",
            (14, 25),
            [-1.0, 0.0, -2.0, 3.0, 3.0, 4.0],
            PartPose::offset(3.5, -3.0, 4.0),
        )
        .part(
            "left_foot",
            (2, 32),
            [-4.0, 0.01, -4.0, 8.0, 0.0, 8.0],
            PartPose::offset(2.0, 3.0, 0.0),
        );
    model_root
        .part(
            "right_leg",
            (0, 25),
            [-2.0, 0.0, -2.0, 3.0, 3.0, 4.0],
            PartPose::offset(-3.5, -3.0, 4.0),
        )
        .part(
            "right_foot",
            (18, 32),
            [-4.0, 0.01, -4.0, 8.0, 0.0, 8.0],
            PartPose::offset(-2.0, 3.0, 0.0),
        );
    LayerDef::create(mesh, 48, 48)
}

const MAX_WALK_ANIMATION_SPEED: f32 = 1.5;
const MAX_SWIM_ANIMATION_SPEED: f32 = 1.0;
const WALK_ANIMATION_SCALE_FACTOR: f32 = 2.5;

pub fn frog_setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    use crate::entities::state::anim_slot::frog as slot;
    let states = &st.extras.anim;
    let age = st.age_ticks;
    apply_state(&anim::FROG_JUMP, model, parts, states[slot::JUMP], age);
    apply_state(&anim::FROG_CROAK, model, parts, states[slot::CROAK], age);
    apply_state(&anim::FROG_TONGUE, model, parts, states[slot::TONGUE], age);

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
    apply_state(
        &anim::FROG_IDLE_WATER,
        model,
        parts,
        states[slot::SWIM_IDLE],
        age,
    );

    parts[model.id("croaking_body")].visible = states[slot::CROAK].is_started();
}

pub fn tadpole_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.part(
        "body",
        (0, 0),
        [-1.5, -1.0, 0.0, 3.0, 2.0, 3.0],
        PartPose::offset(0.0, 22.0, -3.0),
    );
    root.part(
        "tail",
        (0, 0),
        [0.0, -1.0, 0.0, 0.0, 2.0, 7.0],
        PartPose::offset(0.0, 22.0, 0.0),
    );
    LayerDef::create(mesh, 16, 16)
}

pub fn tadpole_setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let amplitude = if st.is_in_water { 1.0 } else { 1.5 };
    parts[model.id("tail")].y_rot = -amplitude * 0.25 * (0.3 * st.age_ticks).sin();
}
