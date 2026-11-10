#[cfg(target_os = "android")]
compile_error!(
    "the `audio` feature is not built for Android: the sounds are 331 MiB, and \
     the mixer's backend is not built for this target. See the feature's comment \
     in Cargo.toml."
);

#[cfg(all(target_arch = "wasm32", not(feature = "browser_audio")))]
compile_error!(
    "on the web, ask for `browser_audio` rather than `audio`. See its comment in Cargo.toml."
);

#[cfg(all(feature = "browser_audio", not(target_arch = "wasm32")))]
compile_error!(
    "`browser_audio` is the web build's sound and needs the Web Audio API; use `audio` here."
);

mod ambience;
mod attributes;
pub mod category;
pub(crate) mod defs;
mod music;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) mod subset;
mod voice;

#[cfg(not(target_arch = "wasm32"))]
pub mod devices;
#[cfg(not(target_arch = "wasm32"))]
mod engine;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod store;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::devices;
#[cfg(target_arch = "wasm32")]
use web::engine;
#[cfg(target_arch = "wasm32")]
pub(crate) use web::{compact, store};

use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub(crate) use attributes::{clear_synced, publish_synced};
pub use category::SoundCategory;
pub use music::{
    MusicFrequency, NowPlaying, jukebox_start, jukebox_stop, now_playing,
    set_minutes_between_songs, set_music_frequency,
};

static DEFS: parking_lot::RwLock<Option<Arc<defs::Defs>>> = parking_lot::RwLock::new(None);

fn defs() -> Option<Arc<defs::Defs>> {
    DEFS.read().clone()
}

static INSTALLED: AtomicBool = AtomicBool::new(false);

static VOLUMES: [AtomicU32; SoundCategory::COUNT] =
    [const { AtomicU32::new(DEFAULT_VOLUME) }; SoundCategory::COUNT];

pub const DEFAULT_VOLUME: u32 = 100;

static LISTENER: Mutex<Listener> = Mutex::new(Listener::ORIGIN);

#[derive(Clone, Copy)]
struct Listener {
    position: [f64; 3],
    forward: [f32; 3],
    right: [f32; 3],
}

impl Listener {
    const ORIGIN: Listener = Listener {
        position: [0.0, 0.0, 0.0],
        forward: [0.0, 0.0, -1.0],
        right: [1.0, 0.0, 0.0],
    };

