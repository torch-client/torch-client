use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

pub fn adult_mesh(g: Grow) -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box_grow(-2.0, -14.0, -10.0, 4.0, 4.0, 9.0, g)
            .tex_offs(0, 14)
            .add_box_grow(-4.0, -16.0, -6.0, 8.0, 18.0, 6.0, g)
            .tex_offs(17, 0)
            .add_box_grow(-4.0, -19.0, -4.0, 3.0, 3.0, 2.0, g)
            .tex_offs(17, 0)
            .add_box_grow(1.0, -19.0, -4.0, 3.0, 3.0, 2.0, g),
        PartPose::offset(0.0, 7.0, -6.0),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(29, 0)
            .add_box_grow(-6.0, -10.0, -7.0, 12.0, 18.0, 10.0, g),
        PartPose::offset_rotation(0.0, 5.0, 2.0, std::f32::consts::FRAC_PI_2, 0.0, 0.0),
    );
    root.child(
        "right_chest",
        CubeList::new()
            .tex_offs(45, 28)
            .add_box_grow(-3.0, 0.0, 0.0, 8.0, 8.0, 3.0, g),
        PartPose::offset_rotation(-8.5, 3.0, 3.0, 0.0, std::f32::consts::FRAC_PI_2, 0.0),
    );
    root.child(
        "left_chest",
        CubeList::new()
            .tex_offs(45, 41)
            .add_box_grow(-3.0, 0.0, 0.0, 8.0, 8.0, 3.0, g),
        PartPose::offset_rotation(5.5, 3.0, 3.0, 0.0, std::f32::consts::FRAC_PI_2, 0.0),
    );
    let leg = CubeList::new()
        .tex_offs(29, 29)
        .add_box_grow(-2.0, 0.0, -2.0, 4.0, 14.0, 4.0, g);
    root.child(
        "right_hind_leg",
        leg.clone(),
        PartPose::offset(-3.5, 10.0, 6.0),
    );
    root.child(
        "left_hind_leg",
        leg.clone(),
        PartPose::offset(3.5, 10.0, 6.0),
    );
    root.child(
        "right_front_leg",
        leg.clone(),
        PartPose::offset(-3.5, 10.0, -5.0),
    );
    root.child("left_front_leg", leg, PartPose::offset(3.5, 10.0, -5.0));
    mesh
}

pub fn baby_mesh(g: Grow) -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box_grow(-3.0, -9.0, -4.0, 6.0, 11.0, 4.0, g)
            .tex_offs(0, 15)
            .add_box_grow(-1.5, -7.0, -7.0, 3.0, 3.0, 3.0, g)
            .tex_offs(20, 4)
            .add_box_grow(0.5, -11.0, -3.0, 2.0, 2.0, 2.0, g)
            .tex_offs(20, 0)
            .add_box_grow(-2.5, -11.0, -3.0, 2.0, 2.0, 2.0, g),
        PartPose::offset(0.0, 12.0, -4.0),
    );
    root.child(
        "right_hind_leg",
        CubeList::new()
            .tex_offs(0, 45)
            .add_box_grow(-1.4, -0.5, -1.5, 3.0, 8.0, 3.0, g),
        PartPose::offset(-2.5, 16.5, 4.5),
    );
    root.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(12, 45)
            .add_box_grow(-1.6, -0.5, -1.5, 3.0, 8.0, 3.0, g),
        PartPose::offset(2.5, 16.5, 4.5),
    );
    root.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(0, 34)
            .add_box_grow(-1.4, -0.5, -1.5, 3.0, 8.0, 3.0, g),
        PartPose::offset(-2.5, 16.5, -3.5),
    );
    root.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(12, 34)
            .add_box_grow(-1.6, -0.5, -1.5, 3.0, 8.0, 3.0, g),
        PartPose::offset(2.5, 16.5, -3.5),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 15)
            .add_box_grow(-4.0, -3.0, -8.5, 8.0, 6.0, 13.0, g),
        PartPose::offset(0.0, 14.0, 2.5),
    );
    root.child(
        "right_chest",
        CubeList::new()
            .tex_offs(45, 28)
            .add_box_grow(-3.0, 0.0, 0.0, 8.0, 8.0, 3.0, g),
        PartPose::offset_rotation(-8.5, 4.0, 3.0, 0.0, std::f32::consts::FRAC_PI_2, 0.0),
    );
    root.child(
        "left_chest",
        CubeList::new()
            .tex_offs(45, 41)
            .add_box_grow(-3.0, 0.0, 0.0, 8.0, 8.0, 3.0, g),
        PartPose::offset_rotation(5.5, 4.0, 3.0, 0.0, std::f32::consts::FRAC_PI_2, 0.0),
    );
    mesh
}

pub fn layer() -> LayerDef {
    LayerDef::create(adult_mesh(Grow::NONE), 128, 64)
}

pub fn decor_layer() -> LayerDef {
    LayerDef::create(adult_mesh(Grow::all(0.5)), 128, 64)
}

pub fn baby_layer() -> LayerDef {
    LayerDef::create(baby_mesh(Grow::NONE), 64, 64)
}

pub fn baby_decor_layer() -> LayerDef {
    LayerDef::create(baby_mesh(Grow::all(0.2)), 64, 64)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let head = model.id("head");
    parts[head].x_rot = st.x_rot * DEG_TO_RAD;
    parts[head].y_rot = st.y_rot * DEG_TO_RAD;

    let pos = st.walk_pos;
    let speed = st.walk_speed;
    let right_hind = model.id("right_hind_leg");
    let left_hind = model.id("left_hind_leg");
    let right_front = model.id("right_front_leg");
    let left_front = model.id("left_front_leg");
    parts[right_hind].x_rot = (pos * 0.6662).cos() * 1.4 * speed;
    parts[left_hind].x_rot = (pos * 0.6662 + std::f32::consts::PI).cos() * 1.4 * speed;
    parts[right_front].x_rot = (pos * 0.6662 + std::f32::consts::PI).cos() * 1.4 * speed;
    parts[left_front].x_rot = (pos * 0.6662).cos() * 1.4 * speed;

    let chest = st.extras.has_chest && !st.extras.is_baby;
    parts[model.id("right_chest")].visible = chest;
    parts[model.id("left_chest")].visible = chest;
}
