use azalea_block::{
    BlockState,
    fluid_state::{FluidKind, FluidState},
};
use azalea_core::{
    direction::Direction,
    position::{BlockPos, Vec3},
};
use azalea_entity::{HasClientLoaded, LocalEntity, Physics, Position, metadata::{Sprinting, Swimming}};
use azalea_registry::builtin::BlockKind;
use azalea_world::{World, WorldName, Worlds};
use bevy_ecs::prelude::*;

pub const PLAYER_EYE_HEIGHT: f64 = 1.62;

pub const SWIMMING_EYE_HEIGHT: f64 = 0.4;

pub const CROUCHING_EYE_HEIGHT: f64 = 1.27;

pub fn player_eye_height(swimming: bool, crouching: bool) -> f64 {
    if swimming {
        SWIMMING_EYE_HEIGHT
    } else if crouching {
        CROUCHING_EYE_HEIGHT
    } else {
        PLAYER_EYE_HEIGHT
    }
}

use crate::collision::legacy_blocks_motion;

#[allow(clippy::type_complexity)]
pub fn update_in_water_state_and_do_fluid_pushing(
    mut query: Query<
        (&mut Physics, &Position, &WorldName),
        (With<LocalEntity>, With<HasClientLoaded>),
    >,
    worlds: Res<Worlds>,
) {
    for (mut physics, position, world_name) in &mut query {
        let Some(world_lock) = worlds.get(world_name) else {
            continue;
        };
        let world = world_lock.read();

        physics.water_fluid_height = 0.;
        physics.lava_fluid_height = 0.;

        update_in_water_state_and_do_water_current_pushing(&mut physics, &world, *position);

        let is_ultrawarm = world
            .registries
            .dimension_type
            .map
            .get(&**world_name)
            .and_then(|i| i.ultrawarm)
            .unwrap_or_default();
        let lava_push_factor = if is_ultrawarm {
            0.007
        } else {
            0.0023333333333333335
        };

        update_fluid_height_and_do_fluid_pushing(
            &mut physics,
            &world,
            FluidKind::Lava,
            lava_push_factor,
        );
    }
}
fn update_in_water_state_and_do_water_current_pushing(
    physics: &mut Physics,
    world: &World,
    _position: Position,
) {

    if update_fluid_height_and_do_fluid_pushing(physics, world, FluidKind::Water, 0.014) {

        physics.reset_fall_distance();
        physics.was_touching_water = true;
        physics.clear_fire();
    } else {
        physics.was_touching_water = false;
    }
}

fn update_fluid_height_and_do_fluid_pushing(
    physics: &mut Physics,
    world: &World,
    checking_fluid: FluidKind,
    fluid_push_factor: f64,
) -> bool {

    let checking_liquids_aabb = physics.bounding_box.deflate_all(0.001);

    let min_x = checking_liquids_aabb.min.x.floor() as i32;
    let min_y = checking_liquids_aabb.min.y.floor() as i32;
    let min_z = checking_liquids_aabb.min.z.floor() as i32;

    let max_x = checking_liquids_aabb.max.x.ceil() as i32;
    let max_y = checking_liquids_aabb.max.y.ceil() as i32;
    let max_z = checking_liquids_aabb.max.z.ceil() as i32;

    let mut min_height_touching = 0.;
    let is_entity_pushable_by_fluid = true;
    let mut touching_fluid = false;
    let mut additional_player_delta = Vec3::ZERO;
    let mut num_fluids_being_touched = 0;

    for cur_x in min_x..max_x {
        for cur_y in min_y..max_y {
            for cur_z in min_z..max_z {
                let cur_pos = BlockPos::new(cur_x, cur_y, cur_z);
                let Some(fluid_at_cur_pos) = world.get_fluid_state(cur_pos) else {
                    continue;
                };
                if fluid_at_cur_pos.kind != checking_fluid {
                    continue;
                }
                let fluid_max_y = (cur_y as f32 + fluid_at_cur_pos.height()) as f64;
                if fluid_max_y < checking_liquids_aabb.min.y {
                    continue;
                }
                touching_fluid = true;
                min_height_touching = f64::max(
                    fluid_max_y - checking_liquids_aabb.min.y,
                    min_height_touching,
                );
                if !is_entity_pushable_by_fluid {
                    continue;
                }
                let mut additional_player_delta_for_fluid =
                    get_fluid_flow(&fluid_at_cur_pos, world, cur_pos);
                if min_height_touching < 0.4 {
                    additional_player_delta_for_fluid *= min_height_touching;
                };

                additional_player_delta += additional_player_delta_for_fluid;
                num_fluids_being_touched += 1;
            }
        }
    }

    if additional_player_delta.length() > 0. {
        additional_player_delta /= num_fluids_being_touched as f64;

        let player_delta = physics.velocity;
        additional_player_delta *= fluid_push_factor;
        const MIN_PUSH: f64 = 0.003;
        const MIN_PUSH_LENGTH: f64 = MIN_PUSH * 1.5;

        if player_delta.x.abs() < MIN_PUSH
            && player_delta.z.abs() < MIN_PUSH
            && additional_player_delta.length() < MIN_PUSH_LENGTH
        {
            additional_player_delta = additional_player_delta.normalize() * MIN_PUSH_LENGTH;
        }

        physics.velocity += additional_player_delta;
    }

    match checking_fluid {
        FluidKind::Water => physics.water_fluid_height = min_height_touching,
        FluidKind::Lava => physics.lava_fluid_height = min_height_touching,
        FluidKind::Empty => panic!("FluidKind::Empty should not be passed to update_fluid_height"),
    };

    touching_fluid
}

