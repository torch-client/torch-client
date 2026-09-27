use crate::{log_debug, log_error};
use azalea::Client;
use azalea_core::position::BlockPos as AzBlockPos;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::client::tracking::{
    current_world, meshed_chunks, received_chunks, reset_chunk_tracking, reset_maps,
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
                send_light(lighting::LightJob::Block { world, pos });
            }

            #[cfg(feature = "audio")]
            {
                let at = [pos.x as f32 + 0.5, pos.y as f32 + 0.5, pos.z as f32 + 0.5];
                if crate::audio::audible_at(at) {
                    use azalea::block::BlockTrait;
                    let name = |state| {
                        let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
                        crate::util::block_model::sound_group(block.id())
                    };
                    let is_fluid = |state| {
                        let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
                        matches!(block.id(), "water" | "lava" | "bubble_column")
                    };
                    let event = if old.is_air() && !new.is_air() && !is_fluid(new) {
                        Some(name(new).place_event)
                    } else {
                        None
                    };
                    if let Some(event) = event {
                        crate::audio::play_at(
                            event,
                            crate::audio::SoundCategory::Blocks,
                            at,
                            1.0,
                            0.8,
                        );
                    }
                }
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

static LIGHT_MAP: OnceLock<Arc<parking_lot::RwLock<lighting::LightMap>>> = OnceLock::new();
static LIGHT_THREAD: OnceLock<lighting::LightThread> = OnceLock::new();

pub(crate) fn light_map() -> &'static Arc<parking_lot::RwLock<lighting::LightMap>> {
    LIGHT_MAP.get_or_init(Default::default)
}

pub(crate) fn start_light_thread() {
    let _ = LIGHT_THREAD.get_or_init(|| {
        lighting::level::spawn(light_map().clone(), |dirty, to_mesh| {
            let Some(q) = CHUNK_Q.get() else {
                crate::diag::add(crate::diag::Stat::LightJobsDropped, to_mesh.len() as u64);
                return;
            };
            log_debug!("light", "queueing {} column meshes", to_mesh.len());
            let mut jobs = Vec::new();
            let mut whole = HashSet::new();
            let mut whole_world = None;
            {
                let received = received_chunks().lock().unwrap();
                let mut meshed = meshed_chunks().lock().unwrap();
                for (world, col) in to_mesh {
                    if !received.contains(&(col.x, col.z)) {
                        continue;
                    }
                    if meshed.insert((col.x, col.z)) {
                        whole.insert((col.x, col.z));
                        whole_world.get_or_insert_with(|| world.clone());
                        jobs.push((world.clone(), col.x, col.z, JobScope::Column));
                    }
                }
            }
            let current = current_world().lock().unwrap().clone();
            if let Some(world) = current {
                let covered = whole_world.is_some_and(|w| Arc::ptr_eq(&w, &world));
                let meshed = meshed_chunks().lock().unwrap();
                for sec in dirty {
                    if covered && whole.contains(&(sec.x, sec.z)) {
                        continue;
                    }
                    if meshed.contains(&(sec.x, sec.z)) {
                        jobs.push((world.clone(), sec.x, sec.z, JobScope::Section(sec.y)));
                    }
                }
            }
            crate::diag::add(crate::diag::Stat::MeshJobs, jobs.len() as u64);
            for (world, cx, cz, scope) in jobs {
                q.push(world, cx, cz, scope);
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

pub(crate) fn note_chunk_received(bot: &Client, cx: i32, cz: i32) {
    crate::diag::on_chunk_received();
    bump_chunk_generation();
    log_debug!("chunks", "received column {cx},{cz}");
    let Ok(world) = bot.world() else { return };
    let mut ready: Vec<lighting::ColumnPos> = Vec::new();
    let mut received = received_chunks().lock().unwrap();
    received.insert((cx, cz));
    let mut meshed = meshed_chunks().lock().unwrap();
    for (tx, tz) in [
        (cx, cz),
        (cx - 1, cz),
        (cx + 1, cz),
        (cx, cz - 1),
        (cx, cz + 1),
    ] {
        if meshed.contains(&(tx, tz)) || !received.contains(&(tx, tz)) {
            continue;
        }
        let surrounded = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)]
            .iter()
            .all(|(dx, dz)| received.contains(&(tx + dx, tz + dz)));
        if !surrounded {
            continue;
        }
        ready.push(lighting::ColumnPos { x: tx, z: tz });
    }
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

static SERVER_LIGHT: OnceLock<Mutex<HashMap<(i32, i32), lighting::ServerLight>>> = OnceLock::new();

fn server_light_stash() -> &'static Mutex<HashMap<(i32, i32), lighting::ServerLight>> {
    SERVER_LIGHT.get_or_init(Default::default)
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

pub(crate) fn reload_all_chunks(bot: &Client, shared: &Arc<SharedMutex>) {
    let cols: Vec<(i32, i32)> = received_chunks().lock().unwrap().iter().copied().collect();
    reset_chunk_tracking();
    if let Some(q) = CHUNK_Q.get() {
        q.reset();
    }
    {
        let mut s = shared.lock().unwrap();
        s.session.clear_chunks = true;
        s.session.pending_chunks.clear();
        s.session.pending_edits.clear();
        s.session.unloaded_chunks.clear();
    }
    if !crate::client::mesh_worker::forward_reset_light() {
        send_light(lighting::LightJob::Reset);
    }
    for (cx, cz) in cols {
        note_chunk_received(bot, cx, cz);
    }
}

const COMPACT_INTERVAL: u32 = 256;
static FORGOTTEN_SINCE_COMPACT: AtomicU32 = AtomicU32::new(0);

pub(crate) fn forget_chunk(
    world: &azalea::local_player::WorldHolder,
    shared: &Arc<SharedMutex>,
    cx: i32,
    cz: i32,
) {
    blockentities::feed::on_forget_chunk(cx, cz);
    bump_chunk_generation();
    received_chunks().lock().unwrap().remove(&(cx, cz));
    meshed_chunks().lock().unwrap().remove(&(cx, cz));
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

pub(crate) fn reset_level(shared: &Arc<SharedMutex>) {
    reset_chunk_tracking();
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
    reset_maps();
    let mut s = shared.lock().unwrap();
    s.session.map_updates.clear();
    s.session.clear_maps = true;
    s.session.block_entities = Default::default();
    s.session.block_entities_version = s.session.block_entities_version.wrapping_add(1);
    s.session.clear_chunks = true;
    s.session.pending_chunks.clear();
    s.session.pending_edits.clear();
    s.session.unloaded_chunks.clear();
}

pub(crate) fn set_dimension(common: &azalea_protocol::packets::common::CommonPlayerSpawnInfo) {
    use crate::renderer::dimension::Dimension;

    crate::renderer::dimension::set_current(Dimension::from_path(common.dimension.path()));
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
