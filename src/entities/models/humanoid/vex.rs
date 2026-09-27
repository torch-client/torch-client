use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;

use super::humanoid::DEG_TO_RAD;

pub fn vex_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh
        .root()
        .child("root", CubeList::new(), PartPose::offset(0.0, -2.5, 0.0));
    root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-2.5, -5.0, -2.5, 5.0, 5.0, 5.0),
        PartPose::offset(0.0, 20.0, 0.0),
    );
    let body = root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 10)
            .add_box(-1.5, 0.0, -1.0, 3.0, 4.0, 2.0)
            .tex_offs(0, 16)
            .add_box_grow(-1.5, 1.0, -1.0, 3.0, 5.0, 2.0, Grow::all(-0.2)),
        PartPose::offset(0.0, 20.0, 0.0),
    );
    body.child(
        "right_arm",
        CubeList::new().tex_offs(23, 0).add_box_grow(
            -1.25,
            -0.5,
            -1.0,
            2.0,
            4.0,
            2.0,
            Grow::all(-0.1),
        ),
        PartPose::offset(-1.75, 0.25, 0.0),
    );
    body.child(
        "left_arm",
        CubeList::new().tex_offs(23, 6).add_box_grow(
            -0.75,
            -0.5,
            -1.0,
            2.0,
            4.0,
            2.0,
            Grow::all(-0.1),
        ),
        PartPose::offset(1.75, 0.25, 0.0),
    );
    body.child(
        "left_wing",
        CubeList::new()
            .tex_offs(16, 14)
            .mirror()
            .add_box(0.0, 0.0, 0.0, 0.0, 5.0, 8.0),
        PartPose::offset(0.5, 1.0, 1.0),
    );
    body.child(
        "right_wing",
        CubeList::new()
            .tex_offs(16, 14)
            .add_box(0.0, 0.0, 0.0, 0.0, 5.0, 8.0),
        PartPose::offset(-0.5, 1.0, 1.0),
    );
    LayerDef::create(mesh, 32, 32)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let head = model.id("head");
    parts[head].y_rot = st.y_rot * DEG_TO_RAD;
    parts[head].x_rot = st.x_rot * DEG_TO_RAD;

    let moving_arm_z_bob = (st.age_ticks * 5.5 * DEG_TO_RAD).cos() * 0.1;
    let right_arm = model.id("right_arm");
    let left_arm = model.id("left_arm");
    parts[right_arm].z_rot = 0.628_318_55 + moving_arm_z_bob;
    parts[left_arm].z_rot = -(0.628_318_55 + moving_arm_z_bob);

    let body = model.id("body");
    if st.extras.charging {
        parts[body].x_rot = 0.0;
        set_arms_charging(
            model,
            parts,
            !st.extras.main_hand.is_empty(),
            !st.extras.off_hand.is_empty(),
            moving_arm_z_bob,
        );
    } else {
        parts[body].x_rot = 0.157_079_64;
    }

    let left_wing = model.id("left_wing");
    let right_wing = model.id("right_wing");
    let wing_y = 1.099_557_4 + (st.age_ticks * 45.836_624 * DEG_TO_RAD).cos() * DEG_TO_RAD * 16.2;
    parts[left_wing].y_rot = wing_y;
    parts[right_wing].y_rot = -wing_y;
    parts[left_wing].x_rot = 0.471_238_88;
    parts[left_wing].z_rot = -0.471_238_88;
    parts[right_wing].x_rot = 0.471_238_88;
    parts[right_wing].z_rot = 0.471_238_88;
}

fn set_arms_charging(
    model: &BakedModel,
    parts: &mut [PartState],
    right_hand_item: bool,
    left_hand_item: bool,
    moving_arm_z_bob: f32,
) {
    let right_arm = model.id("right_arm");
    let left_arm = model.id("left_arm");
    if !right_hand_item && !left_hand_item {
        parts[right_arm].set_rotation(-1.221_730_5, 0.261_799_4, -0.471_238_88 - moving_arm_z_bob);
        parts[left_arm].set_rotation(-1.221_730_5, -0.261_799_4, 0.471_238_88 + moving_arm_z_bob);
    } else {
        if right_hand_item {
            parts[right_arm].set_rotation(
                3.665_191_4,
                0.261_799_4,
                -0.471_238_88 - moving_arm_z_bob,
            );
        }
        if left_hand_item {
            parts[left_arm].set_rotation(
                3.665_191_4,
                -0.261_799_4,
                0.471_238_88 + moving_arm_z_bob,
            );
        }
    }
}
