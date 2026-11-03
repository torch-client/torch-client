use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;

use super::humanoid::{self, BabyTransform, DEG_TO_RAD};

const BODY_PARTS: [&str; 10] = [
    "head",
    "body",
    "right_arm",
    "left_arm",
    "right_leg",
    "left_leg",
    "right_body_stick",
    "left_body_stick",
    "shoulder_stick",
    "base_plate",
];

pub fn armor_stand_mesh() -> MeshDef {
    let mut mesh = humanoid::create_mesh(Grow::NONE, 0.0);
    let root = mesh.root();
    root.part(
        "head",
        (0, 0),
        [-1.0, -7.0, -1.0, 2.0, 7.0, 2.0],
        PartPose::offset(0.0, 1.0, 0.0),
    );
    root.part(
        "body",
        (0, 26),
        [-6.0, 0.0, -1.5, 12.0, 3.0, 3.0],
        PartPose::ZERO,
    );
    root.part(
        "right_arm",
        (24, 0),
        [-2.0, -2.0, -1.0, 2.0, 12.0, 2.0],
        PartPose::offset(-5.0, 2.0, 0.0),
    );
    root.child(
        "left_arm",
        CubeList::new()
            .tex_offs(32, 16)
            .mirror()
            .add_box(0.0, -2.0, -1.0, 2.0, 12.0, 2.0),
        PartPose::offset(5.0, 2.0, 0.0),
    );
    root.part(
        "right_leg",
        (8, 0),
        [-1.0, 0.0, -1.0, 2.0, 11.0, 2.0],
        PartPose::offset(-1.9, 12.0, 0.0),
    );
    root.child(
        "left_leg",
        CubeList::new()
            .tex_offs(40, 16)
            .mirror()
            .add_box(-1.0, 0.0, -1.0, 2.0, 11.0, 2.0),
        PartPose::offset(1.9, 12.0, 0.0),
    );
    root.part(
        "right_body_stick",
        (16, 0),
        [-3.0, 3.0, -1.0, 2.0, 7.0, 2.0],
        PartPose::ZERO,
    );
    root.part(
        "left_body_stick",
        (48, 16),
        [1.0, 3.0, -1.0, 2.0, 7.0, 2.0],
        PartPose::ZERO,
    );
    root.part(
        "shoulder_stick",
        (0, 48),
        [-4.0, 10.0, -1.0, 8.0, 2.0, 2.0],
        PartPose::ZERO,
    );
    root.part(
        "base_plate",
        (0, 32),
        [-6.0, 11.0, -6.0, 12.0, 1.0, 12.0],
        PartPose::offset(0.0, 12.0, 0.0),
    );
    mesh
}

pub fn armor_stand_layer() -> LayerDef {
    LayerDef::create(armor_stand_mesh(), 64, 64)
}

pub fn armor_stand_small_layer() -> LayerDef {
    LayerDef::create(
        humanoid::baby_transform(
            armor_stand_mesh(),
            BabyTransform::HUMANOID,
            &BODY_PARTS,
            &["head"],
        ),
        64,
        64,
    )
}

pub fn armor_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let mut limbs = humanoid::Limbs::load(model, parts);
    humanoid::pose_humanoid(&mut limbs, st, humanoid::ArmPoses::default());
    limbs.store(parts);
    let pose = st.extras.stand_pose;
    for (name, angles) in [
        ("head", pose[0]),
        ("body", pose[1]),
        ("left_arm", pose[2]),
        ("right_arm", pose[3]),
        ("left_leg", pose[4]),
        ("right_leg", pose[5]),
    ] {
        let id = model.id(name);
        parts[id].set_rotation(
            DEG_TO_RAD * angles[0],
            DEG_TO_RAD * angles[1],
            DEG_TO_RAD * angles[2],
        );
    }
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    armor_setup_anim(model, parts, st);

    if let Some(hat) = model.find("hat") {
        parts[hat].visible = false;
    }

    let base_plate = model.id("base_plate");
    parts[base_plate].y_rot = DEG_TO_RAD * -st.body_rot;
    parts[base_plate].visible = st.extras.show_base_plate;

    let show_arms = st.extras.show_arms;
    parts[model.id("left_arm")].visible = show_arms;
    parts[model.id("right_arm")].visible = show_arms;

    let body = st.extras.stand_pose[1];
    for name in ["right_body_stick", "left_body_stick", "shoulder_stick"] {
        let id = model.id(name);
        parts[id].set_rotation(
            DEG_TO_RAD * body[0],
            DEG_TO_RAD * body[1],
            DEG_TO_RAD * body[2],
        );
    }
}
