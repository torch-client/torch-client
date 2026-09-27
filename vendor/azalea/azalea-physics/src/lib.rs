#![feature(trait_alias)]

pub mod client_movement;
pub mod clip;
pub mod collision;
pub mod fluids;
mod inside_block;
pub mod travel;

use std::collections::HashSet;

use azalea_block::{BlockState, BlockTrait};
use azalea_core::{
    math,
    position::{BlockPos, Vec3},
    tick::GameTick,
};
use azalea_entity::{
    ActiveEffects, Attributes, EntityKindComponent, HasClientLoaded, Jumping, LocalEntity,
    LookDirection, OnClimbable, Physics, PlayerAbilities, Pose, Position,
    dimensions::EntityDimensions, metadata::Sprinting, move_relative,
};
use azalea_registry::builtin::{BlockKind, EntityKind, MobEffect};
use azalea_world::{World, WorldName, Worlds};
use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use clip::box_traverse_blocks;
use collision::{BLOCK_SHAPE, VoxelShape, move_colliding};

use crate::collision::{MoveCtx, entity_collisions::update_last_bounding_box};
use crate::inside_block::{InsideBlockCtx, handle_entity_inside_block};

#[derive(Clone, Debug, Eq, Hash, PartialEq, SystemSet)]
pub struct PhysicsSystems;

pub struct PhysicsPlugin;
impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            GameTick,
            (
                fluids::update_in_water_state_and_do_fluid_pushing,
                update_old_position,
                fluids::update_swimming,
                ai_step,
                travel::travel,
                apply_effects_from_blocks,
            )
                .chain()
                .in_set(PhysicsSystems)
                .after(azalea_entity::update_in_loaded_chunk),
        )
        .add_systems(
            Update,
            update_last_bounding_box.after(azalea_entity::update_bounding_box),
        );
    }
}

#[allow(clippy::type_complexity)]
pub fn ai_step(
    mut query: Query<
        (
            &mut Physics,
            Option<&Jumping>,
            &Position,
            &LookDirection,
            &Sprinting,
            &ActiveEffects,
            &WorldName,
            &EntityKindComponent,
        ),
        (With<LocalEntity>, With<HasClientLoaded>),
    >,
    worlds: Res<Worlds>,
) {
    for (
        mut physics,
        jumping,
        position,
        look_direction,
        sprinting,
        active_effects,
        world_name,
        entity_kind,
    ) in &mut query
    {
        let is_player = **entity_kind == EntityKind::Player;

        if physics.no_jump_delay > 0 {
            physics.no_jump_delay -= 1;
        }

        if is_player {
            if physics.velocity.horizontal_distance_squared() < 9.0e-6 {
                physics.velocity.x = 0.;
                physics.velocity.z = 0.;
            }
        } else {
            if physics.velocity.x.abs() < 0.003 {
                physics.velocity.x = 0.;
            }
            if physics.velocity.z.abs() < 0.003 {
                physics.velocity.z = 0.;
            }
        }

        if physics.velocity.y.abs() < 0.003 {
            physics.velocity.y = 0.;
        }

        if is_player {
        } else {
            physics.x_acceleration *= 0.98;
            physics.z_acceleration *= 0.98;
        }

        if jumping == Some(&Jumping(true)) {
            let fluid_height = if physics.is_in_lava() {
                physics.lava_fluid_height
            } else if physics.is_in_water() {
                physics.water_fluid_height
            } else {
                0.
            };

            let in_water = physics.is_in_water() && fluid_height > 0.;
            let fluid_jump_threshold = travel::fluid_jump_threshold();

            if !in_water || physics.on_ground() && fluid_height <= fluid_jump_threshold {
                if !physics.is_in_lava()
                    || physics.on_ground() && fluid_height <= fluid_jump_threshold
                {
                    if (physics.on_ground() || in_water && fluid_height <= fluid_jump_threshold)
                        && physics.no_jump_delay == 0
                    {
                        jump_from_ground(
                            &mut physics,
                            *position,
                            *look_direction,
                            *sprinting,
                            world_name,
                            &worlds,
                            active_effects,
                        );
                        physics.no_jump_delay = 10;
                    }
                } else {
                    jump_in_liquid(&mut physics);
                }
            } else {
                jump_in_liquid(&mut physics);
            }
        } else {
            physics.no_jump_delay = 0;
        }

    }
}

