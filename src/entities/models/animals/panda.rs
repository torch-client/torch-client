use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;

use super::quadruped;
use crate::util::mth::lerp;
use crate::util::mth::rot_lerp_rad;

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 6)
            .add_box(-6.5, -5.0, -4.0, 13.0, 10.0, 9.0)
            .tex_offs(45, 16)
            .add_box(-3.5, 0.0, -6.0, 7.0, 5.0, 2.0)
            .tex_offs(52, 25)
            .add_box(3.5, -8.0, -1.0, 5.0, 4.0, 1.0)
            .tex_offs(52, 25)
            .add_box(-8.5, -8.0, -1.0, 5.0, 4.0, 1.0),
        PartPose::offset(0.0, 11.5, -17.0),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 25)
            .add_box(-9.5, -13.0, -6.5, 19.0, 26.0, 13.0),
        PartPose::offset_rotation(0.0, 10.0, 0.0, std::f32::consts::FRAC_PI_2, 0.0, 0.0),
    );
    let leg = CubeList::new()
        .tex_offs(40, 0)
        .add_box(-3.0, 0.0, -3.0, 6.0, 9.0, 6.0);
    root.child(
        "right_hind_leg",
        leg.clone(),
        PartPose::offset(-5.5, 15.0, 9.0),
    );
    root.child(
        "left_hind_leg",
        leg.clone(),
        PartPose::offset(5.5, 15.0, 9.0),
    );
    root.child(
        "right_front_leg",
        leg.clone(),
        PartPose::offset(-5.5, 15.0, -9.0),
    );
    root.child("left_front_leg", leg, PartPose::offset(5.5, 15.0, -9.0));
    LayerDef::create(mesh, 64, 64)
}

pub fn baby_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 11)
            .add_box(-4.5, -3.5, -5.5, 9.0, 7.0, 11.0),
        PartPose::offset(0.0, 18.5, 2.5),
    );
    root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-3.5, -3.0, -5.0, 7.0, 6.0, 5.0)
            .tex_offs(24, 6)
            .add_box(-2.0, 1.0, -6.0, 4.0, 2.0, 1.0)
            .tex_offs(24, 0)
            .add_box(-4.5, -4.0, -3.5, 3.0, 3.0, 1.0)
            .tex_offs(33, 0)
            .add_box(1.5, -4.0, -3.5, 3.0, 3.0, 1.0),
        PartPose::offset(0.0, 19.0, -3.0),
    );
    root.child(
        "right_hind_leg",
        CubeList::new()
            .tex_offs(0, 34)
            .add_box(-1.5, 0.0, -1.5, 3.0, 2.0, 3.0),
        PartPose::offset(-3.0, 22.0, 6.5),
    );
    root.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(12, 34)
            .add_box(-1.5, 0.0, -1.5, 3.0, 2.0, 3.0),
        PartPose::offset(3.0, 22.0, 6.5),
    );
    root.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(0, 29)
            .add_box(-1.5, 0.0, -1.5, 3.0, 2.0, 3.0),
        PartPose::offset(-3.0, 22.0, -1.5),
    );
    root.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(12, 29)
            .add_box(-1.5, 0.0, -1.5, 3.0, 2.0, 3.0),
        PartPose::offset(3.0, 22.0, -1.5),
    );
    LayerDef::create(mesh, 64, 64)
}

struct Ids {
    head: usize,
    body: usize,
    right_hind: usize,
    left_hind: usize,
    right_front: usize,
    left_front: usize,
}

impl Ids {
    fn new(model: &BakedModel) -> Ids {
        Ids {
            head: model.id("head"),
            body: model.id("body"),
            right_hind: model.id("right_hind_leg"),
            left_hind: model.id("left_hind_leg"),
            right_front: model.id("right_front_leg"),
            left_front: model.id("left_front_leg"),
        }
    }
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(model, parts, st, false);
}

pub fn baby_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(model, parts, st, true);
}

