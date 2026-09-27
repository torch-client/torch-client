use std::collections::HashMap;

use super::ColumnPos;
use super::props::{self, LightProps};

pub const NEGATIVE_INFINITY: i32 = i32::MIN;

#[derive(Clone)]
pub struct ChunkSources {
    min_y: i32,
    heights: Box<[i32; 256]>,
}

#[inline]
const fn index(x: usize, z: usize) -> usize {
    x + z * 16
}

impl ChunkSources {
    pub fn empty(world_min_y: i32) -> Self {
        let min_y = world_min_y - 1;
        Self {
            min_y,
            heights: Box::new([min_y; 256]),
        }
    }

    #[inline]
    pub fn lowest_source_y(&self, x: usize, z: usize) -> i32 {
        let v = self.heights[index(x, z)];
        if v == self.min_y {
            NEGATIVE_INFINITY
        } else {
            v
        }
    }

    pub fn highest_lowest_source_y(&self) -> i32 {
        let max = self.heights.iter().copied().max().unwrap_or(self.min_y);
        if max == self.min_y {
            NEGATIVE_INFINITY
        } else {
            max
        }
    }

    fn set(&mut self, x: usize, z: usize, y: i32) {
        self.heights[index(x, z)] = y;
    }

    pub fn fill_from<F>(&mut self, top_y: i32, mut state_at: F)
    where
        F: FnMut(usize, i32, usize) -> &'static LightProps,
    {
        for z in 0..16usize {
            for x in 0..16usize {
                let mut top = props::air();
                let mut y = top_y;
                let mut found = self.min_y;
                while y > self.min_y {
                    let bottom = state_at(x, y - 1, z);
                    if props::edge_occluded(top, bottom) {
                        found = y;
                        break;
                    }
                    top = bottom;
                    y -= 1;
                }
                self.set(x, z, found.max(self.min_y));
            }
        }
    }

    pub fn update<F>(&mut self, x: usize, y: i32, z: usize, mut state_at: F) -> bool
    where
        F: FnMut(usize, i32, usize) -> &'static LightProps,
    {
        let current = self.heights[index(x, z)];
        if y + 1 < current {
            return false;
        }
        let top = state_at(x, y + 1, z);
        let middle = state_at(x, y, z);
        if self.update_edge(x, z, current, y + 1, top, middle, &mut state_at) {
            return true;
        }
        let bottom = state_at(x, y - 1, z);
        self.update_edge(x, z, current, y, middle, bottom, &mut state_at)
    }

    fn update_edge<F>(
        &mut self,
        x: usize,
        z: usize,
        old_top_edge_y: i32,
        checked_edge_y: i32,
        top: &'static LightProps,
        bottom: &'static LightProps,
        state_at: &mut F,
    ) -> bool
    where
        F: FnMut(usize, i32, usize) -> &'static LightProps,
    {
        if props::edge_occluded(top, bottom) {
            if checked_edge_y > old_top_edge_y {
                self.set(x, z, checked_edge_y);
                return true;
            }
        } else if checked_edge_y == old_top_edge_y {
            let found = self.find_lowest_source_below(x, checked_edge_y - 1, z, bottom, state_at);
            self.set(x, z, found);
            return true;
        }
        false
    }

    fn find_lowest_source_below<F>(
        &self,
        x: usize,
        start_y: i32,
        z: usize,
        start: &'static LightProps,
        state_at: &mut F,
    ) -> i32
    where
        F: FnMut(usize, i32, usize) -> &'static LightProps,
    {
        let mut top = start;
        let mut y = start_y;
        while y - 1 >= self.min_y {
            let bottom = state_at(x, y - 1, z);
            if props::edge_occluded(top, bottom) {
                return y;
            }
            top = bottom;
            y -= 1;
        }
        self.min_y
    }
}

pub struct SkySources {
    world_min_y: i32,
    empty: ChunkSources,
    columns: HashMap<ColumnPos, ChunkSources>,
}

impl SkySources {
    pub fn new(world_min_y: i32) -> Self {
        Self {
            world_min_y,
            empty: ChunkSources::empty(world_min_y),
            columns: HashMap::new(),
        }
    }

    pub fn column(&self, cx: i32, cz: i32) -> Option<&ChunkSources> {
        self.columns.get(&ColumnPos { x: cx, z: cz })
    }

    pub fn column_mut(&mut self, cx: i32, cz: i32) -> Option<&mut ChunkSources> {
        self.columns.get_mut(&ColumnPos { x: cx, z: cz })
    }

    pub fn column_or_empty(&self, cx: i32, cz: i32) -> &ChunkSources {
        self.column(cx, cz).unwrap_or(&self.empty)
    }

    pub fn insert(&mut self, col: ColumnPos, sources: ChunkSources) {
        self.columns.insert(col, sources);
    }

    pub fn remove(&mut self, col: ColumnPos) {
        self.columns.remove(&col);
    }

    pub fn bytes(&self) -> (u64, usize) {
        let per = std::mem::size_of::<[i32; 256]>() as u64;
        (self.columns.len() as u64 * per, self.columns.len())
    }

    pub fn highest_lowest_source_y(&self, col: ColumnPos) -> i32 {
        self.column_or_empty(col.x, col.z).highest_lowest_source_y()
    }

    pub fn new_chunk_sources(&self) -> ChunkSources {
        ChunkSources::empty(self.world_min_y)
    }
}
