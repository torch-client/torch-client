use crate::{log_debug, log_error, log_info, log_warn};
use azalea::FormattedText;
use azalea::prelude::*;
use std::sync::Mutex;
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

use crate::SHARED;
use crate::client::tracking::{
    anim_states, attributes, current_world, equipment, head_yaws, reset_chunk_tracking,
    reset_chunk_tracking_best_effort,
};
use crate::client::{
    bossbar, chat_recv, chat_sign, cooldowns, packets, tablist, tick, viewwindow, worldsync,
};
use crate::play::{flight, no_fall};
use crate::{blockentities, lighting};

static USERNAME: std::sync::RwLock<String> = std::sync::RwLock::new(String::new());

pub(crate) fn set_username(name: String) {
    *USERNAME.write().unwrap() = name;
}

pub(crate) fn username() -> String {
    let name = USERNAME.read().unwrap();
    if name.is_empty() {
        crate::platform::cli::DEFAULT_USERNAME.to_string()
    } else {
        name.clone()
    }
}

async fn run_client(
    account: Account,
    address: impl azalea_protocol::address::ResolvableAddr,
    generation: u64,
    bridged: bool,
) -> azalea::app::AppExit {
    ClientBuilder::new_without_plugins()
        .add_plugins({
            use azalea::app::PluginGroup;
            let group = azalea::DefaultPlugins
                .build()
                .disable::<bevy::log::LogPlugin>();
            #[cfg(feature = "online_mode")]
            let group = group.disable::<azalea::chat_signing::ChatSigningPlugin>();
            group
        })
        .add_plugins({
            use azalea::app::PluginGroup;
            azalea::bot::DefaultBotPlugins
                .build()
                .disable::<azalea::auto_reconnect::AutoReconnectPlugin>()
                .disable::<azalea::auto_respawn::AutoRespawnPlugin>()
                .disable::<azalea::accept_resource_packs::AcceptResourcePacksPlugin>()
        })
        .add_plugins(chat_sign::ChatSigningPlugin)
        .add_plugins(crate::client::outbox::OutboxPlugin)
        .add_plugins(crate::client::tick_rate::TickRatePlugin)
        .add_plugins(flight::FlightPlugin)
        .add_plugins(crate::play::elytra::ElytraPlugin)
        .add_plugins(crate::play::riptide::RiptidePlugin)
        .add_plugins(crate::play::geyser::GeyserPlugin)
        .add_plugins(crate::play::push::PushPlugin)
        .add_plugins(crate::play::piston_push::PistonPushPlugin)
        .add_plugins(no_fall::NoFallPlugin)
        .add_plugins(crate::play::vclip::VClipPlugin)
        .add_plugins(crate::play::riding::RidingPlugin)
        .add_plugins(crate::play::speed::SpeedPlugin)
        .add_plugins(crate::play::velocity::VelocityPlugin)
        .add_plugins(|app: &mut azalea::app::App| {
            app.add_systems(
                azalea_core::tick::GameTick,
                crate::play::pick_box::step_pick_boxes,
            );
        })
        .add_plugins(PacketRelayPlugin)
        .add_plugins(ProtocolPlugin)
        .add_plugins(crate::client::dialogs::ConfigRelayPlugin)
        .add_plugins(crate::client::server_packs::ServerPackPlugin)
        .add_plugins(crate::client::server_screens::CodeOfConductPlugin)
        .add_plugins(BotHeartbeatPlugin)
        .set_handler(handle)
        .set_state(State {
            generation,
            bridged,
            transferring: Default::default(),
        })
        .start(account, address)
        .await
}

static BOT_STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

static BOT_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

static LEAKED_BOT_THREADS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub(crate) fn leaked_bot_threads() -> u32 {
    LEAKED_BOT_THREADS.load(std::sync::atomic::Ordering::Relaxed)
}

const DISCONNECT_LOCK_WAIT: std::time::Duration = std::time::Duration::from_millis(250);

static LIVE_CLIENT: Mutex<Option<Client>> = Mutex::new(None);

fn close_connection() -> bool {
    let Ok(guard) = LIVE_CLIENT.try_lock() else {
        return false;
    };
    let Some(client) = guard.as_ref() else {
        return true;
    };
    let Some(mut ecs) = client.ecs.try_write() else {
        return false;
    };
    ecs.write_message(azalea::disconnect::DisconnectEvent {
        entity: client.entity,
        reason: None,
    });
    true
}

