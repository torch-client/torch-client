use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;

use super::scaling;
use crate::util::mth::DEG_TO_RAD;
use crate::util::mth::rot_lerp;

const CAT_SCALE: f32 = 0.8;

const COLLAR_GROW: Grow = Grow::all(0.01);

const BABY_COLLAR_SCALE: f32 = 1.01;

fn adult_mesh(g: Grow) -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let tail_g = Grow::all(-0.02);
    root.child(
        "head",
        CubeList::new()
            .add_box_grow(-2.5, -2.0, -3.0, 5.0, 4.0, 5.0, g)
            .add_box_at(-1.5, -0.001, -4.0, 3.0, 2.0, 2.0, g, 0, 24)
            .add_box_at(-2.0, -3.0, 0.0, 1.0, 1.0, 2.0, g, 0, 10)
            .add_box_at(1.0, -3.0, 0.0, 1.0, 1.0, 2.0, g, 6, 10),
        PartPose::offset(0.0, 15.0, -9.0),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(20, 0)
            .add_box_grow(-2.0, 3.0, -8.0, 4.0, 16.0, 6.0, g),
        PartPose::offset_rotation(0.0, 12.0, -10.0, std::f32::consts::FRAC_PI_2, 0.0, 0.0),
    );
    root.child(
        "tail1",
        CubeList::new()
            .tex_offs(0, 15)
            .add_box_grow(-0.5, 0.0, 0.0, 1.0, 8.0, 1.0, g),
        PartPose::offset_rotation(0.0, 15.0, 8.0, 0.9, 0.0, 0.0),
    );
    root.child(
        "tail2",
        CubeList::new()
            .tex_offs(4, 15)
            .add_box_grow(-0.5, 0.0, 0.0, 1.0, 8.0, 1.0, tail_g),
        PartPose::offset(0.0, 20.0, 14.0),
    );
    let hind_leg = CubeList::new()
        .tex_offs(8, 13)
        .add_box_grow(-1.0, 0.0, 1.0, 2.0, 6.0, 2.0, g);
    root.child(
        "left_hind_leg",
        hind_leg.clone(),
        PartPose::offset(1.1, 18.0, 5.0),
    );
    root.child(
        "right_hind_leg",
        hind_leg,
        PartPose::offset(-1.1, 18.0, 5.0),
    );
    let front_leg = CubeList::new()
        .tex_offs(40, 0)
        .add_box_grow(-1.0, 0.0, 0.0, 2.0, 10.0, 2.0, g);
    root.child(
        "left_front_leg",
        front_leg.clone(),
        PartPose::offset(1.2, 14.1, -5.0),
    );
    root.child(
        "right_front_leg",
        front_leg,
        PartPose::offset(-1.2, 14.1, -5.0),
    );
    mesh
}

pub fn ocelot_layer() -> LayerDef {
    LayerDef::create(adult_mesh(Grow::NONE), 64, 32)
}

pub fn cat_layer() -> LayerDef {
    LayerDef::create(scaling(adult_mesh(Grow::NONE), CAT_SCALE), 64, 32)
}

pub fn cat_collar_layer() -> LayerDef {
    LayerDef::create(scaling(adult_mesh(COLLAR_GROW), CAT_SCALE), 64, 32)
}

pub fn baby_mesh() -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-2.5, -3.0, -2.875, 5.0, 4.0, 4.0)
            .tex_offs(18, 0)
            .add_box(-2.0, -4.0, -0.875, 1.0, 1.0, 2.0)
            .tex_offs(24, 0)
            .add_box(1.0, -4.0, -0.875, 1.0, 1.0, 2.0)
            .tex_offs(18, 3)
            .add_box(-1.5, -1.0, -3.875, 3.0, 2.0, 1.0),
        PartPose::offset(0.0, 20.0, -3.125),
    );
    root.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(18, 18)
            .add_box(-0.5, 0.0, -1.0, 1.0, 2.0, 2.0),
        PartPose::offset(1.0, 22.0, -1.5),
    );
    root.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(12, 18)
            .add_box(-0.5, 0.0, -1.0, 1.0, 2.0, 2.0),
        PartPose::offset(-1.0, 22.0, -1.5),
    );
    root.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(18, 22)
            .add_box(-0.5, 0.0, -1.0, 1.0, 2.0, 2.0),
        PartPose::offset(1.0, 22.0, 2.5),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 8)
            .add_box(-2.0, -1.5, -3.5, 4.0, 3.0, 7.0),
        PartPose::offset(0.0, 20.5, 0.5),
    );
    root.child(
        "right_hind_leg",
        CubeList::new()
            .tex_offs(12, 22)
            .add_box(-0.5, 0.0, -1.0, 1.0, 2.0, 2.0),
        PartPose::offset(-1.0, 22.0, 2.5),
    );
    root.child(
        "tail1",
        CubeList::new()
            .tex_offs(0, 18)
            .add_box(-0.5, -0.107, 0.0849, 1.0, 1.0, 5.0),
        PartPose::offset_rotation(0.0, 19.107, 3.9151, -0.567232, 0.0, 0.0),
    );
    root.child("tail2", CubeList::new(), PartPose::ZERO);
    mesh
}

