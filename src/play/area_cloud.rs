use azalea::ecs::query::{QueryState, With};
use azalea::ecs::world::WorldId;
use azalea::entity::Position;
use azalea::entity::metadata::{AreaEffectCloud, Radius, Waiting};
use azalea::entity::particle::Particle;
use azalea::prelude::*;
use azalea_world::WorldName;

use crate::play::emit::{self, EMIT_RANGE, Viewer};
use crate::session::{CloudEmit, ParticleEmit, SharedMutex, SideMutex};

type CloudQuery = QueryState<
    (
        &'static WorldName,
        &'static Position,
        &'static Radius,
        &'static Waiting,
        &'static Particle,
    ),
    With<AreaEffectCloud>,
>;

static QUERY: SideMutex<Option<(WorldId, CloudQuery)>> = SideMutex::new(None);

pub(crate) fn default_particle() -> Particle {
    emit::entity_effect(0xFFFF_FFFF)
}

pub(crate) fn tick(bot: &Client, shared: &SharedMutex) {
    let Some(viewer) = Viewer::of(bot) else {
        return;
    };

    let mut cache = QUERY.lock().unwrap();
    let query = crate::play::entity_feed::cached_query(bot, &mut *cache);
    let ecs = bot.ecs.read();
    let mut emits = Vec::new();
    for (name, pos, radius, waiting, particle) in query.iter(&ecs) {
        if !viewer.in_world(name) || !viewer.within(**pos, EMIT_RANGE + f64::from(radius.0)) {
            continue;
        }
        let particle = if *particle == Particle::default() {
            default_particle()
        } else {
            particle.clone()
        };
        emits.push(CloudEmit {
            particle,
            pos: [pos.x, pos.y, pos.z],
            radius: radius.0,
            waiting: waiting.0,
        });
    }
    drop(ecs);
    drop(cache);

    emit::queue(shared, emits, ParticleEmit::Cloud);
}
