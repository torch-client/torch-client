pub const WIDTH: i32 = 64;

pub const MAX_DETAIL: u8 = 8;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct SectionPos {
    pub detail: u8,
    pub x: i32,
    pub z: i32,
}

impl SectionPos {
    pub const fn new(detail: u8, x: i32, z: i32) -> Self {
        Self { detail, x, z }
    }

    #[inline]
    pub fn column_width(self) -> i32 {
        1 << self.detail
    }

    #[inline]
    pub fn block_width(self) -> i32 {
        WIDTH << self.detail
    }

    #[inline]
    pub fn min_block_x(self) -> i32 {
        self.x * self.block_width()
    }

    #[inline]
    pub fn min_block_z(self) -> i32 {
        self.z * self.block_width()
    }

    pub fn center_block(self) -> (f64, f64) {
        let half = f64::from(self.block_width()) / 2.0;
        (
            f64::from(self.min_block_x()) + half,
            f64::from(self.min_block_z()) + half,
        )
    }

    pub fn containing(detail: u8, block_x: i32, block_z: i32) -> Self {
        let shift = 6 + detail as u32;
        Self {
            detail,
            x: block_x >> shift,
            z: block_z >> shift,
        }
    }

    pub fn parent(self) -> Self {
        Self {
            detail: self.detail + 1,
            x: self.x >> 1,
            z: self.z >> 1,
        }
    }

    pub fn child(self, i: u8) -> Self {
        debug_assert!(self.detail > 0 && i < 4);
        Self {
            detail: self.detail - 1,
            x: self.x * 2 + (i & 1) as i32,
            z: self.z * 2 + ((i & 2) >> 1) as i32,
        }
    }

    pub fn adjacent(self, dir: usize) -> Self {
        let (dx, dz) = [(0, -1), (0, 1), (-1, 0), (1, 0)][dir];
        Self {
            detail: self.detail,
            x: self.x + dx,
            z: self.z + dz,
        }
    }

    pub fn is_within(self, other: SectionPos) -> bool {
        if self.detail > other.detail {
            return false;
        }
        let shift = (other.detail - self.detail) as u32;
        self.x >> shift == other.x && self.z >> shift == other.z
    }
}

#[inline]
pub fn column_index(x: i32, z: i32) -> usize {
    (x * WIDTH + z) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn children_sit_inside_their_parent() {
        let p = SectionPos::new(3, -5, 7);
        for i in 0..4 {
            let c = p.child(i);
            assert_eq!(c.parent(), p);
            assert!(c.is_within(p));
        }
    }

    #[test]
    fn containing_floors_negative_blocks() {
        let s = SectionPos::containing(0, -1, -64);
        assert_eq!((s.x, s.z), (-1, -1));
        let s = SectionPos::containing(1, 127, 128);
        assert_eq!((s.x, s.z), (0, 1));
    }
}
