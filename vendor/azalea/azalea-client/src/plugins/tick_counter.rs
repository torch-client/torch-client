use azalea_core::tick::GameTick;
use azalea_physics::PhysicsSystems;
use azalea_world::WorldName;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;

use crate::{mining::MiningSystems, movement::send_position};

#[derive(Clone, Component, Debug, Default, Eq, PartialEq)]
pub struct TicksConnected(pub u64);

pub struct TickCounterPlugin;

impl Plugin for TickCounterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            GameTick,
            increment_counter
                .before(PhysicsSystems)
                .before(MiningSystems)
                .before(send_position),
        );
    }
}

pub fn increment_counter(mut query: Query<&mut TicksConnected, With<WorldName>>) {
    for mut counter in &mut query {
        counter.0 += 1;
    }
}
