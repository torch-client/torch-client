use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

use crate::entities::geom::{
    BakedModel, CubeList, Grow, LayerDef, MeshDef, PartDef, PartId, PartPose, PartState,
};
use crate::entities::state::EntityState;
use crate::renderer::anim::{ArmPose, HeldItem, UseAnimation, use_animation};

use crate::util::mth::ease::{in_out_expo, in_out_sine, in_quad, out_quart};
pub use crate::util::mth::rot_lerp_rad;
pub use crate::util::mth::{DEG_TO_RAD, RAD_TO_DEG};
use crate::util::mth::{lerp, progress};

pub const OVERLAY_SCALE: f32 = 0.25;

pub const HAT_OVERLAY_SCALE: f32 = 0.5;

pub const LEGGINGS_OVERLAY_SCALE: f32 = -0.1;

const DUCK_WALK_ROTATION: f32 = 0.005;

const SPYGLASS_ARM_ROT_Y: f32 = 0.261_799_4;
const SPYGLASS_ARM_ROT_X: f32 = 1.919_862_2;
const SPYGLASS_ARM_CROUCH_ROT_X: f32 = 0.261_799_4;

const HIGHEST_SHIELD_BLOCKING_ANGLE: f32 = -1.396_263_4;
const LOWEST_SHIELD_BLOCKING_ANGLE: f32 = 0.436_332_32;

const HORIZONTAL_SHIELD_MOVEMENT_LIMIT: f32 = 0.523_598_8;

const TOOT_HORN_XROT_BASE: f32 = 1.483_529_8;
const TOOT_HORN_YROT_BASE: f32 = 0.523_598_8;

pub fn create_mesh(g: Grow, y_offset: f32) -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let head = root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box_grow(-4.0, -8.0, -4.0, 8.0, 8.0, 8.0, g),
        PartPose::offset(0.0, 0.0 + y_offset, 0.0),
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
            g.extend(HAT_OVERLAY_SCALE),
        ),
        PartPose::ZERO,
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(16, 16)
            .add_box_grow(-4.0, 0.0, -2.0, 8.0, 12.0, 4.0, g),
        PartPose::offset(0.0, 0.0 + y_offset, 0.0),
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

pub fn player_mesh(g: Grow, slim: bool) -> MeshDef {
    let mut mesh = create_mesh(g, 0.0);
    let root = mesh.root();
    if slim {
        let left_arm = root.child(
            "left_arm",
            CubeList::new()
                .tex_offs(32, 48)
                .add_box_grow(-1.0, -2.0, -2.0, 3.0, 12.0, 4.0, g),
            PartPose::offset(5.0, 2.0, 0.0),
        );
        left_arm.child(
            "left_sleeve",
            CubeList::new().tex_offs(48, 48).add_box_grow(
                -1.0,
                -2.0,
                -2.0,
                3.0,
                12.0,
                4.0,
                g.extend(OVERLAY_SCALE),
            ),
            PartPose::ZERO,
        );
        let right_arm = root.child(
            "right_arm",
            CubeList::new()
                .tex_offs(40, 16)
                .add_box_grow(-2.0, -2.0, -2.0, 3.0, 12.0, 4.0, g),
            PartPose::offset(-5.0, 2.0, 0.0),
        );
        right_arm.child(
            "right_sleeve",
            CubeList::new().tex_offs(40, 32).add_box_grow(
                -2.0,
                -2.0,
                -2.0,
                3.0,
                12.0,
                4.0,
                g.extend(OVERLAY_SCALE),
            ),
            PartPose::ZERO,
        );
    } else {
        let left_arm = root.child(
            "left_arm",
            CubeList::new()
                .tex_offs(32, 48)
                .add_box_grow(-1.0, -2.0, -2.0, 4.0, 12.0, 4.0, g),
            PartPose::offset(5.0, 2.0, 0.0),
        );
        left_arm.child(
            "left_sleeve",
            CubeList::new().tex_offs(48, 48).add_box_grow(
                -1.0,
                -2.0,
                -2.0,
                4.0,
                12.0,
                4.0,
                g.extend(OVERLAY_SCALE),
            ),
            PartPose::ZERO,
        );
        root.get("right_arm").child(
            "right_sleeve",
            CubeList::new().tex_offs(40, 32).add_box_grow(
                -3.0,
                -2.0,
                -2.0,
                4.0,
                12.0,
                4.0,
                g.extend(OVERLAY_SCALE),
            ),
            PartPose::ZERO,
        );
    }
    let left_leg = root.child(
        "left_leg",
        CubeList::new()
            .tex_offs(16, 48)
            .add_box_grow(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0, g),
        PartPose::offset(1.9, 12.0, 0.0),
    );
    left_leg.child(
        "left_pants",
        CubeList::new().tex_offs(0, 48).add_box_grow(
            -2.0,
            0.0,
            -2.0,
            4.0,
            12.0,
            4.0,
            g.extend(OVERLAY_SCALE),
        ),
        PartPose::ZERO,
    );
    root.get("right_leg").child(
        "right_pants",
        CubeList::new().tex_offs(0, 32).add_box_grow(
            -2.0,
            0.0,
            -2.0,
            4.0,
            12.0,
            4.0,
            g.extend(OVERLAY_SCALE),
        ),
        PartPose::ZERO,
    );
    root.get("body").child(
        "jacket",
        CubeList::new().tex_offs(16, 32).add_box_grow(
            -4.0,
            0.0,
            -2.0,
            8.0,
            12.0,
            4.0,
            g.extend(OVERLAY_SCALE),
        ),
        PartPose::ZERO,
    );
    mesh
}

