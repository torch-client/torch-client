use azalea_core::position::{BlockPos, ChunkPos};

pub struct BlockIterator {
    start: BlockPos,
    max_distance: u32,

    pos: BlockPos,
    apothem: u32,
    left: i32,
    right: i32,
}
impl BlockIterator {
    pub fn new(start: BlockPos, max_distance: u32) -> Self {
        Self {
            start,
            max_distance,

            pos: BlockPos {
                x: -1,
                y: -1,
                z: -1,
            },
            apothem: 1,
            left: 1,
            right: 2,
        }
    }
}

impl Iterator for BlockIterator {
    type Item = BlockPos;

    fn next(&mut self) -> Option<Self::Item> {
        if self.apothem > self.max_distance {
            return None;
        }

        self.right -= 1;
        if self.right < 0 {
            self.left -= 1;
            if self.left < 0 {
                self.pos.z += 2;
                if self.pos.z > 1 {
                    self.pos.y += 2;
                    if self.pos.y > 1 {
                        self.pos.x += 2;
                        if self.pos.x > 1 {
                            self.apothem += 1;
                            self.pos.x = -1;
                        }
                        self.pos.y = -1;
                    }
                    self.pos.z = -1;
                }
                self.left = self.apothem as i32;
            }
            self.right = self.left;
        }
        let x = self.pos.x * self.right;
        let y = self.pos.y * ((self.apothem as i32) - self.left);
        let z = self.pos.z * ((self.apothem as i32) - (i32::abs(x) + i32::abs(y)));
        Some(BlockPos { x, y, z } + self.start)
    }
}

pub struct SquareChunkIterator {
    start: ChunkPos,
    number_of_points: u32,

    dir: ChunkPos,

    segment_len: u32,
    pos: ChunkPos,
    segment_passed: u32,
    current_iter: u32,
}
impl SquareChunkIterator {
    pub fn new(start: ChunkPos, max_distance: u32) -> Self {
        Self {
            start,
            number_of_points: u32::pow(max_distance * 2 - 1, 2),

            dir: ChunkPos { x: 1, z: 0 },

            segment_len: 1,
            pos: ChunkPos::default(),
            segment_passed: 0,
            current_iter: 0,
        }
    }

    pub fn set_max_distance(&mut self, max_distance: u32) {
        self.number_of_points = u32::pow(max_distance * 2 - 1, 2);
    }
}
impl Iterator for SquareChunkIterator {
    type Item = ChunkPos;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_iter > self.number_of_points {
            return None;
        }

        let output = self.start + self.dir;

        self.pos.x += self.dir.x;
        self.pos.z += self.dir.z;
        self.segment_passed += 1;

        if self.segment_passed == self.segment_len {
            self.segment_passed = 0;

            (self.dir.x, self.dir.z) = (-self.dir.z, self.dir.x);

            if self.dir.z == 0 {
                self.segment_len += 1;
            }
        }
        self.current_iter += 1;
        Some(output)
    }
}

pub struct ChunkIterator {
    pub max_distance: u32,
    pub start: ChunkPos,
    pub pos: ChunkPos,
    pub layer: u32,
    pub leg: i32,
}
impl ChunkIterator {
    pub fn new(start: ChunkPos, max_distance: u32) -> Self {
        Self {
            max_distance,
            start,
            pos: ChunkPos { x: 2, z: -1 },
            layer: 1,
            leg: -1,
        }
    }
}
impl Iterator for ChunkIterator {
    type Item = ChunkPos;

    fn next(&mut self) -> Option<Self::Item> {
        match self.leg {
            -1 => {
                self.leg = 0;
                return Some(self.start);
            }
            0 => {
                if self.max_distance == 1 {
                    return None;
                }
                self.pos.x -= 1;
                self.pos.z += 1;
                if self.pos.x == 0 {
                    self.leg = 1;
                }
            }
            1 => {
                self.pos.x -= 1;
                self.pos.z -= 1;
                if self.pos.z == 0 {
                    self.leg = 2;
                }
            }
            2 => {
                self.pos.x += 1;
                self.pos.z -= 1;
                if self.pos.x == 0 {
                    self.leg = 3;
                }
            }
            3 => {
                self.pos.x += 1;
                self.pos.z += 1;
                if self.pos.z == 0 {
                    self.pos.x += 1;
                    self.leg = 0;
                    self.layer += 1;
                    if self.layer == self.max_distance {
                        return None;
                    }
                }
            }
            _ => unreachable!(),
        }
        Some(self.start + self.pos)
    }
}
