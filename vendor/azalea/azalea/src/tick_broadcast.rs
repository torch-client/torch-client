use azalea_core::tick::GameTick;
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use derive_more::Deref;
use tokio::sync::broadcast;

use crate::Client;

pub struct TickBroadcastPlugin;
impl Plugin for TickBroadcastPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TickBroadcast(broadcast::channel(1).0))
            .insert_resource(UpdateBroadcast(broadcast::channel(1).0))
            .add_systems(
                GameTick,
                send_tick_broadcast.after(azalea_client::tick_counter::increment_counter),
            )
            .add_systems(Update, send_update_broadcast);
    }
}

#[derive(Deref, Resource)]
pub struct TickBroadcast(broadcast::Sender<()>);

#[derive(Deref, Resource)]
pub struct UpdateBroadcast(broadcast::Sender<()>);

pub fn send_tick_broadcast(tick_broadcast: ResMut<TickBroadcast>) {
    let _ = tick_broadcast.0.send(());
}
pub fn send_update_broadcast(update_broadcast: ResMut<UpdateBroadcast>) {
    let _ = update_broadcast.0.send(());
}

impl Client {
    pub fn get_tick_broadcaster(&self) -> tokio::sync::broadcast::Receiver<()> {
        let ecs = self.ecs.read();
        let tick_broadcast = ecs.resource::<TickBroadcast>();
        tick_broadcast.subscribe()
    }

    pub fn get_update_broadcaster(&self) -> tokio::sync::broadcast::Receiver<()> {
        let ecs = self.ecs.read();
        let update_broadcast = ecs.resource::<UpdateBroadcast>();
        update_broadcast.subscribe()
    }
}
