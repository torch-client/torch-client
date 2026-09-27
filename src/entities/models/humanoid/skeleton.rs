use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use crate::entities::geom::{
    BakedModel, CubeList, Grow, LayerDef, MeshDef, PartDef, PartPose, PartState,
};
use crate::entities::state::EntityState;
use crate::renderer::anim::ArmPose;

use super::humanoid::{self, ArmPoses, Limbs};

fn default_skeleton_mesh(root: &mut PartDef) {
    root.child(
        "right_arm",
        CubeList::new()
            .tex_offs(40, 16)
            .add_box(-1.0, -2.0, -1.0, 2.0, 12.0, 2.0),
        PartPose::offset(-5.0, 2.0, 0.0),
    );
    root.child(
        "left_arm",
        CubeList::new()
            .tex_offs(40, 16)
            .mirror()
            .add_box(-1.0, -2.0, -1.0, 2.0, 12.0, 2.0),
        PartPose::offset(5.0, 2.0, 0.0),
    );
    root.child(
        "right_leg",
        CubeList::new()
            .tex_offs(0, 16)
            .add_box(-1.0, 0.0, -1.0, 2.0, 12.0, 2.0),
        PartPose::offset(-2.0, 12.0, 0.0),
    );
    root.child(
        "left_leg",
        CubeList::new()
            .tex_offs(0, 16)
            .mirror()
            .add_box(-1.0, 0.0, -1.0, 2.0, 12.0, 2.0),
        PartPose::offset(2.0, 12.0, 0.0),
    );
}

pub fn skeleton_layer() -> LayerDef {
    let mut mesh = humanoid::create_mesh(Grow::NONE, 0.0);
    default_skeleton_mesh(mesh.root());
    LayerDef::create(mesh, 64, 32)
}

