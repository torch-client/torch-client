#[cfg(feature = "seed_cracker")]
mod cache;
#[cfg(feature = "seed_cracker")]
mod help;
#[cfg(feature = "seed_cracker")]
mod hud;
#[cfg(all(target_arch = "wasm32", feature = "seed_cracker"))]
mod pool;
#[cfg(feature = "seed_cracker")]
mod run;
#[cfg(feature = "seed_cracker")]
mod search;

#[cfg(feature = "seed_cracker")]
use std::borrow::Cow;
#[cfg(feature = "seed_cracker")]
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering::Relaxed};

use super::Edge;
#[cfg(feature = "seed_cracker")]
use super::list::Entry;
use super::list::{BitList, Built};
use super::registry::Id;

#[cfg(feature = "seed_cracker")]
pub use run::tick;

static EDGE: Edge = Edge::new(Id::SeedCracker);

#[cfg(feature = "seed_cracker")]
static SESSION: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "seed_cracker")]
static CHUNK_ARRIVED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "seed_cracker")]
static HELP: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "seed_cracker")]
static IMPORT: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "seed_cracker")]
static REQUEST: AtomicU8 = AtomicU8::new(0);

#[cfg(feature = "seed_cracker")]
const OWNER: &str = "seed_cracker";

static GROUPS: [&str; 2] = ["Overworld", "End"];

#[cfg(feature = "seed_cracker")]
fn build_finders() -> Built {
    let entries = seedcrack::FinderKind::ALL
        .iter()
        .map(|k| Entry {
            group: (k.dimension() == seedcrack::Dimension::End) as u8,
            label: Cow::Borrowed(k.name()),
            on: true,
            color: 0,
        })
        .collect();
    (entries, Vec::new())
}

#[cfg(not(feature = "seed_cracker"))]
fn build_finders() -> Built {
    (Vec::new(), Vec::new())
}

pub static FINDERS: BitList = BitList::new(
    "seed_cracker",
    "Finders",
    214.0,
    &GROUPS,
    false,
    super::esp::blocks::PRESETS,
    build_finders,
);

pub const HUD_W: f32 = 160.0;
pub const HUD_H: f32 = super::hud_panels::TextPanel::height(10, 1);

#[cfg(feature = "seed_cracker")]
pub fn draw_hud(p: &mut crate::gui::painter::Painter, snap: &crate::gui::Snapshot) {
    hud::draw(p, snap);
}

#[cfg(not(feature = "seed_cracker"))]
pub fn draw_hud(_p: &mut crate::gui::painter::Painter, _snap: &crate::gui::Snapshot) {}

#[cfg(feature = "seed_cracker")]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Request {
    Status = 1,
    Clear = 2,
    Restart = 3,
}

#[cfg(feature = "seed_cracker")]
pub fn request(r: Request) {
    REQUEST.store(r as u8, Relaxed);
}

#[cfg(feature = "seed_cracker")]
fn take_request() -> Option<Request> {
    match REQUEST.swap(0, Relaxed) {
        1 => Some(Request::Status),
        2 => Some(Request::Clear),
        3 => Some(Request::Restart),
        _ => None,
    }
}

#[cfg(feature = "seed_cracker")]
pub fn open_help() {
    HELP.store(true, Relaxed);
}

#[cfg(feature = "seed_cracker")]
pub fn button(key: &str, now: f32) {
    match key {
        "help" => open_help(),
        "share" => cache::share_pressed(now),
        "import" => IMPORT.store(true, Relaxed),
        _ => {}
    }
}

#[cfg(feature = "seed_cracker")]
pub fn button_text(key: &str, text: &'static str, now: f32) -> &'static str {
    if key == "share" {
        cache::share_label(text, now)
    } else {
        text
    }
}

#[cfg(feature = "seed_cracker")]
#[inline]
pub fn chunk_arrived() {
    CHUNK_ARRIVED.store(true, Relaxed);
}

#[cfg(feature = "seed_cracker")]
pub fn logged_in() {
    SESSION.fetch_add(1, Relaxed);
}

#[cfg(feature = "seed_cracker")]
pub fn disconnected() {
    SESSION.fetch_add(1, Relaxed);
}

#[cfg(not(feature = "seed_cracker"))]
pub fn tick(_bot: &azalea::Client, shared: &crate::session::SharedMutex) {
    if EDGE.poll(super::store()) == super::Phase::Started {
        let mut s = shared.lock().unwrap();
        super::chat_math::show_local_colored(
            &mut s.session,
            "Seed Cracker: not in this build (the seed_cracker feature)",
            super::chat_math::RED,
        );
    }
}
