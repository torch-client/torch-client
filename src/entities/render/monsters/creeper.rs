use azalea_registry::builtin::EntityKind;
use bevy::prelude::Vec3;

use crate::entities::models::monsters::creeper;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{Blend, RenderSpec, RootPose, default_root};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Creeper,
        RenderSpec::new("creeper", creeper::layer, texture, creeper::setup_anim).with_root(root),
    );
    registry.add(
        EntityKind::Creeper,
        RenderSpec::new(
            "creeper_charge",
            creeper::armor_layer,
            charge_texture,
            creeper::setup_anim,
        )
        .with_root(root)
        .with_visible(is_powered)
        .with_blend(Blend::Additive),
    );
}

fn texture(_st: &EntityState) -> String {
    "entity/creeper/creeper".to_string()
}

fn charge_texture(_st: &EntityState) -> String {
    "entity/creeper/creeper_armor".to_string()
}

fn is_powered(st: &EntityState) -> bool {
    st.extras.powered
}

fn root(st: &EntityState) -> RootPose {
    let mut pose = default_root(st);
    let mut g = st.extras.swell;
    let wobble = 1.0 + (g * 100.0).sin() * g * 0.01;
    g = g.clamp(0.0, 1.0);
    g *= g;
    g *= g;
    let s = (1.0 + g * 0.4) * wobble;
    let hs = (1.0 + g * 0.1) / wobble;
    pose.hook.scale = Vec3::new(s, hs, s);
    pose
}
