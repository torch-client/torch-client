use std::sync::Arc;

use parking_lot::Mutex;

use crate::modules::list::{BitList, NO_ROW};
use crate::modules::registry::{Id, ore_esp, storage_esp, xray};
use crate::modules::store;

pub const NONE_ENTRY: u32 = u32::MAX;

#[inline]
pub fn row_of(entry: u32) -> u16 {
    (entry >> 16) as u16
}

#[inline]
pub fn slot_of(entry: u32) -> u16 {
    entry as u16
}

#[inline]
fn entry(row: u16, slot: u16) -> u32 {
    (row as u32) << 16 | slot as u32
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Slot {
    pub rgb: u32,
    pub alpha: u8,
    pub lines: bool,
    pub sides: bool,
    pub range2: u32,
}

pub struct Merged {
    pub of_state: Box<[u32]>,
    pub slots: Vec<Slot>,
    pub max_range: i32,
    pub any: bool,
}

impl Merged {
    #[inline]
    pub fn entry(&self, state_id: u32) -> u32 {
        self.of_state
            .get(state_id as usize)
            .copied()
            .unwrap_or(NONE_ENTRY)
    }
}

const SHAPE_LINES: u8 = 0;
const SHAPE_SIDES: u8 = 1;

struct Source {
    list: &'static BitList,
    range: i32,
    alpha: u8,
    lines: bool,
    sides: bool,
}

fn key(sources: &[Option<Source>; 3]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x1000_0000_01b3);
    };
    for s in sources {
        match s {
            None => eat(0),
            Some(s) => {
                eat(s.list.generation() as u64 | 1 << 32);
                eat(s.range as u64);
                eat(s.alpha as u64 | (s.lines as u64) << 8 | (s.sides as u64) << 9);
            }
        }
    }
    h
}

fn sources() -> [Option<Source>; 3] {
    let s = store();
    let one = |id: Id, list: &'static BitList, range: usize, shape: usize, fill: usize| {
        if !s.enabled(id) {
            return None;
        }
        let mode = s.choice(id, shape);
        Some(Source {
            list,
            range: s.num(id, range).max(0.0) as i32,
            alpha: if mode == SHAPE_LINES {
                0
            } else {
                s.num(id, fill).clamp(0.0, 255.0) as u8
            },
            lines: mode != SHAPE_SIDES,
            sides: mode != SHAPE_LINES,
        })
    };
    [
        one(
            Id::OreEsp,
            &super::blocks::ORES,
            ore_esp::RANGE,
            ore_esp::SHAPE,
            ore_esp::FILL,
        ),
        one(
            Id::StorageEsp,
            &super::blocks::STORAGE,
            storage_esp::RANGE,
            storage_esp::SHAPE,
            storage_esp::FILL,
        ),
        one(
            Id::Xray,
            &super::blocks::XRAY,
            xray::RANGE,
            xray::SHAPE,
            xray::FILL,
        ),
    ]
}

fn fold(sources: &[Option<Source>; 3]) -> Merged {
    let mut slots: Vec<Slot> = Vec::new();
    let mut max_range = 0;
    let mut any = false;

    let mut of_row: [Vec<u16>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut of_state = vec![NONE_ENTRY; azalea::block::BlockState::MAX_STATE as usize + 1];

    for (m, source) in sources.iter().enumerate() {
        let Some(source) = source else { continue };
        any = true;
        max_range = max_range.max(source.range);
        of_row[m] = vec![NO_ROW; source.list.count()];
        let index = source.list.index();

        for (state_id, dst) in of_state.iter_mut().enumerate() {
            if *dst != NONE_ENTRY {
                continue;
            }
            let Some(&row) = index.get(state_id) else {
                continue;
            };
            if row == NO_ROW || !source.list.enabled(row as usize) {
                continue;
            }
            let cached = &mut of_row[m][row as usize];
            if *cached == NO_ROW {
                let slot = Slot {
                    rgb: source.list.color(row as usize),
                    alpha: source.alpha,
                    lines: source.lines,
                    sides: source.sides,
                    range2: (source.range * source.range) as u32,
                };
                *cached = match slots.iter().position(|s| *s == slot) {
                    Some(i) => i as u16,
                    None => {
                        slots.push(slot);
                        (slots.len() - 1) as u16
                    }
                };
            }
            *dst = entry(row, *cached);
        }
    }

    Merged {
        of_state: of_state.into_boxed_slice(),
        slots,
        max_range,
        any,
    }
}

static CACHE: Mutex<Option<(u64, Arc<Merged>)>> = Mutex::new(None);

pub fn current() -> (u64, Arc<Merged>) {
    let sources = sources();
    let want = key(&sources);
    let mut cache = CACHE.lock();
    if let Some((have, table)) = cache.as_ref()
        && *have == want
    {
        return (want, table.clone());
    }
    let table = Arc::new(fold(&sources));
    *cache = Some((want, table.clone()));
    (want, table)
}

pub fn forget() {
    *CACHE.lock() = None;
}
