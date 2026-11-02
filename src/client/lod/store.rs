use std::collections::HashSet;

use crate::platform::time::Instant;

use super::pos::{MAX_DETAIL, SectionPos, WIDTH};
use super::section::FullSection;

#[derive(Default)]
pub struct Store {
    sections: crate::lighting::FastMap<SectionPos, FullSection>,
    dirty: [HashSet<SectionPos>; MAX_DETAIL as usize + 1],
}

impl Store {
    pub fn get(&self, pos: SectionPos) -> Option<&FullSection> {
        self.sections.get(&pos)
    }

    pub fn has_data(&self, pos: SectionPos) -> bool {
        self.sections.get(&pos).is_some_and(|s| !s.is_empty)
    }

    pub fn clear(&mut self) {
        self.sections.clear();
        for d in &mut self.dirty {
            d.clear();
        }
    }

    pub fn write_chunk(
        &mut self,
        cx: i32,
        cz: i32,
        columns: &[Vec<u64>],
        changed: &mut HashSet<SectionPos>,
    ) -> bool {
        let pos = SectionPos::containing(0, cx * 16, cz * 16);
        let chunks_wide = WIDTH / 16;
        let (x0, z0) = (
            cx.rem_euclid(chunks_wide) * 16,
            cz.rem_euclid(chunks_wide) * 16,
        );
        let section = self
            .sections
            .entry(pos)
            .or_insert_with(|| FullSection::new(pos));
        let did = section.replace_block(x0, z0, 16, columns);
        if columns.iter().any(|c| !c.is_empty()) {
            section.is_empty = false;
        }
        if did {
            self.dirty[0].insert(pos);
            changed.insert(pos);
        }
        did
    }

    pub fn propagate(
        &mut self,
        is_air: impl Fn(u32) -> bool + Copy,
        changed: &mut HashSet<SectionPos>,
        deadline: Option<Instant>,
    ) {
        let mut rebuilt = 0usize;
        for detail in 0..MAX_DETAIL as usize {
            if self.dirty[detail].is_empty() {
                continue;
            }
            let batch: Vec<SectionPos> = self.dirty[detail].iter().copied().collect();
            for pos in batch {
                if rebuilt > 0 && deadline.is_some_and(|d| Instant::now() >= d) {
                    return;
                }
                rebuilt += 1;
                self.dirty[detail].remove(&pos);
                let parent_pos = pos.parent();
                let mut parent = self
                    .sections
                    .remove(&parent_pos)
                    .unwrap_or_else(|| FullSection::new(parent_pos));
                let did = match self.sections.get(&pos) {
                    Some(child) => parent.update_from_child(child, is_air),
                    None => false,
                };
                self.sections.insert(parent_pos, parent);
                if did {
                    self.dirty[detail + 1].insert(parent_pos);
                    changed.insert(parent_pos);
                }
            }
        }
        self.dirty[MAX_DETAIL as usize].clear();
    }

    pub fn is_settled(&self) -> bool {
        self.dirty.iter().all(HashSet::is_empty)
    }

    pub fn prune(&mut self, eye: (f64, f64), radius: f64) {
        self.sections
            .retain(|pos, _| nearest_distance(*pos, eye) <= radius);
        for d in &mut self.dirty {
            d.retain(|pos| nearest_distance(*pos, eye) <= radius);
        }
    }
}

pub fn nearest_distance(pos: SectionPos, eye: (f64, f64)) -> f64 {
    let (x0, z0) = (f64::from(pos.min_block_x()), f64::from(pos.min_block_z()));
    let w = f64::from(pos.block_width());
    let dx = (x0 - eye.0).max(0.0).max(eye.0 - (x0 + w));
    let dz = (z0 - eye.1).max(0.0).max(eye.1 - (z0 + w));
    (dx * dx + dz * dz).sqrt()
}
