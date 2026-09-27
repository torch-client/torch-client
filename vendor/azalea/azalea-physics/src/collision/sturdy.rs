use azalea_block::{BlockState, BlockTrait};
use azalea_core::{
    direction::{Axis, Direction},
    math::EPSILON,
};
use azalea_registry::{builtin::BlockKind, tags::blocks::UNSTABLE_BOTTOM_CENTER};

use super::{BlockWithShape, VoxelShape};

const CENTER_LO: f64 = 7.0 / 16.0;
const CENTER_HI: f64 = 9.0 / 16.0;

pub fn is_face_sturdy(block_state: BlockState, direction: Direction) -> bool {
    if block_state.is_collision_shape_full() {
        return true;
    }
    if block_state.is_collision_shape_empty() {
        return false;
    }
    face_covers(
        block_state.base_collision_shape(),
        direction,
        (0.0, 1.0),
        (0.0, 1.0),
    )
}

pub fn is_face_sturdy_center(block_state: BlockState, direction: Direction) -> bool {
    if block_state.is_collision_shape_full() {
        return true;
    }
    if block_state.is_collision_shape_empty() {
        return false;
    }
    face_covers(
        block_state.base_collision_shape(),
        direction,
        (CENTER_LO, CENTER_HI),
        (CENTER_LO, CENTER_HI),
    )
}

pub fn can_support_center(block_state: BlockState, direction: Direction) -> bool {
    if direction == Direction::Down && UNSTABLE_BOTTOM_CENTER.contains(&BlockKind::from(block_state))
    {
        return false;
    }
    is_face_sturdy_center(block_state, direction)
}

pub fn is_solid(block_state: BlockState) -> bool {
    if block_state.is_collision_shape_empty() {
        return Box::<dyn BlockTrait>::from(block_state)
            .behavior()
            .force_solid
            == Some(true);
    }
    if block_state.is_collision_shape_full() {
        return Box::<dyn BlockTrait>::from(block_state)
            .behavior()
            .force_solid
            != Some(false);
    }
    if let Some(forced) = Box::<dyn BlockTrait>::from(block_state).behavior().force_solid {
        return forced;
    }
    let bounds = block_state.base_collision_shape().bounds();
    bounds.size() >= 0.7291666666666666 || bounds.get_size(Axis::Y) >= 1.0
}

fn face_covers(
    shape: &VoxelShape,
    direction: Direction,
    u_range: (f64, f64),
    v_range: (f64, f64),
) -> bool {
    let axis = direction_axis(direction);
    let discrete = shape.shape();

    let depth = discrete.size(axis);
    if depth == 0 {
        return false;
    }

    let layer = shape.find_index(
        axis,
        if is_positive(direction) {
            0.9999999
        } else {
            1.0e-7
        },
    );
    if layer < 0 || layer as u32 >= depth {
        return false;
    }
    let layer = layer as u32;

    let (u_axis, v_axis) = other_axes(axis);
    let Some(u_cells) = cells_covering(shape, u_axis, u_range) else {
        return false;
    };
    let Some(v_cells) = cells_covering(shape, v_axis, v_range) else {
        return false;
    };
    for u in u_cells.clone() {
        for v in v_cells.clone() {
            let (x, y, z) = match axis {
                Axis::X => (layer, u, v),
                Axis::Y => (u, layer, v),
                Axis::Z => (u, v, layer),
            };
            if !discrete.is_full(x, y, z) {
                return false;
            }
        }
    }
    true
}

fn cells_covering(
    shape: &VoxelShape,
    axis: Axis,
    range: (f64, f64),
) -> Option<std::ops::Range<u32>> {
    let (lo, hi) = range;
    let coords = shape.get_coords(axis);
    if coords.len() < 2 {
        return None;
    }
    if coords[0] > lo + EPSILON || coords[coords.len() - 1] < hi - EPSILON {
        return None;
    }
    let mut first = None;
    let mut last = None;
    for i in 0..coords.len() - 1 {
        if coords[i + 1] > lo + EPSILON && coords[i] < hi - EPSILON {
            first.get_or_insert(i as u32);
            last = Some(i as u32);
        }
    }
    Some(first?..last? + 1)
}

fn direction_axis(direction: Direction) -> Axis {
    match direction {
        Direction::Down | Direction::Up => Axis::Y,
        Direction::North | Direction::South => Axis::Z,
        Direction::West | Direction::East => Axis::X,
    }
}

fn is_positive(direction: Direction) -> bool {
    matches!(
        direction,
        Direction::Up | Direction::South | Direction::East
    )
}

fn other_axes(axis: Axis) -> (Axis, Axis) {
    match axis {
        Axis::X => (Axis::Y, Axis::Z),
        Axis::Y => (Axis::X, Axis::Z),
        Axis::Z => (Axis::X, Axis::Y),
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

    fn state(block: BlockKind) -> BlockState {
        BlockState::from(block)
    }

    #[test]
    fn a_full_cube_is_sturdy_on_every_face() {
        for direction in ALL {
            assert!(
                is_face_sturdy(state(BlockKind::Stone), direction),
                "stone {direction:?}"
            );
        }
    }

    #[test]
    fn air_is_sturdy_on_no_face() {
        for direction in ALL {
            assert!(!is_face_sturdy(state(BlockKind::Air), direction));
        }
    }

    #[test]
    fn a_bottom_slab_is_sturdy_below_and_not_above() {
        let slab = state(BlockKind::StoneSlab);
        assert!(is_face_sturdy(slab, Direction::Down));
        assert!(!is_face_sturdy(slab, Direction::Up));
        assert!(!is_face_sturdy(slab, Direction::North));
    }

    #[test]
    fn a_fence_supports_the_centre_but_is_not_a_full_face() {
        let fence = state(BlockKind::OakFence);
        assert!(!is_face_sturdy(fence, Direction::Up));
        assert!(is_face_sturdy_center(fence, Direction::Up));
    }
}