pub fn baby_layer() -> LayerDef {
    LayerDef::create(baby_mesh(), 32, 32)
}

pub fn baby_collar_layer() -> LayerDef {
    LayerDef::create(scaling(baby_mesh(), BABY_COLLAR_SCALE), 32, 32)
}

struct Ids {
    head: usize,
    body: usize,
    tail1: usize,
    tail2: usize,
    left_hind: usize,
    right_hind: usize,
    left_front: usize,
    right_front: usize,
}

impl Ids {
    fn new(model: &BakedModel) -> Ids {
        Ids {
            head: model.id("head"),
            body: model.id("body"),
            tail1: model.id("tail1"),
            tail2: model.id("tail2"),
            left_hind: model.id("left_hind_leg"),
            right_hind: model.id("right_hind_leg"),
            left_front: model.id("left_front_leg"),
            right_front: model.id("right_front_leg"),
        }
    }
}

fn prelude(parts: &mut [PartState], id: &Ids, st: &EntityState) {
    let age_scale = st.age_scale;
    if st.extras.crouching {
        parts[id.body].y += 1.0 * age_scale;
        parts[id.head].y += 2.0 * age_scale;
        parts[id.tail1].y += 1.0 * age_scale;
        parts[id.tail2].y += -4.0 * age_scale;
        parts[id.tail2].z += 2.0 * age_scale;
        parts[id.tail1].x_rot = std::f32::consts::FRAC_PI_2;
        parts[id.tail2].x_rot = std::f32::consts::FRAC_PI_2;
    } else if st.extras.sprinting {
        parts[id.tail2].y = parts[id.tail1].y;
        parts[id.tail2].z += 2.0 * age_scale;
        parts[id.tail1].x_rot = std::f32::consts::FRAC_PI_2;
        parts[id.tail2].x_rot = std::f32::consts::FRAC_PI_2;
    }
    parts[id.head].x_rot = st.x_rot * DEG_TO_RAD;
    parts[id.head].y_rot = st.y_rot * DEG_TO_RAD;
}

fn walk(parts: &mut [PartState], id: &Ids, st: &EntityState) {
    let speed = st.walk_speed;
    let pos = st.walk_pos;
    if st.extras.sprinting {
        parts[id.left_hind].x_rot = (pos * 0.6662).cos() * speed;
        parts[id.right_hind].x_rot = (pos * 0.6662 + 0.3).cos() * speed;
        parts[id.left_front].x_rot = (pos * 0.6662 + std::f32::consts::PI + 0.3).cos() * speed;
        parts[id.right_front].x_rot = (pos * 0.6662 + std::f32::consts::PI).cos() * speed;
        parts[id.tail2].x_rot = 1.7278761 + 0.31415927 * pos.cos() * speed;
    } else {
        parts[id.left_hind].x_rot = (pos * 0.6662).cos() * speed;
        parts[id.right_hind].x_rot = (pos * 0.6662 + std::f32::consts::PI).cos() * speed;
        parts[id.left_front].x_rot = (pos * 0.6662 + std::f32::consts::PI).cos() * speed;
        parts[id.right_front].x_rot = (pos * 0.6662).cos() * speed;
        if !st.extras.crouching {
            parts[id.tail2].x_rot = 1.7278761 + 0.7853982 * pos.cos() * speed;
        } else {
            parts[id.tail2].x_rot = 1.7278761 + 0.47123894 * pos.cos() * speed;
        }
    }
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let id = Ids::new(model);
    let age_scale = st.age_scale;
    prelude(parts, &id, st);

    if !st.extras.sitting {
        parts[id.body].x_rot = std::f32::consts::FRAC_PI_2;
        walk(parts, &id, st);
    }

    if st.extras.sitting {
        parts[id.body].x_rot = 0.7853982;
        parts[id.body].y += -4.0 * age_scale;
        parts[id.body].z += 5.0 * age_scale;
        parts[id.head].y += -3.3 * age_scale;
        parts[id.head].z += 1.0 * age_scale;
        parts[id.tail1].y += 8.0 * age_scale;
        parts[id.tail1].z += -2.0 * age_scale;
        parts[id.tail2].y += 2.0 * age_scale;
        parts[id.tail2].z += -0.8 * age_scale;
        parts[id.tail1].x_rot = 1.7278761;
        parts[id.tail2].x_rot = 2.670354;
        parts[id.left_front].x_rot = -0.15707964;
        parts[id.left_front].y += 2.0 * age_scale;
        parts[id.left_front].z -= 2.0 * age_scale;
        parts[id.right_front].x_rot = -0.15707964;
        parts[id.right_front].y += 2.0 * age_scale;
        parts[id.right_front].z -= 2.0 * age_scale;
        parts[id.left_hind].x_rot = -std::f32::consts::FRAC_PI_2;
        parts[id.left_hind].y += 3.0 * age_scale;
        parts[id.left_hind].z -= 4.0 * age_scale;
        parts[id.right_hind].x_rot = -std::f32::consts::FRAC_PI_2;
        parts[id.right_hind].y += 3.0 * age_scale;
        parts[id.right_hind].z -= 4.0 * age_scale;
    }

    let lie = st.extras.lie_down_amount;
    if lie > 0.0 {
        parts[id.head].z_rot = rot_lerp(lie, parts[id.head].z_rot, -1.2707963);
        parts[id.head].y_rot = rot_lerp(lie, parts[id.head].y_rot, 1.2707963);
        parts[id.left_front].x_rot = -1.2707963;
        parts[id.right_front].x_rot = -0.47079635;
        parts[id.right_front].z_rot = -0.2;
        parts[id.right_front].x += age_scale;
        parts[id.left_hind].x_rot = -0.4;
        parts[id.right_hind].x_rot = 0.5;
        parts[id.right_hind].z_rot = -0.5;
        parts[id.right_hind].x += 0.8 * age_scale;
        parts[id.right_hind].y += 2.0 * age_scale;
        let tail_lie = st.extras.lie_down_amount_tail;
        parts[id.tail1].x_rot = rot_lerp(tail_lie, parts[id.tail1].x_rot, 0.8);
        parts[id.tail2].x_rot = rot_lerp(tail_lie, parts[id.tail2].x_rot, -0.4);
    }

    if st.extras.relax_state_one_amount > 0.0 {
        parts[id.head].x_rot = rot_lerp(
            st.extras.relax_state_one_amount,
            parts[id.head].x_rot,
            -0.58177644,
        );
    }
}

