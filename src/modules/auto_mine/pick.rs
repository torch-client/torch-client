use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Mutex, PoisonError};

use azalea::Client;
use azalea_core::position::BlockPos;

use super::{area, draw};
use crate::session::SharedMutex;

const RANGE: f64 = 128.0;

static ARMED: AtomicBool = AtomicBool::new(false);
static CLICK_A: AtomicBool = AtomicBool::new(false);
static CLICK_B: AtomicBool = AtomicBool::new(false);
static CORNERS: Mutex<(Option<BlockPos>, Option<BlockPos>)> = Mutex::new((None, None));

pub fn active() -> bool {
    ARMED.load(Relaxed)
}

pub fn arm() {
    *CORNERS.lock().unwrap_or_else(PoisonError::into_inner) = (None, None);
    CLICK_A.store(false, Relaxed);
    CLICK_B.store(false, Relaxed);
    HINTED.store(false, Relaxed);
    ARMED.store(true, Relaxed);
}

pub fn cancel() {
    ARMED.store(false, Relaxed);
}

pub fn gate(
    attack_held: bool,
    attack_clicked: bool,
    use_held: bool,
    use_clicked: bool,
) -> (bool, bool, bool, bool) {
    if !ARMED.load(Relaxed) {
        return (attack_held, attack_clicked, use_held, use_clicked);
    }
    if attack_clicked {
        CLICK_A.store(true, Relaxed);
    }
    if use_clicked {
        CLICK_B.store(true, Relaxed);
    }
    (false, false, false, false)
}

static HINTED: AtomicBool = AtomicBool::new(false);

pub fn tick(bot: &Client, shared: &SharedMutex) {
    if !HINTED.swap(true, Relaxed) {
        let mut st = shared.lock().unwrap();
        crate::modules::chat_math::show_local(
            &mut st.session,
            if cfg!(feature = "mobile_ui") {
                "AutoMine: tap a block for the first corner, then USE on a block for the second"
            } else {
                "AutoMine: left click a block for the first corner, right click a block for the second"
            },
        );
    }
    let cursor = ray(bot, shared);
    let (a, b) = (CLICK_A.swap(false, Relaxed), CLICK_B.swap(false, Relaxed));
    let corners = {
        let mut c = CORNERS.lock().unwrap_or_else(PoisonError::into_inner);
        if a && let Some(p) = cursor {
            if !cfg!(feature = "mobile_ui") || c.0.is_none() {
                c.0 = Some(p);
            }
        }
        if b && let Some(p) = cursor {
            c.1 = Some(p);
        }
        *c
    };
    match corners {
        (Some(a), Some(b)) => {
            ARMED.store(false, Relaxed);
            draw::clear();
            area::define(bot, shared, a, b);
        }
        (a, b) => draw::preview(a, b, cursor),
    }
}

fn ray(bot: &Client, shared: &SharedMutex) -> Option<BlockPos> {
    let (eye, dir) = match crate::modules::freecam::ray() {
        Some(r) => r,
        None => {
            let st = shared.lock().unwrap();
            let eye = crate::modules::aim_assist::eye(&st, 1.0);
            let dir = crate::modules::freecam::view_vector(st.camera_yaw, st.camera_pitch);
            (eye, dir)
        }
    };
    crate::modules::freecam::far_hit(
        bot,
        eye,
        dir,
        RANGE,
        azalea_registry::builtin::ItemKind::Air,
    )
    .map(|h| h.block_pos)
}
