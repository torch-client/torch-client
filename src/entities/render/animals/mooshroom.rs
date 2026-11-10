use azalea_registry::builtin::EntityKind;
use bevy::math::{Mat4, Vec3};

use crate::entities::TexturePath;
use crate::entities::geom::{BakedModel, PartState};
use crate::entities::models::animals::{cow, quadruped};
use crate::entities::registry::Registry;
use crate::entities::render::add_adult_and_baby;
use crate::entities::state::EntityState;
use crate::entities::{Anchors, BlockMount, BlockRef, RenderSpec};
use crate::util::mth::DEG_TO_RAD;

pub fn register(registry: &mut Registry) {
    add_adult_and_baby(
        registry,
        &[EntityKind::Mooshroom],
        RenderSpec::new(
            "mooshroom",
            cow::layer,
            adult_texture,
            quadruped::setup_anim,
        )
        .with_anchors(head_anchor),
        RenderSpec::new(
            "mooshroom_baby",
            cow::baby_layer,
            baby_texture,
            quadruped::setup_anim,
        ),
    );
    let poses: [(&'static str, bool, fn(&EntityState) -> Mat4); 3] = [
        ("mooshroom_mushroom_back", false, back_pose),
        ("mooshroom_mushroom_middle", false, middle_pose),
        ("mooshroom_mushroom_head", true, head_pose),
    ];
    for (name, on_part, pose) in poses {
        registry.add(
            EntityKind::Mooshroom,
            RenderSpec::mounted_block(
                name,
                mushroom,
                BlockMount {
                    on_part,
                    pose,
                    hurt_overlay: true,
                },
            )
            .with_visible(|st| !st.extras.is_baby)
            .outlined_while_invisible(),
        );
    }
}

fn variant(st: &EntityState) -> &'static str {
    if st.extras.variant_id == 1 {
        "brown"
    } else {
        "red"
    }
}

fn adult_texture(st: &EntityState) -> TexturePath {
    format!("entity/cow/mooshroom_{}", variant(st)).into()
}

fn baby_texture(st: &EntityState) -> TexturePath {
    format!("entity/cow/mooshroom_{}_baby", variant(st)).into()
}

fn head_anchor(model: &BakedModel, s: &[PartState], _st: &EntityState) -> Anchors {
    Anchors {
        block: s[model.id("head")].transform().to_matrix(),
        ..Anchors::default()
    }
}

fn mushroom(st: &EntityState) -> Option<BlockRef> {
    use azalea::block::BlockState;
    use azalea_registry::builtin::BlockKind;
    let block = if st.extras.variant_id == 1 {
        BlockKind::BrownMushroom
    } else {
        BlockKind::RedMushroom
    };
    Some(BlockRef::State(BlockState::from(block).id() as u32))
}

fn flip_and_centre() -> Mat4 {
    Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0)) * Mat4::from_translation(Vec3::splat(-0.5))
}

fn back_pose(_st: &EntityState) -> Mat4 {
    Mat4::from_translation(Vec3::new(0.2, -0.35, 0.5))
        * Mat4::from_rotation_y(-48.0 * DEG_TO_RAD)
        * flip_and_centre()
}

fn middle_pose(_st: &EntityState) -> Mat4 {
    Mat4::from_translation(Vec3::new(0.2, -0.35, 0.5))
        * Mat4::from_rotation_y(42.0 * DEG_TO_RAD)
        * Mat4::from_translation(Vec3::new(0.1, 0.0, -0.6))
        * Mat4::from_rotation_y(-48.0 * DEG_TO_RAD)
        * flip_and_centre()
}

fn head_pose(_st: &EntityState) -> Mat4 {
    Mat4::from_translation(Vec3::new(0.0, -0.7, -0.2))
        * Mat4::from_rotation_y(-78.0 * DEG_TO_RAD)
        * flip_and_centre()
}
