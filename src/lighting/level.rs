use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use azalea::block::BlockState;
use azalea_core::position::{BlockPos, ChunkPos as AzChunkPos, ChunkSectionBlockPos};
use azalea_registry::builtin::BlockKind;
use azalea_world::{Chunk, World};
use parking_lot::RwLock;

use super::data_layer::DataLayer;
use super::engine::{LightEngine, LightWorld};
use super::props;
use super::sources::SkySources;
use super::storage::Layer;
use super::{ColumnPos, SectionPos};

pub struct SectionLight {
    pub block: DataLayer,
    pub sky: DataLayer,
}

#[derive(Default)]
pub struct ServerLight {
    pub sky: Vec<Option<DataLayer>>,
    pub block: Vec<Option<DataLayer>>,
}

#[derive(Default)]
pub struct LightMap {
    sections: HashMap<SectionPos, Arc<SectionLight>>,
    top: HashMap<ColumnPos, i32>,
    lit: HashSet<ColumnPos>,
    lowest: i32,
}

pub enum SectionSample {
    Layers(Arc<SectionLight>),
    Uniform { block: u8, sky: u8 },
    Inherited(Arc<SectionLight>),
}

impl SectionSample {
    #[inline]
    pub fn at(&self, x: usize, y: usize, z: usize) -> (u8, u8) {
        match self {
            SectionSample::Layers(l) => (l.block.get(x, y, z), l.sky.get(x, y, z)),
            SectionSample::Uniform { block, sky } => (*block, *sky),
            SectionSample::Inherited(l) => (0, l.sky.get(x, 0, z)),
        }
    }
}

impl LightMap {
    pub fn bytes(&self) -> (u64, usize) {
        let bytes = self
            .sections
            .values()
            .map(|s| s.block.bytes() + s.sky.bytes())
            .sum();
        (bytes, self.sections.len())
    }

    pub fn at(&self, x: i32, y: i32, z: i32) -> (u8, u8) {
        let sec = SectionPos::of_block(x, y, z);
        self.resolve(sec)
            .at((x & 15) as usize, (y & 15) as usize, (z & 15) as usize)
    }

    pub fn avg_at(&self, cells: &[(i32, i32, i32)]) -> (u8, u8) {
        let (mut block_sum, mut sky_sum) = (0u32, 0u32);
        let mut cached: Option<(SectionPos, SectionSample)> = None;
        for &(x, y, z) in cells {
            let sec = SectionPos::of_block(x, y, z);
            if cached.as_ref().is_none_or(|(at, _)| *at != sec) {
                cached = Some((sec, self.resolve(sec)));
            }
            let sample = &cached.as_ref().expect("resolved above").1;
            let (b, s) = sample.at((x & 15) as usize, (y & 15) as usize, (z & 15) as usize);
            block_sum += b as u32;
            sky_sum += s as u32;
        }
        let n = cells.len().max(1) as u32;
        (
            ((block_sum + n / 2) / n) as u8,
            ((sky_sum + n / 2) / n) as u8,
        )
    }

