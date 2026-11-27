use azalea::app::{App, Plugin, Update};
use azalea::block::BlockState;
use azalea::block_update::{handle_block_update_event, handle_queued_block_changed_acks};
use azalea::ecs::prelude::*;
use azalea::entity::dimensions::EntityDimensions;
use azalea::entity::indexing::EntityIdIndex;
use azalea::entity::inventory::Inventory;
use azalea::entity::metadata::{FallFlying, Sprinting, TicksFrozen};
use azalea::entity::{
    ActiveEffects, Attributes, HasClientLoaded, Jumping, LocalEntity, LookDirection, OnClimbable,
    Physics, PlayerAbilities, Pose, Position,
};
use azalea::interact::BlockStatePredictionHandler;
use azalea::physics::client_movement::ClientMovementState;
use azalea::physics::collision::auto_jump::update_auto_jump;
use azalea::physics::collision::entity_collisions::{
    AabbQuery, CollidableEntityQuery, VehicleCollidableQuery,
};
use azalea::physics::collision::{
    BlockWithShape, MoveCtx, MoverType, MovingBlocks, move_colliding,
};
use azalea::physics::travel::wearing_leather_boots;
use azalea::physics::{EntityMovement, PhysicsSystems, apply_effects_from_blocks_to_player};
use azalea::world::{World as Level, WorldBorder, WorldName, Worlds};
use azalea_core::aabb::Aabb;
use azalea_core::direction::{Axis, Direction};
use azalea_core::entity_id::MinecraftEntityId;
use azalea_core::game_type::GameMode;
use azalea_core::position::{BlockPos, Vec3};
use azalea_core::tick::{GameTick, TickRateManager};
use azalea_registry::builtin::BlockKind;

use crate::blockentities::sim::piston::{self, Near, Step, Ticking, begin_tick};

const PUSH_OFFSET: f64 = 0.01;
const MAX_TICK_MOVEMENT: f64 = 0.51;
const MIN_LIMITED_MOVEMENT: f64 = 9.999999747378752e-6;
const STICKY_BOX_TOP: f64 = 1.5000010000000001;

pub struct PistonPushPlugin;

impl Plugin for PistonPushPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                add_moving_blocks,
                note_written_updates
                    .after(handle_block_update_event)
                    .before(handle_queued_block_changed_acks),
                note_handled_acks.after(handle_queued_block_changed_acks),
            ),
        )
        .add_systems(
            GameTick,
            (
                set_moving_blocks.before(PhysicsSystems),
                tick_moving_blocks
                    .after(azalea::movement::send_position)
                    .after(crate::play::flight::flight_anti_kick),
            ),
        );
    }
}

fn note_written_updates(players: Query<&BlockStatePredictionHandler, With<LocalEntity>>) {
    let handler = players.single().ok();
    piston::on_updates_written(|pos| handler.is_some_and(|h| h.is_predicting(block_pos(pos))));
}

fn note_handled_acks(players: Query<&BlockStatePredictionHandler, With<LocalEntity>>) {
    let handler = players.single().ok();
    piston::on_acks_handled(|pos| handler.is_some_and(|h| h.is_predicting(block_pos(pos))));
}

fn add_moving_blocks(
    players: Query<(Entity, &EntityIdIndex), With<LocalEntity>>,
    holders: Query<(Entity, Has<LocalEntity>), With<MovingBlocks>>,
    mut commands: Commands,
) {
    let Ok((player, id_index)) = players.single() else {
        return;
    };
    let vehicle = vehicle::under_player(id_index);
    for (holder, local) in &holders {
        if !local
            && Some(holder) != vehicle
            && let Ok(mut holder) = commands.get_entity(holder)
        {
            holder.try_remove::<MovingBlocks>();
        }
    }
    for mover in [Some(player), vehicle].into_iter().flatten() {
        if !holders.contains(mover)
            && let Ok(mut mover) = commands.get_entity(mover)
        {
            mover.try_insert(MovingBlocks::default());
        }
    }
}

