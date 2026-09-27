use azalea_registry::builtin::EntityKind;
use bevy::prelude::Quat;

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

fn texture(st: &EntityState) -> String {
    let variant = if st.extras.variant_id == 1 {
        "fox_snow"
    } else {
        "fox"
    };
    let sleep = if st.extras.sleeping { "_sleep" } else { "" };
    let baby = if st.extras.is_baby { "_baby" } else { "" };
    format!("entity/fox/{variant}{sleep}{baby}")
}
