use std::collections::{HashSet, VecDeque};

use super::props::{self, LightProps};
use super::sources::SkySources;
use super::storage::{Layer, LightStorage};
use super::{ColumnPos, Direction, MAX_LEVEL, SectionPos};

pub type Node = (i32, i32, i32);

pub mod queue_entry {
    use super::Direction;

    pub const ALL_DIRECTIONS: u32 = 1008;
    const FLAG_FROM_EMPTY_SHAPE: u32 = 1024;
    const FLAG_INCREASE_FROM_EMISSION: u32 = 2048;

    #[inline]
    const fn with_level(entry: u32, level: u8) -> u32 {
        (entry & !15) | (level as u32 & 15)
    }

    #[inline]
    const fn with_direction(entry: u32, dir: Direction) -> u32 {
        entry | (1 << (dir as u32 + 4))
    }

    #[inline]
    const fn without_direction(entry: u32, dir: Direction) -> u32 {
        entry & !(1 << (dir as u32 + 4))
    }

    pub const fn decrease_all_directions(old_level: u8) -> u32 {
        with_level(ALL_DIRECTIONS, old_level)
    }

    pub const fn decrease_skip_one_direction(old_level: u8, skip: Direction) -> u32 {
        with_level(without_direction(ALL_DIRECTIONS, skip), old_level)
    }

    pub const fn increase_light_from_emission(level: u8, from_empty_shape: bool) -> u32 {
        let mut e = ALL_DIRECTIONS | FLAG_INCREASE_FROM_EMISSION;
        if from_empty_shape {
            e |= FLAG_FROM_EMPTY_SHAPE;
        }
        with_level(e, level)
    }

    pub const fn increase_skip_one_direction(
        level: u8,
        from_empty_shape: bool,
        skip: Direction,
    ) -> u32 {
        let mut e = without_direction(ALL_DIRECTIONS, skip);
        if from_empty_shape {
            e |= FLAG_FROM_EMPTY_SHAPE;
        }
        with_level(e, level)
    }

    pub const fn increase_only_one_direction(
        level: u8,
        from_empty_shape: bool,
        dir: Direction,
    ) -> u32 {
        let mut e = 0;
        if from_empty_shape {
            e |= FLAG_FROM_EMPTY_SHAPE;
        }
        with_level(with_direction(e, dir), level)
    }

    pub const fn increase_sky_source_in_directions(
        down: bool,
        north: bool,
        south: bool,
        west: bool,
        east: bool,
    ) -> u32 {
        let mut e = with_level(0, 15);
        if down {
            e = with_direction(e, Direction::Down);
        }
        if north {
            e = with_direction(e, Direction::North);
        }
        if south {
            e = with_direction(e, Direction::South);
        }
        if west {
            e = with_direction(e, Direction::West);
        }
        if east {
            e = with_direction(e, Direction::East);
        }
        e
    }

    #[inline]
    pub const fn level(entry: u32) -> u8 {
        (entry & 15) as u8
    }

    #[inline]
    pub const fn is_from_empty_shape(entry: u32) -> bool {
        entry & FLAG_FROM_EMPTY_SHAPE != 0
    }

    #[inline]
    pub const fn is_increase_from_emission(entry: u32) -> bool {
        entry & FLAG_INCREASE_FROM_EMISSION != 0
    }

    #[inline]
    pub const fn should_propagate(entry: u32, dir: Direction) -> bool {
        entry & (1 << (dir as u32 + 4)) != 0
    }
}

use queue_entry as qe;

const PULL_LIGHT_IN: u32 = qe::decrease_all_directions(1);

const REMOVE_SKY_SOURCE: u32 = qe::decrease_skip_one_direction(15, Direction::Up);
const ADD_SKY_SOURCE: u32 = qe::increase_skip_one_direction(15, false, Direction::Up);
const REMOVE_TOP_SKY_SOURCE: u32 = qe::decrease_all_directions(15);

pub trait LightWorld {
    fn block_state(&self, x: i32, y: i32, z: i32) -> azalea::block::BlockState;

    fn section_may_emit(&self, _sec: crate::lighting::SectionPos) -> bool {
        true
    }
}

