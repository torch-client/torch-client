use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;

const NECK_PART_COUNT: usize = 5;
const TAIL_PART_COUNT: usize = 12;

const NECK_NAMES: [&str; NECK_PART_COUNT] = ["neck0", "neck1", "neck2", "neck3", "neck4"];
const TAIL_NAMES: [&str; TAIL_PART_COUNT] = [
    "tail0", "tail1", "tail2", "tail3", "tail4", "tail5", "tail6", "tail7", "tail8", "tail9",
    "tail10", "tail11",
];

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let head = root.child(
        "head",
        CubeList::new()
            .add_box_at(-6.0, -1.0, -24.0, 12.0, 5.0, 16.0, Grow::NONE, 176, 44)
            .add_box_at(-8.0, -8.0, -10.0, 16.0, 16.0, 16.0, Grow::NONE, 112, 30)
            .mirror()
            .add_box_at(-5.0, -12.0, -4.0, 2.0, 4.0, 6.0, Grow::NONE, 0, 0)
            .add_box_at(-5.0, -3.0, -22.0, 2.0, 2.0, 4.0, Grow::NONE, 112, 0)
            .mirror()
            .add_box_at(3.0, -12.0, -4.0, 2.0, 4.0, 6.0, Grow::NONE, 0, 0)
            .add_box_at(3.0, -3.0, -22.0, 2.0, 2.0, 4.0, Grow::NONE, 112, 0),
        PartPose::offset(0.0, 20.0, -62.0),
    );
    head.child(
        "jaw",
        CubeList::new().add_box_at(-6.0, 0.0, -16.0, 12.0, 4.0, 16.0, Grow::NONE, 176, 65),
        PartPose::offset(0.0, 4.0, -8.0),
    );

    let spine_cubes = CubeList::new()
        .add_box_at(-5.0, -5.0, -5.0, 10.0, 10.0, 10.0, Grow::NONE, 192, 104)
        .add_box_at(-1.0, -9.0, -3.0, 2.0, 4.0, 6.0, Grow::NONE, 48, 0);
    for i in 0..NECK_PART_COUNT {
        root.child(
            &format!("neck{i}"),
            spine_cubes.clone(),
            PartPose::offset(0.0, 20.0, -12.0 - i as f32 * 10.0),
        );
    }
    for i in 0..TAIL_PART_COUNT {
        root.child(
            &format!("tail{i}"),
            spine_cubes.clone(),
            PartPose::offset(0.0, 10.0, 60.0 + i as f32 * 10.0),
        );
    }

    let body = root.child(
        "body",
        CubeList::new()
            .add_box_at(-12.0, 1.0, -16.0, 24.0, 24.0, 64.0, Grow::NONE, 0, 0)
            .add_box_at(-1.0, -5.0, -10.0, 2.0, 6.0, 12.0, Grow::NONE, 220, 53)
            .add_box_at(-1.0, -5.0, 10.0, 2.0, 6.0, 12.0, Grow::NONE, 220, 53)
            .add_box_at(-1.0, -5.0, 30.0, 2.0, 6.0, 12.0, Grow::NONE, 220, 53),
        PartPose::offset(0.0, 3.0, 8.0),
    );
    let left_wing = body.child(
        "left_wing",
        CubeList::new()
            .mirror()
            .add_box_at(0.0, -4.0, -4.0, 56.0, 8.0, 8.0, Grow::NONE, 112, 88)
            .add_box_at(0.0, 0.0, 2.0, 56.0, 0.0, 56.0, Grow::NONE, -56, 88),
        PartPose::offset(12.0, 2.0, -6.0),
    );
    left_wing.child(
        "left_wing_tip",
        CubeList::new()
            .mirror()
            .add_box_at(0.0, -2.0, -2.0, 56.0, 4.0, 4.0, Grow::NONE, 112, 136)
            .add_box_at(0.0, 0.0, 2.0, 56.0, 0.0, 56.0, Grow::NONE, -56, 144),
        PartPose::offset(56.0, 0.0, 0.0),
    );
    let left_front_leg = body.child(
        "left_front_leg",
        CubeList::new().add_box_at(-4.0, -4.0, -4.0, 8.0, 24.0, 8.0, Grow::NONE, 112, 104),
        PartPose::offset_rotation(12.0, 17.0, -6.0, 1.3, 0.0, 0.0),
    );
    let left_front_leg_tip = left_front_leg.child(
        "left_front_leg_tip",
        CubeList::new().add_box_at(-3.0, -1.0, -3.0, 6.0, 24.0, 6.0, Grow::NONE, 226, 138),
        PartPose::offset_rotation(0.0, 20.0, -1.0, -0.5, 0.0, 0.0),
    );
    left_front_leg_tip.child(
        "left_front_foot",
        CubeList::new().add_box_at(-4.0, 0.0, -12.0, 8.0, 4.0, 16.0, Grow::NONE, 144, 104),
        PartPose::offset_rotation(0.0, 23.0, 0.0, 0.75, 0.0, 0.0),
    );
    let left_hind_leg = body.child(
        "left_hind_leg",
        CubeList::new().add_box_at(-8.0, -4.0, -8.0, 16.0, 32.0, 16.0, Grow::NONE, 0, 0),
        PartPose::offset_rotation(16.0, 13.0, 34.0, 1.0, 0.0, 0.0),
    );
    let left_hind_leg_tip = left_hind_leg.child(
        "left_hind_leg_tip",
        CubeList::new().add_box_at(-6.0, -2.0, 0.0, 12.0, 32.0, 12.0, Grow::NONE, 196, 0),
        PartPose::offset_rotation(0.0, 32.0, -4.0, 0.5, 0.0, 0.0),
    );
    left_hind_leg_tip.child(
        "left_hind_foot",
        CubeList::new().add_box_at(-9.0, 0.0, -20.0, 18.0, 6.0, 24.0, Grow::NONE, 112, 0),
        PartPose::offset_rotation(0.0, 31.0, 4.0, 0.75, 0.0, 0.0),
    );
    let right_wing = body.child(
        "right_wing",
        CubeList::new()
            .add_box_at(-56.0, -4.0, -4.0, 56.0, 8.0, 8.0, Grow::NONE, 112, 88)
            .add_box_at(-56.0, 0.0, 2.0, 56.0, 0.0, 56.0, Grow::NONE, -56, 88),
        PartPose::offset(-12.0, 2.0, -6.0),
    );
    right_wing.child(
        "right_wing_tip",
        CubeList::new()
            .add_box_at(-56.0, -2.0, -2.0, 56.0, 4.0, 4.0, Grow::NONE, 112, 136)
            .add_box_at(-56.0, 0.0, 2.0, 56.0, 0.0, 56.0, Grow::NONE, -56, 144),
        PartPose::offset(-56.0, 0.0, 0.0),
    );
    let right_front_leg = body.child(
        "right_front_leg",
        CubeList::new().add_box_at(-4.0, -4.0, -4.0, 8.0, 24.0, 8.0, Grow::NONE, 112, 104),
        PartPose::offset_rotation(-12.0, 17.0, -6.0, 1.3, 0.0, 0.0),
    );
    let right_front_leg_tip = right_front_leg.child(
        "right_front_leg_tip",
        CubeList::new().add_box_at(-3.0, -1.0, -3.0, 6.0, 24.0, 6.0, Grow::NONE, 226, 138),
        PartPose::offset_rotation(0.0, 20.0, -1.0, -0.5, 0.0, 0.0),
    );
    right_front_leg_tip.child(
        "right_front_foot",
        CubeList::new().add_box_at(-4.0, 0.0, -12.0, 8.0, 4.0, 16.0, Grow::NONE, 144, 104),
        PartPose::offset_rotation(0.0, 23.0, 0.0, 0.75, 0.0, 0.0),
    );
    let right_hind_leg = body.child(
        "right_hind_leg",
        CubeList::new().add_box_at(-8.0, -4.0, -8.0, 16.0, 32.0, 16.0, Grow::NONE, 0, 0),
        PartPose::offset_rotation(-16.0, 13.0, 34.0, 1.0, 0.0, 0.0),
    );
    let right_hind_leg_tip = right_hind_leg.child(
        "right_hind_leg_tip",
        CubeList::new().add_box_at(-6.0, -2.0, 0.0, 12.0, 32.0, 12.0, Grow::NONE, 196, 0),
        PartPose::offset_rotation(0.0, 32.0, -4.0, 0.5, 0.0, 0.0),
    );
    right_hind_leg_tip.child(
        "right_hind_foot",
        CubeList::new().add_box_at(-9.0, 0.0, -20.0, 18.0, 6.0, 24.0, Grow::NONE, 112, 0),
        PartPose::offset_rotation(0.0, 31.0, 4.0, 0.75, 0.0, 0.0),
    );
    LayerDef::create(mesh, 256, 256)
}

