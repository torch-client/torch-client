use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering::Relaxed};

use azalea_core::position::ChunkPos;

pub mod blocks;
pub mod scan;
pub mod table;

use super::list::BitList;
use super::registry::handle::{Enum, Slider, Toggle};
use super::registry::{Id, block_esp, options, ore_esp, storage_esp, xray};
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
    lava: Option<Toggle>,
    lava_fade: Option<Toggle>,
    exposed: Option<Toggle>,
}

const COUNT: usize = 4;

static ESPS: [Esp; COUNT] = [
    Esp {
        edge: Edge::new(Id::OreEsp),
        list: &blocks::ORES,
        range: ore_esp::RANGE,
        shape: ore_esp::SHAPE,
        fill: ore_esp::FILL,
        lava: None,
        lava_fade: None,
        exposed: Some(ore_esp::EXPOSED),
    },
    Esp {
        edge: Edge::new(Id::StorageEsp),
        list: &blocks::STORAGE,
        range: storage_esp::RANGE,
        shape: storage_esp::SHAPE,
        fill: storage_esp::FILL,
        lava: None,
        lava_fade: None,
        exposed: None,
    },
    Esp {
        edge: Edge::new(Id::Xray),
        list: &blocks::XRAY,
        range: xray::RANGE,
        shape: xray::SHAPE,
        fill: xray::FILL,
        lava: Some(xray::LAVA),
        lava_fade: Some(xray::LAVRENDER),
        exposed: Some(xray::EXPOSED),
    },
    Esp {
        edge: Edge::new(Id::BlockEsp),
        list: &blocks::BLOCK_ESP,
        range: block_esp::RANGE,
        shape: block_esp::SHAPE,
        fill: block_esp::FILL,
        lava: None,
        lava_fade: None,
        exposed: None,
    },
];

#[inline]
pub fn mark_dirty() {
    DIRTY.store(true, Relaxed);
}

#[inline]
pub fn any_enabled() -> bool {
    let s = store();
    ESPS.iter().any(|e| s.enabled(e.edge.id())) || s.enabled(Id::SeedXray)
}

fn pos_key(c: ChunkPos, section_y: i32) -> u64 {
    (c.x as u32 as u64) << 40 | (c.z as u32 as u64) << 8 | (section_y as u8 as u64)
}

pub fn tick(bot: &azalea::Client, shared: &crate::session::SharedMutex) {
    let s = store();
    let phases = ESPS.each_ref().map(|e| e.edge.poll(s));
    let seed = super::seed_xray::tick(bot, shared);
    let sound = super::sound_esp::tick();
    let running = seed.running
        || sound.running
        || phases
            .iter()
            .any(|p| matches!(p, Phase::Started | Phase::Running));
    let edged = seed.edged
        || sound.edged
        || phases
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
    let key = key.wrapping_add(seed.key).wrapping_add(sound.key);
    if !table.any && !seed.running && !sound.running {
        if LAST_LEN.swap(0, Relaxed) != 0 {
            shared.lock().unwrap().session.esp = Arc::new(EspFrame::default());
        }
        return;
    }

    let (centre, centre_chunk) = {
        let s = shared.lock().unwrap();
        let p = s.session.player_pos;
        let (cx, cz) = s.session.chunk_pos();
        (p.map(|v| v.floor() as i32), ChunkPos::new(cx, cz))
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
    let (handles, min_y) = handles(&world, centre_chunk, Some(r));

    let mut boxes = Vec::with_capacity(LAST_LEN.load(Relaxed).min(MAX_BOXES));
    let full = table.any
        && with_scratch(|scratch| {
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
    let mut slots = table.slots.clone();
    super::seed_xray::append(bot, centre, dirty, MAX_BOXES, &mut boxes, &mut slots);
    super::sound_esp::append(MAX_BOXES, &mut boxes, &mut slots);
    LAST_LEN.store(boxes.len(), Relaxed);

    let frame = EspFrame { boxes, slots };
    shared.lock().unwrap().session.esp = Arc::new(frame);
}

pub(in crate::modules) type Handle = (ChunkPos, Arc<parking_lot::RwLock<azalea_world::Chunk>>);

fn handles(
    world: &parking_lot::RwLock<azalea_world::World>,
    centre: ChunkPos,
    r: Option<i32>,
) -> (Vec<Handle>, i32) {
    let dist2 = |h: &Handle| {
        let (dx, dz) = (h.0.x - centre.x, h.0.z - centre.z);
        dx * dx + dz * dz
    };
    let w = world.read();
    let Some(r) = r else {
        let mut all: Vec<Handle> = w
            .chunks
            .chunks()
            .iter()
            .filter_map(|at| w.chunks.get(at).map(|c| (**at, c)))
            .collect();
        all.sort_unstable_by_key(dist2);
        return (all, w.chunks.min_y());
    };
    let r2 = r * r;
    let mut handles = Vec::with_capacity(((2 * r + 1) * (2 * r + 1)) as usize);
    for dz in -r..=r {
        for dx in -r..=r {
            if dx * dx + dz * dz > r2 {
                continue;
            }
            let at = ChunkPos::new(centre.x + dx, centre.z + dz);
            if let Some(c) = w.chunks.get(&at) {
                handles.push((at, c));
            }
        }
    }
    handles.sort_unstable_by_key(dist2);
    (handles, w.chunks.min_y())
}

pub(in crate::modules) fn sweep(
    bot: &azalea::Client,
    table: &table::Merged,
    centre: [i32; 3],
    radius: Option<i32>,
    out: &mut Vec<Found>,
) -> bool {
    let Ok(world) = bot.world() else {
        out.clear();
        return false;
    };
    let at = ChunkPos::new(centre[0].div_euclid(16), centre[2].div_euclid(16));
    let (handles, min_y) = handles(&world, at, radius);
    with_scratch(|scratch| scan::scan(&handles, table, min_y, centre, MAX_BOXES, scratch, out))
}

pub(in crate::modules) fn sweep_column(
    column: &Handle,
    table: &table::Merged,
    min_y: i32,
    out: &mut Vec<Found>,
) -> bool {
    let centre = [column.0.x * 16 + 8, 0, column.0.z * 16 + 8];
    with_scratch(|scratch| {
        scan::scan(
            std::slice::from_ref(column),
            table,
            min_y,
            centre,
            MAX_BOXES,
            scratch,
            out,
        )
    })
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
