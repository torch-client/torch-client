use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;

use super::quadruped;

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "head",
        CubeList::new()
            .tex_offs(3, 0)
            .add_box(-3.0, -1.0, -3.0, 6.0, 5.0, 6.0),
        PartPose::offset(0.0, 19.0, -10.0),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(7, 37)
            .add_box(-9.5, 3.0, -10.0, 19.0, 20.0, 6.0)
            .tex_offs(31, 1)
            .add_box(-5.5, 3.0, -13.0, 11.0, 18.0, 3.0),
        PartPose::offset_rotation(0.0, 11.0, -10.0, std::f32::consts::FRAC_PI_2, 0.0, 0.0),
    );
    root.child(
        "egg_belly",
        CubeList::new()
            .tex_offs(70, 33)
            .add_box(-4.5, 3.0, -14.0, 9.0, 18.0, 1.0),
        PartPose::offset_rotation(0.0, 11.0, -10.0, std::f32::consts::FRAC_PI_2, 0.0, 0.0),
    );
    root.child(
        "right_hind_leg",
        CubeList::new()
            .tex_offs(1, 23)
            .add_box(-2.0, 0.0, 0.0, 4.0, 1.0, 10.0),
        PartPose::offset(-3.5, 22.0, 11.0),
    );
    root.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(1, 12)
            .add_box(-2.0, 0.0, 0.0, 4.0, 1.0, 10.0),
        PartPose::offset(3.5, 22.0, 11.0),
    );
    root.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(27, 30)
            .add_box(-13.0, 0.0, -2.0, 13.0, 1.0, 5.0),
        PartPose::offset(-5.0, 21.0, -4.0),
    );
    root.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(27, 24)
            .add_box(0.0, 0.0, -2.0, 13.0, 1.0, 5.0),
        PartPose::offset(5.0, 21.0, -4.0),
    );
    LayerDef::create(mesh, 128, 64)
}

pub fn baby_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-2.0, -1.0, -2.0, 4.0, 2.0, 4.0),
        PartPose::offset(0.0, 22.9, 1.0),
    );
    root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 6)
            .add_box(-1.5, -2.0, -3.0, 3.0, 3.0, 3.0),
        PartPose::offset(0.0, 22.9, -1.0),
    );
    root.child(
        "right_hind_leg",
        CubeList::new()
            .tex_offs(-1, 0)
            .add_box(-2.0, 0.0, -0.5, 2.0, 0.0, 1.0),
        PartPose::offset(-2.0, 23.9, 2.5),
    );
    root.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(-1, 1)
            .add_box(0.0, 0.0, -0.5, 2.0, 0.0, 1.0),
        PartPose::offset(2.0, 23.9, 2.5),
    );
    root.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(8, 6)
            .add_box(-2.0, 0.0, -0.5, 2.0, 0.0, 1.0),
        PartPose::offset(-2.0, 23.9, -0.5),
    );
    root.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(8, 7)
            .add_box(0.0, 0.0, -0.5, 2.0, 0.0, 1.0),
        PartPose::offset(2.0, 23.9, -0.5),
    );
    LayerDef::create(mesh, 16, 16)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    quadruped::setup_anim(model, parts, st);
    let pos = st.walk_pos;
    let speed = st.walk_speed;
    let right_front = model.id("right_front_leg");
    let left_front = model.id("left_front_leg");
    let right_hind = model.id("right_hind_leg");
    let left_hind = model.id("left_hind_leg");

    if st.extras.on_land {
        let lay_egg = if st.extras.laying_egg { 4.0 } else { 1.0 };
        let amplitude = if st.extras.laying_egg { 2.0 } else { 1.0 };
        let swing_pos = pos * 5.0;
        let front_swing = (lay_egg * swing_pos).cos();
        let hind_swing = swing_pos.cos();
        parts[right_front].y_rot = -front_swing * 8.0 * speed * amplitude;
        parts[left_front].y_rot = front_swing * 8.0 * speed * amplitude;
        parts[right_hind].y_rot = -hind_swing * 3.0 * speed;
        parts[left_hind].y_rot = hind_swing * 3.0 * speed;
    } else {
        let swim = 0.5 * speed;
        let amplitude = (pos * 0.6662 * 0.6).cos() * swim;
        parts[right_hind].x_rot = amplitude;
        parts[left_hind].x_rot = -amplitude;
        parts[right_front].z_rot = -amplitude;
        parts[left_front].z_rot = amplitude;
    }

    if let Some(egg_belly) = model.find("egg_belly") {
        let has_egg = st.extras.has_egg && !st.extras.is_baby;
        parts[egg_belly].visible = has_egg;
        if has_egg {
            parts[model.id("root")].y -= 1.0;
        }
    }
}
