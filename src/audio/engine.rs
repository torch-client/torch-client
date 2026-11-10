use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use std::sync::Arc;

use super::SoundCategory;
use super::store::Pack;
use super::voice::{EAR_OFFSET, MAX_VOICES};
pub(super) use super::voice::{Place, Play, StartLoop};

const QUEUE: usize = 64;

enum Command {
    Play(Play),
    Stop(Option<SoundCategory>, Option<&'static str>),
    StartLoop(StartLoop),
    UpdateLoop {
        id: u64,
        volume: f32,
        pitch: f32,
        place: Option<Place>,
    },
    Refold,
    Wake,
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

pub(super) fn update_loop(id: u64, volume: f32, pitch: f32, place: Option<Place>) {
    send(Command::UpdateLoop {
        id,
        volume,
        pitch,
        place,
    });
}

pub(super) fn refold() {
    send(Command::Refold);
}

pub(super) fn end_loop(id: u64) {
    if let Ok(mut ended) = ENDED.lock() {
        ended.push(id);
    }
    wake();
}

pub(super) fn wake() {
    send(Command::Wake);
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

static RELOAD: AtomicBool = AtomicBool::new(false);

pub(super) fn reload() {
    RELOAD.store(true, Ordering::Release);
    wake();
}

fn build_defs(index_id: &str, pack: &Pack) -> Result<super::defs::Defs, String> {
    let text = super::store::load_defs(index_id).ok_or("the sound definitions will not read")?;
    #[cfg(resource_packs)]
    let has = |key: &str| pack.has(key);
    #[cfg(not(resource_packs))]
    let has = {
        let _ = pack;
        |_: &str| true
    };
    super::stack_defs(&text, &has)
}

fn run(index_id: &str) {
    let mut pack = match Pack::open(index_id) {
        Ok(pack) => pack,
        Err(e) => {
            crate::log_warn!("audio", "the sound pack will not open: {e}");
            return;
        }
    };
    RELOAD.store(false, Ordering::Release);
    let defs = match build_defs(index_id, &pack) {
        Ok(defs) => defs,
        Err(e) => {
            crate::log_warn!("audio", "{e}");
            return;
        }
    };

    let Some((stream, mut handle)) = super::devices::open() else {
        return;
    };
    let mut stream = Some(stream);

    let (tx, rx) = std::sync::mpsc::sync_channel(QUEUE);
    if SENDER.set(tx).is_err() {
        return;
    }
    if super::devices::changed() || RELOAD.load(Ordering::Acquire) {
        wake();
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
        if RELOAD.swap(false, Ordering::AcqRel) {
            pack.clear_cache();
            match build_defs(index_id, &pack) {
                Ok(defs) => {
                    crate::log_info!("audio", "sounds reloaded: {} events", defs.len());
                    super::install_defs(defs);
                }
                Err(e) => crate::log_warn!("audio", "{e}"),
            }
        }
        if super::devices::changed() {
            plain.clear();
            spatial.clear();
            drop(stream.take());
            if let Some((opened, opened_handle)) = super::devices::open() {
                stream = Some(opened);
                handle = opened_handle;
                loops.retain(|_, voice| voice.reopen(&mut pack, &handle));
            }
        }
        let sound = match command {
            Command::Play(sound) => sound,
            Command::Wake => continue,
            Command::StartLoop(start) => {
                last_started = last_started.max(start.id);
                let ended_early = early_ends.iter().position(|&id| id == start.id);
                let voice = match ended_early {
                    Some(at) => {
                        early_ends.swap_remove(at);
                        None
                    }
                    None => open_loop(&mut pack, &handle, &start),
                };
                match voice {
                    Some(voice) => {
                        loops.insert(start.id, voice);
                    }
                    None => {
                        if let Some(live) = &start.live {
                            live.store(0, Ordering::Relaxed);
                        }
                    }
                }
                continue;
            }
            Command::UpdateLoop {
                id,
                volume,
                pitch,
                place,
            } => {
                if let Some(voice) = loops.get_mut(&id) {
                    voice.apply(volume, pitch, place);
                }
                continue;
            }
            Command::Refold => {
                loops.values().for_each(LoopVoice::fold);
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

        let Some(cursor) = load(&mut pack, &sound.key, sound.stream) else {
            continue;
        };
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

enum VoiceSink {
    Plain(rodio::Sink),
    Placed(rodio::SpatialSink),
}

type Boxed = Box<dyn rodio::Source<Item = i16> + Send>;

impl VoiceSink {
    fn append(&self, source: Boxed) {
        match self {
            VoiceSink::Plain(sink) => sink.append(source),
            VoiceSink::Placed(sink) => sink.append(source),
        }
    }

    fn position(&self) -> Duration {
        match self {
            VoiceSink::Plain(sink) => sink.get_pos(),
            VoiceSink::Placed(sink) => sink.get_pos(),
        }
    }

    fn set_volume(&self, volume: f32) {
        match self {
            VoiceSink::Plain(sink) => sink.set_volume(volume),
            VoiceSink::Placed(sink) => sink.set_volume(volume),
        }
    }

    fn set_speed(&self, speed: f32) {
        match self {
            VoiceSink::Plain(sink) => sink.set_speed(speed),
            VoiceSink::Placed(sink) => sink.set_speed(speed),
        }
    }
}

struct LoopVoice {
    sink: VoiceSink,
    tag: Tag,
    key: Arc<str>,
    stream: bool,
    entry_volume: f32,
    entry_pitch: f32,
    volume: f32,
    attenuation: f32,
    pan: Option<[f32; 3]>,
    speed: f32,
    live: Option<Arc<AtomicUsize>>,
}

impl LoopVoice {
    fn fold(&self) {
        let volume = (self.volume * self.entry_volume).clamp(0.0, 1.0);
        self.sink
            .set_volume(volume * super::bus_gain(self.tag.1) * self.attenuation);
    }

    fn apply(&mut self, volume: f32, pitch: f32, place: Option<Place>) {
        self.volume = volume;
        if let Some(place) = place {
            self.attenuation = place.attenuation;
            if let VoiceSink::Placed(sink) = &self.sink {
                sink.set_emitter_position(place.pan);
                self.pan = Some(place.pan);
            }
        }
        self.fold();
        self.speed = (pitch * self.entry_pitch).clamp(0.5, 2.0);
        self.sink.set_speed(self.speed);
    }

    fn reopen(&mut self, pack: &mut Pack, handle: &rodio::OutputStreamHandle) -> bool {
        if self
            .live
            .as_ref()
            .is_some_and(|live| live.load(Ordering::Relaxed) != 1)
        {
            return false;
        }
        let at = self.sink.position().mul_f32(self.speed);
        let Some(source) = voice_source(pack, &self.key, self.stream, self.live.as_ref(), at)
        else {
            return false;
        };
        let Some(sink) = voice_sink(handle, self.pan) else {
            return false;
        };
        self.sink = sink;
        self.fold();
        self.sink.set_speed(self.speed);
        self.sink.append(source);
        true
    }
}

impl Drop for LoopVoice {
    fn drop(&mut self) {
        if let Some(live) = &self.live {
            live.store(0, Ordering::Relaxed);
        }
    }
}

fn load(pack: &mut Pack, key: &str, stream: bool) -> Option<std::io::Cursor<Arc<[u8]>>> {
    let bytes = pack.read(key, stream);
    if bytes.is_none() {
        crate::log_warn!("audio", "no such sound in the pack: {key}");
    }
    bytes.map(std::io::Cursor::new)
}

fn open_loop(
    pack: &mut Pack,
    handle: &rodio::OutputStreamHandle,
    start: &StartLoop,
) -> Option<LoopVoice> {
    let source = voice_source(
        pack,
        &start.key,
        start.stream,
        start.live.as_ref(),
        Duration::ZERO,
    )?;
    let (pan, attenuation) = match start.place {
        None => (None, 1.0),
        Some(place) => (Some(place.pan), place.attenuation),
    };
    let sink = voice_sink(handle, pan)?;
    let mut voice = LoopVoice {
        sink,
        tag: (start.event, start.category),
        key: start.key.clone(),
        stream: start.stream,
        entry_volume: start.entry_volume,
        entry_pitch: start.entry_pitch,
        volume: start.volume,
        attenuation,
        pan,
        speed: 1.0,
        live: start.live.clone(),
    };
    voice.apply(start.volume, start.pitch, None);
    voice.sink.append(source);
    Some(voice)
}

fn voice_source(
    pack: &mut Pack,
    key: &str,
    stream: bool,
    live: Option<&Arc<AtomicUsize>>,
    from: Duration,
) -> Option<Boxed> {
    let cursor = load(pack, key, stream)?;
    let source = match live {
        None => rodio::Decoder::new_looped(cursor).map(|decoder| Box::new(decoder) as Boxed),
        Some(live) => rodio::Decoder::new(cursor).map(|mut decoder| {
            skip(&mut decoder, from);
            Box::new(rodio::source::Done::new(decoder, live.clone())) as Boxed
        }),
    };
    match source {
        Ok(source) => Some(source),
        Err(e) => {
            crate::log_warn!("audio", "{key} will not decode: {e}");
            None
        }
    }
}

fn skip(decoder: &mut rodio::Decoder<std::io::Cursor<Arc<[u8]>>>, by: Duration) {
    if by.is_zero() {
        return;
    }
    let rate = f64::from(rodio::Source::sample_rate(&*decoder))
        * f64::from(rodio::Source::channels(&*decoder));
    let samples = (by.as_secs_f64() * rate) as usize;
    decoder.by_ref().take(samples).for_each(drop);
}

fn voice_sink(handle: &rodio::OutputStreamHandle, pan: Option<[f32; 3]>) -> Option<VoiceSink> {
    match pan {
        None => rodio::Sink::try_new(handle).ok().map(VoiceSink::Plain),
        Some(pan) => rodio::SpatialSink::try_new(
            handle,
            pan,
            [-EAR_OFFSET, 0.0, 0.0],
            [EAR_OFFSET, 0.0, 0.0],
        )
        .ok()
        .map(VoiceSink::Placed),
    }
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
