use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};

use super::registry::{Id, flight as setting, options};
use super::{Edge, Phase, store};

pub const SINK_STEP: f64 = 0.031_30;

static EDGE: Edge = Edge::new(Id::Flight);

static PHASE: AtomicU32 = AtomicU32::new(0);

static SINK: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
pub enum State {
    Idle,
    On(f32),
    Release,
}

options! {
    pub enum AntiKick {
        Off = "Off",
        Drop = "Drop",
        Packet = "Packet",
    }
}

pub fn poll() -> State {
    let s = store();
    match EDGE.poll(s) {
        Phase::Off => return State::Idle,
        Phase::Stopped => {
            idle();
            return State::Release;
        }
        Phase::Started => idle(),
        Phase::Running => {}
    }

    let speed = s.num(setting::SPEED);
    let mode = s.choice(setting::ANTI_KICK);
    if mode == AntiKick::Off {
        idle();
        return State::On(speed);
    }

    let every = s.num(setting::EVERY).max(1.0) as u32;
    let hold = s.num(setting::FOR).max(1.0) as u32;
    let window = PHASE.fetch_add(1, Relaxed) % (every + hold) >= every;

    SINK.store(window && mode == AntiKick::Packet, Relaxed);
    if window && mode == AntiKick::Drop {
        State::Release
    } else {
        State::On(speed)
    }
}

fn idle() {
    PHASE.store(0, Relaxed);
    SINK.store(false, Relaxed);
}

pub fn forcing() -> bool {
    store().enabled(Id::Flight)
}

pub fn sinking() -> bool {
    SINK.load(Relaxed)
}