pub fn is_eye_in_fluid(kind: FluidKind, eye_y: f64, position: Position, world: &World) -> bool {
    let pos = BlockPos::new(
        position.x.floor() as i32,
        eye_y.floor() as i32,
        position.z.floor() as i32,
    );
    let Some(fluid) = world.get_fluid_state(pos) else {
        return false;
    };
    if fluid.kind != kind {
        return false;
    }
    let fluid_bottom = pos.y as f64;
    let fluid_top = fluid_bottom + fluid.height() as f64;
    eye_y >= fluid_bottom && eye_y <= fluid_top
}

#[allow(clippy::type_complexity)]
pub fn update_swimming(
    mut query: Query<
        (&Physics, &Position, &Sprinting, &mut Swimming, &WorldName),
        (With<LocalEntity>, With<HasClientLoaded>),
    >,
    worlds: Res<Worlds>,
) {
    for (physics, position, sprinting, mut swimming, world_name) in &mut query {
        let Some(world_lock) = worlds.get(world_name) else {
            continue;
        };
        let world = world_lock.read();

        let is_in_water = physics.is_in_water();
        let eye_y = position.y + player_eye_height(**swimming, false);
        let is_under_water = is_in_water && is_eye_in_fluid(FluidKind::Water, eye_y, *position, &world);

        let should_swim = if **swimming {
            **sprinting && is_in_water
        } else {
            let feet_pos = BlockPos::new(
                position.x.floor() as i32,
                position.y.floor() as i32,
                position.z.floor() as i32,
            );
            let feet_in_water = world
                .get_fluid_state(feet_pos)
                .is_some_and(|f| f.kind == FluidKind::Water);
            **sprinting && is_under_water && feet_in_water
        };

        if **swimming != should_swim {
            **swimming = should_swim;
        }
    }
}

pub fn get_fluid_flow(fluid: &FluidState, world: &World, pos: BlockPos) -> Vec3 {
    let mut z_flow: f64 = 0.;
    let mut x_flow: f64 = 0.;

    let cur_fluid_height = fluid.height();

    for direction in Direction::HORIZONTAL {
        let adjacent_block_pos = pos.offset_with_direction(direction);

        let adjacent_block_state = world
            .get_block_state(adjacent_block_pos)
            .unwrap_or_default();
        let adjacent_fluid_state = FluidState::from(adjacent_block_state);

        if !fluid.affects_flow(&adjacent_fluid_state) {
            continue;
        };
        let mut adjacent_fluid_height = adjacent_fluid_state.height();
        let mut adjacent_height_difference: f32 = 0.;

        if adjacent_fluid_height == 0. {
            if !legacy_blocks_motion(adjacent_block_state) {
                let block_pos_below_adjacent = adjacent_block_pos.down(1);
                let fluid_below_adjacent = world
                    .get_fluid_state(block_pos_below_adjacent)
                    .unwrap_or_default();

                if fluid.affects_flow(&fluid_below_adjacent) {
                    adjacent_fluid_height = fluid_below_adjacent.height();
                    if adjacent_fluid_height > 0. {
                        adjacent_height_difference =
                            cur_fluid_height - (adjacent_fluid_height - 0.8888889);
                    }
                }
            }
        } else if adjacent_fluid_height > 0. {
            adjacent_height_difference = cur_fluid_height - adjacent_fluid_height;
        }

        if adjacent_height_difference != 0. {
            x_flow += (direction.x() as f32 * adjacent_height_difference) as f64;
            z_flow += (direction.z() as f32 * adjacent_height_difference) as f64;
        }
    }

    let mut flow = Vec3::new(x_flow, 0., z_flow);
    if fluid.falling {
        for direction in Direction::HORIZONTAL {
            let adjacent_block_pos = pos.offset_with_direction(direction);
            if is_solid_face(fluid, world, adjacent_block_pos, direction)
                || is_solid_face(fluid, world, adjacent_block_pos.up(1), direction)
            {
                flow = flow.normalize() + Vec3::new(0., -6., 0.);
                break;
            }
        }
    }

    flow.normalize()
}

fn is_solid_face(
    fluid: &FluidState,
    world: &World,
    adjacent_pos: BlockPos,
    direction: Direction,
) -> bool {
    let block_state = world.get_block_state(adjacent_pos).unwrap_or_default();
    let fluid_state = world.get_fluid_state(adjacent_pos).unwrap_or_default();
    if fluid_state.is_same_kind(fluid) {
        return false;
    }
    if direction == Direction::Up {
        return true;
    }
    let registry_block = BlockKind::from(block_state);
    if matches!(
        registry_block,
        BlockKind::Ice | BlockKind::FrostedIce
    ) {
        return false;
    }
    is_face_sturdy(block_state, world, adjacent_pos, direction)
}

fn is_face_sturdy(
    block_state: BlockState,
    _world: &World,
    _pos: BlockPos,
    direction: Direction,
) -> bool {
    crate::collision::is_face_sturdy(block_state, direction)
}