fn set_moving_blocks(
    mut movers: Query<(
        &mut MovingBlocks,
        &Position,
        &EntityDimensions,
        &Physics,
        Has<LocalEntity>,
    )>,
) {
    let reaches: Vec<Aabb> = movers
        .iter()
        .map(|(_, position, dimensions, physics, local)| {
            let pos = if local {
                **position
            } else {
                vehicle::position(**position)
            };
            piston::reach(&dimensions.make_bounding_box(pos), physics.velocity)
        })
        .collect();
    let mut blocks = piston::moving_blocks(&reaches)
        .unwrap_or_default()
        .into_iter();
    for (mut moving, ..) in &mut movers {
        let blocks = blocks.next().unwrap_or_default();
        if !(blocks.is_empty() && moving.is_empty()) {
            *moving = blocks;
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn tick_moving_blocks(
    mut players: Query<
        (
            Entity,
            &mut Physics,
            &mut Position,
            &LookDirection,
            &EntityDimensions,
            &Attributes,
            &WorldName,
            &Sprinting,
            &Jumping,
            &OnClimbable,
            &Pose,
            &ActiveEffects,
            Option<&GameMode>,
            (
                Option<&ClientMovementState>,
                Option<&PlayerAbilities>,
                Option<&Inventory>,
                Option<&FallFlying>,
                Option<&WorldBorder>,
                Option<&mut TicksFrozen>,
                &EntityIdIndex,
            ),
        ),
        (With<LocalEntity>, With<HasClientLoaded>),
    >,
    mut vehicles: vehicle::Vehicles,
    vehicle_query: VehicleCollidableQuery,
    worlds: Res<Worlds>,
    aabb_query: AabbQuery,
    collidable_entity_query: CollidableEntityQuery,
    rate: Res<TickRateManager>,
) {
    let Some(mut ticking) = begin_tick(rate.runs_normally()) else {
        return;
    };

    let player = players.single_mut().ok();
    let level = player.as_ref().and_then(|row| worlds.get(row.6));
    let pushed_vehicle = {
        let level = level.as_ref().map(|level| level.read());
        let mut body = player.zip(level.as_deref()).and_then(|(row, level)| {
            let (
                entity,
                physics,
                position,
                look,
                dimensions,
                attributes,
                _,
                sprinting,
                jumping,
                on_climbable,
                pose,
                active_effects,
                game_mode,
                (movement_state, abilities, inventory, fall_flying, border, ticks_frozen, id_index),
            ) = row;
            if crate::client::tracking::local_vehicle_id().is_some() {
                return vehicle::body(
                    &mut vehicles,
                    &vehicle_query,
                    &aabb_query,
                    &collidable_entity_query,
                    id_index,
                    border,
                    level,
                );
            }
            if game_mode == Some(&GameMode::Spectator) {
                return None;
            }
            let moved = **position - physics.old_position;
            Some(Pushed {
                level,
                entity,
                physics,
                position,
                look: *look,
                dimensions,
                attributes,
                sprinting: *sprinting,
                jumping: *jumping,
                on_climbable: *on_climbable,
                pose: Some(*pose),
                active_effects,
                movement_state,
                abilities,
                inventory,
                fall_flying: fall_flying.is_some_and(|flying| **flying),
                border,
                ticks_frozen,
                aabb_query: &aabb_query,
                collidable_entity_query: &collidable_entity_query,
                vehicle: None,
                near: Near::default(),
                moved,
                piston_deltas: [0.0; 3],
            })
        });
        if let Some(body) = &mut body {
            body.near = ticking.near(&piston::reach(&body.bounds(), body.physics.velocity));
        }
        for (seq, pos) in ticking.order() {
            let Some(step) = ticking.step(seq, pos) else {
                continue;
            };
            if let Some(body) = &mut body
                && step.can_reach(&body.bounds())
            {
                body.move_collided(&mut ticking, &step);
                body.move_stuck(&mut ticking, &step);
            }
            ticking.store(pos);
        }
        body.as_ref().and_then(vehicle::outcome)
    };
    ticking.finish();
    if let Some(outcome) = pushed_vehicle {
        outcome.report();
    }
}

struct Pushed<'a, 'w, 's, 'b> {
    level: &'a Level,
    entity: Entity,
    physics: Mut<'a, Physics>,
    position: Mut<'a, Position>,
    look: LookDirection,
    dimensions: &'a EntityDimensions,
    attributes: &'a Attributes,
    sprinting: Sprinting,
    jumping: Jumping,
    on_climbable: OnClimbable,
    pose: Option<Pose>,
    active_effects: &'a ActiveEffects,
    movement_state: Option<&'a ClientMovementState>,
    abilities: Option<&'a PlayerAbilities>,
    inventory: Option<&'a Inventory>,
    fall_flying: bool,
    border: Option<&'a WorldBorder>,
    ticks_frozen: Option<Mut<'a, TicksFrozen>>,
    aabb_query: &'a AabbQuery<'w, 's, 'b>,
    collidable_entity_query: &'a CollidableEntityQuery<'w, 's>,
    vehicle: Option<vehicle::Vehicle<'a, 'w, 's>>,
    near: Near,
    moved: Vec3,
    piston_deltas: [f64; 3],
}

impl Pushed<'_, '_, '_, '_> {
    fn bounds(&self) -> Aabb {
        self.dimensions.make_bounding_box(**self.position)
    }

    fn move_collided(&mut self, ticking: &mut Ticking, step: &Step) {
        let movement = step.moving.movement();
        let boxes = ticking.pushing(step);
        let Some(shape) = boxes.iter().copied().reduce(|a, b| a.minmax(&b)) else {
            return;
        };
        let aabb = step.place(&shape);
        let search = movement_area(&aabb, movement, Step::DELTA).minmax(&aabb);
        if !search.intersects_aabb(&self.bounds()) {
            return;
        }
        if BlockKind::from(step.moving.moved) == BlockKind::SlimeBlock {
            let velocity = &mut self.physics.velocity;
            match axis(movement) {
                Axis::X => velocity.x = f64::from(movement.x()),
                Axis::Y => velocity.y = f64::from(movement.y()),
                Axis::Z => velocity.z = f64::from(movement.z()),
            }
        }

        let bounds = self.bounds();
        let mut delta = 0.0_f64;
        for shape_aabb in &boxes {
            let moving = movement_area(&step.place(shape_aabb), movement, Step::DELTA);
            if moving.intersects_aabb(&bounds) {
                delta = delta.max(overlap(&moving, movement, &bounds));
                if delta >= Step::DELTA {
                    break;
                }
            }
        }
        if delta <= 0.0 {
            return;
        }
        let delta = delta.min(Step::DELTA) + PUSH_OFFSET;
        self.move_by_piston(ticking, movement, delta, movement);
        if !step.moving.extending && step.moving.source {
            self.fix_within_base(ticking, block_pos(step.pos), movement);
        }
    }

    fn fix_within_base(&mut self, ticking: &Ticking, pos: BlockPos, direction: Direction) {
        let bounds = self.bounds();
        let base = block_box(pos);
        if !bounds.intersects_aabb(&base) {
            return;
        }
        let opposite = direction.opposite();
        let delta = overlap(&base, opposite, &bounds) + PUSH_OFFSET;
        let intersected = overlap(&base, opposite, &bounds.intersect(&base)) + PUSH_OFFSET;
        if (delta - intersected).abs() < 0.01 {
            let delta = delta.min(Step::DELTA) + PUSH_OFFSET;
            self.move_by_piston(ticking, direction, delta, opposite);
        }
    }

    fn move_stuck(&mut self, ticking: &mut Ticking, step: &Step) {
        if BlockKind::from(step.moving.moved) != BlockKind::HoneyBlock {
            return;
        }
        let movement = step.moving.movement();
        if matches!(movement, Direction::Up | Direction::Down) {
            return;
        }
        let top = ticking
            .moved(step)
            .iter()
            .map(|aabb| aabb.max.y)
            .fold(f64::NEG_INFINITY, f64::max);
        let aabb = step.place(&Aabb {
            min: Vec3::new(0.0, top, 0.0),
            max: Vec3::new(1.0, STICKY_BOX_TOP, 1.0),
        });
        if aabb.intersects_aabb(&self.bounds()) && self.sticks(ticking, &aabb, block_pos(step.pos))
        {
            self.move_by_piston(ticking, movement, Step::DELTA, movement);
        }
    }

    fn sticks(&self, ticking: &Ticking, aabb: &Aabb, pos: BlockPos) -> bool {
        let feet = **self.position;
        self.physics.on_ground()
            && (feet.x >= aabb.min.x
                && feet.x <= aabb.max.x
                && feet.z >= aabb.min.z
                && feet.z <= aabb.max.z
                || self.supporting_block(ticking) == Some(pos))
    }

    fn supporting_block(&self, ticking: &Ticking) -> Option<BlockPos> {
        let bounds = self.bounds();
        let under = Aabb {
            min: Vec3::new(bounds.min.x, bounds.min.y - 1.0e-6, bounds.min.z),
            max: Vec3::new(bounds.max.x, bounds.min.y, bounds.max.z),
        };
        let blocks = ticking.moving_blocks(&self.near, None);
        self.find_supporting_block(&blocks, &under).or_else(|| {
            let back = Vec3::new(-self.moved.x, 0.0, -self.moved.z);
            self.find_supporting_block(&blocks, &under.move_relative(back))
        })
    }

    fn find_supporting_block(&self, blocks: &MovingBlocks, under: &Aabb) -> Option<BlockPos> {
        let feet = **self.position;
        let supports = |pos: BlockPos| {
            if blocks.covers(pos) {
                return blocks
                    .boxes
                    .iter()
                    .any(|(cell, aabb)| *cell == pos && aabb.intersects_aabb(under));
            }
            let state: BlockState = self.level.get_block_state(pos).unwrap_or_default();
            let corner = pos.to_vec3_floored();
            state
                .collision_shape(pos)
                .to_aabbs()
                .iter()
                .any(|aabb| aabb.move_relative(corner).intersects_aabb(under))
        };
        let min = BlockPos::from(under.min);
        let max = BlockPos::from(under.max);
        let mut best: Option<(f64, BlockPos)> = None;
        for y in min.y - 1..=max.y + 1 {
            for z in min.z - 1..=max.z + 1 {
                for x in min.x - 1..=max.x + 1 {
                    let pos = BlockPos::new(x, y, z);
                    let distance = (pos.center() - feet).length_squared();
                    if best.is_none_or(|(d, _)| distance <= d) && supports(pos) {
                        best = Some((distance, pos));
                    }
                }
            }
        }
        best.map(|(_, pos)| pos)
    }

    fn move_by_piston(
        &mut self,
        ticking: &Ticking,
        piston_direction: Direction,
        delta: f64,
        direction: Direction,
    ) {
        let blocks = ticking.moving_blocks(&self.near, Some(piston_direction));
        let from = **self.position;
        let movement = limit_piston_movement(
            &mut self.piston_deltas,
            Vec3::new(
                delta * f64::from(direction.x()),
                delta * f64::from(direction.y()),
                delta * f64::from(direction.z()),
            ),
        );
        self.physics.bounding_box = self.bounds();
        if let Some(vehicle) = &mut self.vehicle
            && movement != Vec3::ZERO
        {
            vehicle.will_move(&self.physics);
        }
        let mut ctx = MoveCtx {
            mover_type: MoverType::Piston,
            world: self.level,
            position: self.position.reborrow(),
            physics: &mut self.physics,
            source_entity: self.entity,
            aabb_query: self.aabb_query,
            collidable_entity_query: self.collidable_entity_query,
            vehicle_collisions: self.vehicle.as_ref().and_then(|v| v.collisions()),
            physics_state: self.movement_state,
            attributes: self.attributes,
            abilities: self.abilities,
            active_effects: Some(self.active_effects),
            powder_snow_walkable: wearing_leather_boots(self.inventory),
            stands_on_lava: self.vehicle.as_ref().is_some_and(|v| v.strider),
            fall_flying: self.fall_flying,
            border: self.border,
            moving: Some(&blocks),
            direction: self.look,
            sprinting: self.sprinting,
            on_climbable: self.on_climbable,
            pose: self.pose,
            jumping: self.jumping,
            no_physics: false,
        };
        if movement != Vec3::ZERO {
            move_colliding(&mut ctx, movement);
        }
        update_auto_jump(&mut ctx, from);
        drop(ctx);

        let to = **self.position;
        if movement != Vec3::ZERO {
            self.moved = to - from;
        }
        self.physics.bounding_box = self.bounds();
        let movement = EntityMovement { from, to };
        if let Some(vehicle) = &self.vehicle {
            vehicle.apply_effects(
                &mut self.physics,
                self.dimensions,
                self.level,
                movement,
                self.active_effects,
                &blocks,
            );
            return;
        }
        apply_effects_from_blocks_to_player(
            &mut self.physics,
            self.dimensions,
            self.level,
            movement,
            self.active_effects,
            self.abilities,
            self.movement_state
                .is_some_and(|state| state.trying_to_crouch),
            self.inventory,
            self.ticks_frozen.as_mut().map(|ticks| ticks.reborrow()),
            Some(&blocks),
        );
    }
}

mod vehicle {
    use azalea::physics::collision::entity_collisions::VehicleCollisions;
    use azalea::physics::{InsideKind, apply_effects_from_blocks_for};

    use super::*;
    use crate::play::riding;

    #[allow(clippy::type_complexity)]
    pub type Vehicles<'w, 's> = Query<
        'w,
        's,
        (
            Entity,
            &'static mut Physics,
            &'static mut Position,
            &'static LookDirection,
            &'static EntityDimensions,
            &'static Attributes,
            &'static OnClimbable,
            &'static ActiveEffects,
        ),
        Without<LocalEntity>,
    >;

    pub struct Vehicle<'a, 'w, 's> {
        id: i32,
        boat: bool,
        pub strider: bool,
        query: &'a VehicleCollidableQuery<'w, 's>,
        passengers: [Entity; 4],
        riders: usize,
        last_yd: Option<f64>,
    }

    pub fn under_player(id_index: &EntityIdIndex) -> Option<Entity> {
        let id = crate::client::tracking::local_vehicle_id()?;
        id_index.get_by_minecraft_entity(MinecraftEntityId(id))
    }

    pub fn position(component: Vec3) -> Vec3 {
        riding::steered().map_or(component, |steered| steered.pos)
    }

    pub fn body<'a, 'w, 's, 'b>(
        vehicles: &'a mut Vehicles,
        query: &'a VehicleCollidableQuery<'w, 's>,
        aabb_query: &'a AabbQuery<'w, 's, 'b>,
        collidable_entity_query: &'a CollidableEntityQuery<'w, 's>,
        id_index: &EntityIdIndex,
        border: Option<&'a WorldBorder>,
        level: &'a Level,
    ) -> Option<Pushed<'a, 'w, 's, 'b>> {
        let steered = riding::steered()?;
        if crate::client::tracking::local_vehicle_id() != Some(steered.id) {
            return None;
        }
        let (entity, physics, position, look, dimensions, attributes, on_climbable, active_effects) =
            vehicles.get_mut(under_player(id_index)?).ok()?;
        let (passengers, riders) = riding::passenger_entities(id_index, steered.id);
        let moved = **position - physics.old_position;
        Some(Pushed {
            level,
            entity,
            physics,
            position,
            look: *look,
            dimensions,
            attributes,
            sprinting: Sprinting(false),
            jumping: Jumping(false),
            on_climbable: *on_climbable,
            pose: None,
            active_effects,
            movement_state: None,
            abilities: None,
            inventory: None,
            fall_flying: false,
            border,
            ticks_frozen: None,
            aabb_query,
            collidable_entity_query,
            vehicle: Some(Vehicle {
                id: steered.id,
                boat: steered.boat,
                strider: steered.strider,
                query,
                passengers,
                riders,
                last_yd: None,
            }),
            near: Near::default(),
            moved,
            piston_deltas: [0.0; 3],
        })
    }

    impl<'w, 's> Vehicle<'_, 'w, 's> {
        pub fn collisions(&self) -> Option<VehicleCollisions<'_, 'w, 's>> {
            self.boat.then(|| VehicleCollisions {
                query: self.query,
                passengers: &self.passengers[..self.riders],
            })
        }

        pub fn will_move(&mut self, physics: &Physics) {
            let stuck = physics.stuck_speed_multiplier.length_squared() > 1.0e-7;
            self.last_yd = Some(if stuck { 0.0 } else { physics.velocity.y });
        }

        pub fn apply_effects(
            &self,
            physics: &mut Physics,
            dimensions: &EntityDimensions,
            level: &Level,
            movement: EntityMovement,
            active_effects: &ActiveEffects,
            blocks: &MovingBlocks,
        ) {
            let kind = if self.boat {
                InsideKind::Boat
            } else {
                InsideKind::Living
            };
            apply_effects_from_blocks_for(
                physics,
                dimensions,
                level,
                movement,
                kind,
                active_effects,
                Some(blocks),
            );
        }
    }

    pub struct Outcome {
        id: i32,
        pos: Vec3,
        on_ground: bool,
        last_yd: f64,
    }

    pub fn outcome(body: &Pushed) -> Option<Outcome> {
        let vehicle = body.vehicle.as_ref()?;
        Some(Outcome {
            id: vehicle.id,
            pos: **body.position,
            on_ground: body.physics.on_ground(),
            last_yd: vehicle.last_yd?,
        })
    }

    impl Outcome {
        pub fn report(self) {
            riding::piston_moved(self.id, self.pos, self.on_ground, self.last_yd);
        }
    }
}

