use crate::{log_debug, log_error, log_info, log_warn};
use azalea::FormattedText;
use azalea::prelude::*;
use std::sync::Mutex;
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

use crate::SHARED;
use crate::client::tracking::{
    anim_states, current_world, equipment, head_yaws, max_healths, reset_chunk_tracking,
    reset_chunk_tracking_best_effort,
};
use crate::client::{
    bossbar, chat_recv, chat_sign, cooldowns, packets, tablist, tick, viewwindow, worldsync,
};
use crate::play::{flight, no_fall};
use crate::session::SessionState;
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
        })
        .add_plugins(chat_sign::ChatSigningPlugin)
        .add_plugins(flight::FlightPlugin)
        .add_plugins(no_fall::NoFallPlugin)
        .add_plugins(crate::play::riding::RidingPlugin)
        .add_plugins(PacketRelayPlugin)
        .add_plugins(ProtocolPlugin)
        .add_plugins(crate::client::dialogs::ConfigRelayPlugin)
        .add_plugins(BotHeartbeatPlugin)
        .set_handler(handle)
        .set_state(State { generation })
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
        app.add_systems(azalea::app::Update, relay_packets);
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

fn relay_packets(
    mut events: azalea::ecs::prelude::MessageReader<azalea::packet::game::ReceiveGamePacketEvent>,
    worlds: azalea::ecs::prelude::Query<&azalea::local_player::WorldHolder>,
) {
    let Some(shared) = SHARED.get() else {
        return;
    };
    for event in events.read() {
        let Ok(world) = worlds.get(event.entity) else {
            continue;
        };
        let started = crate::platform::time::Instant::now();
        packets::handle_packet(world, shared, event.packet.as_ref());
        crate::diag::note_event(started.elapsed().as_nanos() as u64, true);
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

pub(crate) fn start_bot(address: String) {
    if let Err(reason) = crate::platform::address::check(&address) {
        log_error!("net", "{reason}");
        if let Some(shared) = SHARED.get() {
            shared.lock().unwrap().session.status = reason;
        }
        return;
    }
    if BOT_STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        log_warn!(
            "net",
            "already connected; ignoring a second connection to {address}"
        );
        return;
    }
    log_info!("net", "connecting to {address}");
    let generation = BOT_GENERATION.load(std::sync::atomic::Ordering::SeqCst);
    crate::diag::clear_session_killed();
    if let Some(shared) = SHARED.get() {
        let mut s = shared.lock().unwrap();
        s.session.status = format!("Connecting to {address}...");
        s.session.current_address = Some(address.clone());
        s.disconnect_requested = false;
        s.quit_requested = false;
        s.reload_chunks_requested = false;
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
    let spawned = spawn_connection(Box::new(move || {
        struct BotThreadObituary;
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
                         the cause; the window will stay up and stay empty until you reconnect."
                    );
                } else {
                    log_warn!(
                        "net",
                        "the bot thread has ended, so no further packet will be read or \
                         answered. The window will stay up and stay empty until you reconnect."
                    );
                }
            }
        }
        let _obituary = BotThreadObituary;
        crate::platform::executor::spawn(async move {
            #[cfg(feature = "online_mode")]
            let account = match crate::client::auth::account().await {
                Ok(account) => account,
                Err(reason) => {
                    log_error!("net", "{reason}");
                    if let Some(shared) = SHARED.get() {
                        shared.lock().unwrap().session.status = reason;
                    }
                    return;
                }
            };
            #[cfg(not(feature = "online_mode"))]
            let account = Account::offline(&username());
            #[cfg(feature = "multiversion")]
            crate::protocol::detect(&address).await;
            #[cfg(any(feature = "eagler", target_arch = "wasm32"))]
            let bridged = match crate::eagler::prepare(
                &crate::platform::address::normalize(&address),
                account.username(),
            )
            .await
            {
                Ok(bridged) => bridged,
                Err(e) => {
                    log_error!("net", "could not reach {address}: {e}");
                    if let Some(shared) = SHARED.get() {
                        shared.lock().unwrap().session.status = format!("Could not connect: {e}");
                    }
                    return;
                }
            };
            #[cfg(not(any(feature = "eagler", target_arch = "wasm32")))]
            let bridged: Option<azalea_protocol::address::ResolvedAddr> = None;

            let result = match &bridged {
                Some(resolved) => run_client(account, resolved, generation).await,
                None => run_client(account, address.as_str(), generation).await,
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
        if BOT_GENERATION.load(std::sync::atomic::Ordering::SeqCst) == generation {
            BOT_STARTED.store(false, std::sync::atomic::Ordering::SeqCst);
            crate::client::worker::reset_biome_table();
        } else {
            let _ = LEAKED_BOT_THREADS.fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |n| Some(n.saturating_sub(1)),
            );
        }
    }));
    if !spawned {
        BOT_STARTED.store(false, std::sync::atomic::Ordering::SeqCst);
        if let Some(shared) = SHARED.get() {
            let mut s = shared.lock().unwrap();
            s.session.status = "Could not start the connection thread".to_string();
            s.session.current_address = None;
            s.disconnected_pending = true;
        }
    }
}

