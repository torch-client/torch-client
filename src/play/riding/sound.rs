use azalea::entity::dimensions::EntityDimensions;
use azalea::entity::indexing::EntityIdIndex;
use azalea::entity::metadata::Silent;
use azalea::entity::{EntityKindComponent, Physics, Position};
use azalea::prelude::*;
use azalea_core::entity_id::MinecraftEntityId;
use azalea_registry::builtin::EntityKind;

use crate::audio::{LoopId, SoundCategory};
use crate::session::SideMutex;
use crate::util::block_model::{EyeFluid, eye_fluid_at};

const MIN_SPEED: f32 = 0.01;

struct Sounds {
    events: &'static [(&'static str, bool)],
    volume_max: f32,
    volume_amplifier: f32,
    minecart: bool,
}

const MINECART: Sounds = Sounds {
    events: &[
        ("entity.minecart.inside.underwater", true),
        ("entity.minecart.inside", false),
    ],
    volume_max: 0.75,
    volume_amplifier: 1.0,
    minecart: true,
};
const HAPPY_GHAST: Sounds = Sounds {
    events: &[("entity.happy_ghast.riding", false)],
    volume_max: 1.0,
    volume_amplifier: 5.0,
    minecart: false,
};
const NAUTILUS: Sounds = Sounds {
    events: &[("entity.nautilus.riding", true)],
    volume_max: 1.0,
    volume_amplifier: 5.0,
    minecart: false,
};

fn sounds_of(kind: EntityKind) -> Option<&'static Sounds> {
    use EntityKind as K;
    match kind {
        K::Minecart
        | K::ChestMinecart
        | K::CommandBlockMinecart
        | K::FurnaceMinecart
        | K::HopperMinecart
        | K::SpawnerMinecart
        | K::TntMinecart => Some(&MINECART),
        K::HappyGhast => Some(&HAPPY_GHAST),
        K::Nautilus | K::ZombieNautilus => Some(&NAUTILUS),
        _ => None,
    }
}

struct Ride {
    vehicle: i32,
    sounds: Option<&'static Sounds>,
    loops: [Option<LoopId>; 2],
}

impl Ride {
    fn stop(&mut self) {
        for id in self.loops.iter_mut().filter_map(Option::take) {
            crate::audio::end_loop(id);
        }
    }
}

static RIDE: SideMutex<Option<Ride>> = SideMutex::new(None);

pub(crate) fn reset() {
    if let Ok(mut ride) = RIDE.try_lock()
        && let Some(mut ride) = ride.take()
    {
        ride.stop();
    }
}

struct Vehicle {
    kind: EntityKind,
    silent: bool,
    velocity: azalea_core::position::Vec3,
    eye: [f64; 3],
}

fn read_vehicle(bot: &Client, id: i32) -> Option<Vehicle> {
    let ecs = bot.ecs.read();
    let index = ecs.get::<EntityIdIndex>(bot.entity)?;
    let entity = index.get_by_minecraft_entity(MinecraftEntityId(id))?;
    let pos = **ecs.get::<Position>(entity)?;
    let eye_height = ecs.get::<EntityDimensions>(entity)?.eye_height;
    Some(Vehicle {
        kind: ecs.get::<EntityKindComponent>(entity)?.0,
        silent: ecs.get::<Silent>(entity).is_some_and(|s| s.0),
        velocity: ecs.get::<Physics>(entity)?.velocity,
        eye: [pos.x, pos.y + f64::from(eye_height), pos.z],
    })
}

fn water_at([x, y, z]: [f64; 3]) -> bool {
    eye_fluid_at(x, y, z) == Some(EyeFluid::Water)
}

fn rider_under_water(bot: &Client) -> bool {
    let Ok(pos) = bot.position() else {
        return false;
    };
    bot.component::<EntityDimensions>()
        .is_ok_and(|dims| water_at([pos.x, pos.y + f64::from(dims.eye_height), pos.z]))
}

pub(crate) fn tick(bot: &Client) {
    let mut guard = RIDE.lock().unwrap();
    let vehicle_id = crate::client::tracking::local_vehicle_id();
    let vehicle = vehicle_id.and_then(|id| read_vehicle(bot, id));
    if (vehicle.is_none() || guard.as_ref().map(|ride| ride.vehicle) != vehicle_id)
        && let Some(mut old) = guard.take()
    {
        old.stop();
    }
    let (Some(vehicle_id), Some(vehicle)) = (vehicle_id, vehicle) else {
        return;
    };
    let ride = guard.get_or_insert_with(|| Ride {
        vehicle: vehicle_id,
        sounds: sounds_of(vehicle.kind),
        loops: [None; 2],
    });
    let Some(sounds) = ride.sounds else { return };
    if vehicle.silent {
        ride.stop();
        ride.sounds = None;
        return;
    }
    let velocity = vehicle.velocity;
    let (speed, under_water) = if sounds.minecart {
        (
            velocity.horizontal_distance() as f32,
            rider_under_water(bot),
        )
    } else {
        (velocity.length() as f32, water_at(vehicle.eye))
    };
    let moving = if speed >= MIN_SPEED {
        sounds.volume_amplifier * speed.min(1.0) * sounds.volume_max
    } else {
        0.0
    };
    for (slot, &(event, underwater)) in ride.loops.iter_mut().zip(sounds.events) {
        *slot = slot.or_else(|| crate::audio::start_loop(event, SoundCategory::Neutral, 0.0, 1.0));
        if let Some(id) = *slot {
            let volume = if underwater == under_water {
                moving
            } else {
                0.0
            };
            crate::audio::update_loop(id, volume, 1.0);
        }
    }
}
