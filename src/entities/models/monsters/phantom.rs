use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let body = mesh.root().child(
        "body",
        CubeList::new()
            .tex_offs(0, 8)
            .add_box(-3.0, -2.0, -8.0, 5.0, 3.0, 9.0),
        PartPose::rotation(-0.1, 0.0, 0.0),
    );
    let tail_base = body.child(
        "tail_base",
        CubeList::new()
            .tex_offs(3, 20)
            .add_box(-2.0, 0.0, 0.0, 3.0, 2.0, 6.0),
        PartPose::offset(0.0, -2.0, 1.0),
    );
    tail_base.child(
        "tail_tip",
        CubeList::new()
            .tex_offs(4, 29)
            .add_box(-1.0, 0.0, 0.0, 1.0, 1.0, 6.0),
        PartPose::offset(0.0, 0.5, 6.0),
    );
    let left_wing_base = body.child(
        "left_wing_base",
        CubeList::new()
            .tex_offs(23, 12)
            .add_box(0.0, 0.0, 0.0, 6.0, 2.0, 9.0),
        PartPose::offset_rotation(2.0, -2.0, -8.0, 0.0, 0.0, 0.1),
    );
    left_wing_base.child(
        "left_wing_tip",
        CubeList::new()
            .tex_offs(16, 24)
            .add_box(0.0, 0.0, 0.0, 13.0, 1.0, 9.0),
        PartPose::offset_rotation(6.0, 0.0, 0.0, 0.0, 0.0, 0.1),
    );
    let right_wing_base = body.child(
        "right_wing_base",
        CubeList::new()
            .tex_offs(23, 12)
            .mirror()
            .add_box(-6.0, 0.0, 0.0, 6.0, 2.0, 9.0),
        PartPose::offset_rotation(-3.0, -2.0, -8.0, 0.0, 0.0, -0.1),
    );
    right_wing_base.child(
        "right_wing_tip",
        CubeList::new()
            .tex_offs(16, 24)
            .mirror()
            .add_box(-13.0, 0.0, 0.0, 13.0, 1.0, 9.0),
        PartPose::offset_rotation(-6.0, 0.0, 0.0, 0.0, 0.0, -0.1),
    );
    body.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -2.0, -5.0, 7.0, 3.0, 5.0),
        PartPose::offset_rotation(0.0, 1.0, -7.0, 0.2, 0.0, 0.0),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    const FLAP_AMOUNT: f32 = 16.0;
    let anim = st.extras.flap_time * 7.448451 * DEG_TO_RAD;

    let z_rot = anim.cos() * FLAP_AMOUNT * DEG_TO_RAD;
    parts[model.id("left_wing_base")].z_rot = z_rot;
    parts[model.id("left_wing_tip")].z_rot = z_rot;
    parts[model.id("right_wing_base")].z_rot = -z_rot;
    parts[model.id("right_wing_tip")].z_rot = -z_rot;

    let tail = -(5.0 + (anim * 2.0).cos() * 5.0) * DEG_TO_RAD;
    parts[model.id("tail_base")].x_rot = tail;
    parts[model.id("tail_tip")].x_rot = tail;
}
