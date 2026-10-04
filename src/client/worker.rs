use crate::platform::time::Instant;
use std::collections::{HashMap, VecDeque};
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use azalea::block::BlockState;
use azalea_core::position::{ChunkPos as AzChunkPos, ChunkSectionBiomePos, ChunkSectionBlockPos};
use azalea_core::registry_holder::RegistryHolder;
use azalea_world::{Chunk, World};
use parking_lot::RwLock;

use crate::ATLAS_ROWS;
use crate::renderer::{self, Occupancy, PendingSection, RenderedBlock, build_section_mesh};
use crate::session::{SharedMutex, SideMutex};
use crate::util::block_model::block_visual;

struct BlockCache(Vec<Option<RenderedBlock>>);

impl BlockCache {
    fn new() -> Self {
        Self(Vec::new())
    }

    #[inline]
    fn get(&mut self, state: BlockState) -> &RenderedBlock {
        let id = state.id() as usize;
        if id >= self.0.len() {
            self.0.resize(id + 1, None);
        }
        self.0[id].get_or_insert_with(|| block_visual(state))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum JobScope {
    Column,
    Section(i32),
}

type JobKey = (i32, i32, JobScope);

struct Job {
    world: Arc<RwLock<World>>,
    chunk_x: i32,
    chunk_z: i32,
    scope: JobScope,
    seq: u64,
    sec_from: i32,
}

struct QueueInner {
    edits: VecDeque<Job>,
    relights: VecDeque<Job>,
    loads: VecDeque<Job>,
    keys: HashMap<JobKey, KeyState>,
    latest_seq: HashMap<(i32, i32), ColumnSeq>,
    next_seq: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum KeyState {
    Queued,
    InFlight { dirty_again: bool, by_edit: bool },
}

const MAX_COLUMN_SECTIONS: usize = 1024;

#[derive(Default)]
struct ColumnSeq {
    base: i32,
    seqs: Vec<u64>,
    queued_column: u64,
}

impl ColumnSeq {
    fn get(&self, sec_y: i32) -> Option<u64> {
        let i = usize::try_from((sec_y >> 4).checked_sub(self.base)?).ok()?;
        self.seqs.get(i).copied().filter(|&seq| seq != 0)
    }

    fn set(&mut self, sec_y: i32, seq: u64) {
        let sy = sec_y >> 4;
        if self.seqs.is_empty() {
            self.base = sy;
        }
        let max = MAX_COLUMN_SECTIONS as i64;
        if sy < self.base {
            let grow = self.base as i64 - sy as i64;
            if self.seqs.len() as i64 + grow > max {
                return;
            }
            self.seqs
                .splice(0..0, std::iter::repeat(0).take(grow as usize));
            self.base = sy;
        }
        let i = sy as i64 - self.base as i64;
        if i >= max {
            return;
        }
        let i = i as usize;
        if i >= self.seqs.len() {
            self.seqs.resize(i + 1, 0);
        }
        self.seqs[i] = seq;
    }
}

const MAX_PENDING_SECTIONS: usize = 512;

const DEFAULT_MESH_THREADS: usize = 2;

pub struct JobQueue {
    inner: SideMutex<QueueInner>,
    ready: parking_lot::Condvar,
    backlog: AtomicUsize,
}

impl JobQueue {
    fn new() -> Self {
        Self {
            inner: SideMutex::new(QueueInner {
                edits: VecDeque::new(),
                relights: VecDeque::new(),
                loads: VecDeque::new(),
                keys: HashMap::new(),
                latest_seq: HashMap::new(),
                next_seq: 0,
            }),
            ready: parking_lot::Condvar::new(),
            backlog: AtomicUsize::new(0),
        }
    }

    pub fn set_backlog(&self, sections: usize) {
        let before = self.backlog.swap(sections, Ordering::Relaxed);
        if before >= MAX_PENDING_SECTIONS && sections < MAX_PENDING_SECTIONS {
            let _inner = self.inner.lock().unwrap();
            self.ready.notify_all();
        }
    }

    pub fn push(&self, world: Arc<RwLock<World>>, chunk_x: i32, chunk_z: i32, scope: JobScope) {
        self.push_in(world, chunk_x, chunk_z, scope, false);
    }

    pub fn push_relight(&self, world: Arc<RwLock<World>>, chunk_x: i32, chunk_z: i32, sy: i32) {
        self.push_in(world, chunk_x, chunk_z, JobScope::Section(sy), true);
    }

    fn push_in(
        &self,
        world: Arc<RwLock<World>>,
        chunk_x: i32,
        chunk_z: i32,
        scope: JobScope,
        relight: bool,
    ) {
        let column_geometry = match scope {
            JobScope::Section(_) => None,
            JobScope::Column => {
                let w = world.read();
                Some((w.chunks.min_y(), (w.chunks.height() / 16) as i32))
            }
        };
        let mut guard = self.inner.lock().unwrap();
        let inner = &mut *guard;
        let key = (chunk_x, chunk_z, scope);
        if relight
            && let JobScope::Section(sy) = scope
            && !inner.keys.contains_key(&key)
            && let Some(column) = inner.latest_seq.get(&(chunk_x, chunk_z))
            && column.queued_column != 0
            && column.get(sy * 16) == Some(column.queued_column)
        {
            return;
        }
        match inner.keys.get_mut(&key) {
            Some(KeyState::Queued) => {
                if !relight
                    && !inner.relights.is_empty()
                    && let Some(i) = inner
                        .relights
                        .iter()
                        .position(|j| (j.chunk_x, j.chunk_z, j.scope) == key)
                    && let Some(job) = inner.relights.remove(i)
                {
                    inner.edits.push_back(job);
                    self.ready.notify_one();
                }
                return;
            }
            Some(KeyState::InFlight {
                dirty_again,
                by_edit,
            }) => {
                *dirty_again = true;
                *by_edit |= !relight;
                return;
            }
            None => {
                inner.keys.insert(key, KeyState::Queued);
            }
        }
        inner.next_seq += 1;
        let seq = inner.next_seq;
        match (scope, column_geometry) {
            (JobScope::Section(sy), _) => {
                inner
                    .latest_seq
                    .entry((chunk_x, chunk_z))
                    .or_default()
                    .set(sy * 16, seq);
            }
            (JobScope::Column, Some((min_y, sec_count))) => {
                let column = inner.latest_seq.entry((chunk_x, chunk_z)).or_default();
                for si in 0..sec_count {
                    let sec_y = min_y + si * 16;
                    let edit = (chunk_x, chunk_z, JobScope::Section(sec_y >> 4));
                    if inner.keys.contains_key(&edit) && column.get(sec_y).is_some() {
                        continue;
                    }
                    column.set(sec_y, seq);
                }
                column.queued_column = seq;
            }
            (JobScope::Column, None) => {}
        }
        let job = Job {
            world,
            chunk_x,
            chunk_z,
            scope,
            seq,
            sec_from: 0,
        };
        match scope {
            JobScope::Section(_) if relight => inner.relights.push_back(job),
            JobScope::Section(_) => inner.edits.push_back(job),
            JobScope::Column => inner.loads.push_back(job),
        }
        self.ready.notify_one();
    }

    fn finish(&self, job_key: JobKey, world: Arc<RwLock<World>>) -> bool {
        let redo = {
            let mut inner = self.inner.lock().unwrap();
            match inner.keys.get(&job_key) {
                Some(&KeyState::InFlight {
                    dirty_again,
                    by_edit,
                }) => {
                    inner.keys.remove(&job_key);
                    dirty_again.then_some(by_edit)
                }
                _ => None,
            }
        };
        if let Some(by_edit) = redo {
            let (cx, cz, scope) = job_key;
            self.push_in(world, cx, cz, scope, !by_edit);
        }
        redo.is_some()
    }

    fn resume(&self, job: Job) {
        let mut inner = self.inner.lock().unwrap();
        inner.loads.push_front(job);
    }

    fn is_current(&self, chunk_x: i32, chunk_z: i32, sec_y: i32, seq: u64) -> bool {
        let inner = self.inner.lock().unwrap();
        inner
            .latest_seq
            .get(&(chunk_x, chunk_z))
            .and_then(|column| column.get(sec_y))
            == Some(seq)
    }

    pub fn forget_chunk(&self, chunk_x: i32, chunk_z: i32) {
        let mut guard = self.inner.lock().unwrap();
        let inner = &mut *guard;
        inner.latest_seq.remove(&(chunk_x, chunk_z));
        let mut dropped = Vec::new();
        let mut keep = |job: &Job| {
            let mine = job.chunk_x == chunk_x && job.chunk_z == chunk_z;
            if mine {
                dropped.push((job.chunk_x, job.chunk_z, job.scope));
            }
            !mine
        };
        inner.loads.retain(&mut keep);
        inner.edits.retain(&mut keep);
        inner.relights.retain(&mut keep);
        for key in dropped {
            inner.keys.remove(&key);
        }
    }

    pub fn reset(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.edits.clear();
        inner.relights.clear();
        inner.loads.clear();
        inner.keys.retain(|_, state| match state {
            KeyState::Queued => false,
            KeyState::InFlight {
                dirty_again,
                by_edit,
            } => {
                *dirty_again = false;
                *by_edit = false;
                true
            }
        });
        inner.latest_seq.clear();
    }

    fn pop(&self) -> Job {
        let mut inner = self.inner.lock().unwrap();
        loop {
            if let Some(job) = Self::take(&self.backlog, &mut inner) {
                return job;
            }
            self.ready.wait(&mut inner);
        }
    }

    fn try_pop(&self) -> Option<Job> {
        let mut inner = self.inner.lock().unwrap();
        Self::take(&self.backlog, &mut inner)
    }

    fn take(backlog: &AtomicUsize, inner: &mut QueueInner) -> Option<Job> {
        let job = inner.edits.pop_front().or_else(|| {
            if backlog.load(Ordering::Relaxed) < MAX_PENDING_SECTIONS {
                inner
                    .relights
                    .pop_front()
                    .or_else(|| inner.loads.pop_front())
            } else {
                None
            }
        })?;
        let key = (job.chunk_x, job.chunk_z, job.scope);
        if job.scope == JobScope::Column
            && let Some(column) = inner.latest_seq.get_mut(&(job.chunk_x, job.chunk_z))
            && column.queued_column == job.seq
        {
            column.queued_column = 0;
        }
        let state = inner.keys.entry(key).or_insert(KeyState::Queued);
        if *state == KeyState::Queued {
            *state = KeyState::InFlight {
                dirty_again: false,
                by_edit: false,
            };
        }
        Some(job)
    }
}

pub fn spawn_chunk_workers(shared: Arc<SharedMutex>) -> Arc<JobQueue> {
    let queue = Arc::new(JobQueue::new());
    let n_workers = if cfg!(target_arch = "wasm32") {
        0
    } else {
        std::env::var("MC_MESH_THREADS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(DEFAULT_MESH_THREADS)
    };
    shared.lock().unwrap().profiling.worker_count = n_workers;
    THREADLESS.store(n_workers == 0, Ordering::Relaxed);
    if n_workers == 0 {
        crate::log_info!(
            "mesh",
            "no chunk workers; meshing on the render thread with a {:?} budget per frame",
            mesh_budget()
        );
    } else {
        crate::log_info!("mesh", "{n_workers} chunk workers");
    }
    for i in 0..n_workers {
        let queue = queue.clone();
        let shared = shared.clone();
        let spawn = thread::Builder::new().name(format!("mesh{i}"));
        let _ = spawn.spawn(move || {
            crate::diag::alloc::label_thread(crate::diag::alloc::Site::Mesh);
            let mut scratch = Scratch::new();
            loop {
                let job = queue.pop();
                run_job(job, &queue, &shared, &mut scratch, None);
            }
        });
    }
    queue
}

pub struct Scratch {
    cache: BlockCache,
    occ: Occupancy,
}

impl Scratch {
    pub fn new() -> Self {
        Self {
            cache: BlockCache::new(),
            occ: Occupancy::new(0, 0, 0, 1),
        }
    }
}

impl Default for Scratch {
    fn default() -> Self {
        Self::new()
    }
}

fn run_job(
    job: Job,
    queue: &JobQueue,
    shared: &Arc<SharedMutex>,
    scratch: &mut Scratch,
    deadline: Option<Instant>,
) {
    let (cx, cz) = (job.chunk_x, job.chunk_z);
    let key = (job.chunk_x, job.chunk_z, job.scope);
    let world = job.world.clone();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        process_job(
            job,
            queue,
            shared,
            &mut scratch.cache,
            &mut scratch.occ,
            deadline,
        )
    }));
    let redo = queue.finish(key, world);
    match result {
        Ok(rest) => {
            if let Some(rest) = rest
                && !redo
            {
                queue.resume(rest);
            }
        }
        Err(payload) => {
            let msg = payload
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| payload.downcast_ref::<String>().map(|s| s.as_str()))
                .unwrap_or("<non-string panic payload>");
            crate::diag::bump(crate::diag::Stat::MeshPanics);
            crate::log_error!(
                "mesh",
                "job for chunk {cx},{cz} panicked: {msg}\n\
                 the worker continues, but that chunk will render as air"
            );
        }
    }
}

