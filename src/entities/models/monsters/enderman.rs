#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

fn humanoid_mesh(g: Grow, y_offset: f32) -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let head = root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box_grow(-4.0, -8.0, -4.0, 8.0, 8.0, 8.0, g),
        PartPose::offset(0.0, y_offset, 0.0),
    );
    head.child(
        "hat",
        CubeList::new().tex_offs(32, 0).add_box_grow(
            -4.0,
            -8.0,
            -4.0,
            8.0,
            8.0,
            8.0,
            g.extend(0.5),
        ),
        PartPose::ZERO,
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(16, 16)
            .add_box_grow(-4.0, 0.0, -2.0, 8.0, 12.0, 4.0, g),
        PartPose::offset(0.0, y_offset, 0.0),
    );
    root.child(
        "right_arm",
        CubeList::new()
            .tex_offs(40, 16)
            .add_box_grow(-3.0, -2.0, -2.0, 4.0, 12.0, 4.0, g),
        PartPose::offset(-5.0, 2.0 + y_offset, 0.0),
    );
    root.child(
        "left_arm",
        CubeList::new()
            .tex_offs(40, 16)
            .mirror()
            .add_box_grow(-1.0, -2.0, -2.0, 4.0, 12.0, 4.0, g),
        PartPose::offset(5.0, 2.0 + y_offset, 0.0),
    );
    root.child(
        "right_leg",
        CubeList::new()
            .tex_offs(0, 16)
            .add_box_grow(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0, g),
        PartPose::offset(-1.9, 12.0 + y_offset, 0.0),
    );
    root.child(
        "left_leg",
        CubeList::new()
            .tex_offs(0, 16)
            .mirror()
            .add_box_grow(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0, g),
        PartPose::offset(1.9, 12.0 + y_offset, 0.0),
    );
    mesh
}

pub fn layer() -> LayerDef {
    let mut mesh = humanoid_mesh(Grow::NONE, -14.0);
    let root = mesh.root();
    let head = root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -8.0, -4.0, 8.0, 8.0, 8.0),
        PartPose::offset(0.0, -13.0, 0.0),
    );
    head.child(
        "hat",
        CubeList::new().tex_offs(0, 16).add_box_grow(
            -4.0,
            -8.0,
            -4.0,
            8.0,
            8.0,
            8.0,
            Grow::all(-0.5),
        ),
        PartPose::ZERO,
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(32, 16)
            .add_box(-4.0, 0.0, -2.0, 8.0, 12.0, 4.0),
        PartPose::offset(0.0, -14.0, 0.0),
    );
    root.child(
        "right_arm",
        CubeList::new()
            .tex_offs(56, 0)
            .add_box(-1.0, -2.0, -1.0, 2.0, 30.0, 2.0),
        PartPose::offset(-5.0, -12.0, 0.0),
    );
    root.child(
        "left_arm",
        CubeList::new()
            .tex_offs(56, 0)
            .mirror()
            .add_box(-1.0, -2.0, -1.0, 2.0, 30.0, 2.0),
        PartPose::offset(5.0, -12.0, 0.0),
    );
    root.child(
        "right_leg",
        CubeList::new()
            .tex_offs(56, 0)
            .add_box(-1.0, 0.0, -1.0, 2.0, 30.0, 2.0),
        PartPose::offset(-2.0, -5.0, 0.0),
    );
    root.child(
        "left_leg",
        CubeList::new()
            .tex_offs(56, 0)
            .mirror()
            .add_box(-1.0, 0.0, -1.0, 2.0, 30.0, 2.0),
        PartPose::offset(2.0, -5.0, 0.0),
    );
    LayerDef::create(mesh, 64, 32)
}

