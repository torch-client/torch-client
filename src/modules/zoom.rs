use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use super::registry::{Id, zoom as setting};
use super::store;

static TOUCH: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "mobile_ui")]
pub fn set_touch(on: bool) {
    TOUCH.store(on, Ordering::Relaxed);
}

#[derive(Clone, Copy)]
pub struct Settings {
    pub on: bool,
    pub fov: f32,
    pub duration: f32,
}

pub fn settings() -> Settings {
    let s = store();
    Settings {
        on: s.enabled(Id::Zoom) || TOUCH.load(Ordering::Relaxed),
        fov: s.num(setting::FOV),
        duration: s.num(setting::DURATION).max(0.001),
    }
}

static RATIO_BITS: AtomicU32 = AtomicU32::new(1.0f32.to_bits());

pub fn set_fov_ratio(ratio: f32) {
    let ratio = if ratio.is_finite() {
        ratio.clamp(0.0, 1.0)
    } else {
        1.0
    };
    RATIO_BITS.store(ratio.to_bits(), Ordering::Relaxed);
}

pub fn look_ratio() -> f32 {
    if !store().flag(setting::SLOW_LOOK) {
        return 1.0;
    }
    f32::from_bits(RATIO_BITS.load(Ordering::Relaxed))
}
