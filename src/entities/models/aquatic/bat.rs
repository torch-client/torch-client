use crate::entities::geom::{BakedModel, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::keyframe::apply;
use crate::entities::models::aquatic::DEG_TO_RAD;
use crate::entities::models::aquatic::animation::bat as anim;
use crate::entities::state::EntityState;

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let body = root.part(
        "body",
        (0, 0),
        [-1.5, 0.0, -1.0, 3.0, 5.0, 2.0],
        PartPose::offset(0.0, 17.0, 0.0),
    );
    body.part(
        "right_wing",
        (12, 0),
        [-2.0, -2.0, 0.0, 2.0, 7.0, 0.0],
        PartPose::offset(-1.5, 0.0, 0.0),
    )
    .part(
        "right_wing_tip",
        (16, 0),
        [-6.0, -2.0, 0.0, 6.0, 8.0, 0.0],
        PartPose::offset(-2.0, 0.0, 0.0),
    );
    body.part(
        "left_wing",
        (12, 7),
        [0.0, -2.0, 0.0, 2.0, 7.0, 0.0],
        PartPose::offset(1.5, 0.0, 0.0),
    )
    .part(
        "left_wing_tip",
        (16, 8),
        [0.0, -2.0, 0.0, 6.0, 8.0, 0.0],
        PartPose::offset(2.0, 0.0, 0.0),
    );
    body.part(
        "feet",
        (16, 16),
        [-1.5, 0.0, 0.0, 3.0, 2.0, 0.0],
        PartPose::offset(0.0, 5.0, 0.0),
    );
    let head = root.part(
        "head",
        (0, 7),
        [-2.0, -3.0, -1.0, 4.0, 3.0, 2.0],
        PartPose::offset(0.0, 17.0, 0.0),
    );
    head.part(
        "right_ear",
        (1, 15),
        [-2.5, -4.0, 0.0, 3.0, 5.0, 0.0],
        PartPose::offset(-1.5, -2.0, 0.0),
    );
    head.part(
        "left_ear",
        (8, 15),
        [-0.1, -3.0, 0.0, 3.0, 5.0, 0.0],
        PartPose::offset(1.1, -3.0, 0.0),
    );
    LayerDef::create(mesh, 32, 32)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let seconds = st.age_ticks / 20.0;
    if st.extras.resting {
        parts[model.id("head")].y_rot = st.y_rot * DEG_TO_RAD;
        apply(&anim::BAT_RESTING, model, parts, seconds, 1.0);
    } else {
        apply(&anim::BAT_FLYING, model, parts, seconds, 1.0);
    }
}