    fn offset_to(&self, at: [f64; 3]) -> [f32; 3] {
        crate::util::offset_f32(self.position, at)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn init(index_id: &str) {
    if ready() {
        return;
    }
    if !store::installed(index_id) {
        crate::log_info!(
            "audio",
            "no sound set for index {index_id}; the client will be silent"
        );
        return;
    }
    INSTALLED.store(true, Ordering::Relaxed);
    engine::start(index_id.to_owned());
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn init_from_archive() {
    if ready() {
        return;
    }
    match store::Sounds::from_archive() {
        Some(sounds) => adopt(sounds),
        None => crate::log_info!(
            "audio",
            "the asset archive holds no sounds; the client will be silent"
        ),
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn adopt(sounds: store::Sounds) {
    if ready() {
        return;
    }
    INSTALLED.store(true, Ordering::Relaxed);
    engine::open(sounds);
}

fn stack_defs(base: &str, has: &dyn Fn(&str) -> bool) -> Result<defs::Defs, String> {
    let mut defs = defs::Defs::default();
    defs.merge("minecraft", base, &|_| true)
        .map_err(|e| format!("sounds.json will not parse: {e}"))?;
    #[cfg(resource_packs)]
    {
        use crate::platform::assets::archive;
        for namespace in archive::pack_sound_namespaces() {
            for file in archive::pack_stack(&format!("assets/{namespace}/sounds.json")) {
                let text = String::from_utf8_lossy(&file);
                let text = text.trim_start_matches('\u{feff}');
                if let Err(e) = defs.merge(&namespace, text, has) {
                    crate::log_warn!(
                        "audio",
                        "invalid sounds.json in a pack, namespace {namespace}: {e}"
                    );
                }
            }
        }
    }
    #[cfg(not(resource_packs))]
    let _ = has;
    defs.finish();
    Ok(defs)
}

#[cfg(resource_packs)]
fn pack_override(key: &str) -> Option<Arc<[u8]>> {
    use crate::platform::assets::archive;
    archive::packs_active()
        .then(|| archive::pack_get(&defs::layer_key(key)))?
        .map(Into::into)
}

#[cfg(not(resource_packs))]
fn pack_override(_key: &str) -> Option<Arc<[u8]>> {
    None
}

pub(crate) fn install_defs(parsed: defs::Defs) {
    *DEFS.write() = Some(Arc::new(parsed));
}

pub(crate) fn packs_changed() {
    engine::reload();
}

pub fn ready() -> bool {
    DEFS.read().is_some()
}

pub fn installed() -> bool {
    INSTALLED.load(Ordering::Relaxed)
}

pub fn set_volumes(volumes: &[u32; SoundCategory::COUNT]) {
    for (slot, value) in volumes.iter().enumerate() {
        VOLUMES[slot].store((*value).min(100), Ordering::Relaxed);
    }
    engine::refold();
}

pub fn set_listener(position: [f64; 3], forward: [f32; 3], right: [f32; 3]) {
    if let Ok(mut listener) = LISTENER.lock() {
        *listener = Listener {
            position,
            forward,
            right,
        };
    }
}

fn gain(category: SoundCategory) -> f32 {
    VOLUMES[category.index()].load(Ordering::Relaxed) as f32 / 100.0
}

pub fn play(event: &str, category: SoundCategory, volume: f32, pitch: f32) {
    debug_assert_ne!(category, SoundCategory::Master, "{event} played on Master");
    submit(event, category, volume, pitch, None, &mut roll);
}

pub fn play_at(event: &str, category: SoundCategory, at: [f64; 3], volume: f32, pitch: f32) {
    debug_assert_ne!(category, SoundCategory::Master, "{event} played on Master");
    submit(event, category, volume, pitch, Some(at), &mut roll);
}

pub fn play_seeded(
    event: &str,
    category: SoundCategory,
    at: [f64; 3],
    volume: f32,
    pitch: f32,
    seed: i64,
) {
    let mut rng = crate::util::javarandom::JavaRandom::new(seed);
    submit(event, category, volume, pitch, Some(at), &mut |bound| {
        rng.next_int(bound.max(1))
    });
}

pub fn stop(category: Option<SoundCategory>, name: Option<&str>) {
    let event = name.map(defs::known);
    if event == Some(None) {
        return;
    }
    engine::stop(category, event.flatten());
}

fn bus_gain(category: SoundCategory) -> f32 {
    let master = gain(SoundCategory::Master);
    if category == SoundCategory::Master {
        master
    } else {
        gain(category) * master
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct LoopId(u64);

static NEXT_LOOP: AtomicU64 = AtomicU64::new(1);

pub fn start_loop(event: &str, category: SoundCategory, volume: f32, pitch: f32) -> Option<LoopId> {
    debug_assert_ne!(category, SoundCategory::Master, "{event} looped on Master");
    start_held(
        event,
        category,
        volume,
        pitch,
        Spot::Nowhere,
        None,
        &mut roll,
    )
    .map(|(id, _, _)| id)
}

pub struct Tracked {
    placed: Placed,
    live: Arc<AtomicUsize>,
    key: Arc<str>,
}

#[derive(Clone, Copy)]
pub struct Placed {
    id: LoopId,
    range: f32,
}

impl Placed {
    pub fn id(&self) -> LoopId {
        self.id
    }
}

impl Tracked {
    pub fn id(&self) -> LoopId {
        self.placed.id
    }

    pub fn placed(&self) -> &Placed {
        &self.placed
    }

    fn key(&self) -> &str {
        &self.key
    }

    pub fn active(&self) -> bool {
        self.live.load(Ordering::Relaxed) == 1
    }
}

fn start_tracked(
    event: &str,
    category: SoundCategory,
    volume: f32,
    at: Option<[f64; 3]>,
) -> Option<Tracked> {
    let spot = at.map_or(Spot::Nowhere, Spot::Linear);
    start_watched(event, category, volume, 1.0, spot, &mut roll)
}

struct Preview {
    active: Option<Tracked>,
    previous: Option<SoundCategory>,
}

static PREVIEW: Mutex<Preview> = Mutex::new(Preview {
    active: None,
    previous: None,
});

static IN_WORLD: AtomicBool = AtomicBool::new(false);

fn preview_event(category: SoundCategory) -> Option<&'static str> {
    match category {
        SoundCategory::Records => Some("block.note_block.guitar"),
        SoundCategory::Weather => Some("entity.lightning_bolt.thunder"),
        SoundCategory::Blocks => Some("block.grass.place"),
        SoundCategory::Hostile => Some("entity.zombie.ambient"),
        SoundCategory::Neutral => Some("entity.cow.ambient"),
        SoundCategory::Players => Some("entity.generic.eat"),
        SoundCategory::Ambient => Some("ambient.cave"),
        SoundCategory::Ui => Some("ui.button.click"),
        SoundCategory::Master | SoundCategory::Music | SoundCategory::Voice => None,
    }
}

pub fn preview(category: SoundCategory, percent: u32) {
    if IN_WORLD.load(Ordering::Relaxed) {
        return;
    }
    let Ok(mut preview) = PREVIEW.lock() else {
        return;
    };
    let switched = preview.previous != Some(category);
    preview.previous = Some(category);
    if !switched && preview.active.as_ref().is_some_and(Tracked::active) {
        return;
    }
    if let Some(voice) = preview.active.take() {
        end_loop(voice.id());
    }
    let Some(event) = preview_event(category) else {
        return;
    };
    let volume = percent as f32 / 100.0;
    if volume * bus_gain(SoundCategory::Ui) > 0.0 {
        preview.active = start_tracked(event, SoundCategory::Ui, volume, None);
    }
}

pub fn start_bound(
    event: &str,
    category: SoundCategory,
    at: [f64; 3],
    volume: f32,
    pitch: f32,
    seed: i64,
) -> Option<Tracked> {
    if volume * bus_gain(category) <= 0.0 {
        return None;
    }
    let mut rng = crate::util::javarandom::JavaRandom::new(seed);
    start_watched(
        event,
        category,
        volume,
        pitch,
        Spot::Linear(at),
        &mut |bound| rng.next_int(bound.max(1)),
    )
}

pub fn start_flat(event: &str, category: SoundCategory, at: [f64; 3]) -> Option<Tracked> {
    start_watched(event, category, 1.0, 1.0, Spot::Flat(at), &mut roll)
}

fn start_watched(
    event: &str,
    category: SoundCategory,
    volume: f32,
    pitch: f32,
    spot: Spot,
    pick: &mut impl FnMut(u32) -> u32,
) -> Option<Tracked> {
    let live = Arc::new(AtomicUsize::new(1));
    let (id, range, key) = start_held(
        event,
        category,
        volume,
        pitch,
        spot,
        Some(live.clone()),
        pick,
    )?;
    Some(Tracked {
        placed: Placed { id, range },
        live,
        key,
    })
}

pub fn start_placed_loop(
    event: &str,
    category: SoundCategory,
    volume: f32,
    pitch: f32,
    at: [f64; 3],
) -> Option<Placed> {
    debug_assert_ne!(category, SoundCategory::Master, "{event} looped on Master");
    let (id, range, _) = start_held(
        event,
        category,
        volume,
        pitch,
        Spot::Linear(at),
        None,
        &mut roll,
    )?;
    Some(Placed { id, range })
}

pub fn start_flat_loop(
    event: &str,
    category: SoundCategory,
    volume: f32,
    pitch: f32,
    at: [f64; 3],
) -> Option<Placed> {
    let (id, range, _) = start_held(
        event,
        category,
        volume,
        pitch,
        Spot::Flat(at),
        None,
        &mut roll,
    )?;
    Some(Placed { id, range })
}

pub fn update_placed(voice: &Placed, at: [f64; 3], volume: f32, pitch: f32) {
    engine::update_loop(voice.id.0, volume, pitch, Some(held_place(at, voice.range)));
}

pub fn play_at_delayed(
    event: &'static str,
    category: SoundCategory,
    at: [f64; 3],
    volume: f32,
    pitch: f32,
) {
    let distance = relative(at).map_or(0.0, |o| (o[0] * o[0] + o[1] * o[1] + o[2] * o[2]).sqrt());
    if distance <= 10.0 {
        play_at(event, category, at, volume, pitch);
        return;
    }
    if let Ok(mut delayed) = DELAYED.lock()
        && delayed.len() < MAX_DELAYED
    {
        let ticks = (distance / 40.0 * 20.0) as u32;
        delayed.push(Delayed {
            ticks,
            event,
            category,
            at,
            volume,
            pitch,
        });
    }
}

struct Delayed {
    ticks: u32,
    event: &'static str,
    category: SoundCategory,
    at: [f64; 3],
    volume: f32,
    pitch: f32,
}

static DELAYED: Mutex<Vec<Delayed>> = Mutex::new(Vec::new());

const MAX_DELAYED: usize = 32;

fn tick_delayed() {
    let Ok(mut delayed) = DELAYED.lock() else {
        return;
    };
    delayed.retain_mut(|sound| {
        if sound.ticks > 0 {
            sound.ticks -= 1;
            return true;
        }
        play_at(
            sound.event,
            sound.category,
            sound.at,
            sound.volume,
            sound.pitch,
        );
        false
    });
}

static START_ORDER: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy)]
enum Spot {
    Nowhere,
    Linear([f64; 3]),
    Flat([f64; 3]),
}

fn start_held(
    event: &str,
    category: SoundCategory,
    volume: f32,
    pitch: f32,
    spot: Spot,
    live: Option<Arc<AtomicUsize>>,
    pick: &mut impl FnMut(u32) -> u32,
) -> Option<(LoopId, f32, Arc<str>)> {
    let defs = defs()?;
    let (name, entry) = defs.resolve(event, pick)?;
    let linear = entry.attenuation * (volume * entry.volume).max(1.0);
    let (at, range) = match spot {
        Spot::Nowhere => (None, linear),
        Spot::Linear(at) => (Some(at), linear),
        Spot::Flat(at) => (Some(at), f32::INFINITY),
    };
    note_subtitle(&defs, name, at, range);
    let place = at.map(|at| held_place(at, range));
    let order = START_ORDER.lock().ok()?;
    let id = NEXT_LOOP.fetch_add(1, Ordering::Relaxed);
    let key = entry.object_key();
    let sent = engine::start_loop(engine::StartLoop {
        id,
        key: key.clone(),
        event: name,
        category,
        entry_volume: entry.volume,
        entry_pitch: entry.pitch,
        volume,
        pitch,
        stream: entry.stream,
        place,
        live,
    });
    drop(order);
    sent.then_some((LoopId(id), range, key))
}

fn held_place(at: [f64; 3], range: f32) -> engine::Place {
    let (attenuation, pan) =
        relative(at).map_or((0.0, [0.0; 3]), |offset| linear_attenuation(offset, range));
    engine::Place { attenuation, pan }
}

pub fn update_loop(id: LoopId, volume: f32, pitch: f32) {
    engine::update_loop(id.0, volume, pitch, None);
}

pub fn end_loop(id: LoopId) {
    engine::end_loop(id.0);
}

fn relative(at: [f64; 3]) -> Option<[f32; 3]> {
    let listener = *LISTENER.lock().ok()?;
    let offset = listener.offset_to(at);

    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    Some([
        dot(offset, listener.right),
        offset[1],
        -dot(offset, listener.forward),
    ])
}

fn submit(
    event: &str,
    category: SoundCategory,
    volume: f32,
    pitch: f32,
    at: Option<[f64; 3]>,
    roll: &mut impl FnMut(u32) -> u32,
) {
    let Some(defs) = defs() else {
        return;
    };
    let Some((name, entry)) = defs.resolve(event, roll) else {
        return;
    };
    let instance_volume = volume * entry.volume;
    let range = entry.attenuation * instance_volume.max(1.0);
    note_subtitle(&defs, name, at, range);

    let bus = bus_gain(category);
    if bus <= 0.0 {
        return;
    }
    let (attenuation, relative) = match at {
        None => (1.0, None),
        Some(at) => {
            let Some(offset) = relative(at) else {
                return;
            };
            let (gain, pan) = linear_attenuation(offset, range);
            (gain, Some(pan))
        }
    };
    if attenuation <= 0.0 {
        return;
    }
    engine::play(engine::Play {
        key: entry.object_key(),
        event: name,
        category,
        volume: instance_volume.clamp(0.0, 1.0) * bus * attenuation,
        pitch: (pitch * entry.pitch).clamp(0.5, 2.0),
        relative,
        stream: entry.stream,
    });
}

static SUBTITLES: AtomicBool = AtomicBool::new(false);

static HEARD: Mutex<Vec<Heard>> = Mutex::new(Vec::new());

const MAX_HEARD: usize = 64;

pub struct Heard {
    pub key: &'static str,
    pub at: Option<[f64; 3]>,
    pub range: f32,
}

pub fn set_subtitles(on: bool) {
    SUBTITLES.store(on, Ordering::Relaxed);
}

pub fn take_heard(into: &mut Vec<Heard>) {
    if SUBTITLES.load(Ordering::Relaxed)
        && let Ok(mut heard) = HEARD.lock()
    {
        into.append(&mut heard);
    }
}

pub fn listener() -> ([f64; 3], [f32; 3], [f32; 3]) {
    let listener = LISTENER
        .lock()
        .map_or(Listener::ORIGIN, |listener| *listener);
    (listener.position, listener.forward, listener.right)
}

fn note_subtitle(defs: &defs::Defs, event: &str, at: Option<[f64; 3]>, range: f32) {
    if !SUBTITLES.load(Ordering::Relaxed) {
        return;
    }
    let Some(key) = defs.subtitle(event) else {
        return;
    };
    let range = if at.is_some() { range } else { f32::INFINITY };
    if let Ok(mut heard) = HEARD.lock()
        && heard.len() < MAX_HEARD
    {
        heard.push(Heard { key, at, range });
    }
}

pub struct Scene {
    pub in_world: bool,
    pub credits: bool,
    pub creative: bool,
    pub spectator: bool,
    pub boss_music: bool,
    pub underwater: bool,
    pub feet: [f64; 3],
    pub eye: [f64; 3],
}

const NEW_PLAYER: u8 = 1;
const STOPPED: u8 = 2;
const STOP_MUSIC: u8 = 4;

static PENDING: AtomicU8 = AtomicU8::new(0);

pub fn level_login() {
    stop_all();
    PENDING.fetch_or(NEW_PLAYER | STOPPED, Ordering::Relaxed);
}

pub fn level_respawn(dimension_changed: bool) {
    if dimension_changed {
        stop_all();
        PENDING.fetch_or(NEW_PLAYER | STOPPED | STOP_MUSIC, Ordering::Relaxed);
    } else {
        PENDING.fetch_or(NEW_PLAYER, Ordering::Relaxed);
    }
}

fn stop_all() {
    stop(None, None);
    music::clear_jukeboxes();
    if let Ok(mut delayed) = DELAYED.lock() {
        delayed.clear();
    }
}

static CREDITS: AtomicBool = AtomicBool::new(false);

pub fn credits_up() -> bool {
    CREDITS.load(Ordering::Relaxed)
}

struct Loaded {
    attributes: attributes::Attributes,
    defs: Arc<defs::Defs>,
    synced: Option<Arc<attributes::Synced>>,
}

impl Loaded {
    fn current(&self, defs: &Arc<defs::Defs>, synced: &Option<Arc<attributes::Synced>>) -> bool {
        let same_synced = match (&self.synced, synced) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        };
        same_synced && Arc::ptr_eq(&self.defs, defs)
    }
}

#[derive(Default)]
pub struct Director {
    music: music::MusicManager,
    ambience: ambience::Ambience,
    attributes: Option<Loaded>,
    in_world: bool,
    credits: bool,
}

impl Director {
    pub fn tick(&mut self, scene: &Scene) {
        #[cfg(target_arch = "wasm32")]
        engine::service();
        let Some(defs) = defs() else {
            return;
        };
        let pending = PENDING.swap(0, Ordering::Relaxed);
        if pending & NEW_PLAYER != 0 {
            self.ambience.new_player(pending & STOPPED != 0);
        }
        if pending & STOP_MUSIC != 0 {
            self.music.stop_playing();
        }
        if self.in_world && !scene.in_world {
            stop_all();
            self.ambience.new_player(true);
        }
        self.in_world = scene.in_world;
        IN_WORLD.store(scene.in_world, Ordering::Relaxed);
        tick_delayed();
        if self.credits && !scene.credits {
            self.music.stop_playing_track(attributes::Music::CREDITS);
        }
        self.credits = scene.credits;
        CREDITS.store(scene.credits, Ordering::Relaxed);

        if !scene.in_world {
            let music = if scene.credits {
                attributes::Music::CREDITS
            } else {
                attributes::Music::MENU
            };
            self.music.tick(Some(music), 1.0);
            return;
        }

        let synced = attributes::synced();
        if !self
            .attributes
            .as_ref()
            .is_some_and(|loaded| loaded.current(&defs, &synced))
        {
            let attributes = attributes::Attributes::load(&defs, synced.as_deref());
            self.attributes = Some(Loaded {
                attributes,
                defs,
                synced,
            });
        }
        let Some(Loaded { attributes, .. }) = &self.attributes else {
            return;
        };
        let dimension = crate::renderer::dimension::current();
        let cell = attributes::Cell {
            row: crate::client::envprobe::camera_biome(),
            id: crate::client::envprobe::camera_biome_id(),
        };

        let (music, volume) = if scene.credits {
            (Some(attributes::Music::CREDITS), 1.0)
        } else {
            let music =
                if dimension == crate::renderer::dimension::Dimension::End && scene.boss_music {
                    Some(attributes::Music::END_BOSS)
                } else {
                    attributes
                        .music(cell, dimension)
                        .and_then(|music| music.select(scene.creative, scene.underwater))
                };
            (music, attributes.music_volume(cell, dimension))
        };
        self.music.tick(music, volume);
        self.ambience
            .tick(scene, attributes.ambient(cell, dimension));
        music::tick_jukeboxes();
    }
}

fn linear_attenuation(offset: [f32; 3], max: f32) -> (f32, [f32; 3]) {
    let distance = (offset[0] * offset[0] + offset[1] * offset[1] + offset[2] * offset[2]).sqrt();
    let gain = (1.0 - distance / max).clamp(0.0, 1.0);
    if distance <= f32::EPSILON {
        return (gain, [0.0; 3]);
    }
    let scale = voice::PAN_RADIUS / distance;
    (
        gain,
        [offset[0] * scale, offset[1] * scale, offset[2] * scale],
    )
}

fn with_rng<R>(f: impl FnOnce(&mut crate::util::javarandom::JavaRandom) -> R) -> Option<R> {
    use crate::util::javarandom::JavaRandom;
    static RNG: Mutex<Option<JavaRandom>> = Mutex::new(None);
    let mut guard = RNG.lock().ok()?;
    let rng = guard.get_or_insert_with(|| {
        let elapsed = crate::platform::time::epoch().elapsed().as_nanos() as i64;
        JavaRandom::new(elapsed ^ (&raw const RNG as usize as i64))
    });
    Some(f(rng))
}

fn roll(bound: u32) -> u32 {
    with_rng(|rng| rng.next_int(bound.max(1))).unwrap_or(0)
}

pub fn random_f32() -> f32 {
    with_rng(|rng| rng.next_f32()).unwrap_or(0.0)
}

pub fn random_seed() -> i64 {
    with_rng(|rng| rng.next_f64().to_bits() as i64).unwrap_or(0)
}
