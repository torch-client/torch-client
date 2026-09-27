use azalea_registry::builtin::EntityKind;
use bevy::math::{Mat4, Quat, Vec3};

use crate::entities::models::objects::boat;
use crate::entities::registry::Registry;
use crate::entities::render::objects::{hook_for, mirror};
use crate::entities::state::EntityState;
use crate::entities::{RenderSpec, RootPose};

pub(crate) const BOATS: &[EntityKind] = &[
    EntityKind::OakBoat,
    EntityKind::SpruceBoat,
    EntityKind::BirchBoat,
    EntityKind::JungleBoat,
    EntityKind::AcaciaBoat,
    EntityKind::CherryBoat,
    EntityKind::DarkOakBoat,
    EntityKind::PaleOakBoat,
    EntityKind::MangroveBoat,
];

pub(crate) const CHEST_BOATS: &[EntityKind] = &[
    EntityKind::OakChestBoat,
    EntityKind::SpruceChestBoat,
    EntityKind::BirchChestBoat,
    EntityKind::JungleChestBoat,
    EntityKind::AcaciaChestBoat,
    EntityKind::CherryChestBoat,
    EntityKind::DarkOakChestBoat,
    EntityKind::PaleOakChestBoat,
    EntityKind::MangroveChestBoat,
];

pub fn register(registry: &mut Registry) {
    registry.add_many(
        BOATS,
        RenderSpec::new("boat", boat::boat_layer, boat_texture, boat::setup_anim).with_root(root),
    );
    registry.add_many(
        CHEST_BOATS,
        RenderSpec::new(
            "chest_boat",
            boat::chest_boat_layer,
            chest_boat_texture,
            boat::setup_anim,
        )
        .with_root(root),
    );
    registry.add(
        EntityKind::BambooRaft,
        RenderSpec::new("raft", boat::raft_layer, raft_texture, boat::setup_anim).with_root(root),
    );
    registry.add(
        EntityKind::BambooChestRaft,
        RenderSpec::new(
            "chest_raft",
            boat::chest_raft_layer,
            chest_raft_texture,
            boat::setup_anim,
        )
        .with_root(root),
    );
}

fn wood(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::SpruceBoat | EntityKind::SpruceChestBoat => "spruce",
        EntityKind::BirchBoat | EntityKind::BirchChestBoat => "birch",
        EntityKind::JungleBoat | EntityKind::JungleChestBoat => "jungle",
        EntityKind::AcaciaBoat | EntityKind::AcaciaChestBoat => "acacia",
        EntityKind::CherryBoat | EntityKind::CherryChestBoat => "cherry",
        EntityKind::DarkOakBoat | EntityKind::DarkOakChestBoat => "dark_oak",
        EntityKind::PaleOakBoat | EntityKind::PaleOakChestBoat => "pale_oak",
        EntityKind::MangroveBoat | EntityKind::MangroveChestBoat => "mangrove",
        EntityKind::BambooRaft | EntityKind::BambooChestRaft => "bamboo",
        _ => "oak",
    }
}

fn boat_texture(st: &EntityState) -> String {
    format!("entity/boat/{}", wood(st.kind))
}

fn chest_boat_texture(st: &EntityState) -> String {
    format!("entity/chest_boat/{}", wood(st.kind))
}

fn raft_texture(_st: &EntityState) -> String {
    "entity/boat/bamboo".to_string()
}

fn chest_raft_texture(_st: &EntityState) -> String {
    "entity/chest_boat/bamboo".to_string()
}

fn root(st: &EntityState) -> RootPose {
    let mut extra_rotation = Quat::IDENTITY;

    let hurt = st.extras.hurt_time;
    if hurt > 0.0 {
        let degrees = hurt.sin() * hurt * st.extras.damage / 10.0 * st.extras.hurt_dir;
        extra_rotation = Quat::from_rotation_x(degrees.to_radians());
    }

    let bubble = st.extras.bubble_time;
    if !st.extras.under_water && !mth_equal(bubble, 0.0) {
        let axis = Vec3::new(1.0, 0.0, 1.0).normalize();
        extra_rotation *= Quat::from_axis_angle(axis, bubble.to_radians());
    }

    RootPose {
        world_offset: Vec3::new(0.0, 0.375, 0.0),
        rotation: Quat::from_rotation_y((180.0 - st.body_rot).to_radians()),
        scale: 1.0,
        extra_rotation,
        extra_offset: Vec3::ZERO,
        hook: hook_for(mirror() * Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2)),
    }
}

fn mth_equal(a: f32, b: f32) -> bool {
    (b - a).abs() < 1.0e-5
}
