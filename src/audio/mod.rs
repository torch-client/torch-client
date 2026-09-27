#[cfg(any(target_arch = "wasm32", target_os = "android"))]
compile_error!(
    "the `audio` feature is desktop-only for now: the sounds are 332 MB in an \
     object store a browser cannot reach, and the mixer's backend is not built \
     for these targets. See the feature's comment in Cargo.toml."
);

pub mod category;
pub(crate) mod defs;
mod engine;
pub(crate) mod store;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

pub use category::SoundCategory;

static DEFS: OnceLock<defs::Defs> = OnceLock::new();

static VOLUMES: [AtomicU32; SoundCategory::COUNT] =
    [const { AtomicU32::new(DEFAULT_VOLUME) }; SoundCategory::COUNT];

pub const DEFAULT_VOLUME: u32 = 100;

static LISTENER: Mutex<Listener> = Mutex::new(Listener::ORIGIN);

#[derive(Clone, Copy)]
struct Listener {
    position: [f32; 3],
    forward: [f32; 3],
    right: [f32; 3],
}

impl Listener {
    const ORIGIN: Listener = Listener {
        position: [0.0, 0.0, 0.0],
        forward: [0.0, 0.0, -1.0],
        right: [1.0, 0.0, 0.0],
    };
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

pub fn set_listener(position: [f32; 3], forward: [f32; 3], right: [f32; 3]) {
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
    submit(event, category, volume, pitch, None);
}

pub fn audible_at(at: [f32; 3]) -> bool {
    if DEFS.get().is_none() {
        return false;
    }
    let Ok(listener) = LISTENER.lock() else {
        return false;
    };
    let dx = at[0] - listener.position[0];
    let dy = at[1] - listener.position[1];
    let dz = at[2] - listener.position[2];
    dx * dx + dy * dy + dz * dz <= ATTENUATION_DISTANCE * ATTENUATION_DISTANCE
}

pub fn play_at(event: &str, category: SoundCategory, at: [f32; 3], volume: f32, pitch: f32) {
    let listener = {
        let Ok(guard) = LISTENER.lock() else {
            return;
        };
        *guard
    };

    let offset = [
        at[0] - listener.position[0],
        at[1] - listener.position[1],
        at[2] - listener.position[2],
    ];
    let square = offset[0] * offset[0] + offset[1] * offset[1] + offset[2] * offset[2];
    let range = ATTENUATION_DISTANCE * volume.max(1.0);
    if square > range * range {
        return;
    }

    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let relative = [
        dot(offset, listener.right),
        offset[1],
        -dot(offset, listener.forward),
    ];
    submit(event, category, volume, pitch, Some(relative));
}

fn submit(
    event: &str,
    category: SoundCategory,
    volume: f32,
    pitch: f32,
    relative: Option<[f32; 3]>,
) {
    let Some(defs) = DEFS.get() else {
        return;
    };
    debug_assert_ne!(category, SoundCategory::Master, "{event} played on Master");

    let master = gain(SoundCategory::Master);
    let category_gain = gain(category);
    if master <= 0.0 || category_gain <= 0.0 {
        return;
    }

    let Some(entry) = defs.pick(event, &mut |bound| roll(bound)) else {
        return;
    };
    engine::play(engine::Play {
        key: entry.object_key(),
        volume: (volume * entry.volume * category_gain * master).clamp(0.0, 1.0),
        pitch: pitch * entry.pitch,
        relative,
    });
}

fn roll(bound: u32) -> u32 {
    use crate::util::javarandom::JavaRandom;
    static RNG: Mutex<Option<JavaRandom>> = Mutex::new(None);
    let Ok(mut guard) = RNG.lock() else {
        return 0;
    };
    let rng = guard.get_or_insert_with(|| {
        let elapsed = crate::platform::time::epoch().elapsed().as_nanos() as i64;
        JavaRandom::new(elapsed ^ (&raw const RNG as usize as i64))
    });
    rng.next_int(bound.max(1))
}
