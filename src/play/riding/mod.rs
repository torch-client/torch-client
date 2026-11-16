mod boat;
mod mount;
#[cfg(feature = "audio")]
pub(crate) mod sound;
mod travel;

use std::sync::Mutex;
use std::sync::atomic::Ordering;

use azalea::app::{App, Plugin};
use azalea::ecs::prelude::*;
use azalea::entity::dimensions::EntityDimensions;
use azalea::entity::indexing::EntityIdIndex;
use azalea::entity::inventory::Inventory;
use azalea::entity::metadata::{
    AbstractHorseStanding, AbstractNautilusDash, CamelDash, LastPoseChangeTick, NoGravity,
    PigBoostTime, Sprinting, StaysStill, StriderBoostTime, Suffocating,
};
use azalea::entity::{
    ActiveEffects, Attributes, EntityKindComponent, HasClientLoaded, Jumping, LastSentPosition,
    LocalEntity, LookDirection, OnClimbable, Physics, Position, view_vector,
};
use azalea::movement::LastSentLookDirection;
use azalea::packet::game::SendGamePacketEvent;
use azalea::physics::PhysicsSystems;
use azalea::physics::client_movement::ClientMovementState;
use azalea::physics::collision::entity_collisions::{
    AabbQuery, CollidableEntityQuery, VehicleCollidableQuery, VehicleCollisions,
};
use azalea::physics::collision::{
    BorderSource, MoveCtx, MoverType, block_speed_factor_for, move_colliding,
};
use azalea::physics::fluids::{FluidPush, PassengerFluidBox, update_fluid_interaction};
use azalea::physics::travel::no_collision;
use azalea::physics::{
    EntityMovement, InsideKind, apply_effects_from_blocks_for, block_jump_factor, jump_boost_power,
};
use azalea::world::{WorldBorder, WorldName, Worlds};
use azalea_core::entity_id::MinecraftEntityId;
use azalea_core::math;
use azalea_core::position::Vec3;
use azalea_core::tick::GameTick;
use azalea_inventory::components::EquipmentSlot;
use azalea_protocol::common::movements::{MoveFlags, PositionMoveRotation, RelativeMovements};
use azalea_protocol::packets::game::c_teleport_entity::ClientboundTeleportEntity;
use azalea_protocol::packets::game::s_move_player_rot::ServerboundMovePlayerRot;
use azalea_protocol::packets::game::s_move_vehicle::ServerboundMoveVehicle;
use azalea_protocol::packets::game::s_paddle_boat::ServerboundPaddleBoat;
use azalea_registry::builtin::{Attribute, EntityKind, ItemKind};

use boat::Boat;
pub(crate) use boat::{Keys, hull_under_water};
use mount::{Gear, Jump, Rider};
pub(crate) use mount::{
    Hull, Mount, Passenger, SeatPose, passenger_attachment, vehicle_attachment,
};

use crate::entities::feed::stand_anim_step;
use crate::util::interp::{Interpolation, Target};
use crate::util::mth::DEG_TO_RAD;

#[derive(Clone, Copy, Debug, Default)]
struct JumpCharge {
    ticks: i32,
    scale: f32,
    was_jumping: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct JumpGate {
    cooldown: i32,
    accepts: bool,
}

impl JumpCharge {
    const NEW: JumpCharge = JumpCharge {
        ticks: 0,
        scale: 0.0,
        was_jumping: false,
    };

