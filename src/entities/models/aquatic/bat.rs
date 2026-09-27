use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::keyframe::apply;
use crate::entities::models::aquatic::DEG_TO_RAD;
use crate::entities::models::aquatic::animation::bat as anim;
use crate::entities::state::EntityState;

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let body = root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-1.5, 0.0, -1.0, 3.0, 5.0, 2.0),
        PartPose::offset(0.0, 17.0, 0.0),
    );
    body.child(
        "right_wing",
        CubeList::new()
            .tex_offs(12, 0)
            .add_box(-2.0, -2.0, 0.0, 2.0, 7.0, 0.0),
        PartPose::offset(-1.5, 0.0, 0.0),
    )
    .child(
        "right_wing_tip",
        CubeList::new()
            .tex_offs(16, 0)
            .add_box(-6.0, -2.0, 0.0, 6.0, 8.0, 0.0),
        PartPose::offset(-2.0, 0.0, 0.0),
    );
    body.child(
        "left_wing",
        CubeList::new()
            .tex_offs(12, 7)
            .add_box(0.0, -2.0, 0.0, 2.0, 7.0, 0.0),
        PartPose::offset(1.5, 0.0, 0.0),
    )
    .child(
        "left_wing_tip",
        CubeList::new()
            .tex_offs(16, 8)
            .add_box(0.0, -2.0, 0.0, 6.0, 8.0, 0.0),
        PartPose::offset(2.0, 0.0, 0.0),
    );
    body.child(
        "feet",
        CubeList::new()
            .tex_offs(16, 16)
            .add_box(-1.5, 0.0, 0.0, 3.0, 2.0, 0.0),
        PartPose::offset(0.0, 5.0, 0.0),
    );
    let head = root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 7)
            .add_box(-2.0, -3.0, -1.0, 4.0, 3.0, 2.0),
        PartPose::offset(0.0, 17.0, 0.0),
    );
    head.child(
        "right_ear",
        CubeList::new()
            .tex_offs(1, 15)
            .add_box(-2.5, -4.0, 0.0, 3.0, 5.0, 0.0),
        PartPose::offset(-1.5, -2.0, 0.0),
    );
    head.child(
        "left_ear",
        CubeList::new()
            .tex_offs(8, 15)
            .add_box(-0.1, -3.0, 0.0, 3.0, 5.0, 0.0),
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
