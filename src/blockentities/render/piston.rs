use azalea_registry::builtin::BlockEntityKind;
use bevy::math::DVec3;
use bevy::prelude::Vec3;

use crate::blockentities::feed::BlockEntityData;
use crate::blockentities::sim::Anim;
use crate::blockentities::{BeRegistry, BeState, Submit, corner};

const VIEW_DISTANCE: f32 = 68.0;

pub fn register(registry: &mut BeRegistry) {
    registry.add_submitter(BlockEntityKind::Piston, submit);
}

fn submit(st: &BeState, out: &mut Submit) {
    let (BlockEntityData::Piston(moving), Anim::Piston(progress)) = (&*st.data, &st.anim) else {
        return;
    };
    if st
        .camera
        .distance_squared_to(crate::util::block_centre(st.pos).to_array())
        >= VIEW_DISTANCE * VIEW_DISTANCE
    {
        return;
    }
    let feet = DVec3::from_array(corner(st.pos)) + DVec3::new(0.5, 0.0, 0.5);
    for (state, offset) in moving.draws(progress.at(st.partial)).into_iter().flatten() {
        out.block(
            state,
            (feet + Vec3::from_array(offset).as_dvec3()).to_array(),
        );
    }
}

#[cfg(test)]
pub(super) fn test_states(_out: &mut Vec<BeState>) {}
