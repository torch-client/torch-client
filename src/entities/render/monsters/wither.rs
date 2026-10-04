use azalea_registry::builtin::EntityKind;
use bevy::prelude::Vec3;

use crate::entities::TexturePath;
use crate::entities::models::monsters::wither;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{Blend, RenderSpec, RootPose, default_root};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Wither,
        RenderSpec::new("wither", wither::layer, texture, wither::setup_anim).with_root(root),
    );
    registry.add(
        EntityKind::Wither,
        RenderSpec::new(
            "wither_armor",
            wither::armor_layer,
            armor_texture,
            wither::setup_anim,
        )
        .with_root(root)
        .with_visible(is_powered)
        .with_blend(Blend::Additive),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    let ticks = st.extras.invulnerable_ticks.floor() as i32;
    if ticks > 0 && (ticks > 80 || ticks / 5 % 2 != 1) {
        "entity/wither/wither_invulnerable".into()
    } else {
        "entity/wither/wither".into()
    }
}

fn armor_texture(_st: &EntityState) -> TexturePath {
    "entity/wither/wither_armor".into()
}

fn is_powered(st: &EntityState) -> bool {
    st.extras.powered
}

fn root(st: &EntityState) -> RootPose {
    let mut pose = default_root(st);
    let mut scale = 2.0;
    if st.extras.invulnerable_ticks > 0.0 {
        scale -= st.extras.invulnerable_ticks / 220.0 * 0.5;
    }
    pose.hook.scale = Vec3::splat(scale);
    pose
}
