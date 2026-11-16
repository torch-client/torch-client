use azalea::block::BlockState;
use azalea::block::properties::{Lit, Open};
use azalea::physics::collision::BlockWithShape;
use azalea::physics::collision::world_collisions::get_block_collisions;
use azalea::world::{World, WorldBorder};
use azalea_core::aabb::Aabb;
use azalea_core::direction::Axis;
use azalea_core::position::{BlockPos, Vec3};
use azalea_registry::builtin::BlockKind;
use azalea_registry::tags::blocks as tags;

const PLAYER_WIDTH: f64 = 0.6;
const PLAYER_HEIGHT: f64 = 1.8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Step {
    pub x: i32,
    pub z: i32,
}

impl Step {
    pub const NORTH: Step = Step { x: 0, z: -1 };
    pub const SOUTH: Step = Step { x: 0, z: 1 };
    pub const WEST: Step = Step { x: -1, z: 0 };
    pub const EAST: Step = Step { x: 1, z: 0 };

    pub fn from_name(name: &str) -> Option<Step> {
        Some(match name {
            "north" => Step::NORTH,
            "south" => Step::SOUTH,
            "west" => Step::WEST,
            "east" => Step::EAST,
            _ => return None,
        })
    }

    fn clockwise(self) -> Step {
        Step {
            x: -self.z,
            z: self.x,
        }
    }

    fn opposite(self) -> Step {
        Step {
            x: -self.x,
            z: -self.z,
        }
    }

    fn is_facing_angle(self, y_angle: f32) -> bool {
        let radians = y_angle * 0.017_453_292;
        let dx = -radians.sin();
        let dz = radians.cos();
        self.x as f32 * dx + self.z as f32 * dz > 0.0
    }
}

pub(crate) fn find_stand_up_position(
    world: &World,
    border: &WorldBorder,
    pos: BlockPos,
    forward: Step,
    yaw: f32,
) -> Option<Vec3> {
    let right = forward.clockwise();
    let side = if right.is_facing_angle(yaw) {
        right.opposite()
    } else {
        right
    };
    if is_bunk_bed(world, pos) {
        let offsets = surround_offsets(forward, side);
        let above = above_offsets(forward);
        let below = pos.down(1);
        return at_offsets(world, border, pos, &offsets, true)
            .or_else(|| at_offsets(world, border, below, &offsets, true))
            .or_else(|| at_offsets(world, border, pos, &above, true))
            .or_else(|| at_offsets(world, border, pos, &offsets, false))
            .or_else(|| at_offsets(world, border, below, &offsets, false))
            .or_else(|| at_offsets(world, border, pos, &above, false));
    }
    let mut offsets = surround_offsets(forward, side).to_vec();
    offsets.extend_from_slice(&above_offsets(forward));
    at_offsets(world, border, pos, &offsets, true)
        .or_else(|| at_offsets(world, border, pos, &offsets, false))
}

fn is_bunk_bed(world: &World, pos: BlockPos) -> bool {
    tags::BEDS.contains(&BlockKind::from(state_at(world, pos.down(1))))
}

fn surround_offsets(f: Step, s: Step) -> [(i32, i32); 10] {
    [
        (s.x, s.z),
        (s.x - f.x, s.z - f.z),
        (s.x - f.x * 2, s.z - f.z * 2),
        (-f.x * 2, -f.z * 2),
        (-s.x - f.x * 2, -s.z - f.z * 2),
        (-s.x - f.x, -s.z - f.z),
        (-s.x, -s.z),
        (-s.x + f.x, -s.z + f.z),
        (f.x, f.z),
        (s.x + f.x, s.z + f.z),
    ]
}

fn above_offsets(f: Step) -> [(i32, i32); 2] {
    [(0, 0), (-f.x, -f.z)]
}

fn at_offsets(
    world: &World,
    border: &WorldBorder,
    pos: BlockPos,
    offsets: &[(i32, i32)],
    dangerous: bool,
) -> Option<Vec3> {
    offsets.iter().find_map(|&(dx, dz)| {
        safe_dismount_location(
            world,
            border,
            BlockPos::new(pos.x + dx, pos.y, pos.z + dz),
            dangerous,
        )
    })
}

