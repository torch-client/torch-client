use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::sync::Arc;

use azalea_buf::AzBuf;
use azalea_core::position::{BlockPos, ChunkPos as AzChunkPos};
use azalea_protocol::packets::game::c_level_chunk_with_light::ClientboundLevelChunkWithLight;
use azalea_protocol::packets::game::c_light_update::ClientboundLightUpdatePacketData;
use azalea_world::{Chunk, World};
use parking_lot::RwLock;
use wasm_bindgen::{JsCast, prelude::Closure};

use super::msg::{self, ToWorker};
use crate::client::worker::{JobQueue, JobScope, Scratch};
use crate::lighting::level::LightWorker;
use crate::lighting::{ColumnPos, LightJob, SectionPos};
use crate::session::SharedMutex;
use crate::{log_error, log_info};

struct Level {
    world: Arc<RwLock<World>>,
    resident: HashMap<(i32, i32), Arc<RwLock<Chunk>>>,
    min_y: i32,
    height: u32,
    forgotten: u32,
}

const COMPACT_INTERVAL: u32 = 256;

const POST_SECTIONS: usize = 64;

struct Mirror {
    shared: Arc<SharedMutex>,
    queue: Arc<JobQueue>,
    scratch: Scratch,
    light: LightWorker,
    level: Option<Level>,
    meshed: HashSet<(i32, i32)>,
    batch: Vec<LightJob>,
    dirty: Vec<SectionPos>,
    added: Vec<ColumnPos>,
    removed: Vec<ColumnPos>,
    page_backlog: usize,
    posted: usize,
}

thread_local! {
    static MIRROR: RefCell<Option<Mirror>> = const { RefCell::new(None) };
    static CHANGED: RefCell<Vec<BlockPos>> = const { RefCell::new(Vec::new()) };
    static ON_MESSAGE: RefCell<Option<Closure<dyn FnMut(web_sys::MessageEvent)>>> =
        const { RefCell::new(None) };
}

fn scope() -> web_sys::DedicatedWorkerGlobalScope {
    js_sys::global().unchecked_into()
}

fn post(bytes: Vec<u8>) {
    let array = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
    array.copy_from(&bytes);
    let buffer = array.buffer();
    let _ = scope().post_message_with_transfer(&buffer, &js_sys::Array::of1(&buffer));
}

pub fn start(assets: Vec<u8>) {
    console_error_panic_hook::set_once();
    crate::diag::panic_report::install_alloc_error_hook();
    if let Err(e) = crate::load_assets(assets) {
        post(msg::failed(&format!(
            "the assets could not be unpacked: {e}"
        )));
        return;
    }
    let started = crate::platform::time::Instant::now();
    crate::bake_block_lookups();
    log_info!(
        "mesh",
        "worker baked the block atlas in {:.0} ms",
        started.elapsed().as_secs_f32() * 1000.0
    );

    if azalea_world::block_change::set_block_change_hook(Box::new(|pos, _old, _new| {
        CHANGED.with(|c| c.borrow_mut().push(pos));
    }))
    .is_err()
    {
        post(msg::failed("the block-change hook was already taken"));
        return;
    }

    let shared = Arc::new(SharedMutex::default());
    let queue = crate::client::worker::spawn_chunk_workers(shared.clone());
    let light = LightWorker::new(crate::client::worldsync::light_map().clone());
    MIRROR.with(|slot| {
        *slot.borrow_mut() = Some(Mirror {
            shared,
            queue,
            scratch: Scratch::new(),
            light,
            level: None,
            meshed: HashSet::new(),
            batch: Vec::new(),
            dirty: Vec::new(),
            added: Vec::new(),
            removed: Vec::new(),
            page_backlog: 0,
            posted: 0,
        });
    });

    let handler =
        Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
            on_message(e)
        });
    scope().set_onmessage(Some(handler.as_ref().unchecked_ref()));
    ON_MESSAGE.with(|slot| *slot.borrow_mut() = Some(handler));
    post(msg::ready());
}

fn on_message(event: web_sys::MessageEvent) {
    let data = event.data();
    let Some(buffer) = data.dyn_ref::<js_sys::ArrayBuffer>() else {
        return;
    };
    let bytes = js_sys::Uint8Array::new(buffer).to_vec();
    let Some(message) = ToWorker::decode(&bytes) else {
        log_error!("mesh", "the page sent a message this worker cannot read");
        return;
    };
    MIRROR.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            scope().set_onmessage(None);
            post(msg::failed(
                "an earlier message panicked; the mirror is unusable",
            ));
            return;
        };
        if let Some(mirror) = slot.as_mut() {
            mirror.handle(message);
        }
    });
}

