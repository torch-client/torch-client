use std::collections::HashMap;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};
use std::sync::{Mutex, OnceLock};

use std::sync::Arc;

use super::SoundCategory;
use super::store::Pack;

pub(super) struct Play {
    pub key: Arc<str>,
    pub event: &'static str,
    pub category: SoundCategory,
    pub volume: f32,
    pub pitch: f32,
    pub relative: Option<[f32; 3]>,
}

const QUEUE: usize = 64;

pub(super) struct StartLoop {
    pub id: u64,
    pub key: Arc<str>,
    pub event: &'static str,
    pub category: SoundCategory,
    pub entry_volume: f32,
    pub entry_pitch: f32,
    pub volume: f32,
    pub pitch: f32,
}

enum Command {
    Play(Play),
    Stop(Option<SoundCategory>, Option<&'static str>),
    StartLoop(StartLoop),
    UpdateLoop { id: u64, volume: f32, pitch: f32 },
    Reap,
}

static ENDED: Mutex<Vec<u64>> = Mutex::new(Vec::new());

static SENDER: OnceLock<SyncSender<Command>> = OnceLock::new();

pub(super) fn start(index_id: String) {
    let spawned = std::thread::Builder::new()
        .name("audio".to_owned())
        .spawn(move || run(&index_id));
    if let Err(e) = spawned {
        crate::log_warn!("audio", "cannot start the mixer thread: {e}");
    }
}

pub(super) fn play(sound: Play) {
    send(Command::Play(sound));
}

pub(super) fn stop(category: Option<SoundCategory>, event: Option<&'static str>) {
    send(Command::Stop(category, event));
}

pub(super) fn start_loop(start: StartLoop) -> bool {
    let Some(sender) = SENDER.get() else {
        return false;
    };
    sender.try_send(Command::StartLoop(start)).is_ok()
}

pub(super) fn update_loop(id: u64, volume: f32, pitch: f32) {
    send(Command::UpdateLoop { id, volume, pitch });
}

pub(super) fn end_loop(id: u64) {
    if let Ok(mut ended) = ENDED.lock() {
        ended.push(id);
    }
    send(Command::Reap);
}

fn send(command: Command) {
    let Some(sender) = SENDER.get() else {
        return;
    };
    match sender.try_send(command) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => {}
        Err(TrySendError::Disconnected(_)) => {}
    }
}

