use crate::log_warn;
use azalea::{Client, ClientInformation};
use std::sync::Arc;

use crate::session::SharedMutex;

pub(crate) fn keep_view_center_on_the_player(partial: &PartialWorldRef, pos: azalea::Vec3) {
    let here = azalea_core::position::ChunkPos::new(
        (pos.x.floor() as i32) >> 4,
        (pos.z.floor() as i32) >> 4,
    );
    recenter_view(partial, here);
}

pub(crate) type PartialWorldRef = std::sync::Arc<parking_lot::RwLock<azalea_world::PartialWorld>>;

const CENTER_SLACK: i32 = 3;

static SERVER_CENTRED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn note_server_centre() {
    SERVER_CENTRED.store(true, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn reset_server_centre() {
    SERVER_CENTRED.store(false, std::sync::atomic::Ordering::Relaxed);
    SKEW_REPORTED.store(false, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn server_centred() -> bool {
    SERVER_CENTRED.load(std::sync::atomic::Ordering::Relaxed)
}

static POSITIONED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn note_player_positioned() {
    POSITIONED.store(true, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn reset_positioned() {
    POSITIONED.store(false, std::sync::atomic::Ordering::Relaxed);
}

fn recenter_view(partial: &PartialWorldRef, here: azalea_core::position::ChunkPos) {
    {
        let guard = partial.read();
        let center = guard.chunks.view_center();
        crate::diag::note_view((center.x, center.z), (here.x, here.z));
        let drift = (here.x - center.x).abs().max((here.z - center.z).abs());
        let adrift = drift > CENTER_SLACK
            && POSITIONED.load(std::sync::atomic::Ordering::Relaxed)
            && !server_centred();
        if guard.chunks.in_range(&here) && !adrift {
            if drift > CENTER_SLACK
                && server_centred()
                && POSITIONED.load(std::sync::atomic::Ordering::Relaxed)
            {
                report_skew(drift, (center.x, center.z), (here.x, here.z));
            }
            return;
        }
    }
    let mut guard = partial.write();
    let was = guard.chunks.view_center();
    guard.chunks.update_view_center(here);
    drop(guard);
    let n = crate::diag::get(crate::diag::Stat::ViewRecentred);
    crate::diag::bump(crate::diag::Stat::ViewRecentred);
    if n >= 4 && (n + 1) % 100 != 0 {
        return;
    }
    let why = if server_centred() {
        "the player had left it entirely, so no chunk of theirs could be stored"
    } else {
        "the server has not said where it belongs, so everything it streamed was \
         being dropped on arrival"
    };
    log_warn!(
        "view",
        "azalea's chunk window was centred on {},{} with the player in chunk {},{} \
         ({} chunks adrift, {} tolerated) and {why}; recentred on the player",
        was.x,
        was.z,
        here.x,
        here.z,
        (here.x - was.x).abs().max((here.z - was.z).abs()),
        CENTER_SLACK
    );
}

fn report_skew(drift: i32, center: (i32, i32), here: (i32, i32)) {
    if SKEW_REPORTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    crate::log_info!(
        "view",
        "the server has the chunk window on {},{} while the player is in chunk {},{} \
         ({drift} chunks apart, {} tolerated). The window is left where the \
         server put it, because that is where it is streaming; the two positions \
         disagreeing is the thing to look at.",
        center.0,
        center.1,
        here.0,
        here.1,
        CENTER_SLACK,
    );
}

static SKEW_REPORTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn note_chunk_packet(partial: &PartialWorldRef, x: i32, z: i32) {
    let pos = azalea_core::position::ChunkPos::new(x, z);
    let guard = partial.read();
    let center = guard.chunks.view_center();
    crate::diag::note_chunk_packet(
        (x, z),
        guard.chunks.in_range(&pos),
        (center.x, center.z),
        (guard.chunks.view_range().saturating_sub(1)) / 2,
    );
}

pub(crate) fn apply_skin_prefs_request(bot: &Client, shared: &Arc<SharedMutex>) {
    let Some(prefs) = std::mem::take(&mut shared.lock().unwrap().session.skin_prefs_request) else {
        return;
    };
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

pub(crate) fn apply_render_distance_request(bot: &Client, shared: &Arc<SharedMutex>) {
    let Some(view_distance) =
        std::mem::take(&mut shared.lock().unwrap().session.render_distance_request)
    else {
        return;
    };
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