pub const fn block_speed_factor(block: BlockKind) -> f32 {
    match block {
        BlockKind::SoulSand | BlockKind::HoneyBlock => 0.4,
        _ => 1.0,
    }
}

fn jump_in_liquid(physics: &mut Physics) {
    physics.velocity.y += 0.04;
}

#[allow(clippy::type_complexity)]
pub fn apply_effects_from_blocks(
    mut query: Query<
        (
            &mut Physics,
            &Position,
            &EntityDimensions,
            &WorldName,
            &ActiveEffects,
            Option<&PlayerAbilities>,
        ),
        (With<LocalEntity>, With<HasClientLoaded>),
    >,
    worlds: Res<Worlds>,
) {
    for (mut physics, position, dimensions, world_name, active_effects, abilities) in &mut query {
        let Some(world_lock) = worlds.get(world_name) else {
            continue;
        };
        let world = world_lock.read();

        let movement_this_tick = [EntityMovement {
            from: physics.old_position,
            to: **position,
        }];

        check_inside_blocks(
            &mut physics,
            dimensions,
            &world,
            &movement_this_tick,
            &InsideBlockCtx {
                active_effects,
                abilities,
                position: **position,
                width: dimensions.width,
            },
        );
    }
}

fn check_inside_blocks(
    physics: &mut Physics,
    dimensions: &EntityDimensions,
    world: &World,
    movements: &[EntityMovement],
    inside_ctx: &InsideBlockCtx,
) -> Vec<BlockState> {
    let mut blocks_inside = Vec::new();
    let mut visited_blocks = HashSet::<BlockState>::new();

    for movement in movements {
        let bounding_box_at_target = dimensions
            .make_bounding_box(movement.to)
            .deflate_all(1.0E-5);

        for traversed_block in
            box_traverse_blocks(movement.from, movement.to, &bounding_box_at_target)
        {

            let traversed_block_state = world.get_block_state(traversed_block).unwrap_or_default();
            if traversed_block_state.is_air() {
                continue;
            }
            if !visited_blocks.insert(traversed_block_state) {
                continue;
            }

            let entity_inside_collision_shape = &*BLOCK_SHAPE;

            if entity_inside_collision_shape != &*BLOCK_SHAPE
                && !collided_with_shape_moving_from(
                    movement.from,
                    movement.to,
                    traversed_block,
                    entity_inside_collision_shape,
                    dimensions,
                )
            {
                continue;
            }

            handle_entity_inside_block(
                world,
                traversed_block_state,
                traversed_block,
                physics,
                inside_ctx,
            );

            blocks_inside.push(traversed_block_state);
        }
    }

    blocks_inside
}

fn collided_with_shape_moving_from(
    from: Vec3,
    to: Vec3,
    traversed_block: BlockPos,
    entity_inside_collision_shape: &VoxelShape,
    dimensions: &EntityDimensions,
) -> bool {
    let bounding_box_from = dimensions.make_bounding_box(from);
    let delta = to - from;
    bounding_box_from.collided_along_vector(
        delta,
        &entity_inside_collision_shape
            .move_relative(traversed_block.to_vec3_floored())
            .to_aabbs(),
    )
}

pub struct EntityMovement {
    pub from: Vec3,
    pub to: Vec3,
}

pub fn jump_from_ground(
    physics: &mut Physics,
    position: Position,
    look_direction: LookDirection,
    sprinting: Sprinting,
    world_name: &WorldName,
    worlds: &Worlds,
    active_effects: &ActiveEffects,
) {
    let world_lock = worlds
        .get(world_name)
        .expect("All entities should be in a valid world");
    let world = world_lock.read();

    let base_jump = jump_power(&world, position);
    let jump_power = base_jump + jump_boost_power(active_effects);
    if jump_power <= 1.0E-5 {
        return;
    }

    let old_delta_movement = physics.velocity;
    physics.velocity = Vec3 {
        x: old_delta_movement.x,
        y: f64::max(jump_power as f64, old_delta_movement.y),
        z: old_delta_movement.z,
    };
    if *sprinting {
        let y_rot = look_direction.y_rot() * 0.017453292;
        physics.velocity += Vec3 {
            x: (-math::sin(y_rot) * 0.2) as f64,
            y: 0.,
            z: (math::cos(y_rot) * 0.2) as f64,
        };
    }

    physics.has_impulse = true;
}

