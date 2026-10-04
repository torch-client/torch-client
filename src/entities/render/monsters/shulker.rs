#![allow(clippy::approx_constant)]

use azalea_registry::builtin::EntityKind;
use bevy::prelude::{Quat, Vec3};

use crate::entities::TexturePath;
use crate::entities::models::monsters::shulker;
use crate::entities::registry::Registry;
use crate::entities::state::{Direction, EntityState};
use crate::entities::{RenderSpec, RootPose, setup_rotations};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Shulker,
        RenderSpec::new("shulker", shulker::layer, texture, shulker::setup_anim).with_root(root),
    );
}

fn texture(st: &EntityState) -> TexturePath {
    const COLORS: [&str; 16] = [
        "white",
        "orange",
        "magenta",
        "light_blue",
        "yellow",
        "lime",
        "pink",
        "gray",
        "light_gray",
        "cyan",
        "purple",
        "blue",
        "brown",
        "green",
        "red",
        "black",
    ];
    match COLORS.get(st.extras.color as usize) {
        Some(color) => format!("entity/shulker/shulker_{color}").into(),
        None => "entity/shulker/shulker".into(),
    }
}

fn face_rotation(face: Direction) -> Quat {
    const HALF_PI: f32 = 1.5707964;
    const PI: f32 = 3.1415927;
    match face {
        Direction::Down => Quat::from_rotation_x(PI),
        Direction::Up => Quat::IDENTITY,
        Direction::North => Quat::from_rotation_x(HALF_PI) * Quat::from_rotation_z(PI),
        Direction::South => Quat::from_rotation_x(HALF_PI),
        Direction::West => Quat::from_rotation_x(HALF_PI) * Quat::from_rotation_z(HALF_PI),
        Direction::East => Quat::from_rotation_x(HALF_PI) * Quat::from_rotation_z(-HALF_PI),
    }
}

fn opposite(face: Direction) -> Direction {
    match face {
        Direction::Down => Direction::Up,
        Direction::Up => Direction::Down,
        Direction::North => Direction::South,
        Direction::South => Direction::North,
        Direction::West => Direction::East,
        Direction::East => Direction::West,
    }
}

fn root(st: &EntityState) -> RootPose {
    let flipped = EntityState {
        body_rot: st.body_rot + 180.0,
        ..st.clone()
    };
    let mut pose = setup_rotations(&flipped, 90.0);
    let rotation = face_rotation(opposite(st.extras.attach_face));
    let pivot = Vec3::new(0.0, 0.5, 0.0);
    pose.extra_offset += pose.extra_rotation * (pivot - rotation * pivot);
    pose.extra_rotation *= rotation;
    pose
}
