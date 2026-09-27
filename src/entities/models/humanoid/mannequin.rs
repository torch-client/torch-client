use crate::entities::geom::{BakedModel, Grow, LayerDef, PartState};
use crate::entities::state::EntityState;
use crate::renderer::anim::arm_poses;

use super::humanoid::{self, ArmPoses, Limbs};

pub fn wide_layer() -> LayerDef {
    LayerDef::create(humanoid::player_mesh(Grow::NONE, false), 64, 64)
}

fn avatar_arm_poses(st: &EntityState) -> ArmPoses {
    let using_hand = if st.extras.using_item {
        Some(st.extras.use_offhand)
    } else {
        None
    };
    let (main, off) = arm_poses(
        &st.extras.main_hand,
        &st.extras.off_hand,
        using_hand,
        st.attack_time > 0.0,
    );
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

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let mut l = Limbs::load(model, parts);
    humanoid::pose_humanoid(&mut l, st, avatar_arm_poses(st));
    l.store(parts);
}