#[allow(clippy::too_many_arguments)]
fn pose_limbs(
    parts: &mut [PartState],
    bounce: f32,
    front_leg: usize,
    front_leg_tip: usize,
    front_foot: usize,
    hind_leg: usize,
    hind_leg_tip: usize,
    hind_foot: usize,
) {
    parts[hind_leg].x_rot = 1.0 + bounce * 0.1;
    parts[hind_leg_tip].x_rot = 0.5 + bounce * 0.1;
    parts[hind_foot].x_rot = 0.75 + bounce * 0.1;
    parts[front_leg].x_rot = 1.3 + bounce * 0.1;
    parts[front_leg_tip].x_rot = -0.5 - bounce * 0.1;
    parts[front_foot].x_rot = 0.75 + bounce * 0.1;
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let flap_time = st.extras.flap_time * 6.2831855;
    parts[model.id("jaw")].x_rot = (flap_time.sin() + 1.0) * 0.2;

    let mut bounce = (flap_time - 1.0).sin() + 1.0;
    bounce = (bounce * bounce + bounce * 2.0) * 0.05;
    let root = model.id("root");
    parts[root].y = (bounce - 2.0) * 16.0;
    parts[root].z = -48.0;
    parts[root].x_rot = bounce * 2.0 * 0.017_453_292;

    let left_wing = model.id("left_wing");
    let left_wing_tip = model.id("left_wing_tip");
    parts[left_wing].x_rot = 0.125 - flap_time.cos() * 0.2;
    parts[left_wing].y_rot = -0.25;
    parts[left_wing].z_rot = -(flap_time.sin() + 0.125) * 0.8;
    parts[left_wing_tip].z_rot = ((flap_time + 2.0).sin() + 0.5) * 0.75;
    let (x_rot, y_rot, z_rot) = (
        parts[left_wing].x_rot,
        parts[left_wing].y_rot,
        parts[left_wing].z_rot,
    );
    let tip_z_rot = parts[left_wing_tip].z_rot;
    let right_wing = model.id("right_wing");
    parts[right_wing].x_rot = x_rot;
    parts[right_wing].y_rot = -y_rot;
    parts[right_wing].z_rot = -z_rot;
    parts[model.id("right_wing_tip")].z_rot = -tip_z_rot;

    pose_limbs(
        parts,
        bounce,
        model.id("left_front_leg"),
        model.id("left_front_leg_tip"),
        model.id("left_front_foot"),
        model.id("left_hind_leg"),
        model.id("left_hind_leg_tip"),
        model.id("left_hind_foot"),
    );
    pose_limbs(
        parts,
        bounce,
        model.id("right_front_leg"),
        model.id("right_front_leg_tip"),
        model.id("right_front_foot"),
        model.id("right_hind_leg"),
        model.id("right_hind_leg_tip"),
        model.id("right_hind_foot"),
    );

    let mut tail_x_rot = 0.0;
    for i in 0..TAIL_PART_COUNT {
        tail_x_rot += (i as f32 * 0.45 + flap_time).sin() * 0.05;
        let id = model.id(TAIL_NAMES[i]);
        parts[id].x_rot = tail_x_rot;
        parts[id].y_rot = 180.0 * 0.017_453_292;
    }

    for i in 0..NECK_PART_COUNT {
        let id = model.id(NECK_NAMES[i]);
        parts[id].x_rot = (i as f32 * 0.45 + flap_time).cos() * 0.15;
    }
}
