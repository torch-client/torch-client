use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use azalea_core::position::ChunkPos;
use azalea_world::palette::Palette;

use super::table::{Merged, NONE_ENTRY, row_of, slot_of};
use crate::modules::list::NO_ROW;

const SWAR_MAX: u32 = 4;

const SECTION: usize = 4096;

pub const FO: u32 = 1 << 0;
pub const BA: u32 = 1 << 1;
pub const RI: u32 = 1 << 2;
pub const LE: u32 = 1 << 3;
pub const TO: u32 = 1 << 4;
pub const BO: u32 = 1 << 5;
pub const FO_RI: u32 = 1 << 6;
pub const FO_LE: u32 = 1 << 7;
pub const BA_RI: u32 = 1 << 8;
pub const BA_LE: u32 = 1 << 9;
pub const TO_FO: u32 = 1 << 10;
pub const TO_BA: u32 = 1 << 11;
pub const TO_RI: u32 = 1 << 12;
pub const TO_LE: u32 = 1 << 13;
pub const BO_FO: u32 = 1 << 14;
pub const BO_BA: u32 = 1 << 15;
pub const BO_RI: u32 = 1 << 16;
pub const BO_LE: u32 = 1 << 17;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Found {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub neigh: u32,
    pub slot: u16,
}

pub struct Layout {
    bits: u32,
    per_word: usize,
}

impl Layout {
    pub fn of(storage: &azalea_world::BitStorage) -> Option<Layout> {
        static VERIFIED: AtomicU64 = AtomicU64::new(u64::MAX);

        let (size, words) = (storage.size(), storage.data.len());
        if size == 0 || words == 0 {
            return None;
        }
        for bits in 1..=32u32 {
            let per_word = 64 / bits as usize;
            if size.div_ceil(per_word) != words {
                continue;
            }
            let seen = (size as u64) << 40 | (words as u64) << 8 | bits as u64;
            if VERIFIED.load(Relaxed) == seen {
                return Some(Layout { bits, per_word });
            }
            let mask = (1u64 << bits) - 1;
            let ok = [0usize, 1, per_word - 1, per_word]
                .into_iter()
                .filter(|&i| i < size)
                .all(|i| {
                    let (w, j) = (i / per_word, i % per_word);
                    (storage.data[w] >> (j as u32 * bits)) & mask == storage.get(i)
                });
            if ok {
                VERIFIED.store(seen, Relaxed);
                return Some(Layout { bits, per_word });
            }
        }
        None
    }

    fn ones(&self) -> u64 {
        let mut o = 0u64;
        for j in 0..self.per_word {
            o |= 1 << (j as u32 * self.bits);
        }
        o
    }
}

#[inline]
fn swar_word(
    word: u64,
    lay: &Layout,
    ones: u64,
    mut candidates: u64,
    base: usize,
    out: &mut impl FnMut(usize, u64),
) {
    let high = ones << (lay.bits - 1);
    let low = high - ones;
    while candidates != 0 {
        let v = candidates.trailing_zeros() as u64;
        candidates &= candidates - 1;
        let x = word ^ (v.wrapping_mul(ones));
        let mut hits = !(((x & low) + low) | x) & high;
        while hits != 0 {
            let j = hits.trailing_zeros() / lay.bits;
            hits &= hits - 1;
            let i = base + j as usize;
            if i < SECTION {
                out(i, v);
            }
        }
    }
}

#[derive(Default)]
pub struct Scratch {
    rows: Vec<u16>,
}

