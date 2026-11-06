use azalea::ecs::query::{Has, QueryState};
use azalea::ecs::world::WorldId;
use azalea::entity::dimensions::EntityDimensions;
use azalea::entity::metadata::{EffectAmbience, EffectParticles, Invisible};
use azalea::entity::{LocalEntity, Position};
use azalea::prelude::*;
use azalea_core::entity_id::MinecraftEntityId;
use azalea_world::WorldName;

use crate::play::emit::{self, Viewer};
use crate::session::{LevelParticle, ParticleEmit, SharedMutex, SideMutex};
use crate::util::javarandom::JavaRandom;

type EffectQuery = QueryState<(
    &'static WorldName,
    &'static Position,
    &'static EntityDimensions,
    &'static MinecraftEntityId,
    &'static EffectParticles,
    &'static EffectAmbience,
    &'static Invisible,
    Has<LocalEntity>,
)>;

struct State {
    query: Option<(WorldId, EffectQuery)>,
    rng: Option<JavaRandom>,
}

static STATE: SideMutex<State> = SideMutex::new(State {
    query: None,
    rng: None,
});

pub(crate) fn tick(bot: &Client, shared: &SharedMutex) {
    let Some(viewer) = Viewer::of(bot) else {
        return;
    };

    let mut state = STATE.lock().unwrap();
    let State { query, rng } = &mut *state;
    let query = crate::play::entity_feed::cached_query(bot, query);
    let rng = rng.get_or_insert_with(JavaRandom::time_seeded);

    let attributes = crate::client::tracking::attributes().lock().unwrap();
    let ecs = bot.ecs.read();
    let mut emits = Vec::new();
    for (name, pos, dims, id, particles, ambience, invisible, local) in query.iter(&ecs) {
        if particles.0.is_empty() || !viewer.in_world(name) || !viewer.near(**pos) {
            continue;
        }
        let bound = if invisible.0 { 15 } else { 4 };
        let factor = if ambience.0 { 5 } else { 1 };
        if rng.next_int(bound * factor) != 0 {
            continue;
        }
        let particle = &particles.0[rng.next_int(particles.0.len() as u32) as usize];
        let scale = emit::box_scale(&attributes, id.0, local);
        let (width, height) = (
            f64::from(dims.width * scale),
            f64::from(dims.height * scale),
        );
        let at = emit::random_point(rng, [pos.x, pos.y, pos.z], width, height, 0.5);
        emits.push(LevelParticle::add(particle.clone(), at, [1.0; 3]));
    }
    drop(ecs);
    drop(attributes);
    drop(state);

    emit::queue(shared, emits, ParticleEmit::Level);
}
