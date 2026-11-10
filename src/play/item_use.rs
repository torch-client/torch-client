use std::collections::HashMap;

use azalea::ecs::query::{Has, QueryState};
use azalea::ecs::world::WorldId;
use azalea::entity::dimensions::EntityDimensions;
use azalea::entity::metadata::{AbstractLivingUsingItem, AbstractLivingUsingOffhand};
use azalea::entity::{LocalEntity, LookDirection, Position};
use azalea::prelude::*;
use azalea_core::entity_id::MinecraftEntityId;
use azalea_registry::builtin::ItemKind;
use azalea_world::WorldName;

use crate::client::tracking::{EntityAttributes, attributes, equipment};
use crate::items::consume::Consumption;
use crate::play::emit::{self, Viewer};
use crate::session::{ItemUseEmit, ParticleEmit, SharedMutex, SideMutex};

pub(crate) const USE_TICK_PARTICLES: u32 = 5;
pub(crate) const FINISH_PARTICLES: u32 = 16;

type UserQuery = QueryState<(
    &'static WorldName,
    &'static Position,
    &'static EntityDimensions,
    &'static LookDirection,
    &'static MinecraftEntityId,
    &'static AbstractLivingUsingItem,
    &'static AbstractLivingUsingOffhand,
    Has<LocalEntity>,
)>;

struct Use {
    item: Option<&'static str>,
    remaining: i32,
    seen: u32,
}

struct State {
    query: Option<(WorldId, UserQuery)>,
    uses: Option<HashMap<i32, Use>>,
    tick: u32,
}

static STATE: SideMutex<State> = SideMutex::new(State {
    query: None,
    uses: None,
    tick: 0,
});

pub(crate) fn reset() {
    if let Ok(mut state) = STATE.try_lock() {
        state.uses = None;
    }
}

pub(crate) fn emit_local(
    bot: &Client,
    shared: &SharedMutex,
    kind: ItemKind,
    consume: &Consumption,
    count: u32,
) {
    let Ok(pos) = bot.position() else { return };
    #[cfg(feature = "audio")]
    play_sound(consume, [pos.x, pos.y, pos.z]);
    if !consume.particles {
        return;
    }
    let (Ok(eye_height), Ok((x_rot, y_rot))) = (
        bot.component::<EntityDimensions>()
            .map(|dims| dims.eye_height),
        bot.component::<LookDirection>()
            .map(|look| (look.x_rot(), look.y_rot())),
    ) else {
        return;
    };
    let emit = ItemUseEmit {
        item: kind.to_str().trim_start_matches("minecraft:"),
        eye: [pos.x, pos.y + f64::from(eye_height), pos.z],
        x_rot,
        y_rot,
        count,
    };
    let mut s = shared.lock().unwrap();
    crate::session::push_particle_emit(&mut s.session.particle_emits, ParticleEmit::ItemUse(emit));
}

#[cfg(feature = "audio")]
fn play_sound(consume: &Consumption, at: [f64; 3]) {
    use crate::audio::{SoundCategory, random_f32};

    let event = crate::items::kinetic::event_name(&consume.sound);
    let (volume, pitch) = if consume.drink {
        (0.5, 0.9 + random_f32() * 0.1)
    } else {
        let volume = if random_f32() < 0.5 { 0.5 } else { 1.0 };
        (volume, 1.0 + 0.2 * (random_f32() - random_f32()))
    };
    crate::audio::play_at(&event, SoundCategory::Players, at, volume, pitch);
}

pub(crate) fn tick(bot: &Client, shared: &SharedMutex) {
    let Some(viewer) = Viewer::of(bot) else {
        return;
    };

    let mut state = STATE.lock().unwrap();
    let State { query, uses, tick } = &mut *state;
    *tick = tick.wrapping_add(1);
    let query = crate::play::entity_feed::cached_query(bot, query);
    let uses = uses.get_or_insert_with(HashMap::new);

    let equipment = equipment().lock().unwrap();
    let attributes = attributes().lock().unwrap();
    let ecs = bot.ecs.read();
    let mut emits = Vec::new();
    for (name, pos, dims, look, id, using, off_hand, local) in query.iter(&ecs) {
        if local || !using.0 || !viewer.in_world(name) {
            continue;
        }
        let worn = equipment.get(&id.0);
        let (held, consume) = worn.map_or(("", None), |worn| {
            let hand = usize::from(off_hand.0);
            let held = if off_hand.0 {
                &worn.off_hand
            } else {
                &worn.main_hand
            };
            (held.id, worn.consume[hand].as_ref())
        });
        let usage = uses.entry(id.0).or_insert_with(|| Use {
            item: Some(held),
            remaining: consume.map_or(0, |c| c.ticks),
            seen: 0,
        });
        usage.seen = *tick;
        if usage.item != Some(held) {
            usage.item = None;
            continue;
        }
        if let Some(consume) = consume
            && consume.particles
            && consume.should_emit(usage.remaining)
            && viewer.near(**pos)
        {
            let scale = EntityAttributes::of(&attributes, id.0).scale();
            emits.push(ItemUseEmit {
                item: held,
                eye: [pos.x, pos.y + f64::from(dims.eye_height * scale), pos.z],
                x_rot: look.x_rot(),
                y_rot: look.y_rot(),
                count: USE_TICK_PARTICLES,
            });
        }
        usage.remaining -= 1;
    }
    drop(ecs);
    drop(attributes);
    drop(equipment);

    let tick = *tick;
    uses.retain(|_, usage| usage.seen == tick);
    drop(state);

    emit::queue(shared, emits, ParticleEmit::ItemUse);
}
