use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering::Relaxed};

use crate::gui::Snapshot;
use crate::gui::painter::Painter;
use crate::modules::hud_panels::TextPanel;
use crate::modules::registry::farmer as setting;
use crate::modules::store;

pub const HUD_W: f32 = 170.0;
pub const HUD_H: f32 = TextPanel::height(4, 1);

const TEXT: u32 = 0xFF_FFFF;
const NOTE: u32 = 0xAA_AAAA;
const BAR: u32 = 0x55_CC33;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub(super) enum State {
    Scanning,
    Harvesting,
    Walking,
    Waiting,
    Collecting,
    Home,
    Paused,
}

impl State {
    const ALL: [State; 7] = [
        State::Scanning,
        State::Harvesting,
        State::Walking,
        State::Waiting,
        State::Collecting,
        State::Home,
        State::Paused,
    ];

    fn label(self) -> &'static str {
        match self {
            State::Scanning => crate::lang::tr!("farmer.state.scanning"),
            State::Harvesting => crate::lang::tr!("farmer.state.harvesting"),
            State::Walking => crate::lang::tr!("farmer.state.walking"),
            State::Waiting => crate::lang::tr!("farmer.state.waiting"),
            State::Collecting => crate::lang::tr!("farmer.state.collecting"),
            State::Home => crate::lang::tr!("farmer.state.home"),
            State::Paused => crate::lang::tr!("farmer.state.paused"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub(super) enum Note {
    #[default]
    None,
    Screen,
    Keys,
    UserRoute,
    Dead,
    Spectator,
    NoWorld,
    FindingPath,
    RouteStill,
    RouteStuck,
    Lane,
    Spot,
    Pass,
    Turning,
    Mob,
    Delay,
    Breaking,
    Confirming,
    Chunks,
    NoSpot,
    Stuck,
}

impl Note {
    const ALL: [Note; 21] = [
        Note::None,
        Note::Screen,
        Note::Keys,
        Note::UserRoute,
        Note::Dead,
        Note::Spectator,
        Note::NoWorld,
        Note::FindingPath,
        Note::RouteStill,
        Note::RouteStuck,
        Note::Lane,
        Note::Spot,
        Note::Pass,
        Note::Turning,
        Note::Mob,
        Note::Delay,
        Note::Breaking,
        Note::Confirming,
        Note::Chunks,
        Note::NoSpot,
        Note::Stuck,
    ];

    pub(super) fn text(self, n: u32) -> Option<String> {
        let s = match self {
            Note::None => return None,
            Note::Screen => crate::lang::tr!("farmer.note.screen"),
            Note::Keys => crate::lang::tr!("farmer.note.keys"),
            Note::UserRoute => crate::lang::tr!("farmer.note.user_route"),
            Note::Dead => crate::lang::tr!("farmer.note.dead"),
            Note::Spectator => crate::lang::tr!("farmer.note.spectator"),
            Note::NoWorld => crate::lang::tr!("farmer.note.no_world"),
            Note::FindingPath => crate::lang::tr!("farmer.note.finding_path"),
            Note::RouteStill => crate::lang::tr!("farmer.note.route_still"),
            Note::RouteStuck => crate::lang::tr!("farmer.note.route_stuck"),
            Note::Lane => crate::lang::tr!("farmer.note.lane"),
            Note::Spot => crate::lang::tr!("farmer.note.spot"),
            Note::Pass => crate::lang::tr!("farmer.note.pass"),
            Note::Turning => crate::lang::tr!("farmer.note.turning"),
            Note::Mob => crate::lang::tr!("farmer.note.mob"),
            Note::Delay => crate::lang::tr!("farmer.note.delay"),
            Note::Breaking => crate::lang::tr!("farmer.note.breaking"),
            Note::Confirming => crate::lang::tr!("farmer.note.confirming"),
            Note::Chunks => crate::lang::tr!("farmer.note.chunks"),
            Note::NoSpot => return Some(crate::lang::trf!("farmer.note.no_spot", n)),
            Note::Stuck => crate::lang::tr!("farmer.note.stuck"),
        };
        Some(s.to_string())
    }
}

static STATE: AtomicU8 = AtomicU8::new(State::Scanning as u8);
static NOTE_KIND: AtomicU8 = AtomicU8::new(Note::None as u8);
static NOTE_N: AtomicU32 = AtomicU32::new(0);
static HARVESTED: AtomicU32 = AtomicU32::new(0);
static TO_GO: AtomicU32 = AtomicU32::new(0);
static UNLOADED: AtomicU32 = AtomicU32::new(0);
static SCANNED: AtomicBool = AtomicBool::new(false);

pub(super) fn publish(
    state: State,
    note: (Note, u32),
    harvested: u32,
    to_go: u32,
    unloaded: u32,
    scanned: bool,
) {
    STATE.store(state as u8, Relaxed);
    NOTE_KIND.store(note.0 as u8, Relaxed);
    NOTE_N.store(note.1, Relaxed);
    HARVESTED.store(harvested, Relaxed);
    TO_GO.store(to_go, Relaxed);
    UNLOADED.store(unloaded, Relaxed);
    SCANNED.store(scanned, Relaxed);
}

pub(super) fn reset() {
    publish(State::Scanning, (Note::None, 0), 0, 0, 0, false);
}

pub fn draw(p: &mut Painter, _: &Snapshot) {
    if !store().flag(setting::HUD) {
        return;
    }
    let state = State::ALL
        .get(STATE.load(Relaxed) as usize)
        .copied()
        .unwrap_or(State::Scanning);
    let note = Note::ALL
        .get(NOTE_KIND.load(Relaxed) as usize)
        .copied()
        .unwrap_or_default();
    let mut panel = TextPanel::new(p, HUD_W);
    match (state, note.text(NOTE_N.load(Relaxed))) {
        (State::Paused, Some(why)) => {
            panel.text(&crate::lang::trf!("farmer.hud.paused", why), TEXT)
        }
        (_, why) => {
            panel.text(&crate::lang::trf!("farmer.hud.state", state.label()), TEXT);
            if let Some(why) = why {
                panel.text(&why, NOTE);
            }
        }
    }
    if !SCANNED.load(Relaxed) {
        panel.text(crate::lang::tr!("farmer.hud.counting"), NOTE);
        return;
    }
    let (harvested, to_go) = (HARVESTED.load(Relaxed), TO_GO.load(Relaxed));
    panel.text(
        &crate::lang::trf!("farmer.hud.counts", harvested, to_go),
        TEXT,
    );
    let unloaded = UNLOADED.load(Relaxed);
    if unloaded > 0 {
        panel.text(&crate::lang::trf!("farmer.hud.unloaded", unloaded), NOTE);
    }
    let total = harvested + to_go;
    panel.bar(
        if total == 0 {
            0.0
        } else {
            harvested as f32 / total as f32
        },
        BAR,
    );
}
