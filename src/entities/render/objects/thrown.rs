use azalea_registry::builtin::EntityKind;
use bevy::math::{Quat, Vec3};
use bevy::prelude::Transform;

use crate::entities::TexturePath;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{CameraView, RenderSpec, RootPose, no_inner};

const THROWN: &[EntityKind] = &[
    EntityKind::Egg,
    EntityKind::Snowball,
    EntityKind::EnderPearl,
    EntityKind::ExperienceBottle,
    EntityKind::SplashPotion,
    EntityKind::LingeringPotion,
    EntityKind::EyeOfEnder,
    EntityKind::SmallFireball,
    EntityKind::Fireball,
    EntityKind::FireworkRocket,
];

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Item,
        RenderSpec::item("item_entity", dropped_item, "ground", no_inner)
            .with_root(item_entity_root)
            .as_item_cluster(dropped_count, dropped_seed),
    );
    registry.add_many(
        THROWN,
        RenderSpec::item("thrown_item", thrown_item, "ground", no_inner)
            .with_camera_root(thrown_root),
    );
    registry.add(
        EntityKind::ItemDisplay,
        RenderSpec::item("item_display", display_item, "none", no_inner)
            .with_camera_root(item_display_root),
    );
    registry.add(
        EntityKind::OminousItemSpawner,
        RenderSpec::item("ominous_item_spawner", spawner_item, "ground", no_inner)
            .with_root(spawner_root),
    );
    registry.add_many(
        &[EntityKind::ItemFrame, EntityKind::GlowItemFrame],
        RenderSpec::item("item_frame_item", frame_item, "fixed", frame_item_inner)
            .with_root(super::frame::frame_root)
            .with_visible(frame_item_visible)
            .ignoring_invisibility(),
    );
}

fn item_key(item: &crate::renderer::anim::HeldItem) -> TexturePath {
    let has_layers = item
        .layers
        .as_ref()
        .is_some_and(|layers| !layers.is_empty());
    if has_layers || item.tint.is_some() {
        item.model_key().into()
    } else {
        item.id.into()
    }
}

fn dropped_item(st: &EntityState) -> TexturePath {
    item_key(&st.extras.item)
}

fn dropped_count(st: &EntityState) -> u32 {
    st.extras.item.count as u32
}

fn dropped_seed(st: &EntityState) -> i64 {
    st.extras
        .item
        .id
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32)) as i64
}

fn item_entity_root(st: &EntityState) -> RootPose {
    let bob_offset =
        crate::util::javarandom::JavaRandom::new(st.id as i64).next_f32() * std::f32::consts::TAU;
    let bob = (st.age_ticks / 10.0 + bob_offset).sin() * 0.1 + 0.1;
    let spin = st.age_ticks / 20.0 + bob_offset;
    RootPose {
        scale: 1.0,
        hook: Transform {
            translation: Vec3::new(0.0, bob, 0.0),
            rotation: Quat::from_rotation_y(spin),
            scale: Vec3::ONE,
        },
        ..RootPose::default()
    }
}

fn thrown_scale(kind: EntityKind) -> f32 {
    match kind {
        EntityKind::Fireball => 3.0,
        EntityKind::SmallFireball => 0.75,
        _ => 1.0,
    }
}

fn thrown_item(st: &EntityState) -> TexturePath {
    if !st.extras.item.is_empty() {
        return item_key(&st.extras.item);
    }
    match st.kind {
        EntityKind::Egg => "egg",
        EntityKind::Snowball => "snowball",
        EntityKind::EnderPearl => "ender_pearl",
        EntityKind::ExperienceBottle => "experience_bottle",
        EntityKind::SplashPotion => "splash_potion",
        EntityKind::LingeringPotion => "lingering_potion",
        EntityKind::EyeOfEnder => "ender_eye",
        EntityKind::SmallFireball | EntityKind::Fireball => "fire_charge",
        EntityKind::FireworkRocket => "firework_rocket",
        _ => "",
    }
    .into()
}

fn thrown_root(st: &EntityState, camera: &CameraView) -> RootPose {
    RootPose {
        scale: 1.0,
        hook: Transform {
            translation: Vec3::ZERO,
            rotation: camera.orientation,
            scale: Vec3::splat(thrown_scale(st.kind)),
        },
        ..RootPose::default()
    }
}

fn display_item(st: &EntityState) -> TexturePath {
    item_key(&st.extras.item)
}

fn item_display_root(st: &EntityState, camera: &CameraView) -> RootPose {
    let inner = Quat::from_rotation_y(std::f32::consts::PI);
    let (mut hook, leftover) = crate::entities::display_root(st, camera, inner);
    if let Some(below) = leftover {
        hook.rotation *= below;
    }
    RootPose {
        scale: 1.0,
        hook,
        ..RootPose::default()
    }
}

fn spawner_item(st: &EntityState) -> TexturePath {
    item_key(&st.extras.item)
}

fn spawner_root(st: &EntityState) -> RootPose {
    const TICKS_SCALING: f32 = 50.0;
    const ROTATION_SPEED: f32 = 40.0;
    let scale = (st.age_ticks / TICKS_SCALING).min(1.0);
    RootPose {
        scale: 1.0,
        hook: Transform {
            translation: Vec3::ZERO,
            rotation: Quat::from_rotation_y((st.age_ticks * ROTATION_SPEED).to_radians()),
            scale: Vec3::splat(scale),
        },
        ..RootPose::default()
    }
}

fn frame_item(st: &EntityState) -> TexturePath {
    item_key(&st.extras.item)
}

fn frame_item_visible(st: &EntityState) -> bool {
    !st.extras.item.is_empty() && !super::frame::has_map(st)
}

fn frame_item_inner(st: &EntityState) -> Transform {
    Transform {
        translation: Vec3::new(0.0, 0.0, super::frame::contents_depth(st)),
        rotation: Quat::from_rotation_z((st.extras.item_rotation as f32 * 45.0).to_radians()),
        scale: Vec3::splat(0.5),
    }
}
