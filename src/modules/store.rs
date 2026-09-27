use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::{Mutex, OnceLock};

use super::registry::{COUNT, Id, Kind, Mode, flat_modules, flat_settings};
use super::value::Value;
use crate::gui::keybinds::Bound;

const WORDS: usize = COUNT.div_ceil(64);

pub struct Store {
    enabled: [AtomicU64; WORDS],
    values: Box<[AtomicU64]>,
    texts: Box<[Mutex<String>]>,
    toggles: Box<[AtomicU32]>,
    binds: Box<[AtomicU32]>,
    setting_base: Box<[u32]>,
    kinds: Box<[Kind]>,
    profile: AtomicU8,
    allowed: [[u64; WORDS]; Mode::ALL.len()],
}

static MODULES: OnceLock<Store> = OnceLock::new();

pub fn store() -> &'static Store {
    MODULES.get_or_init(|| {
        let s = Store::from_registry();
        super::persist::load(&s);
        s
    })
}

pub fn save() {
    if let Some(s) = MODULES.get() {
        super::persist::save(s);
    }
}

impl Store {
    pub(super) fn from_registry() -> Store {
        let mut values = Vec::new();
        let mut setting_base = Vec::with_capacity(COUNT + 1);
        let enabled = [const { AtomicU64::new(0) }; WORDS];
        let mut allowed = [[0u64; WORDS]; Mode::ALL.len()];
        let mut toggles = Vec::with_capacity(COUNT);
        for (i, m) in flat_modules().enumerate() {
            setting_base.push(values.len() as u32);
            if m.on {
                enabled[i / 64].fetch_or(1 << (i % 64), Relaxed);
            }
            for (p, mask) in allowed.iter_mut().enumerate() {
                if (m.mode as usize) <= p {
                    mask[i / 64] |= 1 << (i % 64);
                }
            }
            toggles.push(AtomicU32::new((m.on && m.mode <= Mode::Rage) as u32));
            for s in m.settings {
                values.push(AtomicU64::new(Value::default_of(s.kind).pack()));
            }
        }
        setting_base.push(values.len() as u32);

        Store {
            enabled,
            texts: flat_settings()
                .map(|s| {
                    Mutex::new(match s.kind {
                        Kind::Text { default, .. } => default.to_string(),
                        _ => String::new(),
                    })
                })
                .collect(),
            values: values.into_boxed_slice(),
            toggles: toggles.into_boxed_slice(),
            binds: (0..COUNT).map(|_| AtomicU32::new(0)).collect(),
            setting_base: setting_base.into_boxed_slice(),
            kinds: flat_settings().map(|s| s.kind).collect(),
            profile: AtomicU8::new(Mode::Rage as u8),
            allowed,
        }
    }

    pub fn enabled(&self, id: Id) -> bool {
        self.enabled_at(id as usize)
    }

    pub fn enabled_at(&self, i: usize) -> bool {
        (self.enabled[i / 64].load(Relaxed) & self.mask(i / 64)) >> (i % 64) & 1 != 0
    }

    pub fn armed_at(&self, i: usize) -> bool {
        self.enabled[i / 64].load(Relaxed) >> (i % 64) & 1 != 0
    }

    pub fn armed(&self, id: Id) -> bool {
        self.armed_at(id as usize)
    }

    pub fn allowed_at(&self, i: usize) -> bool {
        self.mask(i / 64) >> (i % 64) & 1 != 0
    }

    pub fn set_enabled_at(&self, i: usize, on: bool) {
        if self.armed_at(i) == on {
            return;
        }
        let bit = 1u64 << (i % 64);
        if on {
            self.enabled[i / 64].fetch_or(bit, Relaxed);
        } else {
            self.enabled[i / 64].fetch_and(!bit, Relaxed);
        }
        if self.allowed_at(i) {
            self.toggles[i].fetch_add(1, Relaxed);
        }
    }

    pub fn set_enabled(&self, id: Id, on: bool) {
        self.set_enabled_at(id as usize, on);
    }

    pub fn toggle_at(&self, i: usize) {
        self.set_enabled_at(i, !self.armed_at(i));
    }

