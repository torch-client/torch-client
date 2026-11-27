use azalea_core::tick::{GameTick, TickRateManager, TickRateSystems};
use azalea_physics::PhysicsSystems;
use azalea_world::{WorldBorder, WorldName};
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
        app.init_resource::<TickRateManager>()
            .configure_sets(
                GameTick,
                TickRateSystems
                    .before(PhysicsSystems)
                    .before(MiningSystems)
                    .before(send_position),
            )
            .add_systems(GameTick, tick_rate.in_set(TickRateSystems))
            .add_observer(
                |_: On<Insert, TicksConnected>, mut rate: ResMut<TickRateManager>| {
                    *rate = TickRateManager::default();
                },
            )
            .add_observer(
                |_: On<Insert, WorldBorder>, mut rate: ResMut<TickRateManager>| {
                    *rate = TickRateManager::default();
                },
            );
    }
}

pub fn tick_rate(mut rate: ResMut<TickRateManager>) {
    rate.tick();
}

pub fn increment_counter(mut query: Query<&mut TicksConnected, With<WorldName>>) {
    for mut counter in &mut query {
        counter.0 += 1;
    }
}
