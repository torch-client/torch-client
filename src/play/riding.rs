use std::sync::Mutex;
use std::sync::atomic::Ordering;

use azalea::app::{App, Plugin};
use azalea::ecs::prelude::*;
use azalea::entity::indexing::EntityIdIndex;
use azalea::entity::inventory::Inventory;
use azalea::entity::metadata::{AbstractHorseStanding, Sprinting};
use azalea::entity::{
    Attributes, EntityKindComponent, Jumping, LocalEntity, LookDirection, OnClimbable, Physics,
    Position,
};
use azalea::packet::game::SendGamePacketEvent;
use azalea::physics::PhysicsSystems;
use azalea::physics::client_movement::ClientMovementState;
use azalea::physics::collision::entity_collisions::{AabbQuery, CollidableEntityQuery};
use azalea::physics::collision::{MoveCtx, MoverType};
use azalea::physics::travel::{travel, travel_ctx};
use azalea::world::{WorldName, Worlds};
use azalea_core::entity_id::MinecraftEntityId;
use azalea_core::position::Vec3;
use azalea_core::tick::GameTick;
use azalea_protocol::packets::game::s_move_vehicle::ServerboundMoveVehicle;
use azalea_registry::builtin::{EntityKind, ItemKind};

#[derive(Default)]
struct JumpCharge {
    ticks: i32,
    scale: f32,
    was_jumping: bool,
}

impl JumpCharge {
    fn tick(&mut self, jumpable: bool, jump_held: bool) -> (Option<f32>, Option<u32>) {
        let was_jumping = std::mem::replace(&mut self.was_jumping, jump_held);

        if !jumpable {
            self.scale = 0.0;
            return (None, None);
        }

        if self.ticks < 0 {
            self.ticks += 1;
            if self.ticks == 0 {
                self.scale = 0.0;
            }
        }

        let mut power = None;
        if was_jumping && !jump_held {
            self.ticks = -10;
            power = Some((self.scale * 100.0).floor().clamp(0.0, 100.0) as u32);
        } else if !was_jumping && jump_held {
            self.ticks = 0;
            self.scale = 0.0;
        } else if was_jumping {
            self.ticks += 1;
            self.scale = if self.ticks < 10 {
                self.ticks as f32 * 0.1
            } else {
                0.8 + 2.0 / (self.ticks - 9) as f32 * 0.1
            };
        }

        (Some(self.scale), power)
    }
}

static CHARGE: Mutex<JumpCharge> = Mutex::new(JumpCharge {
    ticks: 0,
    scale: 0.0,
    was_jumping: false,
});

pub(crate) fn tick_jump_charge(jumpable: bool, jump_held: bool) -> (Option<f32>, Option<u32>) {
    let (scale, power) = CHARGE.lock().unwrap().tick(jumpable, jump_held);
    if let Some(power) = power {
        *PENDING_JUMP.lock().unwrap() = if power >= 90 {
            1.0
        } else {
            0.4 + 0.4 * power as f32 / 90.0
        };
    }
    (scale, power)
}

static PENDING_JUMP: Mutex<f32> = Mutex::new(0.0);

static DRIVEN: Mutex<Option<(i32, Vec3)>> = Mutex::new(None);

pub(crate) fn note_vehicle_correction(pos: Vec3) {
    if let Some(id) = crate::client::tracking::local_vehicle_id() {
        *DRIVEN.lock().unwrap() = Some((id, pos));
    }
}

pub(crate) fn reset(blocking: bool) {
    if blocking {
        *CHARGE.lock().unwrap() = JumpCharge::default();
        *PENDING_JUMP.lock().unwrap() = 0.0;
        *DRIVEN.lock().unwrap() = None;
    } else {
        if let Ok(mut charge) = CHARGE.try_lock() {
            *charge = JumpCharge::default();
        }
        if let Ok(mut pending) = PENDING_JUMP.try_lock() {
            *pending = 0.0;
        }
        if let Ok(mut driven) = DRIVEN.try_lock() {
            *driven = None;
        }
    }
    crate::client::tracking::LOCAL_VEHICLE_ID.store(-1, Ordering::Relaxed);
}

const DEG_TO_RAD: f32 = 0.017453292;

const PLAYER_VEHICLE_ATTACHMENT: f64 = 0.6;

