#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::models::aquatic::DEG_TO_RAD;
use crate::entities::state::EntityState;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ParrotPose {
    Flying = 0,
    Standing = 1,
    Sitting = 2,
    Party = 3,
    OnShoulder = 4,
}

impl ParrotPose {
    pub fn of(st: &EntityState) -> ParrotPose {
        match st.extras.parrot_pose {
            0 => ParrotPose::Flying,
            2 => ParrotPose::Sitting,
            3 => ParrotPose::Party,
            4 => ParrotPose::OnShoulder,
            _ => ParrotPose::Standing,
        }
    }
}

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(2, 8)
            .add_box(-1.5, 0.0, -1.5, 3.0, 6.0, 3.0),
        PartPose::offset_rotation(0.0, 16.5, -3.0, 0.4937, 0.0, 0.0),
    );
    root.child(
        "tail",
        CubeList::new()
            .tex_offs(22, 1)
            .add_box(-1.5, -1.0, -1.0, 3.0, 4.0, 1.0),
        PartPose::offset_rotation(0.0, 21.07, 1.16, 1.015, 0.0, 0.0),
    );
    root.child(
        "left_wing",
        CubeList::new()
            .tex_offs(19, 8)
            .add_box(-0.5, 0.0, -1.5, 1.0, 5.0, 3.0),
        PartPose::offset_rotation(1.5, 16.94, -2.76, -0.6981, -std::f32::consts::PI, 0.0),
    );
    root.child(
        "right_wing",
        CubeList::new()
            .tex_offs(19, 8)
            .add_box(-0.5, 0.0, -1.5, 1.0, 5.0, 3.0),
        PartPose::offset_rotation(-1.5, 16.94, -2.76, -0.6981, -std::f32::consts::PI, 0.0),
    );
    let head = root.child(
        "head",
        CubeList::new()
            .tex_offs(2, 2)
            .add_box(-1.0, -1.5, -1.0, 2.0, 3.0, 2.0),
        PartPose::offset(0.0, 15.69, -2.76),
    );
    head.child(
        "head2",
        CubeList::new()
            .tex_offs(10, 0)
            .add_box(-1.0, -0.5, -2.0, 2.0, 1.0, 4.0),
        PartPose::offset(0.0, -2.0, -1.0),
    );
    head.child(
        "beak1",
        CubeList::new()
            .tex_offs(11, 7)
            .add_box(-0.5, -1.0, -0.5, 1.0, 2.0, 1.0),
        PartPose::offset(0.0, -0.5, -1.5),
    );
    head.child(
        "beak2",
        CubeList::new()
            .tex_offs(16, 7)
            .add_box(-0.5, 0.0, -0.5, 1.0, 2.0, 1.0),
        PartPose::offset(0.0, -1.75, -2.45),
    );
    head.child(
        "feather",
        CubeList::new()
            .tex_offs(2, 18)
            .add_box(0.0, -4.0, -2.0, 0.0, 5.0, 4.0),
        PartPose::offset_rotation(0.0, -2.15, 0.15, -0.2214, 0.0, 0.0),
    );
    let leg = CubeList::new()
        .tex_offs(14, 18)
        .add_box(-0.5, 0.0, -0.5, 1.0, 2.0, 1.0);
    root.child(
        "left_leg",
        leg.clone(),
        PartPose::offset_rotation(1.0, 22.0, -1.05, -0.0299, 0.0, 0.0),
    );
    root.child(
        "right_leg",
        leg,
        PartPose::offset_rotation(-1.0, 22.0, -1.05, -0.0299, 0.0, 0.0),
    );
    LayerDef::create(mesh, 32, 32)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let pose = ParrotPose::of(st);
    let body = model.id("body");
    let tail = model.id("tail");
    let head = model.id("head");
    let left_wing = model.id("left_wing");
    let right_wing = model.id("right_wing");
    let left_leg = model.id("left_leg");
    let right_leg = model.id("right_leg");
    let flap = st.extras.flap_angle;

    match pose {
        ParrotPose::Flying => {
            parts[left_leg].x_rot += 0.698_131_7;
            parts[right_leg].x_rot += 0.698_131_7;
        }
        ParrotPose::Sitting => {
            parts[head].y += 1.0;
            parts[tail].x_rot += 0.523_598_8;
            parts[tail].y += 1.0;
            parts[body].y += 1.0;
            parts[left_wing].z_rot = -0.0873;
            parts[left_wing].y += 1.0;
            parts[right_wing].z_rot = 0.0873;
            parts[right_wing].y += 1.0;
            parts[left_leg].y += 1.0;
            parts[right_leg].y += 1.0;
            parts[left_leg].x_rot += 1.0;
            parts[right_leg].x_rot += 1.0;
        }
        ParrotPose::Party => {
            parts[left_leg].z_rot = -0.349_065_84;
            parts[right_leg].z_rot = 0.349_065_84;
        }
        ParrotPose::Standing | ParrotPose::OnShoulder => {}
    }

    parts[head].x_rot = st.x_rot * DEG_TO_RAD;
    parts[head].y_rot = st.y_rot * DEG_TO_RAD;

    match pose {
        ParrotPose::Sitting => {}
        ParrotPose::Party => {
            let x_pos = st.age_ticks.cos();
            let y_pos = st.age_ticks.sin();
            parts[head].x += x_pos;
            parts[head].y += y_pos;
            parts[head].x_rot = 0.0;
            parts[head].y_rot = 0.0;
            parts[head].z_rot = st.age_ticks.sin() * 0.4;
            parts[body].x += x_pos;
            parts[body].y += y_pos;
            parts[left_wing].z_rot = -0.0873 - flap;
            parts[left_wing].x += x_pos;
            parts[left_wing].y += y_pos;
            parts[right_wing].z_rot = 0.0873 + flap;
            parts[right_wing].x += x_pos;
            parts[right_wing].y += y_pos;
            parts[tail].x += x_pos;
            parts[tail].y += y_pos;
        }
        pose => {
            if pose == ParrotPose::Standing {
                parts[left_leg].x_rot += (st.walk_pos * 0.6662).cos() * 1.4 * st.walk_speed;
                parts[right_leg].x_rot +=
                    (st.walk_pos * 0.6662 + std::f32::consts::PI).cos() * 1.4 * st.walk_speed;
            }
            let bobbing_body = flap * 0.3;
            parts[head].y += bobbing_body;
            parts[tail].x_rot += (st.walk_pos * 0.6662).cos() * 0.3 * st.walk_speed;
            parts[tail].y += bobbing_body;
            parts[body].y += bobbing_body;
            parts[left_wing].z_rot = -0.0873 - flap;
            parts[left_wing].y += bobbing_body;
            parts[right_wing].z_rot = 0.0873 + flap;
            parts[right_wing].y += bobbing_body;
            parts[left_leg].y += bobbing_body;
            parts[right_leg].y += bobbing_body;
        }
    }
}
