use azalea_registry::builtin::EntityKind;
use bevy::prelude::{Quat, Transform};

use crate::entities::TexturePath;
use crate::entities::models::monsters::dragon;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{Blend, RenderSpec, RootPose};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::EnderDragon,
        RenderSpec::new("ender_dragon", dragon::layer, texture, dragon::setup_anim).with_root(root),
    );
    registry.add(
        EntityKind::EnderDragon,
        RenderSpec::new(
            "ender_dragon_eyes",
            dragon::layer,
            eyes_texture,
            dragon::setup_anim,
        )
        .with_root(root)
        .with_blend(Blend::Additive),
    );
}

fn texture(_st: &EntityState) -> TexturePath {
    "entity/enderdragon/dragon".into()
}

fn eyes_texture(_st: &EntityState) -> TexturePath {
    "entity/enderdragon/dragon_eyes".into()
}

fn root(st: &EntityState) -> RootPose {
    RootPose {
        rotation: Quat::from_rotation_y((-st.body_rot).to_radians()),
        scale: st.scale,
        hook: Transform::from_xyz(0.0, 0.0, 1.0),
        ..RootPose::default()
    }
}
