use crate::session::SideMutex;
use std::sync::{Arc, OnceLock, atomic::AtomicU64};

use bevy::platform::collections::{HashMap, HashSet};

use crate::{
    entities,
    renderer::{self, HumanoidAnim},
};
use azalea::prelude::*;

static MESHED: OnceLock<SideMutex<HashSet<(i32, i32)>>> = OnceLock::new();

pub(crate) fn meshed_chunks() -> &'static SideMutex<HashSet<(i32, i32)>> {
    MESHED.get_or_init(Default::default)
}

static RECEIVED: OnceLock<SideMutex<HashSet<(i32, i32)>>> = OnceLock::new();

pub(crate) fn received_chunks() -> &'static SideMutex<HashSet<(i32, i32)>> {
    RECEIVED.get_or_init(Default::default)
}

pub(crate) fn reset_chunk_tracking() {
    received_chunks().lock().unwrap().clear();
    meshed_chunks().lock().unwrap().clear();
}

pub(crate) fn reset_chunk_tracking_best_effort() {
    if let Ok(mut m) = received_chunks().try_lock() {
        m.clear();
    }
    if let Ok(mut m) = meshed_chunks().try_lock() {
        m.clear();
    }
}

static ANIM: OnceLock<SideMutex<HashMap<i32, HumanoidAnim>>> = OnceLock::new();

pub(crate) const LOCAL_ANIM_ID: i32 = -1;

pub(crate) fn anim_states() -> &'static SideMutex<HashMap<i32, HumanoidAnim>> {
    ANIM.get_or_init(Default::default)
}

static ENTITY_ANIM: OnceLock<SideMutex<HashMap<i32, entities::feed::EntityAnim>>> = OnceLock::new();

pub(crate) fn entity_anims() -> &'static SideMutex<HashMap<i32, entities::feed::EntityAnim>> {
    ENTITY_ANIM.get_or_init(Default::default)
}

pub(crate) fn on_living_anim(
    id: i32,
    on_player: impl FnOnce(&mut HumanoidAnim),
    on_entity: impl FnOnce(&mut entities::feed::EntityAnim),
) {
    if let Some(anim) = anim_states().lock().unwrap().get_mut(&id) {
        on_player(anim);
    }
    if let Some(anim) = entity_anims().lock().unwrap().get_mut(&id) {
        on_entity(anim);
    }
}

static CRIT_HITS: OnceLock<SideMutex<Vec<(i32, bool)>>> = OnceLock::new();

pub(crate) fn crit_hits() -> &'static SideMutex<Vec<(i32, bool)>> {
    CRIT_HITS.get_or_init(Default::default)
}

#[derive(Clone, Debug, Default)]
pub struct Equipment {
    pub main_hand: renderer::HeldItem,
    pub off_hand: renderer::HeldItem,
    pub helmet: Option<&'static str>,
    pub chestplate: Option<&'static str>,
    pub leggings: Option<&'static str>,
    pub boots: Option<&'static str>,
    pub body_armor: Option<&'static str>,
    pub saddle: Option<&'static str>,
}

static EQUIPMENT: OnceLock<SideMutex<HashMap<i32, Equipment>>> = OnceLock::new();

pub(crate) fn equipment() -> &'static SideMutex<HashMap<i32, Equipment>> {
    EQUIPMENT.get_or_init(Default::default)
}

static MAX_HEALTH: OnceLock<SideMutex<HashMap<i32, f32>>> = OnceLock::new();

pub(crate) fn max_healths() -> &'static SideMutex<HashMap<i32, f32>> {
    MAX_HEALTH.get_or_init(Default::default)
}

pub(crate) static LOCAL_ENTITY_ID: std::sync::atomic::AtomicI32 =
    std::sync::atomic::AtomicI32::new(i32::MIN);

pub(crate) fn local_entity_id() -> i32 {
    LOCAL_ENTITY_ID.load(std::sync::atomic::Ordering::Relaxed)
}

pub(crate) static LOCAL_VEHICLE_ID: std::sync::atomic::AtomicI32 =
    std::sync::atomic::AtomicI32::new(-1);

pub(crate) fn local_vehicle_id() -> Option<i32> {
    match LOCAL_VEHICLE_ID.load(std::sync::atomic::Ordering::Relaxed) {
        -1 => None,
        id => Some(id),
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct MountAttributes {
    pub movement_speed: Option<f64>,
    pub jump_strength: Option<f64>,
    pub step_height: Option<f64>,
}

static MOUNT_ATTRIBUTES: OnceLock<SideMutex<HashMap<i32, MountAttributes>>> = OnceLock::new();

pub(crate) fn mount_attributes() -> &'static SideMutex<HashMap<i32, MountAttributes>> {
    MOUNT_ATTRIBUTES.get_or_init(Default::default)
}

static META_INDEX_18: OnceLock<SideMutex<HashMap<i32, u8>>> = OnceLock::new();

pub(crate) fn meta_index_18() -> &'static SideMutex<HashMap<i32, u8>> {
    META_INDEX_18.get_or_init(Default::default)
}

static META_DIRTY: OnceLock<SideMutex<HashMap<i32, u8>>> = OnceLock::new();

pub(crate) fn meta_dirty() -> &'static SideMutex<HashMap<i32, u8>> {
    META_DIRTY.get_or_init(Default::default)
}

