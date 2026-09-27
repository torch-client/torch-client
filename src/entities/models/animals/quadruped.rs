use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

pub fn body_mesh(leg_size: i32, mirror_left_leg: bool, mirror_right_leg: bool, g: Grow) -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box_grow(-4.0, -4.0, -8.0, 8.0, 8.0, 8.0, g),
        PartPose::offset(0.0, (18 - leg_size) as f32, -6.0),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(28, 8)
            .add_box_grow(-5.0, -10.0, -7.0, 10.0, 16.0, 8.0, g),
        PartPose::offset_rotation(
            0.0,
            (17 - leg_size) as f32,
            2.0,
            std::f32::consts::FRAC_PI_2,
            0.0,
            0.0,
        ),
    );
    create_legs(root, mirror_left_leg, mirror_right_leg, leg_size, g);
    mesh
}

pub fn create_legs(
    root: &mut crate::entities::geom::PartDef,
    mirror_left_leg: bool,
    mirror_right_leg: bool,
    leg_size: i32,
    g: Grow,
) {
    let right_leg = CubeList::new()
        .mirror_if(mirror_right_leg)
        .tex_offs(0, 16)
        .add_box_grow(-2.0, 0.0, -2.0, 4.0, leg_size as f32, 4.0, g);
    let left_leg = CubeList::new()
        .mirror_if(mirror_left_leg)
        .tex_offs(0, 16)
        .add_box_grow(-2.0, 0.0, -2.0, 4.0, leg_size as f32, 4.0, g);
    let y = (24 - leg_size) as f32;
    root.child(
        "right_hind_leg",
        right_leg.clone(),
        PartPose::offset(-3.0, y, 7.0),
    );
    root.child(
        "left_hind_leg",
        left_leg.clone(),
        PartPose::offset(3.0, y, 7.0),
    );
    root.child(
        "right_front_leg",
        right_leg,
        PartPose::offset(-3.0, y, -5.0),
    );
    root.child("left_front_leg", left_leg, PartPose::offset(3.0, y, -5.0));
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
}

fn pig_body_mesh(g: Grow) -> MeshDef {
    let mut mesh = body_mesh(6, true, false, g);
    mesh.root().child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box_grow(-4.0, -4.0, -8.0, 8.0, 8.0, 8.0, g)
            .tex_offs(16, 16)
            .add_box_grow(-2.0, 0.0, -9.0, 4.0, 3.0, 1.0, g),
        PartPose::offset(0.0, 12.0, -6.0),
    );
    mesh
}

pub fn pig_layer() -> LayerDef {
    LayerDef::create(pig_body_mesh(Grow::NONE), 64, 64)
}

pub fn pig_saddle_layer() -> LayerDef {
    LayerDef::create(pig_body_mesh(Grow::all(0.5)), 64, 64)
}

pub fn cold_pig_layer() -> LayerDef {
    let mut mesh = pig_body_mesh(Grow::NONE);
    mesh.root().child(
        "body",
        CubeList::new()
            .tex_offs(28, 8)
            .add_box(-5.0, -10.0, -7.0, 10.0, 16.0, 8.0)
            .tex_offs(28, 32)
            .add_box_grow(-5.0, -10.0, -7.0, 10.0, 16.0, 8.0, Grow::all(0.5)),
        PartPose::offset_rotation(0.0, 11.0, 2.0, std::f32::consts::FRAC_PI_2, 0.0, 0.0),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn baby_pig_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-3.5, -3.0, -4.5, 7.0, 6.0, 9.0),
        PartPose::offset(0.0, 19.0, 0.5),
    );
    root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 15)
            .add_box_grow(-3.5, -5.0, -5.0, 7.0, 6.0, 6.0, Grow::all(0.025))
            .tex_offs(6, 27)
            .add_box_grow(-1.5, -1.975, -6.0, 3.0, 2.0, 1.0, Grow::all(0.015)),
        PartPose::offset(0.0, 19.0, -2.0),
    );
    root.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-1.0, 0.0, -1.0, 2.0, 2.0, 2.0),
        PartPose::offset(2.5, 22.0, -3.0),
    );
    root.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(23, 0)
            .add_box(-1.0, 0.0, -1.0, 2.0, 2.0, 2.0),
        PartPose::offset(-2.5, 22.0, -3.0),
    );
    root.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(0, 4)
            .add_box(-1.0, 0.0, -1.0, 2.0, 2.0, 2.0),
        PartPose::offset(2.5, 22.0, 4.0),
    );
    root.child(
        "right_hind_leg",
        CubeList::new()
            .tex_offs(23, 4)
            .add_box(-1.0, 0.0, -1.0, 2.0, 2.0, 2.0),
        PartPose::offset(-2.5, 22.0, 4.0),
    );
    LayerDef::create(mesh, 32, 32)
}
