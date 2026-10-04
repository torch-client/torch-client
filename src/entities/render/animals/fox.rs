use azalea_registry::builtin::EntityKind;
use bevy::prelude::Quat;

use crate::entities::TexturePath;
use crate::entities::models::animals::fox;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{RenderSpec, RootPose, default_root};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Fox,
        RenderSpec::new("fox", fox::layer, texture, fox::setup_anim)
            .with_root(root)
            .with_visible(|st| !st.extras.is_baby),
    );
    registry.add(
        EntityKind::Fox,
        RenderSpec::new("fox_baby", fox::baby_layer, texture, fox::baby_setup_anim)
            .with_root(root)
            .with_visible(|st| st.extras.is_baby),
    );
}

fn root(st: &EntityState) -> RootPose {
    let mut pose = default_root(st);
    if st.extras.pouncing || st.extras.faceplanted {
        pose.extra_rotation *= Quat::from_rotation_x((-st.x_rot).to_radians());
    }
    pose
}

fn texture(st: &EntityState) -> TexturePath {
    let snow = st.extras.variant_id == 1;
    match (snow, st.extras.sleeping, st.extras.is_baby) {
        (false, false, false) => "entity/fox/fox",
        (false, false, true) => "entity/fox/fox_baby",
        (false, true, false) => "entity/fox/fox_sleep",
        (false, true, true) => "entity/fox/fox_sleep_baby",
        (true, false, false) => "entity/fox/fox_snow",
        (true, false, true) => "entity/fox/fox_snow_baby",
        (true, true, false) => "entity/fox/fox_snow_sleep",
        (true, true, true) => "entity/fox/fox_snow_sleep_baby",
    }
    .into()
}
