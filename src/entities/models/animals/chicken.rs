use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

fn base_mesh() -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let head = root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-2.0, -6.0, -2.0, 4.0, 6.0, 3.0),
        PartPose::offset(0.0, 15.0, -4.0),
    );
    head.child(
        "beak",
        CubeList::new()
            .tex_offs(14, 0)
            .add_box(-2.0, -4.0, -4.0, 4.0, 2.0, 2.0),
        PartPose::ZERO,
    );
    head.child(
        "red_thing",
        CubeList::new()
            .tex_offs(14, 4)
            .add_box(-1.0, -2.0, -3.0, 2.0, 2.0, 2.0),
        PartPose::ZERO,
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 9)
            .add_box(-3.0, -4.0, -3.0, 6.0, 8.0, 6.0),
        PartPose::offset_rotation(0.0, 16.0, 0.0, std::f32::consts::FRAC_PI_2, 0.0, 0.0),
    );
    let leg = CubeList::new()
        .tex_offs(26, 0)
        .add_box(-1.0, 0.0, -3.0, 3.0, 5.0, 3.0);
    root.child("right_leg", leg.clone(), PartPose::offset(-2.0, 19.0, 1.0));
    root.child("left_leg", leg, PartPose::offset(1.0, 19.0, 1.0));
    root.child(
        "right_wing",
        CubeList::new()
            .tex_offs(24, 13)
            .add_box(0.0, 0.0, -3.0, 1.0, 4.0, 6.0),
        PartPose::offset(-4.0, 13.0, 0.0),
    );
    root.child(
        "left_wing",
        CubeList::new()
            .tex_offs(24, 13)
            .add_box(-1.0, 0.0, -3.0, 1.0, 4.0, 6.0),
        PartPose::offset(4.0, 13.0, 0.0),
    );
    mesh
}

pub fn layer() -> LayerDef {
    LayerDef::create(base_mesh(), 64, 32)
}

pub fn cold_layer() -> LayerDef {
    let mut mesh = base_mesh();
    mesh.root().child(
        "body",
        CubeList::new()
            .tex_offs(0, 9)
            .add_box(-3.0, -4.0, -3.0, 6.0, 8.0, 6.0)
            .tex_offs(38, 9)
            .add_box(0.0, 3.0, -1.0, 0.0, 3.0, 5.0),
        PartPose::offset_rotation(0.0, 16.0, 0.0, std::f32::consts::FRAC_PI_2, 0.0, 0.0),
    );
    mesh.root().child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-2.0, -6.0, -2.0, 4.0, 6.0, 3.0)
            .tex_offs(44, 0)
            .add_box(-3.0, -7.0, -2.015, 6.0, 3.0, 4.0),
        PartPose::offset(0.0, 15.0, -4.0),
    );
    LayerDef::create(mesh, 64, 32)
}

pub fn baby_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-2.0, -2.25, -0.75, 4.0, 4.0, 4.0)
            .tex_offs(10, 8)
            .add_box(-1.0, -0.25, -1.75, 2.0, 1.0, 1.0),
        PartPose::offset(0.0, 20.25, -1.25),
    );
    root.child(
        "left_leg",
        CubeList::new()
            .tex_offs(2, 2)
            .add_box(-0.5, 0.0, 0.0, 1.0, 2.0, 0.0)
            .tex_offs(0, 1)
            .add_box(-0.5, 2.0, -1.0, 1.0, 0.0, 1.0),
        PartPose::offset(1.0, 22.0, 0.5),
    );
    root.child(
        "right_leg",
        CubeList::new()
            .tex_offs(0, 2)
            .add_box(-0.5, 0.0, 0.0, 1.0, 2.0, 0.0)
            .tex_offs(0, 0)
            .add_box(-0.5, 2.0, -1.0, 1.0, 0.0, 1.0),
        PartPose::offset(-1.0, 22.0, 0.5),
    );
    root.child(
        "right_wing",
        CubeList::new()
            .tex_offs(6, 8)
            .add_box(0.0, 0.0, -1.0, 1.0, 0.0, 2.0),
        PartPose::offset(2.0, 20.0, 0.0),
    );
    root.child(
        "left_wing",
        CubeList::new()
            .tex_offs(4, 8)
            .add_box(-1.0, 0.0, -1.0, 1.0, 0.0, 2.0),
        PartPose::offset(-2.0, 20.0, 0.0),
    );
    LayerDef::create(mesh, 16, 16)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let flap_angle = (st.extras.flap.sin() + 1.0) * st.extras.flap_speed;
    let pos = st.walk_pos;
    let speed = st.walk_speed;
    let right_leg = model.id("right_leg");
    let left_leg = model.id("left_leg");
    parts[right_leg].x_rot = (pos * 0.6662).cos() * 1.4 * speed;
    parts[left_leg].x_rot = (pos * 0.6662 + std::f32::consts::PI).cos() * 1.4 * speed;
    let right_wing = model.id("right_wing");
    let left_wing = model.id("left_wing");
    parts[right_wing].z_rot = flap_angle;
    parts[left_wing].z_rot = -flap_angle;

    if let Some(head) = model.find("head") {
        parts[head].x_rot = st.x_rot * DEG_TO_RAD;
        parts[head].y_rot = st.y_rot * DEG_TO_RAD;
    }
}
