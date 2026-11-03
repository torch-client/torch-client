use azalea_core::{aabb::Aabb, position::BlockPos};
use bevy_ecs::component::Component;

pub const MAX_SIZE: f64 = 5.9999968E7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BorderStatus {
    Growing,
    Shrinking,
    Stationary,
}

impl BorderStatus {
    pub fn color(self) -> u32 {
        match self {
            Self::Growing => 4259712,
            Self::Shrinking => 16724016,
            Self::Stationary => 2138367,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Extent {
    Static {
        size: f64,
    },
    Moving {
        from: f64,
        to: f64,
        duration: f64,
        remaining: i64,
        size: f64,
        prev_size: f64,
    },
}

#[derive(Clone, Component, Copy, Debug, PartialEq)]
pub struct WorldBorder {
    pub center_x: f64,
    pub center_z: f64,
    pub absolute_max_size: i32,
    pub warning_blocks: i32,
    pub warning_time: i32,
    extent: Extent,
}

impl Default for WorldBorder {
    fn default() -> Self {
        Self {
            center_x: 0.0,
            center_z: 0.0,
            absolute_max_size: 29999984,
            warning_blocks: 5,
            warning_time: 15,
            extent: Extent::Static { size: MAX_SIZE },
        }
    }
}

fn clamp(value: f64, min: f64, max: f64) -> f64 {
    if value < min { min } else { value.min(max) }
}

impl WorldBorder {
    fn size_at(&self, partial: f32) -> f64 {
        match self.extent {
            Extent::Static { size } => size,
            Extent::Moving { size, prev_size, .. } => prev_size + f64::from(partial) * (size - prev_size),
        }
    }

    fn edge(&self, center: f64, sign: f64, partial: f32) -> f64 {
        let abs = f64::from(self.absolute_max_size);
        clamp(center + sign * self.size_at(partial) / 2.0, -abs, abs)
    }

    pub fn min_x(&self, partial: f32) -> f64 {
        self.edge(self.center_x, -1.0, partial)
    }
    pub fn max_x(&self, partial: f32) -> f64 {
        self.edge(self.center_x, 1.0, partial)
    }
    pub fn min_z(&self, partial: f32) -> f64 {
        self.edge(self.center_z, -1.0, partial)
    }
    pub fn max_z(&self, partial: f32) -> f64 {
        self.edge(self.center_z, 1.0, partial)
    }

    pub fn is_within_bounds_margin(&self, x: f64, z: f64, margin: f64) -> bool {
        x >= self.min_x(0.0) - margin
            && x < self.max_x(0.0) + margin
            && z >= self.min_z(0.0) - margin
            && z < self.max_z(0.0) + margin
    }

    pub fn is_within_bounds(&self, x: f64, z: f64) -> bool {
        self.is_within_bounds_margin(x, z, 0.0)
    }

    pub fn is_within_block(&self, pos: BlockPos) -> bool {
        self.is_within_bounds(f64::from(pos.x), f64::from(pos.z))
    }

    pub fn distance_to_border(&self, x: f64, z: f64) -> f64 {
        let from_north = z - self.min_z(0.0);
        let from_south = self.max_z(0.0) - z;
        let from_west = x - self.min_x(0.0);
        let from_east = self.max_x(0.0) - x;
        from_west.min(from_east).min(from_north).min(from_south)
    }

    pub fn is_inside_close_to_border(&self, x: f64, z: f64, aabb: &Aabb) -> bool {
        let x_size = aabb.max.x - aabb.min.x;
        let z_size = aabb.max.z - aabb.min.z;
        let bb_max = x_size.abs().max(z_size.abs()).max(1.0);
        self.distance_to_border(x, z) < bb_max * 2.0 && self.is_within_bounds_margin(x, z, bb_max)
    }

    pub fn collision_bounds(&self) -> [f64; 4] {
        [
            self.min_x(0.0).floor(),
            self.max_x(0.0).ceil(),
            self.min_z(0.0).floor(),
            self.max_z(0.0).ceil(),
        ]
    }

    pub fn status(&self) -> BorderStatus {
        match self.extent {
            Extent::Static { .. } => BorderStatus::Stationary,
            Extent::Moving { from, to, .. } if to < from => BorderStatus::Shrinking,
            Extent::Moving { .. } => BorderStatus::Growing,
        }
    }

    pub fn size(&self) -> f64 {
        match self.extent {
            Extent::Static { size } | Extent::Moving { size, .. } => size,
        }
    }

    pub fn lerp_target(&self) -> f64 {
        match self.extent {
            Extent::Static { size } => size,
            Extent::Moving { to, .. } => to,
        }
    }

    pub fn lerp_speed(&self) -> f64 {
        match self.extent {
            Extent::Static { .. } => 0.0,
            Extent::Moving { from, to, duration, .. } => (from - to).abs() / duration,
        }
    }

    pub fn set_center(&mut self, x: f64, z: f64) {
        self.center_x = x;
        self.center_z = z;
    }

    pub fn set_size(&mut self, size: f64) {
        self.extent = Extent::Static { size };
    }

    pub fn lerp_size_between(&mut self, from: f64, to: f64, ticks: i64) {
        self.extent = if from == to {
            Extent::Static { size: to }
        } else {
            let start = if ticks == 0 { to } else { from };
            Extent::Moving {
                from,
                to,
                duration: ticks as f64,
                remaining: ticks,
                size: start,
                prev_size: start,
            }
        };
    }

    pub fn tick(&mut self) {
        let Extent::Moving { from, to, duration, remaining, size, prev_size } = &mut self.extent else {
            return;
        };
        *remaining -= 1;
        *prev_size = *size;
        let progress = (*duration - *remaining as f64) / *duration;
        *size = if progress < 1.0 { *from + progress * (*to - *from) } else { *to };
        if *remaining <= 0 {
            let to = *to;
            self.extent = Extent::Static { size: to };
        }
    }

    pub fn is_moving(&self) -> bool {
        matches!(self.extent, Extent::Moving { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerp_reaches_target_and_settles() {
        let mut b = WorldBorder::default();
        b.lerp_size_between(10.0, 20.0, 4);
        assert_eq!(b.status(), BorderStatus::Growing);
        assert_eq!(b.size(), 10.0);
        b.tick();
        assert_eq!((b.size_at(0.0), b.size_at(1.0)), (10.0, 12.5));
        for _ in 0..3 {
            b.tick();
        }
        assert_eq!(b.status(), BorderStatus::Stationary);
        assert_eq!(b.size(), 20.0);
        assert_eq!(b.lerp_speed(), 0.0);
    }

    #[test]
    fn zero_tick_lerp_starts_at_target() {
        let mut b = WorldBorder::default();
        b.lerp_size_between(10.0, 20.0, 0);
        assert_eq!((b.size_at(0.0), b.size()), (20.0, 20.0));
        b.tick();
        assert_eq!(b.status(), BorderStatus::Stationary);
    }

    #[test]
    fn bounds_and_distance() {
        let mut b = WorldBorder::default();
        b.set_size(10.0);
        b.set_center(0.5, 0.0);
        assert_eq!((b.min_x(0.0), b.max_x(0.0)), (-4.5, 5.5));
        assert!(b.is_within_block(BlockPos::new(-4, 0, 4)));
        assert!(!b.is_within_block(BlockPos::new(-5, 0, 0)));
        assert_eq!(b.distance_to_border(0.5, 3.0), 2.0);
        assert_eq!(b.collision_bounds(), [-5.0, 6.0, -5.0, 5.0]);
    }
}