pub fn meshes_on_caller() -> bool {
    THREADLESS.load(Ordering::Relaxed)
}

static THREADLESS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

const DEFAULT_MESH_BUDGET: Duration = Duration::from_millis(4);

const COLUMN_STRIDE: i32 = 4;

pub fn drain_on_caller(queue: &Arc<JobQueue>, scratch: &mut Scratch) {
    if !meshes_on_caller() {
        return;
    }
    let Some(shared) = crate::SHARED.get() else {
        return;
    };
    crate::prof_span!("mesh:drain_on_caller");
    drain_for(queue, shared, scratch, mesh_budget());
}

fn mesh_budget() -> Duration {
    static BUDGET: std::sync::OnceLock<Duration> = std::sync::OnceLock::new();
    *BUDGET.get_or_init(|| {
        std::env::var("MC_MESH_BUDGET_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|ms| *ms > 0)
            .map(Duration::from_millis)
            .unwrap_or(DEFAULT_MESH_BUDGET)
    })
}

#[cfg(target_arch = "wasm32")]
pub fn drain_all(queue: &JobQueue, shared: &Arc<SharedMutex>, scratch: &mut Scratch, cap: usize) {
    while let Some(job) = queue.try_pop() {
        run_job(job, queue, shared, scratch, None);
        let s = shared.lock().unwrap();
        if s.session.pending_chunks.len() + s.session.pending_edits.len() >= cap {
            return;
        }
    }
}

pub fn drain_for(
    queue: &JobQueue,
    shared: &Arc<SharedMutex>,
    scratch: &mut Scratch,
    budget: Duration,
) -> usize {
    let started = Instant::now();
    let deadline = started + budget;
    let mut ran = 0;
    while Instant::now() < deadline {
        let Some(job) = queue.try_pop() else { break };
        run_job(job, queue, shared, scratch, Some(deadline));
        ran += 1;
    }
    ran
}

fn process_job(
    job: Job,
    queue: &JobQueue,
    shared: &Arc<SharedMutex>,
    cache: &mut BlockCache,
    occ: &mut Occupancy,
    deadline: Option<Instant>,
) -> Option<Job> {
    crate::prof_span!("mesh:process_job");
    let job_start = Instant::now();

    if let JobScope::Section(sy) = job.scope
        && !queue.is_current(job.chunk_x, job.chunk_z, sy * 16, job.seq)
    {
        crate::diag::bump(crate::diag::Stat::MeshJobsSuperseded);
        return None;
    }

    let (min_y, height, chunk_arc, neighbors, biomes) = {
        let world = job.world.read();
        let min_y = world.chunks.min_y();
        let height = world.chunks.height();
        let biomes = biome_table(&world.registries);
        let pos = AzChunkPos {
            x: job.chunk_x,
            z: job.chunk_z,
        };
        let arc = match world.chunks.get(&pos) {
            Some(a) => a,
            None => return None,
        };
        let neighbors = [
            (-1i32, 0i32),
            (1, 0),
            (0, -1),
            (0, 1),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
        ]
        .map(|(dx, dz)| {
            let p = AzChunkPos {
                x: job.chunk_x + dx,
                z: job.chunk_z + dz,
            };
            (dx, dz, world.chunks.get(&p))
        });
        (min_y, height, arc, neighbors, biomes)
    };

    let x_base = job.chunk_x * 16;
    let z_base = job.chunk_z * 16;
    let sec_count = (height / 16) as i32;
    let base_sec = min_y >> 4;

    let (sec_lo, sec_hi) = match job.scope {
        JobScope::Column => {
            let lo = job.sec_from.clamp(0, sec_count);
            let hi = match deadline {
                Some(_) => (lo + COLUMN_STRIDE).min(sec_count),
                None => sec_count,
            };
            (lo, hi)
        }
        JobScope::Section(sy) => {
            let i = sy - base_sec;
            if i < 0 || i >= sec_count {
                return None;
            }
            (i, i + 1)
        }
    };

    let win_y0 = min_y + sec_lo * 16 - 1;
    let win_count = (sec_hi - sec_lo) * 16 + 2;
    occ.reset(x_base, z_base, win_y0, win_count);

    {
        let chunk = chunk_arc.read();
        fill_occupancy(
            occ,
            &chunk,
            min_y,
            x_base,
            z_base,
            (0, 16),
            (0, 16),
            cache,
            &biomes,
        );
    }
    if crate::renderer::lighting_enabled() {
        fill_light(occ, &crate::client::worldsync::light_map().read());
    } else {
        occ.fill_light_constant(15, 15);
    }
    for (dx, dz, arc) in neighbors.iter() {
        let (dx, dz) = (*dx, *dz);
        let Some(arc) = arc else { continue };
        let (lx, lz) = match (dx, dz) {
            (-1, 0) => ((15u8, 16u8), (0u8, 16u8)),
            (1, 0) => ((0, 1), (0, 16)),
            (0, -1) => ((0, 16), (15, 16)),
            (0, 1) => ((0, 16), (0, 1)),
            (-1, -1) => ((15, 16), (15, 16)),
            (1, -1) => ((0, 1), (15, 16)),
            (-1, 1) => ((15, 16), (0, 1)),
            _ => ((0, 1), (0, 1)),
        };
        let chunk = arc.read();
        fill_occupancy(
            occ,
            &chunk,
            min_y,
            x_base + dx * 16,
            z_base + dz * 16,
            lx,
            lz,
            cache,
            &biomes,
        );
    }

    let atlas_rows = ATLAS_ROWS.get().copied().unwrap_or(renderer::TILE_PX);
    let mut results: Vec<PendingSection> = Vec::with_capacity((sec_hi - sec_lo) as usize);
    let mut blocks: Vec<(i32, i32, i32, RenderedBlock)> = Vec::with_capacity(4096);
    let mut stopped_at = sec_hi;
    let mut any_current = false;
    {
        let chunk = chunk_arc.read();
        for si in sec_lo..sec_hi {
            if let Some(deadline) = deadline
                && si > sec_lo
                && Instant::now() >= deadline
            {
                stopped_at = si;
                break;
            }
            let sec_y = min_y + si * 16;
            if !queue.is_current(job.chunk_x, job.chunk_z, sec_y, job.seq) {
                continue;
            }
            any_current = true;
            let Some(section) = chunk.sections.get(si as usize) else {
                continue;
            };
            blocks.clear();
            if section.block_count != 0 {
                for ly in 0u8..16 {
                    let y = sec_y + ly as i32;
                    for lx in 0u8..16 {
                        let x = x_base + lx as i32;
                        for lz in 0u8..16 {
                            let state = section.get_block_state(ChunkSectionBlockPos {
                                x: lx,
                                y: ly,
                                z: lz,
                            });
                            if state.is_air() {
                                continue;
                            }
                            let z = z_base + lz as i32;
                            let rb = cache.get(state);
                            if rb.is_solid && occ.enclosed(x, y, z) {
                                continue;
                            }
                            blocks.push((x, y, z, rb.clone()));
                        }
                    }
                }
            }
            let (opaque, water) =
                build_section_mesh(&blocks, x_base, sec_y, z_base, atlas_rows, &occ);
            let vis = crate::renderer::visgraph::resolve(|lx, ly, lz| {
                occ.is_solid(x_base + lx as i32, sec_y + ly as i32, z_base + lz as i32)
            });
            results.push(PendingSection {
                chunk_x: job.chunk_x,
                chunk_z: job.chunk_z,
                sec_y,
                opaque,
                water,
                vis,
            });
        }
    }

    if !any_current {
        crate::diag::bump(crate::diag::Stat::MeshJobsSuperseded);
    }

    let elapsed_ms = job_start.elapsed().as_secs_f32() * 1000.0;

    {
        let mut s = shared.lock().unwrap();
        s.profiling.record_mesh(elapsed_ms);
        crate::diag::add(crate::diag::Stat::SectionsMeshed, results.len() as u64);
        match job.scope {
            JobScope::Section(_) => s.session.pending_edits.extend(results),
            JobScope::Column => s.session.pending_chunks.extend(results),
        }
    }

    (job.scope == JobScope::Column && stopped_at < sec_count && any_current).then(|| Job {
        world: job.world,
        chunk_x: job.chunk_x,
        chunk_z: job.chunk_z,
        scope: job.scope,
        seq: job.seq,
        sec_from: stopped_at,
    })
}

fn fill_light(occ: &mut Occupancy, light: &crate::lighting::LightMap) {
    let (x_base, z_base) = occ.origin();
    let (y_base, y_count) = occ.y_window();
    let sec_lo = y_base >> 4;
    let sec_hi = (y_base + y_count - 1) >> 4;
    for cx in (x_base >> 4) - 1..=(x_base >> 4) + 1 {
        let (wx0, wx1) = ((cx * 16).max(x_base - 1), (cx * 16 + 15).min(x_base + 16));
        if wx0 > wx1 {
            continue;
        }
        for cz in (z_base >> 4) - 1..=(z_base >> 4) + 1 {
            let (wz0, wz1) = ((cz * 16).max(z_base - 1), (cz * 16 + 15).min(z_base + 16));
            if wz0 > wz1 {
                continue;
            }
            for sy in sec_lo..=sec_hi {
                let sample = light.resolve(crate::lighting::SectionPos {
                    x: cx,
                    y: sy,
                    z: cz,
                });
                let (wy0, wy1) = (
                    (sy * 16).max(y_base),
                    (sy * 16 + 15).min(y_base + y_count - 1),
                );
                for y in wy0..=wy1 {
                    for x in wx0..=wx1 {
                        for z in wz0..=wz1 {
                            let (b, s) =
                                sample.at((x & 15) as usize, (y & 15) as usize, (z & 15) as usize);
                            occ.set_light(x, y, z, b, s);
                        }
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
static BIOME_TABLE: RwLock<Option<Arc<Vec<u8>>>> = RwLock::new(None);

pub(crate) fn reset_biome_table() {
    *BIOME_TABLE.write() = None;
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn install_biome_table(table: Vec<u8>) {
    if !table.is_empty() {
        *BIOME_TABLE.write() = Some(Arc::new(table));
    }
}

pub(crate) fn biome_table(registries: &RegistryHolder) -> Arc<Vec<u8>> {
    if let Some(table) = BIOME_TABLE.read().as_ref() {
        return table.clone();
    }
    let registry = azalea::Identifier::from("worldgen/biome");
    let mut table = Vec::new();
    while let Some(ident) =
        registries.protocol_id_to_identifier(registry.clone(), table.len() as u32)
    {
        table.push(crate::util::biome_color::biome_index(ident.path()).unwrap_or(u8::MAX));
    }
    let table = Arc::new(table);
    if !table.is_empty() {
        *BIOME_TABLE.write() = Some(table.clone());
    }
    table
}

fn fill_occupancy(
    occ: &mut Occupancy,
    chunk: &Chunk,
    min_y: i32,
    ox: i32,
    oz: i32,
    lx_range: (u8, u8),
    lz_range: (u8, u8),
    cache: &mut BlockCache,
    biomes: &[u8],
) {
    use azalea_registry::DataRegistry;
    for (sec_idx, section) in chunk.sections.iter().enumerate() {
        if section.block_count == 0 {
            continue;
        }
        let sec_y = min_y + (sec_idx as i32) * 16;
        if !occ.covers_y(sec_y) && !occ.covers_y(sec_y + 15) {
            continue;
        }
        for ly in 0u8..16 {
            let y = sec_y + ly as i32;
            if !occ.covers_y(y) {
                continue;
            }
            for lx in lx_range.0..lx_range.1 {
                let x = ox + lx as i32;
                for lz in lz_range.0..lz_range.1 {
                    let state = section.get_block_state(ChunkSectionBlockPos {
                        x: lx,
                        y: ly,
                        z: lz,
                    });
                    if state.is_air() {
                        continue;
                    }
                    let rb = cache.get(state);
                    let z = oz + lz as i32;
                    occ.set_cull_group(x, y, z, rb.cull_group);
                    let biome = section.get_biome(ChunkSectionBiomePos {
                        x: lx / 4,
                        y: ly / 4,
                        z: lz / 4,
                    });
                    let idx = biomes
                        .get(biome.protocol_id() as usize)
                        .copied()
                        .unwrap_or(u8::MAX);
                    occ.set_biome(x, y, z, idx);
                    if rb.full_cube {
                        occ.set_full_cube(x, y, z);
                    }
                    if rb.emission != 0 {
                        occ.set_emission(x, y, z, rb.emission);
                    }
                    if rb.emissive {
                        occ.set_emissive(x, y, z);
                    }
                    if rb.is_solid {
                        occ.set_solid(x, y, z);
                    } else if let renderer::BlockGeom::Fluid { amount, lava, .. } = rb.geom {
                        occ.set_fluid(x, y, z, amount, lava);
                    } else if rb.waterlogged {
                        occ.set_fluid(x, y, z, 8, false);
                    }
                }
            }
        }
    }
}
