use azalea_core::tick::GameTick;
use azalea_entity::LocalEntity;
use azalea_physics::PhysicsSystems;
use azalea_protocol::packets::game::ServerboundClientTickEnd;
use azalea_world::WorldName;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;

use crate::{mining::MiningSystems, packet::game::SendGamePacketEvent};

pub struct TickEndPlugin;
impl Plugin for TickEndPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            GameTick,
            game_tick_packet
                .after(PhysicsSystems)
                .after(MiningSystems)
                .after(crate::movement::send_position),
        );
    }
}

pub fn game_tick_packet(
    query: Query<Entity, (With<LocalEntity>, With<WorldName>)>,
    mut commands: Commands,
) {
    for entity in query.iter() {
        commands.trigger(SendGamePacketEvent::new(entity, ServerboundClientTickEnd));
    }
}
