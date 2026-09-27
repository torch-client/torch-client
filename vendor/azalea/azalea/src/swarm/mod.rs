mod builder;
mod chat;
mod events;
pub mod prelude;

use std::sync::{
    Arc,
    atomic::{self, AtomicBool},
};

use azalea_client::{account::Account, client_chat::ChatPacket, join::ConnectOpts};
use azalea_entity::LocalEntity;
use azalea_protocol::address::ResolvedAddr;
use azalea_world::Worlds;
use bevy_app::{AppExit, PluginGroup, PluginGroupBuilder};
use bevy_ecs::prelude::*;
pub use builder::SwarmBuilder;
use futures::future::BoxFuture;
use parking_lot::RwLock;
use tokio::{sync::mpsc, task};
use tracing::{debug, error, warn};

use crate::{Client, JoinOpts, client_impl::StartClientOpts};

#[derive(Clone, Resource)]
pub struct Swarm {
    #[doc(alias = "ecs_lock")]
    pub ecs: Arc<RwLock<World>>,

    pub address: Arc<RwLock<ResolvedAddr>>,

    pub worlds: Arc<RwLock<Worlds>>,

    pub(crate) bots_tx: mpsc::UnboundedSender<(Option<crate::Event>, Client)>,
    pub(crate) swarm_tx: mpsc::UnboundedSender<SwarmEvent>,
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum SwarmEvent {
    Login,
    Init,
    Tick,
    Disconnect(Box<Account>, Box<JoinOpts>),
    Chat(ChatPacket),
}

pub type SwarmHandleFn<SS, Fut> = fn(Swarm, SwarmEvent, SS) -> Fut;
pub type BoxSwarmHandleFn<SS, R> =
    Box<dyn Fn(Swarm, SwarmEvent, SS) -> BoxFuture<'static, R> + Send + Sync>;

impl Swarm {
    pub async fn add<S: Component + Clone>(&self, account: &Account, state: S) -> Client {
        self.add_with_opts(account, state, &JoinOpts::default())
            .await
    }
    pub async fn add_with_opts<S: Component + Clone>(
        &self,
        account: &Account,
        state: S,
        join_opts: &JoinOpts,
    ) -> Client {
        debug!(
            "add_with_opts called for account {} with opts {join_opts:?}",
            account.username()
        );

        let mut address = self.address.read().clone();
        if let Some(custom_server_addr) = join_opts.custom_server_addr.clone() {
            address.server = custom_server_addr;
        }
        if let Some(custom_socket_addr) = join_opts.custom_socket_addr {
            address.socket = custom_socket_addr;
        }
        let server_proxy = join_opts.server_proxy.clone();
        let sessionserver_proxy = join_opts.sessionserver_proxy.clone();

        let (tx, rx) = mpsc::unbounded_channel();

        let client = Client::start_client(StartClientOpts {
            ecs_lock: self.ecs.clone(),
            account: account.clone(),
            connect_opts: ConnectOpts {
                address,
                server_proxy,
                sessionserver_proxy,
            },
            event_sender: Some(tx),
        })
        .await;
        {
            let mut ecs = self.ecs.write();
            ecs.entity_mut(client.entity).insert(state);
        }

        let cloned_bot = client.clone();
        let swarm_tx = self.swarm_tx.clone();
        let bots_tx = self.bots_tx.clone();

        let join_opts = join_opts.clone();
        task::spawn_local(Self::event_copying_task(
            rx, swarm_tx, bots_tx, cloned_bot, join_opts,
        ));

        client
    }

    async fn event_copying_task(
        mut rx: mpsc::UnboundedReceiver<crate::Event>,
        swarm_tx: mpsc::UnboundedSender<SwarmEvent>,
        bots_tx: mpsc::UnboundedSender<(Option<crate::Event>, Client)>,
        bot: Client,
        join_opts: JoinOpts,
    ) {
        while let Some(event) = rx.recv().await {
            if rx.len() > 1_000 {
                static WARNED_1_000: AtomicBool = AtomicBool::new(false);
                if !WARNED_1_000.swap(true, atomic::Ordering::Relaxed) {
                    warn!(
                        "The client's Event channel has more than 1,000 items! If you don't need it, consider disabling the `packet-event` feature for `azalea`."
                    )
                }

                if rx.len() > 10_000 {
                    static WARNED_10_000: AtomicBool = AtomicBool::new(false);
                    if !WARNED_10_000.swap(true, atomic::Ordering::Relaxed) {
                        warn!("The client's Event channel has more than 10,000 items!!")
                    }

                    if rx.len() > 100_000 {
                        static WARNED_100_000: AtomicBool = AtomicBool::new(false);
                        if !WARNED_100_000.swap(true, atomic::Ordering::Relaxed) {
                            warn!("The client's Event channel has more than 100,000 items!!!")
                        }

                        if rx.len() > 1_000_000 {
                            static WARNED_1_000_000: AtomicBool = AtomicBool::new(false);
                            if !WARNED_1_000_000.swap(true, atomic::Ordering::Relaxed) {
                                warn!(
                                    "The client's Event channel has more than 1,000,000 items!!!! your code is almost certainly leaking memory"
                                )
                            }
                        }
                    }
                }
            }

            if let crate::Event::Disconnect(_) = event {
                debug!(
                    "Sending SwarmEvent::Disconnect due to receiving an Event::Disconnect from client {}",
                    bot.entity
                );
                let account = bot.account();
                swarm_tx
                    .send(SwarmEvent::Disconnect(
                        Box::new(account),
                        Box::new(join_opts.clone()),
                    ))
                    .unwrap();
            }

            if let Err(e) = bots_tx.send((Some(event), bot.clone())) {
                error!(
                    "Error sending event to swarm, aborting event_copying_task for {}: {e}",
                    bot.entity
                );
                break;
            }
        }
        debug!(
            "client sender ended for {}, this won't trigger SwarmEvent::Disconnect unless the client already sent its own disconnect event",
            bot.entity
        );
    }

    pub fn client_entities(&self) -> Box<[Entity]> {
        let mut ecs = self.ecs.write();
        let mut query = ecs.query_filtered::<Entity, With<LocalEntity>>();
        query.iter(&ecs).collect::<Box<[Entity]>>()
    }

    pub fn exit(&self) {
        self.ecs.write().write_message(AppExit::Success);
    }
}

impl IntoIterator for Swarm {
    type Item = Client;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        let client_entities = self.client_entities();

        client_entities
            .into_iter()
            .map(|entity| Client::new(entity, self.ecs.clone()))
            .collect::<Box<[Client]>>()
            .into_iter()
    }
}

pub struct DefaultSwarmPlugins;

impl PluginGroup for DefaultSwarmPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(chat::SwarmChatPlugin)
            .add(events::SwarmPlugin)
    }
}

#[derive(Clone, Default, Resource)]
pub struct NoSwarmState;
