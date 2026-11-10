use std::sync::atomic::{AtomicBool, AtomicI64, Ordering::Relaxed};

use azalea::Client;

use super::area::{self, Area};
use crate::gui::Snapshot;
use crate::gui::painter::Painter;
use crate::modules::hud_panels::TextPanel;
use crate::modules::registry::auto_mine as setting;
use crate::modules::store;

const EXACT: i64 = 32_768;
const RETRY_TICKS: i64 = 100;

pub const HUD_W: f32 = 120.0;
pub const HUD_H: f32 = TextPanel::height(1, 1);

static VALID: AtomicBool = AtomicBool::new(false);
static APPROX: AtomicBool = AtomicBool::new(false);
static VOLUME: AtomicI64 = AtomicI64::new(0);
static LEFT: AtomicI64 = AtomicI64::new(0);
static NEXT: AtomicI64 = AtomicI64::new(0);

pub fn invalidate() {
    VALID.store(false, Relaxed);
    NEXT.store(0, Relaxed);
}

pub fn changed(delta: i64) {
    if VALID.load(Relaxed) {
        LEFT.fetch_add(delta, Relaxed);
    }
}

pub fn count(bot: &Client, area: &Area) {
    if VALID.load(Relaxed) {
        return;
    }
    let now = crate::client::tracking::game_time();
    if now < NEXT.load(Relaxed) {
        return;
    }
    NEXT.store(now + RETRY_TICKS, Relaxed);
    let Ok(world) = bot.world() else { return };
    let world = world.read();
    let volume = area.volume();
    let stride = if volume <= EXACT {
        1
    } else {
        ((volume as f64 / EXACT as f64).cbrt().ceil()) as i32
    };
    let (mut samples, mut solid) = (0i64, 0i64);
    for x in (area.min.x..=area.max.x).step_by(stride as usize) {
        for y in (area.min.y..=area.max.y).step_by(stride as usize) {
            for z in (area.min.z..=area.max.z).step_by(stride as usize) {
                let Some(state) =
                    world.get_block_state(azalea_core::position::BlockPos::new(x, y, z))
                else {
                    return;
                };
                samples += 1;
                solid += i64::from(area::solid(state));
            }
        }
    }
    if samples == 0 {
        return;
    }
    let left = (solid as f64 * volume as f64 / samples as f64).round() as i64;
    VOLUME.store(volume, Relaxed);
    LEFT.store(left, Relaxed);
    APPROX.store(stride > 1, Relaxed);
    VALID.store(true, Relaxed);
}

pub fn draw(p: &mut Painter, _: &Snapshot) {
    if !store().flag(setting::PROGRESS) || !VALID.load(Relaxed) || !area::live() {
        return;
    }
    let volume = VOLUME.load(Relaxed).max(1);
    let air = (volume - LEFT.load(Relaxed)).clamp(0, volume);
    let approx = if APPROX.load(Relaxed) { "~" } else { "" };
    let mut panel = TextPanel::new(p, HUD_W);
    panel.text(&format!("AutoMine {approx}{air} / {volume} air"), 0xFF_FFFF);
    panel.bar(air as f32 / volume as f32, 0x33_CCFF);
}