pub(crate) fn mark_meta_dirty(id: i32) {
    meta_dirty().lock().unwrap().insert(id, 2);
}

static MAPS: OnceLock<SideMutex<HashMap<i32, Box<[u8; MAP_BYTES]>>>> = OnceLock::new();

pub(crate) const MAP_EDGE: usize = 128;
pub(crate) const MAP_BYTES: usize = MAP_EDGE * MAP_EDGE;

fn maps() -> &'static SideMutex<HashMap<i32, Box<[u8; MAP_BYTES]>>> {
    MAPS.get_or_init(Default::default)
}

pub(crate) fn apply_map_patch(
    id: i32,
    start_x: u8,
    start_y: u8,
    width: u8,
    height: u8,
    colors: &[u8],
) -> Arc<Vec<u8>> {
    let mut maps = maps().lock().unwrap();
    let map = maps.entry(id).or_insert_with(|| Box::new([0u8; MAP_BYTES]));
    for x in 0..width as usize {
        for y in 0..height as usize {
            let Some(color) = colors.get(x + y * width as usize) else {
                continue;
            };
            let (dx, dy) = (start_x as usize + x, start_y as usize + y);
            if dx < MAP_EDGE && dy < MAP_EDGE {
                map[dx + dy * MAP_EDGE] = *color;
            }
        }
    }
    Arc::new(map.to_vec())
}

pub(crate) fn has_map(id: i32) -> bool {
    maps().lock().unwrap().contains_key(&id)
}

pub(crate) fn reset_maps() {
    maps().lock().unwrap().clear();
}

static ON_GROUND: OnceLock<SideMutex<HashMap<i32, bool>>> = OnceLock::new();

pub(crate) fn on_grounds() -> &'static SideMutex<HashMap<i32, bool>> {
    ON_GROUND.get_or_init(Default::default)
}

static VELOCITY: OnceLock<SideMutex<HashMap<i32, ([f64; 3], u64)>>> = OnceLock::new();
static VELOCITY_SEQ: AtomicU64 = AtomicU64::new(1);

pub(crate) fn velocities() -> &'static SideMutex<HashMap<i32, ([f64; 3], u64)>> {
    VELOCITY.get_or_init(Default::default)
}

pub(crate) fn record_velocity(id: i32, delta: azalea_core::position::Vec3) {
    let seq = VELOCITY_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    velocities()
        .lock()
        .unwrap()
        .insert(id, ([delta.x, delta.y, delta.z], seq));
}

static ADD_ENTITY_DATA: OnceLock<SideMutex<HashMap<i32, i32>>> = OnceLock::new();

pub(crate) fn add_entity_data() -> &'static SideMutex<HashMap<i32, i32>> {
    ADD_ENTITY_DATA.get_or_init(Default::default)
}

static PASSENGERS: OnceLock<SideMutex<HashMap<i32, Vec<i32>>>> = OnceLock::new();

pub(crate) fn passengers() -> &'static SideMutex<HashMap<i32, Vec<i32>>> {
    PASSENGERS.get_or_init(Default::default)
}

pub(crate) static GAME_TIME: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

pub(crate) fn game_time() -> i64 {
    GAME_TIME.load(std::sync::atomic::Ordering::Relaxed)
}

pub(crate) fn overworld_clock(
    world: &azalea::local_player::WorldHolder,
    packet: &azalea_protocol::packets::game::c_set_time::ClientboundSetTime,
) -> Option<crate::session::WorldClock> {
    use azalea_core::data_registry::DataRegistryWithKey;
    use azalea_registry::data::WorldClockKey;

    let guard = world.shared.read();
    let registries = &guard.registries;

    let state = packet
        .clock_updates
        .iter()
        .find(|(id, _)| id.key_owned(registries) == Some(WorldClockKey::Overworld))
        .or_else(|| {
            (packet.clock_updates.len() == 1).then(|| packet.clock_updates.iter().next())?
        })?
        .1;

    Some(crate::session::WorldClock {
        ticks: state.total_ticks as f64 + state.partial_tick as f64,
        rate: state.rate,
        drift: 0.0,
    })
}

static HEAD_YAW: OnceLock<SideMutex<HashMap<i32, f32>>> = OnceLock::new();

pub(crate) fn head_yaws() -> &'static SideMutex<HashMap<i32, f32>> {
    HEAD_YAW.get_or_init(Default::default)
}

pub(crate) fn retain_loaded<T>(items: &mut Vec<T>, key: impl Fn(&T) -> (i32, i32)) {
    let loaded = meshed_chunks().lock().unwrap();
    items.retain(|item| loaded.contains(&key(item)));
}
static WORLD: OnceLock<SideMutex<Option<Arc<parking_lot::RwLock<azalea_world::World>>>>> =
    OnceLock::new();

pub(crate) fn current_world()
-> &'static SideMutex<Option<Arc<parking_lot::RwLock<azalea_world::World>>>> {
    WORLD.get_or_init(Default::default)
}

pub(crate) fn set_current_world(world: &Arc<parking_lot::RwLock<azalea_world::World>>) {
    let mut slot = current_world().lock().unwrap();
    if slot.as_ref().is_none_or(|w| !Arc::ptr_eq(w, world)) {
        *slot = Some(world.clone());
    }
}
