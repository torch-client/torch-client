use std::sync::{Arc, atomic};

use azalea_entity::{LocalEntity, indexing::EntityUuidIndex};
use azalea_protocol::{
    address::ResolvedAddr,
    common::client_information::ClientInformation,
    connect::{Connection, ConnectionError, Proxy},
    packets::{
        ClientIntention, ConnectionProtocol, PROTOCOL_VERSION,
        handshake::ServerboundIntention,
        login::{ClientboundLoginPacket, ServerboundHello, ServerboundLoginPacket},
    },
};
use azalea_world::World;
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_tasks::{IoTaskPool, Task, futures_lite::future};
use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::{
    LocalPlayerBundle,
    account::Account,
    connection::RawConnection,
    local_player::WorldHolder,
    packet::login::{InLoginState, SendLoginPacketEvent},
};

pub struct JoinPlugin;
impl Plugin for JoinPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<StartJoinServerEvent>()
            .add_message::<ConnectionFailedEvent>()
            .add_systems(
                Update,
                (
                    handle_start_join_server_event.before(super::login::poll_auth_task),
                    poll_create_connection_task,
                )
                    .chain(),
            );
    }
}

#[derive(Debug, Message)]
pub struct StartJoinServerEvent {
    pub account: Account,
    pub connect_opts: ConnectOpts,

    pub start_join_callback_tx: Option<mpsc::UnboundedSender<Entity>>,
}

#[derive(Clone, Component, Debug)]
pub struct ConnectOpts {
    pub address: ResolvedAddr,
    pub server_proxy: Option<Proxy>,
    pub sessionserver_proxy: Option<Proxy>,
}

#[derive(Message)]
pub struct ConnectionFailedEvent {
    pub entity: Entity,
    pub error: Arc<ConnectionError>,
}

pub fn handle_start_join_server_event(
    mut commands: Commands,
    mut events: MessageReader<StartJoinServerEvent>,
    mut entity_uuid_index: ResMut<EntityUuidIndex>,
    connection_query: Query<&RawConnection>,
) {
    for event in events.read() {
        let uuid = event.account.uuid();
        let entity = if let Some(entity) = entity_uuid_index.get(&uuid) {
            debug!("Reusing entity {entity:?} for client");

            if let Ok(conn) = connection_query.get(entity)
                && conn.is_alive()
            {
                if let Some(start_join_callback_tx) = &event.start_join_callback_tx {
                    warn!(
                        "Received StartJoinServerEvent for {entity:?} but it's already connected. Ignoring the event but replying with Ok."
                    );
                    let _ = start_join_callback_tx.send(entity);
                } else {
                    warn!(
                        "Received StartJoinServerEvent for {entity:?} but it's already connected. Ignoring the event."
                    );
                }
                return;
            }

            entity
        } else {
            let entity = commands.spawn_empty().id();
            debug!("Created new entity {entity:?} for client");
            entity_uuid_index.insert(uuid, entity);
            entity
        };

        if let Some(start_join_callback) = &event.start_join_callback_tx {
            let _ = start_join_callback.send(entity);
        }

        let mut entity_mut = commands.entity(entity);

        entity_mut.insert((
            event.account.to_owned(),
            LocalEntity,
            ClientInformation::default(),
            event.connect_opts.clone(),
        ));

        let task_pool = IoTaskPool::get();
        let connect_opts = event.connect_opts.clone();
        let task = task_pool.spawn(crate::compat(create_conn_and_send_intention_packet(
            connect_opts,
        )));

        entity_mut.insert(CreateConnectionTask(task));
    }
}

pub static HANDSHAKE_PROTOCOL: atomic::AtomicI32 = atomic::AtomicI32::new(PROTOCOL_VERSION);

async fn create_conn_and_send_intention_packet(
    opts: ConnectOpts,
) -> Result<LoginConn, ConnectionError> {
    let mut conn = if azalea_protocol::connect::transport::has_opener() {
        azalea_protocol::connect::transport::open(opts.address.clone()).await?
    } else {
        #[cfg(target_arch = "wasm32")]
        {
            azalea_protocol::connect::transport::open(opts.address.clone()).await?
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(proxy) = opts.server_proxy {
            Connection::new_with_proxy(&opts.address.socket, proxy).await?
        } else {
            Connection::new(&opts.address.socket).await?
        }
    };

    conn.write(ServerboundIntention {
        protocol_version: HANDSHAKE_PROTOCOL.load(atomic::Ordering::Relaxed),
        hostname: opts.address.server.host.clone(),
        port: opts.address.server.port,
        intention: ClientIntention::Login,
    })
    .await?;

    let conn = conn.login();

    Ok(conn)
}

type LoginConn = Connection<ClientboundLoginPacket, ServerboundLoginPacket>;

#[derive(Component)]
pub struct CreateConnectionTask(pub Task<Result<LoginConn, ConnectionError>>);

pub fn poll_create_connection_task(
    mut commands: Commands,
    mut query: Query<(Entity, &mut CreateConnectionTask, &Account)>,
    mut connection_failed_events: MessageWriter<ConnectionFailedEvent>,
) {
    for (entity, mut task, account) in query.iter_mut() {
        if let Some(poll_res) = future::block_on(future::poll_once(&mut task.0)) {
            let mut entity_mut = commands.entity(entity);
            entity_mut.remove::<CreateConnectionTask>();
            let conn = match poll_res {
                Ok(conn) => conn,
                Err(error) => {
                    warn!("failed to create connection: {error}");
                    connection_failed_events.write(ConnectionFailedEvent {
                        entity,
                        error: Arc::new(error),
                    });
                    return;
                }
            };

            let (read_conn, write_conn) = conn.into_split();
            let (read_conn, write_conn) = (read_conn.raw, write_conn.raw);

            let world = World::default();
            let world_holder = WorldHolder::new(
                entity,
                Arc::new(RwLock::new(world)),
            );

            entity_mut.insert((
                LocalPlayerBundle {
                    raw_connection: RawConnection::new(
                        read_conn,
                        write_conn,
                        ConnectionProtocol::Login,
                    ),
                    world_holder,
                    metadata: azalea_entity::metadata::PlayerMetadataBundle::default(),
                },
                InLoginState,
            ));

            commands.trigger(SendLoginPacketEvent::new(
                entity,
                ServerboundHello {
                    name: account.username().to_owned(),
                    profile_id: account.uuid(),
                },
            ));
        }
    }
}
