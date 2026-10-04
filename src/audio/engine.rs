use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};

use std::sync::Arc;

use super::store::Pack;

pub(super) struct Play {
    pub key: Arc<str>,
    pub volume: f32,
    pub pitch: f32,
    pub relative: Option<[f32; 3]>,
}

const QUEUE: usize = 64;

static SENDER: OnceLock<SyncSender<Play>> = OnceLock::new();

pub(super) fn start(index_id: String) {
    let spawned = std::thread::Builder::new()
        .name("audio".to_owned())
        .spawn(move || run(&index_id));
    if let Err(e) = spawned {
        crate::log_warn!("audio", "cannot start the mixer thread: {e}");
    }
}

pub(super) fn play(sound: Play) {
    let Some(sender) = SENDER.get() else {
        return;
    };
    match sender.try_send(sound) {
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

    let mut plain: Vec<rodio::Sink> = Vec::new();
    let mut spatial: Vec<rodio::SpatialSink> = Vec::new();

    while let Ok(sound) = rx.recv() {
        plain.retain(|sink| !sink.empty());
        spatial.retain(|sink| !sink.empty());
        if plain.len() + spatial.len() >= MAX_VOICES {
            continue;
        }

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
                plain.push(sink);
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
                spatial.push(sink);
            }
        }
    }
}

const MAX_VOICES: usize = 32;

const EAR_OFFSET: f32 = 0.125;