    pub fn bind_at(&self, i: usize) -> Bound {
        Bound::from_bits(self.binds[i].load(Relaxed))
    }

    pub fn set_bind_at(&self, i: usize, b: Bound) {
        self.binds[i].store(b.to_bits(), Relaxed);
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

    pub fn toggle_count(&self, i: usize) -> u32 {
        self.toggles[i].load(Relaxed)
    }

    pub fn setting(&self, id: Id, n: usize) -> usize {
        self.setting_base[id as usize] as usize + n
    }

    pub fn raw(&self, index: usize) -> u64 {
        self.values[index].load(Relaxed)
    }

    pub fn set_raw(&self, index: usize, bits: u64) {
        self.values[index].store(bits, Relaxed);
    }

    pub fn value(&self, index: usize) -> Value {
        Value::unpack(self.raw(index), self.kinds[index])
    }

    pub fn kind(&self, index: usize) -> Kind {
        self.kinds[index]
    }

    pub fn set_value(&self, index: usize, v: Value) {
        self.set_raw(index, v.pack());
    }

    pub fn values_len(&self) -> usize {
        self.values.len()
    }

    pub fn settings_of(&self, i: usize) -> std::ops::Range<usize> {
        self.setting_base[i] as usize..self.setting_base[i + 1] as usize
    }

    pub fn num(&self, id: Id, n: usize) -> f32 {
        match self.value(self.setting(id, n)) {
            Value::Num(v) => v,
            _ => 0.0,
        }
    }

    pub fn flag(&self, id: Id, n: usize) -> bool {
        matches!(self.value(self.setting(id, n)), Value::Bool(true))
    }

    pub fn range(&self, id: Id, n: usize) -> (f32, f32) {
        match self.value(self.setting(id, n)) {
            Value::Range(lo, hi) => (lo, hi),
            _ => (0.0, 0.0),
        }
    }

    pub fn choice(&self, id: Id, n: usize) -> u8 {
        match self.value(self.setting(id, n)) {
            Value::Choice(c) => c,
            _ => 0,
        }
    }

    pub fn text_at(&self, index: usize) -> String {
        self.texts[index].lock().unwrap().clone()
    }

    pub fn text(&self, id: Id, n: usize) -> String {
        self.text_at(self.setting(id, n))
    }

    pub fn set_text_at(&self, index: usize, s: &str) {
        *self.texts[index].lock().unwrap() = s.to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::registry;

    #[test]
    fn ids_match_registry() {
        let names: Vec<&str> = flat_modules().map(|m| m.name).collect();
        assert_eq!(names.len(), COUNT, "COUNT disagrees with the tree");
        assert_eq!(names[Id::TriggerBot as usize], "TriggerBot");
        assert_eq!(names[Id::AimAssist as usize], "AimAssist");
        assert_eq!(names[Id::Xray as usize], "Xray");
        assert_eq!(names[Id::OreEsp as usize], "OreEsp");
        assert_eq!(names[Id::StorageEsp as usize], "StorageEsp");
        assert_eq!(names[Id::Nametags as usize], "Nametags");
        assert_eq!(names[Id::Zoom as usize], "Zoom");
        assert_eq!(names[Id::Freecam as usize], "Freecam");
        assert_eq!(names[Id::AutoMine as usize], "AutoMine");
        assert_eq!(names[Id::Flight as usize], "Flight");
        assert_eq!(names[Id::Sneak as usize], "Sneak");
        assert_eq!(names[Id::NoFall as usize], "NoFall");
        assert_eq!(names[Id::AutoSell as usize], "AutoSell");
        assert_eq!(names[Id::Special as usize], "Special");
        for (i, m) in flat_modules().enumerate() {
            let id = Id::ALL[i];
            assert!(
                std::ptr::eq(registry::module(id), m),
                "module({id:?}) walks to {}",
                m.name
            );
        }
    }

    #[test]
    fn each_module_carries_its_own_settings() {
        let at = |id: Id, n: usize| registry::module(id).settings[n].name;
        let bot = registry::trigger_bot::REQUIRE_WEAPON;
        assert_eq!(at(Id::TriggerBot, bot), "Require weapon");
        assert_eq!(
            at(Id::AimAssist, registry::aim_assist::IGNORE_WALLS),
            "Ignore walls"
        );
        assert_eq!(at(Id::Xray, registry::xray::RANGE), "Range");
        assert_eq!(at(Id::Nametags, registry::nametags::DISTANCE), "Distance");
        assert_eq!(
            at(Id::Freecam, registry::freecam::SPRINT_SPEED),
            "Sprint speed"
        );
        assert_eq!(at(Id::Flight, registry::flight::FOR), "Anti kick for");
        assert_eq!(at(Id::Special, registry::special::PHRASE), "Phrase");
        assert_eq!(at(Id::AutoSell, registry::auto_sell::COMMAND), "Command");
    }

    #[test]
    fn toggle_counter_sees_a_round_trip() {
        let s = Store::from_registry();
        let i = Id::TriggerBot as usize;
        let before = s.toggle_count(i);
        let was = s.enabled_at(i);
        s.set_enabled_at(i, !was);
        s.set_enabled_at(i, was);
        assert_eq!(s.enabled_at(i), was);
        assert_eq!(s.toggle_count(i), before + 2);
    }

    #[test]
    fn a_module_that_never_ran_counts_zero() {
        let s = Store::from_registry();
        for i in 0..COUNT {
            assert_eq!(s.enabled_at(i), s.toggle_count(i) > 0, "module {i}");
        }
    }

    #[test]
    fn profile_gates_without_forgetting_the_switch() {
        let s = Store::from_registry();
        let (i, x) = (Id::TriggerBot as usize, Id::Xray as usize);
        s.set_enabled_at(i, true);
        s.set_enabled_at(x, true);
        let before = s.toggle_count(i);

        s.set_profile(Mode::Legit);
        assert!(s.armed_at(i), "the switch is the player's");
        assert!(!s.enabled_at(i), "Legit does not permit a Normal module");
        assert!(s.enabled_at(x), "Xray is Legit and goes on running");
        assert_eq!(s.toggle_count(i), before + 1, "stopping is an edge");
        assert_eq!(s.toggle_count(x), 1, "Xray never stopped");

        s.set_profile(Mode::Normal);
        assert!(s.enabled_at(i));
        assert_eq!(s.toggle_count(i), before + 2, "starting again is an edge");
    }

    #[test]
    fn rage_needs_the_profile() {
        let s = Store::from_registry();
        let f = Id::Flight as usize;
        s.set_enabled_at(f, true);
        assert!(s.armed_at(f));
        assert!(!s.enabled_at(f), "Normal does not permit a Rage module");
        s.set_profile(Mode::Rage);
        assert!(s.enabled_at(f));
    }

    #[test]
    fn a_bind_round_trips_and_starts_unbound() {
        use bevy::prelude::{KeyCode, MouseButton};

        let s = Store::from_registry();
        for i in 0..COUNT {
            assert_eq!(s.bind_at(i), Bound::Unbound, "module {i} ships bound");
        }
        let f = Id::Flight as usize;
        s.set_bind_at(f, Bound::Key(KeyCode::KeyG));
        assert_eq!(s.bind_at(f), Bound::Key(KeyCode::KeyG));
        assert_eq!(
            s.bind_at(Id::Xray as usize),
            Bound::Unbound,
            "one slot each"
        );
        s.set_bind_at(f, Bound::Mouse(MouseButton::Back));
        assert_eq!(s.bind_at(f), Bound::Mouse(MouseButton::Back));
        s.set_bind_at(f, Bound::Unbound);
        assert_eq!(s.bind_at(f), Bound::Unbound);
    }

    #[test]
    fn a_suppressed_module_still_takes_its_switch() {
        let s = Store::from_registry();
        let i = Id::TriggerBot as usize;
        s.set_profile(Mode::Legit);
        s.set_enabled_at(i, true);
        assert!(s.armed_at(i));
        s.set_enabled_at(i, false);
        assert!(!s.armed_at(i));
        assert_eq!(s.toggle_count(i), 0, "neither press started anything");
    }
}