pub(crate) fn shutdown_connection() {
    if let Some(shared) = SHARED.get()
        && let Ok(mut s) = shared.try_lock()
    {
        s.transfer = None;
    }
    close_connection();
    #[cfg(all(unix, not(target_arch = "wasm32")))]
    azalea_protocol::connect::force_close_last_tcp();
}

#[cfg(all(unix, not(target_arch = "wasm32")))]
pub(crate) fn install_exit_signal_handlers() {
    const SIGNALS: [i32; 2] = [2, 15];
    const SIG_DFL: usize = 0;

    unsafe extern "C" {
        fn signal(sig: i32, handler: usize) -> usize;
    }

    unsafe extern "C" fn sever(sig: i32) {
        unsafe extern "C" {
            fn signal(sig: i32, handler: usize) -> usize;
            fn raise(sig: i32) -> i32;
        }
        azalea_protocol::connect::force_close_last_tcp();
        unsafe {
            signal(sig, SIG_DFL);
            raise(sig);
        }
    }

    let handler = sever as unsafe extern "C" fn(i32);
    for sig in SIGNALS {
        unsafe {
            signal(sig, handler as usize);
        }
    }
}

#[cfg(not(all(unix, not(target_arch = "wasm32"))))]
pub(crate) fn install_exit_signal_handlers() {}

pub struct PacketRelayPlugin;

impl azalea::app::Plugin for PacketRelayPlugin {
    fn build(&self, app: &mut azalea::app::App) {
        azalea::connection::set_game_packet_observer(relay_packet);
        app.add_observer(on_transfer);
        app.add_systems(azalea::app::Last, carry_cookies);
        app.add_systems(azalea::app::Update, leave_before_game);
        app.add_systems(azalea::app::Update, crate::client::level_load::publish);
        #[cfg(feature = "audio")]
        app.add_systems(
            azalea::app::Update,
            crate::client::server_sounds::play_server_sounds,
        );
        app.add_systems(
            azalea::app::Update,
            crate::play::event_particles::emit_event_particles,
        );
    }
}

pub(crate) struct ProtocolPlugin;

impl azalea::app::Plugin for ProtocolPlugin {
    #[cfg_attr(
        not(feature = "multiversion"),
        expect(unused_variables, reason = "no system to add without the feature")
    )]
    fn build(&self, app: &mut azalea::app::App) {
        #[cfg(feature = "multiversion")]
        {
            use azalea::ecs::prelude::IntoScheduleConfigs as _;
            app.add_systems(
                azalea::app::PreUpdate,
                install_translator.before(azalea::connection::read_packets),
            );
        }
    }
}

#[cfg(feature = "multiversion")]
fn install_translator(
    mut connections: azalea::ecs::prelude::Query<
        &mut azalea::connection::RawConnection,
        azalea::ecs::prelude::Added<azalea::connection::RawConnection>,
    >,
) {
    for mut connection in &mut connections {
        connection.translator = crate::protocol::translator();
        if connection.translator.is_some() {
            log_info!(
                "net",
                "wire translator installed for protocol {}",
                crate::protocol::session()
            );
        }
    }
}

fn relay_packet(
    ecs: &mut azalea::ecs::prelude::World,
    entity: azalea::ecs::prelude::Entity,
    packet: &azalea_protocol::packets::game::ClientboundGamePacket,
) {
    let Some(shared) = SHARED.get() else {
        return;
    };
    if ecs.get::<State>(entity).is_some_and(State::over) {
        return;
    }
    crate::client::server_packs::game_packet(shared, packet);
    let Some(world) = ecs
        .get::<azalea::local_player::WorldHolder>(entity)
        .map(|w| azalea::local_player::WorldHolder {
            partial: w.partial.clone(),
            shared: w.shared.clone(),
        })
    else {
        return;
    };
    let started = crate::platform::time::Instant::now();
    packets::handle_packet(&world, shared, packet);
    crate::diag::note_event(started.elapsed().as_nanos() as u64, true);
}

