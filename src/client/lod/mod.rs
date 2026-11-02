pub mod blocks;
pub mod cache;
mod curve;
mod ingest;
pub mod mesh;
mod point;
pub mod pos;
mod reduce;
mod render_point;
pub mod render_source;
mod section;
mod store;
mod transform;
mod tree;

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Duration;

use crate::gui::LodQuality;
use crate::platform::time::Instant;

use pos::SectionPos;

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub render_distance: u32,
    pub lod_distance: u32,
    pub quality: LodQuality,
    pub eye: [f64; 3],
    pub in_world: bool,
}

impl Settings {
    const INITIAL: Settings = Settings {
        render_distance: 0,
        lod_distance: crate::gui::LOD_DISTANCE_DEFAULT,
        quality: LodQuality::Medium,
        eye: [0.0; 3],
        in_world: false,
    };

    pub fn lod_chunks(&self) -> u32 {
        self.lod_distance.max(self.render_distance + 1)
    }
}

static SETTINGS: Mutex<Settings> = Mutex::new(Settings::INITIAL);

pub fn real_radius() -> f32 {
    settings().render_distance.max(2) as f32 * 16.0 + 1.0
}

pub fn publish_settings(settings: Settings) {
    *SETTINGS.lock().unwrap_or_else(|e| e.into_inner()) = settings;
}

pub fn settings() -> Settings {
    *SETTINGS.lock().unwrap_or_else(|e| e.into_inner())
}

pub enum LodOp {
    Upload(SectionPos, mesh::LodMesh),
    Remove(SectionPos),
    Clear,
}

static OPS: Mutex<Vec<LodOp>> = Mutex::new(Vec::new());

pub fn take_ops(max: usize) -> Vec<LodOp> {
    let mut ops = OPS.lock().unwrap_or_else(|e| e.into_inner());
    let n = ops.len().min(max);
    ops.drain(..n).collect()
}

fn push_op(op: LodOp) {
    let mut ops = OPS.lock().unwrap_or_else(|e| e.into_inner());
    match &op {
        LodOp::Clear => ops.clear(),
        LodOp::Upload(pos, _) | LodOp::Remove(pos) => {
            let pos = *pos;
            ops.retain(|o| !matches!(o, LodOp::Upload(p, _) | LodOp::Remove(p) if *p == pos));
        }
    }
    ops.push(op);
}

struct Inbox {
    pending: Option<HashMap<(i32, i32), Instant>>,
    reset: bool,
}

static INBOX: Mutex<Inbox> = Mutex::new(Inbox {
    pending: None,
    reset: false,
});

const LIGHT_WAIT: Duration = Duration::from_secs(3);

pub fn note_column(cx: i32, cz: i32) {
    ensure_worker();
    INBOX
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .pending
        .get_or_insert_with(HashMap::new)
        .entry((cx, cz))
        .or_insert_with(Instant::now);
}

pub fn reset() {
    let mut inbox = INBOX.lock().unwrap_or_else(|e| e.into_inner());
    inbox.reset = true;
    if let Some(p) = inbox.pending.as_mut() {
        p.clear();
    }
    drop(inbox);
    cache::reset();
}

fn ensure_worker() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        static STARTED: std::sync::Once = std::sync::Once::new();
        STARTED.call_once(|| {
            let spawn = std::thread::Builder::new().name("lod".into());
            let _ = spawn.spawn(|| {
                let mut worker = Worker::new();
                let mut panics = 0u32;
                loop {
                    let step = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        worker.step(None)
                    }));
                    let busy = match step {
                        Ok(busy) => busy,
                        Err(_) => {
                            panics += 1;
                            crate::log_error!(
                                "lod",
                                "worker step panicked ({panics} so far); far terrain restarts empty"
                            );
                            worker = Worker::new();
                            push_op(LodOp::Clear);
                            false
                        }
                    };
                    if !busy {
                        std::thread::sleep(Duration::from_millis(30));
                    }
                }
            });
        });
    }
}

#[cfg(target_arch = "wasm32")]
pub fn drain_on_caller(budget: Duration) {
    thread_local! {
        static WORKER: std::cell::RefCell<Option<Worker>> = const { std::cell::RefCell::new(None) };
    }
    WORKER.with(|w| {
        let mut w = w.borrow_mut();
        let worker = w.get_or_insert_with(Worker::new);
        worker.step(Some(Instant::now() + budget));
    });
}

struct Worker {
    blocks: blocks::BlockTable,
    store: store::Store,
    scratch: ingest::Scratch,
    unlit: HashSet<(i32, i32)>,
    render: HashMap<SectionPos, render_source::RenderSource>,
    wanted: HashSet<SectionPos>,
    queue: Vec<SectionPos>,
    ready: HashSet<SectionPos>,
    displayed: HashSet<SectionPos>,
    stale: HashMap<SectionPos, Instant>,
    selected_for: Option<SelectKey>,
    reselect: bool,
    min_y: i32,
    max_y: i32,
    steps: u64,
}

type SelectKey = (i32, i32, i32, u32, u32, LodQuality);

const REAL_COVER_MARGIN: f64 = 32.0;

