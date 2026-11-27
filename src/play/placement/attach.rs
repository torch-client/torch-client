use std::str::FromStr;

use azalea::block::{BlockState, BlockTrait};
use azalea::physics::collision::sturdy::{
    can_support_center, is_face_sturdy, is_face_sturdy_center, is_solid,
};
use azalea_core::direction::Direction;
use azalea_registry::{
    builtin::BlockKind,
    tags::blocks::{
        ALL_HANGING_SIGNS, CANNOT_SUPPORT_SNOW_LAYER, STANDING_SIGNS, SUPPORT_OVERRIDE_SNOW_LAYER,
        SUPPORTS_COCOA, WALL_HANGING_SIGNS,
    },
};

use super::{
    PlaceContext, accepts, axis_name, bool_str, is_full_water, property_digit, replaceable,
    rotation_segment, segment_name,
};
use crate::blocks::facing::{
    clockwise, counter_clockwise, from_name as direction_from_name, name as direction_name,
};
use crate::play::redstone::has_neighbor_signal;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::play::placement) enum Mount {
    Torch,
    Sign,
    Banner,
    Skull,
    CoralFan,
    HangingSign,
}

pub(in crate::play::placement) fn wall_variant(standing: BlockKind) -> Option<(BlockKind, Mount)> {
    let id = standing.to_str().trim_start_matches("minecraft:");
    let mount = if matches!(
        standing,
        BlockKind::Torch | BlockKind::SoulTorch | BlockKind::CopperTorch | BlockKind::RedstoneTorch
    ) {
        Mount::Torch
    } else if STANDING_SIGNS.contains(&standing) {
        Mount::Sign
    } else if id.ends_with("_banner") && !id.ends_with("_wall_banner") {
        Mount::Banner
    } else if accepts(standing, "rotation", "0") && accepts(standing, "powered", "true") {
        Mount::Skull
    } else if id.ends_with("coral_fan") {
        Mount::CoralFan
    } else if ALL_HANGING_SIGNS.contains(&standing) && !WALL_HANGING_SIGNS.contains(&standing) {
        Mount::HangingSign
    } else {
        return None;
    };
    let wall_id = if mount == Mount::HangingSign {
        format!("{}_wall_hanging_sign", id.strip_suffix("_hanging_sign")?)
    } else {
        match id.rsplit_once('_') {
            Some((head, last)) => format!("{head}_wall_{last}"),
            None => format!("wall_{id}"),
        }
    };
    BlockKind::from_str(&wall_id).ok().map(|wall| (wall, mount))
}

pub(in crate::play::placement) fn standing_or_wall(
    standing: BlockKind,
    wall: BlockKind,
    mount: Mount,
    ctx: &PlaceContext,
) -> Option<BlockState> {
    let attachment = if mount == Mount::HangingSign {
        Direction::Up
    } else {
        Direction::Down
    };
    let wall_state = wall_placement(standing, wall, mount, ctx);
    for direction in ctx.looking() {
        if direction == attachment.opposite() {
            continue;
        }
        if direction == attachment {
            if let Some(state) = standing_placement(standing, mount, ctx) {
                return Some(state);
            }
        } else if wall_state.is_some() {
            return wall_state;
        }
    }
    None
}

fn standing_placement(standing: BlockKind, mount: Mount, ctx: &PlaceContext) -> Option<BlockState> {
    let below = ctx.at(ctx.pos.down(1));
    let mut placed = Box::<dyn BlockTrait>::from(BlockState::from(standing));
    let survives = match mount {
        Mount::Torch => can_support_center(below, Direction::Up),
        Mount::Sign => {
            let _ = placed.set_property("rotation", rotation_segment(ctx.yaw + 180.0));
            let _ = placed.set_property("waterlogged", ctx.waterlogged());
            is_solid(below)
        }
        Mount::Banner => {
            let _ = placed.set_property("rotation", rotation_segment(ctx.yaw + 180.0));
            is_solid(below)
        }
        Mount::Skull => {
            let _ = placed.set_property("rotation", rotation_segment(ctx.yaw));
            let _ = placed.set_property(
                "powered",
                bool_str(has_neighbor_signal(ctx.pos, ctx.block_at)),
            );
            true
        }
        Mount::CoralFan => {
            let _ = placed.set_property("waterlogged", bool_str(is_full_water(ctx.replaced_state)));
            is_face_sturdy(below, Direction::Up)
        }
        Mount::HangingSign => {
            let above = ctx.at(ctx.pos.up(1));
            let (attached, rotation) = ceiling_hanging(ctx, above);
            let _ = placed.set_property("attached", bool_str(attached));
            let _ = placed.set_property("rotation", rotation);
            let _ = placed.set_property("waterlogged", ctx.waterlogged());
            is_face_sturdy_center(above, Direction::Down)
        }
    };
    survives.then(|| placed.as_block_state())
}

fn ceiling_hanging(ctx: &PlaceContext, above: BlockState) -> (bool, &'static str) {
    let direction = ctx.horizontal();
    let mut attached = !is_face_sturdy(above, Direction::Down) || ctx.secondary_use;
    if ALL_HANGING_SIGNS.contains(&above.as_block_kind()) && !ctx.secondary_use {
        let above_props = Box::<dyn BlockTrait>::from(above);
        let above_direction = match above_props.get_property("facing") {
            Some(facing) => direction_from_name(facing),
            None => match above_props.get_property("rotation") {
                Some("0") => Some(Direction::North),
                Some("4") => Some(Direction::East),
                Some("8") => Some(Direction::South),
                Some("12") => Some(Direction::West),
                _ => None,
            },
        };
        if above_direction.is_some_and(|d| axis_name(d) == axis_name(direction)) {
            attached = false;
        }
    }
    let rotation = if attached {
        rotation_segment(ctx.yaw + 180.0)
    } else {
        segment_name(match direction.opposite() {
            Direction::South => 0,
            Direction::West => 4,
            Direction::North => 8,
            _ => 12,
        })
    };
    (attached, rotation)
}