fn on_transfer(
    event: azalea::ecs::observer::On<azalea::cookies::TransferEvent>,
    mut commands: azalea::ecs::prelude::Commands,
    query: azalea::ecs::prelude::Query<(
        &State,
        &azalea::connection::RawConnection,
        azalea::ecs::prelude::Has<azalea::InConfigState>,
    )>,
    disconnects: azalea::ecs::prelude::Res<
        azalea::ecs::message::Messages<azalea::disconnect::DisconnectEvent>,
    >,
    mut exits: azalea::ecs::prelude::ResMut<azalea::ecs::message::Messages<azalea::app::AppExit>>,
) {
    let Ok((state, connection, in_config)) = query.get(event.entity) else {
        log_warn!(
            "net",
            "a transfer arrived before the connection had its state; ignoring it"
        );
        return;
    };
    let Some(shared) = SHARED.get() else { return };
    if state.over() {
        return;
    }
    let ending = !connection.is_alive()
        || !exits.is_empty()
        || disconnects
            .get_cursor()
            .read(&disconnects)
            .any(|d| d.entity == event.entity);
    if ending {
        log_info!(
            "net",
            "a transfer arrived on a connection that is already ending; ignoring it"
        );
        return;
    }
    let asked: String = event.host.chars().take(255).collect();
    log_info!(
        "net",
        "the server transferred this client to {asked:?} port {}",
        event.port
    );
    let target = if state.bridged {
        Err(crate::lang::tr!("client.status.transfer_unsupported").to_string())
    } else {
        crate::platform::address::transfer_target(&event.host, event.port)
            .ok_or_else(|| crate::gui::tooltip::translate("connect.failed.transfer", &[]))
    };

    let replies = {
        let mut s = shared.lock().unwrap();
        if stale_generation(state) || s.session_ended || s.quit_requested {
            return;
        }
        state
            .transferring
            .store(true, std::sync::atomic::Ordering::SeqCst);
        s.session.server_packs.settle();
        let target = if s.session.server_packs.left() {
            Err(crate::client::server_packs::declined_reason())
        } else {
            target
        };
        let replies = s.session.server_packs.take_replies();
        match target {
            Ok((host, port)) => {
                let server_packs = s.session.server_packs.detach(in_config);
                let pack_address = s.session.pack_address.take();
                s.transfer = Some(Box::new(crate::session::Transfer {
                    host,
                    port,
                    cookies: Default::default(),
                    server_packs,
                    pack_address,
                    ready: false,
                    claimed: false,
                }));
                s.session_ended = true;
                s.clear_session(crate::gui::tooltip::translate("connect.transferring", &[]));
            }
            Err(reason) => {
                log_warn!("net", "the transfer ends here: {reason}");
                let spans = crate::text::parse_formatted(&reason);
                s.end_session(reason, Some(spans));
            }
        }
        replies
    };
    crate::client::server_packs::send_replies(&mut commands, event.entity, in_config, replies);
    crate::diag::on_disconnected();
    reset_session_side_tables(false);
    exits.write(azalea::app::AppExit::Success);
}

fn carry_cookies(query: azalea::ecs::prelude::Query<(&State, &azalea::cookies::ServerCookies)>) {
    for (state, cookies) in &query {
        if !state.transferring() || stale_generation(state) {
            continue;
        }
        let Some(shared) = SHARED.get() else { return };
        if let Some(transfer) = shared.lock().unwrap().transfer.as_mut() {
            transfer.cookies = cookies.map.clone();
        }
    }
}

fn leave_before_game(
    query: azalea::ecs::prelude::Query<
        (azalea::ecs::prelude::Entity, &State),
        azalea::ecs::prelude::Without<azalea::InGameState>,
    >,
    mut disconnect: azalea::ecs::prelude::MessageWriter<azalea::disconnect::DisconnectEvent>,
) {
    let Ok((entity, state)) = query.single() else {
        return;
    };
    let Some(shared) = SHARED.get() else { return };
    if state.over() || !std::mem::take(&mut shared.lock().unwrap().disconnect_requested) {
        return;
    }
    disconnect.write(azalea::disconnect::DisconnectEvent {
        entity,
        reason: None,
    });
}

fn transfer_ready() {
    let Some(shared) = SHARED.get() else { return };
    {
        let mut s = shared.lock().unwrap();
        let Some(transfer) = s.transfer.as_mut() else {
            return;
        };
        transfer.ready = true;
    }
    start_in_background();
}

pub(crate) fn start_in_background() {
    let Some(shared) = SHARED.get() else { return };
    {
        let mut s = shared.lock().unwrap();
        if !crate::platform::notify::backgrounded() {
            return;
        }
        match s.transfer.as_mut() {
            Some(transfer) if transfer.ready && !transfer.claimed => transfer.claimed = true,
            _ => return,
        }
        s.transfer_unseen = true;
    }
    crate::renderer::input::save_on_session_end();
    start_transfer();
}

