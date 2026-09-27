use std::f32::consts::{FRAC_PI_2, PI};

use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;

use super::humanoid::{self, DEG_TO_RAD};
use super::villager::VILLAGER_LIKE_SCALE;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IllagerArmPose {
    Crossed,
    Attacking,
    Spellcasting,
    BowAndArrow,
    CrossbowHold,
    CrossbowCharge,
    Celebrating,
    Neutral,
}

pub fn illager_mesh() -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let head = root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -10.0, -4.0, 8.0, 10.0, 8.0),
        PartPose::offset(0.0, 0.0, 0.0),
    );
    head.child(
        "hat",
        CubeList::new().tex_offs(32, 0).add_box_grow(
            -4.0,
            -10.0,
            -4.0,
            8.0,
            12.0,
            8.0,
            Grow::all(0.45),
        ),
        PartPose::ZERO,
    );
    head.child(
        "nose",
        CubeList::new()
            .tex_offs(24, 0)
            .add_box(-1.0, -1.0, -6.0, 2.0, 4.0, 2.0),
        PartPose::offset(0.0, -2.0, 0.0),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(16, 20)
            .add_box(-4.0, 0.0, -3.0, 8.0, 12.0, 6.0)
            .tex_offs(0, 38)
            .add_box_grow(-4.0, 0.0, -3.0, 8.0, 20.0, 6.0, Grow::all(0.5)),
        PartPose::offset(0.0, 0.0, 0.0),
    );
    root.child(
        "arms",
        CubeList::new()
            .tex_offs(44, 22)
            .add_box(-8.0, -2.0, -2.0, 4.0, 8.0, 4.0)
            .tex_offs(40, 38)
            .add_box(-4.0, 2.0, -2.0, 8.0, 4.0, 4.0),
        PartPose::offset_rotation(0.0, 3.0, -1.0, -0.75, 0.0, 0.0),
    )
    .child(
        "left_shoulder",
        CubeList::new()
            .tex_offs(44, 22)
            .mirror()
            .add_box(4.0, -2.0, -2.0, 4.0, 8.0, 4.0),
        PartPose::ZERO,
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
    root.child(
        "right_arm",
        CubeList::new()
            .tex_offs(40, 46)
            .add_box(-3.0, -2.0, -2.0, 4.0, 12.0, 4.0),
        PartPose::offset(-5.0, 2.0, 0.0),
    );
    root.child(
        "left_arm",
        CubeList::new()
            .tex_offs(40, 46)
            .mirror()
            .add_box(-1.0, -2.0, -2.0, 4.0, 12.0, 4.0),
        PartPose::offset(5.0, 2.0, 0.0),
    );
    mesh
}

pub fn illager_layer() -> LayerDef {
    LayerDef::create(
        humanoid::scaling(illager_mesh(), VILLAGER_LIKE_SCALE),
        64,
        64,
    )
}