fn wall_placement(
    standing: BlockKind,
    wall: BlockKind,
    mount: Mount,
    ctx: &PlaceContext,
) -> Option<BlockState> {
    let mut placed = Box::<dyn BlockTrait>::from(BlockState::from(wall));
    let behind = |d: Direction| ctx.at(ctx.pos + d.normal());
    let direction = match mount {
        Mount::Torch => ctx.first_horizontal(|d| is_face_sturdy(behind(d), d.opposite()))?,
        Mount::Sign => {
            let _ = placed.set_property("waterlogged", ctx.waterlogged());
            ctx.first_horizontal(|d| is_solid(behind(d)))?
        }
        Mount::Banner => ctx.first_horizontal(|d| is_solid(behind(d)))?,
        Mount::Skull => {
            let _ = placed.set_property(
                "powered",
                bool_str(has_neighbor_signal(ctx.pos, ctx.block_at)),
            );
            ctx.first_horizontal(|d| replaceable(behind(d), standing) != Some(true))?
        }
        Mount::CoralFan => {
            let _ = placed.set_property("waterlogged", bool_str(is_full_water(ctx.replaced_state)));
            ctx.first_horizontal(|d| is_face_sturdy(behind(d), d.opposite()))?
        }
        Mount::HangingSign => {
            let _ = placed.set_property("waterlogged", ctx.waterlogged());
            let clicked_axis = axis_name(ctx.hit.direction);
            ctx.first_horizontal(|d| {
                axis_name(d) != clicked_axis && wall_hanging_holds(d.opposite(), ctx)
            })?
        }
    };
    let _ = placed.set_property("facing", direction_name(direction.opposite()));
    Some(placed.as_block_state())
}

fn wall_hanging_holds(facing: Direction, ctx: &PlaceContext) -> bool {
    let attaches = |side: Direction, face: Direction| {
        let state = ctx.at(ctx.pos + side.normal());
        if WALL_HANGING_SIGNS.contains(&state.as_block_kind()) {
            Box::<dyn BlockTrait>::from(state)
                .get_property("facing")
                .and_then(direction_from_name)
                .is_some_and(|other| axis_name(other) == axis_name(facing))
        } else {
            is_face_sturdy(state, face)
        }
    };
    let cw = clockwise(facing);
    let ccw = counter_clockwise(facing);
    attaches(cw, ccw) || attaches(ccw, cw)
}

pub(in crate::play::placement) fn cocoa_placement(ctx: &PlaceContext) -> Option<BlockState> {
    let direction = ctx.first_horizontal(|d| {
        SUPPORTS_COCOA.contains(&ctx.at(ctx.pos + d.normal()).as_block_kind())
    })?;
    let mut placed = Box::<dyn BlockTrait>::from(BlockState::from(BlockKind::Cocoa));
    let _ = placed.set_property("facing", direction_name(direction));
    Some(placed.as_block_state())
}

pub(in crate::play::placement) fn multiface_placement(
    block: BlockKind,
    ctx: &PlaceContext,
) -> Option<BlockState> {
    let direction = ctx
        .looking()
        .into_iter()
        .find(|&d| is_face_sturdy(ctx.at(ctx.pos + d.normal()), d.opposite()))?;
    let mut placed = Box::<dyn BlockTrait>::from(BlockState::from(block));
    let _ = placed.set_property("waterlogged", ctx.waterlogged());
    let _ = placed.set_property(direction_name(direction), "true");
    Some(placed.as_block_state())
}

pub(in crate::play::placement) fn vine_placement(ctx: &PlaceContext) -> Option<BlockState> {
    let above = ctx.at(ctx.pos.up(1));
    let direction = ctx.looking().into_iter().find(|&d| {
        if d == Direction::Down {
            return false;
        }
        if is_face_sturdy(ctx.at(ctx.pos + d.normal()), d.opposite()) {
            return true;
        }
        d != Direction::Up
            && above.as_block_kind() == BlockKind::Vine
            && Box::<dyn BlockTrait>::from(above).get_property(direction_name(d)) == Some("true")
    })?;
    let mut placed = Box::<dyn BlockTrait>::from(BlockState::from(BlockKind::Vine));
    let _ = placed.set_property(direction_name(direction), "true");
    Some(placed.as_block_state())
}

pub(in crate::play::placement) fn snow_placement(ctx: &PlaceContext) -> Option<BlockState> {
    let below = ctx.at(ctx.pos.down(1));
    let kind = below.as_block_kind();
    let survives = if CANNOT_SUPPORT_SNOW_LAYER.contains(&kind) {
        false
    } else if SUPPORT_OVERRIDE_SNOW_LAYER.contains(&kind) {
        true
    } else {
        is_face_sturdy(below, Direction::Up)
            || (kind == BlockKind::Snow && property_digit(below, "layers") == Some(8))
    };
    survives.then(|| BlockState::from(BlockKind::Snow))
}