fn limit_piston_movement(deltas: &mut [f64; 3], movement: Vec3) -> Vec3 {
    if movement.length_squared() <= 1.0e-7 {
        return movement;
    }
    let mut restrict = |axis: usize, amount: f64| {
        let total = (amount + deltas[axis]).clamp(-MAX_TICK_MOVEMENT, MAX_TICK_MOVEMENT);
        let amount = total - deltas[axis];
        deltas[axis] = total;
        if amount.abs() <= MIN_LIMITED_MOVEMENT {
            0.0
        } else {
            amount
        }
    };
    if movement.x != 0.0 {
        Vec3::new(restrict(0, movement.x), 0.0, 0.0)
    } else if movement.y != 0.0 {
        Vec3::new(0.0, restrict(1, movement.y), 0.0)
    } else {
        Vec3::new(0.0, 0.0, restrict(2, movement.z))
    }
}

fn movement_area(aabb: &Aabb, direction: Direction, amount: f64) -> Aabb {
    let Aabb { mut min, mut max } = *aabb;
    match direction {
        Direction::West => (min.x, max.x) = (min.x - amount, min.x),
        Direction::East => (min.x, max.x) = (max.x, max.x + amount),
        Direction::Down => (min.y, max.y) = (min.y - amount, min.y),
        Direction::Up => (min.y, max.y) = (max.y, max.y + amount),
        Direction::North => (min.z, max.z) = (min.z - amount, min.z),
        Direction::South => (min.z, max.z) = (max.z, max.z + amount),
    }
    Aabb { min, max }
}

