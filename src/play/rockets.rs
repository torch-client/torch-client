use std::sync::Arc;

use azalea::ecs::prelude::*;
use azalea::entity::Position;
use azalea::entity::metadata::{FireworkRocket, FireworksItem};
use azalea_core::entity_id::MinecraftEntityId;
use azalea_inventory::components::{FireworkExplosion, Fireworks};
use bevy::platform::collections::HashMap;

use crate::session::SideMutex;
use crate::util::global;

const KEEP_SCANS: u32 = 8;

static SCANS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub(crate) type RocketQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static MinecraftEntityId,
        &'static Position,
        Option<Ref<'static, FireworksItem>>,
    ),
    With<FireworkRocket>,
>;

#[derive(Clone)]
pub(crate) struct Rocket {
    pub(crate) at: [f64; 3],
    pub(crate) velocity: [f64; 3],
    pub(crate) explosions: Arc<[FireworkExplosion]>,
    seen: u32,
}

impl Rocket {
    #[cfg(feature = "audio")]
    pub(crate) fn large(&self) -> bool {
        use azalea_inventory::components::FireworkExplosionShape;
        self.explosions.len() >= 3
            || self
                .explosions
                .iter()
                .any(|e| matches!(e.shape, FireworkExplosionShape::LargeBall))
    }

    #[cfg(feature = "audio")]
    pub(crate) fn twinkle_ticks(&self) -> Option<u32> {
        self.explosions
            .iter()
            .any(|e| e.has_twinkle)
            .then_some(self.explosions.len() as u32 * 2 + 14)
    }
}

global! {
fn table() -> SideMutex<HashMap<i32, Rocket>>
}

pub(crate) fn get(id: i32) -> Option<Rocket> {
    let mut rocket = table().lock().unwrap().get(&id).cloned()?;
    if let Some(anim) = crate::client::tracking::entity_anims()
        .lock()
        .unwrap()
        .get(&id)
    {
        rocket.at = anim.position(1.0);
        rocket.velocity = anim.velocity();
    }
    Some(rocket)
}

pub(crate) fn reset(blocking: bool) {
    if blocking {
        table().lock().unwrap().clear();
    } else if let Ok(mut rockets) = table().try_lock() {
        rockets.clear();
    }
}

pub(crate) fn scan(rockets: &RocketQuery) {
    let now = SCANS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut table = table().lock().unwrap();
    if !rockets.is_empty() {
        let velocities = crate::client::tracking::velocities().lock().unwrap();
        for (id, pos, item) in rockets.iter() {
            let id = id.0;
            let at = [pos.x, pos.y, pos.z];
            let velocity = velocities.get(&id).map_or([0.0; 3], |(v, _)| *v);
            let explosions = || -> Arc<[FireworkExplosion]> {
                let fireworks = item
                    .as_ref()
                    .and_then(|item| item.0.get_component::<Fireworks>());
                fireworks
                    .as_ref()
                    .map_or(&[][..], |f| &f.explosions[..])
                    .into()
            };
            match table.get_mut(&id) {
                Some(rocket) => {
                    rocket.at = at;
                    rocket.velocity = velocity;
                    rocket.seen = now;
                    if item.as_ref().is_some_and(|item| item.is_changed()) {
                        rocket.explosions = explosions();
                    }
                }
                None => {
                    table.insert(
                        id,
                        Rocket {
                            at,
                            velocity,
                            explosions: explosions(),
                            seen: now,
                        },
                    );
                }
            }
        }
    }
    table.retain(|_, rocket| now.wrapping_sub(rocket.seen) <= KEEP_SCANS);
}