pub(crate) fn start_transfer() {
    let Some(shared) = SHARED.get() else { return };
    let transfer = shared.lock().unwrap().transfer.take_if(|t| t.ready);
    let Some(transfer) = transfer else { return };
    let address = crate::platform::address::transfer_name(&transfer.host, transfer.port);
    if let Err(reason) = start(address, Some(transfer)) {
        log_warn!("net", "the transfer was not started: {reason}");
    }
}

pub struct BotHeartbeatPlugin;

impl azalea::app::Plugin for BotHeartbeatPlugin {
    fn build(&self, app: &mut azalea::app::App) {
        app.add_systems(azalea::app::Update, bot_heartbeat);
        #[cfg(feature = "budget")]
        app.add_systems(azalea::app::First, bot_budget_start)
            .add_systems(azalea::app::Last, bot_budget_end)
            .add_systems(azalea_core::tick::GameTick, bot_budget_tick);
    }
}

#[cfg(feature = "budget")]
fn bot_budget_start() {
    crate::diag::budget::bot_start();
}

#[cfg(feature = "budget")]
fn bot_budget_end() {
    crate::diag::budget::bot_end();
}

#[cfg(feature = "budget")]
fn bot_budget_tick() {
    crate::diag::budget::note_tick();
}

fn bot_heartbeat(
    mut updates: azalea::ecs::prelude::Local<u32>,
    mut since: azalea::ecs::prelude::Local<Option<crate::platform::time::Instant>>,
) {
    crate::diag::note_bot_update();
    *updates += 1;
    let now = crate::platform::time::Instant::now();
    let started = since.get_or_insert(now);
    if now.duration_since(*started) >= std::time::Duration::from_secs(1) {
        log_debug!(
            "net",
            "bot app alive: {} updates in the last second",
            *updates
        );
        *updates = 0;
        *started = now;
    }
}

pub(crate) fn start_bot(address: String) -> Result<(), String> {
    if let Err(reason) = crate::platform::address::check(&address) {
        log_error!("net", "{reason}");
        if let Some(shared) = SHARED.get() {
            shared.lock().unwrap().session.status = Some(reason.clone().into());
        }
        return Err(reason);
    }
    start(address, None)
}

