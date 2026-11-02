use azalea::app::{App, Plugin};
use azalea::ecs::prelude::*;
use azalea::entity::metadata::FallFlying;
use azalea::entity::{
    HasClientLoaded, LocalEntity, LookDirection, OnClimbable, Physics, PlayerAbilities,
    input_vector,
};
use azalea::physics::client_movement::ClientMovementState;
use azalea::physics::{PhysicsSystems, ai_step, travel::travel};
use azalea_core::position::Vec3;
use azalea_core::tick::GameTick;

use crate::modules::speed as module;

const NO_INPUT: f64 = 1.0e-7;

pub struct SpeedPlugin;

impl Plugin for SpeedPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            GameTick,
            speed_pre_travel
                .in_set(PhysicsSystems)
                .after(ai_step)
                .after(crate::play::flight::flight_pre_travel)
                .before(travel),
        );
    }
}

#[allow(clippy::type_complexity)]
fn speed_pre_travel(
    mut query: Query<
        (
            &mut Physics,
            &LookDirection,
            &OnClimbable,
            Option<&ClientMovementState>,
            Option<&FallFlying>,
            Option<&PlayerAbilities>,
        ),
        (With<LocalEntity>, With<HasClientLoaded>),
    >,
) {
    if !module::enabled() {
        return;
    }
    if crate::client::tracking::local_vehicle_id().is_some() {
        return;
    }

    for (mut physics, direction, on_climbable, movement, fall_flying, abilities) in &mut query {
        if fall_flying.is_some_and(|f| **f) || **on_climbable || abilities.is_some_and(|a| a.flying)
        {
            continue;
        }
        let Some(speed) = module::per_tick(
            movement.is_some_and(|m| m.trying_to_crouch),
            physics.is_in_water() || physics.is_in_lava(),
            physics.on_ground(),
        ) else {
            continue;
        };

        let input = Vec3::new(
            physics.x_acceleration as f64,
            0.0,
            physics.z_acceleration as f64,
        );
        if input.length_squared() < NO_INPUT {
            continue;
        }

        let target = input_vector(*direction, speed as f32, input.normalize());
        physics.velocity.x = target.x;
        physics.velocity.z = target.z;
        physics.x_acceleration = 0.0;
        physics.z_acceleration = 0.0;
    }
}
