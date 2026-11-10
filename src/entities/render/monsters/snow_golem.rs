use azalea_registry::builtin::EntityKind;
use bevy::math::{Mat4, Vec3};

use crate::entities::TexturePath;
use crate::entities::geom::{BakedModel, PartState};
use crate::entities::models::monsters::snow_golem;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{Anchors, BlockMount, BlockRef, RenderSpec};
use crate::util::mth::DEG_TO_RAD;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::SnowGolem,
        RenderSpec::new(
            "snow_golem",
            snow_golem::layer,
            texture,
            snow_golem::setup_anim,
        )
        .with_anchors(head_anchor),
    );
    registry.add(
        EntityKind::SnowGolem,
        RenderSpec::mounted_block(
            "snow_golem_pumpkin",
            pumpkin,
            BlockMount {
                on_part: true,
                pose: pumpkin_pose,
                hurt_overlay: true,
            },
        )
        .with_visible(|st| st.extras.has_pumpkin)
        .outlined_while_invisible(),
    );
}

fn texture(_st: &EntityState) -> TexturePath {
    "entity/snow_golem/snow_golem".into()
}

fn head_anchor(model: &BakedModel, s: &[PartState], _st: &EntityState) -> Anchors {
    Anchors {
        block: s[model.id("head")].transform().to_matrix(),
        ..Anchors::default()
    }
}

fn pumpkin(st: &EntityState) -> Option<BlockRef> {
    use azalea::block::BlockState;
    use azalea_registry::builtin::BlockKind;
    st.extras
        .has_pumpkin
        .then(|| BlockRef::State(BlockState::from(BlockKind::CarvedPumpkin).id() as u32))
}

fn pumpkin_pose(_st: &EntityState) -> Mat4 {
    const SCALE: f32 = 0.625;
    Mat4::from_translation(Vec3::new(0.0, -0.34375, 0.0))
        * Mat4::from_rotation_y(180.0 * DEG_TO_RAD)
        * Mat4::from_scale(Vec3::new(SCALE, -SCALE, -SCALE))
        * Mat4::from_translation(Vec3::splat(-0.5))
}
