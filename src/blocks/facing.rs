use azalea_core::direction::Direction;

use crate::blocks::direction::Direction as Face;

pub fn from_3d_data_value(value: i32) -> Direction {
    Face::from_3d_data_value(value).into()
}

pub fn name(direction: Direction) -> &'static str {
    Face::from(direction).name()
}

pub fn from_name(name: &str) -> Option<Direction> {
    Face::from_name(name).map(Direction::from)
}

pub fn clockwise(direction: Direction) -> Direction {
    Face::from(direction).clockwise().into()
}

pub fn counter_clockwise(direction: Direction) -> Direction {
    Face::from(direction).counter_clockwise().into()
}
