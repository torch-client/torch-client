use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use azalea_core::{game_type::GameMode, tick::GameTick};
use azalea_entity::{Dead, HasClientLoaded, LocalEntity, Position, dimensions::EntityDimensions};
use azalea_physics::PhysicsSystems;
use azalea_protocol::packets::game::ServerboundPlayerLoaded;
use azalea_registry::identifier::Identifier;
use azalea_world::{WorldName, Worlds};
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_platform::time::Instant;
use tracing::warn;

use crate::{mining::MiningSystems, packet::game::SendGamePacketEvent};

pub struct PlayerLoadedPlugin;
impl Plugin for PlayerLoadedPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            GameTick,
            tick_level_load
                .before(PhysicsSystems)
                .before(MiningSystems)
                .before(crate::movement::send_position),
        );
    }
}

pub const CLIENT_WAIT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LevelLoadingReason {
    NetherPortal,
    EndPortal,
    #[default]
    Other,
}

pub fn determine_level_loading_reason(
    player_died: bool,
    dimension: &Identifier,
    old_dimension: &Identifier,
) -> LevelLoadingReason {
    if player_died {
        return LevelLoadingReason::Other;
    }
    let is = |id: &Identifier, path: &str| id.namespace() == "minecraft" && id.path() == path;
    if is(dimension, "the_nether") || is(old_dimension, "the_nether") {
        LevelLoadingReason::NetherPortal
    } else if is(dimension, "the_end") || is(old_dimension, "the_end") {
        LevelLoadingReason::EndPortal
    } else {
        LevelLoadingReason::Other
    }
}

#[derive(Debug)]
enum ClientState {
    WaitingForServer {
        timeout_after: Instant,
    },
    WaitingForPlayerChunk {
        player_section_ready: Arc<AtomicBool>,
        timeout_after: Instant,
    },
    ClientLevelReady {
        ready_at: Instant,
    },
}

#[derive(Component, Debug)]
pub struct LevelLoadTracker {
    state: ClientState,
    reason: LevelLoadingReason,
    id: u64,
}

static NEXT_TRACKER_ID: AtomicU64 = AtomicU64::new(1);

impl LevelLoadTracker {
    fn new(reason: LevelLoadingReason) -> Self {
        Self {
            state: ClientState::WaitingForServer {
                timeout_after: Instant::now() + CLIENT_WAIT_TIMEOUT,
            },
            reason,
            id: NEXT_TRACKER_ID.fetch_add(1, Ordering::Relaxed),
        }
    }

    pub fn reason(&self) -> LevelLoadingReason {
        self.reason
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn server_started(&self) -> bool {
        !matches!(self.state, ClientState::WaitingForServer { .. })
    }

    pub fn loading_packets_received(&mut self) {
        if let ClientState::WaitingForServer { timeout_after } = self.state {
            self.state = ClientState::WaitingForPlayerChunk {
                player_section_ready: Arc::new(AtomicBool::new(false)),
                timeout_after,
            };
        }
    }

    pub fn player_compiled_section_callback(&self) -> Option<Arc<AtomicBool>> {
        match &self.state {
            ClientState::WaitingForPlayerChunk {
                player_section_ready,
                ..
            } => Some(player_section_ready.clone()),
            _ => None,
        }
    }

    pub fn is_level_ready(&self) -> bool {
        matches!(self.state, ClientState::ClientLevelReady { ready_at } if Instant::now() >= ready_at)
    }
}

pub fn start_waiting_for_new_level(
    commands: &mut Commands,
    player: Entity,
    reason: LevelLoadingReason,
) {
    commands
        .entity(player)
        .remove::<HasClientLoaded>()
        .insert(LevelLoadTracker::new(reason));
}

#[allow(clippy::type_complexity)]
pub fn tick_level_load(
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            &mut LevelLoadTracker,
            &Position,
            Option<&EntityDimensions>,
            Option<&WorldName>,
            Option<&GameMode>,
            Has<Dead>,
            Has<HasClientLoaded>,
        ),
        With<LocalEntity>,
    >,
    worlds: Res<Worlds>,
) {
    let now = Instant::now();
    for (entity, mut tracker, position, dimensions, world_name, game_mode, dead, loaded) in
        &mut query
    {
        let ready = match &tracker.state {
            ClientState::WaitingForServer { timeout_after } => {
                let timed_out = now > *timeout_after;
                if timed_out {
                    warn!(
                        "The server never sent LEVEL_CHUNKS_LOAD_START; letting the player into the world anyway"
                    );
                }
                timed_out
            }
            ClientState::WaitingForPlayerChunk {
                player_section_ready,
                timeout_after,
            } => {
                if now > *timeout_after {
                    warn!(
                        "Timed out while waiting for the client to load chunks, letting the player into the world anyway"
                    );
                    true
                } else {
                    let outside_build_height = world_name
                        .and_then(|name| worlds.get(name))
                        .is_some_and(|world| {
                            let world = world.read();
                            let min_y = world.chunks.min_y();
                            let max_y = min_y + world.chunks.height() as i32;
                            let eye = dimensions.map_or(0.0, |d| f64::from(d.eye_height));
                            [position.y, position.y + eye]
                                .into_iter()
                                .map(|y| y.floor() as i32)
                                .any(|y| y < min_y || y >= max_y)
                        });
                    let spectator = game_mode == Some(&GameMode::Spectator);
                    if outside_build_height || spectator || dead {
                        true
                    } else {
                        player_section_ready.load(Ordering::Acquire)
                    }
                }
            }
            ClientState::ClientLevelReady { .. } => false,
        };
        if ready {
            tracker.state = ClientState::ClientLevelReady { ready_at: now };
        }

        if tracker.is_level_ready() {
            if !loaded {
                commands.trigger(SendGamePacketEvent::new(entity, ServerboundPlayerLoaded));
                commands.entity(entity).insert(HasClientLoaded);
            }
            commands.entity(entity).remove::<LevelLoadTracker>();
        }
    }
}
