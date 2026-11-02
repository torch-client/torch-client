mod boat;
mod mount;
mod travel;

use std::sync::Mutex;
use std::sync::atomic::Ordering;

use azalea::app::{App, Plugin};
use azalea::ecs::prelude::*;
use azalea::entity::dimensions::EntityDimensions;
use azalea::entity::indexing::EntityIdIndex;
use azalea::entity::inventory::Inventory;
use azalea::entity::metadata::{
    AbstractHorseStanding, AbstractNautilusDash, CamelDash, LastPoseChangeTick, PigBoostTime,
    Sprinting, StaysStill, StriderBoostTime, Suffocating,
};
use azalea::entity::{
    Attributes, EntityKindComponent, Jumping, LocalEntity, LookDirection, OnClimbable, Physics,
    Position, view_vector,
};
use azalea::packet::game::SendGamePacketEvent;
use azalea::physics::PhysicsSystems;
use azalea::physics::client_movement::ClientMovementState;
use azalea::physics::collision::entity_collisions::{
    AabbQuery, CollidableEntityQuery, VehicleCollidableQuery, VehicleCollisions,
};
use azalea::physics::collision::{MoveCtx, MoverType, block_speed_factor_for, move_colliding};
use azalea::physics::fluids::{FluidPush, update_fluid_interaction};
use azalea::physics::travel::no_collision;
use azalea::physics::{
    EntityMovement, InsideKind, apply_effects_from_blocks_for, block_jump_factor,
};
use azalea::world::{WorldName, Worlds};
use azalea_core::entity_id::MinecraftEntityId;
use azalea_core::math;
use azalea_core::position::Vec3;
use azalea_core::tick::GameTick;
use azalea_inventory::components::EquipmentSlot;
use azalea_protocol::packets::game::s_move_vehicle::ServerboundMoveVehicle;
use azalea_protocol::packets::game::s_paddle_boat::ServerboundPaddleBoat;
use azalea_registry::builtin::{Attribute, ItemKind};

use boat::Boat;
pub(crate) use boat::{Keys, hull_under_water};
use mount::{Gear, Jump, Rider, SeatPose};
pub(crate) use mount::{Hull, Mount};

use crate::entities::feed::stand_anim_step;
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
    if let Some(d) = RIDING
        .lock()
        .unwrap()
        .driven
        .as_mut()
        .filter(|d| d.id == id)
    {
        d.pos = pos;
        d.y_rot = y_rot;
        d.x_rot = x_rot;
    }
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

const PLAYER_VEHICLE_ATTACHMENT: f64 = 0.6;

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
                    .before(azalea::physics::travel::travel),
                ride_post_travel
                    .after(PhysicsSystems)
                    .before(azalea::movement::send_position),
            ),
        );
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
);

fn camel_pose(last_change: Option<&LastPoseChangeTick>) -> (bool, i64) {
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
        riding.stop_driving();
        return;
    };
    let Ok((rider, rider_look, id_index, movement, inventory, jumping, sprinting)) = local.single()
    else {
        return;
    };
    let Some(vehicle) = id_index.get_by_minecraft_entity(MinecraftEntityId(vehicle_id)) else {
        crate::client::tracking::LOCAL_VEHICLE_ID.store(-1, Ordering::Relaxed);
        riding.stop_driving();
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
        (
            standing,
            camel_dash,
            nautilus_dash,
            pose_change,
            pig_boost,
            strider_boost,
            suffocating,
            still,
        ),
    )) = mounts.get_mut(vehicle)
    else {
        riding.stop_driving();
        return;
    };

    let kind = **kind;
    let Some(mount) = Mount::of(kind) else {
        riding.stop_driving();
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
        riding.stop_driving();
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
        riding.driven = Some(Driven {
            id: vehicle_id,
            mount,
            pos: **position,
            y_rot: direction.y_rot(),
            x_rot: direction.x_rot(),
            on_ground: physics.on_ground(),
            dash_cooldown: 0,
            dash_synced: dashing.unwrap_or(false),
            steering: Steering {
                total: boost_total.unwrap_or(0),
                ..Steering::default()
            },
            jump: None,
            boat: Boat::default(),
        });
    }
    let Some(driven) = riding.driven.as_mut() else {
        return;
    };
    **position = driven.pos;
    *direction = LookDirection::new(driven.y_rot, driven.x_rot);
    physics.set_on_ground(driven.on_ground);
    physics.bounding_box = dimensions.make_bounding_box(driven.pos);
    let start = driven.pos;

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
    update_fluid_interaction(&mut physics, &world, world_name, push);

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
        attributes.movement_speed.base = speed;
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
        active_effects: None,
        powder_snow_walkable: false,
        stands_on_lava: mount == Mount::Strider,
        fall_flying: false,
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
            let physics_now: &Physics = ctx.physics;
            boat.float(&world, &hull, &mut pos, &mut velocity, |target| {
                no_collision(
                    &world,
                    Some(entity),
                    &aabb_query,
                    &collidable_entity_query,
                    physics_now,
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
            boat.moved(velocity.y);
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

    commands.trigger(SendGamePacketEvent::new(
        rider,
        ServerboundMoveVehicle {
            pos: driven.pos,
            look_direction: *direction,
            on_ground: driven.on_ground,
        },
    ));
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
    let jump_power = |amount: f32| {
        jump_strength * amount as f64 * block_jump_factor(world, Position::new(position)) as f64
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
    mut rider: Query<(&mut Position, &mut Physics, &EntityIdIndex), With<LocalEntity>>,
    mounts: Query<
        (
            &Position,
            &Physics,
            &LookDirection,
            &EntityKindComponent,
            Option<&AbstractHorseStanding>,
            Option<&LastPoseChangeTick>,
        ),
        Without<LocalEntity>,
    >,
) {
    let mut riding = RIDING.lock().unwrap();
    riding.turn = None;
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
    let Ok((mount_position, mount_physics, mount_direction, kind, standing, pose_change)) =
        mounts.get(mount)
    else {
        return;
    };

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
    let seat_pos =
        **mount_position + mount::passenger_attachment(**kind, mount_yaw, index, riders, pose);
    physics.velocity = Vec3::ZERO;
    **position = Vec3 {
        x: seat_pos.x,
        y: seat_pos.y - PLAYER_VEHICLE_ATTACHMENT,
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
                riding.driven = Some(Driven {
                    id: 42,
                    mount: Mount::Horse,
                    pos: Vec3::new(1.0, 2.0, 3.0),
                    y_rot: 0.0,
                    x_rot: 0.0,
                    on_ground: true,
                    dash_cooldown: 0,
                    dash_synced: false,
                    steering: Steering::default(),
                    jump: None,
                    boat: Boat::default(),
                });
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
            }
            assert_eq!(
                crate::client::tracking::LOCAL_VEHICLE_ID.load(Ordering::Relaxed),
                -1,
                "{blocking}"
            );
        }
    }
}