pub fn update_old_position(mut query: Query<(&mut Physics, &Position)>) {
    for (mut physics, position) in &mut query {
        physics.set_old_pos(*position);
    }
}

pub fn get_block_pos_below_that_affects_movement(position: Position) -> BlockPos {
    BlockPos::new(
        position.x.floor() as i32,
        (position.y - 0.5f64).floor() as i32,
        position.z.floor() as i32,
    )
}

fn handle_relative_friction_and_calculate_movement(ctx: &mut MoveCtx, block_friction: f32) -> Vec3 {
    move_relative(
        ctx.physics,
        ctx.direction,
        get_friction_influenced_speed(ctx.physics, ctx.attributes, block_friction, ctx.sprinting),
        Vec3::new(
            ctx.physics.x_acceleration as f64,
            ctx.physics.y_acceleration as f64,
            ctx.physics.z_acceleration as f64,
        ),
    );

    ctx.physics.velocity = handle_on_climbable(
        ctx.physics.velocity,
        ctx.on_climbable,
        *ctx.position,
        ctx.world,
        ctx.pose,
    );

    move_colliding(ctx, ctx.physics.velocity);

    if ctx.physics.horizontal_collision || *ctx.jumping {
        let block_at_feet: BlockKind = ctx
            .world
            .chunks
            .get_block_state(BlockPos::from(*ctx.position))
            .unwrap_or_default()
            .into();

        if *ctx.on_climbable || block_at_feet == BlockKind::PowderSnow {
            ctx.physics.velocity.y = 0.2;
        }
    }

    ctx.physics.velocity
}

fn handle_on_climbable(
    velocity: Vec3,
    on_climbable: OnClimbable,
    position: Position,
    world: &World,
    pose: Option<Pose>,
) -> Vec3 {
    if !*on_climbable {
        return velocity;
    }

    const CLIMBING_SPEED: f64 = 0.15_f32 as f64;

    let x = f64::clamp(velocity.x, -CLIMBING_SPEED, CLIMBING_SPEED);
    let z = f64::clamp(velocity.z, -CLIMBING_SPEED, CLIMBING_SPEED);
    let mut y = f64::max(velocity.y, -CLIMBING_SPEED);

    if y < 0.0
        && pose == Some(Pose::Crouching)
        && BlockKind::from(
            world
                .chunks
                .get_block_state(position.into())
                .unwrap_or_default(),
        ) != BlockKind::Scaffolding
    {
        y = 0.;
    }

    Vec3 { x, y, z }
}

fn get_friction_influenced_speed(
    physics: &Physics,
    attributes: &Attributes,
    friction: f32,
    sprinting: Sprinting,
) -> f32 {
    if physics.on_ground() {
        let speed = attributes.movement_speed.calculate() as f32;
        speed * (0.21600002f32 / (friction * friction * friction))
    } else {
        if *sprinting { 0.025999999f32 } else { 0.02 }
    }
}

fn block_jump_factor(world: &World, position: Position) -> f32 {
    let block_at_pos = world.chunks.get_block_state(position.into());
    let block_below = world
        .chunks
        .get_block_state(get_block_pos_below_that_affects_movement(position));

    let block_at_pos_jump_factor = if let Some(block) = block_at_pos {
        Box::<dyn BlockTrait>::from(block).behavior().jump_factor
    } else {
        1.
    };
    if block_at_pos_jump_factor != 1. {
        return block_at_pos_jump_factor;
    }

    if let Some(block) = block_below {
        Box::<dyn BlockTrait>::from(block).behavior().jump_factor
    } else {
        1.
    }
}

fn jump_power(world: &World, position: Position) -> f32 {
    0.42 * block_jump_factor(world, position)
}

fn jump_boost_power(active_effects: &ActiveEffects) -> f32 {
    active_effects
        .get_level(MobEffect::JumpBoost)
        .map(|level| 0.1 * (level + 1) as f32)
        .unwrap_or(0.)
}