fn state_at(world: &World, pos: BlockPos) -> BlockState {
    world.get_block_state(pos).unwrap_or(BlockState::AIR)
}

fn is_dangerous(state: BlockState) -> bool {
    let kind = BlockKind::from(state);
    let burning = tags::FIRE.contains(&kind)
        || kind == BlockKind::Lava
        || kind == BlockKind::MagmaBlock
        || (tags::CAMPFIRES.contains(&kind) && state.property::<Lit>() == Some(true))
        || kind == BlockKind::LavaCauldron;
    burning
        || matches!(
            kind,
            BlockKind::WitherRose
                | BlockKind::SweetBerryBush
                | BlockKind::Cactus
                | BlockKind::PowderSnow
        )
}

fn non_climbable_top(world: &World, pos: BlockPos) -> Option<f64> {
    let state = state_at(world, pos);
    let kind = BlockKind::from(state);
    if tags::CLIMBABLE.contains(&kind)
        || (tags::TRAPDOORS.contains(&kind) && state.property::<Open>() == Some(true))
    {
        return None;
    }
    let shape = state.collision_shape(pos);
    (!shape.is_empty()).then(|| shape.max(Axis::Y))
}

fn floor_height(world: &World, pos: BlockPos) -> f64 {
    match non_climbable_top(world, pos) {
        Some(top) => top,
        None => match non_climbable_top(world, pos.down(1)) {
            Some(below) if below >= 1.0 => below - 1.0,
            _ => f64::NEG_INFINITY,
        },
    }
}

fn border_holds(border: &WorldBorder, aabb: &Aabb) -> bool {
    const SHRINK: f64 = 9.999_999_747_378_752e-6;
    border.is_within_bounds(aabb.min.x, aabb.min.z)
        && border.is_within_bounds(aabb.max.x - SHRINK, aabb.max.z - SHRINK)
}

fn safe_dismount_location(
    world: &World,
    border: &WorldBorder,
    pos: BlockPos,
    dangerous: bool,
) -> Option<Vec3> {
    if dangerous && is_dangerous(state_at(world, pos)) {
        return None;
    }
    let floor = floor_height(world, pos);
    if floor.is_infinite() || floor >= 1.0 {
        return None;
    }
    if dangerous && floor <= 0.0 && is_dangerous(state_at(world, pos.down(1))) {
        return None;
    }
    let at = Vec3::new(pos.x as f64 + 0.5, pos.y as f64 + floor, pos.z as f64 + 0.5);
    let half = PLAYER_WIDTH / 2.0;
    let aabb = Aabb {
        min: Vec3::new(at.x - half, at.y, at.z - half),
        max: Vec3::new(at.x + half, at.y + PLAYER_HEIGHT, at.z + half),
    };
    if get_block_collisions(world, &aabb)
        .iter()
        .any(|shape| !shape.is_empty())
    {
        return None;
    }
    let kind = BlockKind::from(state_at(world, pos));
    let above = BlockKind::from(state_at(world, pos.up(1)));
    if tags::INVALID_SPAWN_INSIDE.contains(&kind) || tags::INVALID_SPAWN_INSIDE.contains(&above) {
        return None;
    }
    border_holds(border, &aabb).then_some(at)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clockwise_runs_north_east_south_west() {
        assert_eq!(Step::NORTH.clockwise(), Step::EAST);
        assert_eq!(Step::EAST.clockwise(), Step::SOUTH);
        assert_eq!(Step::SOUTH.clockwise(), Step::WEST);
        assert_eq!(Step::WEST.clockwise(), Step::NORTH);
    }

    #[test]
    fn facing_angle_matches_yaw_convention() {
        assert!(Step::SOUTH.is_facing_angle(0.0));
        assert!(!Step::NORTH.is_facing_angle(0.0));
        assert!(Step::WEST.is_facing_angle(90.0));
    }
}