fn overlap(outside_of: &Aabb, movement: Direction, aabb: &Aabb) -> f64 {
    match movement {
        Direction::East => outside_of.max.x - aabb.min.x,
        Direction::West => aabb.max.x - outside_of.min.x,
        Direction::Up => outside_of.max.y - aabb.min.y,
        Direction::Down => aabb.max.y - outside_of.min.y,
        Direction::South => outside_of.max.z - aabb.min.z,
        Direction::North => aabb.max.z - outside_of.min.z,
    }
}

fn axis(direction: Direction) -> Axis {
    match direction {
        Direction::West | Direction::East => Axis::X,
        Direction::Down | Direction::Up => Axis::Y,
        Direction::North | Direction::South => Axis::Z,
    }
}

fn block_pos(pos: [i32; 3]) -> BlockPos {
    BlockPos::new(pos[0], pos[1], pos[2])
}

fn block_box(pos: BlockPos) -> Aabb {
    Aabb {
        min: pos.to_vec3_floored(),
        max: BlockPos::new(pos.x + 1, pos.y + 1, pos.z + 1).to_vec3_floored(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNIT: Aabb = Aabb {
        min: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        max: Vec3 {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        },
    };

    const DIRECTIONS: [Direction; 6] = [
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ];

    #[test]
    fn the_movement_area_is_the_slab_ahead_of_the_face() {
        let east = movement_area(&UNIT, Direction::East, 0.5);
        assert_eq!((east.min.x, east.max.x), (1.0, 1.5));
        assert_eq!((east.min.y, east.max.z), (0.0, 1.0));
        let down = movement_area(&UNIT, Direction::Down, 0.5);
        assert_eq!((down.min.y, down.max.y), (-0.5, 0.0));
        let north = movement_area(&UNIT, Direction::North, 0.5);
        assert_eq!((north.min.z, north.max.z), (-0.5, 0.0));
    }

    #[test]
    fn a_box_is_moved_out_of_the_area_along_the_movement() {
        for direction in DIRECTIONS {
            let area = movement_area(&UNIT, direction, 0.5);
            let step = direction.normal_vec3();
            let ahead = UNIT.move_relative(step * 1.2);
            let delta = overlap(&area, direction, &ahead);
            assert!((delta - 0.3).abs() < 1e-12, "{direction:?} {delta}");
            assert!(!area.intersects_aabb(&ahead.move_relative(step * (delta + PUSH_OFFSET))));
        }
    }

    #[test]
    fn two_pushes_in_a_tick_stop_at_the_limit() {
        let mut deltas = [0.0; 3];
        let first = limit_piston_movement(&mut deltas, Vec3::new(0.3, 0.0, 0.0));
        assert_eq!(first, Vec3::new(0.3, 0.0, 0.0));
        let second = limit_piston_movement(&mut deltas, Vec3::new(0.51, 0.0, 0.0));
        assert!((second.x - 0.21).abs() < 1e-12);
        let third = limit_piston_movement(&mut deltas, Vec3::new(0.51, 0.0, 0.0));
        assert_eq!(third, Vec3::ZERO);
        let back = limit_piston_movement(&mut deltas, Vec3::new(-2.0, 0.0, 0.0));
        assert!((back.x + 1.02).abs() < 1e-12);
        let up = limit_piston_movement(&mut deltas, Vec3::new(0.0, 0.51, 0.0));
        assert_eq!(up, Vec3::new(0.0, 0.51, 0.0));
    }

    #[test]
    fn a_movement_too_small_to_count_passes_unlimited() {
        let mut deltas = [0.51, 0.0, 0.0];
        let tiny = Vec3::new(1.0e-4, 0.0, 0.0);
        assert_eq!(limit_piston_movement(&mut deltas, tiny), tiny);
        assert_eq!(deltas, [0.51, 0.0, 0.0]);
    }
}