pub fn scaling(mesh: MeshDef, factor: f32) -> MeshDef {
    let y_offset = 24.016 * (1.0 - factor);
    mesh.transformed(|pose| pose.scaled(factor).translated(0.0, y_offset, 0.0))
}

#[derive(Clone, Copy, Debug)]
pub struct BabyTransform {
    pub scale_head: bool,
    pub baby_y_head_offset: f32,
    pub baby_z_head_offset: f32,
    pub baby_head_scale: f32,
    pub baby_body_scale: f32,
    pub body_y_offset: f32,
}

impl BabyTransform {
    pub const HUMANOID: BabyTransform = BabyTransform {
        scale_head: true,
        baby_y_head_offset: 16.0,
        baby_z_head_offset: 0.0,
        baby_head_scale: 2.0,
        baby_body_scale: 2.0,
        body_y_offset: 24.0,
    };
}

pub fn baby_transform(
    mesh: MeshDef,
    t: BabyTransform,
    parts: &[&str],
    head_parts: &[&str],
) -> MeshDef {
    let head_scale = if t.scale_head {
        1.5 / t.baby_head_scale
    } else {
        1.0
    };
    let body_scale = 1.0 / t.baby_body_scale;
    let mut mesh = mesh;
    let root = mesh.root();
    for name in parts {
        let is_head = head_parts.contains(name);
        root.get(name).transformed(|pose| {
            if is_head {
                pose.translated(0.0, t.baby_y_head_offset, t.baby_z_head_offset)
                    .scaled(head_scale)
            } else {
                pose.translated(0.0, t.body_y_offset, 0.0)
                    .scaled(body_scale)
            }
        });
    }
    mesh
}

pub fn humanoid_layer() -> LayerDef {
    LayerDef::create(create_mesh(Grow::NONE, 0.0), 64, 64)
}

pub fn clear_cubes(root: &mut PartDef, paths: &[&str]) {
    for path in paths {
        match path.rsplit_once('/') {
            Some((parent, name)) => {
                root.get(parent).clear_child(name);
            }
            None => {
                root.clear_child(path);
            }
        }
    }
}

pub struct Limbs {
    pub head: PartState,
    pub body: PartState,
    pub right_arm: PartState,
    pub left_arm: PartState,
    pub right_leg: PartState,
    pub left_leg: PartState,
    ids: [PartId; 6],
}

impl Limbs {
    pub fn load(model: &BakedModel, parts: &[PartState]) -> Limbs {
        let ids = [
            model.id("head"),
            model.id("body"),
            model.id("right_arm"),
            model.id("left_arm"),
            model.id("right_leg"),
            model.id("left_leg"),
        ];
        Limbs {
            head: parts[ids[0]],
            body: parts[ids[1]],
            right_arm: parts[ids[2]],
            left_arm: parts[ids[3]],
            right_leg: parts[ids[4]],
            left_leg: parts[ids[5]],
            ids,
        }
    }

