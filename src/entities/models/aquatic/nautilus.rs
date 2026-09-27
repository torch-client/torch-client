#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::keyframe::apply_walk;
use crate::entities::models::aquatic::DEG_TO_RAD;
use crate::entities::models::aquatic::animation::nautilus as anim;
use crate::entities::state::EntityState;

pub fn body_mesh() -> MeshDef {
    let mut mesh = MeshDef::new();
    let nautilus = mesh
        .root()
        .child("root", CubeList::new(), PartPose::offset(0.0, 29.0, -6.0));
    nautilus.child(
        "shell",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-7.0, -10.0, -7.0, 14.0, 10.0, 16.0)
            .tex_offs(0, 26)
            .add_box(-7.0, 0.0, -7.0, 14.0, 8.0, 20.0)
            .tex_offs(48, 26)
            .add_box(-7.0, 0.0, 6.0, 14.0, 8.0, 0.0),
        PartPose::offset(0.0, -13.0, 5.0),
    );
    let body = nautilus.child(
        "body",
        CubeList::new()
            .tex_offs(0, 54)
            .add_box(-5.0, -4.51, -3.0, 10.0, 8.0, 14.0)
            .tex_offs(0, 76)
            .add_box(-5.0, -4.51, 7.0, 10.0, 8.0, 0.0),
        PartPose::offset(0.0, -8.5, 12.3),
    );
    body.child(
        "upper_mouth",
        CubeList::new().tex_offs(54, 54).add_box_grow(
            -5.0,
            -2.0,
            0.0,
            10.0,
            4.0,
            4.0,
            Grow::all(-0.001),
        ),
        PartPose::offset(0.0, -2.51, 7.0),
    );
    body.child(
        "inner_mouth",
        CubeList::new()
            .tex_offs(54, 70)
            .add_box(-3.0, -2.0, -0.5, 6.0, 4.0, 4.0),
        PartPose::offset(0.0, -0.51, 7.5),
    );
    body.child(
        "lower_mouth",
        CubeList::new().tex_offs(54, 62).add_box_grow(
            -5.0,
            -1.98,
            0.0,
            10.0,
            4.0,
            4.0,
            Grow::all(-0.001),
        ),
        PartPose::offset(0.0, 1.49, 7.0),
    );
    mesh
}

pub fn nautilus_layer() -> LayerDef {
    LayerDef::create(body_mesh(), 128, 128)
}

