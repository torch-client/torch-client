use bevy_platform::time::Instant;
use std::{
    fmt::Debug,
    mem,
    sync::Arc,
    thread,
    time::Duration,
};

use azalea_core::tick::GameTick;
use azalea_entity::{
    EntityUpdateSystems, PlayerAbilities, indexing::EntityIdIndex, inventory::Inventory,
};
use azalea_physics::client_movement::ClientMovementState;
use azalea_world::Worlds;
use bevy_app::{App, AppExit, Plugin, PluginsState, SubApp, Update};
use bevy_ecs::{
    message::MessageCursor,
    prelude::*,
    schedule::{InternedScheduleLabel, LogLevel, ScheduleBuildSettings},
};
use parking_lot::RwLock;
use tokio::sync::oneshot;
use tracing::{info, warn};

use crate::{
    attack,
    block_update::{QueuedBlockChangedAcks, QueuedServerBlockUpdates},
    chunks::ChunkBatchInfo,
    connection::RawConnection,
    cookies::ServerCookies,
    interact::BlockStatePredictionHandler,
    local_player::{Experience, Hunger, PermissionLevel, TabList, WorldHolder},
    mining,
    movement::LastSentLookDirection,
    player::retroactively_add_game_profile_component,
};
#[derive(Bundle)]
pub struct LocalPlayerBundle {
    pub raw_connection: RawConnection,
    pub world_holder: WorldHolder,

    pub metadata: azalea_entity::metadata::PlayerMetadataBundle,
}

#[derive(Bundle, Default)]
pub struct JoinedClientBundle {
    pub physics_state: ClientMovementState,
    pub inventory: Inventory,
    pub tab_list: TabList,
    pub block_state_prediction_handler: BlockStatePredictionHandler,
    pub queued_server_block_updates: QueuedServerBlockUpdates,
    pub queued_block_changed_acks: QueuedBlockChangedAcks,
    pub last_sent_direction: LastSentLookDirection,
    pub abilities: PlayerAbilities,
    pub permission_level: PermissionLevel,
    pub chunk_batch_info: ChunkBatchInfo,
    pub hunger: Hunger,
    pub experience: Experience,
    pub cookies: ServerCookies,

    pub entity_id_index: EntityIdIndex,

    pub mining: mining::MineBundle,
    pub attack: attack::AttackBundle,

    pub in_game_state: InGameState,
}

#[derive(Clone, Component, Debug, Default)]
pub struct InGameState;
#[derive(Clone, Component, Debug, Default)]
pub struct InConfigState;

pub struct AzaleaPlugin;
impl Plugin for AzaleaPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                retroactively_add_game_profile_component
                    .after(EntityUpdateSystems::Index)
                    .after(crate::join::handle_start_join_server_event),
            ),
        )
        .init_resource::<Worlds>()
        .init_resource::<TabList>();
    }
}

#[doc(hidden)]
pub fn start_ecs_runner(
    app: &mut SubApp,
) -> (
    Arc<RwLock<World>>,
    impl FnOnce(),
    oneshot::Receiver<AppExit>,
) {
    if app.plugins_state() != PluginsState::Cleaned {
        if app.plugins_state() == PluginsState::Adding {
            info!("Waiting for plugins to load ...");
            while app.plugins_state() == PluginsState::Adding {
                thread::yield_now();
            }
        }
        app.finish();
        app.cleanup();
    }

    if !matches!(std::env::var("MC_EXEC").as_deref(), Ok("mt")) {
        use bevy_ecs::schedule::{ExecutorKind, Schedules};
        let mut schedules = app.world_mut().resource_mut::<Schedules>();
        for (_, schedule) in schedules.iter_mut() {
            schedule.set_executor_kind(ExecutorKind::SingleThreaded);
        }
    }

    let ecs = Arc::new(RwLock::new(mem::take(app.world_mut())));

    let ecs_clone = ecs.clone();
    let outer_schedule_label = *app.update_schedule.as_ref().unwrap();

    let (appexit_tx, appexit_rx) = oneshot::channel();
    let start_running_systems = move || {
        tokio::task::spawn_local(async move {
            let appexit = run_schedule_loop(ecs_clone, outer_schedule_label).await;
            appexit_tx.send(appexit)
        });
    };

    (ecs, start_running_systems, appexit_rx)
}

async fn run_schedule_loop(
    ecs: Arc<RwLock<World>>,
    outer_schedule_label: InternedScheduleLabel,
) -> AppExit {
    let mut last_update: Option<Instant> = None;
    let mut last_tick: Option<Instant> = None;

    #[cfg(not(target_arch = "wasm32"))]
    const UPDATE_DURATION_TARGET: Duration = Duration::from_micros(1_000_000 / 60);
    #[cfg(target_arch = "wasm32")]
    const UPDATE_DURATION_TARGET: Duration = Duration::from_micros(1_000_000 / 30);
    const GAME_TICK_DURATION_TARGET: Duration = Duration::from_micros(1_000_000 / 20);

    loop {
        let now = Instant::now();
        if let Some(last_update) = last_update {
            let elapsed = now.duration_since(last_update);
            if elapsed < UPDATE_DURATION_TARGET {
                crate::sleep(UPDATE_DURATION_TARGET - elapsed).await;
            }
        }
        last_update = Some(now);

        let mut ecs = ecs.write();

        ecs.run_schedule(outer_schedule_label);
        if last_tick
            .map(|last_tick| last_tick.elapsed() > GAME_TICK_DURATION_TARGET)
            .unwrap_or(true)
        {
            if let Some(last_tick) = &mut last_tick {
                *last_tick += GAME_TICK_DURATION_TARGET;

                if (now - *last_tick) > GAME_TICK_DURATION_TARGET * 10 {
                    warn!(
                        "GameTick is more than 10 ticks behind, skipping ticks so we don't have to burst too much"
                    );
                    *last_tick = now;
                }
            } else {
                last_tick = Some(now);
            }
            ecs.run_schedule(GameTick);
        }

        ecs.clear_trackers();
        if let Some(exit) = should_exit(&mut ecs) {
            ecs.clear_all();

            return exit;
        }
    }
}

fn should_exit(ecs: &mut World) -> Option<AppExit> {
    let mut reader = MessageCursor::default();

    let events = ecs.get_resource::<Messages<AppExit>>()?;
    let mut events = reader.read(events);

    if events.len() != 0 {
        return Some(
            events
                .find(|exit| exit.is_error())
                .cloned()
                .unwrap_or(AppExit::Success),
        );
    }

    None
}

pub struct AmbiguityLoggerPlugin;
impl Plugin for AmbiguityLoggerPlugin {
    fn build(&self, app: &mut App) {
        app.edit_schedule(Update, |schedule| {
            schedule.set_build_settings(ScheduleBuildSettings {
                ambiguity_detection: LogLevel::Warn,
                ..Default::default()
            });
        });
        app.edit_schedule(GameTick, |schedule| {
            schedule.set_build_settings(ScheduleBuildSettings {
                ambiguity_detection: LogLevel::Warn,
                ..Default::default()
            });
        });
    }
}
