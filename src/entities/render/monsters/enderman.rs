use azalea_registry::builtin::EntityKind;
use bevy::math::{Mat4, Vec3};

use crate::entities::TexturePath;
use crate::entities::models::monsters::enderman;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{Blend, BlockMount, BlockRef, RenderSpec};
use crate::util::mth::DEG_TO_RAD;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Enderman,
        RenderSpec::new("enderman", enderman::layer, texture, enderman::setup_anim),
    );
    registry.add(
        EntityKind::Enderman,
        RenderSpec::new(
            "enderman_eyes",
            enderman::layer,
            eyes_texture,
            enderman::setup_anim,
        )
        .with_blend(Blend::Additive)
        .ignoring_invisibility(),
    );
    registry.add(
        EntityKind::Enderman,
        RenderSpec::mounted_block(
            "enderman_carried_block",
            carried_block,
            BlockMount {
                on_part: false,
                pose: carried_block_pose,
                hurt_overlay: false,
            },
        )
        .with_visible(|st| st.extras.carried_block.is_some())
        .ignoring_invisibility(),
    );
}

fn texture(_st: &EntityState) -> TexturePath {
    "entity/enderman/enderman".into()
}

fn eyes_texture(_st: &EntityState) -> TexturePath {
    "entity/enderman/enderman_eyes".into()
}

fn carried_block(st: &EntityState) -> Option<BlockRef> {
    st.extras.carried_block.map(BlockRef::State)
}

fn carried_block_pose(_st: &EntityState) -> Mat4 {
    Mat4::from_translation(Vec3::new(0.0, 0.6875, -0.75))
        * Mat4::from_rotation_x(20.0 * DEG_TO_RAD)
        * Mat4::from_rotation_y(45.0 * DEG_TO_RAD)
        * Mat4::from_translation(Vec3::new(0.25, 0.1875, 0.25))
        * Mat4::from_scale(Vec3::new(-0.5, -0.5, 0.5))
        * Mat4::from_rotation_y(90.0 * DEG_TO_RAD)
}
