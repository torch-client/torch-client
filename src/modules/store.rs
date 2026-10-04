use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::{Mutex, OnceLock, PoisonError};

use super::registry::{COUNT, Id, Kind, MODULES, Mode, Options, SETTING_COUNT, SETTINGS, handle};
use super::value::{self, Value};
use crate::gui::keybinds::Bound;

const WORDS: usize = COUNT.div_ceil(64);

pub struct Store {
    enabled: [AtomicU64; WORDS],
    values: [AtomicU64; SETTING_COUNT],
    texts: Box<[(u16, Mutex<String>)]>,
    toggles: [AtomicU32; COUNT],
    binds: [AtomicU32; COUNT],
    profile: AtomicU8,
    allowed: [[u64; WORDS]; Mode::ALL.len()],
}

static MODULES_STORE: OnceLock<Store> = OnceLock::new();

pub fn store() -> &'static Store {
    MODULES_STORE.get_or_init(|| {
        let s = Store::from_registry();
        super::persist::load(&s);
        s
    })
}

pub fn save() {
    if let Some(s) = MODULES_STORE.get() {
        super::persist::save(s);
    }
}

impl Store {
    pub(super) fn from_registry() -> Store {
        let mut allowed = [[0u64; WORDS]; Mode::ALL.len()];
        for (i, m) in MODULES.iter().enumerate() {
            for (p, mask) in allowed.iter_mut().enumerate() {
                if (m.mode as usize) <= p {
                    mask[i / 64] |= 1 << (i % 64);
                }
            }
        }

        Store {
            enabled: [const { AtomicU64::new(0) }; WORDS],
            values: std::array::from_fn(|i| AtomicU64::new(value::default_bits(SETTINGS[i].kind))),
            texts: SETTINGS
                .iter()
                .enumerate()
                .filter_map(|(i, s)| match s.kind {
                    Kind::Text { default, .. } => Some((i as u16, Mutex::new(default.to_string()))),
                    _ => None,
                })
                .collect(),
            toggles: [const { AtomicU32::new(0) }; COUNT],
            binds: [const { AtomicU32::new(0) }; COUNT],
            profile: AtomicU8::new(Mode::Rage as u8),
            allowed,
        }
    }

    pub fn enabled(&self, id: Id) -> bool {
        let i = id as usize;
        (self.enabled[i / 64].load(Relaxed) & self.mask(i / 64)) >> (i % 64) & 1 != 0
    }

    pub fn armed(&self, id: Id) -> bool {
        let i = id as usize;
        self.enabled[i / 64].load(Relaxed) >> (i % 64) & 1 != 0
    }

    pub fn allowed(&self, id: Id) -> bool {
        let i = id as usize;
        self.mask(i / 64) >> (i % 64) & 1 != 0
    }

    pub fn set_enabled(&self, id: Id, on: bool) {
        if self.armed(id) == on {
            return;
        }
        let i = id as usize;
        let bit = 1u64 << (i % 64);
        if on {
            self.enabled[i / 64].fetch_or(bit, Relaxed);
        } else {
            self.enabled[i / 64].fetch_and(!bit, Relaxed);
        }
        if self.allowed(id) {
            self.toggles[i].fetch_add(1, Relaxed);
        }
    }

    pub fn toggle(&self, id: Id) {
        self.set_enabled(id, !self.armed(id));
    }

    pub fn toggle_count(&self, id: Id) -> u32 {
        self.toggles[id as usize].load(Relaxed)
    }

    pub fn bind(&self, id: Id) -> Bound {
        Bound::from_bits(self.binds[id as usize].load(Relaxed))
    }

    pub fn set_bind(&self, id: Id, b: Bound) {
        self.binds[id as usize].store(b.to_bits(), Relaxed);
    }

    fn mask(&self, word: usize) -> u64 {
        self.allowed[self.profile.load(Relaxed) as usize][word]
    }

    pub fn profile(&self) -> Mode {
        Mode::ALL[self.profile.load(Relaxed) as usize]
    }

    pub fn set_profile(&self, m: Mode) {
        let old = self.profile.swap(m as u8, Relaxed) as usize;
        if old == m as usize {
            return;
        }
        let (before, after) = (&self.allowed[old], &self.allowed[m as usize]);
        for w in 0..WORDS {
            let mut changed = self.enabled[w].load(Relaxed) & (before[w] ^ after[w]);
            while changed != 0 {
                self.toggles[w * 64 + changed.trailing_zeros() as usize].fetch_add(1, Relaxed);
                changed &= changed - 1;
            }
        }
    }

    fn bits(&self, slot: u16) -> u64 {
        self.values[slot as usize].load(Relaxed)
    }

    pub fn flag(&self, h: handle::Toggle) -> bool {
        value::flag(self.bits(h.0))
    }

    pub fn num(&self, h: handle::Slider) -> f32 {
        value::num(self.bits(h.0))
    }

    pub fn range(&self, h: handle::Range) -> (f32, f32) {
        value::range(self.bits(h.0))
    }

