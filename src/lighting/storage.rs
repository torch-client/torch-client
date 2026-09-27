use std::collections::{HashMap, HashSet};

use super::data_layer::DataLayer;
use super::{ColumnPos, SectionPos};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct SectionState(u8);

impl SectionState {
    const HAS_DATA: u8 = 32;

    fn with_data(self, has: bool) -> Self {
        Self(if has {
            self.0 | Self::HAS_DATA
        } else {
            self.0 & !Self::HAS_DATA
        })
    }

    fn with_neighbor_count(self, count: i32) -> Self {
        debug_assert!(
            (0..=26).contains(&count),
            "neighbour count {count} out of range"
        );
        Self((self.0 & !31) | (count as u8 & 31))
    }

    fn neighbor_count(self) -> i32 {
        (self.0 & 31) as i32
    }

    fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    Block,
    Sky,
}

pub struct LightStorage {
    layer: Layer,
    section_states: HashMap<SectionPos, SectionState>,
    sections: HashMap<SectionPos, DataLayer>,
    columns_with_sources: HashSet<ColumnPos>,
    affected: HashSet<SectionPos>,
    top_sections: HashMap<ColumnPos, i32>,
    current_lowest_y: i32,
    queued: HashMap<SectionPos, DataLayer>,
}

impl LightStorage {
    pub fn new(layer: Layer) -> Self {
        Self {
            layer,
            section_states: HashMap::new(),
            sections: HashMap::new(),
            columns_with_sources: HashSet::new(),
            affected: HashSet::new(),
            top_sections: HashMap::new(),
            current_lowest_y: i32::MAX,
            queued: HashMap::new(),
        }
    }

    pub fn queue_section_data(&mut self, sec: SectionPos, data: Option<DataLayer>) {
        match data {
            Some(data) => {
                self.queued.insert(sec, data);
            }
            None => {
                self.queued.remove(&sec);
            }
        }
    }

    pub fn install_queued(&mut self, cols: &[ColumnPos]) {
        for col in cols {
            let secs: Vec<SectionPos> = self
                .queued
                .keys()
                .copied()
                .filter(|sec| sec.column() == *col)
                .collect();
            for sec in secs {
                let Some(data) = self.queued.remove(&sec) else {
                    continue;
                };
                if !self.storing_light_for_section(sec) {
                    continue;
                }
                self.sections.insert(sec, data);
                self.mark_section_and_neighbors_affected(sec);
            }
        }
    }

    pub fn drop_queued(&mut self, col: ColumnPos) {
        self.queued.retain(|sec, _| sec.column() != col);
    }

    pub fn layer(&self) -> Layer {
        self.layer
    }

    pub fn bytes(&self) -> (u64, usize) {
        let layers: u64 = self.sections.values().map(|d| d.bytes()).sum();
        let states = (self.section_states.len()
            * (std::mem::size_of::<SectionPos>() + std::mem::size_of::<SectionState>()))
            as u64;
        let tops = (self.top_sections.len()
            * (std::mem::size_of::<ColumnPos>() + std::mem::size_of::<i32>()))
            as u64;
        let cols = (self.columns_with_sources.len() * std::mem::size_of::<ColumnPos>()) as u64;
        (layers + states + tops + cols, self.sections.len())
    }

    #[inline]
    pub fn storing_light_for_section(&self, sec: SectionPos) -> bool {
        self.sections.contains_key(&sec)
    }

    pub fn data_layer_to_write(&mut self, sec: SectionPos) -> Option<&mut DataLayer> {
        self.sections.get_mut(&sec)
    }

    pub fn data_layer(&self, sec: SectionPos) -> Option<&DataLayer> {
        self.sections.get(&sec)
    }

    #[inline]
    pub fn get_stored_level(&self, x: i32, y: i32, z: i32) -> u8 {
        match self.sections.get(&SectionPos::of_block(x, y, z)) {
            Some(layer) => layer.get(rel(x), rel(y), rel(z)),
            None => 0,
        }
    }

    #[inline]
    pub fn set_stored_level(&mut self, x: i32, y: i32, z: i32, level: u8) {
        let sec = SectionPos::of_block(x, y, z);
        if let Some(layer) = self.sections.get_mut(&sec) {
            layer.set(rel(x), rel(y), rel(z), level);
        }
        for dx in around(x) {
            for dy in around(y) {
                for dz in around(z) {
                    self.affected
                        .insert(SectionPos::of_block(x + dx, y + dy, z + dz));
                }
            }
        }
    }