    pub fn resolve(&self, sec: SectionPos) -> SectionSample {
        if let Some(layers) = self.sections.get(&sec) {
            return SectionSample::Layers(layers.clone());
        }
        let col = sec.column();
        let lit = self.lit.contains(&col);
        let top = self.top.get(&col).copied().unwrap_or(self.lowest);
        if top == self.lowest || sec.y >= top {
            return SectionSample::Uniform {
                block: 0,
                sky: if lit { 15 } else { 0 },
            };
        }
        let mut probe = sec;
        loop {
            probe.y += 1;
            if probe.y >= top {
                return SectionSample::Uniform { block: 0, sky: 15 };
            }
            if let Some(layers) = self.sections.get(&probe) {
                return SectionSample::Inherited(layers.clone());
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn section_layers(&self, sec: SectionPos) -> Option<&SectionLight> {
        self.sections.get(&sec).map(|l| &**l)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn column_state(&self, col: ColumnPos) -> (bool, Option<i32>) {
        (self.lit.contains(&col), self.top.get(&col).copied())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn lowest_section(&self) -> i32 {
        self.lowest
    }

    #[cfg(target_arch = "wasm32")]
    pub fn set_lowest_section(&mut self, y: i32) {
        self.lowest = y;
    }

    #[cfg(target_arch = "wasm32")]
    pub fn put_section(&mut self, sec: SectionPos, layers: Option<SectionLight>) {
        match layers {
            Some(layers) => {
                self.sections.insert(sec, Arc::new(layers));
            }
            None => {
                self.sections.remove(&sec);
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn set_column(&mut self, col: ColumnPos, lit: bool, top: Option<i32>) {
        if lit {
            self.lit.insert(col);
        } else {
            self.lit.remove(&col);
        }
        match top {
            Some(top) => {
                self.top.insert(col, top);
            }
            None => {
                self.top.remove(&col);
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn drop_column(&mut self, col: ColumnPos) {
        self.lit.remove(&col);
        self.top.remove(&col);
    }
}

static ENGINE_BYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static ENGINE_SECTIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn engine_bytes() -> (u64, usize) {
    use std::sync::atomic::Ordering::Relaxed;
    (
        ENGINE_BYTES.load(Relaxed),
        ENGINE_SECTIONS.load(Relaxed) as usize,
    )
}

struct WorldView {
    world: Arc<RwLock<World>>,
    chunks: HashMap<ColumnPos, Option<Arc<RwLock<Chunk>>>>,
    min_y: i32,
    height: u32,
}

impl WorldView {
    fn new(world: Arc<RwLock<World>>) -> Self {
        let (min_y, height) = {
            let w = world.read();
            (w.chunks.min_y(), w.chunks.height())
        };
        Self {
            world,
            chunks: HashMap::new(),
            min_y,
            height,
        }
    }

    fn max_y(&self) -> i32 {
        self.min_y + self.height as i32 - 1
    }

    fn chunk(&mut self, col: ColumnPos) -> Option<Arc<RwLock<Chunk>>> {
        if let Some(hit) = self.chunks.get(&col) {
            return hit.clone();
        }
        let found = self
            .world
            .read()
            .chunks
            .get(&AzChunkPos { x: col.x, z: col.z });
        self.chunks.insert(col, found.clone());
        found
    }

    fn section_empty(&mut self, sec: SectionPos) -> bool {
        let min_y = self.min_y;
        let Some(chunk) = self.chunk(sec.column()) else {
            return true;
        };
        let chunk = chunk.read();
        let index = sec.y - (min_y >> 4);
        match usize::try_from(index)
            .ok()
            .and_then(|i| chunk.sections.get(i))
        {
            Some(section) => section.block_count == 0,
            None => true,
        }
    }

    fn highest_filled_top(&mut self, col: ColumnPos) -> Option<i32> {
        let min_y = self.min_y;
        let chunk = self.chunk(col)?;
        let chunk = chunk.read();
        let index = chunk.sections.iter().rposition(|s| s.block_count != 0)?;
        Some(((min_y >> 4) + index as i32 + 1) << 4)
    }

    fn section_may_emit(&mut self, sec: SectionPos) -> bool {
        use azalea_world::palette::Palette;
        let min_y = self.min_y;
        let Some(chunk) = self.chunk(sec.column()) else {
            return false;
        };
        let chunk = chunk.read();
        let index = sec.y - (min_y >> 4);
        let Some(section) = usize::try_from(index)
            .ok()
            .and_then(|i| chunk.sections.get(i))
        else {
            return false;
        };
        let emits = |state: &BlockState| props::of(*state).emission != 0;
        match &section.states.palette {
            Palette::SingleValue(v) => emits(v),
            Palette::Linear(v) | Palette::Hashmap(v) => v.iter().any(emits),
            Palette::Global => true,
        }
    }

    fn clear(&mut self) {
        self.chunks.clear();
    }
}

impl LightWorld for RefCellView<'_> {
    fn block_state(&self, x: i32, y: i32, z: i32) -> BlockState {
        self.0.borrow_mut().state_at(x, y, z)
    }

    fn section_may_emit(&self, sec: SectionPos) -> bool {
        self.0.borrow_mut().section_may_emit(sec)
    }
}

struct RefCellView<'a>(&'a std::cell::RefCell<WorldView>);

fn state_in_chunk(
    chunk: &Chunk,
    min_y: i32,
    max_y: i32,
    lx: usize,
    y: i32,
    lz: usize,
) -> BlockState {
    if y < min_y || y > max_y {
        return BlockState::AIR;
    }
    let index = ((y - min_y) >> 4) as usize;
    match chunk.sections.get(index) {
        Some(section) => section.get_block_state(ChunkSectionBlockPos {
            x: lx as u8,
            y: (y & 15) as u8,
            z: lz as u8,
        }),
        None => BlockState::AIR,
    }
}

impl WorldView {
    fn state_at(&mut self, x: i32, y: i32, z: i32) -> BlockState {
        if y < self.min_y || y > self.max_y() {
            return BlockState::AIR;
        }
        let col = ColumnPos {
            x: x >> 4,
            z: z >> 4,
        };
        let min_y = self.min_y;
        let Some(chunk) = self.chunk(col) else {
            return BlockState::from(BlockKind::Bedrock);
        };
        let chunk = chunk.read();
        let index = ((y - min_y) >> 4) as usize;
        match chunk.sections.get(index) {
            Some(section) => section.get_block_state(ChunkSectionBlockPos {
                x: (x & 15) as u8,
                y: (y & 15) as u8,
                z: (z & 15) as u8,
            }),
            None => BlockState::AIR,
        }
    }
}

pub struct LevelLight {
    block: LightEngine,
    sky: LightEngine,
    sources: SkySources,
    min_y: i32,
    section_count: i32,
}

impl LevelLight {
    fn new(min_y: i32, height: u32) -> Self {
        Self {
            block: LightEngine::new(Layer::Block),
            sky: LightEngine::new(Layer::Sky),
            sources: SkySources::new(min_y),
            min_y,
            section_count: (height / 16) as i32,
        }
    }

    fn bottom_section(&self) -> i32 {
        self.min_y >> 4
    }

    fn add_chunk(&mut self, view: &std::cell::RefCell<WorldView>, col: ColumnPos) {
        let bottom = self.bottom_section();
        self.register_sections(view, col);
        self.fill_sources(view, col);

        self.sky.set_sky_light_enabled(&self.sources, col);
        self.sky.propagate_sky_light_sources(&self.sources, col);
        self.block.propagate_block_light_sources(
            &RefCellView(view),
            col,
            bottom,
            bottom + self.section_count,
        );
    }

    fn register_sections(&mut self, view: &std::cell::RefCell<WorldView>, col: ColumnPos) {
        let bottom = self.bottom_section();
        for i in -1..=self.section_count {
            let sec = col.section(bottom + i);
            let empty = view.borrow_mut().section_empty(sec);
            self.block.update_section_status(sec, empty);
            self.sky.update_section_status(sec, empty);
        }
    }

    fn fill_sources(&mut self, view: &std::cell::RefCell<WorldView>, col: ColumnPos) {
        let mut sources = self.sources.new_chunk_sources();
        let top_y = view.borrow_mut().highest_filled_top(col);
        if let Some(top_y) = top_y {
            let mut v = view.borrow_mut();
            let min_y = v.min_y;
            let max_y = v.max_y();
            match v.chunk(col) {
                Some(chunk) => {
                    let chunk = chunk.read();
                    sources.fill_from(top_y, |lx, y, lz| {
                        props::of(state_in_chunk(&chunk, min_y, max_y, lx, y, lz))
                    });
                }
                None => sources.fill_from(top_y, |_, _, _| {
                    props::of(BlockState::from(BlockKind::Bedrock))
                }),
            }
        }
        self.sources.insert(col, sources);
    }

    fn apply_server_light(
        &mut self,
        view: &std::cell::RefCell<WorldView>,
        col: ColumnPos,
        light: &ServerLight,
    ) {
        let bottom = self.bottom_section();
        for i in 0..=(self.section_count + 1) {
            let sec = col.section(bottom - 1 + i);
            let idx = i as usize;
            self.sky
                .storage
                .queue_section_data(sec, light.sky.get(idx).cloned().flatten());
            self.block
                .storage
                .queue_section_data(sec, light.block.get(idx).cloned().flatten());
        }

        self.register_sections(view, col);
        self.sky.storage.install_queued(&[col]);
        self.block.storage.install_queued(&[col]);
        self.fill_sources(view, col);

        self.sky.storage.set_light_enabled(col, true);
        self.block.storage.set_light_enabled(col, true);
    }

    fn apply_light_update(&mut self, col: ColumnPos, light: &ServerLight) {
        let bottom = self.bottom_section();
        for i in 0..=(self.section_count + 1) {
            let sec = col.section(bottom - 1 + i);
            let idx = i as usize;
            self.sky
                .storage
                .queue_section_data(sec, light.sky.get(idx).cloned().flatten());
            self.block
                .storage
                .queue_section_data(sec, light.block.get(idx).cloned().flatten());
        }

        self.sky.storage.install_queued(&[col]);
        self.block.storage.install_queued(&[col]);
        self.sky.storage.set_light_enabled(col, true);
        self.block.storage.set_light_enabled(col, true);
    }

    fn bytes(&self) -> (u64, usize) {
        let (block, block_secs) = self.block.bytes();
        let (sky, sky_secs) = self.sky.bytes();
        let (sources, _) = self.sources.bytes();
        (block + sky + sources, block_secs.max(sky_secs))
    }

    fn remove_chunk(&mut self, col: ColumnPos) {
        let bottom = self.bottom_section();
        for i in -1..=self.section_count {
            let sec = col.section(bottom + i);
            self.block.update_section_status(sec, true);
            self.sky.update_section_status(sec, true);
        }
        self.block.storage.set_light_enabled(col, false);
        self.sky.storage.set_light_enabled(col, false);
        self.block.storage.drop_queued(col);
        self.sky.storage.drop_queued(col);
        self.sources.remove(col);
    }

    fn check_block(&mut self, view: &std::cell::RefCell<WorldView>, pos: BlockPos) {
        let col = ColumnPos {
            x: pos.x >> 4,
            z: pos.z >> 4,
        };
        let sec = SectionPos::of_block(pos.x, pos.y, pos.z);
        let empty = view.borrow_mut().section_empty(sec);
        self.block.update_section_status(sec, empty);
        self.sky.update_section_status(sec, empty);
        if let Some(sources) = self.sources.column_mut(col.x, col.z) {
            let mut v = view.borrow_mut();
            sources.update(
                (pos.x & 15) as usize,
                pos.y,
                (pos.z & 15) as usize,
                |lx, y, lz| {
                    props::of(v.state_at(col.x * 16 + lx as i32, y, col.z * 16 + lz as i32))
                },
            );
        }
        self.block.check_block(pos.x, pos.y, pos.z);
        self.sky.check_block(pos.x, pos.y, pos.z);
    }

    fn run(&mut self, view: &std::cell::RefCell<WorldView>) {
        let w = RefCellView(view);
        self.block.run_light_updates(&w, None);
        self.sky.run_light_updates(&w, Some(&self.sources));
    }
}

pub enum LightJob {
    Chunk {
        world: Arc<RwLock<World>>,
        col: ColumnPos,
        mesh: Vec<ColumnPos>,
        light: Option<ServerLight>,
    },
    Block {
        world: Arc<RwLock<World>>,
        pos: BlockPos,
    },
    Light {
        col: ColumnPos,
        light: ServerLight,
    },
    Unload {
        col: ColumnPos,
    },
    Reset,
}

pub struct LightThread {
    tx: Sender<LightJob>,
}

impl LightThread {
    pub fn send(&self, job: LightJob) {
        let _ = self.tx.send(job);
    }
}

pub fn spawn<F>(map: Arc<RwLock<LightMap>>, mut on_dirty: F) -> LightThread
where
    F: FnMut(&[SectionPos], &[(Arc<RwLock<World>>, ColumnPos)]) + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();

    #[cfg(target_arch = "wasm32")]
    {
        let worker = LightWorker::new(map);
        WEB_LIGHT.with(|cell| {
            *cell.borrow_mut() = Some(WebLight {
                rx,
                worker,
                on_dirty: Box::new(on_dirty),
            });
        });
        return LightThread { tx };
    }

    #[cfg(not(target_arch = "wasm32"))]
    std::thread::Builder::new()
        .name("light".into())
        .spawn(move || {
            crate::diag::alloc::label_thread(crate::diag::alloc::Site::Light);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                light_loop(rx, map, &mut on_dirty)
            }));
            if result.is_err() {
                crate::diag::light_thread_died();
                crate::log_error!(
                    "light",
                    "engine thread died; no further chunk will ever mesh"
                );
            }
        })
        .expect("spawn light thread");
    LightThread { tx }
}

pub struct LightWorker {
    map: Arc<RwLock<LightMap>>,
    level: Option<LevelLight>,
    view: Option<std::cell::RefCell<WorldView>>,
}

impl LightWorker {
    pub fn new(map: Arc<RwLock<LightMap>>) -> Self {
        Self {
            map,
            level: None,
            view: None,
        }
    }

    pub fn try_take(rx: &Receiver<LightJob>, max: usize) -> Option<Vec<LightJob>> {
        let mut batch = Vec::new();
        while batch.len() < max {
            let Ok(job) = rx.try_recv() else { break };
            batch.push(job);
        }
        (!batch.is_empty()).then_some(batch)
    }

    pub fn run_batch<F>(&mut self, batch: Vec<LightJob>, on_dirty: &mut F)
    where
        F: FnMut(&[SectionPos], &[(Arc<RwLock<World>>, ColumnPos)]),
    {
        let (level, view, map) = (&mut self.level, &mut self.view, &self.map);
        if let Some(v) = view.as_ref() {
            v.borrow_mut().clear();
        }

        let mut columns_added: Vec<ColumnPos> = Vec::new();
        let mut columns_removed: Vec<ColumnPos> = Vec::new();
        let mut to_mesh: Vec<(Arc<RwLock<World>>, ColumnPos)> = Vec::new();
        for job in batch {
            match job {
                LightJob::Reset => {
                    *level = None;
                    *view = None;
                    *map.write() = LightMap::default();
                    columns_added.clear();
                    to_mesh.clear();
                    continue;
                }
                LightJob::Unload { col } => {
                    if let Some(lv) = level.as_mut() {
                        lv.remove_chunk(col);
                        columns_removed.push(col);
                    }
                    to_mesh.retain(|(_, c)| *c != col);
                    columns_added.retain(|c| *c != col);
                }
                LightJob::Chunk {
                    world,
                    col,
                    mesh,
                    light,
                } => {
                    crate::prof_span!("light:add_chunk");
                    to_mesh.extend(mesh.into_iter().map(|c| (world.clone(), c)));
                    let v = view.get_or_insert_with(|| {
                        std::cell::RefCell::new(WorldView::new(world.clone()))
                    });
                    let lv = level.get_or_insert_with(|| {
                        let b = v.borrow();
                        LevelLight::new(b.min_y, b.height)
                    });
                    match light {
                        Some(light) => lv.apply_server_light(v, col, &light),
                        None => lv.add_chunk(v, col),
                    }
                    columns_added.push(col);
                }
                LightJob::Light { col, light } => {
                    crate::prof_span!("light:server_update");
                    if let Some(lv) = level.as_mut() {
                        lv.apply_light_update(col, &light);
                    }
                }
                LightJob::Block { world, pos } => {
                    crate::prof_span!("light:check_block");
                    let v = view.get_or_insert_with(|| {
                        std::cell::RefCell::new(WorldView::new(world.clone()))
                    });
                    if let Some(lv) = level.as_mut() {
                        lv.check_block(v, pos);
                    }
                }
            }
        }

        {
            use std::sync::atomic::Ordering::Relaxed;
            let (bytes, sections) = level.as_ref().map(LevelLight::bytes).unwrap_or((0, 0));
            ENGINE_BYTES.store(bytes, Relaxed);
            ENGINE_SECTIONS.store(sections as u64, Relaxed);
        }

        let (Some(lv), Some(v)) = (level.as_mut(), view.as_ref()) else {
            return;
        };
        let started = crate::platform::time::Instant::now();
        {
            crate::prof_span!("light:run");
            lv.run(v);
        }
        let dirty = {
            crate::prof_span!("light:publish");
            publish(lv, map, &columns_added, &columns_removed)
        };
        crate::diag::add(crate::diag::Stat::ColumnsLit, columns_added.len() as u64);
        crate::log_debug!(
            "light",
            "{} columns, {} unloaded, {} to mesh, {} sections dirty, {:.1} ms",
            columns_added.len(),
            columns_removed.len(),
            to_mesh.len(),
            dirty.len(),
            started.elapsed().as_secs_f32() * 1000.0
        );
        if !dirty.is_empty() || !to_mesh.is_empty() {
            crate::prof_span!("light:on_dirty");
            on_dirty(&dirty, &to_mesh);
        }
    }
}

#[cfg(target_arch = "wasm32")]
struct WebLight {
    rx: Receiver<LightJob>,
    worker: LightWorker,
    #[allow(clippy::type_complexity)]
    on_dirty: Box<dyn FnMut(&[SectionPos], &[(Arc<RwLock<World>>, ColumnPos)])>,
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static WEB_LIGHT: std::cell::RefCell<Option<WebLight>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
const WEB_LIGHT_BUDGET: std::time::Duration = std::time::Duration::from_millis(3);

#[cfg(target_arch = "wasm32")]
const WEB_LIGHT_BATCH: usize = 16;

pub fn pump() {
    #[cfg(target_arch = "wasm32")]
    WEB_LIGHT.with(|cell| {
        let mut slot = cell.borrow_mut();
        let Some(state) = slot.as_mut() else { return };
        let WebLight {
            rx,
            worker,
            on_dirty,
        } = state;
        let started = crate::platform::time::Instant::now();
        while started.elapsed() < WEB_LIGHT_BUDGET {
            let Some(batch) = LightWorker::try_take(rx, WEB_LIGHT_BATCH) else {
                break;
            };
            worker.run_batch(batch, on_dirty);
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn light_loop<F: FnMut(&[SectionPos], &[(Arc<RwLock<World>>, ColumnPos)])>(
    rx: Receiver<LightJob>,
    map: Arc<RwLock<LightMap>>,
    on_dirty: &mut F,
) {
    let mut worker = LightWorker::new(map);
    while let Ok(first) = rx.recv() {
        let mut batch = vec![first];
        batch.extend(LightWorker::try_take(&rx, usize::MAX).unwrap_or_default());
        worker.run_batch(batch, on_dirty);
    }
}

fn publish(
    level: &mut LevelLight,
    map: &Arc<RwLock<LightMap>>,
    columns_added: &[ColumnPos],
    columns_removed: &[ColumnPos],
) -> Vec<SectionPos> {
    let mut affected = level.block.storage.take_affected();
    affected.extend(level.sky.storage.take_affected());

    let mut guard = map.write();
    guard.lowest = level.sky.storage.bottom_section_y();
    for sec in &affected {
        let block = level.block.storage.data_layer(*sec).cloned();
        let sky = level.sky.storage.data_layer(*sec).cloned();
        if block.is_none() && sky.is_none() {
            guard.sections.remove(sec);
        } else {
            guard.sections.insert(
                *sec,
                Arc::new(SectionLight {
                    block: block.unwrap_or_else(DataLayer::empty),
                    sky: sky.unwrap_or_else(DataLayer::empty),
                }),
            );
        }
        guard
            .top
            .insert(sec.column(), level.sky.storage.top_section_y(sec.column()));
    }
    for col in columns_added {
        guard.lit.insert(*col);
        guard
            .top
            .insert(*col, level.sky.storage.top_section_y(*col));
    }
    for col in columns_removed {
        guard.lit.remove(col);
        guard.top.remove(col);
    }
    drop(guard);
    affected.into_iter().collect()
}
