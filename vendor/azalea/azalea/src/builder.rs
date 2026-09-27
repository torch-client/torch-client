use std::time::Duration;

use azalea_client::{DefaultPlugins, account::Account};
use azalea_protocol::address::ResolvableAddr;
use bevy_app::{AppExit, Plugins};
use bevy_ecs::component::Component;

use crate::{
    HandleFn, JoinOpts, NoState,
    bot::DefaultBotPlugins,
    swarm::{self, SwarmBuilder},
};

pub struct ClientBuilder<S, R>
where
    S: Default + Send + Sync + Clone + Component + 'static,
    R: Send + 'static,
    Self: Send,
{
    swarm: SwarmBuilder<S, swarm::NoSwarmState, R, ()>,
}
impl ClientBuilder<NoState, ()> {
    #[must_use]
    pub fn new() -> Self {
        Self::new_without_plugins()
            .add_plugins(DefaultPlugins)
            .add_plugins(DefaultBotPlugins)
    }

    #[must_use]
    pub fn new_without_plugins() -> Self {
        Self {
            swarm: SwarmBuilder::new_without_plugins(),
        }
    }

    #[must_use]
    pub fn set_handler<S, Fut, R>(self, handler: HandleFn<S, Fut>) -> ClientBuilder<S, R>
    where
        S: Default + Send + Sync + Clone + Component + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        ClientBuilder {
            swarm: self.swarm.set_handler(handler),
        }
    }
}
impl<S, R> ClientBuilder<S, R>
where
    S: Default + Send + Sync + Clone + Component + 'static,
    R: Send + 'static,
{
    #[must_use]
    pub fn set_state(mut self, state: S) -> Self {
        self.swarm.states = vec![state];
        self
    }
    #[must_use]
    pub fn add_plugins<M>(mut self, plugins: impl Plugins<M>) -> Self {
        self.swarm = self.swarm.add_plugins(plugins);
        self
    }

    #[must_use]
    pub fn reconnect_after(mut self, delay: impl Into<Option<Duration>>) -> Self {
        self.swarm.reconnect_after = delay.into();
        self
    }

    pub async fn start(mut self, account: Account, address: impl ResolvableAddr) -> AppExit {
        self.swarm.accounts = vec![(account, JoinOpts::default())];
        if self.swarm.states.is_empty() {
            self.swarm.states = vec![S::default()];
        }
        self.swarm.start(address).await
    }

    pub async fn start_with_opts(
        mut self,
        account: Account,
        address: impl ResolvableAddr,
        opts: JoinOpts,
    ) -> AppExit {
        self.swarm.accounts = vec![(account, opts.clone())];
        if self.swarm.states.is_empty() {
            self.swarm.states = vec![S::default()];
        }
        self.swarm.start_with_opts(address, opts).await
    }
}
impl Default for ClientBuilder<NoState, ()> {
    fn default() -> Self {
        Self::new()
    }
}