pub fn scan(
    chunks: &[(
        ChunkPos,
        std::sync::Arc<parking_lot::RwLock<azalea_world::Chunk>>,
    )],
    table: &Merged,
    min_y: i32,
    centre: [i32; 3],
    cap: usize,
    scratch: &mut Scratch,
    out: &mut Vec<Found>,
) -> bool {
    crate::prof_span!("modules:esp_scan");
    out.clear();
    let range = table.max_range;
    let mut full = false;

    for (pos, chunk) in chunks {
        let chunk = chunk.read();
        let (bx, bz) = (pos.x * 16, pos.z * 16);
        let sections = chunk.sections.len();
        if scratch.rows.len() < sections * SECTION {
            scratch.rows.resize(sections * SECTION, NO_ROW);
        }
        let column_start = out.len();

        for (si, section) in chunk.sections.iter().enumerate() {
            let y0 = min_y + (si as i32) * 16;
            if y0 + 16 < centre[1] - range || y0 > centre[1] + range {
                continue;
            }
            if section.block_count == 0 {
                continue;
            }
            let states = &section.states;

            let mut entries = [NONE_ENTRY; 64];
            let (mask, wide) = match &states.palette {
                Palette::SingleValue(v) => {
                    entries[0] = table.entry(v.id() as u32);
                    ((entries[0] != NONE_ENTRY) as u64, false)
                }
                Palette::Linear(v) | Palette::Hashmap(v) if v.len() <= 64 => {
                    let mut m = 0u64;
                    for (i, st) in v.iter().enumerate() {
                        entries[i] = table.entry(st.id() as u32);
                        if entries[i] != NONE_ENTRY {
                            m |= 1 << i;
                        }
                    }
                    (m, false)
                }
                _ => (u64::MAX, true),
            };
            if mask == 0 {
                continue;
            }

            let mut emit = |i: usize, entry: u32| {
                if out.len() >= cap {
                    full = true;
                    return;
                }
                let (lx, ly, lz) = (i & 15, i >> 8, (i >> 4) & 15);
                let (x, y, z) = (bx + lx as i32, y0 + ly as i32, bz + lz as i32);
                let slot = slot_of(entry);
                let (dx, dy, dz) = (x - centre[0], y - centre[1], z - centre[2]);
                let d2 = dx * dx + dy * dy + dz * dz;
                if d2 as u32 > table.slots[slot as usize].range2 {
                    return;
                }
                scratch.rows[si * SECTION + i] = row_of(entry);
                out.push(Found {
                    x,
                    y,
                    z,
                    neigh: 0,
                    slot,
                });
            };

            let storage = &states.storage;
            let Some(lay) = Layout::of(storage) else {
                if mask & 1 != 0 {
                    for i in 0..SECTION {
                        emit(i, entries[0]);
                    }
                }
                continue;
            };

            let entry_mask = (1u64 << lay.bits) - 1;
            let ones = lay.ones();

            if !wide && mask.count_ones() <= SWAR_MAX {
                for (w, &word) in storage.data.iter().enumerate() {
                    swar_word(word, &lay, ones, mask, w * lay.per_word, &mut |i, v| {
                        emit(i, entries[v as usize])
                    });
                }
            } else {
                for (w, &word) in storage.data.iter().enumerate() {
                    let base = w * lay.per_word;
                    if base >= SECTION {
                        break;
                    }
                    for j in 0..lay.per_word {
                        let i = base + j;
                        if i >= SECTION {
                            break;
                        }
                        let v = (word >> (j as u32 * lay.bits)) & entry_mask;
                        let e = if wide {
                            table.entry(v as u32)
                        } else if v < 64 && mask >> v & 1 != 0 {
                            entries[v as usize]
                        } else {
                            NONE_ENTRY
                        };
                        if e != NONE_ENTRY {
                            emit(i, e);
                        }
                    }
                }
            }
        }

        neighbours(&mut out[column_start..], &scratch.rows, min_y, sections);
        for f in &out[column_start..] {
            let (lx, lz) = ((f.x - bx) as usize, (f.z - bz) as usize);
            let gy = (f.y - min_y) as usize;
            scratch.rows[gy * 256 + lz * 16 + lx] = NO_ROW;
        }
        if full {
            break;
        }
    }
    full
}

