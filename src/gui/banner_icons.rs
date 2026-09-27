use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::gui::atlas::BANNER_ICON_SLOTS;

#[derive(Default)]
struct Icons {
    slots: HashMap<String, usize>,
    ready: Vec<bool>,
    pending: Vec<(usize, String)>,
    edge: u32,
}

impl Icons {
    fn request(&mut self, key: &str, edge: u32) {
        if self.edge != edge {
            self.edge = edge;
            self.slots.clear();
            self.ready.clear();
            self.pending.clear();
        }

        if self.slots.contains_key(key) || self.slots.len() >= BANNER_ICON_SLOTS {
            return;
        }

        let slot = self.slots.len();
        self.slots.insert(key.to_string(), slot);
        self.ready.push(false);
        self.pending.push((slot, key.to_string()));
    }

    fn slot(&self, key: &str) -> Option<usize> {
        let slot = *self.slots.get(key)?;
        self.ready
            .get(slot)
            .copied()
            .unwrap_or(false)
            .then_some(slot)
    }
}

static ICONS: OnceLock<Mutex<Icons>> = OnceLock::new();

fn icons() -> MutexGuard<'static, Icons> {
    match ICONS.get_or_init(Default::default).lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub fn request(key: &str, edge: u32) {
    icons().request(key, edge);
}

pub fn slot(key: &str) -> Option<usize> {
    icons().slot(key)
}

pub fn take_pending() -> (u32, Vec<(usize, String)>) {
    let mut icons = icons();
    (icons.edge, std::mem::take(&mut icons.pending))
}

pub fn mark_ready(slot: usize) {
    let mut icons = icons();
    if let Some(ready) = icons.ready.get_mut(slot) {
        *ready = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_claims_one_slot_and_answers_once_baked() {
        let mut icons = Icons::default();
        icons.request("red_banner!creeper.15", 16);
        icons.request("red_banner!creeper.15", 16);
        assert_eq!(icons.slots.len(), 1);
        assert_eq!(icons.pending.len(), 1);

        assert_eq!(icons.slot("red_banner!creeper.15"), None);
        icons.ready[0] = true;
        assert_eq!(icons.slot("red_banner!creeper.15"), Some(0));
    }

    #[test]
    fn the_block_stops_rather_than_evicting() {
        let mut icons = Icons::default();
        for i in 0..BANNER_ICON_SLOTS + 4 {
            icons.request(&format!("white_banner!border.{i:02}"), 16);
        }
        assert_eq!(icons.slots.len(), BANNER_ICON_SLOTS);
        assert_eq!(icons.pending.len(), BANNER_ICON_SLOTS);
    }

    #[test]
    fn a_scale_change_rebakes_the_block() {
        let mut icons = Icons::default();
        icons.request("red_banner!creeper.15", 16);
        icons.ready[0] = true;
        assert_eq!(icons.slot("red_banner!creeper.15"), Some(0));

        icons.request("red_banner!creeper.15", 32);
        assert_eq!(icons.slot("red_banner!creeper.15"), None);
        assert_eq!(icons.pending.len(), 1);
    }
}
