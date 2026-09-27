#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

fn triangle_wave(value: f32, period: f32) -> f32 {
    ((value % period - period * 0.5).abs() - period * 0.25) / (period * 0.25)
}

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let neck = root.child(
        "neck",
        CubeList::new()
            .tex_offs(68, 73)
            .add_box(-5.0, -1.0, -18.0, 10.0, 10.0, 18.0),
        PartPose::offset(0.0, -7.0, 5.5),
    );
    let head = neck.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-8.0, -20.0, -14.0, 16.0, 20.0, 16.0)
            .tex_offs(0, 0)
            .add_box(-2.0, -6.0, -18.0, 4.0, 8.0, 4.0),
        PartPose::offset(0.0, 16.0, -17.0),
    );
    head.child(
        "right_horn",
        CubeList::new()
            .tex_offs(74, 55)
            .add_box(0.0, -14.0, -2.0, 2.0, 14.0, 4.0),
        PartPose::offset_rotation(-10.0, -14.0, -8.0, 1.0995574, 0.0, 0.0),
    );
    head.child(
        "left_horn",
        CubeList::new()
            .tex_offs(74, 55)
            .mirror()
            .add_box(0.0, -14.0, -2.0, 2.0, 14.0, 4.0),
        PartPose::offset_rotation(8.0, -14.0, -8.0, 1.0995574, 0.0, 0.0),
    );
    head.child(
        "mouth",
        CubeList::new()
            .tex_offs(0, 36)
            .add_box(-8.0, 0.0, -16.0, 16.0, 3.0, 16.0),
        PartPose::offset(0.0, -2.0, 2.0),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 55)
            .add_box(-7.0, -10.0, -7.0, 14.0, 16.0, 20.0)
            .tex_offs(0, 91)
            .add_box(-6.0, 6.0, -7.0, 12.0, 13.0, 18.0),
        PartPose::offset_rotation(0.0, 1.0, 2.0, 1.5707964, 0.0, 0.0),
    );
    root.child(
        "right_hind_leg",
        CubeList::new()
            .tex_offs(96, 0)
            .add_box(-4.0, 0.0, -4.0, 8.0, 37.0, 8.0),
        PartPose::offset(-8.0, -13.0, 18.0),
    );
    root.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(96, 0)
            .mirror()
            .add_box(-4.0, 0.0, -4.0, 8.0, 37.0, 8.0),
        PartPose::offset(8.0, -13.0, 18.0),
    );
    root.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(64, 0)
            .add_box(-4.0, 0.0, -4.0, 8.0, 37.0, 8.0),
        PartPose::offset(-8.0, -13.0, -5.0),
    );
    root.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(64, 0)
            .mirror()
            .add_box(-4.0, 0.0, -4.0, 8.0, 37.0, 8.0),
        PartPose::offset(8.0, -13.0, -5.0),
    );
    LayerDef::create(mesh, 128, 128)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let neck = model.id("neck");
    let head = model.id("head");
    let mouth = model.id("mouth");

    let stunned = st.extras.stunned_ticks;
    let attack = st.extras.attack_ticks;
    if attack > 0.0 {
        let wave = triangle_wave(attack, 10.0);
        let lift = (1.0 + wave) * 0.5;
        let head_pos = lift * lift * lift * 12.0;
        let drop = head_pos * parts[neck].x_rot.sin();
        parts[neck].z = -6.5 + head_pos;
        parts[neck].y = -7.0 - drop;
        if attack > 5.0 {
            parts[mouth].x_rot = ((-4.0 + attack) / 4.0).sin() * 3.1415927 * 0.4;
        } else {
            parts[mouth].x_rot = 0.15707964 * (3.1415927 * attack / 10.0).sin();
        }
    } else {
        let drop = -parts[neck].x_rot.sin();
        parts[neck].x = 0.0;
        parts[neck].y = -7.0 - drop;
        parts[neck].z = 5.5;
        let is_stunned = stunned > 0.0;
        parts[neck].x_rot = if is_stunned { 0.21991149 } else { 0.0 };
        parts[mouth].x_rot = 3.1415927 * if is_stunned { 0.05 } else { 0.01 };
        if is_stunned {
            let speed = stunned as f64 / 40.0;
            parts[neck].x = (speed * 10.0).sin() as f32 * 3.0;
        } else if st.extras.roar_animation > 0.0 {
            let roar = (st.extras.roar_animation * 3.1415927 * 0.25).sin();
            parts[mouth].x_rot = 1.5707964 * roar;
        }
    }

    parts[head].x_rot = st.x_rot * DEG_TO_RAD;
    parts[head].y_rot = st.y_rot * DEG_TO_RAD;

    let pos = st.walk_pos;
    let leg = 0.4 * st.walk_speed;
    parts[model.id("right_hind_leg")].x_rot = (pos * 0.6662).cos() * leg;
    parts[model.id("left_hind_leg")].x_rot = (pos * 0.6662 + 3.1415927).cos() * leg;
    parts[model.id("right_front_leg")].x_rot = (pos * 0.6662 + 3.1415927).cos() * leg;
    parts[model.id("left_front_leg")].x_rot = (pos * 0.6662).cos() * leg;
}
