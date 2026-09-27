use azalea_registry::builtin::EntityKind;
use bevy::prelude::{Quat, Vec3};

use crate::entities::models::animals::feline;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{RenderSpec, RootPose, default_root};

use super::dye;

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Cat,
        RenderSpec::new("cat", feline::cat_layer, texture, feline::setup_anim)
            .with_root(root)
            .with_visible(|st| !st.extras.is_baby),
    );
    registry.add(
        EntityKind::Cat,
        RenderSpec::new(
            "cat_baby",
            feline::baby_layer,
            texture,
            feline::baby_setup_anim,
        )
        .with_root(root)
        .with_visible(|st| st.extras.is_baby),
    );
    registry.add(
        EntityKind::Cat,
        RenderSpec::new(
            "cat_collar",
            feline::cat_collar_layer,
            collar_texture,
            feline::setup_anim,
        )
        .with_root(root)
        .with_visible(|st| st.extras.tame && !st.extras.is_baby)
        .with_tint(|st| dye::dye_tint(st.extras.collar_color)),
    );
    registry.add(
        EntityKind::Cat,
        RenderSpec::new(
            "cat_collar_baby",
            feline::baby_collar_layer,
            collar_texture,
            feline::baby_setup_anim,
        )
        .with_root(root)
        .with_visible(|st| st.extras.tame && st.extras.is_baby)
        .with_tint(|st| dye::dye_tint(st.extras.collar_color)),
    );
}

fn root(st: &EntityState) -> RootPose {
    let mut pose = default_root(st);
    let lie = st.extras.lie_down_amount;
    if lie > 0.0 {
        let mut offset = Vec3::new(0.4 * lie, 0.15 * lie, 0.1 * lie);
        if st.extras.lying_on_sleeping_player {
            offset += Quat::from_rotation_z(rot_lerp(lie, 0.0, 90.0).to_radians())
                * Vec3::new(0.15 * lie, 0.0, 0.0);
        }
        pose.extra_offset += offset;
        pose.extra_rotation *= Quat::from_rotation_z(rot_lerp(lie, 0.0, 90.0).to_radians());
    }
    pose
}

fn rot_lerp(a: f32, from: f32, to: f32) -> f32 {
    from + a * (to - from)
}

fn texture(st: &EntityState) -> String {
    let variant = match &st.extras.variant {
        Some(id) => id.rsplit(':').next().unwrap_or("tabby"),
        None => "tabby",
    };
    if let Some(entry) = crate::util::variants::cat(variant) {
        return entry.texture(st.extras.is_baby).to_string();
    }
    if st.extras.is_baby {
        format!("entity/cat/cat_{variant}_baby")
    } else {
        format!("entity/cat/cat_{variant}")
    }
}

fn collar_texture(st: &EntityState) -> String {
    if st.extras.is_baby {
        "entity/cat/cat_collar_baby".to_string()
    } else {
        "entity/cat/cat_collar".to_string()
    }
}