fn start(address: String, transfer: Option<Box<crate::session::Transfer>>) -> Result<(), String> {
    if BOT_STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        log_warn!(
            "net",
            "already connected; ignoring a second connection to {address}"
        );
        if let (Some(transfer), Some(shared)) = (transfer, SHARED.get()) {
            let s = shared.lock().unwrap();
            transfer.server_packs.abandon(&s.session.server_packs);
        }
        return Err(crate::lang::tr!("client.status.still_closing").to_string());
    }
    log_info!("net", "connecting to {address}");
    let generation = BOT_GENERATION.load(std::sync::atomic::Ordering::SeqCst);
    let (dial, carried) = match transfer {
        Some(transfer) => {
            let t = *transfer;
            let server = azalea_protocol::address::ServerAddr {
                host: t.host,
                port: t.port,
            };
            (
                Some((server, t.cookies)),
                Some((t.server_packs, t.pack_address)),
            )
        }
        None => (None, None),
    };
    if let Some(shared) = SHARED.get() {
        let mut s = shared.lock().unwrap();
        if carried.is_none() {
            s.transfer = None;
            s.transfer_unseen = false;
        }
        s.session.status = Some(crate::lang::trf!("client.status.connecting", address).into());
        s.session.current_address = Some(address.clone());
        match carried {
            Some((server_packs, pack_address)) => {
                let policy = pack_address
                    .as_deref()
                    .map(crate::gui::serverlist::pack_policy)
                    .unwrap_or_default();
                s.session.server_packs.resume(server_packs, policy);
                s.session.pack_address = pack_address;
                s.session.render_distance_request = s.render_distance_sent;
                s.session.skin_prefs_request = s.skin_prefs_sent;
            }
            None => {
                s.session
                    .server_packs
                    .begin(crate::gui::serverlist::pack_policy(&address));
                s.session.pack_address = Some(address.clone());
            }
        }
        s.disconnect_requested = false;
        s.quit_requested = false;
        s.reload_chunks_requested = false;
        s.remesh_chunks_requested = false;
        s.session_ended = false;
        s.end_notice = None;
    }
    #[cfg(target_arch = "wasm32")]
    let spawn_connection = |body: Box<dyn FnOnce() + 'static>| {
        body();
        true
    };
    #[cfg(not(target_arch = "wasm32"))]
    let spawn_connection = |body: Box<dyn FnOnce() + Send + 'static>| match thread::Builder::new()
        .name("bot".into())
        .spawn(move || {
            crate::diag::alloc::label_thread(crate::diag::alloc::Site::Bot);
            body()
        }) {
        Ok(_) => true,
        Err(e) => {
            log_error!("net", "could not start the bot thread: {e}");
            false
        }
    };
    if dial.is_some() {
        crate::platform::notify::transferred(&address);
    } else {
        crate::platform::notify::connected(&address);
    }
    crate::diag::set_watching(true);
    let spawned = spawn_connection(Box::new(move || {
        struct BotThreadObituary {
            generation: u64,
        }
        impl Drop for BotThreadObituary {
            fn drop(&mut self) {
                if let Ok(mut live) = LIVE_CLIENT.try_lock() {
                    *live = None;
                }
                #[cfg(all(unix, not(target_arch = "wasm32")))]
                azalea_protocol::connect::forget_last_tcp();
                crate::diag::note_bot_stopped();
                if std::thread::panicking() {
                    log_error!(
                        "net",
                        "the bot thread is unwinding from a panic: the connection is gone and \
                         no further packet will be read or answered. The panic report above is \
                         the cause."
                    );
                } else {
                    log_warn!(
                        "net",
                        "the bot thread has ended, so no further packet will be read or \
                         answered."
                    );
                }
                if BOT_GENERATION.load(std::sync::atomic::Ordering::SeqCst) != self.generation {
                    let _ = LEAKED_BOT_THREADS.fetch_update(
                        std::sync::atomic::Ordering::Relaxed,
                        std::sync::atomic::Ordering::Relaxed,
                        |n| Some(n.saturating_sub(1)),
                    );
                    return;
                }
                let (first, notice, transferring) =
                    SHARED.get().map_or((false, None, false), |shared| {
                        let mut s = shared.lock().unwrap();
                        if std::thread::panicking() && s.transfer.take().is_some() {
                            s.session_ended = false;
                        }
                        let status = if s.transfer.is_some() {
                            crate::gui::tooltip::translate("connect.transferring", &[])
                        } else {
                            crate::lang::tr!("client.status.connection_lost").to_string()
                        };
                        let first = s.end_session(status, None);
                        (first, s.end_notice.take(), s.transfer.is_some())
                    });
                if first || transferring {
                    reset_session_side_tables(false);
                }
                crate::client::worker::reset_biome_table();
                if !transferring {
                    crate::platform::notify::disconnected(notice.as_deref());
                }
                crate::diag::set_watching(false);
                BOT_STARTED.store(false, std::sync::atomic::Ordering::SeqCst);
                if transferring {
                    transfer_ready();
                }
            }
        }
        crate::platform::executor::spawn(async move {
            let _obituary = BotThreadObituary { generation };
            #[cfg(feature = "online_mode")]
            let account = match crate::client::auth::account().await {
                Ok(account) => account,
                Err(reason) => {
                    log_error!("net", "{reason}");
                    if let Some(shared) = SHARED.get() {
                        let spans = crate::text::parse_formatted(&reason);
                        shared.lock().unwrap().end_session(reason, Some(spans));
                    }
                    return;
                }
            };
            #[cfg(not(feature = "online_mode"))]
            let account = Account::offline(&username());
            #[cfg(feature = "multiversion")]
            if dial.is_none() {
                crate::protocol::detect(&address).await;
            }
            #[cfg(any(feature = "eagler", target_arch = "wasm32"))]
            let prepared = if dial.is_some() {
                Ok(None)
            } else {
                crate::eagler::prepare(
                    &crate::platform::address::normalize(&address),
                    account.username(),
                )
                .await
            };
            #[cfg(any(feature = "eagler", target_arch = "wasm32"))]
            let bridged = match prepared {
                Ok(bridged) => bridged,
                Err(e) => {
                    let (status, spans) = match e.downcast_ref::<crate::eagler::Refused>() {
                        Some(refused) => {
                            log_warn!("net", "{address} refused the login: {refused}");
                            (
                                crate::lang::trf!("client.status.refused", refused),
                                refused.reason.clone(),
                            )
                        }
                        None => {
                            log_warn!("net", "could not reach {address}: {e}");
                            let status = crate::lang::trf!("client.status.could_not_connect", e);
                            let spans = crate::text::parse_formatted(&status);
                            (status, spans)
                        }
                    };
                    if let Some(shared) = SHARED.get() {
                        shared.lock().unwrap().end_session(status, Some(spans));
                    }
                    return;
                }
            };
            #[cfg(not(any(feature = "eagler", target_arch = "wasm32")))]
            let bridged: Option<azalea_protocol::address::ResolvedAddr> = None;

            let result = if let Some((server, cookies)) = dial {
                let resolved = match azalea_protocol::address::ResolvedAddr::new(server).await {
                    Ok(resolved) => resolved,
                    Err(e) => {
                        log_warn!("net", "could not resolve {address}: {e}");
                        let reason = crate::gui::tooltip::translate(
                            "disconnect.genericReason",
                            &[crate::gui::tooltip::translate(
                                "disconnect.unknownHost",
                                &[],
                            )],
                        );
                        if let Some(shared) = SHARED.get() {
                            let spans = crate::text::parse_formatted(&reason);
                            shared.lock().unwrap().end_session(reason, Some(spans));
                        }
                        return;
                    }
                };
                let cookies = azalea::cookies::ServerCookies { map: cookies };
                if !arm_connection(generation, Some(cookies), false) {
                    return;
                }
                run_client(account, &resolved, generation, false).await
            } else {
                if !arm_connection(generation, None, bridged.is_some()) {
                    return;
                }
                match &bridged {
                    Some(resolved) => run_client(account, resolved, generation, true).await,
                    None => run_client(account, address.as_str(), generation, false).await,
                }
            };
            match result {
                azalea::app::AppExit::Success => log_warn!(
                    "net",
                    "the azalea app returned cleanly, which ends the connection. If you did not \
                     ask to disconnect, something inside the app asked it to exit."
                ),
                azalea::app::AppExit::Error(code) => log_error!("net", "azalea exited: {code:?}"),
            }
        });
    }));
    if !spawned {
        crate::platform::notify::disconnected(None);
        crate::diag::set_watching(false);
        BOT_STARTED.store(false, std::sync::atomic::Ordering::SeqCst);
        if let Some(shared) = SHARED.get() {
            let status = crate::lang::tr!("client.status.thread_failed").to_string();
            let spans = crate::text::parse_formatted(&status);
            shared.lock().unwrap().end_session(status, Some(spans));
        }
    }
    Ok(())
}

