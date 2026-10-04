use super::registry::{Id, Kind, Options, SETTINGS, handle};
use super::value;
use crate::gui::keybinds::Bound;

pub struct Store;

static STORE: Store = Store;

#[inline(always)]
pub fn store() -> &'static Store {
    &STORE
}

pub fn save() {}

#[inline(always)]
fn bits(slot: u16) -> u64 {
    value::default_bits(SETTINGS[slot as usize].kind)
}

impl Store {
    #[inline(always)]
    pub fn enabled(&self, _: Id) -> bool {
        false
    }

    #[inline(always)]
    pub fn armed(&self, _: Id) -> bool {
        false
    }

    #[inline(always)]
    pub fn set_enabled(&self, _: Id, _: bool) {}

    #[inline(always)]
    pub fn toggle(&self, _: Id) {}

    #[inline(always)]
    pub fn toggle_count(&self, _: Id) -> u32 {
        0
    }

    #[inline(always)]
    pub fn bind(&self, _: Id) -> Bound {
        Bound::Unbound
    }

    #[inline(always)]
    pub fn flag(&self, h: handle::Toggle) -> bool {
        value::flag(bits(h.0))
    }

    #[inline(always)]
    pub fn num(&self, h: handle::Slider) -> f32 {
        value::num(bits(h.0))
    }

    #[inline(always)]
    pub fn range(&self, h: handle::Range) -> (f32, f32) {
        value::range(bits(h.0))
    }

    #[inline(always)]
    pub fn choice<T: Options>(&self, h: handle::Enum<T>) -> T {
        T::from_index(value::choice(bits(h.0)))
    }

    pub fn text(&self, h: handle::Text) -> String {
        match SETTINGS[h.0 as usize].kind {
            Kind::Text { default, .. } => default.to_string(),
            _ => String::new(),
        }
    }
}
