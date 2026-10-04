use azalea_core::direction::Direction;

pub fn name(direction: Direction) -> &'static str {
    match direction {
        Direction::Down => "down",
        Direction::Up => "up",
        Direction::North => "north",
        Direction::South => "south",
        Direction::West => "west",
        Direction::East => "east",
    }
}

pub fn from_name(name: &str) -> Option<Direction> {
    Some(match name {
        "down" => Direction::Down,
        "up" => Direction::Up,
        "north" => Direction::North,
        "south" => Direction::South,
        "west" => Direction::West,
        "east" => Direction::East,
        _ => return None,
    })
}

pub fn clockwise(direction: Direction) -> Direction {
    match direction {
        Direction::North => Direction::East,
        Direction::East => Direction::South,
        Direction::South => Direction::West,
        Direction::West => Direction::North,
        other => other,
    }
}

pub fn counter_clockwise(direction: Direction) -> Direction {
    match direction {
        Direction::North => Direction::West,
        Direction::West => Direction::South,
        Direction::South => Direction::East,
        Direction::East => Direction::North,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Direction; 6] = [
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ];

    #[test]
    fn names_round_trip() {
        for direction in ALL {
            assert_eq!(from_name(name(direction)), Some(direction));
        }
        assert_eq!(from_name("sideways"), None);
    }

    #[test]
    fn the_two_rotations_undo_each_other() {
        for direction in ALL {
            assert_eq!(counter_clockwise(clockwise(direction)), direction);
        }
        assert_eq!(clockwise(Direction::North), Direction::East);
        assert_eq!(counter_clockwise(Direction::North), Direction::West);
    }
}
