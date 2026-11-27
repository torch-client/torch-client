use azalea::block::{BlockState, BlockTrait};
use azalea::physics::collision::{BlockWithShape, sturdy::is_face_sturdy};
use azalea_core::{direction::Direction, position::BlockPos};
use azalea_registry::builtin::BlockKind;

use super::{PlaceContext, accepts, replaceable};
use crate::blocks::facing::{
    clockwise, counter_clockwise, from_name as direction_from_name, name as direction_name,
};
use crate::play::redstone::has_neighbor_signal;

pub(super) fn door_placement(block: BlockKind, ctx: &PlaceContext) -> Option<BlockState> {
    let above = ctx.pos.up(1);
    if replaceable(ctx.at(above), block) != Some(true) {
        return None;
    }
    if !is_face_sturdy(ctx.at(ctx.pos.down(1)), Direction::Up) {
        return None;
    }
    let powered =
        has_neighbor_signal(ctx.pos, ctx.block_at) || has_neighbor_signal(above, ctx.block_at);
    let powered = if powered { "true" } else { "false" };
    let mut placed = Box::<dyn BlockTrait>::from(BlockState::from(block));
    let _ = placed.set_property("facing", direction_name(ctx.horizontal()));
    let _ = placed.set_property("hinge", hinge(ctx));
    let _ = placed.set_property("powered", powered);
    let _ = placed.set_property("open", powered);
    let _ = placed.set_property("half", "lower");
    Some(placed.as_block_state())
}

fn hinge(ctx: &PlaceContext) -> &'static str {
    let place_direction = ctx.horizontal();
    let left = counter_clockwise(place_direction);
    let right = clockwise(place_direction);
    let above = ctx.pos.up(1);
    let full = |at: BlockPos| ctx.at(at).is_collision_shape_full();
    let balance = -i32::from(full(ctx.pos + left.normal()))
        - i32::from(full(above + left.normal()))
        + i32::from(full(ctx.pos + right.normal()))
        + i32::from(full(above + right.normal()));
    let lower_door = |at: BlockPos| {
        let state = ctx.at(at);
        accepts(state.as_block_kind(), "hinge", "left")
            && Box::<dyn BlockTrait>::from(state).get_property("half") == Some("lower")
    };
    let door_left = lower_door(ctx.pos + left.normal());
    let door_right = lower_door(ctx.pos + right.normal());

    if (door_left && !door_right) || balance > 0 {
        return "right";
    }
    if (door_right && !door_left) || balance < 0 {
        return "left";
    }
    let normal = place_direction.normal();
    let (step_x, step_z) = (normal.x, normal.z);
    let click_x = ctx.hit.location.x - f64::from(ctx.pos.x);
    let click_z = ctx.hit.location.z - f64::from(ctx.pos.z);
    let left_side = (step_x >= 0 || click_z >= 0.5)
        && (step_x <= 0 || click_z <= 0.5)
        && (step_z >= 0 || click_x <= 0.5)
        && (step_z <= 0 || click_x >= 0.5);
    if left_side { "left" } else { "right" }
}

pub(super) fn bed_placement(block: BlockKind, ctx: &PlaceContext) -> Option<BlockState> {
    let facing = ctx.horizontal();
    if replaceable(ctx.at(ctx.pos + facing.normal()), block) != Some(true) {
        return None;
    }
    let mut placed = Box::<dyn BlockTrait>::from(BlockState::from(block));
    let _ = placed.set_property("facing", direction_name(facing));
    let _ = placed.set_property("part", "foot");
    Some(placed.as_block_state())
}

pub(super) fn partner(state: BlockState, pos: BlockPos) -> Option<(BlockPos, BlockState)> {
    let mut other = Box::<dyn BlockTrait>::from(state);
    if other.get_property("half") == Some("lower")
        && accepts(state.as_block_kind(), "hinge", "left")
    {
        other.set_property("half", "upper").ok()?;
        return Some((pos.up(1), other.as_block_state()));
    }
    if other.get_property("part") == Some("foot") {
        let facing = other.get_property("facing").and_then(direction_from_name)?;
        other.set_property("part", "head").ok()?;
        return Some((pos + facing.normal(), other.as_block_state()));
    }
    None
}
