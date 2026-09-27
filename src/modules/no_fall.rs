use super::registry::{Id, no_fall as setting};
use super::store;

const FALL_SPEED: f64 = -0.5;

pub fn enabled() -> bool {
    store().enabled(Id::NoFall)
}

pub fn spoofing(velocity_y: f64, fall_flying: bool, mace: bool) -> bool {
    let s = store();
    if !s.enabled(Id::NoFall) {
        return false;
    }
    if mace && s.flag(Id::NoFall, setting::PAUSE_ON_MACE) {
        return false;
    }
    if super::flight::forcing() {
        return true;
    }
    !fall_flying && velocity_y <= FALL_SPEED
}