pub fn parched_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(16, 16)
            .add_box(-4.0, 0.0, -2.0, 8.0, 12.0, 4.0)
            .tex_offs(28, 0)
            .add_box(-4.0, 10.0, -2.0, 8.0, 1.0, 4.0)
            .tex_offs(16, 48)
            .add_box_grow(-4.0, 0.0, -2.0, 8.0, 12.0, 4.0, Grow::all(0.025)),
        PartPose::offset(0.0, 0.0, 0.0),
    );
    root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -8.0, -4.0, 8.0, 8.0, 8.0)
            .tex_offs(0, 32)
            .add_box_grow(-4.0, -8.0, -4.0, 8.0, 8.0, 8.0, Grow::all(0.2)),
        PartPose::offset(0.0, 0.0, 0.0),
    )
    .child("hat", CubeList::new(), PartPose::ZERO);
    root.child(
        "right_arm",
        CubeList::new()
            .tex_offs(40, 16)
            .add_box(-1.0, -2.0, -1.0, 2.0, 12.0, 2.0)
            .tex_offs(42, 33)
            .add_box(-1.55, -2.025, -1.5, 3.0, 12.0, 3.0),
        PartPose::offset(-5.5, 2.0, 0.0),
    );
    root.child(
        "left_arm",
        CubeList::new()
            .tex_offs(56, 16)
            .add_box(-1.0, -2.0, -1.0, 2.0, 12.0, 2.0)
            .tex_offs(40, 48)
            .add_box(-1.45, -2.025, -1.5, 3.0, 12.0, 3.0),
        PartPose::offset(5.5, 2.0, 0.0),
    );
    root.child(
        "right_leg",
        CubeList::new()
            .tex_offs(0, 16)
            .add_box(-1.0, 0.0, -1.0, 2.0, 12.0, 2.0)
            .tex_offs(0, 49)
            .add_box(-1.5, -0.0, -1.5, 3.0, 12.0, 3.0),
        PartPose::offset(-2.0, 12.0, 0.0),
    );
    root.child(
        "left_leg",
        CubeList::new()
            .tex_offs(0, 16)
            .add_box(-1.0, 0.0, -1.0, 2.0, 12.0, 2.0)
            .tex_offs(4, 49)
            .add_box(-1.5, 0.0, -1.5, 3.0, 12.0, 3.0),
        PartPose::offset(2.0, 12.0, 0.0),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn stray_outer_layer() -> LayerDef {
    LayerDef::create(
        humanoid::create_mesh(Grow::all(humanoid::OVERLAY_SCALE), 0.0),
        64,
        32,
    )
}

pub fn bogged_outer_layer() -> LayerDef {
    LayerDef::create(humanoid::create_mesh(Grow::all(0.2), 0.0), 64, 32)
}

pub fn bogged_layer() -> LayerDef {
    let mut mesh = humanoid::create_mesh(Grow::NONE, 0.0);
    {
        let root = mesh.root();
        default_skeleton_mesh(root);
    }
    let head = mesh.root().get("head");
    let mushrooms = head.child("mushrooms", CubeList::new(), PartPose::ZERO);
    mushrooms.child(
        "red_mushroom_1",
        CubeList::new()
            .tex_offs(50, 16)
            .add_box(-3.0, -3.0, 0.0, 6.0, 4.0, 0.0),
        PartPose::offset_rotation(3.0, -8.0, 3.0, 0.0, FRAC_PI_4, 0.0),
    );
    mushrooms.child(
        "red_mushroom_2",
        CubeList::new()
            .tex_offs(50, 16)
            .add_box(-3.0, -3.0, 0.0, 6.0, 4.0, 0.0),
        PartPose::offset_rotation(3.0, -8.0, 3.0, 0.0, 2.356_194_5, 0.0),
    );
    mushrooms.child(
        "brown_mushroom_1",
        CubeList::new()
            .tex_offs(50, 22)
            .add_box(-3.0, -3.0, 0.0, 6.0, 4.0, 0.0),
        PartPose::offset_rotation(-3.0, -8.0, -3.0, 0.0, FRAC_PI_4, 0.0),
    );
    mushrooms.child(
        "brown_mushroom_2",
        CubeList::new()
            .tex_offs(50, 22)
            .add_box(-3.0, -3.0, 0.0, 6.0, 4.0, 0.0),
        PartPose::offset_rotation(-3.0, -8.0, -3.0, 0.0, 2.356_194_5, 0.0),
    );
    mushrooms.child(
        "brown_mushroom_3",
        CubeList::new()
            .tex_offs(50, 28)
            .add_box(-3.0, -4.0, 0.0, 6.0, 4.0, 0.0),
        PartPose::offset_rotation(-2.0, -1.0, 4.0, -FRAC_PI_2, 0.0, FRAC_PI_4),
    );
    mushrooms.child(
        "brown_mushroom_4",
        CubeList::new()
            .tex_offs(50, 28)
            .add_box(-3.0, -4.0, 0.0, 6.0, 4.0, 0.0),
        PartPose::offset_rotation(-2.0, -1.0, 4.0, -FRAC_PI_2, 0.0, 2.356_194_5),
    );
    LayerDef::create(mesh, 64, 32)
}

pub fn arm_poses(st: &EntityState) -> ArmPoses {
    let mut poses = humanoid::mob_arm_poses(st);
    if st.extras.aggressive && is_holding_bow(st) {
        if st.extras.left_handed {
            poses.left = ArmPose::BowAndArrow;
        } else {
            poses.right = ArmPose::BowAndArrow;
        }
    }
    poses
}

fn is_holding_bow(st: &EntityState) -> bool {
    st.extras.main_hand.id == "bow"
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let poses = arm_poses(st);
    let mut l = Limbs::load(model, parts);
    humanoid::pose_humanoid(&mut l, st, poses);
    if st.extras.aggressive && !is_holding_bow(st) {
        let attack_time = st.attack_time;
        let attack2 = (attack_time * PI).sin();
        let attack = ((1.0 - (1.0 - attack_time) * (1.0 - attack_time)) * PI).sin();
        l.right_arm.z_rot = 0.0;
        l.left_arm.z_rot = 0.0;
        l.right_arm.y_rot = -(0.1 - attack2 * 0.6);
        l.left_arm.y_rot = 0.1 - attack2 * 0.6;
        l.right_arm.x_rot = -FRAC_PI_2;
        l.left_arm.x_rot = -FRAC_PI_2;
        l.right_arm.x_rot -= attack2 * 1.2 - attack * 0.4;
        l.left_arm.x_rot -= attack2 * 1.2 - attack * 0.4;
        humanoid::bob_arms(&mut l, st.age_ticks);
    }
    l.store(parts);
}

pub fn bogged_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup_anim(model, parts, st);
    if let Some(mushrooms) = model.find("mushrooms") {
        parts[mushrooms].visible = !st.extras.sheared;
    }
}
