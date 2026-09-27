use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

pub fn body_layer(g: Grow) -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box_grow(-4.0, -8.0, -4.0, 8.0, 8.0, 8.0, g),
        PartPose::offset(0.0, 6.0, 0.0),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(16, 16)
            .add_box_grow(-4.0, 0.0, -2.0, 8.0, 12.0, 4.0, g),
        PartPose::offset(0.0, 6.0, 0.0),
    );
    let leg = CubeList::new()
        .tex_offs(0, 16)
        .add_box_grow(-2.0, 0.0, -2.0, 4.0, 6.0, 4.0, g);
    root.child(
        "right_hind_leg",
        leg.clone(),
        PartPose::offset(-2.0, 18.0, 4.0),
    );
    root.child(
        "left_hind_leg",
        leg.clone(),
        PartPose::offset(2.0, 18.0, 4.0),
    );
    root.child(
        "right_front_leg",
        leg.clone(),
        PartPose::offset(-2.0, 18.0, -4.0),
    );
    root.child("left_front_leg", leg, PartPose::offset(2.0, 18.0, -4.0));
    LayerDef::create(mesh, 64, 32)
}

pub fn layer() -> LayerDef {
    body_layer(Grow::NONE)
}

pub fn armor_layer() -> LayerDef {
    body_layer(Grow::all(2.0))
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let head = model.id("head");
    parts[head].y_rot = st.y_rot * DEG_TO_RAD;
    parts[head].x_rot = st.x_rot * DEG_TO_RAD;

    let speed = st.walk_speed;
    let pos = st.walk_pos;
    let right_hind = model.id("left_hind_leg");
    let left_hind = model.id("right_hind_leg");
    let right_front = model.id("left_front_leg");
    let left_front = model.id("right_front_leg");
    parts[right_hind].x_rot = (pos * 0.6662).cos() * 1.4 * speed;
    parts[left_hind].x_rot = (pos * 0.6662 + std::f32::consts::PI).cos() * 1.4 * speed;
    parts[right_front].x_rot = (pos * 0.6662 + std::f32::consts::PI).cos() * 1.4 * speed;
    parts[left_front].x_rot = (pos * 0.6662).cos() * 1.4 * speed;
}
