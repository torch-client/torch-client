use std::{
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
    time::Duration,
};

static RX: AtomicU64 = AtomicU64::new(0);
static TX: AtomicU64 = AtomicU64::new(0);
static READ_LAG_MICROS: AtomicU64 = AtomicU64::new(0);
static RX_QUEUED: AtomicUsize = AtomicUsize::new(0);

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

pub fn note_read_lag(lag: Duration, queued: usize) {
    let micros = u64::try_from(lag.as_micros()).unwrap_or(u64::MAX);
    READ_LAG_MICROS.store(micros, Ordering::Relaxed);
    RX_QUEUED.store(queued, Ordering::Relaxed);
}

pub fn read_lag() -> Duration {
    Duration::from_micros(READ_LAG_MICROS.load(Ordering::Relaxed))
}

pub fn rx_queued() -> usize {
    RX_QUEUED.load(Ordering::Relaxed)
}

pub fn reset() {
    RX.store(0, Ordering::Relaxed);
    TX.store(0, Ordering::Relaxed);
    READ_LAG_MICROS.store(0, Ordering::Relaxed);
    RX_QUEUED.store(0, Ordering::Relaxed);
}