    fn tick(&mut self, gate: Option<JumpGate>, jump_held: bool) -> (Option<f32>, Option<u32>) {
        let was_jumping = std::mem::replace(&mut self.was_jumping, jump_held);

        let Some(gate) = gate else {
            self.scale = 0.0;
            return (None, None);
        };
        if gate.cooldown != 0 {
            return (Some(self.scale), None);
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

#[derive(Clone, Copy, Debug, Default)]
struct Steering {
    boosting: bool,
    time: i32,
    total: i32,
}

impl Steering {
    fn sync(&mut self, total: i32) {
        if total != self.total {
            self.total = total;
            self.boosting = true;
            self.time = 0;
        }
    }

    fn tick(&mut self) {
        if self.boosting {
            let past = self.time > self.total;
            self.time += 1;
            if past {
                self.boosting = false;
            }
        }
    }

    fn factor(&self) -> f32 {
        if self.boosting && self.total > 0 {
            1.0 + 1.15 * math::sin(self.time as f32 / self.total as f32 * std::f32::consts::PI)
        } else {
            1.0
        }
    }
}

#[derive(Debug)]
struct Driven {
    id: i32,
    mount: Mount,
    pos: Vec3,
    y_rot: f32,
    x_rot: f32,
    on_ground: bool,
    dash_cooldown: i32,
    dash_synced: bool,
    steering: Steering,
    jump: Option<JumpGate>,
    boat: Boat,
    interp: Interpolation,
    moved: bool,
}

impl Driven {
    fn new(id: i32, mount: Mount, pos: Vec3, look: LookDirection, on_ground: bool) -> Driven {
        Driven {
            id,
            mount,
            pos,
            y_rot: look.y_rot(),
            x_rot: look.x_rot(),
            on_ground,
            dash_cooldown: 0,
            dash_synced: false,
            steering: Steering::default(),
            jump: None,
            boat: Boat::default(),
            interp: Interpolation::new(),
            moved: false,
        }
    }

    fn packet(&self) -> ServerboundMoveVehicle {
        ServerboundMoveVehicle {
            pos: self.pos,
            look_direction: LookDirection::new(self.y_rot, self.x_rot),
            on_ground: self.on_ground,
        }
    }

    fn with_target<R>(&mut self, f: impl FnOnce(&mut Interpolation, Target<'_>) -> R) -> R {
        let mut pos = [self.pos.x, self.pos.y, self.pos.z];
        let r = f(
            &mut self.interp,
            Target {
                pos: &mut pos,
                y_rot: &mut self.y_rot,
                x_rot: &mut self.x_rot,
            },
        );
        self.pos = Vec3::new(pos[0], pos[1], pos[2]);
        r
    }

    fn correct(&mut self, pos: Vec3, y_rot: f32, x_rot: f32) -> ServerboundMoveVehicle {
        let current = self.interp.position([self.pos.x, self.pos.y, self.pos.z]);
        if pos.distance_to(Vec3::new(current[0], current[1], current[2])) > 9.999999747378752E-6 {
            self.interp.cancel();
            self.pos = pos;
            self.y_rot = y_rot % 360.0;
            self.x_rot = x_rot.clamp(-90.0, 90.0) % 360.0;
        }
        self.packet()
    }

    fn teleport(
        &mut self,
        change: &PositionMoveRotation,
        relative: &RelativeMovements,
        on_ground: bool,
        physics: &mut Physics,
    ) -> Option<ServerboundMoveVehicle> {
        let velocity = physics.velocity;
        let mut position = Position::new(self.pos);
        let mut look = LookDirection::new(self.y_rot, self.x_rot);
        relative.apply(change, &mut position, &mut look, physics);
        physics.velocity = velocity;
        let target = *position;
        let echo = if self.pos.distance_squared_to(target) > 4096.0 {
            self.interp.cancel();
            self.pos = target;
            self.y_rot = look.y_rot();
            self.x_rot = look.x_rot();
            true
        } else {
            self.with_target(|interp, entity| {
                interp.interpolate_to(
                    entity,
                    [target.x, target.y, target.z],
                    look.y_rot(),
                    look.x_rot(),
                );
            });
            false
        };
        self.on_ground = on_ground;
        echo.then(|| self.packet())
    }
}

enum VehicleUpdate {
    Correction {
        id: i32,
        pos: Vec3,
        y_rot: f32,
        x_rot: f32,
    },
    Teleport {
        id: i32,
        change: PositionMoveRotation,
        relative: RelativeMovements,
        on_ground: bool,
    },
}

impl VehicleUpdate {
    fn id(&self) -> i32 {
        match self {
            VehicleUpdate::Correction { id, .. } | VehicleUpdate::Teleport { id, .. } => *id,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Seat {
    vehicle: i32,
    stand: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RiderTurn {
    pub delta: f32,
    pub boat_yaw: f32,
}

pub(crate) const RIDER_YAW_LIMIT: f32 = 105.0;

struct Riding {
    charge: JumpCharge,
    pending_jump: f32,
    keys: Keys,
    driven: Option<Driven>,
    seat: Seat,
    turn: Option<RiderTurn>,
    updates: Vec<VehicleUpdate>,
    boarded: Option<i32>,
}

impl Riding {
    const NEW: Riding = Riding {
        charge: JumpCharge::NEW,
        pending_jump: 0.0,
        keys: Keys::NONE,
        driven: None,
        seat: Seat {
            vehicle: -1,
            stand: 0.0,
        },
        turn: None,
        updates: Vec::new(),
        boarded: None,
    };

    fn hands_busy(&self) -> bool {
        self.driven
            .as_ref()
            .is_some_and(|d| matches!(d.mount, Mount::Boat(_)) && self.keys.any())
    }

    fn stop_driving(&mut self) {
        self.driven = None;
        self.pending_jump = 0.0;
    }

    fn idle(&mut self) {
        self.stop_driving();
        self.updates.clear();
    }
}

static RIDING: Mutex<Riding> = Mutex::new(Riding::NEW);

pub(crate) struct TickState {
    pub jump_charge: Option<f32>,
    pub jump_power: Option<u32>,
    pub jump_cooldown: bool,
    pub hands_busy: bool,
    pub rider_turn: Option<RiderTurn>,
}

pub(crate) fn tick(jump_held: bool) -> TickState {
    let mut riding = RIDING.lock().unwrap();
    let gate = riding.driven.as_ref().and_then(|d| d.jump);
    let (jump_charge, jump_power) = riding.charge.tick(gate, jump_held);
    if let Some(power) = jump_power
        && gate.is_some_and(|g| g.accepts)
    {
        riding.pending_jump = if power >= 90 {
            1.0
        } else {
            0.4 + 0.4 * power as f32 / 90.0
        };
    }
    TickState {
        jump_charge,
        jump_power,
        jump_cooldown: gate.is_some_and(|g| g.cooldown > 0),
        hands_busy: riding.hands_busy(),
        rider_turn: riding.turn,
    }
}

pub(crate) fn set_keys(keys: Keys) {
    RIDING.lock().unwrap().keys = keys;
}

pub(crate) fn boat_yaw() -> Option<f32> {
    RIDING.lock().unwrap().turn.map(|t| t.boat_yaw)
}

pub(crate) fn hands_busy() -> bool {
    RIDING.lock().unwrap().hands_busy()
}

pub(crate) fn vehicle_can_sprint() -> bool {
    RIDING
        .lock()
        .unwrap()
        .driven
        .as_ref()
        .is_some_and(|d| d.mount.can_sprint())
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DrivenPose {
    pub id: i32,
    pub pos: Vec3,
    pub y_rot: f32,
    pub x_rot: f32,
    pub paddles: Option<[bool; 2]>,
}

pub(crate) fn driven_pose() -> Option<DrivenPose> {
    RIDING.lock().unwrap().driven.as_ref().map(|d| DrivenPose {
        id: d.id,
        pos: d.pos,
        y_rot: d.y_rot,
        x_rot: d.x_rot,
        paddles: matches!(d.mount, Mount::Boat(_)).then_some(d.boat.paddles),
    })
}

pub(crate) fn note_vehicle_correction(pos: Vec3, y_rot: f32, x_rot: f32) {
    let Some(id) = crate::client::tracking::local_vehicle_id() else {
        return;
    };
    RIDING
        .lock()
        .unwrap()
        .updates
        .push(VehicleUpdate::Correction {
            id,
            pos,
            y_rot,
            x_rot,
        });
}

pub(crate) fn note_vehicle_teleport(p: &ClientboundTeleportEntity) {
    if crate::client::tracking::local_vehicle_id() != Some(p.id.0) {
        return;
    }
    RIDING
        .lock()
        .unwrap()
        .updates
        .push(VehicleUpdate::Teleport {
            id: p.id.0,
            change: p.change.clone(),
            relative: p.relative.clone(),
            on_ground: p.on_ground,
        });
}

pub(crate) fn note_boarded(vehicle: i32) {
    RIDING.lock().unwrap().boarded = Some(vehicle);
}

pub(crate) fn reset(blocking: bool) {
    let guard = if blocking {
        Some(RIDING.lock().unwrap())
    } else {
        RIDING.try_lock().ok()
    };
    if let Some(mut riding) = guard {
        *riding = Riding::NEW;
    }
    crate::client::tracking::LOCAL_VEHICLE_ID.store(-1, Ordering::Relaxed);
}

fn seat(vehicle_id: i32) -> Option<(usize, usize)> {
    let map = crate::client::tracking::passengers().lock().unwrap();
    let riders = map.get(&vehicle_id)?;
    let local = crate::client::tracking::local_entity_id();
    let index = riders.iter().position(|id| *id == local)?;
    Some((index, riders.len()))
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
                    .before(azalea::physics::travel::travel)
                    .before(azalea::movement::send_player_input_packet),
                ride_post_travel
                    .after(PhysicsSystems)
                    .before(azalea::movement::send_position),
                send_rider_rotation
                    .after(ride_post_travel)
                    .after(azalea::movement::send_player_input_packet)
                    .before(azalea::movement::send_sprinting_if_needed)
                    .before(azalea::movement::send_position),
            ),
        );
    }
}

#[allow(clippy::type_complexity)]
fn send_rider_rotation(
    mut local: Query<
        (
            Entity,
            &Position,
            &LookDirection,
            &mut Physics,
            &mut ClientMovementState,
            &mut LastSentPosition,
            &mut LastSentLookDirection,
        ),
        (With<LocalEntity>, With<HasClientLoaded>),
    >,
    mut commands: Commands,
) {
    if crate::client::tracking::local_vehicle_id().is_none() {
        return;
    }
    for (entity, position, direction, mut physics, mut movement, mut last_pos, mut last_look) in
        &mut local
    {
        let on_ground = physics.on_ground();
        let horizontal_collision = physics.horizontal_collision;
        commands.trigger(SendGamePacketEvent::new(
            entity,
            ServerboundMovePlayerRot {
                look_direction: *direction,
                flags: MoveFlags {
                    on_ground,
                    horizontal_collision,
                },
            },
        ));
        let vehicle_move = RIDING
            .lock()
            .unwrap()
            .driven
            .as_mut()
            .and_then(|d| std::mem::take(&mut d.moved).then(|| d.packet()));
        if let Some(packet) = vehicle_move {
            commands.trigger(SendGamePacketEvent::new(entity, packet));
        }
        **last_pos = **position;
        last_look.y_rot = direction.y_rot();
        last_look.x_rot = direction.x_rot();
        movement.position_remainder = 0;
        movement.last_horizontal_collision = horizontal_collision;
        physics.set_last_on_ground(on_ground);
    }
}

type MountMeta = (
    Option<&'static AbstractHorseStanding>,
    Option<&'static CamelDash>,
    Option<&'static AbstractNautilusDash>,
    Option<&'static LastPoseChangeTick>,
    Option<&'static PigBoostTime>,
    Option<&'static StriderBoostTime>,
    Option<&'static Suffocating>,
    Option<&'static StaysStill>,
    Option<&'static NoGravity>,
);

pub(crate) fn camel_pose(last_change: Option<&LastPoseChangeTick>) -> (bool, i64) {
    let last = last_change.map_or(0, |t| t.0);
    let now = crate::client::tracking::GAME_TIME.load(Ordering::Relaxed);
    (last < 0, now - last.abs())
}

fn camel_refuses(last_change: Option<&LastPoseChangeTick>) -> bool {
    let (sitting, time) = camel_pose(last_change);
    sitting || time < 52
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn ride_pre_travel(
    local: Query<
        (
            Entity,
            &LookDirection,
            &EntityIdIndex,
            &ClientMovementState,
            &Inventory,
            &Jumping,
            Option<&Sprinting>,
            Option<&WorldBorder>,
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
            &EntityDimensions,
            &WorldName,
            &OnClimbable,
            &ActiveEffects,
            MountMeta,
        ),
        Without<LocalEntity>,
    >,
    worlds: Res<Worlds>,
    aabb_query: AabbQuery,
    collidable_entity_query: CollidableEntityQuery,
    vehicle_query: VehicleCollidableQuery,
    mut commands: Commands,
) {
    let mut riding = RIDING.lock().unwrap();
    let Some(vehicle_id) = crate::client::tracking::local_vehicle_id() else {
        riding.idle();
        return;
    };
    let Ok((rider, rider_look, id_index, movement, inventory, jumping, sprinting, border)) =
        local.single()
    else {
        return;
    };
    let Some(vehicle) = id_index.get_by_minecraft_entity(MinecraftEntityId(vehicle_id)) else {
        crate::client::tracking::LOCAL_VEHICLE_ID.store(-1, Ordering::Relaxed);
        riding.idle();
        return;
    };
    let Ok((
        entity,
        kind,
        mut position,
        mut physics,
        mut direction,
        mut attributes,
        dimensions,
        world_name,
        on_climbable,
        active_effects,
        (
            standing,
            camel_dash,
            nautilus_dash,
            pose_change,
            pig_boost,
            strider_boost,
            suffocating,
            still,
            no_gravity,
        ),
    )) = mounts.get_mut(vehicle)
    else {
        riding.idle();
        return;
    };

    let kind = **kind;
    let Some(mount) = Mount::of(kind) else {
        riding.idle();
        return;
    };
    let worn = crate::client::tracking::equipment()
        .lock()
        .unwrap()
        .get(&vehicle_id)
        .map(|w| (w.saddle.is_some(), w.body_armor.is_some()));
    let gear = Gear {
        saddled: worn.is_some_and(|(saddle, _)| saddle),
        harnessed: worn.is_some_and(|(_, body)| body),
        still: still.is_some_and(|s| s.0),
        held: [
            inventory.held_item().kind(),
            inventory
                .get_equipment(EquipmentSlot::Offhand)
                .map_or(ItemKind::Air, |stack| stack.kind()),
        ],
    };
    if seat(vehicle_id).is_none_or(|(index, _)| index != 0) || !mount.steered(&gear) {
        riding.idle();
        return;
    }

    let Some(world_lock) = worlds.get(world_name) else {
        return;
    };
    let world = world_lock.read();

    let dashing = camel_dash.map(|d| d.0).or(nautilus_dash.map(|d| d.0));
    let boost_total = pig_boost.map(|b| b.0).or(strider_boost.map(|b| b.0));
    let riding = &mut *riding;
    if riding.driven.as_ref().is_none_or(|d| d.id != vehicle_id) {
        riding.stop_driving();
        let mut driven = Driven::new(
            vehicle_id,
            mount,
            **position,
            *direction,
            physics.on_ground(),
        );
        driven.dash_synced = dashing.unwrap_or(false);
        driven.steering.total = boost_total.unwrap_or(0);
        riding.driven = Some(driven);
    }
    let Some(driven) = riding.driven.as_mut() else {
        return;
    };

    for update in riding.updates.drain(..) {
        if update.id() != vehicle_id {
            continue;
        }
        let echo = match update {
            VehicleUpdate::Correction {
                pos, y_rot, x_rot, ..
            } => Some(driven.correct(pos, y_rot, x_rot)),
            VehicleUpdate::Teleport {
                change,
                relative,
                on_ground,
                ..
            } => driven.teleport(&change, &relative, on_ground, &mut physics),
        };
        if let Some(packet) = echo {
            commands.trigger(SendGamePacketEvent::new(rider, packet));
        }
    }

    **position = driven.pos;
    *direction = LookDirection::new(driven.y_rot, driven.x_rot);
    physics.set_on_ground(driven.on_ground);
    physics.bounding_box = dimensions.make_bounding_box(driven.pos);

    let (movement_speed, jump_strength, step_height, flying_speed) = {
        let map = crate::client::tracking::attributes().lock().unwrap();
        let attrs = map.get(&vehicle_id);
        let value = |attribute| attrs.and_then(|a| a.value(attribute));
        (
            value(Attribute::MovementSpeed).unwrap_or_else(|| mount::default_movement_speed(kind)),
            value(Attribute::JumpStrength).unwrap_or_else(|| mount::default_jump_strength(mount)),
            value(Attribute::StepHeight).unwrap_or_else(|| mount::default_step_height(mount)),
            value(Attribute::FlyingSpeed).unwrap_or(mount::DEFAULT_FLYING_SPEED),
        )
    };
    attributes.step_height.base = step_height;

    if let Mount::Boat(_) = mount {
        driven.boat.update_status(&world, &physics.bounding_box);
    }
    let push = if mount.pushed_by_fluid() {
        FluidPush::Entity
    } else {
        FluidPush::None
    };
    update_fluid_interaction(&mut physics, &world, world_name, push, None);

    if driven.interp.has_active_interpolation() {
        let border_source = BorderSource::at(border, driven.pos.x, driven.pos.z);
        driven.with_target(|interp, target| {
            interp.interpolate_with(target, |shifted| {
                no_collision(
                    &world,
                    Some(entity),
                    &aabb_query,
                    &collidable_entity_query,
                    border_source,
                    &dimensions.make_bounding_box(Vec3::new(shifted[0], shifted[1], shifted[2])),
                    false,
                    Default::default(),
                )
            });
        });
        **position = driven.pos;
        *direction = LookDirection::new(driven.y_rot, driven.x_rot);
        physics.bounding_box = dimensions.make_bounding_box(driven.pos);
    }
    let start = driven.pos;

    let ridden = if let Mount::Boat(_) = mount {
        None
    } else {
        let v = &mut physics.velocity;
        for c in [&mut v.x, &mut v.y, &mut v.z] {
            if c.abs() < 0.003 {
                *c = 0.0;
            }
        }

        if let Some(total) = boost_total {
            driven.steering.sync(total);
        }
        if let (Some(dashing), Jump::Dash { cooldown }) = (dashing, mount.jump())
            && dashing != driven.dash_synced
        {
            driven.dash_synced = dashing;
            if driven.dash_cooldown == 0 {
                driven.dash_cooldown = cooldown;
            }
        }

        let refuse = mount == Mount::Camel && camel_refuses(pose_change);
        let on_ground = physics.on_ground();
        let rearing_idle = on_ground && standing.is_some_and(|s| s.0) && riding.pending_jump == 0.0;
        let input = mount.ridden_input(
            Rider {
                xxa: movement.move_vector.x,
                zza: movement.move_vector.y,
                x_rot: rider_look.x_rot(),
                jumping: jumping.0,
            },
            refuse,
            rearing_idle,
            flying_speed,
        );

        let rotation = mount.ridden_rotation(
            (driven.y_rot, driven.x_rot),
            (rider_look.y_rot(), rider_look.x_rot()),
            refuse,
        );
        if matches!(mount, Mount::Pig | Mount::Strider) {
            driven.steering.tick();
        }
        let jump = if mount == Mount::Nautilus || on_ground {
            std::mem::take(&mut riding.pending_jump)
        } else {
            0.0
        };
        if jump > 0.0
            && let Jump::Dash { cooldown } = mount.jump()
        {
            driven.dash_cooldown = cooldown;
        }

        let speed = mount.ridden_speed(
            movement_speed,
            driven.steering.factor(),
            sprinting.is_some_and(|s| s.0) && driven.dash_cooldown == 0,
            suffocating.is_some_and(|s| s.0),
            physics.is_in_water(),
        );
        attributes.movement_speed.apply_snapshot(speed, &[]);
        Some(Ridden {
            input,
            rotation,
            refuse,
            jump,
            speed,
        })
    };

    let mut passengers = [Entity::PLACEHOLDER; 4];
    let mut riders = 0;
    for id in crate::client::tracking::passengers()
        .lock()
        .unwrap()
        .get(&vehicle_id)
        .into_iter()
        .flatten()
        .take(passengers.len())
    {
        if let Some(e) = id_index.get_by_minecraft_entity(MinecraftEntityId(*id)) {
            passengers[riders] = e;
            riders += 1;
        }
    }

    physics.base_tick_powder_snow();
    let mut ctx = MoveCtx {
        mover_type: MoverType::Own,
        world: &world,
        position: position.reborrow(),
        physics: &mut physics,
        source_entity: entity,
        aabb_query: &aabb_query,
        collidable_entity_query: &collidable_entity_query,
        vehicle_collisions: matches!(mount, Mount::Boat(_)).then_some(VehicleCollisions {
            query: &vehicle_query,
            passengers: &passengers[..riders],
        }),
        physics_state: None,
        attributes: &attributes,
        abilities: None,
        active_effects: Some(active_effects),
        powder_snow_walkable: false,
        stands_on_lava: mount == Mount::Strider,
        fall_flying: false,
        border,
        direction: *direction,
        sprinting: Sprinting(false),
        on_climbable: *on_climbable,
        pose: None,
        jumping: Jumping(false),
        no_physics: false,
    };

    match ridden {
        None => {
            let boat = &mut driven.boat;
            let hull = ctx.physics.bounding_box;
            let mut pos = **ctx.position;
            let mut velocity = ctx.physics.velocity;
            let start_y = pos.y;
            let border_source = BorderSource::at(border, pos.x, pos.z);
            let gravity = if no_gravity.is_some_and(|g| g.0) {
                0.0
            } else {
                boat::GRAVITY
            };
            boat.float(&world, &hull, gravity, &mut pos, &mut velocity, |target| {
                no_collision(
                    &world,
                    Some(entity),
                    &aabb_query,
                    &collidable_entity_query,
                    border_source,
                    &hull.move_relative(Vec3::new(0.0, target - start_y, 0.0)),
                    false,
                    Default::default(),
                )
            });
            if pos != **ctx.position {
                **ctx.position = pos;
                ctx.physics.bounding_box = dimensions.make_bounding_box(pos);
            }
            let mut y_rot = driven.y_rot;
            let paddles = boat.control(riding.keys, &mut y_rot, &mut velocity);
            ctx.physics.velocity = velocity;
            ctx.direction = LookDirection::new(y_rot, driven.x_rot);
            commands.trigger(SendGamePacketEvent::new(
                rider,
                ServerboundPaddleBoat {
                    left: paddles[0],
                    right: paddles[1],
                },
            ));
            let stuck = ctx.physics.stuck_speed_multiplier.length_squared() > 1.0e-7;
            boat.moved(if stuck { 0.0 } else { velocity.y });
            move_colliding(&mut ctx, velocity);
            drop(ctx);

            physics.bounding_box = dimensions.make_bounding_box(**position);
            let movement = EntityMovement {
                from: start,
                to: **position,
            };
            for _ in 0..2 {
                apply_effects_from_blocks_for(
                    &mut physics,
                    dimensions,
                    &world,
                    movement,
                    InsideKind::Boat,
                    active_effects,
                );
            }
            driven.y_rot = y_rot;
            driven.jump = None;
        }
        Some(r) => {
            ctx.direction = LookDirection::new(r.rotation.0, r.rotation.1);
            if r.jump > 0.0 {
                riders_jump(
                    mount,
                    &mut ctx,
                    r.jump,
                    r.input,
                    *rider_look,
                    jump_strength,
                    movement_speed,
                );
            }
            travel::travel(mount, &mut ctx, r.input, r.speed, flying_speed, r.refuse);
            drop(ctx);

            physics.bounding_box = dimensions.make_bounding_box(**position);
            let movement = EntityMovement {
                from: start,
                to: **position,
            };
            apply_effects_from_blocks_for(
                &mut physics,
                dimensions,
                &world,
                movement,
                InsideKind::Living,
                active_effects,
            );
            if mount == Mount::Strider {
                travel::float_strider(&mut physics, **position, &world);
            }
            driven.dash_cooldown = (driven.dash_cooldown - 1).max(0);

            (driven.y_rot, driven.x_rot) = r.rotation;
            driven.jump = (mount.jump() != Jump::None && !r.refuse).then(|| JumpGate {
                cooldown: driven.dash_cooldown,
                accepts: mount != Mount::Camel || physics.on_ground(),
            });
        }
    }

    *direction = LookDirection::new(driven.y_rot, driven.x_rot);
    driven.pos = **position;
    driven.on_ground = physics.on_ground();
    driven.moved = true;
}

fn set_fluid_box(
    commands: &mut Commands,
    local: Entity,
    now: Option<PassengerFluidBox>,
    want: Option<PassengerFluidBox>,
) {
    if now == want {
        return;
    }
    match want {
        Some(b) => {
            commands.entity(local).insert(b);
        }
        None => {
            commands.entity(local).remove::<PassengerFluidBox>();
        }
    }
}

struct Ridden {
    input: Vec3,
    rotation: (f32, f32),
    refuse: bool,
    jump: f32,
    speed: f64,
}

fn riders_jump(
    mount: Mount,
    ctx: &mut MoveCtx,
    amount: f32,
    input: Vec3,
    rider_look: LookDirection,
    jump_strength: f64,
    movement_speed: f64,
) {
    let (world, attributes, position) = (ctx.world, ctx.attributes, **ctx.position);
    let boost = ctx.active_effects.map_or(0.0, jump_boost_power) as f64;
    let jump_power = |amount: f32| {
        jump_strength * amount as f64 * block_jump_factor(world, Position::new(position)) as f64
            + boost
    };
    let speed_factor = || block_speed_factor_for(world, position, attributes) as f64;
    match mount.jump() {
        Jump::None => {}
        Jump::Leap => {
            ctx.physics.velocity.y = jump_power(amount);
            if input.z > 0.0 {
                let yaw = ctx.direction.y_rot() * DEG_TO_RAD;
                ctx.physics.velocity.x += (-0.4 * math::sin(yaw) * amount) as f64;
                ctx.physics.velocity.z += (0.4 * math::cos(yaw) * amount) as f64;
            }
        }
        Jump::Dash { .. } if mount == Mount::Camel => {
            let look = view_vector(ctx.direction);
            let flat = Vec3::new(look.x, 0.0, look.z).normalize();
            let push = (22.2222 * amount) as f64 * movement_speed * speed_factor();
            let lift = (1.4285 * amount) as f64 * jump_power(1.0);
            ctx.physics.velocity += flat * push + Vec3::new(0.0, lift, 0.0);
        }
        Jump::Dash { .. } => {
            let momentum = if ctx.physics.is_in_water() { 1.2 } else { 0.5 };
            let push = (momentum * amount) as f64 * movement_speed * speed_factor();
            ctx.physics.velocity += view_vector(rider_look) * push;
        }
    }
}

#[allow(clippy::type_complexity)]
fn ride_post_travel(
    mut rider: Query<
        (
            Entity,
            &mut Position,
            &mut Physics,
            &mut LookDirection,
            &EntityIdIndex,
            Option<&PassengerFluidBox>,
        ),
        With<LocalEntity>,
    >,
    mounts: Query<
        (
            &Position,
            &Physics,
            &LookDirection,
            &EntityKindComponent,
            &WorldName,
            Option<&AbstractHorseStanding>,
            Option<&LastPoseChangeTick>,
        ),
        Without<LocalEntity>,
    >,
    worlds: Res<Worlds>,
    mut commands: Commands,
) {
    let mut riding = RIDING.lock().unwrap();
    riding.turn = None;
    let Ok((local, mut position, mut physics, mut look, id_index, fluid_box_now)) =
        rider.single_mut()
    else {
        return;
    };
    let fluid_box_now = fluid_box_now.copied();
    let Some(vehicle_id) = crate::client::tracking::local_vehicle_id() else {
        set_fluid_box(&mut commands, local, fluid_box_now, None);
        return;
    };
    let Some((index, riders)) = seat(vehicle_id) else {
        set_fluid_box(&mut commands, local, fluid_box_now, None);
        return;
    };
    let Some(mount) = id_index.get_by_minecraft_entity(MinecraftEntityId(vehicle_id)) else {
        set_fluid_box(&mut commands, local, fluid_box_now, None);
        return;
    };
    let Ok((
        mount_position,
        mount_physics,
        mount_direction,
        kind,
        world_name,
        standing,
        pose_change,
    )) = mounts.get(mount)
    else {
        set_fluid_box(&mut commands, local, fluid_box_now, None);
        return;
    };

    let fluid_box = Hull::of(**kind).and_then(|_| {
        let under_water = match &riding.driven {
            Some(d) if d.id == vehicle_id => d.boat.is_under_water(),
            _ => worlds
                .get(world_name)
                .is_some_and(|w| hull_under_water(&w.read(), &mount_physics.bounding_box)),
        };
        (!under_water).then_some(PassengerFluidBox::AboveHull(
            mount_physics.bounding_box.max.y,
        ))
    });
    set_fluid_box(&mut commands, local, fluid_box_now, fluid_box);

    if riding.boarded.take() == Some(vehicle_id) && Hull::of(**kind).is_some() {
        *look = LookDirection::new(mount_direction.y_rot(), look.x_rot());
        commands
            .entity(local)
            .insert(azalea::movement::ServerSetLook);
    }

    let riding = &mut *riding;
    if riding.seat.vehicle != vehicle_id {
        riding.seat = Seat {
            vehicle: vehicle_id,
            stand: 0.0,
        };
    }
    let stand_old = riding.seat.stand;
    riding.seat.stand = stand_anim_step(stand_old, standing.is_some_and(|s| s.0));
    let (camel_sitting, camel_pose_time) = camel_pose(pose_change);
    let pose = SeatPose {
        stand: stand_old,
        camel_sitting,
        camel_pose_time,
    };

    let mount_yaw = mount_direction.y_rot();
    let (mount_scale, own_scale) = {
        use crate::client::tracking::{EntityAttributes, attributes, local_entity_id};
        let map = attributes().lock().unwrap();
        let scale = |id: i32| EntityAttributes::of(&map, id).scale() as f64;
        (scale(vehicle_id), scale(local_entity_id()))
    };
    let passenger = mount::Passenger {
        kind: EntityKind::Player,
        animal: false,
        index,
        of: riders,
    };
    let seat_pos = **mount_position
        + mount::passenger_attachment(**kind, mount_yaw, mount_scale, passenger, pose);
    physics.velocity = Vec3::ZERO;
    **position = Vec3 {
        x: seat_pos.x,
        y: seat_pos.y - mount::vehicle_attachment(EntityKind::Player, own_scale, 0.0, 0.0),
        z: seat_pos.z,
    };
    physics.set_on_ground(mount_physics.on_ground());

    if Hull::of(**kind).is_some() {
        let delta = match &riding.driven {
            Some(d) if d.id == vehicle_id => d.boat.delta_rotation,
            _ => 0.0,
        };
        riding.turn = Some(RiderTurn {
            delta,
            boat_yaw: mount_yaw,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HORSE: Option<JumpGate> = Some(JumpGate {
        cooldown: 0,
        accepts: true,
    });

    #[test]
    fn charge_ramps_then_tails() {
        let mut c = JumpCharge::default();
        assert_eq!(c.tick(HORSE, true), (Some(0.0), None));
        for expected in 1..10 {
            let (scale, power) = c.tick(HORSE, true);
            assert!((scale.unwrap() - expected as f32 * 0.1).abs() < 1e-6);
            assert_eq!(power, None);
        }
        let (scale, _) = c.tick(HORSE, true);
        assert!((scale.unwrap() - 1.0).abs() < 1e-6);
        let (scale, _) = c.tick(HORSE, true);
        assert!((scale.unwrap() - 0.9).abs() < 1e-6);
    }

    #[test]
    fn release_sends_the_charge_once() {
        let mut c = JumpCharge::default();
        for _ in 0..6 {
            c.tick(HORSE, true);
        }
        let (_, power) = c.tick(HORSE, false);
        assert_eq!(power, Some(50));
        for _ in 0..12 {
            assert_eq!(c.tick(HORSE, false).1, None);
        }
    }

    #[test]
    fn no_mount_publishes_nothing() {
        let mut c = JumpCharge::default();
        assert_eq!(c.tick(None, true), (None, None));
        assert_eq!(c.tick(None, false), (None, None));
    }

    #[test]
    fn a_cooling_dash_holds_the_charge() {
        let cooling = Some(JumpGate {
            cooldown: 12,
            accepts: true,
        });
        let mut c = JumpCharge::default();
        for _ in 0..5 {
            assert_eq!(c.tick(cooling, true), (Some(0.0), None));
        }
        assert_eq!(c.tick(cooling, false).1, None);
    }

    #[test]
    fn the_stick_boost_rises_and_falls() {
        let mut s = Steering::default();
        s.sync(140);
        for _ in 0..70 {
            s.tick();
        }
        assert!((s.factor() - 2.15).abs() < 1e-3);
        for _ in 0..71 {
            s.tick();
        }
        assert!(s.boosting);
        s.tick();
        assert!(!s.boosting);
        assert_eq!(s.factor(), 1.0);
    }

    fn boat_at(pos: Vec3, y_rot: f32) -> Driven {
        Driven::new(
            7,
            Mount::Boat(Hull::Boat),
            pos,
            LookDirection::new(y_rot, 0.0),
            false,
        )
    }

    #[test]
    fn a_correction_in_place_keeps_the_heading_but_still_answers() {
        let mut d = boat_at(Vec3::new(1.0, 2.0, 3.0), 30.0);
        let echo = d.correct(Vec3::new(1.0, 2.0, 3.0), 120.0, 5.0);
        assert_eq!(d.y_rot, 30.0);
        assert_eq!(echo.look_direction.y_rot(), 30.0);
        assert_eq!(echo.pos, Vec3::new(1.0, 2.0, 3.0));

        let echo = d.correct(Vec3::new(1.0, 2.0, 4.0), 400.0, 5.0);
        assert_eq!(d.pos, Vec3::new(1.0, 2.0, 4.0));
        assert_eq!(d.y_rot, 40.0);
        assert_eq!(echo.look_direction.y_rot(), 40.0);
    }

    #[test]
    fn a_teleport_interpolates_near_and_snaps_far() {
        let absolute = RelativeMovements::default();
        let mut physics = Physics::new(&EntityDimensions::new(1.375, 0.5625), Vec3::ZERO);
        let change = |pos| PositionMoveRotation {
            pos,
            delta: Vec3::ZERO,
            look_direction: LookDirection::new(90.0, 0.0),
        };

        let mut d = boat_at(Vec3::ZERO, 0.0);
        assert!(
            d.teleport(
                &change(Vec3::new(3.0, 0.0, 0.0)),
                &absolute,
                true,
                &mut physics
            )
            .is_none()
        );
        assert!(d.interp.has_active_interpolation());
        assert_eq!(d.pos, Vec3::ZERO);
        assert!(d.on_ground);
        d.with_target(|interp, target| interp.interpolate(target));
        assert!((d.pos.x - 1.0).abs() < 1e-9);

        let mut d = boat_at(Vec3::ZERO, 0.0);
        let echo = d.teleport(
            &change(Vec3::new(100.0, 0.0, 0.0)),
            &absolute,
            false,
            &mut physics,
        );
        assert_eq!(echo.map(|e| e.pos), Some(Vec3::new(100.0, 0.0, 0.0)));
        assert_eq!(d.y_rot, 90.0);
        assert!(!d.interp.has_active_interpolation());
    }

    #[test]
    fn reset_clears_the_charge_and_the_mount_either_way() {
        for blocking in [true, false] {
            {
                let mut riding = RIDING.lock().unwrap();
                riding.charge = JumpCharge {
                    ticks: 7,
                    scale: 0.7,
                    was_jumping: true,
                };
                riding.pending_jump = 0.75;
                riding.driven = Some(Driven::new(
                    42,
                    Mount::Horse,
                    Vec3::new(1.0, 2.0, 3.0),
                    LookDirection::default(),
                    true,
                ));
                riding.updates.push(VehicleUpdate::Correction {
                    id: 42,
                    pos: Vec3::ZERO,
                    y_rot: 0.0,
                    x_rot: 0.0,
                });
                riding.boarded = Some(42);
            }
            crate::client::tracking::LOCAL_VEHICLE_ID.store(42, Ordering::Relaxed);

            reset(blocking);

            {
                let riding = RIDING.lock().unwrap();
                assert_eq!(riding.charge.ticks, 0, "{blocking}");
                assert_eq!(riding.charge.scale, 0.0, "{blocking}");
                assert!(!riding.charge.was_jumping, "{blocking}");
                assert_eq!(riding.pending_jump, 0.0, "{blocking}");
                assert!(riding.driven.is_none(), "{blocking}");
                assert!(riding.updates.is_empty(), "{blocking}");
                assert_eq!(riding.boarded, None, "{blocking}");
            }
            assert_eq!(
                crate::client::tracking::LOCAL_VEHICLE_ID.load(Ordering::Relaxed),
                -1,
                "{blocking}"
            );
        }
    }
}
