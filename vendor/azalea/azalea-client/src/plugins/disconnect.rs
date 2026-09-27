use azalea_chat::FormattedText;
use azalea_core::entity_id::MinecraftEntityId;
use azalea_entity::{
    EntityBundle, HasClientLoaded, InLoadedChunk, LocalEntity, metadata::PlayerMetadataBundle,
};
use bevy_app::{App, Plugin, PostUpdate};
use bevy_ecs::prelude::*;
use derive_more::Deref;
use tracing::info;

use super::login::IsAuthenticated;
#[cfg(feature = "online-mode")]
use crate::chat_signing;
use crate::{
    client::JoinedClientBundle, connection::RawConnection, local_player::WorldHolder, mining,
    tick_counter::TicksConnected,
};

pub struct DisconnectPlugin;
impl Plugin for DisconnectPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<DisconnectEvent>().add_systems(
            PostUpdate,
            (
                update_read_packets_task_running_component,
                remove_components_from_disconnected_players,
                disconnect_on_connection_dead,
            )
                .chain(),
        );
    }
}

#[derive(Message)]
pub struct DisconnectEvent {
    pub entity: Entity,
    pub reason: Option<FormattedText>,
}

#[derive(Bundle)]
pub struct RemoveOnDisconnectBundle {
    pub joined_client: JoinedClientBundle,

    pub entity: EntityBundle,
    pub minecraft_entity_id: MinecraftEntityId,
    pub world_holder: WorldHolder,
    pub player_metadata: PlayerMetadataBundle,
    pub in_loaded_chunk: InLoadedChunk,
    pub raw_connection: RawConnection,
    pub is_connection_alive: IsConnectionAlive,
    #[cfg(feature = "online-mode")]
    pub chat_signing_session: chat_signing::ChatSigningSession,
    pub is_authenticated: IsAuthenticated,
    pub has_client_loaded: HasClientLoaded,
    pub ticks_alive: TicksConnected,

    pub mining: mining::Mining,
}

pub fn remove_components_from_disconnected_players(
    mut commands: Commands,
    mut events: MessageReader<DisconnectEvent>,
    mut loaded_by_query: Query<&mut azalea_entity::LoadedBy>,
) {
    for DisconnectEvent { entity, reason } in events.read() {
        info!(
            "A client {entity:?} was disconnected{}",
            if let Some(reason) = reason {
                format!(": {reason}")
            } else {
                "".to_owned()
            }
        );
        commands
            .entity(*entity)
            .remove::<RemoveOnDisconnectBundle>();

        for mut loaded_by in &mut loaded_by_query.iter_mut() {
            loaded_by.remove(entity);
        }
    }
}

#[derive(Clone, Component, Copy, Debug, Deref)]
pub struct IsConnectionAlive(bool);

fn update_read_packets_task_running_component(
    query: Query<(Entity, &RawConnection)>,
    mut commands: Commands,
) {
    for (entity, raw_connection) in &query {
        let running = raw_connection.is_alive();
        commands.entity(entity).insert(IsConnectionAlive(running));
    }
}

#[allow(clippy::type_complexity)]
fn disconnect_on_connection_dead(
    query: Query<(Entity, &IsConnectionAlive), (Changed<IsConnectionAlive>, With<LocalEntity>)>,
    mut disconnect_events: MessageWriter<DisconnectEvent>,
) {
    for (entity, &is_connection_alive) in &query {
        if !*is_connection_alive {
            disconnect_events.write(DisconnectEvent {
                entity,
                reason: None,
            });
        }
    }
}