fn setup(
    model: &BakedModel,
    parts: &mut [PartState],
    st: &EntityState,
    pose: IllagerArmPose,
    hat_visible: bool,
) {
    let head = model.id("head");
    parts[head].y_rot = st.y_rot * DEG_TO_RAD;
    parts[head].x_rot = st.x_rot * DEG_TO_RAD;
    let head_y_rot = parts[head].y_rot;
    let head_x_rot = parts[head].x_rot;

    let hat = model.id("hat");
    parts[hat].visible = hat_visible;

    let right_arm = model.id("right_arm");
    let left_arm = model.id("left_arm");
    let right_leg = model.id("right_leg");
    let left_leg = model.id("left_leg");

    if st.extras.passenger {
        parts[right_arm].set_rotation(-0.628_318_55, 0.0, 0.0);
        parts[left_arm].set_rotation(-0.628_318_55, 0.0, 0.0);
        parts[right_leg].set_rotation(-1.413_716_7, 0.314_159_27, 0.078_539_82);
        parts[left_leg].set_rotation(-1.413_716_7, -0.314_159_27, -0.078_539_82);
    } else {
        let speed = st.walk_speed;
        let pos = st.walk_pos;
        parts[right_arm].set_rotation((pos * 0.6662 + PI).cos() * 2.0 * speed * 0.5, 0.0, 0.0);
        parts[left_arm].set_rotation((pos * 0.6662).cos() * 2.0 * speed * 0.5, 0.0, 0.0);
        parts[right_leg].set_rotation((pos * 0.6662).cos() * 1.4 * speed * 0.5, 0.0, 0.0);
        parts[left_leg].set_rotation((pos * 0.6662 + PI).cos() * 1.4 * speed * 0.5, 0.0, 0.0);
    }

    match pose {
        IllagerArmPose::Attacking => {
            let mut limbs = humanoid::Limbs::load(model, parts);
            if st.extras.main_hand.is_empty() {
                humanoid::animate_zombie_arms(&mut limbs, st, true);
            } else {
                humanoid::swing_weapon_down(
                    &mut limbs,
                    !st.extras.left_handed,
                    st.attack_time,
                    st.age_ticks,
                );
            }
            limbs.store(parts);
        }
        IllagerArmPose::Spellcasting => {
            parts[right_arm].z = 0.0;
            parts[right_arm].x = -5.0;
            parts[left_arm].z = 0.0;
            parts[left_arm].x = 5.0;
            parts[right_arm].x_rot = (st.age_ticks * 0.6662).cos() * 0.25;
            parts[left_arm].x_rot = (st.age_ticks * 0.6662).cos() * 0.25;
            parts[right_arm].z_rot = 2.356_194_5;
            parts[left_arm].z_rot = -2.356_194_5;
            parts[right_arm].y_rot = 0.0;
            parts[left_arm].y_rot = 0.0;
        }
        IllagerArmPose::BowAndArrow => {
            parts[right_arm].y_rot = -0.1 + head_y_rot;
            parts[right_arm].x_rot = -FRAC_PI_2 + head_x_rot;
            parts[left_arm].x_rot = -0.942_477_9 + head_x_rot;
            parts[left_arm].y_rot = head_y_rot - 0.4;
            parts[left_arm].z_rot = FRAC_PI_2;
        }
        IllagerArmPose::CrossbowHold => {
            let mut limbs = humanoid::Limbs::load(model, parts);
            humanoid::crossbow_hold(&mut limbs, true);
            limbs.store(parts);
        }
        IllagerArmPose::CrossbowCharge => {
            let mut limbs = humanoid::Limbs::load(model, parts);
            humanoid::crossbow_charge(
                &mut limbs,
                true,
                st.extras.max_crossbow_charge,
                st.extras.ticks_using_item,
            );
            limbs.store(parts);
        }
        IllagerArmPose::Celebrating => {
            parts[right_arm].z = 0.0;
            parts[right_arm].x = -5.0;
            parts[right_arm].x_rot = (st.age_ticks * 0.6662).cos() * 0.05;
            parts[right_arm].z_rot = 2.670_354;
            parts[right_arm].y_rot = 0.0;
            parts[left_arm].z = 0.0;
            parts[left_arm].x = 5.0;
            parts[left_arm].x_rot = (st.age_ticks * 0.6662).cos() * 0.05;
            parts[left_arm].z_rot = -2.356_194_5;
            parts[left_arm].y_rot = 0.0;
        }
        IllagerArmPose::Crossed | IllagerArmPose::Neutral => {}
    }

    let crossed_arms = pose == IllagerArmPose::Crossed;
    let arms = model.id("arms");
    parts[arms].visible = crossed_arms;
    parts[left_arm].visible = !crossed_arms;
    parts[right_arm].visible = !crossed_arms;
}

pub fn spellcaster_pose(st: &EntityState) -> IllagerArmPose {
    if st.extras.spell > 0 {
        IllagerArmPose::Spellcasting
    } else if st.extras.celebrating {
        IllagerArmPose::Celebrating
    } else {
        IllagerArmPose::Crossed
    }
}

pub fn illusioner_pose(st: &EntityState) -> IllagerArmPose {
    if st.extras.spell > 0 {
        IllagerArmPose::Spellcasting
    } else if st.extras.aggressive {
        IllagerArmPose::BowAndArrow
    } else {
        IllagerArmPose::Crossed
    }
}

pub fn pillager_pose(st: &EntityState) -> IllagerArmPose {
    if st.extras.charging_crossbow {
        IllagerArmPose::CrossbowCharge
    } else if st.extras.main_hand.id == "crossbow" || st.extras.off_hand.id == "crossbow" {
        IllagerArmPose::CrossbowHold
    } else if st.extras.aggressive {
        IllagerArmPose::Attacking
    } else {
        IllagerArmPose::Neutral
    }
}

pub fn vindicator_pose(st: &EntityState) -> IllagerArmPose {
    if st.extras.aggressive {
        IllagerArmPose::Attacking
    } else if st.extras.celebrating {
        IllagerArmPose::Celebrating
    } else {
        IllagerArmPose::Crossed
    }
}

pub fn evoker_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(model, parts, st, spellcaster_pose(st), false);
}

pub fn illusioner_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(model, parts, st, illusioner_pose(st), true);
}

pub fn pillager_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(model, parts, st, pillager_pose(st), false);
}

pub fn vindicator_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(model, parts, st, vindicator_pose(st), false);
}
