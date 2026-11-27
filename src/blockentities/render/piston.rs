use azalea_registry::builtin::BlockEntityKind;
use bevy::math::DVec3;
use bevy::prelude::Vec3;

use crate::blockentities::sim::piston::MovingCopy;
use crate::blockentities::{BeRegistry, BeState, Submit, corner};
use crate::entities::CameraView;

const VIEW_DISTANCE: f32 = 68.0;

pub fn register(registry: &mut BeRegistry) {
    registry.add_submitter(BlockEntityKind::Piston, no_data);
}

fn no_data(_: &BeState, _: &mut Submit) {}

pub fn in_range(pos: [i32; 3], camera: &CameraView) -> bool {
    camera.distance_squared_to(crate::util::block_centre(pos).to_array())
        < VIEW_DISTANCE * VIEW_DISTANCE
}

pub fn submit_copy(pos: [i32; 3], copy: &MovingCopy, progress: f32, out: &mut Submit) {
    let feet = DVec3::from_array(corner(pos)) + DVec3::new(0.5, 0.0, 0.5);
    let [block, base] = copy.moving.draws(progress);
    let base = base.filter(|_| !copy.skip_base);
    for (state, offset) in [block, base].into_iter().flatten() {
        out.block(
            state,
            (feet + Vec3::from_array(offset).as_dvec3()).to_array(),
        );
    }
}

#[cfg(test)]
pub(super) fn test_states(_out: &mut Vec<BeState>) {}
