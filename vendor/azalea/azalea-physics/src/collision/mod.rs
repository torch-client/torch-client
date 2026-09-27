mod blocks;
mod discrete_voxel_shape;
pub mod entity_collisions;
mod mergers;
mod shape;
mod shape_offset;
pub mod sturdy;
pub mod world_collisions;

use std::{ops::Add, sync::LazyLock};

use azalea_block::{BlockState, BlockTrait, fluid_state::FluidState};
use azalea_core::{
    aabb::Aabb,
    direction::Axis,
    math::{self, EPSILON},
    position::{BlockPos, Vec3},
};
use azalea_entity::{
    ActiveEffects, Attributes, Jumping, LookDirection, OnClimbable, Physics, PlayerAbilities, Pose,
    Position, metadata::Sprinting,
};
use azalea_registry::builtin::BlockKind;
use azalea_world::{ChunkStorage, World};
use bevy_ecs::{entity::Entity, world::Mut};
pub use blocks::BlockWithShape;
pub use discrete_voxel_shape::*;
use entity_collisions::{CollidableEntityQuery, get_entity_collisions};
pub use shape::*;
pub use sturdy::{is_face_sturdy, is_solid};
use tracing::warn;

use self::world_collisions::{PowderSnowCollision, get_block_collisions_for};
use crate::{
    block_speed_factor, client_movement::ClientMovementState,
    collision::entity_collisions::AabbQuery, get_block_pos_below_that_affects_movement,
    travel::no_collision,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoverType {
    Own,
    Player,
    Piston,
    ShulkerBox,
    Shulker,
}

impl MoveCtx<'_, '_, '_, '_> {
    fn powder_snow(&self) -> PowderSnowCollision {
        PowderSnowCollision {
            walkable: self.powder_snow_walkable,
            descending: self.physics_state.is_some_and(|state| state.trying_to_crouch),
            entity_bottom: self.position.y,
            fall_distance: self.physics.fall_distance,
        }
    }
}

fn collide(ctx: &MoveCtx, movement: Vec3) -> Vec3 {
    let powder_snow = ctx.powder_snow();
    let entity_bounding_box = ctx.physics.bounding_box;
    let entity_collisions = get_entity_collisions(
        ctx.world,
        &entity_bounding_box.expand_towards(movement),
        Some(ctx.source_entity),
        ctx.aabb_query,
        ctx.collidable_entity_query,
    );
    let world = ctx.world;
    let collided_delta = if movement.length_squared() == 0.0 {
        movement
    } else {
        collide_bounding_box(
            movement,
            &entity_bounding_box,
            world,
            &entity_collisions,
            powder_snow,
        )
    };

    let x_collision = movement.x != collided_delta.x;
    let y_collision = movement.y != collided_delta.y;
    let z_collision = movement.z != collided_delta.z;

    let on_ground = ctx.physics.on_ground() || y_collision && movement.y < 0.;

    let max_up_step = 0.6;
    if max_up_step > 0. && on_ground && (x_collision || z_collision) {
        let mut step_to_delta = collide_bounding_box(
            movement.with_y(max_up_step),
            &entity_bounding_box,
            world,
            &entity_collisions,
            powder_snow,
        );
        let directly_up_delta = collide_bounding_box(
            Vec3::ZERO.with_y(max_up_step),
            &entity_bounding_box.expand_towards(Vec3::new(movement.x, 0., movement.z)),
            world,
            &entity_collisions,
            powder_snow,
        );
        if directly_up_delta.y < max_up_step {
            let target_movement = collide_bounding_box(
                movement.with_y(0.),
                &entity_bounding_box.move_relative(directly_up_delta),
                world,
                &entity_collisions,
                powder_snow,
            )
            .add(directly_up_delta);
            if target_movement.horizontal_distance_squared()
                > step_to_delta.horizontal_distance_squared()
            {
                step_to_delta = target_movement;
            }
        }

        if step_to_delta.horizontal_distance_squared()
            > collided_delta.horizontal_distance_squared()
        {
            return step_to_delta.add(collide_bounding_box(
                Vec3::ZERO.with_y(-step_to_delta.y + movement.y),
                &entity_bounding_box.move_relative(step_to_delta),
                world,
                &entity_collisions,
                powder_snow,
            ));
        }
    }

    collided_delta
}

pub struct MoveCtx<'world, 'state, 'a, 'b> {
    pub mover_type: MoverType,
    pub world: &'a World,
    pub position: Mut<'a, Position>,
    pub physics: &'a mut Physics,
    pub source_entity: Entity,
    pub aabb_query: &'a AabbQuery<'world, 'state, 'b>,
    pub collidable_entity_query: &'a CollidableEntityQuery<'world, 'state>,
    pub physics_state: Option<&'a ClientMovementState>,
    pub attributes: &'a Attributes,
    pub abilities: Option<&'a PlayerAbilities>,
    pub active_effects: Option<&'a ActiveEffects>,
    pub powder_snow_walkable: bool,
    pub fall_flying: bool,

    pub direction: LookDirection,
    pub sprinting: Sprinting,
    pub on_climbable: OnClimbable,
    pub pose: Option<Pose>,
    pub jumping: Jumping,

    pub no_physics: bool,
}

pub fn move_colliding(ctx: &mut MoveCtx, mut movement: Vec3) {

    if ctx.no_physics {
        let position = &mut ctx.position;
        ***position = Vec3 {
            x: position.x + movement.x,
            y: position.y + movement.y,
            z: position.z + movement.z,
        };
        ctx.physics.horizontal_collision = false;
        ctx.physics.vertical_collision = false;
        ctx.physics.set_on_ground(false);
        return;
    }

    let stuck = ctx.physics.stuck_speed_multiplier;
    if stuck.length_squared() > 1.0e-7 {
        if ctx.mover_type != MoverType::Piston {
            movement = Vec3 {
                x: movement.x * stuck.x,
                y: movement.y * stuck.y,
                z: movement.z * stuck.z,
            };
        }
        ctx.physics.stuck_speed_multiplier = Vec3::ZERO;
        ctx.physics.velocity = Vec3::ZERO;
    }

    movement = maybe_back_off_from_edge(ctx, movement);
    let collide_result = collide(ctx, movement);

    let move_distance_sqr = collide_result.length_squared();

    let position = &mut ctx.position;
    let physics = &mut *ctx.physics;
    let world = ctx.world;

    if move_distance_sqr > EPSILON || movement.length_squared() - move_distance_sqr < EPSILON {

        let new_pos = {
            Vec3 {
                x: position.x + collide_result.x,
                y: position.y + collide_result.y,
                z: position.z + collide_result.z,
            }
        };

        if new_pos != ***position {
            ***position = new_pos;
        }
    }

    let x_collision = !math::equal(movement.x, collide_result.x);
    let z_collision = !math::equal(movement.z, collide_result.z);
    let horizontal_collision = x_collision || z_collision;
    physics.horizontal_collision = horizontal_collision;

    let vertical_collision = movement.y != collide_result.y;
    physics.vertical_collision = vertical_collision;
    let on_ground = vertical_collision && movement.y < 0.;
    physics.set_on_ground(on_ground);

    physics.minor_horizontal_collision =
        horizontal_collision && is_horizontal_collision_minor(physics, ctx.direction, collide_result);

    let block_pos_below = azalea_entity::on_pos_legacy(&world.chunks, **position);
    let block_state_below = world.get_block_state(block_pos_below).unwrap_or_default();

    check_fall_damage(
        physics,
        collide_result.y,
        block_state_below,
        block_pos_below,
    );

    if horizontal_collision {
        let delta_movement = &physics.velocity;
        physics.velocity = Vec3 {
            x: if x_collision { 0. } else { delta_movement.x },
            y: delta_movement.y,
            z: if z_collision { 0. } else { delta_movement.z },
        }
    }

    if vertical_collision {
        let bounce_multiplier = Box::<dyn BlockTrait>::from(block_state_below)
            .behavior()
            .bounce_multiplier;
        match bounce_multiplier {
            Some(bounce_multiplier)
                if physics.velocity.y < 0. && ctx.pose != Some(Pose::Crouching) =>
            {
                physics.velocity.y = -physics.velocity.y * bounce_multiplier as f64;
            }
            _ => physics.velocity.y = 0.,
        }
    }

    if on_ground {
        if BlockKind::from(block_state_below) == BlockKind::SlimeBlock
            && physics.velocity.y.abs() < 0.1
            && ctx.pose != Some(Pose::Crouching)
        {
            let scale = 0.4 + physics.velocity.y.abs() * 0.2;
            physics.velocity.x *= scale;
            physics.velocity.z *= scale;
        }
    }

    let speed_factor = block_speed_factor_for(world, ***position, ctx.attributes);
    if speed_factor != 1.0 {
        physics.velocity.x *= f64::from(speed_factor);
        physics.velocity.z *= f64::from(speed_factor);
    }

}

fn is_horizontal_collision_minor(
    physics: &Physics,
    direction: LookDirection,
    movement: Vec3,
) -> bool {
    const MIN_LENGTH_SQR: f64 = 9.999999747378752e-6;
    const MINOR_ANGLE: f64 = 0.13962633907794952;

    let y_rot = direction.y_rot() * 0.017453292;
    let (sin, cos) = (f64::from(math::sin(y_rot)), f64::from(math::cos(y_rot)));
    let wanted_x = f64::from(physics.x_acceleration) * cos - f64::from(physics.z_acceleration) * sin;
    let wanted_z = f64::from(physics.z_acceleration) * cos + f64::from(physics.x_acceleration) * sin;

    let wanted_sqr = wanted_x * wanted_x + wanted_z * wanted_z;
    let moved_sqr = movement.x * movement.x + movement.z * movement.z;
    if wanted_sqr < MIN_LENGTH_SQR || moved_sqr < MIN_LENGTH_SQR {
        return false;
    }

    let dot = wanted_x * movement.x + wanted_z * movement.z;
    f64::acos(dot / f64::sqrt(wanted_sqr * moved_sqr)) < MINOR_ANGLE
}

fn block_speed_factor_for(world: &World, position: Vec3, attributes: &Attributes) -> f32 {
    let here = BlockKind::from(world.get_block_state(BlockPos::from(position)).unwrap_or_default());
    let factor = block_speed_factor(here);

    let factor = if factor == 1.0 && !matches!(here, BlockKind::Water | BlockKind::BubbleColumn) {
        let below = get_block_pos_below_that_affects_movement(Position::new(position));
        block_speed_factor(BlockKind::from(
            world.get_block_state(below).unwrap_or_default(),
        ))
    } else {
        factor
    };

    if factor == 1.0 {
        return 1.0;
    }
    let efficiency = attributes.movement_efficiency.calculate() as f32;
    factor + efficiency * (1.0 - factor)
}

fn check_fall_damage(
    physics: &mut Physics,
    delta_y: f64,
    _block_state_below: BlockState,
    _block_pos_below: BlockPos,
) {
    if !physics.is_in_water() && delta_y < 0. {
        physics.fall_distance -= delta_y as f32 as f64;
    }

    if physics.on_ground() {

        physics.fall_distance = 0.;
    }
}

fn maybe_back_off_from_edge(move_ctx: &mut MoveCtx, mut movement: Vec3) -> Vec3 {
    let is_staying_on_ground_surface = move_ctx.physics_state.is_some_and(|s| s.trying_to_crouch);
    let max_up_step = get_max_up_step(move_ctx.attributes);

    let fall_ctx = CanFallAtLeastCtx {
        physics: move_ctx.physics,
        world: move_ctx.world,
        source_entity: move_ctx.source_entity,
        aabb_query: move_ctx.aabb_query,
        collidable_entity_query: move_ctx.collidable_entity_query,
        powder_snow: move_ctx.powder_snow(),
    };

    let Some(abilities) = move_ctx.abilities else {
        return movement;
    };

    let is_backing_off = !abilities.flying
        && movement.y <= 0.
        && matches!(move_ctx.mover_type, MoverType::Own | MoverType::Player)
        && is_staying_on_ground_surface
        && is_above_ground(&fall_ctx, max_up_step);
    if !is_backing_off {
        return movement;
    }

    let min_movement = 0.05;
    let min_movement_x = movement.x.signum() * min_movement;
    let min_movement_z = movement.z.signum() * min_movement;

    while movement.x != 0. && can_fall_at_least(&fall_ctx, movement.x, 0., max_up_step as f64) {
        if movement.x.abs() <= min_movement {
            movement.x = 0.;
            break;
        }

        movement.x -= min_movement_x
    }
    while movement.z != 0. && can_fall_at_least(&fall_ctx, 0., movement.z, max_up_step as f64) {
        if movement.z.abs() <= min_movement {
            movement.z = 0.;
            break;
        }

        movement.z -= min_movement_z
    }
    while movement.x != 0.0
        && movement.z != 0.0
        && can_fall_at_least(&fall_ctx, movement.x, movement.z, max_up_step as f64)
    {
        if movement.x.abs() <= min_movement {
            movement.x = 0.;
        } else {
            movement.x -= min_movement_x;
        }
        if movement.z.abs() <= min_movement {
            movement.z = 0.;
        } else {
            movement.z -= min_movement_z;
        }
    }

    movement
}

fn get_max_up_step(attributes: &Attributes) -> f32 {
    attributes.step_height.calculate() as f32
}

fn is_above_ground(ctx: &CanFallAtLeastCtx, max_up_step: f32) -> bool {
    ctx.physics.on_ground()
        && ctx.physics.fall_distance < max_up_step as f64
        && !can_fall_at_least(ctx, 0., 0., max_up_step as f64 - ctx.physics.fall_distance)
}

pub struct CanFallAtLeastCtx<'world, 'state, 'a, 'b> {
    physics: &'a Physics,
    world: &'a World,
    source_entity: Entity,
    aabb_query: &'a AabbQuery<'world, 'state, 'b>,
    collidable_entity_query: &'a CollidableEntityQuery<'world, 'state>,
    powder_snow: PowderSnowCollision,
}

fn can_fall_at_least(
    ctx: &CanFallAtLeastCtx,
    delta_x: f64,
    delta_z: f64,
    max_up_step: f64,
) -> bool {
    let aabb = ctx.physics.bounding_box;
    let aabb = Aabb {
        min: Vec3 {
            x: aabb.min.x + EPSILON + delta_x,
            y: aabb.min.y - max_up_step - EPSILON,
            z: aabb.min.z + EPSILON + delta_z,
        },
        max: Vec3 {
            x: aabb.max.x - EPSILON + delta_x,
            y: aabb.min.y,
            z: aabb.max.z - EPSILON + delta_z,
        },
    };
    no_collision(
        ctx.world,
        Some(ctx.source_entity),
        ctx.aabb_query,
        ctx.collidable_entity_query,
        ctx.physics,
        &aabb,
        false,
        ctx.powder_snow,
    )
}

fn collide_bounding_box(
    movement: Vec3,
    entity_bounding_box: &Aabb,
    world: &World,
    entity_collisions: &[VoxelShape],
    powder_snow: PowderSnowCollision,
) -> Vec3 {
    let mut collision_boxes: Vec<VoxelShape> = Vec::with_capacity(entity_collisions.len() + 1);

    if !entity_collisions.is_empty() {
        collision_boxes.extend_from_slice(entity_collisions);
    }

    let block_collisions = get_block_collisions_for(
        world,
        &entity_bounding_box.expand_towards(movement),
        powder_snow,
    );
    collision_boxes.extend(block_collisions);
    collide_with_shapes(movement, *entity_bounding_box, &collision_boxes)
}

fn collide_with_shapes(
    mut movement: Vec3,
    mut entity_box: Aabb,
    collision_boxes: &[VoxelShape],
) -> Vec3 {
    if collision_boxes.is_empty() {
        return movement;
    }

    if movement.y != 0. {
        movement.y = Shapes::collide(Axis::Y, &entity_box, collision_boxes, movement.y);
        if movement.y != 0. {
            entity_box = entity_box.move_relative(Vec3::new(0., movement.y, 0.));
        }
    }

    let more_z_movement = movement.x.abs() < movement.z.abs();

    if more_z_movement && movement.z != 0. {
        movement.z = Shapes::collide(Axis::Z, &entity_box, collision_boxes, movement.z);
        if movement.z != 0. {
            entity_box = entity_box.move_relative(Vec3::new(0., 0., movement.z));
        }
    }

    if movement.x != 0. {
        movement.x = Shapes::collide(Axis::X, &entity_box, collision_boxes, movement.x);
        if movement.x != 0. {
            entity_box = entity_box.move_relative(Vec3::new(movement.x, 0., 0.));
        }
    }

    if !more_z_movement && movement.z != 0. {
        movement.z = Shapes::collide(Axis::Z, &entity_box, collision_boxes, movement.z);
    }

    movement
}

pub fn fluid_shape(fluid: &FluidState, world: &ChunkStorage, pos: BlockPos) -> &'static VoxelShape {
    if fluid.amount == 9 {
        let fluid_state_above = world.get_fluid_state(pos.up(1)).unwrap_or_default();
        if fluid_state_above.kind == fluid.kind {
            return &BLOCK_SHAPE;
        }
    }
    if fluid.amount > 9 {
        warn!("Tried to calculate shape for fluid with height > 9: {fluid:?} at {pos}");
        return &EMPTY_SHAPE;
    }

    static FLUID_SHAPES: LazyLock<[VoxelShape; 10]> = LazyLock::new(|| {
        [
            calculate_shape_for_fluid(0),
            calculate_shape_for_fluid(1),
            calculate_shape_for_fluid(2),
            calculate_shape_for_fluid(3),
            calculate_shape_for_fluid(4),
            calculate_shape_for_fluid(5),
            calculate_shape_for_fluid(6),
            calculate_shape_for_fluid(7),
            calculate_shape_for_fluid(8),
            calculate_shape_for_fluid(9),
        ]
    });

    &FLUID_SHAPES[fluid.amount as usize]
}
fn calculate_shape_for_fluid(amount: u8) -> VoxelShape {
    box_shape(0.0, 0.0, 0.0, 1.0, (f32::from(amount) / 9.0) as f64, 1.0)
}

pub fn legacy_blocks_motion(block: BlockState) -> bool {
    if block == BlockState::AIR {
        return false;
    }

    let registry_block = BlockKind::from(block);
    legacy_calculate_solid(block)
        && registry_block != BlockKind::Cobweb
        && registry_block != BlockKind::BambooSapling
}

pub fn legacy_calculate_solid(block: BlockState) -> bool {
    let block_trait = Box::<dyn BlockTrait>::from(block);
    if let Some(solid) = block_trait.behavior().force_solid {
        return solid;
    }

    let shape = block.base_collision_shape();
    if shape.is_empty() {
        return false;
    }
    let bounds = shape.bounds();
    bounds.size() >= 0.7291666666666666 || bounds.get_size(Axis::Y) >= 1.0
}