fn setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState, baby: bool) {
    quadruped::setup_anim(model, parts, st);
    let id = Ids::new(model);
    let e = &st.extras;
    let age = st.age_ticks;

    if e.unhappy_counter > 0 {
        parts[id.head].y_rot = 0.35 * (0.6 * age).sin();
        parts[id.head].z_rot = 0.35 * (0.6 * age).sin();
        parts[id.right_front].x_rot = -0.75 * (0.3 * age).sin();
        parts[id.left_front].x_rot = 0.75 * (0.3 * age).sin();
    } else {
        parts[id.head].z_rot = 0.0;
    }

    if e.sneezing {
        if e.sneeze_time < 15 {
            parts[id.head].x_rot = -0.7853982 * e.sneeze_time as f32 / 14.0;
        } else if e.sneeze_time < 20 {
            let internal = ((e.sneeze_time - 15) / 5) as f32;
            parts[id.head].x_rot = -0.7853982 + 0.7853982 * internal;
        }
    }

    if e.sit_amount > 0.0 {
        if baby {
            animate_sitting_baby(parts, &id, e.sit_amount);
        } else {
            animate_sitting(parts, &id, e.sit_amount);
        }
        if e.eating {
            parts[id.head].x_rot = std::f32::consts::FRAC_PI_2 + 0.2 * (age * 0.6).sin();
            parts[id.right_front].x_rot = -0.4 - 0.2 * (age * 0.6).sin();
            parts[id.left_front].x_rot = -0.4 - 0.2 * (age * 0.6).sin();
        }
        if e.scared {
            parts[id.head].x_rot = 2.1707964;
            parts[id.right_front].x_rot = -0.9;
            parts[id.left_front].x_rot = -0.9;
        }
    } else {
        parts[id.right_hind].z_rot = 0.0;
        parts[id.left_hind].z_rot = 0.0;
        parts[id.right_front].z_rot = 0.0;
        parts[id.left_front].z_rot = 0.0;
    }

    if e.lie_on_back_amount > 0.0 {
        parts[id.right_hind].x_rot = -0.6 * (age * 0.15).sin();
        parts[id.left_hind].x_rot = 0.6 * (age * 0.15).sin();
        parts[id.right_front].x_rot = 0.3 * (age * 0.25).sin();
        parts[id.left_front].x_rot = -0.3 * (age * 0.25).sin();
        parts[id.head].x_rot = rot_lerp_rad(
            e.lie_on_back_amount,
            parts[id.head].x_rot,
            std::f32::consts::FRAC_PI_2,
        );
    }

    if e.roll_amount > 0.0 {
        parts[id.head].x_rot = rot_lerp_rad(e.roll_amount, parts[id.head].x_rot, 2.0561945);
        parts[id.right_hind].x_rot = -0.5 * (age * 0.5).sin();
        parts[id.left_hind].x_rot = 0.5 * (age * 0.5).sin();
        parts[id.right_front].x_rot = 0.5 * (age * 0.5).sin();
        parts[id.left_front].x_rot = -0.5 * (age * 0.5).sin();
    }
}

fn animate_sitting(parts: &mut [PartState], id: &Ids, sit: f32) {
    parts[id.body].x_rot = rot_lerp_rad(sit, parts[id.body].x_rot, 1.7407963);
    parts[id.head].x_rot = rot_lerp_rad(sit, parts[id.head].x_rot, std::f32::consts::FRAC_PI_2);
    parts[id.right_front].z_rot = -0.27079642;
    parts[id.left_front].z_rot = 0.27079642;
    parts[id.right_hind].z_rot = 0.5707964;
    parts[id.left_hind].z_rot = -0.5707964;
}

fn animate_sitting_baby(parts: &mut [PartState], id: &Ids, sit: f32) {
    parts[id.body].x_rot = rot_lerp_rad(sit, parts[id.body].x_rot, 0.17453292);
    parts[id.body].z = lerp(sit, parts[id.body].z, -1.5);
    parts[id.head].z = lerp(sit, parts[id.head].z, -11.5);
    parts[id.head].y = lerp(sit, parts[id.head].y, 17.5);
    parts[id.right_front].z = lerp(sit, parts[id.right_front].z, -5.0);
    parts[id.left_front].z = lerp(sit, parts[id.left_front].z, -5.0);
    parts[id.right_hind].z = lerp(sit, parts[id.right_hind].z, 3.0);
    parts[id.left_hind].z = lerp(sit, parts[id.left_hind].z, 3.0);
    parts[id.right_front].z_rot = -0.27079642;
    parts[id.left_front].z_rot = 0.27079642;
    parts[id.right_hind].z_rot = 0.5707964;
    parts[id.left_hind].z_rot = -0.5707964;
}
