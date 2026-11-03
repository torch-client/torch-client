use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::keyframe::{self, AnimationDefinition};
use crate::entities::state::EntityState;

use super::animations::{rabbit as adult_anim, rabbit_baby as baby_anim};
use crate::util::mth::DEG_TO_RAD;

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let body = root.part(
        "body",
        (0, 0),
        [-4.0, -6.0, -9.0, 8.0, 6.0, 10.0],
        PartPose::offset_rotation(0.0, 23.0, 4.0, -0.3927, 0.0, 0.0),
    );
    body.part(
        "tail",
        (20, 16),
        [-2.0, -3.0084, -1.0125, 4.0, 4.0, 4.0],
        PartPose::offset(0.0, -4.9916, 0.0125),
    );
    let head = body.part(
        "head",
        (0, 16),
        [-2.5, -3.0, -4.0, 5.0, 5.0, 5.0],
        PartPose::offset_rotation(0.0, -5.2929, -8.1213, 0.3927, 0.0, 0.0),
    );
    head.part(
        "left_ear",
        (32, 0),
        [-1.0, -4.2929, -0.1213, 2.0, 5.0, 1.0],
        PartPose::offset(1.5, -3.7071, -0.8787),
    );
    head.part(
        "right_ear",
        (26, 0),
        [-1.0, -4.2929, -0.1213, 2.0, 5.0, 1.0],
        PartPose::offset(-1.5, -3.7071, -0.8787),
    );
    let front_legs = body.child(
        "frontlegs",
        CubeList::new(),
        PartPose::offset(0.0, -1.5349, -6.3108),
    );
    front_legs.part(
        "right_front_leg",
        (36, 18),
        [-0.9, -1.0, -0.9, 2.0, 4.0, 2.0],
        PartPose::offset_rotation(-2.0, 1.9239, 0.3827, 0.3927, 0.0, 0.0),
    );
    front_legs.part(
        "left_front_leg",
        (44, 18),
        [-1.0, -1.0, -1.0, 2.0, 4.0, 2.0],
        PartPose::offset_rotation(2.0, 1.9239, 0.4827, 0.3927, 0.0, 0.0),
    );
    let back_legs = root.child(
        "backlegs",
        CubeList::new(),
        PartPose::offset(0.0, 23.0, 4.0),
    );
    let right_back_leg = back_legs.child(
        "right_hind_leg",
        CubeList::new(),
        PartPose::offset(-3.0, 0.5, 0.0),
    );
    right_back_leg.part(
        "right_haunch",
        (20, 24),
        [-1.0, 0.0, -5.0, 2.0, 1.0, 6.0],
        PartPose::offset_rotation(0.0, -0.5, 0.0, 0.0, 0.3927, 0.0),
    );
    let left_back_leg = back_legs.child(
        "left_hind_leg",
        CubeList::new(),
        PartPose::offset(3.0, 0.5, 0.0),
    );
    left_back_leg.part(
        "left_haunch",
        (36, 24),
        [-1.0, 0.0, -5.0, 2.0, 1.0, 6.0],
        PartPose::offset_rotation(0.0, -0.5, 0.0, 0.0, -0.3927, 0.0),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn baby_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let body = root.child("body", CubeList::new(), PartPose::offset(0.0, 23.0, 1.6));
    body.part(
        "body_r1",
        (0, 8),
        [-2.0, -2.0, -3.0, 4.0, 3.0, 6.0],
        PartPose::offset_rotation(0.0, -2.0, -1.6, -0.5236, 0.0, 0.0),
    );
    let tail = body.child("tail", CubeList::new(), PartPose::offset(0.0, -2.2, 2.0));
    tail.part(
        "tail_r1",
        (0, 21),
        [-1.4, -2.0268, -1.0177, 3.0, 3.0, 3.0],
        PartPose::offset_rotation(-0.1, 0.0, 0.0, -0.5236, 0.0, 0.0),
    );
    let head = body.part(
        "head",
        (0, 0),
        [-2.5, -3.0, -3.0, 5.0, 4.0, 4.0],
        PartPose::offset(0.0, -5.0, -2.6),
    );
    head.part(
        "right_ear",
        (18, 0),
        [-1.0, -3.5, -0.5, 2.0, 4.0, 1.0],
        PartPose::offset(-1.5, -3.5, -0.5),
    );
    head.part(
        "left_ear",
        (24, 0),
        [-1.0, -3.5, -0.5, 2.0, 4.0, 1.0],
        PartPose::offset(1.5, -3.5, -0.5),
    );
    let front_legs = body.child(
        "frontlegs",
        CubeList::new(),
        PartPose::offset(0.0, -2.5, -2.6),
    );
    let left_front_leg = front_legs.child(
        "left_front_leg",
        CubeList::new(),
        PartPose::offset_rotation(1.0, 1.0, -0.5, 0.3927, 0.0, 0.0),
    );
    left_front_leg.part(
        "left_front_leg_r1",
        (18, 8),
        [-0.5, -1.5, -0.5, 1.0, 3.0, 1.0],
        PartPose::offset_rotation(0.0, 1.0, 0.0, -0.3927, 0.0, 0.0),
    );
    let right_front_leg = front_legs.child(
        "right_front_leg",
        CubeList::new(),
        PartPose::offset_rotation(-1.0, 1.0, -0.5, 0.3927, 0.0, 0.0),
    );
    right_front_leg.part(
        "right_front_leg_r1",
        (14, 8),
        [-0.5, -1.5, -0.5, 1.0, 3.0, 1.0],
        PartPose::offset_rotation(0.0, 1.0, 0.0, -0.3927, 0.0, 0.0),
    );
    let back_legs = root.child(
        "backlegs",
        CubeList::new(),
        PartPose::offset(0.0, 23.0, 2.0),
    );
    let left_back_leg = back_legs.child(
        "left_hind_leg",
        CubeList::new(),
        PartPose::offset_rotation(1.5, 0.5, 0.5, 0.0, 3.1416, 0.0),
    );
    left_back_leg.part(
        "left_haunch",
        (10, 17),
        [-2.0, -0.5, 0.0, 2.0, 1.0, 3.0],
        PartPose::offset_rotation(1.0, 0.0, 0.5, 0.0, -0.7854, 0.0),
    );
    let right_back_leg = back_legs.child(
        "right_hind_leg",
        CubeList::new(),
        PartPose::offset_rotation(-1.5, 0.5, 0.5, 0.0, 3.1416, 0.0),
    );
    right_back_leg.part(
        "right_haunch",
        (0, 17),
        [-2.0, -0.5, 0.0, 2.0, 1.0, 3.0],
        PartPose::offset_rotation(0.5, 0.0, -0.9, 0.0, 0.7854, 0.0),
    );
    LayerDef::create(mesh, 32, 32)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(model, parts, st, &adult_anim::HOP);
}

pub fn baby_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(model, parts, st, &baby_anim::HOP);
}

fn setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState, hop: &AnimationDefinition) {
    let head = model.id("head");
    parts[head].y_rot = st.y_rot * DEG_TO_RAD;
    parts[head].x_rot = st.x_rot * DEG_TO_RAD;
    keyframe::apply(
        hop,
        model,
        parts,
        st.extras.jump_completion * hop.length_seconds,
        1.0,
    );
}
