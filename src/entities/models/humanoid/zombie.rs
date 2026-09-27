use std::f32::consts::FRAC_PI_2;

use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;
use crate::renderer::anim::ArmPose;

use super::humanoid::{self, ArmPoses, Limbs, rot_lerp_rad};

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let poses = arm_poses(st);
    let mut l = Limbs::load(model, parts);
    humanoid::pose_humanoid(&mut l, st, poses);
    humanoid::animate_zombie_arms(&mut l, st, st.extras.aggressive);
    l.store(parts);
}

pub fn arm_poses(st: &EntityState) -> ArmPoses {
    humanoid::mob_arm_poses(st)
}

pub fn zombie_layer() -> LayerDef {
    humanoid::humanoid_layer()
}

pub fn husk_layer() -> LayerDef {
    LayerDef::create(
        humanoid::scaling(humanoid::create_mesh(Grow::NONE, 0.0), 1.0625),
        64,
        64,
    )
}

pub fn giant_layer() -> LayerDef {
    LayerDef::create(
        humanoid::scaling(humanoid::create_mesh(Grow::NONE, 0.0), 6.0),
        64,
        64,
    )
}

pub fn baby_zombie_layer_grow(g: Grow) -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(16, 16)
            .add_box_grow(-2.0, -2.5, -1.0, 4.0, 5.0, 2.0, g),
        PartPose::offset(0.0, 17.5, 0.0),
    );
    let head = root.child(
        "head",
        CubeList::new()
            .tex_offs(3, 3)
            .add_box_grow(-3.0, -6.25, -3.0, 6.0, 6.0, 6.0, Grow::all(0.0))
            .tex_offs(35, 3)
            .add_box_grow(-3.0, -6.15, -3.0, 6.0, 6.0, 6.0, Grow::all(0.25)),
        PartPose::offset(0.0, 15.25, 0.0),
    );
    head.child("hat", CubeList::new(), PartPose::ZERO);
    root.child(
        "right_arm",
        CubeList::new()
            .tex_offs(36, 16)
            .add_box_grow(-1.0, -0.5, -1.0, 2.0, 5.0, 2.0, g),
        PartPose::offset(-3.0, 15.5, 0.0),
    );
    root.child(
        "left_arm",
        CubeList::new()
            .tex_offs(28, 16)
            .add_box_grow(-1.0, -0.5, -1.0, 2.0, 5.0, 2.0, g),
        PartPose::offset(3.0, 15.5, 0.0),
    );
    root.child(
        "right_leg",
        CubeList::new()
            .tex_offs(8, 16)
            .add_box_grow(-1.0, 0.0, -1.0, 2.0, 4.0, 2.0, g),
        PartPose::offset(-1.0, 20.0, 0.0),
    );
    root.child(
        "left_leg",
        CubeList::new()
            .tex_offs(0, 16)
            .add_box_grow(-1.0, 0.0, -1.0, 2.0, 4.0, 2.0, g),
        PartPose::offset(1.0, 20.0, 0.0),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn baby_zombie_layer() -> LayerDef {
    baby_zombie_layer_grow(Grow::NONE)
}

pub fn drowned_layer_grow(g: Grow) -> LayerDef {
    let mut mesh = humanoid::create_mesh(g, 0.0);
    let root = mesh.root();
    root.child(
        "left_arm",
        CubeList::new()
            .tex_offs(32, 48)
            .add_box_grow(-1.0, -2.0, -2.0, 4.0, 12.0, 4.0, g),
        PartPose::offset(5.0, 2.0, 0.0),
    );
    root.child(
        "left_leg",
        CubeList::new()
            .tex_offs(16, 48)
            .add_box_grow(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0, g),
        PartPose::offset(1.9, 12.0, 0.0),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn drowned_layer() -> LayerDef {
    drowned_layer_grow(Grow::NONE)
}

pub fn drowned_outer_layer() -> LayerDef {
    drowned_layer_grow(Grow::all(humanoid::OVERLAY_SCALE))
}

pub fn baby_drowned_layer() -> LayerDef {
    baby_zombie_layer_grow(Grow::NONE)
}

pub fn baby_drowned_outer_layer() -> LayerDef {
    baby_zombie_layer_grow(Grow::all(humanoid::OVERLAY_SCALE))
}

pub fn drowned_arm_poses(st: &EntityState) -> ArmPoses {
    let mut poses = humanoid::mob_arm_poses(st);
    if st.extras.aggressive && st.extras.main_hand.id == "trident" {
        if st.extras.left_handed {
            poses.left = ArmPose::ThrowTrident;
        } else {
            poses.right = ArmPose::ThrowTrident;
        }
    }
    poses
}

pub fn drowned_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let poses = drowned_arm_poses(st);
    let mut l = Limbs::load(model, parts);
    humanoid::pose_humanoid(&mut l, st, poses);
    humanoid::animate_zombie_arms(&mut l, st, st.extras.aggressive);

    if poses.left == ArmPose::ThrowTrident {
        l.left_arm.x_rot = l.left_arm.x_rot * 0.5 - std::f32::consts::PI;
        l.left_arm.y_rot = 0.0;
    }
    if poses.right == ArmPose::ThrowTrident {
        l.right_arm.x_rot = l.right_arm.x_rot * 0.5 - std::f32::consts::PI;
        l.right_arm.y_rot = 0.0;
    }

    let swim = st.extras.swim_amount;
    if swim > 0.0 {
        let flail = (0.1 * st.age_ticks).sin();
        l.right_arm.x_rot =
            rot_lerp_rad(swim, l.right_arm.x_rot, -2.513_274_2) + swim * 0.35 * flail;
        l.left_arm.x_rot = rot_lerp_rad(swim, l.left_arm.x_rot, -2.513_274_2) - swim * 0.35 * flail;
        l.right_arm.z_rot = rot_lerp_rad(swim, l.right_arm.z_rot, -0.15);
        l.left_arm.z_rot = rot_lerp_rad(swim, l.left_arm.z_rot, 0.15);
        l.left_leg.x_rot -= swim * 0.55 * flail;
        l.right_leg.x_rot += swim * 0.55 * flail;
        l.head.x_rot = 0.0;
    }
    l.store(parts);
}

pub fn zombie_villager_layer() -> LayerDef {
    let mut mesh = humanoid::create_mesh(Grow::NONE, 0.0);
    let root = mesh.root();
    let head = root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -10.0, -4.0, 8.0, 10.0, 8.0)
            .tex_offs(24, 0)
            .add_box(-1.0, -3.0, -6.0, 2.0, 4.0, 2.0),
        PartPose::ZERO,
    );
    let hat = head.child(
        "hat",
        CubeList::new().tex_offs(32, 0).add_box_grow(
            -4.0,
            -10.0,
            -4.0,
            8.0,
            10.0,
            8.0,
            Grow::all(0.5),
        ),
        PartPose::ZERO,
    );
    hat.child(
        "hat_rim",
        CubeList::new()
            .tex_offs(30, 47)
            .add_box(-8.0, -8.0, -6.0, 16.0, 16.0, 1.0),
        PartPose::rotation(-FRAC_PI_2, 0.0, 0.0),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(16, 20)
            .add_box(-4.0, 0.0, -3.0, 8.0, 12.0, 6.0)
            .tex_offs(0, 38)
            .add_box_grow(-4.0, 0.0, -3.0, 8.0, 20.0, 6.0, Grow::all(0.05)),
        PartPose::ZERO,
    );
    root.child(
        "right_arm",
        CubeList::new()
            .tex_offs(44, 22)
            .add_box(-3.0, -2.0, -2.0, 4.0, 12.0, 4.0),
        PartPose::offset(-5.0, 2.0, 0.0),
    );
    root.child(
        "left_arm",
        CubeList::new()
            .tex_offs(44, 22)
            .mirror()
            .add_box(-1.0, -2.0, -2.0, 4.0, 12.0, 4.0),
        PartPose::offset(5.0, 2.0, 0.0),
    );
    root.child(
        "right_leg",
        CubeList::new()
            .tex_offs(0, 22)
            .add_box(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0),
        PartPose::offset(-2.0, 12.0, 0.0),
    );
    root.child(
        "left_leg",
        CubeList::new()
            .tex_offs(0, 22)
            .mirror()
            .add_box(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0),
        PartPose::offset(2.0, 12.0, 0.0),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn zombie_villager_no_hat_layer() -> LayerDef {
    let mut layer = zombie_villager_layer();
    humanoid::clear_cubes(layer.mesh.root(), &["head", "head/hat", "head/hat/hat_rim"]);
    layer
}

pub fn baby_zombie_villager_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 15)
            .add_box(-2.0, -2.75, -1.5, 4.0, 5.0, 3.0)
            .tex_offs(16, 22)
            .add_box_grow(-2.0, -2.75, -1.5, 4.0, 6.0, 3.0, Grow::all(0.1)),
        PartPose::offset(0.0, 18.75, 0.0),
    );
    let head = root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -8.0, -3.5, 8.0, 8.0, 7.0),
        PartPose::offset(0.0, 16.0, 0.0),
    );
    head.child(
        "hat",
        CubeList::new().tex_offs(0, 31).add_box_grow(
            -4.0,
            -4.0,
            -3.5,
            8.0,
            8.0,
            7.0,
            Grow::all(0.3),
        ),
        PartPose::offset(0.0, -4.0, 0.0),
    );
    head.child(
        "hat_rim",
        CubeList::new()
            .tex_offs(0, 46)
            .add_box(-7.0, -0.5, -6.0, 14.0, 1.0, 12.0),
        PartPose::offset(0.0, -4.5, 0.0),
    );
    head.child(
        "nose",
        CubeList::new()
            .tex_offs(23, 0)
            .add_box(-1.0, -1.0, -0.5, 2.0, 2.0, 1.0),
        PartPose::offset(0.0, -1.0, -4.0),
    );
    root.child(
        "right_arm",
        CubeList::new()
            .tex_offs(24, 15)
            .add_box(-1.0, -0.5, -1.0, 2.0, 5.0, 2.0),
        PartPose::offset(-3.0, 15.5, 0.0),
    );
    root.child(
        "left_arm",
        CubeList::new()
            .tex_offs(16, 15)
            .add_box(-1.0, -0.5, -1.0, 2.0, 5.0, 2.0),
        PartPose::offset(3.0, 15.5, 0.0),
    );
    root.child(
        "right_leg",
        CubeList::new()
            .tex_offs(8, 23)
            .add_box(-1.0, -0.5, -1.0, 2.0, 3.0, 2.0),
        PartPose::offset(-1.0, 21.5, 0.0),
    );
    root.child(
        "left_leg",
        CubeList::new()
            .tex_offs(0, 23)
            .add_box(-1.0, -0.5, -1.0, 2.0, 3.0, 2.0),
        PartPose::offset(1.0, 21.5, 0.0),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn baby_zombie_villager_no_hat_layer() -> LayerDef {
    let mut layer = baby_zombie_villager_layer();
    humanoid::clear_cubes(
        layer.mesh.root(),
        &["head", "head/hat", "head/hat_rim", "head/nose"],
    );
    layer
}
