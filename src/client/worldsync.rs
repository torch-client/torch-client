use crate::util::global;
use crate::{log_debug, log_error, log_warn};
use azalea::Client;
use azalea_core::position::BlockPos as AzBlockPos;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::client::tracking::{
    current_world, meshed_chunks, received_chunks, reset_chunk_tracking, reset_maps,
    set_current_world,
};
use crate::client::worker::JobScope;
use crate::session::SharedMutex;
use crate::{CHUNK_Q, SHARED, blockentities, lighting};

pub(crate) fn install_block_change_hook() {
    let hook_installed =
        azalea_world::block_change::set_block_change_hook(Box::new(|pos, old, new| {
            crate::blockentities::feed::on_block_change(pos, new);
            if crate::modules::esp::any_enabled() {
                crate::modules::esp::mark_dirty();
            }
            crate::modules::auto_mine::block_changed(pos, old, new);
            #[cfg(lua_mods)]
            {
                crate::modules::script::ghost::block_changed(pos.x, pos.y, pos.z);
                crate::modules::script::block_changed(pos.x, pos.z);
            }
            #[cfg(feature = "render_lod")]
            crate::client::lod::note_column(pos.x >> 4, pos.z >> 4);
            let (Some(world), Some(q)) = (current_world().lock().unwrap().clone(), CHUNK_Q.get())
            else {
                return;
            };
            if !crate::client::mesh_worker::forward_block_change(pos, new) {
                {
                    let meshed = meshed_chunks().lock().unwrap();
                    for (cx, cz, scope) in remesh_jobs(pos) {
                        if meshed.contains(&(cx, cz)) {
                            q.push(world.clone(), cx, cz, scope);
                        }
                    }
                }
                send_light(lighting::LightJob::Block { pos });
            }

            if !old.is_air()
                && new.is_air()
                && let Some(shared) = SHARED.get()
                && let Ok(mut s) = shared.try_lock()
                && crate::play::interaction::take_local_break(pos)
            {
                crate::session::push_particle_emit(
                    &mut s.session.particle_emits,
                    crate::session::ParticleEmit::Event(crate::session::LevelEventEmit {
                        id: 2001,
                        pos: [pos.x, pos.y, pos.z],
                        data: old.id() as u32,
                        global: false,
                    }),
                );
            }
        }));
    if hook_installed.is_err() {
        log_error!(
            "blocks",
            "the block-change hook could not be installed because something else already owns \
             it; no block placed or broken will ever be redrawn this run."
        );
    }
}

global! {
pub(crate) fn light_map() -> Arc<parking_lot::RwLock<lighting::LightMap>>
}

#[cfg(any(feature = "audio", lua_mods))]
pub(crate) fn light_at(x: i32, y: i32, z: i32) -> Option<(u8, u8)> {
    let map = light_map().read();
    map.is_lit(lighting::SectionPos::of_block(x, y, z).column())
        .then(|| map.at(x, y, z))
}

static LIGHT_THREAD: OnceLock<lighting::LightThread> = OnceLock::new();

pub(crate) fn start_light_thread() {
    let _ = LIGHT_THREAD.get_or_init(|| {
        lighting::level::spawn(light_map().clone(), |lit_in, dirty, to_mesh| {
            let Some(q) = CHUNK_Q.get() else {
                crate::diag::add(crate::diag::Stat::LightJobsDropped, to_mesh.len() as u64);
                return;
            };
            log_debug!("light", "queueing {} column meshes", to_mesh.len());
            let mut jobs = Vec::new();
            let mut whole = HashSet::new();
            {
                let received = received_chunks().lock().unwrap();
                let mut meshed = meshed_chunks().lock().unwrap();
                let current = current_world().lock().unwrap().clone();
                let Some(current) = current.filter(|w| Arc::ptr_eq(w, lit_in)) else {
                    log_debug!(
                        "light",
                        "a light pass for a level already left queued nothing"
                    );
                    return;
                };
                for col in to_mesh.iter().map(|(_, col)| col) {
                    if !received.contains(&(col.x, col.z))
                        || !surrounded(|c| received.contains(&c), col.x, col.z)
                    {
                        continue;
                    }
                    if whole.insert((col.x, col.z)) {
                        meshed.insert((col.x, col.z));
                        jobs.push((current.clone(), col.x, col.z, JobScope::Column));
                    }
                }
                for sec in dirty {
                    if whole.contains(&(sec.x, sec.z)) {
                        continue;
                    }
                    if meshed.contains(&(sec.x, sec.z)) {
                        jobs.push((current.clone(), sec.x, sec.z, JobScope::Section(sec.y)));
                    }
                }
            }
            crate::diag::add(crate::diag::Stat::MeshJobs, jobs.len() as u64);
            for (world, cx, cz, scope) in jobs {
                match scope {
                    JobScope::Section(sy) => q.push_relight(world, cx, cz, sy),
                    JobScope::Column => q.push(world, cx, cz, scope),
                }
            }
        })
    });
}

