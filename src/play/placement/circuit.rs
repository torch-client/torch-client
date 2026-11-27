use azalea::block::{BlockState, BlockTrait};
use azalea::physics::collision::{
    BlockWithShape,
    sturdy::{can_support_center, is_face_sturdy},
};
use azalea_core::direction::Direction;
use azalea_registry::{
    builtin::BlockKind,
    tags::blocks::{BUTTONS, PRESSURE_PLATES, TRAPDOORS},
};

use super::{PlaceContext, bool_str};
use crate::blocks::facing::{
    clockwise, counter_clockwise, from_name as direction_from_name, name as direction_name,
};

const HORIZONTAL: [Direction; 4] = [
    Direction::North,
    Direction::East,
    Direction::South,
    Direction::West,
];

pub(super) fn rigid_top(state: BlockState) -> bool {
    is_face_sturdy(state, Direction::Up)
        || matches!(
            state.as_block_kind(),
            BlockKind::Hopper
                | BlockKind::Cauldron
                | BlockKind::WaterCauldron
                | BlockKind::LavaCauldron
                | BlockKind::PowderSnowCauldron
                | BlockKind::Composter
        )
}

fn wire_survives_on(state: BlockState) -> bool {
    is_face_sturdy(state, Direction::Up) || state.as_block_kind() == BlockKind::Hopper
}

fn conductor(state: BlockState) -> bool {
    state.is_collision_shape_full()
}

fn signal_source(state: BlockState) -> bool {
    let kind = state.as_block_kind();
    BUTTONS.contains(&kind)
        || PRESSURE_PLATES.contains(&kind)
        || kind.to_str().ends_with("lightning_rod")
        || matches!(
            kind,
            BlockKind::DaylightDetector
                | BlockKind::DetectorRail
                | BlockKind::Repeater
                | BlockKind::Comparator
                | BlockKind::Lever
                | BlockKind::Lectern
                | BlockKind::Jukebox
                | BlockKind::RedstoneTorch
                | BlockKind::RedstoneWallTorch
                | BlockKind::Observer
                | BlockKind::RedstoneWire
                | BlockKind::TrappedChest
                | BlockKind::RedstoneBlock
                | BlockKind::SculkSensor
                | BlockKind::CalibratedSculkSensor
                | BlockKind::Target
                | BlockKind::TripwireHook
        )
}

fn facing_of(state: BlockState) -> Option<Direction> {
    Box::<dyn BlockTrait>::from(state)
        .get_property("facing")
        .and_then(direction_from_name)
}

fn wire_connects_to(state: BlockState, direction: Option<Direction>) -> bool {
    match state.as_block_kind() {
        BlockKind::RedstoneWire => true,
        BlockKind::Repeater => {
            let facing = facing_of(state);
            direction.is_some_and(|d| facing == Some(d) || facing == Some(d.opposite()))
        }
        BlockKind::Observer => direction.is_some() && facing_of(state) == direction,
        _ => signal_source(state) && direction.is_some(),
    }
}

fn connecting_side(ctx: &PlaceContext, direction: Direction, can_connect_up: bool) -> &'static str {
    let relative_pos = ctx.pos + direction.normal();
    let relative = ctx.at(relative_pos);
    if can_connect_up {
        let placeable_above =
            TRAPDOORS.contains(&relative.as_block_kind()) || wire_survives_on(relative);
        if placeable_above && wire_connects_to(ctx.at(relative_pos.up(1)), None) {
            return if is_face_sturdy(relative, direction.opposite()) {
                "up"
            } else {
                "side"
            };
        }
    }
    let none = !wire_connects_to(relative, Some(direction))
        && (conductor(relative) || !wire_connects_to(ctx.at(relative_pos.down(1)), None));
    if none { "none" } else { "side" }
}