#[derive(Clone, Component, Default)]
pub struct State {
    generation: u64,
}

fn stale_generation(state: &State) -> bool {
    state.generation != BOT_GENERATION.load(std::sync::atomic::Ordering::SeqCst)
}

async fn handle(bot: Client, event: Event, state: State) -> eyre::Result<()> {
    if stale_generation(&state) {
        bot.exit();
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
    shared: &'static std::sync::Arc<crate::session::SharedMutex>,
) -> eyre::Result<()> {
    match event {
        Event::Init => {
            viewwindow::apply_render_distance_request(&bot, shared);
            viewwindow::apply_skin_prefs_request(&bot, shared);
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
            shared.lock().unwrap().session.status = "Connected — waiting for chunks...".to_string();
        }
        Event::Spawn => {
            crate::diag::on_spawned();
            log_info!("net", "spawned; the world is up");
            let mut s = shared.lock().unwrap();
            s.in_world = true;
            if let Ok(pos) = bot.position() {
                s.session.player_pos = [pos.x as f32, pos.y as f32, pos.z as f32];
                s.session.status = format!("Spawned at ({:.1},{:.1},{:.1})", pos.x, pos.y, pos.z);
            }
        }
        Event::Tick => tick::handle_tick(&bot, shared),
        Event::ReceiveChunk(chunk_pos) => {
            worldsync::note_chunk_received(&bot, chunk_pos.x, chunk_pos.z)
        }
        Event::Chat(m) => chat_recv::handle_chat(&bot, shared, m),
        Event::Death(packet) => {
            let cause = packet.map(|p| crate::client::chat_text::to_spans(&p.message));
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
            s.in_world = false;
            s.disconnected_pending = true;
            s.disconnect_reason = reason.as_ref().map(crate::client::chat_text::to_spans);
            let health_epoch = s.session.health_epoch;
            s.session = SessionState::default();
            s.session.health_epoch = health_epoch;
            s.session.clear_chunks = true;
            s.session.status = format!("Disconnected: {}", msg);
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
            s.in_world = false;
            s.disconnected_pending = true;
            let health_epoch = s.session.health_epoch;
            s.session = SessionState::default();
            s.session.health_epoch = health_epoch;
            s.session.clear_chunks = true;
            s.session.status = format!("Connection failed: {}", err);
            drop(s);
        }
        _ => {}
    }
    Ok(())
}

fn reset_session_side_tables(blocking: bool) {
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
        max_healths().lock().unwrap().clear();
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
        if let Ok(mut m) = max_healths().try_lock() {
            m.clear();
        }
    }
    crate::play::riding::reset(blocking);

    tablist::reset();
    bossbar::reset();
    cooldowns::reset();
    blockentities::feed::reset();
    crate::play::dialog::clear_tags();
    crate::client::advancements::reset();
    crate::client::packets::reset_unsecure_warning();
    crate::client::packets::reset_bad_light_layers();
    tick::reset_movement_warning();
}

pub(crate) fn request_disconnect() {
    let Some(shared) = SHARED.get() else { return };
    reset_session_side_tables(true);

    let mut s = shared.lock().unwrap();
    s.disconnect_requested = true;
    s.in_world = false;
    let health_epoch = s.session.health_epoch;
    s.session = SessionState::default();
    s.session.health_epoch = health_epoch;
    s.session.clear_chunks = true;
    s.session.status = "Disconnected".to_string();
}

pub(crate) fn force_disconnect() -> bool {
    let Some(shared) = SHARED.get() else {
        return true;
    };
    let Ok(mut s) = shared.try_lock() else {
        return false;
    };
    s.disconnect_requested = true;
    s.in_world = false;
    s.disconnected_pending = true;
    s.disconnect_by_player = true;
    let health_epoch = s.session.health_epoch;
    s.session = SessionState::default();
    s.session.health_epoch = health_epoch;
    s.session.clear_chunks = true;
    s.session.status = "Disconnected (bot thread stalled)".to_string();
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
    BOT_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    BOT_STARTED.store(false, std::sync::atomic::Ordering::SeqCst);
    LEAKED_BOT_THREADS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    reset_session_side_tables(false);
    true
}
