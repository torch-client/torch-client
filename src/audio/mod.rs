#[cfg(any(target_arch = "wasm32", target_os = "android"))]
compile_error!(
    "the `audio` feature is desktop-only for now: the sounds are 331 MiB in an \
     object store a browser cannot reach, and the mixer's backend is not built \
     for these targets. See the feature's comment in Cargo.toml."
);

pub mod category;
pub(crate) mod defs;
mod engine;
pub(crate) mod store;

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

pub use category::SoundCategory;

static DEFS: OnceLock<defs::Defs> = OnceLock::new();

static VOLUMES: [AtomicU32; SoundCategory::COUNT] =
    [const { AtomicU32::new(DEFAULT_VOLUME) }; SoundCategory::COUNT];

pub const DEFAULT_VOLUME: u32 = 100;

static LISTENER: Mutex<Listener> = Mutex::new(Listener::ORIGIN);

#[derive(Clone, Copy)]
struct Listener {
    position: [f64; 3],
    forward: [f32; 3],
    right: [f32; 3],
}

impl Listener {
    const ORIGIN: Listener = Listener {
        position: [0.0, 0.0, 0.0],
        forward: [0.0, 0.0, -1.0],
        right: [1.0, 0.0, 0.0],
    };

    fn offset_to(&self, at: [f64; 3]) -> [f32; 3] {
        crate::util::offset_f32(self.position, at)
    }
}

const ATTENUATION_DISTANCE: f32 = 16.0;

pub fn init(index_id: &str) {
    if DEFS.get().is_some() {
        return;
    }
    if !store::installed(index_id) {
        crate::log_info!(
            "audio",
            "no sound set for index {index_id}; the client will be silent"
        );
        return;
    }
    engine::start(index_id.to_owned());
}

pub(crate) fn install_defs(parsed: defs::Defs) {
    let _ = DEFS.set(parsed);
}

pub fn ready() -> bool {
    DEFS.get().is_some()
}

pub fn set_volumes(volumes: &[u32; SoundCategory::COUNT]) {
    for (slot, value) in volumes.iter().enumerate() {
        VOLUMES[slot].store((*value).min(100), Ordering::Relaxed);
    }
}

pub fn set_listener(position: [f64; 3], forward: [f32; 3], right: [f32; 3]) {
    if let Ok(mut listener) = LISTENER.lock() {
        *listener = Listener {
            position,
            forward,
            right,
        };
    }
}

fn gain(category: SoundCategory) -> f32 {
    VOLUMES[category.index()].load(Ordering::Relaxed) as f32 / 100.0
}

pub fn play(event: &str, category: SoundCategory, volume: f32, pitch: f32) {
    debug_assert_ne!(category, SoundCategory::Master, "{event} played on Master");
    submit(event, category, volume, pitch, None, &mut roll);
}

pub fn audible_at(at: [f64; 3]) -> bool {
    if DEFS.get().is_none() {
        return false;
    }
    let Ok(listener) = LISTENER.lock() else {
        return false;
    };
    let [dx, dy, dz] = listener.offset_to(at);
    dx * dx + dy * dy + dz * dz <= ATTENUATION_DISTANCE * ATTENUATION_DISTANCE
}

pub fn play_at(event: &str, category: SoundCategory, at: [f64; 3], volume: f32, pitch: f32) {
    debug_assert_ne!(category, SoundCategory::Master, "{event} played on Master");
    play_placed(event, category, at, volume, pitch, &mut roll);
}

pub fn play_seeded(
    event: &str,
    category: SoundCategory,
    at: [f64; 3],
    volume: f32,
    pitch: f32,
    seed: i64,
) {
    let mut rng = crate::util::javarandom::JavaRandom::new(seed);
    play_placed(event, category, at, volume, pitch, &mut |bound| {
        rng.next_int(bound.max(1))
    });
}

fn play_placed(
    event: &str,
    category: SoundCategory,
    at: [f64; 3],
    volume: f32,
    pitch: f32,
    roll: &mut impl FnMut(u32) -> u32,
) {
    if let Some(relative) = place(at, volume) {
        submit(event, category, volume, pitch, Some(relative), roll);
    }
}

pub fn stop(category: Option<SoundCategory>, name: Option<&str>) {
    let event = name.map(|name| DEFS.get().and_then(|defs| defs.key(name)));
    if event == Some(None) {
        return;
    }
    engine::stop(category, event.flatten());
}

fn bus_gain(category: SoundCategory) -> f32 {
    let master = gain(SoundCategory::Master);
    if category == SoundCategory::Master {
        master
    } else {
        gain(category) * master
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct LoopId(u64);

static NEXT_LOOP: AtomicU64 = AtomicU64::new(1);

pub fn start_loop(event: &str, category: SoundCategory, volume: f32, pitch: f32) -> Option<LoopId> {
    debug_assert_ne!(category, SoundCategory::Master, "{event} looped on Master");
    let defs = DEFS.get()?;
    let (name, entry) = defs.resolve(event, &mut roll)?;
    let id = NEXT_LOOP.fetch_add(1, Ordering::Relaxed);
    let sent = engine::start_loop(engine::StartLoop {
        id,
        key: entry.object_key(),
        event: name,
        category,
        entry_volume: entry.volume,
        entry_pitch: entry.pitch,
        volume,
        pitch,
    });
    sent.then_some(LoopId(id))
}

pub fn update_loop(id: LoopId, volume: f32, pitch: f32) {
    engine::update_loop(id.0, volume, pitch);
}

pub fn end_loop(id: LoopId) {
    engine::end_loop(id.0);
}

fn place(at: [f64; 3], volume: f32) -> Option<[f32; 3]> {
    let listener = *LISTENER.lock().ok()?;

    let offset = listener.offset_to(at);
    let square = offset[0] * offset[0] + offset[1] * offset[1] + offset[2] * offset[2];
    let range = ATTENUATION_DISTANCE * volume.max(1.0);
    if square > range * range {
        return None;
    }

    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    Some([
        dot(offset, listener.right),
        offset[1],
        -dot(offset, listener.forward),
    ])
}

fn submit(
    event: &str,
    category: SoundCategory,
    volume: f32,
    pitch: f32,
    relative: Option<[f32; 3]>,
    roll: &mut impl FnMut(u32) -> u32,
) {
    let Some(defs) = DEFS.get() else {
        return;
    };
    let bus = bus_gain(category);
    if bus <= 0.0 {
        return;
    }

    let Some((name, entry)) = defs.resolve(event, roll) else {
        return;
    };
    engine::play(engine::Play {
        key: entry.object_key(),
        event: name,
        category,
        volume: (volume * entry.volume * bus).clamp(0.0, 1.0),
        pitch: pitch * entry.pitch,
        relative,
    });
}

fn with_rng<R>(f: impl FnOnce(&mut crate::util::javarandom::JavaRandom) -> R) -> Option<R> {
    use crate::util::javarandom::JavaRandom;
    static RNG: Mutex<Option<JavaRandom>> = Mutex::new(None);
    let mut guard = RNG.lock().ok()?;
    let rng = guard.get_or_insert_with(|| {
        let elapsed = crate::platform::time::epoch().elapsed().as_nanos() as i64;
        JavaRandom::new(elapsed ^ (&raw const RNG as usize as i64))
    });
    Some(f(rng))
}

fn roll(bound: u32) -> u32 {
    with_rng(|rng| rng.next_int(bound.max(1))).unwrap_or(0)
}

pub fn random_f32() -> f32 {
    with_rng(|rng| rng.next_f32()).unwrap_or(0.0)
}
