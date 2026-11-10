use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use super::SoundCategory;

pub(super) struct Play {
    pub key: Arc<str>,
    pub event: &'static str,
    pub category: SoundCategory,
    pub volume: f32,
    pub pitch: f32,
    pub relative: Option<[f32; 3]>,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub stream: bool,
}

pub(super) struct StartLoop {
    pub id: u64,
    pub key: Arc<str>,
    pub event: &'static str,
    pub category: SoundCategory,
    pub entry_volume: f32,
    pub entry_pitch: f32,
    pub volume: f32,
    pub pitch: f32,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub stream: bool,
    pub place: Option<Place>,
    pub live: Option<Arc<AtomicUsize>>,
}

#[derive(Clone, Copy)]
pub(super) struct Place {
    pub attenuation: f32,
    pub pan: [f32; 3],
}

pub(super) const MAX_VOICES: usize = 32;

pub(super) const EAR_OFFSET: f32 = 0.125;

pub const PAN_RADIUS: f32 = 1.0 - EAR_OFFSET;
