use azalea::block::{BlockState, BlockTrait};
use azalea::physics::collision::BlockWithShape;
use azalea_core::{direction::Direction, position::BlockPos};
use azalea_registry::builtin::BlockKind;

use crate::blocks::facing;

const DIRECTIONS: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

pub fn has_neighbor_signal(
    pos: BlockPos,
    block_at: &dyn Fn(BlockPos) -> Option<BlockState>,
) -> bool {
    DIRECTIONS
        .iter()
        .any(|&direction| signal(pos + direction.normal(), direction, block_at) > 0)
}

fn signal(
    pos: BlockPos,
    direction: Direction,
    block_at: &dyn Fn(BlockPos) -> Option<BlockState>,
) -> u8 {
    let Some(state) = block_at(pos) else {
        return 0;
    };
    let own = block_signal(state, direction);
    if state.is_collision_shape_full() {
        own.max(direct_signal_to(pos, block_at))
    } else {
        own
    }
}

fn direct_signal_to(pos: BlockPos, block_at: &dyn Fn(BlockPos) -> Option<BlockState>) -> u8 {
    let mut best = 0;
    for direction in DIRECTIONS {
        let Some(state) = block_at(pos + direction.normal()) else {
            continue;
        };
        best = best.max(block_direct_signal(state, direction));
        if best >= 15 {
            return best;
        }
    }
    best
}

fn block_signal(state: BlockState, direction: Direction) -> u8 {
    let kind = state.as_block_kind();
    let id = kind.to_str();
    let block = Box::<dyn BlockTrait>::from(state);
    let powered = block.get_property("powered") == Some("true");

    if kind == BlockKind::RedstoneBlock {
        return 15;
    }

    if kind == BlockKind::Lever || id.ends_with("_button") {
        return if powered { 15 } else { 0 };
    }

    if kind == BlockKind::RedstoneTorch {
        let lit = block.get_property("lit") == Some("true");
        return if lit && direction != Direction::Up {
            15
        } else {
            0
        };
    }
    if kind == BlockKind::RedstoneWallTorch {
        let lit = block.get_property("lit") == Some("true");
        let facing = block.get_property("facing");
        return if lit && facing != Some(facing::name(direction)) {
            15
        } else {
            0
        };
    }

    if kind == BlockKind::Repeater || kind == BlockKind::Comparator || kind == BlockKind::Observer {
        if !powered || block.get_property("facing") != Some(facing::name(direction)) {
            return 0;
        }
        return if kind == BlockKind::Comparator { 1 } else { 15 };
    }

    if kind == BlockKind::RedstoneWire {
        if direction == Direction::Down {
            return 0;
        }
        let power = block
            .get_property("power")
            .and_then(|p| p.parse().ok())
            .unwrap_or(0);
        if power == 0 {
            return 0;
        }
        if direction == Direction::Up {
            return power;
        }
        let side = block.get_property(facing::name(direction.opposite()));
        return if matches!(side, Some("side") | Some("up")) {
            power
        } else {
            0
        };
    }

    if id.ends_with("_pressure_plate") {
        if let Some(power) = block.get_property("power").and_then(|p| p.parse().ok()) {
            return power;
        }
        return if powered { 15 } else { 0 };
    }
    if matches!(
        kind,
        BlockKind::Target
            | BlockKind::DaylightDetector
            | BlockKind::SculkSensor
            | BlockKind::CalibratedSculkSensor
    ) {
        return block
            .get_property("power")
            .and_then(|p| p.parse().ok())
            .unwrap_or(0);
    }

    if matches!(
        kind,
        BlockKind::DetectorRail
            | BlockKind::TripwireHook
            | BlockKind::Lectern
            | BlockKind::LightningRod
    ) {
        return if powered { 15 } else { 0 };
    }

    0
}