impl Mirror {
    fn handle(&mut self, message: ToWorker) {
        self.apply(message);
        let changed: Vec<BlockPos> = CHANGED.with(|c| std::mem::take(&mut *c.borrow_mut()));
        for pos in changed {
            self.on_block_changed(pos);
        }
        if !self.batch.is_empty() {
            let batch = std::mem::take(&mut self.batch);
            self.run_light(batch);
        }
        self.publish_light();
        self.mesh();
    }

    fn apply(&mut self, message: ToWorker) {
        match message {
            ToWorker::Options { lighting, smooth } => {
                crate::renderer::set_lighting_enabled(lighting);
                crate::renderer::set_smooth_lighting(smooth);
            }
            ToWorker::Level { dim, min_y, height } => {
                crate::renderer::dimension::set_current(crate::renderer::dimension::from_id(dim));
                if self
                    .level
                    .as_ref()
                    .is_none_or(|l| l.min_y != min_y || l.height != height)
                {
                    self.new_level(min_y, height);
                }
            }
            ToWorker::BiomeTable(table) => crate::client::worker::install_biome_table(table),
            ToWorker::Chunk {
                cx,
                cz,
                mesh,
                packet,
            } => self.chunk(cx, cz, mesh, &packet),
            ToWorker::LightUpdate { cx, cz, data } => {
                let Ok(data) =
                    ClientboundLightUpdatePacketData::azalea_read(&mut Cursor::new(&data[..]))
                else {
                    log_error!("mesh", "a light update for {cx},{cz} would not decode");
                    return;
                };
                self.batch.push(LightJob::Light {
                    col: ColumnPos { x: cx, z: cz },
                    light: crate::client::packets::server_light(&data),
                });
            }
            ToWorker::BlockChange { x, y, z, state } => {
                let Some(level) = self.level.as_ref() else {
                    return;
                };
                let state = azalea::block::BlockState::try_from(state)
                    .unwrap_or(azalea::block::BlockState::AIR);
                level
                    .world
                    .read()
                    .set_block_state(BlockPos { x, y, z }, state);
            }
            ToWorker::Unload { cx, cz } => self.unload(cx, cz),
            ToWorker::Backlog(sections) => {
                self.page_backlog = sections as usize;
                self.posted = 0;
            }
            ToWorker::ResetLight => {
                self.queue.reset();
                self.posted = 0;
                self.meshed.clear();
                self.clear_light_delta();
                self.batch.push(LightJob::Reset);
            }
            ToWorker::ResetLevel => {
                self.queue.reset();
                self.posted = 0;
                self.meshed.clear();
                self.level = None;
                self.clear_light_delta();
                self.batch.clear();
                self.batch.push(LightJob::Reset);
                crate::client::worker::reset_biome_table();
                let mut s = self.shared.lock().unwrap();
                s.session.pending_chunks.clear();
                s.session.pending_edits.clear();
            }
        }
    }

    fn clear_light_delta(&mut self) {
        self.dirty.clear();
        self.added.clear();
        self.removed.clear();
    }

    fn new_level(&mut self, min_y: i32, height: u32) {
        let world = World {
            chunks: azalea_world::ChunkStorage::new(height, min_y),
            ..Default::default()
        };
        self.level = Some(Level {
            world: Arc::new(RwLock::new(world)),
            resident: HashMap::new(),
            min_y,
            height,
            forgotten: 0,
        });
        self.meshed.clear();
        self.clear_light_delta();
        self.batch.push(LightJob::Reset);
    }

