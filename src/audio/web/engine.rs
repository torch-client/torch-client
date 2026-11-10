use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use parking_lot::Mutex;

use super::js;
use super::store::Sounds;
use crate::audio::SoundCategory;
use crate::audio::voice::{MAX_VOICES, PAN_RADIUS};
pub(in crate::audio) use crate::audio::voice::{Place, Play, StartLoop};

const PAN_STRENGTH: f32 = 0.41;

const SHOT_IDS: u64 = 1 << 52;

type Tag = (&'static str, SoundCategory);

struct Held {
    entry_volume: f32,
    entry_pitch: f32,
    volume: f32,
    pitch: f32,
    attenuation: f32,
    pan: Option<[f32; 3]>,
    live: Option<Arc<AtomicUsize>>,
}

impl Held {
    fn gain(&self, category: SoundCategory) -> f32 {
        let volume = (self.volume * self.entry_volume).clamp(0.0, 1.0);
        volume * crate::audio::bus_gain(category) * self.attenuation
    }

    fn rate(&self) -> f32 {
        (self.pitch * self.entry_pitch).clamp(0.5, 2.0)
    }

    fn push(&self, id: u64, category: SoundCategory) {
        js::mc_audio_set(id as f64, self.gain(category), self.rate(), pan(self.pan));
    }
}

struct Voice {
    tag: Tag,
    held: Option<Held>,
}

impl Voice {
    fn over(&self) {
        if let Some(live) = self.held.as_ref().and_then(|held| held.live.as_ref()) {
            live.store(0, Ordering::Relaxed);
        }
    }
}

struct State {
    sounds: Sounds,
    voices: HashMap<u64, Voice>,
    shots: usize,
    next_shot: u64,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn pan(relative: Option<[f32; 3]>) -> f32 {
    relative.map_or(0.0, |[x, _, _]| {
        (x / PAN_RADIUS).clamp(-1.0, 1.0) * PAN_STRENGTH
    })
}

fn ensure(sounds: &Sounds, key: &str) -> bool {
    if js::mc_audio_known(key) {
        return true;
    }
    let Some(bytes) = sounds.read(key) else {
        crate::log_warn!("audio", "no such sound in the set: {key}");
        return false;
    };
    js::mc_audio_load(key, &bytes)
}

pub(in crate::audio) fn open(sounds: Sounds) {
    let defs = match build_defs(&sounds) {
        Ok(defs) => defs,
        Err(e) => {
            crate::log_warn!("audio", "{e}");
            return;
        }
    };
    crate::log_info!(
        "audio",
        "web audio ready: {} events over {} files (Opus at {} bit/s where the browser could encode it)",
        defs.len(),
        sounds.len(),
        super::compact::BITRATE
    );
    *STATE.lock() = Some(State {
        sounds,
        voices: HashMap::new(),
        shots: 0,
        next_shot: SHOT_IDS,
    });
    crate::audio::install_defs(defs);
}

fn build_defs(sounds: &Sounds) -> Result<crate::audio::defs::Defs, String> {
    #[cfg(resource_packs)]
    let has = |key: &str| sounds.has(key);
    #[cfg(not(resource_packs))]
    let has = |_: &str| true;
    crate::audio::stack_defs(&sounds.defs, &has)
}

pub(in crate::audio) fn service() {
    let ended = js::mc_audio_ended();
    if ended.length() == 0 {
        return;
    }
    let mut state = STATE.lock();
    let Some(state) = state.as_mut() else {
        return;
    };
    for id in ended.iter().filter_map(|id| id.as_f64()) {
        if let Some(voice) = state.voices.remove(&(id as u64)) {
            if voice.held.is_none() {
                state.shots -= 1;
            }
            voice.over();
        }
    }
}

pub(in crate::audio) fn play(sound: Play) {
    service();
    let mut state = STATE.lock();
    let Some(state) = state.as_mut() else {
        return;
    };
    if state.shots >= MAX_VOICES || !ensure(&state.sounds, &sound.key) {
        return;
    }
    let id = state.next_shot;
    let started = js::mc_audio_start(
        id as f64,
        &sound.key,
        sound.volume,
        sound.pitch.max(0.01),
        pan(sound.relative),
        sound.relative.is_some(),
        false,
    );
    if started {
        state.next_shot += 1;
        state.shots += 1;
        state.voices.insert(
            id,
            Voice {
                tag: (sound.event, sound.category),
                held: None,
            },
        );
    }
}

pub(in crate::audio) fn stop(category: Option<SoundCategory>, event: Option<&'static str>) {
    let mut state = STATE.lock();
    let Some(state) = state.as_mut() else {
        return;
    };
    let hit = |tag: &Tag| category.is_none_or(|c| c == tag.1) && event.is_none_or(|e| e == tag.0);
    let stopped: Vec<u64> = state
        .voices
        .iter()
        .filter(|(_, voice)| hit(&voice.tag))
        .map(|(id, _)| *id)
        .collect();
    for id in stopped {
        if let Some(voice) = state.voices.remove(&id) {
            js::mc_audio_stop(id as f64);
            if voice.held.is_none() {
                state.shots -= 1;
            }
            voice.over();
        }
    }
}

pub(in crate::audio) fn start_loop(start: StartLoop) -> bool {
    service();
    let mut state = STATE.lock();
    let Some(state) = state.as_mut() else {
        return false;
    };
    let (pan_to, attenuation) = match start.place {
        None => (None, 1.0),
        Some(place) => (Some(place.pan), place.attenuation),
    };
    let held = Held {
        entry_volume: start.entry_volume,
        entry_pitch: start.entry_pitch,
        volume: start.volume,
        pitch: start.pitch,
        attenuation,
        pan: pan_to,
        live: start.live,
    };
    let voice = Voice {
        tag: (start.event, start.category),
        held: Some(held),
    };
    if !ensure(&state.sounds, &start.key) {
        voice.over();
        return true;
    }
    let Some(held) = &voice.held else {
        return false;
    };
    let started = js::mc_audio_start(
        start.id as f64,
        &start.key,
        held.gain(start.category),
        held.rate(),
        pan(held.pan),
        held.pan.is_some(),
        held.live.is_none(),
    );
    if started {
        state.voices.insert(start.id, voice);
    }
    started
}

pub(in crate::audio) fn update_loop(id: u64, volume: f32, pitch: f32, place: Option<Place>) {
    let mut state = STATE.lock();
    let Some(voice) = state.as_mut().and_then(|state| state.voices.get_mut(&id)) else {
        return;
    };
    let category = voice.tag.1;
    let Some(held) = voice.held.as_mut() else {
        return;
    };
    held.volume = volume;
    held.pitch = pitch;
    if let Some(place) = place {
        held.attenuation = place.attenuation;
        if held.pan.is_some() {
            held.pan = Some(place.pan);
        }
    }
    held.push(id, category);
}

pub(in crate::audio) fn refold() {
    let state = STATE.lock();
    let Some(state) = state.as_ref() else {
        return;
    };
    for (id, voice) in &state.voices {
        if let Some(held) = &voice.held {
            held.push(*id, voice.tag.1);
        }
    }
}

pub(in crate::audio) fn end_loop(id: u64) {
    let mut state = STATE.lock();
    let Some(state) = state.as_mut() else {
        return;
    };
    if let Some(voice) = state.voices.remove(&id) {
        js::mc_audio_stop(id as f64);
        if voice.held.is_none() {
            state.shots -= 1;
        }
        voice.over();
    }
}

pub(in crate::audio) fn reload() {
    let state = STATE.lock();
    let Some(state) = state.as_ref() else {
        return;
    };
    js::mc_audio_forget();
    match build_defs(&state.sounds) {
        Ok(defs) => {
            crate::log_info!("audio", "sounds reloaded: {} events", defs.len());
            crate::audio::install_defs(defs);
        }
        Err(e) => crate::log_warn!("audio", "{e}"),
    }
}
