#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::models::aquatic::DEG_TO_RAD;
use crate::entities::state::EntityState;

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh
        .root()
        .child("root", CubeList::new(), PartPose::offset(0.0, 23.5, 0.0));
    root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-2.5, -5.0, -2.5, 5.0, 5.0, 5.0),
        PartPose::offset(0.0, -3.99, 0.0),
    );
    let body = root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 10)
            .add_box(-1.5, 0.0, -1.0, 3.0, 4.0, 2.0)
            .tex_offs(0, 16)
            .add_box_grow(-1.5, 0.0, -1.0, 3.0, 5.0, 2.0, Grow::all(-0.2)),
        PartPose::offset(0.0, -4.0, 0.0),
    );
    body.child(
        "right_arm",
        CubeList::new().tex_offs(23, 0).add_box_grow(
            -0.75,
            -0.5,
            -1.0,
            1.0,
            4.0,
            2.0,
            Grow::all(-0.01),
        ),
        PartPose::offset(-1.75, 0.5, 0.0),
    );
    body.child(
        "left_arm",
        CubeList::new().tex_offs(23, 6).add_box_grow(
            -0.25,
            -0.5,
            -1.0,
            1.0,
            4.0,
            2.0,
            Grow::all(-0.01),
        ),
        PartPose::offset(1.75, 0.5, 0.0),
    );
    body.child(
        "right_wing",
        CubeList::new()
            .tex_offs(16, 14)
            .add_box(0.0, 1.0, 0.0, 0.0, 5.0, 8.0),
        PartPose::offset(-0.5, 0.0, 0.6),
    );
    body.child(
        "left_wing",
        CubeList::new()
            .tex_offs(16, 14)
            .add_box(0.0, 1.0, 0.0, 0.0, 5.0, 8.0),
        PartPose::offset(0.5, 0.0, 0.6),
    );
    LayerDef::create(mesh, 32, 32)
}

const FLYING_ANIMATION_X_ROT: f32 = 0.785_398_2;
const MAX_HAND_HOLDING_ITEM_X_ROT_RAD: f32 = -1.134_464;
const MIN_HAND_HOLDING_ITEM_X_ROT_RAD: f32 = -1.047_197_6;

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let root = model.id("root");
    let head = model.id("head");
    let body = model.id("body");
    let right_arm = model.id("right_arm");
    let left_arm = model.id("left_arm");
    let right_wing = model.id("right_wing");
    let left_wing = model.id("left_wing");

    let animation_speed = st.walk_speed;
    let animation_pos = st.walk_pos;
    let flap_speed = st.age_ticks * 20.0 * DEG_TO_RAD + animation_pos;
    let flap_amount = flap_speed.cos() * std::f32::consts::PI * 0.15 + animation_speed;
    let idle_bob_speed = st.age_ticks * 9.0 * DEG_TO_RAD;
    let flying_factor = (animation_speed / 0.3).min(1.0);
    let idle_bob_factor = 1.0 - flying_factor;
    let holding_item_factor = st.extras.holding_progress;

    if st.extras.dancing {
        let dance_speed = st.age_ticks * 8.0 * DEG_TO_RAD;
        let root_tilt_z = dance_speed.cos() * 16.0 * DEG_TO_RAD;
        let spinning_progress = st.extras.spinning_progress;
        let head_tilt_z = dance_speed.cos() * 14.0 * DEG_TO_RAD;
        let head_tilt_y = dance_speed.cos() * 30.0 * DEG_TO_RAD;
        if st.extras.spinning {
            parts[root].y_rot = 12.566_371 * spinning_progress;
        }
        parts[root].z_rot = root_tilt_z * (1.0 - spinning_progress);
        parts[head].y_rot = head_tilt_y * (1.0 - spinning_progress);
        parts[head].z_rot = head_tilt_z * (1.0 - spinning_progress);
    } else {
        parts[head].x_rot = st.x_rot * DEG_TO_RAD;
        parts[head].y_rot = st.y_rot * DEG_TO_RAD;
    }

    parts[right_wing].x_rot = 0.436_332_32 * (1.0 - flying_factor);
    parts[right_wing].y_rot = -0.785_398_2 + flap_amount;
    parts[left_wing].x_rot = 0.436_332_32 * (1.0 - flying_factor);
    parts[left_wing].y_rot = 0.785_398_2 - flap_amount;
    parts[body].x_rot = flying_factor * FLYING_ANIMATION_X_ROT;

    let arm_x_rot = holding_item_factor
        * (MIN_HAND_HOLDING_ITEM_X_ROT_RAD
            + flying_factor * (MAX_HAND_HOLDING_ITEM_X_ROT_RAD - MIN_HAND_HOLDING_ITEM_X_ROT_RAD));
    parts[root].y += idle_bob_speed.cos() * 0.25 * idle_bob_factor;
    parts[right_arm].x_rot = arm_x_rot;
    parts[left_arm].x_rot = arm_x_rot;
    let arm_idle_bob_factor = idle_bob_factor * (1.0 - holding_item_factor);
    let arm_spread = 0.436_332_32
        - (idle_bob_speed + 4.712_389).cos() * std::f32::consts::PI * 0.075 * arm_idle_bob_factor;
    parts[left_arm].z_rot = -arm_spread;
    parts[right_arm].z_rot = arm_spread;
    parts[right_arm].y_rot = 0.279_252_68 * holding_item_factor;
    parts[left_arm].y_rot = -0.279_252_68 * holding_item_factor;
}
