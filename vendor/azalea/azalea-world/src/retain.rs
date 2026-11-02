use std::sync::{
    OnceLock,
    atomic::{AtomicU32, Ordering},
};

use azalea_core::position::ChunkPos;

static SERVER_RADIUS: AtomicU32 = AtomicU32::new(0);

static RADIUS_FLOOR: AtomicU32 = AtomicU32::new(0);

pub fn note_server_radius(radius: u32) -> u32 {
    SERVER_RADIUS.store(radius, Ordering::Relaxed);
    effective_radius()
}

pub fn set_radius_floor(radius: u32) {
    RADIUS_FLOOR.store(radius, Ordering::Relaxed);
}

pub fn effective_radius() -> u32 {
    SERVER_RADIUS
        .load(Ordering::Relaxed)
        .max(RADIUS_FLOOR.load(Ordering::Relaxed))
}

pub type ForgetFilter = fn(ChunkPos) -> bool;

static FILTER: OnceLock<ForgetFilter> = OnceLock::new();

pub fn set_forget_filter(filter: ForgetFilter) {
    let _ = FILTER.set(filter);
}

pub fn keeps_forgotten(pos: ChunkPos) -> bool {
    FILTER.get().is_some_and(|f| f(pos))
}