pub(crate) fn send_light(job: lighting::LightJob) {
    match LIGHT_THREAD.get() {
        Some(t) => t.send(job),
        None => crate::diag::bump(crate::diag::Stat::LightJobsDropped),
    }
}

pub(crate) fn remesh_jobs(pos: AzBlockPos) -> Vec<(i32, i32, JobScope)> {
    let (cx, cz, sy) = (pos.x >> 4, pos.z >> 4, pos.y >> 4);
    let mut v = vec![(cx, cz, JobScope::Section(sy))];
    match pos.y.rem_euclid(16) {
        0 => v.push((cx, cz, JobScope::Section(sy - 1))),
        15 => v.push((cx, cz, JobScope::Section(sy + 1))),
        _ => {}
    }
    if pos.x.rem_euclid(16) == 0 {
        v.push((cx - 1, cz, JobScope::Section(sy)));
    }
    if pos.x.rem_euclid(16) == 15 {
        v.push((cx + 1, cz, JobScope::Section(sy)));
    }
    if pos.z.rem_euclid(16) == 0 {
        v.push((cx, cz - 1, JobScope::Section(sy)));
    }
    if pos.z.rem_euclid(16) == 15 {
        v.push((cx, cz + 1, JobScope::Section(sy)));
    }
    v
}

static CHUNK_GENERATION: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub(crate) fn chunk_generation() -> u32 {
    CHUNK_GENERATION.load(std::sync::atomic::Ordering::Relaxed)
}

