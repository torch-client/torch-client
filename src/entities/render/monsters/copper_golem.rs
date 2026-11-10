use azalea_registry::builtin::EntityKind;
use bevy::math::{Mat4, Quat, Vec3};

use crate::entities::TexturePath;
use crate::entities::models::monsters::copper_golem;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{Blend, BlockMount, BlockRef, RenderSpec, rotate_around};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::CopperGolem,
        RenderSpec::new(
            "copper_golem",
            copper_golem::layer,
            texture,
            copper_golem::setup_anim,
        ),
    );
    registry.add(
        EntityKind::CopperGolem,
        RenderSpec::new(
            "copper_golem_eyes",
            copper_golem::layer,
            eye_texture,
            copper_golem::setup_anim,
        )
        .with_blend(Blend::Additive),
    );
    registry.add(
        EntityKind::CopperGolem,
        RenderSpec::mounted_block(
            "copper_golem_antenna_block",
            antenna_block,
            BlockMount {
                on_part: true,
                pose: antenna_pose,
                hurt_overlay: false,
            },
        )
        .with_visible(|st| st.extras.block_state.is_some())
        .ignoring_invisibility(),
    );
}

fn antenna_block(st: &EntityState) -> Option<BlockRef> {
    st.extras.block_state.map(BlockRef::State)
}

fn antenna_pose(_st: &EntityState) -> Mat4 {
    Mat4::from_translation(Vec3::new(-0.5, 0.0, -0.5))
        * rotate_around(
            Quat::from_rotation_z(std::f32::consts::PI),
            Vec3::splat(0.5),
        )
        .to_matrix()
}

fn texture(st: &EntityState) -> TexturePath {
    match st.extras.weather_state {
        1 => "entity/copper_golem/copper_golem_exposed".into(),
        2 => "entity/copper_golem/copper_golem_weathered".into(),
        3 => "entity/copper_golem/copper_golem_oxidized".into(),
        _ => "entity/copper_golem/copper_golem".into(),
    }
}

fn eye_texture(st: &EntityState) -> TexturePath {
    match st.extras.weather_state {
        1 => "entity/copper_golem/copper_golem_eyes_exposed".into(),
        2 => "entity/copper_golem/copper_golem_eyes_weathered".into(),
        3 => "entity/copper_golem/copper_golem_eyes_oxidized".into(),
        _ => "entity/copper_golem/copper_golem_eyes".into(),
    }
}
