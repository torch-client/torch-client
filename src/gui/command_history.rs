use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use crate::platform::storage;

const MAX_SAVED: usize = 50;

struct History {
    commands: VecDeque<String>,
    dirty: bool,
}

static HISTORY: OnceLock<Mutex<History>> = OnceLock::new();

fn history() -> &'static Mutex<History> {
    HISTORY.get_or_init(|| {
        let mut commands = VecDeque::new();
        if let Some(text) = storage::COMMAND_HISTORY.load() {
            for line in text.lines().filter(|l| !l.is_empty()) {
                if commands.len() >= MAX_SAVED {
                    commands.pop_front();
                }
                commands.push_back(line.to_string());
            }
        }
        Mutex::new(History {
            commands,
            dirty: false,
        })
    })
}

pub fn load() -> Vec<String> {
    history().lock().unwrap().commands.iter().cloned().collect()
}

pub fn record(command: &str) {
    let mut h = history().lock().unwrap();
    if h.commands.back().map(String::as_str) == Some(command) {
        return;
    }
    if h.commands.len() >= MAX_SAVED {
        h.commands.pop_front();
    }
    h.commands.push_back(command.to_string());
    h.dirty = true;
}

pub fn save() {
    let Some(lock) = HISTORY.get() else {
        return;
    };
    let mut h = lock.lock().unwrap();
    if !std::mem::take(&mut h.dirty) {
        return;
    }
    let len = h.commands.iter().map(|c| c.len() + 1).sum();
    let mut out = String::with_capacity(len);
    for command in &h.commands {
        out.push_str(command);
        out.push('\n');
    }
    storage::COMMAND_HISTORY.store(&out);
}
