use azalea_registry::builtin::EntityKind;
use bevy::math::Vec3;
use bevy::prelude::Transform;

use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{BlockRef, CameraView, RenderSpec, RootPose};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::FallingBlock,
        RenderSpec::block("falling_block", falling_block).with_root(falling_block_root),
    );
    registry.add(
        EntityKind::Tnt,
        RenderSpec::block("tnt", tnt_block).with_root(tnt_root),
    );
    registry.add(
        EntityKind::BlockDisplay,
        RenderSpec::block("block_display", display_block).with_camera_root(block_display_root),
    );
}

fn falling_block(st: &EntityState) -> Option<BlockRef> {
    st.extras.block_state.map(BlockRef::State)
}

fn falling_block_root(_st: &EntityState) -> RootPose {
    RootPose {
        scale: 1.0,
        hook: Transform::from_xyz(-0.5, 0.0, -0.5),
        ..RootPose::default()
    }
}

fn tnt_block(st: &EntityState) -> Option<BlockRef> {
    Some(BlockRef::State(st.extras.block_state.unwrap_or_else(
        || {
            use azalea::block::BlockState;
            use azalea_registry::builtin::BlockKind;
            BlockState::from(BlockKind::Tnt).id() as u32
        },
    )))
}

fn tnt_root(st: &EntityState) -> RootPose {
    let fuse = st.extras.fuse as f32;
    let scale = if fuse < 10.0 {
        let mut g = (1.0 - fuse / 10.0).clamp(0.0, 1.0);
        g *= g;
        g *= g;
        1.0 + g * 0.3
    } else {
        1.0
    };
    RootPose {
        scale: 1.0,
        hook: Transform {
            translation: Vec3::new(0.0, 0.5, 0.0) + scale * Vec3::splat(-0.5),
            rotation: bevy::math::Quat::IDENTITY,
            scale: Vec3::splat(scale),
        },
        ..RootPose::default()
    }
}

fn display_block(st: &EntityState) -> Option<BlockRef> {
    st.extras.display_block.map(BlockRef::State)
}

fn block_display_root(st: &EntityState, camera: &CameraView) -> RootPose {
    let (hook, _) = crate::entities::display_root(st, camera, bevy::math::Quat::IDENTITY);
    RootPose {
        scale: 1.0,
        hook,
        ..RootPose::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tnt_swell_ramps_over_the_last_ten_ticks() {
        let scale_at = |fuse: i32| {
            let mut st = EntityState::new(0, EntityKind::Tnt);
            st.extras.shared_mut().fuse = fuse;
            tnt_root(&st).hook.scale.x
        };
        assert_eq!(scale_at(80), 1.0);
        assert_eq!(scale_at(10), 1.0);
        assert_eq!(scale_at(0), 1.3);
        assert!(scale_at(5) < 1.03, "{}", scale_at(5));
    }
}
