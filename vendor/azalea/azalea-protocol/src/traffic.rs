use std::sync::atomic::{AtomicU64, Ordering};

static RX: AtomicU64 = AtomicU64::new(0);
static TX: AtomicU64 = AtomicU64::new(0);

pub fn rx() -> u64 {
    RX.load(Ordering::Relaxed)
}

pub fn tx() -> u64 {
    TX.load(Ordering::Relaxed)
}

pub fn note_rx(n: usize) {
    RX.fetch_add(n as u64, Ordering::Relaxed);
}

pub fn note_tx(n: usize) {
    TX.fetch_add(n as u64, Ordering::Relaxed);
}

pub fn reset() {
    RX.store(0, Ordering::Relaxed);
    TX.store(0, Ordering::Relaxed);
}
