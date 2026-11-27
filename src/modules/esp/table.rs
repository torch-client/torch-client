use std::sync::Arc;

use parking_lot::Mutex;

use super::blocks::LAVA_RGB;
use super::{COUNT, ESPS, Shape};
use crate::modules::list::{BitList, NO_ROW};
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
    pub exposed: bool,
    pub fade: bool,
}

pub struct Merged {
    pub of_state: Box<[u32]>,
    pub slots: Vec<Slot>,
    pub max_range: i32,
    pub any: bool,
    pub exposed: bool,
}

impl Merged {
    #[inline]
    pub fn entry(&self, state_id: u32) -> u32 {
        self.of_state
            .get(state_id as usize)
            .copied()
            .unwrap_or(NONE_ENTRY)
    }

    pub(in crate::modules) fn of_groups(
        groups: u16,
        group: impl Fn(azalea::block::BlockState) -> Option<u16>,
    ) -> Merged {
        let mut of_state = vec![NONE_ENTRY; azalea::block::BlockState::MAX_STATE as usize + 1];
        for id in 0..=azalea::block::BlockState::MAX_STATE {
            let Ok(state) = azalea::block::BlockState::try_from(id) else {
                continue;
            };
            if let Some(g) = group(state).filter(|g| *g < groups) {
                of_state[id as usize] = entry(g, g);
            }
        }
        let slot = Slot {
            rgb: 0,
            alpha: 0,
            lines: false,
            sides: false,
            range2: u32::MAX,
            exposed: false,
            fade: false,
        };
        Merged {
            of_state: of_state.into_boxed_slice(),
            slots: vec![slot; groups as usize],
            max_range: ALL_LOADED,
            any: groups > 0,
            exposed: false,
        }
    }

    pub(in crate::modules) fn with_range(&self, range: i32) -> Merged {
        let range2 = (range * range) as u32;
        Merged {
            of_state: self.of_state.clone(),
            slots: self.slots.iter().map(|s| Slot { range2, ..*s }).collect(),
            max_range: range,
            any: self.any,
            exposed: self.exposed,
        }
    }
}

const ALL_LOADED: i32 = 1 << 16;

struct Source {
    list: &'static BitList,
    range: i32,
    alpha: u8,
    lines: bool,
    sides: bool,
    lava: bool,
    exposed: bool,
    lava_fade: bool,
}

fn key(sources: &[Option<Source>; COUNT]) -> u64 {
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
                eat(s.alpha as u64
                    | (s.lines as u64) << 8
                    | (s.sides as u64) << 9
                    | (s.lava as u64) << 10
                    | (s.exposed as u64) << 11
                    | (s.lava_fade as u64) << 12);
            }
        }
    }
    h
}

pub(in crate::modules) fn look(shape: Shape, fill: f32) -> (u8, bool, bool) {
    let alpha = if shape == Shape::Lines {
        0
    } else {
        fill.clamp(0.0, 255.0) as u8
    };
    (alpha, shape != Shape::Sides, shape != Shape::Lines)
}

fn sources() -> [Option<Source>; COUNT] {
    let s = store();
    ESPS.each_ref().map(|e| {
        if !s.enabled(e.edge.id()) {
            return None;
        }
        let (alpha, lines, sides) = look(s.choice(e.shape), s.num(e.fill));
        Some(Source {
            list: e.list,
            range: s.num(e.range).max(0.0) as i32,
            alpha,
            lines,
            sides,
            lava: e.lava.is_some_and(|t| s.flag(t)),
            exposed: e.exposed.is_some_and(|t| s.flag(t)),
            lava_fade: e.lava_fade.is_some_and(|t| s.flag(t)),
        })
    })
}

fn fold(sources: &[Option<Source>; COUNT]) -> Merged {
    let mut slots: Vec<Slot> = Vec::new();
    let mut max_range = 0;
    let mut any = false;

    let mut of_row: [Vec<u16>; COUNT] = std::array::from_fn(|_| Vec::new());
    let mut of_state = vec![NONE_ENTRY; azalea::block::BlockState::MAX_STATE as usize + 1];

    for (m, source) in sources.iter().enumerate() {
        let Some(source) = source else { continue };
        any = true;
        max_range = max_range.max(source.range);
        of_row[m] = vec![NO_ROW; source.list.count()];
        let index = source.list.index();
        let lava_row = if source.lava || source.lava_fade {
            source.list.find("lava").map(|r| r as u16)
        } else {
            None
        };
        let lava = lava_row.filter(|_| source.lava);

        for (state_id, dst) in of_state.iter_mut().enumerate() {
            if *dst != NONE_ENTRY {
                continue;
            }
            let Some(&row) = index.get(state_id) else {
                continue;
            };
            let forced = lava == Some(row);
            if row == NO_ROW || !(forced || source.list.enabled(row as usize)) {
                continue;
            }
            let cached = &mut of_row[m][row as usize];
            if *cached == NO_ROW {
                let slot = Slot {
                    rgb: if forced {
                        LAVA_RGB
                    } else {
                        source.list.color(row as usize)
                    },
                    alpha: source.alpha,
                    lines: source.lines,
                    sides: source.sides,
                    range2: (source.range * source.range) as u32,
                    exposed: source.exposed,
                    fade: source.lava_fade && lava_row == Some(row),
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

    let exposed = slots.iter().any(|s| s.exposed);
    Merged {
        of_state: of_state.into_boxed_slice(),
        slots,
        max_range,
        any,
        exposed,
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