fn y_rot(v: Vec3, radians: f32) -> Vec3 {
    let (sin, cos) = (radians.sin() as f64, radians.cos() as f64);
    Vec3 {
        x: v.x * cos + v.z * sin,
        y: v.y,
        z: v.z * cos - v.x * sin,
    }
}

fn seat(vehicle_id: i32) -> Option<(usize, usize)> {
    let map = crate::client::tracking::passengers().lock().unwrap();
    let riders = map.get(&vehicle_id)?;
    let local = crate::client::tracking::local_entity_id();
    let index = riders.iter().position(|id| *id == local)?;
    Some((index, riders.len()))
}

fn passenger_attachment(kind: EntityKind, yaw: f32, index: usize, riders: usize) -> Vec3 {
    use crate::entities::render::objects::boat;
    use EntityKind as K;

    let height = azalea::entity::dimensions::EntityDimensions::from(kind).height as f64;
    let (y, z) = match kind {
        K::Horse => (1.44375, 0.0),
        K::SkeletonHorse | K::ZombieHorse => (1.31875, 0.0),
        K::Donkey => (1.1125, 0.0),
        K::Mule => (1.2125, 0.0),
        K::Llama | K::TraderLlama => (1.37, -0.3),
        K::Pig => (0.86875, 0.0),
        K::Camel => (
            height - 0.375,
            if riders > 1 && index > 0 { -0.7 } else { 0.5 },
        ),
        K::Minecart
        | K::ChestMinecart
        | K::FurnaceMinecart
        | K::TntMinecart
        | K::HopperMinecart
        | K::SpawnerMinecart
        | K::CommandBlockMinecart => (0.1875, 0.0),
        _ if boat::BOATS.contains(&kind) => (height / 3.0, boat_seat(index, riders, 0.0)),
        _ if boat::CHEST_BOATS.contains(&kind) => (height / 3.0, boat_seat(index, riders, 0.15)),
        K::BambooRaft => (height * 0.8888889, boat_seat(index, riders, 0.0)),
        K::BambooChestRaft => (height * 0.8888889, boat_seat(index, riders, 0.15)),
        _ => (height, 0.0),
    };
    y_rot(Vec3 { x: 0.0, y, z }, -yaw * DEG_TO_RAD)
}

fn boat_seat(index: usize, riders: usize, single: f64) -> f64 {
    if riders <= 1 {
        single
    } else if index == 0 {
        0.2
    } else {
        -0.6
    }
}

fn controls(kind: EntityKind, vehicle_id: i32, held: ItemKind) -> bool {
    use EntityKind as K;

    let saddled = crate::client::tracking::equipment()
        .lock()
        .unwrap()
        .get(&vehicle_id)
        .is_some_and(|worn| worn.saddle.is_some());
    if !saddled {
        return false;
    }
    match kind {
        K::Horse | K::Donkey | K::Mule | K::SkeletonHorse | K::ZombieHorse | K::Camel => true,
        K::Pig => held == ItemKind::CarrotOnAStick,
        K::Strider => held == ItemKind::WarpedFungusOnAStick,
        _ => false,
    }
}

fn ridden_input(kind: EntityKind, x_impulse: f32, z_impulse: f32, rearing: bool) -> Vec3 {
    use EntityKind as K;

    match kind {
        K::Pig | K::Strider => Vec3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
        _ => {
            if rearing && *PENDING_JUMP.lock().unwrap() == 0.0 {
                return Vec3::ZERO;
            }
            let forward = if z_impulse <= 0.0 {
                z_impulse * 0.25
            } else {
                z_impulse
            };
            Vec3 {
                x: (x_impulse * 0.5) as f64,
                y: 0.0,
                z: forward as f64,
            }
        }
    }
}

fn ridden_speed(kind: EntityKind, speed: f64) -> f64 {
    use EntityKind as K;

    match kind {
        K::Pig => speed * 0.225,
        K::Strider => speed * 0.55,
        _ => speed,
    }
}

fn execute_riders_jump(
    physics: &mut Physics,
    yaw: f32,
    jump_strength: f64,
    amount: f32,
    input: Vec3,
) {
    physics.velocity.y = jump_strength * amount as f64;
    if input.z > 0.0 {
        let (sin, cos) = ((yaw * DEG_TO_RAD).sin(), (yaw * DEG_TO_RAD).cos());
        physics.velocity.x += (-0.4 * sin * amount) as f64;
        physics.velocity.z += (0.4 * cos * amount) as f64;
    }
}