#[inline]
fn state_props<W: LightWorld>(world: &W, x: i32, y: i32, z: i32) -> &'static LightProps {
    props::of(world.block_state(x, y, z))
}

pub struct LightEngine {
    pub storage: LightStorage,
    nodes_to_check: HashSet<Node>,
    increase: VecDeque<(Node, u32)>,
    decrease: VecDeque<(Node, u32)>,
}

impl LightEngine {
    pub fn new(layer: Layer) -> Self {
        Self {
            storage: LightStorage::new(layer),
            nodes_to_check: HashSet::new(),
            increase: VecDeque::new(),
            decrease: VecDeque::new(),
        }
    }

    pub fn bytes(&self) -> (u64, usize) {
        let (storage, sections) = self.storage.bytes();
        let node = std::mem::size_of::<(Node, u32)>() as u64;
        let queues = (self.increase.capacity() + self.decrease.capacity()) as u64 * node;
        let pending = (self.nodes_to_check.len() * std::mem::size_of::<Node>()) as u64;
        (storage + queues + pending, sections)
    }

    pub fn check_block(&mut self, x: i32, y: i32, z: i32) {
        self.nodes_to_check.insert((x, y, z));
    }

    pub fn update_section_status(&mut self, sec: SectionPos, section_empty: bool) {
        self.storage.update_section_status(sec, section_empty);
    }

    fn enqueue_increase(&mut self, node: Node, data: u32) {
        self.increase.push_back((node, data));
    }

    fn enqueue_decrease(&mut self, node: Node, data: u32) {
        self.decrease.push_back((node, data));
    }

    pub fn run_light_updates<W: LightWorld>(
        &mut self,
        world: &W,
        sources: Option<&SkySources>,
    ) -> usize {
        for node in std::mem::take(&mut self.nodes_to_check) {
            match self.storage.layer() {
                Layer::Block => self.check_node_block(world, node),
                Layer::Sky => self.check_node_sky(world, sources, node),
            }
        }

        let mut count = 0;
        while let Some((node, data)) = self.decrease.pop_front() {
            count += 1;
            match self.storage.layer() {
                Layer::Block => self.propagate_decrease_block(world, node, data),
                Layer::Sky => self.propagate_decrease_sky(world, node, data),
            }
        }
        while let Some((node, data)) = self.increase.pop_front() {
            count += 1;
            let mut from_level = self.storage.get_stored_level(node.0, node.1, node.2);
            let target = qe::level(data);
            if qe::is_increase_from_emission(data) && from_level < target {
                self.storage
                    .set_stored_level(node.0, node.1, node.2, target);
                from_level = target;
            }
            if from_level == target {
                match self.storage.layer() {
                    Layer::Block => self.propagate_increase_block(world, node, data, from_level),
                    Layer::Sky => self.propagate_increase_sky(world, node, data, from_level),
                }
            }
        }
        count
    }

    fn emission(&self, node: Node, p: &LightProps) -> u8 {
        let sec = SectionPos::of_block(node.0, node.1, node.2);
        if p.emission > 0 && self.storage.light_on_in_section(sec) {
            p.emission
        } else {
            0
        }
    }

    fn check_node_block<W: LightWorld>(&mut self, world: &W, node: Node) {
        let (x, y, z) = node;
        if !self
            .storage
            .storing_light_for_section(SectionPos::of_block(x, y, z))
        {
            return;
        }
        let p = state_props(world, x, y, z);
        let emission = self.emission(node, p);
        let old_level = self.storage.get_stored_level(x, y, z);
        if emission < old_level {
            self.storage.set_stored_level(x, y, z, 0);
            self.enqueue_decrease(node, qe::decrease_all_directions(old_level));
        } else {
            self.enqueue_decrease(node, PULL_LIGHT_IN);
        }
        if emission > 0 {
            self.enqueue_increase(
                node,
                qe::increase_light_from_emission(emission, p.empty_shape),
            );
        }
    }

