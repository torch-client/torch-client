use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VehicleLook {
    pub seq: u32,
    pub turn: f32,
    pub boat_yaw: f32,
}

pub static SESSION_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Default)]
pub struct SharedState {
    pub camera_yaw: f32,
    pub camera_pitch: f32,
    pub pending_look: Option<(f32, f32)>,
    pub move_flags: u8,
    pub move_keys: u8,
    pub hand_move_keys: u8,
    pub screen_open: bool,
    pub profiling: ChunkProfiling,
    pub quit_requested: bool,
    pub disconnect_requested: bool,
    pub disconnected_pending: bool,
    pub session_ended: bool,
    pub session_epoch: u64,
    pub end_notice: Option<String>,
    pub disconnect_reason: Option<Vec<crate::text::Span>>,
    pub reload_chunks_requested: bool,
    pub remesh_chunks_requested: bool,
    pub leave_bed_requested: bool,
    pub respawn_requested: bool,
    pub respawns: u32,
    pub in_world: bool,
    pub session: SessionState,
    pub render_distance_sent: Option<u32>,
    pub skin_prefs_sent: Option<SkinPrefs>,
    pub skin_prefs: SkinPrefs,
    pub chat_signing_allowed: bool,
}

impl SharedState {
    pub fn end_session(&mut self, status: String, reason: Option<Vec<crate::text::Span>>) -> bool {
        let first = !std::mem::replace(&mut self.session_ended, true);
        if first {
            self.disconnected_pending = true;
            self.end_notice = Some(match &reason {
                Some(spans) if !spans.is_empty() => {
                    spans.iter().map(|span| span.text.as_str()).collect()
                }
                _ => "Connection lost".to_string(),
            });
            self.disconnect_reason = reason;
        }
        self.clear_session(status);
        first
    }

    pub fn clear_session(&mut self, status: String) {
        self.in_world = false;
        self.session_epoch = self.session_epoch.wrapping_add(1);
        SESSION_EPOCH.store(self.session_epoch, std::sync::atomic::Ordering::Relaxed);
        let health_epoch = self.session.health_epoch;
        let block_entities_version = self.session.block_entities_version;
        self.session = SessionState::default();
        self.session.health_epoch = health_epoch;
        self.session.block_entities_version = block_entities_version.wrapping_add(1);
        self.session.clear_chunks = true;
        self.session.status = Some(status.into());
    }
}

#[derive(Default, Clone)]
pub struct ChunkProfiling {
    pub mesh_avg_ms: f32,
    pub mesh_peak_ms: f32,
    pub chunks_meshed: u64,
    pub queue_depth: usize,
    pub worker_count: usize,
    pub poll_avg_ms: f32,
    pub poll_peak_ms: f32,
    pub lock_wait_avg_ms: f32,
}

impl ChunkProfiling {
    pub fn record_mesh(&mut self, elapsed_ms: f32) {
        const A: f32 = 0.05;
        self.mesh_avg_ms = self.mesh_avg_ms * (1.0 - A) + elapsed_ms * A;
        if elapsed_ms > self.mesh_peak_ms {
            self.mesh_peak_ms = elapsed_ms;
        }
        self.chunks_meshed += 1;
    }

    pub fn record_poll(&mut self, elapsed_ms: f32) {
        const A: f32 = 0.1;
        self.poll_avg_ms = self.poll_avg_ms * (1.0 - A) + elapsed_ms * A;
        if elapsed_ms > self.poll_peak_ms {
            self.poll_peak_ms = elapsed_ms;
        }
    }

    pub fn record_lock_wait(&mut self, elapsed_ms: f32) {
        const A: f32 = 0.1;
        self.lock_wait_avg_ms = self.lock_wait_avg_ms * (1.0 - A) + elapsed_ms * A;
    }

    pub fn reset_peaks(&mut self) {
        self.mesh_peak_ms = 0.0;
        self.poll_peak_ms = 0.0;
    }
}

pub struct SharedMutex(parking_lot::Mutex<SharedState>);

const TOAST_QUEUE_MAX: usize = 32;

pub fn queue_toasts(
    shared: &SharedMutex,
    events: impl IntoIterator<Item = crate::gui::toast::ToastEvent>,
) {
    push_toasts(&mut shared.lock().unwrap().session, events);
}

pub fn push_toasts(
    session: &mut SessionState,
    events: impl IntoIterator<Item = crate::gui::toast::ToastEvent>,
) {
    let queue = &mut session.toast_incoming;
    for event in events {
        if queue.len() >= TOAST_QUEUE_MAX {
            break;
        }
        queue.push(event);
    }
}

#[derive(Debug)]
pub struct WouldBlock;

pub struct SideMutex<T>(parking_lot::Mutex<T>);

impl<T> SideMutex<T> {
    pub const fn new(value: T) -> Self {
        SideMutex(parking_lot::Mutex::new(value))
    }

    #[allow(clippy::result_unit_err, reason = "mirrors `std::sync::Mutex::lock`")]
    pub fn lock(&self) -> Result<parking_lot::MutexGuard<'_, T>, std::convert::Infallible> {
        Ok(self.0.lock())
    }

    pub fn try_lock(&self) -> Result<parking_lot::MutexGuard<'_, T>, WouldBlock> {
        self.0.try_lock().ok_or(WouldBlock)
    }
}

impl<T: Default> Default for SideMutex<T> {
    fn default() -> Self {
        SideMutex::new(T::default())
    }
}

impl SharedMutex {
    pub const fn new(state: SharedState) -> Self {
        SharedMutex(parking_lot::Mutex::new(state))
    }

    #[allow(clippy::result_unit_err, reason = "mirrors `std::sync::Mutex::lock`")]
    pub fn lock(
        &self,
    ) -> Result<parking_lot::MutexGuard<'_, SharedState>, std::convert::Infallible> {
        Ok(self.0.lock())
    }

    pub fn try_lock(&self) -> Result<parking_lot::MutexGuard<'_, SharedState>, WouldBlock> {
        self.0.try_lock().ok_or(WouldBlock)
    }

    pub fn try_lock_for(
        &self,
        timeout: std::time::Duration,
    ) -> Result<parking_lot::MutexGuard<'_, SharedState>, WouldBlock> {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = timeout;
            self.try_lock()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.0.try_lock_for(timeout).ok_or(WouldBlock)
        }
    }
}

impl Default for SharedMutex {
    fn default() -> Self {
        SharedMutex::new(SharedState::default())
    }
}
