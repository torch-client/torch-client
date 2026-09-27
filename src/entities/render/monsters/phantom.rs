use azalea_registry::builtin::EntityKind;
use bevy::prelude::{Quat, Vec3};

use crate::entities::models::monsters::phantom;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{Blend, RenderSpec, RootPose, default_root, hook_scale_then_translate};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Phantom,
        RenderSpec::new("phantom", phantom::layer, texture, phantom::setup_anim).with_root(root),
    );
    registry.add(
        EntityKind::Phantom,
        RenderSpec::new(
            "phantom_eyes",
            phantom::layer,
            eyes_texture,
            phantom::setup_anim,
        )
        .with_root(root)
        .with_blend(Blend::Additive),
    );
}

fn texture(_st: &EntityState) -> String {
    "entity/phantom/phantom".to_string()
}

fn eyes_texture(_st: &EntityState) -> String {
    "entity/phantom/phantom_eyes".to_string()
}

fn root(st: &EntityState) -> RootPose {
    let mut pose = default_root(st);
    pose.extra_rotation *= Quat::from_rotation_x(st.x_rot.to_radians());
    let scale = 1.0 + 0.15 * st.extras.size as f32;
    pose.hook = hook_scale_then_translate(Vec3::splat(scale), Vec3::new(0.0, 1.3125, 0.1875));
    pose
}
