use std::{collections::HashMap, sync::Arc};

use azalea_auth::game_profile::GameProfile;
use azalea_client::{
    DefaultPlugins,
    account::Account,
    connection::RawConnection,
    disconnect::DisconnectEvent,
    join::{ConnectOpts, StartJoinServerEvent},
    local_player::{Experience, Hunger, TabList, WorldHolder},
    packet::game::SendGamePacketEvent,
    player::{GameProfileComponent, PlayerInfo},
    start_ecs_runner,
    tick_counter::TicksConnected,
};
use azalea_core::{
    data_registry::{DataRegistryWithKey, ResolvableDataRegistry},
    entity_id::MinecraftEntityId,
};
use azalea_entity::indexing::{EntityIdIndex, EntityUuidIndex};
use azalea_protocol::{
    address::{ResolvableAddr, ResolvedAddr},
    connect::Proxy,
    packets::{Packet, game::ServerboundGamePacket},
    resolve::ResolveError,
};
use azalea_registry::{DataRegistryKeyRef, identifier::Identifier};
use azalea_world::{PartialWorld, World, WorldName};
use bevy_app::{App, AppExit};
use bevy_ecs::{entity::Entity, resource::Resource, world::Mut};
use parking_lot::RwLock;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::{
    bot::DefaultBotPlugins,
    client_impl::error::AzaleaResult,
    entity_ref::EntityRef,
    events::{Event, LocalPlayerEvents},
    swarm::DefaultSwarmPlugins,
};

pub mod attack;
pub mod chat;
pub mod client_information;
pub mod entity_query;
pub mod error;
pub mod interact;
pub mod inventory;
pub mod mining;
pub mod movement;

#[derive(Clone)]
pub struct Client {
    pub entity: Entity,

    pub ecs: Arc<RwLock<bevy_ecs::world::World>>,
}

pub struct StartClientOpts {
    pub ecs_lock: Arc<RwLock<bevy_ecs::world::World>>,
    pub account: Account,
    pub connect_opts: ConnectOpts,
    pub event_sender: Option<mpsc::UnboundedSender<Event>>,
}

impl StartClientOpts {
    pub fn new(
        account: Account,
        address: ResolvedAddr,
        event_sender: Option<mpsc::UnboundedSender<Event>>,
    ) -> StartClientOpts {
        Self::new_with_appexit_rx(account, address, event_sender).0
    }

    pub fn new_with_appexit_rx(
        account: Account,
        address: ResolvedAddr,
        event_sender: Option<mpsc::UnboundedSender<Event>>,
    ) -> (StartClientOpts, oneshot::Receiver<AppExit>) {
        let mut app = App::new();
        app.add_plugins((DefaultPlugins, DefaultBotPlugins, DefaultSwarmPlugins));

        let (ecs_lock, start_running_systems, appexit_rx) = start_ecs_runner(app.main_mut());
        start_running_systems();

        (
            Self {
                ecs_lock,
                account,
                connect_opts: ConnectOpts {
                    address,
                    server_proxy: None,
                    sessionserver_proxy: None,
                },
                event_sender,
            },
            appexit_rx,
        )
    }

    pub fn proxy(self, proxy: Proxy) -> Self {
        self.server_proxy(proxy.clone()).sessionserver_proxy(proxy)
    }
    pub fn server_proxy(mut self, proxy: Proxy) -> Self {
        self.connect_opts.server_proxy = Some(proxy);
        self
    }
    pub fn sessionserver_proxy(mut self, proxy: Proxy) -> Self {
        self.connect_opts.sessionserver_proxy = Some(proxy);
        self
    }
}

impl Client {
    pub fn new(entity: Entity, ecs: Arc<RwLock<bevy_ecs::world::World>>) -> Self {
        Self {
            entity,

            ecs,
        }
    }

    pub async fn join(
        account: Account,
        address: impl ResolvableAddr,
    ) -> Result<(Self, mpsc::UnboundedReceiver<Event>), ResolveError> {
        let address = address.resolve().await?;
        let (tx, rx) = mpsc::unbounded_channel();

        let client = Self::start_client(StartClientOpts::new(account, address, Some(tx))).await;
        Ok((client, rx))
    }

    pub async fn join_with_proxy(
        account: Account,
        address: impl ResolvableAddr,
        proxy: Proxy,
    ) -> Result<(Self, mpsc::UnboundedReceiver<Event>), ResolveError> {
        let address = address.resolve().await?;
        let (tx, rx) = mpsc::unbounded_channel();

        let client =
            Self::start_client(StartClientOpts::new(account, address, Some(tx)).proxy(proxy)).await;
        Ok((client, rx))
    }

    pub async fn start_client(
        StartClientOpts {
            ecs_lock,
            account,
            connect_opts,
            event_sender,
        }: StartClientOpts,
    ) -> Self {

        let (start_join_callback_tx, mut start_join_callback_rx) =
            mpsc::unbounded_channel::<Entity>();

        ecs_lock.write().write_message(StartJoinServerEvent {
            account,
            connect_opts,
            start_join_callback_tx: Some(start_join_callback_tx),
        });

        let entity = start_join_callback_rx.recv().await.expect(
            "start_join_callback should not be dropped before sending a message, this is a bug in Azalea",
        );

        if let Some(event_sender) = event_sender {
            ecs_lock
                .write()
                .entity_mut(entity)
                .insert(LocalPlayerEvents(event_sender));
        }

        Client::new(entity, ecs_lock)
    }

    pub fn write_packet(&self, packet: impl Packet<ServerboundGamePacket>) {
        let packet = packet.into_variant();
        self.ecs
            .write()
            .commands()
            .trigger(SendGamePacketEvent::new(self.entity, packet));
    }

