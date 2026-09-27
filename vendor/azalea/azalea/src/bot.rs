use std::f64::consts::PI;

use azalea_client::mining::Mining;
use azalea_core::{
    position::{BlockPos, Vec3},
    tick::GameTick,
};
use azalea_entity::{
    Jumping, LocalEntity, LookDirection, Position, clamp_look_direction,
    dimensions::EntityDimensions, metadata::Player, update_dimensions,
};
use azalea_physics::PhysicsSystems;
use bevy_app::Update;
use bevy_ecs::prelude::*;
use tracing::trace;

use crate::{
    Client,
    app::{App, Plugin, PluginGroup, PluginGroupBuilder},
    ecs::{
        component::Component,
        entity::Entity,
        query::{With, Without},
        system::{Commands, Query},
    },
};

#[derive(Clone, Default)]
pub struct BotPlugin;
impl Plugin for BotPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<LookAtEvent>()
            .add_message::<JumpEvent>()
            .add_systems(
                Update,
                (
                    insert_bot,
                    look_at_listener
                        .before(clamp_look_direction)
                        .after(update_dimensions),
                    jump_listener,
                ),
            )
            .add_systems(
                GameTick,
                stop_jumping
                    .after(PhysicsSystems)
                    .after(azalea_client::movement::send_player_input_packet),
            );
    }
}

#[derive(Component, Default)]
pub struct Bot {
    jumping_once: bool,
}

#[allow(clippy::type_complexity)]
fn insert_bot(
    mut commands: Commands,
    mut query: Query<Entity, (Without<Bot>, With<LocalEntity>, With<Player>)>,
) {
    for entity in &mut query {
        commands.entity(entity).insert(Bot::default());
    }
}

fn stop_jumping(mut query: Query<(&mut Jumping, &mut Bot)>) {
    for (mut jumping, mut bot) in &mut query {
        if bot.jumping_once && **jumping {
            bot.jumping_once = false;
            **jumping = false;
        }
    }
}

impl Client {
    pub fn jump(&self) {
        let mut ecs = self.ecs.write();
        ecs.write_message(JumpEvent {
            entity: self.entity,
        });
    }

    pub fn look_at(&self, position: Vec3) {
        let mut ecs = self.ecs.write();
        ecs.write_message(LookAtEvent {
            entity: self.entity,
            position,
        });
    }

    pub async fn wait_ticks(&self, n: usize) {
        let mut receiver = self.get_tick_broadcaster();
        for _ in 0..n {
            let _ = receiver.recv().await;
        }
    }
    pub async fn wait_updates(&self, n: usize) {
        let mut receiver = self.get_update_broadcaster();
        for _ in 0..n {
            let _ = receiver.recv().await;
        }
    }

    pub async fn mine(&self, position: BlockPos) {
        self.start_mining(position);

        let mut receiver = self.get_tick_broadcaster();
        while receiver.recv().await.is_ok() {
            let ecs = self.ecs.read();
            if ecs.get::<Mining>(self.entity).is_none() {
                break;
            }
        }
    }
}

#[derive(Message)]
pub struct JumpEvent {
    pub entity: Entity,
}

pub fn jump_listener(
    mut query: Query<(&mut Jumping, &mut Bot)>,
    mut events: MessageReader<JumpEvent>,
) {
    for event in events.read() {
        if let Ok((mut jumping, mut bot)) = query.get_mut(event.entity) {
            **jumping = true;
            bot.jumping_once = true;
        }
    }
}

#[derive(Message)]
pub struct LookAtEvent {
    pub entity: Entity,
    pub position: Vec3,
}
fn look_at_listener(
    mut events: MessageReader<LookAtEvent>,
    mut query: Query<(&Position, &EntityDimensions, &mut LookDirection)>,
) {
    for event in events.read() {
        if let Ok((position, dimensions, mut look_direction)) = query.get_mut(event.entity) {
            let new_look_direction =
                direction_looking_at(position.up(dimensions.eye_height.into()), event.position);

            trace!("look at {} (currently at {})", event.position, **position);
            look_direction.update(new_look_direction);
        }
    }
}

pub fn direction_looking_at(current: Vec3, target: Vec3) -> LookDirection {
    let delta = target - current;
    let y_rot = (PI - f64::atan2(-delta.x, -delta.z)) * (180.0 / PI);
    let ground_distance = f64::sqrt(delta.x * delta.x + delta.z * delta.z);
    let x_rot = f64::atan2(delta.y, ground_distance) * -(180.0 / PI);

    LookDirection::new(y_rot as f32, x_rot as f32)
}

pub struct DefaultBotPlugins;

impl PluginGroup for DefaultBotPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(BotPlugin)
            .add(crate::pathfinder::PathfinderPlugin)
            .add(crate::container::ContainerPlugin)
            .add(crate::auto_respawn::AutoRespawnPlugin)
            .add(crate::accept_resource_packs::AcceptResourcePacksPlugin)
            .add(crate::tick_broadcast::TickBroadcastPlugin)
            .add(crate::events::EventsPlugin)
            .add(crate::auto_reconnect::AutoReconnectPlugin)
    }
}
