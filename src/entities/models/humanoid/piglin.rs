use crate::entities::geom::{
    BakedModel, CubeList, Grow, LayerDef, MeshDef, PartDef, PartPose, PartState,
};
use crate::entities::state::EntityState;

use super::humanoid::{self, DEG_TO_RAD, Limbs};

const ADULT_EAR_ANGLE_IN_DEGREES: f32 = 30.0;

const BABY_EAR_ANGLE_IN_DEGREES: f32 = 5.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PiglinArmPose {
    AttackingWithMeleeWeapon,
    CrossbowHold,
    CrossbowCharge,
    AdmiringItem,
    Dancing,
    Default,
}

fn add_head(g: Grow, mesh: &mut MeshDef) -> &mut PartDef {
    let head = mesh.root().child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box_grow(-5.0, -8.0, -4.0, 10.0, 8.0, 8.0, g)
            .tex_offs(31, 1)
            .add_box_grow(-2.0, -4.0, -5.0, 4.0, 4.0, 1.0, g)
            .tex_offs(2, 4)
            .add_box_grow(2.0, -2.0, -5.0, 1.0, 2.0, 1.0, g)
            .tex_offs(2, 0)
            .add_box_grow(-3.0, -2.0, -5.0, 1.0, 2.0, 1.0, g),
        PartPose::ZERO,
    );
    head.child(
        "left_ear",
        CubeList::new()
            .tex_offs(51, 6)
            .add_box_grow(0.0, 0.0, -2.0, 1.0, 5.0, 4.0, g),
        PartPose::offset_rotation(4.5, -6.0, 0.0, 0.0, 0.0, -0.523_598_8),
    );
    head.child(
        "right_ear",
        CubeList::new()
            .tex_offs(39, 6)
            .add_box_grow(-1.0, 0.0, -2.0, 1.0, 5.0, 4.0, g),
        PartPose::offset_rotation(-4.5, -6.0, 0.0, 0.0, 0.0, 0.523_598_8),
    );
    head
}

pub fn adult_piglin_layer() -> LayerDef {
    let mut mesh = humanoid::player_mesh(Grow::NONE, false);
    mesh.root().child(
        "body",
        CubeList::new()
            .tex_offs(16, 16)
            .add_box(-4.0, 0.0, -2.0, 8.0, 12.0, 4.0),
        PartPose::ZERO,
    );
    let head = add_head(Grow::NONE, &mut mesh);
    head.clear_child("hat");
    LayerDef::create(mesh, 64, 64)
}

pub fn baby_piglin_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 13)
            .add_box(-3.0, -3.0, -1.0, 6.0, 5.0, 3.0),
        PartPose::offset(0.0, 18.0, -0.5),
    );
    let head = root.child(
        "head",
        CubeList::new()
            .tex_offs(21, 30)
            .add_box(-1.5, -3.0, -4.5, 3.0, 3.0, 1.0)
            .tex_offs(0, 0)
            .add_box(-4.5, -6.0, -3.5, 9.0, 6.0, 7.0),
        PartPose::offset(0.0, 15.0, 0.0),
    );
    head.child("hat", CubeList::new(), PartPose::offset(0.0, 0.0, 0.0));
    head.child(
        "left_ear",
        CubeList::new(),
        PartPose::offset(4.2, -4.0, 0.0),
    )
    .child(
        "left_ear_r1",
        CubeList::new()
            .tex_offs(0, 21)
            .add_box(-0.5, -3.0, -2.0, 1.0, 6.0, 4.0),
        PartPose::offset_rotation(1.0, 1.75, 0.0, 0.0, 0.0, -0.610_9),
    );
    head.child(
        "right_ear",
        CubeList::new(),
        PartPose::offset(-4.2, -4.0, 0.0),
    )
    .child(
        "right_ear_r1",
        CubeList::new()
            .tex_offs(18, 13)
            .add_box(-0.5, -3.0, -2.0, 1.0, 6.0, 4.0),
        PartPose::offset_rotation(-1.0, 1.75, 0.0, 0.0, 0.0, 0.610_9),
    );
    root.child(
        "left_arm",
        CubeList::new()
            .tex_offs(28, 13)
            .add_box(-1.0, 0.0, -1.5, 2.0, 5.0, 3.0),
        PartPose::offset(4.0, 15.0, 0.0),
    );
    root.child(
        "right_arm",
        CubeList::new()
            .tex_offs(10, 30)
            .add_box(-1.0, 0.0, -1.5, 2.0, 5.0, 3.0),
        PartPose::offset(-4.0, 15.0, 0.0),
    );
    root.child(
        "right_leg",
        CubeList::new()
            .tex_offs(22, 23)
            .add_box(-1.5, 0.0, -1.5, 3.0, 4.0, 3.0),
        PartPose::offset(-1.5, 20.0, 0.0),
    );
    root.child(
        "left_leg",
        CubeList::new()
            .tex_offs(10, 23)
            .add_box(-1.5, 0.0, -1.5, 3.0, 4.0, 3.0),
        PartPose::offset(1.5, 20.0, 0.0),
    );
    LayerDef::create(mesh, 64, 64)
}

