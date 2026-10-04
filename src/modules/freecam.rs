#[cfg(feature = "click_gui")]
use super::registry::Id;
use super::registry::freecam as setting;
use super::store;

#[cfg(feature = "click_gui")]
pub fn active() -> bool {
    store().enabled(Id::Freecam)
}

#[cfg(feature = "click_gui")]
pub fn toggle() {
    store().toggle(Id::Freecam);
}

#[cfg(not(feature = "click_gui"))]
static ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(not(feature = "click_gui"))]
pub fn active() -> bool {
    ACTIVE.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(not(feature = "click_gui"))]
pub fn toggle() {
    ACTIVE.fetch_xor(true, std::sync::atomic::Ordering::Relaxed);
}

pub fn speed(sprint: bool) -> f32 {
    let n = if sprint {
        setting::SPRINT_SPEED
    } else {
        setting::SPEED
    };
    store().num(n)
}
