use azalea::entity::particle::{ColorParticle, Particle};
use azalea::prelude::*;
use azalea_core::color::ArgbColor;
use azalea_core::position::Vec3;
use azalea_world::WorldName;
use bevy::platform::collections::HashMap;

use crate::client::tracking::EntityAttributes;
use crate::session::{ParticleEmit, SharedMutex};
use crate::util::javarandom::JavaRandom;

pub(crate) const EMIT_RANGE: f64 = 36.0;

pub(crate) struct Viewer {
    pos: Vec3,
    world: WorldName,
}

impl Viewer {
    pub(crate) fn of(bot: &Client) -> Option<Viewer> {
        let pos = bot.position().ok()?;
        let world = {
            let name = bot.component::<WorldName>().ok()?;
            name.clone()
        };
        Some(Viewer { pos, world })
    }

    pub(crate) fn in_world(&self, name: &WorldName) -> bool {
        *name == self.world
    }

    pub(crate) fn within(&self, pos: Vec3, range: f64) -> bool {
        pos.distance_squared_to(self.pos) <= range * range
    }

    pub(crate) fn near(&self, pos: Vec3) -> bool {
        self.within(pos, EMIT_RANGE)
    }
}

pub(crate) fn random_point(
    rng: &mut JavaRandom,
    feet: [f64; 3],
    width: f64,
    height: f64,
    spread: f64,
) -> [f64; 3] {
    let x = feet[0] + width * ((2.0 * rng.next_f64() - 1.0) * spread);
    let y = feet[1] + height * rng.next_f64();
    let z = feet[2] + width * ((2.0 * rng.next_f64() - 1.0) * spread);
    [x, y, z]
}

pub(crate) fn box_scale(
    _attributes: &HashMap<i32, EntityAttributes>,
    _id: i32,
    _local: bool,
) -> f32 {
    1.0
}

pub(crate) fn entity_effect(argb: u32) -> Particle {
    let [a, r, g, b] = argb.to_be_bytes();
    Particle::EntityEffect(ColorParticle {
        color: ArgbColor::new(a, r, g, b),
    })
}

pub(crate) fn queue<T>(shared: &SharedMutex, emits: Vec<T>, wrap: fn(T) -> ParticleEmit) {
    if emits.is_empty() {
        return;
    }
    let mut s = shared.lock().unwrap();
    crate::session::push_particle_emits(&mut s.session.particle_emits, emits.into_iter().map(wrap));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_random_point_stays_in_the_box() {
        let mut rng = JavaRandom::new(7);
        for _ in 0..256 {
            let [x, y, z] = random_point(&mut rng, [10.0, 64.0, -3.0], 0.6, 1.8, 0.5);
            assert!((x - 10.0).abs() <= 0.3 && (z + 3.0).abs() <= 0.3);
            assert!((64.0..=65.8).contains(&y));
        }
    }
}
