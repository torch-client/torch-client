use azalea_registry::builtin::EntityKind;
use bevy::math::{Mat4, Quat, Vec3};

use crate::entities::models::objects::projectile;
use crate::entities::registry::Registry;
use crate::entities::render::objects::{hook_for, mirror};
use crate::entities::state::EntityState;
use crate::entities::{Blend, LightMode, RenderSpec, RootPose};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Arrow,
        RenderSpec::new(
            "arrow",
            projectile::arrow_layer,
            arrow_texture,
            projectile::arrow_setup_anim,
        )
        .with_root(arrow_root),
    );
    registry.add(
        EntityKind::SpectralArrow,
        RenderSpec::new(
            "spectral_arrow",
            projectile::arrow_layer,
            spectral_arrow_texture,
            projectile::arrow_setup_anim,
        )
        .with_root(arrow_root),
    );
    registry.add(
        EntityKind::Trident,
        RenderSpec::new(
            "trident",
            projectile::trident_layer,
            trident_texture,
            projectile::no_anim,
        )
        .with_root(trident_root),
    );
    registry.add(
        EntityKind::LlamaSpit,
        RenderSpec::new(
            "llama_spit",
            projectile::llama_spit_layer,
            llama_spit_texture,
            projectile::no_anim,
        )
        .with_root(llama_spit_root),
    );
    registry.add(
        EntityKind::ShulkerBullet,
        RenderSpec::new(
            "shulker_bullet",
            projectile::shulker_bullet_layer,
            shulker_bullet_texture,
            projectile::shulker_bullet_setup_anim,
        )
        .with_root(shulker_bullet_root),
    );
    registry.add(
        EntityKind::ShulkerBullet,
        RenderSpec::new(
            "shulker_bullet_glow",
            projectile::shulker_bullet_layer,
            shulker_bullet_texture,
            projectile::shulker_bullet_setup_anim,
        )
        .with_root(shulker_bullet_glow_root)
        .with_blend(Blend::Translucent)
        .with_tint(shulker_bullet_glow_tint),
    );
    registry.add_many(
        &[EntityKind::WindCharge, EntityKind::BreezeWindCharge],
        RenderSpec::new(
            "wind_charge",
            projectile::wind_charge_layer,
            wind_charge_texture,
            projectile::wind_charge_setup_anim,
        )
        .with_root(wind_charge_root)
        .with_blend(Blend::Translucent)
        .with_light(LightMode::Flat),
    );
    registry.add(
        EntityKind::WitherSkull,
        RenderSpec::new(
            "wither_skull",
            projectile::wither_skull_layer,
            wither_skull_texture,
            projectile::wither_skull_setup_anim,
        )
        .with_root(wither_skull_root)
        .with_blend(Blend::Translucent),
    );
}

fn arrow_texture(st: &EntityState) -> String {
    if st.extras.arrow_tipped {
        "entity/projectiles/arrow_tipped".to_string()
    } else {
        "entity/projectiles/arrow".to_string()
    }
}

fn spectral_arrow_texture(_st: &EntityState) -> String {
    "entity/projectiles/arrow_spectral".to_string()
}

fn trident_texture(_st: &EntityState) -> String {
    "entity/trident/trident".to_string()
}

fn llama_spit_texture(_st: &EntityState) -> String {
    "entity/llama/llama_spit".to_string()
}

fn shulker_bullet_texture(_st: &EntityState) -> String {
    "entity/shulker/spark".to_string()
}

fn wind_charge_texture(_st: &EntityState) -> String {
    "entity/projectiles/wind_charge".to_string()
}

fn wither_skull_texture(st: &EntityState) -> String {
    if st.extras.skull_dangerous {
        "entity/wither/wither_invulnerable".to_string()
    } else {
        "entity/wither/wither".to_string()
    }
}

fn arrow_root(st: &EntityState) -> RootPose {
    RootPose {
        world_offset: Vec3::ZERO,
        rotation: Quat::from_rotation_y((st.body_rot - 90.0).to_radians()),
        scale: 1.0,
        extra_rotation: Quat::from_rotation_z(st.x_rot.to_radians()),
        extra_offset: Vec3::ZERO,
        hook: hook_for(Mat4::IDENTITY),
    }
}

fn trident_root(st: &EntityState) -> RootPose {
    RootPose {
        world_offset: Vec3::ZERO,
        rotation: Quat::from_rotation_y((st.body_rot - 90.0).to_radians()),
        scale: 1.0,
        extra_rotation: Quat::from_rotation_z((st.x_rot + 90.0).to_radians()),
        extra_offset: Vec3::ZERO,
        hook: hook_for(Mat4::IDENTITY),
    }
}

fn llama_spit_root(st: &EntityState) -> RootPose {
    RootPose {
        world_offset: Vec3::new(0.0, 0.15, 0.0),
        ..arrow_root(st)
    }
}

fn shulker_bullet_root(st: &EntityState) -> RootPose {
    shulker_bullet_pose(st, 0.5)
}

fn shulker_bullet_glow_root(st: &EntityState) -> RootPose {
    shulker_bullet_pose(st, 0.75)
}

fn shulker_bullet_pose(st: &EntityState, scale: f32) -> RootPose {
    let tc = st.age_ticks;
    RootPose {
        world_offset: Vec3::new(0.0, 0.15, 0.0),
        rotation: Quat::from_rotation_y(((tc * 0.1).sin() * 180.0).to_radians()),
        scale: 1.0,
        extra_rotation: Quat::from_rotation_x(((tc * 0.1).cos() * 180.0).to_radians())
            * Quat::from_rotation_z(((tc * 0.15).sin() * 360.0).to_radians()),
        extra_offset: Vec3::ZERO,
        hook: hook_for(mirror() * Mat4::from_scale(Vec3::splat(scale))),
    }
}

fn shulker_bullet_glow_tint(_st: &EntityState) -> [f32; 4] {
    [1.0, 1.0, 1.0, 39.0 / 255.0]
}

fn wind_charge_root(_st: &EntityState) -> RootPose {
    RootPose {
        scale: 1.0,
        hook: hook_for(Mat4::IDENTITY),
        ..RootPose::default()
    }
}

fn wither_skull_root(_st: &EntityState) -> RootPose {
    RootPose {
        scale: 1.0,
        hook: hook_for(mirror()),
        ..RootPose::default()
    }
}