fn run(index_id: &str) {
    let mut pack = match Pack::open(index_id) {
        Ok(pack) => pack,
        Err(e) => {
            crate::log_warn!("audio", "the sound pack will not open: {e}");
            return;
        }
    };
    let Some(text) = super::store::load_defs(index_id) else {
        crate::log_warn!("audio", "the sound definitions will not read");
        return;
    };
    let defs = match super::defs::Defs::parse(&text) {
        Ok(defs) => defs,
        Err(e) => {
            crate::log_warn!("audio", "sounds.json will not parse: {e}");
            return;
        }
    };
    drop(text);

    let stream = match rodio::OutputStream::try_default() {
        Ok(pair) => pair,
        Err(e) => {
            crate::log_warn!("audio", "no output device: {e}");
            return;
        }
    };
    let (_stream, handle) = stream;

    let (tx, rx) = std::sync::mpsc::sync_channel(QUEUE);
    if SENDER.set(tx).is_err() {
        return;
    }
    crate::log_info!(
        "audio",
        "mixer running: {} events over {} files",
        defs.len(),
        pack.len()
    );
    super::install_defs(defs);

    let mut plain: Vec<(rodio::Sink, Tag)> = Vec::new();
    let mut spatial: Vec<(rodio::SpatialSink, Tag)> = Vec::new();

    let mut loops: HashMap<u64, LoopVoice> = HashMap::new();
    let mut early_ends: Vec<u64> = Vec::new();
    let mut last_started: u64 = 0;

    while let Ok(command) = rx.recv() {
        reap_ended(&mut loops, &mut early_ends, last_started);
        let sound = match command {
            Command::Play(sound) => sound,
            Command::Reap => continue,
            Command::StartLoop(start) => {
                last_started = last_started.max(start.id);
                if let Some(at) = early_ends.iter().position(|&id| id == start.id) {
                    early_ends.swap_remove(at);
                    continue;
                }
                if let Some(voice) = open_loop(&mut pack, &handle, &start) {
                    loops.insert(start.id, voice);
                }
                continue;
            }
            Command::UpdateLoop { id, volume, pitch } => {
                if let Some(voice) = loops.get(&id) {
                    voice.apply(volume, pitch);
                }
                continue;
            }
            Command::Stop(category, event) => {
                let hit = |tag: &Tag| {
                    category.is_none_or(|c| c == tag.1) && event.is_none_or(|e| e == tag.0)
                };
                plain.retain(|(_, tag)| !hit(tag));
                spatial.retain(|(_, tag)| !hit(tag));
                loops.retain(|_, voice| !hit(&voice.tag));
                continue;
            }
        };
        plain.retain(|(sink, _)| !sink.empty());
        spatial.retain(|(sink, _)| !sink.empty());
        if plain.len() + spatial.len() >= MAX_VOICES {
            continue;
        }
        let tag = (sound.event, sound.category);

        let Some(bytes) = pack.read(&sound.key) else {
            crate::log_warn!("audio", "no such sound in the pack: {}", sound.key);
            continue;
        };
        let cursor = std::io::Cursor::new(bytes);
        let decoder = match rodio::Decoder::new(cursor) {
            Ok(decoder) => decoder,
            Err(e) => {
                crate::log_warn!("audio", "{} will not decode: {e}", sound.key);
                continue;
            }
        };
        let source = rodio::Source::speed(decoder, sound.pitch.max(0.01));

        match sound.relative {
            None => {
                let Ok(sink) = rodio::Sink::try_new(&handle) else {
                    continue;
                };
                sink.set_volume(sound.volume);
                sink.append(source);
                plain.push((sink, tag));
            }
            Some([x, y, z]) => {
                let Ok(sink) = rodio::SpatialSink::try_new(
                    &handle,
                    [x, y, z],
                    [-EAR_OFFSET, 0.0, 0.0],
                    [EAR_OFFSET, 0.0, 0.0],
                ) else {
                    continue;
                };
                sink.set_volume(sound.volume);
                sink.append(source);
                spatial.push((sink, tag));
            }
        }
    }
}

type Tag = (&'static str, SoundCategory);

struct LoopVoice {
    sink: rodio::Sink,
    tag: Tag,
    entry_volume: f32,
    entry_pitch: f32,
}

impl LoopVoice {
    fn apply(&self, volume: f32, pitch: f32) {
        let gain = (volume * self.entry_volume).clamp(0.0, 1.0) * super::bus_gain(self.tag.1);
        self.sink.set_volume(gain);
        self.sink
            .set_speed((pitch * self.entry_pitch).clamp(0.5, 2.0));
    }
}

fn open_loop(
    pack: &mut Pack,
    handle: &rodio::OutputStreamHandle,
    start: &StartLoop,
) -> Option<LoopVoice> {
    let Some(bytes) = pack.read(&start.key) else {
        crate::log_warn!("audio", "no such sound in the pack: {}", start.key);
        return None;
    };
    let decoder = match rodio::Decoder::new_looped(std::io::Cursor::new(bytes)) {
        Ok(decoder) => decoder,
        Err(e) => {
            crate::log_warn!("audio", "{} will not decode: {e}", start.key);
            return None;
        }
    };
    let sink = rodio::Sink::try_new(handle).ok()?;
    let voice = LoopVoice {
        sink,
        tag: (start.event, start.category),
        entry_volume: start.entry_volume,
        entry_pitch: start.entry_pitch,
    };
    voice.apply(start.volume, start.pitch);
    voice.sink.append(decoder);
    Some(voice)
}

fn reap_ended(loops: &mut HashMap<u64, LoopVoice>, early_ends: &mut Vec<u64>, last_started: u64) {
    let Ok(mut ended) = ENDED.lock() else {
        return;
    };
    for id in ended.drain(..) {
        if id > last_started {
            early_ends.push(id);
        } else {
            loops.remove(&id);
        }
    }
}

const MAX_VOICES: usize = 32;

const EAR_OFFSET: f32 = 0.125;

pub const PAN_RADIUS: f32 = 1.0 - EAR_OFFSET;