fn block_direct_signal(state: BlockState, direction: Direction) -> u8 {
    let kind = state.as_block_kind();
    let id = kind.to_str();
    let block = Box::<dyn BlockTrait>::from(state);

    if kind == BlockKind::Lever || id.ends_with("_button") {
        if block.get_property("powered") != Some("true") {
            return 0;
        }
        let connected = match block.get_property("face") {
            Some("ceiling") => Direction::Down,
            Some("floor") => Direction::Up,
            _ => match block.get_property("facing") {
                Some(name) => match facing::from_name(name) {
                    Some(d) => d,
                    None => return 0,
                },
                None => return 0,
            },
        };
        return if connected == direction { 15 } else { 0 };
    }

    if id.ends_with("_pressure_plate") {
        return if direction == Direction::Up {
            block_signal(state, direction)
        } else {
            0
        };
    }

    if kind == BlockKind::RedstoneTorch || kind == BlockKind::RedstoneWallTorch {
        return if direction == Direction::Down {
            block_signal(state, direction)
        } else {
            0
        };
    }

    if matches!(
        kind,
        BlockKind::Repeater | BlockKind::Comparator | BlockKind::Observer | BlockKind::RedstoneWire
    ) {
        return block_signal(state, direction);
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: BlockPos = BlockPos { x: 0, y: 0, z: 0 };

    fn world(entries: Vec<(BlockPos, BlockState)>) -> impl Fn(BlockPos) -> Option<BlockState> {
        move |pos| {
            Some(
                entries
                    .iter()
                    .find(|(p, _)| *p == pos)
                    .map(|(_, s)| *s)
                    .unwrap_or(BlockState::AIR),
            )
        }
    }

    fn powered_lever() -> BlockState {
        let mut lever = Box::<dyn BlockTrait>::from(BlockState::from(BlockKind::Lever));
        let _ = lever.set_property("powered", "true");
        let _ = lever.set_property("face", "floor");
        lever.as_block_state()
    }

    #[test]
    fn nothing_around_is_no_signal() {
        let w = world(vec![]);
        assert!(!has_neighbor_signal(ORIGIN, &w));
    }

    #[test]
    fn a_lever_beside_us_is_a_signal() {
        let w = world(vec![(BlockPos { x: 1, y: 0, z: 0 }, powered_lever())]);
        assert!(has_neighbor_signal(ORIGIN, &w));
    }

    #[test]
    fn an_unflipped_lever_is_not() {
        let w = world(vec![(
            BlockPos { x: 1, y: 0, z: 0 },
            BlockState::from(BlockKind::Lever),
        )]);
        assert!(!has_neighbor_signal(ORIGIN, &w));
    }

    #[test]
    fn a_redstone_block_is_a_signal() {
        let w = world(vec![(
            BlockPos { x: 0, y: 1, z: 0 },
            BlockState::from(BlockKind::RedstoneBlock),
        )]);
        assert!(has_neighbor_signal(ORIGIN, &w));
    }

    #[test]
    fn a_lever_strong_powers_through_the_block_it_is_on() {
        let w = world(vec![
            (
                BlockPos { x: 0, y: 1, z: 0 },
                BlockState::from(BlockKind::Stone),
            ),
            (BlockPos { x: 0, y: 2, z: 0 }, powered_lever()),
        ]);
        assert!(has_neighbor_signal(ORIGIN, &w));
    }

    #[test]
    fn strong_power_does_not_cross_a_slab() {
        let w = world(vec![
            (
                BlockPos { x: 0, y: 1, z: 0 },
                BlockState::from(BlockKind::StoneSlab),
            ),
            (BlockPos { x: 0, y: 2, z: 0 }, powered_lever()),
        ]);
        assert!(!has_neighbor_signal(ORIGIN, &w));
    }

    #[test]
    fn a_repeater_powers_only_what_it_faces() {
        let mut repeater = Box::<dyn BlockTrait>::from(BlockState::from(BlockKind::Repeater));
        let _ = repeater.set_property("powered", "true");
        let _ = repeater.set_property("facing", "north");
        let repeater = repeater.as_block_state();

        let w = world(vec![(BlockPos { x: 0, y: 0, z: 1 }, repeater)]);
        assert!(has_neighbor_signal(ORIGIN, &w));

        let w = world(vec![(BlockPos { x: 0, y: 0, z: -1 }, repeater)]);
        assert!(!has_neighbor_signal(ORIGIN, &w));
    }
}
