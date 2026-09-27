pub mod data_layer;
pub mod engine;
pub mod level;
pub mod props;
pub mod sources;
pub mod storage;

#[cfg(test)]
mod tests;

pub use level::{LightJob, LightMap, LightThread, ServerLight};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SectionPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl SectionPos {
    #[inline]
    pub const fn of_block(x: i32, y: i32, z: i32) -> Self {
        Self {
            x: x >> 4,
            y: y >> 4,
            z: z >> 4,
        }
    }

    #[inline]
    pub const fn column(self) -> ColumnPos {
        ColumnPos {
            x: self.x,
            z: self.z,
        }
    }

    #[inline]
    pub const fn min_block_y(self) -> i32 {
        self.y << 4
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ColumnPos {
    pub x: i32,
    pub z: i32,
}

impl ColumnPos {
    #[inline]
    pub const fn section(self, y: i32) -> SectionPos {
        SectionPos {
            x: self.x,
            y,
            z: self.z,
        }
    }
}

pub use crate::direction::Direction;

pub const MAX_LEVEL: u8 = 15;