    pub fn disconnect(&self) {
        self.ecs.write().write_message(DisconnectEvent {
            entity: self.entity,
            reason: None,
        });
    }

    pub fn exit(&self) {
        self.ecs.write().write_message(AppExit::Success);
    }

    pub fn with_raw_connection<R>(&self, f: impl FnOnce(&RawConnection) -> R) -> AzaleaResult<R> {
        self.query_self::<&RawConnection, _>(f)
    }
    pub fn with_raw_connection_mut<R>(
        &self,
        f: impl FnOnce(Mut<'_, RawConnection>) -> R,
    ) -> AzaleaResult<R> {
        self.query_self::<&mut RawConnection, _>(f)
    }

    pub fn resource<T: Resource + Clone>(&self) -> T {
        self.ecs.read().resource::<T>().clone()
    }

    pub fn map_resource<T: Resource, R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let ecs = self.ecs.read();
        let value = ecs.resource::<T>();
        f(value)
    }

    pub fn map_get_resource<T: Resource, R>(&self, f: impl FnOnce(Option<&T>) -> R) -> R {
        let ecs = self.ecs.read();
        let value = ecs.get_resource::<T>();
        f(value)
    }

    pub fn world(&self) -> AzaleaResult<Arc<RwLock<World>>> {
        let world_holder = self.component::<WorldHolder>()?;
        Ok(world_holder.shared.clone())
    }

    pub fn partial_world(&self) -> AzaleaResult<Arc<RwLock<PartialWorld>>> {
        let world_holder = self.component::<WorldHolder>()?;
        Ok(world_holder.partial.clone())
    }

    pub fn logged_in(&self) -> bool {
        self.query_self::<&WorldName, _>(|_| {}).is_ok()
    }

    pub fn entity(&self) -> EntityRef {
        self.entity_ref_for(self.entity)
    }

    pub fn entity_ref_for(&self, entity: Entity) -> EntityRef {
        EntityRef::new(self.clone(), entity)
    }
}

impl Client {
    pub fn hunger(&self) -> AzaleaResult<Hunger> {
        Ok(self.component::<Hunger>()?.to_owned())
    }

    pub fn experience(&self) -> AzaleaResult<Experience> {
        Ok(self.component::<Experience>()?.to_owned())
    }

    pub fn username(&self) -> String {
        self.account().username().to_owned()
    }
    pub fn server_username(&self) -> AzaleaResult<String> {
        Ok(self.profile()?.name.to_owned())
    }

    pub fn uuid(&self) -> Uuid {
        self.account().uuid()
    }

    pub fn tab_list(&self) -> AzaleaResult<HashMap<Uuid, PlayerInfo>> {
        Ok((**self.component::<TabList>()?).clone())
    }

    pub fn profile(&self) -> AzaleaResult<GameProfile> {
        Ok((**self.component::<GameProfileComponent>()?).clone())
    }

    pub fn account(&self) -> Account {
        self.component::<Account>()
            .expect(
                "clients cannot exist without an Account, and Account isn't removed from clients",
            )
            .clone()
    }

    pub fn player_uuid_by_username(&self, username: &str) -> AzaleaResult<Option<Uuid>> {
        Ok(self
            .tab_list()?
            .values()
            .find(|player| player.profile.name == username)
            .map(|player| player.profile.uuid))
    }

    pub fn entity_id_by_uuid(&self, uuid: Uuid) -> Option<Entity> {
        self.map_resource::<EntityUuidIndex, _>(|entity_uuid_index| entity_uuid_index.get(&uuid))
    }
    pub fn entity_by_uuid(&self, uuid: Uuid) -> Option<EntityRef> {
        self.entity_id_by_uuid(uuid).map(|e| self.entity_ref_for(e))
    }

    pub fn entity_id_by_minecraft_id(&self, id: MinecraftEntityId) -> AzaleaResult<Option<Entity>> {
        self.query_self::<&EntityIdIndex, _>(|entity_id_index| {
            entity_id_index.get_by_minecraft_entity(id)
        })
    }
    pub fn entity_by_minecraft_id(&self, id: MinecraftEntityId) -> AzaleaResult<Option<EntityRef>> {
        Ok(self
            .entity_id_by_minecraft_id(id)?
            .map(|e| EntityRef::new(self.clone(), e)))
    }

    pub fn with_registry_holder<R>(
        &self,
        f: impl FnOnce(&azalea_core::registry_holder::RegistryHolder) -> R,
    ) -> AzaleaResult<R> {
        let world = self.world()?;
        let registries = &world.read().registries;
        Ok(f(registries))
    }

    #[deprecated = "use `bot.resolve_registry_key(registry).map(|r| r.into_ident())` instead."]
    pub fn resolve_registry_name(
        &self,
        registry: &impl ResolvableDataRegistry,
    ) -> AzaleaResult<Option<Identifier>> {
        self.with_registry_holder(|registries| registry.key(registries).map(|r| r.into_ident()))
    }

    pub fn resolve_registry_key<R: ResolvableDataRegistry>(
        &self,
        registry: &R,
    ) -> AzaleaResult<Option<R::Key>> {
        self.with_registry_holder(|registries| registry.key_owned(registries))
    }

    pub fn with_resolved_registry<R: ResolvableDataRegistry, Ret>(
        &self,
        registry: R,
        f: impl FnOnce(&Identifier, &R::DeserializesTo) -> Ret,
    ) -> AzaleaResult<Option<Ret>> {
        self.with_registry_holder(|registries| {
            registry
                .resolve(registries)
                .map(|(name, data)| f(name, data))
        })
    }

    pub fn ticks_connected(&self) -> u64 {
        self.component::<TicksConnected>().map(|c| c.0).unwrap_or(0)
    }
}
