use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use azalea::Client;
use azalea::app::{App, Plugin};
use azalea::ecs::prelude::*;
use azalea::entity::LocalEntity;
use azalea::entity::metadata::Player;
use azalea_core::entity_id::MinecraftEntityId;
use azalea_core::tick::{Frozen, FrozenEntities, GameTick, TickRateManager, TickRateSystems};

use crate::session::WorldRun;

pub struct TickRatePlugin;

impl Plugin for TickRatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Frozen>()
            .configure_sets(
                GameTick,
                TickRateSystems.before(crate::play::pick_box::step_pick_boxes),
            )
            .add_systems(
                GameTick,
                find_frozen
                    .in_set(TickRateSystems)
                    .after(azalea::tick_counter::tick_rate),
            )
            .add_observer(|_: On<Insert, azalea::tick_counter::TicksConnected>| new_level())
            .add_observer(|_: On<Insert, azalea::world::WorldBorder>| new_level());
    }
}

static LEVEL: AtomicU32 = AtomicU32::new(0);

fn new_level() {
    LEVEL.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn level() -> u32 {
    LEVEL.load(Ordering::Relaxed)
}

#[allow(clippy::type_complexity)]
fn find_frozen(
    rate: Res<TickRateManager>,
    mut frozen: ResMut<Frozen>,
    players: Query<&MinecraftEntityId, Or<(With<Player>, With<LocalEntity>)>>,
) {
    if rate.runs_normally() {
        if frozen.0.is_some() {
            frozen.0 = None;
        }
        return;
    }
    let vehicle_of = crate::client::tracking::passengers()
        .lock()
        .unwrap()
        .iter()
        .flat_map(|(&vehicle, riders)| riders.iter().map(move |&rider| (rider, vehicle)))
        .collect();
    let players = players.iter().map(|id| id.0);
    frozen.0 = Some(Arc::new(FrozenEntities::new(vehicle_of, players)));
}

#[derive(Clone, Debug, Default)]
pub(crate) struct WorldTick {
    pub(crate) run: WorldRun,
    pub(crate) rate: f32,
    frozen: Frozen,
}

impl WorldTick {
    pub(crate) fn of(bot: &Client) -> WorldTick {
        let ecs = bot.ecs.read();
        let Some(rate) = ecs.get_resource::<TickRateManager>() else {
            return WorldTick {
                rate: crate::session::TickRate::default().0,
                ..WorldTick::default()
            };
        };
        WorldTick {
            run: WorldRun {
                frozen: rate.is_frozen(),
                runs: rate.runs_normally(),
            },
            rate: rate.tickrate(),
            frozen: ecs.get_resource::<Frozen>().cloned().unwrap_or_default(),
        }
    }

    pub(crate) fn runs(&self) -> bool {
        self.run.runs
    }

    pub(crate) fn entity_ticks(&self, id: i32) -> bool {
        self.frozen.entity_ticks(id)
    }

    pub(crate) fn entity_frozen(&self, id: i32) -> bool {
        self.frozen
            .0
            .as_ref()
            .is_some_and(|frozen| frozen.is_frozen(id))
    }
}
