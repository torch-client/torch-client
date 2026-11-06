use std::collections::BTreeMap;

use azalea::ecs::query::{QueryState, With};
use azalea::ecs::world::WorldId;
use azalea::entity::Position;
use azalea::entity::dimensions::EntityDimensions;
use azalea::entity::metadata::{Arrow, EffectColor, InGround};
use azalea::prelude::*;
use azalea_core::entity_id::MinecraftEntityId;
use azalea_core::position::Vec3;
use azalea_world::WorldName;

use crate::play::emit::{self, Viewer};
use crate::session::{LevelParticle, ParticleEmit, SharedMutex, SideMutex};
use crate::util::javarandom::JavaRandom;

const POOF_PARTICLES: u32 = 20;

type ArrowQuery = QueryState<
    (
        &'static WorldName,
        &'static Position,
        &'static EntityDimensions,
        &'static MinecraftEntityId,
        &'static EffectColor,
        &'static InGround,
    ),
    With<Arrow>,
>;

#[derive(Clone, Copy)]
struct Tipped {
    in_ground_time: u32,
    color: i32,
}

struct Seen {
    id: i32,
    feet: [f64; 3],
    size: [f32; 2],
    color: i32,
    in_ground: bool,
}

struct State {
    query: Option<(WorldId, ArrowQuery)>,
    rng: Option<JavaRandom>,
    tipped: BTreeMap<i32, Tipped>,
    poofs: Vec<i32>,
}

static STATE: SideMutex<State> = SideMutex::new(State {
    query: None,
    rng: None,
    tipped: BTreeMap::new(),
    poofs: Vec::new(),
});

pub(crate) fn poof(entity: i32) {
    STATE.lock().unwrap().poofs.push(entity);
}

pub(crate) fn reset() {
    if let Ok(mut state) = STATE.try_lock() {
        state.tipped.clear();
        state.poofs.clear();
    }
}

fn add(
    emits: &mut Vec<LevelParticle>,
    rng: &mut JavaRandom,
    argb: u32,
    feet: [f64; 3],
    size: [f32; 2],
) {
    let [width, height] = size.map(f64::from);
    let pos = emit::random_point(rng, feet, width, height, 0.5);
    emits.push(LevelParticle::add(emit::entity_effect(argb), pos, [0.0; 3]));
}

pub(crate) fn tick(bot: &Client, shared: &SharedMutex) {
    let Some(viewer) = Viewer::of(bot) else {
        return;
    };

    let mut state = STATE.lock().unwrap();
    let State {
        query,
        rng,
        tipped,
        poofs,
    } = &mut *state;
    let query = crate::play::entity_feed::cached_query(bot, query);
    let rng = rng.get_or_insert_with(JavaRandom::time_seeded);
    let last_tick = std::mem::take(tipped);

    let ecs = bot.ecs.read();
    let arrows: Vec<Seen> = query
        .iter(&ecs)
        .filter(|(name, _, _, id, color, _)| {
            viewer.in_world(name) && (color.0 != -1 || poofs.contains(&id.0))
        })
        .map(|(_, pos, dims, id, color, in_ground)| Seen {
            id: id.0,
            feet: [pos.x, pos.y, pos.z],
            size: [dims.width, dims.height],
            color: color.0,
            in_ground: in_ground.0,
        })
        .collect();
    drop(ecs);
    if arrows.is_empty() {
        poofs.clear();
        return;
    }

    let anims = crate::client::tracking::entity_anims().lock().unwrap();
    let mut emits = Vec::new();
    for arrow in arrows {
        let known = last_tick.get(&arrow.id);
        let feet = anims
            .get(&arrow.id)
            .map_or(arrow.feet, |anim| anim.position(1.0));
        if poofs.contains(&arrow.id) {
            let argb = if arrow.color != -1 {
                Some(arrow.color)
            } else {
                known.map(|k| k.color)
            };
            if let Some(argb) = argb {
                for _ in 0..POOF_PARTICLES {
                    add(&mut emits, rng, argb as u32 | 0xFF00_0000, feet, arrow.size);
                }
            }
        }
        if arrow.color == -1 {
            continue;
        }
        let in_ground_time = if arrow.in_ground {
            known.map_or(0, |k| k.in_ground_time) + 1
        } else {
            0
        };
        tipped.insert(
            arrow.id,
            Tipped {
                in_ground_time,
                color: arrow.color,
            },
        );

        if !viewer.near(Vec3::new(feet[0], feet[1], feet[2])) {
            continue;
        }
        let amount = match (arrow.in_ground, in_ground_time % 5) {
            (false, _) => 2,
            (true, 0) => 1,
            (true, _) => 0,
        };
        for _ in 0..amount {
            add(&mut emits, rng, arrow.color as u32, feet, arrow.size);
        }
    }
    drop(anims);
    poofs.clear();
    drop(state);

    emit::queue(shared, emits, ParticleEmit::Level);
}
