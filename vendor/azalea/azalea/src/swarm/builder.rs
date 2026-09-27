use std::{
    collections::{HashMap, hash_map},
    mem,
    sync::{
        Arc,
        atomic::{self, AtomicBool},
    },
    time::Duration,
};

use azalea_client::{DefaultPlugins, account::Account, start_ecs_runner};
use azalea_protocol::address::{ResolvableAddr, ResolvedAddr};
use azalea_world::Worlds;
use bevy_app::{App, AppExit, Plugins, SubApp};
use bevy_ecs::{component::Component, resource::Resource};
use futures::future::join_all;
use parking_lot::RwLock;
use tokio::{sync::mpsc, task};
use tracing::{debug, error, warn};

use crate::{
    BoxHandleFn, HandleFn, JoinOpts, NoState, TokioRuntimeHandle,
    auto_reconnect::{AutoReconnectDelay, DEFAULT_RECONNECT_DELAY},
    bot::DefaultBotPlugins,
    swarm::{
        BoxSwarmHandleFn, DefaultSwarmPlugins, NoSwarmState, Swarm, SwarmEvent, SwarmHandleFn,
    },
};

pub struct SwarmBuilder<S, SS, R, SR>
where
    S: Send + Sync + Clone + Component + 'static,
    SS: Default + Send + Sync + Clone + Resource + 'static,
    Self: Send,
{
    pub(crate) app: SubApp,
    pub(crate) accounts: Vec<(Account, JoinOpts)>,
    pub(crate) states: Vec<S>,
    pub(crate) swarm_state: SS,
    pub(crate) handler: Option<BoxHandleFn<S, R>>,
    pub(crate) swarm_handler: Option<BoxSwarmHandleFn<SS, SR>>,

    pub(crate) join_delay: Option<Duration>,

    pub(crate) reconnect_after: Option<Duration>,
}
impl SwarmBuilder<NoState, NoSwarmState, (), ()> {
    #[must_use]
    pub fn new() -> Self {
        Self::new_without_plugins().add_plugins((
            DefaultPlugins,
            DefaultBotPlugins,
            DefaultSwarmPlugins,
        ))
    }

    #[must_use]
    pub fn new_without_plugins() -> Self {
        SwarmBuilder {

            app: mem::take(App::new().main_mut()),
            accounts: Vec::new(),
            states: Vec::new(),
            swarm_state: NoSwarmState,
            handler: None,
            swarm_handler: None,
            join_delay: None,
            reconnect_after: Some(DEFAULT_RECONNECT_DELAY),
        }
    }
}

impl<SS, SR> SwarmBuilder<NoState, SS, (), SR>
where
    SS: Default + Send + Sync + Clone + Resource + 'static,
{
    #[must_use]
    pub fn set_handler<S, Fut, R>(self, handler: HandleFn<S, Fut>) -> SwarmBuilder<S, SS, R, SR>
    where
        Fut: Future<Output = R> + Send + 'static,
        S: Send + Sync + Clone + Component + Default + 'static,
    {
        SwarmBuilder {
            handler: Some(Box::new(move |bot, event, state: S| {
                Box::pin(handler(bot, event, state))
            })),
            states: vec![S::default(); self.accounts.len()],
            app: self.app,
            ..self
        }
    }
}

impl<S, R> SwarmBuilder<S, NoSwarmState, R, ()>
where
    S: Send + Sync + Clone + Component + 'static,
{
    #[must_use]
    pub fn set_swarm_handler<SS, Fut, SR>(
        self,
        handler: SwarmHandleFn<SS, Fut>,
    ) -> SwarmBuilder<S, SS, R, SR>
    where
        SS: Default + Send + Sync + Clone + Resource + 'static,
        Fut: Future<Output = SR> + Send + 'static,
    {
        SwarmBuilder {
            handler: self.handler,
            app: self.app,
            accounts: self.accounts,
            states: self.states,
            swarm_state: SS::default(),
            swarm_handler: Some(Box::new(move |swarm, event, state| {
                Box::pin(handler(swarm, event, state))
            })),
            join_delay: self.join_delay,
            reconnect_after: self.reconnect_after,
        }
    }
}

