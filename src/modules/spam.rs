use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

use super::registry::{Id, spam as setting};
use super::{Edge, Phase, roll, store};
use crate::session::SharedMutex;

const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

static COOLDOWN: AtomicU32 = AtomicU32::new(0);

static EDGE: Edge = Edge::new(Id::Spam);

pub fn tick(shared: &SharedMutex) {
    let s = store();
    match EDGE.poll(s) {
        Phase::Off | Phase::Stopped => return,
        Phase::Started => COOLDOWN.store(0, Relaxed),
        Phase::Running => {}
    }
    let left = COOLDOWN.load(Relaxed);
    if left > 0 {
        COOLDOWN.store(left - 1, Relaxed);
        return;
    }

    let message = s.text(setting::MESSAGE);
    let message = message.trim();
    if message.is_empty() {
        return;
    }

    COOLDOWN.store(roll_whole(s.range(setting::DELAY)).max(1) as u32, Relaxed);

    let mut line = String::with_capacity(message.len() + 34);
    let command = message.starts_with('/');
    if s.flag(setting::PREFIX) && !command {
        push_run(&mut line, s.range(setting::PREFIX_LENGTH));
        line.push(' ');
    }
    line.push_str(message);
    if s.flag(setting::SUFFIX) {
        line.push(' ');
        push_run(&mut line, s.range(setting::SUFFIX_LENGTH));
    }

    shared.lock().unwrap().session.outgoing_chat.push(line);
}

fn roll_whole((lo, hi): (f32, f32)) -> usize {
    let (lo, hi) = (lo.min(hi).round(), hi.max(lo).round());
    (roll(lo, hi + 1.0) as usize).min(hi as usize)
}

fn push_run(line: &mut String, length: (f32, f32)) {
    for _ in 0..roll_whole(length).max(1) {
        let i = (roll(0.0, CHARSET.len() as f32) as usize).min(CHARSET.len() - 1);
        line.push(CHARSET[i] as char);
    }
}
