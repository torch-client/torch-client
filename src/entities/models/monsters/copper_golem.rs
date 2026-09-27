use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::keyframe;
use crate::entities::models::monsters::copper_golem_anim;
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

const MAX_WALK_ANIMATION_SPEED: f32 = 2.0;
const WALK_ANIMATION_SCALE_FACTOR: f32 = 2.5;
const Z_FIGHT_MITIGATION: f32 = 0.015;

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new().transformed(|pose| pose.translated(0.0, 24.0, 0.0));
    let root = mesh.root();
    let body = root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 15)
            .add_box_grow(-4.0, -6.0, -3.0, 8.0, 6.0, 6.0, Grow::NONE),
        PartPose::offset(0.0, -5.0, 0.0),
    );
    body.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box_grow(
                -4.0,
                -5.0,
                -5.0,
                8.0,
                5.0,
                10.0,
                Grow::all(Z_FIGHT_MITIGATION),
            )
            .tex_offs(56, 0)
            .add_box_grow(-1.0, -2.0, -6.0, 2.0, 3.0, 2.0, Grow::NONE)
            .tex_offs(37, 8)
            .add_box_grow(
                -1.0,
                -9.0,
                -1.0,
                2.0,
                4.0,
                2.0,
                Grow::all(-Z_FIGHT_MITIGATION),
            )
            .tex_offs(37, 0)
            .add_box_grow(
                -2.0,
                -13.0,
                -2.0,
                4.0,
                4.0,
                4.0,
                Grow::all(-Z_FIGHT_MITIGATION),
            ),
        PartPose::offset(0.0, -6.0, 0.0),
    );
    body.child(
        "right_arm",
        CubeList::new()
            .tex_offs(36, 16)
            .add_box_grow(-3.0, -1.0, -2.0, 3.0, 10.0, 4.0, Grow::NONE),
        PartPose::offset(-4.0, -6.0, 0.0),
    );
    body.child(
        "left_arm",
        CubeList::new()
            .tex_offs(50, 16)
            .add_box_grow(0.0, -1.0, -2.0, 3.0, 10.0, 4.0, Grow::NONE),
        PartPose::offset(4.0, -6.0, 0.0),
    );
    root.child(
        "right_leg",
        CubeList::new()
            .tex_offs(0, 27)
            .add_box_grow(-4.0, 0.0, -2.0, 4.0, 5.0, 4.0, Grow::NONE),
        PartPose::offset(0.0, -5.0, 0.0),
    );
    root.child(
        "left_leg",
        CubeList::new()
            .tex_offs(16, 27)
            .add_box_grow(0.0, 0.0, -2.0, 4.0, 5.0, 4.0, Grow::NONE),
        PartPose::offset(0.0, -5.0, 0.0),
    );
    LayerDef::create(mesh, 64, 64)
}

fn pose_held_item_arms_if_still(parts: &mut [PartState], right_arm: usize, left_arm: usize) {
    parts[right_arm].x_rot = parts[right_arm].x_rot.min(-0.872_664_63);
    parts[left_arm].x_rot = parts[left_arm].x_rot.min(-0.872_664_63);
    parts[right_arm].y_rot = parts[right_arm].y_rot.min(-0.113_446_4);
    parts[left_arm].y_rot = parts[left_arm].y_rot.max(0.113_446_4);
    parts[right_arm].z_rot = parts[right_arm].z_rot.min(-0.064_577_185);
    parts[left_arm].z_rot = parts[left_arm].z_rot.max(0.064_577_185);
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let head = model.id("head");
    parts[head].x_rot = st.x_rot * DEG_TO_RAD;
    parts[head].y_rot = st.y_rot * DEG_TO_RAD;

    let holding = !st.extras.main_hand.is_empty() || !st.extras.off_hand.is_empty();
    if holding {
        keyframe::apply_walk(
            &copper_golem_anim::COPPER_GOLEM_WALK_ITEM,
            model,
            parts,
            st.walk_pos,
            st.walk_speed,
            MAX_WALK_ANIMATION_SPEED,
            WALK_ANIMATION_SCALE_FACTOR,
        );
        pose_held_item_arms_if_still(parts, model.id("right_arm"), model.id("left_arm"));
    } else {
        keyframe::apply_walk(
            &copper_golem_anim::COPPER_GOLEM_WALK,
            model,
            parts,
            st.walk_pos,
            st.walk_speed,
            MAX_WALK_ANIMATION_SPEED,
            WALK_ANIMATION_SCALE_FACTOR,
        );
    }

    let seconds = st.age_ticks / 20.0;
    let idle = &copper_golem_anim::COPPER_GOLEM_IDLE;
    keyframe::apply(idle, model, parts, seconds % idle.length_seconds, 1.0);

    let interaction = match st.extras.copper_golem_state {
        1 => Some(&copper_golem_anim::COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET),
        2 => Some(&copper_golem_anim::COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET),
        3 => Some(&copper_golem_anim::COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP),
        4 => Some(&copper_golem_anim::COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP),
        _ => None,
    };
    if let Some(animation) = interaction {
        keyframe::apply(animation, model, parts, seconds, 1.0);
    }
}