const REMESH_DELAY: Duration = Duration::from_secs(2);

const NATIVE_SLICE: Duration = Duration::from_millis(50);

impl Worker {
    fn new() -> Self {
        Self {
            blocks: blocks::BlockTable::new(),
            store: store::Store::default(),
            scratch: ingest::Scratch::default(),
            unlit: HashSet::new(),
            render: HashMap::new(),
            wanted: HashSet::new(),
            queue: Vec::new(),
            ready: HashSet::new(),
            displayed: HashSet::new(),
            stale: HashMap::new(),
            selected_for: None,
            reselect: false,
            min_y: -64,
            max_y: 320,
            steps: 0,
        }
    }

    fn clear(&mut self) {
        self.store.clear();
        self.unlit.clear();
        self.render.clear();
        self.wanted.clear();
        self.queue.clear();
        self.ready.clear();
        self.displayed.clear();
        self.stale.clear();
        self.selected_for = None;
        push_op(LodOp::Clear);
    }

    fn step(&mut self, deadline: Option<Instant>) -> bool {
        self.steps += 1;
        let deadline = deadline.unwrap_or_else(|| Instant::now() + NATIVE_SLICE);
        let past = || Instant::now() >= deadline;

        let reset = std::mem::take(&mut INBOX.lock().unwrap_or_else(|e| e.into_inner()).reset);
        if reset {
            self.clear();
        }
        if self.blocks.refresh() {
            self.render.clear();
            self.ready.clear();
            self.reselect = true;
        }

        let mut changed = HashSet::new();
        let now = Instant::now();
        let ingest_by = now + deadline.saturating_duration_since(now) / 2;
        let ingested = self.ingest(ingest_by, &mut changed);

        let air = self.blocks.air_states();
        let is_air = |id: u32| air.contains(&point::id_state(id));
        self.store.propagate(is_air, &mut changed, Some(deadline));

        let now = Instant::now();
        for pos in &changed {
            self.render.remove(pos);
            for p in std::iter::once(*pos).chain((0..4).map(|d| pos.adjacent(d))) {
                if self.displayed.contains(&p) {
                    self.stale.entry(p).or_insert(now);
                } else {
                    self.ready.remove(&p);
                }
            }
        }
        if !changed.is_empty() {
            self.reselect = true;
        }
        let due: Vec<SectionPos> = self
            .stale
            .iter()
            .filter(|(_, since)| now.duration_since(**since) >= REMESH_DELAY)
            .map(|(p, _)| *p)
            .collect();
        for p in due {
            self.stale.remove(&p);
            self.ready.remove(&p);
            if self.wanted.contains(&p) {
                self.queue.push(p);
            }
        }

        let settings = settings();
        if !settings.in_world {
            return ingested > 0;
        }
        let eye = (settings.eye[0], settings.eye[2]);
        let key = (
            (eye.0.floor() as i32) >> 4,
            (settings.eye[1].floor() as i32) >> 4,
            (eye.1.floor() as i32) >> 4,
            settings.render_distance,
            settings.lod_chunks(),
            settings.quality,
        );
        if self.reselect || self.selected_for != Some(key) {
            self.select(eye, &settings, key);
        }

        let mut meshed = 0;
        while let Some(pos) = self.queue.pop() {
            if past() && meshed > 0 {
                self.queue.push(pos);
                break;
            }
            self.mesh(pos);
            meshed += 1;
        }
        self.retire();

        if self.steps % 200 == 0 {
            let radius = f64::from(settings.lod_chunks()) * 16.0;
            let keep = radius * 1.5 + f64::from(pos::WIDTH << pos::MAX_DETAIL);
            self.store.prune(eye, keep);
            self.unlit.retain(|&(cx, cz)| {
                let (dx, dz) = (
                    f64::from(cx * 16 + 8) - eye.0,
                    f64::from(cz * 16 + 8) - eye.1,
                );
                dx * dx + dz * dz <= keep * keep
            });
        }

        ingested > 0 || meshed > 0 || !self.store.is_settled()
    }

