use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering::Relaxed};

use azalea_core::position::ChunkPos;

pub mod blocks;
pub mod scan;
pub mod table;

use super::list::BitList;
use super::registry::handle::{Enum, Slider};
use super::registry::{Id, options, ore_esp, storage_esp, xray};
use super::{Edge, Phase, store};
pub use scan::Found;
pub use table::Slot;

options! {
    pub enum Shape {
        Lines = "Lines",
        Sides = "Sides",
        Both = "Both",
    }
}

#[derive(Default)]
pub struct EspFrame {
    pub boxes: Vec<Found>,
    pub slots: Vec<Slot>,
}

const MIN_TICKS: u32 = 10;

const IDLE_TICKS: u32 = 600;

const MAX_BOXES: usize = 40_000;

static LAST_POS: AtomicU64 = AtomicU64::new(u64::MAX);
static LAST_KEY: AtomicU64 = AtomicU64::new(u64::MAX);
static SINCE: AtomicU32 = AtomicU32::new(u32::MAX);
static LAST_LEN: AtomicUsize = AtomicUsize::new(0);
static DIRTY: AtomicBool = AtomicBool::new(false);

struct Esp {
    edge: Edge,
    list: &'static BitList,
    range: Slider,
    shape: Enum<Shape>,
    fill: Slider,
}

const COUNT: usize = 3;

static ESPS: [Esp; COUNT] = [
    Esp {
        edge: Edge::new(Id::OreEsp),
        list: &blocks::ORES,
        range: ore_esp::RANGE,
        shape: ore_esp::SHAPE,
        fill: ore_esp::FILL,
    },
    Esp {
        edge: Edge::new(Id::StorageEsp),
        list: &blocks::STORAGE,
        range: storage_esp::RANGE,
        shape: storage_esp::SHAPE,
        fill: storage_esp::FILL,
    },
    Esp {
        edge: Edge::new(Id::Xray),
        list: &blocks::XRAY,
        range: xray::RANGE,
        shape: xray::SHAPE,
        fill: xray::FILL,
    },
];

#[inline]
pub fn mark_dirty() {
    DIRTY.store(true, Relaxed);
}

#[inline]
pub fn any_enabled() -> bool {
    let s = store();
    ESPS.iter().any(|e| s.enabled(e.edge.id()))
}

fn pos_key(c: ChunkPos, section_y: i32) -> u64 {
    (c.x as u32 as u64) << 40 | (c.z as u32 as u64) << 8 | (section_y as u8 as u64)
}

pub fn tick(bot: &azalea::Client, shared: &Arc<crate::session::SharedMutex>) {
    let s = store();
    let phases = ESPS.each_ref().map(|e| e.edge.poll(s));
    let running = phases
        .iter()
        .any(|p| matches!(p, Phase::Started | Phase::Running));
    let edged = phases
        .iter()
        .any(|p| matches!(p, Phase::Started | Phase::Stopped));

    if !running {
        if edged {
            forget();
            shared.lock().unwrap().session.esp = Arc::new(EspFrame::default());
        }
        return;
    }
    if edged {
        SINCE.store(u32::MAX, Relaxed);
        LAST_POS.store(u64::MAX, Relaxed);
        LAST_KEY.store(u64::MAX, Relaxed);
    }

    let since = SINCE.load(Relaxed).saturating_add(1);
    SINCE.store(since, Relaxed);

    let (key, table) = table::current();
    if !table.any {
        if LAST_LEN.swap(0, Relaxed) != 0 {
            shared.lock().unwrap().session.esp = Arc::new(EspFrame::default());
        }
        return;
    }

    let (centre, centre_chunk) = {
        let s = shared.lock().unwrap();
        let p = s.session.player_pos;
        let centre = [p[0] as i32, p[1] as i32, p[2] as i32];
        (
            centre,
            ChunkPos::new(centre[0].div_euclid(16), centre[2].div_euclid(16)),
        )
    };

    let pos = pos_key(centre_chunk, centre[1].div_euclid(16));
    let dirty = DIRTY.load(Relaxed);
    let changed = dirty || pos != LAST_POS.load(Relaxed) || key != LAST_KEY.load(Relaxed);
    if !(changed && since >= MIN_TICKS || since >= IDLE_TICKS) {
        return;
    }
    SINCE.store(0, Relaxed);
    LAST_POS.store(pos, Relaxed);
    LAST_KEY.store(key, Relaxed);
    DIRTY.store(false, Relaxed);

    let Ok(world) = bot.world() else { return };
    let r = (table.max_range / 16) + 1;
    let r2 = r * r;

    let (handles, min_y) = {
        let w = world.read();
        let mut handles = Vec::with_capacity(((2 * r + 1) * (2 * r + 1)) as usize);
        for dz in -r..=r {
            for dx in -r..=r {
                if dx * dx + dz * dz > r2 {
                    continue;
                }
                let at = ChunkPos::new(centre_chunk.x + dx, centre_chunk.z + dz);
                if let Some(c) = w.chunks.get(&at) {
                    handles.push((at, c));
                }
            }
        }
        (handles, w.chunks.min_y())
    };

    let mut boxes = Vec::with_capacity(LAST_LEN.load(Relaxed).min(MAX_BOXES));
    let full = with_scratch(|scratch| {
        scan::scan(
            &handles, &table, min_y, centre, MAX_BOXES, scratch, &mut boxes,
        )
    });
    if full {
        static WARNED: AtomicBool = AtomicBool::new(false);
        if !WARNED.swap(true, Relaxed) {
            crate::log_warn!(
                "esp",
                "block ESP hit its cap of {} boxes; narrow the range or the block list",
                MAX_BOXES
            );
        }
    }
    LAST_LEN.store(boxes.len(), Relaxed);

    let frame = EspFrame {
        boxes,
        slots: table.slots.clone(),
    };
    shared.lock().unwrap().session.esp = Arc::new(frame);
}

fn with_scratch<R>(f: impl FnOnce(&mut scan::Scratch) -> R) -> R {
    use std::cell::RefCell;
    thread_local! {
        static SCRATCH: RefCell<scan::Scratch> = RefCell::new(scan::Scratch::default());
    }
    SCRATCH.with(|s| f(&mut s.borrow_mut()))
}

pub fn forget() {
    LAST_POS.store(u64::MAX, Relaxed);
    LAST_KEY.store(u64::MAX, Relaxed);
    SINCE.store(u32::MAX, Relaxed);
    LAST_LEN.store(0, Relaxed);
    DIRTY.store(false, Relaxed);
    table::forget();
}
