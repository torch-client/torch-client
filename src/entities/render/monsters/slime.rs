use azalea_registry::builtin::EntityKind;
use bevy::prelude::Vec3;

use crate::entities::models::monsters::slime;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{Blend, RenderSpec, RootPose, default_root, hook_scale_then_translate};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Slime,
        RenderSpec::new("slime", slime::inner_layer, texture, slime::setup_anim)
            .with_root(slime_root),
    );
    registry.add(
        EntityKind::Slime,
        RenderSpec::new(
            "slime_outer",
            slime::outer_layer,
            texture,
            slime::setup_anim,
        )
        .with_root(slime_root)
        .with_blend(Blend::Translucent),
    );
    registry.add(
        EntityKind::MagmaCube,
        RenderSpec::new(
            "magma_cube",
            slime::magma_cube_layer,
            magma_cube_texture,
            slime::magma_cube_setup_anim,
        )
        .with_root(magma_cube_root),
    );
}

fn texture(_st: &EntityState) -> String {
    "entity/slime/slime".to_string()
}

fn magma_cube_texture(_st: &EntityState) -> String {
    "entity/slime/magmacube".to_string()
}

fn slime_root(st: &EntityState) -> RootPose {
    let mut pose = default_root(st);
    let size = st.extras.size as f32;
    let ss = st.extras.squish / (size * 0.5 + 1.0);
    let w = 1.0 / (ss + 1.0);
    let inner = Vec3::new(w * size, 1.0 / w * size, w * size);
    pose.hook = hook_scale_then_translate(Vec3::splat(0.999), Vec3::new(0.0, 0.001, 0.0));
    pose.hook.scale *= inner;
    pose
}

fn magma_cube_root(st: &EntityState) -> RootPose {
    let mut pose = default_root(st);
    let size = st.extras.size as f32;
    let ss = st.extras.squish / (size * 0.5 + 1.0);
    let w = 1.0 / (ss + 1.0);
    pose.hook.scale = Vec3::new(w * size, 1.0 / w * size, w * size);
    pose
}
