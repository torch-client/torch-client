use azalea_registry::builtin::EntityKind;
use bevy::prelude::Quat;

use crate::entities::models::monsters::iron_golem;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{RenderSpec, RootPose, default_root};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::IronGolem,
        RenderSpec::new(
            "iron_golem",
            iron_golem::layer,
            texture,
            iron_golem::setup_anim,
        )
        .with_root(root),
    );
    registry.add(
        EntityKind::IronGolem,
        RenderSpec::new(
            "iron_golem_crackiness",
            iron_golem::layer,
            crackiness_texture,
            iron_golem::setup_anim,
        )
        .with_root(root)
        .with_visible(is_cracked),
    );
}

fn texture(_st: &EntityState) -> String {
    "entity/iron_golem/iron_golem".to_string()
}

fn crackiness_texture(st: &EntityState) -> String {
    match st.extras.crackiness {
        3 => "entity/iron_golem/iron_golem_crackiness_high".to_string(),
        2 => "entity/iron_golem/iron_golem_crackiness_medium".to_string(),
        _ => "entity/iron_golem/iron_golem_crackiness_low".to_string(),
    }
}

fn is_cracked(st: &EntityState) -> bool {
    st.extras.crackiness != 0
}

fn root(st: &EntityState) -> RootPose {
    let mut pose = default_root(st);
    if st.walk_speed as f64 >= 0.01 {
        const PERIOD: f32 = 13.0;
        let wp = st.walk_pos + 6.0;
        let triangle_wave = ((wp % PERIOD - 6.5).abs() - 3.25) / 3.25;
        pose.extra_rotation *= Quat::from_rotation_z((6.5 * triangle_wave).to_radians());
    }
    pose
}