    fn chunk(&mut self, cx: i32, cz: i32, mesh: Vec<(i32, i32)>, packet: &[u8]) {
        let Some(level) = self.level.as_mut() else {
            log_error!("mesh", "a chunk arrived for {cx},{cz} before its level did");
            return;
        };
        let mut light = None;
        if packet.is_empty() && !level.resident.contains_key(&(cx, cz)) {
            log_error!("mesh", "column {cx},{cz} arrived with no blocks to mesh");
        }
        if !packet.is_empty() {
            let Ok(p) = ClientboundLevelChunkWithLight::azalea_read(&mut Cursor::new(packet))
            else {
                log_error!("mesh", "the chunk packet for {cx},{cz} would not decode");
                return;
            };
            let chunk = match Chunk::read_with_dimension_height(
                &mut Cursor::new(&p.chunk_data.data[..]),
                level.height,
                level.min_y,
                &p.chunk_data.heightmaps,
            ) {
                Ok(chunk) => chunk,
                Err(e) => {
                    log_error!("mesh", "the chunk data for {cx},{cz} would not decode: {e}");
                    return;
                }
            };
            let arc = level
                .world
                .write()
                .chunks
                .upsert(AzChunkPos { x: cx, z: cz }, chunk);
            level.resident.insert((cx, cz), arc);
            light = Some(crate::client::packets::server_light(&p.light_data));
        }
        for col in &mesh {
            self.meshed.insert(*col);
        }
        let col = ColumnPos { x: cx, z: cz };
        self.added.push(col);
        self.batch.push(LightJob::Chunk {
            world: level.world.clone(),
            col,
            mesh: mesh.into_iter().map(|(x, z)| ColumnPos { x, z }).collect(),
            light,
        });
    }

    fn unload(&mut self, cx: i32, cz: i32) {
        self.meshed.remove(&(cx, cz));
        self.queue.forget_chunk(cx, cz);
        self.removed.push(ColumnPos { x: cx, z: cz });
        self.batch.push(LightJob::Unload {
            col: ColumnPos { x: cx, z: cz },
        });
        let Some(level) = self.level.as_mut() else {
            return;
        };
        level.resident.remove(&(cx, cz));
        level.forgotten += 1;
        if level.forgotten >= COMPACT_INTERVAL {
            level.forgotten = 0;
            level.world.write().chunks.compact_dead();
        }
    }

    fn on_block_changed(&mut self, pos: BlockPos) {
        let Some(world) = self.level.as_ref().map(|l| l.world.clone()) else {
            return;
        };
        for (cx, cz, scope) in crate::client::worldsync::remesh_jobs(pos) {
            if self.meshed.contains(&(cx, cz)) {
                self.queue.push(world.clone(), cx, cz, scope);
            }
        }
        self.batch.push(LightJob::Block { world, pos });
    }

    fn run_light(&mut self, batch: Vec<LightJob>) {
        let level_world = self.level.as_ref().map(|l| l.world.clone());
        let Mirror {
            light,
            queue,
            meshed,
            dirty,
            ..
        } = self;
        let mut whole: HashSet<(i32, i32)> = HashSet::new();
        let mut on_dirty =
            |sections: &[SectionPos], to_mesh: &[(Arc<RwLock<World>>, ColumnPos)]| {
                for (world, col) in to_mesh {
                    whole.insert((col.x, col.z));
                    queue.push(world.clone(), col.x, col.z, JobScope::Column);
                }
                if let Some(world) = level_world.as_ref() {
                    for sec in sections {
                        if whole.contains(&(sec.x, sec.z)) {
                            continue;
                        }
                        if meshed.contains(&(sec.x, sec.z)) {
                            queue.push_relight(world.clone(), sec.x, sec.z, sec.y);
                        }
                    }
                }
                dirty.extend_from_slice(sections);
            };
        light.run_batch(batch, &mut on_dirty);
    }

    fn publish_light(&mut self) {
        if self.dirty.is_empty() && self.added.is_empty() && self.removed.is_empty() {
            return;
        }
        let dirty = std::mem::take(&mut self.dirty);
        let mut columns = std::mem::take(&mut self.added);
        let removed = std::mem::take(&mut self.removed);
        let mut seen: HashSet<(i32, i32)> = columns.iter().map(|c| (c.x, c.z)).collect();
        for sec in &dirty {
            if seen.insert((sec.x, sec.z)) {
                columns.push(ColumnPos { x: sec.x, z: sec.z });
            }
        }
        let map = crate::client::worldsync::light_map().read();
        post(msg::light(&map, &dirty, &columns, &removed));
    }

    fn mesh(&mut self) {
        self.queue.set_backlog(self.page_backlog + self.posted);
        crate::client::worker::drain_all(
            &self.queue,
            &self.shared,
            &mut self.scratch,
            POST_SECTIONS,
        );
        let (chunks, edits) = {
            let mut s = self.shared.lock().unwrap();
            (
                std::mem::take(&mut s.session.pending_chunks),
                std::mem::take(&mut s.session.pending_edits),
            )
        };
        if chunks.is_empty() && edits.is_empty() {
            return;
        }
        self.posted += chunks.len() + edits.len();
        post(msg::sections(&chunks, &edits));
    }
}
