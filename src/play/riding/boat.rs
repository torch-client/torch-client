use azalea::block::fluid_state::{FluidKind, FluidState};
use azalea::block::{BlockState, BlockTrait};
use azalea::physics::collision::BlockWithShape;
use azalea_core::aabb::Aabb;
use azalea_core::math;
use azalea_core::position::{BlockPos, Vec3};
use azalea_registry::builtin::BlockKind;
use azalea_world::World;

use crate::util::mth::DEG_TO_RAD;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Keys {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
}

impl Keys {
    pub(crate) const NONE: Keys = Keys {
        left: false,
        right: false,
        up: false,
        down: false,
    };

    pub(crate) fn any(self) -> bool {
        self.left || self.right || self.up || self.down
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Status {
    InWater,
    UnderWater,
    UnderFlowingWater,
    OnLand,
    InAir,
}

#[derive(Debug, Default)]
pub(crate) struct Boat {
    status: Option<Status>,
    old_status: Option<Status>,
    water_level: f64,
    land_friction: f32,
    last_yd: f64,
    pub(crate) delta_rotation: f32,
    pub(crate) paddles: [bool; 2],
}

impl Boat {
    pub(crate) fn update_status(&mut self, world: &World, hull: &Aabb) {
        self.old_status = self.status;
        let status = if let Some(under) = underwater(world, hull) {
            self.water_level = hull.max.y;
            under
        } else if let Some(level) = in_water(world, hull) {
            self.water_level = level;
            Status::InWater
        } else {
            let friction = ground_friction(world, hull);
            if friction > 0.0 {
                self.land_friction = friction;
                Status::OnLand
            } else {
                Status::InAir
            }
        };
        self.status = Some(status);
    }

    pub(crate) fn float(
        &mut self,
        world: &World,
        hull: &Aabb,
        position: &mut Vec3,
        velocity: &mut Vec3,
        snap_free: impl FnOnce(f64) -> bool,
    ) {
        let height = hull.max.y - hull.min.y;
        let status = self.status;
        if self.old_status == Some(Status::InAir)
            && !matches!(status, Some(Status::InAir | Status::OnLand))
        {
            self.water_level = position.y + height;
            let target = (water_level_above(world, hull, self.last_yd) as f64 - height) + 0.101;
            if snap_free(target) {
                position.y = target;
                velocity.y = 0.0;
                self.last_yd = 0.0;
            }
            self.status = Some(Status::InWater);
            return;
        }

        let mut vspeed = -GRAVITY;
        let mut buoyancy = 0.0;
        let inv_friction = match status {
            Some(Status::InWater) => {
                buoyancy = (self.water_level - position.y) / height;
                0.9
            }
            Some(Status::UnderFlowingWater) => {
                vspeed = -7.0E-4;
                0.9
            }
            Some(Status::UnderWater) => {
                buoyancy = 0.009999999776482582;
                0.45
            }
            Some(Status::InAir) => 0.9,
            Some(Status::OnLand) => self.land_friction,
            None => 0.05,
        };
        velocity.x *= inv_friction as f64;
        velocity.y += vspeed;
        velocity.z *= inv_friction as f64;
        self.delta_rotation *= inv_friction;
        if buoyancy > 0.0 {
            velocity.y = (velocity.y + buoyancy * (GRAVITY / 0.65)) * 0.75;
        }
    }

    pub(crate) fn control(&mut self, k: Keys, y_rot: &mut f32, velocity: &mut Vec3) -> [bool; 2] {
        let mut acceleration = 0.0f32;
        if k.left {
            self.delta_rotation -= 1.0;
        }
        if k.right {
            self.delta_rotation += 1.0;
        }
        if k.right != k.left && !k.up && !k.down {
            acceleration += 0.005;
        }
        *y_rot += self.delta_rotation;
        if k.up {
            acceleration += 0.04;
        }
        if k.down {
            acceleration -= 0.005;
        }
        velocity.x += (math::sin(-*y_rot * DEG_TO_RAD) * acceleration) as f64;
        velocity.z += (math::cos(*y_rot * DEG_TO_RAD) * acceleration) as f64;
        self.paddles = [k.right && !k.left || k.up, k.left && !k.right || k.up];
        self.paddles
    }

    pub(crate) fn moved(&mut self, delta_y: f64) {
        self.last_yd = delta_y;
    }
}

const GRAVITY: f64 = 0.04;

fn water(world: &World, pos: BlockPos) -> Option<(FluidState, f32)> {
    let fluid = world
        .get_fluid_state(pos)
        .filter(|f| f.kind == FluidKind::Water)?;
    let height = fluid.height_under(&world.get_fluid_state(pos.up(1)).unwrap_or_default());
    Some((fluid, height))
}

fn span(min: f64, max: f64) -> std::ops::Range<i32> {
    min.floor() as i32..max.ceil() as i32
}

pub(crate) fn hull_under_water(world: &World, hull: &Aabb) -> bool {
    underwater(world, hull).is_some()
}

fn underwater(world: &World, hull: &Aabb) -> Option<Status> {
    let top = hull.max.y + 0.001;
    let mut under = false;
    for x in span(hull.min.x, hull.max.x) {
        for y in span(hull.max.y, top) {
            for z in span(hull.min.z, hull.max.z) {
                let pos = BlockPos::new(x, y, z);
                if let Some((fluid, height)) = water(world, pos)
                    && top < (y as f32 + height) as f64
                {
                    if !fluid.is_source() {
                        return Some(Status::UnderFlowingWater);
                    }
                    under = true;
                }
            }
        }
    }
    under.then_some(Status::UnderWater)
}

fn in_water(world: &World, hull: &Aabb) -> Option<f64> {
    let mut level = f64::MIN;
    let mut wet = false;
    for x in span(hull.min.x, hull.max.x) {
        for y in span(hull.min.y, hull.min.y + 0.001) {
            for z in span(hull.min.z, hull.max.z) {
                if let Some((_, height)) = water(world, BlockPos::new(x, y, z)) {
                    let surface = (y as f32 + height) as f64;
                    level = level.max(surface);
                    wet |= hull.min.y < surface;
                }
            }
        }
    }
    wet.then_some(level)
}

fn ground_friction(world: &World, hull: &Aabb) -> f32 {
    let sliver = Aabb {
        min: Vec3::new(hull.min.x, hull.min.y - 0.001, hull.min.z),
        max: Vec3::new(hull.max.x, hull.min.y, hull.max.z),
    };
    let (x0, x1) = (
        sliver.min.x.floor() as i32 - 1,
        sliver.max.x.ceil() as i32 + 1,
    );
    let (y0, y1) = (
        sliver.min.y.floor() as i32 - 1,
        sliver.max.y.ceil() as i32 + 1,
    );
    let (z0, z1) = (
        sliver.min.z.floor() as i32 - 1,
        sliver.max.z.ceil() as i32 + 1,
    );

    let mut friction = 0.0f32;
    let mut count = 0u32;
    for x in x0..x1 {
        for z in z0..z1 {
            let edges = u8::from(x == x0 || x == x1 - 1) + u8::from(z == z0 || z == z1 - 1);
            if edges == 2 {
                continue;
            }
            for y in y0..y1 {
                if edges > 0 && (y == y0 || y == y1 - 1) {
                    continue;
                }
                let pos = BlockPos::new(x, y, z);
                let Some(state) = world.get_block_state(pos) else {
                    continue;
                };
                if touches(state, pos, &sliver) {
                    friction += Box::<dyn BlockTrait>::from(state).behavior().friction;
                    count += 1;
                }
            }
        }
    }
    friction / count as f32
}

fn touches(state: BlockState, pos: BlockPos, sliver: &Aabb) -> bool {
    if state.is_collision_shape_empty() || BlockKind::from(state) == BlockKind::LilyPad {
        return false;
    }
    let origin = Vec3::new(pos.x as f64, pos.y as f64, pos.z as f64);
    let local = Aabb {
        min: sliver.min - origin,
        max: sliver.max - origin,
    };
    state
        .collision_shape(pos)
        .to_aabbs()
        .iter()
        .any(|b| b.intersects_aabb(&local))
}

fn water_level_above(world: &World, hull: &Aabb, last_yd: f64) -> f32 {
    let (x0, x1) = (hull.min.x.floor() as i32, hull.max.x.ceil() as i32);
    let (y0, y1) = (
        hull.max.y.floor() as i32,
        (hull.max.y - last_yd).ceil() as i32,
    );
    let (z0, z1) = (hull.min.z.floor() as i32, hull.max.z.ceil() as i32);
    'layers: for y in y0..y1 {
        let mut height = 0.0f32;
        for x in x0..x1 {
            for z in z0..z1 {
                if let Some((_, h)) = water(world, BlockPos::new(x, y, z)) {
                    height = height.max(h);
                }
                if height >= 1.0 {
                    continue 'layers;
                }
            }
        }
        if height < 1.0 {
            return y as f32 + height;
        }
    }
    (y1 + 1) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boat(status: Status) -> Boat {
        Boat {
            status: Some(status),
            old_status: Some(status),
            ..Boat::default()
        }
    }

    fn steady_speed(status: Status, land_friction: f32, ticks: u32) -> f64 {
        let up = Keys {
            up: true,
            ..Keys::default()
        };
        let mut b = boat(status);
        b.land_friction = land_friction;
        let world = World::default();
        let hull = Aabb {
            min: Vec3::new(0.0, 0.0, 0.0),
            max: Vec3::new(1.375, 0.5625, 1.375),
        };
        let mut pos = Vec3::ZERO;
        let mut velocity = Vec3::ZERO;
        let mut yaw = 0.0;
        for _ in 0..ticks {
            b.float(&world, &hull, &mut pos, &mut velocity, |_| true);
            b.control(up, &mut yaw, &mut velocity);
            velocity.y = 0.0;
        }
        velocity.z
    }

    #[test]
    fn rowing_on_still_water_settles_at_eight_blocks_a_second() {
        assert!((steady_speed(Status::InAir, 0.0, 400) - 0.4).abs() < 1e-6);
    }

    #[test]
    fn ice_settles_where_its_friction_puts_it() {
        assert!((steady_speed(Status::OnLand, 0.98, 4000) - 2.0).abs() < 1e-3);
        assert!((steady_speed(Status::OnLand, 0.989, 8000) - 0.04 / 0.011).abs() < 1e-3);
    }

    #[test]
    fn turning_in_place_creeps_and_coasts() {
        let left = Keys {
            left: true,
            ..Keys::default()
        };
        let mut b = boat(Status::InAir);
        let mut yaw = 0.0;
        let mut velocity = Vec3::ZERO;
        assert_eq!(b.control(left, &mut yaw, &mut velocity), [false, true]);
        assert_eq!(yaw, -1.0);
        assert!(velocity.horizontal_distance() > 0.004);
        let world = World::default();
        let hull = Aabb {
            min: Vec3::ZERO,
            max: Vec3::new(1.375, 0.5625, 1.375),
        };
        b.float(&world, &hull, &mut Vec3::ZERO, &mut velocity, |_| true);
        assert!((b.delta_rotation + 0.9).abs() < 1e-6);
    }

    #[test]
    fn forward_works_both_paddles() {
        let keys = Keys {
            up: true,
            left: true,
            ..Keys::default()
        };
        let mut b = boat(Status::InWater);
        assert_eq!(b.control(keys, &mut 0.0, &mut Vec3::ZERO), [true, true]);
    }
}