pub(crate) fn bump_chunk_generation() {
    CHUNK_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

const NEIGHBOURS: [(i32, i32); 8] = [
    (-1, 0),
    (1, 0),
    (0, -1),
    (0, 1),
    (-1, -1),
    (1, -1),
    (-1, 1),
    (1, 1),
];

fn surrounded(received: impl Fn((i32, i32)) -> bool, x: i32, z: i32) -> bool {
    NEIGHBOURS.iter().all(|(dx, dz)| received((x + dx, z + dz)))
}

fn ready_after_arrival(
    received: impl Fn((i32, i32)) -> bool,
    meshed: impl Fn((i32, i32)) -> bool,
    cx: i32,
    cz: i32,
) -> Vec<lighting::ColumnPos> {
    std::iter::once((0, 0))
        .chain(NEIGHBOURS)
        .map(|(dx, dz)| (cx + dx, cz + dz))
        .filter(|&t| !meshed(t) && received(t) && surrounded(&received, t.0, t.1))
        .map(|(x, z)| lighting::ColumnPos { x, z })
        .collect()
}

global! {
fn resent_waiting() -> crate::session::SideMutex<HashSet<(i32, i32)>>
}

static RESENT_WAITING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn forget_resent(cx: i32, cz: i32) {
    if RESENT_WAITING.load(Ordering::Relaxed) {
        let mut waiting = resent_waiting().lock().unwrap();
        waiting.remove(&(cx, cz));
        RESENT_WAITING.store(!waiting.is_empty(), Ordering::Relaxed);
    }
}

fn clear_resent() {
    if RESENT_WAITING.load(Ordering::Relaxed) {
        resent_waiting().lock().unwrap().clear();
        RESENT_WAITING.store(false, Ordering::Relaxed);
    }
}

pub(crate) fn note_chunk_received(bot: &Client, cx: i32, cz: i32) {
    crate::diag::on_chunk_received();
    crate::modules::seed_xray::chunk_arrived();
    #[cfg(feature = "seed_cracker")]
    crate::modules::seed_cracker::chunk_arrived();
    bump_chunk_generation();
    #[cfg(lua_mods)]
    {
        crate::modules::script::ghost::chunk_changed(cx, cz);
        crate::modules::script::chunk_arrived(cx, cz);
    }
    log_debug!("chunks", "received column {cx},{cz}");
    let Ok(world) = bot.world() else { return };
    if world
        .read()
        .chunks
        .get(&azalea_core::position::ChunkPos::new(cx, cz))
        .is_none()
    {
        log_debug!(
            "chunks",
            "column {cx},{cz} is not in the current level; stale arrival ignored"
        );
        return;
    }
    #[cfg(feature = "render_lod")]
    crate::client::lod::note_column(cx, cz);
    let mut received = received_chunks().lock().unwrap();
    let resent = !received.insert((cx, cz));
    let mut meshed = meshed_chunks().lock().unwrap();
    #[cfg(feature = "render_lod")]
    if crate::client::lod::cache::reclaim(cx, cz) {
        meshed.remove(&(cx, cz));
    }
    let resend = resent && meshed.contains(&(cx, cz));
    let resend_ready = resend && surrounded(|c| received.contains(&c), cx, cz);
    let mut waiting = (resend || RESENT_WAITING.load(Ordering::Relaxed))
        .then(|| resent_waiting().lock().unwrap());
    if resend && let Some(waiting) = waiting.as_mut() {
        if resend_ready {
            waiting.remove(&(cx, cz));
        } else {
            waiting.insert((cx, cz));
        }
    }
    let mut ready = ready_after_arrival(
        |c| received.contains(&c),
        |c| meshed.contains(&c) && !waiting.as_ref().is_some_and(|w| w.contains(&c)),
        cx,
        cz,
    );
    if resend_ready {
        ready.push(lighting::ColumnPos { x: cx, z: cz });
    }
    if let Some(waiting) = waiting.as_mut() {
        for col in &ready {
            waiting.remove(&(col.x, col.z));
        }
        RESENT_WAITING.store(!waiting.is_empty(), Ordering::Relaxed);
    }
    drop(waiting);
    let to_worker = crate::client::mesh_worker::active();
    if to_worker {
        for col in &ready {
            meshed.insert((col.x, col.z));
        }
    }
    drop(meshed);
    drop(received);
    if to_worker {
        crate::client::mesh_worker::forward_chunk(&world, cx, cz, &ready);
        return;
    }
    send_light(lighting::LightJob::Chunk {
        world: world.clone(),
        col: lighting::ColumnPos { x: cx, z: cz },
        mesh: ready,
        light: take_server_light(cx, cz),
    });
}

global! {
fn server_light_stash() -> Mutex<HashMap<(i32, i32), lighting::ServerLight>>
}

pub(crate) fn stash_server_light(cx: i32, cz: i32, light: lighting::ServerLight) {
    server_light_stash().lock().unwrap().insert((cx, cz), light);
}

fn take_server_light(cx: i32, cz: i32) -> Option<lighting::ServerLight> {
    server_light_stash().lock().unwrap().remove(&(cx, cz))
}

pub(crate) fn send_server_light(cx: i32, cz: i32, light: lighting::ServerLight) {
    send_light(lighting::LightJob::Light {
        col: lighting::ColumnPos { x: cx, z: cz },
        light,
    });
}

pub(crate) fn drop_server_light(cx: i32, cz: i32) {
    server_light_stash().lock().unwrap().remove(&(cx, cz));
    crate::client::mesh_worker::drop_chunk_packet(cx, cz);
}

pub(crate) fn reload_all_chunks(bot: &Client, shared: &SharedMutex) {
    let mut cols: Vec<(i32, i32)> = received_chunks().lock().unwrap().iter().copied().collect();
    sort_nearest_first(bot, &mut cols);
    if let Some(q) = CHUNK_Q.get() {
        q.next_level();
    }
    reset_chunk_tracking();
    clear_resent();
    if let Some(q) = CHUNK_Q.get() {
        q.reset();
    }
    {
        let mut s = shared.lock().unwrap();
        s.session.clear_chunks = true;
        s.session.pending_chunks.clear();
        s.session.pending_edits.clear();
        s.session.meshed_columns.cleared();
        s.session.unloaded_chunks.clear();
    }
    if !crate::client::mesh_worker::forward_reset_light() {
        send_light(lighting::LightJob::Reset);
    }
    for (cx, cz) in cols {
        note_chunk_received(bot, cx, cz);
    }
}

pub(crate) fn remesh_all_chunks(bot: &Client) {
    if crate::client::mesh_worker::active() {
        #[cfg(resource_packs)]
        crate::client::mesh_worker::forward_packs();
        return;
    }
    let (Ok(world), Some(q)) = (bot.world(), CHUNK_Q.get()) else {
        return;
    };
    let mut cols: Vec<(i32, i32)> = meshed_chunks().lock().unwrap().iter().copied().collect();
    sort_nearest_first(bot, &mut cols);
    for (cx, cz) in cols {
        q.push(world.clone(), cx, cz, JobScope::Column);
    }
}

fn sort_nearest_first(bot: &Client, cols: &mut [(i32, i32)]) {
    let Ok(pos) = bot.position() else { return };
    let (px, pz) = ((pos.x.floor() as i32) >> 4, (pos.z.floor() as i32) >> 4);
    cols.sort_unstable_by_key(|&(x, z)| (x - px).pow(2) + (z - pz).pow(2));
}

const COMPACT_INTERVAL: u32 = 256;
static FORGOTTEN_SINCE_COMPACT: AtomicU32 = AtomicU32::new(0);

pub(crate) fn forget_chunk(
    world: &azalea::local_player::WorldHolder,
    shared: &SharedMutex,
    cx: i32,
    cz: i32,
) {
    blockentities::feed::on_forget_chunk(cx, cz);
    bump_chunk_generation();
    received_chunks().lock().unwrap().remove(&(cx, cz));
    meshed_chunks().lock().unwrap().remove(&(cx, cz));
    forget_resent(cx, cz);
    if let Some(q) = CHUNK_Q.get() {
        q.forget_chunk(cx, cz);
    }
    shared
        .lock()
        .unwrap()
        .session
        .unloaded_chunks
        .push((cx, cz));
    if !crate::client::mesh_worker::forward_unload(cx, cz) {
        send_light(lighting::LightJob::Unload {
            col: lighting::ColumnPos { x: cx, z: cz },
        });
    }
    drop_server_light(cx, cz);

    if FORGOTTEN_SINCE_COMPACT.fetch_add(1, Ordering::Relaxed) + 1 >= COMPACT_INTERVAL {
        FORGOTTEN_SINCE_COMPACT.store(0, Ordering::Relaxed);
        world.shared.write().chunks.compact_dead();
    }
}

const MESH_RETRIES: u8 = 2;

pub(crate) struct LostJob {
    pub cx: i32,
    pub cz: i32,
    pub seq: u64,
    pub epoch: u32,
    pub attempt: u8,
}

enum LostOutcome {
    Requeue,
    GaveUp,
    NoBlocks,
}

pub(crate) fn mesh_job_lost(
    q: &crate::client::worker::JobQueue,
    world: &Arc<parking_lot::RwLock<azalea_world::World>>,
    job: LostJob,
) {
    let LostJob {
        cx,
        cz,
        seq,
        epoch,
        attempt,
    } = job;
    if column_kept_by_lod(cx, cz) {
        return;
    }
    let present = world
        .read()
        .chunks
        .get(&azalea_core::position::ChunkPos::new(cx, cz))
        .is_some();
    let outcome = {
        let received = received_chunks().lock().unwrap();
        let meshed = meshed_chunks().lock().unwrap();
        if !meshed.contains(&(cx, cz)) || !q.owns_any(cx, cz, seq, epoch) {
            log_debug!(
                "mesh",
                "a job for {cx},{cz} built nothing; a newer one owns the column"
            );
            return;
        }
        let ready = received.contains(&(cx, cz)) && surrounded(|c| received.contains(&c), cx, cz);
        if present && ready && attempt < MESH_RETRIES {
            LostOutcome::Requeue
        } else {
            let mut waiting = resent_waiting().lock().unwrap();
            waiting.insert((cx, cz));
            RESENT_WAITING.store(true, Ordering::Relaxed);
            if present {
                LostOutcome::GaveUp
            } else {
                LostOutcome::NoBlocks
            }
        }
    };
    match outcome {
        LostOutcome::Requeue => {
            crate::diag::bump(crate::diag::Stat::MeshRequeued);
            log_warn!(
                "mesh",
                "column {cx},{cz} ended its mesh job with nothing built; queued again ({} of \
                 {MESH_RETRIES})",
                attempt + 1
            );
            q.push_retry(world.clone(), cx, cz, attempt + 1);
        }
        LostOutcome::GaveUp => {
            crate::diag::bump(crate::diag::Stat::MeshGaveUp);
            log_error!(
                "mesh",
                "column {cx},{cz} ended its mesh job with nothing built {} times; left unbuilt \
                 until it next arrives or a neighbour does",
                attempt + 1
            );
        }
        LostOutcome::NoBlocks => {
            log_debug!(
                "mesh",
                "column {cx},{cz} has no blocks in azalea's storage; left for the next arrival \
                 beside it"
            );
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn mesh_worker_lost(cx: i32, cz: i32) {
    let Some(world) = current_world().lock().unwrap().clone() else {
        return;
    };
    if column_kept_by_lod(cx, cz) {
        return;
    }
    let present = world
        .read()
        .chunks
        .get(&azalea_core::position::ChunkPos::new(cx, cz))
        .is_some();
    let (outcome, attempt) = {
        let received = received_chunks().lock().unwrap();
        let mut meshed = meshed_chunks().lock().unwrap();
        if !meshed.contains(&(cx, cz)) {
            return;
        }
        if !present {
            meshed.remove(&(cx, cz));
            (LostOutcome::NoBlocks, 0)
        } else {
            let attempt = crate::client::mesh_worker::note_retry(cx, cz);
            let ready =
                received.contains(&(cx, cz)) && surrounded(|c| received.contains(&c), cx, cz);
            if ready && attempt <= MESH_RETRIES {
                (LostOutcome::Requeue, attempt)
            } else {
                meshed.remove(&(cx, cz));
                (LostOutcome::GaveUp, attempt)
            }
        }
    };
    match outcome {
        LostOutcome::Requeue => {
            crate::diag::bump(crate::diag::Stat::MeshRequeued);
            log_warn!(
                "mesh",
                "the mesh worker had no blocks for column {cx},{cz}; asked again ({attempt} of \
                 {MESH_RETRIES})"
            );
            let col = lighting::ColumnPos { x: cx, z: cz };
            crate::client::mesh_worker::forward_chunk(&world, cx, cz, &[col]);
        }
        LostOutcome::GaveUp => {
            crate::diag::bump(crate::diag::Stat::MeshGaveUp);
            log_error!(
                "mesh",
                "the mesh worker has no blocks for column {cx},{cz}; left unbuilt until the \
                 server sends it again"
            );
        }
        LostOutcome::NoBlocks => {
            log_debug!(
                "mesh",
                "column {cx},{cz} has no blocks in azalea's storage; unmarked"
            );
        }
    }
}

#[cfg(feature = "render_lod")]
fn column_kept_by_lod(cx: i32, cz: i32) -> bool {
    crate::client::lod::cache::is_cached(cx, cz)
}

#[cfg(not(feature = "render_lod"))]
fn column_kept_by_lod(_cx: i32, _cz: i32) -> bool {
    false
}

pub(crate) fn reset_level(world: &azalea::local_player::WorldHolder, shared: &SharedMutex) {
    set_current_world(&world.shared);
    if let Some(q) = CHUNK_Q.get() {
        q.next_level();
    }
    reset_chunk_tracking();
    clear_resent();
    bump_chunk_generation();
    crate::renderer::environment::reset_weather();
    crate::client::envprobe::reset();
    if !crate::client::mesh_worker::forward_reset_level() {
        send_light(lighting::LightJob::Reset);
    }
    server_light_stash().lock().unwrap().clear();
    crate::client::viewwindow::reset_positioned();
    crate::client::viewwindow::reset_server_centre();
    if let Some(q) = CHUNK_Q.get() {
        q.reset();
    }
    blockentities::feed::reset();
    #[cfg(feature = "render_lod")]
    crate::client::lod::reset();
    reset_maps();
    let mut s = shared.lock().unwrap();
    s.session.map_updates.clear();
    s.session.clear_maps = true;
    s.session.block_entities = Default::default();
    s.session.block_entities_version = s.session.block_entities_version.wrapping_add(1);
    s.session.clear_chunks = true;
    s.session.pending_chunks.clear();
    s.session.pending_edits.clear();
    s.session.meshed_columns.cleared();
    s.session.unloaded_chunks.clear();
}

pub(crate) fn set_dimension(
    world: &azalea::local_player::WorldHolder,
    common: &azalea_protocol::packets::common::CommonPlayerSpawnInfo,
) {
    use crate::renderer::dimension::Dimension;

    let type_path = {
        let shared = world.shared.read();
        common
            .dimension_type(&shared.registries)
            .map(|(id, _)| id.path().to_owned())
    };
    let path = type_path.unwrap_or_else(|| common.dimension.path().to_owned());
    crate::renderer::dimension::set_current(Dimension::from_path(&path));
    crate::renderer::dimension::set_level(&common.dimension);
    crate::renderer::dimension::set_sea_level(common.sea_level);
    crate::modules::seed_xray::spawned(common.seed);
}

#[cfg(test)]
mod readiness_tests {
    use super::*;
    use std::collections::HashSet;

    fn ready(
        received: &HashSet<(i32, i32)>,
        meshed: &HashSet<(i32, i32)>,
        at: (i32, i32),
    ) -> Vec<(i32, i32)> {
        let mut v: Vec<_> = ready_after_arrival(
            |c| received.contains(&c),
            |c| meshed.contains(&c),
            at.0,
            at.1,
        )
        .into_iter()
        .map(|c| (c.x, c.z))
        .collect();
        v.sort();
        v
    }

    fn block_without(missing: &[(i32, i32)]) -> HashSet<(i32, i32)> {
        let mut s = HashSet::new();
        for x in -1..=1 {
            for z in -1..=1 {
                if !missing.contains(&(x, z)) {
                    s.insert((x, z));
                }
            }
        }
        s
    }

    #[test]
    fn a_missing_corner_holds_the_column_back() {
        let received = block_without(&[(1, 1)]);
        let meshed = HashSet::new();
        assert!(ready(&received, &meshed, (0, 0)).is_empty());
    }

    #[test]
    fn the_last_corner_completes_its_diagonal() {
        let received = block_without(&[]);
        let meshed = HashSet::new();
        assert_eq!(ready(&received, &meshed, (1, 1)), vec![(0, 0)]);
    }

    #[test]
    fn meshed_and_absent_columns_are_skipped() {
        let received = block_without(&[]);
        let meshed: HashSet<_> = [(0, 0)].into_iter().collect();
        assert!(ready(&received, &meshed, (1, 1)).is_empty());

        let received = block_without(&[(0, 0)]);
        assert!(ready(&received, &HashSet::new(), (1, 1)).is_empty());
    }

    #[test]
    fn a_square_meshes_its_interior() {
        let mut received = HashSet::new();
        let mut meshed = HashSet::new();
        for x in -2..=2 {
            for z in -2..=2 {
                received.insert((x, z));
                for c in ready(&received, &meshed, (x, z)) {
                    meshed.insert(c);
                }
            }
        }
        assert_eq!(meshed, block_without(&[]));
    }
}

#[cfg(test)]
mod remesh_tests {
    use super::*;
    use std::collections::HashSet;

    fn jobs(x: i32, y: i32, z: i32) -> Vec<(i32, i32, JobScope)> {
        remesh_jobs(AzBlockPos::new(x, y, z))
    }

    #[test]
    fn interior_block_touches_one_section() {
        assert_eq!(jobs(8, 72, 8), vec![(0, 0, JobScope::Section(4))]);
    }

    #[test]
    fn section_boundary_pulls_in_the_neighbour_section() {
        assert_eq!(
            jobs(8, 64, 8),
            vec![(0, 0, JobScope::Section(4)), (0, 0, JobScope::Section(3))]
        );
        assert_eq!(
            jobs(8, 79, 8),
            vec![(0, 0, JobScope::Section(4)), (0, 0, JobScope::Section(5))]
        );
    }

    #[test]
    fn chunk_edges_pull_in_the_neighbour_chunk() {
        assert_eq!(
            jobs(-16, 72, 8),
            vec![(-1, 0, JobScope::Section(4)), (-2, 0, JobScope::Section(4))]
        );
        assert_eq!(
            jobs(-1, 72, 8),
            vec![(-1, 0, JobScope::Section(4)), (0, 0, JobScope::Section(4))]
        );
    }

    #[test]
    fn corner_of_a_section_hits_every_neighbour() {
        let got: HashSet<_> = jobs(0, 64, 0).into_iter().collect();
        let want: HashSet<_> = [
            (0, 0, JobScope::Section(4)),
            (0, 0, JobScope::Section(3)),
            (-1, 0, JobScope::Section(4)),
            (0, -1, JobScope::Section(4)),
        ]
        .into_iter()
        .collect();
        assert_eq!(got, want);
    }
}
