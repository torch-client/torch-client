use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};

use azalea_core::position::ChunkPos;

static CACHED: Mutex<Option<HashSet<(i32, i32)>>> = Mutex::new(None);

static PLAYER_CHUNK: AtomicI64 = AtomicI64::new(0);

static RADIUS: AtomicU32 = AtomicU32::new(0);

fn pack(x: i32, z: i32) -> i64 {
    (x as i64) << 32 | (z as u32 as i64)
}

fn unpack(v: i64) -> (i32, i32) {
    ((v >> 32) as i32, v as i32)
}

fn within(x: i32, z: i32, radius: u32) -> bool {
    let (px, pz) = unpack(PLAYER_CHUNK.load(Ordering::Relaxed));
    let (dx, dz) = ((x - px) as i64, (z - pz) as i64);
    dx * dx + dz * dz < (radius as i64) * (radius as i64)
}

pub fn install() {
    azalea_world::retain::set_forget_filter(keep);
}

fn keep(pos: ChunkPos) -> bool {
    let radius = RADIUS.load(Ordering::Relaxed);
    if radius == 0 || !within(pos.x, pos.z, radius) {
        return false;
    }
    CACHED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashSet::new)
        .insert((pos.x, pos.z));
    true
}

pub fn is_cached(cx: i32, cz: i32) -> bool {
    CACHED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_some_and(|c| c.contains(&(cx, cz)))
}

pub fn reclaim(cx: i32, cz: i32) -> bool {
    CACHED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_mut()
        .is_some_and(|c| c.remove(&(cx, cz)))
}

pub fn reset() {
    if let Some(c) = CACHED.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        c.clear();
    }
}

pub(crate) fn tick(bot: &azalea::Client, shared: &crate::session::SharedMutex, pos: azalea::Vec3) {
    let (px, pz) = ((pos.x.floor() as i32) >> 4, (pos.z.floor() as i32) >> 4);
    PLAYER_CHUNK.store(pack(px, pz), Ordering::Relaxed);

    let radius = super::settings().render_distance;
    let (Ok(partial), Ok(world)) = (bot.partial_world(), bot.world()) else {
        return;
    };
    if RADIUS.swap(radius, Ordering::Relaxed) != radius {
        azalea_world::retain::set_radius_floor(radius);
        partial.write().chunks.update_view_radius(
            azalea_world::chunk::calculate_chunk_storage_range(
                azalea_world::retain::effective_radius(),
            ),
        );
    }

    let evict: Vec<(i32, i32)> = {
        let mut cached = CACHED.lock().unwrap_or_else(|e| e.into_inner());
        let Some(cached) = cached.as_mut() else {
            return;
        };
        let out: Vec<_> = cached
            .iter()
            .copied()
            .filter(|&(x, z)| !within(x, z, radius))
            .collect();
        for c in &out {
            cached.remove(c);
        }
        out
    };
    if evict.is_empty() {
        return;
    }
    let holder = azalea::local_player::WorldHolder {
        partial: partial.clone(),
        shared: world,
    };
    for (cx, cz) in evict {
        partial.write().chunks.forget_held(&ChunkPos::new(cx, cz));
        crate::client::worldsync::forget_chunk(&holder, shared, cx, cz);
    }
}