fn bob_model_part(part: &mut PartState, age_ticks: f32, scale: f32) {
    part.z_rot += scale * ((age_ticks * 0.09).cos() * 0.05 + 0.05);
    part.x_rot += scale * (age_ticks * 0.067).sin() * 0.05;
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let head = model.id("head");
    let hat = model.id("hat");
    let body = model.id("body");
    let right_arm = model.id("right_arm");
    let left_arm = model.id("left_arm");
    let right_leg = model.id("right_leg");
    let left_leg = model.id("left_leg");

    parts[head].x_rot = st.x_rot * DEG_TO_RAD;
    parts[head].y_rot = st.y_rot * DEG_TO_RAD;

    let pos = st.walk_pos;
    let speed = st.walk_speed;
    parts[right_arm].x_rot = (pos * 0.6662 + 3.1415927).cos() * 2.0 * speed * 0.5;
    parts[left_arm].x_rot = (pos * 0.6662).cos() * 2.0 * speed * 0.5;
    parts[right_leg].x_rot = (pos * 0.6662).cos() * 1.4 * speed;
    parts[left_leg].x_rot = (pos * 0.6662 + 3.1415927).cos() * 1.4 * speed;
    parts[right_leg].y_rot = 0.005;
    parts[left_leg].y_rot = -0.005;
    parts[right_leg].z_rot = 0.005;
    parts[left_leg].z_rot = -0.005;
    parts[right_arm].y_rot = 0.0;
    parts[left_arm].y_rot = 0.0;

    setup_attack_animation(parts, st, body, head, right_arm, left_arm);

    bob_model_part(&mut parts[right_arm], st.age_ticks, 1.0);
    bob_model_part(&mut parts[left_arm], st.age_ticks, -1.0);

    parts[head].visible = true;
    parts[right_arm].x_rot *= 0.5;
    parts[left_arm].x_rot *= 0.5;
    parts[right_leg].x_rot *= 0.5;
    parts[left_leg].x_rot *= 0.5;
    parts[right_arm].x_rot = parts[right_arm].x_rot.clamp(-0.4, 0.4);
    parts[left_arm].x_rot = parts[left_arm].x_rot.clamp(-0.4, 0.4);
    parts[right_leg].x_rot = parts[right_leg].x_rot.clamp(-0.4, 0.4);
    parts[left_leg].x_rot = parts[left_leg].x_rot.clamp(-0.4, 0.4);
    if st.extras.carried_block.is_some() {
        parts[right_arm].x_rot = -0.5;
        parts[left_arm].x_rot = -0.5;
        parts[right_arm].z_rot = 0.05;
        parts[left_arm].z_rot = -0.05;
    }
    if st.extras.creepy {
        parts[head].y -= 5.0;
        parts[hat].y += 5.0;
    }
}

fn setup_attack_animation(
    parts: &mut [PartState],
    st: &EntityState,
    body: usize,
    head: usize,
    right_arm: usize,
    left_arm: usize,
) {
    let attack_time = st.attack_time;
    if attack_time <= 0.0 {
        return;
    }
    let mut body_y_rot = (attack_time.sqrt() * 6.2831855).sin() * 0.2;
    if st.swing_left {
        body_y_rot *= -1.0;
    }
    parts[body].y_rot = body_y_rot;
    let age_scale = st.age_scale;
    parts[right_arm].z = body_y_rot.sin() * 5.0 * age_scale;
    parts[right_arm].x = -body_y_rot.cos() * 5.0 * age_scale;
    parts[left_arm].z = -body_y_rot.sin() * 5.0 * age_scale;
    parts[left_arm].x = body_y_rot.cos() * 5.0 * age_scale;
    parts[right_arm].y_rot += body_y_rot;
    parts[left_arm].y_rot += body_y_rot;
    parts[left_arm].x_rot += body_y_rot;

    let swing = 1.0 - (1.0 - attack_time).powi(4);
    let a = (swing * 3.1415927).sin();
    let b = (attack_time * 3.1415927).sin() * -(parts[head].x_rot - 0.7) * 0.75;
    let arm = if st.swing_left { left_arm } else { right_arm };
    parts[arm].x_rot -= a * 1.2 + b;
    parts[arm].y_rot += body_y_rot * 2.0;
    parts[arm].z_rot += (attack_time * 3.1415927).sin() * -0.4;
}