impl<S, SS, R, SR> SwarmBuilder<S, SS, R, SR>
where
    S: Send + Sync + Clone + Component + 'static,
    SS: Default + Send + Sync + Clone + Resource + 'static,
    R: Send + 'static,
    SR: Send + 'static,
{
    #[must_use]
    pub fn add_accounts(mut self, accounts: Vec<Account>) -> Self
    where
        S: Default,
    {
        for account in accounts {
            self = self.add_account(account);
        }

        self
    }

    #[must_use]
    pub fn add_account(self, account: Account) -> Self
    where
        S: Default,
    {
        self.add_account_with_state_and_opts(account, S::default(), JoinOpts::default())
    }

    #[must_use]
    pub fn add_account_with_state(self, account: Account, state: S) -> Self {
        self.add_account_with_state_and_opts(account, state, JoinOpts::default())
    }

    #[must_use]
    pub fn add_account_with_opts(self, account: Account, opts: JoinOpts) -> Self
    where
        S: Default,
    {
        self.add_account_with_state_and_opts(account, S::default(), opts)
    }

    #[must_use]
    pub fn add_account_with_state_and_opts(
        mut self,
        account: Account,
        state: S,
        join_opts: JoinOpts,
    ) -> Self {
        self.accounts.push((account, join_opts));
        self.states.push(state);
        self
    }

    #[must_use]
    pub fn set_swarm_state(mut self, swarm_state: SS) -> Self {
        self.swarm_state = swarm_state;
        self
    }

    #[must_use]
    pub fn add_plugins<M>(mut self, plugins: impl Plugins<M>) -> Self {
        self.app.add_plugins(plugins);
        self
    }

    #[must_use]
    pub fn join_delay(mut self, delay: Duration) -> Self {
        self.join_delay = Some(delay);
        self
    }

    #[must_use]
    pub fn reconnect_after(mut self, delay: impl Into<Option<Duration>>) -> Self {
        self.reconnect_after = delay.into();
        self
    }

    pub async fn start(self, address: impl ResolvableAddr) -> AppExit {
        self.start_with_opts(address, JoinOpts::default()).await
    }

    #[doc(hidden)]
    #[deprecated = "renamed to `start_with_opts`."]
    pub async fn start_with_default_opts(
        self,
        address: impl ResolvableAddr,
        default_join_opts: JoinOpts,
    ) -> AppExit {
        self.start_with_opts(address, default_join_opts).await
    }

    pub async fn start_with_opts(
        mut self,
        address: impl ResolvableAddr,
        join_opts: JoinOpts,
    ) -> AppExit {
        assert_eq!(
            self.accounts.len(),
            self.states.len(),
            "There must be exactly one state per bot."
        );

        debug!("Starting Azalea {}", env!("CARGO_PKG_VERSION"));

        let address = if let Some(socket_addr) = join_opts.custom_socket_addr {
            let server_addr = if let Some(server_addr) = join_opts
                .custom_server_addr
                .clone()
                .or_else(|| address.clone().server_addr().ok())
            {
                server_addr
            } else {
                error!(
                    "Failed to parse address: {address:?}. If this was expected, consider passing in a `ServerAddr` instead."
                );
                return AppExit::error();
            };

            ResolvedAddr {
                server: server_addr,
                socket: socket_addr,
            }
        } else {
            let Ok(addr) = address.clone().resolve().await else {
                error!(
                    "Failed to resolve address: {address:?}. If this was expected, consider resolving the address earlier with `ResolvableAddr::resolve`."
                );
                return AppExit::error();
            };
            addr
        };

        let worlds = Arc::new(RwLock::new(Worlds::default()));

        let (bots_tx, mut bots_rx) = mpsc::unbounded_channel();
        let (swarm_tx, mut swarm_rx) = mpsc::unbounded_channel();

        swarm_tx.send(SwarmEvent::Init).unwrap();

        let main_schedule_label = self.app.update_schedule.unwrap();

        let local_set = task::LocalSet::new();

        local_set.run_until(async move {
            let (ecs_lock, start_running_systems, appexit_rx) = start_ecs_runner(&mut self.app);

            let swarm = Swarm {
                ecs: ecs_lock.clone(),

                address: Arc::new(RwLock::new(address)),
                worlds,

                bots_tx,

                swarm_tx: swarm_tx.clone(),
            };

            {
                let mut ecs = ecs_lock.write();
                #[cfg(not(target_arch = "wasm32"))]
                ecs.insert_resource(TokioRuntimeHandle(tokio::runtime::Handle::current()));
                ecs.insert_resource(swarm.clone());
                ecs.insert_resource(self.swarm_state.clone());
                if let Some(reconnect_after) = self.reconnect_after {
                    ecs.insert_resource(AutoReconnectDelay {
                        delay: reconnect_after,
                    });
                } else {
                    ecs.remove_resource::<AutoReconnectDelay>();
                }
                ecs.run_schedule(main_schedule_label);
                ecs.clear_trackers();
            }

            start_running_systems();

            let swarm_clone = swarm.clone();
            let join_delay = self.join_delay;
            let accounts = self.accounts.clone();
            let states = self.states.clone();

            task::spawn_local(async move {
                if let Some(join_delay) = join_delay {
                    for ((account, bot_join_opts), state) in accounts.iter().zip(states) {
                        let mut join_opts = join_opts.clone();
                        join_opts.update(bot_join_opts);
                        let _ = swarm_clone.add_with_opts(account, state, &join_opts).await;
                        azalea_client::sleep(join_delay).await;
                    }
                } else {
                    let swarm_borrow = &swarm_clone;
                    join_all(accounts.iter().zip(states).map(
                        |((account, bot_join_opts), state)| async {
                            let mut join_opts = join_opts.clone();
                            join_opts.update(bot_join_opts);
                            let _ = swarm_borrow
                                .clone()
                                .add_with_opts(account, state, &join_opts)
                                .await;
                        },
                    ))
                    .await;
                }

                swarm_tx.send(SwarmEvent::Login).unwrap();
            });

            let swarm_state = self.swarm_state;

            let swarm_clone = swarm.clone();
            let swarm_handler_task = task::spawn_local(async move {
                while let Some(event) = swarm_rx.recv().await {
                    if let Some(swarm_handler) = &self.swarm_handler {
                        task::spawn_local((swarm_handler)(
                            swarm_clone.clone(),
                            event,
                            swarm_state.clone(),
                        ));
                    }
                }

                unreachable!(
                    "The `Swarm` here contains a sender for the `SwarmEvent`s, so swarm_rx.recv() will never fail"
                );
            });

            let client_handler_task = task::spawn_local(async move {
                while let Some((Some(first_event), first_bot)) = bots_rx.recv().await {
                    if bots_rx.len() > 1_000 {
                        static WARNED: AtomicBool = AtomicBool::new(false);
                        if !WARNED.swap(true, atomic::Ordering::Relaxed) {
                            warn!("the Client Event channel has more than 1000 items!")
                        }
                    }

                    if let Some(handler) = &self.handler {
                        let ecs_mutex = first_bot.ecs.clone();
                        let mut ecs = ecs_mutex.write();
                        let mut query = ecs.query::<Option<&S>>();
                        let Ok(Some(first_bot_state)) = query.get(&ecs, first_bot.entity) else {
                            error!(
                                "the first bot ({} / {}) is missing the required state component! none of the client handler functions will be called.",
                                first_bot.username(),
                                first_bot.entity
                            );
                            continue;
                        };
                        let first_bot_entity = first_bot.entity;
                        let first_bot_state = first_bot_state.clone();

                        task::spawn_local((handler)(first_bot, first_event, first_bot_state.clone()));

                        let mut states = HashMap::new();
                        states.insert(first_bot_entity, first_bot_state);
                        while let Ok((Some(event), bot)) = bots_rx.try_recv() {
                            let state = match states.entry(bot.entity) {
                                hash_map::Entry::Occupied(e) => e.into_mut(),
                                hash_map::Entry::Vacant(e) => {
                                    let Ok(Some(state)) = query.get(&ecs, bot.entity) else {
                                        error!(
                                            "one of our bots ({} / {}) is missing the required state component! its client handler function will not be called.",
                                            bot.username(),
                                            bot.entity
                                        );
                                        continue;
                                    };
                                    let state = state.clone();
                                    e.insert(state)
                                }
                            };
                            task::spawn_local((handler)(bot, event, state.clone()));
                        }
                    }
                }
            });

            let app_exit = appexit_rx
                .await
                .expect("appexit_tx shouldn't be dropped by the ECS runner before sending");

            swarm_handler_task.abort();
            client_handler_task.abort();

            app_exit
        }).await
    }
}

impl Default for SwarmBuilder<NoState, NoSwarmState, (), ()> {
    fn default() -> Self {
        Self::new()
    }
}
