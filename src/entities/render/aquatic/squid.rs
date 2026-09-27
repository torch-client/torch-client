use azalea_registry::builtin::EntityKind;
use bevy::prelude::*;

use crate::entities::models::aquatic::squid;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{RenderSpec, RootPose};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Squid,
        RenderSpec::new(
            "squid",
            squid::squid_layer,
            squid_texture,
            squid::setup_anim,
        )
        .with_root(root)
        .with_visible(is_adult),
    );
    registry.add(
        EntityKind::Squid,
        RenderSpec::new(
            "squid_baby",
            squid::baby_squid_layer,
            squid_baby_texture,
            squid::setup_anim,
        )
        .with_root(root)
        .with_visible(is_baby),
    );
    registry.add(
        EntityKind::GlowSquid,
        RenderSpec::new(
            "glow_squid",
            squid::squid_layer,
            glow_squid_texture,
            squid::setup_anim,
        )
        .with_root(root)
        .with_visible(is_adult),
    );
    registry.add(
        EntityKind::GlowSquid,
        RenderSpec::new(
            "glow_squid_baby",
            squid::baby_squid_layer,
            glow_squid_baby_texture,
            squid::setup_anim,
        )
        .with_root(root)
        .with_visible(is_baby),
    );
}

fn is_adult(st: &EntityState) -> bool {
    !st.extras.is_baby
}

fn is_baby(st: &EntityState) -> bool {
    st.extras.is_baby
}

fn squid_texture(_st: &EntityState) -> String {
    "entity/squid/squid".to_string()
}

fn squid_baby_texture(_st: &EntityState) -> String {
    "entity/squid/squid_baby".to_string()
}

fn glow_squid_texture(_st: &EntityState) -> String {
    "entity/squid/glow_squid".to_string()
}

fn glow_squid_baby_texture(_st: &EntityState) -> String {
    "entity/squid/glow_squid_baby".to_string()
}

fn root(st: &EntityState) -> RootPose {
    let lift = if st.extras.is_baby { 0.25 } else { 0.5 };
    let drop = if st.extras.is_baby { -0.6 } else { -1.2 };
    RootPose {
        world_offset: Vec3::new(0.0, lift * st.scale, 0.0),
        rotation: Quat::from_rotation_y((180.0 - st.body_rot).to_radians())
            * Quat::from_rotation_x(st.extras.squid_x_body_rot.to_radians())
            * Quat::from_rotation_y(st.extras.squid_z_body_rot.to_radians()),
        scale: st.scale,
        extra_offset: Vec3::new(0.0, drop, 0.0),
        ..RootPose::default()
    }
}