pub struct RidingPlugin;

impl Plugin for RidingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            GameTick,
            (
                ride_pre_travel
                    .in_set(PhysicsSystems)
                    .after(azalea::physics::ai_step)
                    .before(travel),
                ride_post_travel
                    .after(PhysicsSystems)
                    .before(azalea::movement::send_position),
            ),
        );
    }
}

#[allow(clippy::type_complexity)]
fn ride_pre_travel(
    local: Query<
        (
            Entity,
            &LookDirection,
            &EntityIdIndex,
            &ClientMovementState,
            &Inventory,
        ),
        With<LocalEntity>,
    >,
    mut mounts: Query<
        (
            Entity,
            &EntityKindComponent,
            &mut Position,
            &mut Physics,
            &mut LookDirection,
            &mut Attributes,
            &WorldName,
            &OnClimbable,
            Option<&AbstractHorseStanding>,
        ),
        Without<LocalEntity>,
    >,
    worlds: Res<Worlds>,
    aabb_query: AabbQuery,
    collidable_entity_query: CollidableEntityQuery,
    mut commands: Commands,
) {
    let Some(vehicle_id) = crate::client::tracking::local_vehicle_id() else {
        return;
    };
    let Ok((rider, rider_direction, id_index, movement, inventory)) = local.single() else {
        return;
    };
    let Some(mount) = id_index.get_by_minecraft_entity(MinecraftEntityId(vehicle_id)) else {
        crate::client::tracking::LOCAL_VEHICLE_ID.store(-1, Ordering::Relaxed);
        return;
    };
    let Ok((
        entity,
        kind,
        mut position,
        mut physics,
        mut direction,
        mut attributes,
        world_name,
        on_climbable,
        standing,
    )) = mounts.get_mut(mount)
    else {
        return;
    };

    let kind = **kind;
    if seat(vehicle_id).is_none_or(|(index, _)| index != 0)
        || !controls(kind, vehicle_id, inventory.held_item().kind())
    {
        return;
    }

    match *DRIVEN.lock().unwrap() {
        Some((id, driven_pos)) if id == vehicle_id => **position = driven_pos,
        _ => {}
    }

    *direction = LookDirection::new(rider_direction.y_rot(), rider_direction.x_rot() * 0.5);
    let yaw = direction.y_rot();

    let attrs = crate::client::tracking::mount_attributes()
        .lock()
        .unwrap()
        .get(&vehicle_id)
        .copied()
        .unwrap_or_default();

    let rearing = physics.on_ground() && standing.is_some_and(|s| s.0);
    let input = ridden_input(
        kind,
        movement.move_vector.x,
        movement.move_vector.y,
        rearing,
    );

    {
        let mut pending = PENDING_JUMP.lock().unwrap();
        if physics.on_ground() {
            if *pending > 0.0 {
                execute_riders_jump(
                    &mut physics,
                    yaw,
                    attrs.jump_strength.unwrap_or(DEFAULT_JUMP_STRENGTH),
                    *pending,
                    input,
                );
            }
            *pending = 0.0;
        }
    }

    attributes.movement_speed.base = ridden_speed(
        kind,
        attrs
            .movement_speed
            .unwrap_or_else(|| default_movement_speed(kind)),
    );
    attributes.step_height.base = attrs.step_height.unwrap_or(DEFAULT_STEP_HEIGHT);

    let Some(world_lock) = worlds.get(world_name) else {
        return;
    };
    let world = world_lock.read();

    physics.x_acceleration = input.x as f32;
    physics.y_acceleration = input.y as f32;
    physics.z_acceleration = input.z as f32;

    let mut ctx = MoveCtx {
        mover_type: MoverType::Own,
        world: &world,
        position: position.reborrow(),
        physics: &mut physics,
        source_entity: entity,
        aabb_query: &aabb_query,
        collidable_entity_query: &collidable_entity_query,
        physics_state: None,
        attributes: &attributes,
        abilities: None,
        active_effects: None,
        powder_snow_walkable: false,
        fall_flying: false,
        direction: *direction,
        sprinting: Sprinting(false),
        on_climbable: *on_climbable,
        pose: None,
        jumping: Jumping(false),
        no_physics: false,
    };
    travel_ctx(&mut ctx);
    drop(ctx);
    *DRIVEN.lock().unwrap() = Some((vehicle_id, **position));

    commands.trigger(SendGamePacketEvent::new(
        rider,
        ServerboundMoveVehicle {
            pos: **position,
            look_direction: *direction,
        },
    ));
}