pub fn baby_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let id = Ids::new(model);
    prelude(parts, &id, st);

    if !st.extras.sitting {
        walk(parts, &id, st);
    } else {
        parts[id.body].x_rot += -0.43633232;
        parts[id.body].y += 1.0;
        parts[id.head].z += 0.75;
        parts[id.tail1].x_rot += 0.5454154;
        parts[id.tail1].y += 4.0;
        parts[id.tail1].z -= 0.9;
        parts[id.left_hind].z -= 0.9;
        parts[id.right_hind].z -= 0.9;
    }

    let lie = st.extras.lie_down_amount;
    if lie > 0.0 {
        parts[id.body].x += 1.0;
        parts[id.head].x_rot = rot_lerp(lie, parts[id.head].x_rot, 0.17453292);
        parts[id.head].z_rot = rot_lerp(lie, parts[id.head].z_rot, -1.3089969);
        parts[id.head].x += 1.0;
        parts[id.head].y += 0.75;
        parts[id.head].z -= 0.5;
        parts[id.right_front].x_rot = -0.7853982;
        parts[id.right_front].x += 3.5;
        parts[id.right_front].y -= 0.5;
        parts[id.left_front].x_rot = -std::f32::consts::FRAC_PI_2;
        parts[id.left_front].x += 1.0;
        parts[id.left_front].y -= 1.0;
        parts[id.left_front].z -= 2.0;
        parts[id.right_hind].x_rot = 0.6981317;
        parts[id.right_hind].y_rot = 0.34906584;
        parts[id.right_hind].z_rot = -0.34906584;
        parts[id.right_hind].x += 2.5;
        parts[id.right_hind].y -= 0.25;
        parts[id.right_hind].z += 0.5;
        parts[id.left_hind].x += 1.0;
        parts[id.left_hind].z -= 1.0;
        let tail_lie = st.extras.lie_down_amount_tail;
        parts[id.tail1].x_rot += rot_lerp(tail_lie, parts[id.tail1].x_rot, -0.5235988);
        parts[id.tail1].y_rot += rot_lerp(tail_lie, parts[id.tail1].y_rot, 0.0);
        parts[id.tail1].z_rot += rot_lerp(tail_lie, parts[id.tail1].z_rot, -0.17453292);
        parts[id.tail1].x += 1.0;
        parts[id.tail1].y += 0.5;
        parts[id.tail1].z -= 0.25;
    }

    if st.extras.relax_state_one_amount > 0.0 {
        parts[id.head].x_rot = rot_lerp(
            st.extras.relax_state_one_amount,
            parts[id.head].x_rot,
            -0.58177644,
        );
    }
}
