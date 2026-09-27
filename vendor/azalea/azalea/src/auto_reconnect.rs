use bevy_platform::time::Instant;
use std::time::Duration;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;

use super::{
    disconnect::DisconnectEvent,
    join::{ConnectOpts, ConnectionFailedEvent, StartJoinServerEvent},
};
use crate::account::Account;

pub const DEFAULT_RECONNECT_DELAY: Duration = Duration::from_secs(5);

pub struct AutoReconnectPlugin;
impl Plugin for AutoReconnectPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AutoReconnectDelay::new(DEFAULT_RECONNECT_DELAY))
            .add_systems(
                Update,
                (start_rejoin_on_disconnect, rejoin_after_delay)
                    .chain()
                    .before(super::join::handle_start_join_server_event),
            );
    }
}

pub fn start_rejoin_on_disconnect(
    mut commands: Commands,
    mut disconnect_events: MessageReader<DisconnectEvent>,
    mut connection_failed_events: MessageReader<ConnectionFailedEvent>,
    auto_reconnect_delay_res: Option<Res<AutoReconnectDelay>>,
    auto_reconnect_delay_query: Query<&AutoReconnectDelay>,
) {
    for entity in disconnect_events
        .read()
        .map(|e| e.entity)
        .chain(connection_failed_events.read().map(|e| e.entity))
    {
        let Some(delay) = get_delay(
            &auto_reconnect_delay_res,
            auto_reconnect_delay_query,
            entity,
        ) else {
            continue;
        };

        let reconnect_after = Instant::now() + delay;
        commands.entity(entity).insert(InternalReconnectAfter {
            instant: reconnect_after,
        });
    }
}

fn get_delay(
    auto_reconnect_delay_res: &Option<Res<AutoReconnectDelay>>,
    auto_reconnect_delay_query: Query<&AutoReconnectDelay>,
    entity: Entity,
) -> Option<Duration> {
    let delay = if let Ok(c) = auto_reconnect_delay_query.get(entity) {
        Some(c.delay)
    } else {
        auto_reconnect_delay_res.as_ref().map(|r| r.delay)
    };

    if delay == Some(Duration::MAX) {
        return None;
    }
    delay
}

pub fn rejoin_after_delay(
    mut commands: Commands,
    mut join_events: MessageWriter<StartJoinServerEvent>,
    query: Query<(Entity, &InternalReconnectAfter, &Account, &ConnectOpts)>,
) {
    for (entity, reconnect_after, account, connect_opts) in query.iter() {
        if Instant::now() >= reconnect_after.instant {
            commands.entity(entity).remove::<InternalReconnectAfter>();

            join_events.write(StartJoinServerEvent {
                account: account.clone(),
                connect_opts: connect_opts.clone(),
                start_join_callback_tx: None,
            });
        }
    }
}

#[derive(Clone, Component, Debug, Resource)]
pub struct AutoReconnectDelay {
    pub delay: Duration,
}
impl AutoReconnectDelay {
    pub fn new(delay: Duration) -> Self {
        Self { delay }
    }
}

#[derive(Clone, Component, Debug)]
pub struct InternalReconnectAfter {
    pub instant: Instant,
}
