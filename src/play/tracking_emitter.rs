use std::sync::Arc;

use azalea::entity::particle::Particle;
use azalea::prelude::*;

use crate::session::{ParticleEmit, SharedMutex, SideMutex, TrackingEmit};

pub(crate) const DEFAULT_LIFE: u32 = 3;

struct Emitter {
    entity: i32,
    particle: Particle,
    life_time: u32,
    life: u32,
    #[cfg(feature = "audio")]
    sound: Option<&'static str>,
}

static EMITTERS: SideMutex<Vec<Emitter>> = SideMutex::new(Vec::new());

pub(crate) fn start(entity: i32, particle: Particle, life_time: u32, sound: Option<&'static str>) {
    #[cfg(not(feature = "audio"))]
    let _ = sound;
    EMITTERS.lock().unwrap().push(Emitter {
        entity,
        particle,
        life_time,
        life: 0,
        #[cfg(feature = "audio")]
        sound,
    });
}

pub(crate) fn reset() {
    if let Ok(mut emitters) = EMITTERS.try_lock() {
        emitters.clear();
    }
}

pub(crate) fn tick(bot: &Client, shared: &Arc<SharedMutex>) {
    let rounds = {
        let mut emitters = EMITTERS.lock().unwrap();
        if emitters.is_empty() {
            return;
        }
        advance(bot, &mut emitters)
    };
    if rounds.is_empty() {
        return;
    }
    let mut s = shared.lock().unwrap();
    crate::session::push_particle_emits(
        &mut s.session.particle_emits,
        rounds.into_iter().map(ParticleEmit::Tracking),
    );
}

fn advance(bot: &Client, emitters: &mut Vec<Emitter>) -> Vec<TrackingEmit> {
    use azalea::entity::Position;
    use azalea::entity::dimensions::EntityDimensions;
    use azalea::entity::indexing::EntityIdIndex;
    use azalea_core::entity_id::MinecraftEntityId;

    let local = crate::client::tracking::local_entity_id();
    let ecs = bot.ecs.read();
    let index = ecs.get::<EntityIdIndex>(bot.entity);
    let mut rounds = Vec::with_capacity(emitters.len());
    emitters.retain_mut(|emitter| {
        let entity = if emitter.entity == local {
            Some(bot.entity)
        } else {
            index.and_then(|i| i.get_by_minecraft_entity(MinecraftEntityId(emitter.entity)))
        };
        let Some(entity) = entity else { return false };
        let (Some(pos), Some(dims)) = (
            ecs.get::<Position>(entity),
            ecs.get::<EntityDimensions>(entity),
        ) else {
            return false;
        };
        let feet = [pos.x, pos.y, pos.z];
        #[cfg(feature = "audio")]
        if emitter.life == 0
            && let Some(sound) = emitter.sound
        {
            let source = ecs
                .get::<azalea::entity::EntityKindComponent>(entity)
                .map_or(crate::audio::SoundCategory::Hostile, |kind| {
                    sound_source(kind.0)
                });
            crate::audio::play_at(sound, source, feet, 1.0, 1.0);
        }
        rounds.push(TrackingEmit {
            particle: emitter.particle.clone(),
            pos: feet,
            size: [dims.width, dims.height],
        });
        emitter.life += 1;
        emitter.life < emitter.life_time
    });
    rounds
}

#[cfg(feature = "audio")]
pub(crate) fn sound_source(
    kind: azalea_registry::builtin::EntityKind,
) -> crate::audio::SoundCategory {
    use crate::audio::SoundCategory;
    use azalea_registry::builtin::EntityKind as K;
    match kind {
        K::Player => SoundCategory::Players,
        K::Zombie
        | K::Husk
        | K::Drowned
        | K::Giant
        | K::ZombieVillager
        | K::Skeleton
        | K::Stray
        | K::WitherSkeleton
        | K::Bogged
        | K::Parched
        | K::Piglin
        | K::PiglinBrute
        | K::ZombifiedPiglin
        | K::Pillager
        | K::Vindicator
        | K::Evoker
        | K::Illusioner
        | K::Witch
        | K::Vex => SoundCategory::Hostile,
        _ => SoundCategory::Neutral,
    }
}