pub(super) fn wire_placement(ctx: &PlaceContext) -> Option<BlockState> {
    if !wire_survives_on(ctx.at(ctx.pos.down(1))) {
        return None;
    }
    let can_connect_up = !conductor(ctx.at(ctx.pos.up(1)));
    let mut sides = HORIZONTAL.map(|direction| connecting_side(ctx, direction, can_connect_up));
    let connected = sides.map(|side| side != "none");
    let [north, east, south, west] = connected;
    let north_south_empty = !north && !south;
    let east_west_empty = !east && !west;
    if !west && north_south_empty {
        sides[3] = "side";
    }
    if !east && north_south_empty {
        sides[1] = "side";
    }
    if !north && east_west_empty {
        sides[0] = "side";
    }
    if !south && east_west_empty {
        sides[2] = "side";
    }

    let mut placed = Box::<dyn BlockTrait>::from(BlockState::from(BlockKind::RedstoneWire));
    for (direction, side) in HORIZONTAL.into_iter().zip(sides) {
        let _ = placed.set_property(direction_name(direction), side);
    }
    Some(placed.as_block_state())
}

pub(super) fn diode_placement(block: BlockKind, ctx: &PlaceContext) -> Option<BlockState> {
    if !rigid_top(ctx.at(ctx.pos.down(1))) {
        return None;
    }
    let facing = ctx.horizontal().opposite();
    let mut placed = Box::<dyn BlockTrait>::from(BlockState::from(block));
    let _ = placed.set_property("facing", direction_name(facing));
    if block == BlockKind::Repeater {
        let locked = [clockwise(facing), counter_clockwise(facing)]
            .into_iter()
            .any(|side| {
                let neighbour = ctx.at(ctx.pos + side.normal());
                matches!(
                    neighbour.as_block_kind(),
                    BlockKind::Repeater | BlockKind::Comparator
                ) && Box::<dyn BlockTrait>::from(neighbour).get_property("powered") == Some("true")
                    && facing_of(neighbour) == Some(side)
            });
        let _ = placed.set_property("locked", bool_str(locked));
    }
    Some(placed.as_block_state())
}

pub(super) fn rail_placement(block: BlockKind, ctx: &PlaceContext) -> Option<BlockState> {
    if !rigid_top(ctx.at(ctx.pos.down(1))) {
        return None;
    }
    let shape = match ctx.horizontal() {
        Direction::East | Direction::West => "east_west",
        _ => "north_south",
    };
    let mut placed = Box::<dyn BlockTrait>::from(BlockState::from(block));
    let _ = placed.set_property("shape", shape);
    let _ = placed.set_property("waterlogged", ctx.waterlogged());
    Some(placed.as_block_state())
}

pub(super) fn pressure_plate_placement(block: BlockKind, ctx: &PlaceContext) -> Option<BlockState> {
    let below = ctx.at(ctx.pos.down(1));
    (rigid_top(below) || can_support_center(below, Direction::Up)).then(|| BlockState::from(block))
}

pub(super) fn tripwire_placement(ctx: &PlaceContext) -> Option<BlockState> {
    let mut placed = Box::<dyn BlockTrait>::from(BlockState::from(BlockKind::Tripwire));
    for direction in HORIZONTAL {
        let neighbour = ctx.at(ctx.pos + direction.normal());
        let connects = match neighbour.as_block_kind() {
            BlockKind::TripwireHook => facing_of(neighbour) == Some(direction.opposite()),
            BlockKind::Tripwire => true,
            _ => false,
        };
        let _ = placed.set_property(direction_name(direction), bool_str(connects));
    }
    Some(placed.as_block_state())
}

pub(super) fn tripwire_hook_placement(ctx: &PlaceContext) -> Option<BlockState> {
    let direction =
        ctx.first_horizontal(|d| is_face_sturdy(ctx.at(ctx.pos + d.normal()), d.opposite()))?;
    let mut placed = Box::<dyn BlockTrait>::from(BlockState::from(BlockKind::TripwireHook));
    let _ = placed.set_property("facing", direction_name(direction.opposite()));
    let _ = placed.set_property("powered", "false");
    let _ = placed.set_property("attached", "false");
    Some(placed.as_block_state())
}
