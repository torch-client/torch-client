#![allow(clippy::approx_constant)]

use azalea_registry::builtin::EntityKind;

use crate::entities::TexturePath;
use crate::entities::models::monsters::warden;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{Blend, RenderSpec};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Warden,
        RenderSpec::new("warden", warden::layer, texture, warden::setup_anim),
    );
    registry.add(
        EntityKind::Warden,
        RenderSpec::new(
            "warden_bioluminescent",
            warden::bioluminescent_layer,
            bioluminescent_texture,
            warden::setup_anim,
        )
        .with_blend(Blend::Translucent),
    );
    registry.add(
        EntityKind::Warden,
        RenderSpec::new(
            "warden_pulsating_spots_1",
            warden::pulsating_spots_layer,
            pulsating_spots_1_texture,
            warden::setup_anim,
        )
        .with_blend(Blend::Translucent)
        .with_visible(spots_1_visible)
        .with_tint(spots_1_tint),
    );
    registry.add(
        EntityKind::Warden,
        RenderSpec::new(
            "warden_pulsating_spots_2",
            warden::pulsating_spots_layer,
            pulsating_spots_2_texture,
            warden::setup_anim,
        )
        .with_blend(Blend::Translucent)
        .with_visible(spots_2_visible)
        .with_tint(spots_2_tint),
    );
    registry.add(
        EntityKind::Warden,
        RenderSpec::new(
            "warden_tendrils",
            warden::tendrils_layer,
            texture,
            warden::setup_anim,
        )
        .with_blend(Blend::Translucent)
        .with_visible(tendrils_visible)
        .with_tint(tendrils_tint),
    );
    registry.add(
        EntityKind::Warden,
        RenderSpec::new(
            "warden_heart",
            warden::heart_layer,
            heart_texture,
            warden::setup_anim,
        )
        .with_blend(Blend::Translucent)
        .with_visible(heart_visible)
        .with_tint(heart_tint),
    );
}

fn texture(_st: &EntityState) -> TexturePath {
    "entity/warden/warden".into()
}

fn bioluminescent_texture(_st: &EntityState) -> TexturePath {
    "entity/warden/warden_bioluminescent_layer".into()
}

fn pulsating_spots_1_texture(_st: &EntityState) -> TexturePath {
    "entity/warden/warden_pulsating_spots_1".into()
}

fn pulsating_spots_2_texture(_st: &EntityState) -> TexturePath {
    "entity/warden/warden_pulsating_spots_2".into()
}

fn heart_texture(_st: &EntityState) -> TexturePath {
    "entity/warden/warden_heart".into()
}

fn spots_1_alpha(st: &EntityState) -> f32 {
    ((st.age_ticks * 0.045).cos() * 0.25).max(0.0)
}

fn spots_2_alpha(st: &EntityState) -> f32 {
    ((st.age_ticks * 0.045 + 3.1415927).cos() * 0.25).max(0.0)
}

fn alpha_visible(alpha: f32) -> bool {
    alpha > 1.0e-5
}

fn spots_1_visible(st: &EntityState) -> bool {
    alpha_visible(spots_1_alpha(st))
}

fn spots_2_visible(st: &EntityState) -> bool {
    alpha_visible(spots_2_alpha(st))
}

fn tendrils_visible(st: &EntityState) -> bool {
    alpha_visible(st.extras.tendril_animation)
}

fn heart_visible(st: &EntityState) -> bool {
    alpha_visible(st.extras.heart_animation)
}

fn white(alpha: f32) -> [f32; 4] {
    const STEPS: f32 = 32.0;
    [1.0, 1.0, 1.0, (alpha * STEPS).round() / STEPS]
}

fn spots_1_tint(st: &EntityState) -> [f32; 4] {
    white(spots_1_alpha(st))
}

fn spots_2_tint(st: &EntityState) -> [f32; 4] {
    white(spots_2_alpha(st))
}

fn tendrils_tint(st: &EntityState) -> [f32; 4] {
    white(st.extras.tendril_animation)
}

fn heart_tint(st: &EntityState) -> [f32; 4] {
    white(st.extras.heart_animation)
}
