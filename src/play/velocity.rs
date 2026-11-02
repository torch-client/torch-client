use azalea::app::{App, Plugin};
use azalea::ecs::prelude::*;
use azalea::entity::{HasClientLoaded, LocalEntity, Physics};
use azalea::physics::{COBWEB, COBWEB_WEAVING, PhysicsSystems, ai_step, travel::travel};
use azalea_core::position::Vec3;
use azalea_core::tick::GameTick;

use crate::modules::velocity as module;

pub struct VelocityPlugin;

impl Plugin for VelocityPlugin {
    fn build(&self, app: &mut App) {
        let _ = azalea::movement::set_knockback_filter(module::keeps);
        let _ = azalea::physics::travel::set_full_water_efficiency_hook(module::no_water_drag);

        app.add_systems(
            GameTick,
            velocity_clear_web
                .in_set(PhysicsSystems)
                .after(ai_step)
                .before(travel),
        );
    }
}

fn velocity_clear_web(mut query: Query<&mut Physics, (With<LocalEntity>, With<HasClientLoaded>)>) {
    if !module::no_web() {
        return;
    }
    for mut physics in &mut query {
        let stuck = physics.stuck_speed_multiplier;
        if stuck == COBWEB || stuck == COBWEB_WEAVING {
            physics.stuck_speed_multiplier = Vec3::ZERO;
        }
    }
}