pub fn baby_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let nautilus = mesh
        .root()
        .child("root", CubeList::new(), PartPose::offset(-0.5, 28.0, -0.5));
    nautilus.child(
        "shell",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-6.0, -4.0, -1.0, 7.0, 4.0, 7.0)
            .tex_offs(0, 11)
            .add_box(-6.0, 0.0, -1.0, 7.0, 4.0, 9.0)
            .tex_offs(23, 11)
            .add_box(-6.0, 0.0, 5.0, 7.0, 4.0, 0.0),
        PartPose::offset(3.0, -8.0, -2.0),
    );
    let body = nautilus.child(
        "body",
        CubeList::new()
            .tex_offs(0, 24)
            .add_box(-2.5, -3.01, -1.0, 5.0, 4.0, 7.0)
            .tex_offs(0, 35)
            .add_box(-2.5, -3.01, 4.1, 5.0, 4.0, 0.0),
        PartPose::offset(0.5, -5.0, 3.0),
    );
    body.child(
        "upper_mouth",
        CubeList::new().tex_offs(24, 24).add_box_grow(
            -2.5,
            -1.0,
            0.0,
            5.0,
            2.0,
            2.0,
            Grow::all(-0.001),
        ),
        PartPose::offset(0.0, -2.01, 3.9),
    );
    body.child(
        "inner_mouth",
        CubeList::new()
            .tex_offs(24, 32)
            .add_box(-1.5, -1.0, -1.0, 3.0, 2.0, 2.0),
        PartPose::offset(0.0, -1.01, 4.9),
    );
    body.child(
        "lower_mouth",
        CubeList::new().tex_offs(24, 28).add_box_grow(
            -2.5,
            -1.0,
            0.0,
            5.0,
            2.0,
            2.0,
            Grow::all(-0.001),
        ),
        PartPose::offset(0.0, -0.01, 3.9),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn armor_layer() -> LayerDef {
    let mut mesh = body_mesh();
    let nautilus = mesh
        .root()
        .child("root", CubeList::new(), PartPose::offset(0.0, 29.0, -6.0));
    nautilus.child(
        "shell",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box_grow(-7.0, -10.0, -7.0, 14.0, 10.0, 16.0, Grow::all(0.01))
            .tex_offs(0, 26)
            .add_box_grow(-7.0, 0.0, -7.0, 14.0, 8.0, 20.0, Grow::all(0.01))
            .tex_offs(48, 26)
            .add_box(-7.0, 0.0, 6.0, 14.0, 8.0, 0.0),
        PartPose::offset(0.0, -13.0, 5.0),
    );
    LayerDef::create(mesh, 128, 128)
}

pub fn saddle_layer() -> LayerDef {
    let mut mesh = body_mesh();
    let nautilus = mesh
        .root()
        .child("root", CubeList::new(), PartPose::offset(0.0, 29.0, -6.0));
    nautilus.child(
        "shell",
        CubeList::new().tex_offs(0, 0).add_box_grow(
            -7.0,
            -10.0,
            -7.0,
            14.0,
            10.0,
            16.0,
            Grow::all(0.2),
        ),
        PartPose::offset(0.0, -13.0, 5.0),
    );
    LayerDef::create(mesh, 128, 128)
}

pub fn zombie_coral_layer() -> LayerDef {
    let mut mesh = body_mesh();
    let corals = mesh.root().get("root/shell").child(
        "corals",
        CubeList::new(),
        PartPose::offset(8.0, 4.5, -8.0),
    );
    let yellow = corals.child(
        "yellow_coral",
        CubeList::new(),
        PartPose::offset(0.0, -11.0, 11.0),
    );
    yellow.child(
        "yellow_coral_second",
        CubeList::new()
            .tex_offs(0, 85)
            .add_box(-4.5, -3.5, 0.0, 6.0, 8.0, 0.0),
        PartPose::offset_rotation(0.0, 0.0, 2.0, 0.0, -0.7854, 0.0),
    );
    yellow.child(
        "yellow_coral_first",
        CubeList::new()
            .tex_offs(0, 85)
            .add_box(-4.5, -3.5, 0.0, 6.0, 8.0, 0.0),
        PartPose::offset_rotation(0.0, 0.0, 0.0, 0.0, 0.7854, 0.0),
    );
    corals
        .child(
            "pink_coral",
            CubeList::new()
                .tex_offs(-8, 94)
                .add_box(-4.5, 4.5, 0.0, 6.0, 0.0, 8.0),
            PartPose::offset(-12.5, -18.0, 11.0),
        )
        .child(
            "pink_coral_second",
            CubeList::new()
                .tex_offs(-8, 94)
                .add_box(-3.0, 0.0, -4.0, 6.0, 0.0, 8.0),
            PartPose::offset_rotation(-1.5, 4.5, 4.0, 0.0, 0.0, 1.5708),
        );
    let blue = corals.child(
        "blue_coral",
        CubeList::new(),
        PartPose::offset(-14.0, 0.0, 5.5),
    );
    blue.child(
        "blue_second",
        CubeList::new()
            .tex_offs(0, 102)
            .add_box(-3.5, -5.5, 0.0, 5.0, 10.0, 0.0),
        PartPose::offset_rotation(0.0, 0.0, -2.0, 0.0, 0.7854, 0.0),
    );
    blue.child(
        "blue_first",
        CubeList::new()
            .tex_offs(0, 102)
            .add_box(-3.5, -5.5, 0.0, 5.0, 10.0, 0.0),
        PartPose::offset_rotation(0.0, 0.0, 0.0, 0.0, -0.7854, 0.0),
    );
    let red = corals.child(
        "red_coral",
        CubeList::new(),
        PartPose::offset(0.0, 0.0, 0.0),
    );
    red.child(
        "red_coral_second",
        CubeList::new()
            .tex_offs(0, 112)
            .add_box(-2.5, -5.5, 0.0, 4.0, 10.0, 0.0),
        PartPose::offset_rotation(-0.5, -1.0, 1.5, 0.0, -0.829, 0.0),
    );
    red.child(
        "red_coral_first",
        CubeList::new()
            .tex_offs(0, 112)
            .add_box(-4.5, -5.5, 0.0, 6.0, 10.0, 0.0),
        PartPose::offset_rotation(0.0, 0.0, 0.0, 0.0, 0.7854, 0.0),
    );
    LayerDef::create(mesh, 128, 128)
}

const SWIM_ANIMATION_SPEED_MAX: f32 = 2.0;
const SWIM_ANIMATION_SCALE_FACTOR: f32 = 3.0;
const IDLE_SWIM_ANIMATION_SPEED: f32 = 0.2;
const IDLE_SWIM_ANIMATION_SCALE: f32 = 5.0;

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let body = model.id("body");
    parts[body].y_rot = st.y_rot.clamp(-10.0, 10.0) * DEG_TO_RAD;
    parts[body].x_rot = st.x_rot.clamp(-10.0, 10.0) * DEG_TO_RAD;
    apply_walk(
        &anim::SWIMMING,
        model,
        parts,
        st.walk_pos + st.age_ticks / IDLE_SWIM_ANIMATION_SCALE,
        st.walk_speed + IDLE_SWIM_ANIMATION_SPEED,
        SWIM_ANIMATION_SPEED_MAX,
        SWIM_ANIMATION_SCALE_FACTOR,
    );
}

pub fn zombie_coral_setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup_anim(model, parts, st);
    parts[model.id("corals")].visible = st.extras.body_armor.is_none();
}