    fn propagate_increase_block<W: LightWorld>(
        &mut self,
        world: &W,
        from: Node,
        data: u32,
        from_level: u8,
    ) {
        let mut from_props: Option<&LightProps> = None;
        for dir in Direction::ALL {
            if !qe::should_propagate(data, dir) {
                continue;
            }
            let Some((to, to_props, new_level)) = self.step_increase(world, from, from_level, dir)
            else {
                continue;
            };
            let fp = *from_props.get_or_insert_with(|| {
                if qe::is_from_empty_shape(data) {
                    props::air()
                } else {
                    state_props(world, from.0, from.1, from.2)
                }
            });
            if props::shape_occludes(fp, to_props, dir) {
                continue;
            }
            self.storage.set_stored_level(to.0, to.1, to.2, new_level);
            if new_level > 1 {
                self.enqueue_increase(
                    to,
                    qe::increase_skip_one_direction(
                        new_level,
                        to_props.empty_shape,
                        dir.opposite(),
                    ),
                );
            }
        }
    }

    fn step_increase<'w, W: LightWorld>(
        &self,
        world: &'w W,
        from: Node,
        from_level: u8,
        dir: Direction,
    ) -> Option<(Node, &'static LightProps, u8)> {
        let (dx, dy, dz) = dir.offset();
        let to = (from.0 + dx, from.1 + dy, from.2 + dz);
        if !self
            .storage
            .storing_light_for_section(SectionPos::of_block(to.0, to.1, to.2))
        {
            return None;
        }
        let to_level = self.storage.get_stored_level(to.0, to.1, to.2);
        if from_level.saturating_sub(1) <= to_level {
            return None;
        }
        let to_props = state_props(world, to.0, to.1, to.2);
        let new_level = from_level.saturating_sub(to_props.opacity());
        if new_level <= to_level {
            return None;
        }
        let _ = world;
        Some((to, to_props, new_level))
    }

    fn propagate_decrease_block<W: LightWorld>(&mut self, world: &W, from: Node, data: u32) {
        let old_from_level = qe::level(data);
        for dir in Direction::ALL {
            if !qe::should_propagate(data, dir) {
                continue;
            }
            let (dx, dy, dz) = dir.offset();
            let to = (from.0 + dx, from.1 + dy, from.2 + dz);
            if !self
                .storage
                .storing_light_for_section(SectionPos::of_block(to.0, to.1, to.2))
            {
                continue;
            }
            let to_level = self.storage.get_stored_level(to.0, to.1, to.2);
            if to_level == 0 {
                continue;
            }
            if to_level <= old_from_level.saturating_sub(1) {
                let to_props = state_props(world, to.0, to.1, to.2);
                let to_emission = self.emission(to, to_props);
                self.storage.set_stored_level(to.0, to.1, to.2, 0);
                if to_emission < to_level {
                    self.enqueue_decrease(
                        to,
                        qe::decrease_skip_one_direction(to_level, dir.opposite()),
                    );
                }
                if to_emission > 0 {
                    self.enqueue_increase(
                        to,
                        qe::increase_light_from_emission(to_emission, to_props.empty_shape),
                    );
                }
            } else {
                self.enqueue_increase(
                    to,
                    qe::increase_only_one_direction(to_level, false, dir.opposite()),
                );
            }
        }
    }

    pub fn propagate_block_light_sources<W: LightWorld>(
        &mut self,
        world: &W,
        col: ColumnPos,
        sec_lo: i32,
        sec_hi: i32,
    ) {
        self.storage.set_light_enabled(col, true);
        for sy in sec_lo..sec_hi {
            let sec = col.section(sy);
            if !self.storage.storing_light_for_section(sec) {
                continue;
            }
            if !world.section_may_emit(sec) {
                continue;
            }
            let base_y = sy << 4;
            for ly in 0..16 {
                for lz in 0..16 {
                    for lx in 0..16 {
                        let (x, y, z) = (col.x * 16 + lx, base_y + ly, col.z * 16 + lz);
                        let p = state_props(world, x, y, z);
                        if p.emission == 0 {
                            continue;
                        }
                        self.enqueue_increase(
                            (x, y, z),
                            qe::increase_light_from_emission(p.emission, p.empty_shape),
                        );
                    }
                }
            }
        }
    }

    fn lowest_source_y(sources: Option<&SkySources>, x: i32, z: i32, default: i32) -> i32 {
        match sources.and_then(|s| s.column(x >> 4, z >> 4)) {
            Some(c) => c.lowest_source_y((x & 15) as usize, (z & 15) as usize),
            None => default,
        }
    }

    fn check_node_sky<W: LightWorld>(
        &mut self,
        world: &W,
        sources: Option<&SkySources>,
        node: Node,
    ) {
        let (x, y, z) = node;
        let sec = SectionPos::of_block(x, y, z);
        let lowest = if self.storage.light_on_in_section(sec) {
            Self::lowest_source_y(sources, x, z, i32::MAX)
        } else {
            i32::MAX
        };
        if lowest != i32::MAX {
            self.update_sources_in_column(sources, x, z, lowest);
        }
        if !self.storage.storing_light_for_section(sec) {
            return;
        }
        let _ = world;
        if y >= lowest {
            self.enqueue_decrease(node, REMOVE_SKY_SOURCE);
            self.enqueue_increase(node, ADD_SKY_SOURCE);
        } else {
            let old_level = self.storage.get_stored_level(x, y, z);
            if old_level > 0 {
                self.storage.set_stored_level(x, y, z, 0);
                self.enqueue_decrease(node, qe::decrease_all_directions(old_level));
            } else {
                self.enqueue_decrease(node, PULL_LIGHT_IN);
            }
        }
    }

    fn update_sources_in_column(
        &mut self,
        sources: Option<&SkySources>,
        x: i32,
        z: i32,
        lowest: i32,
    ) {
        let world_bottom = self.storage.bottom_section_y() << 4;
        self.remove_sources_below(x, z, lowest, world_bottom);
        self.add_sources_above(sources, x, z, lowest, world_bottom);
    }

    fn remove_sources_below(&mut self, x: i32, z: i32, lowest: i32, world_bottom: i32) {
        if lowest <= world_bottom {
            return;
        }
        let start_y = lowest - 1;
        let mut sy = start_y >> 4;
        while self.storage.has_light_data_at_or_below(sy) {
            let sec = SectionPos {
                x: x >> 4,
                y: sy,
                z: z >> 4,
            };
            if self.storage.storing_light_for_section(sec) {
                let sec_bottom = sy << 4;
                let sec_top = sec_bottom + 15;
                let mut y = sec_top.min(start_y);
                while y >= sec_bottom {
                    if self.storage.get_stored_level(x, y, z) != MAX_LEVEL {
                        return;
                    }
                    self.storage.set_stored_level(x, y, z, 0);
                    let data = if y == lowest - 1 {
                        REMOVE_TOP_SKY_SOURCE
                    } else {
                        REMOVE_SKY_SOURCE
                    };
                    self.enqueue_decrease((x, y, z), data);
                    y -= 1;
                }
            }
            sy -= 1;
        }
    }

    fn add_sources_above(
        &mut self,
        sources: Option<&SkySources>,
        x: i32,
        z: i32,
        lowest: i32,
        world_bottom: i32,
    ) {
        let neighbor_lowest = Self::lowest_source_y(sources, x - 1, z, i32::MIN)
            .max(Self::lowest_source_y(sources, x + 1, z, i32::MIN))
            .max(Self::lowest_source_y(sources, x, z - 1, i32::MIN))
            .max(Self::lowest_source_y(sources, x, z + 1, i32::MIN));
        let start_y = lowest.max(world_bottom);
        let mut sec = SectionPos {
            x: x >> 4,
            y: start_y >> 4,
            z: z >> 4,
        };
        while !self.storage.is_above_data(sec) {
            if self.storage.storing_light_for_section(sec) {
                let sec_bottom = sec.min_block_y();
                let sec_top = sec_bottom + 15;
                let mut y = sec_bottom.max(start_y);
                while y <= sec_top {
                    if self.storage.get_stored_level(x, y, z) == MAX_LEVEL {
                        return;
                    }
                    self.storage.set_stored_level(x, y, z, MAX_LEVEL);
                    if y < neighbor_lowest || y == lowest {
                        self.enqueue_increase((x, y, z), ADD_SKY_SOURCE);
                    }
                    y += 1;
                }
            }
            sec.y += 1;
        }
    }

    fn propagate_increase_sky<W: LightWorld>(
        &mut self,
        world: &W,
        from: Node,
        data: u32,
        from_level: u8,
    ) {
        let mut from_props: Option<&LightProps> = None;
        let empty_below = self.count_empty_sections_below_if_at_border(from);
        for dir in Direction::ALL {
            if !qe::should_propagate(data, dir) {
                continue;
            }
            let Some((to, to_props, new_level)) = self.step_increase(world, from, from_level, dir)
            else {
                continue;
            };
            let fp = *from_props.get_or_insert_with(|| {
                if qe::is_from_empty_shape(data) {
                    props::air()
                } else {
                    state_props(world, from.0, from.1, from.2)
                }
            });
            if props::shape_occludes(fp, to_props, dir) {
                continue;
            }
            self.storage.set_stored_level(to.0, to.1, to.2, new_level);
            if new_level > 1 {
                self.enqueue_increase(
                    to,
                    qe::increase_skip_one_direction(
                        new_level,
                        to_props.empty_shape,
                        dir.opposite(),
                    ),
                );
            }
            self.propagate_from_empty_sections(to, dir, new_level, true, empty_below);
        }
    }

    fn propagate_decrease_sky<W: LightWorld>(&mut self, world: &W, from: Node, data: u32) {
        let _ = world;
        let empty_below = self.count_empty_sections_below_if_at_border(from);
        let old_from_level = qe::level(data);
        for dir in Direction::ALL {
            if !qe::should_propagate(data, dir) {
                continue;
            }
            let (dx, dy, dz) = dir.offset();
            let to = (from.0 + dx, from.1 + dy, from.2 + dz);
            if !self
                .storage
                .storing_light_for_section(SectionPos::of_block(to.0, to.1, to.2))
            {
                continue;
            }
            let to_level = self.storage.get_stored_level(to.0, to.1, to.2);
            if to_level == 0 {
                continue;
            }
            if to_level <= old_from_level.saturating_sub(1) {
                self.storage.set_stored_level(to.0, to.1, to.2, 0);
                self.enqueue_decrease(
                    to,
                    qe::decrease_skip_one_direction(to_level, dir.opposite()),
                );
                self.propagate_from_empty_sections(to, dir, to_level, false, empty_below);
            } else {
                self.enqueue_increase(
                    to,
                    qe::increase_only_one_direction(to_level, false, dir.opposite()),
                );
            }
        }
    }

    fn count_empty_sections_below_if_at_border(&self, node: Node) -> i32 {
        let (x, y, z) = node;
        if y & 15 != 0 {
            return 0;
        }
        let (lx, lz) = (x & 15, z & 15);
        if lx != 0 && lx != 15 && lz != 0 && lz != 15 {
            return 0;
        }
        let (sx, sy, sz) = (x >> 4, y >> 4, z >> 4);
        let mut empty = 0;
        while !self.storage.storing_light_for_section(SectionPos {
            x: sx,
            y: sy - empty - 1,
            z: sz,
        }) && self.storage.has_light_data_at_or_below(sy - empty - 1)
        {
            empty += 1;
        }
        empty
    }

    fn propagate_from_empty_sections(
        &mut self,
        to: Node,
        dir: Direction,
        level: u8,
        increase: bool,
        empty_below: i32,
    ) {
        if empty_below == 0 {
            return;
        }
        let (x, y, z) = to;
        let crossed = match dir {
            Direction::North => (z & 15) == 15,
            Direction::South => (z & 15) == 0,
            Direction::West => (x & 15) == 15,
            Direction::East => (x & 15) == 0,
            _ => false,
        };
        if !crossed {
            return;
        }
        let (sx, sz) = (x >> 4, z >> 4);
        let mut sy = (y >> 4) - 1;
        let bottom = sy - empty_below + 1;
        while sy >= bottom {
            if self.storage.storing_light_for_section(SectionPos {
                x: sx,
                y: sy,
                z: sz,
            }) {
                let sec_min_y = sy << 4;
                for ly in (0..16).rev() {
                    let node = (x, sec_min_y + ly, z);
                    if increase {
                        self.storage.set_stored_level(node.0, node.1, node.2, level);
                        if level > 1 {
                            self.enqueue_increase(
                                node,
                                qe::increase_skip_one_direction(level, true, dir.opposite()),
                            );
                        }
                    } else {
                        self.storage.set_stored_level(node.0, node.1, node.2, 0);
                        self.enqueue_decrease(
                            node,
                            qe::decrease_skip_one_direction(level, dir.opposite()),
                        );
                    }
                }
            }
            sy -= 1;
        }
    }

    pub fn set_sky_light_enabled(&mut self, sources: &SkySources, col: ColumnPos) {
        self.storage.set_light_enabled(col, true);
        let highest = sources.highest_lowest_source_y(col);
        if highest == super::sources::NEGATIVE_INFINITY {
            return;
        }
        let lowest_fully_source_section = ((highest - 1) >> 4) + 1;
        let top = self.storage.top_section_y(col);
        let bottom = self
            .storage
            .bottom_section_y()
            .max(lowest_fully_source_section);
        for sy in (bottom..top).rev() {
            if let Some(layer) = self.storage.data_layer_to_write(col.section(sy)) {
                if layer.is_empty() {
                    layer.fill(MAX_LEVEL);
                }
            }
        }
    }

    pub fn propagate_sky_light_sources(&mut self, sources: &SkySources, col: ColumnPos) {
        self.storage.set_light_enabled(col, true);
        let here = sources.column_or_empty(col.x, col.z);
        let north = sources.column_or_empty(col.x, col.z - 1);
        let south = sources.column_or_empty(col.x, col.z + 1);
        let west = sources.column_or_empty(col.x - 1, col.z);
        let east = sources.column_or_empty(col.x + 1, col.z);
        let top = self.storage.top_section_y(col);
        let bottom = self.storage.bottom_section_y();
        let (min_x, min_z) = (col.x * 16, col.z * 16);

        for sy in (bottom..top).rev() {
            let sec = col.section(sy);
            if self.storage.data_layer(sec).is_none() {
                continue;
            }
            let sec_min_y = sy << 4;
            let sec_max_y = sec_min_y + 15;
            let mut sources_below = false;
            let mut seeds: Vec<(Node, u32)> = Vec::new();

            for z in 0..16usize {
                for x in 0..16usize {
                    let lowest = here.lowest_source_y(x, z);
                    if lowest > sec_max_y {
                        continue;
                    }
                    let n = if z == 0 {
                        north.lowest_source_y(x, 15)
                    } else {
                        here.lowest_source_y(x, z - 1)
                    };
                    let s = if z == 15 {
                        south.lowest_source_y(x, 0)
                    } else {
                        here.lowest_source_y(x, z + 1)
                    };
                    let w = if x == 0 {
                        west.lowest_source_y(15, z)
                    } else {
                        here.lowest_source_y(x - 1, z)
                    };
                    let e = if x == 15 {
                        east.lowest_source_y(0, z)
                    } else {
                        here.lowest_source_y(x + 1, z)
                    };
                    let neighbor_lowest = n.max(s).max(w).max(e);

                    let layer = self
                        .storage
                        .data_layer_to_write(sec)
                        .expect("checked above");
                    let mut y = sec_max_y;
                    while y >= sec_min_y.max(lowest) {
                        layer.set(x, (y & 15) as usize, z, MAX_LEVEL);
                        if y == lowest || y < neighbor_lowest {
                            seeds.push((
                                (min_x + x as i32, y, min_z + z as i32),
                                qe::increase_sky_source_in_directions(
                                    y == lowest,
                                    y < n,
                                    y < s,
                                    y < w,
                                    y < e,
                                ),
                            ));
                        }
                        y -= 1;
                    }
                    if lowest < sec_min_y {
                        sources_below = true;
                    }
                }
            }
            for (node, data) in seeds {
                self.enqueue_increase(node, data);
            }
            if !sources_below {
                break;
            }
        }
    }
}