#[allow(clippy::type_complexity)]
fn ride_post_travel(
    mut rider: Query<(&mut Position, &mut Physics, &EntityIdIndex), With<LocalEntity>>,
    mounts: Query<
        (&Position, &Physics, &LookDirection, &EntityKindComponent),
        Without<LocalEntity>,
    >,
) {
    let Some(vehicle_id) = crate::client::tracking::local_vehicle_id() else {
        return;
    };
    let Some((index, riders)) = seat(vehicle_id) else {
        return;
    };
    let Ok((mut position, mut physics, id_index)) = rider.single_mut() else {
        return;
    };
    let Some(mount) = id_index.get_by_minecraft_entity(MinecraftEntityId(vehicle_id)) else {
        return;
    };
    let Ok((mount_position, mount_physics, mount_direction, kind)) = mounts.get(mount) else {
        return;
    };

    let seat_pos =
        **mount_position + passenger_attachment(**kind, mount_direction.y_rot(), index, riders);
    physics.velocity = Vec3::ZERO;
    **position = Vec3 {
        x: seat_pos.x,
        y: seat_pos.y - PLAYER_VEHICLE_ATTACHMENT,
        z: seat_pos.z,
    };
    physics.set_on_ground(mount_physics.on_ground());
}

const DEFAULT_JUMP_STRENGTH: f64 = 0.7;

const DEFAULT_STEP_HEIGHT: f64 = 1.0;

fn default_movement_speed(kind: EntityKind) -> f64 {
    use EntityKind as K;

    match kind {
        K::Camel => 0.09000000357627869,
        K::Pig => 0.25,
        K::Strider => 0.17499999701976776,
        _ => 0.22499999403953552,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charge_ramps_then_tails() {
        let mut c = JumpCharge::default();
        assert_eq!(c.tick(true, true), (Some(0.0), None));
        for expected in 1..10 {
            let (scale, power) = c.tick(true, true);
            assert!((scale.unwrap() - expected as f32 * 0.1).abs() < 1e-6);
            assert_eq!(power, None);
        }
        let (scale, _) = c.tick(true, true);
        assert!((scale.unwrap() - 1.0).abs() < 1e-6);
        let (scale, _) = c.tick(true, true);
        assert!((scale.unwrap() - 0.9).abs() < 1e-6);
    }

    #[test]
    fn release_sends_the_charge_once() {
        let mut c = JumpCharge::default();
        for _ in 0..6 {
            c.tick(true, true);
        }
        let (_, power) = c.tick(true, false);
        assert_eq!(power, Some(50));
        for _ in 0..12 {
            assert_eq!(c.tick(true, false).1, None);
        }
    }

    #[test]
    fn no_mount_publishes_nothing() {
        let mut c = JumpCharge::default();
        assert_eq!(c.tick(false, true), (None, None));
        assert_eq!(c.tick(false, false), (None, None));
    }

    #[test]
    fn reset_clears_the_charge_and_the_mount_either_way() {
        for blocking in [true, false] {
            *CHARGE.lock().unwrap() = JumpCharge {
                ticks: 7,
                scale: 0.7,
                was_jumping: true,
            };
            *PENDING_JUMP.lock().unwrap() = 0.75;
            *DRIVEN.lock().unwrap() = Some((42, Vec3::new(1.0, 2.0, 3.0)));
            crate::client::tracking::LOCAL_VEHICLE_ID.store(42, Ordering::Relaxed);

            reset(blocking);

            {
                let charge = CHARGE.lock().unwrap();
                assert_eq!(charge.ticks, 0, "{blocking}");
                assert_eq!(charge.scale, 0.0, "{blocking}");
                assert!(!charge.was_jumping, "{blocking}");
            }
            assert_eq!(*PENDING_JUMP.lock().unwrap(), 0.0, "{blocking}");
            assert!(DRIVEN.lock().unwrap().is_none(), "{blocking}");
            assert_eq!(
                crate::client::tracking::LOCAL_VEHICLE_ID.load(Ordering::Relaxed),
                -1,
                "{blocking}"
            );
        }
    }
}