    fn ingest(&mut self, deadline: Instant, changed: &mut HashSet<SectionPos>) -> usize {
        let Some(world) = crate::client::tracking::current_world()
            .lock()
            .unwrap()
            .clone()
        else {
            return 0;
        };
        let lighting = crate::renderer::lighting_enabled();
        let ready: Vec<((i32, i32), bool)> = {
            let light = crate::client::worldsync::light_map().read();
            let is_lit =
                |c: (i32, i32)| light.is_lit(crate::lighting::ColumnPos { x: c.0, z: c.1 });
            let mut inbox = INBOX.lock().unwrap_or_else(|e| e.into_inner());
            let now = Instant::now();
            let pending = inbox.pending.get_or_insert_with(HashMap::new);
            if lighting {
                self.unlit.retain(|&c| {
                    if is_lit(c) {
                        pending.entry(c).or_insert(now);
                        false
                    } else {
                        true
                    }
                });
            }
            let ready: Vec<_> = pending
                .iter()
                .filter_map(|(&c, since)| {
                    let lit = !lighting || is_lit(c);
                    (lit || now.duration_since(*since) >= LIGHT_WAIT).then_some((c, lit))
                })
                .collect();
            for (c, _) in &ready {
                pending.remove(c);
            }
            ready
        };
        let light_map = crate::client::worldsync::light_map();
        let mut done = 0;
        for (i, &((cx, cz), lit)) in ready.iter().enumerate() {
            if Instant::now() >= deadline && done > 0 {
                let mut inbox = INBOX.lock().unwrap_or_else(|e| e.into_inner());
                let pending = inbox.pending.get_or_insert_with(HashMap::new);
                for &(c, _) in &ready[i..] {
                    pending.entry(c).or_insert_with(Instant::now);
                }
                break;
            }
            let (chunk, min_y, height, biomes) = {
                let w = world.read();
                let Some(chunk) = w.chunks.get(&azalea_core::position::ChunkPos::new(cx, cz))
                else {
                    continue;
                };
                (
                    chunk,
                    w.chunks.min_y(),
                    w.chunks.height() as i32,
                    crate::client::worker::biome_table(&w.registries),
                )
            };
            self.min_y = min_y;
            self.max_y = min_y + height;
            let light = if !lighting {
                ingest::LightFrom::Off
            } else if lit {
                self.unlit.remove(&(cx, cz));
                ingest::LightFrom::Engine(light_map)
            } else {
                self.unlit.insert((cx, cz));
                ingest::LightFrom::Unlit
            };
            let columns = {
                let chunk = chunk.read();
                ingest::chunk_columns(
                    &chunk,
                    cx,
                    cz,
                    min_y,
                    height,
                    &biomes,
                    light,
                    &mut self.blocks,
                    &mut self.scratch,
                )
            };
            self.store.write_chunk(cx, cz, &columns, changed);
            done += 1;
        }
        done
    }

    fn select(&mut self, eye: (f64, f64), settings: &Settings, key: SelectKey) {
        self.reselect = false;
        self.selected_for = Some(key);
        let curve = curve::Curve::new(settings.quality);
        let real = tree::RealArea {
            eye: settings.eye,
            radius: (f64::from(real_radius()) - REAL_COVER_MARGIN).max(0.0),
            min_y: self.min_y,
            max_y: self.max_y,
        };
        let radius = f64::from(settings.lod_chunks()) * 16.0;
        self.wanted = tree::select(&self.store, eye, radius, real, &curve);
        self.ready
            .retain(|p| self.wanted.contains(p) || self.displayed.contains(p));
        let mut queue: Vec<SectionPos> = self
            .wanted
            .iter()
            .copied()
            .filter(|p| !self.ready.contains(p))
            .collect();
        let dist = |p: &SectionPos| store::nearest_distance(*p, eye);
        queue.sort_by(|a, b| dist(b).total_cmp(&dist(a)));
        self.queue = queue;
    }

    fn render_source(&mut self, pos: SectionPos) -> Option<&render_source::RenderSource> {
        if !self.render.contains_key(&pos) {
            let full = self.store.get(pos)?;
            let dim = crate::renderer::dimension::current();
            let cave_culling =
                !dim.has_ceiling() && dim != crate::renderer::dimension::Dimension::End;
            let rs = transform::to_render_source(
                full,
                &mut self.blocks,
                self.min_y,
                curve::max_vertical_slices(pos.detail),
                cave_culling,
            );
            self.render.insert(pos, rs);
        }
        self.render.get(&pos)
    }

    fn mesh(&mut self, pos: SectionPos) {
        if !self.wanted.contains(&pos) {
            return;
        }
        self.render_source(pos);
        for d in 0..4 {
            self.render_source(pos.adjacent(d));
        }
        let Some(center) = self.render.get(&pos) else {
            self.ready.insert(pos);
            return;
        };
        let adjacent: [Option<&render_source::RenderSource>; 4] =
            std::array::from_fn(|d| self.render.get(&pos.adjacent(d)));
        let shade = crate::renderer::dimension::current().cardinal();
        let built = mesh::build(center, adjacent, shade);
        if built.is_empty() {
            if self.displayed.remove(&pos) {
                push_op(LodOp::Remove(pos));
            }
        } else {
            self.displayed.insert(pos);
            push_op(LodOp::Upload(pos, built));
        }
        self.ready.insert(pos);
    }

    fn retire(&mut self) {
        let stale: Vec<SectionPos> = self
            .displayed
            .iter()
            .copied()
            .filter(|d| !self.wanted.contains(d))
            .filter(|d| {
                self.wanted
                    .iter()
                    .filter(|w| w.is_within(*d) || d.is_within(**w))
                    .all(|w| self.ready.contains(w))
            })
            .collect();
        for d in stale {
            self.displayed.remove(&d);
            self.ready.remove(&d);
            self.stale.remove(&d);
            push_op(LodOp::Remove(d));
        }
        if self.queue.is_empty() && self.render.len() > self.wanted.len() * 2 {
            let keep: HashSet<SectionPos> = self
                .wanted
                .iter()
                .flat_map(|p| std::iter::once(*p).chain((0..4).map(move |d| p.adjacent(d))))
                .collect();
            self.render.retain(|p, _| keep.contains(p));
        }
    }
}
