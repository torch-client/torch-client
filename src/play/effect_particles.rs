use std::sync::Arc;

use azalea::ecs::query::{Has, QueryState};
use azalea::ecs::world::WorldId;
use azalea::entity::dimensions::EntityDimensions;
use azalea::entity::metadata::{EffectAmbience, EffectParticles, Invisible};
use azalea::entity::{LocalEntity, Position};
use azalea::prelude::*;
use azalea_core::entity_id::MinecraftEntityId;
use azalea_world::WorldName;

use crate::client::tracking::EntityAttributes;
use crate::session::{LevelParticle, ParticleEmit, SharedMutex, SideMutex};
use crate::util::javarandom::JavaRandom;

const EMIT_RANGE: f64 = 36.0;

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

pub(crate) fn tick(bot: &Client, shared: &Arc<SharedMutex>) {
    let Ok(origin) = bot.position() else { return };
    let world_name = {
        let Ok(name) = bot.component::<WorldName>() else {
            return;
        };
        name.clone()
    };

    let mut state = STATE.lock().unwrap();
    let State { query, rng } = &mut *state;
    let query = crate::play::entity_feed::cached_query(bot, query);
    let rng = rng.get_or_insert_with(JavaRandom::time_seeded);

    let attributes = crate::client::tracking::attributes().lock().unwrap();
    let ecs = bot.ecs.read();
    let mut emits = Vec::new();
    for (name, pos, dims, id, particles, ambience, invisible, local) in query.iter(&ecs) {
        if particles.0.is_empty()
            || *name != world_name
            || pos.distance_squared_to(origin) > EMIT_RANGE * EMIT_RANGE
        {
            continue;
        }
        let bound = if invisible.0 { 15 } else { 4 };
        let factor = if ambience.0 { 5 } else { 1 };
        if rng.next_int(bound * factor) != 0 {
            continue;
        }
        let particle = &particles.0[rng.next_int(particles.0.len() as u32) as usize];
        let scale = if local {
            1.0
        } else {
            EntityAttributes::of(&attributes, id.0).scale()
        };
        let (width, height) = (
            f64::from(dims.width * scale),
            f64::from(dims.height * scale),
        );
        let x = pos.x + (2.0 * rng.next_f64() - 1.0) * 0.5 * width;
        let y = pos.y + rng.next_f64() * height;
        let z = pos.z + (2.0 * rng.next_f64() - 1.0) * 0.5 * width;
        emits.push(LevelParticle {
            particle: particle.clone(),
            pos: [x, y, z],
            dist: [1.0; 3],
            max_speed: 1.0,
            count: 0,
            override_limiter: false,
            always_show: false,
        });
    }
    drop(ecs);
    drop(attributes);
    drop(state);

    if emits.is_empty() {
        return;
    }
    let mut s = shared.lock().unwrap();
    crate::session::push_particle_emits(
        &mut s.session.particle_emits,
        emits.into_iter().map(ParticleEmit::Level),
    );
}
