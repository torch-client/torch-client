#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

const RIBCAGE_X_ROT_OFFSET: f32 = 0.065;
const TAIL_X_ROT_OFFSET: f32 = 0.265;
const RIBCAGE_X_ROT: f32 = 0.20420352;

pub fn body_layer(g: Grow) -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "shoulders",
        CubeList::new()
            .tex_offs(0, 16)
            .add_box_grow(-10.0, 3.9, -0.5, 20.0, 3.0, 3.0, g),
        PartPose::ZERO,
    );
    root.child(
        "ribcage",
        CubeList::new()
            .tex_offs(0, 22)
            .add_box_grow(0.0, 0.0, 0.0, 3.0, 10.0, 3.0, g)
            .tex_offs(24, 22)
            .add_box_grow(-4.0, 1.5, 0.5, 11.0, 2.0, 2.0, g)
            .tex_offs(24, 22)
            .add_box_grow(-4.0, 4.0, 0.5, 11.0, 2.0, 2.0, g)
            .tex_offs(24, 22)
            .add_box_grow(-4.0, 6.5, 0.5, 11.0, 2.0, 2.0, g),
        PartPose::offset_rotation(-2.0, 6.9, -0.5, RIBCAGE_X_ROT, 0.0, 0.0),
    );
    root.child(
        "tail",
        CubeList::new()
            .tex_offs(12, 22)
            .add_box_grow(0.0, 0.0, 0.0, 3.0, 6.0, 3.0, g),
        PartPose::offset_rotation(
            -2.0,
            6.9 + RIBCAGE_X_ROT.cos() * 10.0,
            -0.5 + RIBCAGE_X_ROT.sin() * 10.0,
            0.83252203,
            0.0,
            0.0,
        ),
    );
    root.child(
        "center_head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box_grow(-4.0, -4.0, -4.0, 8.0, 8.0, 8.0, g),
        PartPose::ZERO,
    );
    let side_head = CubeList::new()
        .tex_offs(32, 0)
        .add_box_grow(-4.0, -4.0, -4.0, 6.0, 6.0, 6.0, g);
    root.child(
        "right_head",
        side_head.clone(),
        PartPose::offset(-8.0, 4.0, 0.0),
    );
    root.child("left_head", side_head, PartPose::offset(10.0, 4.0, 0.0));
    LayerDef::create(mesh, 64, 64)
}

pub fn layer() -> LayerDef {
    body_layer(Grow::NONE)
}

pub fn armor_layer() -> LayerDef {
    body_layer(Grow::all(0.5))
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    for (index, name) in [(0usize, "right_head"), (1, "left_head")] {
        let id = model.id(name);
        parts[id].y_rot = (st.extras.head_y_rots[index] - st.body_rot) * DEG_TO_RAD;
        parts[id].x_rot = st.extras.head_x_rots[index] * DEG_TO_RAD;
    }

    let anim = (st.age_ticks * 0.1).cos();
    let ribcage = model.id("ribcage");
    let tail = model.id("tail");
    parts[ribcage].x_rot = (RIBCAGE_X_ROT_OFFSET + 0.05 * anim) * 3.1415927;
    let x_rot = parts[ribcage].x_rot;
    parts[tail].set_pos(-2.0, 6.9 + x_rot.cos() * 10.0, -0.5 + x_rot.sin() * 10.0);
    parts[tail].x_rot = (TAIL_X_ROT_OFFSET + 0.1 * anim) * 3.1415927;

    let center = model.id("center_head");
    parts[center].y_rot = st.y_rot * DEG_TO_RAD;
    parts[center].x_rot = st.x_rot * DEG_TO_RAD;
}