fn setup_ears(model: &BakedModel, parts: &mut [PartState], st: &EntityState, baby: bool) {
    let default_angle = if baby {
        BABY_EAR_ANGLE_IN_DEGREES
    } else {
        ADULT_EAR_ANGLE_IN_DEGREES
    } * DEG_TO_RAD;
    let frequency = st.age_ticks * 0.1 + st.walk_pos * 0.5;
    let amplitude = 0.08 + st.walk_speed * 0.4;
    if let Some(left_ear) = model.find("left_ear") {
        parts[left_ear].z_rot = -default_angle - (frequency * 1.2).cos() * amplitude;
    }
    if let Some(right_ear) = model.find("right_ear") {
        parts[right_ear].z_rot = default_angle + frequency.cos() * amplitude;
    }
}

pub fn arm_pose(st: &EntityState, brute: bool) -> PiglinArmPose {
    let melee = st.extras.aggressive && is_melee_weapon(st.extras.main_hand.id);
    if brute {
        return if melee {
            PiglinArmPose::AttackingWithMeleeWeapon
        } else {
            PiglinArmPose::Default
        };
    }
    if st.extras.dancing {
        PiglinArmPose::Dancing
    } else if st.extras.admiring {
        PiglinArmPose::AdmiringItem
    } else if melee {
        PiglinArmPose::AttackingWithMeleeWeapon
    } else if st.extras.charging_crossbow {
        PiglinArmPose::CrossbowCharge
    } else if st.extras.main_hand.id == "crossbow" && st.extras.main_hand.charged {
        PiglinArmPose::CrossbowHold
    } else {
        PiglinArmPose::Default
    }
}

fn is_melee_weapon(id: &str) -> bool {
    id.ends_with("_sword") || id.ends_with("_axe")
}

fn setup_attack(l: &mut Limbs, st: &EntityState, poses: humanoid::ArmPoses) {
    let brute = st.kind == azalea_registry::builtin::EntityKind::PiglinBrute;
    if st.attack_time > 0.0 && arm_pose(st, brute) == PiglinArmPose::AttackingWithMeleeWeapon {
        humanoid::swing_weapon_down(l, !st.extras.left_handed, st.attack_time, st.age_ticks);
    } else {
        humanoid::setup_attack_animation(l, st, poses);
    }
}

pub fn piglin_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let poses = humanoid::mob_arm_poses(st);
    let mut l = Limbs::load(model, parts);
    humanoid::pose_humanoid_with(&mut l, st, poses, setup_attack);
    l.store(parts);
    setup_ears(model, parts, st, st.extras.is_baby);

    let brute = st.kind == azalea_registry::builtin::EntityKind::PiglinBrute;
    let pose = arm_pose(st, brute);
    let right_arm_holds = !st.extras.left_handed;
    let mut l = Limbs::load(model, parts);
    let left_ear = model.find("left_ear");
    let right_ear = model.find("right_ear");
    match pose {
        PiglinArmPose::Dancing => {
            let dance_pos = st.age_ticks / 60.0;
            if let Some(right_ear) = right_ear {
                parts[right_ear].z_rot = 0.523_598_8 + DEG_TO_RAD * (dance_pos * 30.0).sin() * 10.0;
            }
            if let Some(left_ear) = left_ear {
                parts[left_ear].z_rot = -0.523_598_8 - DEG_TO_RAD * (dance_pos * 30.0).cos() * 10.0;
            }
            l.head.x += (dance_pos * 10.0).sin();
            l.head.y += (dance_pos * 40.0).sin() + 0.4;
            l.right_arm.z_rot = DEG_TO_RAD * (70.0 + (dance_pos * 40.0).cos() * 10.0);
            l.left_arm.z_rot = l.right_arm.z_rot * -1.0;
            l.right_arm.y += (dance_pos * 40.0).sin() * 0.5 - 0.5;
            l.left_arm.y += (dance_pos * 40.0).sin() * 0.5 + 0.5;
            l.body.y += (dance_pos * 40.0).sin() * 0.35;
        }
        PiglinArmPose::AttackingWithMeleeWeapon if st.attack_time == 0.0 => {
            l.arm(!right_arm_holds).x_rot = -1.8;
        }
        PiglinArmPose::CrossbowHold => humanoid::crossbow_hold(&mut l, right_arm_holds),
        PiglinArmPose::CrossbowCharge => humanoid::crossbow_charge(
            &mut l,
            right_arm_holds,
            st.extras.max_crossbow_charge,
            st.extras.ticks_using_item,
        ),
        PiglinArmPose::AdmiringItem => {
            l.head.x_rot = 0.5;
            l.head.y_rot = 0.0;
            if st.extras.left_handed {
                l.right_arm.y_rot = -0.5;
                l.right_arm.x_rot = -0.9;
            } else {
                l.left_arm.y_rot = 0.5;
                l.left_arm.x_rot = -0.9;
            }
        }
        _ => {}
    }
    l.store(parts);
}

pub fn zombified_piglin_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let poses = humanoid::mob_arm_poses(st);
    let mut l = Limbs::load(model, parts);
    humanoid::pose_humanoid(&mut l, st, poses);
    humanoid::animate_zombie_arms(&mut l, st, st.extras.aggressive);
    l.store(parts);
    setup_ears(model, parts, st, st.extras.is_baby);
}
