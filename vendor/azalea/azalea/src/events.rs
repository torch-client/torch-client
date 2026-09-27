use std::sync::Arc;

use azalea_chat::FormattedText;
use azalea_client::join::ConnectionFailedEvent;
use azalea_core::{entity_id::MinecraftEntityId, position::ChunkPos, tick::GameTick};
use azalea_entity::{Dead, InLoadedChunk};
use azalea_protocol::{
    connect::ConnectionError, packets::game::c_player_combat_kill::ClientboundPlayerCombatKill,
};
use azalea_world::WorldName;
use bevy_app::{App, Plugin, PreUpdate, Update};
use bevy_ecs::prelude::*;
use derive_more::{Deref, DerefMut};
use tokio::sync::mpsc;

use crate::{
    chunks::ReceiveChunkEvent,
    client_chat::{ChatPacket, ChatReceivedEvent},
    disconnect::DisconnectEvent,
    packet::game::{
        AddPlayerEvent, DeathEvent, KeepAliveEvent, RemovePlayerEvent, UpdatePlayerEvent,
    },
    player::PlayerInfo,
};

#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Event {
    Init,
    Login,
    Spawn,
    Chat(ChatPacket),
    Tick,
    #[cfg(feature = "packet-event")]
    Packet(Arc<azalea_protocol::packets::game::ClientboundGamePacket>),
    AddPlayer(PlayerInfo),
    RemovePlayer(PlayerInfo),
    UpdatePlayer(PlayerInfo),
    Death(Option<Arc<ClientboundPlayerCombatKill>>),
    KeepAlive(u64),
    Disconnect(Option<FormattedText>),
    ConnectionFailed(Arc<ConnectionError>),
    ReceiveChunk(ChunkPos),
}

#[derive(Component, Deref, DerefMut)]
pub struct LocalPlayerEvents(pub mpsc::UnboundedSender<Event>);

pub struct EventsPlugin;
impl Plugin for EventsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                chat_listener,
                login_listener,
                spawn_listener,
                #[cfg(feature = "packet-event")]
                packet_listener,
                add_player_listener,
                update_player_listener,
                remove_player_listener,
                death_listener.after(azalea_client::packet::death_event_on_0_health),
                disconnect_listener,
                connection_failed_listener.after(azalea_client::join::poll_create_connection_task),
                receive_chunk_listener,
            ),
        )
        .add_systems(
            PreUpdate,
            init_listener.before(super::connection::read_packets),
        )
        .add_systems(GameTick, tick_listener)
        .add_observer(keepalive_listener);
    }
}

pub fn init_listener(query: Query<&LocalPlayerEvents, Added<LocalPlayerEvents>>) {
    for local_player_events in &query {
        let _ = local_player_events.send(Event::Init);
    }
}

pub fn login_listener(
    query: Query<(Entity, &LocalPlayerEvents), Added<MinecraftEntityId>>,
    mut commands: Commands,
) {
    for (entity, local_player_events) in &query {
        let _ = local_player_events.send(Event::Login);
        commands.entity(entity).remove::<SentSpawnEvent>();
    }
}

#[derive(Component)]
pub struct SentSpawnEvent;
#[allow(clippy::type_complexity)]
pub fn spawn_listener(
    query: Query<(Entity, &LocalPlayerEvents), (Added<InLoadedChunk>, Without<SentSpawnEvent>)>,
    mut commands: Commands,
) {
    for (entity, local_player_events) in &query {
        let _ = local_player_events.send(Event::Spawn);
        commands.entity(entity).insert(SentSpawnEvent);
    }
}

pub fn chat_listener(
    query: Query<&LocalPlayerEvents>,
    mut events: MessageReader<ChatReceivedEvent>,
) {
    for event in events.read() {
        if let Ok(local_player_events) = query.get(event.entity) {
            let _ = local_player_events.send(Event::Chat(event.packet.clone()));
        }
    }
}

pub fn tick_listener(query: Query<&LocalPlayerEvents, With<WorldName>>) {
    for local_player_events in &query {
        let _ = local_player_events.send(Event::Tick);
    }
}

#[cfg(feature = "packet-event")]
pub fn packet_listener(
    query: Query<&LocalPlayerEvents>,
    mut events: MessageReader<super::packet::game::ReceiveGamePacketEvent>,
) {
    for event in events.read() {
        if let Ok(local_player_events) = query.get(event.entity) {
            let _ = local_player_events.send(Event::Packet(event.packet.clone()));
        }
    }
}

pub fn add_player_listener(
    query: Query<&LocalPlayerEvents>,
    mut events: MessageReader<AddPlayerEvent>,
) {
    for event in events.read() {
        if let Ok(local_player_events) = query.get(event.entity) {
            let _ = local_player_events.send(Event::AddPlayer(event.info.clone()));
        }
    }
}

pub fn update_player_listener(
    query: Query<&LocalPlayerEvents>,
    mut events: MessageReader<UpdatePlayerEvent>,
) {
    for event in events.read() {
        if let Ok(local_player_events) = query.get(event.entity) {
            let _ = local_player_events.send(Event::UpdatePlayer(event.info.clone()));
        }
    }
}

pub fn remove_player_listener(
    query: Query<&LocalPlayerEvents>,
    mut events: MessageReader<RemovePlayerEvent>,
) {
    for event in events.read() {
        if let Ok(local_player_events) = query.get(event.entity) {
            let _ = local_player_events.send(Event::RemovePlayer(event.info.clone()));
        }
    }
}

pub fn death_listener(query: Query<&LocalPlayerEvents>, mut events: MessageReader<DeathEvent>) {
    for event in events.read() {
        if let Ok(local_player_events) = query.get(event.entity) {
            let _ = local_player_events.send(Event::Death(event.packet.clone().map(|p| p.into())));
        }
    }
}

pub fn dead_component_listener(query: Query<&LocalPlayerEvents, Added<Dead>>) {
    for local_player_events in &query {
        local_player_events.send(Event::Death(None)).unwrap();
    }
}

pub fn keepalive_listener(keep_alive: On<KeepAliveEvent>, query: Query<&LocalPlayerEvents>) {
    if let Ok(local_player_events) = query.get(keep_alive.entity) {
        let _ = local_player_events.send(Event::KeepAlive(keep_alive.id));
    }
}

pub fn disconnect_listener(
    query: Query<&LocalPlayerEvents>,
    mut events: MessageReader<DisconnectEvent>,
) {
    for event in events.read() {
        if let Ok(local_player_events) = query.get(event.entity) {
            let _ = local_player_events.send(Event::Disconnect(event.reason.clone()));
        }
    }
}

pub fn connection_failed_listener(
    query: Query<&LocalPlayerEvents>,
    mut events: MessageReader<ConnectionFailedEvent>,
) {
    for event in events.read() {
        if let Ok(local_player_events) = query.get(event.entity) {
            let _ = local_player_events.send(Event::ConnectionFailed(event.error.clone()));
        }
    }
}

pub fn receive_chunk_listener(
    query: Query<&LocalPlayerEvents>,
    mut events: MessageReader<ReceiveChunkEvent>,
) {
    for event in events.read() {
        if let Ok(local_player_events) = query.get(event.entity) {
            let _ = local_player_events.send(Event::ReceiveChunk(ChunkPos::new(
                event.packet.x,
                event.packet.z,
            )));
        }
    }
}