fn neighbours(found: &mut [Found], rows: &[u16], min_y: i32, sections: usize) {
    let height = (sections * 16) as i32;
    for f in found.iter_mut() {
        let (lx, lz) = (f.x.rem_euclid(16), f.z.rem_euclid(16));
        let gy = f.y - min_y;
        let mine = rows[(gy * 256 + lz * 16 + lx) as usize];
        let same = |dx: i32, dy: i32, dz: i32| {
            let (x, y, z) = (lx + dx, gy + dy, lz + dz);
            if !(0..16).contains(&x) || !(0..16).contains(&z) || !(0..height).contains(&y) {
                return false;
            }
            rows[(y * 256 + z * 16 + x) as usize] == mine
        };
        let mut n = 0;
        for (bit, dx, dy, dz) in [
            (FO, 0, 0, 1),
            (BA, 0, 0, -1),
            (RI, 1, 0, 0),
            (LE, -1, 0, 0),
            (TO, 0, 1, 0),
            (BO, 0, -1, 0),
            (FO_RI, 1, 0, 1),
            (FO_LE, -1, 0, 1),
            (BA_RI, 1, 0, -1),
            (BA_LE, -1, 0, -1),
            (TO_FO, 0, 1, 1),
            (TO_BA, 0, 1, -1),
            (TO_RI, 1, 1, 0),
            (TO_LE, -1, 1, 0),
            (BO_FO, 0, -1, 1),
            (BO_BA, 0, -1, -1),
            (BO_RI, 1, -1, 0),
            (BO_LE, -1, -1, 0),
        ] {
            if same(dx, dy, dz) {
                n |= bit;
            }
        }
        f.neigh = n;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn swar(word: u64, bits: u32, candidates: u64) -> Vec<(usize, u64)> {
        let lay = Layout {
            bits,
            per_word: 64 / bits as usize,
        };
        let ones = lay.ones();
        let mut hits = Vec::new();
        swar_word(word, &lay, ones, candidates, 0, &mut |i, v| {
            hits.push((i, v))
        });
        hits.sort_unstable();
        hits
    }

    #[test]
    fn swar_finds_each_match() {
        let mut word = 0u64;
        for j in 0..16 {
            let v = if [0, 5, 15].contains(&j) { 2u64 } else { 7 };
            word |= v << (j * 4);
        }
        assert_eq!(swar(word, 4, 1 << 2), vec![(0, 2), (5, 2), (15, 2)]);
    }

    #[test]
    fn swar_does_not_bleed_along_a_row() {
        let mut word = 2u64;
        for j in 1..16 {
            word |= 3 << (j * 4);
        }
        assert_eq!(swar(word, 4, 1 << 2), vec![(0, 2)]);
    }

    #[test]
    fn swar_ignores_the_padding() {
        let mut word = 0u64;
        for j in 1..12 {
            word |= 1 << (j * 5);
        }
        assert_eq!(swar(word, 5, 1), vec![(0, 0)]);
    }

    fn found(x: i32, y: i32, z: i32) -> Found {
        Found {
            x,
            y,
            z,
            neigh: 0,
            slot: 0,
        }
    }

    #[test]
    fn a_pair_shares_one_face() {
        let mut rows = vec![NO_ROW; 16 * 4096];
        let idx = |x: i32, y: i32, z: i32| (y * 256 + z * 16 + x) as usize;
        rows[idx(1, 5, 1)] = 7;
        let mut one = [found(1, 5, 1)];
        neighbours(&mut one, &rows, 0, 16);
        assert_eq!(one[0].neigh, 0, "a lone block has no neighbours");

        rows[idx(2, 5, 1)] = 7;
        let mut two = [found(1, 5, 1), found(2, 5, 1)];
        neighbours(&mut two, &rows, 0, 16);
        assert_eq!(two[0].neigh, RI);
        assert_eq!(two[1].neigh, LE);
    }

    #[test]
    fn a_different_row_is_not_a_neighbour() {
        let mut rows = vec![NO_ROW; 16 * 4096];
        let idx = |x: i32, y: i32, z: i32| (y * 256 + z * 16 + x) as usize;
        rows[idx(1, 5, 1)] = 7;
        rows[idx(2, 5, 1)] = 8;
        let mut two = [found(1, 5, 1), found(2, 5, 1)];
        neighbours(&mut two, &rows, 0, 16);
        assert_eq!(two[0].neigh, 0);
        assert_eq!(two[1].neigh, 0);
    }

    #[test]
    fn a_section_boundary_is_not_a_seam() {
        let mut rows = vec![NO_ROW; 16 * 4096];
        let idx = |x: i32, y: i32, z: i32| (y * 256 + z * 16 + x) as usize;
        rows[idx(3, 15, 3)] = 4;
        rows[idx(3, 16, 3)] = 4;
        let mut two = [found(3, 15 - 64, 3), found(3, 16 - 64, 3)];
        neighbours(&mut two, &rows, -64, 16);
        assert_eq!(two[0].neigh, TO);
        assert_eq!(two[1].neigh, BO);
    }

    #[test]
    fn a_missing_diagonal_stays_clear() {
        let mut rows = vec![NO_ROW; 16 * 4096];
        let idx = |x: i32, y: i32, z: i32| (y * 256 + z * 16 + x) as usize;
        for (x, y) in [(1, 5), (2, 5), (1, 6)] {
            rows[idx(x, y, 1)] = 2;
        }
        let mut one = [found(1, 5, 1)];
        neighbours(&mut one, &rows, 0, 16);
        assert_eq!(one[0].neigh & RI, RI);
        assert_eq!(one[0].neigh & TO, TO);
        assert_eq!(one[0].neigh & TO_RI, 0, "the corner above right is empty");
    }
}