    fn mark_section_and_neighbors_affected(&mut self, sec: SectionPos) {
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    self.affected.insert(SectionPos {
                        x: sec.x + dx,
                        y: sec.y + dy,
                        z: sec.z + dz,
                    });
                }
            }
        }
    }

    pub fn take_affected(&mut self) -> HashSet<SectionPos> {
        std::mem::take(&mut self.affected)
    }

    pub fn set_light_enabled(&mut self, col: ColumnPos, enable: bool) {
        if enable {
            self.columns_with_sources.insert(col);
        } else {
            self.columns_with_sources.remove(&col);
        }
    }

    #[inline]
    pub fn light_on_in_section(&self, sec: SectionPos) -> bool {
        self.columns_with_sources.contains(&sec.column())
    }

    pub fn update_section_status(&mut self, sec: SectionPos, section_empty: bool) {
        let state = self.state(sec);
        let new_state = state.with_data(!section_empty);
        if state == new_state {
            return;
        }
        self.put_section_state(sec, new_state);
        let increment = if section_empty { -1 } else { 1 };
        for dx in -1..=1i32 {
            for dy in -1..=1i32 {
                for dz in -1..=1i32 {
                    if dx == 0 && dy == 0 && dz == 0 {
                        continue;
                    }
                    let n = SectionPos {
                        x: sec.x + dx,
                        y: sec.y + dy,
                        z: sec.z + dz,
                    };
                    let ns = self.state(n);
                    self.put_section_state(
                        n,
                        ns.with_neighbor_count(ns.neighbor_count() + increment),
                    );
                }
            }
        }
    }

    fn state(&self, sec: SectionPos) -> SectionState {
        self.section_states.get(&sec).copied().unwrap_or_default()
    }

    fn put_section_state(&mut self, sec: SectionPos, state: SectionState) {
        if !state.is_empty() {
            let was = self.section_states.insert(sec, state).unwrap_or_default();
            if was.is_empty() {
                self.initialize_section(sec);
            }
        } else if self
            .section_states
            .remove(&sec)
            .is_some_and(|s| !s.is_empty())
        {
            self.remove_section(sec);
        }
    }

    fn initialize_section(&mut self, sec: SectionPos) {
        let layer = self.create_data_layer(sec);
        self.sections.insert(sec, layer);
        self.on_node_added(sec);
        self.mark_section_and_neighbors_affected(sec);
    }

    fn remove_section(&mut self, sec: SectionPos) {
        self.sections.remove(&sec);
        self.on_node_removed(sec);
        self.mark_section_and_neighbors_affected(sec);
    }

    fn create_data_layer(&self, sec: SectionPos) -> DataLayer {
        if let Some(queued) = self.queued.get(&sec) {
            return queued.clone();
        }
        if self.layer == Layer::Block {
            return DataLayer::empty();
        }
        let top = self.top_section_y(sec.column());
        if top != self.current_lowest_y && sec.y < top {
            let mut above = SectionPos {
                y: sec.y + 1,
                ..sec
            };
            loop {
                if let Some(data) = self.sections.get(&above) {
                    return data.repeat_first_layer();
                }
                above.y += 1;
                if above.y > top {
                    break;
                }
            }
        }
        if self.light_on_in_section(sec) {
            DataLayer::filled(15)
        } else {
            DataLayer::empty()
        }
    }

    fn on_node_added(&mut self, sec: SectionPos) {
        if self.layer != Layer::Sky {
            return;
        }
        if self.current_lowest_y > sec.y {
            self.current_lowest_y = sec.y;
        }
        let col = sec.column();
        if self.top_section_y(col) < sec.y + 1 {
            self.top_sections.insert(col, sec.y + 1);
        }
    }

    fn on_node_removed(&mut self, sec: SectionPos) {
        if self.layer != Layer::Sky {
            return;
        }
        let col = sec.column();
        if self.top_section_y(col) != sec.y + 1 {
            return;
        }
        let mut y = sec.y;
        let mut probe = sec;
        while !self.storing_light_for_section(probe) && self.has_light_data_at_or_below(y) {
            y -= 1;
            probe = SectionPos { y, ..sec };
        }
        if self.storing_light_for_section(probe) {
            self.top_sections.insert(col, y + 1);
        } else {
            self.top_sections.remove(&col);
        }
    }

    #[inline]
    pub fn top_section_y(&self, col: ColumnPos) -> i32 {
        self.top_sections
            .get(&col)
            .copied()
            .unwrap_or(self.current_lowest_y)
    }

    #[inline]
    pub fn bottom_section_y(&self) -> i32 {
        self.current_lowest_y
    }

    #[inline]
    pub fn has_light_data_at_or_below(&self, section_y: i32) -> bool {
        section_y >= self.current_lowest_y
    }

    #[inline]
    pub fn is_above_data(&self, sec: SectionPos) -> bool {
        let top = self.top_section_y(sec.column());
        top == self.current_lowest_y || sec.y >= top
    }
}

#[inline]
fn rel(v: i32) -> usize {
    (v & 15) as usize
}

#[inline]
fn around(v: i32) -> impl Iterator<Item = i32> {
    let low = (v & 15) == 0;
    let high = (v & 15) == 15;
    [Some(0), low.then_some(-1), high.then_some(1)]
        .into_iter()
        .flatten()
}