    pub fn choice<T: Options>(&self, h: handle::Enum<T>) -> T {
        T::from_index(value::choice(self.bits(h.0)))
    }

    pub fn text(&self, h: handle::Text) -> String {
        self.text_at(h.0 as usize)
    }

    pub fn value(&self, slot: usize) -> Option<Value> {
        Value::unpack(self.values[slot].load(Relaxed), SETTINGS[slot].kind)
    }

    pub fn set_value(&self, slot: usize, v: Value) {
        self.values[slot].store(v.pack(), Relaxed);
    }

    fn text_slot(&self, slot: usize) -> &Mutex<String> {
        self.texts
            .iter()
            .find(|(i, _)| *i as usize == slot)
            .map(|(_, m)| m)
            .expect("not a text setting")
    }

    pub fn text_at(&self, slot: usize) -> String {
        self.text_slot(slot)
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn with_text<R>(&self, slot: usize, f: impl FnOnce(&str) -> R) -> R {
        f(&self
            .text_slot(slot)
            .lock()
            .unwrap_or_else(PoisonError::into_inner))
    }

    pub fn set_text_at(&self, slot: usize, s: &str) {
        let mut t = self
            .text_slot(slot)
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        t.clear();
        t.push_str(s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_counter_sees_a_round_trip() {
        let s = Store::from_registry();
        let i = Id::TriggerBot;
        let before = s.toggle_count(i);
        let was = s.enabled(i);
        s.set_enabled(i, !was);
        s.set_enabled(i, was);
        assert_eq!(s.enabled(i), was);
        assert_eq!(s.toggle_count(i), before + 2);
    }

    #[test]
    fn a_module_that_never_ran_counts_zero() {
        let s = Store::from_registry();
        for id in Id::ALL {
            assert_eq!(s.enabled(id), s.toggle_count(id) > 0, "{id:?}");
        }
    }

    #[test]
    fn profile_gates_without_forgetting_the_switch() {
        let s = Store::from_registry();
        let (i, x) = (Id::TriggerBot, Id::Xray);
        s.set_enabled(i, true);
        s.set_enabled(x, true);
        let before = s.toggle_count(i);

        s.set_profile(Mode::Legit);
        assert!(s.armed(i), "the switch is the player's");
        assert!(!s.enabled(i), "Legit does not permit a Normal module");
        assert!(s.enabled(x), "Xray is Legit and goes on running");
        assert_eq!(s.toggle_count(i), before + 1, "stopping is an edge");
        assert_eq!(s.toggle_count(x), 1, "Xray never stopped");

        s.set_profile(Mode::Normal);
        assert!(s.enabled(i));
        assert_eq!(s.toggle_count(i), before + 2, "starting again is an edge");
    }

    #[test]
    fn rage_needs_the_profile() {
        let s = Store::from_registry();
        s.set_profile(Mode::Normal);
        let f = Id::Flight;
        s.set_enabled(f, true);
        assert!(s.armed(f));
        assert!(!s.enabled(f), "Normal does not permit a Rage module");
        s.set_profile(Mode::Rage);
        assert!(s.enabled(f));
    }

    #[test]
    fn a_bind_round_trips_and_starts_unbound() {
        use bevy::prelude::{KeyCode, MouseButton};

        let s = Store::from_registry();
        for id in Id::ALL {
            assert_eq!(s.bind(id), Bound::Unbound, "{id:?} ships bound");
        }
        let f = Id::Flight;
        s.set_bind(f, Bound::Key(KeyCode::KeyG));
        assert_eq!(s.bind(f), Bound::Key(KeyCode::KeyG));
        assert_eq!(s.bind(Id::Xray), Bound::Unbound, "one slot each");
        s.set_bind(f, Bound::Mouse(MouseButton::Back));
        assert_eq!(s.bind(f), Bound::Mouse(MouseButton::Back));
        s.set_bind(f, Bound::Unbound);
        assert_eq!(s.bind(f), Bound::Unbound);
    }

    #[test]
    fn a_suppressed_module_still_takes_its_switch() {
        let s = Store::from_registry();
        let i = Id::TriggerBot;
        s.set_profile(Mode::Legit);
        s.set_enabled(i, true);
        assert!(s.armed(i));
        s.set_enabled(i, false);
        assert!(!s.armed(i));
        assert_eq!(s.toggle_count(i), 0, "neither press started anything");
    }

    #[test]
    fn typed_reads_see_the_defaults() {
        use crate::modules::registry::{auto_sell, flight, nametags};
        let s = Store::from_registry();
        assert!((s.num(flight::SPEED) - 0.05).abs() < 1e-6);
        assert_eq!(
            s.choice(flight::ANTI_KICK),
            crate::modules::flight::AntiKick::Packet
        );
        assert!(s.flag(nametags::GAMEMODE));
        assert_eq!(s.text(auto_sell::COMMAND), "sellall inventory %s");
        assert!(s.with_text(auto_sell::COMMAND.0 as usize, |t| t
            == "sellall inventory %s"));
    }
}