fn arm_connection(
    generation: u64,
    cookies: Option<azalea::cookies::ServerCookies>,
    use_transport: bool,
) -> bool {
    let Some(shared) = SHARED.get() else {
        return false;
    };
    let s = shared.lock().unwrap();
    if s.session_ended || generation != BOT_GENERATION.load(std::sync::atomic::Ordering::SeqCst) {
        return false;
    }
    *azalea::join::NEXT_TRANSFER.lock() = cookies;
    azalea::join::USE_TRANSPORT.store(use_transport, std::sync::atomic::Ordering::SeqCst);
    true
}

#[derive(Clone, Component, Default)]
pub struct State {
    generation: u64,
    bridged: bool,
    transferring: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl State {
    fn transferring(&self) -> bool {
        self.transferring.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub(crate) fn over(&self) -> bool {
        self.transferring() || stale_generation(self)
    }
}

fn stale_generation(state: &State) -> bool {
    state.generation != BOT_GENERATION.load(std::sync::atomic::Ordering::SeqCst)
}

async fn handle(bot: Client, event: Event, state: State) -> eyre::Result<()> {
    if stale_generation(&state) {
        bot.exit();
        return Ok(());
    }
    if state.transferring() {
        return Ok(());
    }
    let shared = match SHARED.get() {
        Some(s) => s,
        None => return Ok(()),
    };
    let started = crate::platform::time::Instant::now();
    let result = handle_event(bot, event, shared).await;
    crate::diag::note_event(started.elapsed().as_nanos() as u64, false);
    result
}

async fn handle_event(
    bot: Client,
    event: Event,
    shared: &'static crate::session::SharedMutex,
) -> eyre::Result<()> {
    match event {
        Event::Init => {
            viewwindow::apply_client_information_requests(&bot, shared);
        }
        Event::Login => {
            crate::diag::on_connected();
            if let Ok(mut live) = LIVE_CLIENT.try_lock() {
                *live = Some(bot.clone());
            }
            log_info!(
                "net",
                "connected; waiting for the server to stream the world"
            );
            {
                let mut s = shared.lock().unwrap();
                s.session.status =
                    Some(crate::lang::tr!("client.status.waiting_for_chunks").into());
                if !s.session_ended {
                    s.in_world = true;
                }
            }
            #[cfg(lua_mods)]
            crate::modules::script::joined();
        }
        Event::Spawn => {
            crate::diag::on_spawned();
            log_info!("net", "spawned; the world is up");
            #[cfg(lua_mods)]
            crate::modules::script::spawned();
            let mut s = shared.lock().unwrap();
            if s.session_ended {
                return Ok(());
            }
            s.in_world = true;
            if let Ok(pos) = bot.position() {
                s.session.player_pos = [pos.x, pos.y, pos.z];
                s.session.status = Some(
                    crate::lang::trf!(
                        "client.status.spawned",
                        format!("{:.1}", pos.x),
                        format!("{:.1}", pos.y),
                        format!("{:.1}", pos.z)
                    )
                    .into(),
                );
            }
        }
        Event::Tick => tick::handle_tick(&bot, shared),
        Event::ReceiveChunk(_) => {}
        Event::Chat(m) => chat_recv::handle_chat(&bot, shared, m),
        Event::Death(packet) => {
            let cause = packet.map(|p| crate::client::chat_text::to_spans(&p.message));
            #[cfg(lua_mods)]
            crate::modules::script::died(cause.iter().flatten().map(|s| s.text.as_str()).collect());
            let mut s = shared.lock().unwrap();
            s.session.death_cause = cause;
        }
        Event::Disconnect(reason) => {
            let msg = reason
                .as_ref()
                .map(|r| r.to_string())
                .unwrap_or_else(|| "no reason".to_string());
            crate::diag::on_disconnected();
            log_warn!("net", "disconnected: {msg}");
            bot.exit();
            reset_session_side_tables(false);

            let Ok(mut s) = shared.try_lock_for(DISCONNECT_LOCK_WAIT) else {
                log_warn!(
                    "net",
                    "the shared state stayed locked for longer than the disconnect handler was \
                     willing to wait, so the disconnect screen will not say why this connection \
                     ended. The connection is closed either way."
                );
                return Ok(());
            };
            let status = match &reason {
                Some(_) => crate::lang::trf!("client.status.disconnected_because", msg.as_str()),
                None => crate::lang::tr!("client.status.connection_lost").to_string(),
            };
            s.end_session(
                status,
                reason.as_ref().map(crate::client::chat_text::to_spans),
            );
            if let Some(FormattedText::Translatable(t)) = &reason
                && matches!(
                    t.key.as_str(),
                    "multiplayer.disconnect.chat_validation_failed"
                        | "multiplayer.disconnect.too_many_pending_chats"
                        | "multiplayer.disconnect.illegal_characters"
                )
            {
                s.session.chat_send_error = Some(msg);
            }
            drop(s);
        }
        Event::ConnectionFailed(err) => {
            crate::diag::on_disconnected();
            log_error!("net", "connection failed: {err}");
            bot.exit();
            let Ok(mut s) = shared.try_lock_for(DISCONNECT_LOCK_WAIT) else {
                log_warn!(
                    "net",
                    "the shared state stayed locked for longer than the failed-connection handler \
                     was willing to wait, so the menu will not say why the connection failed."
                );
                return Ok(());
            };
            let status = crate::lang::trf!("client.status.connection_failed", err);
            let spans = crate::text::parse_formatted(&status);
            s.end_session(status, Some(spans));
            drop(s);
        }
        _ => {}
    }
    Ok(())
}

fn reset_session_side_tables(blocking: bool) {
    azalea::registry::unknown::reset();
    #[cfg(feature = "seed_cracker")]
    crate::modules::seed_cracker::disconnected();
    crate::modules::sound_esp::disconnected();
    crate::modules::basefinder::disconnected();
    if blocking {
        reset_chunk_tracking();
    } else {
        reset_chunk_tracking_best_effort();
    }
    worldsync::send_light(lighting::LightJob::Reset);

    if blocking {
        *current_world().lock().unwrap() = None;
        anim_states().lock().unwrap().clear();
        head_yaws().lock().unwrap().clear();
        equipment().lock().unwrap().clear();
        attributes().lock().unwrap().clear();
    } else {
        if let Ok(mut world) = current_world().try_lock() {
            *world = None;
        }
        if let Ok(mut m) = anim_states().try_lock() {
            m.clear();
        }
        if let Ok(mut m) = head_yaws().try_lock() {
            m.clear();
        }
        if let Ok(mut m) = equipment().try_lock() {
            m.clear();
        }
        if let Ok(mut m) = attributes().try_lock() {
            m.clear();
        }
    }
    {
        fn clear<T: Default>(map: &crate::session::SideMutex<T>) {
            if let Ok(mut guard) = map.try_lock() {
                *guard = T::default();
            }
        }
        use crate::client::tracking::{
            add_entity_data, cart_lerps, entity_anims, leashes, meta_dirty, meta_index_18,
            on_grounds, passengers, velocities,
        };
        clear(cart_lerps());
        crate::client::tracking::MINECART_IMPROVEMENTS
            .store(false, std::sync::atomic::Ordering::Relaxed);
        clear(entity_anims());
        clear(meta_index_18());
        clear(meta_dirty());
        clear(on_grounds());
        clear(velocities());
        clear(add_entity_data());
        clear(passengers());
        clear(leashes());
        crate::play::tracking_emitter::reset();
        crate::play::item_use::reset();
        crate::play::arrow_particles::reset();
        crate::play::entity_feed::reset_snapshots();
        #[cfg(feature = "audio")]
        crate::play::riding::sound::reset();
        #[cfg(feature = "audio")]
        crate::play::entity_sounds::reset();
    }
    crate::client::tracking::LOCAL_ENTITY_ID.store(i32::MIN, std::sync::atomic::Ordering::Relaxed);
    crate::client::tracking::set_camera_entity(None);
    crate::play::riding::reset(blocking);
    crate::play::rockets::reset(blocking);
    crate::play::interaction::reset_session_state();
    tick::reset_session_locals(blocking);

    tablist::reset();
    bossbar::reset();
    cooldowns::reset();
    blockentities::feed::reset();
    crate::play::dialog::clear_tags();
    crate::client::recipes::clear_item_tags();
    crate::client::packets::reset_unsecure_warning();
    crate::client::packets::reset_bad_light_layers();
    tick::reset_movement_warning();
}

pub(crate) fn request_disconnect() {
    let Some(shared) = SHARED.get() else { return };
    reset_session_side_tables(true);

    let mut s = shared.lock().unwrap();
    s.disconnect_requested = true;
    s.session_ended = true;
    s.transfer_unseen = false;
    let cancelled = s.transfer.take();
    s.clear_session(crate::lang::tr!("client.status.disconnected").to_string());
    drop(s);
    if cancelled.is_some() {
        crate::platform::notify::disconnected(None);
    }
}

pub(crate) fn force_disconnect() -> bool {
    let Some(shared) = SHARED.get() else {
        return true;
    };
    let Ok(mut s) = shared.try_lock() else {
        return false;
    };
    if !crate::diag::force_disconnect_pending()
        || !BOT_STARTED.load(std::sync::atomic::Ordering::SeqCst)
    {
        return true;
    }
    if s.transfer.take().is_some() {
        s.session_ended = false;
    }
    let status = crate::lang::tr!("client.status.stalled").to_string();
    let spans = crate::text::parse_formatted(crate::lang::tr!("client.status.stalled_reason"));
    s.end_session(status, Some(spans));
    let notice = s.end_notice.take();
    drop(s);

    if !close_connection() {
        log_warn!(
            "net",
            "the bot app's own ECS could not be reached to ask it to close the connection, so \
             the socket is being severed directly instead."
        );
    }
    #[cfg(all(unix, not(target_arch = "wasm32")))]
    azalea_protocol::connect::force_close_last_tcp();
    if let Ok(mut live) = LIVE_CLIENT.try_lock() {
        *live = None;
    }
    crate::platform::notify::disconnected(notice.as_deref());
    crate::diag::set_watching(false);
    BOT_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    BOT_STARTED.store(false, std::sync::atomic::Ordering::SeqCst);
    LEAKED_BOT_THREADS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    reset_session_side_tables(false);
    true
}
