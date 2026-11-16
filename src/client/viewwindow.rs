use crate::log_warn;
use azalea::{Client, ClientInformation};
use azalea_core::position::ChunkPos;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::session::SharedMutex;

static POSITIONED: AtomicBool = AtomicBool::new(false);

static POSITIONED_IN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(crate) fn note_player_positioned(partial: &azalea_world::PartialWorld) {
    POSITIONED_IN.store(partial.chunks.generation(), Ordering::Relaxed);
    POSITIONED.store(true, Ordering::Relaxed);
}

pub(crate) fn reset_positioned() {
    POSITIONED.store(false, Ordering::Relaxed);
    STREAM_REACH.store(0, Ordering::Relaxed);
    *LAST_SWEEP.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

const SWEEP_MARGIN: u32 = 4;

static STREAM_REACH: AtomicU32 = AtomicU32::new(0);

const MAX_STREAM_REACH: u32 = 34;

static PLAYER_CHUNK: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

pub(crate) fn note_stream_reach(x: i32, z: i32) {
    if !POSITIONED.load(Ordering::Relaxed) {
        return;
    }
    let packed = PLAYER_CHUNK.load(Ordering::Relaxed);
    let (px, pz) = ((packed >> 32) as i32, packed as i32);
    let reach = (x - px).unsigned_abs().max((z - pz).unsigned_abs());
    if reach <= MAX_STREAM_REACH {
        STREAM_REACH.fetch_max(reach, Ordering::Relaxed);
    }
}

static LAST_SWEEP: Mutex<Option<(ChunkPos, ChunkPos, u32)>> = Mutex::new(None);

fn chunk_distance(a: ChunkPos, b: ChunkPos) -> u32 {
    (a.x - b.x).unsigned_abs().max((a.z - b.z).unsigned_abs())
}

pub(crate) fn sweep_far_columns(bot: &Client, shared: &SharedMutex, pos: azalea::Vec3) {
    if !POSITIONED.load(Ordering::Relaxed) {
        return;
    }
    let here = ChunkPos::new((pos.x.floor() as i32) >> 4, (pos.z.floor() as i32) >> 4);
    PLAYER_CHUNK.store(
        ((here.x as i64) << 32) | (here.z as u32 as i64),
        Ordering::Relaxed,
    );
    let (Ok(partial), Ok(world)) = (bot.partial_world(), bot.world()) else {
        return;
    };
    let requested = bot
        .component::<ClientInformation>()
        .map(|info| info.view_distance as u32)
        .unwrap_or(0);
    let (center, announced) = {
        let guard = partial.read();
        if guard.chunks.generation() != POSITIONED_IN.load(Ordering::Relaxed) {
            return;
        }
        (guard.chunks.view_center(), guard.chunks.chunk_radius())
    };
    crate::diag::note_view((center.x, center.z), (here.x, here.z));
    let reach = announced
        .max(azalea_world::chunk::calculate_chunk_storage_range(
            requested,
        ))
        .max(STREAM_REACH.load(Ordering::Relaxed))
        + SWEEP_MARGIN;
    {
        let mut last = LAST_SWEEP.lock().unwrap_or_else(|e| e.into_inner());
        if *last == Some((center, here, reach)) {
            return;
        }
        *last = Some((center, here, reach));
    }
    let far: Vec<ChunkPos> = partial
        .read()
        .chunks
        .loaded()
        .map(|(col, _)| col)
        .filter(|&col| chunk_distance(col, here) > reach && chunk_distance(col, center) > reach)
        .collect();
    let dropped: Vec<ChunkPos> = far
        .into_iter()
        .filter(|col| !crate::client::worldsync::column_kept_by_lod(col.x, col.z))
        .collect();
    if !dropped.is_empty() {
        let mut guard = partial.write();
        for col in &dropped {
            guard.chunks.forget_held(col);
        }
    }
    if dropped.is_empty() {
        return;
    }
    let before = crate::diag::get(crate::diag::Stat::ChunksSwept);
    crate::diag::add(crate::diag::Stat::ChunksSwept, dropped.len() as u64);
    if before < 64 {
        log_warn!(
            "chunks",
            "let go of {} columns more than {reach} chunks from both the player ({},{}) and the \
             server's centre ({},{}) that the server never forgot",
            dropped.len(),
            here.x,
            here.z,
            center.x,
            center.z
        );
    }
    let holder = azalea::local_player::WorldHolder {
        partial,
        shared: world,
    };
    for col in dropped {
        crate::client::worldsync::forget_chunk(&holder, shared, col.x, col.z);
    }
}

pub(crate) fn apply_client_information_requests(bot: &Client, shared: &SharedMutex) {
    let (view_distance, prefs) = {
        let mut s = shared.lock().unwrap();
        (
            s.session.render_distance_request.take(),
            s.session.skin_prefs_request.take(),
        )
    };
    if let Some(view_distance) = view_distance {
        apply_render_distance(bot, view_distance);
    }
    if let Some(prefs) = prefs {
        apply_skin_prefs(bot, prefs);
    }
}

fn apply_render_distance(bot: &Client, view_distance: u32) {
    let mut info = bot
        .component::<ClientInformation>()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    info.view_distance = view_distance.clamp(
        crate::gui::options::RENDER_DISTANCE_MIN as u32,
        crate::gui::options::RENDER_DISTANCE_MAX as u32,
    ) as u8;
    if let Err(e) = bot.set_client_information(info) {
        log_warn!("net", "could not apply the render distance: {e}");
    }
}

fn apply_skin_prefs(bot: &Client, prefs: crate::session::SkinPrefs) {
    let mut info = bot
        .component::<ClientInformation>()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    info.main_hand = if prefs.main_hand_left {
        azalea::entity::HumanoidArm::Left
    } else {
        azalea::entity::HumanoidArm::Right
    };
    #[cfg(feature = "skins")]
    {
        let on = |bit: u8| prefs.skin_parts & (1 << bit) != 0;
        info.model_customization =
            azalea_protocol::common::client_information::ModelCustomization {
                cape: on(0),
                jacket: on(1),
                left_sleeve: on(2),
                right_sleeve: on(3),
                left_pants: on(4),
                right_pants: on(5),
                hat: on(6),
            };
    }
    if let Err(e) = bot.set_client_information(info) {
        log_warn!("net", "could not apply the skin settings: {e}");
    }
}
