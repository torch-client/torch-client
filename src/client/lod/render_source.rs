use super::pos::{SectionPos, WIDTH};
use super::render_point;

pub struct RenderSource {
    pub pos: SectionPos,
    pub max_vertical: usize,
    pub y_offset: i32,
    pub(crate) data: Vec<u64>,
    pub(crate) void_data: Vec<u64>,
    pub(crate) attributes: Vec<u8>,
    is_empty: bool,
}

impl RenderSource {
    pub fn new(pos: SectionPos, max_vertical: usize, y_offset: i32) -> Self {
        let columns = (WIDTH * WIDTH) as usize;
        Self {
            pos,
            max_vertical,
            y_offset,
            data: vec![render_point::EMPTY; columns * max_vertical],
            void_data: vec![render_point::EMPTY; columns],
            attributes: vec![0; columns * max_vertical],
            is_empty: true,
        }
    }

    #[inline]
    pub fn detail(&self) -> u8 {
        self.pos.detail
    }

    #[inline]
    fn column_offset(&self, x: i32, z: i32) -> usize {
        (x * WIDTH + z) as usize * self.max_vertical
    }

    #[inline]
    pub fn point(&self, x: i32, z: i32, i: usize) -> u64 {
        self.data[self.column_offset(x, z) + i]
    }

    #[inline]
    pub fn column(&self, x: i32, z: i32) -> &[u64] {
        let o = self.column_offset(x, z);
        &self.data[o..o + self.max_vertical]
    }

    #[inline]
    pub fn column_mut(&mut self, x: i32, z: i32) -> &mut [u64] {
        let o = self.column_offset(x, z);
        let n = self.max_vertical;
        &mut self.data[o..o + n]
    }

    #[inline]
    pub fn void_point(&self, x: i32, z: i32) -> u64 {
        self.void_data[(x * WIDTH + z) as usize]
    }

    #[inline]
    pub fn set_void_point(&mut self, x: i32, z: i32, p: u64) {
        self.void_data[(x * WIDTH + z) as usize] = p;
    }

    pub fn set_attributes(
        &mut self,
        x: i32,
        z: i32,
        i: usize,
        fluid_amount: u8,
        waterlogged: bool,
    ) {
        let o = self.column_offset(x, z) + i;
        self.attributes[o] = (fluid_amount & 0x0F) | if waterlogged { 0x80 } else { 0 };
    }

    #[inline]
    pub fn fluid_amount(&self, x: i32, z: i32, i: usize) -> u8 {
        self.attributes[self.column_offset(x, z) + i] & 0x0F
    }

    #[inline]
    pub fn is_waterlogged(&self, x: i32, z: i32, i: usize) -> bool {
        self.attributes[self.column_offset(x, z) + i] & 0x80 != 0
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.is_empty
    }

    #[inline]
    pub fn mark_not_empty(&mut self) {
        self.is_empty = false;
    }

    pub fn has_non_void_points(&self) -> bool {
        !self.is_empty
            && self
                .data
                .iter()
                .any(|&p| render_point::exists(p) && !render_point::has_zero_height(p))
    }
}