    pub fn store(&self, parts: &mut [PartState]) {
        parts[self.ids[0]] = self.head;
        parts[self.ids[1]] = self.body;
        parts[self.ids[2]] = self.right_arm;
        parts[self.ids[3]] = self.left_arm;
        parts[self.ids[4]] = self.right_leg;
        parts[self.ids[5]] = self.left_leg;
    }

    pub fn arm(&mut self, left: bool) -> &mut PartState {
        if left {
            &mut self.left_arm
        } else {
            &mut self.right_arm
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ArmPoses {
    pub right: ArmPose,
    pub left: ArmPose,
}

pub fn mob_arm_pose(item: &HeldItem) -> ArmPose {
    if use_animation(&item.id) == UseAnimation::Spear {
        ArmPose::Spear
    } else {
        ArmPose::Empty
    }
}

pub fn mob_arm_poses(st: &EntityState) -> ArmPoses {
    let main = mob_arm_pose(&st.extras.main_hand);
    let off = mob_arm_pose(&st.extras.off_hand);
    if st.extras.left_handed {
        ArmPoses {
            right: off,
            left: main,
        }
    } else {
        ArmPoses {
            right: main,
            left: off,
        }
    }
}

fn quadratic_arm_update(x: f32) -> f32 {
    -65.0 * x + x * x
}

pub fn bob_model_part(part: &mut PartState, age_ticks: f32, scale: f32) {
    part.z_rot += scale * ((age_ticks * 0.09).cos() * 0.05 + 0.05);
    part.x_rot += scale * (age_ticks * 0.067).sin() * 0.05;
}

pub fn bob_arms(l: &mut Limbs, age_ticks: f32) {
    bob_model_part(&mut l.right_arm, age_ticks, 1.0);
    bob_model_part(&mut l.left_arm, age_ticks, -1.0);
}

pub fn animate_crossbow_charge(
    holding: &mut PartState,
    pulling: &mut PartState,
    max_charge_duration: f32,
    ticks_using_item: f32,
    holding_in_right_arm: bool,
) {
    let sign = if holding_in_right_arm { 1.0 } else { -1.0 };
    holding.y_rot = -0.8 * sign;
    holding.x_rot = -0.970_796_35;
    pulling.x_rot = holding.x_rot;
    let use_ticks = ticks_using_item.clamp(0.0, max_charge_duration);
    let alpha = if max_charge_duration > 0.0 {
        use_ticks / max_charge_duration
    } else {
        0.0
    };
    pulling.y_rot = lerp(alpha, 0.4, 0.85) * sign;
    pulling.x_rot = lerp(alpha, pulling.x_rot, -FRAC_PI_2);
}

pub fn animate_crossbow_hold(
    holding: &mut PartState,
    shooting: &mut PartState,
    head: &PartState,
    holding_in_right_arm: bool,
) {
    let sign = if holding_in_right_arm { 1.0 } else { -1.0 };
    holding.y_rot = -0.3 * sign + head.y_rot;
    shooting.y_rot = 0.6 * sign + head.y_rot;
    holding.x_rot = -FRAC_PI_2 + head.x_rot + 0.1;
    shooting.x_rot = -1.5 + head.x_rot;
}

pub fn crossbow_charge(l: &mut Limbs, right: bool, max_charge: f32, ticks_using_item: f32) {
    let (holding, pulling) = if right {
        (&mut l.right_arm, &mut l.left_arm)
    } else {
        (&mut l.left_arm, &mut l.right_arm)
    };
    animate_crossbow_charge(holding, pulling, max_charge, ticks_using_item, right);
}

pub fn crossbow_hold(l: &mut Limbs, right: bool) {
    let head = l.head;
    let (holding, shooting) = if right {
        (&mut l.right_arm, &mut l.left_arm)
    } else {
        (&mut l.left_arm, &mut l.right_arm)
    };
    animate_crossbow_hold(holding, shooting, &head, right);
}

pub fn swing_weapon_down(l: &mut Limbs, main_arm_right: bool, attack_time: f32, age_ticks: f32) {
    let attack2 = (attack_time * PI).sin();
    let attack = ((1.0 - (1.0 - attack_time) * (1.0 - attack_time)) * PI).sin();
    l.right_arm.z_rot = 0.0;
    l.left_arm.z_rot = 0.0;
    l.right_arm.y_rot = 0.157_079_64;
    l.left_arm.y_rot = -0.157_079_64;
    if main_arm_right {
        l.right_arm.x_rot = -1.884_955_8 + (age_ticks * 0.09).cos() * 0.15;
        l.left_arm.x_rot = -0.0 + (age_ticks * 0.19).cos() * 0.5;
        l.right_arm.x_rot += attack2 * 2.2 - attack * 0.4;
        l.left_arm.x_rot += attack2 * 1.2 - attack * 0.4;
    } else {
        l.right_arm.x_rot = -0.0 + (age_ticks * 0.19).cos() * 0.5;
        l.left_arm.x_rot = -1.884_955_8 + (age_ticks * 0.09).cos() * 0.15;
        l.right_arm.x_rot += attack2 * 1.2 - attack * 0.4;
        l.left_arm.x_rot += attack2 * 2.2 - attack * 0.4;
    }
    bob_arms(l, age_ticks);
}

pub fn animate_zombie_arms(l: &mut Limbs, st: &EntityState, aggressive: bool) {
    if !st.extras.is_baby || st.extras.main_hand.is_empty() {
        let attack_time = st.attack_time;
        let arm_drop = -PI / if aggressive { 1.5 } else { 2.25 };
        let attack_y = (attack_time * PI).sin();
        let attack_x = ((1.0 - (1.0 - attack_time) * (1.0 - attack_time)) * PI).sin();
        l.right_arm.z_rot = 0.0;
        l.right_arm.y_rot = -(0.1 - attack_y * 0.6);
        l.right_arm.x_rot = arm_drop;
        l.right_arm.x_rot += attack_y * 1.2 - attack_x * 0.4;
        l.left_arm.z_rot = 0.0;
        l.left_arm.y_rot = 0.1 - attack_y * 0.6;
        l.left_arm.x_rot = arm_drop;
        l.left_arm.x_rot += attack_y * 1.2 - attack_x * 0.4;
        bob_arms(l, st.age_ticks);
    }
}

fn pose_blocking_arm(arm: &mut PartState, head: &PartState, right: bool) {
    arm.x_rot = arm.x_rot * 0.5 - 0.942_477_9
        + head
            .x_rot
            .clamp(HIGHEST_SHIELD_BLOCKING_ANGLE, LOWEST_SHIELD_BLOCKING_ANGLE);
    arm.y_rot = (if right { -30.0 } else { 30.0 }) * DEG_TO_RAD
        + head.y_rot.clamp(
            -HORIZONTAL_SHIELD_MOVEMENT_LIMIT,
            HORIZONTAL_SHIELD_MOVEMENT_LIMIT,
        );
}

fn spear_third_person_hand_use(
    arm: &mut PartState,
    head: &PartState,
    st: &EntityState,
    holding_in_right_arm: bool,
) {
    let invert = if holding_in_right_arm { 1.0 } else { -1.0 };
    arm.y_rot = -0.1 * invert + head.y_rot;
    arm.x_rot = -FRAC_PI_2 + head.x_rot + 0.8;
    if st.extras.fall_flying || st.extras.swim_amount > 0.0 {
        arm.x_rot -= 0.959_931_1;
    }
    arm.y_rot = DEG_TO_RAD * (RAD_TO_DEG * arm.y_rot).clamp(-60.0, 60.0);
    arm.x_rot = DEG_TO_RAD * (RAD_TO_DEG * arm.x_rot).clamp(-120.0, 30.0);
}

fn pose_arm(
    arm: &mut PartState,
    other: &mut PartState,
    head: &PartState,
    st: &EntityState,
    pose: ArmPose,
    right: bool,
) {
    let invert = if right { 1.0 } else { -1.0 };
    match pose {
        ArmPose::Empty => {
            arm.y_rot = 0.0;
        }
        ArmPose::Item => {
            arm.x_rot = arm.x_rot * 0.5 - 0.314_159_27;
            arm.y_rot = 0.0;
        }
        ArmPose::Block => {
            pose_blocking_arm(arm, head, right);
        }
        ArmPose::BowAndArrow => {
            arm.y_rot = -0.1 * invert + head.y_rot;
            other.y_rot = 0.1 * invert + head.y_rot + 0.4 * invert;
            arm.x_rot = -FRAC_PI_2 + head.x_rot;
            other.x_rot = -FRAC_PI_2 + head.x_rot;
        }
        ArmPose::ThrowTrident => {
            arm.x_rot = arm.x_rot * 0.5 - PI;
            arm.y_rot = 0.0;
        }
        ArmPose::CrossbowCharge => {
            animate_crossbow_charge(
                arm,
                other,
                st.extras.max_crossbow_charge,
                st.extras.ticks_using_item,
                right,
            );
        }
        ArmPose::CrossbowHold => {
            animate_crossbow_hold(arm, other, head, right);
        }
        ArmPose::Spyglass => {
            arm.x_rot = (head.x_rot
                - SPYGLASS_ARM_ROT_X
                - if st.extras.crouching {
                    SPYGLASS_ARM_CROUCH_ROT_X
                } else {
                    0.0
                })
            .clamp(-2.4, 3.3);
            arm.y_rot = head.y_rot - SPYGLASS_ARM_ROT_Y * invert;
        }
        ArmPose::TootHorn => {
            arm.x_rot = head.x_rot.clamp(-1.2, 1.2) - TOOT_HORN_XROT_BASE;
            arm.y_rot = head.y_rot - TOOT_HORN_YROT_BASE * invert;
        }
        ArmPose::Brush => {
            arm.x_rot = arm.x_rot * 0.5 - 0.628_318_55;
            arm.y_rot = 0.0;
        }
        ArmPose::Spear => {
            spear_third_person_hand_use(arm, head, st, right);
        }
    }
}

pub fn setup_attack_animation(l: &mut Limbs, st: &EntityState, poses: ArmPoses) {
    let attack_time = st.attack_time;
    if attack_time <= 0.0 {
        return;
    }
    l.body.y_rot = (attack_time.sqrt() * TAU).sin() * 0.2;
    if st.swing_left {
        l.body.y_rot *= -1.0;
    }
    let age_scale = st.age_scale;
    l.right_arm.z = l.body.y_rot.sin() * 5.0 * age_scale;
    l.right_arm.x = -l.body.y_rot.cos() * 5.0 * age_scale;
    l.left_arm.z = -l.body.y_rot.sin() * 5.0 * age_scale;
    l.left_arm.x = l.body.y_rot.cos() * 5.0 * age_scale;
    l.right_arm.y_rot += l.body.y_rot;
    l.left_arm.y_rot += l.body.y_rot;
    l.left_arm.x_rot += l.body.y_rot;

    let stab = if st.swing_left {
        poses.left
    } else {
        poses.right
    } == ArmPose::Spear;
    let body_yaw = l.body.y_rot;
    if stab {
        l.right_arm.y_rot -= body_yaw;
        l.left_arm.y_rot -= body_yaw;
        l.left_arm.x_rot -= body_yaw;
        let prepare = in_out_sine(progress(attack_time, 0.0, 0.05));
        let attack = in_quad(progress(attack_time, 0.05, 0.2));
        let retract = in_out_expo(progress(attack_time, 0.4, 1.0));
        let arm = l.arm(st.swing_left);
        arm.x_rot += (90.0 * prepare - 120.0 * attack + 30.0 * retract) * DEG_TO_RAD;
    } else {
        let swing = out_quart(attack_time);
        let arc = (swing * PI).sin();
        let head_x_rot = l.head.x_rot;
        let aim = (attack_time * PI).sin() * -(head_x_rot - 0.7) * 0.75;
        let arm = l.arm(st.swing_left);
        arm.x_rot -= arc * 1.2 + aim;
        arm.y_rot += body_yaw * 2.0;
        arm.z_rot += (attack_time * PI).sin() * -0.4;
    }
}

pub fn pose_humanoid_with(
    l: &mut Limbs,
    st: &EntityState,
    poses: ArmPoses,
    setup_attack: fn(&mut Limbs, &EntityState, ArmPoses),
) {
    let swim_amount = st.extras.swim_amount;
    let fall_flying = st.extras.fall_flying;

    l.head.x_rot = st.x_rot * DEG_TO_RAD;
    l.head.y_rot = st.y_rot * DEG_TO_RAD;
    if fall_flying {
        l.head.x_rot = -FRAC_PI_4;
    } else if swim_amount > 0.0 {
        l.head.x_rot = rot_lerp_rad(swim_amount, l.head.x_rot, -FRAC_PI_4);
    }

    let pos = st.walk_pos;
    let speed = st.walk_speed;
    let speed_value = st.extras.speed_value.max(1.0);
    l.right_arm.x_rot = (pos * 0.6662 + PI).cos() * 2.0 * speed * 0.5 / speed_value;
    l.left_arm.x_rot = (pos * 0.6662).cos() * 2.0 * speed * 0.5 / speed_value;
    l.right_leg.x_rot = (pos * 0.6662).cos() * 1.4 * speed / speed_value;
    l.left_leg.x_rot = (pos * 0.6662 + PI).cos() * 1.4 * speed / speed_value;
    l.right_leg.y_rot = DUCK_WALK_ROTATION;
    l.left_leg.y_rot = -DUCK_WALK_ROTATION;
    l.right_leg.z_rot = DUCK_WALK_ROTATION;
    l.left_leg.z_rot = -DUCK_WALK_ROTATION;

    if st.extras.passenger {
        l.right_arm.x_rot += -0.628_318_55;
        l.left_arm.x_rot += -0.628_318_55;
        l.right_leg.x_rot = -1.413_716_7;
        l.right_leg.y_rot = 0.314_159_27;
        l.right_leg.z_rot = 0.078_539_82;
        l.left_leg.x_rot = -1.413_716_7;
        l.left_leg.y_rot = -0.314_159_27;
        l.left_leg.z_rot = -0.078_539_82;
    }

    let right_handed = !st.extras.left_handed;
    let right_first = if st.extras.using_item {
        (!st.extras.use_offhand) == right_handed
    } else {
        let two_handed = if right_handed {
            poses.left.two_handed()
        } else {
            poses.right.two_handed()
        };
        right_handed == two_handed
    };
    let head = l.head;
    if right_first {
        pose_arm(
            &mut l.right_arm,
            &mut l.left_arm,
            &head,
            st,
            poses.right,
            true,
        );
        if !poses.right.affects_offhand_pose() {
            pose_arm(
                &mut l.left_arm,
                &mut l.right_arm,
                &head,
                st,
                poses.left,
                false,
            );
        }
    } else {
        pose_arm(
            &mut l.left_arm,
            &mut l.right_arm,
            &head,
            st,
            poses.left,
            false,
        );
        if !poses.left.affects_offhand_pose() {
            pose_arm(
                &mut l.right_arm,
                &mut l.left_arm,
                &head,
                st,
                poses.right,
                true,
            );
        }
    }

    setup_attack(l, st, poses);

    if st.extras.crouching {
        l.body.x_rot = 0.5;
        l.right_arm.x_rot += 0.4;
        l.left_arm.x_rot += 0.4;
        l.right_leg.z += 4.0;
        l.left_leg.z += 4.0;
        l.head.y += 4.2;
        l.body.y += 3.2;
        l.left_arm.y += 3.2;
        l.right_arm.y += 3.2;
    }

    if poses.right != ArmPose::Spyglass {
        bob_model_part(&mut l.right_arm, st.age_ticks, 1.0);
    }
    if poses.left != ArmPose::Spyglass {
        bob_model_part(&mut l.left_arm, st.age_ticks, -1.0);
    }

    if swim_amount > 0.0 {
        let swim_pos = pos % 26.0;
        let right_swim =
            if poses.right != ArmPose::Spear && (st.swing_left || st.attack_time <= 0.0) {
                swim_amount
            } else {
                0.0
            };
        let left_swim = if poses.left != ArmPose::Spear && (!st.swing_left || st.attack_time <= 0.0)
        {
            swim_amount
        } else {
            0.0
        };
        if !st.extras.using_item {
            let reach = quadratic_arm_update(swim_pos) / quadratic_arm_update(14.0);
            if swim_pos < 14.0 {
                l.left_arm.x_rot = rot_lerp_rad(left_swim, l.left_arm.x_rot, 0.0);
                l.right_arm.x_rot = lerp(right_swim, l.right_arm.x_rot, 0.0);
                l.left_arm.y_rot = rot_lerp_rad(left_swim, l.left_arm.y_rot, PI);
                l.right_arm.y_rot = lerp(right_swim, l.right_arm.y_rot, PI);
                l.left_arm.z_rot =
                    rot_lerp_rad(left_swim, l.left_arm.z_rot, PI + 1.870_796_4 * reach);
                l.right_arm.z_rot = lerp(right_swim, l.right_arm.z_rot, PI - 1.870_796_4 * reach);
            } else if swim_pos < 22.0 {
                let t = (swim_pos - 14.0) / 8.0;
                l.left_arm.x_rot = rot_lerp_rad(left_swim, l.left_arm.x_rot, FRAC_PI_2 * t);
                l.right_arm.x_rot = lerp(right_swim, l.right_arm.x_rot, FRAC_PI_2 * t);
                l.left_arm.y_rot = rot_lerp_rad(left_swim, l.left_arm.y_rot, PI);
                l.right_arm.y_rot = lerp(right_swim, l.right_arm.y_rot, PI);
                l.left_arm.z_rot =
                    rot_lerp_rad(left_swim, l.left_arm.z_rot, 5.012_389 - 1.870_796_4 * t);
                l.right_arm.z_rot =
                    lerp(right_swim, l.right_arm.z_rot, 1.270_796_3 + 1.870_796_4 * t);
            } else if swim_pos < 26.0 {
                let t = (swim_pos - 22.0) / 4.0;
                l.left_arm.x_rot =
                    rot_lerp_rad(left_swim, l.left_arm.x_rot, FRAC_PI_2 - FRAC_PI_2 * t);
                l.right_arm.x_rot = lerp(right_swim, l.right_arm.x_rot, FRAC_PI_2 - FRAC_PI_2 * t);
                l.left_arm.y_rot = rot_lerp_rad(left_swim, l.left_arm.y_rot, PI);
                l.right_arm.y_rot = lerp(right_swim, l.right_arm.y_rot, PI);
                l.left_arm.z_rot = rot_lerp_rad(left_swim, l.left_arm.z_rot, PI);
                l.right_arm.z_rot = lerp(right_swim, l.right_arm.z_rot, PI);
            }
        }
        l.left_leg.x_rot = lerp(
            swim_amount,
            l.left_leg.x_rot,
            0.3 * (pos * 0.333_333_34 + PI).cos(),
        );
        l.right_leg.x_rot = lerp(
            swim_amount,
            l.right_leg.x_rot,
            0.3 * (pos * 0.333_333_34).cos(),
        );
    }
}

pub fn pose_humanoid(l: &mut Limbs, st: &EntityState, poses: ArmPoses) {
    pose_humanoid_with(l, st, poses, setup_attack_animation);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::geom::bake;
    use azalea_registry::builtin::EntityKind;

    #[test]
    fn humanoid_mesh_has_the_parts_setup_anim_writes() {
        let baked = bake(&humanoid_layer());
        for name in [
            "head",
            "hat",
            "body",
            "right_arm",
            "left_arm",
            "right_leg",
            "left_leg",
        ] {
            assert!(baked.find(name).is_some(), "missing {name}");
        }
        let mut parts = baked.rest_pose();
        let st = EntityState::new(0, EntityKind::Zombie);
        let mut limbs = Limbs::load(&baked, &parts);
        pose_humanoid(&mut limbs, &st, mob_arm_poses(&st));
        limbs.store(&mut parts);
    }

    #[test]
    fn scaling_keeps_the_feet_on_the_ground() {
        let baked = bake(&LayerDef::create(
            scaling(create_mesh(Grow::NONE, 0.0), 6.0),
            64,
            64,
        ));
        let root = baked.parts[0].initial;
        assert_eq!(root.x_scale, 6.0);
        assert!((root.y - -120.08).abs() < 1e-3, "{}", root.y);
    }
}
